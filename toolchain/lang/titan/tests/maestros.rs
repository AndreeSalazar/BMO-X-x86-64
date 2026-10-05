//! **WHAT THE TAB PLACES, COMPILES** (`PLAN_TALLER` 8.15): every master node
//! of F1's TAB (`bmo_titan_lector::maestros`) is put into the seed package
//! (`asteroids`) the way F1 puts it -- its file with the header renamed, its
//! `[layout]` row, `mod <name>` in main -- and the whole package goes through
//! the WHOLE frontend: tree, names, the checker and the calculation.
//!
//! The bench already proves each master compiles ALONE; this proves it still
//! does as a MODULE of someone else's package. If a master ever needs more
//! than the TAB writes (a `use`, a permission the seed lacks), it fails here,
//! with its name, before the owner presses Enter on it.

use bmo_titan_front::lower_package;
use bmo_titan_lector::hang::Sink;
use bmo_titan_lector::maestros::{self, MASTERS};
use bmo_titan_lector::organize::Disk;
use bmo_titan_lector::{read_package, Fetch, Source};
use std::collections::BTreeMap;

const ROOT: &str = "titan/asteroids";

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

fn seed() -> Mem {
    Mem(bmo_titan_lector::seed::FILES.iter().map(|(p, t)| (p.as_bytes().to_vec(), t.as_bytes().to_vec())).collect())
}

#[test]
fn every_master_placed_in_the_seed_still_compiles() {
    // The seed alone compiles: what fails below is the master's.
    let mut d = seed();
    let compile = |d: &Mem| {
        let text = |p: &str| d.0.get(format!("{ROOT}/{p}").as_bytes()).map(|b| String::from_utf8(b.clone()).unwrap());
        lower_package("src/main.titan", &text("src/main.titan").unwrap(), &mut |p| text(p))
    };
    compile(&d).unwrap_or_else(|m| panic!("the seed itself: {:?} {}", m.code, m.what));
    for m in MASTERS.iter() {
        let mut d2 = Mem(d.0.clone());
        let mut buf = [0u8; 4096];
        let l = read_package(&mut d2, ROOT.as_bytes(), &mut buf);
        let (mut a, mut b) = (vec![0u8; 16384], vec![0u8; 16384]);
        let p = maestros::place(&mut d2, ROOT.as_bytes(), &l, m, 0, 0, &mut a, &mut b).unwrap_or_else(|e| panic!("nivel{}/{}: not placed: {e:?}", m.level, m.name));
        if let Err(e) = compile(&d2) {
            panic!("nivel{}/{} placed as `{}` does not compile: {:?} {} ({:?})", m.level, m.name, String::from_utf8_lossy(p.name.as_bytes()), e.code, e.what, e.file);
        }
    }
    // and two at once, the second with its number: still one package
    let mut buf = [0u8; 4096];
    for m in [MASTERS.iter().find(|m| m.level == 3 && m.name == "semaforo").unwrap(), MASTERS.iter().find(|m| m.level == 8 && m.name == "semaforo").unwrap()] {
        let l = read_package(&mut d, ROOT.as_bytes(), &mut buf);
        let (mut a, mut b) = (vec![0u8; 16384], vec![0u8; 16384]);
        maestros::place(&mut d, ROOT.as_bytes(), &l, m, 0, 0, &mut a, &mut b).unwrap();
    }
    assert!(d.0.contains_key(format!("{ROOT}/src/semaforo_2.titan").as_bytes()));
    compile(&d).unwrap_or_else(|m| panic!("semaforo + semaforo_2: {:?} {}", m.code, m.what));
}

/// What the node in F1 says it prints (`titan_lector::traits::Said`, the
/// PRINTER on the node) is what the compiler prints when it runs the
/// program: the reader only says it when every argument is a written text,
/// and then the two agree -- for the seed's `hola` and for every master.
#[test]
fn what_the_printer_on_the_node_says_is_what_the_program_prints() {
    use bmo_titan_front::ir::{Op, Value};
    let printed = |src: &str| -> Vec<String> {
        let m = lower_package("src/main.titan", src, &mut |_| None).unwrap_or_else(|e| panic!("{:?} {}", e.code, e.what));
        m.flat
            .unwrap()
            .iter()
            .filter_map(|op| match op {
                Op::Write { parts, .. } => Some(parts.iter().map(|p| if let Value::Text(t, _) = p { t.clone() } else { String::from("\u{0}") }).collect()),
                _ => None,
            })
            .collect()
    };
    let hola = bmo_titan_lector::seed::FILES.iter().find(|f| f.0 == "titan/hola/src/main.titan").unwrap().1;
    assert_eq!(printed(hola), ["hola mundo"]);
    let mut checked = 0;
    for src in MASTERS.iter().filter(|m| m.asks == bmo_titan_contrato::Permissions::NONE).map(|m| m.source).chain([hola]) {
        let said = bmo_titan_lector::traits::scan(src.as_bytes()).says;
        if !said.exact {
            continue;
        }
        let said = String::from_utf8(said.as_bytes().to_vec()).unwrap();
        let lines = printed(src);
        assert!(lines.iter().any(|l| if_cut(l, &said)), "the node says `{said}` and the program prints {lines:?}");
        checked += 1;
    }
    // hola, and the masters that print a written text first (most print what
    // they calculate: there the node shows the arguments as written).
    assert!(checked >= 3, "the printer showed an exact text only {checked} times");
}

fn if_cut(line: &str, said: &str) -> bool {
    line == said || (said.len() == bmo_titan_lector::traits::SAID && line.starts_with(said))
}
