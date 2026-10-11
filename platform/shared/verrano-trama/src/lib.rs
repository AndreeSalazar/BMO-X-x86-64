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
