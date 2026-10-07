//! **LAS PUERTAS DE LOS SUB-DIRECTORES** (H4.3 de `PLAN_LOS_DOCE_DIRECTORES`,
//! 07-10): una faena de la app, repartida en Ring 3 por los obreros.
//!
//! [carril]  ROJO      manda a OTROS nucleos al espacio de quien llama
//! [consumo] NADA      corre cuando una app reparte una faena suya
//!
//! ```text
//!    info        cuantos obreros sanos (las partes que caben son estos + 1)
//!    preparar    los bloques, sus bytes y las partes (dos numeros: ver
//!                bmo_orquesta::ring3::empaquetar)
//!    repartir    la funcion y su argumento: las partes 1..n a los obreros
//!    esperar     0 si sigue; ACABADA | las partes que la app rehace
//! ```
//!
//! ** Vive aparte (L6b): son las unicas puertas que mandan a otro nucleo al
//! espacio de quien llama. Lo que pisa el CPU para eso esta en
//! `plat/smp/ring3.rs`; el juez del pedido, con banco, en `bmo-orquesta`.

use super::*;
use crate::ring0::plat::smp::ring3;
use bmo_orquesta::ring3::{desempaquetar, NoRing3, ACABADA};

fn no(n: NoRing3) -> BmoStatus {
    BmoStatus::negado(n as u32, 0)
}

/// `SUB_INFO`: los obreros sanos ahora.
pub(super) fn info() -> BmoStatus {
    BmoStatus::ok_value(ring3::sanos() as u64)
}

/// `SUB_PREPARAR`: los bloques (uno por parte, seguidos) y lo empaquetado.
pub(super) fn preparar(bloques: u64, empaquetado: u64) -> BmoStatus {
    let Some((pid, _, _)) = scheduler::espacio_actual() else { return no(NoRing3::NoEsRing3) };
    let (bytes, partes) = desempaquetar(empaquetado);
    ring3::preparar(pid, bloques, bytes, partes);
    BmoStatus::ok_value(0)
}

/// `SUB_REPARTIR`: juzga y despierta. La parte 0 la corre quien llama.
pub(super) fn repartir(funcion: u64, arg: u64) -> BmoStatus {
    let Some((pid, cr3, gs)) = scheduler::espacio_actual() else { return no(NoRing3::NoEsRing3) };
    match ring3::repartir(pid, cr3, gs, funcion, arg) {
        Ok(faena) => BmoStatus::ok_value(faena),
        Err(n) => no(n),
    }
}

/// `SUB_ESPERAR`: si sigue en marcha, cede el turno (el sonido y los demas
/// hilos corren mientras) y dice 0.
pub(super) fn esperar() -> BmoStatus {
    let pid = scheduler::current_pid();
    match ring3::esperar(pid) {
        Ok(None) => {
            scheduler::yield_current();
            BmoStatus::ok_value(0)
        }
        Ok(Some(mal)) => BmoStatus::ok_value(ACABADA | mal),
        Err(n) => no(n),
    }
}
