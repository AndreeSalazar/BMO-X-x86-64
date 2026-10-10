//! ** F4 DE `docs/plan/EL_FOCO.md`: NAVEGAR EN TITAN++, EJECUTADO
//! (`Ultra_userspace/apps/navegar/`). Las MISMAS pruebas que tenia
//! `navegar.inti` (las de `toolchain/lang/inti/emisor-x86_64/tests/
//! navegar.rs`), contra el de TITAN++: sin escritorio, el mensaje por consola;
//! con escritorio, la ventana del mensaje o la de la pagina -- la que ofrece
//! el antenista, o la del disco --, sus NO con nombre y linea, la flecha que
//! desplaza, y la `q` que cierra.
//!
//! ** Y los MISMOS PIXELES que el de INTI: el 10-10, antes de quitarlo, se
//! corrieron los dos sobre el mensaje y sobre la lamina de example.com y
//! dieron la misma ventana, canal a canal (el de TITAN++ es opaco: el alfa a
//! 0xFF). Su huella quedo aqui (`HUELLA_*`): si un dia cambia un pixel, se
//! sabe.

use std::path::{Path, PathBuf};

use bmo_abi::syscalls::surface::{SUP_CABECERA, SUP_EV_CARACTER, SUP_MAGIC};
use bmo_lower::emu::{cargar_bex, run, Machine};

fn navegar() -> Vec<u8> {
    let pkg = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/navegar");
    let src = std::fs::read_to_string(pkg.join("src/main.titan")).expect("el main de NAVEGAR");
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

fn maquina() -> Machine {
    cargar_bex(&navegar()).expect("el .bex carga")
}

/// La lamina de example.com, tal cual la sirve la antena.
fn lamina_ejemplo() -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/antena/ejemplo.lamina")).expect("la lamina de ejemplo")
}

fn q() -> u64 {
    SUP_EV_CARACTER | 0x100 | 0x200 | b'q' as u64
}

/// Lanzado por el escritorio: lo prestado, el disco y los eventos antes de
/// la `q`.
fn con(prestada: Option<&[u8]>, disco: Option<&[u8]>, eventos: &[u64]) -> Machine {
    let mut m = maquina();
    m.padre = 7;
    m.prestamo_pendiente = prestada.map(<[u8]>::to_vec);
    if let Some(d) = disco {
        m.archivos.insert("datos/ejemplo.lam".into(), d.to_vec());
    }
    for &e in eventos {
        m.buzon_pendiente.push_back(e);
    }
    m.buzon_pendiente.push_back(q());
    run(m, 800_000_000)
}

/// La ventana ofrecida: `(ancho, alto, pixel(x, y))`, el pixel SIN su alfa.
fn pantalla(m: &Machine) -> (u64, u64, impl Fn(u64, u64) -> u32 + '_) {
    assert_eq!(m.ofertas.len(), 1, "UNA superficie: {:?}", m.ofertas);
    let (base, desde, _, destino) = m.ofertas[0];
    assert_eq!(destino, 7);
    let s = base + desde;
    let campo = |i: u64| m.read_u64(s + 4 * i) as u32;
    assert_eq!(campo(0), SUP_MAGIC as u32);
    assert_eq!(campo(7), 64, "64 ranuras de buzon");
    let (w, h) = (campo(1) as u64, campo(2) as u64);
    (w, h, move |x: u64, y: u64| m.read_u64(s + SUP_CABECERA + 4 * (y * w + x)) as u32 & 0x00FF_FFFF)
}

fn cuenta(pixel: &dyn Fn(u64, u64) -> u32, w: u64, y0: u64, y1: u64, color: u32) -> usize {
    (y0..y1).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| pixel(x, y) == color).count()
}

/// La huella de la ventana: FNV-1a de sus pixeles (sin alfa), fila a fila.
fn huella(m: &Machine) -> u64 {
    let (w, h, pixel) = pantalla(m);
    let mut f = 0xcbf2_9ce4_8422_2325u64;
    for y in 0..h {
        for x in 0..w {
            for b in pixel(x, y).to_le_bytes() {
                f = (f ^ b as u64).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    f
}

#[test]
fn sin_nadie_que_componga_dice_que_falta_la_antena_por_consola() {
    let m = run(maquina(), 50_000_000);
    assert!(m.exited, "{}", m.console);
    let d = &m.console;
    assert!(d.starts_with("NAVEGAR"), "empieza por su nombre: {:?}", d);
    assert!(d.contains("ANTENA"), "nombra a la antena: {:?}", d);
    assert!(d.contains("Ninguna conectada"), "dice que no hay ninguna: {:?}", d);
    assert!(d.contains("no finge que si"), "y que no finge: {:?}", d);
    assert!(d.contains("toolchain/tools/antena"), "y donde esta la antena: {:?}", d);
    assert_eq!(d.matches('\n').count(), 4, "cuatro lineas: {:?}", d);
    assert!(m.ofertas.is_empty(), "sin padre no se ofrece ninguna superficie");
}

#[test]
fn con_escritorio_abre_una_ventana_con_el_mensaje_y_la_q_la_cierra() {
    let m = con(None, None, &[]);
    assert!(m.exited, "{}", m.console);
    let (w, h, pixel) = pantalla(&m);
    assert_eq!((w, h), (480, 112));
    assert!(m.read_u64(m.ofertas[0].0 + 20) as u32 >= 1, "pinto al menos una vez");
    assert!(cuenta(&pixel, w, 0, h, 0x00FF_FFFF) > 200, "el titulo, en blanco");
    assert!(cuenta(&pixel, w, 0, h, 0x00FF_AA00) > 200, "el aviso de la antena, en naranja");
    assert_eq!(pixel(0, 0), 0x0014_1414, "el fondo");
    assert!(m.buzon_pendiente.is_empty(), "el DIRECTOR entrego la tecla");
    assert_eq!(m.console, "", "con ventana no escribe por consola");
}

#[test]
fn con_la_lamina_de_ejemplo_pinta_la_pagina_en_640x400() {
    let m = con(None, Some(&lamina_ejemplo()), &[]);
    let (w, h, pixel) = pantalla(&m);
    assert_eq!((w, h), (640, 400), "la ventana de la lamina, no la del mensaje");
    assert_eq!(pixel(0, 0), 0x00EE_EEEE, "el fondo de la lamina");
    assert_eq!(pixel(639, 399), 0x00EE_EEEE, "hasta la ultima esquina");
    assert!(cuenta(&pixel, w, 121, 153, 0) > 300, "el titulo a escala 2 tiene tinta");
    assert!(cuenta(&pixel, w, 169, 217, 0) > 300, "el parrafo tiene tinta");
    assert!(cuenta(&pixel, w, 231, 247, 0x0033_4488) > 50, "el enlace, en su color");
    assert_eq!(cuenta(&pixel, w, 0, 121, 0), 0, "nada negro antes del titulo");
    assert_eq!(m.console, "");
}

#[test]
fn la_lamina_ofrecida_por_el_antenista_se_pinta_sin_tocar_el_disco() {
    let roja = b"LAMINA 640 400 1\nCAJA 0 0 640 400 ff0000\n";
    let m = con(Some(&lamina_ejemplo()), Some(roja), &[]);
    let (w, h, pixel) = pantalla(&m);
    assert_eq!((w, h), (640, 400));
    assert_eq!(pixel(0, 0), 0x00EE_EEEE, "el fondo de la PRESTADA, no la roja del disco");
    assert!(cuenta(&pixel, w, 121, 153, 0) > 300, "el titulo de example.com, desde la memoria prestada");
    assert_eq!(cuenta(&pixel, w, 0, 400, 0x00FF_0000), 0, "ni un pixel rojo: el disco no se leyo");
}

#[test]
fn sin_oferta_va_al_disco() {
    let roja = b"LAMINA 640 400 1\nCAJA 0 0 640 400 ff0000\n";
    let m = con(None, Some(roja), &[]);
    let (w, _, pixel) = pantalla(&m);
    assert_eq!(pixel(0, 0), 0x00FF_0000, "la del disco");
    assert_eq!(cuenta(&pixel, w, 0, 400, 0x00FF_0000), 640 * 400, "entera");
}

#[test]
fn una_lamina_prestada_y_mal_hecha_se_niega_igual() {
    let mala = b"LAMINA 640 800 2\nCAJA 0 0 640 800 eeeeee\nCAJA 600 0 100 10 ff0000\n";
    let m = con(Some(mala), Some(&lamina_ejemplo()), &[]);
    let (w, _, pixel) = pantalla(&m);
    assert!(cuenta(&pixel, w, 8, 24, 0x00FF_AA00) > 100, "el aviso en naranja");
    assert_eq!(cuenta(&pixel, w, 0, 400, 0x00FF_0000), 0, "la caja de fuera no se pinta");
}

#[test]
fn la_flecha_abajo_desplaza_la_lamina_32_pixeles() {
    let abajo = 0x100 | 0x200 | 0x50;
    let m = con(None, Some(&lamina_ejemplo()), &[abajo]);
    let (w, _, pixel) = pantalla(&m);
    assert!(cuenta(&pixel, w, 89, 121, 0) > 300, "el titulo subio 32 pixeles");
    assert_eq!(cuenta(&pixel, w, 121, 137, 0), 0, "y debajo del titulo ya no hay titulo");
}

#[test]
fn una_lamina_con_una_caja_fuera_se_niega_con_su_nombre() {
    let mala = b"LAMINA 640 800 2\nCAJA 0 0 640 800 eeeeee\nCAJA 600 0 100 10 ff0000\n";
    let m = con(None, Some(mala), &[]);
    let (w, h, pixel) = pantalla(&m);
    assert_eq!((w, h), (640, 400));
    assert_eq!(pixel(0, 0), 0x0014_1414, "el fondo del aviso");
    assert!(cuenta(&pixel, w, 8, 24, 0x00FF_AA00) > 100, "el aviso en naranja");
    assert_eq!(pixel(0, 60), 0x00EE_EEEE, "debajo, lo que se pinto antes del rechazo");
    assert_eq!(cuenta(&pixel, w, 0, 400, 0x00FF_0000), 0, "la caja de fuera no se pinta");
}

#[test]
fn una_lamina_con_un_color_malo_se_niega_en_su_linea() {
    let mala = b"LAMINA 640 800 2\nCAJA 0 0 640 800 eeeeee\nTEXTO 10 10 1 zz0000 hola\n";
    let m = con(None, Some(mala), &[]);
    let (w, _, pixel) = pantalla(&m);
    assert!(cuenta(&pixel, w, 8, 24, 0x00FF_AA00) > 100, "el aviso en naranja");
    assert_eq!(cuenta(&pixel, w, 40, 60, 0), 0, "\"hola\" no se pinto");
}

/// ** LOS MISMOS PIXELES QUE EL DE INTI (10-10): la huella de la ventana del
/// mensaje, de la de example.com, de la desplazada y de la del rechazo.
#[test]
fn las_huellas_del_de_inti() {
    let mala = b"LAMINA 640 800 2\nCAJA 0 0 640 800 eeeeee\nCAJA 600 0 100 10 ff0000\n";
    let huellas = [
        huella(&con(None, None, &[])),
        huella(&con(None, Some(&lamina_ejemplo()), &[])),
        huella(&con(None, Some(&lamina_ejemplo()), &[0x100 | 0x200 | 0x50])),
        huella(&con(None, Some(mala), &[])),
    ];
    assert_eq!(huellas, [HUELLA_MENSAJE, HUELLA_EJEMPLO, HUELLA_ABAJO, HUELLA_RECHAZO]);
}

/// Las de `navegar.inti`, medidas el 10-10 con el mismo FNV-1a sobre sus
/// pixeles (sin alfa) en el mismo emulador: IGUALES a las de TITAN++.
const HUELLA_MENSAJE: u64 = 0x18ed_c5cb_48a5_a3dc;
const HUELLA_EJEMPLO: u64 = 0xe652_d02d_7f41_2b62;
const HUELLA_ABAJO: u64 = 0x4153_6067_684e_2b62;
const HUELLA_RECHAZO: u64 = 0x9ef5_046b_98d9_93e0;
