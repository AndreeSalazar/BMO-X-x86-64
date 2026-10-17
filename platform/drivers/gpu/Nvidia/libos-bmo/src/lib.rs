//! LIBOS NVIDIA -- interpretar copias tiene un propietario, separado de GA10x.
//! capa: puro -- recibe bytes; no conoce al kernel ni adquiere autoridad de aparato
//! [carril] VERDE
//! [consumo] NADA -- consultas explicitas sobre copias del llamador
//! [cuesta] NADA -- diagnostico sobre bytes; no cambia el estado de la tarjeta
//!
//! Origen: lectores agregados en f153f3d, OpenRM 570.144 / libos-v3.1.0.
//! Se conserva su contrato. El nombre no implica implementar LIBOS ni que
//! exista un diccionario disponible de la misma build del firmware.

#![no_std]
#![forbid(unsafe_code)]

pub mod anillo;
pub mod diccionario;
mod fuente;

pub use anillo::{resumen, resumen_put, Diccionario, Error, Lector, Registro, Resumen, MAX_ARGS};
pub use fuente::Fuente;
