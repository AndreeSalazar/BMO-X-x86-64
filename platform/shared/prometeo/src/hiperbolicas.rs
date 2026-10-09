//! **LOS HIPERBOLICOS en f32** (DL13, 09-10): senh, cosh y tanh, recetas de
//! `cuentas.rs` (las de Cephes, `sinhf`, `coshf` y `tanhf`, con FFMA y sin
//! saltos).
//!
//! ```text
//!    cosh    e^|x|/2 + e^-|x|/2: los dos con el MISMO n y r (e^-a = e^-r
//!            2^-n), cada uno escalado en su sitio -- sin inverso --
//!    senh    |x| < 1: su polinomio (restar dos e^x casi iguales perderia
//!            todo); si no, e^|x|/2 - e^-|x|/2; el signo de x
//!    tanh    |x| < 0.625: su polinomio; si no, 1 - 2/(e^2|x| + 1) (UN
//!            cociente); desde |x| = 9.5, 1 (lo que es, en f32); el signo
//!    lo grande   |x| se queda en 89.5 (el infinito ya llego); un NaN, NaN
//! ```

use crate::cuentas::{cociente, con_signo, horner, kf, potencia, Bits, Como, Cuentas, E, F, SIGNO, UNO};
use crate::exponencial::{e_r, escala, partes};

/// senh(x) = x + x^3 P(x^2), |x| < 1 (Cephes `sinhf`).
const SENH: [f32; 3] = [2.037_219_129_45e-4, 8.330_283_762_39e-3, 1.666_671_602_11e-1];
/// tanh(x) = x + x^3 P(x^2), |x| < 0.625 (Cephes `tanhf`).
const TANH: [f32; 5] = [-5.704_988_727_45e-3, 2.063_908_879_54e-2, -5.373_971_555_31e-2, 1.333_144_220_36e-1, -3.333_328_194_22e-1];
const TECHO: f32 = 89.5;

/// e^a/2 y e^-a/2 (0 <= a <= 89.5).
fn mitades<M: Cuentas>(m: &mut M, a: M::V, uno: M::V) -> (M::V, M::V) {
    let (r, n) = partes(m, a);
    let z = m.mul(F::R(r), F::R(r));
    let ep = e_r(m, r, z, uno, false);
    let en = e_r(m, r, z, uno, true);
    // e^a/2 = e^r 2^(n-1), en dos mitades (n llega a 129).
    let n1 = m.entera(n, E::K(u32::MAX));
    let mas = escala(m, ep, n1, uno);
    // e^-a/2 = e^-r 2^(-n-1): desde n = 125 no cuenta al lado del otro, y
    // 2^-126 es el ultimo normal. -n - 1 = !n.
    let tope = m.compara_entero(Como::Gt, n, E::K(125));
    let n = m.elige(tope, E::K(125), E::R(n));
    let mn = m.bits(Bits::Ox, n, E::K(u32::MAX));
    let s = potencia(m, mn, uno);
    let menos = m.mul(F::R(en), F::R(s));
    (mas, menos)
}

/// |x|, con su techo.
fn abs_techo<M: Cuentas>(m: &mut M, x: M::V, techo: f32) -> M::V {
    let a = m.bits(Bits::Y, x, E::K(!SIGNO));
    let alto = m.compara(Como::Gt, F::R(a), kf(techo));
    m.elige(alto, E::K(techo.to_bits()), E::R(a))
}

/// **cosh(x).**
pub fn coseno_h<M: Cuentas>(m: &mut M, x: M::V) -> M::V {
    let uno = m.k(UNO);
    let a = abs_techo(m, x, TECHO);
    let (mas, menos) = mitades(m, a, uno);
    m.suma(F::R(mas), F::R(menos))
}

/// **senh(x).**
pub fn seno_h<M: Cuentas>(m: &mut M, x: M::V) -> M::V {
    let uno = m.k(UNO);
    let a = abs_techo(m, x, TECHO);
    let (mas, menos) = mitades(m, a, uno);
    let lejos = m.suma(F::R(mas), F::Menos(menos));
    let z = m.mul(F::R(a), F::R(a));
    let p = horner(m, z, &SENH);
    let q = m.mul(F::R(z), F::R(a));
    let cerca = m.fma(F::R(p), F::R(q), F::R(a));
    let chico = m.compara(Como::Lt, F::R(a), kf(1.0));
    let v = m.elige(chico, E::R(cerca), E::R(lejos));
    con_signo(m, v, x)
}

/// **tanh(x).**
pub fn tangente_h<M: Cuentas>(m: &mut M, x: M::V) -> M::V {
    let uno = m.k(UNO);
    let a = abs_techo(m, x, 9.5);
    // Lejos: 1 - 2/(e^2a + 1).
    let dos_a = m.suma(F::R(a), F::R(a));
    let (r, n) = partes(m, dos_a);
    let z = m.mul(F::R(r), F::R(r));
    let e = e_r(m, r, z, uno, false);
    let s = potencia(m, n, uno);
    let e = m.mul(F::R(e), F::R(s));
    let den = m.suma(F::R(e), F::R(uno));
    let q = cociente(m, kf(2.0), den, uno, 3);
    let lejos = m.suma(F::R(uno), F::Menos(q));
    // Cerca: el polinomio.
    let z = m.mul(F::R(a), F::R(a));
    let p = horner(m, z, &TANH);
    let q = m.mul(F::R(z), F::R(a));
    let cerca = m.fma(F::R(p), F::R(q), F::R(a));
    let chico = m.compara(Como::Lt, F::R(a), kf(0.625));
    let v = m.elige(chico, E::R(cerca), E::R(lejos));
    con_signo(m, v, x)
}
