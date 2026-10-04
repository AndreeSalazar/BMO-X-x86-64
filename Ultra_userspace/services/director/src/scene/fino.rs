//! **LO FINO** -- como se pinta lo que flota encima del escritorio.
//!
//! [consumo] NADA      no corre por su cuenta: son piezas que pinta quien
//!                     las llama, cuando el compositor pinta (L6h)
//!
//! El propietario (04-10): *"mejorar mi escritorio en Ring 3, y elegancia, la
//! de Francia"*. La elegancia no se AGREGA: se QUITA. Lo que habia era neon,
//! marcos de dos pixeles, flechas `> ` de terminal y blanco de pantalla; lo
//! que queda es esto, y nada mas:
//!
//! ```text
//!    la tarjeta   noche honda, esquinas de 14 px de verdad (la curva
//!                 calculada, no una escalera), una sombra que cae suave y
//!                 un HILO de laton de un pixel, a media voz
//!    el rotulo    versalitas en laton, espaciadas un 18 % -- la letra de
//!                 los carteles de una galeria, no la de una consola
//!    el filete    una raya de laton que se apaga hacia los dos extremos,
//!                 como el filete de una pagina bien compuesta
//!    la marca     lo elegido: una pastilla un tono por encima y un punto
//!                 del acento. Sin flechas: se ve sin leerlo
//!    la letra     la de la casa (`bmo-letra`), proporcional y suave; el
//!                 marfil para lo que se lee y la perla para lo demas
//! ```
//!
//! Los colores son los de `tema.maqueta` (`.fino`, `.marca`, `.marfil`,
//! `.perla`, `.laton`), generados a `tema_gen.rs`: aqui no se inventa
//! ninguno. Las curvas las pinta `bmo-pinta`, el mismo pintor que hace la
//! foto de una maqueta en el anfitrion.
//!
//! [!] Una pieza suave MEZCLA con lo que hay debajo: pintarla dos veces
//! encima de si misma oscurece su borde. Quien la use tiene que devolver el
//! fondo antes de repintar (el conmutador guarda lo de debajo; la rejilla
//! repinta su celda desde el modelo del escritorio).

use bmo_userland as bmo;

use super::tema_gen::{FINO_BORDE, FINO_FONDO, MARCA_FONDO, MARFIL, PERLA};

/// El radio de una tarjeta.
pub(crate) const RADIO: i32 = 14;
/// Lo que la sombra de una tarjeta se sale por cada lado.
pub(crate) const HALO: u32 = 14;

/// Lo que se lee.
pub(crate) const CUERPO: bmo::Estilo = bmo::Estilo::normal(15);
/// Lo elegido: el mismo cuerpo, un punto mas firme.
pub(crate) const CUERPO_FIRME: bmo::Estilo = bmo::Estilo::media(15);
/// La letra chica de un pie.
pub(crate) const PIE: bmo::Estilo = bmo::Estilo::normal(12);
/// El rotulo: versalitas espaciadas.
pub(crate) const ROTULO: bmo::Estilo = bmo::Estilo::media(11).espaciado(180).mayusculas();

/// La tinta de lo que se lee, de lo secundario y de los rotulos.
pub(crate) const TINTA: u32 = MARFIL;
pub(crate) const TENUE: u32 = PERLA;

/// El hilo de laton, a media voz: el laton mezclado con la noche.
pub(crate) fn hilo() -> u32 {
    bmo::entre(FINO_FONDO, FINO_BORDE, 120)
}

/// **La tarjeta**: sombra, cuerpo y hilo, en `(x, y, w, h)`. La sombra se sale
/// [`HALO`] por cada lado; quien la ponga tiene que contar con ello.
pub(crate) fn tarjeta(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32) {
    let (x, y, w, h) = (x as i32, y as i32, w as i32, h as i32);
    // La sombra no es un marco: es negro que cae, mas fuerte abajo que en
    // los lados porque la luz viene de arriba.
    p.pieza(&bmo::Pieza::Resplandor { x, y: y + 4, w, h, r: RADIO, alcance: HALO as i32 - 4, argb: 0x9000_0000 }, 0, 0, None);
    p.pieza(&bmo::Pieza::Resplandor { x, y, w, h, r: RADIO, alcance: HALO as i32, argb: 0x3800_0000 }, 0, 0, None);
    // El cuerpo: un pelo mas claro arriba, como un papel con luz rasante.
    let arriba = bmo::entre(FINO_FONDO, MARCA_FONDO, 90);
    p.pieza(&bmo::Pieza::Degradado { x, y, w, h, r: RADIO, de: arriba, a: FINO_FONDO, vertical: true }, 0, 0, None);
    p.pieza(&bmo::Pieza::Borde { x, y, w, h, r: RADIO, grosor: 1, c: hilo() }, 0, 0, None);
}

/// **El rotulo** con la base en `y`, en laton (o en lo que `tinta` diga
/// mientras se enciende). Devuelve el ancho.
pub(crate) fn rotulo(p: &bmo::Pantalla, x: u32, y: u32, texto: &[u8], tinta: u32) -> u32 {
    p.letra(x as i32, y as i32, texto, tinta, ROTULO).max(0) as u32
}

/// **El filete**: una raya de un pixel que se enciende en el centro y se
/// apaga hacia los dos extremos.
pub(crate) fn filete(p: &bmo::Pantalla, x: u32, y: u32, w: u32) {
    let mitad = (w / 2) as i32;
    let (x, y) = (x as i32, y as i32);
    p.pieza(&bmo::Pieza::Degradado { x, y, w: mitad, h: 1, r: 0, de: FINO_FONDO, a: hilo(), vertical: false }, 0, 0, None);
    p.pieza(&bmo::Pieza::Degradado { x: x + mitad, y, w: w as i32 - mitad, h: 1, r: 0, de: hilo(), a: FINO_FONDO, vertical: false }, 0, 0, None);
}

/// **Un texto en su caja**, con la base puesta donde la pondria el
/// navegador en una caja de `alto` (`line-height`). Devuelve el ancho.
pub(crate) fn texto(p: &bmo::Pantalla, x: u32, y: u32, alto: u32, s: &[u8], c: u32, e: bmo::Estilo) -> u32 {
    p.letra_en_caja(x as i32, y as i32, alto as i32, s, c, e).max(0) as u32
}

/// La letra de los titulos de ventana: el nombre, y su segunda linea de voz.
pub(crate) const TITULO: bmo::Estilo = bmo::Estilo::media(14);
pub(crate) const SUBTITULO: bmo::Estilo = bmo::Estilo::normal(13);

/// **El titulo de una ventana** en su barra (`y` es lo de arriba de la
/// ventana): el nombre firme y, si lo hay, lo secundario, separados por
/// un hueco y no por guiones. Devuelve la x donde acaba, para quien siga.
///
/// Lo pinta cada ventana justo DESPUES de `paint_chrome`, que acaba de
/// repintar la barra: la letra suave cae siempre sobre barra limpia.
pub(crate) fn titulo(p: &bmo::Pantalla, x: u32, y: u32, nombre: &[u8], tinta: u32, sub: &[u8], tinta_sub: u32) -> u32 {
    let alto = super::TITLE_H - 1;
    let mut fx = x + texto(p, x, y, alto, nombre, tinta, TITULO);
    if !sub.is_empty() {
        fx += 12;
        fx += texto(p, fx, y, alto, sub, tinta_sub, SUBTITULO);
    }
    fx
}

/// **El punto de color** de una barra de titulo: redondo, de 8, centrado.
pub(crate) fn punto(p: &bmo::Pantalla, x: u32, y: u32, c: u32) {
    let cy = y as i32 + (super::TITLE_H as i32 - 1) / 2 - 4;
    p.caja_redonda(x as i32, cy, 8, 8, 4, c);
}
