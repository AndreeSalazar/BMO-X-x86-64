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

use bmo_titan_contrato::sample;
use bmo_titan_lector::library::{self, Library};
use bmo_titan_lector::{read_package, seed, Fetch, Loaded, Source};
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
}

impl Store {
    pub fn open() -> Store {
        let mut s = Store { origin: Origin::Memory(""), library: None, chosen: 0, loaded: Loaded::from_graph(sample::asteroids_graph()) };
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
            self.read_all();
        }
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
                self.loaded = read_package(&mut Estratos, path.as_bytes(), &mut buf);
                self.origin = Origin::Estratos(g);
            }
            None => {
                self.loaded = Loaded::from_graph(sample::asteroids_graph());
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
