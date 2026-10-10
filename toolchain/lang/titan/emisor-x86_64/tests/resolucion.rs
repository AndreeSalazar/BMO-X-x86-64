//! ** R1 DE `docs/plan/EL_FOCO.md`: RESOLUCION EN TITAN++, EJECUTADA
//! (`Ultra_userspace/apps/resolucion/`). La app que elige la medida de las
//! ventanas de TITAN++ y la GUARDA (`director.guarda`, TA4): sin escritorio,
//! lo dice; con el, la lista con la elegida marcada, las flechas, las cifras
//! y el raton que eligen, Enter que guarda, el disco que dice que no, y la
//! que ya estaba guardada, elegida al abrirse.
//!
//! ** Y quien la LEE: el ejemplo del banco `nivel11/ventana` se abre con la
//! medida guardada, y sin ella con la suya (320 x 200).

use std::path::Path;

use bmo_abi::syscalls::surface::{SUP_CABECERA, SUP_EV_CARACTER, SUP_EV_RATON};
use bmo_lower::emu::{cargar_bex, run, Machine};

const RUTA: &str = "datos/resolucion.txt";

fn paquete(dir: &Path) -> Vec<u8> {
    let src = std::fs::read_to_string(dir.join("src/main.titan")).expect("el main");
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(dir.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

fn resolucion() -> Vec<u8> {
    paquete(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/resolucion"))
}

/// Una tecla pulsada (su scancode) y una letra.
fn tecla(c: u64) -> u64 {
    0x100 | 0x200 | c
}
fn letra(c: u8) -> u64 {
    SUP_EV_CARACTER | 0x100 | 0x200 | c as u64
}
const ARRIBA: u64 = 72;
const ABAJO: u64 = 80;
const ENTER: u64 = 28;

/// Lanzada por el escritorio con `guardado` en el disco (o nada), los
/// eventos y Esc.
fn con(guardado: Option<&str>, eventos: &[u64], disco_falla: bool) -> Machine {
    let mut m = cargar_bex(&resolucion()).expect("el .bex carga");
    m.padre = 7;
    if let Some(g) = guardado {
        m.archivos.insert(RUTA.into(), g.as_bytes().to_vec());
    }
    if disco_falla {
        m.fallar_al_guardar(RUTA);
    }
    for &e in eventos {
        m.buzon_pendiente.push_back(e);
    }
    m.buzon_pendiente.push_back(letra(27));
    let m = run(m, 400_000_000);
    assert!(m.exited, "{}", m.console);
    m
}

fn guardado(m: &Machine) -> Option<String> {
    m.archivos.get(RUTA).map(|b| String::from_utf8(b.clone()).expect("texto"))
}

/// La ventana: `(ancho, alto, pixel(x, y))`, sin alfa.
fn pantalla(m: &Machine) -> (u64, u64, impl Fn(u64, u64) -> u32 + '_) {
    assert_eq!(m.ofertas.len(), 1, "UNA superficie: {:?}", m.ofertas);
    let (base, desde, _, destino) = m.ofertas[0];
    assert_eq!(destino, 7);
    let s = base + desde;
    let (w, h) = (m.read_u64(s + 4) as u32 as u64, m.read_u64(s + 8) as u32 as u64);
    (w, h, move |x: u64, y: u64| m.read_u64(s + SUP_CABECERA + 4 * (y * w + x)) as u32 & 0x00FF_FFFF)
}

const ELEGIDA: u32 = 0x33_66_99;
const FONDO: u32 = 0x14_14_14;

/// La fila marcada: la unica con el color de la elegida a su izquierda.
fn marcada(m: &Machine) -> usize {
    let (w, h, pixel) = pantalla(m);
    assert_eq!((w, h), (360, 296));
    let filas: Vec<usize> = (0..8).filter(|i| pixel(9, 53 + 24 * *i as u64) == ELEGIDA).collect();
    assert_eq!(filas.len(), 1, "una marcada: {:?}", filas);
    for i in 0..8 {
        if i != filas[0] {
            assert_eq!(pixel(9, 53 + 24 * i as u64), FONDO);
        }
    }
    filas[0]
}

/// ** Sin escritorio: lo dice y no espera nada.
#[test]
fn from_the_shell_it_says_so() {
    let m = run(cargar_bex(&resolucion()).expect("el .bex carga"), 50_000_000);
    assert!(m.exited);
    assert_eq!(m.console, "RESOLUCION es una ventana: abrela desde el escritorio\n");
}

/// ** Sin nada guardado: 1280 x 720 marcada; abajo y Enter guarda 1366 x
/// 768; las flechas no pasan de la primera ni de la ultima.
#[test]
fn arrows_choose_and_enter_saves() {
    let m = con(None, &[], false);
    assert_eq!((marcada(&m), guardado(&m)), (3, None), "sin tocar nada, nada se guarda");
    let m = con(None, &[tecla(ABAJO), tecla(ENTER)], false);
    assert_eq!((marcada(&m), guardado(&m).as_deref()), (4, Some("1366 768\n")));
    let arriba = [tecla(ARRIBA); 6];
    let m = con(None, &[&arriba[..], &[tecla(ENTER)]].concat(), false);
    assert_eq!(guardado(&m).as_deref(), Some("640 360\n"));
    let abajo = [tecla(ABAJO); 9];
    let m = con(None, &[&abajo[..], &[tecla(ENTER)]].concat(), false);
    assert_eq!(guardado(&m).as_deref(), Some("2560 1440\n"));
}

/// ** Lo guardado se elige al abrirse; las cifras y el raton eligen; una
/// medida que no es de la lista, o mal escrita, deja 1280 x 720.
#[test]
fn what_was_saved_digits_and_the_mouse() {
    assert_eq!(marcada(&con(Some("1920 1080\n"), &[], false)), 6);
    assert_eq!(marcada(&con(Some("1920 1080"), &[], false)), 6, "sin salto de linea, tambien");
    for raro in ["1921 1080\n", "1920x1080\n", "1920 1080 7\n", "", "123456 7\n"] {
        assert_eq!(marcada(&con(Some(raro), &[], false)), 3, "{:?}", raro);
    }
    let m = con(Some("800 600\n"), &[letra(b'6'), tecla(ENTER)], false);
    assert_eq!(guardado(&m).as_deref(), Some("1600 900\n"));
    // El raton: la fila de la y 52 + 24 * 2 .. + 24, con el boton.
    let raton = SUP_EV_RATON | 0x100 | 0x200 | 1 | 40 << 16 | 110 << 32;
    let m = con(None, &[raton, tecla(ENTER)], false);
    assert_eq!(guardado(&m).as_deref(), Some("1024 768\n"));
    let sin_boton = SUP_EV_RATON | 0x100 | 40 << 16 | 110 << 32;
    assert_eq!(marcada(&con(None, &[sin_boton], false)), 3, "moverlo no elige");
}

/// ** El disco que dice que no: no queda NADA guardado, y se ve.
#[test]
fn the_disk_says_no() {
    let m = con(None, &[tecla(ENTER)], true);
    assert_eq!(guardado(&m), None);
    let (_, _, pixel) = pantalla(&m);
    let aviso = (12..200).flat_map(|x| (272..288).map(move |y| (x, y))).filter(|&(x, y)| pixel(x, y) == 0xFF_AA_00).count();
    assert!(aviso > 0, "el aviso, en su color");
}

/// ** QUIEN LA LEE: `nivel11/ventana` se abre con la medida guardada --
/// el degradado entero de lado a lado, la barra a media altura --, y sin
/// ella (o rara) con la suya.
#[test]
fn the_bank_window_takes_the_saved_size() {
    let ventana = paquete(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../ejemplos/nivel11/ventana"));
    for (disco, w, h) in [(Some("640 360\n"), 640, 360), (None, 320, 200), (Some("10 10\n"), 320, 200)] {
        let mut m = cargar_bex(&ventana).expect("el .bex carga");
        m.padre = 7;
        if let Some(d) = disco {
            m.archivos.insert(RUTA.into(), d.as_bytes().to_vec());
        }
        let m = run(m, 400_000_000);
        assert!(m.exited, "{}", m.console);
        let (aw, ah, pixel) = pantalla(&m);
        assert_eq!((aw, ah), (w, h), "{:?}", disco);
        assert_eq!((pixel(0, 0), pixel(w - 1, 39)), (0xFF_00_00, 0xFF_FC_00), "el degradado, de rojo a amarillo");
        let bar = (1198 % (w - 20)) as u64;
        assert_eq!((pixel(bar, h / 2), pixel(bar + 19, h / 2 + 39)), (0xFF_FF_FF, 0xFF_FF_FF), "la barra, a media altura");
    }
}
