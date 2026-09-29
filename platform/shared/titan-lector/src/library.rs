//! **THE LIBRARY** -- `titan/biblioteca.toml`: which packages exist and where.
//!
//! ```toml
//!    [packages]
//!    asteroids = "titan/asteroids"
//! ```
//!
//! An index and not a folder listing, on purpose (`PLAN_TALLER` 8.6): today a
//! Ring 3 program can only walk ESTRATOS through the ONE cursor the desktop's
//! F12 panel uses, and walking it from F1 would move what F12 shows.

use crate::text::{is_name, lines, quoted, trim, Path};
use bmo_titan_contrato::{Name, Text};

pub const MAX_PACKAGES: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LibraryError {
    /// Line `n` is not `name = "path"` inside `[packages]`.
    BadLine(usize),
    TooMany,
}

pub struct Library {
    entries: [(Name, Path); MAX_PACKAGES],
    n: usize,
}

impl Library {
    pub fn packages(&self) -> &[(Name, Path)] {
        &self.entries[..self.n]
    }
}

pub fn parse(text: &[u8]) -> Result<Library, LibraryError> {
    let mut lib = Library { entries: [(Text::new(""), Path::EMPTY); MAX_PACKAGES], n: 0 };
    let mut in_packages = false;
    for (n, line) in lines(text) {
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }
        if let Some(head) = line.strip_prefix(b"[").and_then(|l| l.strip_suffix(b"]")) {
            in_packages = trim(head) == b"packages";
            continue;
        }
        if !in_packages {
            continue;
        }
        let eq = line.iter().position(|&c| c == b'=').ok_or(LibraryError::BadLine(n))?;
        let key = trim(&line[..eq]);
        let path = quoted(trim(&line[eq + 1..])).ok_or(LibraryError::BadLine(n))?;
        if !is_name(key) || path.is_empty() {
            return Err(LibraryError::BadLine(n));
        }
        if lib.n >= MAX_PACKAGES {
            return Err(LibraryError::TooMany);
        }
        let name = Text::new(core::str::from_utf8(key).map_err(|_| LibraryError::BadLine(n))?);
        lib.entries[lib.n] = (name, Path::new(&[path]).ok_or(LibraryError::BadLine(n))?);
        lib.n += 1;
    }
    Ok(lib)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_lists_the_packages_in_order() {
        let lib = parse(b"[packages]\nasteroids = \"titan/asteroids\"\ntetris = \"titan/tetris\"\n").unwrap();
        assert_eq!(lib.packages().len(), 2);
        assert_eq!(lib.packages()[1].0.as_bytes(), b"tetris");
        assert_eq!(lib.packages()[1].1.as_bytes(), b"titan/tetris");
    }

    #[test]
    fn a_bad_row_is_refused_with_its_line() {
        assert_eq!(parse(b"[packages]\nAsteroids = \"x\"\n").err(), Some(LibraryError::BadLine(2)));
        assert_eq!(parse(b"[packages]\nasteroids = titan\n").err(), Some(LibraryError::BadLine(2)));
    }
}
