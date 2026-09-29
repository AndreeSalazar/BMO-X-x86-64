//! **THE HEADER OF A MODULE** -- the only part of a `.titan` this reader
//! understands, decided with the owner on 29-09 (`TITAN_MAESTRO` 4.3, U3):
//!
//! ```text
//!    mod physics "mueve los cuerpos y resuelve los choques"   <- FIRST line
//!    use ship, gpu                                            <- what it uses
//!    mod collide                                              <- its children
//!    mod rules in "reglas/juego.titan"                        <- one that says
//!                                                                where it lives
//! ```
//!
//! The first line that is not blank IS the module's name and its one line of
//! purpose; without it the file is refused (U3: a module says what it does).
//! `mod a, b` (no quote) declares children, which live in files of their own,
//! where cargo would put them (`text::default_place`). `mod a in "path"` puts
//! one anywhere in the package: the PARENT says where its child is, so the
//! disk can be in any order and F1 still shows the tree the code declares
//! (the owner, 29-09). `in` is already one of the 25 words: no word is added.
//! Everything else is the body, and the body is the grammar's (T0) -- not this.

use crate::text::{after_word, commas, is_name, is_package_path, lines, quoted, trim};
use bmo_titan_contrato::{Line, Name, Text};

pub const MAX_CHILDREN: usize = 16;
pub const MAX_USES: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeaderError {
    /// The file has no first line of the form `mod name "what it does"`.
    NoHeader,
    /// Line `n` of the header is not understood (a bad name, a quote in the
    /// wrong place, a path that leaves the package...).
    BadLine(usize),
    /// More children or uses than fit.
    TooMany(usize),
}

/// A `mod` of the header. Its path, if it says one, is a range of the text
/// the header was read from: a header costs no path buffers.
#[derive(Clone, Copy)]
pub struct Child {
    pub name: Name,
    at: u16,
    len: u16,
}

impl Child {
    /// The path its `mod ... in "..."` gave, in `text` (the same bytes the
    /// header was parsed from). `None`: it lives in its default place.
    pub fn path<'t>(&self, text: &'t [u8]) -> Option<&'t [u8]> {
        let (a, n) = (self.at as usize, self.len as usize);
        (n > 0).then(|| text.get(a..a + n)).flatten()
    }
}

#[derive(Clone, Copy)]
pub struct Header {
    pub name: Name,
    pub purpose: Line,
    children: [Child; MAX_CHILDREN],
    n_children: usize,
    uses: [Name; MAX_USES],
    n_uses: usize,
}

impl Header {
    pub fn children(&self) -> &[Child] {
        &self.children[..self.n_children]
    }

    pub fn uses(&self) -> &[Name] {
        &self.uses[..self.n_uses]
    }

    fn child(&mut self, name: Name, at: usize, len: usize, n: usize) -> Result<(), HeaderError> {
        if self.n_children >= MAX_CHILDREN {
            return Err(HeaderError::TooMany(n));
        }
        self.children[self.n_children] = Child { name, at: at as u16, len: len as u16 };
        self.n_children += 1;
        Ok(())
    }
}

fn name_of(s: &[u8]) -> Option<Name> {
    is_name(s).then(|| Text::new(core::str::from_utf8(s).unwrap_or("")))
}

/// Where `part` starts inside `whole` (it is a slice of it).
fn offset(whole: &[u8], part: &[u8]) -> usize {
    part.as_ptr() as usize - whole.as_ptr() as usize
}

pub fn parse(text: &[u8]) -> Result<Header, HeaderError> {
    let empty = Text::new("");
    let mut h = Header {
        name: empty,
        purpose: Text::new(""),
        children: [Child { name: empty, at: 0, len: 0 }; MAX_CHILDREN],
        n_children: 0,
        uses: [empty; MAX_USES],
        n_uses: 0,
    };
    if text.len() > u16::MAX as usize {
        // A path is kept as a u16 range; no header is this long.
        return Err(HeaderError::NoHeader);
    }
    let mut first = true;
    for (n, line) in lines(text) {
        if line.is_empty() {
            continue;
        }
        if first {
            first = false;
            let rest = after_word(line, b"mod").ok_or(HeaderError::NoHeader)?;
            let space = rest.iter().position(|c| c.is_ascii_whitespace()).ok_or(HeaderError::NoHeader)?;
            h.name = name_of(&rest[..space]).ok_or(HeaderError::BadLine(n))?;
            let purpose = quoted(trim(&rest[space..])).ok_or(HeaderError::NoHeader)?;
            h.purpose = Text::new(core::str::from_utf8(purpose).map_err(|_| HeaderError::BadLine(n))?);
            continue;
        }
        if let Some(rest) = after_word(line, b"mod") {
            if rest.contains(&b'"') {
                // `mod x in "path"`: one child, and where it lives. Anything
                // else with a quote is a second `mod x "..."`: a file is ONE
                // module.
                let space = rest.iter().position(|c| c.is_ascii_whitespace()).ok_or(HeaderError::BadLine(n))?;
                let name = name_of(&rest[..space]).ok_or(HeaderError::BadLine(n))?;
                let path = after_word(trim(&rest[space..]), b"in").and_then(quoted).ok_or(HeaderError::BadLine(n))?;
                if !is_package_path(path) {
                    return Err(HeaderError::BadLine(n));
                }
                h.child(name, offset(text, path), path.len(), n)?;
            } else {
                for piece in commas(rest) {
                    let name = name_of(piece).ok_or(HeaderError::BadLine(n))?;
                    h.child(name, 0, 0, n)?;
                }
            }
        } else if let Some(rest) = after_word(line, b"use") {
            for piece in commas(rest) {
                let name = name_of(piece).ok_or(HeaderError::BadLine(n))?;
                if h.n_uses >= MAX_USES {
                    return Err(HeaderError::TooMany(n));
                }
                h.uses[h.n_uses] = name;
                h.n_uses += 1;
            }
        }
        // Anything else is the body: not this reader's.
    }
    if first {
        return Err(HeaderError::NoHeader);
    }
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_the_owner_chose() {
        let h = parse(b"mod physics \"mueve los cuerpos\"\nuse ship, gpu\nmod collide\n\nfn body() is ignored\n").unwrap();
        assert_eq!(h.name.as_bytes(), b"physics");
        assert_eq!(h.purpose.as_bytes(), b"mueve los cuerpos");
        assert_eq!(h.uses().len(), 2);
        assert_eq!(h.uses()[1].as_bytes(), b"gpu");
        assert_eq!(h.children()[0].name.as_bytes(), b"collide");
    }

    #[test]
    fn the_parent_says_where_a_child_lives() {
        let t = b"mod main \"x\"\r\nmod ship, rock\r\nmod rules in \"reglas/juego.titan\"\r\n";
        let h = parse(t).unwrap();
        let got: std::vec::Vec<_> = h.children().iter().map(|c| (c.name.as_bytes(), c.path(t))).collect();
        assert_eq!(got, [(&b"ship"[..], None), (b"rock", None), (b"rules", Some(&b"reglas/juego.titan"[..]))]);
    }

    #[test]
    fn a_path_out_of_the_package_or_badly_said_is_refused_with_its_line() {
        assert_eq!(parse(b"mod a \"x\"\nmod b in \"../b.titan\"\n").err(), Some(HeaderError::BadLine(2)));
        assert_eq!(parse(b"mod a \"x\"\nmod b in \"/b.titan\"\n").err(), Some(HeaderError::BadLine(2)));
        assert_eq!(parse(b"mod a \"x\"\nmod b at \"b.titan\"\n").err(), Some(HeaderError::BadLine(2)));
        assert_eq!(parse(b"mod a \"x\"\nmod b, c in \"b.titan\"\n").err(), Some(HeaderError::BadLine(2)));
    }

    #[test]
    fn a_module_that_does_not_say_what_it_does_is_refused() {
        assert_eq!(parse(b"mod ship\n").err(), Some(HeaderError::NoHeader));
        assert_eq!(parse(b"use ship\nmod a \"x\"\n").err(), Some(HeaderError::NoHeader));
        assert_eq!(parse(b"\n\n").err(), Some(HeaderError::NoHeader));
    }

    #[test]
    fn a_bad_name_is_refused_with_its_line() {
        assert_eq!(parse(b"mod ship \"x\"\nuse Rock\n").err(), Some(HeaderError::BadLine(2)));
        assert_eq!(parse(b"mod ship \"x\"\nmod other \"y\"\n").err(), Some(HeaderError::BadLine(2)));
    }
}
