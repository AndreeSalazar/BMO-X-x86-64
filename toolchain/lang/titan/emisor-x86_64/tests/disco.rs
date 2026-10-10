//! ** TA4 EN EL ANFITRION: lo que un programa de TITAN++ ESCRIBE en el disco
//! byte a byte (`director.crea`, `escribe`, `cierra`) -- lo que piden las
//! herramientas de imagen que fueron de INTI (1d de `docs/plan/EL_FOCO.md`).
//! `director.guarda` (el fichero entero de un texto) se prueba en
//! `entrada.rs`.

use bmo_lower::emu::{cargar_bex, run, Machine};

const SCREEN: &str = "[package]\nname = \"x\"\n\n[permissions]\nscreen = true\n";

fn corre(cuerpo: &str, preparar: impl FnOnce(&mut Machine)) -> Machine {
    let src = format!("mod main \"x\"\nuse director\n\nfn main()\n{}", cuerpo);
    let bex = bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| (p == "Titan.toml").then(|| SCREEN.to_string())).unwrap_or_else(|e| panic!("{:?}", e));
    let mut m = cargar_bex(&bex).expect("el .bex carga");
    preparar(&mut m);
    let m = run(m, 20_000_000);
    assert!(m.exited, "{}", m.console);
    m
}

/// ** Veinte bytes -- dos palabras de siete y una de seis --, cada uno el
/// suyo; y el fichero, entero, solo al cerrar.
#[test]
fn byte_by_byte_and_whole_at_the_close() {
    let m = corre("    print(director.crea(\"datos/x.bin\"))\n    for i in range(20)\n        director.escribe(i * 13)\n    print(director.cierra())\n", |_| {});
    assert_eq!(m.console, "true\ntrue\n");
    let esperado: Vec<u8> = (0..20).map(|i| (i * 13) as u8).collect();
    assert_eq!(m.archivos.get("datos/x.bin"), Some(&esperado));
    // Siete justos, y ninguno.
    let m = corre("    print(director.crea(\"datos/siete\"), \" \", director.cierra())\n    print(director.crea(\"datos/7\"))\n    for i in range(7)\n        director.escribe(255 - i)\n    print(director.cierra())\n", |_| {});
    assert_eq!(m.console, "true true\ntrue\ntrue\n");
    assert_eq!(m.archivos.get("datos/siete").map(Vec::len), Some(0));
    assert_eq!(m.archivos.get("datos/7"), Some(&vec![255, 254, 253, 252, 251, 250, 249]));
}

/// ** Un byte fuera de 0..255 NO se recorta: no se escribe, y `cierra` dice
/// que no (lo demas si llega: el kernel guarda lo que tiene).
#[test]
fn a_byte_out_of_range_is_refused_not_cut() {
    for malo in ["256", "-1", "1000000"] {
        let m = corre(&format!("    if director.crea(\"datos/x\")\n        director.escribe(1)\n        director.escribe({})\n        director.escribe(2)\n        print(director.cierra())\n", malo), |_| {});
        assert_eq!(m.console, "false\n", "{}", malo);
        assert_eq!(m.archivos.get("datos/x"), Some(&vec![1, 2]), "{}", malo);
    }
}

/// ** UNO a la vez; sin crear, escribir no hace nada y cerrar dice que no;
/// sin ruta, no; y el disco que dice que no al cerrar.
#[test]
fn the_ones_that_say_no() {
    let m = corre("    print(director.crea(\"datos/a\"), \" \", director.crea(\"datos/b\"), \" \", director.cierra(), \" \", director.cierra())\n    director.escribe(5)\n    print(director.crea(\"\"), \" \", director.crea(\"datos/b\"), \" \", director.cierra())\n", |_| {});
    assert_eq!(m.console, "true false true false\nfalse true true\n");
    assert_eq!(m.archivos.get("datos/a").map(Vec::len), Some(0));
    assert_eq!(m.archivos.get("datos/b").map(Vec::len), Some(0), "el 5 llego sin fichero abierto: no se escribe");
    let m = corre("    print(director.crea(\"datos/no\"))\n    director.escribe(9)\n    print(director.cierra())\n", |m| m.fallar_al_guardar("datos/no"));
    assert_eq!(m.console, "true\nfalse\n");
    assert!(!m.archivos.contains_key("datos/no"));
}

/// ** Escribir no toca lo TENIDO: se lee un fichero, se escribe otro con lo
/// leido, y lo tenido sigue ahi.
#[test]
fn writing_keeps_what_is_held() {
    let m = corre("    if director.fichero(\"datos/in\") and director.crea(\"datos/out\")\n        for i in range(director.medida())\n            director.escribe(director.byte(director.medida() - 1 - i))\n        print(director.cierra(), \" \", director.medida(), \" \", director.byte(0))\n", |m| {
        m.archivos.insert("datos/in".into(), b"BMO-X TITAN".to_vec());
    });
    assert_eq!(m.console, "true 11 66\n");
    assert_eq!(m.archivos.get("datos/out").map(Vec::as_slice), Some(&b"NATIT X-OMB"[..]));
}

/// ** Lo que el compilador dice: `escribe` no da nada, y lleva un int.
#[test]
fn what_the_compiler_says() {
    let no = |c: &str| {
        let src = format!("mod main \"x\"\nuse director\n\nfn main()\n{}", c);
        format!("{:?}", bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| (p == "Titan.toml").then(|| SCREEN.to_string())).expect_err("tiene que decir que no"))
    };
    assert!(no("    let x = director.escribe(1)\n").contains("si todo entro lo dice"));
    assert!(no("    director.escribe(\"a\")\n").contains("un int de 0 a 255"));
    assert!(no("    print(director.crea(3))\n").contains("RUTA"));
}
