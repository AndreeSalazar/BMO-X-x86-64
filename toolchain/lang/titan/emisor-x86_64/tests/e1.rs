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
fn decimals_run_in_the_machine_exactly_as_the_calculation_says() {
    let body = "    let r = lee()\n    let precio = 12.50\n    let iva = precio * 0.21\n    print(r, precio + 1, \" \", iva, \" \", precio / 4, \" \", round(10.0 / 3, 2), \" \", -0.05)\n";
    assert_eq!(ran(body, "", "x\n").0, "x13.50 2.625 3.125 3.33 -0.05\n");
    let body = "    let r = lee()\n    print(r, 1.0 / 3)\n";
    assert!(ran(body, "", "x\n").0.starts_with("NO T0062 al correr, linea 5"));
}

/// `entero(lee())`: a typed int, or -1 when what was typed is not a number.
const ENTERO: &str = "\nfn entero(t: text) -> int\n    match numero(t)\n        Es(n)\n            return n\n        NoEs\n            return -1\n";

#[test]
fn numero_reads_what_was_typed_by_the_rule_of_the_prelude() {
    // the rule of `prelude::parse`: blanks around, a sign, digits, 64 bits
    let body = "    match numero(lee())\n        Es(n)\n            print(\"Es \", n)\n        NoEs\n            print(\"NoEs\")\n";
    for (typed, says) in [
        ("42", "Es 42"),
        ("  42\t", "Es 42"),
        ("+7", "Es 7"),
        ("-0", "Es 0"),
        ("-9223372036854775808", "Es -9223372036854775808"),
        ("9223372036854775807", "Es 9223372036854775807"),
        ("9223372036854775808", "NoEs"),
        ("4x", "NoEs"),
        ("", "NoEs"),
        ("-", "NoEs"),
        ("1 2", "NoEs"),
        ("12.5", "NoEs"),
    ] {
        assert_eq!(ran(body, "", &format!("{typed}\n")).0, format!("{says}\n"), "typed {typed:?}");
        assert_eq!(bmo_titan_front::prelude::parse(typed).map_or("NoEs".to_string(), |n| format!("Es {n}")), says, "the prelude, for {typed:?}");
    }
}

#[test]
fn a_typed_number_chooses_the_cell_and_leaving_the_table_is_a_no() {
    let body = "    let t = [10, 20, 30]\n    let i = entero(lee())\n    print(t[i])\n";
    assert_eq!(ran(body, ENTERO, "1\n").0, "20\n");
    assert_eq!(ran(body, ENTERO, "2\n").0, "30\n");
    for typed in ["3\n", "nada\n", "-5\n"] {
        let (out, exited) = ran(body, ENTERO, typed);
        assert!(exited && out.starts_with("NO T0072 al correr, linea 6:"), "typed {typed:?}: {out:?}");
    }
    // a cell written with a typed index
    let body = "    let mut t = [0, 0, 0]\n    t[entero(lee())] = 7\n    print(t)\n";
    assert_eq!(ran(body, ENTERO, "2\n").0, "[0, 0, 7]\n");
    assert!(ran(body, ENTERO, "9\n").0.starts_with("NO T0072 al correr, linea 5:"));
}

#[test]
fn the_pic_is_looked_at_when_the_number_comes_from_outside() {
    let body = "    let mut saldo: dec(7, 2) = 900.00\n    saldo = saldo * entero(lee())\n    print(saldo)\n";
    assert_eq!(ran(body, ENTERO, "2\n").0, "1800.00\n");
    assert_eq!(ran(body, ENTERO, "-111\n").0, "-99900.00\n");
    for typed in ["200\n", "-112\n"] {
        let (out, exited) = ran(body, ENTERO, typed);
        assert!(exited && out.starts_with("NO T0074 al correr, linea 5:"), "typed {typed:?}: {out:?}");
    }
}

#[test]
fn cases_records_and_traits_carry_what_was_typed() {
    let fns = format!("{ENTERO}
enum Cobro
    Hecho(dec(9, 2))
    Falta(dec(9, 2))

fn cobra(saldo: dec(9, 2), importe: dec(9, 2)) -> Cobro
    if importe > saldo
        return Falta(importe - saldo)
    return Hecho(saldo - importe)

trait Crece
    fn crece(mut x: Crece)

type Nave
    nombre: text
    nivel: int

trait Crece for Nave
    fn crece(mut n: Nave)
        n.nivel = n.nivel + 1

fn sube(mut x: Crece)
    crece(mut x)
");
    let body = "    let nombre = lee()\n    let veces = entero(lee())\n    let mut n = Nave { nombre: nombre, nivel: 0 }\n    for i in range(veces)\n        sube(mut n)\n    print(n)\n    match cobra(500.00, n.nivel * 100.25)\n        Hecho(queda)\n            print(\"queda \", queda)\n        Falta(cuanto)\n            print(\"faltan \", cuanto)\n";
    assert_eq!(ran(body, &fns, "centauro\n3\n").0, "Nave { nombre: \"centauro\", nivel: 3 }\nqueda 199.25\n");
    assert_eq!(ran(body, &fns, "vega\n6\n").0, "Nave { nombre: \"vega\", nivel: 6 }\nfaltan 101.50\n");
}

#[test]
fn a_recursion_as_deep_as_was_typed_stops_at_the_line_of_its_call() {
    let fns = format!("{ENTERO}\nfn baja(n: int) -> int\n    if n == 0\n        return 0\n    return 1 + baja(n - 1)\n");
    let body = "    print(baja(entero(lee())))\n";
    assert_eq!(ran(body, &fns, "300\n").0, "300\n");
    let (out, exited) = ran(body, &fns, "1000000\n");
    assert!(exited && out.starts_with("NO T0066 al correr, linea 16:"), "{out:?}");
}

#[test]
fn a_text_that_does_not_fit_is_a_no_never_a_cut() {
    // joined: 248 bytes is the most a text keeps when running
    let body = "    let mut s = lee()\n    while true\n        s = s + s\n";
    let (out, exited) = ran(body, "", "abcdefgh\n");
    assert!(exited && out.starts_with("NO T0060 al correr, linea 6: el texto pasaria de 248 bytes"), "{out:?}");
    // typed: a line that fills what the console keeps may come cut
    let body = "    let s = lee()\n    print(s)\n";
    let full = "x".repeat(126);
    assert_eq!(ran(body, "", &format!("{full}\n")).0, format!("{full}\n"));
    let (out, exited) = ran(body, "", &format!("{}\n", "x".repeat(300)));
    assert!(exited && out.starts_with("NO T0060 al correr, linea 4: la linea tecleada pasa de 126 bytes"), "{out:?}");
}

/// Every program of the bench: (name, its folder, its root file, its text).
fn bench() -> Vec<(String, std::path::PathBuf, String, String)> {
    use std::path::Path;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ejemplos");
    let mut levels: Vec<_> = std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).collect();
    levels.sort();
    let mut all = Vec::new();
    for level in levels {
        let mut items: Vec<_> = std::fs::read_dir(&level).unwrap().map(|e| e.unwrap().path()).collect();
        items.sort();
        for item in items {
            let (dir, rootfile) = if item.is_dir() { (item.clone(), "src/main.titan".to_string()) } else { (level.clone(), item.file_name().unwrap().to_string_lossy().into_owned()) };
            let Ok(src) = std::fs::read_to_string(dir.join(&rootfile)) else { continue };
            let name = format!("{}/{}", level.file_name().unwrap().to_string_lossy(), item.file_name().unwrap().to_string_lossy());
            all.push((name, dir, rootfile, src));
        }
    }
    all
}

/// Runs a `.bex` typing what the program's `# entra:` lines say.
fn console(bex: Vec<u8>, src: &str) -> (String, bool) {
    let input: String = src.lines().filter_map(|l| l.strip_prefix("# entra: ")).map(|l| format!("{l}\n")).collect();
    let mut m = cargar_bex(&bex).unwrap();
    m.poner_entrada(&input);
    let m = run(m, 20_000_000);
    (m.console.clone(), m.exited)
}

/// ** THE ORACLE. Every BIEN program of the bench that does not use the 3060
/// is built TWICE: as the build does it (E0 runs it when compiling, unless it
/// reads) and forced through E1 (it runs in the machine). Both run in the
/// emulator with what the example types, and they must write the SAME,
/// byte for byte -- the calculation is the yardstick of E1.
#[test]
fn e1_writes_what_the_calculation_writes_for_every_program_of_the_bench() {
    let (mut checked, mut wrong) = (0, Vec::new());
    for (name, dir, rootfile, src) in bench() {
        if !src.starts_with("# espera: BIEN") || src.contains("gpu") {
            continue;
        }
        let read = |p: &str| std::fs::read_to_string(dir.join(p)).ok();
        let e0 = console(bmo_titan_x86_64::build_package(&rootfile, &src, &mut |p| read(p)).unwrap(), &src);
        match bmo_titan_x86_64::build_package_e1(&rootfile, &src, &mut |p| read(p)) {
            Ok(bex) => {
                let e1 = console(bex, &src);
                if e1 != e0 {
                    wrong.push(format!("{name}:\n  E0 {:?}\n  E1 {:?}", e0, e1));
                }
            }
            Err(e) => wrong.push(format!("{name}: E1 no lo emite: {e:?}")),
        }
        checked += 1;
    }
    assert!(wrong.is_empty(), "{} de {} programas escriben otra cosa por E1:\n{}", wrong.len(), checked, wrong.join("\n"));
    assert!(checked >= 25, "the oracle looked at {checked} programs");
    eprintln!("oraculo: {checked} programas, E1 == calculo");
}

/// ** THE ORACLE OF THE NO. The NOs of the bench that the calculation finds
/// by RUNNING the program (it overflows, divides by zero, does not end
/// exact, leaves the table, breaks a PIC, does not stop) are the SAME NO
/// when E1 runs it: same code, same line. Only a loop without end is
/// different, and honestly so: the machine has no budget, it keeps going.
#[test]
fn e1_traps_where_the_calculation_said_no() {
    let runtime = ["T0060", "T0061", "T0062", "T0066", "T0072", "T0074"];
    let (mut checked, mut wrong) = (0, Vec::new());
    for (name, dir, rootfile, src) in bench() {
        let Some(code) = src.lines().next().and_then(|l| l.strip_prefix("# espera: ")) else { continue };
        if !runtime.contains(&code) {
            continue;
        }
        let read = |p: &str| std::fs::read_to_string(dir.join(p)).ok();
        let line = match bmo_titan_x86_64::build_package(&rootfile, &src, &mut |p| read(p)) {
            Err(bmo_titan_x86_64::Failure::Source(m)) => m.line,
            other => panic!("{name}: the calculation says {code}, and gave {:?}", other.map(|_| ())),
        };
        let bex = bmo_titan_x86_64::build_package_e1(&rootfile, &src, &mut |p| read(p)).unwrap_or_else(|e| panic!("{name}: E1 no lo emite: {e:?}"));
        // a loop without end runs until the emulator's budget stops it
        let ran = std::panic::catch_unwind(|| console(bex, &src));
        let (out, exited) = ran.unwrap_or_default();
        let want = format!("NO {code} al correr, linea {line}:");
        let endless = code == "T0066" && !exited && out.is_empty();
        if !(exited && out.starts_with(&want)) && !endless {
            wrong.push(format!("{name}: E0 {code} en la linea {line}, E1 {out:?} (acabo: {exited})"));
        }
        checked += 1;
    }
    assert!(wrong.is_empty(), "{} de {} NO no coinciden:\n{}", wrong.len(), checked, wrong.join("\n"));
    assert!(checked >= 8, "the oracle of the NO looked at {checked} programs");
    eprintln!("oraculo del NO: {checked} programas, el mismo NO");
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A number expression over `a`, `b`, `c` (typed ints) and literals.
fn expr(r: &mut Rng, depth: u32) -> String {
    if depth == 0 || r.pick(3) == 0 {
        return match r.pick(7) {
            0 => "a".into(),
            1 => "b".into(),
            2 => "c".into(),
            3 => format!("{}", r.pick(20) as i64 - 5),
            4 => format!("{}.{}", r.pick(30), r.pick(100)),
            5 => format!("{}.{:03}", r.pick(5), r.pick(1000)),
            _ => format!("{}", [0i64, 1, 3, 1000, 4611686018427387904, 9223372036854775807][r.pick(6) as usize]),
        };
    }
    let (x, y) = (expr(r, depth - 1), expr(r, depth - 1));
    let int = |r: &mut Rng| ["a", "b", "c", "-1", "7"][r.pick(5) as usize];
    match r.pick(8) {
        7 => format!("({} % {})", int(r), int(r)),
        0 => format!("({x} + {y})"),
        1 => format!("({x} - {y})"),
        2 => format!("({x} * {y})"),
        3 => format!("({x} / {y})"),
        4 => format!("round({x} / {y}, {})", r.pick(4)),
        5 => format!("(-{x})"),
        _ => format!("round({x}, {})", r.pick(3)),
    }
}

/// ** E1 AGAINST THE CALCULATION, AT RANDOM. The numbers come TYPED (E1 cannot
/// know them when compiling); the same program with the numbers WRITTEN is
/// run by the calculation (E0). Either both write the same line, or the NO
/// the calculation gives when compiling (T0060, T0061, T0062) is the NO E1
/// gives when running -- with the same code.
#[test]
fn e1_and_the_calculation_agree_on_random_arithmetic() {
    let head = "mod main \"azar\"\n\nfn entero(t: text) -> int\n    match numero(t)\n        Es(n)\n            return n\n        NoEs\n            return 0\n\nfn main()\n";
    // E1_AZAR=semilla,casos para buscar mas lejos (los de siempre: 400)
    let env = std::env::var("E1_AZAR").unwrap_or_default();
    let mut it = env.split(',').map(|x| x.trim().parse::<u64>().ok());
    let seed = it.next().flatten().filter(|s| *s != 0).unwrap_or(0x9E37_79B9_7F4A_7C15);
    let cases = it.next().flatten().unwrap_or(400);
    let mut r = Rng(seed);
    let (mut same, mut traps, mut wrong) = (0, 0, Vec::new());
    for case in 0..cases {
        let vals: Vec<i64> = (0..3).map(|_| [0i64, 1, -1, 2, 7, 10, 100, 12345, -98765, 3037000499, 9223372036854775807, -9223372036854775808][r.pick(12) as usize]).collect();
        // un numero, o dos comparados (los 128 bits de alinear: 0.001 < 2^62)
        let e = match r.pick(4) {
            0 => format!("{} {} {}", expr(&mut r, 2), ["<", "<=", ">", ">=", "==", "!="][r.pick(6) as usize], expr(&mut r, 2)),
            _ => expr(&mut r, 3),
        };
        let typed = format!("{head}    let a = entero(lee())\n    let b = entero(lee())\n    let c = entero(lee())\n    print({e})\n");
        // -9223372036854775808 escrito es -(9223372036854775808): no cabe
        let lit = |v: i64| if v == i64::MIN { "(-9223372036854775807 - 1)".to_string() } else { v.to_string() };
        let written = format!("{head}    let a = {}\n    let b = {}\n    let c = {}\n    print({e})\n", lit(vals[0]), lit(vals[1]), lit(vals[2]));
        let input: String = vals.iter().map(|v| format!("{v}\n")).collect();
        let e0 = match bmo_titan_x86_64::build(&written, "azar.titan") {
            Ok(bex) => {
                let m = run(cargar_bex(&bex).unwrap(), 2_000_000);
                Ok(m.console.clone())
            }
            Err(bmo_titan_x86_64::Failure::Source(m)) => Err(format!("T{:04}", m.code.number())),
            Err(other) => panic!("case {case}: {other:?}\n{written}"),
        };
        let bex = bmo_titan_x86_64::build(&typed, "azar.titan").unwrap_or_else(|f| panic!("case {case}: E1 does not build: {f:?}\n{typed}"));
        let mut m = cargar_bex(&bex).unwrap();
        m.poner_entrada(&input);
        let out = run(m, 20_000_000).console.clone();
        match &e0 {
            Ok(text) if *text == out => same += 1,
            Err(code) if out.starts_with(&format!("NO {code} al correr")) => traps += 1,
            _ => wrong.push(format!("case {case}: a={} b={} c={} print({e})\n  E0 {:?}\n  E1 {:?}", vals[0], vals[1], vals[2], e0, out)),
        }
    }
    assert!(wrong.is_empty(), "{} de {cases} no coinciden:\n{}", wrong.len(), wrong.iter().take(15).cloned().collect::<Vec<_>>().join("\n"));
    eprintln!("azar: {same} iguales, {traps} con el mismo NO");
    assert!(same > 100 && traps > 30, "the random cases cover both: {same} same, {traps} traps");
}

/// ** LISTS AND MAPS IN THE MACHINE (level 13, `docs/plan/PLAN_LISTAS_Y_MAPAS.md`,
/// L6): each program built by the calculation (E0) and FORCED through E1 must
/// write the same, byte for byte -- and E1 says it if anything of the heap
/// was left unfreed at the end, so "the same" also means "nothing leaked".
#[test]
fn lists_and_maps_run_in_the_machine_as_the_calculation_says() {
    let programs: &[&str] = &[
        // push, pop, index, for, a cell changed, len
        "fn main()\n    let mut l: [int] = []\n    for i in range(10)\n        push(mut l, i * i)\n    print(l, \" \", len(l), \" \", l[3])\n    let u = pop(mut l)\n    match u\n        Hay(x)\n            print(\"saco \", x)\n        NoHay\n            print(\"nada\")\n    l[0] = 100\n    let mut s = 0\n    for x in l\n        s = s + x\n    print(s, \" \", l)\n",
        // pop until empty, and once more
        "fn main()\n    let mut l: [text] = [\"a\", \"b\"]\n    for i in range(3)\n        match pop(mut l)\n            Hay(t)\n                print(t)\n            NoHay\n                print(\"vacia \", l)\n",
        // copies are values: b does not change with a
        "fn main()\n    let mut a: [int] = [1, 2]\n    let b = a\n    push(mut a, 3)\n    let c = [a, b]\n    print(a, \" \", b, \" \", len(c))\n",
        // maps: put, replace, get, has, remove, for over keys, written maps, equality
        "fn main()\n    let mut stock: {text: int} = {}\n    put(mut stock, \"pan\", 3)\n    put(mut stock, \"leche\", 2)\n    put(mut stock, \"pan\", 5)\n    put(mut stock, \"sal\", 1)\n    print(stock, \" \", len(stock), \" \", has(stock, \"leche\"), \" \", has(stock, \"te\"))\n    remove(mut stock, \"leche\")\n    for k in stock\n        match get(stock, k)\n            Hay(n)\n                print(k, \"=\", n)\n            NoHay\n                print(\"?\")\n    let m = {1: \"uno\", 2: \"dos\", 1: \"otra vez uno\"}\n    print(m, \" \", m == {2: \"dos\", 1: \"otra vez uno\"}, \" \", stock == {\"sal\": 1, \"pan\": 5}, \" \", stock == {\"pan\": 5})\n",
        // decimals keep their type; lists of records with lists inside
        "type Nave\n    nombre: text\n    carga: [dec]\nfn main()\n    let mut precios: [dec] = [1.5]\n    push(mut precios, 2)\n    print(precios, \" \", precios[1] / 4)\n    let mut flota: [Nave] = []\n    push(mut flota, Nave { nombre: \"a\", carga: [] })\n    push(mut flota, Nave { nombre: \"b\", carga: [1.25, 2] })\n    flota[0].carga = [9.5]\n    let copia = flota\n    flota[1].nombre = \"bb\"\n    print(flota)\n    print(copia == flota, \" \", copia[1].carga == [1.25, 2.00])\n",
        // through fns: a copy, a mut lent, a list given back, Opcion returned
        "fn suma(l: [int]) -> int\n    let mut s = 0\n    for x in l\n        s = s + x\n    return s\nfn crece(mut l: [int], n: int)\n    for i in range(n)\n        push(mut l, i)\nfn pares(n: int) -> [int]\n    let mut out: [int] = []\n    for i in range(n)\n        if i % 2 == 0\n            push(mut out, i)\n    return out\nfn busca(m: {text: int}, k: text) -> Opcion[int]\n    return get(m, k)\nfn main()\n    let mut l: [int] = []\n    crece(mut l, 4)\n    print(l, \" \", suma(l), \" \", pares(9))\n    let m = {\"a\": 1}\n    print(busca(m, \"a\"), \" \", busca(m, \"z\"))\n",
        // lists of lists, many pushes (growth), and a list in a map
        "fn main()\n    let mut t: [[int]] = []\n    for i in range(4)\n        let mut fila: [int] = []\n        for j in range(i)\n            push(mut fila, j)\n        push(mut t, fila)\n    print(t)\n    let mut big: [int] = []\n    for i in range(1000)\n        push(mut big, i)\n    let mut s = 0\n    for x in big\n        s = s + x\n    print(len(big), \" \", s)\n    let mut por: {text: [int]} = {}\n    put(mut por, \"pares\", [0, 2])\n    put(mut por, \"impares\", [1])\n    put(mut por, \"pares\", [0, 2, 4])\n    print(por)\n",
        // a table and a repeat into lists; `l = ...` drops the old one
        "fn main()\n    let mut l: [int] = [0; 5]\n    l = [7, 8]\n    l = [9]\n    push(mut l, 1)\n    let ll: [[int]] = [[1], [2, 3]]\n    print(l, \" \", ll, \" \", ll[1][0], \" \", [] == l)\n",
        // a tree: a type that holds a list of itself, walked by recursion
        "type Nodo\n    valor: int\n    hijos: [Nodo]\nfn suma(n: Nodo) -> int\n    let mut s = n.valor\n    for h in n.hijos\n        s = s + suma(h)\n    return s\nfn main()\n    let hoja = Nodo { valor: 3, hijos: [] }\n    let mut raiz = Nodo { valor: 1, hijos: [] }\n    push(mut raiz.hijos, hoja)\n    print(suma(raiz))\n",
        // an enum that carries a list; a record as the key of a map; take
        "enum Bolsa\n    Vacia\n    Llena([text])\ntype Punto\n    x: int\n    y: int\nfn cuenta(take b: Bolsa) -> int\n    match b\n        Vacia\n            return 0\n        Llena(cosas)\n            return len(cosas)\nfn main()\n    let b = Llena([\"pan\", \"sal\"])\n    let c = b\n    print(b, \" \", cuenta(take b), \" \", c == Llena([\"pan\", \"sal\"]))\n    let mut mundo: {Punto: text} = {}\n    put(mut mundo, Punto { x: 0, y: 0 }, \"piedra\")\n    put(mut mundo, Punto { x: 1, y: 0 }, \"agua\")\n    put(mut mundo, Punto { x: 0, y: 0 }, \"tierra\")\n    print(mundo, \" \", has(mundo, Punto { x: 1, y: 0 }))\n",
        // COBOL's PIC inside a list: what goes in must fit
        "fn main()\n    let mut saldos: [dec(5, 2)] = [1.5]\n    push(mut saldos, 10)\n    print(saldos)\n    push(mut saldos, 1000)\n    print(saldos)\n",
    ];
    let mut wrong = Vec::new();
    for (k, body) in programs.iter().enumerate() {
        let src = format!("mod main \"listas\"\n\n{body}");
        let console = |bex: Vec<u8>| {
            let m = run(cargar_bex(&bex).unwrap(), 50_000_000);
            (m.console.clone(), m.exited)
        };
        let e1 = match bmo_titan_x86_64::build_package_e1("listas.titan", &src, &mut |_| None) {
            Ok(bex) => console(bex),
            Err(e) => (format!("E1 no lo emite: {e:?}"), false),
        };
        // un NO del calculo (al compilar) es el mismo NO de E1 (al correr),
        // en la misma linea, despues de lo que ya escribio
        let same = match bmo_titan_x86_64::build_package("listas.titan", &src, &mut |_| None) {
            Ok(bex) => console(bex) == e1,
            Err(bmo_titan_x86_64::Failure::Source(m)) => e1.0.contains(&format!("NO T{:04} al correr, linea {}:", m.code.number(), m.line)),
            Err(other) => panic!("program {k}: {other:?}"),
        };
        if !same {
            wrong.push(format!("program {k}:\n{src}\n  E1 {:?}", e1));
        }
    }
    assert!(wrong.is_empty(), "{} de {} programas de listas escriben otra cosa por E1:\n{}", wrong.len(), programs.len(), wrong.join("\n"));
}

/// ** LISTS AND MAPS AT RANDOM: programs of pushes, pops, cells, copies and
/// map entries, built by the calculation and forced through E1. Either both
/// write the same, or the NO the calculation gives when compiling is the NO
/// E1 gives when running -- and nothing of the heap is left (E1 would say).
/// `E1_AZAR_LISTAS=semilla,casos` to look further.
#[test]
fn lists_and_maps_agree_with_the_calculation_at_random() {
    let env = std::env::var("E1_AZAR_LISTAS").unwrap_or_default();
    let mut it = env.split(',').map(|x| x.trim().parse::<u64>().ok());
    let seed = it.next().flatten().filter(|s| *s != 0).unwrap_or(0xC0FF_EE12_3456_789B);
    let cases = it.next().flatten().unwrap_or(150);
    let mut r = Rng(seed);
    let keys = ["\"a\"", "\"b\"", "\"c\"", "\"d\""];
    let (mut same, mut traps, mut wrong) = (0, 0, Vec::new());
    for case in 0..cases {
        let mut body = String::from("fn main()\n    let mut l: [int] = []\n    let mut m: {text: int} = {}\n    let mut ll: [[int]] = []\n");
        for step in 0..(4 + r.pick(14)) {
            let n = r.pick(50) as i64 - 10;
            let k = keys[r.pick(4) as usize];
            body += &match r.pick(14) {
                0 | 1 => format!("    push(mut l, {n})\n"),
                2 => format!("    let x{step} = pop(mut l)\n    match x{step}\n        Hay(v)\n            print(\"pop \", v)\n        NoHay\n            print(\"pop nada\")\n"),
                3 => format!("    l[{}] = {n}\n", r.pick(5)),
                4 => format!("    print(l, \" \", len(l))\n"),
                5 => format!("    put(mut m, {k}, {n})\n"),
                6 => format!("    remove(mut m, {k})\n"),
                7 => format!("    match get(m, {k})\n        Hay(v)\n            print({k}, \" \", v)\n        NoHay\n            print({k}, \" no\")\n"),
                8 => format!("    print(m, \" \", has(m, {k}), \" \", len(m))\n"),
                9 => "    push(mut ll, l)\n".into(),
                10 => format!("    let c{step} = l\n    push(mut l, 1)\n    print(c{step} == l, \" \", c{step})\n"),
                11 => format!("    l = [{n}, {}]\n", n + 1),
                12 => format!("    let mut s{step} = 0\n    for v in l\n        s{step} = s{step} + v\n    for q in m\n        s{step} = s{step} + 1\n    print(s{step})\n"),
                _ => format!("    print(ll, \" \", l == [{n}])\n"),
            };
        }
        // los tres cambian al menos una vez: un `mut` que no cambia es un NO
        body += "    push(mut l, 0)\n    put(mut m, \"z\", 0)\n    push(mut ll, l)\n    print(l, m, ll)\n";
        let src = format!("mod main \"azar\"\n\n{body}");
        let e0 = match bmo_titan_x86_64::build_package("azar.titan", &src, &mut |_| None) {
            Ok(bex) => Ok(run(cargar_bex(&bex).unwrap(), 50_000_000).console.clone()),
            Err(bmo_titan_x86_64::Failure::Source(m)) => Err(format!("T{:04}", m.code.number())),
            Err(other) => panic!("case {case}: {other:?}\n{src}"),
        };
        let bex = bmo_titan_x86_64::build_package_e1("azar.titan", &src, &mut |_| None).unwrap_or_else(|f| panic!("case {case}: E1 does not build: {f:?}\n{src}"));
        let out = run(cargar_bex(&bex).unwrap(), 50_000_000).console.clone();
        match &e0 {
            Ok(text) if *text == out => same += 1,
            Err(code) if out.contains(&format!("NO {code} al correr")) => traps += 1,
            _ => wrong.push(format!("case {case}:\n{src}\n  E0 {:?}\n  E1 {:?}", e0, out)),
        }
    }
    assert!(wrong.is_empty(), "{} de {cases} no coinciden:\n{}", wrong.len(), wrong.iter().take(3).cloned().collect::<Vec<_>>().join("\n"));
    eprintln!("azar de listas: {same} iguales, {traps} con el mismo NO");
    assert!(same > 30 && traps > 10, "the random cases cover both: {same} same, {traps} traps");
}
