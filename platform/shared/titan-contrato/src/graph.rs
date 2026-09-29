//! **THE GRAPH** -- a TITAN++ package as nodes and `use` edges (U3).
//!
//! The compiler already demands that dependencies only go DOWN; this shape
//! demands it too, at insert time: `connect` refuses an edge that would close
//! a cycle, with the same verdict the compiler gives. So F1 cannot draw a
//! graph the compiler would reject.
//!
//! Fixed capacity (`MAX_NODES`, `MAX_EDGES`): no allocator, and a package that
//! outgrows it says `Full` instead of growing in silence.

use crate::{Line, Name, Text};

pub const MAX_NODES: usize = 32;
pub const MAX_EDGES: usize = 64;

/// What a node IS. Everything in F1 is a node (the owner, 29-09).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeKind {
    /// The manifest (`Titan.toml`): the package, its permissions and the
    /// positions of every other node.
    Root,
    /// A module: one `.titan` file.
    Module,
    /// Another package this one depends on (by path, with its hash).
    Dependency,
    /// The RTX 3060: loans to the GPU travel to this node (U1).
    Gpu,
    /// The DIRECTOR: loans between programs travel to this node (U1).
    Director,
}

/// The language written in the node's header.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Titan,
    Inti,
    C,
    Rust,
    /// Special nodes (the 3060, the DIRECTOR) have no source.
    None,
}

impl Lang {
    pub const fn label(self) -> &'static str {
        match self {
            Lang::Titan => "TITAN",
            Lang::Inti => "INTI",
            Lang::C => "C",
            Lang::Rust => "RUST",
            Lang::None => "",
        }
    }
}

/// An index into `Graph::nodes`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeId(pub u8);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Node {
    pub kind: NodeKind,
    pub lang: Lang,
    pub name: Name,
    /// The one line that says what the module does (U3).
    pub purpose: Line,
    /// Position in the canvas, in world pixels (the `[layout]` of the root).
    pub x: i32,
    pub y: i32,
}

impl Node {
    pub const EMPTY: Node = Node {
        kind: NodeKind::Module,
        lang: Lang::None,
        name: Text::new(""),
        purpose: Text::new(""),
        x: 0,
        y: 0,
    };

    pub const fn new(kind: NodeKind, lang: Lang, name: &str, purpose: &str, x: i32, y: i32) -> Node {
        Node { kind, lang, name: Text::new(name), purpose: Text::new(purpose), x, y }
    }
}

/// `from` uses `to`: the dependency points DOWN.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
}

/// What a package may touch (the `[permissions]` of `Titan.toml`, U2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Permission {
    Screen,
    Input,
    Sound,
    Gpu,
    Disk,
    Net,
}

impl Permission {
    pub const ALL: [Permission; 6] =
        [Permission::Screen, Permission::Input, Permission::Sound, Permission::Gpu, Permission::Disk, Permission::Net];

    /// The key in `Titan.toml`, which is also what F1 prints on the badge.
    pub const fn key(self) -> &'static str {
        match self {
            Permission::Screen => "screen",
            Permission::Input => "input",
            Permission::Sound => "sound",
            Permission::Gpu => "gpu",
            Permission::Disk => "disk",
            Permission::Net => "net",
        }
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// A set of permissions, one bit each.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Permissions(u8);

impl Permissions {
    pub const NONE: Permissions = Permissions(0);

    pub const fn with(self, p: Permission) -> Permissions {
        Permissions(self.0 | p.bit())
    }

    pub const fn allows(self, p: Permission) -> bool {
        self.0 & p.bit() != 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GraphError {
    /// No room left: `MAX_NODES` or `MAX_EDGES`.
    Full,
    NoSuchNode(NodeId),
    /// A module that uses itself.
    SelfUse(NodeId),
    /// The same `use` twice.
    Duplicate(NodeId, NodeId),
    /// `from -> to` would close a cycle: `to` already reaches `from`.
    Cycle { from: NodeId, to: NodeId },
}

pub struct Graph {
    nodes: [Node; MAX_NODES],
    n_nodes: u8,
    edges: [Edge; MAX_EDGES],
    n_edges: u8,
    /// What the package asked for in its manifest.
    pub permissions: Permissions,
}

impl Default for Graph {
    fn default() -> Self {
        Graph::new()
    }
}

impl Graph {
    pub const fn new() -> Graph {
        Graph {
            nodes: [Node::EMPTY; MAX_NODES],
            n_nodes: 0,
            edges: [Edge { from: NodeId(0), to: NodeId(0) }; MAX_EDGES],
            n_edges: 0,
            permissions: Permissions::NONE,
        }
    }

    pub fn add(&mut self, node: Node) -> Result<NodeId, GraphError> {
        let n = self.n_nodes as usize;
        if n >= MAX_NODES {
            return Err(GraphError::Full);
        }
        self.nodes[n] = node;
        self.n_nodes += 1;
        Ok(NodeId(n as u8))
    }

    /// `from` uses `to`. Refused if it would close a cycle, so the graph is
    /// always one the compiler accepts.
    pub fn connect(&mut self, from: NodeId, to: NodeId) -> Result<(), GraphError> {
        self.node(from).ok_or(GraphError::NoSuchNode(from))?;
        self.node(to).ok_or(GraphError::NoSuchNode(to))?;
        if from == to {
            return Err(GraphError::SelfUse(from));
        }
        if self.edge_between(from, to) {
            return Err(GraphError::Duplicate(from, to));
        }
        if self.reaches(to, from) {
            return Err(GraphError::Cycle { from, to });
        }
        let n = self.n_edges as usize;
        if n >= MAX_EDGES {
            return Err(GraphError::Full);
        }
        self.edges[n] = Edge { from, to };
        self.n_edges += 1;
        Ok(())
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes[..self.n_nodes as usize]
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges[..self.n_edges as usize]
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes().get(id.0 as usize)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        let n = self.n_nodes as usize;
        self.nodes[..n].get_mut(id.0 as usize)
    }

    pub fn edge_between(&self, from: NodeId, to: NodeId) -> bool {
        self.edges().iter().any(|e| e.from == from && e.to == to)
    }

    /// Is there a path `a -> ... -> b` following `use` edges?
    pub fn reaches(&self, a: NodeId, b: NodeId) -> bool {
        if a == b {
            return true;
        }
        // MAX_NODES = 32: the visited set fits in one word.
        let mut visited: u32 = 1 << a.0;
        let mut stack = [0u8; MAX_NODES];
        let mut top = 1;
        stack[0] = a.0;
        while top > 0 {
            top -= 1;
            let cur = stack[top];
            for e in self.edges() {
                if e.from.0 != cur {
                    continue;
                }
                if e.to == b {
                    return true;
                }
                let bit = 1u32 << e.to.0;
                if visited & bit == 0 {
                    visited |= bit;
                    stack[top] = e.to.0;
                    top += 1;
                }
            }
        }
        false
    }

    /// The manifest node, if the package has one.
    pub fn root(&self) -> Option<NodeId> {
        self.nodes().iter().position(|n| n.kind == NodeKind::Root).map(|i| NodeId(i as u8))
    }
}

const _: () = assert!(MAX_NODES <= 32, "reaches() keeps the visited set in a u32");
const _: () = assert!(MAX_EDGES <= 255 && MAX_NODES <= 255, "counts are u8");

#[cfg(test)]
mod tests {
    use super::*;

    fn module(name: &str) -> Node {
        Node::new(NodeKind::Module, Lang::Titan, name, "", 0, 0)
    }

    #[test]
    fn a_cycle_is_refused_with_the_edge_that_would_close_it() {
        let mut g = Graph::new();
        let a = g.add(module("a")).unwrap();
        let b = g.add(module("b")).unwrap();
        let c = g.add(module("c")).unwrap();
        g.connect(a, b).unwrap();
        g.connect(b, c).unwrap();
        assert_eq!(g.connect(c, a), Err(GraphError::Cycle { from: c, to: a }));
        assert_eq!(g.edges().len(), 2, "the refused edge is not stored");
    }

    #[test]
    fn self_use_and_duplicates_are_refused() {
        let mut g = Graph::new();
        let a = g.add(module("a")).unwrap();
        let b = g.add(module("b")).unwrap();
        assert_eq!(g.connect(a, a), Err(GraphError::SelfUse(a)));
        g.connect(a, b).unwrap();
        assert_eq!(g.connect(a, b), Err(GraphError::Duplicate(a, b)));
    }

    #[test]
    fn unknown_nodes_are_named() {
        let mut g = Graph::new();
        let a = g.add(module("a")).unwrap();
        assert_eq!(g.connect(a, NodeId(9)), Err(GraphError::NoSuchNode(NodeId(9))));
    }

    #[test]
    fn a_diamond_is_not_a_cycle() {
        let mut g = Graph::new();
        let top = g.add(module("top")).unwrap();
        let l = g.add(module("l")).unwrap();
        let r = g.add(module("r")).unwrap();
        let bottom = g.add(module("bottom")).unwrap();
        g.connect(top, l).unwrap();
        g.connect(top, r).unwrap();
        g.connect(l, bottom).unwrap();
        assert_eq!(g.connect(r, bottom), Ok(()));
        assert!(g.reaches(top, bottom));
        assert!(!g.reaches(bottom, top));
    }

    #[test]
    fn a_full_graph_says_full() {
        let mut g = Graph::new();
        for _ in 0..MAX_NODES {
            g.add(module("m")).unwrap();
        }
        assert_eq!(g.add(module("one-more")), Err(GraphError::Full));
    }

    #[test]
    fn permissions_are_a_set() {
        let p = Permissions::NONE.with(Permission::Screen).with(Permission::Gpu);
        assert!(p.allows(Permission::Screen) && p.allows(Permission::Gpu));
        assert!(!p.allows(Permission::Net));
    }
}
