//! ** F2 y F3 DE `docs/plan/EL_FOCO.md`, EN EL ANFITRION: lo que la ventana
//! de un programa de TITAN++ LEE -- lo que le ofrecieron, o un fichero -- y lo
//! que le LLEGA por su buzon; y la LETRA de BMO-X en ella.
//!
//! El emulador hace de kernel y de DIRECTOR de mentira: `prestamo_pendiente`
//! es lo que el antenista ofrece, `archivos` el disco, y `buzon_pendiente` lo
//! que el DIRECTOR deja en el buzon cada vez que la app duerme.

use bmo_abi::syscalls::surface::{SUP_CABECERA, SUP_EV_CARACTER, SUP_EV_CONFIGURE, SUP_EV_RATON};
use bmo_lower::emu::{cargar_bex, run, Machine};

const SCREEN: &str = "[package]\nname = \"x\"\n\n[permissions]\nscreen = true\n";

fn bex(main: &str) -> Vec<u8> {
    bmo_titan_x86_64::build_package("src/main.titan", main, &mut |p| (p == "Titan.toml").then(|| SCREEN.to_string())).unwrap_or_else(|e| panic!("{:?}", e))
}

fn maquina(main: &str) -> Machine {
    let mut m = cargar_bex(&bex(main)).expect("el .bex carga");
    m.padre = 7;
    m
}

fn programa(cuerpo: &str) -> String {
    format!("mod main \"x\"\nuse director\n\nfn main()\n{}", cuerpo)
}

const VENTANA: &str = "    if not director.ventana(64, 32)\n        print(\"sin ventana\")\n        return\n";

/// ** Lo que OFRECE el antenista: se toma, se mide y se lee byte a byte; y
/// fuera de lo tenido, -1.
#[test]
fn what_was_offered_is_taken_and_read() {
    let src = programa(&format!(
        "{}    print(director.medida(), \" \", director.byte(0))\n    if director.toma()\n        print(director.medida(), \" \", director.byte(0), \" \", director.byte(4), \" \", director.byte(5), \" \", director.byte(-1))\n",
        VENTANA
    ));
    let mut m = maquina(&src);
    m.prestamo_pendiente = Some(b"LAMIN".to_vec());
    let m = run(m, 2_000_000);
    assert!(m.exited, "{}", m.console);
    assert_eq!(m.console, "0 -1\n5 76 78 -1 -1\n");
}

/// ** Sin nada ofrecido, `toma` lo intenta ocho veces -- un fotograma entre
/// una y otra -- y dice que no; sin ventana, no lo intenta.
#[test]
fn nothing_offered_and_no_window() {
    let m = run(maquina(&programa(&format!("{}    print(director.toma(), \" \", director.medida())\n", VENTANA))), 2_000_000);
    assert_eq!(m.console, "false 0\n");
    let m = run(maquina(&programa("    print(director.toma(), \" \", director.byte(0), \" \", director.evento(), \" \", director.se_ve())\n")), 2_000_000);
    assert_eq!(m.console, "false -1 0 false\n");
}

/// ** Un FICHERO entero, por su ruta (de mas de 8 bytes, y no multiplo); y
/// los que no: el que no esta, el vacio y el de mas de 256 KiB.
#[test]
fn a_whole_file_and_the_ones_it_refuses() {
    let src = programa(&format!(
        "{}    print(director.fichero(\"datos/no.lam\"), \" \", director.fichero(\"datos/vacio\"), \" \", director.fichero(\"datos/enorme\"))\n    if director.fichero(\"datos/ejemplo.lam\")\n        print(director.medida(), \" \", director.byte(0), \" \", director.byte(9))\n",
        VENTANA
    ));
    let mut m = maquina(&src);
    m.archivos.insert("datos/ejemplo.lam".into(), b"LAMINA 1 2 0\n".to_vec());
    m.archivos.insert("datos/vacio".into(), Vec::new());
    m.archivos.insert("datos/enorme".into(), vec![1u8; 256 * 1024 + 1]);
    let m = run(m, 20_000_000);
    assert!(m.exited, "{}", m.console);
    assert_eq!(m.console, "false false false\n13 76 50\n");
}

/// ** LA ENTRADA: cada evento del buzon, con lo que es y sus datos -- una
/// tecla pulsada y soltada, una letra, el raton y un cambio de ventana --; y
/// con el buzon vacio, 0.
#[test]
fn the_mailbox_events() {
    let src = programa(&format!(
        "{}    for k in range(6)\n        director.espera(16)\n        let que = director.evento()\n        print(que, \" \", director.codigo(), \" \", director.raton_x(), \" \", director.raton_y(), \" \", director.botones())\n    print(director.se_ve())\n",
        VENTANA
    ));
    let mut m = maquina(&src);
    for e in [0x100 | 0x200 | 80, 0x100 | 72, SUP_EV_CARACTER | 0x100 | 0x200 | b'q' as u64, SUP_EV_RATON | 0x100 | 0x200 | 1 | 30 << 16 | 12 << 32, SUP_EV_CONFIGURE | 0x100 | 640 << 16 | 400 << 32] {
        m.buzon_pendiente.push_back(e);
    }
    let m = run(m, 2_000_000);
    assert!(m.exited, "{}", m.console);
    assert_eq!(m.console, "1 80 0 0 80\n2 72 0 0 72\n3 113 0 0 113\n4 1 30 12 1\n5 0 640 400 0\n0 0 0 0 0\ntrue\n");
}

const BLANCO: u32 = 0xFFFF_FFFF;
/// El fondo de una ventana nueva: todo a cero.
const NEGRO: u32 = 0;

/// Los pixeles de la ventana de 64 x 32 de `m`.
fn pixeles(m: &Machine) -> impl Fn(u64, u64) -> u32 + '_ {
    let s = m.ofertas[0].0;
    move |x, y| m.read_u64(s + SUP_CABECERA + 4 * (y * 64 + x)) as u32
}

/// Las filas del glifo de `b` de la fuente de BMO-X: el ASCII, un extra, o `?`.
fn glifo(b: u8) -> [u8; 16] {
    static FUENTE: [[u8; 16]; 120] = include!("../../../../../Ultra_userspace/userland/src/font16_data.rs");
    static EXTRAS: [u8; 25] = include!("../../../../../Ultra_userspace/userland/src/font16_extra.rs");
    if (32..127).contains(&b) {
        FUENTE[(b - 32) as usize]
    } else {
        EXTRAS.iter().position(|&e| e == b).map(|i| FUENTE[95 + i]).unwrap_or(FUENTE[(b'?' - 32) as usize])
    }
}

/// Que el glifo `b` este en (x0, y0) con escala `e` -- y solo el, sobre negro,
/// en su caja de 8e x 16e --, recortado a la ventana.
fn esta(p: &dyn Fn(u64, u64) -> u32, b: u8, x0: u64, y0: u64, e: u64) {
    let g = glifo(b);
    for y in y0..(y0 + 16 * e).min(32) {
        for x in x0..(x0 + 8 * e).min(64) {
            let (f, c) = ((y - y0) / e, (x - x0) / e);
            let on = g[f as usize] & (0x80 >> c) != 0;
            assert_eq!(p(x, y), if on { BLANCO } else { NEGRO }, "el glifo {:?} en ({}, {}), escala {}", b as char, x, y, e);
        }
    }
}

/// ** LA LETRA: un glifo ASCII, uno extra (n con tilde), uno que no esta (`?`),
/// a escala 2 y recortado por la derecha; y `texto` da la x de detras.
#[test]
fn the_letter_of_bmo_x() {
    let src = programa(&format!(
        "{}    let mut x = director.letra(0, 0, 65, 1, 16777215)\n    x = director.letra(x, 0, 241, 1, 16777215)\n    x = director.letra(x, 0, 200, 1, 16777215)\n    print(x, \" \", director.letra(48, 0, 87, 2, 16777215), \" \", director.letra(0, 0, 66, 0, 255))\n    print(director.texto(0, 16, \"Hi!\", 1, 16777215))\n    director.presenta()\n",
        VENTANA
    ));
    let m = run(maquina(&src), 20_000_000);
    assert!(m.exited, "{}", m.console);
    assert_eq!(m.console, "24 64 0\n24\n");
    let p = pixeles(&m);
    esta(&p, b'A', 0, 0, 1);
    esta(&p, 0xF1, 8, 0, 1);
    esta(&p, 200, 16, 0, 1);
    esta(&p, b'W', 48, 0, 2);
    esta(&p, b'H', 0, 16, 1);
    esta(&p, b'i', 8, 16, 1);
    esta(&p, b'!', 16, 16, 1);
}

/// ** Lo tenido NO pide ventana: se toma o se lee antes, y la ventana se
/// elige despues (NAVEGAR: 640 x 400 con pagina, 480 x 112 sin ella).
#[test]
fn what_is_held_comes_before_the_window() {
    let src = programa("    if director.fichero(\"datos/ejemplo.lam\")\n        print(director.medida(), \" \", director.byte(1))\n    print(director.ventana(64, 32), \" \", director.byte(2))\n");
    let mut m = maquina(&src);
    m.archivos.insert("datos/ejemplo.lam".into(), b"LAMINA".to_vec());
    let m = run(m, 2_000_000);
    assert_eq!(m.console, "6 65\ntrue 77\n");
}
