//! **READING A PACKAGE** -- `Titan.toml` and the header of each module, into
//! the contract's `Graph`. It follows `mod` like cargo does: it never lists a
//! folder, it goes where the code says (`PLAN_TALLER` 8.6, decided 29-09).
//!
//! ```text
//!    titan/asteroids/Titan.toml          the ROOT node (name, permissions,
//!                                        [layout])
//!    src/main.titan                      always the first module
//!      mod ship, rock, physics      ->   src/ship.titan, src/rock.titan ...
//!    src/physics.titan
//!      mod collide                  ->   src/physics/collide.titan
//!      mod rules in "x/r.titan"     ->   x/r.titan: the PARENT says where
//!    use gpu / use director         ->   the 3060 and the DIRECTOR nodes
//! ```
//!
//! The files come out in the order of the DECLARED tree (each one under the
//! module that says `mod` of it), not of the folders: the disk may be in any
//! order, F1 shows the hierarchy the code declares (the owner, 29-09).
//!
//! An edge is a `mod` (the parent depends on its children) or a `use`. Both
//! go through `Graph::connect`, which refuses a cycle.
//!
//! Nothing here stops at the first problem: a package with a missing file
//! still shows everything else, and the missing piece is a PROBLEM with its
//! name -- the same idea as the 4-part message: say what and where.

use crate::header::{self, HeaderError};
use crate::manifest::{self, ManifestError};
use crate::text::{default_place, Path};
use crate::traits::Traits;
use bmo_titan_contrato::sample::{DIRECTOR_NAME, DIRECTOR_PURPOSE, GPU_NAME, GPU_PURPOSE, ROOT_PURPOSE};
use bmo_titan_contrato::{Graph, GraphError, Lang, Line, Name, Node, NodeId, NodeKind, Permission, Text, MAX_EDGES, MAX_NODES};

/// What a source answers when asked for a file.
pub enum Fetch {
    /// The file, in the first `n` bytes of the buffer.
    Found(usize),
    Missing,
    /// It exists and does not fit in the buffer.
    TooBig,
}

/// Where the files come from. F1 reads ESTRATOS; the tests, a table.
pub trait Source {
    fn fetch(&mut self, path: &[u8], buf: &mut [u8]) -> Fetch;
}

pub const MAX_PROBLEMS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProblemKind {
    NoManifest,
    BadManifest(ManifestError),
    NoMain,
    /// `a` is the file; the header error says where.
    BadHeader(HeaderError),
    /// The file `a` says `mod b`.
    NameMismatch,
    /// `b` declares `mod a` and its file is not there.
    MissingModule,
    /// Two nodes called `a`.
    Duplicate,
    /// `a` uses `b` and there is no `b`.
    UnknownUse,
    /// `a` uses `b` and the manifest did not ask for this permission (U2).
    NoPermission(Permission),
    /// `a -> b` would close a cycle.
    Cycle,
    /// More nodes, edges or files than fit.
    Full,
    /// The file `a` does not fit in the read buffer.
    TooBig,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Problem {
    pub kind: ProblemKind,
    pub a: Name,
    pub b: Name,
}

/// A small writer into one line of text.
pub struct Say {
    b: [u8; 72],
    n: usize,
}

impl Default for Say {
    fn default() -> Say {
        Say::new()
    }
}

impl Say {
    pub fn new() -> Say {
        Say { b: [0; 72], n: 0 }
    }

    pub fn t(mut self, s: &[u8]) -> Say {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
        self
    }

    pub fn num(self, v: usize) -> Say {
        let mut d = [0u8; 20];
        let (mut k, mut v) = (0, v);
        loop {
            d[k] = b'0' + (v % 10) as u8;
            k += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        d[..k].reverse();
        self.t(&d[..k])
    }

    pub fn done(self) -> Line {
        Text::new(core::str::from_utf8(&self.b[..self.n]).unwrap_or("?"))
    }
}

impl Problem {
    /// What the owner reads in the EXPLORER. Spanish, ASCII, no tilde.
    pub fn describe(&self) -> Line {
        let (a, b) = (self.a.as_bytes(), self.b.as_bytes());
        let s = Say::new();
        match self.kind {
            ProblemKind::NoManifest => s.t(b"no hay Titan.toml en el paquete"),
            ProblemKind::BadManifest(ManifestError::BadLine(l)) => s.t(b"Titan.toml no se entiende en la linea ").num(l),
            ProblemKind::BadManifest(ManifestError::NoName) => s.t(b"Titan.toml no dice [package] name"),
            ProblemKind::BadManifest(ManifestError::TooManyPositions) => s.t(b"Titan.toml: demasiadas filas en [layout]"),
            ProblemKind::NoMain => s.t(b"no hay src/main.titan: no hay por donde empezar"),
            ProblemKind::BadHeader(HeaderError::NoHeader) => s.t(a).t(b": falta la primera linea mod x \"que hace\""),
            ProblemKind::BadHeader(HeaderError::BadLine(l)) => s.t(a).t(b": la cabecera no se entiende, linea ").num(l),
            ProblemKind::BadHeader(HeaderError::TooMany(l)) => s.t(a).t(b": demasiados nombres en la linea ").num(l),
            ProblemKind::NameMismatch => s.t(a).t(b" dice mod ").t(b).t(b": el fichero y el nombre no casan"),
            ProblemKind::MissingModule => s.t(b).t(b" declara mod ").t(a).t(b" y su fichero no esta"),
            ProblemKind::Duplicate => s.t(b"hay dos nodos ").t(a),
            ProblemKind::UnknownUse => s.t(a).t(b" usa ").t(b).t(b" y no hay ningun modulo ").t(b),
            ProblemKind::NoPermission(p) => s.t(a).t(b" usa ").t(b).t(b" y Titan.toml no pide ").t(p.key().as_bytes()),
            ProblemKind::Cycle => s.t(a).t(b" -> ").t(b).t(b" cerraria un ciclo: solo se baja"),
            ProblemKind::Full => s.t(b"el paquete no cabe: 32 nodos y 64 cables como mucho"),
            ProblemKind::TooBig => s.t(a).t(b" no cabe en el buffer de lectura"),
        }
        .done()
    }
}

/// A file the reader used, relative to the package, and the node it made.
#[derive(Clone, Copy)]
pub struct FileEntry {
    pub path: Path,
    pub node: NodeId,
    /// Who declared it: the module whose `mod` names it (the root, for
    /// `Titan.toml` itself and for `main`).
    pub parent: NodeId,
    /// Steps under `Titan.toml` in the declared tree: 0 for it, 1 for main.
    pub depth: u8,
    /// What its body does (`traits.rs`): how F1 draws its node, live.
    pub traits: Traits,
}

impl FileEntry {
    const EMPTY: FileEntry = FileEntry { path: Path::EMPTY, node: NodeId(0), parent: NodeId(0), depth: 0, traits: Traits::NONE };
}

pub struct Loaded {
    pub graph: Graph,
    files: [FileEntry; MAX_NODES],
    n_files: usize,
    problems: [Problem; MAX_PROBLEMS],
    n_problems: usize,
    /// Problems that did not fit in the list: said as a number, not dropped.
    pub more_problems: usize,
}

impl Loaded {
    /// Nothing read yet. `const`, so a program can put one in place without a
    /// second copy on its stack.
    pub const fn new() -> Loaded {
        let none = Problem { kind: ProblemKind::Full, a: Text::new(""), b: Text::new("") };
        Loaded {
            graph: Graph::new(),
            files: [FileEntry::EMPTY; MAX_NODES],
            n_files: 0,
            problems: [none; MAX_PROBLEMS],
            n_problems: 0,
            more_problems: 0,
        }
    }

    /// Empties it for a new read, in place.
    fn clear(&mut self) {
        self.graph = Graph::new();
        self.n_files = 0;
        self.n_problems = 0;
        self.more_problems = 0;
    }

    /// Shows a graph that did not come from files, in place: no files, no
    /// problems.
    pub fn show(&mut self, graph: Graph) {
        self.clear();
        self.graph = graph;
    }

    /// A package that did not come from files (the hand-written sample, when
    /// there is no ESTRATOS): its graph, no files, no problems.
    pub fn from_graph(graph: Graph) -> Loaded {
        let mut l = Loaded::new();
        l.graph = graph;
        l
    }

    pub fn files(&self) -> &[FileEntry] {
        &self.files[..self.n_files]
    }

    /// The file a node came from.
    pub fn file_of(&self, node: NodeId) -> Option<&FileEntry> {
        self.files().iter().find(|f| f.node == node)
    }

    pub fn problems(&self) -> &[Problem] {
        &self.problems[..self.n_problems]
    }

    fn problem(&mut self, kind: ProblemKind, a: &[u8], b: &[u8]) {
        let name = |s: &[u8]| Text::new(core::str::from_utf8(s).unwrap_or("?"));
        if self.n_problems < MAX_PROBLEMS {
            self.problems[self.n_problems] = Problem { kind, a: name(a), b: name(b) };
            self.n_problems += 1;
        } else {
            self.more_problems += 1;
        }
    }

    fn file(&mut self, path: Path, node: NodeId, parent: NodeId, depth: usize, traits: Traits) {
        if self.n_files < MAX_NODES {
            self.files[self.n_files] = FileEntry { path, node, parent, depth: depth as u8, traits };
            self.n_files += 1;
        }
    }

    /// Puts the files in the order of the declared tree: each one right under
    /// its parent, brothers in the order they were declared (the reader went
    /// breadth-first, so brothers are already in that order). In place: two
    /// small index arrays, no second table of paths.
    fn tree_order(&mut self) {
        let n = self.n_files;
        let mut seq = [0u8; MAX_NODES];
        let mut k = 0;
        let mut stack = [0u8; MAX_NODES];
        let mut top = 0;
        for i in (0..n).rev().filter(|&i| self.files[i].depth == 0) {
            stack[top] = i as u8;
            top += 1;
        }
        while top > 0 {
            top -= 1;
            let i = stack[top] as usize;
            seq[k] = i as u8;
            k += 1;
            let f = self.files[i];
            for j in (0..n).rev() {
                let c = &self.files[j];
                if j != i && c.parent == f.node && c.depth == f.depth + 1 && top < MAX_NODES {
                    stack[top] = j as u8;
                    top += 1;
                }
            }
        }
        // new[i] = old[seq[i]], applied with swaps: what an earlier step moved
        // away is found by following `seq` until it points at or after `i`.
        for i in 0..k {
            let mut j = seq[i] as usize;
            while j < i {
                j = seq[j] as usize;
            }
            self.files.swap(i, j);
        }
        self.n_files = k;
    }
}

/// Where a node goes when `[layout]` does not say: one row per depth.
struct AutoLayout {
    per_depth: [i32; MAX_NODES],
}

impl AutoLayout {
    fn place(&mut self, depth: usize) -> (i32, i32) {
        let d = depth.min(MAX_NODES - 1);
        let i = self.per_depth[d];
        self.per_depth[d] += 1;
        (60 + i * 320, 20 + d as i32 * 150)
    }
}

/// A module waiting to be read: its file, from the folder of `Titan.toml`.
#[derive(Clone, Copy)]
struct Pending {
    file: Path,
    declared: Name,
    parent: NodeId,
    depth: usize,
}

pub fn read_package<S: Source>(src: &mut S, root: &[u8], buf: &mut [u8]) -> Loaded {
    let mut out = Loaded::new();
    read_package_into(src, root, buf, &mut out);
    out
}

/// The same, into a `Loaded` that already exists. What a program with a
/// small stack calls: a `Loaded` is ~8 KiB, and returning one by value costs
/// a second copy of it on the way out (F1 measured it: `pila.py --ring3`).
pub fn read_package_into<S: Source>(src: &mut S, root: &[u8], buf: &mut [u8], out: &mut Loaded) {
    out.clear();
    let mut auto = AutoLayout { per_depth: [0; MAX_NODES] };

    // -- the manifest: the root node --------------------------------------
    let Some(path) = Path::new(&[root, b"/Titan.toml"]) else {
        out.problem(ProblemKind::Full, root, b"");
        return;
    };
    let manifest = match src.fetch(path.as_bytes(), buf) {
        Fetch::Missing => {
            out.problem(ProblemKind::NoManifest, b"", b"");
            return;
        }
        Fetch::TooBig => {
            out.problem(ProblemKind::TooBig, b"Titan.toml", b"");
            return;
        }
        Fetch::Found(n) => match manifest::parse(&buf[..n.min(buf.len())]) {
            Ok(m) => m,
            Err(e) => {
                out.problem(ProblemKind::BadManifest(e), b"", b"");
                return;
            }
        },
    };
    out.graph.permissions = manifest.permissions;
    let place = |auto: &mut AutoLayout, key: &[u8], depth: usize| manifest.position(key).unwrap_or_else(|| auto.place(depth));
    let (x, y) = place(&mut auto, manifest.name.as_bytes(), 0);
    let root_name = core::str::from_utf8(manifest.name.as_bytes()).unwrap_or("?");
    let Ok(root_id) = out.graph.add(Node::new(NodeKind::Root, Lang::None, root_name, ROOT_PURPOSE, x, y)) else {
        out.problem(ProblemKind::Full, b"", b"");
        return;
    };
    out.file(Path::new(&[b"Titan.toml"]).unwrap_or(Path::EMPTY), root_id, root_id, 0, Traits::NONE);

    // -- the modules, following `mod` from src/main.titan ------------------
    let empty = Text::new("");
    let mut queue = [Pending { file: Path::EMPTY, declared: empty, parent: root_id, depth: 0 }; MAX_NODES];
    let (mut head, mut tail) = (0, 0);
    queue[tail] = Pending { file: Path::new(&[MAIN]).unwrap_or(Path::EMPTY), declared: Text::new("main"), parent: root_id, depth: 1 };
    tail += 1;
    // What each module uses, resolved once every module exists. Flat: one
    // row per `use`, as many as there can be edges (a table per module was
    // 13 KiB of stack for rows that are mostly empty).
    let mut uses: [(NodeId, Name); MAX_EDGES] = [(root_id, empty); MAX_EDGES];
    let mut n_uses = 0;
    let mut max_depth = 1;

    while head < tail {
        let p = queue[head];
        head += 1;
        let file = p.file;
        let Some(full) = Path::new(&[root, b"/", file.as_bytes()]) else {
            out.problem(ProblemKind::Full, p.declared.as_bytes(), b"");
            continue;
        };
        let parent_name = out.graph.node(p.parent).map(|n| n.name).unwrap_or(empty);
        let n = match src.fetch(full.as_bytes(), buf) {
            Fetch::Found(n) => n.min(buf.len()),
            Fetch::Missing if p.depth == 1 => {
                out.problem(ProblemKind::NoMain, b"", b"");
                continue;
            }
            Fetch::Missing => {
                out.problem(ProblemKind::MissingModule, p.declared.as_bytes(), parent_name.as_bytes());
                continue;
            }
            Fetch::TooBig => {
                out.problem(ProblemKind::TooBig, file.as_bytes(), b"");
                continue;
            }
        };
        let h = match header::parse(&buf[..n]) {
            Ok(h) => h,
            Err(e) => {
                out.problem(ProblemKind::BadHeader(e), file.as_bytes(), b"");
                continue;
            }
        };
        if h.name != p.declared {
            out.problem(ProblemKind::NameMismatch, file.as_bytes(), h.name.as_bytes());
            continue;
        }
        if out.graph.find(h.name.as_bytes()).is_some() {
            out.problem(ProblemKind::Duplicate, h.name.as_bytes(), b"");
            continue;
        }
        let (x, y) = place(&mut auto, h.name.as_bytes(), p.depth);
        let name = core::str::from_utf8(h.name.as_bytes()).unwrap_or("?");
        let purpose = core::str::from_utf8(h.purpose.as_bytes()).unwrap_or("?");
        let Ok(id) = out.graph.add(Node::new(NodeKind::Module, Lang::Titan, name, purpose, x, y)) else {
            out.problem(ProblemKind::Full, h.name.as_bytes(), b"");
            continue;
        };
        out.file(file, id, p.parent, p.depth, crate::traits::scan(&buf[..n]));
        max_depth = max_depth.max(p.depth);
        edge(out, p.parent, id);

        for child in h.children() {
            // Where the parent says, or where cargo would put it.
            let place = match child.path(&buf[..n]) {
                Some(path) => Path::new(&[path]),
                None => default_place(file.as_bytes(), child.name.as_bytes()),
            };
            match place {
                Some(file) if tail < MAX_NODES => {
                    queue[tail] = Pending { file, declared: child.name, parent: id, depth: p.depth + 1 };
                    tail += 1;
                }
                _ => out.problem(ProblemKind::Full, child.name.as_bytes(), b""),
            }
        }
        for used in h.uses() {
            if n_uses < MAX_EDGES {
                uses[n_uses] = (id, *used);
                n_uses += 1;
            } else {
                out.problem(ProblemKind::Full, used.as_bytes(), b"");
            }
        }
    }

    // -- the uses, now that every module exists -----------------------------
    for &(user, used) in &uses[..n_uses] {
        let user_name = out.graph.node(user).map(|n| n.name).unwrap_or(empty);
        let special = match used.as_bytes() {
            b"gpu" => Some((NodeKind::Gpu, GPU_NAME, GPU_PURPOSE, Permission::Gpu)),
            b"director" => Some((NodeKind::Director, DIRECTOR_NAME, DIRECTOR_PURPOSE, Permission::Screen)),
            _ => None,
        };
        let target = match special {
            Some((kind, name, purpose, perm)) => {
                // U2 in its smallest form: using the 3060 or the screen
                // needs the manifest to have asked for it.
                if !out.graph.permissions.allows(perm) {
                    out.problem(ProblemKind::NoPermission(perm), user_name.as_bytes(), used.as_bytes());
                    continue;
                }
                match out.graph.find(name.as_bytes()) {
                    Some(id) => Some(id),
                    None => {
                        let (x, y) = place(&mut auto, used.as_bytes(), max_depth + 1);
                        out.graph.add(Node::new(kind, Lang::None, name, purpose, x, y)).ok()
                    }
                }
            }
            None => out.graph.find(used.as_bytes()),
        };
        match target {
            Some(t) => edge(out, user, t),
            None if special.is_some() => out.problem(ProblemKind::Full, used.as_bytes(), b""),
            None => out.problem(ProblemKind::UnknownUse, user_name.as_bytes(), used.as_bytes()),
        }
    }
    out.tree_order();
}

/// Where every package starts.
pub const MAIN: &[u8] = b"src/main.titan";

const _: () = assert!(MAX_NODES <= u8::MAX as usize, "tree_order keeps indices in u8");

/// A `mod` or a `use`. The same edge twice is fine (a child that is also
/// used); a cycle is a problem with both names.
fn edge(out: &mut Loaded, from: NodeId, to: NodeId) {
    match out.graph.connect(from, to) {
        Ok(()) | Err(GraphError::Duplicate(..)) => {}
        Err(GraphError::Cycle { .. }) => {
            let a = out.graph.node(from).map(|n| n.name).unwrap_or(Text::new("?"));
            let b = out.graph.node(to).map(|n| n.name).unwrap_or(Text::new("?"));
            out.problem(ProblemKind::Cycle, a.as_bytes(), b.as_bytes());
        }
        Err(_) => out.problem(ProblemKind::Full, b"", b""),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::seed;
    use bmo_titan_contrato::sample;
    use std::vec::Vec;

    /// A source that is a table: the seed files, with some replaced or taken out.
    pub struct Table<'a> {
        pub files: Vec<(&'a str, &'a str)>,
    }

    impl<'a> Table<'a> {
        pub fn seed() -> Table<'a> {
            Table { files: seed::FILES.to_vec() }
        }

        pub fn put(mut self, path: &'a str, text: &'a str) -> Table<'a> {
            self.files.retain(|(p, _)| *p != path);
            self.files.push((path, text));
            self
        }

        pub fn remove(mut self, path: &str) -> Table<'a> {
            self.files.retain(|(p, _)| *p != path);
            self
        }
    }

    impl Source for Table<'_> {
        fn fetch(&mut self, path: &[u8], buf: &mut [u8]) -> Fetch {
            match self.files.iter().find(|(p, _)| p.as_bytes() == path) {
                None => Fetch::Missing,
                Some((_, t)) if t.len() > buf.len() => Fetch::TooBig,
                Some((_, t)) => {
                    buf[..t.len()].copy_from_slice(t.as_bytes());
                    Fetch::Found(t.len())
                }
            }
        }
    }

    fn read(t: &mut Table) -> Loaded {
        let mut buf = [0u8; 4096];
        read_package(t, b"titan/asteroids", &mut buf)
    }

    fn kinds(l: &Loaded) -> Vec<ProblemKind> {
        l.problems().iter().map(|p| p.kind).collect()
    }

    #[test]
    fn the_seed_files_are_the_hand_written_sample() {
        let got = read(&mut Table::seed());
        assert!(got.problems().is_empty(), "{:?}", kinds(&got));
        let (want, _) = sample::asteroids();
        assert_eq!(got.graph.nodes().len(), want.nodes().len());
        for w in want.nodes() {
            let g = got.graph.node(got.graph.find(w.name.as_bytes()).expect("same names")).unwrap();
            assert_eq!((g.kind, g.purpose, g.x, g.y), (w.kind, w.purpose, w.x, w.y), "{:?}", w.name);
        }
        assert_eq!(got.graph.edges().len(), want.edges().len());
        for e in want.edges() {
            let (a, b) = (want.node(e.from).unwrap().name, want.node(e.to).unwrap().name);
            let (ga, gb) = (got.graph.find(a.as_bytes()).unwrap(), got.graph.find(b.as_bytes()).unwrap());
            assert!(got.graph.edge_between(ga, gb), "{:?} -> {:?}", a, b);
        }
        assert_eq!(got.graph.permissions, want.permissions);
        // And so the sample's script fits the graph read from files.
        assert!(sample::script_for(&got.graph).is_some());
    }

    #[test]
    fn every_file_it_read_points_at_its_node() {
        let got = read(&mut Table::seed());
        let paths: Vec<&[u8]> = got.files().iter().map(|f| f.path.as_bytes()).collect();
        assert!(paths.contains(&&b"Titan.toml"[..]));
        assert!(paths.contains(&&b"src/physics/collide.titan"[..]));
        assert_eq!(got.files().len(), 6);
    }

    #[test]
    fn the_files_come_in_the_order_of_the_declared_tree() {
        let got = read(&mut Table::seed());
        let rows: Vec<(u8, &[u8])> = got.files().iter().map(|f| (f.depth, f.path.as_bytes())).collect();
        assert_eq!(
            rows,
            [
                (0, &b"Titan.toml"[..]),
                (1, b"src/main.titan"),
                (2, b"src/ship.titan"),
                (2, b"src/rock.titan"),
                (2, b"src/physics.titan"),
                (3, b"src/physics/collide.titan"),
            ]
        );
    }

    #[test]
    fn a_deep_tree_is_listed_depth_first_not_in_reading_order() {
        // Read breadth-first: main a b c d e f. Listed as the tree: a c f d b e.
        let mut t = Table {
            files: std::vec![
                ("p/Titan.toml", "[package]\nname = \"p\"\n"),
                ("p/src/main.titan", "mod main \"m\"\nmod a, b\n"),
                ("p/src/a.titan", "mod a \"a\"\nmod c, d\n"),
                ("p/src/b.titan", "mod b \"b\"\nmod e\n"),
                ("p/src/a/c.titan", "mod c \"c\"\nmod f\n"),
                ("p/src/a/d.titan", "mod d \"d\"\n"),
                ("p/src/b/e.titan", "mod e \"e\"\n"),
                ("p/src/a/c/f.titan", "mod f \"f\"\n"),
            ],
        };
        let mut buf = [0u8; 4096];
        let got = read_package(&mut t, b"p", &mut buf);
        assert!(got.problems().is_empty(), "{:?}", kinds(&got));
        let names: Vec<&[u8]> = got.files().iter().map(|f| got.graph.node(f.node).unwrap().name.as_bytes()).collect();
        assert_eq!(names, [&b"p"[..], b"main", b"a", b"c", b"f", b"d", b"b", b"e"]);
        let depths: Vec<u8> = got.files().iter().map(|f| f.depth).collect();
        assert_eq!(depths, [0, 1, 2, 3, 4, 3, 2, 3]);
    }

    #[test]
    fn the_disk_can_be_in_any_order_the_parent_says_where() {
        // collide lives far from physics, and ship under a folder of its own:
        // the graph and the tree are the same as the seed's.
        let t = Table::seed()
            .remove("titan/asteroids/src/physics/collide.titan")
            .remove("titan/asteroids/src/ship.titan")
            .put("titan/asteroids/cosas/choques.titan", "mod collide \"quien toca a quien\"\n")
            .put("titan/asteroids/naves/ship.titan", "mod ship \"la nave: se mueve y dispara\"\n")
            .put(
                "titan/asteroids/src/main.titan",
                "mod main \"arranca el juego y el bucle del fotograma\"\nuse director\nmod ship in \"naves/ship.titan\"\nmod rock, physics\n",
            )
            .put(
                "titan/asteroids/src/physics.titan",
                "mod physics \"mueve los cuerpos y resuelve los choques\"\nuse ship, gpu\nmod collide in \"cosas/choques.titan\"\n",
            );
        let got = read(&mut { t });
        assert!(got.problems().is_empty(), "{:?}", kinds(&got));
        assert!(sample::script_for(&got.graph).is_some());
        let collide = got.file_of(got.graph.find(b"collide").unwrap()).unwrap();
        assert_eq!((collide.path.as_bytes(), collide.depth), (&b"cosas/choques.titan"[..], 3));
        assert_eq!(got.files()[2].path.as_bytes(), b"naves/ship.titan");
    }

    #[test]
    fn a_missing_module_is_named_and_the_rest_still_shows() {
        let got = read(&mut Table::seed().remove("titan/asteroids/src/rock.titan"));
        assert_eq!(kinds(&got), [ProblemKind::MissingModule]);
        assert_eq!(got.problems()[0].describe().as_bytes(), b"main declara mod rock y su fichero no esta");
        assert!(got.graph.find(b"physics").is_some() && got.graph.find(b"rock").is_none());
    }

    #[test]
    fn a_cycle_is_refused_with_both_names() {
        let t = Table::seed().put("titan/asteroids/src/physics/collide.titan", "mod collide \"x\"\nuse physics\n");
        let got = read(&mut { t });
        assert_eq!(kinds(&got), [ProblemKind::Cycle]);
        assert_eq!(got.problems()[0].describe().as_bytes(), b"collide -> physics cerraria un ciclo: solo se baja");
    }

    #[test]
    fn using_the_3060_without_asking_for_it_is_the_u2_problem() {
        let toml = "[package]\nname = \"asteroids\"\n[permissions]\nscreen = true\n";
        let got = read(&mut Table::seed().put("titan/asteroids/Titan.toml", toml));
        assert_eq!(kinds(&got), [ProblemKind::NoPermission(Permission::Gpu)]);
        assert!(got.graph.find(GPU_NAME.as_bytes()).is_none());
    }

    #[test]
    fn the_file_and_its_name_must_agree() {
        let got = read(&mut Table::seed().put("titan/asteroids/src/ship.titan", "mod nave \"la nave\"\n"));
        // And physics, which uses ship, now uses a module that is not there.
        assert_eq!(kinds(&got), [ProblemKind::NameMismatch, ProblemKind::UnknownUse]);
        assert_eq!(got.problems()[0].describe().as_bytes(), b"src/ship.titan dice mod nave: el fichero y el nombre no casan");
    }

    #[test]
    fn an_unknown_use_and_a_headless_file_are_named() {
        let t = Table::seed()
            .put("titan/asteroids/src/rock.titan", "mod rock \"x\"\nuse ufo\n")
            .put("titan/asteroids/src/ship.titan", "fn main()\n");
        let got = read(&mut { t });
        assert!(kinds(&got).contains(&ProblemKind::UnknownUse));
        assert!(kinds(&got).contains(&ProblemKind::BadHeader(HeaderError::NoHeader)));
    }

    #[test]
    fn no_manifest_or_no_main_stop_early_and_say_so() {
        assert_eq!(kinds(&read(&mut Table::seed().remove("titan/asteroids/Titan.toml"))), [ProblemKind::NoManifest]);
        assert_eq!(kinds(&read(&mut Table::seed().remove("titan/asteroids/src/main.titan"))), [ProblemKind::NoMain]);
    }

    #[test]
    fn without_a_layout_every_node_still_gets_a_place_of_its_own() {
        let toml = "[package]\nname = \"asteroids\"\n[permissions]\nscreen = true\ngpu = true\n";
        let got = read(&mut Table::seed().put("titan/asteroids/Titan.toml", toml));
        let mut spots: Vec<(i32, i32)> = got.graph.nodes().iter().map(|n| (n.x, n.y)).collect();
        spots.sort();
        spots.dedup();
        assert_eq!(spots.len(), got.graph.nodes().len());
    }
}
