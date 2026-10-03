//! Las pruebas del formato: que cada verbo vuelve igual, y que cada cosa que
//! no esta escrita se rechaza con su nombre.

use super::*;

extern crate alloc;
use alloc::vec::Vec;

fn ida_y_vuelta(m: Mensaje<'_>) -> Vec<u8> {
    let mut b = [0u8; 70_000];
    let n = escribir(&m, &mut b).unwrap();
    assert_eq!(leer(&b[..n]).unwrap(), m);
    b[..n].to_vec()
}

const SUMA: [u8; 32] = [0xAB; 32];
/// Corazon rojo, y un pulgar con tono: secuencias de verdad, en escapes para
/// que el fuente siga siendo ASCII.
const CORAZON: &str = "\u{2764}\u{FE0F}";
const PULGAR_TONO: &str = "\u{1F44D}\u{1F3FD}";
const TECLA_UNO: &str = "1\u{FE0F}\u{20E3}";

#[test]
fn cada_verbo_vuelve_igual() {
    assert_eq!(ida_y_vuelta(Mensaje::Texto("hola nova")), b"\x01hola nova");
    ida_y_vuelta(Mensaje::Texto("dos\nlineas y un emoji \u{1F3AE}"));
    assert_eq!(ida_y_vuelta(Mensaje::Zumbido), [ZUMBIDO]);
    assert_eq!(ida_y_vuelta(Mensaje::Guino(3)), [GUINO, 3]);
    let o = ida_y_vuelta(Mensaje::Oferta { id: 7, bytes: 4_404_019, suma: &SUMA, nombre: "atardecer en la sierra.jpg" });
    assert_eq!(o.len(), 1 + 4 + 8 + 32 + 26);
    assert_eq!(ida_y_vuelta(Mensaje::Si(7)), [SI, 0, 0, 0, 7]);
    assert_eq!(ida_y_vuelta(Mensaje::No(7)), [NO, 0, 0, 0, 7]);
    let datos = alloc::vec![0x5Au8; TROZO_MAX];
    assert_eq!(ida_y_vuelta(Mensaje::Trozo { id: 7, numero: 41, datos: &datos }).len(), TROZO_CABECERA + TROZO_MAX);
    ida_y_vuelta(Mensaje::Pide { que: Que::Pagina, nombre: "inicio" });
    ida_y_vuelta(Mensaje::Pide { que: Que::Muro, nombre: "todo_2026-10" });
    for e in [CORAZON, PULGAR_TONO, TECLA_UNO] {
        ida_y_vuelta(Mensaje::Reaccion { tuyo: true, numero: 12, emoji: e });
    }
}

#[test]
fn el_trozo_mas_grande_cabe_en_un_mensaje_de_noise() {
    use crate::noise::{ETIQUETA, MENSAJE_MAX};
    assert!(TROZO_CABECERA + TROZO_MAX + ETIQUETA <= MENSAJE_MAX);
    assert!(1 + TEXTO_MAX + ETIQUETA <= MENSAJE_MAX);
}

#[test]
fn lo_que_no_esta_escrito_se_rechaza_con_nombre() {
    assert_eq!(leer(&[]), Err(Rechazo::Vacio));
    assert_eq!(leer(&[0x00]), Err(Rechazo::Verbo));
    assert_eq!(leer(&[0x0A]), Err(Rechazo::Verbo));
    assert_eq!(leer(&[0xFF, 1, 2]), Err(Rechazo::Verbo));
    // TEXTO
    assert_eq!(leer(&[TEXTO]), Err(Rechazo::Medida), "un texto vacio no es un mensaje");
    let mut largo = alloc::vec![b'a'; TEXTO_MAX + 2];
    largo[0] = TEXTO;
    assert_eq!(leer(&largo), Err(Rechazo::Medida));
    assert_eq!(leer(&[TEXTO, 0xC3]), Err(Rechazo::Utf8), "UTF-8 cortado a medias");
    assert_eq!(leer(&[TEXTO, 0xFF, 0xFE]), Err(Rechazo::Utf8));
    assert_eq!(leer(b"\x01hola\x1b[2J"), Err(Rechazo::Control), "un escape de terminal no se muestra");
    assert_eq!(leer(b"\x01a\x00b"), Err(Rechazo::Control));
    // ZUMBIDO y GUINO
    assert_eq!(leer(&[ZUMBIDO, 0]), Err(Rechazo::Medida));
    assert_eq!(leer(&[GUINO]), Err(Rechazo::Medida));
    assert_eq!(leer(&[GUINO, 0]), Err(Rechazo::Guino));
    assert_eq!(leer(&[GUINO, GUINOS + 1]), Err(Rechazo::Guino));
    assert_eq!(leer(&[GUINO, 1, 1]), Err(Rechazo::Medida));
    // SI, NO, TROZO, PIDE
    assert_eq!(leer(&[SI, 0, 0, 7]), Err(Rechazo::Medida));
    assert_eq!(leer(&[NO, 0, 0, 0, 0, 7]), Err(Rechazo::Medida));
    assert_eq!(leer(&[TROZO, 0, 0, 0, 1, 0, 0, 0, 1]), Err(Rechazo::Medida), "un trozo sin datos");
    assert_eq!(leer(&[PIDE, 4, b'a']), Err(Rechazo::Que));
    assert_eq!(leer(&[PIDE, 3]), Err(Rechazo::Medida));
    assert_eq!(leer(b"\x08\x03Inicio"), Err(Rechazo::Nombre), "mayusculas no");
    assert_eq!(leer(b"\x08\x03../clave"), Err(Rechazo::Nombre), "ni rutas");
    assert_eq!(leer(b"\x08\x03a b"), Err(Rechazo::Nombre));
}

fn oferta(bytes: u64, nombre: &[u8]) -> Vec<u8> {
    let mut v = alloc::vec![OFERTA, 0, 0, 0, 1];
    v.extend_from_slice(&bytes.to_be_bytes());
    v.extend_from_slice(&SUMA);
    v.extend_from_slice(nombre);
    v
}

#[test]
fn la_oferta_no_deja_burlar() {
    assert!(leer(&oferta(1, b"a")).is_ok());
    assert_eq!(leer(&oferta(0, b"a")), Err(Rechazo::Bytes), "cero bytes no es un envio");
    assert_eq!(leer(&oferta(ENVIO_MAX + 1, b"a")), Err(Rechazo::Bytes));
    assert_eq!(leer(&oferta(10, b"")), Err(Rechazo::Medida), "sin nombre");
    assert_eq!(leer(&oferta(10, &[b'n'; NOMBRE_MAX + 1])), Err(Rechazo::Medida));
    // `foto` + U+202E + `gpj.exe` se ve como `fotoexe.jpg`.
    let mut giro = b"foto".to_vec();
    giro.extend_from_slice("\u{202E}".as_bytes());
    giro.extend_from_slice(b"gpj.exe");
    assert_eq!(leer(&oferta(10, &giro)), Err(Rechazo::Nombre));
    assert_eq!(leer(&oferta(10, b"a\nb.jpg")), Err(Rechazo::Nombre));
    // Y el tipo NO esta: un `.bex` que se llama `.jpg` pasa aqui. Lo para el JUEZ.
    assert!(leer(&oferta(10, b"vacaciones.jpg")).is_ok());
}

#[test]
fn una_reaccion_es_un_emoji_y_no_texto() {
    let r = |e: &[u8]| {
        let mut v = alloc::vec![REACCION, 0];
        v.extend_from_slice(&5u64.to_be_bytes());
        v.extend_from_slice(e);
        leer(&v).map(|_| ())
    };
    assert_eq!(r(CORAZON.as_bytes()), Ok(()));
    assert_eq!(r(b"hola"), Err(Rechazo::NoEsEmoji));
    assert_eq!(r(b"1"), Err(Rechazo::NoEsEmoji), "un digito solo no es el teclado");
    assert_eq!(r(" \u{2764}".as_bytes()), Err(Rechazo::NoEsEmoji));
    assert_eq!(r("a\u{2764}".as_bytes()), Err(Rechazo::NoEsEmoji));
    assert_eq!(r(&[]), Err(Rechazo::Medida));
    assert_eq!(r(&[0xF0, 0x9F]), Err(Rechazo::Utf8));
    let mut otra = alloc::vec![REACCION, 2];
    otra.extend_from_slice(&5u64.to_be_bytes());
    otra.extend_from_slice(CORAZON.as_bytes());
    assert_eq!(leer(&otra), Err(Rechazo::Bandera));
}

#[test]
fn escribir_no_manda_lo_que_rechazaria() {
    let mut b = [0u8; 600];
    assert_eq!(escribir(&Mensaje::Texto(""), &mut b), Err(Rechazo::Medida));
    assert_eq!(escribir(&Mensaje::Guino(9), &mut b), Err(Rechazo::Guino));
    assert_eq!(escribir(&Mensaje::Pide { que: Que::Pagina, nombre: "../x" }, &mut b), Err(Rechazo::Nombre));
    assert_eq!(escribir(&Mensaje::Reaccion { tuyo: false, numero: 1, emoji: "ok" }, &mut b), Err(Rechazo::NoEsEmoji));
    assert_eq!(escribir(&Mensaje::Texto("hola"), &mut [0u8; 3]), Err(Rechazo::SinSitio));
}
