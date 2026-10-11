//! ** TA5 de `docs/plan/PLAN_LA_TINTA.md` (10-10): LA MEDIDA, escrita ANTES
//! de prometer latencia. Cuantas instrucciones le cuesta a E1 cada pixel en
//! los tres bucles de un programa de dibujo:
//!
//! ```text
//!    pintar el lienzo     px[i] = byte(v)          una celda de [byte]
//!    leer el lienzo       v = px[i]                otra
//!    a la ventana         director.pixel(x, y, c)  la superficie del DIRECTOR
//! ```
//!
//! Se mide en el emulador, que cuenta las instrucciones: un programa con N
//! pixeles y otro con 2N, y la resta entre N (lo fijo -- cargar, abrir la
//! ventana -- se va en la resta). Cada numero tiene su TECHO: si E1 se
//! vuelve mas lento, la prueba lo dice. Lo que dura de verdad lo dice el
//! metal con `director.ms()`, que se prueba aqui contra el reloj del
//! emulador (1 GHz: una instruccion, un ns).

use bmo_lower::emu::{cargar_bex, run, Machine};

const SCREEN: &str = "[package]\nname = \"x\"\n\n[permissions]\nscreen = true\n";

fn corre(main: &str) -> Machine {
    let bex = bmo_titan_x86_64::build_package("src/main.titan", main, &mut |p| (p == "Titan.toml").then(|| SCREEN.to_string())).unwrap_or_else(|e| panic!("{:?}", e));
    let mut m = cargar_bex(&bex).expect("el .bex carga");
    m.padre = 7;
    let m = run(m, 900_000_000);
    assert!(m.exited, "{}", m.console);
    m
}

/// Las instrucciones por pixel de `cuerpo` (con `N` dentro), por la resta.
fn por_pixel(cuerpo: &str) -> f64 {
    let pasos = |n: u64| {
        let src = format!("mod main \"medida\"\nuse director\n\nfn main()\n    let r = director.ventana(640, 360)\n    let N = {n}\n{cuerpo}    print(r)\n");
        let m = corre(&src);
        assert_eq!(m.console, "true\n", "{}", src);
        m.pasos
    };
    let n = 100_000u64;
    (pasos(2 * n) - pasos(n)) as f64 / n as f64
}

/// Los tres bucles, con su techo. Los numeros de hoy salen por `eprintln`.
#[test]
fn what_a_pixel_costs_in_e1() {
    let pintar = por_pixel("    let mut px: [byte] = []\n    for i in range(N)\n        push(mut px, byte(0))\n    for i in range(N)\n        px[i] = byte(i % 256)\n");
    let crear = por_pixel("    let mut px: [byte] = []\n    for i in range(N)\n        push(mut px, byte(0))\n");
    let leer = por_pixel("    let mut px: [byte] = []\n    for i in range(N)\n        push(mut px, byte(0))\n    let mut s = 0\n    for i in range(N)\n        s = s + px[i]\n");
    let ventana = por_pixel("    for i in range(N)\n        director.pixel(i % 640, i % 360, 16777215)\n");
    let (pintar, leer) = (pintar - crear, leer - crear);
    eprintln!("E1, instrucciones por pixel: crear {crear:.1}, pintar {pintar:.1}, leer {leer:.1}, a la ventana {ventana:.1}");
    // Los techos: lo de hoy con margen. Si suben, E1 se volvio mas lento.
    assert!(crear < 70.0, "crear una celda: {crear}");
    assert!(pintar < 80.0, "pintar una celda: {pintar}");
    assert!(leer < 65.0, "leer una celda: {leer}");
    assert!(ventana < 105.0, "un pixel a la ventana: {ventana}");
}

/// ** `director.ms()`: los milisegundos del reloj -- en el emulador, el de
/// 1 GHz --. Un trabajo de ~5 millones de instrucciones mide ~5 ms.
#[test]
fn the_clock_measures_what_ran() {
    let m = corre("mod main \"reloj\"\nuse director\n\nfn main()\n    let t0 = director.ms()\n    let mut s = 0\n    for i in range(200000)\n        s = s + i % 7\n    let t1 = director.ms()\n    print(t1 - t0)\n    print(s > 0)\n");
    let mut lineas = m.console.lines();
    let ms: u64 = lineas.next().unwrap().parse().unwrap();
    assert_eq!(lineas.next(), Some("true"));
    assert!((2..40).contains(&ms), "el bucle midio {ms} ms en un reloj de 1 GHz");
}
