//! **HANGING A FILE UNDER ANOTHER** -- what the EXPLORER does when a file is
//! dragged onto another (`PLAN_TALLER` 8.7): the new parent says `mod` of it,
//! the old one stops saying it. Two headers change; the file itself does NOT
//! move, not one byte of it.
//!
//! ```text
//!    drag collide.titan onto main.titan
//!
//!    src/main.titan      + mod collide in "src/physics/collide.titan"
//!    src/physics.titan   - mod collide
//!    src/physics/collide.titan            (untouched: it stays where it is)
//! ```
//!
//! The path is written only when the file is NOT where cargo would look for
//! it under its new parent (`text::default_place`): hang it back under
//! physics and the line is a plain `mod collide` again.
//!
//! Two steps, so a check first: `plan` touches no disk and says NO before a
//! byte is written -- the EXPLORER colours the drop target with it while the
//! button is still down. Then `hang` writes the NEW parent first: if the second
//! write fails, the child is declared twice (a problem that is said) instead
//! of by nobody (a file gone from view).

use crate::edit::{add_child, remove_child, EditError};
use crate::package::{Fetch, Loaded, Say, Source};
use crate::text::{default_place, Path};
use bmo_titan_contrato::{Line, Name, NodeId, NodeKind};

/// Where the writes go. F1 writes ESTRATOS; the tests, a table.
pub trait Sink {
    /// Creates the file or publishes a new version of it. `false`: not saved.
    fn store(&mut self, path: &[u8], bytes: &[u8]) -> bool;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HangError {
    /// Only a module moves; `main` and `Titan.toml` are where a package starts.
    NotMovable,
    /// Only a module takes children (not `Titan.toml`, the 3060, the DIRECTOR).
    NotAParent,
    /// Onto itself.
    Same,
    /// It already hangs there.
    AlreadyThere,
    /// The new parent already depends on the child: the `mod` closes a cycle.
    WouldCycle,
    /// A header could not be read, or does not fit.
    Read,
    Edit(EditError),
    /// ESTRATOS did not save it.
    Write,
}

/// A hang that passed the check: who, under whom, from whom, and the path
/// to write (`None`: the file is where cargo would look).
#[derive(Clone, Copy)]
pub struct Plan {
    pub child: NodeId,
    pub parent: NodeId,
    pub old: NodeId,
    name: Name,
    path: Option<Path>,
}

fn is_module(l: &Loaded, id: NodeId) -> bool {
    l.graph.node(id).map(|n| n.kind) == Some(NodeKind::Module)
}

/// The check, without touching the disk.
pub fn plan(l: &Loaded, child: NodeId, parent: NodeId) -> Result<Plan, HangError> {
    let c = l.file_of(child).filter(|f| f.depth >= 2 && is_module(l, child)).ok_or(HangError::NotMovable)?;
    if child == parent {
        return Err(HangError::Same);
    }
    let p = l.file_of(parent).filter(|_| is_module(l, parent)).ok_or(HangError::NotAParent)?;
    if c.parent == parent {
        return Err(HangError::AlreadyThere);
    }
    if l.graph.reaches(child, parent) {
        return Err(HangError::WouldCycle);
    }
    let name = l.graph.node(child).map(|n| n.name).ok_or(HangError::NotMovable)?;
    let path = match default_place(p.path.as_bytes(), name.as_bytes()) {
        Some(home) if home == c.path => None,
        _ => Some(c.path),
    };
    Ok(Plan { child, parent, old: c.parent, name, path })
}

/// The writes: the new parent's header, then the old one's. `text` and `out`
/// are the two buffers a header is read into and rewritten into.
pub fn hang<S: Source + Sink>(io: &mut S, root: &[u8], l: &Loaded, p: &Plan, text: &mut [u8], out: &mut [u8]) -> Result<(), HangError> {
    let name = p.name.as_bytes();
    let path = p.path.as_ref().map(|x| x.as_bytes());
    rewrite(io, root, l, p.parent, text, out, |t, o| add_child(t, name, path, o))?;
    rewrite(io, root, l, p.old, text, out, |t, o| remove_child(t, name, o))
}

fn rewrite<S: Source + Sink>(
    io: &mut S,
    root: &[u8],
    l: &Loaded,
    node: NodeId,
    text: &mut [u8],
    out: &mut [u8],
    edit: impl FnOnce(&[u8], &mut [u8]) -> Result<usize, EditError>,
) -> Result<(), HangError> {
    let f = l.file_of(node).ok_or(HangError::Read)?;
    let full = Path::new(&[root, b"/", f.path.as_bytes()]).ok_or(HangError::Read)?;
    let n = match io.fetch(full.as_bytes(), text) {
        Fetch::Found(n) => n.min(text.len()),
        Fetch::Missing | Fetch::TooBig => return Err(HangError::Read),
    };
    let m = edit(&text[..n], out).map_err(HangError::Edit)?;
    if io.store(full.as_bytes(), &out[..m]) {
        Ok(())
    } else {
        Err(HangError::Write)
    }
}

/// What the owner reads after a drop. Spanish, ASCII, no tilde. `None` for a
/// drop that asked for nothing (onto itself).
pub fn note(r: Result<(), HangError>, child: &[u8], parent: &[u8]) -> Option<Line> {
    let s = Say::new();
    let s = match r {
        Ok(()) => s.t(child).t(b" cuelga ahora de ").t(parent).t(b"; su fichero no se movio"),
        Err(HangError::Same) => return None,
        Err(HangError::NotMovable) => s.t(child).t(b" no se cuelga: main y Titan.toml son la raiz"),
        Err(HangError::NotAParent) => s.t(b"solo se cuelga de un modulo .titan"),
        Err(HangError::AlreadyThere) => s.t(child).t(b" ya cuelga de ").t(parent),
        // The child already reaches the parent (a `use`, or a `mod` further
        // down): a `mod` the other way closes the loop.
        Err(HangError::WouldCycle) => s.t(child).t(b" ya depende de ").t(parent).t(b": seria un ciclo"),
        Err(HangError::Read) => s.t(b"no pude leer una cabecera en ESTRATOS"),
        Err(HangError::Edit(EditError::NotDeclared)) => s.t(b"el padre de ").t(child).t(b" no declara su mod"),
        Err(HangError::Edit(EditError::DoesNotFit)) => s.t(b"la cabecera nueva no cabe en el buffer"),
        Err(HangError::Edit(EditError::NoHeader)) => s.t(b"un padre no empieza por mod x \"que hace\""),
        Err(HangError::Write) => s.t(b"ESTRATOS no guardo el cambio"),
    };
    Some(s.done())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::tests::Table;
    use crate::package::read_package;
    use crate::seed;
    use std::string::String;
    use std::vec::Vec;

    /// The seed as a disk that can be written: owned texts.
    struct Disk {
        files: Vec<(String, Vec<u8>)>,
        refuse_writes_after: usize,
    }

    impl Disk {
        fn seed() -> Disk {
            let files = seed::FILES.iter().map(|(p, t)| (String::from(*p), t.as_bytes().to_vec())).collect();
            Disk { files, refuse_writes_after: usize::MAX }
        }

        fn get(&self, path: &str) -> &str {
            let f = self.files.iter().find(|(p, _)| p == path).expect(path);
            core::str::from_utf8(&f.1).unwrap()
        }

        fn read(&mut self) -> Loaded {
            let mut buf = [0u8; 4096];
            read_package(self, b"titan/asteroids", &mut buf)
        }
    }

    impl Source for Disk {
        fn fetch(&mut self, path: &[u8], buf: &mut [u8]) -> Fetch {
            let t: Vec<(&str, &str)> = self.files.iter().map(|(p, t)| (p.as_str(), core::str::from_utf8(t).unwrap())).collect();
            Table { files: t }.fetch(path, buf)
        }
    }

    impl Sink for Disk {
        fn store(&mut self, path: &[u8], bytes: &[u8]) -> bool {
            if self.refuse_writes_after == 0 {
                return false;
            }
            self.refuse_writes_after -= 1;
            let path = String::from_utf8(path.to_vec()).unwrap();
            self.files.retain(|(p, _)| *p != path);
            self.files.push((path, bytes.to_vec()));
            true
        }
    }

    fn id(l: &Loaded, name: &str) -> NodeId {
        l.graph.find(name.as_bytes()).unwrap()
    }

    fn drop_on(d: &mut Disk, child: &str, parent: &str) -> Result<(), HangError> {
        let l = d.read();
        let p = plan(&l, id(&l, child), id(&l, parent))?;
        let (mut a, mut b) = ([0u8; 4096], [0u8; 4096]);
        hang(d, b"titan/asteroids", &l, &p, &mut a, &mut b)
    }

    #[test]
    fn hanging_rewrites_two_headers_and_moves_no_file() {
        let mut d = Disk::seed();
        let collide_before = String::from(d.get("titan/asteroids/src/physics/collide.titan"));
        drop_on(&mut d, "collide", "main").unwrap();
        // Under the last header line, before the body (which goes as it was).
        assert!(d.get("titan/asteroids/src/main.titan").contains("mod ship, rock, physics\nmod collide in \"src/physics/collide.titan\"\n\nfn main()"));
        assert!(!d.get("titan/asteroids/src/physics.titan").contains("mod collide"));
        assert_eq!(d.get("titan/asteroids/src/physics/collide.titan"), collide_before);
        let l = d.read();
        assert!(l.problems().is_empty(), "{:?}", l.problems());
        let c = l.file_of(id(&l, "collide")).unwrap();
        assert_eq!((c.parent, c.depth), (id(&l, "main"), 2));
    }

    #[test]
    fn hanging_it_back_home_writes_a_plain_mod_again() {
        let mut d = Disk::seed();
        drop_on(&mut d, "collide", "main").unwrap();
        drop_on(&mut d, "collide", "physics").unwrap();
        assert_eq!(d.get("titan/asteroids/src/physics.titan"), seed::FILES[4].1);
        assert_eq!(d.get("titan/asteroids/src/main.titan"), seed::FILES[1].1);
    }

    #[test]
    fn the_check_says_no_before_a_byte_is_written() {
        let d = &mut Disk::seed();
        let l = d.read();
        let e = |c: &str, p: &str| plan(&l, id(&l, c), id(&l, p)).err();
        assert_eq!(e("main", "ship"), Some(HangError::NotMovable));
        assert_eq!(e("ship", "ship"), Some(HangError::Same));
        assert_eq!(e("ship", "main"), Some(HangError::AlreadyThere));
        // physics uses ship: hanging physics under ship closes a cycle.
        assert_eq!(e("physics", "ship"), Some(HangError::WouldCycle));
        assert_eq!(e("ship", "asteroids"), Some(HangError::NotAParent));
        assert_eq!(e("ship", "3060"), Some(HangError::NotAParent));
        assert!(e("rock", "ship").is_none());
    }

    #[test]
    fn a_second_write_that_fails_leaves_it_declared_twice_not_lost() {
        let mut d = Disk::seed();
        d.refuse_writes_after = 1;
        assert_eq!(drop_on(&mut d, "rock", "ship"), Err(HangError::Write));
        let l = d.read();
        assert!(l.graph.find(b"rock").is_some());
        assert!(l.problems().iter().any(|p| p.describe().as_bytes() == b"hay dos nodos rock"));
    }

    #[test]
    fn every_answer_is_said_in_one_line() {
        let r = [Ok(()), Err(HangError::WouldCycle), Err(HangError::Edit(EditError::NotDeclared)), Err(HangError::Write)];
        for x in r {
            let line = note(x, b"collide", b"physics").unwrap();
            assert!(!line.is_empty() && line.len() < 72, "{:?}", line.as_bytes());
        }
        assert!(note(Err(HangError::Same), b"a", b"a").is_none());
        assert_eq!(note(Ok(()), b"collide", b"main").unwrap().as_bytes(), b"collide cuelga ahora de main; su fichero no se movio");
        // physics uses ship: it is physics that already depends on ship.
        assert_eq!(note(Err(HangError::WouldCycle), b"physics", b"ship").unwrap().as_bytes(), b"physics ya depende de ship: seria un ciclo");
    }
}
