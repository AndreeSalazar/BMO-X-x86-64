//! **LOS ARCOS en f32** (DL13, 09-10): arcotangente, arcoseno y arcocoseno,
//! recetas de `cuentas.rs` (las de Cephes, `atanf`, `asinf` y `acosf`, con
//! FFMA y sin saltos).
//!
//! ```text
//!    atan     |x| > tan(3pi/8): pi/2 + atan(-1/|x|); |x| > tan(pi/8): pi/4 +
//!             atan((|x| - 1)/(|x| + 1)); si no, atan(|x|) -- UN cociente
//!             (`cuentas::cociente`) con el numerador y el denominador que
//!             tocan --, el polinomio, y el signo de x. Desde |x| = 2^100,
//!             pi/2 (lo que es, en f32)
//!    asin     |x| > 1/2: pi/2 - 2 asin(raiz((1 - |x|)/2)); si no, el
//!             polinomio en |x|; el signo de x
//!    acos     x < -1/2: pi - 2 asin(raiz((1 + x)/2)); x > 1/2: 2 asin(raiz((1
//!             - x)/2)); si no, pi/2 - asin(x)
//!    la raiz  Newton con FFMA desde la semilla de los bits (0x5F3759DF), y
//!             una correccion con su residuo exacto: sin la unidad especial
//!    fuera    |x| > 1 en asin y acos: NaN; un NaN, NaN
//! ```

use crate::cuentas::{cociente, con_signo, horner, kf, Bits, Como, Corre, Cuentas, E, F, NAN, SIGNO, UNO};

/// atan(t) = t + t^3 P(t^2), |t| <= tan(pi/8) (Cephes `atanf`).
const ATAN: [f32; 4] = [8.053_744_495_38e-2, -1.387_768_560_32e-1, 1.997_771_064_78e-1, -3.333_294_915_39e-1];
/// asin(w) = w + w^3 P(w^2), w <= 1/2 (Cephes `asinf`).
const ASIN: [f32; 5] = [4.216_319_904_8e-2, 2.418_131_104_9e-2, 4.547_002_599_8e-2, 7.495_300_268_6e-2, 1.666_675_242_2e-1];
const TAN_3PI_8: f32 = 2.414_213_5;
const TAN_PI_8: f32 = 0.414_213_57;
const PI: f32 = core::f32::consts::PI;
const PI_2: f32 = core::f32::consts::FRAC_PI_2;
const PI_4: f32 = core::f32::consts::FRAC_PI_4;
/// Lo que les falta en f32: pi/2 - PI_2 y pi/4 - PI_4.
const PI_2_BAJO: f32 = -4.371_139e-8;
const PI_4_BAJO: f32 = -2.185_569_5e-8;
/// Desde aqui, atan(x) es pi/2 en f32 (y -1/x cabe en el inverso).
const ENORME: f32 = 1.267_650_6e30;

/// **atan(x).**
pub fn arcotangente<M: Cuentas>(m: &mut M, x: M::V) -> M::V {
    let uno = m.k(UNO);
    let a = m.bits(Bits::Y, x, E::K(!SIGNO));
    let enorme = m.compara(Como::Gt, F::R(a), kf(ENORME));
    let a = m.elige(enorme, E::K(ENORME.to_bits()), E::R(a));
    let grande = m.compara(Como::Gt, F::R(a), kf(TAN_3PI_8));
    let medio = m.compara(Como::Gt, F::R(a), kf(TAN_PI_8));
    // El numerador y el denominador de t.
    let am1 = m.suma(F::R(a), kf(-1.0));
    let ap1 = m.suma(F::R(a), kf(1.0));
    let num = m.elige(medio, E::R(am1), E::R(a));
    let num = m.elige(grande, E::K((-1.0f32).to_bits()), E::R(num));
    let den = m.elige(medio, E::R(ap1), E::R(uno));
    let den = m.elige(grande, E::R(a), E::R(den));
    let t = cociente(m, F::R(num), den, uno, 3);
    let z = m.mul(F::R(t), F::R(t));
    let p = horner(m, z, &ATAN);
    let q = m.mul(F::R(z), F::R(t));
    let y = m.fma(F::R(p), F::R(q), F::R(t));
    // + pi/4 o pi/2, en dos trozos: lo bajo antes (la resta con pi/4, cerca
    // de tan(pi/8), se come los bits de arriba).
    let bajo = m.elige(medio, E::K(PI_4_BAJO.to_bits()), E::K(0));
    let bajo = m.elige(grande, E::K(PI_2_BAJO.to_bits()), E::R(bajo));
    let y = m.suma(F::R(y), F::R(bajo));
    let y0 = m.elige(medio, E::K(PI_4.to_bits()), E::K(0));
    let y0 = m.elige(grande, E::K(PI_2.to_bits()), E::R(y0));
    let y = m.suma(F::R(y), F::R(y0));
    con_signo(m, y, x)
}

/// **La raiz de z** (0 <= z <= 1/2): 1/raiz(z) por Newton (tres pasos desde
/// la semilla de los bits), z por ella, y una correccion con el residuo
/// exacto. raiz(0) = 0.
pub(crate) fn raiz<M: Cuentas>(m: &mut M, z: M::V) -> M::V {
    // 0x5F3759DF - (z >> 1) = !(z >> 1) + 0x5F3759E0.
    let s = m.corre(Corre::Derecha, z, 1);
    let s = m.bits(Bits::Ox, s, E::K(u32::MAX));
    let mut y = m.entera(s, E::K(0x5F37_59E0));
    let h = m.mul(F::R(z), kf(0.5));
    for _ in 0..3 {
        // y += y (1/2 - h y^2).
        let r = m.mul(F::R(h), F::R(y));
        let e = m.fma(F::Menos(r), F::R(y), kf(0.5));
        y = m.fma(F::R(y), F::R(e), F::R(y));
    }
    let s = m.mul(F::R(z), F::R(y));
    let e = m.fma(F::Menos(s), F::R(s), F::R(z));
    let hy = m.mul(F::R(y), kf(0.5));
    m.fma(F::R(e), F::R(hy), F::R(s))
}

/// asin(w) para 0 <= w <= 1/2, con z = w^2 (o lo que la receta da por el).
fn asin_w<M: Cuentas>(m: &mut M, w: M::V, z: M::V) -> M::V {
    let p = horner(m, z, &ASIN);
    let q = m.mul(F::R(z), F::R(w));
    m.fma(F::R(p), F::R(q), F::R(w))
}

/// **asin(x)**, o **acos(x)** con `coseno`.
pub fn arcoseno_o_arcocoseno<M: Cuentas>(m: &mut M, x: M::V, coseno: bool) -> M::V {
    let a = m.bits(Bits::Y, x, E::K(!SIGNO));
    let grande = m.compara(Como::Gt, F::R(a), kf(0.5));
    // (1 - |x|)/2, exacto; o x^2.
    let zg = m.fma(F::R(a), kf(-0.5), kf(0.5));
    let zc = m.mul(F::R(a), F::R(a));
    let z = m.elige(grande, E::R(zg), E::R(zc));
    let s = raiz(m, z);
    let w = m.elige(grande, E::R(s), E::R(a));
    let y = asin_w(m, w, z);
    let v = if coseno {
        // Lo chico: pi/2 - asin(x). Lo grande: 2y, o pi - 2y si x < 0.
        let ys = con_signo(m, y, x);
        let chico = m.suma(kf(PI_2), F::Menos(ys));
        let doble = m.suma(F::R(y), F::R(y));
        let resto = m.fma(F::R(y), kf(-2.0), kf(PI));
        let negativo = m.compara(Como::Lt, F::R(x), kf(0.0));
        let g = m.elige(negativo, E::R(resto), E::R(doble));
        m.elige(grande, E::R(g), E::R(chico))
    } else {
        // pi/2 - 2y, con lo bajo de pi/2 antes (cerca de 1/2 se resta casi
        // todo).
        let g = m.fma(F::R(y), kf(-2.0), kf(PI_2_BAJO));
        let g = m.suma(F::R(g), kf(PI_2));
        let v = m.elige(grande, E::R(g), E::R(y));
        con_signo(m, v, x)
    };
    // Fuera de [-1, 1], NaN (un NaN ya lo es).
    let fuera = m.compara(Como::Gt, F::R(a), kf(1.0));
    m.elige(fuera, E::K(NAN), E::R(v))
}
