//! **LOS SUB-DIRECTORES, desde Ring 3** (H4.3 de `PLAN_LOS_DOCE_DIRECTORES`,
//! 07-10): una faena de la app repartida por los obreros del kernel, cada
//! uno en Ring 3 con el espacio de quien llama (despues de `smp all`).
//!
//! ```text
//!    info()                       obreros sanos (las partes: estos + 1)
//!    preparar(bloques, paquete)   los bloques y `empaquetar(bytes, partes)`
//!    repartir(funcion, arg)       las partes 1..n a los obreros
//!    esperar()                    None si sigue; Some(lo que fallo)
//! ```
//!
//! El juez del pedido y lo de empaquetar viven en `bmo_orquesta::ring3`;
//! aqui solo la puerta.

use crate::*;

/// El bit de "ya acabo" en lo que contesta `esperar` (espejo de
/// `bmo_orquesta::ring3::ACABADA`).
const ACABADA: u64 = 1 << 63;

/// **Cuantos obreros sanos hay** (0: ninguno en pie).
pub fn info() -> u32 {
    let st = invoke(CURRENT_TASK, OP_SUB_INFO, 0, 0, 0);
    if st.ok() { st.value as u32 } else { 0 }
}

/// **Preparar**: los bloques (uno por parte, seguidos) y lo empaquetado.
pub fn preparar(bloques: u64, empaquetado: u64) -> bool {
    invoke(CURRENT_TASK, OP_SUB_PREPARAR, bloques, empaquetado, 0).ok()
}

/// **Repartir** las partes `1..n`: el numero de la faena, o el motivo del
/// NO (`bmo_orquesta::ring3::NoRing3`).
pub fn repartir(funcion: u64, arg: u64) -> Result<u64, u32> {
    let st = invoke(CURRENT_TASK, OP_SUB_REPARTIR, funcion, arg, 0);
    if st.ok() { Ok(st.value) } else { Err(if st.flags == 0 { st.code } else { st.flags }) }
}

/// **Esperar**: `None` si sigue (el kernel ya cedio el turno), o la mascara
/// de las partes que NO salieron bien (las rehace quien llama). Un NO del
/// kernel (no hay faena de este proceso) es `Some(todo)`: se rehace todo.
pub fn esperar() -> Option<u64> {
    let st = invoke(CURRENT_TASK, OP_SUB_ESPERAR, 0, 0, 0);
    if !st.ok() {
        return Some(!ACABADA);
    }
    if st.value & ACABADA != 0 { Some(st.value & !ACABADA) } else { None }
}
