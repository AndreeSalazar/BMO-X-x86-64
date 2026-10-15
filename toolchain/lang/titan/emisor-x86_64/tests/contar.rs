//! ** C4 DE `docs/plan/EL_FOCO.md` (LB5): un programa de TITAN++ se CUENTA a si
//! mismo con los contadores del Zen 3 -- `director.contar()` y
//! `director.contado(k)` --. En el anfitrion el emulador hace de kernel: su
//! contador de instrucciones retiradas es el metro (`pasos`), y los demas
//! dicen 0 (no tiene cache ni predictor). En el Ryzen, los de verdad.

use bmo_lower::emu::{cargar_bex, run};

const SCREEN: &str = "[package]\nname = \"x\"\n\n[permissions]\nscreen = true\n";

fn bex(main: &str) -> Vec<u8> {
    bmo_titan_x86_64::build_package("src/main.titan", main, &mut |p| (p == "Titan.toml").then(|| SCREEN.to_string())).unwrap_or_else(|e| panic!("{:?}", e))
}

/// Un bucle de `n` vueltas, contado: lo que imprime el programa.
fn cuenta(n: u32) -> (u64, u64, u64) {
    let fuente = format!(
        "mod main \"cuenta un bucle\"
use director

fn main()
    if not director.contar()
        print(\"no se cuenta\")
        return
    let a = director.contado(1)
    let mut s = 0
    for i in range({n})
        s = s + i
    let b = director.contado(1)
    print(s, \" \", b - a, \" \", director.contado(0))
"
    );
    let m = run(cargar_bex(&bex(&fuente)).expect("carga"), 5_000_000);
    assert!(m.exited, "{}", m.console);
    let v: Vec<u64> = m.console.trim().split(' ').map(|x| x.parse().unwrap_or_else(|_| panic!("{}", m.console))).collect();
    (v[0], v[1], v[2])
}

/// Lo contado es lineal en las vueltas: lo de fuera del bucle se cancela, y
/// cada vuelta cuesta lo mismo. Un contador que no contara (o que contara
/// otra cosa) no daria la misma diferencia dos veces.
#[test]
fn titan_counts_itself_and_the_count_is_linear_in_the_loop() {
    let (s0, c0, ciclos) = cuenta(0);
    let (s1, c1, _) = cuenta(1000);
    let (s2, c2, _) = cuenta(2000);
    assert_eq!((s0, s1, s2), (0, 499_500, 1_999_000));
    assert_eq!(ciclos, 0, "el emulador no tiene reloj de ciclos: lo dice con 0");
    let por_vuelta = (c1 - c0) / 1000;
    assert!(por_vuelta >= 3, "una vuelta son varias instrucciones: {}", por_vuelta);
    assert_eq!(c1 - c0, 1000 * por_vuelta, "cada vuelta cuesta lo mismo");
    assert_eq!(c2 - c1, c1 - c0, "mil vueltas mas, lo mismo otra vez");
}

/// Sin `director.contar()`, `director.contado` dice 0 (el kernel dice que no).
#[test]
fn without_counting_the_count_is_zero() {
    let m = run(
        cargar_bex(&bex("mod main \"sin contar\"
use director

fn main()
    print(director.contado(1))
"))
        .expect("carga"),
        1_000_000,
    );
    assert!(m.exited, "{}", m.console);
    assert_eq!(m.console.trim(), "0");
}
