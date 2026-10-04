//! THE BENCH of each level (TITAN_MAESTRO 14.14): every `.titan` in
//! `ejemplos/nivelN/` says in its first line what it expects --
//!
//! ```text
//!    # espera: BIEN      the frontend accepts it (and the emitter's bench
//!                        RUNS it and compares its `# sale:` lines)
//!    # espera: T0040     the frontend says NO, with exactly this code
//! ```
//!
//! A FOLDER in `ejemplos/nivelN/` is a PACKAGE (level 9): its
//! `src/main.titan` says what it expects, and the other files are found the
//! way the compiler finds them, following `mod`.
//!
//! -- through the WHOLE frontend: tree, names, the checker (`juez.rs`) and the
//! calculation (`calc.rs`). A level is done only when its list passes whole,
//! and every code of the language is provoked by some example of some level:
//! a NO that no example provokes is a NO nobody has seen.

use bmo_titan_front::{lower_package, Code, Message};
use std::path::{Path, PathBuf};

const LEVELS: [&str; 10] = ["nivel0", "nivel1", "nivel2", "nivel3", "nivel4", "nivel5", "nivel6", "nivel7", "nivel8", "nivel9"];

/// One example: its name, what it expects, the text of its root file, the
/// folder its paths start from, and its root's path from there.
struct Example {
    name: String,
    want: String,
    src: String,
    dir: PathBuf,
    root: String,
}

impl Example {
    fn lower(&self) -> Result<bmo_titan_front::ir::Module, Message> {
        lower_package(&self.root, &self.src, &mut |p| std::fs::read_to_string(self.dir.join(p)).ok())
    }

    /// The NO, drawn on the file it is in.
    fn render(&self, m: &Message) -> (String, usize) {
        let file = m.file.clone().unwrap_or_else(|| self.root.clone());
        let text = std::fs::read_to_string(self.dir.join(&file)).unwrap_or_else(|_| self.src.clone());
        (m.render(&format!("{}/{}", self.name, file), &text), text.lines().count())
    }
}

fn bench(level: &str) -> Vec<Example> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("ejemplos").join(level);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the bench folder") {
        let path = entry.unwrap().path();
        let (base, root) = if path.is_dir() {
            (path.clone(), "src/main.titan".to_string())
        } else if path.extension().and_then(|e| e.to_str()) == Some("titan") {
            (dir.clone(), path.file_name().unwrap().to_string_lossy().into_owned())
        } else {
            continue;
        };
        let src = std::fs::read_to_string(base.join(&root)).unwrap_or_else(|_| panic!("{}: a package has its src/main.titan", path.display()));
        let want = src
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("# espera: "))
            .unwrap_or_else(|| panic!("{}: its first line must say `# espera: ...`", path.display()))
            .trim()
            .to_string();
        out.push(Example { name: path.file_name().unwrap().to_string_lossy().into_owned(), want, src, dir: base, root });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[test]
fn every_level_passes_whole_and_every_code_is_seen() {
    let mut seen = Vec::new();
    for level in LEVELS {
        let mut bien = 0;
        for ex in bench(level) {
            let name = format!("{}/{}", level, ex.name);
            match (ex.want.as_str(), ex.lower()) {
                ("BIEN", Ok(_)) => bien += 1,
                ("BIEN", Err(m)) => panic!("{} should compile:\n{}", name, ex.render(&m).0),
                (code, Ok(_)) => panic!("{} should say {} and it compiled", name, code),
                (code, Err(m)) => {
                    let (r, lines) = ex.render(&m);
                    assert_eq!(m.code.label(), code, "{}:\n{}", name, r);
                    for part in ["QUE      ", "DONDE    ", "POR QUE  ", "COMO     "] {
                        assert!(r.contains(part), "{}: without {}", name, part.trim());
                    }
                    assert!(m.line >= 1 && m.line <= lines + 1, "{}: line {}\n{}", name, m.line, r);
                    seen.push(m.code);
                }
            }
        }
        assert!(bien >= 2, "{} has its programs that compile", level);
    }
    for code in Code::ALL {
        assert!(seen.contains(&code), "no example provokes {}", code.label());
    }
}
