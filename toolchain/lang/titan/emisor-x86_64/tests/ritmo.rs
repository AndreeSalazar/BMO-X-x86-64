//! ** EL RITMO de `director.espera` (10-10, el cuello de botella de
//! `docs/plan/EL_FOCO.md`): se duerme hasta el PLAZO del fotograma siguiente,
//! no `ms` despues del trabajo. Lo mide el reloj del emulador (1 GHz: cada
//! instruccion un ns, y un WAIT adelanta lo que pidio).
//!
//! S0 de `PLAN_VERRANO` (09-10): el cubo hacia su trabajo y DESPUES dormia
//! 16 ms -- 18,7 ms por fotograma --. Con el plazo, el fotograma mide 16.

use bmo_abi::syscalls::surface::NR_WAIT;
use bmo_lower::emu::{cargar_bex, run, Machine};

const SCREEN: &str = "[package]\nname = \"x\"\n\n[permissions]\nscreen = true\n";

fn corre(cuerpo: &str) -> Machine {
    let src = format!("mod main \"x\"\nuse director\n\nfn main()\n{}", cuerpo);
    let bex = bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| (p == "Titan.toml").then(|| SCREEN.to_string())).unwrap_or_else(|e| panic!("{:?}", e));
    let m = run(cargar_bex(&bex).expect("el .bex carga"), 400_000_000);
    assert!(m.exited, "{}", m.console);
    m
}

/// Lo que pidio dormir cada WAIT, en ns.
fn reposos(m: &Machine) -> Vec<u64> {
    m.syscalls.iter().filter(|c| c.nr == NR_WAIT as u64).map(|c| c.arg0).collect()
}

/// Los ns que pasaron en el reloj del emulador.
fn reloj(m: &Machine) -> u64 {
    m.pasos + m.dormido_ns
}

/// Un programa de `trabajo` vueltas por fotograma y `espera(ms)`, 5 veces.
fn fotogramas(trabajo: u32, ms: u32) -> String {
    format!("    let mut s = 0\n    for f in range(5)\n        for i in range({})\n            s = s + i % 7\n        director.espera({})\n    print(s > 0)\n", trabajo, ms)
}

/// ** Con trabajo de ~2 ms por fotograma, CINCO fotogramas miden 5 x 16 ms
/// (mas el trabajo del primero), no 5 x (2 + 16): el trabajo cae DENTRO del
/// fotograma. El primer reposo es entero; los demas, lo que falta.
#[test]
fn the_frame_lasts_what_was_asked() {
    let m = corre(&fotogramas(50_000, 16));
    let s = reposos(&m);
    assert_eq!(s.len(), 5, "{:?}", s);
    assert_eq!(s[0], 16_000_000, "el primero, entero");
    let trabajo = 16_000_000 - s[1];
    assert!((1_000_000..4_000_000).contains(&trabajo), "un fotograma de trabajo: {} ns", trabajo);
    for &x in &s[1..] {
        assert!(x.abs_diff(s[1]) < 50_000, "todos duermen lo mismo: {:?}", s);
    }
    let total = reloj(&m);
    assert!(total < 5 * 16_000_000 + trabajo + 1_000_000, "cinco fotogramas de 16 ms y el trabajo del primero: {} ns", total);
    assert!(total > 5 * 16_000_000, "{} ns", total);
}

/// ** Tarde (trabajo de ~20 ms en fotogramas de 16): no duerme -- CEDE el
/// turno (WAIT de 0) -- y se pone en hora: no amontona fotogramas.
#[test]
fn late_it_yields_and_catches_up() {
    let m = corre(&fotogramas(500_000, 16));
    let s = reposos(&m);
    assert_eq!(s.len(), 5);
    assert_eq!(s[0], 16_000_000);
    assert!(s[1..].iter().all(|&x| x == 0), "tarde, cede sin dormir: {:?}", s);
}

/// ** Mas de un segundo, la siesta de siempre; y 0 o menos, nada.
#[test]
fn long_sleeps_and_none() {
    let m = corre("    director.espera(1500)\n    director.espera(0)\n    director.espera(-3)\n    print(1)\n");
    assert_eq!(reposos(&m), vec![1_500_000_000]);
}
