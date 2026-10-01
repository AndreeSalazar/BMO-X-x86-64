//! **EL BORDE VIVO de las ventanas SIN MARCO** (01-10). Pedido: *"esta F4 no
//! necesita bordes... que solo se mantenga minimizar, maximizar y cerrar...
//! y tiene animacion, eso en los bordes nada mas"*.
//!
//! [consumo] LATE      solo con el puntero sobre una ventana sin marco, o
//!                     mientras sus botones asoman o se esconden ([`anima`],
//!                     que mira el reloj); en reposo, nada (L6h)
//!
//! ```text
//!    el puntero entra       dos luces corren por el borde de un pixel: una
//!                           del acento hacia el blanco, otra magenta al reves
//!    se acerca arriba       la pastilla de los tres botones se abre desde la
//!                           derecha en 8 fotogramas, y la raya de agarrar crece
//!    se va                  la pastilla se cierra; cada paso vuelve a pegar
//!                           la app, que es lo que habia debajo
//! ```
//!
//! La ventana la dibuja `Chrome` (`paint_vivo`, `paint_pastilla`); aqui solo
//! se decide CUANDO, una vez por fotograma que pinta, despues de las apps.

use bmo_userland as bmo;

use crate::desktop::Desktop;
use crate::scene::chrome::{ASOMA, ZONA_ASOMA};
use crate::scene::surface::MAX;
use crate::scene::BOX_EDGE;

const FOTOGRAMA_MS: u64 = 33;

struct Estado {
    por_ms: u64,
    /// Hay algo que animar: una luz corriendo o unos botones a medias.
    vivo: bool,
    pintado: u64,
    /// La caja con la luz encendida el fotograma anterior: al irse el
    /// puntero, su borde vuelve a estar quieto.
    encendida: [bool; MAX],
}

static mut ESTADO: Estado = Estado { por_ms: 0, vivo: false, pintado: 0, encendida: [false; MAX] };

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

/// **Este fotograma**: asomar o esconder los botones un paso, y la luz del
/// borde. `tapado`: la pantalla es de otro (pantalla completa, prestada).
pub(crate) fn poner(dsk: &mut Desktop, p: &bmo::Pantalla, tapado: bool) {
    let e = estado();
    if e.por_ms == 0 {
        e.por_ms = (bmo::info(bmo::INFO_TSC_HZ) / 1000).max(1);
    }
    let ahora = bmo::ciclos();
    let toca = ahora.wrapping_sub(e.pintado) >= FOTOGRAMA_MS * e.por_ms;
    let ms = ahora / e.por_ms;
    let (px, py) = (dsk.tick.ax, dsk.tick.ay);
    // La de debajo del puntero, si es una app y nada la tapa.
    let encima = if px != u32::MAX && crate::desktop::paint::app_encima(dsk, px, py) { dsk.table.at(px, py) } else { None };
    let mut vivo = false;
    for i in 0..MAX {
        let Some(s) = dsk.table.get_mut(i) else {
            e.encendida[i] = false;
            continue;
        };
        if !s.chrome.sin_marco || s.chrome.minimized || s.chrome.is_fullscreen() || tapado {
            e.encendida[i] = false;
            continue;
        }
        let sobre = encima == Some(i);
        let cerca = sobre && py < s.chrome.y + ZONA_ASOMA;
        let meta = if cerca || s.chrome.grabbed() { ASOMA } else { 0 };
        if s.chrome.asoma != meta {
            vivo = true;
            if toca {
                if meta > s.chrome.asoma {
                    s.chrome.asoma += 1;
                    s.chrome.paint_pastilla(p);
                } else {
                    // Lo que se esconde vuelve a ser de la app: se repega y la
                    // pastilla, mas chica, se pinta encima al componer.
                    s.chrome.asoma -= 1;
                    s.repegar();
                }
            }
        }
        if sobre {
            vivo = true;
            if toca {
                s.chrome.paint_vivo(p, BOX_EDGE, ms, true);
            }
        } else if e.encendida[i] {
            s.chrome.paint_vivo(p, BOX_EDGE, ms, false);
        }
        e.encendida[i] = sobre;
    }
    e.vivo = vivo;
    if toca && vivo {
        e.pintado = ahora;
    }
}

/// **Pide fotograma** mientras algo se anima. Solo lee el reloj.
pub(crate) fn anima() -> bool {
    let e = estado();
    e.vivo && bmo::ciclos().wrapping_sub(e.pintado) >= FOTOGRAMA_MS * e.por_ms
}
