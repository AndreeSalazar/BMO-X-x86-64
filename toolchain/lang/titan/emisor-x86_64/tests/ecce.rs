//! ** PL1 y PL3 de `docs/plan/PLAN_ECCE.md` (15-10): ECCE, EL BLOC DE NOTAS
//! (`Ultra_userspace/apps/ecce/`), ejecutado en el emulador con un escritorio
//! de mentira que le manda las teclas.
//!
//! ```text
//!    escribir      las letras entran donde esta el cursor, Enter parte,
//!                  Retroceso y Supr borran; Esc guarda datos/notas.txt
//!    la nota       la de antes se abre (sin los \r de Windows) y el cursor
//!                  queda al final
//!    moverse       flechas, Inicio y Fin; arriba y abajo por la linea que
//!                  SE VE, partida en el ultimo espacio que cabe
//!    nada nuevo    Esc sin escribir no toca el disco
//!    la ventana    la cabecera con su raya de oro, y la tinta de lo escrito
//! ```

use std::path::Path;

use bmo_abi::syscalls::surface::{SUP_CABECERA, SUP_EV_CARACTER};
use bmo_lower::emu::{cargar_bex, run, Machine};

const NOTAS: &str = "datos/notas.txt";

fn ecce() -> Vec<u8> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/ecce");
    let src = std::fs::read_to_string(dir.join("src/main.titan")).expect("el main");
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(dir.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

fn letra(c: u8) -> u64 {
    SUP_EV_CARACTER | 0x100 | 0x200 | c as u64
}

/// Una tecla pulsada, por su scancode (los de navegacion: `bmo_uhid::SC_*`).
fn tecla(sc: u64) -> u64 {
    0x100 | 0x200 | sc
}

const ARRIBA: u64 = 0x66;
const ABAJO: u64 = 0x67;
const IZQUIERDA: u64 = 0x68;
const DERECHA: u64 = 0x69;
const INICIO: u64 = 0x6B;
const SUPR: u64 = 0x6D;
const FIN: u64 = 0x6E;
const RETROCESO: u8 = 8;
const ENTER: u8 = 13;

fn texto(s: &str) -> Vec<u64> {
    s.bytes().map(letra).collect()
}

/// ECCE lanzada por el escritorio, con `nota` en el disco (o ninguna), la
/// medida de la ventana (o ninguna), los eventos y Esc.
fn con(nota: Option<&[u8]>, medida: Option<&str>, eventos: &[u64]) -> Machine {
    let mut m = cargar_bex(&ecce()).expect("el .bex carga");
    m.padre = 7;
    if let Some(n) = nota {
        m.archivos.insert(NOTAS.into(), n.to_vec());
    }
    if let Some(t) = medida {
        m.archivos.insert("datos/resol.txt".into(), t.as_bytes().to_vec());
    }
    for &e in eventos {
        m.buzon_pendiente.push_back(e);
    }
    m.buzon_pendiente.push_back(letra(27));
    let m = run(m, 3_000_000_000);
    assert!(m.exited, "{}", m.console);
    m
}

fn nota(m: &Machine) -> Option<String> {
    m.archivos.get(NOTAS).map(|b| String::from_utf8_lossy(b).into_owned())
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

const FONDO: u32 = 0x18_18_20;
const TINTA: u32 = 0xE8_E2_D0;
const ORO: u32 = 0xD8_A6_57;

#[test]
fn without_a_desktop_it_says_so() {
    let mut m = cargar_bex(&ecce()).unwrap();
    m.padre = 0;
    let m = run(m, 50_000_000);
    assert_eq!(m.console, "ECCE es una ventana: abrela desde el escritorio\n");
    assert!(nota(&m).is_none());
}

/// ** Escribir, partir la linea, borrar con Retroceso, y Esc guarda.
#[test]
fn typing_enter_and_backspace_are_saved_on_esc() {
    let mut ev = texto("hola");
    ev.push(letra(ENTER));
    ev.extend(texto("mundoo"));
    ev.push(letra(RETROCESO));
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("hola\nmundo"));
    assert_eq!(m.console, "");
}

/// ** La nota de antes se abre sin los `\r` de Windows, con el cursor al
/// final; las tildes y la ene son un byte Latin-1 y se guardan igual.
#[test]
fn the_old_note_opens_with_the_cursor_at_the_end() {
    let m = con(Some(b"una\r\nnota "), None, &[letra(0xF1), letra(b'u')]);
    assert_eq!(m.archivos.get(NOTAS).map(|v| v.as_slice()), Some(&b"una\nnota \xF1u"[..]));
}

/// ** Esc sin escribir nada: el disco no se toca (ni se crea una nota vacia).
#[test]
fn esc_without_changes_writes_nothing() {
    let m = con(None, None, &[tecla(IZQUIERDA), tecla(ARRIBA)]);
    assert!(nota(&m).is_none(), "{:?}", nota(&m));
    let m = con(Some(b"igual"), None, &[tecla(INICIO)]);
    assert_eq!(nota(&m).as_deref(), Some("igual"));
}

/// ** Moverse: izquierda, Inicio, Fin, Supr, y arriba y abajo por las lineas
/// partidas por un `\n`, a la misma columna (o al final, si es mas corta).
#[test]
fn arrows_home_end_and_delete_move_through_the_text() {
    // "abc|" -> izquierda dos veces -> "a|bc" -> X -> "aX|bc"
    let mut ev = texto("abc");
    ev.extend([tecla(IZQUIERDA), tecla(IZQUIERDA)]);
    ev.push(letra(b'X'));
    // Inicio va a 0 y Supr quita la `a`: "|Xbc".
    ev.extend([tecla(INICIO), tecla(SUPR)]);
    // Fin y una letra al final; derecha en el final no hace nada.
    ev.extend([tecla(FIN), tecla(DERECHA), letra(b'!')]);
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("Xbc!"));

    // Arriba y abajo: "largo\nab\nfinal", el cursor al final de "final" (col 5).
    // Arriba: "ab" es mas corta, queda en su final (col 2) -> "ab|" + "1".
    // Arriba otra vez: "largo" en col 3 -> "lar|go" + "2".
    // Abajo: "ab1" en col 4 -> su final (3) -> + "3".
    let mut ev = vec![tecla(ARRIBA), letra(b'1'), tecla(ARRIBA), letra(b'2'), tecla(ABAJO), letra(b'3')];
    ev.push(tecla(ABAJO));
    ev.push(tecla(ABAJO));
    ev.push(letra(b'4'));
    let m = con(Some(b"largo\nab\nfinal"), None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("lar2go\nab13\nfina4l"));
}

/// ** Las lineas que SE VEN: una ventana estrecha parte en el ultimo espacio
/// que cabe, y arriba sube por esas lineas, no por los `\n`.
#[test]
fn up_moves_through_the_wrapped_lines() {
    // 240 de ancho: (240 - 2 * 28) / 8 = 23 columnas.
    // "aaaa bbbb cccc dddd eeee ffff" son 29: la letra 24 cae en "eeee", asi
    // que parte en el espacio de antes: "aaaa bbbb cccc dddd" + "eeee ffff".
    // El cursor al final (col 9 de "eeee ffff"); arriba va a la col 9 de la
    // primera: "aaaa bbbb|" -> "X".
    let m = con(Some(b"aaaa bbbb cccc dddd eeee ffff"), Some("240 160\n"), &[tecla(ARRIBA), letra(b'X')]);
    assert_eq!(nota(&m).as_deref(), Some("aaaa bbbbX cccc dddd eeee ffff"));
}

/// ** Retroceso al principio y Supr al final no rompen nada; Tab son cuatro
/// espacios.
#[test]
fn edges_do_nothing_and_tab_is_four_spaces() {
    let mut ev = vec![letra(RETROCESO), tecla(SUPR), letra(b'a'), letra(9), letra(b'b'), tecla(SUPR)];
    ev.extend([tecla(INICIO), letra(RETROCESO)]);
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("a    b"));
}

/// ** La ventana: la cabecera con su raya de oro, el fondo de noche, y la
/// tinta de lo escrito donde va la primera letra.
#[test]
fn the_window_shows_the_ink_on_the_night() {
    let m = con(Some(b"M"), None, &[]);
    let (w, h, pixel) = pantalla(&m);
    assert_eq!((w, h), (720, 480));
    // La raya de oro bajo la cabecera, de lado a lado.
    for x in [0, 300, 719] {
        assert_eq!(pixel(x, 39), ORO, "x {x}");
    }
    // El fondo del texto, lejos de la letra.
    assert_eq!(pixel(400, 200), FONDO);
    // La `M` en (28, 52): alguno de sus pixeles es TINTA.
    let tinta = (52..68).flat_map(|y| (28..36).map(move |x| (x, y))).filter(|&(x, y)| pixel(x, y) == TINTA).count();
    assert!(tinta > 10, "la M tiene {tinta} pixeles de tinta");
    // El cursor detras de la `M`: una barra de oro en x = 36.
    assert_eq!(pixel(36, 60), ORO);
}
