//! **THE SAMPLE PACKAGE** -- `asteroids`, until the real checker exists.
//!
//! Two pieces, and since 29-09 they are separate on purpose:
//!
//! ```text
//!    asteroids()     the GRAPH, written by hand. `titan-lector` reads the
//!                    same package from real files (Titan.toml + .titan) and
//!                    its tests prove both graphs are the same
//!    script_for(g)   the checker's EVENTS, tied to node NAMES, not to
//!                    positions: they fit the hand-written graph and the one
//!                    read from ESTRATOS alike -- and any package that has
//!                    these modules
//! ```
//!
//! One event of each kind, so F1 shows every animation before a single
//! `.titan` is checked (that waits for T0-T4). The texts are what the owner
//! reads: Spanish, ASCII, no tilde. The layout is world pixels.

use crate::event::{Diagnostic, Event, EventKind, Mode, Place, Script};
use crate::graph::{Graph, Lang, Node, NodeKind, NodeId, Permission, Permissions};

/// The names `script_for` needs. The special nodes carry these names wherever
/// the graph comes from.
pub const GPU_NAME: &str = "3060";
pub const DIRECTOR_NAME: &str = "DIRECTOR";
/// What the three nodes that are not modules say about themselves.
pub const ROOT_PURPOSE: &str = "Titan.toml -- el nodo principal";
pub const GPU_PURPOSE: &str = "sm_86: corre las `gpu fn`";
pub const DIRECTOR_PURPOSE: &str = "el escritorio: muestra la ventana";

/// The graph and the script of `asteroids`, written by hand.
pub fn asteroids() -> (Graph, Script) {
    let g = asteroids_graph();
    let s = script_for(&g).expect("sample: the script fits its own graph");
    (g, s)
}

/// Only the graph: what F1 shows when there is no ESTRATOS to read. Without
/// building a script to throw away (Ring 3 has 64 KiB of stack).
pub fn asteroids_graph() -> Graph {
    let mut g = Graph::new();
    let n = |g: &mut Graph, node: Node| g.add(node).expect("sample: room for its nodes");

    let root = n(&mut g, Node::new(NodeKind::Root, Lang::None, "asteroids", ROOT_PURPOSE, 380, 20));
    let main = n(&mut g, Node::new(NodeKind::Module, Lang::Titan, "main", "arranca el juego y el bucle del fotograma", 380, 150));
    let ship = n(&mut g, Node::new(NodeKind::Module, Lang::Titan, "ship", "la nave: se mueve y dispara", 60, 300));
    let rock = n(&mut g, Node::new(NodeKind::Module, Lang::Titan, "rock", "las rocas: se parten al chocar", 380, 300));
    let physics = n(&mut g, Node::new(NodeKind::Module, Lang::Titan, "physics", "mueve los cuerpos y resuelve los choques", 700, 300));
    let collide = n(&mut g, Node::new(NodeKind::Module, Lang::Titan, "collide", "quien toca a quien", 700, 460));
    let gpu = n(&mut g, Node::new(NodeKind::Gpu, Lang::None, GPU_NAME, GPU_PURPOSE, 1020, 460));
    let director = n(&mut g, Node::new(NodeKind::Director, Lang::None, DIRECTOR_NAME, DIRECTOR_PURPOSE, 60, 460));

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
    g
}

/// The checker's events for any graph that has the `asteroids` modules.
/// `None` if one is missing -- F1 then shows the graph with no animation,
/// instead of animating a loan to a node that is not there.
pub fn script_for(g: &Graph) -> Option<Script> {
    let id = |name: &str| g.find(name.as_bytes());
    let (main, ship, rock, physics) = (id("main")?, id("ship")?, id("rock")?, id("physics")?);
    let (gpu, director) = (id(GPU_NAME)?, id(DIRECTOR_NAME)?);

    let mut s = Script::new();
    let mut e = |kind: EventKind, value: &str| s.push(Event::new(kind, value)).ok();
    let travel = |mode, from: NodeId, to: NodeId| EventKind::Borrow { mode, from, to };

    // step(mut hero): the ship module changes the hero, and gives it back.
    e(travel(Mode::Mut, main, ship), "hero")?;
    e(EventKind::Return { from: main, to: ship }, "hero")?;
    // fire(take bullet): the bullet belongs to the rocks from now on.
    e(EventKind::Take { from: main, to: rock }, "bullet")?;
    // physics.step(mut world), and inside it the 3060 does the heavy part.
    e(travel(Mode::Mut, main, physics), "world")?;
    e(travel(Mode::Mut, physics, gpu), "buf")?;
    e(EventKind::Return { from: physics, to: gpu }, "buf")?;
    e(EventKind::Return { from: main, to: physics }, "world")?;
    // The surface is lent to the DIRECTOR while it composes the frame.
    e(travel(Mode::Read, main, director), "frame")?;
    e(EventKind::Return { from: main, to: director }, "frame")?;

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
        .ok()?;
    s.push(Event::new(EventKind::Conflict { first, second, diag: clash }, "hero")).ok()?;

    // The permission NO: main sends the score and the manifest said net = false.
    let denied = s
        .diagnose(Diagnostic::new(
            "main usa la red y el paquete no la pidio",
            Place { node: main, line: 41 },
            "Titan.toml dice net = false: el kernel tambien diria NO",
            "quita el envio, o pide net = true en [permissions]",
        ))
        .ok()?;
    s.push(Event::new(EventKind::Denied { node: main, permission: Permission::Net, diag: denied }, "score")).ok()?;

    // Only a script its graph accepts leaves this function.
    s.check(g).ok()?;
    Some(s)
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
    fn a_graph_without_the_modules_gets_no_script_instead_of_a_wrong_one() {
        let mut g = Graph::new();
        g.add(Node::new(NodeKind::Module, Lang::Titan, "main", "", 0, 0)).unwrap();
        assert!(script_for(&g).is_none());
    }

    #[test]
    fn the_script_follows_names_not_positions() {
        // The same package with its nodes added in another order.
        let (orig, _) = asteroids();
        let mut g = Graph::new();
        for n in orig.nodes().iter().rev() {
            g.add(*n).unwrap();
        }
        for e in orig.edges() {
            let (a, b) = (orig.node(e.from).unwrap(), orig.node(e.to).unwrap());
            g.connect(g.find(a.name.as_bytes()).unwrap(), g.find(b.name.as_bytes()).unwrap()).unwrap();
        }
        let s = script_for(&g).expect("same names, same script");
        assert_eq!(s.check(&g), Ok(()));
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
