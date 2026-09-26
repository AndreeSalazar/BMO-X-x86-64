//! **TRABAJOS** -- LOS TRABAJOS: lo que la 3060 dibuja o calcula, cada uno con su juez en la CPU.
//!
//! [carril]  VERDE
//!
//! Por que va junto: empujes, QMD y comprobaciones sobre bytes; lo que tocan de la tarjeta lo tocan por `motores` y `memoria`.
//!
//! Las reglas de todas las carpetas, y el mapa de la tarjeta: `ANATOMIA.md`,
//! al lado de `Cargo.toml`.

pub mod lienzo;
pub mod blur;
pub mod fractal;
pub mod triangulo;
pub mod raster;
pub mod color3d;
pub mod giro;
pub mod pantalla;
pub mod video;
/// El volcado por la 3060: el motor de copia lleva el lienzo del escritorio
/// a la pantalla (compositor por GPU, paso 1; 2026-09-25).
pub mod volcado;
pub mod escena;
/// X5: el cubo del estudio D3D por la 3060, sin Windows (2026-09-25).
pub mod cubo;
/// VERRANO V0: la tuberia FIJA -- dos programas que no cambian y los datos
/// en un buffer (2026-09-25).
pub mod tuberia;
/// VERRANO V1b: el anillo -- la CPU prepara el fotograma N+1 mientras la
/// 3060 dibuja el N (2026-09-26).
pub mod anillo;
