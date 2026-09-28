//! # bmo-sm86 -- la lengua de la RTX 3060, pura
//!
//! generacion: hijo -- de campos a palabras de SASS; no toca la tarjeta
//! capa: puro -- ni un `unsafe`, ni un aparato: lo usan el emisor de PROTON-X
//! y el banco del driver
//!
//! [carril]  VERDE
//!
//! Salio de `bmo-gpu-ga10x/src/sass/` el 28-09: el codificador (E2) es logica
//! pura, y el emisor de PROTON-X (Ring 3) no puede enlazar un driver (L8).
//! Aqui va lo de NVIDIA que no toca la tarjeta; el juez (J1) sigue en el
//! driver, que es quien sube los programas, y juzga este codificador en SUS
//! pruebas.

#![no_std]
#![forbid(unsafe_code)]

pub mod codifica;
