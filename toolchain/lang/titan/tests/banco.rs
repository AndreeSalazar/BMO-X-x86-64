//! THE BENCH of each level (TITAN_MAESTRO 14.14): every `.titan` in
//! `ejemplos/nivelN/` says in its first line what it expects --
//!
//! ```text
//!    # espera: BIEN      the frontend accepts it
//!    # espera: T0040     the frontend says NO, with exactly this code
//! ```
//!
//! -- and a level is done only when its list passes WHOLE, with every code of
//! the language used at least once: a NO that no example provokes is a NO
//! nobody has seen.

use bmo_titan_front::{compile, Code};
use std::path::Path;

fn bench(level: &str) -> Vec<(String, String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("ejemplos").join(level);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the bench folder") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("titan") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let want = src
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("# espera: "))
            .unwrap_or_else(|| panic!("{}: its first line must say `# espera: ...`", path.display()))
            .trim()
            .to_string();
        out.push((path.file_name().unwrap().to_string_lossy().into_owned(), want, src));
    }
    out.sort();
    out
}

#[test]
fn level_0_the_whole_list_passes_and_every_code_is_seen() {
    let mut seen = Vec::new();
    let mut bien = 0;
    for (name, want, src) in bench("nivel0") {
        match (want.as_str(), compile(&src)) {
            ("BIEN", Ok(_)) => bien += 1,
            ("BIEN", Err(m)) => panic!("{} should compile:\n{}", name, m.render(&name, &src)),
            (code, Ok(_)) => panic!("{} should say {} and it compiled", name, code),
            (code, Err(m)) => {
                assert_eq!(m.code.label(), code, "{}:\n{}", name, m.render(&name, &src));
                let r = m.render(&name, &src);
                for part in ["QUE      ", "DONDE    ", "POR QUE  ", "COMO     "] {
                    assert!(r.contains(part), "{}: without {}", name, part.trim());
                }
                assert!(m.line >= 1 && m.line <= src.lines().count() + 1, "{}: line {}", name, m.line);
                seen.push(m.code);
            }
        }
    }
    assert!(bien >= 2, "level 0 has its programs that compile");
    for code in Code::ALL {
        assert!(seen.contains(&code), "no example of level 0 provokes {}", code.label());
    }
}
