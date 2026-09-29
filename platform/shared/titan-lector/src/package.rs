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
//!    use gpu / use director         ->   the 3060 and the DIRECTOR nodes
//! ```
//!
//! An edge is a `mod` (the parent depends on its children) or a `use`. Both
//! go through `Graph::connect`, which refuses a cycle.
//!
//! Nothing here stops at the first problem: a package with a missing file
//! still shows everything else, and the missing piece is a PROBLEM with its
//! name -- the same idea as the 4-part message: say what and where.

use crate::header::{self, HeaderError, MAX_USES};
use crate::manifest::{self, ManifestError};
use crate::text::Path;
use bmo_titan_contrato::sample::{DIRECTOR_NAME, DIRECTOR_PURPOSE, GPU_NAME, GPU_PURPOSE, ROOT_PURPOSE};
use bmo_titan_contrato::{Graph, GraphError, Lang, Line, Name, Node, NodeId, NodeKind, Permission, Text, MAX_NODES};

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
struct Say {
    b: [u8; 72],
    n: usize,
}

impl Say {
    fn t(mut self, s: &[u8]) -> Say {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
        self
    }

    fn num(self, v: usize) -> Say {
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

    fn done(self) -> Line {
        Text::new(core::str::from_utf8(&self.b[..self.n]).unwrap_or("?"))
    }
}

impl Problem {
    /// What the owner reads in the EXPLORER. Spanish, ASCII, no tilde.
    pub fn describe(&self) -> Line {
        let (a, b) = (self.a.as_bytes(), self.b.as_bytes());
        let s = Say { b: [0; 72], n: 0 };
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
    fn new() -> Loaded {
        let none = Problem { kind: ProblemKind::Full, a: Text::new(""), b: Text::new("") };
        Loaded {
            graph: Graph::new(),
            files: [FileEntry { path: Path::EMPTY, node: NodeId(0) }; MAX_NODES],
            n_files: 0,
            problems: [none; MAX_PROBLEMS],
            n_problems: 0,
            more_problems: 0,
        }
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

    fn file(&mut self, path: Path, node: NodeId) {
        if self.n_files < MAX_NODES {
            self.files[self.n_files] = FileEntry { path, node };
            self.n_files += 1;
        }
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

/// A module waiting to be read: its path under `src/` without `.titan`.
#[derive(Clone, Copy)]
struct Pending {
    stem: Path,
    declared: Name,
    parent: NodeId,
    depth: usize,
}

pub fn read_package<S: Source>(src: &mut S, root: &[u8], buf: &mut [u8]) -> Loaded {
    let mut out = Loaded::new();
    let mut auto = AutoLayout { per_depth: [0; MAX_NODES] };

    // -- the manifest: the root node --------------------------------------
    let Some(path) = Path::new(&[root, b"/Titan.toml"]) else {
        out.problem(ProblemKind::Full, root, b"");
        return out;
    };
    let manifest = match src.fetch(path.as_bytes(), buf) {
        Fetch::Missing => {
            out.problem(ProblemKind::NoManifest, b"", b"");
            return out;
        }
        Fetch::TooBig => {
            out.problem(ProblemKind::TooBig, b"Titan.toml", b"");
            return out;
        }
        Fetch::Found(n) => match manifest::parse(&buf[..n.min(buf.len())]) {
            Ok(m) => m,
            Err(e) => {
                out.problem(ProblemKind::BadManifest(e), b"", b"");
                return out;
            }
        },
    };
    out.graph.permissions = manifest.permissions;
    let place = |auto: &mut AutoLayout, key: &[u8], depth: usize| manifest.position(key).unwrap_or_else(|| auto.place(depth));
    let (x, y) = place(&mut auto, manifest.name.as_bytes(), 0);
    let root_name = core::str::from_utf8(manifest.name.as_bytes()).unwrap_or("?");
    let Ok(root_id) = out.graph.add(Node::new(NodeKind::Root, Lang::None, root_name, ROOT_PURPOSE, x, y)) else {
        out.problem(ProblemKind::Full, b"", b"");
        return out;
    };
    out.file(Path::new(&[b"Titan.toml"]).unwrap_or(Path::EMPTY), root_id);

    // -- the modules, following `mod` from src/main.titan ------------------
    let empty = Text::new("");
    let mut queue = [Pending { stem: Path::EMPTY, declared: empty, parent: root_id, depth: 0 }; MAX_NODES];
    let (mut head, mut tail) = (0, 0);
    queue[tail] = Pending { stem: Path::new(&[b"main"]).unwrap_or(Path::EMPTY), declared: Text::new("main"), parent: root_id, depth: 1 };
    tail += 1;
    // What each module uses, resolved once every module exists.
    let mut uses: [(NodeId, [Name; MAX_USES], usize); MAX_NODES] = [(root_id, [empty; MAX_USES], 0); MAX_NODES];
    let mut n_uses = 0;
    let mut max_depth = 1;

    while head < tail {
        let p = queue[head];
        head += 1;
        let Some(file) = Path::new(&[b"src/", p.stem.as_bytes(), b".titan"]) else {
            out.problem(ProblemKind::Full, p.declared.as_bytes(), b"");
            continue;
        };
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
        out.file(file, id);
        max_depth = max_depth.max(p.depth);
        edge(&mut out, p.parent, id);

        for child in h.children() {
            // main's children live next to it; anyone else's, in its folder.
            let stem = if p.depth == 1 {
                Path::new(&[child.as_bytes()])
            } else {
                Path::new(&[p.stem.as_bytes(), b"/", child.as_bytes()])
            };
            match stem {
                Some(stem) if tail < MAX_NODES => {
                    queue[tail] = Pending { stem, declared: *child, parent: id, depth: p.depth + 1 };
                    tail += 1;
                }
                _ => out.problem(ProblemKind::Full, child.as_bytes(), b""),
            }
        }
        if !h.uses().is_empty() && n_uses < MAX_NODES {
            let mut list = [empty; MAX_USES];
            list[..h.uses().len()].copy_from_slice(h.uses());
            uses[n_uses] = (id, list, h.uses().len());
            n_uses += 1;
        }
    }

    // -- the uses, now that every module exists -----------------------------
    for &(user, list, count) in &uses[..n_uses] {
        let user_name = out.graph.node(user).map(|n| n.name).unwrap_or(empty);
        for used in &list[..count] {
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
                Some(t) => edge(&mut out, user, t),
                None if special.is_some() => out.problem(ProblemKind::Full, used.as_bytes(), b""),
                None => out.problem(ProblemKind::UnknownUse, user_name.as_bytes(), used.as_bytes()),
            }
        }
    }
    out
}

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
