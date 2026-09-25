//! **ABRIR Y CERRAR, COMO UN TUBO DE RAYOS CATODICOS** (2026-09-25) -- el
//! marco de neon de una ventana que nace o que se va:
//!
//! ```text
//!    ABRE    un punto -> una linea -> el marco entero       (240 ms)
//!    CIERRA  el marco -> una linea -> un punto que se apaga (240 ms)
//! ```
//!
//! [consumo] LATE      los 240 ms de cada transicion, a ~30 fotogramas por
//!                     segundo (`desktop::transicion::anima`); sin ventanas
//!                     que nazcan o se vayan, nada (L6h)
//!
//! Aqui no se sabe QUE ventana es ni CUANDO: llega su caja, si abre o cierra y
//! la edad (`desktop::transicion` decide, esto solo pinta). Se pone como el
//! globo: guarda lo que tapa --las cuatro tiras del marco de ESTE fotograma--
//! y lo devuelve al principio del siguiente.

use bmo_userland as bmo;

use super::globo::mezcla;

/// Lo que dura una transicion.
pub(crate) const DURA_MS: u64 = 240;
/// El marco: 2 px de neon y 2 de resplandor por fuera.
const GROSOR: u32 = 4;
/// Dos transiciones a la vez (una que se va y otra que llega).
pub(crate) const HUECOS: usize = 2;
/// Cuatro tiras de un marco de hasta 4K, por hueco.
const GUARDADO: usize = (2 * (3840 + 2 * GROSOR) * GROSOR + 2 * 2160 * GROSOR) as usize;

const CIAN: u32 = 0x0000_F0FF;
const MAGENTA: u32 = 0x00FF_2BD6;
const BLANCO: u32 = 0x00FF_FFFF;

struct Hueco {
    px: [u32; GUARDADO],
    tiras: [(u32, u32, u32, u32); 4],
    n: usize,
    puesto: bool,
}

const VACIO: Hueco = Hueco { px: [0; GUARDADO], tiras: [(0, 0, 0, 0); 4], n: 0, puesto: false };
static mut HUECOS_: [Hueco; HUECOS] = [VACIO; HUECOS];

fn hueco(k: usize) -> &'static mut Hueco {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut (*core::ptr::addr_of_mut!(HUECOS_))[k] }
}

/// **Quita las transiciones**: devuelve lo que tapaban, al reves de como se
/// pusieron. Al PRINCIPIO del fotograma, el ultimo de las capas.
pub(crate) fn quitar(p: &bmo::Pantalla) {
    for k in (0..HUECOS).rev() {
        let h = hueco(k);
        if !h.puesto {
            continue;
        }
        let mut i = 0usize;
        for &(x, y, w, t) in &h.tiras[..h.n] {
            p.marcar(x, y, w, t);
            for dy in 0..t {
                for dx in 0..w {
                    p.punto_ya_marcado(x + dx, y + dy, h.px[i]);
                    i += 1;
                }
            }
        }
        h.puesto = false;
    }
}

/// **La caja de este instante**: `(x, y, w, h)` escalada desde el centro, y
/// cuanto brilla (256 = entero).
fn caja_en(c: (u32, u32, u32, u32), abre: bool, edad_ms: u64) -> ((u32, u32, u32, u32), u32) {
    let t = (edad_ms.min(DURA_MS) * 1000 / DURA_MS) as u32; // 0..1000
    // Al cerrar, la misma pelicula al reves.
    let t = if abre { t } else { 1000 - t };
    // Primero el ancho (0..400), despues el alto (400..1000).
    let fw = (t * 1000 / 400).min(1000);
    let fh = if t < 400 { 0 } else { (t - 400) * 1000 / 600 };
    let (x, y, w, h) = c;
    let ww = (w as u64 * fw as u64 / 1000).max(2) as u32;
    let hh = (h as u64 * fh as u64 / 1000).max(2) as u32;
    let caja = (x + (w - ww.min(w)) / 2, y + (h - hh.min(h)) / 2, ww.min(w), hh.min(h));
    // Brilla del todo mientras es linea; se desvanece al llegar al marco.
    let brillo = if t < 400 { 256 } else { 256 - (t - 400) * 100 / 600 };
    (caja, brillo)
}

/// **Pone la transicion** del hueco `k`: la ventana `caja`, que abre o cierra,
/// de `edad_ms`. Al FINAL del fotograma, debajo de las demas capas.
pub(crate) fn poner(p: &bmo::Pantalla, k: usize, caja: (u32, u32, u32, u32), abre: bool, edad_ms: u64) {
    let h = hueco(k);
    if h.puesto || caja.2 < 4 || caja.3 < 4 || edad_ms >= DURA_MS {
        return;
    }
    let ((x, y, w, hh), brillo) = caja_en(caja, abre, edad_ms);
    let neon = if abre { CIAN } else { MAGENTA };
    let (x0, y0) = (x.saturating_sub(GROSOR / 2), y.saturating_sub(GROSOR / 2));
    let x1 = (x + w + GROSOR / 2).min(p.ancho);
    let y1 = (y + hh + GROSOR / 2).min(p.alto);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    // Las tiras: arriba y abajo enteras; los lados, entre medias.
    let alto_t = GROSOR.min(y1 - y0);
    let mut tiras = [(0u32, 0u32, 0u32, 0u32); 4];
    let mut n = 0;
    let mut tira = |t: (u32, u32, u32, u32)| {
        if t.2 > 0 && t.3 > 0 && n < 4 {
            tiras[n] = t;
            n += 1;
        }
    };
    tira((x0, y0, x1 - x0, alto_t));
    if y1 - y0 > 2 * GROSOR {
        tira((x0, y1 - GROSOR, x1 - x0, GROSOR));
        let lado = (y1 - GROSOR) - (y0 + GROSOR);
        tira((x0, y0 + GROSOR, GROSOR.min(x1 - x0), lado));
        tira((x1.saturating_sub(GROSOR).max(x0), y0 + GROSOR, GROSOR.min(x1 - x0), lado));
    } else if y1 - y0 > alto_t {
        tira((x0, y0 + alto_t, x1 - x0, y1 - y0 - alto_t));
    }
    let total: usize = tiras[..n].iter().map(|t| (t.2 * t.3) as usize).sum();
    if total > GUARDADO {
        return;
    }
    p.sincronizar_lectura();
    let mut i = 0usize;
    for &(tx, ty, tw, th) in &tiras[..n] {
        for dy in 0..th {
            for dx in 0..tw {
                h.px[i] = p.read(tx + dx, ty + dy);
                i += 1;
            }
        }
    }
    h.tiras = tiras;
    h.n = n;
    h.puesto = true;
    // Pintar: el centro de la tira en neon (y casi blanco mientras es linea),
    // los bordes de la tira como resplandor.
    let mut i = 0usize;
    for &(tx, ty, tw, th) in &tiras[..n] {
        p.marcar(tx, ty, tw, th);
        for dy in 0..th {
            for dx in 0..tw {
                let (px, py) = (tx + dx, ty + dy);
                // A cuanto esta del borde ideal del marco (0 = encima).
                let dxb = px.abs_diff(x).min(px.abs_diff(x + w - 1));
                let dyb = py.abs_diff(y).min(py.abs_diff(y + hh - 1));
                let dentro_x = px >= x && px < x + w;
                let dentro_y = py >= y && py < y + hh;
                let d = if dentro_x && dentro_y { dxb.min(dyb) } else if dentro_x { dyb } else if dentro_y { dxb } else { dxb.max(dyb) };
                let alfa = match d {
                    0 => 255,
                    1 => 200,
                    2 => 90,
                    _ => 30,
                } * brillo / 256;
                let color = if d == 0 { mezcla(neon, BLANCO, brillo / 2) } else { neon };
                p.punto_ya_marcado(px, py, mezcla(h.px[i], color, alfa));
                i += 1;
            }
        }
    }
}
