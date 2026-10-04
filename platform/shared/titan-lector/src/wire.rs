//! **A CABLE DRAWN IS A `use` WRITTEN** -- escalon 10 of `PLAN_TALLER` 8.4:
//! in F1 a cable is pulled from a node's OUT pin to another node (the Unreal
//! Engine 5 way of joining nodes), and what that writes is ONE `use` in the
//! header of the first. The text stays the truth: the cable on screen is the
//! `use` in the file, and the next beat reads it back.
//!
//! ```text
//!    physics --(drag)--> ship     src/physics.titan:  use ship
//! ```
//!
//! ** And it is refused exactly where the compiler would refuse it (U3: the
//! dependencies only go DOWN): a cable that closes a cycle is red BEFORE it
//! is let go, with the reason, and nothing is written.

use crate::edit::{self, EditError};
use crate::hang::Sink;
use crate::package::{Fetch, Loaded, Say, Source};
use crate::text::Path;
use bmo_titan_contrato::{Line, NodeId, NodeKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireError {
    /// Onto itself.
    Same,
    /// Only a module (a `.titan`) has a header to write the `use` in.
    NotAModule,
    /// Toward the package (Titan.toml) or a node that is not a module, the
    /// 3060 or the DIRECTOR: there is nothing to `use` there.
    NotUsable,
    /// There is already a cable from one to the other.
    Already,
    /// The other one already depends on this one: a `use` back closes a cycle
    /// (and the compiler would refuse it).
    WouldCycle,
    Read,
    Edit(EditError),
    Write,
}

/// Could a cable go from `from` to `to`? Touches no disk: F1 asks it while
/// the cable is still being pulled, to paint it green or red.
pub fn plan(l: &Loaded, from: NodeId, to: NodeId) -> Result<(), WireError> {
    if from == to {
        return Err(WireError::Same);
    }
    let kind = |id: NodeId| l.graph.node(id).map(|n| n.kind);
    if kind(from) != Some(NodeKind::Module) || l.file_of(from).is_none() {
        return Err(WireError::NotAModule);
    }
    if !matches!(kind(to), Some(NodeKind::Module | NodeKind::Gpu | NodeKind::Director)) {
        return Err(WireError::NotUsable);
    }
    if l.graph.edge_between(from, to) {
        return Err(WireError::Already);
    }
    if l.graph.reaches(to, from) {
        return Err(WireError::WouldCycle);
    }
    Ok(())
}

/// Writes the `use`: the header of `from`, read and saved with one more name.
pub fn wire<S: Source + Sink>(io: &mut S, root: &[u8], l: &Loaded, from: NodeId, to: NodeId, text: &mut [u8], out: &mut [u8]) -> Result<(), WireError> {
    plan(l, from, to)?;
    let file = l.file_of(from).ok_or(WireError::NotAModule)?.path;
    let name = l.graph.node(to).ok_or(WireError::NotUsable)?.name;
    let full = Path::new(&[root, b"/", file.as_bytes()]).ok_or(WireError::Read)?;
    let n = match io.fetch(full.as_bytes(), text) {
        Fetch::Found(n) => n,
        _ => return Err(WireError::Read),
    };
    let k = edit::add_use(&text[..n], name.as_bytes(), out).map_err(WireError::Edit)?;
    if io.store(full.as_bytes(), &out[..k]) {
        Ok(())
    } else {
        Err(WireError::Write)
    }
}

/// What the owner reads: one line. Spanish, ASCII, no tilde.
pub fn note(r: Result<(), WireError>, from: &[u8], to: &[u8]) -> Option<Line> {
    let s = Say::new();
    let s = match r {
        Ok(()) => s.t(from).t(b" usa ahora ").t(to).t(b"; vuelve 1 en F12 lo deshace"),
        Err(WireError::Same) => return None,
        Err(WireError::NotAModule) => s.t(b"solo un modulo .titan tira cables: tiene cabecera donde escribir el use"),
        Err(WireError::NotUsable) => s.t(b"ahi no hay nada que usar: un modulo, la 3060 o el sistema"),
        Err(WireError::Already) => s.t(from).t(b" ya usa ").t(to),
        Err(WireError::WouldCycle) => s.t(to).t(b" ya depende de ").t(from).t(b": seria un ciclo"),
        Err(WireError::Read) => s.t(b"no pude leer la cabecera en ESTRATOS"),
        Err(WireError::Edit(_)) => s.t(b"la cabecera no se dejo reescribir"),
        Err(WireError::Write) => s.t(b"ESTRATOS no guardo el use"),
    };
    Some(s.done())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::read_package;
    use std::string::String;
    use std::vec::Vec;

    struct Disk(Vec<(String, Vec<u8>)>);

    impl Source for Disk {
        fn fetch(&mut self, path: &[u8], buf: &mut [u8]) -> Fetch {
            match self.0.iter().find(|(p, _)| p.as_bytes() == path) {
                Some((_, b)) => {
                    buf[..b.len()].copy_from_slice(b);
                    Fetch::Found(b.len())
                }
                None => Fetch::Missing,
            }
        }
    }

    impl Sink for Disk {
        fn store(&mut self, path: &[u8], bytes: &[u8]) -> bool {
            let path = String::from_utf8(path.to_vec()).unwrap();
            self.0.retain(|(p, _)| *p != path);
            self.0.push((path, bytes.to_vec()));
            true
        }
    }

    fn seed() -> Disk {
        Disk(crate::seed::FILES.iter().map(|(p, t)| (String::from(*p), t.as_bytes().to_vec())).collect())
    }

    fn id(l: &Loaded, n: &str) -> NodeId {
        l.graph.find(n.as_bytes()).unwrap()
    }

    #[test]
    fn a_cable_down_writes_a_use_and_reads_back_as_an_edge() {
        let mut d = seed();
        let mut buf = [0u8; 4096];
        let l = read_package(&mut d, b"titan/asteroids", &mut buf);
        let (rock, ship) = (id(&l, "rock"), id(&l, "ship"));
        assert_eq!(plan(&l, rock, ship), Ok(()));
        let (mut a, mut b) = ([0u8; 4096], [0u8; 4096]);
        wire(&mut d, b"titan/asteroids", &l, rock, ship, &mut a, &mut b).unwrap();
        let l2 = read_package(&mut d, b"titan/asteroids", &mut buf);
        assert!(l2.problems().is_empty());
        assert!(l2.graph.edge_between(id(&l2, "rock"), id(&l2, "ship")));
    }

    #[test]
    fn a_cable_up_is_refused_before_it_is_let_go() {
        let mut d = seed();
        let mut buf = [0u8; 4096];
        let l = read_package(&mut d, b"titan/asteroids", &mut buf);
        // physics already uses ship: ship -> physics would close the loop.
        assert_eq!(plan(&l, id(&l, "ship"), id(&l, "physics")), Err(WireError::WouldCycle));
        assert_eq!(plan(&l, id(&l, "physics"), id(&l, "ship")), Err(WireError::Already));
        assert_eq!(plan(&l, id(&l, "ship"), id(&l, "ship")), Err(WireError::Same));
        let gpu = NodeId(l.graph.nodes().iter().position(|n| n.kind == NodeKind::Gpu).unwrap() as u8);
        assert_eq!(plan(&l, gpu, id(&l, "ship")), Err(WireError::NotAModule));
        assert_eq!(plan(&l, id(&l, "rock"), gpu), Ok(()));
        assert_eq!(plan(&l, id(&l, "ship"), id(&l, "asteroids")), Err(WireError::NotUsable));
        assert_eq!(note(Err(WireError::WouldCycle), b"ship", b"physics").unwrap().as_bytes(), b"physics ya depende de ship: seria un ciclo");
    }
}
