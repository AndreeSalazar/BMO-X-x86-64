//! **EL PORQUE DEL ULTIMO NO de la puerta de las apps** (12-10) -- la mitad
//! del kernel de `bmo_gpu_ga10x::porque`: cada camino que niega una receta
//! apunta aqui SU porque (no solo el `motivo 79` de siempre), y PROTON-X lo
//! lee con `INFO_GPU_PORQUE` tras un NO y lo dice con palabras.
//!
//! [carril]  VERDE     dos palabras atomicas; nada toca la tarjeta
//! [consumo] NADA      una escritura por NO
//!
//! Lo lee SOLO quien recibio el NO (como la libreta): con el porque va el
//! pid de quien mando la receta, y a otro pid se le contesta 0. Es lo que
//! `PLAN_LA_LENTE` llama LE3 (el diario de los NO), para la 3060: el ultimo,
//! con su motivo, sin pasar por `cabina fallos`.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use bmo_gpu_ga10x::porque::{empaquetar, Porque};

use super::IOMMU_NO_BLUR_PREPARAR;

/// El ultimo porque (`porque::empaquetar`; 0 = ninguno) y de quien.
static ULTIMO: AtomicU64 = AtomicU64::new(0);
static ULTIMO_PID: AtomicU32 = AtomicU32::new(0);

/// Apuntar un NO (de quien llama ahora: la receta corre en su syscall).
pub(super) fn apuntar(p: Porque, sub: u8, detalle: u64) {
    ULTIMO_PID.store(crate::ring0::task::scheduler::current_pid(), Ordering::Release);
    ULTIMO.store(empaquetar(p, sub, detalle as u32), Ordering::Release);
}

/// Apuntar y devolver el `motivo 79`: el NO de casi todos los caminos.
pub(super) fn no(p: Porque, sub: u8, detalle: u64) -> u32 {
    apuntar(p, sub, detalle);
    IOMMU_NO_BLUR_PREPARAR
}

/// El `motivo 79`, para quien ya apunto su porque.
pub(super) fn motivo() -> u32 {
    IOMMU_NO_BLUR_PREPARAR
}

/// **`INFO_GPU_PORQUE`**: el ultimo porque, si es de quien pregunta; 0 si no.
pub fn info() -> u64 {
    let x = ULTIMO.load(Ordering::Acquire);
    if ULTIMO_PID.load(Ordering::Acquire) == crate::ring0::task::scheduler::current_pid() {
        x
    } else {
        0
    }
}
