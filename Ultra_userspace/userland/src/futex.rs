//! **EL FUTEX, desde Ring 3** (07-10): como `futex(2)` de Linux. Un
//! cerrojo LIBRE se toma con una atomica, sin llamar a nadie; solo para
//! ESPERAR se llama al kernel, y el que suelta despierta a los que esperan.
//!
//! ```text
//!    esperar(&palabra, visto, ms)   duerme si `palabra` aun vale `visto`
//!                                   (u32::MAX ms: sin plazo); vuelve en el
//!                                   acto si ya cambio
//!    despertar(&palabra, n)         despierta a lo mas n
//! ```
//!
//! Al volver de `esperar` (despertado, plazo cumplido, o porque ya habia
//! cambiado) se mira la palabra OTRA VEZ: el kernel no promete cual de las
//! tres fue. El juez, con banco: `bmo_futex`.

use core::sync::atomic::AtomicU32;

use crate::*;

/// Sin plazo.
pub const SIEMPRE: u32 = u32::MAX;

/// Por que no (espejo de `bmo_abi::...::FUTEX_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoFutex {
    Desalineada,
    NoEsRing3,
    /// Ya no valia lo visto: no se durmio (no es un error: mira otra vez).
    Cambio,
    Otro(u32),
}

/// **Esperar** mientras `palabra` valga `visto`, como mucho `ms`.
pub fn esperar(palabra: &AtomicU32, visto: u32, ms: u32) -> Result<(), NoFutex> {
    let st = invoke(CURRENT_TASK, OP_FUTEX_ESPERAR, palabra.as_ptr() as u64, visto as u64 | (ms as u64) << 32, 0);
    if st.ok() {
        return Ok(());
    }
    Err(match if st.flags == 0 { st.code } else { st.flags } {
        1 => NoFutex::Desalineada,
        2 => NoFutex::NoEsRing3,
        3 => NoFutex::Cambio,
        m => NoFutex::Otro(m),
    })
}

/// **Despertar** a lo mas `n` de los que esperan en `palabra`: cuantos.
pub fn despertar(palabra: &AtomicU32, n: u32) -> u32 {
    let st = invoke(CURRENT_TASK, OP_FUTEX_DESPERTAR, palabra.as_ptr() as u64, n as u64, 0);
    if st.ok() { st.value as u32 } else { 0 }
}
