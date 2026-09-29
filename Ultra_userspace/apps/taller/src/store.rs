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
use bmo_titan_lector::hang::{self, HangError, Plan, Sink};
use bmo_titan_lector::library::{self, Library};
use bmo_titan_lector::{read_package_into, seed, Fetch, Loaded, Source};
use bmo_userland as bmo;

/// Big enough for any manifest or header F1 reads; lives only while reading.
const READ_BUF: usize = 4096;

/// ESTRATOS through `Archivo::leer_de` (the kernel looks in ESTRATOS first).
struct Estratos;

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
}

impl Source for Disk<'_> {
    fn fetch(&mut self, path: &[u8], buf: &mut [u8]) -> Fetch {
        Estratos.fetch(path, buf)
    }
}

impl Sink for Disk<'_> {
    fn store(&mut self, path: &[u8], bytes: &[u8]) -> bool {
        let at = (bytes.as_ptr() as usize).wrapping_sub(self.block.base() as usize);
        if at + bytes.len() > HANG_BUF {
            // Not in the block: refused, never copied from somewhere unknown.
            return false;
        }
        bmo::estratos::guardar_desde(path, self.block.handle(), at as u64, bytes.len() as u64) != 0
    }
}

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
}

impl Store {
    pub fn open() -> Store {
        let mut s = Store { origin: Origin::Memory(""), library: None, chosen: 0, loaded: Loaded::new(), note: None };
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
        hang::hang(&mut Disk { block: &block }, root.as_bytes(), &self.loaded, p, text, out)
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
            }
            None => {
                self.loaded.show(sample::asteroids_graph());
                self.origin = Origin::Memory("titan/biblioteca.toml no lista ningun paquete");
            }
        }
    }
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
