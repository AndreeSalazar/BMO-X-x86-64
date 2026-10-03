//! **LA RAIZ EN LA PUERTA** -- lo que un proceso encerrado puede pedir. H3 de
//! `docs/plan/PLAN_HERMES.md`; la raiz misma vive en `task/raiz.rs`.
//!
//! [carril]  ROJO      lo que un proceso encerrado puede pedir del disco
//! [consumo] NADA      corre solo cuando una tarea cruza la puerta
//!
//! Tres cosas, y las tres son un NO o la forma de pedir uno:
//!
//! ```text
//!    fuera       el NO de una ruta que se sale de la raiz, con su motivo
//!    encerrado   pregunta del brazo que cubre las cuatro operaciones que ven
//!                el volumen ENTERO sin ruta (cursor, nombres, sellar, disco)
//!    raiz_hijo   TASK_OP_RAIZ_HIJO: "el siguiente hijo que lance vive aqui"
//! ```
//!
//! ** Vive aparte y no en `mod.rs` por L6b: es una sola pregunta --que puede
//! nombrar un proceso encerrado-- y repartida entre los brazos se perderia
//! la cuenta de cuales la hacen.

use super::*;
use bmo_raiz_juicio::NoRuta;
use crate::ring0::task::raiz;

/// **El NO de una ruta fuera de la raiz.** Una ruta que no se puede escribir
/// dentro NO se abre en otro sitio: no hay segunda oportunidad.
pub(super) fn fuera(pid: u32, n: NoRuta) -> BmoStatus {
    crate::ring0::cabina::warn("raiz", raiz::motivo(n), pid as u64);
    BmoStatus::err(cap::ERROR_PERMISSION_DENIED)
}

/// **Esta encerrado quien llama?** Para el brazo de las operaciones sin ruta.
pub(super) fn encerrado() -> bool {
    raiz::encerrado(scheduler::current_pid())
}

/// El NO de una operacion que ve el volumen entero, a un proceso encerrado.
/// El cursor de ESTRATOS lee los nombres de todo el disco; sellar y el disco
/// cambian el volumen entero. Ninguna lleva ruta, y por eso ninguna cabe en
/// una carpeta.
pub(super) fn no_sin_ruta() -> BmoStatus {
    let pid = scheduler::current_pid();
    crate::ring0::cabina::warn("raiz", "un proceso encerrado pidio una operacion que ve el volumen entero", pid as u64);
    BmoStatus::err(cap::ERROR_PERMISSION_DENIED)
}

/// **`TASK_OP_RAIZ_HIJO`**: la ruta del renglon (`TASK_OP_RUTA`) es la raiz
/// del SIGUIENTE hijo que lance quien llama, leida dentro de su propia raiz.
/// Se toma CRUDA: la raiz del padre la pone el juez, no el renglon.
pub(super) fn raiz_hijo() -> BmoStatus {
    let pid = scheduler::current_pid();
    match raiz::pedir_para_hijo(pid, ruta_tomar_cruda(pid)) {
        Ok(()) => {
            crate::ring0::cabina::info("raiz", "el siguiente hijo de este proceso nacera encerrado", pid as u64);
            BmoStatus::ok_value(0)
        }
        Err(n) => fuera(pid, n),
    }
}
