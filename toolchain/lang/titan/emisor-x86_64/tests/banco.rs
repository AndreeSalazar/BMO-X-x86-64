//! THE BENCH THAT RUNS (TITAN_MAESTRO 14.14): every `BIEN` program of a level
//! is BUILT to a `.bex`, loaded the way the kernel's loader loads it, RUN in
//! the emulator, and its console compared with what the example says it
//! prints --
//!
//! ```text
//!    # espera: BIEN
//!    # sale: hola            one line per line the console must show
//! ```
//!
//! -- so a level is "done" when its programs DO what they say, not when they
//! compile. The NO programs are the frontend's bench
//! (`toolchain/lang/titan/tests/banco.rs`); here a NO must write nothing.

use bmo_lower::emu::{cargar_bex, run};
use std::path::Path;

fn examples(level: &str) -> Vec<(String, String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("ejemplos").join(level);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the bench folder") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("titan") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let want = src.lines().next().and_then(|l| l.strip_prefix("# espera: ")).unwrap().trim().to_string();
        out.push((path.file_name().unwrap().to_string_lossy().into_owned(), want, src));
    }
    out.sort();
    out
}

/// What the example says the console shows: its `# sale:` lines, in order.
fn says(src: &str) -> String {
    src.lines().filter_map(|l| l.strip_prefix("# sale: ")).map(|l| format!("{}\n", l)).collect()
}

#[test]
fn every_bien_program_of_every_level_runs_and_prints_what_it_says() {
    for level in ["nivel0", "nivel1", "nivel2"] {
        let ran = run_level(level);
        assert!(ran >= 2, "{} has programs that run", level);
    }
}

fn run_level(level: &str) -> usize {
    let mut ran = 0;
    for (name, want, src) in examples(level) {
        let built = bmo_titan_x86_64::build(&src, &name);
        if want != "BIEN" {
            assert!(built.is_err(), "{}: a NO program must write no .bex", name);
            continue;
        }
        let bex = built.unwrap_or_else(|e| panic!("{}: {:?}", name, e));
        let expected = says(&src);
        assert!(!expected.is_empty(), "{}: a BIEN program says what it prints (`# sale:`)", name);
        let m = run(cargar_bex(&bex).unwrap_or_else(|e| panic!("{}: {}", name, e)), 100_000);
        assert!(m.exited, "{}: the program did not reach EXIT", name);
        assert_eq!(m.console, expected, "{}: the console", name);
        ran += 1;
    }
    ran
}

/// ** The same source gives the same bytes: the `.bex` can be audited and
/// rebuilt (U4 will lean on this).
#[test]
fn the_same_source_gives_the_same_bex() {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../ejemplos/nivel0/hola.titan")).unwrap();
    assert_eq!(bmo_titan_x86_64::build(&src, "hola.titan").unwrap(), bmo_titan_x86_64::build(&src, "hola.titan").unwrap());
}
