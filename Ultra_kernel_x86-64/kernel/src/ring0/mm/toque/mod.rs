//! **EL TOQUE: la RAM que un programa USA, frente a la que tiene** (EL0 de
//! `docs/plan/PLAN_EL_TOQUE.md`, 15-10).
//!
//! [carril]  VERDE     hoy solo cuenta: la carpeta de carriles que pidio el
//!                     propietario ("tienen que tener carril ESPECIFICO"),
//!                     con el verde primero
//! [consumo] NADA      corre al morir un programa y cuando alguien pregunta
//!
//! ```text
//!    verde.rs     LA CUENTA (EL0): de las paginas de usuario de un espacio,
//!                 cuantas estan, cuantas TOCO la CPU (bit A) y cuantas
//!                 ESCRIBIO (bit D). Solo lee las tablas
//!    amarilla.rs  (EL4, despues) el barrido de lo frio
//!    roja.rs      (EL1, despues) el fallo de pagina que materializa
//! ```
//!
//! Medir antes de construir: si un programa toca casi todo lo que tiene,
//! este plan vale poco, y la cuenta lo dira.

pub mod verde;
