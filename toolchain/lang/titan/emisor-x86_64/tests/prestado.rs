//! ** PRESTADO PARA LEER (16-10, LE2 de PL18 de `docs/plan/PLAN_ECCE.md`).
//!
//! `fn f(n: T)` es una copia; si `f` solo LEE `n`, la maquina pasa un puntero
//! (`e1/mod.rs`, `solo_se_lee`). La vara:
//!
//! ```text
//!    lo mismo     lo que escribe es lo de la copia, celda a celda
//!    sin copiar   mil llamadas con una lista de 20000 cuestan lo que leerla
//!    lo que se    una fn que se QUEDA la lista (un let, un push, un return)
//!    queda        sigue copiando: el que llama conserva la suya intacta
//! ```

use bmo_lower::emu::{cargar_bex, run, Machine};

const TOML: &str = "[package]\nname = \"x\"\n[permissions]\nscreen = true\n";

/// El programa con `d.bin` de `n` bytes (de 0 a 255, en vuelta) en el disco.
fn corre(cuerpo: &str, n: usize) -> Machine {
    let src = format!("mod main \"prestado\"\nuse director\n\n{cuerpo}");
    let bex = bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| (p == "Titan.toml").then(|| TOML.to_string())).unwrap_or_else(|e| panic!("{e:?}"));
    let mut m = cargar_bex(&bex).unwrap();
    m.archivos.insert("d.bin".into(), (0..n).map(|i| i as u8).collect());
    let m = run(m, 3_000_000_000);
    assert!(m.exited, "{}", m.console);
    m
}

const LEE: &str = "fn carga() -> [byte]\n    let mut a: [byte] = []\n    if director.fichero(\"d.bin\")\n        for i in range(director.medida())\n            push(mut a, byte(director.byte(i)))\n    return a\n\n";

/// ** Solo lee: una celda, `len` y un `for`; mil veces, sin copiar.
#[test]
fn a_list_that_is_only_read_is_not_copied() {
    let cuerpo = format!("{LEE}fn suma(a: [byte], k: int) -> int\n    let mut s = a[k] + len(a)\n    for b in a\n        if b == 255\n            s = s + 1\n    return s\n\nfn celda(a: [byte], k: int) -> int\n    return a[k]\n\nfn main()\n    let a = carga()\n    let mut s = 0\n    for k in range(1000)\n        s = s + celda(a, k)\n    print(s, \" \", suma(a, 3))\n");
    let chico = corre(&cuerpo, 1000);
    let grande = corre(&cuerpo, 20000);
    // Las celdas van de 0 a 255 en vuelta: las mil primeras suman 124716;
    // `suma`: a[3] + len + cuantos 255 hay.
    assert_eq!(chico.console, "124716 1006\n");
    assert_eq!(grande.console, format!("124716 {}\n", 3 + 20000 + 20000 / 256));
    // Mil llamadas con 19000 bytes mas: si se copiara, ~10 pasos por byte y
    // llamada (190 millones). Leerla y recorrerla una vez es mucho menos.
    let extra = grande.pasos - chico.pasos;
    assert!(extra < 12_000_000, "las llamadas copian: {extra} pasos de mas");
}

/// ** La que se QUEDA la lista sigue copiando: cambiarla (en su copia) no
/// toca la del que llama.
#[test]
fn a_function_that_keeps_the_list_still_copies_it() {
    let cuerpo = format!("{LEE}fn otra(a: [byte]) -> [byte]\n    let mut b = a\n    b[0] = byte(99)\n    push(mut b, byte(7))\n    return b\n\nfn guarda(a: [byte]) -> [byte]\n    return a\n\nfn main()\n    let a = carga()\n    let b = otra(a)\n    let c = guarda(a)\n    print(a[0], \" \", len(a), \" \", b[0], \" \", len(b), \" \", c[0], \" \", len(c))\n");
    assert_eq!(corre(&cuerpo, 10).console, "0 10 99 11 0 10\n");
}
