//! **MOTORES** -- LOS MOTORES: canales, copiador, contexto grafico, computo, SASS y la clase 3D.
//!
//! [carril]  AMARILLO
//!
//! Por que va junto: cada trabajo lo paga un semaforo que solo escribe el motor.
//!
//! Las reglas de todas las carpetas, y el mapa de la tarjeta: `ANATOMIA.md`,
//! al lado de `Cargo.toml`.

/// L1d2b: el canal AMPERE_CHANNEL_GPFIFO_A, sus 368 B exactos (2026-09-24).
pub mod canal;
/// L1d2d y L1d3: el copiador y la primera copia VRAM a VRAM (2026-09-24).
pub mod copia;
/// M5 G0: los buferes de contexto que pide el motor grafico (2026-09-24).
pub mod gr;
pub mod computo;
pub mod sombreador;
pub mod tresde;
