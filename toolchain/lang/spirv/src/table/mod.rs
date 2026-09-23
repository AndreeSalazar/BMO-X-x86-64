//! Las tablas de SPIR-V: la forma de cada instruccion, sus nombres y las de
//! `GLSL.std.450`. GENERADO por `herramientas/table.py`; ver cada fichero.
//!
//! [consumo]  NADA   datos constantes

mod rows;
pub mod glsl;
pub mod op;

pub use rows::TABLE;
pub use glsl::GLSL450;
