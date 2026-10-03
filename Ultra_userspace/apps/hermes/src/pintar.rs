//! **LA VENTANA DE HERMES** -- la cara de la maqueta (H1,
//! `docs/arte/maqueta_hermes.html`), de izquierda a derecha:
//!
//! ```text
//!    barra    HERMES, su programa, y la X que cierra
//!    riel     las alas arriba; una burbuja REDONDA por seccion, cada una
//!             con su aro de color y SU gesto vivo; la elegida se vuelve una
//!             caja redonda que brilla, con su pastilla blanca al borde
//!    lista    el nombre en negrita, su rotulo (CONVERSACIONES, TU
//!             BIBLIOTECA...) y filas de DOS lineas con su cara redonda; abajo
//!             quien eres: el gato de BMO-X
//!    centro   la cabecera (cara, nombre, de que va y su pildora de estado,
//!             con la raya de color debajo) y la seccion, que ENTRA a su
//!             manera: los mensajes desde su lado, el MURO voltea, el CANAL
//!             se enciende como una tele, los pasos de ENVIOS en orden...
//!    panel    las TARJETAS (`panel.rs`): que es la seccion, la conexion,
//!             las tres jaulas, lo que suena. Si la ventana es estrecha, se
//!             esconde y el centro se queda con todo
//! ```
//!
//! *** Nada inventado: lo que aun no existe (la red, los amigos, las fotos,
//! el video) se dice con su escalon del plan, y lo que existe es de verdad:
//! lo escrito se guarda, la ONDA toca en el escritorio y sus barras son el
//! medidor del maestro.

use crate::canvas::Canvas;
use crate::charla::Mensaje;
use crate::mates::{azar, coseno, entre, fase, seno};
use crate::piezas::{caja, cara, negrita, negrita_fit, pildora, punto, redonda, rotulo};
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

pub(crate) fn ancho() -> i32 {
    (MEDIDA.load(core::sync::atomic::Ordering::Relaxed) >> 16) as i32
}

pub(crate) fn alto() -> i32 {
    (MEDIDA.load(core::sync::atomic::Ordering::Relaxed) & 0xFFFF) as i32
}

/// Donde acaba todo lo de encima del REPRODUCTOR (la barra de abajo).
pub(crate) fn suelo() -> i32 {
    alto() - REPRO
}

/// El panel de las tarjetas, a la derecha: solo si queda un centro que se lea.
pub(crate) fn ancho_panel() -> i32 {
    if ancho() >= 1180 { 268 } else { 0 }
}

/// Lo que mide el centro, entre la lista y el panel.
pub(crate) fn ancho_centro() -> i32 {
    ancho() - X_CENTRO - ancho_panel()
}

/// El REPRODUCTOR de abajo, a lo ancho, como el de la maqueta.
pub(crate) const REPRO: i32 = 66;
/// La barra de arriba (HERMES, su programa y la X).
pub(crate) const BARRA: i32 = 30;
const RIEL: i32 = 64;
const LISTA: i32 = 240;
pub(crate) const X_CENTRO: i32 = RIEL + LISTA;
/// Donde acaban las cabeceras de la lista y del centro.
pub(crate) const CABECERA: i32 = BARRA + 56;
const BURBUJA: i32 = 42;
const PASO_RIEL: i32 = 50;
/// Las alas y su raya van encima de las burbujas.
const Y_RIEL: i32 = BARRA + 50;
const FILA: i32 = 52;
/// Las filas empiezan debajo del rotulo.
const Y_FILAS: i32 = CABECERA + 38;
const CAJA_ALTO: i32 = 50;
/// Quien eres, abajo de la lista.
const PIE: i32 = 66;

// La paleta de la maqueta (`maqueta_hermes.html`).
pub(crate) const NEGRO: Color = 0x0005_060A;
pub(crate) const FONDO: Color = 0x0008_0A10;
pub(crate) const PANEL: Color = 0x000B_0E15;
pub(crate) const PANEL2: Color = 0x0010_141D;
pub(crate) const LINEA: Color = 0x001B_2130;
pub(crate) const TEXTO: Color = 0x00C9_D3DB;
pub(crate) const TENUE: Color = 0x006E_7A89;
pub(crate) const GRIS: Color = 0x009A_A2B4;
pub(crate) const BLANCO: Color = 0x00F2_F7F9;
pub(crate) const CIAN: Color = 0x005E_F2E6;
pub(crate) const ROSA: Color = 0x00FF_2E88;
pub(crate) const AZUL: Color = 0x003D_A5FF;
pub(crate) const AMBAR: Color = 0x00FF_C24D;
pub(crate) const VERDE: Color = 0x004D_E38F;
pub(crate) const ROJO: Color = 0x00FF_4F5E;
pub(crate) const LIMA: Color = 0x00B6_FF5C;
pub(crate) const MORADO: Color = 0x00A5_5DFF;

/// **Las secciones**, en el orden del riel de la maqueta: nombre, color, y lo
/// que es (va en la tarjeta QUE ES del panel).
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

/// De que va cada seccion, en corto: al lado del nombre en la cabecera.
const CORTO: [&[u8]; 9] = [
    b"en esta maquina",
    b"un canal por tema",
    b"tus fotos",
    b"tu video",
    b"toda tu musica y todos los sonidos",
    b"sin scripts, sin Google",
    b"nada entra sin tu permiso",
    b"por huella, sin servidor",
    b"tres procesos aparte",
];

/// El rotulo de la lista de cada seccion.
const ROTULOS: [&[u8]; 9] =
    [b"conversaciones", b"canales", b"tus fotos", b"emisiones", b"tu biblioteca", b"paginas", b"envios", b"amigos", b"procesos"];

pub const MENSAJES: usize = 0;
pub const TERTULIAS: usize = 1;
pub const MURO: usize = 2;
pub const CANAL: usize = 3;
pub const ONDA: usize = 4;
pub const PAGINAS: usize = 5;
pub const ENVIOS: usize = 6;
pub const AMIGOS: usize = 7;
pub const JAULAS: usize = 8;

/// Cuantas piezas tiene la ONDA.
pub const PIEZAS_N: usize = PIEZAS.len();

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
    /// El REPRODUCTOR: si se pidio la pausa, y el volumen pedido (0..=100).
    pub pausada: bool,
    pub volumen: u32,
}

/// Donde cayo un clic.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Golpe {
    Seccion(usize),
    Item(usize),
    Escribir,
    Zumbido,
    /// La X de la barra.
    Cerrar,
    /// Un mando del REPRODUCTOR de abajo.
    Repro(crate::reproductor::Mando),
    /// La carita o el gato de la caja de escribir: lo que ponen.
    Poner(&'static [u8]),
}

pub(crate) fn dentro(x: i32, y: i32, (bx, by, bw, bh): (i32, i32, i32, i32)) -> bool {
    x >= bx && y >= by && x < bx + bw && y < by + bh
}

fn caja_cerrar() -> (i32, i32, i32, i32) {
    (ancho() - 38, 3, 32, BARRA - 6)
}

fn caja_burbuja(i: usize) -> (i32, i32, i32, i32) {
    ((RIEL - BURBUJA) / 2, Y_RIEL + i as i32 * PASO_RIEL, BURBUJA, BURBUJA)
}

/// Las filas de la lista que caben, y desde cual se empieza (la elegida
/// siempre se ve).
fn filas_visibles(sec: usize, item: usize) -> (usize, usize) {
    let caben = ((suelo() - Y_FILAS - PIE - 8) / FILA).max(1) as usize;
    let n = cuantos_items(sec);
    let desde = if item >= caben { item + 1 - caben } else { 0 };
    (desde, caben.min(n - desde.min(n)))
}

fn caja_fila(j: usize) -> (i32, i32, i32, i32) {
    (RIEL + 8, Y_FILAS + j as i32 * FILA, LISTA - 16, FILA - 4)
}

fn caja_escribir() -> (i32, i32, i32, i32) {
    (X_CENTRO + 20, suelo() - CAJA_ALTO - 16, ancho_centro() - 40, CAJA_ALTO)
}

/// El ZUMBIDO va DENTRO de la caja de escribir, a la derecha, como en la
/// maqueta.
fn caja_zumbido() -> (i32, i32, i32, i32) {
    let (ex, ey, ew, eh) = caja_escribir();
    (ex + ew - 8 - 96, ey + 9, 96, eh - 18)
}

/// La carita (0) y el gato (1) de la caja de escribir, a la izquierda del
/// ZUMBIDO (o del borde, en una tertulia).
fn caja_icono(sec: usize, k: i32) -> (i32, i32, i32, i32) {
    let (ex, ey, ew, eh) = caja_escribir();
    let der = if sec == MENSAJES { caja_zumbido().0 - 8 } else { ex + ew - 10 };
    (der - 28 * (2 - k), ey + (eh - 26) / 2, 26, 26)
}

/// Lo que pone cada icono de la caja.
const PONE: [&[u8]; 2] = [b" :)", b" nya"];

fn caja_atajo(k: usize) -> (i32, i32, i32, i32) {
    let w = (ancho_centro() - 40 - 2 * 16) / 3;
    (X_CENTRO + 20 + k as i32 * (w + 16), CABECERA + 210, w, 70)
}

/// Los atajos de la pagina de inicio: a donde llevan.
const ATAJOS: [(usize, &[u8]); 3] = [(MENSAJES, b"tus notas"), (ONDA, b"la ONDA"), (TERTULIAS, b"#general")];

/// **Que hay debajo de un clic.**
pub fn golpe(x: i32, y: i32, sec: usize, item: usize) -> Option<Golpe> {
    if dentro(x, y, caja_cerrar()) {
        return Some(Golpe::Cerrar);
    }
    if let Some(m) = crate::reproductor::golpe(x, y) {
        return Some(Golpe::Repro(m));
    }
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
    // El ZUMBIDO y los iconos antes que la caja: estan dentro de ella.
    if canal_de(sec, item).is_some() {
        for k in 0..2 {
            if dentro(x, y, caja_icono(sec, k)) {
                return Some(Golpe::Poner(PONE[k as usize]));
            }
        }
    }
    if sec == MENSAJES && dentro(x, y, caja_zumbido()) {
        return Some(Golpe::Zumbido);
    }
    if canal_de(sec, item).is_some() && dentro(x, y, caja_escribir()) {
        return Some(Golpe::Escribir);
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
pub(crate) fn llega(ms: u32, desde: u32, retraso: u32, dura: u32) -> i32 {
    let t = ms.wrapping_sub(desde);
    if t <= retraso {
        return 0;
    }
    let u = (((t - retraso) * 256) / dura.max(1)).min(256) as i32;
    // 1 - (1 - u)^2: frena al llegar.
    256 - (256 - u) * (256 - u) / 256
}

/// Un texto partido en lineas de `max` letras como mucho, por los espacios.
pub(crate) fn partir(t: &[u8], max: usize) -> Vec<&[u8]> {
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
pub(crate) fn nivel(db: i32) -> i32 {
    ((db + 60 * 256).clamp(0, 60 * 256) * 256) / (60 * 256)
}

/// **Un fotograma entero.**
pub fn pintar(cv: &mut Canvas, v: &Vista) {
    cv.clear(FONDO);
    barra(cv, v);
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
    if ancho_panel() > 0 {
        crate::panel::panel(cv, v, ancho() - ancho_panel());
    }
    crate::reproductor::pintar(cv, v);
    if let Some(t0) = v.zumbido {
        let t = v.ms.wrapping_sub(t0);
        if t < 700 {
            let a = (700 - t) * 120 / 700;
            let (x, w) = (X_CENTRO, ancho_centro());
            cv.glow(x + 4, BARRA + 4, w - 8, suelo() - BARRA - 8, ROSA, 4, a);
            cv.frame(x, BARRA, w, suelo() - BARRA, 2, mezclar(ROSA, FONDO, a * 2, 256));
        }
    }
}

// ============================== LA BARRA ==============================

/// **La barra de arriba**: como la ventana de la maqueta. HERMES no lleva el
/// marco del escritorio (`SIN_MARCO`), asi que la barra es suya.
fn barra(cv: &mut Canvas, v: &Vista) {
    cv.rect(0, 0, ancho(), BARRA, NEGRO);
    cv.rect(0, BARRA - 1, ancho(), 1, LINEA);
    let fin = negrita(cv, 14, 7, b"HERMES", BLANCO);
    cv.text(14 + fin + 14, 7, b"sys/hermes.bex", TENUE, 1);
    let ayuda: &[u8] = b"Tab secciones  /  escribir  Esc cierra";
    let (cx, cy, cw, ch) = caja_cerrar();
    cv.text(cx - 16 - ayuda.len() as i32 * 8, 7, ayuda, mezclar(TENUE, NEGRO, 170, 256), 1);
    let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (cx, cy, cw, ch)));
    if encima {
        redonda(cv, cx, cy, cw, ch, 6, mezclar(ROJO, NEGRO, 120, 256));
    }
    // La X, de dos rayas gruesas.
    let (mx, my) = (cx + cw / 2, cy + ch / 2);
    for g in 0..2 {
        cv.line((mx - 5 + g, my - 5), (mx + 5 + g, my + 5), if encima { BLANCO } else { GRIS });
        cv.line((mx - 5 + g, my + 5), (mx + 5 + g, my - 5), if encima { BLANCO } else { GRIS });
    }
}

// ============================== EL RIEL ==============================

fn riel(cv: &mut Canvas, v: &Vista) {
    cv.rect(0, BARRA, RIEL, suelo() - BARRA, NEGRO);
    cv.rect(RIEL - 1, BARRA, 1, suelo() - BARRA, LINEA);
    // Las alas de HERMES, y su raya.
    crate::piezas::alas(cv, RIEL / 2, BARRA + 20, CIAN);
    cv.rect(RIEL / 2 - 14, BARRA + 42, 28, 1, LINEA);
    for (i, &(_, color, _)) in SECCIONES.iter().enumerate() {
        let (x, y, w, h) = caja_burbuja(i);
        let elegida = i == v.sec;
        let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (x, y, w, h)));
        let (cx, cy) = (x + w / 2, y + h / 2);
        if elegida {
            // Una caja redonda que brilla, y su pastilla blanca al borde.
            let entra = llega(v.ms, v.desde_sec, 0, 220) as u32;
            cv.glow(x, y, w, h, color, 4, 30 * entra / 256);
            caja(cv, x, y, w, h, 13, mezclar(color, NEGRO, 52, 256), mezclar(color, NEGRO, 200, 256));
            redonda(cv, 0, y + 8, 4, h - 16, 2, BLANCO);
        } else {
            // Redonda, con su aro de color apagado.
            cv.disc(cx, cy, w / 2, mezclar(color, NEGRO, if encima { 150 } else { 60 }, 256));
            cv.disc(cx, cy, w / 2 - 1, if encima { 0x0016_1B26 } else { 0x0010_131B });
            // Un trozo de aro mas vivo que da la vuelta despacio, como los
            // de la maqueta: cada burbuja a su paso.
            let a0 = fase(v.ms, 9000 + i as u32 * 700) + i as i32 * 29;
            crate::piezas::arco(cv, cx, cy, w / 2, a0, a0 + 48, mezclar(color, NEGRO, 190, 256));
            if encima {
                redonda(cv, 0, y + 16, 3, h - 32, 1, TEXTO);
            }
        }
        let bg = if elegida { mezclar(color, NEGRO, 52, 256) } else if encima { 0x0016_1B26 } else { 0x0010_131B };
        crate::iconos::icono(cv, i, cx, cy, v.ms, if elegida || encima { color } else { mezclar(color, NEGRO, 190, 256) }, bg);
    }
}

// ============================== LA LISTA ==============================

/// La segunda linea de una fila: lo ultimo escrito, cuantos hay, o de que va.
fn segunda<'a>(v: &'a Vista, k: usize, d: &'a mut [u8; 24]) -> &'a [u8] {
    match v.sec {
        MENSAJES => v.charla.last().map_or(&b"aun no hay nada escrito"[..], |m| &m.texto[..]),
        TERTULIAS => match v.cuentas.get(k).copied().unwrap_or(0) {
            0 => b"aun vacia",
            n => {
                let nd = crate::fmt_num(n as u64, &mut d[..]);
                d[nd..nd + 9].copy_from_slice(b" mensajes");
                &d[..nd + 9]
            }
        },
        _ => CORTO[v.sec],
    }
}

fn lista(cv: &mut Canvas, v: &Vista) {
    let (nombre, color, _) = SECCIONES[v.sec];
    cv.rect(RIEL, BARRA, LISTA, suelo() - BARRA, PANEL);
    cv.rect(X_CENTRO - 1, BARRA, 1, suelo() - BARRA, LINEA);
    cv.rect(RIEL, CABECERA - 1, LISTA, 1, LINEA);
    // El nombre de la seccion, en negrita, entra escribiendose.
    let n = ((v.ms.wrapping_sub(v.desde_sec) / 35) as usize).min(nombre.len());
    negrita(cv, RIEL + 16, BARRA + 20, &nombre[..n], BLANCO);
    rotulo(cv, RIEL + 16, CABECERA + 14, ROTULOS[v.sec], mezclar(TENUE, PANEL, 200, 256));
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
            redonda(cv, x, y, w, h, 9, mezclar(color, PANEL, 34, 256));
        } else if encima {
            redonda(cv, x, y, w, h, 9, PANEL2);
        }
        let (cx, cy) = (x + 24, y + h / 2);
        let suena = v.sec == ONDA && v.pedida == Some(k) && v.sonando;
        // La cara de la fila.
        match v.sec {
            ONDA => crate::onda::portada(cv, x + 8, y + (h - 34) / 2, 34, k, suena, v.ms),
            TERTULIAS => {
                redonda(cv, x + 7, y + (h - 34) / 2, 34, 34, 10, mezclar(color, PANEL, 60, 256));
                negrita(cv, cx - 4, cy - 8, b"#", color);
            }
            MENSAJES => {
                cara(cv, cx, cy, 17, CIAN, b'N', PANEL);
                punto(cv, cx + 12, cy + 12, VERDE, if elegida { mezclar(color, PANEL, 34, 256) } else { PANEL });
            }
            s => cara(cv, cx, cy, 17, mezclar(color, PANEL, 200, 256), SECCIONES[s].0[0].to_ascii_uppercase(), PANEL),
        }
        let tinta = mezclar(if elegida || suena { BLANCO } else { TEXTO }, PANEL, t as u32, 256);
        // A la derecha, cuantos hay (si hay).
        let mut der = x + w - 8;
        if let Some(&c) = v.cuentas.get(k) {
            if c > 0 && v.sec != TERTULIAS {
                let mut d = [0u8; 8];
                let nd = crate::fmt_num(c as u64, &mut d);
                let bw = nd as i32 * 8 + 12;
                redonda(cv, der - bw, y + 8, bw, 18, 9, mezclar(color, PANEL, 110, 256));
                cv.text(der - bw + 6, y + 9, &d[..nd], BLANCO, 1);
                der -= bw + 6;
            }
        }
        let tx = x + 50;
        let nombre_item = nombre_item(v.sec, k);
        if elegida {
            negrita_fit(cv, tx, y + 7, nombre_item, tinta, der - tx);
        } else {
            cv.text_fit(tx, y + 7, nombre_item, tinta, der - tx);
        }
        // La segunda linea.
        let mut d = [0u8; 24];
        if v.sec == ONDA {
            let p = &PIEZAS[k];
            let nd = crate::fmt_num(p.bpm as u64, &mut d);
            let fin = cv.text(tx, y + 26, &d[..nd], TENUE, 1);
            let estilo: &[u8] = if p.estilo == Estilo::NekoPhonk { b" pulsos - neko" } else { b" pulsos" };
            cv.text_fit(tx + fin, y + 26, estilo, if p.estilo == Estilo::NekoPhonk { mezclar(ROSA, PANEL, 190, 256) } else { TENUE }, x + w - 8 - tx - fin);
        } else {
            let s = segunda(v, k, &mut d);
            cv.text_fit(tx, y + 26, s, TENUE, x + w - 8 - tx);
        }
    }
    // Abajo, quien eres: el gato chico de BMO-X.
    let y0 = suelo() - PIE;
    cv.rect(RIEL, y0, LISTA, PIE, PANEL);
    cv.rect(RIEL, y0, LISTA, 1, LINEA);
    gato_chico(cv, RIEL + 12, y0 + 11, v.ms);
    negrita(cv, RIEL + 60, y0 + 14, b"BMO-X (tu)", BLANCO);
    cv.text(RIEL + 60, y0 + 36, b"sin clave aun: H6", TENUE, 1);
    let luz = if v.ms / 1200 % 2 == 0 { AMBAR } else { mezclar(AMBAR, PANEL, 120, 256) };
    punto(cv, RIEL + 46, y0 + 50, luz, PANEL);
}

/// El gato de BMO-X a un cuarto (38 x 45), que parpadea de vez en cuando.
pub(crate) fn gato_chico(cv: &mut Canvas, x: i32, y: i32, ms: u32) {
    gato(cv, x, y, ms, 4);
}

/// El gato de BMO-X a `1/div`, que parpadea de vez en cuando.
pub(crate) fn gato(cv: &mut Canvas, x: i32, y: i32, ms: u32, div: u32) {
    use crate::gato::{EYES, HEIGHT, STROKE, WIDTH};
    let bit = |m: &[u8], fx: u32, fy: u32| {
        let i = (fy * WIDTH + fx) as usize;
        m[i / 8] >> (i % 8) & 1 == 1
    };
    let parpadea = ms % 4000 < 150;
    for by in 0..HEIGHT / div {
        for bx in 0..WIDTH / div {
            let (mut trazo, mut ojo) = (false, false);
            for d in 0..div * div {
                let (fx, fy) = (bx * div + d % div, by * div + d / div);
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

/// La pildora de estado de la cabecera: lo que la seccion ES hoy.
fn estado(v: &Vista) -> (&'static [u8], Color) {
    match v.sec {
        MENSAJES => (b"solo aqui", CIAN),
        TERTULIAS => (b"sin red: H6", AZUL),
        MURO => (b"H9", MORADO),
        CANAL => (b"SIN EMISION", ROJO),
        ONDA if v.sonando => (b"SUENA", LIMA),
        ONDA => (b"parada", TENUE),
        PAGINAS => (b"sin Google", AMBAR),
        ENVIOS => (b"H8", VERDE),
        AMIGOS => (b"H6", ROSA),
        _ => (b"3 jaulas", GRIS),
    }
}

fn centro(cv: &mut Canvas, v: &Vista, dx: i32) {
    let (_, color, sub) = SECCIONES[v.sec];
    let (x0, w) = (X_CENTRO + dx, ancho_centro());
    // La cabecera: la cara, el nombre en negrita, de que va, y su pildora.
    // En la ONDA el nombre de la pieza ya va grande en la ficha: arriba, la
    // seccion.
    let titulo = if v.sec == ONDA { b"la ONDA".as_slice() } else { nombre_item(v.sec, v.item) };
    let t = llega(v.ms, v.desde_item, 0, 260);
    let cy = BARRA + 28;
    match v.sec {
        TERTULIAS => {
            redonda(cv, x0 + 18, cy - 15, 30, 30, 9, mezclar(color, FONDO, 60, 256));
            negrita(cv, x0 + 29, cy - 8, b"#", color);
        }
        MENSAJES => {
            cara(cv, x0 + 33, cy, 15, CIAN, b'N', FONDO);
            punto(cv, x0 + 44, cy + 11, VERDE, FONDO);
        }
        s => cara(cv, x0 + 33, cy, 15, mezclar(color, FONDO, 200, 256), SECCIONES[s].0[0].to_ascii_uppercase(), FONDO),
    }
    let tx = x0 + 60 + (256 - t) * 16 / 256;
    let fin = negrita(cv, tx, cy - 8, titulo, mezclar(BLANCO, FONDO, t as u32, 256));
    let (texto_p, color_p) = estado(v);
    let px = pildora(cv, x0 + w - 18, cy - 11, texto_p, color_p, FONDO);
    // Sin panel, lo que la seccion es va aqui (el panel lo lleva en su tarjeta).
    let corto = if ancho_panel() > 0 { CORTO[v.sec] } else { sub };
    cv.text_fit(tx + fin + 12, cy - 8, corto, TENUE, px - 16 - (tx + fin + 12));
    cv.rect(X_CENTRO, CABECERA - 1, w, 1, LINEA);
    // La raya de color de la maqueta: nace del nombre y se apaga.
    let rw = w * 2 / 5 * t / 256;
    cv.gradient(X_CENTRO, CABECERA - 2, rw, 2, color, FONDO);
    match v.sec {
        MENSAJES | TERTULIAS => charla(cv, v, x0, color),
        MURO => muro(cv, v, x0),
        CANAL => tele(cv, v, x0),
        ONDA => crate::onda::la_onda(cv, v, x0),
        PAGINAS => paginas(cv, v, x0),
        ENVIOS => envios(cv, v, x0),
        AMIGOS => amigos(cv, v, x0),
        JAULAS => jaulas(cv, v, x0),
        _ => {}
    }
    if !v.aviso.is_empty() {
        cv.text_fit(x0 + 20, suelo() - 14 - 16 - if canal_de(v.sec, v.item).is_some() { CAJA_ALTO + 12 } else { 0 }, v.aviso, AMBAR, w - 40);
    }
}

/// **La charla**, como la de la maqueta: sin bocadillos, la cara y el nombre
/// al empezar cada racha, y el texto debajo; la caja de escribir abajo, con
/// el ZUMBIDO dentro.
fn charla(cv: &mut Canvas, v: &Vista, x0: i32, color: Color) {
    let w = ancho_centro();
    let (ex, ey, ew, eh) = caja_escribir();
    let ex = ex + x0 - X_CENTRO;
    let tx = x0 + 20 + 52;
    let por_linea = ((x0 + w - 24 - tx) / 8).max(10) as usize;
    let tope = CABECERA + 16;
    let mut y = ey - 16;
    if v.charla.is_empty() {
        let t = llega(v.ms, v.desde_item, 100, 400);
        let c = mezclar(TENUE, FONDO, t as u32, 256);
        let mid = (tope + ey) / 2;
        cv.text(x0 + (w - 30 * 8) / 2, mid - 20, b"aun no hay nada escrito aqui", c, 1);
        cv.text(x0 + (w - 46 * 8) / 2, mid + 4, b"lo que escribas se queda en sys/hermsg.txt", c, 1);
    }
    let n = v.charla.len();
    // Del mas nuevo hacia arriba, hasta que no quepan.
    for (j, m) in v.charla.iter().rev().enumerate() {
        let i = n - 1 - j;
        // Una racha empieza si es el primero, o si el de antes es de hace
        // mas de cinco minutos (o no se sabe cuando fue).
        let empieza = i == 0 || m.cuando == 0 || m.cuando.saturating_sub(v.charla[i - 1].cuando) > 300;
        let lineas = partir(&m.texto, por_linea);
        let alto_m = lineas.len() as i32 * 20 + if empieza { 24 } else { 0 };
        y -= alto_m;
        if y < tope {
            break;
        }
        // ** ENTRAN DESDE SU LADO: en tus notas los mensajes son tuyos y
        // llegan por la derecha; en una tertulia suben desde abajo.
        let base = if j == 0 { v.enviado.max(v.desde_item) } else { v.desde_item };
        let t = llega(v.ms, base, if j == 0 { 0 } else { j as u32 * 35 }, 300);
        let (dx, dy) = if v.sec == MENSAJES { ((256 - t) * 120 / 256, 0) } else { (0, (256 - t) * 40 / 256) };
        let a = t as u32;
        if empieza {
            gato(cv, x0 + 20 + dx + 6, y + dy + 2, v.ms, 6);
            let fin = negrita(cv, tx + dx, y + dy + 2, b"BMO-X (tu)", mezclar(BLANCO, FONDO, a, 256));
            if v.sec == TERTULIAS {
                cv.text(tx + dx + fin + 10, y + dy + 2, CANALES[v.item % CANALES.len()], mezclar(color, FONDO, a * 3 / 4, 256), 1);
            }
        }
        let y_t = y + dy + if empieza { 24 } else { 0 };
        for (k, l) in lineas.iter().enumerate() {
            cv.text(tx + dx, y_t + k as i32 * 20, l, mezclar(TEXTO, FONDO, a, 256), 1);
        }
        y -= 10;
    }
    // La caja de escribir: redonda, con su + a la izquierda.
    let borde = if v.escribiendo { mezclar(color, FONDO, 200, 256) } else { LINEA };
    caja(cv, ex, ey, ew, eh, 12, if v.escribiendo { PANEL2 } else { PANEL }, borde);
    negrita(cv, ex + 16, ey + 17, b"+", TENUE);
    let tope_x = caja_icono(v.sec, 0).0 + x0 - X_CENTRO - 8;
    // La carita y el gato: ponen " :)" y " nya" en lo que se escribe.
    for k in 0..2 {
        let (ix, iy, iw, ih) = caja_icono(v.sec, k);
        let ix = ix + x0 - X_CENTRO;
        let encima = v.puntero.map_or(false, |(px, py)| dentro(px, py, (ix, iy, iw, ih)));
        let c = if encima { BLANCO } else { GRIS };
        let (cx, cy) = (ix + iw / 2, iy + ih / 2);
        crate::piezas::arco(cv, cx, cy, 9, 0, 256, c);
        if k == 0 {
            cv.rect(cx - 4, cy - 3, 2, 2, c);
            cv.rect(cx + 3, cy - 3, 2, 2, c);
            crate::piezas::arco(cv, cx, cy, 5, 20, 108, c);
        } else {
            // Las orejas y los ojos del gato.
            crate::piezas::trazo(cv, (cx - 8, cy - 4), (cx - 6, cy - 11), c);
            crate::piezas::trazo(cv, (cx - 6, cy - 11), (cx - 2, cy - 8), c);
            crate::piezas::trazo(cv, (cx + 7, cy - 4), (cx + 5, cy - 11), c);
            crate::piezas::trazo(cv, (cx + 5, cy - 11), (cx + 1, cy - 8), c);
            cv.rect(cx - 4, cy - 1, 2, 3, c);
            cv.rect(cx + 3, cy - 1, 2, 3, c);
            cv.rect(cx, cy + 3, 1, 1, c);
        }
    }
    let tx = ex + 40;
    if v.escribiendo || !v.borrador.is_empty() {
        let cabe = ((tope_x - tx - 6) / 8).max(1) as usize;
        let ver = &v.borrador[v.borrador.len().saturating_sub(cabe)..];
        let fin = cv.text(tx, ey + 17, ver, BLANCO, 1);
        if v.escribiendo && v.ms / 500 % 2 == 0 {
            cv.rect(tx + fin + 1, ey + 15, 2, 20, color);
        }
    } else {
        let fin = if v.sec == MENSAJES {
            cv.text(tx, ey + 17, b"Escribe una nota", TENUE, 1)
        } else {
            let f = cv.text(tx, ey + 17, b"Escribe en ", TENUE, 1);
            f + cv.text(tx + f, ey + 17, CANALES[v.item % CANALES.len()], TENUE, 1)
        };
        cv.text_fit(tx + fin, ey + 17, b"  (/ o clic; Enter manda, Esc sale)", mezclar(TENUE, PANEL, 150, 256), tope_x - tx - fin);
    }
    if v.sec == MENSAJES {
        // El ZUMBIDO: uno cada 10 s, como en la maqueta.
        let (bx, by, bw, bh) = caja_zumbido();
        let bx = bx + x0 - X_CENTRO;
        let c = if v.zumbido_listo { ROSA } else { mezclar(ROSA, PANEL, 90, 256) };
        redonda(cv, bx, by, bw, bh, 8, mezclar(c, PANEL, 60, 256));
        cv.text(bx + (bw - 7 * 8) / 2, by + (bh - 16) / 2, b"ZUMBIDO", c, 1);
    }
    if v.sin_guardar {
        cv.text(ex + ew - 30 * 8, ey - 18, b"no se pudo guardar en el disco", ROJO, 1);
    }
}

/// **El MURO**: seis postales que se voltean una tras otra.
fn muro(cv: &mut Canvas, v: &Vista, x0: i32) {
    let w = ancho_centro() - 40;
    let cw = (w - 2 * 16) / 3;
    let ch = ((suelo() - CABECERA - 80) / 2 - 16).min(cw * 3 / 4);
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
    cv.text(x0 + 20, suelo() - 30, b"postales dibujadas aqui con una semilla cada una; tus fotos de verdad: H9", TENUE, 1);
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
    let wc = ancho_centro() - 40;
    let tw = wc.min((suelo() - CABECERA - 100) * 16 / 9);
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

/// **Las paginas**: el inicio, con su buscador que se escribe solo.
fn paginas(cv: &mut Canvas, v: &Vista, x0: i32) {
    let w = ancho_centro() - 40;
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
    cv.text_fit(x0 + 20, suelo() - 30, b"el buscador buscara en TU maquina; las paginas de tus amigos llegan con H6 y H13-H15", TENUE, w);
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
    let w = ancho_centro() - 40;
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
    let w = ancho_centro() - 40;
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
    let w = ancho_centro() - 40;
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
