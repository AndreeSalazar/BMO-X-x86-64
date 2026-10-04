//! **EDITING A HEADER** -- adding or taking out ONE `mod` line, and nothing
//! else: every other byte of the file comes out as it went in (its body, its
//! comments, its `\r\n`). What F1 writes when a file is hung under another
//! (`hang.rs`).
//!
//! ```text
//!    add_child     mod main "..."          mod main "..."
//!                  use director     ->     use director
//!                  mod ship, rock          mod ship, rock
//!                                          mod collide in "src/physics/collide.titan"
//!
//!    remove_child  mod ship, rock, physics  ->  mod ship, physics
//!                  mod rules in "r.titan"   ->  (the line goes)
//! ```
//!
//! The new line goes after the LAST line of the header that already says
//! `mod` or `use`, so the header stays together at the top.

use crate::text::{after_word, commas, trim};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditError {
    /// The file does not start with `mod name "..."`.
    NoHeader,
    /// No `mod` line names that child.
    NotDeclared,
    /// The result does not fit in the output buffer.
    DoesNotFit,
}

/// A line of the text with where it is: `start..end` is the line without its
/// `\n`, `next` is where the next one starts.
#[derive(Clone, Copy)]
struct Raw {
    start: usize,
    end: usize,
    next: usize,
}

fn raw_lines(text: &[u8]) -> impl Iterator<Item = Raw> + '_ {
    let mut at = 0;
    core::iter::from_fn(move || {
        if at >= text.len() {
            return None;
        }
        let start = at;
        let end = text[at..].iter().position(|&c| c == b'\n').map(|i| at + i).unwrap_or(text.len());
        at = end + 1;
        Some(Raw { start, end, next: at.min(text.len()) })
    })
}

/// The header's lines, from its first one: `(line, trimmed text, is first)`.
fn header_lines(text: &[u8]) -> impl Iterator<Item = (Raw, &[u8], bool)> + '_ {
    let mut seen = false;
    raw_lines(text).filter_map(move |r| {
        let t = trim(&text[r.start..r.end]);
        if t.is_empty() {
            return None;
        }
        let first = !seen;
        seen = true;
        Some((r, t, first))
    })
}

/// A writer into a fixed buffer.
struct Out<'a> {
    buf: &'a mut [u8],
    n: usize,
}

impl Out<'_> {
    fn put(&mut self, s: &[u8]) -> Result<(), EditError> {
        let end = self.n + s.len();
        self.buf.get_mut(self.n..end).ok_or(EditError::DoesNotFit)?.copy_from_slice(s);
        self.n = end;
        Ok(())
    }
}

/// `\r\n` if the file already uses it, so a new line looks like the others.
fn newline(text: &[u8]) -> &'static [u8] {
    if text.windows(2).any(|w| w == b"\r\n") {
        b"\r\n"
    } else {
        b"\n"
    }
}

/// Adds `mod child` -- or `mod child in "path"` -- to the header of `text`,
/// into `out`. The length written.
pub fn add_child(text: &[u8], child: &[u8], path: Option<&[u8]>, out: &mut [u8]) -> Result<usize, EditError> {
    let mut after = None;
    for (r, t, first) in header_lines(text) {
        if first && after_word(t, b"mod").is_none() {
            return Err(EditError::NoHeader);
        }
        if first || after_word(t, b"mod").is_some() || after_word(t, b"use").is_some() {
            after = Some(r);
        }
    }
    let r = after.ok_or(EditError::NoHeader)?;
    let nl = newline(text);
    let mut o = Out { buf: out, n: 0 };
    o.put(&text[..r.next])?;
    if r.next == r.end {
        // The last header line is also the last of the file, without `\n`.
        o.put(nl)?;
    }
    o.put(b"mod ")?;
    o.put(child)?;
    if let Some(p) = path {
        o.put(b" in \"")?;
        o.put(p)?;
        o.put(b"\"")?;
    }
    o.put(nl)?;
    o.put(&text[r.next..])?;
    Ok(o.n)
}

/// Takes `child` out of the header's `mod` lines, into `out`. A line that
/// only declared it goes; a list keeps the others, in their order.
pub fn remove_child(text: &[u8], child: &[u8], out: &mut [u8]) -> Result<usize, EditError> {
    for (r, t, first) in header_lines(text) {
        if first {
            if after_word(t, b"mod").is_none() {
                return Err(EditError::NoHeader);
            }
            continue;
        }
        let Some(rest) = after_word(t, b"mod") else { continue };
        let mut o = Out { buf: out, n: 0 };
        if rest.contains(&b'"') {
            // `mod x in "path"`: the whole line is that one child.
            let name = rest.split(|c| c.is_ascii_whitespace()).next().unwrap_or(b"");
            if name != child {
                continue;
            }
            o.put(&text[..r.start])?;
            o.put(&text[r.next..])?;
            return Ok(o.n);
        }
        if !commas(rest).any(|n| n == child) {
            continue;
        }
        o.put(&text[..r.start])?;
        let mut others = commas(rest).filter(|&n| n != child).peekable();
        if others.peek().is_some() {
            // Keep the line's indentation and its own ending.
            let line = &text[r.start..r.end];
            let indent = line.iter().position(|c| !c.is_ascii_whitespace()).unwrap_or(0);
            o.put(&line[..indent])?;
            o.put(b"mod ")?;
            for (i, n) in others.enumerate() {
                if i > 0 {
                    o.put(b", ")?;
                }
                o.put(n)?;
            }
            let ending = if line.ends_with(b"\r") { &b"\r"[..] } else { b"" };
            o.put(ending)?;
            o.put(&text[r.end..r.next])?;
        }
        o.put(&text[r.next..])?;
        return Ok(o.n);
    }
    Err(EditError::NotDeclared)
}

/// Adds `name` to the header's `use`: to the end of the first `use` line if
/// there is one (`use ship` -> `use ship, gpu`), or as a new `use name` line
/// right under the module's first line. What a cable drawn in F1 writes
/// (`wire.rs`).
pub fn add_use(text: &[u8], name: &[u8], out: &mut [u8]) -> Result<usize, EditError> {
    let mut first = None;
    for (r, t, is_first) in header_lines(text) {
        if is_first {
            if after_word(t, b"mod").is_none() {
                return Err(EditError::NoHeader);
            }
            first = Some(r);
            continue;
        }
        if let Some(rest) = after_word(t, b"use") {
            if commas(rest).any(|n| n == name) {
                // Already there: the text is the truth, nothing to write.
                let mut o = Out { buf: out, n: 0 };
                o.put(text)?;
                return Ok(o.n);
            }
            // The end of this line, before its `\r` if it has one.
            let line = &text[r.start..r.end];
            let end = r.start + line.len() - if line.ends_with(b"\r") { 1 } else { 0 };
            let mut o = Out { buf: out, n: 0 };
            o.put(&text[..end])?;
            o.put(b", ")?;
            o.put(name)?;
            o.put(&text[end..])?;
            return Ok(o.n);
        }
        if after_word(t, b"mod").is_none() {
            break;
        }
    }
    let r = first.ok_or(EditError::NoHeader)?;
    let nl = newline(text);
    let mut o = Out { buf: out, n: 0 };
    o.put(&text[..r.next])?;
    if r.next == r.end {
        o.put(nl)?;
    }
    o.put(b"use ")?;
    o.put(name)?;
    o.put(nl)?;
    o.put(&text[r.next..])?;
    Ok(o.n)
}

/// The module's OWN name, in its first line: `mod rock "..."` -> `mod roca
/// "..."`, into `out`. What it does, its `use`, its children and its body go
/// out as they came in (the EXPLORER renames a file and its module together:
/// a `.titan` is called like its module, or the reader says it does not match).
pub fn rename_module(text: &[u8], new: &[u8], out: &mut [u8]) -> Result<usize, EditError> {
    let (r, t, _) = header_lines(text).next().ok_or(EditError::NoHeader)?;
    let rest = after_word(t, b"mod").ok_or(EditError::NoHeader)?;
    let old = rest.split(|c| c.is_ascii_whitespace() || *c == b'"').next().unwrap_or(b"");
    if old.is_empty() {
        return Err(EditError::NoHeader);
    }
    // Where the old name sits inside the line, in bytes of the whole text.
    let line = &text[r.start..r.end];
    let word = line.windows(3).position(|w| w == b"mod").ok_or(EditError::NoHeader)?;
    let at = r.start + word + 3 + line[word + 3..].iter().position(|c| !c.is_ascii_whitespace()).ok_or(EditError::NoHeader)?;
    let mut o = Out { buf: out, n: 0 };
    o.put(&text[..at])?;
    o.put(new)?;
    o.put(&text[at + old.len()..])?;
    Ok(o.n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header;

    #[test]
    fn a_use_joins_the_use_line_or_opens_one_under_the_first_line() {
        let mut out = [0u8; 256];
        let n = add_use(b"mod p \"x\"\nuse ship\nmod collide\n\nfn main()\n", b"gpu", &mut out).unwrap();
        assert_eq!(&out[..n], b"mod p \"x\"\nuse ship, gpu\nmod collide\n\nfn main()\n");
        let n = add_use(b"mod p \"x\"\nmod collide\n", b"ship", &mut out).unwrap();
        assert_eq!(&out[..n], b"mod p \"x\"\nuse ship\nmod collide\n");
        let n = add_use(b"mod p \"x\"\r\nuse a\r\n", b"b", &mut out).unwrap();
        assert_eq!(&out[..n], b"mod p \"x\"\r\nuse a, b\r\n");
        // Already there: the same bytes.
        let n = add_use(b"mod p \"x\"\nuse a\n", b"a", &mut out).unwrap();
        assert_eq!(&out[..n], b"mod p \"x\"\nuse a\n");
        let h = header::parse(b"mod p \"x\"\nuse ship, gpu\nmod collide\n").unwrap();
        assert_eq!(h.uses().len(), 2);
    }

    #[test]
    fn renaming_a_module_touches_only_its_own_name() {
        let t = "\nmod rock \"una roca\"\nuse ship\nmod tiny\n\nfn main() {}\n";
        let mut out = [0u8; 256];
        let n = rename_module(t.as_bytes(), b"roca", &mut out).unwrap();
        assert_eq!(&out[..n], "\nmod roca \"una roca\"\nuse ship\nmod tiny\n\nfn main() {}\n".as_bytes());
        assert_eq!(header::parse(&out[..n]).unwrap().name.as_bytes(), b"roca");
        assert_eq!(rename_module(b"use x\n", b"y", &mut out).err(), Some(EditError::NoHeader));
    }

    fn add(text: &str, child: &str, path: Option<&str>) -> std::string::String {
        let mut out = [0u8; 512];
        let n = add_child(text.as_bytes(), child.as_bytes(), path.map(str::as_bytes), &mut out).unwrap();
        std::string::String::from_utf8(out[..n].to_vec()).unwrap()
    }

    fn remove(text: &str, child: &str) -> Result<std::string::String, EditError> {
        let mut out = [0u8; 512];
        let n = remove_child(text.as_bytes(), child.as_bytes(), &mut out)?;
        Ok(std::string::String::from_utf8(out[..n].to_vec()).unwrap())
    }

    #[test]
    fn a_child_goes_under_the_last_header_line_and_the_body_is_untouched() {
        let t = "mod main \"m\"\nuse director\nmod ship, rock\n\nfn main() {}\n";
        assert_eq!(add(t, "collide", Some("src/physics/collide.titan")), "mod main \"m\"\nuse director\nmod ship, rock\nmod collide in \"src/physics/collide.titan\"\n\nfn main() {}\n");
        assert_eq!(add("mod ship \"s\"", "wing", None), "mod ship \"s\"\nmod wing\n");
    }

    #[test]
    fn crlf_files_get_crlf_lines() {
        assert_eq!(add("mod a \"x\"\r\nuse b\r\n", "c", None), "mod a \"x\"\r\nuse b\r\nmod c\r\n");
        assert_eq!(remove("mod a \"x\"\r\nmod b, c\r\nbody\r\n", "b").unwrap(), "mod a \"x\"\r\nmod c\r\nbody\r\n");
    }

    #[test]
    fn taking_one_out_of_a_list_keeps_the_others_in_order() {
        let t = "mod main \"m\"\n  mod ship, rock, physics\n";
        assert_eq!(remove(t, "rock").unwrap(), "mod main \"m\"\n  mod ship, physics\n");
        assert_eq!(remove("mod main \"m\"\nmod rock\nuse x\n", "rock").unwrap(), "mod main \"m\"\nuse x\n");
        assert_eq!(remove("mod m \"m\"\nmod r in \"a/r.titan\"\n", "r").unwrap(), "mod m \"m\"\n");
    }

    #[test]
    fn what_is_not_there_is_said_not_invented() {
        assert_eq!(remove("mod main \"m\"\nmod ship\n", "rock").err(), Some(EditError::NotDeclared));
        // The module's own first line is not one of its children.
        assert_eq!(remove("mod rock \"r\"\n", "rock").err(), Some(EditError::NotDeclared));
        assert_eq!(remove("use x\n", "x").err(), Some(EditError::NoHeader));
        let mut small = [0u8; 8];
        assert_eq!(add_child(b"mod a \"x\"\n", b"b", None, &mut small).err(), Some(EditError::DoesNotFit));
    }

    #[test]
    fn what_it_writes_the_reader_reads_back() {
        let t = add("mod p \"p\"\nuse ship\nmod collide\n", "rules", Some("r/rules.titan"));
        let h = header::parse(t.as_bytes()).unwrap();
        let names: std::vec::Vec<_> = h.children().iter().map(|c| c.name.as_bytes()).collect();
        assert_eq!(names, [&b"collide"[..], b"rules"]);
        assert_eq!(h.children()[1].path(t.as_bytes()), Some(&b"r/rules.titan"[..]));
        let back = remove(&t, "rules").unwrap();
        assert_eq!(back, "mod p \"p\"\nuse ship\nmod collide\n");
    }
}
