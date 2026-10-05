//! `calc::pruebas` -- the tests of the calculation, cut out of `calc.rs` on
//! 05-10 (L6a: no file over 1.000 lines) when level 13 brought the lists.

use super::*;

pub(super) fn run(src: &str) -> Result<Module, Message> {
    let m = crate::ir::lower(&crate::compile(src).unwrap());
    crate::juez::judge(&m)?;
    fold(&m)
}

pub(super) fn printed(src: &str) -> Vec<String> {
    let m = run(src).unwrap();
    m.flat
        .as_ref()
        .expect("calc runs the program")
        .iter()
        .filter_map(|op| match op {
            Op::Write { parts, .. } => Some(
                parts
                    .iter()
                    .map(|p| match p {
                        Value::Int(n, _) => n.to_string(),
                        Value::Text(t, _) => t.clone(),
                        Value::Bool(b, _) => b.to_string(),
                        other => panic!("not folded: {:?}", other),
                    })
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

#[test]
fn it_calculates_with_the_school_precedence() {
    assert_eq!(printed("mod main \"x\"\nfn main()\n    let a = 2 + 3 * 4\n    print(a, \" \", (2 + 3) * 4, \" \", -a % 5, \" \", 12 / 4)\n"), ["14 20 -4 3"]);
    assert_eq!(printed("mod main \"x\"\nfn main()\n    let s = \"ho\" + \"la\"\n    print(s, \"!\")\n"), ["hola!"]);
}

#[test]
fn each_no_of_the_calculation_with_its_code() {
    let code = |src: &str| run(src).unwrap_err().code;
    assert_eq!(code("mod main \"x\"\nfn main()\n    print(9223372036854775807 + 1)\n"), Code::Overflow);
    assert_eq!(code("mod main \"x\"\nfn main()\n    print(-9223372036854775807 - 2)\n"), Code::Overflow);
    assert_eq!(code("mod main \"x\"\nfn main()\n    let z = 0\n    print(5 % z)\n"), Code::DivZero);
    assert_eq!(code("mod main \"x\"\nfn main()\n    print(7 / 2)\n"), Code::Inexact);
    assert_eq!(code("mod main \"x\"\nfn main()\n    print(\"a\" + 1)\n"), Code::Mixed);
    assert_eq!(code("mod main \"x\"\nfn main()\n    print(\"a\" * \"b\")\n"), Code::Mixed);
}

#[test]
fn every_if_is_decided_when_compiling_and_the_other_side_is_dead() {
    let src = "mod main \"x\"\nfn main()\n    let v = 3\n    if v > 5\n        print(\"mucho\")\n    else if v > 1\n        print(\"algo\")\n    else\n        print(\"poco\")\n    print(v == 3, \" \", not (v < 0) and v != 4)\n";
    assert_eq!(printed(src), ["algo", "true true"]);
    let m = run(src).unwrap();
    assert_eq!(m.functions[0].blocks.iter().filter(|b| b.dead).count(), 2, "{}", m.show());
}

#[test]
fn a_dead_side_is_not_calculated_but_its_classes_are_judged() {
    // The guard works: 10 / d is never calculated when d is 0.
    assert_eq!(printed("mod main \"x\"\nfn main()\n    let d = 0\n    if d != 0\n        print(10 / d)\n    else\n        print(\"nada\")\n    print(d != 0 and 10 / d > 1)\n"), ["nada", "false"]);
    // A NO of CLASS is a NO wherever it sits.
    let e = run("mod main \"x\"\nfn main()\n    if false\n        print(\"a\" + 1)\n").unwrap_err();
    assert_eq!(e.code, Code::Mixed);
}

#[test]
fn a_condition_is_a_yes_or_no_and_nothing_else() {
    let code = |src: &str| run(src).unwrap_err().code;
    assert_eq!(code("mod main \"x\"\nfn main()\n    let vidas = 3\n    if vidas\n        print(\"a\")\n"), Code::NotBool);
    assert_eq!(code("mod main \"x\"\nfn main()\n    print(not 3)\n"), Code::NotBool);
    assert_eq!(code("mod main \"x\"\nfn main()\n    print(1 > 0 and 2)\n"), Code::NotBool);
    assert_eq!(code("mod main \"x\"\nfn main()\n    print(1 == \"1\")\n"), Code::Mixed);
    assert_eq!(code("mod main \"x\"\nfn main()\n    print(\"a\" < \"b\")\n"), Code::Mixed);
    assert_eq!(code("mod main \"x\"\nfn main()\n    let mut ok = true\n    ok = 1\n"), Code::Retype);
}

#[test]
fn loops_are_run_turn_by_turn_when_compiling() {
    let src = "mod main \"x\"\nfn main()\n    for i in range(1, 4)\n        print(i, \" x 3 = \", i * 3)\n    let mut n = 10\n    while n > 0\n        n = n - 4\n        if n < 5\n            continue\n        print(\"n \", n)\n    print(\"fin \", n)\n";
    assert_eq!(printed(src), ["1 x 3 = 3", "2 x 3 = 6", "3 x 3 = 9", "n 6", "fin -2"]);
}

#[test]
fn a_loop_that_does_not_end_is_said_at_its_line() {
    let e = run("mod main \"x\"\nfn main()\n    let mut n = 0\n    while n >= 0\n        n = n + 1\n").unwrap_err();
    assert_eq!((e.code, e.line), (Code::NoEnd, 4));
    // A `while true` with its `break` ends.
    assert_eq!(printed("mod main \"x\"\nfn main()\n    let mut n = 1\n    while true\n        n = n * 2\n        if n > 100\n            break\n    print(n)\n"), ["128"]);
    // `range` counts with numbers.
    assert_eq!(run("mod main \"x\"\nfn main()\n    for i in range(\"tres\")\n        print(i)\n").unwrap_err().code, Code::Mixed);
}

#[test]
fn calls_carry_values_and_recursion_runs_with_its_stop() {
    let src = "mod main \"x\"\nfn fact(n: int) -> int\n    if n <= 1\n        return 1\n    return n * fact(n - 1)\nfn par(n: int) -> bool\n    return n % 2 == 0\nfn main()\n    print(fact(10), \" \", par(fact(3)))\n";
    assert_eq!(printed(src), ["3628800 true"]);
    // A deep but finite recursion runs: 5000 calls nested.
    assert_eq!(printed("mod main \"x\"\nfn suma(n: int) -> int\n    if n == 0\n        return 0\n    return n + suma(n - 1)\nfn main()\n    print(suma(5000))\n"), ["12502500"]);
    // One that never stops is said, not a compiler that bursts.
    assert_eq!(run("mod main \"x\"\nfn f(n: int) -> int\n    return f(n + 1)\nfn main()\n    print(f(0))\n").unwrap_err().code, Code::NoEnd);
    // The class of each value given is the one its parameter says.
    assert_eq!(run("mod main \"x\"\nfn f(n: int) -> int\n    return n\nfn main()\n    print(f(true))\n").unwrap_err().code, Code::WrongType);
    assert_eq!(run("mod main \"x\"\nfn f(n: int) -> text\n    return n\nfn main()\n    print(f(1))\n").unwrap_err().code, Code::WrongType);
}

#[test]
fn tables_and_records_are_values_with_their_cells_and_fields() {
    let src = "mod main \"x\"\ntype Nave\n    x: dec\n    nombre: text\nfn main()\n    let mut t = [3, 1, 2]\n    t[0] = 9\n    let mut n = Nave { nombre: \"centauro\", x: 1.5 }\n    n.x = n.x * 2\n    print(t, \" \", len(t), \" \", t[2])\n    print(n)\n    let mut s = 0\n    for v in t\n        s = s + v\n    print(s, \" \", [0; 3])\n";
    assert_eq!(printed(src), ["[9, 1, 2] 3 2", "Nave { x: 3.0, nombre: \"centauro\" }", "12 [0, 0, 0]"]);
    // A cell outside its table: seen when compiling.
    assert_eq!(run("mod main \"x\"\nfn main()\n    let t = [1, 2, 3]\n    print(t[3])\n").unwrap_err().code, Code::Outside);
    // A field the record does not have.
    assert_eq!(run("mod main \"x\"\ntype P\n    x: int\nfn main()\n    let p = P { x: 1 }\n    print(p.y)\n").unwrap_err().code, Code::Field);
    // One class per table.
    assert_eq!(run("mod main \"x\"\nfn main()\n    print([1, \"dos\"])\n").unwrap_err().code, Code::WrongType);
}

#[test]
fn mut_gives_back_the_changes_and_cobol_precision_is_enforced() {
    let src = "mod main \"x\"\nfn ordena(mut t: [int; 3])\n    for i in range(3)\n        for j in range(2 - i)\n            if t[j] > t[j + 1]\n                let m = t[j + 1]\n                t[j + 1] = t[j]\n                t[j] = m\nfn main()\n    let mut t = [3, 1, 2]\n    ordena(mut t)\n    print(t)\n";
    assert_eq!(printed(src), ["[1, 2, 3]"]);
    // dec(7, 2): padded to its decimals; COBOL's PIC.
    assert_eq!(printed("mod main \"x\"\nfn main()\n    let precio: dec(7, 2) = 12.5\n    print(precio)\n"), ["12.50"]);
    // SIZE ERROR: too many digits, or too many decimals -- never cut.
    assert_eq!(run("mod main \"x\"\nfn main()\n    let p: dec(5, 2) = 1234.5\n    print(p)\n").unwrap_err().code, Code::Size);
    assert_eq!(run("mod main \"x\"\nfn main()\n    let p: dec(7, 2) = 1.255\n    print(p)\n").unwrap_err().code, Code::Size);
    // ... unless the rounding is WRITTEN: COBOL's ROUNDED, half away from zero.
    assert_eq!(printed("mod main \"x\"\nfn main()\n    let p: dec(7, 2) = round(1.255, 2)\n    print(p, \" \", round(10.00 / 3, 2), \" \", round(-2.345, 2), \" \", round(7 / 2, 0))\n"), ["1.26 3.33 -2.35 4"]);
    // A `mut` dec(7, 2) keeps its type at every change.
    assert_eq!(run("mod main \"x\"\nfn main()\n    let mut saldo: dec(5, 2) = 900.00\n    saldo = saldo * 200\n    print(saldo)\n").unwrap_err().code, Code::Size);
}

/// Level 8: a case carries its data, `match` takes it out, and every
/// rule of the cases has its NO.
#[test]
fn enums_carry_their_data_and_match_covers_every_case() {
    const FORMA: &str = "mod main \"x\"\nenum Forma\n    Circulo(dec)\n    Rect(dec, dec)\n    Nada\n";
    let src = format!("{}fn area(f: Forma) -> dec\n    match f\n        Circulo(r)\n            return 3 * r * r\n        Rect(a, b)\n            return a * b\n        Nada\n            return 0\nfn main()\n    for f in [Circulo(1.5), Rect(2, 2.5), Nada]\n        print(f, \" \", area(f))\n    print(Circulo(2.0) == Circulo(2), \" \", Nada == Circulo(1.0))\n", FORMA);
    assert_eq!(printed(&src), ["Circulo(1.5) 6.75", "Rect(2, 2.5) 5.0", "Nada 0", "true false"]);
    let code = |body: &str| crate::lower(&format!("{}{}", FORMA, body)).unwrap_err().code;
    // A case left out, and a case that is not one.
    assert_eq!(code("fn main()\n    let f = Nada\n    match f\n        Circulo(r)\n            print(r)\n        Nada\n            print(0)\n"), Code::Missing);
    assert_eq!(code("fn main()\n    let f = Nada\n    match f\n        Cuadrado\n            print(0)\n"), Code::Case);
    // Twice, or with the wrong number of names.
    assert_eq!(code("fn main()\n    let f = Nada\n    match f\n        Nada\n            print(0)\n        Nada\n            print(1)\n"), Code::Case);
    assert_eq!(code("fn main()\n    let f = Nada\n    match f\n        Circulo\n            print(0)\n        Rect(a, b)\n            print(a)\n        Nada\n            print(1)\n"), Code::Case);
    // A case with data, bare; a case alone on its line; a value named like a case.
    assert_eq!(code("fn main()\n    let f = Circulo\n    print(f)\n"), Code::Args);
    assert_eq!(code("fn main()\n    Circulo(1.0)\n"), Code::Result);
    assert_eq!(code("fn main()\n    let Nada = 1\n    print(Nada)\n"), Code::Taken);
    // The data of a case, and the value of a match, of their class.
    assert_eq!(code("fn main()\n    print(Circulo(\"dos\"))\n"), Code::WrongType);
    assert_eq!(code("fn main()\n    match 3\n        Circulo(r)\n            print(r)\n        Rect(a, b)\n            print(a)\n        Nada\n            print(0)\n"), Code::WrongType);
    // What an arm names lives in its arm.
    assert_eq!(code("fn main()\n    let f = Circulo(1.0)\n    match f\n        Circulo(r)\n            print(r)\n        Rect(a, b)\n            print(a)\n        Nada\n            print(0)\n    print(r)\n"), Code::Gone);
    // An enum that holds itself would never end.
    assert_eq!(crate::lower("mod main \"x\"\nenum Lista\n    Vacia\n    Uno(int, Lista)\nfn main()\n    print(1)\n").unwrap_err().code, Code::Case);
}

#[test]
fn an_inexact_division_says_the_quotient_and_the_rest() {
    let e = run("mod main \"x\"\nfn main()\n    print(7 / 2)\n").unwrap_err();
    assert!(e.why.contains("da 3 y sobra 1"), "{}", e.why);
}
