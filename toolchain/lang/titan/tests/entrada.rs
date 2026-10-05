//! **E1, the front's half** (`docs/plan/PLAN_LA_ENTRADA.md`, R1 and R2):
//! `lee()` -- the line typed on the program's own console -- is a library
//! function like `print`, a text, and a program that reads is NOT run when
//! compiling: its classes and its loans are judged, and the rest is emitted.

use bmo_titan_front::{lower, Code};

fn code(src: &str) -> Code {
    lower(src).map(|_| ()).unwrap_err().code
}

#[test]
fn a_program_that_reads_is_judged_but_not_run() {
    let src = "mod main \"saluda a quien teclee\"\n\nfn main()\n    print(\"como te llamas?\")\n    let nombre = lee()\n    print(\"hola \", nombre)\n";
    let m = lower(src).unwrap();
    assert!(m.reads_outside());
    assert!(m.flat.is_none(), "what is typed is not known yet: nothing is run");
    assert!(m.functions.iter().flat_map(|f| &f.blocks).all(|b| !b.dead), "every block stays: the emitter decides at run time");
}

#[test]
fn a_program_that_does_not_read_is_still_run_whole() {
    let m = lower("mod main \"x\"\n\nfn main()\n    print(\"hola\")\n").unwrap();
    assert!(!m.reads_outside());
    assert!(m.flat.is_some(), "E0 stays exactly as it was");
}

#[test]
fn lee_takes_nothing_gives_a_text_and_what_it_gives_is_kept() {
    let pre = "mod main \"x\"\n\nfn main()\n";
    assert_eq!(code(&format!("{pre}    let a = lee(1)\n    print(a)\n")), Code::Args);
    assert_eq!(code(&format!("{pre}    lee()\n")), Code::Result);
    // a text: it does not add to a number (T0063), not even typed
    assert_eq!(code(&format!("{pre}    let a = lee()\n    print(a + 1)\n")), Code::Mixed);
    // and a yes/no is asked of it like of any text (T0065)
    assert_eq!(code(&format!("{pre}    let a = lee()\n    if a\n        print(1)\n")), Code::NotBool);
}

#[test]
fn what_is_typed_can_decide() {
    let src = "mod main \"x\"\n\nfn main()\n    let r = lee()\n    if r == \"si\"\n        print(\"bien\")\n    else\n        print(\"otra vez\")\n";
    let m = lower(src).unwrap();
    assert!(m.flat.is_none());
}

/// `numero(t)` (R6, D3): a CASE the `match` has to look at whole -- here
/// with texts known when compiling, so it is run (E0) like any program.
#[test]
fn numero_gives_a_case_and_the_match_looks_at_both() {
    let src = |t: &str| format!("mod main \"x\"\n\nfn main()\n    match numero(\"{t}\")\n        Es(n)\n            print(\"es \", n * 2)\n        NoEs\n            print(\"no es un numero\")\n");
    let printed = |t: &str| -> Vec<String> {
        let m = lower(&src(t)).unwrap();
        m.flat.unwrap().iter().filter_map(|op| match op {
            bmo_titan_front::ir::Op::Write { parts, .. } => Some(parts.iter().map(|p| match p {
                bmo_titan_front::ir::Value::Text(t, _) => t.clone(),
                _ => String::new(),
            }).collect()),
            _ => None,
        }).collect()
    };
    assert_eq!(printed("21"), ["es 42"]);
    assert_eq!(printed(" -4 "), ["es -8"]);
    assert_eq!(printed("doce"), ["no es un numero"]);
    // the match has to have both cases (T0079), and numero reads a text
    assert_eq!(code("mod main \"x\"\n\nfn main()\n    match numero(\"1\")\n        Es(n)\n            print(n)\n"), Code::Missing);
    assert_eq!(code("mod main \"x\"\n\nfn main()\n    match numero(1)\n        Es(n)\n            print(n)\n        NoEs\n            print(0)\n"), Code::WrongType);
    // a program that uses numero cannot name Es itself (T0055)
    assert_eq!(code("mod main \"x\"\n\nenum Si\n    Es\n    No\n\nfn main()\n    match numero(\"1\")\n        Es(n)\n            print(n)\n        NoEs\n            print(0)\n"), Code::Taken);
    // and one that does not use it may
    assert!(lower("mod main \"x\"\n\nenum Si\n    Es\n    No\n\nfn main()\n    print(Es)\n").is_ok());
}

#[test]
fn numero_used_as_the_int_says_to_look_at_its_case() {
    let src = "mod main \"x\"\n\nfn main()\n    let n = numero(lee())\n    print(n + 1)\n";
    let m = lower(src).map(|_| ()).unwrap_err();
    assert_eq!(m.code, Code::Mixed);
    assert!(m.why.contains("da un CASO") && m.how.contains("match"), "{m:?}");
}
