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
//! revise antes ([`crate::puerta::Puerta::leer_libreta`]) -- es de la puerta.
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
//! # Lo que falta, dicho: el pegamento del KERNEL (Ring 0)
//!
//! El cuerpo no escribe en memoria: lo apunta en un registro. Quien lo pasa
//! a la libreta es el pegamento de `bmo-gpu-ga10x` que pega el kernel (un
//! `FSETP.NAN` y un `@P STG` a la pagina de la libreta que es SUYA, con el
//! numero del PSO de la receta) y quien se la da a la app entre fotogramas.
//! Eso es Ring 0: se decide con el propietario. Hasta entonces la puerta
//! emite SIN libreta ([`crate::pso`]), y lo de aqui lo prueba el banco
//! (`pruebas_libreta.rs`): lo que apunta el simulador es lo que dice la CPU.
//! Encenderla cambia el codigo emitido: sube `vivo::VERSION_EMISOR`.

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

/// **Las hojas apuntadas de una libreta**, como la dara el kernel: una
/// palabra por PSO (su numero en la puerta), distinta de 0 si la 3060
/// apunto algo raro en el fotograma. Lo que no llega a palabra no cuenta.
pub fn apuntados(libreta: &[u8]) -> Vec<usize> {
    libreta.chunks_exact(4).enumerate().filter(|(_, w)| w.iter().any(|&b| b != 0)).map(|(k, _)| k).collect()
}
