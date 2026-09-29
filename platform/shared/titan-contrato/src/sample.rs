//! **THE SAMPLE PACKAGE** -- `asteroids`, written by hand until the real front
//! exists (T0 waits for the owner's grammar).
//!
//! It is the package of `TITAN_MAESTRO` section 4 (`asteroids/`, `ship`,
//! `rock`, `physics`, `collide`) plus the two special nodes, and a script with
//! ONE event of each kind, so F1 can show every animation in the Ryzen before a
//! single `.titan` is compiled. The same function feeds F1 and the tests: what
//! the tests accept is exactly what F1 draws.
//!
//! The texts are what the owner reads: Spanish, ASCII, no tilde. The layout is
//! world pixels, dependencies drawn top to bottom.

use crate::event::{Diagnostic, Event, EventKind, Mode, Place, Script};
use crate::graph::{Graph, Lang, Node, NodeKind, Permission, Permissions};

/// The graph and the script of `asteroids`. Panics only if this file is
/// inconsistent -- which `tests::the_sample_is_consistent` rules out.
pub fn asteroids() -> (Graph, Script) {
    let mut g = Graph::new();
    let n = |g: &mut Graph, node: Node| g.add(node).expect("sample: room for its nodes");

    let root = n(&mut g, Node::new(NodeKind::Root, Lang::None, "asteroids", "Titan.toml -- el nodo principal", 380, 20));
    let main = n(&mut g, Node::new(NodeKind::Module, Lang::Titan, "main", "arranca el juego y el bucle del fotograma", 380, 150));
    let ship = n(&mut g, Node::new(NodeKind::Module, Lang::Titan, "ship", "la nave: se mueve y dispara", 60, 300));
    let rock = n(&mut g, Node::new(NodeKind::Module, Lang::Titan, "rock", "las rocas: se parten al chocar", 380, 300));
    let physics = n(&mut g, Node::new(NodeKind::Module, Lang::Titan, "physics", "mueve los cuerpos y resuelve los choques", 700, 300));
    let collide = n(&mut g, Node::new(NodeKind::Module, Lang::Titan, "collide", "quien toca a quien", 700, 460));
    let gpu = n(&mut g, Node::new(NodeKind::Gpu, Lang::None, "3060", "sm_86: corre las `gpu fn`", 1020, 460));
    let director = n(&mut g, Node::new(NodeKind::Director, Lang::None, "DIRECTOR", "el escritorio: muestra la ventana", 60, 460));

    g.permissions = Permissions::NONE
        .with(Permission::Screen)
        .with(Permission::Input)
        .with(Permission::Sound)
        .with(Permission::Gpu);

    for (from, to) in [
        (root, main),
        (main, ship),
        (main, rock),
        (main, physics),
        (main, director),
        (physics, ship),
        (physics, collide),
        (physics, gpu),
    ] {
        g.connect(from, to).expect("sample: its uses go down");
    }

    let mut s = Script::new();
    let e = |s: &mut Script, kind: EventKind, value: &str| s.push(Event::new(kind, value)).expect("sample: room for its events");

    // step(mut hero): the ship module changes the hero, and gives it back.
    e(&mut s, EventKind::Borrow { mode: Mode::Mut, from: main, to: ship }, "hero");
    e(&mut s, EventKind::Return { from: main, to: ship }, "hero");
    // fire(take bullet): the bullet belongs to the rocks from now on.
    e(&mut s, EventKind::Take { from: main, to: rock }, "bullet");
    // physics.step(mut world), and inside it the 3060 does the heavy part.
    e(&mut s, EventKind::Borrow { mode: Mode::Mut, from: main, to: physics }, "world");
    e(&mut s, EventKind::Borrow { mode: Mode::Mut, from: physics, to: gpu }, "buf");
    e(&mut s, EventKind::Return { from: physics, to: gpu }, "buf");
    e(&mut s, EventKind::Return { from: main, to: physics }, "world");
    // The surface is lent to the DIRECTOR while it composes the frame.
    e(&mut s, EventKind::Borrow { mode: Mode::Read, from: main, to: director }, "frame");
    e(&mut s, EventKind::Return { from: main, to: director }, "frame");

    // The ERROR: step(mut hero, hero.map) -- changed and read at once.
    let first = Place { node: ship, line: 12 };
    let second = Place { node: physics, line: 30 };
    let clash = s
        .diagnose(Diagnostic::new(
            "`hero` se CAMBIA y se LEE a la vez",
            first,
            "step(mut hero, hero.map): mientras se cambia, nadie mas lo lee",
            "copia el mapa antes: let m = hero.map; step(mut hero, m)",
        ))
        .expect("sample: room for its diagnostics");
    e(&mut s, EventKind::Conflict { first, second, diag: clash }, "hero");

    // The permission NO: main sends the score and the manifest said net = false.
    let denied = s
        .diagnose(Diagnostic::new(
            "main usa la red y el paquete no la pidio",
            Place { node: main, line: 41 },
            "Titan.toml dice net = false: el kernel tambien diria NO",
            "quita el envio, o pide net = true en [permissions]",
        ))
        .expect("sample: room for its diagnostics");
    e(&mut s, EventKind::Denied { node: main, permission: Permission::Net, diag: denied }, "score");

    (g, s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeKind;

    #[test]
    fn the_sample_is_consistent() {
        let (g, s) = asteroids();
        assert_eq!(s.check(&g), Ok(()));
    }

    #[test]
    fn the_sample_shows_every_kind_of_event() {
        let (_, s) = asteroids();
        let has = |f: fn(&EventKind) -> bool| s.events().iter().any(|e| f(&e.kind));
        assert!(has(|k| matches!(k, EventKind::Borrow { mode: Mode::Mut, .. })));
        assert!(has(|k| matches!(k, EventKind::Return { .. })));
        assert!(has(|k| matches!(k, EventKind::Take { .. })));
        assert!(has(|k| matches!(k, EventKind::Conflict { .. })));
        assert!(has(|k| matches!(k, EventKind::Denied { .. })));
    }

    #[test]
    fn everything_is_a_node_including_the_manifest_the_3060_and_the_director() {
        let (g, _) = asteroids();
        for kind in [NodeKind::Root, NodeKind::Module, NodeKind::Gpu, NodeKind::Director] {
            assert!(g.nodes().iter().any(|n| n.kind == kind), "{kind:?}");
        }
        assert_eq!(g.root().map(|r| r.0), Some(0));
    }

    #[test]
    fn the_sample_asks_for_no_network_so_the_denied_event_is_honest() {
        let (g, _) = asteroids();
        assert!(!g.permissions.allows(Permission::Net));
    }

    #[test]
    fn the_texts_fit_without_being_cut() {
        let (g, s) = asteroids();
        for n in g.nodes() {
            assert!(n.purpose.len() < 72, "{:?}", core::str::from_utf8(n.purpose.as_bytes()));
        }
        for d in s.diagnostics() {
            for t in [&d.what, &d.why, &d.fix] {
                assert!(t.len() < 72, "{:?}", core::str::from_utf8(t.as_bytes()));
            }
        }
    }
}
