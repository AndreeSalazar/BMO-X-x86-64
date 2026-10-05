//! **LISTS AND MAPS, the front's half** (level 13,
//! `docs/plan/PLAN_LISTAS_Y_MAPAS.md`, L1 and L2): what grows while the
//! program runs, judged and RUN when compiling (E0). The machine's half (E1)
//! is measured against this in `emisor-x86_64/tests/e1.rs`.

use bmo_titan_front::ir::{Op, Value};
use bmo_titan_front::{lower, Code};

/// What the program prints, run when compiling: one string per `print`.
fn printed(body: &str) -> Vec<String> {
    let src = format!("mod main \"x\"\n\n{body}");
    let m = lower(&src).unwrap_or_else(|e| panic!("{e:?}\n{src}"));
    m.flat
        .expect("nothing comes from outside: the calculation runs it")
        .iter()
        .filter_map(|op| match op {
            Op::Write { parts, .. } => Some(
                parts
                    .iter()
                    .map(|p| match p {
                        Value::Text(t, _) => t.clone(),
                        other => panic!("not folded: {other:?}"),
                    })
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

fn code(body: &str) -> Code {
    let src = format!("mod main \"x\"\n\n{body}");
    lower(&src).map(|_| ()).unwrap_err().code
}

#[test]
fn a_list_grows_and_shrinks_and_pop_says_what_it_took() {
    let body = "fn main()\n    let mut l: [int] = []\n    for i in range(4)\n        push(mut l, i * 10)\n    print(l, \" \", len(l), \" \", l[2])\n    let ultimo = pop(mut l)\n    match ultimo\n        Hay(x)\n            print(\"saco \", x, \", quedan \", l)\n        NoHay\n            print(\"vacia\")\n    l[0] = 7\n    let mut s = 0\n    for x in l\n        s = s + x\n    print(s)\n";
    assert_eq!(printed(body), ["[0, 10, 20, 30] 4 20", "saco 30, quedan [0, 10, 20]", "37"]);
    // pop on an empty list: NoHay, and the list stays empty.
    let body = "fn main()\n    let mut l: [text] = []\n    let x = pop(mut l)\n    match x\n        Hay(t)\n            print(t)\n        NoHay\n            print(\"nada \", l)\n";
    assert_eq!(printed(body), ["nada []"]);
}

#[test]
fn a_map_keeps_its_keys_in_order_and_get_gives_a_case() {
    let body = "fn main()\n    let mut stock: {text: int} = {}\n    put(mut stock, \"pan\", 3)\n    put(mut stock, \"leche\", 2)\n    put(mut stock, \"pan\", 5)\n    print(stock, \" \", len(stock), \" \", has(stock, \"leche\"))\n    for k in stock\n        match get(stock, k)\n            Hay(n)\n                print(k, \": \", n)\n            NoHay\n                print(\"?\")\n    remove(mut stock, \"pan\")\n    match get(stock, \"pan\")\n        Hay(n)\n            print(n)\n        NoHay\n            print(\"sin pan: \", stock)\n";
    assert_eq!(printed(body), ["{\"pan\": 5, \"leche\": 2} 2 true", "pan: 5", "leche: 2", "sin pan: {\"leche\": 2}"]);
    // A map written: its types from what it holds.
    assert_eq!(printed("fn main()\n    let m = {1: \"uno\", 2: \"dos\"}\n    print(m, \" \", m == {2: \"dos\", 1: \"uno\"})\n"), ["{1: \"uno\", 2: \"dos\"} true"]);
}

#[test]
fn a_list_is_a_value_and_copies_like_everything_else() {
    // D2: `let b = a` is ANOTHER list; push changes only the one lent.
    let body = "fn main()\n    let mut a: [int] = [1, 2]\n    let b = a\n    push(mut a, 3)\n    print(a, \" \", b)\n";
    assert_eq!(printed(body), ["[1, 2, 3] [1, 2]"]);
    // Through a fn: a copy, a `mut` lent, and a list given back.
    let body = "fn suma(l: [int]) -> int\n    let mut s = 0\n    for x in l\n        s = s + x\n    return s\nfn crece(mut l: [int], n: int)\n    for i in range(n)\n        push(mut l, i)\nfn pares(n: int) -> [int]\n    let mut out: [int] = []\n    for i in range(n)\n        if i % 2 == 0\n            push(mut out, i)\n    return out\nfn main()\n    let mut l: [int] = []\n    crece(mut l, 4)\n    print(l, \" \", suma(l), \" \", pares(7))\n";
    assert_eq!(printed(body), ["[0, 1, 2, 3] 6 [0, 2, 4, 6]"]);
}

#[test]
fn a_list_keeps_the_type_it_said() {
    // An int into a [dec] is a decimal, as in `let x: dec = 2`.
    let body = "fn main()\n    let mut precios: [dec] = [1.5]\n    push(mut precios, 2)\n    print(precios, \" \", precios[1] / 4)\n";
    assert_eq!(printed(body), ["[1.5, 2] 0.5"]);
    // Records with lists in them, and lists of records.
    let body = "type Nave\n    nombre: text\n    carga: [int]\nfn main()\n    let mut flota: [Nave] = []\n    push(mut flota, Nave { nombre: \"a\", carga: [] })\n    push(mut flota, Nave { nombre: \"b\", carga: [1, 2] })\n    flota[0].carga = [9]\n    print(flota)\n";
    assert_eq!(printed(body), ["[Nave { nombre: \"a\", carga: [9] }, Nave { nombre: \"b\", carga: [1, 2] }]"]);
}

#[test]
fn every_rule_of_lists_and_maps_has_its_no() {
    // `[]` and `{}` say nothing: their type has to.
    assert_eq!(code("fn main()\n    let l = []\n    print(l)\n"), Code::WrongType);
    assert_eq!(code("fn main()\n    let m = {}\n    print(m)\n"), Code::WrongType);
    // push changes its list: lent, with `mut`, and the list `let mut`.
    assert_eq!(code("fn main()\n    let mut l: [int] = []\n    push(l, 1)\n    print(l)\n"), Code::Mode);
    assert_eq!(code("fn main()\n    let l: [int] = []\n    push(mut l, 1)\n    print(l)\n"), Code::NotMut);
    // Of its class.
    assert_eq!(code("fn main()\n    let mut l: [int] = []\n    push(mut l, \"uno\")\n    print(l)\n"), Code::WrongType);
    assert_eq!(code("fn main()\n    let mut m: {text: int} = {}\n    put(mut m, 1, 1)\n    print(m)\n"), Code::WrongType);
    // A map has no numbered cells, and a decimal is not a key.
    assert_eq!(code("fn main()\n    let m = {\"a\": 1}\n    print(m[0])\n"), Code::Mixed);
    assert_eq!(code("fn main()\n    let m = {1.5: 1}\n    print(m)\n"), Code::WrongType);
    // The values each fn takes, and what it gives.
    assert_eq!(code("fn main()\n    let mut l: [int] = []\n    push(mut l)\n    print(l)\n"), Code::Args);
    assert_eq!(code("fn main()\n    let m = {\"a\": 1}\n    get(m, \"a\")\n"), Code::Result);
    assert_eq!(code("fn main()\n    let mut l: [int] = []\n    let x = push(mut l, 1)\n    print(x)\n"), Code::Result);
    // pop stands alone: it changes the list AND gives.
    assert_eq!(code("fn main()\n    let mut l: [int] = [1]\n    print(pop(mut l))\n"), Code::Result);
    // Hay and NoHay come from the library, never written by hand.
    assert_eq!(code("fn main()\n    let m = {\"a\": 1}\n    let x = get(m, \"a\")\n    let y = Hay(3)\n    print(y)\n"), Code::Case);
    // Outside the list: seen when compiling, as with a table.
    assert_eq!(code("fn main()\n    let mut l: [int] = []\n    push(mut l, 1)\n    print(l[1])\n"), Code::Outside);
    // `match` on the case: both arms.
    assert_eq!(code("fn main()\n    let m = {\"a\": 1}\n    match get(m, \"a\")\n        Hay(n)\n            print(n)\n"), Code::Missing);
}

#[test]
fn opcion_is_a_type_that_can_be_written() {
    let body = "fn busca(m: {text: int}, k: text) -> Opcion[int]\n    return get(m, k)\nfn main()\n    let m = {\"a\": 1}\n    for k in [\"a\", \"b\"]\n        match busca(m, k)\n            Hay(n)\n                print(k, \" lleva \", n)\n            NoHay\n                print(k, \" no esta\")\n";
    assert_eq!(printed(body), ["a lleva 1", "b no esta"]);
}

#[test]
fn a_part_of_a_value_can_be_lent_to_the_library() {
    // `push(mut nave.carga, x)`: the part changes IN its value (level 13).
    let body = "type Nave\n    nombre: text\n    carga: [int]\nfn main()\n    let mut n = Nave { nombre: \"a\", carga: [] }\n    push(mut n.carga, 1)\n    push(mut n.carga, 2)\n    let x = pop(mut n.carga)\n    let mut flota: [Nave] = [n]\n    push(mut flota[0].carga, 7)\n    print(n, \" \", flota, \" \", x)\n";
    assert_eq!(printed(body), ["Nave { nombre: \"a\", carga: [1] } [Nave { nombre: \"a\", carga: [1, 7] }] Hay(2)"]);
    // A part of a value that is not `let mut` does not change.
    assert_eq!(code("type Nave\n    carga: [int]\nfn main()\n    let n = Nave { carga: [] }\n    push(mut n.carga, 1)\n    print(n)\n"), Code::NotMut);
}
