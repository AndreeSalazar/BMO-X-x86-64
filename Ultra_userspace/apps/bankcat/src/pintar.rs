//! **LA VENTANA DE BANK CAT** -- la cara de la maqueta
//! (`docs/arte/maqueta_bankcat.html`), de la misma familia que HERMES:
//!
//! ```text
//!    barra    BANK CAT, su programa, de donde sale el dinero y la X
//!    riel     cartera, mover, mercado, el libro, cajero y reglas
//!    lista    el monedero de CAB y lo que el motor contesto
//!    centro   el GATO HUCHA, el saldo grande y los tres botones (cartera);
//!             el libro de esta sesion; o la tarjeta de lo que aun no hay
//!    panel    quien lleva la cuenta (COBOL, COMP-3...) y la red
//! ```
//!
//! *** Ni un centimo se calcula aqui: el saldo es lo que el motor COBOL
//! contesto, y la cara solo lo muestra (y lo escribe en castellano con
//! `bmo_bankcat::formato`).

use crate::canvas::Canvas;
use crate::gato::{self, Humor};
use crate::piezas::{arco, caja, negrita, negrita_fit, pildora, redonda, rotulo, trazo};
use alloc::vec::Vec;
use bmo_bankcat::{formato, Centimos, Estado};
use bmo_dibujo::{mezclar, Color, Lienzo};

pub const ANCHO: u32 = 1200;
pub const ALTO: u32 = 720;
pub const MINIMO: (u32, u32) = (900, 600);

static MEDIDA: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(ANCHO << 16 | ALTO);

pub fn medir(w: u32, h: u32) {
    MEDIDA.store(w.max(MINIMO.0) << 16 | h.max(MINIMO.1), core::sync::atomic::Ordering::Relaxed);
}

fn ancho() -> i32 {
    (MEDIDA.load(core::sync::atomic::Ordering::Relaxed) >> 16) as i32
}

fn alto() -> i32 {
    (MEDIDA.load(core::sync::atomic::Ordering::Relaxed) & 0xFFFF) as i32
}

const BARRA: i32 = 30;
const RIEL: i32 = 64;
const LISTA: i32 = 240;
const X_CENTRO: i32 = RIEL + LISTA;
const CABECERA: i32 = BARRA + 52;

fn ancho_panel() -> i32 {
    if ancho() >= 1120 { 268 } else { 0 }
}

fn ancho_centro() -> i32 {
    ancho() - X_CENTRO - ancho_panel()
}

// La paleta de la maqueta.
const NEGRO: Color = 0x0005_060A;
const FONDO: Color = 0x0008_0A10;
const PANEL: Color = 0x000B_0E15;
const TARJETA: Color = 0x000D_1118;
const LINEA: Color = 0x001B_2130;
const TEXTO: Color = 0x00C9_D3DB;
const TENUE: Color = 0x006E_7A89;
const GRIS: Color = 0x009A_A2B4;
const BLANCO: Color = 0x00F2_F7F9;
const CIAN: Color = 0x005E_F2E6;
const ROSA: Color = 0x00FF_2E88;
const AZUL: Color = 0x003D_A5FF;
const AMBAR: Color = 0x00FF_C24D;
const VERDE: Color = 0x004D_E38F;
const ROJO: Color = 0x00FF_4F5E;
const ORO: Color = 0x00FF_D45E;

pub const CARTERA: usize = 0;
pub const MOVER: usize = 1;
pub const MERCADO: usize = 2;
pub const LIBRO: usize = 3;
pub const CAJERO: usize = 4;
pub const REGLAS: usize = 5;

/// Las secciones: nombre, color y de que van.
pub const SECCIONES: [(&[u8], Color, &[u8]); 6] = [
    (b"Cartera", ORO, b"lo tuyo, al centimo"),
    (b"Mover", CIAN, b"enviar y pedir, por HERMES"),
    (b"Mercado", ROSA, b"cambiar con amigos"),
    (b"El libro", AZUL, b"lo lleva COBOL"),
    (b"Cajero", AMBAR, b"comprar CAB con dinero"),
    (b"Reglas", GRIS, b"lo que el gato no negocia"),
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

/// Los tres botones: que dicen y su color.
pub const BOTONES: [(&[u8], Color); 3] = [(b"Cobrar 50,00", VERDE), (b"Pagar 19,99", ROSA), (b"3 x 19,99", CIAN)];

fn dentro(x: i32, y: i32, (bx, by, bw, bh): (i32, i32, i32, i32)) -> bool {
    x >= bx && y >= by && x < bx + bw && y < by + bh
}

fn caja_cerrar() -> (i32, i32, i32, i32) {
    (ancho() - 38, 3, 32, BARRA - 6)
}

fn caja_burbuja(i: usize) -> (i32, i32, i32, i32) {
    ((RIEL - 42) / 2, BARRA + 40 + i as i32 * 52, 42, 42)
}

/// Donde va el gato y donde el saldo: lado a lado si cabe, si no apilados.
fn sitio_gato() -> (i32, i32) {
    (X_CENTRO + 24, CABECERA + 26)
}

fn x_saldo() -> i32 {
    X_CENTRO + 24 + gato::ANCHO + 30
}

/// Lo que mide la columna del saldo, entre el gato y el panel.
fn ancho_saldo() -> i32 {
    X_CENTRO + ancho_centro() - 24 - x_saldo()
}

/// Los tres botones, uno debajo de otro en la columna del saldo.
fn caja_boton(k: usize) -> (i32, i32, i32, i32) {
    (x_saldo(), CABECERA + 196 + k as i32 * 44, ancho_saldo().min(240), 36)
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

/// Un texto partido en lineas de `max` letras, por los espacios.
fn partir(t: &[u8], max: usize) -> Vec<&[u8]> {
    let mut v = Vec::new();
    let mut resto = t;
    while !resto.is_empty() {
        if resto.len() <= max {
            v.push(resto);
            break;
        }
        let corte = resto[..max].iter().rposition(|&c| c == b' ').filter(|&k| k > 0).unwrap_or(max);
        v.push(&resto[..corte]);
        resto = &resto[corte..];
        while resto.first() == Some(&b' ') {
            resto = &resto[1..];
        }
    }
    v
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
    if ancho_panel() > 0 {
        panel(cv, v, ancho() - ancho_panel());
    }
}

fn barra(cv: &mut Canvas, v: &Vista) {
    cv.rect(0, 0, ancho(), BARRA, NEGRO);
    cv.rect(0, BARRA - 1, ancho(), 1, LINEA);
    let fin = negrita(cv, 14, 7, b"BANK CAT", BLANCO);
    cv.text(14 + fin + 14, 7, b"sys/bankcat.bex - el libro: cobol/11/libro.bex", TENUE, 1);
    let (cx, cy, cw, ch) = caja_cerrar();
    cv.text(cx - 16 - 18 * 8, 7, b"Tab secciones  Esc", mezclar(TENUE, NEGRO, 170, 256), 1);
    let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (cx, cy, cw, ch)));
    if encima {
        redonda(cv, cx, cy, cw, ch, 6, mezclar(ROJO, NEGRO, 120, 256));
    }
    let (mx, my) = (cx + cw / 2, cy + ch / 2);
    let c = if encima { BLANCO } else { GRIS };
    for g in 0..2 {
        cv.line((mx - 5 + g, my - 5), (mx + 5 + g, my + 5), c);
        cv.line((mx - 5 + g, my + 5), (mx + 5 + g, my - 5), c);
    }
}

/// El icono de linea de cada seccion, como los de la maqueta.
fn icono(cv: &mut Canvas, i: usize, cx: i32, cy: i32, c: Color, bg: Color) {
    match i {
        // La cartera.
        CARTERA => {
            redonda(cv, cx - 10, cy - 7, 20, 15, 4, c);
            redonda(cv, cx - 8, cy - 5, 16, 11, 2, bg);
            cv.rect(cx - 9, cy - 3, 18, 2, c);
            cv.rect(cx + 3, cy + 2, 4, 2, c);
        }
        // Dos flechas, ida y vuelta.
        MOVER => {
            trazo(cv, (cx - 9, cy - 4), (cx + 8, cy - 4), c);
            trazo(cv, (cx + 8, cy - 4), (cx + 4, cy - 8), c);
            trazo(cv, (cx + 9, cy + 4), (cx - 8, cy + 4), c);
            trazo(cv, (cx - 8, cy + 4), (cx - 4, cy + 8), c);
        }
        // La tienda: un toldo y su puerta.
        MERCADO => {
            trazo(cv, (cx - 9, cy - 3), (cx - 7, cy - 9), c);
            trazo(cv, (cx - 7, cy - 9), (cx + 7, cy - 9), c);
            trazo(cv, (cx + 7, cy - 9), (cx + 9, cy - 3), c);
            trazo(cv, (cx - 9, cy - 3), (cx + 9, cy - 3), c);
            trazo(cv, (cx - 8, cy - 3), (cx - 8, cy + 9), c);
            trazo(cv, (cx + 8, cy - 3), (cx + 8, cy + 9), c);
            trazo(cv, (cx - 8, cy + 9), (cx + 8, cy + 9), c);
            redonda(cv, cx - 3, cy + 2, 6, 7, 1, c);
        }
        // El libro: una hoja con sus renglones.
        LIBRO => {
            redonda(cv, cx - 8, cy - 10, 16, 20, 3, c);
            redonda(cv, cx - 6, cy - 8, 12, 16, 1, bg);
            for k in 0..3 {
                cv.rect(cx - 4, cy - 5 + k * 5, 8 - (k / 2) * 3, 2, c);
            }
        }
        // El cajero: una caja con la flecha que baja.
        CAJERO => {
            redonda(cv, cx - 9, cy - 9, 18, 18, 4, c);
            redonda(cv, cx - 7, cy - 7, 14, 14, 2, bg);
            trazo(cv, (cx, cy - 4), (cx, cy + 4), c);
            trazo(cv, (cx - 3, cy + 1), (cx, cy + 4), c);
            trazo(cv, (cx + 3, cy + 1), (cx, cy + 4), c);
        }
        // El escudo de las reglas, con su visto.
        _ => {
            trazo(cv, (cx - 8, cy - 9), (cx + 7, cy - 9), c);
            trazo(cv, (cx - 8, cy - 9), (cx - 8, cy + 1), c);
            trazo(cv, (cx + 7, cy - 9), (cx + 7, cy + 1), c);
            trazo(cv, (cx - 8, cy + 1), (cx, cy + 10), c);
            trazo(cv, (cx + 7, cy + 1), (cx, cy + 10), c);
            trazo(cv, (cx - 4, cy - 1), (cx - 1, cy + 2), c);
            trazo(cv, (cx - 1, cy + 2), (cx + 4, cy - 4), c);
        }
    }
}

fn riel(cv: &mut Canvas, v: &Vista) {
    cv.rect(0, BARRA, RIEL, alto() - BARRA, NEGRO);
    cv.rect(RIEL - 1, BARRA, 1, alto() - BARRA, LINEA);
    let w = negrita(cv, 0, -100, b"CAB", ORO);
    negrita(cv, (RIEL - w) / 2, BARRA + 12, b"CAB", ORO);
    for (i, &(_, color, _)) in SECCIONES.iter().enumerate() {
        let (x, y, w, h) = caja_burbuja(i);
        let elegida = i == v.sec;
        let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (x, y, w, h)));
        let (cx, cy) = (x + w / 2, y + h / 2);
        let bg = if elegida { mezclar(color, NEGRO, 52, 256) } else if encima { 0x0016_1B26 } else { 0x0010_131B };
        if elegida {
            cv.glow(x, y, w, h, color, 4, 30);
            caja(cv, x, y, w, h, 13, bg, mezclar(color, NEGRO, 200, 256));
            redonda(cv, 0, y + 8, 4, h - 16, 2, BLANCO);
        } else {
            cv.disc(cx, cy, w / 2, mezclar(color, NEGRO, if encima { 150 } else { 70 }, 256));
            cv.disc(cx, cy, w / 2 - 1, bg);
        }
        icono(cv, i, cx, cy, if elegida || encima { color } else { mezclar(color, NEGRO, 190, 256) }, bg);
    }
}

fn lista(cv: &mut Canvas, v: &Vista) {
    let (nombre, color, _) = SECCIONES[v.sec];
    cv.rect(RIEL, BARRA, LISTA, alto() - BARRA, PANEL);
    cv.rect(X_CENTRO - 1, BARRA, 1, alto() - BARRA, LINEA);
    cv.rect(RIEL, CABECERA - 1, LISTA, 1, LINEA);
    negrita(cv, RIEL + 16, BARRA + 18, nombre, BLANCO);
    rotulo(cv, RIEL + 16, CABECERA + 14, b"tus monederos", mezclar(TENUE, PANEL, 200, 256));
    // El monedero de CAB.
    let (x, y) = (RIEL + 8, CABECERA + 38);
    redonda(cv, x, y, LISTA - 16, 48, 9, mezclar(color, PANEL, 30, 256));
    cv.disc(x + 24, y + 24, 17, ORO);
    negrita(cv, x + 20, y + 16, b"$", 0x0005_060A);
    negrita(cv, x + 50, y + 7, b"CAB", BLANCO);
    cv.text(x + 50, y + 26, b"la moneda de la casa", TENUE, 1);
    let mut d = [0u8; 32];
    let n = cifra(v.saldo, &mut d);
    cv.text(x + LISTA - 24 - n as i32 * 8, y + 7, &d[..n], ORO, 1);
    // Lo que el motor contesto, de mas nuevo a mas viejo.
    rotulo(cv, RIEL + 16, CABECERA + 104, b"lo ultimo", mezclar(TENUE, PANEL, 200, 256));
    let mut yy = CABECERA + 128;
    for a in v.libro.iter().rev().take(((alto() - yy - 80) / 24).max(0) as usize) {
        let (signo, c) = if a.estado != Estado::Hecho {
            (&b"NO "[..], ROJO)
        } else if a.cambio > 0 {
            (&b"+ "[..], VERDE)
        } else if a.cambio < 0 {
            (&b"- "[..], ROSA)
        } else {
            (&b"= "[..], TENUE)
        };
        let fin = cv.text(RIEL + 16, yy, signo, c, 1);
        if a.estado == Estado::Hecho {
            let n = formato(a.cambio.abs(), &mut d).unwrap_or(0);
            cv.text(RIEL + 16 + fin, yy, &d[..n], TEXTO, 1);
        } else {
            cv.text_fit(RIEL + 16 + fin, yy, a.estado.texto().as_bytes(), TENUE, LISTA - 32 - fin);
        }
        yy += 24;
    }
    if v.libro.is_empty() {
        cv.text(RIEL + 16, yy, b"aun nada", TENUE, 1);
    }
    // Abajo, quien eres.
    let y0 = alto() - 60;
    cv.rect(RIEL, y0, LISTA, 1, LINEA);
    cv.disc(RIEL + 30, y0 + 30, 16, ORO);
    negrita(cv, RIEL + 26, y0 + 22, b"B", 0x0005_060A);
    negrita(cv, RIEL + 56, y0 + 12, b"BMO-X (tu)", BLANCO);
    cv.text(RIEL + 56, y0 + 32, b"cuenta 0001", TENUE, 1);
}

fn centro(cv: &mut Canvas, v: &Vista) {
    let (nombre, color, sub) = SECCIONES[v.sec];
    let (x0, w) = (X_CENTRO, ancho_centro());
    let cy = BARRA + 26;
    let fin = negrita(cv, x0 + 20, cy - 8, nombre, BLANCO);
    cv.text(x0 + 20 + fin + 12, cy - 8, sub, TENUE, 1);
    let (pt, pc): (&[u8], Color) = match v.sec {
        CARTERA if v.esperando > 0 => (b"el motor piensa", AMBAR),
        CARTERA | LIBRO if v.saldo.is_some() => (b"libro cuadrado", VERDE),
        CARTERA | LIBRO => (b"sin motor", ROJO),
        MOVER | MERCADO => (b"sin red: H6", AZUL),
        CAJERO => (b"decision D1", AMBAR),
        _ => (b"4 reglas", GRIS),
    };
    pildora(cv, x0 + w - 18, cy - 11, pt, pc, FONDO);
    cv.rect(x0, CABECERA - 1, w, 1, LINEA);
    let t = llega(v.ms, v.desde_sec, 260);
    cv.gradient(x0, CABECERA - 2, w * 2 / 5 * t / 256, 2, color, FONDO);
    match v.sec {
        CARTERA => cartera(cv, v),
        LIBRO => libro(cv, v),
        MOVER => tarjeta_texto(
            cv,
            b"enviar a un amigo",
            b"Un envio se firmara con TU clave y el amigo lo apuntara en SU libro: los dos libros tienen que decir lo mismo, o el gato no lo da por hecho. Pide la red de HERMES (H6): hoy no hay amigos con quien mover CAB, y no se finge ninguno.",
        ),
        MERCADO => tarjeta_texto(
            cv,
            b"cambiar con amigos",
            b"Un cambio es de DOS lados a la vez (entrega contra pago): o pasan la cosa y los CAB, o no pasa nada. Es el syncpoint de CICS, y ESTRATOS ya lo tiene en el fondo. Llega con H6, como MOVER.",
        ),
        CAJERO => tarjeta_texto(
            cv,
            b"comprar CAB con dinero: decision del propietario (D1)",
            b"Si se hace: solo de ENTRADA, como el Platinum de Warframe. Pagas y recibes CAB; los CAB nunca vuelven a ser dinero. El pago lo haria la ANTENA, nunca BMO-X: aqui no se ve una tarjeta ni se habla con un banco. Sacar dinero, no: eso ya es dinero electronico, con licencia. Hasta que se decida, BANK CAT es un circuito CERRADO.",
        ),
        REGLAS => reglas(cv),
        _ => {}
    }
    if !v.aviso.is_empty() {
        cv.text_fit(x0 + 20, alto() - 30, v.aviso, AMBAR, w - 40);
    }
}

/// **La cartera**: el gato, el saldo, los botones y sus tres tarjetas.
fn cartera(cv: &mut Canvas, v: &Vista) {
    let (gx, gy) = sitio_gato();
    let t = v.ms.wrapping_sub(v.humor_desde);
    let humor = if t < gato::DURA_MS { v.humor } else { Humor::Quieto };
    gato::pintar(cv, gx, gy, v.ms, humor, t);
    let x = x_saldo();
    let y = CABECERA + 40;
    rotulo(cv, x, y, b"tu saldo", TENUE);
    let mut d = [0u8; 32];
    let n = cifra(v.saldo, &mut d);
    // La cifra grande, a la escala que quepa junto a su CAB.
    let escala = (4..=4).chain(3..=3).chain(2..=2).find(|e| n as i32 * 8 * e + 60 <= ancho_saldo()).unwrap_or(2);
    let fin = cv.text(x, y + 26, &d[..n], ORO, escala);
    cv.text(x + fin + 10, y + 26 + escala * 16 - 32, b"CAB", AMBAR, 2);
    let explica: &[u8] = b"Centimos ENTEROS (COMP-3): 19,99 x 3 = 59,97. Lo calcula el motor COBOL.";
    for (k, l) in partir(explica, (ancho_saldo() / 8).max(10) as usize).iter().take(3).enumerate() {
        cv.text(x, y + 100 + k as i32 * 18, l, TENUE, 1);
    }
    for (k, &(texto, c)) in BOTONES.iter().enumerate() {
        let (bx, by, bw, bh) = caja_boton(k);
        let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (bx, by, bw, bh)));
        caja(cv, bx, by, bw, bh, 10, mezclar(c, PANEL, if encima { 70 } else { 36 }, 256), mezclar(c, PANEL, 200, 256));
        let tw = texto.len() as i32 * 8 + 1;
        negrita(cv, bx + (bw - tw) / 2, by + 12, texto, BLANCO);
    }
    // Las tarjetas de abajo.
    let ty = (gy + gato::ALTO + 14).max(caja_boton(2).1 + 52);
    let tw = (ancho_centro() - 48 - 2 * 12) / 3;
    let x0 = X_CENTRO + 24;
    let h = (alto() - ty - 20).min(110);
    if h < 60 {
        return;
    }
    caja(cv, x0, ty, tw, h, 10, TARJETA, LINEA);
    rotulo(cv, x0 + 14, ty + 12, b"el gato dice", TENUE);
    for (k, l) in partir(v.frase, ((tw - 28) / 8) as usize).iter().take(3).enumerate() {
        cv.text(x0 + 14, ty + 38 + k as i32 * 18, l, TEXTO, 1);
    }
    let x1 = x0 + tw + 12;
    caja(cv, x1, ty, tw, h, 10, TARJETA, LINEA);
    rotulo(cv, x1 + 14, ty + 12, b"esta sesion", TENUE);
    let entro: Centimos = v.libro.iter().filter(|a| a.cambio > 0).map(|a| a.cambio).sum();
    let salio: Centimos = v.libro.iter().filter(|a| a.cambio < 0).map(|a| -a.cambio).sum();
    for (k, (que, c, val)) in [(&b"entro"[..], VERDE, entro), (b"salio", ROSA, salio)].into_iter().enumerate() {
        let yy = ty + 38 + k as i32 * 22;
        cv.text(x1 + 14, yy, que, TENUE, 1);
        let n = formato(val, &mut d).unwrap_or(0);
        cv.text(x1 + tw - 14 - n as i32 * 8, yy, &d[..n], c, 1);
    }
    let x2 = x1 + tw + 12;
    caja(cv, x2, ty, tw, h, 10, TARJETA, LINEA);
    rotulo(cv, x2 + 14, ty + 12, b"el libro", TENUE);
    cv.text(x2 + 14, ty + 38, b"asientos", TENUE, 1);
    let hechos = v.libro.iter().filter(|a| a.estado == Estado::Hecho && a.cambio != 0).count();
    let mut e = [0u8; 8];
    let ne = crate::fmt_num(hechos as u64, &mut e);
    cv.text(x2 + tw - 14 - ne as i32 * 8, ty + 38, &e[..ne], TEXTO, 1);
    cv.text(x2 + 14, ty + 60, b"cuadra", TENUE, 1);
    let cuadra: &[u8] = if v.saldo.is_some() { b"debe = haber" } else { b"--" };
    cv.text(x2 + tw - 14 - cuadra.len() as i32 * 8, ty + 60, cuadra, VERDE, 1);
}

/// **El libro** de esta sesion: cada respuesta del motor, con el saldo.
fn libro(cv: &mut Canvas, v: &Vista) {
    let x0 = X_CENTRO + 20;
    let w = ancho_centro() - 40;
    let mut y = CABECERA + 20;
    for (k, cab) in [(0, &b"n"[..]), (60, b"que paso"), (w - 300, b"importe"), (w - 150, b"saldo")] {
        cv.text(x0 + k, y, cab, TENUE, 1);
    }
    y += 26;
    cv.rect(x0, y - 6, w, 1, LINEA);
    let caben = ((alto() - y - 40) / 24).max(0) as usize;
    let desde = v.libro.len().saturating_sub(caben);
    let mut d = [0u8; 32];
    for (i, a) in v.libro.iter().enumerate().skip(desde) {
        let mut n = [0u8; 8];
        let nn = crate::fmt_num(i as u64 + 1, &mut n);
        cv.text(x0, y, &n[..nn], TENUE, 1);
        let que: &[u8] = match (a.estado, a.cambio) {
            (Estado::Hecho, c) if c > 0 => b"entra al HABER",
            (Estado::Hecho, c) if c < 0 => b"sale del DEBE",
            (Estado::Hecho, _) => b"cuadrado",
            (e, _) => e.texto().as_bytes(),
        };
        cv.text_fit(x0 + 60, y, que, if a.estado == Estado::Hecho { TEXTO } else { ROJO }, w - 380);
        if a.cambio != 0 {
            let nd = formato(a.cambio, &mut d).unwrap_or(0);
            cv.text(x0 + w - 170 - nd as i32 * 8, y, &d[..nd], if a.cambio > 0 { VERDE } else { ROSA }, 1);
        }
        let nd = formato(a.saldo, &mut d).unwrap_or(0);
        cv.text(x0 + w - nd as i32 * 8, y, &d[..nd], ORO, 1);
        y += 24;
    }
    if v.libro.is_empty() {
        cv.text(x0, y, b"aun nada: los movimientos de esta sesion se apuntan aqui", TENUE, 1);
    }
    cv.text_fit(x0, alto() - 30, b"el libro vive en la memoria del motor mientras el escritorio vive; guardarlo en el disco es BC4", TENUE, w);
}

/// Una tarjeta de lo que AUN no hay: su titulo y un parrafo, partido al
/// ancho que quede.
fn tarjeta_texto(cv: &mut Canvas, titulo: &[u8], parrafo: &[u8]) {
    let (x, y, w) = (X_CENTRO + 20, CABECERA + 24, ancho_centro() - 40);
    let lineas = partir(parrafo, ((w - 32) / 8).max(10) as usize);
    let h = 52 + lineas.len() as i32 * 20;
    caja(cv, x, y, w, h, 12, TARJETA, mezclar(AMBAR, TARJETA, 120, 256));
    negrita_fit(cv, x + 16, y + 14, titulo, AMBAR, w - 32);
    for (k, l) in lineas.iter().enumerate() {
        cv.text(x + 16, y + 42 + k as i32 * 20, l, TEXTO, 1);
    }
}

fn reglas(cv: &mut Canvas) {
    const R: [(&[u8], &[u8]); 4] = [
        (b"1. partida doble", b"cada movimiento va al DEBE o al HABER, y inicial + haber - debe es el saldo"),
        (b"2. nada se borra", b"un error se corrige con otro asiento, como en un banco"),
        (b"3. sin servidor", b"los CAB entre amigos iran por HERMES, firmados, en los DOS libros"),
        (b"4. centimos enteros", b"nada de coma flotante: COBOL y su COMP-3"),
    ];
    let (x0, w) = (X_CENTRO + 20, ancho_centro() - 40);
    let tw = (w - 12) / 2;
    for (k, &(t, d)) in R.iter().enumerate() {
        let (x, y) = (x0 + (k as i32 % 2) * (tw + 12), CABECERA + 24 + (k as i32 / 2) * 112);
        caja(cv, x, y, tw, 100, 10, TARJETA, LINEA);
        negrita_fit(cv, x + 14, y + 14, t, BLANCO, tw - 28);
        for (j, l) in partir(d, ((tw - 28) / 8) as usize).iter().take(3).enumerate() {
            cv.text(x + 14, y + 40 + j as i32 * 18, l, TEXTO, 1);
        }
    }
}

fn panel(cv: &mut Canvas, v: &Vista, x0: i32) {
    let w = ancho_panel();
    cv.rect(x0, BARRA, w, alto() - BARRA, PANEL);
    cv.rect(x0, BARRA, 1, alto() - BARRA, LINEA);
    let (x, tw) = (x0 + 14, w - 28);
    let par = |cv: &mut Canvas, y: i32, k: &[u8], val: &[u8], c: Color| {
        cv.text(x + 14, y, k, TENUE, 1);
        cv.text(x + tw - 14 - val.len() as i32 * 8, y, val, c, 1);
    };
    let mut y = BARRA + 14;
    caja(cv, x, y, tw, 150, 10, TARJETA, LINEA);
    rotulo(cv, x + 14, y + 14, b"quien lleva la cuenta", TENUE);
    par(cv, y + 42, b"el libro", b"COBOL", TEXTO);
    par(cv, y + 64, b"los numeros", b"COMP-3", TEXTO);
    par(cv, y + 86, b"compilado a", b"x86-64", TEXTO);
    par(cv, y + 108, b"el motor", if v.saldo.is_some() { b"vivo" } else { b"aun no" }, if v.saldo.is_some() { VERDE } else { AMBAR });
    y += 164;
    caja(cv, x, y, tw, 132, 10, TARJETA, LINEA);
    rotulo(cv, x + 14, y + 14, b"el gato hucha", TENUE);
    for (k, l) in [&b"cobra: monedas en los ojos"[..], b"paga: una lagrima", b"sin saldo: no fia", b"(de prueba: sin H6 ni D1)"].iter().enumerate() {
        cv.text(x + 14, y + 42 + k as i32 * 20, l, TEXTO, 1);
    }
    y += 146;
    caja(cv, x, y, tw, 92, 10, TARJETA, LINEA);
    rotulo(cv, x + 14, y + 14, b"la red", TENUE);
    cv.text(x + 14, y + 42, b"BANK CAT no tiene red", TEXTO, 1);
    cv.text(x + 14, y + 62, b"propia: sale por HERMES", TEXTO, 1);
    // Un detalle vivo: la moneda que gira.
    let a = crate::mates::fase(v.ms, 3000);
    let r = (crate::mates::coseno(a).abs() * 14 / 256).max(2);
    let (mx, my) = (x + tw - 30, y + 30);
    for j in -14..=14 {
        let mut medio = 0;
        while (medio + 1) * (medio + 1) <= 196 - j * j {
            medio += 1;
        }
        let largo = r * medio / 14;
        cv.rect(mx - largo, my + j, 2 * largo, 1, ORO);
    }
    arco(cv, mx, my, 14, 0, 256, 0x00C9_8A1B);
}
