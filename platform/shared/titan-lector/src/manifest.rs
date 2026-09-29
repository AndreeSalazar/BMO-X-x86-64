//! **THE MANIFEST** -- the part of `Titan.toml` F1 needs, and nothing more.
//!
//! ```toml
//!    [package]        name = "asteroids"      (version, edition: read past)
//!    [permissions]    screen = true           true or a "string" = asked for
//!                     net = false             false = not asked for
//!    [layout]         main = [380, 150]       where each node sits (the owner:
//!                                             the positions live in the MAIN node)
//! ```
//!
//! A subset of TOML, and valid TOML: an unknown section or key is read past
//! (a manifest can say more than F1 uses), but a line this reader cannot
//! understand is refused with its number, never guessed.

use crate::text::{is_name, lines, number, quoted, trim};
use bmo_titan_contrato::{Name, Permission, Permissions, Text, MAX_NODES};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ManifestError {
    /// `[package]` has no `name`, or it is not a name of the language.
    NoName,
    /// Line `n` is not `[section]`, `key = value` or a `#` comment.
    BadLine(usize),
    /// More `[layout]` rows than nodes a graph can have.
    TooManyPositions,
}

pub struct Manifest {
    pub name: Name,
    pub permissions: Permissions,
    layout: [(Name, i32, i32); MAX_NODES],
    n_layout: usize,
}

impl Manifest {
    /// Where `[layout]` puts the node with this name.
    pub fn position(&self, name: &[u8]) -> Option<(i32, i32)> {
        self.layout[..self.n_layout].iter().find(|(n, ..)| n.as_bytes() == name).map(|&(_, x, y)| (x, y))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Package,
    Permissions,
    Layout,
    Other,
}

pub fn parse(text: &[u8]) -> Result<Manifest, ManifestError> {
    let mut m = Manifest {
        name: Text::new(""),
        permissions: Permissions::NONE,
        layout: [(Text::new(""), 0, 0); MAX_NODES],
        n_layout: 0,
    };
    let mut section = Section::Other;
    for (n, line) in lines(text) {
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }
        if let Some(head) = line.strip_prefix(b"[").and_then(|l| l.strip_suffix(b"]")) {
            section = match trim(head) {
                b"package" => Section::Package,
                b"permissions" => Section::Permissions,
                b"layout" => Section::Layout,
                _ => Section::Other,
            };
            continue;
        }
        let eq = line.iter().position(|&c| c == b'=').ok_or(ManifestError::BadLine(n))?;
        let (key, value) = (trim(&line[..eq]), trim(&line[eq + 1..]));
        match section {
            Section::Package if key == b"name" => {
                let name = quoted(value).filter(|v| is_name(v)).ok_or(ManifestError::NoName)?;
                m.name = Text::new(core::str::from_utf8(name).map_err(|_| ManifestError::NoName)?);
            }
            Section::Permissions => {
                let asked = match value {
                    b"true" => true,
                    b"false" => false,
                    v if quoted(v).is_some() => true,
                    _ => return Err(ManifestError::BadLine(n)),
                };
                // An unknown permission is read past: it is not F1's to judge.
                if let Some(p) = Permission::ALL.into_iter().find(|p| p.key().as_bytes() == key) {
                    if asked {
                        m.permissions = m.permissions.with(p);
                    }
                }
            }
            Section::Layout => {
                let pair = value.strip_prefix(b"[").and_then(|v| v.strip_suffix(b"]")).ok_or(ManifestError::BadLine(n))?;
                let mut it = pair.split(|&c| c == b',').map(trim);
                let (x, y) = match (it.next().and_then(number), it.next().and_then(number), it.next()) {
                    (Some(x), Some(y), None) => (x, y),
                    _ => return Err(ManifestError::BadLine(n)),
                };
                if m.n_layout >= MAX_NODES {
                    return Err(ManifestError::TooManyPositions);
                }
                let key = core::str::from_utf8(key).map_err(|_| ManifestError::BadLine(n))?;
                m.layout[m.n_layout] = (Text::new(key), x, y);
                m.n_layout += 1;
            }
            _ => {}
        }
    }
    if m.name.is_empty() {
        return Err(ManifestError::NoName);
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOML: &[u8] = b"[package]\nname = \"asteroids\"\nversion = \"0.1.0\"\n\n\
        [permissions]\nscreen = true\ngpu = \"compute\"\nnet = false\n\n\
        [layout]\nmain = [380, 150]\n# a comment\n";

    #[test]
    fn it_reads_the_name_the_permissions_and_the_layout() {
        let m = parse(TOML).unwrap();
        assert_eq!(m.name.as_bytes(), b"asteroids");
        assert!(m.permissions.allows(Permission::Screen) && m.permissions.allows(Permission::Gpu));
        assert!(!m.permissions.allows(Permission::Net) && !m.permissions.allows(Permission::Disk));
        assert_eq!(m.position(b"main"), Some((380, 150)));
        assert_eq!(m.position(b"ship"), None);
    }

    #[test]
    fn a_line_it_cannot_understand_is_refused_with_its_number() {
        assert_eq!(parse(b"[package]\nname = \"a\"\nwhat is this\n").err(), Some(ManifestError::BadLine(3)));
        assert_eq!(parse(b"[layout]\nmain = [1]\n").err(), Some(ManifestError::BadLine(2)));
    }

    #[test]
    fn a_package_without_a_proper_name_is_refused() {
        assert_eq!(parse(b"[package]\nversion = \"1\"\n").err(), Some(ManifestError::NoName));
        assert_eq!(parse(b"[package]\nname = \"Big Name\"\n").err(), Some(ManifestError::NoName));
    }
}
