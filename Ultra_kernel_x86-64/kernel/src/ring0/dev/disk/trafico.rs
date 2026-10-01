//! **EL TRAFICO DEL DISCO** -- bytes leidos y escritos desde el arranque (01-10).
//!
//! [carril]  VERDE     dos contadores; no decide nada
//! [consumo] NADA      un `fetch_add` por comando que ya se mando
//!
//! ** Por que existe: el propietario quiere ver *"en tiempo real como se
//! ejecutan, en velocidad"* (la solapa PROCESOS y la de DISCOS de ESTRATOS).
//! Un MiB/s es una RESTA de dos lecturas de un contador separadas por un
//! tiempo, y hasta hoy no habia contador: `banda.rs` mide una vez, con su
//! propia lectura, y `cuentas_dma` solo separa directo de rebotado.
//!
//! Se cuenta en los TRES embudos, despues de que el disco conteste, con lo que
//! de verdad llego: `transfer::mandar_lectura` (el disco propio), `ajeno::leer`
//! (el disco Personal) y `write` (las dos ramas). No cambia que se lee ni que
//! se escribe: solo lo suma. Sale por `INFO_DISCO_LEIDO` / `INFO_DISCO_ESCRITO`.

use core::sync::atomic::{AtomicU64, Ordering};

static LEIDO: AtomicU64 = AtomicU64::new(0);
static ESCRITO: AtomicU64 = AtomicU64::new(0);

pub(super) fn leido(bytes: u64) {
    LEIDO.fetch_add(bytes, Ordering::Relaxed);
}

pub(super) fn escrito(bytes: u64) {
    ESCRITO.fetch_add(bytes, Ordering::Relaxed);
}

/// `(leidos, escritos)` en bytes desde el arranque, de todos los discos.
pub fn trafico() -> (u64, u64) {
    (LEIDO.load(Ordering::Relaxed), ESCRITO.load(Ordering::Relaxed))
}
