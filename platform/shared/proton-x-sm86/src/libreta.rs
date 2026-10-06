//! **LA LIBRETA DE LA 3060** (9d, 06-10) -- que la GPU misma apunte lo
//! RARO mientras dibuja, y que la CPU aprenda de eso entre fotogramas.
//!
//! [carril]  VERDE     unas instrucciones al final del cuerpo; nada si se
//!                     emite sin libreta (lo de hoy, ver abajo)
//! [cuesta]  DATO      una instruccion por componente de salida
//! [riesgo]  ESPEJO    la cuenta de la CPU ([`termometro`]) es la MISMA que
//!                     corre la 3060: sumas IEEE al mas cercano, y por 0
//! [consumo] NADA      la libreta la lee la CPU entre fotogramas, no la GPU
//!
//! Es de ESTA tarjeta, aislado aqui (pedido del propietario: "OJO aislar
//! eso", por si un dia hay una AMD): otro emisor (RDNA) tendria su libreta en
//! su crate, con sus instrucciones. Lo comun -- que un PSO apuntado se
//! revise antes ([`crate::puerta::Puerta::apunto`]) -- es de la puerta.
//!
//! # El TERMOMETRO
//!
//! ```text
//!    t = |o0| + |o1| + ... + |on|     cada componente ESCRITO de cada salida,
//!                                     en su orden (FADD con |.|)
//!    t = t * 0                        FMUL: +0 si todo es normal; NaN si una
//!                                     salida es NaN o infinita (o si la suma
//!                                     se pasa de f32::MAX: tambien raro)
//! ```
//!
//! Queda en el registro de DETRAS de las salidas (`R(4 * salidas)`), que el
//! EXIT tambien espera. Solo FADD y FMUL: las que ya corrieron en el metal.
//! Lo raro es donde la 3060 y la CPU mas se separan (la carga de un NaN, un
//! MUFU en su borde, un infinito que se resta): ahi el vigia
//! ([`crate::vivo::revisar`]) tiene que mirar YA, no dentro de 256 lotes.
//!
//! # El pegamento del KERNEL (Ring 0, con permiso del propietario, 06-10)
//!
//! El cuerpo no escribe en memoria: lo deja en un registro, y la receta dice
//! cual (`+92`). El pegamento que pone el kernel (`bmo_gpu_ga10x::libreta`)
//! hace `FSETP.NAN` y `@P STG` de una constante a UNA palabra SUYA, que pone
//! a 0 antes de cada dibujo y lee despues; si la 3060 apunto, el `Ok` de la
//! receta lleva el bit `cubo::LIBRETA_RARO`, y solo lo ve la app que la
//! mando. AISLADO: ni la app elige donde ni que se escribe, ni otra app ve
//! lo de esta. Si un PSO con libreta no cabe en el pegado, la puerta lo
//! vuelve a emitir sin ella. Encenderla cambio el codigo: `VERSION_EMISOR` 2.

use alloc::vec::Vec;

use bmo_sm86::codifica::{self as c, Fuente};

use crate::planifica::Meta;
use crate::Clase;

/// El registro del termometro: el de detras de las salidas.
pub fn registro(salidas: usize) -> u8 {
    (4 * salidas) as u8
}

/// **Las instrucciones del termometro** en `t`, sobre los registros de
/// salida `escritas` (en su orden), sin control (lo pone el planificador).
pub(crate) fn instrucciones(escritas: &[u8], t: u8) -> Vec<((u64, u64), Meta)> {
    let fma = |w: (u64, u64), lee: [Option<u8>; 3]| (w, Meta::de(Clase::Fma, Some(t), lee));
    let mut v = Vec::with_capacity(escritas.len() + 1);
    match escritas {
        // Sin salidas escritas: siempre +0 (nada que mirar).
        [] => v.push(((c::mov(t, Fuente::Imm(0), 0)), Meta::de(Clase::Alu, Some(t), [None; 3]))),
        // Una: |o0| + -0 (sumar -0 no cambia nada, como `Abs`).
        [a] => v.push(fma(c::fadd(t, c::abs(*a), Fuente::Imm(0x8000_0000), false, 0), [Some(*a), None, None])),
        [a, b, resto @ ..] => {
            v.push(fma(c::fadd(t, c::abs(*a), c::abs(*b), false, 0), [Some(*a), Some(*b), None]));
            for &x in resto {
                v.push(fma(c::fadd(t, c::r(t), c::abs(x), false, 0), [Some(t), Some(x), None]));
            }
        }
    }
    if !escritas.is_empty() {
        v.push(fma(c::fmul(t, c::r(t), Fuente::Imm(0), false, 0), [Some(t), None, None]));
    }
    v
}

/// **Lo que tiene que apuntar la 3060**, contado en la CPU: el termometro
/// de esas salidas (sus bits, en el orden de `escritas`). Las sumas de f32
/// de Rust son las de IEEE al mas cercano, como el FADD de la 3060.
pub fn termometro(salidas: &[u32]) -> u32 {
    let t = match salidas {
        [] => return 0,
        [a] => f32::from_bits(*a).abs() + -0.0,
        [a, b, resto @ ..] => resto.iter().fold(f32::from_bits(*a).abs() + f32::from_bits(*b).abs(), |t, x| t + f32::from_bits(*x).abs()),
    };
    (t * 0.0).to_bits()
}

/// Si un termometro dice RARO (NaN).
pub fn raro(bits: u32) -> bool {
    f32::from_bits(bits).is_nan()
}
