//! **THE STORE** -- the library in ESTRATOS: where it is, seeding it once, and
//! reading it again the moment it changes (`PLAN_TALLER` 8.6).
//!
//! ```text
//!    ESTRATOS mounted    titan/biblioteca.toml -> the packages; the chosen one
//!                        read with titan-lector, following `mod`
//!    no library yet      F1 SEEDS `asteroids` (titan-lector::seed): every
//!                        write is a commit, so the seed is in the history too
//!    no ESTRATOS         the hand-written sample, in memory, and it says so
//! ```
//!
//! ** REAL TIME, for one number. In ESTRATOS writing IS committing, and every
//! commit raises `INFO_ES_GENERACION`. `refresh` asks that number once per
//! loop (one syscall) and reads the files again only when it moved: a file
//! renamed from F12, a `vuelve` to yesterday -- F1 shows it within a beat.
//!
//! It never walks ESTRATOS with the kernel's cursor: that cursor is ONE and it
//! is the F12 panel's; walking it from here would move what F12 shows.
//!
//! ** WRITING (8.7): hanging a file under another rewrites two headers
//! (`titan-lector::hang`) and saves them as new versions. It does not read
//! the package again itself: the generation moved, so the next beat does,
//! and F1 sees its own change exactly as anyone else's.

use bmo_titan_contrato::{sample, Line, NodeId};
use bmo_titan_lector::explorer::{Lister, Tree};
use bmo_titan_lector::hang::{self, HangError, Plan, Sink};
use bmo_titan_lector::organize::{self, At, Disk as Organize, Done, OrgError};
use bmo_titan_lector::library::{self, Library};
use bmo_titan_lector::{read_package_into, seed, Fetch, Loaded, Source};
use bmo_userland as bmo;

/// Big enough for any manifest or header F1 reads; lives only while reading.
const READ_BUF: usize = 4096;

/// ESTRATOS through `Archivo::leer_de` (the kernel looks in ESTRATOS first).
pub(crate) struct Estratos;

/// ** What a folder holds, through the kernel's DIRECTORIO object (01-10): a
/// handle of THIS process, so walking the package never moves the cursor of
/// F12 (the warning at the top of this file still holds). Only what is in
/// ESTRATOS: a folder of the same name on FAT32 (`titan/` holds the `.bex`)
/// is listed by the same object and skipped here.
struct Shelf;

impl Lister for Shelf {
    fn list(&mut self, folder: &[u8], put: &mut dyn FnMut(&[u8], bool, u64)) -> bool {
        let Ok(d) = bmo::Directorio::open(folder) else { return false };
        let mut name = [0u8; 64];
        while let Some((k, is_folder, bytes, in_estratos)) = d.siguiente_con_origen(&mut name) {
            if in_estratos {
                put(&name[..k], is_folder, bytes);
            }
        }
        true
        // `d` closes here (Drop): the walk keeps ONE of the eight open.
    }
}

impl Source for Estratos {
    fn fetch(&mut self, path: &[u8], buf: &mut [u8]) -> Fetch {
        let Ok(f) = bmo::Archivo::leer_de(path) else { return Fetch::Missing };
        let size = f.size() as usize;
        if size > buf.len() {
            return Fetch::TooBig;
        }
        Fetch::Found(f.read(&mut buf[..size]))
        // `f` closes itself here (Drop): reading often costs no handles.
    }
}

/// ESTRATOS with a borrowed block behind it: the two halves a hang reads a
/// header into and rewrites it into. The rewritten header is already where
/// `guardar_desde` takes it from, so saving copies nothing.
struct Disk<'m> {
    block: &'m bmo::Memoria,
    /// How many bytes of the block are ours to hand to the kernel.
    len: usize,
}

impl Disk<'_> {
    /// Where `bytes` starts inside the block, if it is all inside it.
    fn offset(&self, bytes: &[u8]) -> Option<u64> {
        let at = (bytes.as_ptr() as usize).wrapping_sub(self.block.base() as usize);
        (at + bytes.len() <= self.len).then_some(at as u64)
    }
}

impl Source for Disk<'_> {
    fn fetch(&mut self, path: &[u8], buf: &mut [u8]) -> Fetch {
        Estratos.fetch(path, buf)
    }
}

impl Sink for Disk<'_> {
    fn store(&mut self, path: &[u8], bytes: &[u8]) -> bool {
        // Not in the block: refused, never copied from somewhere unknown.
        let Some(at) = self.offset(bytes) else { return false };
        bmo::estratos::guardar_desde(path, self.block.handle(), at, bytes.len() as u64) != 0
    }
}

/// The verbs of the EXPLORER (`PLAN_TALLER` 8.10), each ONE gesture of
/// ESTRATOS: every one is a version, and `vuelve` undoes it.
impl Organize for Disk<'_> {
    fn make_folder(&mut self, path: &[u8]) -> bool {
        bmo::estratos::crear_carpeta(path) != 0
    }
    fn create(&mut self, path: &[u8], bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            // An empty file goes by the line: a block of zero bytes is no block.
            return bmo::estratos::crear_fichero(path, b"") != 0;
        }
        let Some(at) = self.offset(bytes) else { return false };
        bmo::estratos::crear_desde(path, self.block.handle(), at, bytes.len() as u64) != 0
    }
    fn rename(&mut self, path: &[u8], new: &[u8]) -> bool {
        bmo::estratos::renombrar(path, new) != 0
    }
    fn remove(&mut self, path: &[u8]) -> bool {
        bmo::estratos::quitar(path) != 0
    }
}

/// The block of one EXPLORER gesture: two halves, and the bigger one is what a
/// moved file can measure (it travels through it whole).
const GESTURE_HALF: usize = 64 * 1024;

/// A header read in, and rewritten: two halves of one block.
const HANG_BUF: usize = 2 * READ_BUF;

/// Where the package on screen came from.
#[derive(Clone, Copy, PartialEq)]
pub enum Origin {
    /// ESTRATOS, at this generation.
    Estratos(u64),
    /// In memory, and why.
    Memory(&'static str),
}

pub struct Store {
    pub origin: Origin,
    library: Option<Library>,
    /// Which package of the library is on screen.
    pub chosen: usize,
    pub loaded: Loaded,
    /// What the last drop did, or why not: one line for the EXPLORER, and
    /// whether it was done.
    pub note: Option<(Line, bool)>,
    /// ** THE PACKAGE AS IT IS ON DISK, in the owner's order (`PLAN_TALLER`
    /// 8.10). In a borrowed block, not on the stack (~8 KiB of a 64 KiB stack
    /// that is already at 55): `None` if there was no memory for it, and the
    /// column says so.
    pub tree: Option<&'static mut Tree>,
    /// The second line of a note: what a gesture left behind, or why not.
    pub detail: Option<Line>,
}

impl Store {
    pub fn open() -> Store {
        let mut s = Store { origin: Origin::Memory(""), library: None, chosen: 0, loaded: Loaded::new(), note: None, tree: tree_block(), detail: None };
        s.loaded.show(sample::asteroids_graph());
        if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
            s.origin = Origin::Memory("ESTRATOS no esta montado: ejemplo en memoria");
            return s;
        }
        if library_text().is_none() {
            if let Err(path) = seed_library() {
                bmo::consola("TALLER: no pude sembrar ");
                bmo::consola(path);
                bmo::consola(" en ESTRATOS\n");
                s.origin = Origin::Memory("no se pudo sembrar ESTRATOS: ejemplo en memoria");
                return s;
            }
            bmo::consola("TALLER: sembre titan/asteroids en ESTRATOS\n");
        }
        s.read_all();
        s
    }

    /// Reads again if ESTRATOS changed since the last read. `true` if it did.
    pub fn refresh(&mut self) -> bool {
        match self.origin {
            Origin::Estratos(g) if g != generation() => {
                self.read_all();
                true
            }
            _ => false,
        }
    }

    /// Puts another package of the library on screen.
    pub fn choose(&mut self, i: usize) {
        if i < self.packages().len() && i != self.chosen {
            self.chosen = i;
            self.note = None;
            self.read_all();
        }
    }

    /// Could `child` hang under `parent`? Touches no disk: the EXPLORER asks
    /// it while the button is still down, to colour the target.
    pub fn plan(&self, child: NodeId, parent: NodeId) -> Result<Plan, HangError> {
        hang::plan(&self.loaded, child, parent)
    }

    /// Hangs `child` under `parent` in ESTRATOS, and says how it went.
    pub fn hang(&mut self, child: NodeId, parent: NodeId) {
        let r = self.plan(child, parent).and_then(|p| self.write(&p));
        let name = |id| self.loaded.graph.node(id).map(|n| n.name).unwrap_or(bmo_titan_contrato::Text::new("?"));
        let done = r.is_ok();
        self.note = hang::note(r, name(child).as_bytes(), name(parent).as_bytes()).map(|l| (l, done));
        self.detail = None;
    }

    /// Could a cable be pulled from `from` to `to`? Touches no disk: asked
    /// while the cable is still in the hand, to paint it green or red.
    pub fn wire_plan(&self, from: NodeId, to: NodeId) -> Result<(), bmo_titan_lector::wire::WireError> {
        bmo_titan_lector::wire::plan(&self.loaded, from, to)
    }

    /// **A cable let go is a `use` written** (escalon 10, `PLAN_TALLER` 8.13):
    /// one header saved with one more name; the next beat reads it back.
    pub fn wire(&mut self, from: NodeId, to: NodeId) {
        let r = (|| {
            let root = self.root().ok_or(bmo_titan_lector::wire::WireError::Read)?;
            let block = bmo::Memoria::request(HANG_BUF as u64).ok_or(bmo_titan_lector::wire::WireError::Read)?;
            // SAFETY: as in `write`: our block, HANG_BUF bytes, two halves
            // that do not overlap, alive until the end of this closure.
            let all = unsafe { core::slice::from_raw_parts_mut(block.base(), HANG_BUF) };
            let (text, out) = all.split_at_mut(READ_BUF);
            bmo_titan_lector::wire::wire(&mut Disk { block: &block, len: HANG_BUF }, root.as_bytes(), &self.loaded, from, to, text, out)
        })();
        let name = |id| self.loaded.graph.node(id).map(|n| n.name).unwrap_or(bmo_titan_contrato::Text::new("?"));
        let done = r.is_ok();
        self.note = bmo_titan_lector::wire::note(r, name(from).as_bytes(), name(to).as_bytes()).map(|l| (l, done));
        self.detail = None;
    }

    /// Says one thing in the EXPLORER's note, with a second line if any.
    pub fn say(&mut self, line: Line, detail: Option<Line>, ok: bool) {
        self.note = Some((line, ok));
        self.detail = detail;
    }

    /// **One gesture of the EXPLORER**: a borrowed block for its two halves,
    /// the tree and what was read, and the note it leaves. It does not read
    /// the package again: the generation moved, and the next beat does.
    fn gesture(
        &mut self,
        what: &[u8],
        quiet: bool,
        f: impl FnOnce(&mut Disk, &At, &mut Tree, &mut [u8], &mut [u8]) -> Result<Done, OrgError>,
    ) {
        let r = (|| {
            let root = self.root().ok_or(OrgError::NoSuch)?;
            let tree = self.tree.as_deref_mut().ok_or(OrgError::NoSuch)?;
            let block = bmo::Memoria::request(2 * GESTURE_HALF as u64).ok_or(OrgError::TooBig)?;
            // SAFETY: `block` is ours, mapped and 2 * GESTURE_HALF bytes long,
            // and outlives both halves (it drops at the end of this closure);
            // the halves do not overlap, and the only other reader is the
            // kernel, inside the gestures that take them by offset.
            let all = unsafe { core::slice::from_raw_parts_mut(block.base(), 2 * GESTURE_HALF) };
            let (text, out) = all.split_at_mut(GESTURE_HALF);
            let at = At { root: root.as_bytes(), loaded: &self.loaded };
            f(&mut Disk { block: &block, len: 2 * GESTURE_HALF }, &at, tree, text, out)
        })();
        if quiet && r.is_ok() {
            return;
        }
        let (line, detail, ok) = organize::note(r, what);
        self.say(line, detail, ok);
    }

    /// Opens or closes folder `i`, and remembers it in Titan.toml.
    pub fn toggle(&mut self, i: usize) {
        if let Some(t) = self.tree.as_deref_mut() {
            t.toggle(i);
        }
        self.gesture(b"plegar", true, |io, at, t, a, b| organize::arrange(io, at, t, a, b));
    }

    /// `moving` before (or after) its sibling `target`: the owner's order.
    pub fn place(&mut self, moving: usize, target: usize, after: bool) {
        self.gesture(b"orden guardado en Titan.toml", false, |io, at, t, a, b| organize::place(io, at, t, moving, target, after, a, b));
    }

    pub fn create(&mut self, folder: Option<usize>, name: &[u8], is_folder: bool, declarer: Option<NodeId>) {
        let what = bmo_titan_lector::Say::new().t(if is_folder { b"carpeta nueva " } else { b"nuevo " }).t(name).done();
        self.gesture(what.as_bytes(), false, |io, at, t, a, b| organize::create(io, at, t, folder, name, is_folder, declarer, a, b));
    }

    pub fn rename(&mut self, i: usize, name: &[u8]) {
        let old = self.tree.as_deref().map(|t| bmo_titan_lector::Say::new().t(t.name(i)).done());
        let what = bmo_titan_lector::Say::new().t(old.as_ref().map_or(&b"?"[..], |o| o.as_bytes())).t(b" -> ").t(name).done();
        self.gesture(what.as_bytes(), false, |io, at, t, a, b| organize::rename(io, at, t, i, name, a, b));
    }

    pub fn remove(&mut self, i: usize) {
        let what = self.tree.as_deref().map(|t| bmo_titan_lector::Say::new().t(b"quitado ").t(t.name(i)).done());
        let what = what.unwrap_or(Line::new("quitar"));
        self.gesture(what.as_bytes(), false, |io, at, t, a, b| organize::remove(io, at, t, i, a, b));
    }

    pub fn move_into(&mut self, i: usize, folder: Option<usize>) {
        let what = self.tree.as_deref().map(|t| {
            let to = folder.map_or(&b"la raiz del paquete"[..], |f| t.name(f));
            bmo_titan_lector::Say::new().t(t.name(i)).t(b" -> ").t(to).done()
        });
        let what = what.unwrap_or(Line::new("mover"));
        self.gesture(what.as_bytes(), false, |io, at, t, a, b| organize::move_into(io, at, t, i, folder, a, b));
    }

    pub fn declare(&mut self, i: usize, parent: NodeId) {
        let pname = self.loaded.graph.node(parent).map(|n| n.name);
        let what = self.tree.as_deref().map(|t| {
            bmo_titan_lector::Say::new().t(pname.as_ref().map_or(&b"?"[..], |n| n.as_bytes())).t(b" declara ").t(t.name(i)).done()
        });
        let what = what.unwrap_or(Line::new("declarar"));
        self.gesture(what.as_bytes(), false, |io, at, t, a, b| organize::declare(io, at, t, i, parent, a, b));
    }

    fn write(&self, p: &Plan) -> Result<(), HangError> {
        let root = self.packages().get(self.chosen).map(|x| x.1).ok_or(HangError::Write)?;
        let block = bmo::Memoria::request(HANG_BUF as u64).ok_or(HangError::Write)?;
        // SAFETY: `block` is ours, mapped and HANG_BUF bytes long, and it
        // outlives both halves (it drops at the end of this function); the
        // halves do not overlap, and the only other user of these bytes is
        // the kernel, reading them inside `guardar_desde`.
        let all = unsafe { core::slice::from_raw_parts_mut(block.base(), HANG_BUF) };
        let (text, out) = all.split_at_mut(READ_BUF);
        hang::hang(&mut Disk { block: &block, len: HANG_BUF }, root.as_bytes(), &self.loaded, p, text, out)
    }

    pub fn packages(&self) -> &[(bmo_titan_contrato::Name, bmo_titan_lector::Path)] {
        self.library.as_ref().map(|l| l.packages()).unwrap_or(&[])
    }

    fn read_all(&mut self) {
        // The generation BEFORE reading: a change while reading is read next time.
        let g = generation();
        let mut buf = [0u8; READ_BUF];
        self.library = library_text_in(&mut buf).and_then(|n| library::parse(&buf[..n]).ok());
        self.chosen = self.chosen.min(self.packages().len().saturating_sub(1));
        match self.packages().get(self.chosen).map(|p| p.1) {
            Some(path) => {
                // In place: a `Loaded` is ~8 KiB, and a copy of it is stack.
                read_package_into(&mut Estratos, path.as_bytes(), &mut buf, &mut self.loaded);
                self.origin = Origin::Estratos(g);
                if let Some(t) = self.tree.as_deref_mut() {
                    t.fill(&mut Shelf, path.as_bytes());
                    // The owner's arrangement lives in Titan.toml.
                    if let Some(toml) = bmo_titan_lector::text::Path::new(&[path.as_bytes(), b"/Titan.toml"]) {
                        if let Fetch::Found(n) = Estratos.fetch(toml.as_bytes(), &mut buf) {
                            t.apply(&buf[..n]);
                        }
                    }
                }
            }
            None => {
                self.loaded.show(sample::asteroids_graph());
                self.origin = Origin::Memory("titan/biblioteca.toml no lista ningun paquete");
                if let Some(t) = self.tree.as_deref_mut() {
                    t.clear();
                }
            }
        }
    }

    /// The folder of the package on screen, from the root of the volume.
    pub fn root(&self) -> Option<bmo_titan_lector::Path> {
        self.packages().get(self.chosen).map(|p| p.1)
    }
}

/// A block for the tree. Every field of a `Tree` is an integer, so ANY bytes
/// are a valid one; `clear` makes it empty.
fn tree_block() -> Option<&'static mut Tree> {
    let block = bmo::Memoria::request(core::mem::size_of::<Tree>() as u64)?;
    // SAFETY: the block is ours, mapped, page-aligned (more than `Tree`'s
    // alignment) and at least `size_of::<Tree>()` bytes; a `Tree` is plain
    // integers, so whatever the block holds is a valid value; and the block is
    // never given back (`forget`), so the reference lives as long as F1.
    let t: &'static mut Tree = unsafe { &mut *(block.base() as *mut Tree) };
    core::mem::forget(block);
    t.clear();
    Some(t)
}

fn generation() -> u64 {
    bmo::info(bmo::INFO_ES_GENERACION)
}

fn library_text() -> Option<usize> {
    let mut buf = [0u8; READ_BUF];
    library_text_in(&mut buf)
}

fn library_text_in(buf: &mut [u8]) -> Option<usize> {
    match Estratos.fetch(seed::LIBRARY.as_bytes(), buf) {
        Fetch::Found(n) => Some(n),
        _ => None,
    }
}

/// Writes the seed: folders first, then the files, the index LAST (a seed cut
/// halfway is finished the next time, because the index is what F1 looks for).
/// `guardar_desde` creates or publishes a new version: nothing is lost either
/// way. `Err` names the path that failed.
fn seed_library() -> Result<(), &'static str> {
    let block = bmo::Memoria::request(READ_BUF as u64).ok_or("la memoria para sembrar")?;
    for folder in seed::FOLDERS {
        // It may already be there (a seed cut halfway): not an error.
        bmo::estratos::crear_carpeta(folder.as_bytes());
    }
    for (path, text) in seed::FILES {
        let bytes = text.as_bytes();
        if bytes.len() > READ_BUF {
            return Err(path);
        }
        // SAFETY: `block` is ours and READ_BUF bytes long; `bytes` fits (checked).
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), block.base(), bytes.len()) };
        if bmo::estratos::guardar_desde(path.as_bytes(), block.handle(), 0, bytes.len() as u64) == 0 {
            return Err(path);
        }
    }
    Ok(())
}
