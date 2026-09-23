//! Las tablas de SPIR-V: la forma de cada instruccion, sus nombres y las de
//! `GLSL.std.450`. GENERADO por `herramientas/tabla.py`; ver cada fichero.
//!
//! [consumo]  NADA   datos constantes

mod filas;
pub mod glsl;
pub mod op;

pub use filas::TABLA;
pub use glsl::GLSL450;
