//! **THE MASTER NODES** -- what the TAB of F1 offers (`PLAN_TALLER` 8.15, the
//! owner on 05-10: "NODOS maestro ya hechos como ejemplos y porque, como
//! tutoriales y pruebas", the Houdini and Blender way).
//!
//! ```text
//!    the node        a program of the TITAN++ bench (`maestros_gen.rs`,
//!                    written by `toolchain/tools/maestros`, never by hand)
//!    its family      its LEVEL: the TAB, in order, IS the course
//!    filter          what is typed, against its name, its family, what it
//!                    says, its why and its words ("match", "dinero", "3060")
//!    place           `src/<name>.titan` with its header renamed, its row in
//!                    `[layout]` where the mouse was, and `mod <name>` in main
//! ```
//!
//! ** Why it cannot lie: the bench IS the catalogue. Every master is a program
//! the compiler accepts and both benches RUN on every build, comparing its
//! `# sale:` lines; its guardian (`maestros.py --check`) fails the build if
//! the table and the bench say different things. And what is placed compiles:
//! `toolchain/lang/titan/tests/maestros.rs` puts every master into the seed
//! package and compiles it.
//!
//! The order of the three writes is on purpose: the file first, `main` last.
//! Cut halfway, what is left is a file nobody declares (harmless, the EXPLORER
//! shows it), never a `mod` that points at nothing (T0083).

use crate::edit::{self, EditError};
use crate::organize::Disk;
use crate::package::{Fetch, Loaded, Say, MAIN};
use crate::text::{lines, trim, Path};
use bmo_titan_contrato::{Line, Name, Permission, Permissions, Text};

pub use crate::maestros_gen::{COUNT, FAMILIES, MASTERS};

/// One master node: a program of the bench, as the TAB shows it.
pub struct Master {
    /// Its file in the bench, without `.titan`: also the module it becomes.
    pub name: &'static str,
    /// Its level: the family it is listed under.
    pub level: u8,
    /// Why it exists: the comment right before its `mod main`.
    pub why: &'static str,
    /// What it does: the text of its `mod main "..."`.
    pub says: &'static str,
    /// The words of TITAN++ it uses, in the order of the ladder.
    pub words: &'static [&'static str],
    /// What its `Titan.toml` asks for: the package it goes into must allow it.
    pub asks: Permissions,
    /// Its proof: what it prints when it runs, line by line.
    pub out: &'static [&'static str],
    /// The program, without the bench's `# espera` line.
    pub source: &'static str,
}

impl Master {
    /// The name of its family: the title of its level in GRAMATICA.md.
    pub fn family(&self) -> &'static str {
        FAMILIES.get(self.level as usize).copied().unwrap_or("?")
    }

    /// Does what was typed name it? Empty names everything; case does not
    /// matter, and the level number counts too ("8").
    pub fn matches(&self, query: &[u8]) -> bool {
        let q = trim(query);
        if q.is_empty() {
            return true;
        }
        let mut level = [0u8; 2];
        let lv = digits(self.level, &mut level);
        q == lv
            || [self.name, self.family(), self.says, self.why].iter().any(|f| contains(f.as_bytes(), q))
            || self.words.iter().any(|w| w.as_bytes().eq_ignore_ascii_case(q))
    }
}

fn digits(v: u8, out: &mut [u8; 2]) -> &[u8] {
    if v >= 10 {
        *out = [b'0' + v / 10, b'0' + v % 10];
        &out[..]
    } else {
        out[0] = b'0' + v;
        &out[..1]
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    needle.len() <= hay.len() && hay.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
}

/// The masters that `query` names, in the order of the course: their indices
/// in `MASTERS`, into `out`. How many.
pub fn filter(query: &[u8], out: &mut [u8]) -> usize {
    let mut n = 0;
    for (i, m) in MASTERS.iter().enumerate() {
        if n < out.len() && m.matches(query) {
            out[n] = i as u8;
            n += 1;
        }
    }
    n
}

const _: () = assert!(COUNT <= u8::MAX as usize, "filter keeps indices in u8");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaceError {
    /// The package has no `src/main.titan` to declare it in.
    NoMain,
    /// The master asks for something the package's `Titan.toml` does not.
    Asks(Permission),
    /// `name`, `name_2` ... `name_9`: all taken.
    NoName,
    /// The package already has as many nodes as a graph can hold.
    Full,
    TooBig,
    Read,
    Write,
    Edit(EditError),
}

/// Where a master would go: the module name it gets (its own, or with `_2`
/// if that one is taken) and its file. Touches no disk but to ask whether the
/// file is free.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Placement {
    pub name: Name,
    pub file: Path,
}

/// The name the master gets in this package, and its file.
pub fn plan<D: Disk>(io: &mut D, root: &[u8], l: &Loaded, m: &Master) -> Result<Placement, PlaceError> {
    if !l.files().iter().any(|f| f.path.as_bytes() == MAIN) {
        return Err(PlaceError::NoMain);
    }
    for p in Permission::ALL {
        if m.asks.allows(p) && !l.graph.permissions.allows(p) {
            return Err(PlaceError::Asks(p));
        }
    }
    if l.graph.nodes().len() >= bmo_titan_contrato::MAX_NODES {
        return Err(PlaceError::Full);
    }
    let mut probe = [0u8; 1];
    for k in 1..10u8 {
        let mut b = [0u8; 24];
        let base = m.name.as_bytes();
        let mut n = base.len().min(21);
        b[..n].copy_from_slice(&base[..n]);
        if k > 1 {
            b[n] = b'_';
            b[n + 1] = b'0' + k;
            n += 2;
        }
        let name: Name = Text::new(core::str::from_utf8(&b[..n]).unwrap_or("?"));
        // Taken by a node of the package -- or by the program itself: a
        // module may not share its name with one of its own (`let area` in a
        // module `area` is T0055), so that one is `area_2`.
        if l.graph.find(name.as_bytes()).is_some() || uses_word(m.source.as_bytes(), name.as_bytes()) {
            continue;
        }
        let file = Path::new(&[b"src/", name.as_bytes(), b".titan"]).ok_or(PlaceError::TooBig)?;
        let full = Path::new(&[root, b"/", file.as_bytes()]).ok_or(PlaceError::TooBig)?;
        if matches!(io.fetch(full.as_bytes(), &mut probe), Fetch::Missing) {
            return Ok(Placement { name, file });
        }
    }
    Err(PlaceError::NoName)
}

/// Does the program use `word` as a name? Comments and texts do not count.
fn uses_word(src: &[u8], word: &[u8]) -> bool {
    let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    for (_, line) in lines(src) {
        let (mut i, mut text) = (0, false);
        while i < line.len() {
            let c = line[i];
            if text {
                if c == b'\\' {
                    i += 1;
                } else if c == b'"' {
                    text = false;
                }
            } else if c == b'"' {
                text = true;
            } else if c == b'#' {
                break;
            } else if is_id(c) && (i == 0 || !is_id(line[i - 1])) {
                let end = line[i..].iter().position(|&c| !is_id(c)).map_or(line.len(), |k| i + k);
                if &line[i..end] == word {
                    return true;
                }
                i = end;
                continue;
            }
            i += 1;
        }
    }
    false
}

/// The master's text with its header renamed: `mod main "..."` becomes
/// `mod <name> "..."`, every other byte as it was. The length written.
pub fn renamed(m: &Master, name: &[u8], out: &mut [u8]) -> Result<usize, PlaceError> {
    let src = m.source.as_bytes();
    let at = src.windows(9).position(|w| w == b"mod main ").filter(|&i| i == 0 || src[i - 1] == b'\n').ok_or(PlaceError::Edit(EditError::NoHeader))?;
    let mut n = 0;
    for part in [&src[..at], b"mod ", name, b" ", &src[at + 9..]] {
        out.get_mut(n..n + part.len()).ok_or(PlaceError::TooBig)?.copy_from_slice(part);
        n += part.len();
    }
    Ok(n)
}

/// `Titan.toml` with `name = [x, y]` as the last row of `[layout]` (the
/// section is made at the end if there is none). The length written.
pub fn with_position(toml: &[u8], name: &[u8], x: i32, y: i32, out: &mut [u8]) -> Result<usize, PlaceError> {
    // Where the row goes: after the last `key = value` of [layout].
    let mut inside = false;
    let mut after = None;
    let mut at = 0;
    for (_, line) in lines(toml) {
        let end = toml[at..].iter().position(|&c| c == b'\n').map_or(toml.len(), |i| at + i + 1);
        let t = trim(line);
        if t.starts_with(b"[") {
            inside = t == b"[layout]";
            if inside {
                after = Some(end);
            }
        } else if inside && t.contains(&b'=') {
            after = Some(end);
        }
        at = end;
    }
    let mut row = Say::new().t(name).t(b" = [");
    for (i, v) in [x, y].into_iter().enumerate() {
        if i == 1 {
            row = row.t(b", ");
        }
        if v < 0 {
            row = row.t(b"-");
        }
        row = row.num(v.unsigned_abs() as usize);
    }
    let row = row.t(b"]").done();
    let (head, tail): (&[u8], &[u8]) = match after {
        Some(k) => (&toml[..k], &toml[k..]),
        None => (toml, b""),
    };
    let lead: &[u8] = match after {
        Some(_) if head.last() == Some(&b'\n') => b"",
        Some(_) => b"\n",
        None if toml.is_empty() || toml.ends_with(b"\n\n") => b"[layout]\n",
        None if toml.ends_with(b"\n") => b"\n[layout]\n",
        None => b"\n\n[layout]\n",
    };
    let mut n = 0;
    for part in [head, lead, row.as_bytes(), b"\n", tail] {
        out.get_mut(n..n + part.len()).ok_or(PlaceError::TooBig)?.copy_from_slice(part);
        n += part.len();
    }
    Ok(n)
}

/// **Places a master** in the package at `root`, at `(x, y)` of the canvas:
/// three writes (`vuelve 3` undoes them). `text` and `out` are the two halves
/// of the gesture's block, like every other gesture.
#[allow(clippy::too_many_arguments)]
pub fn place<D: Disk>(io: &mut D, root: &[u8], l: &Loaded, m: &Master, x: i32, y: i32, text: &mut [u8], out: &mut [u8]) -> Result<Placement, PlaceError> {
    let p = plan(io, root, l, m)?;
    let full = |rel: &[u8]| Path::new(&[root, b"/", rel]).ok_or(PlaceError::TooBig);
    let read = |io: &mut D, rel: &[u8], buf: &mut [u8]| match io.fetch(full(rel)?.as_bytes(), buf) {
        Fetch::Found(n) => Ok(n),
        Fetch::TooBig => Err(PlaceError::TooBig),
        Fetch::Missing => Err(PlaceError::Read),
    };
    // 1. the file: the program, as the bench runs it, under its new name
    let k = renamed(m, p.name.as_bytes(), out)?;
    if !io.create(full(p.file.as_bytes())?.as_bytes(), &out[..k]) {
        return Err(PlaceError::Write);
    }
    // 2. where it sits: a row of [layout]
    let n = read(io, b"Titan.toml", text)?;
    let k = with_position(&text[..n], p.name.as_bytes(), x, y, out)?;
    if !io.store(full(b"Titan.toml")?.as_bytes(), &out[..k]) {
        return Err(PlaceError::Write);
    }
    // 3. main declares it: from here on it is a module of the package
    let n = read(io, MAIN, text)?;
    let k = edit::add_child(&text[..n], p.name.as_bytes(), None, out).map_err(PlaceError::Edit)?;
    if !io.store(full(MAIN)?.as_bytes(), &out[..k]) {
        return Err(PlaceError::Write);
    }
    Ok(p)
}

/// What the owner reads: one line. Spanish, ASCII, no tilde.
pub fn note(r: Result<Placement, PlaceError>, master: &[u8]) -> (Line, bool) {
    let s = Say::new();
    let s = match r {
        Ok(p) => s.t(b"puesto ").t(p.name.as_bytes()).t(b"; vuelve 3 lo deshace"),
        Err(PlaceError::NoMain) => s.t(b"el paquete no tiene src/main.titan donde declararlo"),
        Err(PlaceError::Asks(p)) => s.t(master).t(b" pide ").t(p.key().as_bytes()).t(b": dalo en [permissions] de Titan.toml"),
        Err(PlaceError::NoName) => s.t(b"ya hay ").t(master).t(b" hasta el _9: renombra alguno"),
        Err(PlaceError::Full) => s.t(b"el paquete ya tiene todos los nodos que caben"),
        Err(PlaceError::TooBig) => s.t(b"no cabe en el buffer del taller"),
        Err(PlaceError::Read) => s.t(b"no pude leer el paquete en ESTRATOS"),
        Err(PlaceError::Write) => s.t(b"ESTRATOS no lo guardo (CABINA, F11, dice por que)"),
        Err(PlaceError::Edit(_)) => s.t(b"la cabecera de main no se dejo reescribir"),
    };
    (s.done(), r.is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hang::Sink;
    use crate::package::{read_package, Source};
    use std::collections::BTreeMap;
    use std::string::String;
    use std::vec::Vec;

    /// ESTRATOS in memory, like `organize::tests`: `create` never overwrites.
    #[derive(Default)]
    struct Mem(BTreeMap<Vec<u8>, Vec<u8>>);

    impl Source for Mem {
        fn fetch(&mut self, path: &[u8], buf: &mut [u8]) -> Fetch {
            match self.0.get(path) {
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
            self.0.insert(path.to_vec(), bytes.to_vec());
            true
        }
    }

    impl Disk for Mem {
        fn make_folder(&mut self, _: &[u8]) -> bool {
            true
        }
        fn create(&mut self, path: &[u8], bytes: &[u8]) -> bool {
            !self.0.contains_key(path) && self.store(path, bytes)
        }
        fn rename(&mut self, _: &[u8], _: &[u8]) -> bool {
            false
        }
        fn remove(&mut self, path: &[u8]) -> bool {
            self.0.remove(path).is_some()
        }
    }

    const ROOT: &[u8] = b"titan/asteroids";

    fn seed() -> Mem {
        Mem(crate::seed::FILES.iter().map(|(p, t)| (p.as_bytes().to_vec(), t.as_bytes().to_vec())).collect())
    }

    fn text(m: &Mem, p: &str) -> String {
        String::from_utf8(m.0[p.as_bytes()].clone()).unwrap()
    }

    fn master(level: u8, name: &str) -> &'static Master {
        MASTERS.iter().find(|m| m.level == level && m.name == name).unwrap()
    }

    fn put(d: &mut Mem, m: &Master, x: i32, y: i32) -> Result<Placement, PlaceError> {
        let mut buf = [0u8; 4096];
        let l = read_package(d, ROOT, &mut buf);
        let (mut a, mut b) = (std::vec![0u8; 8192], std::vec![0u8; 8192]);
        place(d, ROOT, &l, m, x, y, &mut a, &mut b)
    }

    /// The TAB shows the whole course: every level has its family name and
    /// at least one master, in order, each with its why and its proof.
    #[test]
    fn every_family_has_masters_and_every_master_its_why_and_its_proof() {
        let last = MASTERS.iter().map(|m| m.level).max().unwrap();
        assert_eq!(FAMILIES.len(), last as usize + 1);
        for lv in 0..=last {
            assert!(MASTERS.iter().any(|m| m.level == lv), "the family of level {lv} is empty");
        }
        assert!(MASTERS.windows(2).all(|w| w[0].level <= w[1].level), "the TAB goes in the order of the course");
        for m in MASTERS.iter() {
            assert!(crate::text::is_name(m.name.as_bytes()), "{}", m.name);
            assert!(!m.why.is_empty() && !m.says.is_empty() && !m.out.is_empty(), "{}", m.name);
            assert!(m.source.contains("\nmod main \"") || m.source.starts_with("mod main \""), "{}", m.name);
            assert!(!m.source.contains("# espera"), "{}: the bench's line stays in the bench", m.name);
            assert!(m.words.contains(&"fn"), "{}", m.name);
        }
        assert_eq!(master(0, "hola").family(), "un programa que saluda");
        assert_eq!(master(11, "mezcla").family(), "la 3060");
    }

    #[test]
    fn typing_filters_by_name_family_why_and_words() {
        let mut out = [0u8; COUNT];
        let all = filter(b"", &mut out);
        assert_eq!(all, MASTERS.len());
        let n = filter(b"match", &mut out);
        let got: Vec<&Master> = out[..n].iter().map(|&i| &MASTERS[i as usize]).collect();
        assert!(MASTERS.iter().filter(|m| m.level == 8).all(|m| got.iter().any(|g| core::ptr::eq(*g, m))), "every node of level 8 uses match");
        assert!(got.iter().all(|m| m.level >= 8), "nothing before level 8 says match");
        let n = filter(b"DINERO", &mut out);
        assert!(out[..n].iter().any(|&i| MASTERS[i as usize].name == "factura"), "case does not matter");
        let n = filter(b"3060", &mut out);
        assert!(n >= 2 && out[..n].iter().all(|&i| MASTERS[i as usize].level >= 9));
        let n = filter(b"8", &mut out);
        assert!(out[..n].iter().all(|&i| MASTERS[i as usize].level == 8 || MASTERS[i as usize].why.contains('8')));
        assert_eq!(filter(b"nada de esto existe", &mut out), 0);
    }

    #[test]
    fn placing_writes_the_file_the_row_and_the_mod_and_reads_back_as_a_node() {
        let mut d = seed();
        let p = put(&mut d, master(8, "semaforo"), 700, 620).unwrap();
        assert_eq!(p.name.as_bytes(), b"semaforo");
        let file = text(&d, "titan/asteroids/src/semaforo.titan");
        assert!(file.contains("\nmod semaforo \"un semaforo de tres colores\"\n"), "{file}");
        assert!(!file.contains("mod main"));
        assert!(text(&d, "titan/asteroids/src/main.titan").contains("\nmod semaforo\n"));
        let toml = text(&d, "titan/asteroids/Titan.toml");
        assert!(toml.contains("director = [60, 460]\nsemaforo = [700, 620]\n"), "{toml}");
        let mut buf = [0u8; 4096];
        let l = read_package(&mut d, ROOT, &mut buf);
        assert!(l.problems().is_empty());
        let id = l.graph.find(b"semaforo").unwrap();
        assert_eq!((l.graph.node(id).unwrap().x, l.graph.node(id).unwrap().y), (700, 620));
    }

    #[test]
    fn a_name_already_there_gets_its_number() {
        let mut d = seed();
        put(&mut d, master(3, "semaforo"), 0, 0).unwrap();
        // the same name, from another level: the second is `semaforo_2`
        assert_eq!(put(&mut d, master(8, "semaforo"), 10, -5).unwrap().name.as_bytes(), b"semaforo_2");
        assert!(text(&d, "titan/asteroids/Titan.toml").contains("semaforo_2 = [10, -5]\n"));
        // a file on the disk that no module declares also takes the name
        d.0.insert(b"titan/asteroids/src/primos.titan".to_vec(), b"x".to_vec());
        assert_eq!(put(&mut d, master(4, "primos"), 0, 0).unwrap().name.as_bytes(), b"primos_2");
    }

    #[test]
    fn a_name_the_program_uses_inside_is_taken_too() {
        let mut d = seed();
        // `let area = ...` inside: a module `area` would clash with it
        assert_eq!(put(&mut d, master(1, "area"), 0, 0).unwrap().name.as_bytes(), b"area_2");
        assert!(uses_word(b"mod main \"area\"\n# area\nfn f()\n    print(\"area\")\n    let areas = 1\n", b"area") == false);
        assert!(uses_word(b"fn f()\n    let x = area + 1\n", b"area"));
    }

    #[test]
    fn what_the_master_asks_for_the_package_must_allow() {
        let mut d = seed();
        let toml = text(&d, "titan/asteroids/Titan.toml").replace("gpu = \"compute\"", "gpu = false");
        d.0.insert(b"titan/asteroids/Titan.toml".to_vec(), toml.into_bytes());
        assert_eq!(put(&mut d, master(11, "mezcla"), 0, 0), Err(PlaceError::Asks(Permission::Gpu)));
        assert!(!d.0.contains_key(&b"titan/asteroids/src/mezcla.titan"[..]), "refused before a byte is written");
        let (line, ok) = note(Err(PlaceError::Asks(Permission::Gpu)), b"mezcla");
        assert!(!ok);
        assert_eq!(line.as_bytes(), b"mezcla pide gpu: dalo en [permissions] de Titan.toml");
    }

    #[test]
    fn a_manifest_without_layout_gets_one() {
        let mut out = [0u8; 256];
        let n = with_position(b"[package]\nname = \"x\"\n", b"a", 1, 2, &mut out).unwrap();
        assert_eq!(&out[..n], b"[package]\nname = \"x\"\n\n[layout]\na = [1, 2]\n");
        let n = with_position(b"[layout]\nb = [3, 4]\n\n[explorer]\n", b"a", 5, 6, &mut out).unwrap();
        assert_eq!(&out[..n], b"[layout]\nb = [3, 4]\na = [5, 6]\n\n[explorer]\n");
    }
}
