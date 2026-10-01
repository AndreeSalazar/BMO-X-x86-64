//! **INICIO** -- la portada de ESTRATOS, al estilo del Explorador de Windows
//! (2026-10-01).
//!
//! [consumo] NADA      mide los discos y lee la historia al ENTRAR; pintar
//!                     solo mira lo ya leido (L6h)
//!
//! El propietario, con la maqueta `ESTRATOS estilo Windows` delante: *"estan
//! perfecto, aplicar todo eso ... en mi BMO-X fui por ESTRATOS y solo veo el
//! benchmark"*. Es la lamina 1 de esa maqueta, en verde neon:
//!
//! ```text
//!    +-------------+-------------------------------------------------+
//!    | > Inicio    |  v Acceso rapido                                |
//!    |   Biblioteca|  [juegos] [documentos] [cache] [proton-x]       |
//!    |   juegos    |  v Discos                                       |
//!    |   documentos|  [ESTRATOS ===   ] [DATOS ==  ] [D: ====] [EFI] |
//!    |   cache     |  v Reciente (las versiones de ESTRATOS)         |
//!    |   proton-x  |  version   cuando               quien           |
//!    | ESTE EQUIPO |  ...                                            |
//!    |   ESTRATOS  |                                                 |
//!    |   DATOS ... |                                                 |
//!    | [gato vigila]                                                 |
//!    +-------------+-------------------------------------------------+
//! ```
//!
//! Todo se abre con un clic (o las flechas y ENTRAR): una carpeta fijada va al
//! explorador DENTRO de ella, un disco al explorador de ese volumen. Reciente
//! son las versiones de ESTRATOS -- lo que se escribio y cuando, que es lo que
//! este volumen sabe y un FAT32 no.

use bmo_userland as bmo;

use super::*;
use crate::scene::iconos::{self, dibujos};
use crate::scene::zonas::Zona;
use crate::text::decimal;

/// Las carpetas fijadas, en ESTRATOS. Las crea `carpeta NOMBRE` en la consola.
pub(crate) const FIJADAS: [&str; 4] = ["juegos", "documentos", "cache", "proton-x"];

/// Lo que hace un clic en el Inicio.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Accion {
    Ir(View),
    /// Abrir la carpeta fijada `k` en el explorador.
    Carpeta(usize),
    /// Abrir la unidad `k` de `equipo::TODAS`.
    Unidad(usize),
}

/// Las filas de la columna de la izquierda: texto, color y que hacen.
/// `None` es un separador.
const LADO: [Option<(&str, u32, Accion)>; 13] = [
    Some(("Inicio", DATA_TITLE, Accion::Ir(View::Inicio))),
    Some(("Biblioteca", 0x00A6_FF3B, Accion::Ir(View::Biblioteca))),
    Some(("Procesos", 0x005E_F2E6, Accion::Ir(View::Procesos))),
    None,
    Some(("juegos", 0x00A6_FF3B, Accion::Carpeta(0))),
    Some(("documentos", DATA_TITLE, Accion::Carpeta(1))),
    Some(("cache", DATA_TITLE, Accion::Carpeta(2))),
    Some(("proton-x", 0x005E_F2E6, Accion::Carpeta(3))),
    None,
    Some(("ESTRATOS (F:)", DATA_TITLE, Accion::Unidad(2))),
    Some(("BMO (A:)", 0x0060_A5FA, Accion::Unidad(0))),
    Some(("Personal (D:)", 0x00F0_B060, Accion::Unidad(1))),
    Some(("EFI", 0x00A7_8BFA, Accion::Unidad(3))),
];

/// Los ocho blancos de las flechas: las cuatro fijadas y los cuatro discos.
pub(crate) const BLANCOS: usize = 8;

pub(crate) fn blanco(k: usize) -> Accion {
    if k < 4 {
        Accion::Carpeta(k)
    } else {
        // Los discos en el orden de la fila: ESTRATOS, DATOS, Personal, EFI.
        Accion::Unidad([2, 0, 1, 3][(k - 4).min(3)])
    }
}

// ===================================================================
//  La geometria -- UNA para pintar y para acertar con el raton
// ===================================================================

const LADO_W: u32 = 220;
const FILA_LADO: u32 = 26;
const SEP: u32 = 14;
const CAB: u32 = 26;
const GAP: u32 = 12;
const TARJETA_H: u32 = 52;
const DISCO_H: u32 = 66;
const FILA: u32 = 26;

struct Partes {
    lado: Zona,
    principal: Zona,
}

fn partes(z: &Zona) -> Partes {
    let lado = Zona { x: z.x + 8, y: z.y + 8, w: LADO_W, h: z.h.saturating_sub(16) };
    let px = lado.x + LADO_W + 16;
    let principal = Zona { x: px, y: z.y + 8, w: (z.x + z.w).saturating_sub(px + 12), h: z.h.saturating_sub(16) };
    Partes { lado, principal }
}

/// La fila `k` de la columna: su `y`, o `None` si es un separador.
fn fila_lado(l: &Zona, k: usize) -> u32 {
    let mut y = l.y + 8;
    for (i, f) in LADO.iter().enumerate() {
        if i == k {
            return y;
        }
        y += if f.is_some() { FILA_LADO } else { SEP };
        // "ESTE EQUIPO" va despues del segundo separador.
        if f.is_none() && i == 8 {
            y += FILA_LADO - 6;
        }
    }
    y
}

/// Cuantas tarjetas por fila: cuatro si caben con su texto (una tarjeta de
/// disco pide ~175 px), si no dos, en dos filas.
fn columnas(p: &Zona) -> u32 {
    if p.w >= 4 * 175 + 3 * GAP { 4 } else { 2 }
}

/// La tarjeta `k` (de cuatro) de un bloque que empieza en `y`.
fn tarjeta(p: &Zona, y: u32, k: usize, h: u32) -> Zona {
    let c = columnas(p);
    let w = p.w.saturating_sub((c - 1) * GAP) / c;
    let (col, fila) = (k as u32 % c, k as u32 / c);
    Zona { x: p.x + col * (w + GAP), y: y + fila * (h + GAP), w, h }
}

/// El alto de un bloque de cuatro tarjetas de alto `h`.
fn bloque(p: &Zona, h: u32) -> u32 {
    let filas = 4 / columnas(p);
    filas * h + (filas - 1) * GAP
}

fn y_rapido(p: &Zona) -> u32 {
    p.y + CAB
}

fn y_discos(p: &Zona) -> u32 {
    y_rapido(p) + bloque(p, TARJETA_H) + 10 + CAB
}

fn y_reciente(p: &Zona) -> u32 {
    y_discos(p) + bloque(p, DISCO_H) + 10 + CAB
}

/// **Sobre que cayo el puntero**, si sobre algo que se abre.
pub(crate) fn en(z: &Zona, px: u32, py: u32) -> Option<Accion> {
    let pt = partes(z);
    if pt.lado.contiene(px, py) {
        for (k, f) in LADO.iter().enumerate() {
            if let Some((_, _, a)) = f {
                let y = fila_lado(&pt.lado, k);
                if py >= y && py < y + FILA_LADO {
                    return Some(*a);
                }
            }
        }
        return None;
    }
    for k in 0..4 {
        if tarjeta(&pt.principal, y_rapido(&pt.principal), k, TARJETA_H).contiene(px, py) {
            return Some(blanco(k));
        }
        if tarjeta(&pt.principal, y_discos(&pt.principal), k, DISCO_H).contiene(px, py) {
            return Some(blanco(4 + k));
        }
    }
    None
}

// ===================================================================
//  Pintar
// ===================================================================

fn caja(p: &bmo::Pantalla, z: &Zona, borde: u32, fondo: u32, encendida: bool) {
    crate::scene::borde::marco(p, z.x, z.y, z.w, z.h, crate::scene::RADIUS, borde, fondo);
    if encendida {
        let r = crate::scene::RADIUS;
        p.rect(z.x + r, z.y, z.w.saturating_sub(2 * r), 1, DATA_TITLE);
    }
}

fn cabecera(p: &bmo::Pantalla, x: u32, y: u32, titulo: &str) {
    let ty = y + (CAB - bmo::GLIFO_ALTO) / 2;
    p.texto(x, ty, "v", DATA_TITLE);
    p.texto(x + 2 * bmo::GLIFO_ANCHO, ty, titulo, INK);
}

/// `bytes` como "30.3 GB".
fn gb(bytes: u64, dst: &mut [u8; 24]) -> usize {
    let decimas = bytes / 100_000_000;
    let mut b = [0u8; 10];
    let n = decimal(decimas / 10, &mut b);
    dst[..n].copy_from_slice(&b[..n]);
    dst[n] = b'.';
    dst[n + 1] = b'0' + (decimas % 10) as u8;
    dst[n + 2..n + 5].copy_from_slice(b" GB");
    n + 5
}

fn lado(p: &bmo::Pantalla, l: &Zona) {
    p.rect(l.x, l.y, l.w, l.h, 0x0007_110B);
    p.rect(l.x + l.w - 1, l.y, 1, l.h, DATA_EDGE);
    for (k, f) in LADO.iter().enumerate() {
        let y = fila_lado(l, k);
        match f {
            None => {
                p.rect(l.x + 10, y + SEP / 2, l.w.saturating_sub(20), 1, DATA_EDGE);
                if k == 8 {
                    p.texto(l.x + 14, y + SEP + 2, "ESTE EQUIPO", INK_DIM);
                }
            }
            Some((texto, color, a)) => {
                let es = *a == Accion::Ir(View::Inicio);
                if es {
                    p.rect(l.x + 4, y + 2, l.w.saturating_sub(10), FILA_LADO - 4, NODE_SEL);
                    p.rect(l.x + 4, y + 2, 3, FILA_LADO - 4, DATA_TITLE);
                }
                // El dibujo chico de cada fila: carpeta, disco o capas.
                let d: &[bmo_dibujo::Capa] = match a {
                    Accion::Unidad(2) | Accion::Ir(View::Inicio) => &dibujos::ESTRATOS,
                    Accion::Unidad(3) => &dibujos::CHIP,
                    Accion::Unidad(_) => &dibujos::DISCO,
                    _ => &dibujos::CARPETA,
                };
                let fondo = if es { NODE_SEL } else { 0x0007_110B };
                iconos::vector(p, l.x + 14, y + (FILA_LADO - 16) / 2, 16, d, &iconos::paleta(*color, *color), fondo);
                let ty = y + (FILA_LADO - bmo::GLIFO_ALTO) / 2;
                p.texto(l.x + 40, ty, texto, if es { INK } else { 0x00C9_D8CF });
            }
        }
    }
    // -- el gato vigila, abajo --
    let card = Zona { x: l.x + 8, y: (l.y + l.h).saturating_sub(84), w: l.w.saturating_sub(16), h: 76 };
    if card.y > fila_lado(l, LADO.len() - 1) + FILA_LADO {
        caja(p, &card, DATA_EDGE, 0x000D_1B13, false);
        let montado = bmo::info(bmo::INFO_ES_MONTADO) != 0;
        crate::scene::gato_vivo::miniatura(p, card.x + 10, card.y + 8, 3, 0x00F2_F7F9, DATA_TITLE, false);
        let tx = card.x + 72;
        p.texto(tx, card.y + 18, "el gato vigila", INK);
        p.texto(tx, card.y + 18 + bmo::GLIFO_ALTO + 6, if montado { "ESTRATOS sano" } else { "sin ESTRATOS" }, if montado { DATA_TITLE } else { INK_BAD });
    }
}

fn rapido(p: &bmo::Pantalla, pr: &Zona, sel: usize) {
    cabecera(p, pr.x, pr.y, "Acceso rapido");
    for (k, nombre) in FIJADAS.iter().enumerate() {
        let t = tarjeta(pr, y_rapido(pr), k, TARJETA_H);
        let es = sel == k;
        let color = if k == 0 { 0x00A6_FF3B } else if k == 3 { 0x005E_F2E6 } else { DATA_TITLE };
        caja(p, &t, if es { sel_neon() } else { DATA_EDGE }, if es { SEL_FONDO } else { NODE_BG }, true);
        let fondo = if es { SEL_FONDO } else { NODE_BG };
        iconos::vector(p, t.x + 12, t.y + (TARJETA_H - 32) / 2, 32, &dibujos::CARPETA, &iconos::paleta(color, color), fondo);
        p.texto(t.x + 60, t.y + 8, nombre, INK);
        p.texto(t.x + 60, t.y + 8 + bmo::GLIFO_ALTO + 4, "ESTRATOS", INK_DIM);
    }
}

fn discos(p: &bmo::Pantalla, pr: &Zona, sel: usize) {
    cabecera(p, pr.x, y_discos(pr) - CAB, "Discos");
    for k in 0..4 {
        let t = tarjeta(pr, y_discos(pr), k, DISCO_H);
        let u = [2usize, 0, 1, 3][k];
        let (nombre, color, d): (&str, u32, &[bmo_dibujo::Capa]) = match u {
            2 => ("ESTRATOS (F:)", DATA_TITLE, &dibujos::ESTRATOS),
            0 => ("BMO (A:)", 0x0060_A5FA, &dibujos::DISCO),
            1 => ("Personal (D:)", 0x00F0_B060, &dibujos::DISCO),
            _ => ("EFI", 0x00A7_8BFA, &dibujos::CHIP),
        };
        let es = sel == 4 + k;
        let fondo = if es { SEL_FONDO } else { NODE_BG };
        caja(p, &t, if es { sel_neon() } else if u == 2 { DATA_TITLE } else { DATA_EDGE }, fondo, u == 2);
        iconos::vector(p, t.x + 10, t.y + 10, 32, d, &iconos::paleta(color, color), fondo);
        let tx = t.x + 50;
        p.texto(tx, t.y + 10, nombre, INK);
        let (bytes, libres) = super::equipo::medida_de(u);
        let bw = t.w.saturating_sub(64);
        let by = t.y + 10 + bmo::GLIFO_ALTO + 8;
        p.rect(tx, by, bw, 8, 0x0007_110B);
        let mut b = [0u8; 24];
        match (bytes, libres) {
            (0, _) => {
                p.texto(tx, by + 14, "sin medir", INK_DIM);
            }
            (total, Some(l)) => {
                let usado = total.saturating_sub(l);
                let w = ((usado as u128 * bw as u128) / total.max(1) as u128) as u32;
                p.rect(tx, by, w.max(1), 8, color);
                let n = gb(l, &mut b);
                let x = p.texto_bytes(tx, by + 14, &b[..n], INK);
                p.texto(x, by + 14, " libres", INK_DIM);
            }
            (total, None) => {
                let n = gb(total, &mut b);
                let x = p.texto_bytes(tx, by + 14, &b[..n], INK);
                p.texto(x, by + 14, " (libre: sin contar)", INK_DIM);
            }
        }
    }
}

fn reciente(p: &bmo::Pantalla, pr: &Zona) {
    let y0 = y_reciente(pr);
    cabecera(p, pr.x, y0 - CAB, "Reciente -- las versiones de ESTRATOS");
    let alto = (pr.y + pr.h).saturating_sub(y0);
    if alto < 2 * FILA {
        return;
    }
    let t = Zona { x: pr.x, y: y0, w: pr.w, h: alto };
    caja(p, &t, DATA_EDGE, 0x000D_1B13, false);
    let cols = [t.x + 16, t.x + 16 + t.w * 40 / 100, t.x + 16 + t.w * 75 / 100];
    let mut y = t.y + 8;
    for (k, c) in ["Version", "Cuando", "Quien"].iter().enumerate() {
        p.texto(cols[k], y, c, INK_DIM);
    }
    y += bmo::GLIFO_ALTO + 8;
    p.rect(t.x + 8, y, t.w.saturating_sub(16), 1, DATA_TITLE);
    y += 4;
    let cuantas = bmo::estratos::hist_cuantas();
    if cuantas == 0 {
        p.texto(cols[0], y + 6, "sin versiones: ESTRATOS no esta montado o esta vacio", INK_DIM);
        return;
    }
    let mut b = [0u8; 10];
    let mut i = 0u64;
    while i < cuantas && y + FILA <= t.y + t.h {
        let ty = y + (FILA - bmo::GLIFO_ALTO) / 2;
        if i == 0 {
            p.rect(t.x + 6, y + 2, t.w.saturating_sub(12), FILA - 4, NODE_SEL);
        }
        p.rect(t.x + 8, y + 6, 3, FILA - 12, DATA_TITLE);
        let mut nom = [0u8; 40];
        let x = if bmo::estratos::hist_con_nombre(i) {
            let n = bmo::estratos::hist_nombre(i, &mut nom);
            p.texto_bytes(cols[0], ty, &nom[..n], DATA_TITLE)
        } else {
            let x = p.texto(cols[0], ty, "v", INK);
            let n = decimal(cuantas - i, &mut b);
            p.texto_bytes(x, ty, &b[..n], INK)
        };
        if i == 0 {
            p.texto(x + 8, ty, "(ahora)", INK_DIM);
        }
        match bmo_rtc::desempaquetar(bmo::estratos::hist_cuando(i)) {
            Some(f) => {
                let mut fb = [0u8; 24];
                let n = bmo_rtc::escribir(&f, &mut fb);
                p.texto_bytes(cols[1], ty, &fb[..n.min(19)], 0x00C9_D8CF);
            }
            None => {
                p.texto(cols[1], ty, "sin fecha", INK_DIM);
            }
        }
        let x = p.texto(cols[2], ty, "pid ", INK_DIM);
        let n = decimal(bmo::estratos::hist_quien(i), &mut b);
        p.texto_bytes(x, ty, &b[..n], 0x00C9_D8CF);
        y += FILA;
        i += 1;
    }
}

/// **Pinta el Inicio** en `z`; `sel` es el blanco de las flechas (0..8).
pub(crate) fn paint(p: &bmo::Pantalla, z: &Zona, sel: usize) {
    if !z.hay() {
        return;
    }
    let pt = partes(z);
    lado(p, &pt.lado);
    rapido(p, &pt.principal, sel);
    discos(p, &pt.principal, sel);
    reciente(p, &pt.principal);
}
