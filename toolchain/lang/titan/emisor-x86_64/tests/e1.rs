//! **E1 RUNS** (`docs/plan/PLAN_LA_ENTRADA.md`, R3): a program that reads
//! (`lee()`) is emitted to run, and the emulator TYPES for it
//! (`poner_entrada`) -- what it prints depends on what was typed.

use bmo_lower::emu::{cargar_bex, run};

/// Builds `body` (the lines of `main`, with `fns` after it), types `input`
/// and gives back what the console shows, and whether it reached EXIT.
fn ran(body: &str, fns: &str, input: &str) -> (String, bool) {
    let src = format!("mod main \"prueba de E1\"\n\nfn main()\n{body}{fns}");
    let bex = bmo_titan_x86_64::build(&src, "e1.titan").unwrap_or_else(|e| panic!("{e:?}\n{src}"));
    let mut m = cargar_bex(&bex).unwrap();
    m.poner_entrada(input);
    let m = run(m, 2_000_000);
    (m.console.clone(), m.exited)
}

#[test]
fn it_asks_and_answers_with_what_was_typed() {
    let body = "    print(\"como te llamas?\")\n    let nombre = lee()\n    print(\"hola \", nombre, \"!\")\n";
    assert_eq!(ran(body, "", "Ada\n"), ("como te llamas?\nhola Ada!\n".into(), true));
    assert_eq!(ran(body, "", "Grace Hopper\n").0, "como te llamas?\nhola Grace Hopper!\n");
}

#[test]
fn what_is_typed_decides() {
    let body = "    let r = lee()\n    if r == \"si\"\n        print(\"bien\")\n    else\n        print(\"otra vez\")\n    print(r != \"si\")\n";
    assert_eq!(ran(body, "", "si\n").0, "bien\nfalse\n");
    assert_eq!(ran(body, "", "no\n").0, "otra vez\ntrue\n");
    // the same length and other bytes: not equal
    assert_eq!(ran(body, "", "so\n").0, "otra vez\ntrue\n");
}

#[test]
fn it_counts_the_lines_until_fin() {
    let body = "    let mut n = 0\n    let mut r = lee()\n    while r != \"fin\"\n        n = n + 1\n        r = lee()\n    print(\"lineas: \", n)\n";
    assert_eq!(ran(body, "", "a\nb\nc\nfin\n"), ("lineas: 3\n".into(), true));
    assert_eq!(ran(body, "", "fin\n").0, "lineas: 0\n");
}

#[test]
fn ints_and_loops_and_calls_run_in_the_machine() {
    let body = "    let r = lee()\n    let mut suma = 0\n    for i in range(5)\n        suma = suma + i * 10\n    saluda()\n    print(r, \": \", suma, \" \", suma / 4, \" \", suma % 7, \" \", -suma)\n";
    let fns = "\nfn saluda()\n    print(\"una fn sin valores\")\n";
    assert_eq!(ran(body, fns, "total\n").0, "una fn sin valores\ntotal: 100 25 2 -100\n");
}

#[test]
fn what_only_fails_when_running_traps_with_its_line() {
    // T0060: no wrap, ever
    let body = "    let r = lee()\n    let mut x = 9223372036854775807\n    x = x + 1\n    print(r, x)\n";
    let (out, exited) = ran(body, "", "a\n");
    assert!(exited);
    assert_eq!(out, "NO T0060 al correr, linea 6: el resultado no cabe en 64 bits\n");
    // T0062: 7 / 2 between ints is not 3
    let body = "    let r = lee()\n    let a = 7\n    print(r, a / 2)\n";
    assert_eq!(ran(body, "", "x\n").0, "NO T0062 al correr, linea 6: una division que no es exacta (entre enteros, 7 / 2 no es 3)\n");
    // T0061
    let body = "    let r = lee()\n    let a = 0\n    print(r, 5 % a)\n";
    assert_eq!(ran(body, "", "x\n").0, "NO T0061 al correr, linea 6: una division por cero\n");
}

#[test]
fn what_e1_cannot_do_yet_is_said_when_compiling() {
    let src = "mod main \"x\"\n\nfn main()\n    let r = lee()\n    let d = 1.5\n    print(r, d)\n";
    let e = bmo_titan_x86_64::build(src, "e1.titan").unwrap_err();
    let text = format!("{e:?}");
    assert!(text.contains("R6"), "{text}");
}
