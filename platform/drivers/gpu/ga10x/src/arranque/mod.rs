//! **ARRANQUE** -- EL ARRANQUE Y EL APAGADO DEL GSP: firmware firmado, falcons, la WPR2.
//!
//! [carril]  ROJO
//!
//! Por que va junto: aqui se decide que firmware corre en la tarjeta y que region protegida monta: un error aqui NO se ve en esta sesion, se ve en la SIGUIENTE.
//!
//! Las reglas de todas las carpetas, y el mapa de la tarjeta: `ANATOMIA.md`,
//! al lado de `Cargo.toml`.

/// M0d3: el DMA de un falcon, la prueba de fuego de la traduccion (2026-09-24).
pub mod falcon;
/// L0a: la VBIOS leida -- donde esta FWSEC y que firma pide (2026-09-24).
pub mod vbios;
/// L0b: FWSEC-FRTS preparado -- la orden y la firma, sobre bytes (2026-09-24).
pub mod fwsec;
/// L0c1: el booter y el bootloader RISC-V, leidos sobre bytes (2026-09-24).
pub mod booter;
/// L0c1: las secciones del GSP-RM, sin traerse sus 63 MB (2026-09-24).
pub mod elf;
/// L0c1: el reparto de la VRAM y la `GspFwWprMeta` (2026-09-24).
pub mod wpr;
/// L0c3a: los argumentos de LIBOS, los logs, `rmargs` y las colas (2026-09-24).
pub mod libos;
/// L0c4b2b: las ordenes del secuenciador que pide el GSP (2026-09-24).
pub mod secuenciador;
/// L0c4b2c: correr el secuenciador, por tramos y solo en el falcon del GSP (2026-09-24).
pub mod correr;
/// L0c5: apagar el GSP en orden (2026-09-25).
pub mod descarga;
