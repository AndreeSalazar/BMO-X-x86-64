//! # bmo-verrano-trama -- VERRANO, su TRAMA
//!
//! generacion: hijo -- triangulos a pixeles con las reglas de D3D; no sabe quien dibuja ni con que GPU
//! capa: puro -- ni un `unsafe`, ni un aparato: cuentas sobre la memoria que le dan
//!
//! [carril]  VERDE     aritmetica pura
//!
//! **VC2 (11-10), lo decidio el propietario:** *"PROTON-X [...] solo le
//! emula a la ABI MUY PRECISAS [...] pero VERRANO DEVORA DirectX para poder
//! tener FPS NATIVOS"*. La trama de la casa -- la de D3D12 entera en la CPU,
//! que nacio en PROTON-X (P3b3, 27-09) -- es de VERRANO desde aqui; PROTON-X
//! la usa y la reexporta con su nombre de siempre (`bmo_proton_x::trama`,
//! `mezcla`, `stencil`, `cuadros`), asi que nadie que la llame cambio una
//! linea.
//!
//! ```text
//!    trama     triangulos a pixeles: el recorte, la cobertura, la
//!              perspectiva, la profundidad (P3b3, P3c4)
//!    cuadros   los pixeles en cuadros de 2x2 y en olas (E2.5)
//!    mezcla    la mezcla de D3D12 (N5.11)
//!    stencil   el stencil de D3D12
//!    juez      el de los `Draw` y los `Dispatch` de VERRANO: los dibuja
//!              ESTA trama (VC2 paso 2), y el Programa de la casa entra
//!              como etapa (`Casa`)
//!    estudio   el render de CALIDAD, al estilo RenderMan: trazado de
//!              caminos en la CPU (11-10)
//!    vivo      ESTUDIO en tiempo real: la luz que se recuerda (11-10)
//! ```
//!
//! Aparte de `bmo-verrano` por la memoria: esta pide `alloc` y el Programa
//! de PROMETEO; el VERRANO de base no pide nada (lo usan el driver de la
//! 3060 y el DIRECTOR, sin asignador).

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

/// La trama: triangulos a pixeles, con las reglas de D3D (P3b3).
pub mod trama;
/// Los pixeles en cuadros de 2x2 y en olas (E2.5).
pub mod cuadros;
/// La mezcla de D3D12 (N5.11).
pub mod mezcla;
/// El stencil de D3D12.
pub mod stencil;
/// VC2 paso 2: el juez de los `Draw` y los `Dispatch` de VERRANO, con ESTA
/// trama (una sola), y el Programa de la casa como etapa (`Casa`).
pub mod juez;
/// VERRANO ESTUDIO (11-10): el render de calidad al estilo RenderMan --
/// trazado de caminos en la CPU: luz de area, sombras suaves, luz
/// indirecta, metal y vidrio, la curva del cine --.
pub mod estudio;
/// ESTUDIO VIVO (11-10): la luz de ESTUDIO en tiempo real porque se
/// RECUERDA -- un cuarto de los pixeles por fotograma, la imagen que se
/// acumula, y la memoria de la luz del mundo que sobrevive a la camara --.
pub mod vivo;

#[cfg(test)]
mod pruebas_juez;
