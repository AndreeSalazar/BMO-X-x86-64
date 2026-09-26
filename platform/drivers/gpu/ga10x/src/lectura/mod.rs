//! **LECTURA** -- LO QUE SE LEE ANTES DE MANDAR: quien es la tarjeta, como esta y lo que sobrevive.
//!
//! [carril]  VERDE
//!
//! Por que va junto: lee registros que el kernel ya leyo, o constantes; la unica escritura es la del aviso del VBLANK (E2), detras de su candado.
//!
//! Las reglas de todas las carpetas, y el mapa de la tarjeta: `ANATOMIA.md`,
//! al lado de `Cargo.toml`.

/// LA 3060 12G, y solo ella: la identidad que el kernel exige (2026-09-25).
pub mod identidad;
/// La temperatura y el enlace PCIe de la 3060, en solo lectura (2026-09-24).
pub mod salud;
/// E2: el VBLANK por interrupcion -- que registros y en que orden (2026-09-24).
pub mod vblank;
/// Lo que SOBREVIVE a un reinicio: el unico propietario de sus direcciones (26-09).
pub mod aon;
