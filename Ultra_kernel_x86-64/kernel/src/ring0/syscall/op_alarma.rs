//! **LA PUERTA DE LA ALARMA** (07-10, EXPROPIAR): una tarea arma su alarma
//! de reloj. Lo que hace el tick, en `task/alarma.rs`; el juez, en
//! `bmo-alarma`, con banco.
//!
//! [carril]  AMARILLO  valida y apunta; el RIP lo cambia el tick
//! [consumo] NADA      una vez, al armar
//!
//! ```text
//!    TASK_OP_ALARMA(puerta, buzon, empaquetar(bytes de la puerta, ms))
//!    ms = 0 la apaga
//! ```
//!
//! ** Tres argumentos: `rdx`, `r10` y `r8` (el tercero no pasa por
//! `invoke_current_task`, que solo lleva dos).

use super::*;

pub(super) fn armar(puerta: u64, buzon: u64, paquete: u64) -> BmoStatus {
    // SAFETY: en una puerta (SYSCALL con IF a cero): la promesa de
    // `current_tid_en_trap`.
    let tid = unsafe { scheduler::current_tid_en_trap() };
    match crate::ring0::task::alarma::armar(tid, puerta, buzon, paquete) {
        Ok(()) => BmoStatus::ok_value(0),
        Err(n) => BmoStatus::negado(n as u32, 0),
    }
}
