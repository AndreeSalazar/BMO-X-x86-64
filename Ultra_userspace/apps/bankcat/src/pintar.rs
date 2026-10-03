//! **LA VENTANA DE BANK CAT** -- la maqueta (`docs/arte/maqueta_bankcat.html`),
//! MEDIDA: cada caja de aqui es la que el navegador da para la maqueta
//! (03-10, `toolchain/tools/espejo-cara`), y la letra es la de las maquetas
//! (`bmo-letra`), no la de 8 x 16. El ESPEJO de cara dice cuanto se parece.
//!
//! ```text
//!    barra    BANK CAT, su programa, de donde sale el dinero y la X   (36)
//!    riel     cartera, mover, mercado, el libro, cajero y reglas      (64)
//!    lista    el monedero de CAB y lo que el motor contesto          (250)
//!    centro   el GATO HUCHA, el saldo grande y los tres botones (cartera);
//!             el libro de esta sesion; o la tarjeta de lo que aun no hay
//!    panel    el gato hucha, quien lleva la cuenta y la red          (290)
//! ```
//!
//! Lo que la maqueta trae DE EJEMPLO (amigos, fichas, asientos de otro dia)
//! aqui no se finge: la lista dice lo que el motor contesto de verdad.
//!
//! *** Ni un centimo se calcula aqui: el saldo es lo que el motor COBOL
//! contesto, y la cara solo lo muestra (y lo escribe en castellano con
//! `bmo_bankcat::formato`).

use crate::canvas::Canvas;
use crate::gato::{self, Humor};
use crate::piezas::{caja, icono_svg, medir as mide, parrafo, partir, redonda, sombra, texto, texto_cabe, texto_der, Estilo};
use crate::tinta::*;
use alloc::vec::Vec;
use bmo_bankcat::{formato, Centimos, Estado};
use bmo_dibujo::{mezclar, Color, Lienzo};

/// La ventana de la maqueta: 1240 x 677 (su `.ventana`, con su borde).
pub const ANCHO: u32 = 1240;
pub const ALTO: u32 = 677;
pub const MINIMO: (u32, u32) = (900, 600);

static MEDIDA: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(ANCHO << 16 | ALTO);

/// La ventana cambio de medida.
pub fn medir(w: u32, h: u32) {
    MEDIDA.store(w.max(MINIMO.0) << 16 | h.max(MINIMO.1), core::sync::atomic::Ordering::Relaxed);
}

fn ancho() -> i32 {
    (MEDIDA.load(core::sync::atomic::Ordering::Relaxed) >> 16) as i32
}

fn alto() -> i32 {
    (MEDIDA.load(core::sync::atomic::Ordering::Relaxed) & 0xFFFF) as i32
}

// -- Las columnas y la barra, del `grid-template-columns: 64px 250px 1fr 290px`
// de la maqueta, con el borde de un pixel de la ventana.
const BARRA: i32 = 36;
const X_RIEL: i32 = 1;
const X_LISTA: i32 = 65;
const X_CENTRO: i32 = 315;
const PANEL: i32 = 290;

fn x_panel() -> i32 {
    if ancho() >= 1060 { ancho() - 1 - PANEL } else { ancho() - 1 }
}

fn ancho_centro() -> i32 {
    x_panel() - X_CENTRO
}

/// La tarjeta de la maqueta (`#0D1118`) y la fila elegida (`#10202A`).
const TARJETA: Color = 0x000D_1118;
const FILA_SEL: Color = 0x0010_202A;
const BURBUJA: Color = 0x0010_131B;

// -- Las letras de la maqueta: Plex 14 el cuerpo, 15 el titulo de la lista,
// 11 los rotulos espaciados, 12 los numeros chicos, 44 la cifra.
const CUERPO: Estilo = Estilo::normal(14);
const NOMBRE: Estilo = Estilo::media(14);
const FUERTE: Estilo = Estilo::negrita(14);
const CHICA: Estilo = Estilo::normal(12);
const ROTULO: Estilo = Estilo::normal(11).espaciado(140).mayusculas();
const ROTULO_MEDIA: Estilo = Estilo::media(11).espaciado(140).mayusculas();
const PIXEL: Estilo = Estilo::normal(11).espaciado(60).mayusculas();
const CIFRA: Estilo = Estilo::normal(44);
/// La altura de linea del cuerpo (`line-height: 1.45` de 14 px).
const LINEA_CUERPO: i32 = 20;

pub const CARTERA: usize = 0;
pub const MOVER: usize = 1;
pub const MERCADO: usize = 2;
pub const LIBRO: usize = 3;
pub const CAJERO: usize = 4;
pub const REGLAS: usize = 5;

/// Las secciones: nombre, color y de que van (los `SECS` de la maqueta).
pub const SECCIONES: [(&[u8], Color, &[u8]); 6] = [
    (b"Cartera", ORO, b"lo tuyo, al centimo"),
    (b"Mover", CIAN, b"enviar y pedir, por HERMES"),
    (b"Mercado", ROSA, b"cambiar con amigos"),
    (b"El libro", AZUL, b"lo lleva COBOL"),
    (b"Cajero", AMBAR, b"comprar CAB con dinero"),
    (b"Reglas", GRIS, b"lo que el gato no negocia"),
];

/// Los iconos del riel: los `<svg>` de la maqueta (viewBox 24, trazo 2).
const ICONOS: [&str; 6] = [
    "M6 6h12q3 0 3 3v7q0 3-3 3H6q-3 0-3-3V9q0-3 3-3zM3 10h18M16 14h2",
    "M4 8h14l-4-4M20 16H6l4 4",
    "M4 9l2-5h12l2 5M4 9v11h16V9M4 9h16M9 20v-6h6v6",
    "M5 4h11l3 3v13H5zM8 9h8M8 13h8M8 17h5",
    "M7 4h10q3 0 3 3v10q0 3-3 3H7q-3 0-3-3V7q0-3 3-3zM8 9h8M12 13v5M10 16l2 2 2-2",
    "M12 3l8 3v6c0 5-4 8-8 9-4-1-8-4-8-9V6zM9 12l2 2 4-4",
];

/// Un movimiento de esta sesion, como lo contesto el motor.
#[derive(Clone, Copy)]
pub struct Asiento {
    /// Cuanto cambio el saldo (0 si no se hizo).
    pub cambio: Centimos,
    pub estado: Estado,
    pub saldo: Centimos,
}

/// Lo que hace falta para pintar un fotograma.
pub struct Vista<'a> {
    pub sec: usize,
    pub ms: u32,
    pub desde_sec: u32,
    pub puntero: Option<(i32, i32)>,
    /// El saldo que dijo el motor (`None`: aun no contesto).
    pub saldo: Option<Centimos>,
    pub libro: &'a [Asiento],
    pub humor: Humor,
    pub humor_desde: u32,
    pub frase: &'a [u8],
    /// Ordenes que el motor aun no contesto.
    pub esperando: u32,
    pub aviso: &'a [u8],
}

/// Donde cayo un clic.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Golpe {
    Seccion(usize),
    /// Uno de los tres botones de la cartera.
    Boton(usize),
    Cerrar,
}

/// Los tres botones: que dicen y su color (los de la maqueta).
pub const BOTONES: [(&[u8], Color); 3] = [(b"Recibir 50,00", VERDE), (b"Enviar 19,99", ROSA), (b"3 x 19,99", CIAN)];

fn dentro(x: i32, y: i32, (bx, by, bw, bh): (i32, i32, i32, i32)) -> bool {
    x >= bx && y >= by && x < bx + bw && y < by + bh
}

fn caja_cerrar() -> (i32, i32, i32, i32) {
    (ancho() - 40, 4, 32, BARRA - 8)
}

fn caja_burbuja(i: usize) -> (i32, i32, i32, i32) {
    (11, 78 + i as i32 * 54, 44, 44)
}

/// La columna del saldo: a la derecha del gato (260 + 24 de hueco).
fn x_saldo() -> i32 {
    X_CENTRO + 20 + gato::ANCHO + 24
}

fn ancho_saldo() -> i32 {
    X_CENTRO + ancho_centro() - 20 - x_saldo()
}

/// Donde empieza el bloque del saldo: centrado con el gato, como el
/// `align-items: center` de la maqueta.
fn y_saldo() -> i32 {
    130
}

/// Lo que dice el saldo debajo de la cifra (el `<p>` de la maqueta).
const EXPLICA: &[u8] = b"Sin decimales de mentira: el libro guarda centimos enteros (PIC S9(13)V99 COMP-3). 19,99 x 3 es 59,97, no 59,969999.";

/// Los botones de la maqueta: `padding: 9px 16px`, en fila, y el que no
/// cabe baja (el `flex-wrap`).
fn caja_boton(k: usize) -> (i32, i32, i32, i32) {
    // Debajo del parrafo (`margin: 6px 0 16px`), que baja si es estrecho.
    let lineas = partir(EXPLICA, CUERPO, ancho_saldo()).len().min(4) as i32;
    let (mut x, mut y) = (x_saldo(), y_saldo() + 71 + lineas * LINEA_CUERPO + 16);
    let fin = x_saldo() + ancho_saldo();
    for (i, &(t, _)) in BOTONES.iter().enumerate() {
        let w = mide(t, FUERTE) + 34;
        if x > x_saldo() && x + w > fin {
            x = x_saldo();
            y += 50;
        }
        if i == k {
            return (x, y, w, 40);
        }
        x += w + 10;
    }
    (0, 0, 0, 0)
}

pub fn golpe(x: i32, y: i32, sec: usize) -> Option<Golpe> {
    if dentro(x, y, caja_cerrar()) {
        return Some(Golpe::Cerrar);
    }
    for i in 0..SECCIONES.len() {
        if dentro(x, y, caja_burbuja(i)) {
            return Some(Golpe::Seccion(i));
        }
    }
    if sec == CARTERA {
        for k in 0..BOTONES.len() {
            if dentro(x, y, caja_boton(k)) {
                return Some(Golpe::Boton(k));
            }
        }
    }
    None
}

fn llega(ms: u32, desde: u32, dura: u32) -> i32 {
    let u = ((ms.wrapping_sub(desde) * 256) / dura.max(1)).min(256) as i32;
    256 - (256 - u) * (256 - u) / 256
}

/// El saldo en castellano, o una raya si el motor aun no contesto.
fn cifra(c: Option<Centimos>, d: &mut [u8; 32]) -> usize {
    match c.map(|c| formato(c, d)) {
        Some(Ok(n)) => n,
        _ => {
            d[..2].copy_from_slice(b"--");
            2
        }
    }
}

/// **Un fotograma entero.**
pub fn pintar(cv: &mut Canvas, v: &Vista) {
    cv.clear(FONDO);
    barra(cv, v);
    riel(cv, v);
    lista(cv, v);
    centro(cv, v);
    if x_panel() < ancho() - 1 {
        panel(cv, v);
    }
    marco(cv);
}

/// El borde de la ventana (1 px, radio 14, suave) y, fuera de sus
/// esquinas, el negro del escritorio: el `border-radius: 14px` de la
/// `.ventana`.
fn marco(cv: &mut Canvas) {
    let (w, h) = (ancho(), alto());
    cv.frame(0, 0, w, h, 1, LINEA);
    let r = 14;
    for (cx, cy, sx, sy) in [(r, r, -1, -1), (w - 1 - r, r, 1, -1), (r, h - 1 - r, -1, 1), (w - 1 - r, h - 1 - r, 1, 1)] {
        for j in 0..=r {
            for i in 0..=r {
                let (x, y) = (cx + sx * i, cy + sy * j);
                // La distancia al centro de la esquina, en 1/16 de pixel.
                let d = crate::piezas::raiz(((i * 16) * (i * 16) + (j * 16) * (j * 16)) as u64) as i32;
                let aro = (16 - (d - r * 16).abs()).clamp(0, 16);
                let fuera = (d - r * 16).clamp(0, 16);
                if aro > 0 {
                    cv.blend(x, y, LINEA, aro as u32, 16);
                }
                if fuera > 0 {
                    cv.blend(x, y, NEGRO, fuera as u32, 16);
                }
            }
        }
    }
}

fn barra(cv: &mut Canvas, v: &Vista) {
    cv.rect(1, 1, ancho() - 2, BARRA - 1, NEGRO);
    cv.rect(1, BARRA - 1, ancho() - 2, 1, LINEA);
    let titulo = Estilo::negrita(13).espaciado(80).mayusculas();
    let fin = texto(cv, 15, 8, 20, b"BANK CAT", BLANCO, titulo);
    texto(cv, 15 + fin + 12, 9, 19, "sys/bankcat.bex \u{b7} el libro: cobol/11/libro.bex".as_bytes(), TENUE, Estilo::normal(13));
    let (cx, cy, cw, ch) = caja_cerrar();
    let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (cx, cy, cw, ch)));
    if encima {
        redonda(cv, cx, cy, cw, ch, 6, mezclar(ROJO, NEGRO, 120, 256));
    }
    let (mx, my) = ((cx + cw / 2) * 64, (cy + ch / 2) * 64);
    let c = if encima { BLANCO } else { GRIS };
    for (a, b) in [((-4, -4), (4, 4)), ((-4, 4), (4, -4))] {
        let p = [(mx + a.0 * 64, my + a.1 * 64), (mx + b.0 * 64, my + b.1 * 64)];
        bmo_letra::pluma(&p, 80, false, |x, y, al| cv.blend(x, y, c, al as u32, 255));
    }
}

fn riel(cv: &mut Canvas, v: &Vista) {
    cv.rect(X_RIEL, BARRA, 64, alto() - BARRA - 1, NEGRO);
    cv.rect(X_LISTA - 1, BARRA, 1, alto() - BARRA - 1, LINEA);
    let w = mide(b"CAB", PIXEL);
    texto(cv, 33 - w / 2, 48, 16, b"CAB", ORO, PIXEL);
    for (i, &(_, color, _)) in SECCIONES.iter().enumerate() {
        let (x, y, w, h) = caja_burbuja(i);
        let elegida = i == v.sec;
        let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (x, y, w, h)));
        if elegida {
            sombra(cv, x, y, w, h, 13, color, 14, 90);
            caja(cv, x, y, w, h, 13, mezclar(color, NEGRO, 51, 256), color);
            redonda(cv, 1, y + 10, 4, h - 20, 2, BLANCO);
        } else {
            cv.disc(x + w / 2, y + h / 2, w / 2, mezclar(color, BURBUJA, 102, 256));
            cv.disc(x + w / 2, y + h / 2, w / 2 - 1, BURBUJA);
        }
        let tinta = if elegida || encima { color } else { mezclar(color, 0, 192, 256) };
        icono_svg(cv, x + 11, y + 11, 22, ICONOS[i], tinta);
    }
}

/// Una cara redonda con su letra (`.cara` de la maqueta: 34 px, negrita).
fn cara(cv: &mut Canvas, x: i32, y: i32, c: Color, letra: &[u8]) {
    cv.disc(x + 17, y + 17, 17, c);
    let w = mide(letra, FUERTE);
    texto(cv, x + 17 - w / 2, y + 7, 20, letra, NEGRO, FUERTE);
}

fn lista(cv: &mut Canvas, v: &Vista) {
    let (nombre, _, _) = SECCIONES[v.sec];
    let h = alto() - BARRA - 1;
    cv.rect(X_LISTA, BARRA, 250, h, crate::tinta::PANEL);
    cv.rect(X_CENTRO - 1, BARRA, 1, h, LINEA);
    texto(cv, X_LISTA + 16, 54, 22, nombre, BLANCO, Estilo::negrita(15));
    cv.rect(X_LISTA, 90, 249, 1, LINEA);
    texto(cv, X_LISTA + 16, 105, 16, b"tus monederos", TENUE, ROTULO);
    // El monedero de CAB: la fila elegida.
    let (x, y) = (X_LISTA + 8, 129);
    redonda(cv, x, y, 233, 77, 10, FILA_SEL);
    cara(cv, x + 8, y + 21, ORO, b"$");
    texto(cv, x + 52, y + 8, 20, b"CAB", BLANCO, NOMBRE);
    let mut d = [0u8; 32];
    let n = cifra(v.saldo, &mut d);
    let izq = texto_der(cv, x + 225, y + 30, 17, &d[..n], TEXTO, CHICA);
    let mut yy = y + 32;
    for l in partir(b"la moneda de la casa", CHICA, izq - 10 - (x + 52)).iter().take(2) {
        texto(cv, x + 52, yy, 17, l, TENUE, CHICA);
        yy += 17;
    }
    // Lo que el motor contesto, de mas nuevo a mas viejo: lo de verdad, en
    // el sitio donde la maqueta pone amigos de ejemplo.
    texto(cv, X_LISTA + 16, 282, 16, b"lo ultimo", TENUE, ROTULO);
    let mut yy = 306;
    let caben = ((alto() - 63 - yy) / 30).max(0) as usize;
    for a in v.libro.iter().rev().take(caben) {
        let (signo, c): (&[u8], Color) = if a.estado != Estado::Hecho {
            (b"NO", ROJO)
        } else if a.cambio > 0 {
            (b"+", VERDE)
        } else if a.cambio < 0 {
            (b"-", ROSA)
        } else {
            (b"=", TENUE)
        };
        texto(cv, X_LISTA + 16, yy, LINEA_CUERPO, signo, c, NOMBRE);
        let x2 = X_LISTA + 16 + 28;
        if a.estado == Estado::Hecho {
            let n = formato(a.cambio.abs(), &mut d).unwrap_or(0);
            texto(cv, x2, yy, LINEA_CUERPO, &d[..n], TEXTO, CUERPO);
            let n = formato(a.saldo, &mut d).unwrap_or(0);
            texto_der(cv, X_LISTA + 234, yy + 2, 17, &d[..n], TENUE, CHICA);
        } else {
            texto_cabe(cv, x2, yy, LINEA_CUERPO, a.estado.texto().as_bytes(), TENUE, CHICA, X_LISTA + 234 - x2);
        }
        yy += 30;
    }
    if v.libro.is_empty() {
        parrafo(cv, X_LISTA + 16, yy, 218, 17, b"aun nada: el libro se abre con el primer movimiento", TENUE, CHICA, 3);
    }
    // Abajo, quien eres.
    let y0 = alto() - 63;
    cv.rect(X_LISTA, y0, 249, 1, LINEA);
    cara(cv, X_LISTA + 16, y0 + 15, ORO, b"B");
    texto(cv, X_LISTA + 60, y0 + 15, 16, b"BMO-X (tu)", BLANCO, FUERTE);
    texto(cv, X_LISTA + 60, y0 + 33, 17, b"cuenta 0001", TENUE, CHICA);
}

/// **Una pildora** de estado: texto de 12 en negrita, en su color, sobre su
/// color al 12 %, con borde. Acaba en `der`.
fn pildora(cv: &mut Canvas, der: i32, y: i32, s: &[u8], c: Color, fondo: Color) {
    let e = Estilo::negrita(12);
    let w = mide(s, e) + 22;
    caja(cv, der - w, y, w, 23, 11, mezclar(c, fondo, 31, 256), c);
    texto(cv, der - w + 11, y + 3, 17, s, c, e);
}

fn centro(cv: &mut Canvas, v: &Vista) {
    let (nombre, _, sub) = SECCIONES[v.sec];
    let (x0, w) = (X_CENTRO, ancho_centro());
    let fin = texto(cv, x0 + 20, 50, LINEA_CUERPO, nombre, BLANCO, FUERTE);
    texto(cv, x0 + 20 + fin + 10, 50, LINEA_CUERPO, sub, TENUE, CUERPO);
    let (pt, pc): (&[u8], Color) = match v.sec {
        CARTERA if v.esperando > 0 => (b"el motor piensa", AMBAR),
        CARTERA if v.saldo.is_some() => (b"libro cuadrado", VERDE),
        CARTERA | LIBRO if v.saldo.is_none() => (b"sin motor", ROJO),
        LIBRO => (b"partida doble", AZUL),
        MOVER => (b"sin red: H6", AZUL),
        MERCADO => (b"llega con H6", ROSA),
        CAJERO => (b"decision D1", AMBAR),
        _ => (b"4 reglas", GRIS),
    };
    pildora(cv, x0 + w - 20, 48, pt, pc, FONDO);
    cv.rect(x0, 83, w, 1, LINEA);
    // La raya de oro de la cabecera: el 40 %, del oro a nada.
    let t = llega(v.ms, v.desde_sec, 260);
    let largo = w * 2 / 5 * t / 256;
    for i in 0..largo {
        let c = mezclar(ORO, FONDO, (256 - i * 256 / w.max(1) * 5 / 2).max(0) as u32, 256);
        cv.rect(x0 + i, 83, 1, 2, c);
    }
    match v.sec {
        CARTERA => cartera(cv, v),
        LIBRO => libro(cv, v),
        MOVER => tarjetas_texto(
            cv,
            &[
                (b"enviar a un amigo", b"Se firmara con TU clave y el amigo lo apuntara en SU libro. Los dos libros tienen que decir lo mismo: si no, el gato no lo da por hecho. Pide la red de HERMES (H6): hoy no hay amigos con quien mover CAB, y no se finge ninguno."),
                (b"pedir", b"Un cobro sera un mensaje de HERMES con un importe. El otro dice si o no; nada sale solo."),
            ],
        ),
        MERCADO => tarjetas_texto(
            cv,
            &[(b"cambiar con amigos", b"Un cambio es de DOS lados a la vez (entrega contra pago): o pasan la cosa y los CAB, o no pasa nada. Es el syncpoint de CICS, y ESTRATOS ya lo tiene en el fondo. Llega con H6, como MOVER.")],
        ),
        CAJERO => cajero(cv),
        REGLAS => tarjetas_texto(
            cv,
            &[
                (b"1. partida doble", b"Cada movimiento va al DEBE o al HABER, y inicial + haber - debe es el saldo. Si no cuadra, no se apunta."),
                (b"2. nada se borra", b"Un error se corrige con otro asiento, como en un banco. El libro solo crece."),
                (b"3. sin servidor de nadie", b"Los CAB entre amigos iran por HERMES y quedaran en los DOS libros, firmados."),
                (b"4. centimos enteros", b"Nada de coma flotante: COBOL y su COMP-3, y la ley del JUEZ (toda cuenta con ON SIZE ERROR)."),
            ],
        ),
        _ => {}
    }
    if !v.aviso.is_empty() {
        texto_cabe(cv, x0 + 20, alto() - 30, LINEA_CUERPO, v.aviso, AMBAR, CUERPO, w - 40);
    }
}

/// **Una tarjeta** de la maqueta (`.tarjeta`: borde, radio 12, relleno 14) con
/// su rotulo; devuelve donde empieza su contenido.
fn tarjeta(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, titulo: &[u8]) -> i32 {
    caja(cv, x, y, w, h, 12, TARJETA, LINEA);
    texto(cv, x + 15, y + 15, 16, titulo, TENUE, ROTULO_MEDIA);
    y + 39
}

/// Un par de la maqueta (`.par`): la clave en tenue, el valor a la derecha.
fn par(cv: &mut Canvas, x: i32, y: i32, w: i32, k: &[u8], val: &[u8], c: Color) {
    texto(cv, x, y + 3, LINEA_CUERPO, k, TENUE, CUERPO);
    texto_der(cv, x + w, y + 3, LINEA_CUERPO, val, c, CUERPO);
}

/// El alto de una tarjeta con un parrafo de `ancho`.
fn alto_parrafo(s: &[u8], ancho: i32) -> i32 {
    39 + partir(s, CUERPO, ancho).len() as i32 * LINEA_CUERPO + 15
}

/// Las columnas de las tarjetas (`repeat(auto-fit, minmax(210px, 1fr))`,
/// hueco 12) en `w`.
fn columnas(w: i32, n: usize) -> (i32, i32) {
    let k = (((w + 12) / 222).max(1) as usize).min(n.max(1)) as i32;
    (k, (w - 12 * (k - 1)) / k)
}

/// **La cartera**: el gato, el saldo, los botones y sus tres tarjetas.
fn cartera(cv: &mut Canvas, v: &Vista) {
    let (gx, gy) = (X_CENTRO + 20, 105);
    let t = v.ms.wrapping_sub(v.humor_desde);
    let humor = if t < gato::DURA_MS { v.humor } else { Humor::Quieto };
    gato::pintar(cv, gx, gy, v.ms, humor, t);
    let x = x_saldo();
    let y = y_saldo();
    texto(cv, x, y + 1, 16, b"tu saldo", TENUE, PIXEL);
    let mut d = [0u8; 32];
    let n = cifra(v.saldo, &mut d);
    // La cifra grande. (Su `text-shadow` de la maqueta es oro al 20 % con
    // 22 px de difuminado: no se ve sobre este fondo, y no se pinta.)
    let fin = texto(cv, x, y + 17, 48, &d[..n], ORO, CIFRA);
    texto(cv, x + fin + 12, y + 39, 20, b"CAB", AMBAR, Estilo::normal(18));
    parrafo(cv, x, y + 71, ancho_saldo(), LINEA_CUERPO, EXPLICA, TENUE, CUERPO, 4);
    for (k, &(t, c)) in BOTONES.iter().enumerate() {
        let (bx, by, bw, bh) = caja_boton(k);
        let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (bx, by, bw, bh)));
        caja(cv, bx, by, bw, bh, 10, mezclar(c, crate::tinta::PANEL, if encima { 67 } else { 36 }, 256), c);
        let tw = mide(t, FUERTE);
        texto(cv, bx + (bw - tw) / 2, by + 10, LINEA_CUERPO, t, BLANCO, FUERTE);
    }
    // Las tarjetas de abajo (22 por debajo del gato).
    let ty = gy + gato::ALTO + 22;
    let x0 = X_CENTRO + 20;
    let (k, tw) = columnas(ancho_centro() - 40, 3);
    let caja_de = |i: i32| (x0 + (i % k) * (tw + 12), ty + (i / k) * (107 + 12));
    let entro: Centimos = v.libro.iter().filter(|a| a.cambio > 0).map(|a| a.cambio).sum();
    let salio: Centimos = v.libro.iter().filter(|a| a.cambio < 0).map(|a| -a.cambio).sum();
    for i in 0..3 {
        let (cx, cy) = caja_de(i);
        if cy + 60 > alto() {
            break;
        }
        match i {
            0 => {
                let y = tarjeta(cv, cx, cy, tw, 107, b"el gato dice");
                parrafo(cv, cx + 15, y, tw - 30, LINEA_CUERPO, v.frase, TEXTO, CUERPO, 3);
            }
            1 => {
                let y = tarjeta(cv, cx, cy, tw, 107, b"esta sesion");
                let mut s = [0u8; 34];
                for (j, (que, c, val, signo)) in [(&b"entro"[..], VERDE, entro, b'+'), (b"salio", ROJO, salio, b'-')].into_iter().enumerate() {
                    s[0] = signo;
                    s[1] = b' ';
                    let n = formato(val, (&mut s[2..]).try_into().unwrap_or(&mut [0u8; 32])).unwrap_or(0);
                    par(cv, cx + 15, y + j as i32 * 26, tw - 30, que, &s[..n + 2], c);
                }
            }
            _ => {
                let y = tarjeta(cv, cx, cy, tw, 107, b"el libro");
                let hechos = v.libro.iter().filter(|a| a.estado == Estado::Hecho && a.cambio != 0).count();
                let mut e = [0u8; 8];
                let ne = crate::fmt_num(hechos as u64, &mut e);
                par(cv, cx + 15, y, tw - 30, b"asientos", &e[..ne], TEXTO);
                let cuadra: &[u8] = if v.saldo.is_some() { b"debe = haber" } else { b"--" };
                par(cv, cx + 15, y + 26, tw - 30, b"cuadra", cuadra, VERDE);
            }
        }
    }
}

/// **El libro** de esta sesion: la tabla de la maqueta (n, que paso,
/// importe, saldo), con lo que el motor contesto de verdad.
fn libro(cv: &mut Canvas, v: &Vista) {
    let x0 = X_CENTRO + 20;
    let w = ancho_centro() - 40;
    let mut y = 105;
    let th = Estilo::media(12);
    texto(cv, x0 + 6, y + 8, 17, b"n", TENUE, th);
    texto(cv, x0 + 70, y + 8, 17, b"que paso", TENUE, th);
    texto_der(cv, x0 + w - 150, y + 8, 17, b"importe", TENUE, th);
    texto_der(cv, x0 + w - 6, y + 8, 17, b"saldo", TENUE, th);
    y += 34;
    cv.rect(x0, y - 1, w, 1, LINEA);
    let caben = ((alto() - y - 60) / 37).max(0) as usize;
    let desde = v.libro.len().saturating_sub(caben);
    let mut d = [0u8; 32];
    for (i, a) in v.libro.iter().enumerate().skip(desde) {
        let mut n = [b'0'; 8];
        let mut m = [0u8; 8];
        let nn = crate::fmt_num(i as u64 + 1, &mut m);
        n[4 - nn.min(4)..4].copy_from_slice(&m[..nn.min(4)]);
        texto(cv, x0 + 6, y + 8, LINEA_CUERPO, &n[..4], TENUE, CUERPO);
        let que: &[u8] = match (a.estado, a.cambio) {
            (Estado::Hecho, c) if c > 0 => b"entra al HABER",
            (Estado::Hecho, c) if c < 0 => b"sale del DEBE",
            (Estado::Hecho, _) => b"cuadrado",
            (e, _) => e.texto().as_bytes(),
        };
        texto_cabe(cv, x0 + 70, y + 8, LINEA_CUERPO, que, if a.estado == Estado::Hecho { TEXTO } else { ROJO }, CUERPO, w - 330);
        if a.cambio != 0 {
            let mut s = [0u8; 34];
            s[0] = if a.cambio > 0 { b'+' } else { b'-' };
            s[1] = b' ';
            let nd = formato(a.cambio.abs(), (&mut s[2..]).try_into().unwrap_or(&mut [0u8; 32])).unwrap_or(0);
            texto_der(cv, x0 + w - 150, y + 8, LINEA_CUERPO, &s[..nd + 2], if a.cambio > 0 { VERDE } else { ROJO }, CUERPO);
        }
        let nd = formato(a.saldo, &mut d).unwrap_or(0);
        texto_der(cv, x0 + w - 6, y + 8, LINEA_CUERPO, &d[..nd], ORO, CUERPO);
        y += 37;
        cv.rect(x0, y - 1, w, 1, LINEA);
    }
    if v.libro.is_empty() {
        texto(cv, x0 + 6, y + 8, LINEA_CUERPO, b"aun nada: los movimientos de esta sesion se apuntan aqui", TENUE, CUERPO);
    }
    let rot = Estilo::normal(12).espaciado(140).mayusculas();
    texto(cv, x0, alto() - 52, 17, b"lo guarda el motor", TENUE, rot);
    texto_cabe(cv, x0, alto() - 32, LINEA_CUERPO, b"bankcat.dat se reescribe tras cada movimiento (BC4) y se carga al nacer", TENUE, CUERPO, w);
}

/// Tarjetas de texto en su rejilla (MOVER, MERCADO, REGLAS).
fn tarjetas_texto(cv: &mut Canvas, t: &[(&[u8], &[u8])]) {
    let (x0, y0, w) = (X_CENTRO + 20, 105, ancho_centro() - 40);
    let (k, tw) = columnas(w, t.len());
    let mut y = y0;
    for fila in t.chunks(k as usize) {
        let h = fila.iter().map(|(_, p)| alto_parrafo(p, tw - 30)).max().unwrap_or(60);
        for (i, &(titulo, p)) in fila.iter().enumerate() {
            let x = x0 + i as i32 * (tw + 12);
            let yy = tarjeta(cv, x, y, tw, h, titulo);
            parrafo(cv, x + 15, yy, tw - 30, LINEA_CUERPO, p, TEXTO, CUERPO, 12);
        }
        y += h + 12;
    }
}

/// **El cajero**: el candado de la maqueta (borde de rayas ambar) y sus tres
/// opciones, con la que NO en rojo.
fn cajero(cv: &mut Canvas) {
    let (x, y, w) = (X_CENTRO + 20, 105, ancho_centro() - 40);
    const OPCIONES: [(bool, &[u8], &[u8]); 3] = [
        (true, b"El pago lo haria la ANTENA, no BMO-X.", b"El movil paga por una tienda o pasarela; BMO-X nunca ve una tarjeta ni habla con un banco. Le llega un recibo firmado y lo apunta."),
        (true, b"Precio, impuestos y devoluciones, por escrito.", b"Vender moneda de juego tiene reglas en cada pais; van en las condiciones antes del primer centimo."),
        (false, b"Cambiar CAB por dinero (sacar), no.", b"Eso ya es dinero electronico: licencias, identidad de cada usuario, contra el blanqueo. Fuera de BANK CAT."),
    ];
    let intro: &[u8] = b"Como el Platinum de Warframe: el dinero entra en UN sentido. Compras CAB, los usas y los cambias con otros; los CAB no vuelven a ser dinero. Hasta que el propietario decida (D1), BANK CAT es un circuito CERRADO.";
    let ti = w - 34;
    let mut h = 17 + 28 + partir(intro, CUERPO, ti).len() as i32 * LINEA_CUERPO;
    // Cada opcion: 10 de aire, su titulo, su parrafo y 10 de aire.
    let alturas: Vec<i32> = OPCIONES.iter().map(|o| 22 + (1 + partir(o.2, CUERPO, ti - 34).len() as i32) * LINEA_CUERPO).collect();
    h += alturas.iter().sum::<i32>() + 16;
    redonda(cv, x, y, w, h, 12, mezclar(AMBAR, FONDO, 10, 256));
    // El borde de rayas (`1px dashed`): seis encendidos, cuatro apagados.
    for i in (12..w - 12).step_by(10) {
        cv.rect(x + i, y, 6.min(w - 12 - i), 1, AMBAR);
        cv.rect(x + i, y + h - 1, 6.min(w - 12 - i), 1, AMBAR);
    }
    for j in (12..h - 12).step_by(10) {
        cv.rect(x, y + j, 1, 6.min(h - 12 - j), AMBAR);
        cv.rect(x + w - 1, y + j, 1, 6.min(h - 12 - j), AMBAR);
    }
    for (cx, cy, a0) in [(x + 12, y + 12, 90), (x + w - 13, y + 12, 0), (x + 12, y + h - 13, 180), (x + w - 13, y + h - 13, 270)] {
        let mut p = Vec::new();
        bmo_letra::arco(cx * 64 + 32, cy * 64 + 32, 12 * 64, 12 * 64, a0, a0 + 90, &mut p);
        bmo_letra::pluma(&p, 64, false, |px, py, al| cv.blend(px, py, AMBAR, al as u32, 255));
    }
    let mut yy = y + 17;
    texto(cv, x + 17, yy, 21, b"Comprar CAB con dinero de verdad: decision del propietario", AMBAR, Estilo::negrita(15));
    yy += 29;
    yy += parrafo(cv, x + 17, yy, ti, LINEA_CUERPO, intro, TEXTO, CUERPO, 8) as i32 * LINEA_CUERPO + 8;
    for (k, &(si, t, p)) in OPCIONES.iter().enumerate() {
        cv.rect(x + 17, yy, ti, 1, LINEA);
        let c = if si { VERDE } else { ROJO };
        // La marca: un visto o una equis, a pluma.
        let (mx, my) = ((x + 23) * 64, (yy + 21) * 64);
        if si {
            bmo_letra::pluma(&[(mx - 320, my), (mx - 96, my + 224), (mx + 352, my - 288)], 110, false, |px, py, al| cv.blend(px, py, c, al as u32, 255));
        } else {
            for (a, b) in [((-288, -288), (288, 288)), ((-288, 288), (288, -288))] {
                bmo_letra::pluma(&[(mx + a.0, my + a.1), (mx + b.0, my + b.1)], 110, false, |px, py, al| cv.blend(px, py, c, al as u32, 255));
            }
        }
        texto(cv, x + 17 + 34, yy + 11, LINEA_CUERPO, t, BLANCO, FUERTE);
        parrafo(cv, x + 17 + 34, yy + 11 + LINEA_CUERPO, ti - 34, LINEA_CUERPO, p, TEXTO, CUERPO, 4);
        yy += alturas[k];
    }
}

fn panel(cv: &mut Canvas, v: &Vista) {
    let x0 = x_panel();
    cv.rect(x0, BARRA, PANEL, alto() - BARRA - 1, crate::tinta::PANEL);
    cv.rect(x0, BARRA, 1, alto() - BARRA - 1, LINEA);
    let (x, tw) = (x0 + 15, PANEL - 29);
    let ti = tw - 30;
    let mut y = BARRA + 14;
    // El gato hucha.
    let p0: &[u8] = b"Recibe y le salen monedas en los ojos y salta el monoculo; pagas y se le cae una lagrima. Si no hay saldo, no fia.";
    let h = alto_parrafo(p0, ti);
    let yy = tarjeta(cv, x, y, tw, h, b"el gato hucha");
    parrafo(cv, x + 15, yy, ti, LINEA_CUERPO, p0, TEXTO, CUERPO, 8);
    y += h + 12;
    // Quien lleva la cuenta.
    let h = 39 + 5 * 26 + 16;
    let yy = tarjeta(cv, x, y, tw, h, b"quien lleva la cuenta");
    let vivo = v.saldo.is_some();
    for (k, (a, b, c)) in [
        (&b"el libro"[..], &b"COBOL"[..], TEXTO),
        (b"los numeros", b"COMP-3", TEXTO),
        (b"compilado a", b"x86-64", TEXTO),
        (b"el juez", b"pasa", VERDE),
        (b"el motor", if vivo { &b"vivo"[..] } else { b"aun no" }, if vivo { VERDE } else { AMBAR }),
    ]
    .into_iter()
    .enumerate()
    {
        par(cv, x + 15, yy + k as i32 * 26, ti, a, b, c);
    }
    y += h + 12;
    // La red.
    let p2: &[u8] = b"BANK CAT no tiene red propia: lo que sale, saldra por HERMES. Celoso, como todo BMO-X.";
    let h = alto_parrafo(p2, ti);
    if y + h < alto() - 8 {
        let yy = tarjeta(cv, x, y, tw, h, b"la red");
        parrafo(cv, x + 15, yy, ti, LINEA_CUERPO, p2, TEXTO, CUERPO, 8);
    }
}
