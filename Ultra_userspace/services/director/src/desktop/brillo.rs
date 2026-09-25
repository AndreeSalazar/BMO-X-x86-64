//! **EL DESTELLO DEL FOCO: cuando y de quien** (2026-09-25). Pedido del
//! propietario: *"en mi escritorio dale TODO animado"*. La primera animacion
//! del escritorio entero es la del FOCO: la ventana que lo toma se enciende en
//! neon y se apaga sola, y asi se ve a donde van las teclas sin buscar.
//!
//! [consumo] LATE      ~700 ms a ~30 fotogramas por segundo tras cada cambio
//!                     de foco ([`anima`], que solo mira el reloj). El borde
//!                     vivo de despues NO pide fotogramas: avanza en los del
//!                     cuarto de segundo que el escritorio ya pinta (L6h)
//!
//! La cara la pinta `scene::brillo`; esto dice cuando nace (`foco::seguir`
//! lo enciende) y sobre que caja (la de la ventana AHORA: si se mueve
//! mientras brilla, el destello la sigue).

use bmo_userland as bmo;

use crate::desktop::{Desktop, Ventana};

/// Lo que dura un destello.
const DURA_MS: u64 = 700;
const FOTOGRAMA_MS: u64 = 33;

struct Estado {
    por_ms: u64,
    ventana: Option<Ventana>,
    desde: u64,
    pintado: u64,
}

static mut ESTADO: Estado = Estado { por_ms: 0, ventana: None, desde: 0, pintado: 0 };

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

/// **Encender** el destello de `v`: lo llama `foco::seguir` al cambiar el foco.
pub(crate) fn encender(v: Ventana) {
    let e = estado();
    if e.por_ms == 0 {
        e.por_ms = (bmo::info(bmo::INFO_TSC_HZ) / 1000).max(1);
    }
    e.ventana = Some(v);
    e.desde = bmo::ciclos();
    e.pintado = 0;
}

/// **Apagar**: nadie tiene el foco.
pub(crate) fn apagar() {
    estado().ventana = None;
}

/// **El borde vivo de este fotograma** (y el destello, si vive). Al FINAL
/// del fotograma que pinta, ANTES del globo. `tapado`: una ventana a pantalla
/// completa.
pub(crate) fn poner(dsk: &Desktop, p: &bmo::Pantalla, tapado: bool) {
    let e = estado();
    let Some(v) = e.ventana else { return };
    let ahora = bmo::ciclos();
    let ms = ahora.wrapping_sub(e.desde) / e.por_ms.max(1);
    let Some(caja) = crate::desktop::foco::caja(dsk, v) else { return };
    // A pantalla completa (o tapado por una) no hay borde que encender.
    if tapado || caja.2 >= p.ancho && caja.3 >= p.alto {
        return;
    }
    e.pintado = ahora;
    crate::scene::brillo::poner(p, caja, ms, DURA_MS, ahora / e.por_ms.max(1));
}

/// **Pide fotograma** mientras vive un destello: uno cada [`FOTOGRAMA_MS`].
/// Lo pregunta el bucle en cada vuelta: solo lee el reloj.
pub(crate) fn anima() -> bool {
    let e = estado();
    let ahora = bmo::ciclos();
    // Solo el DESTELLO pide fotogramas; el borde vivo va en los del cuarto.
    e.ventana.is_some()
        && ahora.wrapping_sub(e.desde) < DURA_MS * e.por_ms
        && ahora.wrapping_sub(e.pintado) >= FOTOGRAMA_MS * e.por_ms
}
