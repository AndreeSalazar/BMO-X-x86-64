//! **LA ALARMA, desde Ring 3** (07-10, EXPROPIAR): como un signal de reloj
//! de Linux, pero con lo minimo. Cada `ms`, si el tick pilla a esta tarea en
//! Ring 3 y fuera de su PUERTA, el kernel deja en el BUZON el RIP que
//! llevaba y la manda a la puerta. La puerta guarda todo, hace lo suyo y
//! salta al RIP del buzon. El juez, con banco: `bmo_alarma`.

use crate::*;

/// Por que no (espejo de `bmo_abi::...::ALARMA_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoAlarma {
    FueraDeRing3,
    PuertaMala,
    BuzonMalo,
    PeriodoMalo,
    BuzonNoEscribible,
    Otro(u32),
}

/// **Armar** la alarma de esta tarea: la puerta `[inicio, fin)`, el buzon
/// (8 bytes alineados, escribibles) y cada `ms` (0 la apaga).
pub fn armar(inicio: u64, fin: u64, buzon: u64, ms: u32) -> Result<(), NoAlarma> {
    let bytes = fin.saturating_sub(inicio).min(u32::MAX as u64);
    let st = invoke(CURRENT_TASK, OP_ALARMA, inicio, buzon, bytes | (ms as u64) << 32);
    if st.ok() {
        return Ok(());
    }
    Err(match if st.flags == 0 { st.code } else { st.flags } {
        1 => NoAlarma::FueraDeRing3,
        2 => NoAlarma::PuertaMala,
        3 => NoAlarma::BuzonMalo,
        4 => NoAlarma::PeriodoMalo,
        5 => NoAlarma::BuzonNoEscribible,
        m => NoAlarma::Otro(m),
    })
}
