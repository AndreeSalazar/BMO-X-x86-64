//! **SPIR-V a SM86: el emisor de la RTX 3060** (`PLAN_LA_LENGUA_DE_LA_3060`).
//!
//! Hoy tiene UNA pieza, la casilla E1: [`check`], el SUBCONJUNTO de SPIR-V
//! que este emisor promete traducir. Lo que no entra se rechaza CON MOTIVO,
//! como en S2, y ANTES de emitir una sola instruccion:
//!
//! ```text
//!    .spv --> lector (S1) --> juez neutro (S2, con la etapa pedida)
//!                                  |
//!                                  v
//!                       SUBCONJUNTO SM86 (E1, aqui)     "SM86_V1: <motivo>"
//!                                  |
//!                                  v
//!                       el emisor (E3) ... el juez del SASS (J1) ... el BSF
//! ```
//!
//! Por que es un crate aparte y no un fichero del juez: el lector y el juez de
//! SPIR-V son NEUTROS (no nombran ninguna maquina, el guardian `isa` lo
//! vigila), y una AMD tendria su propio subconjunto con SUS reglas. El lector
//! no se entera de que la 3060 existe.
//!
//! La API habla ingles (decision del propietario, 23-09, como el resto de
//! `lang/spirv/`); los comentarios y los textos de pantalla, castellano.
//!
//! [consumo]  NADA   juzga cuando se le pide; no hay estado vivo
//!
//! capa: puro -- logica sin hardware, `no_std` sin `alloc` y sin un solo
//! `unsafe` (`forbid` abajo lo garantiza), probada en el anfitrion.

#![no_std]
#![forbid(unsafe_code)]

mod subconjunto;

pub use bmo_spirv_front::Stage;
pub use subconjunto::{check, Fit, Reason, Refusal, SLOTS};
