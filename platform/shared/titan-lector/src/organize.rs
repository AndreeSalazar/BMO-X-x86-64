//! **ORGANIZING THE DISK** -- the EXPLORER's gestures: new file, new folder,
//! rename, remove, move into a folder, declare, and the owner's order. Each
//! one writes ESTRATOS through a `Disk`, and fixes the `.titan` headers it
//! would otherwise break.
//!
//! ```text
//!    the COLUMN organizes the DISK    folders, files, names, order
//!    the CANVAS organizes MODULES     who declares whom (`hang.rs`)
//! ```
//!
//! ** A `.titan` is a file AND a module, so a disk gesture on one carries its
//! module along -- or says plainly what it could not carry:
//!
//! ```text
//!    rename rock.titan -> roca.titan   the file, its own `mod rock` line, and
//!                                      its parent's `mod rock` -> `mod roca`
//!    move it to another folder         its parent's `mod` gets `in "path"`
//!    remove it                         its parent stops declaring it
//!    a new one, with a module picked   it is born declared by that module
//! ```
//!
//! What it does NOT rewrite, on purpose: the `use rock` lines of OTHER files
//! (that would be reading and writing the whole package for one name), the
//! children a module keeps in a folder named after it, and the `mod ... in`
//! paths that went through a renamed folder. Those show up in PROBLEMS with
//! their names, and the canvas marks them in red: the reader already says
//! exactly what broke, and the note says to look there.
//!
//! Nothing is lost by any of this: in ESTRATOS every write is a version, and
//! `vuelve N` in F12 undoes N of them. Each gesture says its N.

use crate::edit::{self, EditError};
use crate::explorer::{check_name, NameError, PlaceError, SettingsError, Tree};
use crate::hang::Sink;
use crate::package::{Fetch, FileEntry, Loaded, Say, Source};
use crate::text::{default_place, Path};
use bmo_titan_contrato::{Line, NodeId};

/// What the EXPLORER needs from a disk besides reading and saving.
pub trait Disk: Source + Sink {
    fn make_folder(&mut self, path: &[u8]) -> bool;
    /// A NEW file: `false` if the name is taken (it never overwrites).
    fn create(&mut self, path: &[u8], bytes: &[u8]) -> bool;
    /// Gives `path` the name `new` (a name, not a path: it stays in its folder).
    fn rename(&mut self, path: &[u8], new: &[u8]) -> bool;
    /// Stops naming `path`. In ESTRATOS it is still in the history.
    fn remove(&mut self, path: &[u8]) -> bool;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrgError {
    Name(NameError),
    /// `Titan.toml` and `src/main.titan` are where a package starts.
    Protected,
    /// A `.titan` stays a `.titan`, and nothing becomes one by renaming.
    KeepTitan,
    /// Moving folders is not here yet.
    FolderMove,
    /// It is already there.
    Same,
    /// Only a `.titan` can be declared by a module.
    NotTitan,
    /// That `.titan` is already declared by someone.
    Declared,
    /// That item or node is not there any more.
    NoSuch,
    /// The file does not fit in the buffer to be moved.
    TooBig,
    Read,
    Write,
    Edit(EditError),
    Settings,
}

/// What a gesture did: how many versions it wrote (`vuelve N` undoes it), and
/// what it could not carry along.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Done {
    pub writes: u8,
    pub left: Left,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Left {
    Nothing,
    /// A module was renamed: the `use` of others still say the old name.
    Uses,
    /// A folder was renamed or a module moved: paths through it may break.
    Paths,
}

/// Where the gesture happens: the package and what was read of it.
pub struct At<'a> {
    /// `titan/asteroids`.
    pub root: &'a [u8],
    pub loaded: &'a Loaded,
}

impl At<'_> {
    fn full(&self, rel: &[u8]) -> Result<Path, OrgError> {
        Path::new(&[self.root, b"/", rel]).ok_or(OrgError::TooBig)
    }

    /// The module whose file is `rel`, if a `mod` declares it.
    fn module(&self, rel: &[u8]) -> Option<&FileEntry> {
        self.loaded.files().iter().find(|f| f.depth > 0 && f.path.as_bytes() == rel)
    }

    fn file_of(&self, node: NodeId) -> Option<&FileEntry> {
        self.loaded.file_of(node)
    }
}

fn protected(rel: &[u8]) -> bool {
    rel == b"Titan.toml" || rel == crate::package::MAIN
}

fn stem(name: &[u8]) -> Option<&[u8]> {
    name.strip_suffix(b".titan")
}

/// The `in "..."` a parent needs to find `rel`: none if cargo would look there.
fn place_for<'r>(parent_rel: &[u8], child: &[u8], rel: &'r [u8]) -> Option<&'r [u8]> {
    match default_place(parent_rel, child) {
        Some(p) if p.as_bytes() == rel => None,
        _ => Some(rel),
    }
}

/// Reads `rel` of the package into `buf`.
fn read<D: Disk>(io: &mut D, at: &At, rel: &[u8], buf: &mut [u8]) -> Result<usize, OrgError> {
    match io.fetch(at.full(rel)?.as_bytes(), buf) {
        Fetch::Found(n) => Ok(n),
        Fetch::TooBig => Err(OrgError::TooBig),
        Fetch::Missing => Err(OrgError::Read),
    }
}

fn save<D: Disk>(io: &mut D, at: &At, rel: &[u8], bytes: &[u8]) -> Result<(), OrgError> {
    if io.store(at.full(rel)?.as_bytes(), bytes) {
        Ok(())
    } else {
        Err(OrgError::Write)
    }
}

/// The parent's header: `old` out (if given), `new` in at `path` (if given).
/// Uses both buffers and leaves the result saved.
#[allow(clippy::too_many_arguments)]
fn redeclare<D: Disk>(
    io: &mut D,
    at: &At,
    parent_rel: &[u8],
    old: Option<&[u8]>,
    new: Option<(&[u8], Option<&[u8]>)>,
    text: &mut [u8],
    out: &mut [u8],
) -> Result<(), OrgError> {
    let n = read(io, at, parent_rel, text)?;
    let n = match old {
        Some(child) => {
            let m = edit::remove_child(&text[..n], child, out).map_err(OrgError::Edit)?;
            text[..m].copy_from_slice(&out[..m]);
            m
        }
        None => n,
    };
    let n = match new {
        Some((child, path)) => {
            let m = edit::add_child(&text[..n], child, path, out).map_err(OrgError::Edit)?;
            text[..m].copy_from_slice(&out[..m]);
            m
        }
        None => n,
    };
    // `text` is where the Disk can take it from (the block of the gesture).
    save(io, at, parent_rel, &text[..n])
}

/// The rel path of a new entry `name` inside `folder`.
fn inside(tree: &Tree, folder: Option<usize>, name: &[u8]) -> Result<Path, OrgError> {
    match folder {
        None => Path::new(&[name]),
        Some(f) => {
            let p = tree.path_of(f).ok_or(OrgError::NoSuch)?;
            Path::new(&[p.as_bytes(), b"/", name])
        }
    }
    .ok_or(OrgError::TooBig)
}

/// **The owner's order and folds, written into Titan.toml.**
pub fn arrange<D: Disk>(io: &mut D, at: &At, tree: &Tree, text: &mut [u8], out: &mut [u8]) -> Result<Done, OrgError> {
    let n = read(io, at, b"Titan.toml", text)?;
    let m = tree.settings(&text[..n], out).map_err(|SettingsError::DoesNotFit| OrgError::Settings)?;
    save(io, at, b"Titan.toml", &out[..m])?;
    Ok(Done { writes: 1, left: Left::Nothing })
}

/// `moving` before or after `target` (siblings), and the order saved.
#[allow(clippy::too_many_arguments)]
pub fn place<D: Disk>(io: &mut D, at: &At, tree: &mut Tree, moving: usize, target: usize, after: bool, text: &mut [u8], out: &mut [u8]) -> Result<Done, OrgError> {
    match tree.place(moving, target, after) {
        Ok(()) => arrange(io, at, tree, text, out),
        Err(PlaceError::Same) => Err(OrgError::Same),
        Err(PlaceError::OtherFolder) => Err(OrgError::FolderMove),
        Err(PlaceError::NoSuch) => Err(OrgError::NoSuch),
    }
}

/// **A new file or folder** called `name` inside `folder` (None = the package
/// folder). A new `.titan` is born with its header, and declared by
/// `declarer` if a module was picked.
#[allow(clippy::too_many_arguments)]
pub fn create<D: Disk>(
    io: &mut D,
    at: &At,
    tree: &Tree,
    folder: Option<usize>,
    name: &[u8],
    is_folder: bool,
    declarer: Option<NodeId>,
    text: &mut [u8],
    out: &mut [u8],
) -> Result<Done, OrgError> {
    check_name(name).map_err(OrgError::Name)?;
    tree.free(folder, name, None).map_err(OrgError::Name)?;
    let rel = inside(tree, folder, name)?;
    let full = at.full(rel.as_bytes())?;
    if is_folder {
        return if io.make_folder(full.as_bytes()) { Ok(Done { writes: 1, left: Left::Nothing }) } else { Err(OrgError::Write) };
    }
    let Some(module) = stem(name) else {
        return if io.create(full.as_bytes(), b"") { Ok(Done { writes: 1, left: Left::Nothing }) } else { Err(OrgError::Write) };
    };
    // `mod x "..."`: the first line a module must have (U3). Written byte by
    // byte: a `Line` would turn the `\n` into `?`.
    let mut h = 0;
    for part in [&b"mod "[..], module, b" \"nuevo: di aqui que hace\"\n"] {
        out.get_mut(h..h + part.len()).ok_or(OrgError::TooBig)?.copy_from_slice(part);
        h += part.len();
    }
    if !io.create(full.as_bytes(), &out[..h]) {
        return Err(OrgError::Write);
    }
    let Some(parent) = declarer.and_then(|d| at.file_of(d)) else {
        return Ok(Done { writes: 1, left: Left::Nothing });
    };
    let prel = parent.path;
    redeclare(io, at, prel.as_bytes(), None, Some((module, place_for(prel.as_bytes(), module, rel.as_bytes()))), text, out)?;
    Ok(Done { writes: 2, left: Left::Nothing })
}

/// **Renames** item `i` to `new`, and carries its module along.
#[allow(clippy::too_many_arguments)]
pub fn rename<D: Disk>(io: &mut D, at: &At, tree: &mut Tree, i: usize, new: &[u8], text: &mut [u8], out: &mut [u8]) -> Result<Done, OrgError> {
    let rel = tree.path_of(i).ok_or(OrgError::NoSuch)?;
    let old = tree.name(i);
    if old == new {
        return Err(OrgError::Same);
    }
    if protected(rel.as_bytes()) {
        return Err(OrgError::Protected);
    }
    check_name(new).map_err(OrgError::Name)?;
    if !tree.is_folder(i) && stem(old).is_some() != stem(new).is_some() {
        return Err(OrgError::KeepTitan);
    }
    tree.free(tree.parent(i), new, Some(i)).map_err(OrgError::Name)?;
    let module = at.module(rel.as_bytes()).copied();
    let new_rel = inside(tree, tree.parent(i), new)?;
    let old_stem = Path::new(&[stem(old).unwrap_or(b"")]).ok_or(OrgError::TooBig)?;

    if !io.rename(at.full(rel.as_bytes())?.as_bytes(), new) {
        return Err(OrgError::Write);
    }
    let mut writes = 1u8;
    let mut left = if tree.is_folder(i) { Left::Paths } else { Left::Nothing };
    if let (Some(m), Some(new_stem)) = (module, stem(new)) {
        // Its own first line, in the file under its NEW name.
        let n = read(io, at, new_rel.as_bytes(), text)?;
        let k = edit::rename_module(&text[..n], new_stem, out).map_err(OrgError::Edit)?;
        save(io, at, new_rel.as_bytes(), &out[..k])?;
        writes += 1;
        // And the parent that declares it.
        let parent = at.file_of(m.parent).ok_or(OrgError::NoSuch)?.path;
        let path = place_for(parent.as_bytes(), new_stem, new_rel.as_bytes());
        redeclare(io, at, parent.as_bytes(), Some(old_stem.as_bytes()), Some((new_stem, path)), text, out)?;
        writes += 1;
        left = Left::Uses;
    }
    // Its place and folds hang from the item: the order is written with the
    // new name, so nothing the owner arranged moves.
    tree.rename(i, new);
    if arrange(io, at, tree, text, out).is_ok() {
        writes += 1;
    }
    Ok(Done { writes, left })
}

/// **Removes** item `i`; a module stops being declared first.
pub fn remove<D: Disk>(io: &mut D, at: &At, tree: &Tree, i: usize, text: &mut [u8], out: &mut [u8]) -> Result<Done, OrgError> {
    let rel = tree.path_of(i).ok_or(OrgError::NoSuch)?;
    if protected(rel.as_bytes()) {
        return Err(OrgError::Protected);
    }
    let mut writes = 0u8;
    if let (Some(m), Some(s)) = (at.module(rel.as_bytes()), stem(tree.name(i))) {
        let parent = at.file_of(m.parent).ok_or(OrgError::NoSuch)?.path;
        redeclare(io, at, parent.as_bytes(), Some(s), None, text, out)?;
        writes += 1;
    }
    if !io.remove(at.full(rel.as_bytes())?.as_bytes()) {
        return Err(OrgError::Write);
    }
    Ok(Done { writes: writes + 1, left: Left::Nothing })
}

/// **Moves** file `i` into `folder` (None = the package folder). The bytes go
/// through `text`, so a file bigger than it is refused, not cut.
#[allow(clippy::too_many_arguments)]
pub fn move_into<D: Disk>(io: &mut D, at: &At, tree: &Tree, i: usize, folder: Option<usize>, text: &mut [u8], out: &mut [u8]) -> Result<Done, OrgError> {
    if tree.is_folder(i) {
        return Err(OrgError::FolderMove);
    }
    let rel = tree.path_of(i).ok_or(OrgError::NoSuch)?;
    if protected(rel.as_bytes()) {
        return Err(OrgError::Protected);
    }
    if tree.parent(i) == folder {
        return Err(OrgError::Same);
    }
    let name = tree.name(i);
    tree.free(folder, name, None).map_err(OrgError::Name)?;
    let new_rel = inside(tree, folder, name)?;
    let n = read(io, at, rel.as_bytes(), text)?;
    if !io.create(at.full(new_rel.as_bytes())?.as_bytes(), &text[..n]) {
        return Err(OrgError::Write);
    }
    if !io.remove(at.full(rel.as_bytes())?.as_bytes()) {
        return Err(OrgError::Write);
    }
    let mut writes = 2u8;
    let mut left = Left::Nothing;
    if let (Some(m), Some(s)) = (at.module(rel.as_bytes()), stem(name)) {
        let parent = at.file_of(m.parent).ok_or(OrgError::NoSuch)?.path;
        let path = place_for(parent.as_bytes(), s, new_rel.as_bytes());
        redeclare(io, at, parent.as_bytes(), Some(s), Some((s, path)), text, out)?;
        writes += 1;
        left = Left::Paths;
    }
    Ok(Done { writes, left })
}

/// **Declares** the undeclared `.titan` item `i` as a child of `parent`: it
/// becomes a node of the canvas.
#[allow(clippy::too_many_arguments)]
pub fn declare<D: Disk>(io: &mut D, at: &At, tree: &Tree, i: usize, parent: NodeId, text: &mut [u8], out: &mut [u8]) -> Result<Done, OrgError> {
    let rel = tree.path_of(i).ok_or(OrgError::NoSuch)?;
    let s = stem(tree.name(i)).ok_or(OrgError::NotTitan)?;
    if at.module(rel.as_bytes()).is_some() {
        return Err(OrgError::Declared);
    }
    let p = at.file_of(parent).ok_or(OrgError::NoSuch)?.path;
    if stem(p.as_bytes().rsplit(|&c| c == b'/').next().unwrap_or(b"")).is_none() {
        return Err(OrgError::NotTitan);
    }
    redeclare(io, at, p.as_bytes(), None, Some((s, place_for(p.as_bytes(), s, rel.as_bytes()))), text, out)?;
    Ok(Done { writes: 1, left: Left::Nothing })
}

/// What the owner reads after a gesture, in two lines of the column: what
/// happened, and -- if anything -- what is left or why not. Spanish, ASCII.
pub fn note(r: Result<Done, OrgError>, what: &[u8]) -> (Line, Option<Line>, bool) {
    match r {
        Ok(d) => {
            let first = Say::new().t(what).t(b"; vuelve ").num(d.writes as usize).t(b" lo deshace").done();
            let left: Option<&[u8]> = match d.left {
                Left::Nothing => None,
                Left::Uses => Some(b"los use de otros ficheros siguen con el nombre viejo: PROBLEMAS"),
                Left::Paths => Some(b"lo que usaba la ruta vieja sale en PROBLEMAS"),
            };
            (first, left.map(|l| Say::new().t(l).done()), true)
        }
        Err(e) => {
            let why: &[u8] = match e {
                OrgError::Name(n) => n.say().as_bytes(),
                OrgError::Protected => b"Titan.toml y src/main.titan son donde empieza el paquete",
                OrgError::KeepTitan => b"un .titan sigue siendo .titan (y al reves)",
                OrgError::FolderMove => b"mover carpetas llega despues: renombrala o mueve sus ficheros",
                OrgError::Same => b"ya esta ahi",
                OrgError::NotTitan => b"solo un .titan se declara, y solo un modulo declara",
                OrgError::Declared => b"ya lo declara un modulo: para cambiarlo, cuelgalo en el lienzo",
                OrgError::NoSuch => b"ya no esta en el disco",
                OrgError::TooBig => b"no cabe en el buffer del taller",
                OrgError::Read => b"no pude leerlo en ESTRATOS",
                OrgError::Write => b"ESTRATOS no lo guardo (CABINA, F11, dice por que)",
                OrgError::Edit(_) => b"una cabecera no se dejo reescribir",
                OrgError::Settings => b"el Titan.toml nuevo no cabe",
            };
            (Say::new().t(what).t(b": no").done(), Some(Say::new().t(why).done()), false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explorer::{Lister, MAX_ITEMS};
    use crate::package::read_package;
    use std::boxed::Box;
    use std::collections::BTreeMap;
    use std::string::String;
    use std::vec::Vec;

    /// ESTRATOS in memory: files with bytes, folders as a set. Lists in the
    /// order things were made, like the disk does.
    #[derive(Default)]
    struct Mem {
        files: BTreeMap<Vec<u8>, Vec<u8>>,
        folders: Vec<Vec<u8>>,
        made: Vec<Vec<u8>>,
        writes: usize,
    }

    impl Mem {
        fn seed() -> Mem {
            let mut m = Mem::default();
            for f in crate::seed::FOLDERS {
                m.make_folder(f.as_bytes());
            }
            for (p, t) in crate::seed::FILES {
                m.create(p.as_bytes(), t.as_bytes());
            }
            m.writes = 0;
            m
        }
        fn text(&self, p: &str) -> String {
            String::from_utf8(self.files[p.as_bytes()].clone()).unwrap()
        }
    }

    impl Source for Mem {
        fn fetch(&mut self, path: &[u8], buf: &mut [u8]) -> Fetch {
            match self.files.get(path) {
                Some(b) if b.len() > buf.len() => Fetch::TooBig,
                Some(b) => {
                    buf[..b.len()].copy_from_slice(b);
                    Fetch::Found(b.len())
                }
                None => Fetch::Missing,
            }
        }
    }

    impl Sink for Mem {
        fn store(&mut self, path: &[u8], bytes: &[u8]) -> bool {
            if !self.files.contains_key(path) {
                self.made.push(path.to_vec());
            }
            self.files.insert(path.to_vec(), bytes.to_vec());
            self.writes += 1;
            true
        }
    }

    impl Disk for Mem {
        fn make_folder(&mut self, path: &[u8]) -> bool {
            if self.folders.iter().any(|f| f == path) {
                return false;
            }
            self.folders.push(path.to_vec());
            self.made.push(path.to_vec());
            self.writes += 1;
            true
        }
        fn create(&mut self, path: &[u8], bytes: &[u8]) -> bool {
            if self.files.contains_key(path) {
                return false;
            }
            self.store(path, bytes)
        }
        fn rename(&mut self, path: &[u8], new: &[u8]) -> bool {
            let cut = path.iter().rposition(|&c| c == b'/').map_or(0, |i| i + 1);
            let mut to = path[..cut].to_vec();
            to.extend_from_slice(new);
            // Everything under it too, keeping the order it was made in.
            let mut moved = false;
            for p in self.made.iter_mut() {
                if p == path || (p.starts_with(path) && p.get(path.len()) == Some(&b'/')) {
                    let mut q = to.clone();
                    q.extend_from_slice(&p[path.len()..]);
                    if let Some(b) = self.files.remove(p.as_slice()) {
                        self.files.insert(q.clone(), b);
                    }
                    if let Some(f) = self.folders.iter_mut().find(|f| f.as_slice() == p.as_slice()) {
                        *f = q.clone();
                    }
                    *p = q;
                    moved = true;
                }
            }
            self.writes += 1;
            moved
        }
        fn remove(&mut self, path: &[u8]) -> bool {
            let had = self.files.remove(path).is_some() || self.folders.iter().any(|f| f == path);
            self.folders.retain(|f| f != path);
            self.made.retain(|p| p != path);
            self.writes += 1;
            had
        }
    }

    impl Lister for Mem {
        fn list(&mut self, folder: &[u8], put: &mut dyn FnMut(&[u8], bool, u64)) -> bool {
            for p in &self.made {
                if p.starts_with(folder) && p.get(folder.len()) == Some(&b'/') {
                    let rest = &p[folder.len() + 1..];
                    if !rest.contains(&b'/') {
                        put(rest, self.folders.iter().any(|f| f == p), 0);
                    }
                }
            }
            true
        }
    }

    const ROOT: &[u8] = b"titan/asteroids";

    /// The package read again, and its tree, after each gesture: what F1 does
    /// when the generation moves.
    fn look(m: &mut Mem) -> (Loaded, Box<Tree>) {
        let mut buf = [0u8; 4096];
        let l = read_package(m, ROOT, &mut buf);
        let mut t: Box<Tree> = crate::explorer::boxed();
        t.fill(m, ROOT);
        if let Fetch::Found(n) = m.fetch(b"titan/asteroids/Titan.toml", &mut buf) {
            t.apply(&buf[..n]);
        }
        (l, t)
    }

    fn bufs() -> (Vec<u8>, Vec<u8>) {
        (std::vec![0u8; 8192], std::vec![0u8; 8192])
    }

    fn names(l: &Loaded) -> Vec<String> {
        l.graph.nodes().iter().map(|n| String::from_utf8(n.name.as_bytes().to_vec()).unwrap()).collect()
    }

    #[test]
    fn the_seed_reads_clean_and_the_tree_has_every_file() {
        let mut m = Mem::seed();
        let (l, t) = look(&mut m);
        assert!(l.problems().is_empty(), "{:?}", l.problems().len());
        assert!(t.find(b"src/physics/collide.titan").is_some());
        assert!(t.len() < MAX_ITEMS);
    }

    #[test]
    fn renaming_a_module_carries_its_header_and_its_parent() {
        let mut m = Mem::seed();
        let (l, mut t) = look(&mut m);
        let (mut a, mut b) = bufs();
        let rock = t.find(b"src/rock.titan").unwrap();
        let at = At { root: ROOT, loaded: &l };
        let d = rename(&mut m, &at, &mut t, rock, b"roca.titan", &mut a, &mut b).unwrap();
        assert_eq!(d.left, Left::Uses);
        assert!(m.text("titan/asteroids/src/roca.titan").starts_with("mod roca "));
        assert!(m.text("titan/asteroids/src/main.titan").contains("roca"));
        assert!(!m.text("titan/asteroids/src/main.titan").contains("rock"));
        let (l2, _) = look(&mut m);
        assert!(names(&l2).contains(&String::from("roca")), "{:?}", names(&l2));
        // `ship` and `physics` used `rock`: those are the PROBLEMS the note
        // points at, never silently rewritten.
        for p in l2.problems() {
            let line = p.describe();
            assert!(core::str::from_utf8(line.as_bytes()).unwrap().contains("rock"));
        }
    }

    #[test]
    fn a_new_titan_is_born_with_its_header_and_declared_by_the_picked_module() {
        let mut m = Mem::seed();
        let (l, t) = look(&mut m);
        let (mut a, mut b) = bufs();
        let ship = l.graph.find(b"ship").unwrap();
        let src = t.find(b"src");
        let at = At { root: ROOT, loaded: &l };
        let d = create(&mut m, &at, &t, src, b"wing.titan", false, Some(ship), &mut a, &mut b).unwrap();
        assert_eq!(d.writes, 2);
        assert!(m.text("titan/asteroids/src/wing.titan").starts_with("mod wing \""));
        // src/ship.titan's children live in src/ship/ for cargo: this one needs `in`.
        assert!(m.text("titan/asteroids/src/ship.titan").contains("mod wing in \"src/wing.titan\""));
        let (l2, _) = look(&mut m);
        assert!(l2.problems().is_empty());
        assert!(names(&l2).contains(&String::from("wing")));
        // And a name that is taken, or not a module's, is said before writing.
        let w = m.writes;
        let at = At { root: ROOT, loaded: &l2 };
        let (_, t2) = look(&mut m);
        assert_eq!(create(&mut m, &at, &t2, src, b"wing.titan", false, None, &mut a, &mut b), Err(OrgError::Name(NameError::Taken)));
        assert_eq!(create(&mut m, &at, &t2, src, b"Wing.titan", false, None, &mut a, &mut b), Err(OrgError::Name(NameError::BadModule)));
        assert_eq!(m.writes, w);
    }

    #[test]
    fn folders_and_plain_files_are_just_made() {
        let mut m = Mem::seed();
        let (l, t) = look(&mut m);
        let (mut a, mut b) = bufs();
        let at = At { root: ROOT, loaded: &l };
        create(&mut m, &at, &t, None, b"docs", true, None, &mut a, &mut b).unwrap();
        let (l, t) = look(&mut m);
        let at = At { root: ROOT, loaded: &l };
        create(&mut m, &at, &t, t.find(b"docs"), b"LEEME.txt", false, None, &mut a, &mut b).unwrap();
        let (_, t) = look(&mut m);
        assert!(t.is_folder(t.find(b"docs").unwrap()));
        assert_eq!(t.parent(t.find(b"docs/LEEME.txt").unwrap()), t.find(b"docs"));
    }

    #[test]
    fn removing_a_module_undeclares_it_first_and_main_stays() {
        let mut m = Mem::seed();
        let (l, t) = look(&mut m);
        let (mut a, mut b) = bufs();
        let at = At { root: ROOT, loaded: &l };
        let collide = t.find(b"src/physics/collide.titan").unwrap();
        assert_eq!(remove(&mut m, &at, &t, collide, &mut a, &mut b).unwrap().writes, 2);
        assert!(!m.text("titan/asteroids/src/physics.titan").contains("collide"));
        let (l2, _) = look(&mut m);
        assert!(l2.problems().is_empty());
        assert_eq!(remove(&mut m, &at, &t, t.find(b"src/main.titan").unwrap(), &mut a, &mut b), Err(OrgError::Protected));
        assert_eq!(remove(&mut m, &at, &t, t.find(b"Titan.toml").unwrap(), &mut a, &mut b), Err(OrgError::Protected));
    }

    #[test]
    fn moving_a_module_to_another_folder_tells_its_parent_where() {
        let mut m = Mem::seed();
        let (mut a, mut b) = bufs();
        let (l, t) = look(&mut m);
        let at = At { root: ROOT, loaded: &l };
        create(&mut m, &at, &t, None, b"naves", true, None, &mut a, &mut b).unwrap();
        let (l, t) = look(&mut m);
        let at = At { root: ROOT, loaded: &l };
        let ship = t.find(b"src/ship.titan").unwrap();
        move_into(&mut m, &at, &t, ship, t.find(b"naves"), &mut a, &mut b).unwrap();
        assert!(m.files.get(&b"titan/asteroids/src/ship.titan"[..]).is_none());
        assert!(m.text("titan/asteroids/src/main.titan").contains("mod ship in \"naves/ship.titan\""));
        let (l2, _) = look(&mut m);
        assert!(l2.problems().is_empty(), "{}", core::str::from_utf8(l2.problems()[0].describe().as_bytes()).unwrap());
        assert!(names(&l2).contains(&String::from("ship")));
    }

    #[test]
    fn the_owner_s_order_survives_a_rename_and_a_new_look() {
        let mut m = Mem::seed();
        let (mut a, mut b) = bufs();
        let (l, mut t) = look(&mut m);
        let at = At { root: ROOT, loaded: &l };
        let (rock, main) = (t.find(b"src/rock.titan").unwrap(), t.find(b"src/main.titan").unwrap());
        place(&mut m, &at, &mut t, rock, main, false, &mut a, &mut b).unwrap();
        assert!(m.text("titan/asteroids/Titan.toml").contains("\"rock.titan\", \"main.titan\""));
        let (l, mut t) = look(&mut m);
        let at = At { root: ROOT, loaded: &l };
        let rock = t.find(b"src/rock.titan").unwrap();
        rename(&mut m, &at, &mut t, rock, b"roca.titan", &mut a, &mut b).unwrap();
        let (_, t) = look(&mut m);
        let src = t.find(b"src").unwrap();
        // Where the owner put it -- right before main -- under its new name.
        let order: Vec<Vec<u8>> = t.rows().filter(|&(i, _)| t.parent(i) == Some(src)).map(|(i, _)| t.name(i).to_vec()).collect();
        let at_roca = order.iter().position(|n| n == b"roca.titan").unwrap();
        assert_eq!(order[at_roca + 1], b"main.titan");
    }

    #[test]
    fn an_undeclared_titan_is_declared_by_dropping_it_on_a_module() {
        let mut m = Mem::seed();
        let (mut a, mut b) = bufs();
        let (l, t) = look(&mut m);
        let at = At { root: ROOT, loaded: &l };
        create(&mut m, &at, &t, t.find(b"src"), b"ufo.titan", false, None, &mut a, &mut b).unwrap();
        let (l, t) = look(&mut m);
        assert!(!names(&l).contains(&String::from("ufo")));
        let at = At { root: ROOT, loaded: &l };
        let ufo = t.find(b"src/ufo.titan").unwrap();
        declare(&mut m, &at, &t, ufo, l.graph.find(b"main").unwrap(), &mut a, &mut b).unwrap();
        let (l2, _) = look(&mut m);
        assert!(names(&l2).contains(&String::from("ufo")));
        assert!(m.text("titan/asteroids/src/main.titan").contains("mod ufo\n"));
        let at = At { root: ROOT, loaded: &l2 };
        assert_eq!(declare(&mut m, &at, &t, ufo, l2.graph.find(b"main").unwrap(), &mut a, &mut b), Err(OrgError::Declared));
    }

    #[test]
    fn every_note_is_one_line_of_the_column() {
        let (line, left, ok) = note(Ok(Done { writes: 3, left: Left::Uses }), b"rock -> roca");
        assert!(ok && line.as_bytes() == b"rock -> roca; vuelve 3 lo deshace" && left.is_some());
        let (line, why, ok) = note(Err(OrgError::Protected), b"quitar main.titan");
        assert!(!ok && line.as_bytes() == b"quitar main.titan: no");
        assert!(why.unwrap().as_bytes().ends_with(b"empieza el paquete"));
    }
}
