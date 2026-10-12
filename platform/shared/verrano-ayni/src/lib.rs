//! # bmo-verrano-ayni -- AYNI: que la GPU gaste MENOS
//!
//! generacion: hijo -- decide cuanto sombrear cada baldosa; no sabe quien dibuja ni con que GPU
//! capa: puro -- ni un `unsafe`, ni un aparato: cuentas sobre la memoria que le dan
//!
//! [carril]  VERDE     aritmetica pura
//!
//! **12-10, lo pidio el propietario:** *"no es mi objetivo FPS, ya con FSR de
//! AMD es suficiente; lo que yo quiero es REDUCIR EL COSTO de GPU"*, y
//! despues: *"la GPU solo se encarga de pixeles gordos, pero la CPU es la
//! cabeza total"*. El plan entero: `docs/plan/PLAN_AYNI.md`.
//!
//! ```text
//!    pacha     los RASGOS de cada baldosa de 16x16: la luz, el contraste,
//!              el borde, el detalle y lo que cambio desde el fotograma de
//!              antes (la 4D), mas lo que solo PROTON-X sabe
//!    maestra   la LECCION: de una baldosa sombreada entera, cuanto error
//!              habria dado cada tasa gruesa (un pixel repetido por cuadro)
//!    uma       la red DIMINUTA (8 -> 16 -> 4) que predice ese error antes
//!              de dibujar. Nace sabiendo lo basico (`semilla`) y afina con
//!              el juego
//!    mano      la 5D: donde va a parar el puntero y con que CONFIANZA.
//!              Para ahorrar o preparar, NUNCA para actuar
//!    ayni      el fotograma: mirar lo dibujado, aprender de las maestras y
//!              decidir el mapa del siguiente, con las reglas que la red no
//!              rompe
//! ```
//!
//! Lo que sale es un MAPA: una [`Tasa`] por baldosa. La 3060 lo obedece por
//! hardware (la imagen de tasas de AMPERE_B); la trama de la CPU, por
//! software. Ninguno de los dos sabe por que: la cabeza es la CPU.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod ayni;
pub mod maestra;
pub mod mano;
pub mod pacha;
pub mod semilla;
pub mod uma;

#[cfg(test)]
mod pruebas;

pub use ayni::{Ayni, Config, Cuentas};
pub use mano::{Muestra, Prediccion};
pub use pacha::{Baldosa, Extra, Imagen, Rasgos, LADO};
pub use uma::Uma;

/// **Cuantas veces corre el sombreador de pixeles en una baldosa.** Los
/// nombres son los de la tabla de AMPERE_B (`clc797.h`,
/// `..._INDEX_TO_RATE_A`): `X2x1` es UNA invocacion por cada 2 pixeles de
/// ancho y 1 de alto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Tasa {
    /// Una por pixel: lo de siempre, sin AYNI.
    X1 = 0,
    X2x1 = 1,
    X1x2 = 2,
    X2x2 = 3,
    X4x4 = 4,
    /// Ninguna: el pixel de antes se queda (`PS_X0_CULL_RASTER_PIXELS`).
    /// Solo cuando quien dibuja GARANTIZA que nada cambio ahi (A2).
    Cero = 5,
}

impl Tasa {
    /// El cuadro que cubre una invocacion, `(ancho, alto)`.
    pub const fn bloque(self) -> (usize, usize) {
        match self {
            Tasa::X1 | Tasa::Cero => (1, 1),
            Tasa::X2x1 => (2, 1),
            Tasa::X1x2 => (1, 2),
            Tasa::X2x2 => (2, 2),
            Tasa::X4x4 => (4, 4),
        }
    }

    /// Invocaciones del sombreador en una baldosa entera de 16x16.
    pub const fn invocaciones(self) -> u32 {
        match self {
            Tasa::Cero => 0,
            t => {
                let (w, h) = t.bloque();
                (pacha::PIX / (w * h)) as u32
            }
        }
    }
}
