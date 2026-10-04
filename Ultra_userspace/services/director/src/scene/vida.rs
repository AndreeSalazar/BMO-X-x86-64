//! **LA VIDA** -- el reloj de las animaciones del escritorio (04-10).
//!
//! [consumo] LATE      solo mientras vive una animacion: pide fotograma cada
//!                     ~16 ms hasta que acaba la ultima, y en reposo NADA --
//!                     se anima cuando pasa algo, no en bucle (L6h)
//!
//! El propietario: *"que TODOS tengan vida y epico, y la barra lateral con
//! animaciones especiales y unicas"*. Cada animacion del escritorio es un
//! [`Paso`]: cuando empezo y cuanto dura. La curva es la de MAQUETA
//! (`bmo_pinta::curva`, el `cubic-bezier` de CSS en enteros): la misma cuenta
//! que una transicion compilada.
//!
//! Las animaciones no tienen bucle propio. Al empezar, apuntan hasta cuando
//! hay vida ([`VIVA_HASTA`]); el bucle del escritorio pregunta [`anima`] y
//! pinta mientras haga falta, y quien anima se repinta en ese fotograma
//! mirando su `Paso`.

use bmo_userland as bmo;
use core::sync::atomic::{AtomicU64, Ordering};

/// El rebote de los frameworks: se pasa un poco y vuelve.
pub(crate) const REBOTE: [i32; 4] = [340, 1560, 640, 1000];
/// `ease-out`: rapido al salir, suave al llegar.
pub(crate) const SALIDA: [i32; 4] = [0, 0, 580, 1000];

/// Ciclos del TSC por milisegundo (0 = sin preguntar todavia).
static POR_MS: AtomicU64 = AtomicU64::new(0);
/// Hasta cuando hay algo moviendose, en ms.
static VIVA_HASTA: AtomicU64 = AtomicU64::new(0);
/// El ultimo fotograma pintado con vida, en ms.
static PINTADO: AtomicU64 = AtomicU64::new(0);

/// Lo que dura, como mucho, un fotograma de animacion.
const FOTOGRAMA_MS: u64 = 16;

/// **El reloj**, en milisegundos.
pub(crate) fn ahora_ms() -> u64 {
    let mut p = POR_MS.load(Ordering::Relaxed);
    if p == 0 {
        p = (bmo::info(bmo::INFO_TSC_HZ) / 1000).max(1);
        POR_MS.store(p, Ordering::Relaxed);
    }
    bmo::ciclos() / p
}

/// **Una animacion**: cuando empezo y cuanto dura.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Paso {
    desde: u64,
    dura: u32,
}

impl Paso {
    /// La que ya acabo (o nunca empezo): su avance es el final.
    pub(crate) const QUIETO: Paso = Paso { desde: 0, dura: 0 };

    /// **Empieza ahora**, y apunta que hay vida hasta su final.
    pub(crate) fn empezar(dura: u32) -> Paso {
        let desde = ahora_ms();
        VIVA_HASTA.fetch_max(desde + dura as u64, Ordering::Relaxed);
        Paso { desde, dura }
    }

    /// Cuanto ha avanzado con esta `curva` (milesimas; con el rebote pasa
    /// de 1000).
    pub(crate) fn k(&self, curva: [i32; 4]) -> i32 {
        if self.dura == 0 {
            return 1000;
        }
        let ms = ahora_ms().saturating_sub(self.desde).min(u32::MAX as u64) as u32;
        bmo::avance(ms, 0, self.dura, curva)
    }

    /// Todavia se mueve?
    pub(crate) fn vivo(&self) -> bool {
        self.dura > 0 && ahora_ms() < self.desde + self.dura as u64
    }
}

/// **Pide fotograma** mientras algo se mueve. Solo lee el reloj.
pub(crate) fn anima() -> bool {
    let ahora = ahora_ms();
    ahora < VIVA_HASTA.load(Ordering::Relaxed) + FOTOGRAMA_MS && ahora.saturating_sub(PINTADO.load(Ordering::Relaxed)) >= FOTOGRAMA_MS
}

/// El compositor pinto un fotograma: la proxima peticion espera uno entero.
pub(crate) fn pinto() {
    PINTADO.store(ahora_ms(), Ordering::Relaxed);
}

/// Un entero de `a` a `b`, `k` milesimas (puede pasarse: el rebote).
pub(crate) fn entre(a: i32, b: i32, k: i32) -> i32 {
    bmo::entre_i(a, b, k)
}
