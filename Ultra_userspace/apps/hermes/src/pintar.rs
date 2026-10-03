//! **LA VENTANA DE HERMES** -- la cara de la maqueta (H1), de izquierda a
//! derecha:
//!
//! ```text
//!    riel     una burbuja por seccion, cada una con SU gesto vivo; la
//!             elegida se vuelve cuadrada y lleva su pastilla
//!    lista    lo de la seccion (tus notas, las tertulias, las piezas de la
//!             ONDA...) y abajo quien eres: el gato de BMO-X
//!    centro   la seccion, que ENTRA a su manera: los mensajes desde su
//!             lado, el MURO voltea, el CANAL se enciende como una tele, los
//!             pasos de ENVIOS se encienden en orden, las jaulas bajan
//! ```
//!
//! *** Nada inventado: lo que aun no existe (la red, los amigos, las fotos,
//! el video) se dice con su escalon del plan, y lo que existe es de verdad:
//! lo escrito se guarda, la ONDA toca en el escritorio y sus barras son el
//! medidor del maestro.

use crate::canvas::Canvas;
use crate::charla::Mensaje;
use crate::mates::{azar, coseno, entre, fase, onda, seno};
use alloc::vec::Vec;
use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_fondo::{Estilo, PIEZAS};
use bmo_userland as bmo;

pub const ANCHO: u32 = 1280;
pub const ALTO: u32 = 760;
/// Lo minimo que cabe: el riel, la lista y un centro que se lea.
pub const MINIMO: (u32, u32) = (960, 600);

/// La medida de verdad: la de la superficie de ahora (maximizar la cambia y
/// HERMES se vuelve a pintar a su medida, sin estirar nada).
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

const RIEL: i32 = 72;
const LISTA: i32 = 248;
const X_CENTRO: i32 = RIEL + LISTA;
const CABECERA: i32 = 64;
const BURBUJA: i32 = 48;
const PASO_RIEL: i32 = 58;
const FILA: i32 = 34;
const Y_FILAS: i32 = CABECERA + 12;
const CAJA_ALTO: i32 = 44;

// La paleta de la maqueta (`maqueta_hermes.html`).
const NEGRO: Color = 0x0005_060A;
const FONDO: Color = 0x0008_0A10;
const PANEL: Color = 0x000B_0E15;
const PANEL2: Color = 0x0010_141D;
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
const LIMA: Color = 0x00B6_FF5C;
const MORADO: Color = 0x00A5_5DFF;

/// **Las secciones**, en el orden del riel de la maqueta: nombre, color, y lo
/// que dice debajo del titulo.
pub const SECCIONES: [(&[u8], Color, &[u8]); 9] = [
    (b"Mensajes", CIAN, b"en una sola maquina: tus notas, guardadas. Los amigos llegan con H6, por huella y sin servidor"),
    (b"Tertulias", AZUL, b"una tertulia es un canal: hoy lo tuyo en el; con H6, lo de todos los que esten"),
    (b"El MURO", MORADO, b"tus fotos llegan con H9 (PNG y JPEG ya se leen); estas postales se dibujan aqui"),
    (b"El CANAL", ROJO, b"emite con H10: video comprimido de un BMO-X a otro"),
    (b"la ONDA", LIMA, b"trece piezas compuestas aqui, en este procesador: ninguna viene de fuera"),
    (b"Paginas", AMBAR, b"tu pagina en .maqueta, sin scripts y sin Google: H13 a H15"),
    (b"ENVIOS", VERDE, b"como llegara algo de un amigo: nada entra sin tu permiso ni sin el JUEZ (H8)"),
    (b"Amigos", ROSA, b"un amigo es una clave aceptada UNA vez, comparando la huella en voz alta (H6)"),
    (b"Las jaulas", GRIS, b"tres procesos, tres jaulas: ninguno ve lo del otro (H3, H4, H8)"),
];

pub const MENSAJES: usize = 0;
pub const TERTULIAS: usize = 1;
pub const MURO: usize = 2;
pub const CANAL: usize = 3;
pub const ONDA: usize = 4;
pub const PAGINAS: usize = 5;
pub const ENVIOS: usize = 6;
pub const AMIGOS: usize = 7;
pub const JAULAS: usize = 8;

/// Las tertulias de la casa.
pub const CANALES: [&[u8]; 4] = [b"#general", b"#bmo-x", b"#juegos", b"#musica"];

/// Lo que hay en la lista de una seccion.
pub fn cuantos_items(sec: usize) -> usize {
    match sec {
        TERTULIAS => CANALES.len(),
        ONDA => PIEZAS.len(),
        _ => 1,
    }
}

fn nombre_item(sec: usize, k: usize) -> &'static [u8] {
    match sec {
        MENSAJES => b"tus notas",
        TERTULIAS => CANALES[k % CANALES.len()],
        MURO => b"tu muro",
        CANAL => b"tu canal",
        ONDA => PIEZAS[k % PIEZAS.len()].nombre.as_bytes(),
        PAGINAS => b"hermes://tu/inicio",
        ENVIOS => b"como llega algo",
        AMIGOS => b"tu",
        _ => b"PUERTA, APP y JUEZ",
    }
}

/// El canal de charla de un item, si la seccion es de escribir.
pub fn canal_de(sec: usize, k: usize) -> Option<&'static [u8]> {
    match sec {
        MENSAJES => Some(b"notas"),
        TERTULIAS => Some(CANALES[k % CANALES.len()]),
        _ => None,
    }
}

/// Lo que hace falta para pintar un fotograma.
pub struct Vista<'a> {
    pub sec: usize,
    pub item: usize,
    pub ms: u32,
    /// Cuando cambio la seccion y cuando el item: las entradas se animan.
    pub desde_sec: u32,
    pub desde_item: u32,
    pub puntero: Option<(i32, i32)>,
    /// La charla del canal de delante, y cuantos hay en cada item.
    pub charla: &'a [&'a Mensaje],
    pub cuentas: &'a [usize],
    pub escribiendo: bool,
    pub borrador: &'a [u8],
    /// Cuando se mando el ultimo mensaje (entra deslizando).
    pub enviado: u32,
    /// El ZUMBIDO: cuando empezo el ultimo, y si ya se puede otro.
    pub zumbido: Option<u32>,
    pub zumbido_listo: bool,
    /// La ONDA: la pieza pedida, si suena el fondo, y el medidor del maestro.
    pub pedida: Option<usize>,
    /// Cuando se pidio (el disco gira y la vuelta del bucle cuenta desde ahi).
    pub pedida_desde: u32,
    pub sonando: bool,
    pub pico: [i32; 2],
    pub rms: [i32; 2],
    pub sin_guardar: bool,
    pub aviso: &'a [u8],
}

/// Donde cayo un clic.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Golpe {
    Seccion(usize),
    Item(usize),
    Escribir,
    Zumbido,
}

fn dentro(x: i32, y: i32, (bx, by, bw, bh): (i32, i32, i32, i32)) -> bool {
    x >= bx && y >= by && x < bx + bw && y < by + bh
}

fn caja_burbuja(i: usize) -> (i32, i32, i32, i32) {
    ((RIEL - BURBUJA) / 2, 12 + i as i32 * PASO_RIEL, BURBUJA, BURBUJA)
}

/// Las filas de la lista que caben, y desde cual se empieza (la elegida
/// siempre se ve).
fn filas_visibles(sec: usize, item: usize) -> (usize, usize) {
    let caben = ((alto() - Y_FILAS - 110) / FILA).max(1) as usize;
    let n = cuantos_items(sec);
    let desde = if item >= caben { item + 1 - caben } else { 0 };
    (desde, caben.min(n - desde.min(n)))
}

fn caja_fila(j: usize) -> (i32, i32, i32, i32) {
    (RIEL + 8, Y_FILAS + j as i32 * FILA, LISTA - 16, FILA - 4)
}

fn caja_escribir() -> (i32, i32, i32, i32) {
    (X_CENTRO + 20, alto() - CAJA_ALTO - 16, ancho() - X_CENTRO - 40, CAJA_ALTO)
}

fn caja_zumbido() -> (i32, i32, i32, i32) {
    (ancho() - 20 - 112, 14, 112, 32)
}

fn caja_atajo(k: usize) -> (i32, i32, i32, i32) {
    let w = (ancho() - X_CENTRO - 40 - 2 * 16) / 3;
    (X_CENTRO + 20 + k as i32 * (w + 16), CABECERA + 210, w, 70)
}

/// Los atajos de la pagina de inicio: a donde llevan.
const ATAJOS: [(usize, &[u8]); 3] = [(MENSAJES, b"tus notas"), (ONDA, b"la ONDA"), (TERTULIAS, b"#general")];

/// **Que hay debajo de un clic.**
pub fn golpe(x: i32, y: i32, sec: usize, item: usize) -> Option<Golpe> {
    for i in 0..SECCIONES.len() {
        if dentro(x, y, caja_burbuja(i)) {
            return Some(Golpe::Seccion(i));
        }
    }
    let (desde, n) = filas_visibles(sec, item);
    for j in 0..n {
        if dentro(x, y, caja_fila(j)) {
            return Some(Golpe::Item(desde + j));
        }
    }
    if canal_de(sec, item).is_some() && dentro(x, y, caja_escribir()) {
        return Some(Golpe::Escribir);
    }
    if sec == MENSAJES && dentro(x, y, caja_zumbido()) {
        return Some(Golpe::Zumbido);
    }
    if sec == PAGINAS {
        for (k, &(s, _)) in ATAJOS.iter().enumerate() {
            if dentro(x, y, caja_atajo(k)) {
                return Some(Golpe::Seccion(s));
            }
        }
    }
    None
}

/// De 0 a 256 en `dura` ms, empezando `retraso` ms despues de `desde`, con
/// un frenado al final (sale rapido y se posa).
fn llega(ms: u32, desde: u32, retraso: u32, dura: u32) -> i32 {
    let t = ms.wrapping_sub(desde);
    if t <= retraso {
        return 0;
    }
    let u = (((t - retraso) * 256) / dura.max(1)).min(256) as i32;
    // 1 - (1 - u)^2: frena al llegar.
    256 - (256 - u) * (256 - u) / 256
}

/// Un texto partido en lineas de `max` letras como mucho, por los espacios.
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

/// De dBFS (1/256) a 0..=256, con -60 dB como suelo.
fn nivel(db: i32) -> i32 {
    ((db + 60 * 256).clamp(0, 60 * 256) * 256) / (60 * 256)
}

/// **Un fotograma entero.**
pub fn pintar(cv: &mut Canvas, v: &Vista) {
    cv.clear(FONDO);
    riel(cv, v);
    lista(cv, v);
    // El ZUMBIDO sacude el centro y lo enciende en rosa (700 ms).
    let sacude = match v.zumbido {
        Some(t0) if v.ms.wrapping_sub(t0) < 700 => {
            let t = v.ms.wrapping_sub(t0) as i32;
            seno(t * 256 / 70) * 10 * (700 - t) / 700 / 256
        }
        _ => 0,
    };
    centro(cv, v, sacude);
    if let Some(t0) = v.zumbido {
        let t = v.ms.wrapping_sub(t0);
        if t < 700 {
            let a = (700 - t) * 120 / 700;
            cv.glow(X_CENTRO + 4, 4, ancho() - X_CENTRO - 8, alto() - 8, ROSA, 4, a);
            cv.frame(X_CENTRO, 0, ancho() - X_CENTRO, alto(), 2, mezclar(ROSA, FONDO, a * 2, 256));
        }
    }
}

// ============================== EL RIEL ==============================

fn riel(cv: &mut Canvas, v: &Vista) {
    cv.rect(0, 0, RIEL, alto(), NEGRO);
    cv.rect(RIEL - 1, 0, 1, alto(), LINEA);
    for (i, &(_, color, _)) in SECCIONES.iter().enumerate() {
        let (x, y, w, h) = caja_burbuja(i);
        let elegida = i == v.sec;
        let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (x, y, w, h)));
        let (cx, cy) = (x + w / 2, y + h / 2);
        if elegida {
            // Cuadrada (con las esquinas comidas) y su pastilla a la izquierda.
            cv.rect(x + 3, y, w - 6, h, mezclar(color, PANEL2, 60, 256));
            cv.rect(x, y + 3, w, h - 6, mezclar(color, PANEL2, 60, 256));
            cv.rect(0, y + 8, 4, h - 16, BLANCO);
        } else {
            cv.disc(cx, cy, w / 2, if encima { PANEL2 } else { PANEL });
            if encima {
                cv.rect(0, y + 18, 3, h - 36, TEXTO);
            }
        }
        icono(cv, i, cx, cy, v.ms, if elegida || encima { color } else { mezclar(color, PANEL, 150, 256) });
    }
}

/// **El gesto de cada seccion**: nueve, y ninguno se repite.
fn icono(cv: &mut Canvas, i: usize, cx: i32, cy: i32, ms: u32, c: Color) {
    match i {
        // Un bocadillo, y tres puntos que se escriben por turno.
        MENSAJES => {
            cv.frame(cx - 13, cy - 10, 26, 17, 2, c);
            cv.rect(cx - 8, cy + 7, 5, 4, c);
            let turno = (ms / 250 % 4) as i32;
            for k in 0..3 {
                if k < turno {
                    cv.rect(cx - 7 + k * 6, cy - 3, 3, 3, c);
                }
            }
        }
        // Tres que hablan: cabezas que botan, cada una a su tiempo.
        TERTULIAS => {
            for k in 0..3 {
                let bota = onda(ms + k as u32 * 180, 700) * 4 / 256;
                cv.disc(cx - 10 + k * 10, cy - 2 - bota, 4, c);
                cv.rect(cx - 14 + k * 10, cy + 5 - bota / 2, 9, 4, c);
            }
        }
        // Una foto que se voltea.
        MURO => {
            let w = (coseno(fase(ms, 1600)).abs() * 22 / 256).max(2);
            cv.frame(cx - w / 2, cy - 9, w, 18, 2, c);
            if w > 8 {
                cv.disc(cx, cy - 2, 2, c);
            }
        }
        // Una tele con su raya que baja.
        CANAL => {
            cv.frame(cx - 14, cy - 10, 28, 19, 2, c);
            cv.rect(cx - 5, cy + 10, 10, 2, c);
            let y = cy - 8 + (ms / 60 % 15) as i32;
            cv.rect(cx - 12, y, 24, 1, mezclar(c, PANEL, 160, 256));
        }
        // Cuatro barras que bailan.
        ONDA => {
            for k in 0..4 {
                let a = 4 + onda(ms + k as u32 * 97, 300 + k as u32 * 70) * 14 / 256;
                cv.rect(cx - 11 + k * 6, cy + 8 - a, 4, a, c);
            }
        }
        // Una pagina con su cursor.
        PAGINAS => {
            cv.frame(cx - 9, cy - 12, 18, 24, 2, c);
            cv.rect(cx - 5, cy - 6, 10, 2, c);
            cv.rect(cx - 5, cy - 1, 7, 2, c);
            if ms / 500 % 2 == 0 {
                cv.rect(cx - 5, cy + 4, 2, 5, c);
            }
        }
        // Una flecha que llega, una y otra vez.
        ENVIOS => {
            let x = cx - 14 + (ms / 30 % 18) as i32;
            cv.rect(x, cy - 1, 12, 3, c);
            for k in 0..5 {
                cv.rect(x + 12 + k, cy - 5 + k, 1, 11 - 2 * k, c);
            }
            cv.rect(cx + 12, cy - 9, 2, 18, mezclar(c, PANEL, 120, 256));
        }
        // Dos que se dan vueltas.
        AMIGOS => {
            let a = fase(ms, 2400);
            let (dx, dy) = (coseno(a) * 8 / 256, seno(a) * 4 / 256);
            cv.disc(cx + dx, cy + dy, 5, c);
            cv.disc(cx - dx, cy - dy, 4, mezclar(c, PANEL, 140, 256));
        }
        // Barrotes, y una tabla que baja y sube.
        _ => {
            for k in 0..4 {
                cv.rect(cx - 11 + k * 7, cy - 11, 2, 22, c);
            }
            let y = cy - 11 + onda(ms, 2000) * 20 / 256;
            cv.rect(cx - 13, y, 26, 2, c);
        }
    }
}

// ============================== LA LISTA ==============================

fn lista(cv: &mut Canvas, v: &Vista) {
    let (nombre, color, _) = SECCIONES[v.sec];
    cv.rect(RIEL, 0, LISTA, alto(), PANEL);
    cv.rect(X_CENTRO - 1, 0, 1, alto(), LINEA);
    cv.rect(RIEL, CABECERA - 1, LISTA, 1, LINEA);
    // El nombre de la seccion entra escribiendose.
    let n = ((v.ms.wrapping_sub(v.desde_sec) / 35) as usize).min(nombre.len());
    cv.text(RIEL + 16, 20, &nombre[..n], color, 2);
    let (desde, filas) = filas_visibles(v.sec, v.item);
    for j in 0..filas {
        let k = desde + j;
        let (x, y, w, h) = caja_fila(j);
        // Cada fila entra desde la izquierda, una tras otra.
        let t = llega(v.ms, v.desde_sec, j as u32 * 30, 220);
        let x = x - (256 - t) * 40 / 256;
        let elegida = k == v.item;
        let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (x, y, w, h)));
        if elegida {
            cv.rect(x, y, w, h, mezclar(color, PANEL, 50, 256));
            cv.rect(x, y, 3, h, color);
        } else if encima {
            cv.rect(x, y, w, h, PANEL2);
        }
        let tinta = mezclar(if elegida { BLANCO } else { TEXTO }, PANEL, t as u32, 256);
        if v.sec == ONDA {
            // La ONDA: la que se pidio lleva su ecualizador chico.
            let p = &PIEZAS[k];
            let suena = v.pedida == Some(k) && v.sonando;
            if suena {
                for b in 0..3 {
                    let a = 3 + onda(v.ms + b as u32 * 110, 380) * 10 / 256;
                    cv.rect(x + 8 + b * 4, y + h / 2 + 6 - a, 3, a, LIMA);
                }
            }
            cv.text_fit(x + 26, y + 9, p.nombre.as_bytes(), if suena { LIMA } else { tinta }, w - 70);
            let mut d = [0u8; 8];
            let nd = crate::fmt_num(p.bpm as u64, &mut d);
            cv.text(x + w - 8 - nd as i32 * 8, y + 9, &d[..nd], TENUE, 1);
            if p.estilo == Estilo::NekoPhonk {
                cv.rect(x + w - 44, y + 13, 6, 6, ROSA);
            }
            continue;
        }
        let pre: &[u8] = if v.sec == TERTULIAS { b"" } else { b"> " };
        let fin = cv.text(x + 12, y + 9, pre, color, 1);
        cv.text_fit(x + 12 + fin, y + 9, nombre_item(v.sec, k), tinta, w - 60);
        if let Some(&c) = v.cuentas.get(k) {
            if c > 0 {
                let mut d = [0u8; 8];
                let nd = crate::fmt_num(c as u64, &mut d);
                let bw = nd as i32 * 8 + 10;
                cv.rect(x + w - bw - 6, y + 7, bw, 18, mezclar(color, PANEL, 90, 256));
                cv.text(x + w - bw + 5 - 6, y + 9, &d[..nd], BLANCO, 1);
            }
        }
    }
    // Abajo, quien eres: el gato chico de BMO-X, y lo que aun no hay.
    let y0 = alto() - 96;
    cv.rect(RIEL, y0, LISTA, 96, NEGRO);
    cv.rect(RIEL, y0, LISTA, 1, LINEA);
    gato_chico(cv, RIEL + 14, y0 + 16, v.ms);
    cv.text(RIEL + 70, y0 + 22, b"tu BMO-X", BLANCO, 1);
    cv.text(RIEL + 70, y0 + 42, b"sin red todavia", TENUE, 1);
    cv.text(RIEL + 70, y0 + 60, b"(la PUERTA es H4)", TENUE, 1);
    let luz = if v.ms / 1200 % 2 == 0 { AMBAR } else { mezclar(AMBAR, NEGRO, 120, 256) };
    cv.disc(RIEL + 50, y0 + 66, 5, luz);
}

/// El gato de BMO-X a un cuarto (38 x 45), que parpadea de vez en cuando.
fn gato_chico(cv: &mut Canvas, x: i32, y: i32, ms: u32) {
    use crate::gato::{EYES, HEIGHT, STROKE, WIDTH};
    let bit = |m: &[u8], fx: u32, fy: u32| {
        let i = (fy * WIDTH + fx) as usize;
        m[i / 8] >> (i % 8) & 1 == 1
    };
    let parpadea = ms % 4000 < 150;
    for by in 0..HEIGHT / 4 {
        for bx in 0..WIDTH / 4 {
            let (mut trazo, mut ojo) = (false, false);
            for d in 0..16 {
                let (fx, fy) = (bx * 4 + d % 4, by * 4 + d / 4);
                trazo |= bit(&STROKE, fx, fy);
                ojo |= bit(&EYES, fx, fy);
            }
            let c = if ojo && !parpadea {
                CIAN
            } else if trazo || ojo {
                TEXTO
            } else {
                continue;
            };
            cv.put(x + bx as i32, y + by as i32, c);
        }
    }
}

// ============================== EL CENTRO ==============================

fn centro(cv: &mut Canvas, v: &Vista, dx: i32) {
    let (_, color, sub) = SECCIONES[v.sec];
    let (x0, w) = (X_CENTRO + dx, ancho() - X_CENTRO);
    cv.rect(X_CENTRO, CABECERA - 1, w, 1, LINEA);
    // La cabecera: el item y lo que es, de verdad.
    // En la ONDA el nombre de la pieza ya va grande en la ficha: arriba, la
    // seccion.
    let titulo = if v.sec == ONDA { b"la musica de BMO-X".as_slice() } else { nombre_item(v.sec, v.item) };
    let t = llega(v.ms, v.desde_item, 0, 260);
    cv.text(x0 + 20 + (256 - t) * 20 / 256, 14, titulo, mezclar(BLANCO, FONDO, t as u32, 256), 2);
    cv.text_fit(x0 + 20, 44, sub, TENUE, w - 40 - if v.sec == MENSAJES { 130 } else { 0 });
    match v.sec {
        MENSAJES | TERTULIAS => charla(cv, v, x0, color),
        MURO => muro(cv, v, x0),
        CANAL => tele(cv, v, x0),
        ONDA => la_onda(cv, v, x0),
        PAGINAS => paginas(cv, v, x0),
        ENVIOS => envios(cv, v, x0),
        AMIGOS => amigos(cv, v, x0),
        JAULAS => jaulas(cv, v, x0),
        _ => {}
    }
    if !v.aviso.is_empty() {
        cv.text_fit(x0 + 20, alto() - 14 - 16 - if canal_de(v.sec, v.item).is_some() { CAJA_ALTO + 12 } else { 0 }, v.aviso, AMBAR, w - 40);
    }
}

/// **La charla**: los mensajes desde su lado, y la caja de escribir.
fn charla(cv: &mut Canvas, v: &Vista, x0: i32, color: Color) {
    let w = ancho() - X_CENTRO;
    if v.sec == MENSAJES {
        // El ZUMBIDO: uno cada 10 s, como en la maqueta.
        let (bx, by, bw, bh) = caja_zumbido();
        let c = if v.zumbido_listo { ROSA } else { mezclar(ROSA, FONDO, 70, 256) };
        cv.rect(bx + x0 - X_CENTRO, by, bw, bh, mezclar(c, FONDO, 50, 256));
        cv.frame(bx + x0 - X_CENTRO, by, bw, bh, 2, c);
        cv.text(bx + x0 - X_CENTRO + 24, by + 9, b"ZUMBIDO", if v.zumbido_listo { BLANCO } else { TENUE }, 1);
    }
    let (ex, ey, ew, eh) = caja_escribir();
    let ex = ex + x0 - X_CENTRO;
    let por_linea = ((ew - 80) / 8).max(10) as usize;
    let tope = CABECERA + 16;
    let mut y = ey - 18;
    if v.charla.is_empty() {
        let t = llega(v.ms, v.desde_item, 100, 400);
        let c = mezclar(TENUE, FONDO, t as u32, 256);
        let mid = (tope + ey) / 2;
        cv.text(x0 + (w - 30 * 8) / 2, mid - 20, b"aun no hay nada escrito aqui", c, 1);
        cv.text(x0 + (w - 46 * 8) / 2, mid + 4, b"lo que escribas se queda en sys/hermsg.txt", c, 1);
    }
    // Del mas nuevo hacia arriba, hasta que no quepan.
    for (j, m) in v.charla.iter().rev().enumerate() {
        let lineas = partir(&m.texto, por_linea);
        let alto_b = lineas.len() as i32 * 18 + 16;
        y -= alto_b;
        if y < tope {
            break;
        }
        let ancho_b = lineas.iter().map(|l| l.len()).max().unwrap_or(0) as i32 * 8 + 24;
        // ** ENTRAN DESDE SU LADO: en tus notas los mensajes son tuyos y
        // llegan por la derecha; en una tertulia suben desde abajo.
        let base = if j == 0 { v.enviado.max(v.desde_item) } else { v.desde_item };
        let t = llega(v.ms, base, if j == 0 { 0 } else { j as u32 * 35 }, 300);
        let (bx, by) = if v.sec == MENSAJES {
            (x0 + w - 20 - ancho_b + (256 - t) * 260 / 256, y)
        } else {
            (x0 + 20, y + (256 - t) * 60 / 256)
        };
        let fondo_b = if v.sec == MENSAJES { mezclar(color, FONDO, 40, 256) } else { PANEL2 };
        cv.rect(bx, by, ancho_b, alto_b, mezclar(fondo_b, FONDO, t as u32, 256));
        if v.sec == MENSAJES {
            cv.rect(bx + ancho_b - 3, by, 3, alto_b, mezclar(color, FONDO, t as u32, 256));
        } else {
            cv.rect(bx, by, 3, alto_b, mezclar(color, FONDO, t as u32, 256));
        }
        for (k, l) in lineas.iter().enumerate() {
            cv.text(bx + 12, by + 8 + k as i32 * 18, l, mezclar(TEXTO, FONDO, t as u32, 256), 1);
        }
        y -= 8;
    }
    // La caja de escribir.
    cv.rect(ex, ey, ew, eh, if v.escribiendo { PANEL2 } else { PANEL });
    cv.frame(ex, ey, ew, eh, 1, if v.escribiendo { color } else { LINEA });
    if v.escribiendo || !v.borrador.is_empty() {
        let cabe = ((ew - 30) / 8) as usize;
        let ver = &v.borrador[v.borrador.len().saturating_sub(cabe)..];
        let fin = cv.text(ex + 12, ey + 14, ver, BLANCO, 1);
        if v.escribiendo && v.ms / 500 % 2 == 0 {
            cv.rect(ex + 13 + fin, ey + 12, 2, 20, color);
        }
    } else {
        let donde: &[u8] = if v.sec == MENSAJES { b"escribe una nota" } else { b"escribe en la tertulia" };
        let fin = cv.text(ex + 12, ey + 14, donde, TENUE, 1);
        cv.text(ex + 12 + fin, ey + 14, b"  (/ o clic; Enter manda, Esc sale)", mezclar(TENUE, PANEL, 150, 256), 1);
    }
    if v.sin_guardar {
        cv.text(ex + ew - 30 * 8, ey - 16, b"no se pudo guardar en el disco", ROJO, 1);
    }
}

/// **El MURO**: seis postales que se voltean una tras otra.
fn muro(cv: &mut Canvas, v: &Vista, x0: i32) {
    let w = ancho() - X_CENTRO - 40;
    let cw = (w - 2 * 16) / 3;
    let ch = ((alto() - CABECERA - 80) / 2 - 16).min(cw * 3 / 4);
    for k in 0..6 {
        let (col, fila) = (k % 3, k / 3);
        let (x, y) = (x0 + 20 + col * (cw + 16), CABECERA + 24 + fila * (ch + 16));
        // El volteo: de la espalda (0) a la cara (128), en vueltas de 256.
        let f = llega(v.ms, v.desde_sec, k as u32 * 110, 520) / 2;
        let ancho_v = (coseno(f).abs() * cw / 256).max(1);
        let xv = x + (cw - ancho_v) / 2;
        if f < 64 {
            cv.rect(xv, y, ancho_v, ch, PANEL2);
            cv.frame(xv, y, ancho_v, ch, 2, mezclar(MORADO, FONDO, 120, 256));
            if ancho_v > 30 {
                cv.text(xv + ancho_v / 2 - 8, y + ch / 2 - 16, b"H", MORADO, 2);
            }
            continue;
        }
        postal(cv, xv, y, ancho_v, ch, k as u32, v.ms);
    }
    cv.text(x0 + 20, alto() - 30, b"postales dibujadas aqui con una semilla cada una; tus fotos de verdad: H9", TENUE, 1);
}

/// Un paisaje de una semilla: cielo, sol, montes y una estrella que pasa.
fn postal(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, semilla: u32, ms: u32) {
    let r = |i: u32| azar(semilla * 977 + i);
    let paleta = [(0x0014_1E4A, 0x00FF_7A59), (0x0006_2A3A, 0x005E_F2E6), (0x001E_0B36, 0x00FF_2E88), (0x0010_2410, 0x00B6_FF5C), (0x0024_1408, 0x00FF_C24D), (0x000A_1030, 0x003D_A5FF)];
    let (arriba, abajo) = paleta[(r(0) % paleta.len() as u32) as usize];
    for j in 0..h {
        cv.rect(x, y + j, w, 1, mezclar(abajo, arriba, j as u32 * 256 / h.max(1) as u32, 256));
    }
    let (sx, sy) = (x + w * (20 + (r(1) % 60) as i32) / 100, y + h * (20 + (r(2) % 25) as i32) / 100);
    if (sx - x) < w - 8 {
        cv.disc(sx, sy, (h / 9).max(3).min(w / 4), mezclar(BLANCO, abajo, 180, 256));
    }
    // Dos cordilleras: la de detras mas clara.
    for capa in 0..2 {
        let base = y + h * (62 + capa * 14) / 100;
        let c = if capa == 0 { mezclar(arriba, NEGRO, 150, 256) } else { NEGRO };
        let mut alt = 0;
        for i in 0..w {
            let paso = (r(10 + capa as u32 * 100 + (i as u32 / 6)) % 7) as i32 - 3;
            alt = (alt + paso).clamp(-h / 6, h / 6);
            cv.rect(x + i, base - alt, 1, y + h - (base - alt), c);
        }
    }
    // Una estrella fugaz, de vez en cuando.
    let t = (ms + r(3) % 5000) % 5000;
    if t < 400 {
        let px = x + w * t as i32 / 400;
        let py = y + h / 6 + (t as i32 * h / 3000);
        for k in 0..10 {
            cv.blend(px - k, py - k / 3, BLANCO, (10 - k) as u32 * 20, 256);
        }
    }
    cv.frame(x, y, w, h, 1, LINEA);
}

/// **El CANAL**: se enciende como una tele vieja -- una raya, se abre, y
/// las barras de cuando no hay emision.
fn tele(cv: &mut Canvas, v: &Vista, x0: i32) {
    let wc = ancho() - X_CENTRO - 40;
    let tw = wc.min((alto() - CABECERA - 100) * 16 / 9);
    let th = tw * 9 / 16;
    let (tx, ty) = (x0 + 20 + (wc - tw) / 2, CABECERA + 30);
    cv.rect(tx - 10, ty - 10, tw + 20, th + 20, PANEL2);
    cv.rect(tx, ty, tw, th, NEGRO);
    let t = v.ms.wrapping_sub(v.desde_sec);
    let (cx, cy) = (tx + tw / 2, ty + th / 2);
    if t < 220 {
        // La raya que se estira desde el centro.
        let lw = tw * t as i32 / 220;
        cv.rect(cx - lw / 2, cy - 1, lw, 3, BLANCO);
    } else {
        let abre = if t < 480 { th * (t as i32 - 220) / 260 } else { th };
        let (y0, alto_v) = (cy - abre / 2, abre.max(3));
        // Las barras de color, con la nieve encima.
        let colores = [0x00C0_C0C0u32, 0x00C0_C000, 0x0000_C0C0, 0x0000_C000, 0x00C0_00C0, 0x00C0_0000, 0x0000_00C0];
        let bw = tw / colores.len() as i32;
        for (k, &c) in colores.iter().enumerate() {
            let w = if k == colores.len() - 1 { tw - bw * k as i32 } else { bw };
            cv.rect(tx + k as i32 * bw, y0, w, alto_v, c);
        }
        let tic = v.ms / 40;
        for k in 0..(tw * alto_v / 60) {
            let r = azar(k as u32 ^ (tic << 12));
            let (px, py) = (tx + (r % tw as u32) as i32, y0 + ((r >> 12) % alto_v.max(1) as u32) as i32);
            cv.rect(px, py, 2, 1, if r >> 30 == 0 { BLANCO } else { NEGRO });
        }
        // La raya de barrido que baja.
        let ry = y0 + (v.ms / 8 % alto_v.max(1) as u32) as i32;
        cv.rect(tx, ry, tw, 2, mezclar(BLANCO, NEGRO, 60, 256));
        if t > 480 && v.ms / 700 % 2 == 0 {
            let msg = b"SIN EMISION";
            let mw = msg.len() as i32 * 24;
            cv.rect(cx - mw / 2 - 12, cy - 30, mw + 24, 60, NEGRO);
            cv.text(cx - mw / 2, cy - 24, msg, BLANCO, 3);
        }
    }
    // El piloto rojo de la tele.
    cv.disc(tx + tw, ty + th + 4, 3, if t < 220 || v.ms / 900 % 2 == 0 { ROJO } else { mezclar(ROJO, NEGRO, 100, 256) });
}

/// El color de una pieza: el suyo en el disco, en la ficha y en las barras.
fn color_pieza(k: usize) -> Color {
    if PIEZAS[k % PIEZAS.len()].estilo == Estilo::NekoPhonk {
        return ROSA;
    }
    [CIAN, LIMA, AZUL, MORADO, AMBAR, VERDE, CIAN, LIMA, AZUL, MORADO][k % 10]
}

/// Lo que tarda una vuelta del bucle, en ms: ocho compases de cuatro pulsos
/// (`bmo_fondo::compositor::PASOS` son 128 semicorcheas).
fn vuelta_ms(bpm: u32) -> u32 {
    32 * 60_000 / bpm.max(1)
}

/// `m:ss` en `b`; devuelve cuantos bytes.
fn reloj(ms: u32, b: &mut [u8; 8]) -> usize {
    let s = ms / 1000;
    let mut n = crate::fmt_num((s / 60) as u64, &mut b[..]);
    b[n] = b':';
    b[n + 1] = b'0' + (s % 60 / 10) as u8;
    b[n + 2] = b'0' + (s % 10) as u8;
    n += 3;
    n
}

/// Los picos del ecualizador, que caen despacio: el unico estado de la ONDA
/// entre fotogramas (la app es un solo hilo).
static PICOS: [core::sync::atomic::AtomicI32; 48] = [const { core::sync::atomic::AtomicI32::new(0) }; 48];

/// **La ONDA**: el DISCO de la pieza que gira mientras suena, su ficha, la
/// vuelta del bucle y un ecualizador que sigue al medidor del MAESTRO.
fn la_onda(cv: &mut Canvas, v: &Vista, x0: i32) {
    let w = ancho() - X_CENTRO - 40;
    let k = v.pedida.unwrap_or(v.item) % PIEZAS.len();
    let p = &PIEZAS[k];
    let suena = v.pedida == Some(k) && v.sonando;
    let color = color_pieza(k);
    let entra = llega(v.ms, v.desde_item, 0, 450);
    let y = CABECERA + 24;

    // ** EL DISCO: surcos, el brillo que gira, la etiqueta y el agujero. Gira
    // a 33 vueltas por minuto mientras suena; parado, se queda donde estaba.
    const R: i32 = 96;
    let (cx, cy) = (x0 + 20 + R + 8, y + R + 12);
    let giro = if suena { fase(v.ms.wrapping_sub(v.pedida_desde), 1818) } else { 0 };
    cv.disc(cx + 4, cy + 6, R, mezclar(NEGRO, FONDO, 140, 256));
    cv.disc(cx, cy, R, 0x0007_080C);
    let mut r = R - 4;
    while r > 34 {
        let pasos = (r * 3).max(24);
        for q in 0..pasos {
            let a = q * 256 / pasos;
            // El brillo: dos lobulos opuestos que giran con el disco.
            let rel = (a - giro).rem_euclid(128);
            let luz = if rel < 22 { (22 - rel) * 7 } else { 0 } as u32;
            let base = if r % 8 == 0 { 0x0028_2E3C } else if r % 4 == 0 { 0x0018_1C26 } else { 0x0010_1219 };
            let c = mezclar(mezclar(color, BLANCO, 110, 256), base, luz.min(140), 256);
            cv.put(cx + coseno(a) * r / 256, cy + seno(a) * r / 256, c);
        }
        r -= 2;
    }
    cv.disc(cx, cy, 34, color);
    cv.disc(cx, cy, 30, mezclar(color, NEGRO, 60, 256));
    // Una marca en la etiqueta, para que se VEA girar.
    let (mx, my) = (cx + coseno(giro) * 22 / 256, cy + seno(giro) * 22 / 256);
    cv.disc(mx, my, 3, BLANCO);
    cv.disc(cx, cy, 4, FONDO);
    // El brazo: posado en el surco si suena, levantado si no.
    let (bx, by) = (cx + R + 22, cy - R + 6);
    let (px, py) = if suena { (cx + R / 2, cy + 8) } else { (cx + R + 6, cy + 30) };
    cv.disc(bx, by, 7, PANEL2);
    cv.disc(bx, by, 3, GRIS);
    for g in 0..3 {
        cv.line((bx + g - 1, by), (px + g - 1, py), GRIS);
    }
    cv.rect(px - 5, py - 3, 10, 7, if suena { color } else { TENUE });

    // ** LA FICHA, a la derecha del disco.
    let tx = cx + R + 48;
    let tw = (x0 + 20 + w - tx).max(80);
    let ty = y + 8 + (256 - entra) * 16 / 256;
    let etiqueta: &[u8] = if suena { b"AHORA SUENA" } else if v.pedida == Some(k) { b"PEDIDA" } else { b"ELIGE Y SUENA" };
    cv.text(tx, ty, etiqueta, if suena { color } else { TENUE }, 1);
    let escala = if (p.nombre.len() as i32) * 24 <= tw { 3 } else { 2 };
    cv.text(tx, ty + 22, p.nombre.as_bytes(), mezclar(BLANCO, FONDO, entra as u32, 256), escala);
    // Las fichas chicas: pulsos, timbre, escala, estilo.
    let mut d = [0u8; 12];
    let nd = crate::fmt_num(p.bpm as u64, &mut d);
    let timbre: &[u8] = match p.timbre {
        bmo_fondo::Timbre::Seno => b"seno",
        bmo_fondo::Timbre::Cuadrada => b"cuadrada 8 bits",
        bmo_fondo::Timbre::Sierra => b"sierra",
        bmo_fondo::Timbre::Triangulo => b"triangulo",
    };
    let modo: &[u8] = match p.escala {
        bmo_fondo::Escala::Frigia => b"frigia",
        bmo_fondo::Escala::Menor => b"menor",
        bmo_fondo::Escala::Mayor => b"mayor",
        bmo_fondo::Escala::Penta => b"pentatonica",
        bmo_fondo::Escala::Dorica => b"dorica",
    };
    let estilo: &[u8] = if p.estilo == Estilo::NekoPhonk { b"NEKO PHONK" } else { b"ambiente" };
    let mut fx = tx;
    let fy = ty + 22 + escala * 16 + 14;
    for (kk, ficha) in [&d[..nd], timbre, modo, estilo].iter().enumerate() {
        let fw = ficha.len() as i32 * 8 + 16 + if kk == 0 { 7 * 8 } else { 0 };
        if fx + fw > tx + tw {
            break;
        }
        cv.rect(fx, fy, fw, 22, mezclar(color, FONDO, 36, 256));
        cv.frame(fx, fy, fw, 22, 1, mezclar(color, FONDO, 120, 256));
        let fin = cv.text(fx + 8, fy + 3, ficha, TEXTO, 1);
        if kk == 0 {
            cv.text(fx + 8 + fin, fy + 3, b" pulsos", TENUE, 1);
        }
        fx += fw + 8;
    }
    // La vuelta del bucle: por donde va y cuanto mide (la musica de fondo
    // es un bucle SIN COSTURA: al acabar sigue, no se para).
    let total = vuelta_ms(p.bpm);
    let va = if suena { v.ms.wrapping_sub(v.pedida_desde) % total } else { 0 };
    let (ly, lw) = (fy + 40, tw.min(520));
    cv.rect(tx, ly, lw, 4, LINEA);
    cv.rect(tx, ly, (lw as u64 * va as u64 / total as u64) as i32, 4, color);
    if suena {
        let px = tx + (lw as u64 * va as u64 / total as u64) as i32;
        cv.disc(px, ly + 1, 5, BLANCO);
    }
    let mut b1 = [0u8; 8];
    let mut b2 = [0u8; 8];
    let (n1, n2) = (reloj(va, &mut b1), reloj(total, &mut b2));
    cv.text(tx, ly + 12, &b1[..n1], TEXTO, 1);
    cv.text(tx + lw - n2 as i32 * 8, ly + 12, &b2[..n2], TENUE, 1);
    cv.text(tx + n1 as i32 * 8 + 16, ly + 12, b"en bucle, sin costura", TENUE, 1);
    let estado: &[u8] = if suena {
        b"suena en el ESCRITORIO: sigue con HERMES cerrado, y la PASTILLA de arriba la manda"
    } else if v.pedida == Some(k) {
        b"pedida al escritorio: si no suena, `fondo` en Ejecutar dice por que"
    } else {
        b"un clic en la lista y suena: la compone y la toca el escritorio"
    };
    cv.text_fit(tx, ly + 34, estado, if suena { mezclar(color, TEXTO, 120, 256) } else { TENUE }, tw);

    // ** EL ECUALIZADOR: 48 barras finas en degradado. La fuerza es la del
    // medidor del MAESTRO (lo que sale por el cable), con una curva de
    // espectro (los graves arriba) y un baile propio de cada barra; los picos
    // caen despacio, y el suelo refleja.
    let ey = cy + R + 40;
    let eh = (alto() - ey - 70).max(60);
    for q in 1..4 {
        cv.rect(x0 + 20, ey + eh * q / 4, w, 1, mezclar(LINEA, FONDO, 140, 256));
    }
    let barras = 48;
    let paso = (w / barras).max(4);
    let ancho_b = (paso - 3).max(2);
    let crece = llega(v.ms, v.desde_sec, 0, 600);
    for b in 0..barras {
        let lado = (b % 2) as usize;
        // Los graves (a la izquierda) suben mas que los agudos.
        let curva = 256 - b * 120 / barras;
        let fuerza = if v.sonando { nivel(v.rms[lado]) } else { 0 };
        let baila = if v.sonando { onda(v.ms + b as u32 * 71, 210 + (b as u32 % 9) * 37) } else { 0 };
        let a = ((fuerza * curva / 256 * 3 / 4 + baila * fuerza / 256 / 3) * eh / 256 * crece / 256).clamp(2, eh * 9 / 10);
        let bx = x0 + 20 + b * paso;
        let c = mezclar(color, CIAN, b as u32 * 256 / barras as u32, 256);
        // El degradado: cuatro tramos, mas oscuros abajo.
        for t in 0..4 {
            let (y0, y1) = (ey + eh - a * (t + 1) / 4, ey + eh - a * t / 4);
            cv.rect(bx, y0, ancho_b, y1 - y0, mezclar(c, FONDO, 256 - (3 - t as u32) * 45, 256));
        }
        cv.rect(bx, ey + eh - a, ancho_b, 2, mezclar(BLANCO, c, 110, 256));
        // El reflejo, que se apaga.
        for t in 0..3 {
            let h = (a / 10).max(1);
            cv.rect(bx, ey + eh + 3 + t * h, ancho_b, h, mezclar(c, FONDO, [60, 30, 12][t as usize], 256));
        }
        // El pico, que sube de golpe y cae despacio.
        let pk = &PICOS[b as usize];
        let antes = pk.load(core::sync::atomic::Ordering::Relaxed);
        // El pico sale del medidor (la punta, no la fuerza): mas alto que la barra.
        let punta = if v.sonando { (nivel(v.pico[lado]) * curva / 256 * 3 / 4 * eh / 256 * crece / 256).min(eh * 9 / 10) } else { 0 };
        let sube = a.max(punta);
        let ahora = if sube > antes { sube } else { (antes - 2).max(0) };
        pk.store(ahora, core::sync::atomic::Ordering::Relaxed);
        if v.sonando && ahora > a + 3 {
            cv.rect(bx, ey + eh - ahora, ancho_b, 2, mezclar(BLANCO, c, 160, 256));
        }
    }
}

/// **Las paginas**: el inicio, con su buscador que se escribe solo.
fn paginas(cv: &mut Canvas, v: &Vista, x0: i32) {
    let w = ancho() - X_CENTRO - 40;
    let y = CABECERA + 60;
    let titulo = b"hermes";
    let tw = titulo.len() as i32 * 8 * 4;
    cv.text(x0 + 20 + (w - tw) / 2, y, titulo, AMBAR, 4);
    let (bx, bw) = (x0 + 20 + w / 8, w * 3 / 4);
    cv.rect(bx, y + 80, bw, 40, PANEL2);
    cv.frame(bx, y + 80, bw, 40, 2, mezclar(AMBAR, FONDO, 140, 256));
    let dir = b"hermes://tu/inicio";
    let n = ((v.ms.wrapping_sub(v.desde_sec) / 60) as usize).min(dir.len());
    let fin = cv.text(bx + 14, y + 92, &dir[..n], BLANCO, 1);
    if v.ms / 500 % 2 == 0 {
        cv.rect(bx + 15 + fin, y + 90, 2, 20, AMBAR);
    }
    for (k, &(s, nombre)) in ATAJOS.iter().enumerate() {
        let (ax, ay, aw, ah) = caja_atajo(k);
        let ax = ax + x0 - X_CENTRO;
        let t = llega(v.ms, v.desde_sec, 300 + k as u32 * 120, 300);
        let ay = ay + (256 - t) * 30 / 256;
        let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (ax, ay, aw, ah)));
        let c = SECCIONES[s].1;
        cv.rect(ax, ay, aw, ah, if encima { PANEL2 } else { PANEL });
        cv.frame(ax, ay, aw, ah, 1, mezclar(c, FONDO, t as u32, 256));
        cv.text(ax + 14, ay + 16, nombre, mezclar(c, FONDO, t as u32, 256), 1);
        cv.text(ax + 14, ay + 40, b"atajo: un clic", TENUE, 1);
    }
    cv.text_fit(x0 + 20, alto() - 30, b"el buscador buscara en TU maquina; las paginas de tus amigos llegan con H6 y H13-H15", TENUE, w);
}

/// **ENVIOS**: los cinco pasos de algo que llega, encendiendose en orden.
fn envios(cv: &mut Canvas, v: &Vista, x0: i32) {
    const PASOS: [(&[u8], &[u8]); 5] = [
        (b"OFERTA", b"un amigo ofrece"),
        (b"PERMISO", b"tu dices si"),
        (b"CUARENTENA", b"llega aparte"),
        (b"EL JUEZ", b"lo mira por dentro"),
        (b"TU CARPETA", b"solo entonces"),
    ];
    let w = ancho() - X_CENTRO - 40;
    let pw = (w - 4 * 24) / 5;
    let y = CABECERA + 80;
    let t = v.ms.wrapping_sub(v.desde_sec);
    for (k, &(nombre, que)) in PASOS.iter().enumerate() {
        let x = x0 + 20 + k as i32 * (pw + 24);
        let encendido = t > 200 + k as u32 * 350;
        let c = if encendido { VERDE } else { LINEA };
        cv.rect(x, y, pw, 110, if encendido { mezclar(VERDE, FONDO, 30, 256) } else { PANEL });
        cv.frame(x, y, pw, 110, 2, c);
        if encendido {
            let destello = 256u32.saturating_sub((t - 200 - k as u32 * 350) * 256 / 300);
            if destello > 0 {
                cv.glow(x, y, pw, 110, VERDE, 6, destello * 60 / 256);
            }
        }
        let mut d = [0u8; 2];
        d[0] = b'1' + k as u8;
        cv.text(x + 12, y + 12, &d[..1], c, 2);
        cv.text_fit(x + 12, y + 52, nombre, if encendido { BLANCO } else { TENUE }, pw - 20);
        cv.text_fit(x + 12, y + 76, que, TENUE, pw - 20);
        if k < 4 {
            let ax = x + pw + 4;
            cv.rect(ax, y + 54, 14, 2, c);
            for j in 0..4 {
                cv.rect(ax + 14 + j, y + 51 + j, 1, 8 - 2 * j, c);
            }
        }
    }
    // Ya encendidos, un paquete los recorre una y otra vez.
    if t > 200 + 5 * 350 {
        let total = 5 * (pw + 24);
        let px = x0 + 20 + ((v.ms / 4) as i32 % total);
        cv.disc(px, y + 128, 5, VERDE);
        cv.rect(x0 + 20, y + 128, w, 1, mezclar(VERDE, FONDO, 60, 256));
    }
    let lineas: [&[u8]; 3] = [
        b"asi llegara algo de un amigo. Nada se abre solo: cada paso se ve, y el JUEZ es otro",
        b"proceso, en su jaula, que mira los bytes por dentro antes de que toquen tu carpeta.",
        b"Hoy no llega nada: hace falta la PUERTA (H4) y un amigo de verdad (H6). Es H8.",
    ];
    for (k, l) in lineas.iter().enumerate() {
        cv.text_fit(x0 + 20, y + 170 + k as i32 * 22, l, TEXTO, w);
    }
}

/// **Amigos**: tu huella, que se escribe; y como se acepta a alguien.
fn amigos(cv: &mut Canvas, v: &Vista, x0: i32) {
    let w = ancho() - X_CENTRO - 40;
    let y = CABECERA + 40;
    cv.text(x0 + 20, y, b"tu huella", ROSA, 2);
    // Dieciseis grupos de cuatro: los de una huella de verdad. Sin clave
    // todavia, se escriben puntos -- no una huella inventada.
    let n = (v.ms.wrapping_sub(v.desde_sec) / 25) as usize;
    let gw = 5 * 8 + 12;
    for g in 0..16usize {
        let (fila, col) = (g / 8, g % 8);
        let (gx, gy) = (x0 + 20 + col as i32 * gw, y + 50 + fila as i32 * 30);
        let hecho = n.saturating_sub(g * 4).min(4);
        let mut b = [b' '; 4];
        for c in b.iter_mut().take(hecho) {
            *c = b'.';
        }
        cv.rect(gx - 4, gy - 4, gw - 8, 24, PANEL2);
        cv.text(gx, gy, &b, ROSA, 1);
    }
    let t = llega(v.ms, v.desde_sec, 1700, 400) as u32;
    let lineas: [&[u8]; 5] = [
        b"aun sin clave: se crea con H6 (X25519, una vez, y no sale de esta maquina)",
        b"un amigo es su clave aceptada UNA vez: os leeis las huellas en voz alta, o en",
        b"persona, y si coinciden ya nadie se puede poner en medio. Sin cuentas, sin",
        b"servidor, sin numero de telefono. HERMES no es la ANTENA: no comparten nada.",
        b"",
    ];
    for (k, l) in lineas.iter().enumerate() {
        cv.text_fit(x0 + 20, y + 130 + k as i32 * 22, l, mezclar(TEXTO, FONDO, t, 256), w);
    }
    // Los dos que se aceptan: dos gatos que se dan la mano en el centro.
    let (cx, cy) = (x0 + 20 + w / 2, y + 300);
    let a = fase(v.ms, 3000);
    let d = 60 + seno(a) * 10 / 256;
    cv.disc(cx - d, cy, 18, mezclar(ROSA, FONDO, 120, 256));
    cv.disc(cx + d, cy, 18, mezclar(CIAN, FONDO, 120, 256));
    cv.text(cx - d - 8, cy - 8, b"tu", BLANCO, 1);
    cv.text(cx + d - 4, cy - 8, b"?", BLANCO, 1);
    for k in 0..((2 * d - 40) / 8) {
        if (k + (v.ms / 120) as i32) % 3 != 0 {
            cv.rect(cx - d + 20 + k * 8, cy, 4, 2, TENUE);
        }
    }
}

/// **Las jaulas**: tres que bajan con rebote, y lo que hay en cada una.
fn jaulas(cv: &mut Canvas, v: &Vista, x0: i32) {
    const TRES: [(&[u8], &[u8], &[u8], Color, bool); 3] = [
        (b"LA PUERTA", b"la red y tu clave", b"H4: aun no existe", AMBAR, false),
        (b"LA APP", b"esta ventana", b"corre ahora", CIAN, true),
        (b"EL JUEZ", b"lo que llega", b"H8: aun no existe", ROSA, false),
    ];
    let w = ancho() - X_CENTRO - 40;
    let jw = (w - 2 * 30) / 3;
    let jh = 200;
    let y_fin = CABECERA + 70;
    for (k, &(nombre, que, estado, c, vive)) in TRES.iter().enumerate() {
        let x = x0 + 20 + k as i32 * (jw + 30);
        // La caida, con un rebote al tocar suelo.
        let t = llega(v.ms, v.desde_sec, k as u32 * 150, 450);
        let rebote = if t >= 256 { 0 } else { seno(t / 2) * 16 / 256 };
        let y = entre(-jh - 20, y_fin, t) - rebote;
        cv.rect(x, y, jw, jh, PANEL);
        for b in 0..7 {
            cv.rect(x + 8 + b * (jw - 16) / 6, y, 3, jh, mezclar(c, PANEL, 90, 256));
        }
        cv.rect(x, y, jw, 4, c);
        cv.rect(x, y + jh - 4, jw, 4, c);
        cv.rect(x + 14, y + 40, jw - 28, 100, PANEL2);
        cv.text_fit(x + 24, y + 54, nombre, c, jw - 40);
        cv.text_fit(x + 24, y + 78, que, TEXTO, jw - 40);
        let luz = if vive { if v.ms / 600 % 2 == 0 { VERDE } else { mezclar(VERDE, PANEL2, 120, 256) } } else { TENUE };
        cv.disc(x + 30, y + 112, 5, luz);
        cv.text_fit(x + 42, y + 105, estado, if vive { VERDE } else { TENUE }, jw - 60);
    }
    let y = y_fin + jh + 40;
    let lineas: [&[u8]; 3] = [
        b"la PUERTA es la unica con red; la APP no tiene ninguna autoridad; el JUEZ solo",
        b"lee lo que llega. Se hablan por una cola (`bmo-cola`) y ninguno ve la memoria ni",
        b"los ficheros del otro: si uno cae o lo atacan, los otros dos siguen enteros.",
    ];
    for (k, l) in lineas.iter().enumerate() {
        cv.text_fit(x0 + 20, y + k as i32 * 22, l, TEXTO, w);
    }
}

/// El medidor del maestro: pico y fuerza de los dos lados, en 1/256 dB.
pub fn medidor() -> ([i32; 2], [i32; 2]) {
    let m = bmo::info(bmo::INFO_AUDIO_MEDIDOR);
    let d = |s: u32| ((m >> s) & 0xFFFF) as u16 as i16 as i32;
    ([d(0), d(16)], [d(32), d(48)])
}
