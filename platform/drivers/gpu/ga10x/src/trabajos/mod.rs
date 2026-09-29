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
pub mod imagen;
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
/// VERRANO E5: el pegamento de los programas EMITIDOS (lo que rodea al
/// cuerpo que sale de PROTON-X: cargas, salidas y su SPH; 2026-09-28).
pub mod pegamento;
/// P3b4b: el destino de la app -- la 3060 dibuja en la RAM de un proceso
/// (su back buffer), prestada por la IOMMU (2026-09-28).
pub mod destino;
/// P3b4c: la PROFUNDIDAD en la 3060 -- la superficie ZF32 en VRAM y la
/// regla de D3D12, la prueba la hace el hardware (2026-09-28).
pub mod profundidad;
/// P3b4c.6b: la SOMBRA del color -- con Z, el color va en bloque a la VRAM
/// y el motor de copia lo lleva al destino pitch (2026-09-29).
pub mod sombra;
/// P3b4c.8 T0: las TEXTURAS de la 3060 -- sus descriptores (TIC y TSC) y
/// las piscinas; el `TEX` y el kernel, despues (2026-09-29).
pub mod texturas;
/// P3b4c: la RECETA (VRN2) -- lo que manda una APP: los cuerpos y como se
/// cargan; el pegamento lo pone el kernel (2026-09-28).
pub mod receta;
