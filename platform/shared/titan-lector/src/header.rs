//! **THE HEADER OF A MODULE** -- the only part of a `.titan` this reader
//! understands, decided with the owner on 29-09 (`TITAN_MAESTRO` 4.3, U3):
//!
//! ```text
//!    mod physics "mueve los cuerpos y resuelve los choques"   <- FIRST line
//!    use ship, gpu                                            <- what it uses
//!    mod collide                                              <- its children
//! ```
//!
//! The first line that is not blank IS the module's name and its one line of
//! purpose; without it the file is refused (U3: a module says what it does).
//! `mod a, b` (no quote) declares children, which live in files of their own.
//! Everything else is the body, and the body is the grammar's (T0) -- not this.

use crate::text::{after_word, commas, is_name, lines, quoted};
use bmo_titan_contrato::{Line, Name, Text};

pub const MAX_CHILDREN: usize = 16;
pub const MAX_USES: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeaderError {
    /// The file has no first line of the form `mod name "what it does"`.
    NoHeader,
    /// Line `n` of the header is not understood (a bad name, a quote in the
    /// wrong place...).
    BadLine(usize),
    /// More children or uses than fit.
    TooMany(usize),
}

#[derive(Clone, Copy)]
pub struct Header {
    pub name: Name,
    pub purpose: Line,
    children: [Name; MAX_CHILDREN],
    n_children: usize,
    uses: [Name; MAX_USES],
    n_uses: usize,
}

impl Header {
    pub fn children(&self) -> &[Name] {
        &self.children[..self.n_children]
    }

    pub fn uses(&self) -> &[Name] {
        &self.uses[..self.n_uses]
    }
}

fn name_of(s: &[u8]) -> Option<Name> {
    is_name(s).then(|| Text::new(core::str::from_utf8(s).unwrap_or("")))
}

/// Adds the names of `a, b, c` to `list`.
fn collect(list: &mut [Name], count: &mut usize, rest: &[u8], n: usize) -> Result<(), HeaderError> {
    for piece in commas(rest) {
        let name = name_of(piece).ok_or(HeaderError::BadLine(n))?;
        if *count >= list.len() {
            return Err(HeaderError::TooMany(n));
        }
        list[*count] = name;
        *count += 1;
    }
    Ok(())
}

pub fn parse(text: &[u8]) -> Result<Header, HeaderError> {
    let empty = Text::new("");
    let mut h = Header {
        name: empty,
        purpose: Text::new(""),
        children: [empty; MAX_CHILDREN],
        n_children: 0,
        uses: [empty; MAX_USES],
        n_uses: 0,
    };
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
            let purpose = quoted(crate::text::trim(&rest[space..])).ok_or(HeaderError::NoHeader)?;
            h.purpose = Text::new(core::str::from_utf8(purpose).map_err(|_| HeaderError::BadLine(n))?);
            continue;
        }
        if let Some(rest) = after_word(line, b"mod") {
            if rest.contains(&b'"') {
                // A second `mod x "..."`: a file is ONE module.
                return Err(HeaderError::BadLine(n));
            }
            collect(&mut h.children, &mut h.n_children, rest, n)?;
        } else if let Some(rest) = after_word(line, b"use") {
            collect(&mut h.uses, &mut h.n_uses, rest, n)?;
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
        assert_eq!(h.children()[0].as_bytes(), b"collide");
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
