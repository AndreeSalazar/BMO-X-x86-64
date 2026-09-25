//! **EL DESTELLO DEL FOCO** -- cuando una ventana toma el foco, un resplandor
//! de neon la rodea, se enciende de golpe y se apaga en menos de un segundo:
//! lo que Hyprland hace con el borde activo, aqui como una capa aparte.
//!
//! [consumo] LATE      solo los ~700 ms que dura cada destello, a ~30
//!                     fotogramas por segundo (`desktop::brillo::anima`); con
//!                     el foco quieto, nada (L6h)
//!
//! Aqui no se sabe CUANDO ni DE QUIEN: llega la caja de la ventana y la edad
//! del destello (`desktop::brillo` decide, esto solo pinta).
//!
//! Se pone como el globo: guarda lo que tapa --cuatro tiras alrededor de la
//! ventana-- al FINAL del fotograma, debajo del globo, del recorte y del
//! cursor, y lo devuelve al PRINCIPIO del siguiente. El resplandor se MEZCLA
//! con lo guardado: se ve el fondo a traves.
//!
//! ```text
//!    0..120 ms    se enciende: de nada a todo, blanco en el borde
//!    120..700     se apaga despacio, de cian a magenta
//! ```

use bmo_userland as bmo;

use super::globo::{mezcla, onda};

/// Lo que sale por fuera de la ventana.
const GROSOR: u32 = 10;
/// Cuatro tiras de una ventana de hasta 4K.
const GUARDADO: usize = (2 * (3840 + 2 * GROSOR) * GROSOR + 2 * 2160 * GROSOR) as usize;
const SUBE_MS: u64 = 120;

const CIAN: u32 = 0x0000_F0FF;
const MAGENTA: u32 = 0x00FF_2BD6;
const BLANCO: u32 = 0x00FF_FFFF;
/// Cuanto brilla cada anillo, de dentro afuera, sobre 256.
const ANILLOS: [u32; GROSOR as usize] = [256, 235, 200, 160, 124, 92, 64, 40, 22, 10];

struct Brillo {
    px: [u32; GUARDADO],
    tiras: [(u32, u32, u32, u32); 4],
    n: usize,
    puesto: bool,
}

static mut BRILLO: Brillo = Brillo { px: [0; GUARDADO], tiras: [(0, 0, 0, 0); 4], n: 0, puesto: false };

fn brillo() -> &'static mut Brillo {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(BRILLO) }
}

/// **Quita el destello**: devuelve lo que tapaba. Al PRINCIPIO del fotograma,
/// despues del globo. Si no estaba, no hace nada.
pub(crate) fn quitar(p: &bmo::Pantalla) {
    let b = brillo();
    if !b.puesto {
        return;
    }
    let mut k = 0usize;
    for &(x, y, w, h) in &b.tiras[..b.n] {
        p.marcar(x, y, w, h);
        for dy in 0..h {
            for dx in 0..w {
                p.punto_ya_marcado(x + dx, y + dy, b.px[k]);
                k += 1;
            }
        }
    }
    b.puesto = false;
}

/// **Pone el destello** alrededor de la ventana `(x, y, w, h)`, de `edad_ms`
/// de los `vida_ms` que dura. Al FINAL del fotograma, antes del globo.
pub(crate) fn poner(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32), edad_ms: u64, vida_ms: u64) {
    let b = brillo();
    if b.puesto || w == 0 || h == 0 || edad_ms >= vida_ms {
        return;
    }
    // Lo que brilla ahora, sobre 256: sube de golpe y baja despacio.
    let fuerza = if edad_ms < SUBE_MS {
        (edad_ms * 256 / SUBE_MS) as u32
    } else {
        let q = 256 - ((edad_ms - SUBE_MS) * 256 / (vida_ms - SUBE_MS)) as u32;
        q * q / 256
    };
    let neon = mezcla(CIAN, MAGENTA, onda(edad_ms, 900));
    // Las cuatro tiras, recortadas a la pantalla.
    let (x0, y0) = (x.saturating_sub(GROSOR), y.saturating_sub(GROSOR));
    let x1 = (x + w + GROSOR).min(p.ancho);
    let y1 = (y + h + GROSOR).min(p.alto);
    let mut tiras = [(0u32, 0u32, 0u32, 0u32); 4];
    let mut n = 0;
    let mut tira = |t: (u32, u32, u32, u32)| {
        if t.2 > 0 && t.3 > 0 {
            tiras[n] = t;
            n += 1;
        }
    };
    tira((x0, y0, x1 - x0, y.saturating_sub(y0)));
    tira((x0, (y + h).min(y1), x1 - x0, y1.saturating_sub(y + h)));
    tira((x0, y, x.saturating_sub(x0), h.min(y1.saturating_sub(y))));
    tira(((x + w).min(x1), y, x1.saturating_sub(x + w), h.min(y1.saturating_sub(y))));
    let total: usize = tiras[..n].iter().map(|t| (t.2 * t.3) as usize).sum();
    if total > GUARDADO {
        return;
    }
    // Guardar TODO antes de pintar nada: las tiras no se pisan, pero asi no
    // importa si algun dia lo hacen.
    p.sincronizar_lectura();
    let mut k = 0usize;
    for &(tx, ty, tw, th) in &tiras[..n] {
        for dy in 0..th {
            for dx in 0..tw {
                b.px[k] = p.read(tx + dx, ty + dy);
                k += 1;
            }
        }
    }
    b.tiras = tiras;
    b.n = n;
    b.puesto = true;
    // Pintar: cada pixel segun a cuantos pixeles esta de la ventana.
    let mut k = 0usize;
    for &(tx, ty, tw, th) in &tiras[..n] {
        p.marcar(tx, ty, tw, th);
        for dy in 0..th {
            for dx in 0..tw {
                let (px, py) = (tx + dx, ty + dy);
                let fx = if px < x { x - px } else if px >= x + w { px - (x + w) + 1 } else { 0 };
                let fy = if py < y { y - py } else if py >= y + h { py - (y + h) + 1 } else { 0 };
                let d = fx.max(fy).clamp(1, GROSOR) as usize - 1;
                let alfa = ANILLOS[d] * fuerza / 256;
                // El anillo de dentro, al encenderse, casi blanco.
                let color = if d == 0 { mezcla(neon, BLANCO, fuerza / 2) } else { neon };
                p.punto_ya_marcado(px, py, mezcla(b.px[k], color, alfa));
                k += 1;
            }
        }
    }
}
