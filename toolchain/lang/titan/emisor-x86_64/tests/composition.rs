//! Composicion y traits para bloques: el E1 real, sin resolverlo al compilar.
use bmo_lower::emu::{cargar_bex, run};

const SOURCE: &str = include_str!("../../ejemplos/nivel10/bloques.titan");

fn native(source: &str) -> String {
    let bex = bmo_titan_x86_64::build_package_e1("blocks.titan", source, &mut |_| None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let bytes = bex.len();
    let m = run(cargar_bex(&bex).unwrap(), 5_000_000);
    assert!(m.exited, "{}", m.console);
    eprintln!("composicion E1: {bytes} bytes de fichero, {} instrucciones", m.pasos);
    let calculated = bmo_titan_x86_64::build(source, "blocks.titan")
        .unwrap_or_else(|e| panic!("calculo: {e:?}"));
    let expected = run(cargar_bex(&calculated).unwrap(), 5_000_000);
    assert!(expected.exited);
    assert_eq!(m.console, expected.console, "E1 y el calculo deben coincidir");
    m.console
}

#[test]
fn nested_scalar_fields_change_only_the_component() {
    let source = r#"mod main "campos"
type State
    value: int
type Body
    state: State
    untouched: int
type Actor
    body: Body
fn bump(mut n: int) -> int
    n = n + 1
    return n
fn main()
    let mut a = Actor { body: Body { state: State { value: 10 }, untouched: 99 } }
    let result = bump(mut a.body.state.value)
    print(result, " ", a.body.state.value, " ", a.body.untouched)
"#;
    assert_eq!(native(source), "11 11 99\n");
}

#[test]
fn heap_components_keep_copies_independent_and_release_once() {
    let source = r#"mod main "componentes con monton"
type Bag
    items: [int]
    label: text
    marker: int
fn grow(mut items: [int])
    for i in range(40)
        push(mut items, i)
fn rename(mut label: text)
    label = "nuevo"
fn main()
    let mut b = Bag { items: [7], label: "antes", marker: 91 }
    let copy = b
    grow(mut b.items)
    rename(mut b.label)
    print(len(b.items), " ", b.items[40], " ", b.label, " ", b.marker)
    print(len(copy.items), " ", copy.items[0], " ", copy.label, " ", copy.marker)
"#;
    assert_eq!(native(source), "41 39 nuevo 91\n1 7 antes 91\n");
}

#[test]
fn independent_owners_can_lend_fields_in_one_call() {
    let source = format!("{FIELD_HEAD}\nfn main()\n    let mut p = Pair {{ left: Cell {{ n: 1 }}, right: Cell {{ n: 2 }} }}\n    let mut q = Pair {{ left: Cell {{ n: 5 }}, right: Cell {{ n: 6 }} }}\n    both(mut p.left, mut q.right)\n    print(p.left.n, \" \", p.right.n, \" \", q.left.n, \" \", q.right.n)\n");
    assert_eq!(native(&source), "2 2 5 7\n");
}

#[test]
fn byte_and_bounded_decimal_fields_keep_their_declared_types() {
    let source = r#"mod main "campos con limites"
type Account
    flag: byte
    amount: dec(3, 1)
fn set_flag(mut flag: byte)
    flag = byte(250)
fn add(mut amount: dec(3, 1))
    amount = amount + 1
fn main()
    let mut a = Account { flag: byte(1), amount: 9.0 }
    set_flag(mut a.flag)
    add(mut a.amount)
    print(a.flag, " ", a.amount)
"#;
    assert_eq!(native(source), "250 10.0\n");
}

#[test]
fn a_large_table_field_is_borrowed_without_copying_the_owner() {
    let source = r#"mod main "campo grande"
type Chunk
    blocks: [int; 3000]
    marker: int
fn fill(mut blocks: [int; 3000])
    blocks[0] = 17
    blocks[2999] = 23
fn main()
    let mut chunk = Chunk { blocks: [0; 3000], marker: 99 }
    fill(mut chunk.blocks)
    print(chunk.blocks[0], " ", chunk.blocks[2999], " ", chunk.marker)
"#;
    assert_eq!(native(source), "17 23 99\n");
}

#[test]
fn record_initialization_evaluates_a_repeated_value_once() {
    let source = r#"mod main "inicializar una vez"
type State
    values: [int; 3]
    marker: int
fn next_value(mut n: int) -> int
    n = n + 1
    return n
fn main()
    let mut n = 0
    let s = State { values: [next_value(mut n); 3], marker: next_value(mut n) }
    print(n, " ", s.values, " ", s.marker)
"#;
    assert_eq!(native(source), "2 [1, 1, 1] 2\n");
}

#[test]
fn record_initialization_keeps_decimal_limits_inside_repeated_fields() {
    let source = "mod main \"limite al construir\"\ntype State\n    values: [dec(3, 1); 3]\nfn main()\n    let s = State { values: [99.9; 3] }\n    print(s.values)\n";
    assert_eq!(native(source), "[99.9, 99.9, 99.9]\n");
    let bad = source.replace("[99.9; 3]", "[100.0; 3]");
    let line = match bmo_titan_x86_64::build(&bad, "limits.titan").unwrap_err() {
        bmo_titan_x86_64::Failure::Source(m) => { assert_eq!(m.code.number(), 74); m.line },
        other => panic!("{other:?}"),
    };
    let bex = bmo_titan_x86_64::build_package_e1("limits.titan", &bad, &mut |_| None).unwrap();
    let m = run(cargar_bex(&bex).unwrap(), 5_000_000);
    assert!(m.exited);
    assert!(m.console.starts_with(&format!("NO T0074 al correr, linea {line}:")), "{}", m.console);
}

#[test]
fn component_functions_can_live_in_another_module() {
    let source = r#"mod main "componentes entre modulos"
mod health
type Block
    state: health.State
fn main()
    let mut b = Block { state: health.State { value: 10 } }
    health.reduce(mut b.state, 3)
    print(b.state.value)
"#;
    let module = "mod health \"estado reutilizable\"\npub type State\n    value: int\npub fn reduce(mut state: State, force: int)\n    state.value = state.value - force\n";
    for force in [false, true] {
        let mut read = |p: &str| (p == "health.titan").then(|| module.to_string());
        let bex = if force {
            bmo_titan_x86_64::build_package_e1("main.titan", source, &mut read)
        } else {
            bmo_titan_x86_64::build_package("main.titan", source, &mut read)
        }.unwrap_or_else(|e| panic!("{e:?}"));
        let m = run(cargar_bex(&bex).unwrap(), 5_000_000);
        assert!(m.exited);
        assert_eq!(m.console, "7\n");
    }
}

#[test]
fn a_component_can_be_borrowed_as_a_trait() {
    let source = r#"mod main "componente como trait"
trait Counter
    fn advance(mut c: Counter)
type Count
    value: int
type Holder
    count: Count
    marker: int
trait Counter for Count
    fn advance(mut c: Count)
        c.value = c.value + 1
fn twice(mut c: Counter)
    advance(mut c)
    advance(mut c)
fn main()
    let mut h = Holder { count: Count { value: 4 }, marker: 90 }
    twice(mut h.count)
    print(h.count.value, " ", h.marker)
"#;
    assert_eq!(native(source), "6 90\n");
}

#[test]
fn trait_wrappers_return_heap_components_without_losing_ownership() {
    let source = r#"mod main "trait con monton"
trait Growable
    fn append(mut x: Growable)
type Bag
    items: [int]
type Holder
    bag: Bag
    marker: int
trait Growable for Bag
    fn append(mut x: Bag)
        push(mut x.items, 8)
fn twice(mut x: Growable)
    append(mut x)
    append(mut x)
fn main()
    let mut h = Holder { bag: Bag { items: [7] }, marker: 91 }
    let copy = h.bag
    twice(mut h.bag)
    print(h.bag.items, " ", h.marker, " ", copy.items)
"#;
    assert_eq!(native(source), "[7, 8, 8] 91 [7]\n");
}

const FIELD_HEAD: &str = r#"mod main "rechazos de campos"
type Cell
    n: int
type Pair
    left: Cell
    right: Cell
fn change(mut a: Cell)
    a.n = a.n + 1
fn both(mut a: Cell, mut b: Cell)
    a.n = a.n + 1
    b.n = b.n + 1
fn see(mut a: Cell, b: Pair)
    a.n = a.n + b.right.n
fn inspect(b: Pair, mut a: Cell)
    a.n = a.n + b.right.n
fn read(mut a: Cell, n: int)
    a.n = a.n + n
fn merge(mut b: Pair, mut a: Cell)
    b.left.n = b.left.n + 1
    a.n = a.n + 1
fn next(mut a: Cell) -> int
    a.n = a.n + 1
    return a.n
fn consume(take p: Pair)
    print(p.left.n)
"#;

fn rejected(body: &str, code: u16) {
    let source = format!("{FIELD_HEAD}\nfn main()\n{body}");
    for force in [false, true] {
        let error = if force {
            bmo_titan_x86_64::build_package_e1("fields.titan", &source, &mut |_| None)
        } else {
            bmo_titan_x86_64::build(&source, "fields.titan")
        }.expect_err("el prestamo no es valido");
        match error {
            bmo_titan_x86_64::Failure::Source(m) => assert_eq!(m.code.number(), code, "{m:?}"),
            other => panic!("se esperaba rechazo del fuente: {other:?}"),
        }
    }
}

#[test]
fn overlapping_owners_are_rejected_in_both_argument_orders() {
    let head = "    let mut p = Pair { left: Cell { n: 1 }, right: Cell { n: 2 } }\n";
    for call in ["both(mut p.left, mut p.left)", "both(mut p.left, mut p.right)",
                 "see(mut p.left, p)", "inspect(p, mut p.left)",
                 "read(mut p.left, p.left.n)", "read(mut p.left, p.right.n)",
                 "merge(mut p, mut p.left)", "read(mut p.left, next(mut p.right))"] {
        rejected(&format!("{head}    {call}\n"), 76);
    }
}

#[test]
fn immutable_partial_take_and_indexed_loans_remain_rejected() {
    let literal = "Pair { left: Cell { n: 1 }, right: Cell { n: 2 } }";
    rejected(&format!("    let p = {literal}\n    change(mut p.left)\n"), 56);
    rejected(&format!("    let p = {literal}\n    change(take p.left)\n"), 77);
    rejected("    let mut cells = [Cell { n: 1 }]\n    change(mut cells[0])\n", 77);
}

#[test]
fn field_contracts_keep_dead_owners_and_wrong_types_out() {
    let head = "    let mut p = Pair { left: Cell { n: 1 }, right: Cell { n: 2 } }\n";
    rejected(&format!("{head}    consume(take p)\n    change(mut p.left)\n"), 75);
    rejected(&format!("{head}    change(mut p.left.n)\n"), 71);
    rejected(&format!("{head}    change(mut p.missing)\n"), 73);
    rejected("    let p = Pair { left: Cell { n: 1 }, right: Cell { n: 2 } }\n    change(p.left)\n", 77);
}

#[test]
fn nested_state_survives_mutable_trait_calls_for_both_types() {
    assert_eq!(native(SOURCE), "piedra: 7 false\nmadera: 0 true\n");
}

#[test]
fn a_shared_component_handles_zero_and_excess_damage() {
    let source = SOURCE.replace("mine(mut stone, 3)", "mine(mut stone, 0)")
        .replace("mine(mut wood, 3)", "mine(mut wood, 100)");
    assert_eq!(native(&source), "piedra: 10 false\nmadera: 0 true\n");
}

#[test]
fn types_without_the_behavior_are_refused_at_the_call() {
    let source = SOURCE.replace("    let mut stone =", "    let mut state = Durability { remaining: 10 }\n    let stone =")
        .replace("mine(mut stone, 3)", "mine(mut state, 3)");
    let error = bmo_titan_x86_64::build_package_e1("blocks.titan", &source, &mut |_| None)
        .expect_err("el componente no implementa Breakable");
    match error {
        bmo_titan_x86_64::Failure::Source(m) => assert_eq!(m.code.number(), 85),
        other => panic!("se esperaba T0085, no {other:?}"),
    }
}
