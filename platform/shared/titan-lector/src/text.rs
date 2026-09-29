//! **THE SMALL TEXT TOOLS** -- lines, words and quotes over bytes, no allocator.
//!
//! Everything here is ASCII. A byte that is not printable ASCII is not
//! "interpreted": the callers refuse it with the line number.

/// The lines of a text, without their `\r\n` or `\n`, and with the 1-based
/// line number that a message will print.
pub fn lines(text: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
    text.split(|&c| c == b'\n').enumerate().map(|(i, l)| (i + 1, trim(l)))
}

pub fn trim(s: &[u8]) -> &[u8] {
    let start = s.iter().position(|c| !c.is_ascii_whitespace()).unwrap_or(s.len());
    let end = s.iter().rposition(|c| !c.is_ascii_whitespace()).map(|i| i + 1).unwrap_or(start);
    &s[start..end.max(start)]
}

/// `line` starts with the word `word` followed by a space: the rest, trimmed.
pub fn after_word<'a>(line: &'a [u8], word: &[u8]) -> Option<&'a [u8]> {
    let rest = line.strip_prefix(word)?;
    match rest.first() {
        Some(c) if c.is_ascii_whitespace() => Some(trim(rest)),
        _ => None,
    }
}

/// A name of the language: lowercase letters, digits and `_`, starting with a
/// letter. The same rule for modules, packages and layout keys.
pub fn is_name(s: &[u8]) -> bool {
    matches!(s.first(), Some(c) if c.is_ascii_lowercase())
        && s.iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

/// A double-quoted string that is the WHOLE of `s`: its contents.
pub fn quoted(s: &[u8]) -> Option<&[u8]> {
    let inner = s.strip_prefix(b"\"")?.strip_suffix(b"\"")?;
    (!inner.contains(&b'"')).then_some(inner)
}

/// Splits `a, b, c` into its trimmed pieces.
pub fn commas(s: &[u8]) -> impl Iterator<Item = &[u8]> {
    s.split(|&c| c == b',').map(trim)
}

/// A signed decimal number.
pub fn number(s: &[u8]) -> Option<i32> {
    let (neg, digits) = match s.strip_prefix(b"-") {
        Some(d) => (true, d),
        None => (false, s),
    };
    if digits.is_empty() || digits.len() > 9 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let v = digits.iter().fold(0i32, |acc, &c| acc * 10 + (c - b'0') as i32);
    Some(if neg { -v } else { v })
}

/// A path in a fixed buffer: `titan/asteroids/src/physics/collide.titan` fits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Path {
    bytes: [u8; 128],
    len: u8,
}

impl Path {
    pub const EMPTY: Path = Path { bytes: [0; 128], len: 0 };

    pub fn new(parts: &[&[u8]]) -> Option<Path> {
        let mut p = Path::EMPTY;
        for part in parts {
            p.push(part)?;
        }
        Some(p)
    }

    /// Appends bytes as they are (the caller puts the `/`). `None` if it
    /// would not fit: a cut path is another file.
    pub fn push(&mut self, part: &[u8]) -> Option<()> {
        let (n, m) = (self.len as usize, part.len());
        if n + m > self.bytes.len() {
            return None;
        }
        self.bytes[n..n + m].copy_from_slice(part);
        self.len = (n + m) as u8;
        Some(())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_trimmed_and_numbered() {
        let t = b"  mod a \"x\"\r\n\nuse b\n";
        let v: std::vec::Vec<_> = lines(t).collect();
        assert_eq!(v[0], (1, &b"mod a \"x\""[..]));
        assert_eq!(v[1], (2, &b""[..]));
        assert_eq!(v[2], (3, &b"use b"[..]));
    }

    #[test]
    fn a_word_needs_its_space() {
        assert_eq!(after_word(b"use ship, rock", b"use"), Some(&b"ship, rock"[..]));
        assert_eq!(after_word(b"useful", b"use"), None);
    }

    #[test]
    fn names_are_lowercase_identifiers() {
        assert!(is_name(b"collide") && is_name(b"step_2"));
        assert!(!is_name(b"Ship") && !is_name(b"2d") && !is_name(b"") && !is_name(b"a-b"));
    }

    #[test]
    fn quotes_and_numbers() {
        assert_eq!(quoted(b"\"hola\""), Some(&b"hola"[..]));
        assert_eq!(quoted(b"\"a\"b\""), None);
        assert_eq!(number(b"-40"), Some(-40));
        assert_eq!(number(b"4x"), None);
    }

    #[test]
    fn a_path_that_does_not_fit_is_refused_not_cut() {
        let long = [b'a'; 200];
        assert!(Path::new(&[&long]).is_none());
        assert_eq!(Path::new(&[b"titan/", b"asteroids"]).unwrap().as_bytes(), b"titan/asteroids");
    }
}
