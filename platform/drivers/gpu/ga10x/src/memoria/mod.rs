//! **MEMORIA** -- LA MEMORIA DE LA TARJETA: la ventana PRAMIN y las tablas de la MMU.
//!
//! [carril]  ROJO
//!
//! Por que va junto: una PTE mal escrita da a un motor memoria que no es suya.
//!
//! Las reglas de todas las carpetas, y el mapa de la tarjeta: `ANATOMIA.md`,
//! al lado de `Cargo.toml`.

/// L1c2: la CPU escribe en la VRAM por la ventana PRAMIN, sin pisar nada (2026-09-24).
pub mod vram;
/// L1d: el formato de la MMU (PDE, PTE, los cinco niveles) y mapear una pagina (2026-09-24).
pub mod mmu;
