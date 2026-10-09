//! **EXP2, LOG2 Y e^x en f32** (DL13, 09-10): recetas de `cuentas.rs`.
//!
//! ```text
//!    exp2      x = n + f, n el entero mas cercano (la MAGIA) y f en
//!              [-1/2, 1/2] EXACTO; 2^f por el polinomio de Cephes (`exp2f`)
//!              con FFMA; y por 2^n en DOS mitades (cada una normal), asi
//!              que el unico redondeo de la escala es el del final: un
//!              subnormal sale bien redondeado. x se lleva antes a [-152,
//!              129]: lo de fuera da lo mismo (0 o infinito). 2^n de un
//!              entero, EXACTO
//!    log2      x = m 2^e con m en [raiz(1/2), raiz(2)) (el truco de los bits
//!              de musl), un subnormal por 2^23 antes; log(m) por el
//!              polinomio de Cephes (`logf`), y la suma de Cephes que no
//!              pierde bits: e + z + y + (z + y)(log2(e) - 1). log2 de una
//!              potencia de dos, EXACTO. Negativos y NaN, NaN; 0, -inf; inf,
//!              inf
//!    e^a       (para los hiperbolicos) a = n ln2 + r con ln2 en dos trozos
//!              (Cephes `expf`), e^r por su polinomio, y la escala en dos
//!              mitades
//! ```

use crate::cuentas::{a_float, horner, kf, potencia, Bits, Como, Corre, Cuentas, E, F, INFINITO, MAGIA, NAN, UNO};

/// 2^f = 1 + f P(f) en [-1/2, 1/2] (Cephes `exp2f`).
const EXP2: [f32; 6] = [1.535_336_188_319_5e-4, 1.339_887_440_266_574e-3, 9.618_437_357_674_64e-3, 5.550_332_471_162_809e-2, 2.402_264_791_363_012e-1, 6.931_472_028_550_421e-1];
/// log(1 + z) = z - z^2/2 + z^3 P(z) (Cephes `logf`).
const LOG: [f32; 9] = [7.037_683_629_2e-2, -1.151_461_031_0e-1, 1.167_699_874_0e-1, -1.242_014_084_6e-1, 1.424_932_278_7e-1, -1.666_805_766_5e-1, 2.000_071_476_5e-1, -2.499_999_399_3e-1, 3.333_333_117_4e-1];
/// log2(e) - 1.
const LOG2EA: f32 = 0.442_695_04;
/// e^r = 1 + r + r^2 P(r) (Cephes `expf`).
pub(crate) const EXPE: [f32; 6] = [1.987_569_15e-4, 1.398_199_950_7e-3, 8.333_451_907_3e-3, 4.166_579_589_4e-2, 1.666_666_545_9e-1, 5.000_000_120_1e-1];
const LOG2E: f32 = 1.442_695_04;
/// ln 2 en dos trozos (Cephes): C1 + C2.
const LN2_1: f32 = 0.693_359_375;
const LN2_2: f32 = -2.121_944_4e-4;
const DOS_23: f32 = 8_388_608.0;
const MIN_NORMAL: f32 = 1.175_494_4e-38;

/// v * 2^n, con -152 <= n <= 130: en dos mitades, cada una normal; el
/// unico redondeo, el del segundo producto.
pub(crate) fn escala<M: Cuentas>(m: &mut M, v: M::V, n: M::V, uno: M::V) -> M::V {
    let n1 = m.corre(Corre::DerechaConSigno, n, 1);
    let n2 = m.entera(n, E::Menos(n1));
    let s1 = potencia(m, n1, uno);
    let s2 = potencia(m, n2, uno);
    let v = m.mul(F::R(v), F::R(s1));
    m.mul(F::R(v), F::R(s2))
}

/// **2^x.**
pub fn exp2<M: Cuentas>(m: &mut M, x: M::V) -> M::V {
    // A [-152, 129] (un NaN sigue NaN: las dos preguntas dan falso).
    let bajo = m.compara(Como::Lt, F::R(x), kf(-152.0));
    let x = m.elige(bajo, E::K((-152.0f32).to_bits()), E::R(x));
    let alto = m.compara(Como::Gt, F::R(x), kf(129.0));
    let x = m.elige(alto, E::K(129.0f32.to_bits()), E::R(x));
    let w = m.suma(F::R(x), kf(MAGIA));
    let n = m.suma(F::R(w), kf(-MAGIA));
    let f = m.suma(F::R(x), F::Menos(n));
    let p = horner(m, f, &EXP2);
    let uno = m.k(UNO);
    let v = m.fma(F::R(p), F::R(f), F::R(uno));
    let n = m.entera(w, E::K(MAGIA.to_bits().wrapping_neg()));
    escala(m, v, n, uno)
}

/// **log2(x).**
pub fn log2<M: Cuentas>(m: &mut M, x: M::V) -> M::V {
    // Un subnormal, por 2^23 (exacto), y 23 menos de exponente.
    let sub = m.compara(Como::Lt, F::R(x), kf(MIN_NORMAL));
    let xs = m.mul(F::R(x), kf(DOS_23));
    let xs = m.elige(sub, E::R(xs), E::R(x));
    // m en [raiz(1/2), raiz(2)): los bits, corridos para que el exponente
    // suba en raiz(2) (musl).
    let b = m.entera(xs, E::K(0x3F80_0000 - 0x3F35_04F3));
    let e = m.corre(Corre::Derecha, b, 23);
    let e = m.entera(e, E::K((-127i32) as u32));
    let e23 = m.entera(e, E::K((-23i32) as u32));
    let e = m.elige(sub, E::R(e23), E::R(e));
    let mb = m.bits(Bits::Y, b, E::K(0x007F_FFFF));
    let mb = m.entera(mb, E::K(0x3F35_04F3));
    let z = m.suma(F::R(mb), kf(-1.0));
    // log(1 + z) = z + y, y = z^3 P(z) - z^2/2.
    let zz = m.mul(F::R(z), F::R(z));
    let p = horner(m, z, &LOG);
    let t = m.mul(F::R(zz), F::R(z));
    let y = m.mul(F::R(p), F::R(t));
    let y = m.fma(F::R(zz), kf(-0.5), F::R(y));
    // log2 = e + z + y + (z + y)(log2(e) - 1), sin perder lo chico.
    let a = m.fma(F::R(y), kf(LOG2EA), F::R(y));
    let a = m.fma(F::R(z), kf(LOG2EA), F::R(a));
    let a = m.suma(F::R(a), F::R(z));
    let ef = a_float(m, e);
    let r = m.suma(F::R(a), F::R(ef));
    // Lo que no es un positivo finito.
    let positivo = m.compara(Como::Gt, F::R(x), kf(0.0));
    let r = m.elige(positivo, E::R(r), E::K(NAN));
    let cero = m.compara(Como::Eq, F::R(x), kf(0.0));
    let r = m.elige(cero, E::K(f32::NEG_INFINITY.to_bits()), E::R(r));
    let infinito = m.compara(Como::Eq, F::R(x), F::K(INFINITO));
    m.elige(infinito, E::K(INFINITO), E::R(r))
}

/// **e^a = e^r 2^n** (a <= 90): r (|r| <= ln2/2, ln2 en dos trozos) y n
/// (entero).
pub(crate) fn partes<M: Cuentas>(m: &mut M, a: M::V) -> (M::V, M::V) {
    let w = m.fma(F::R(a), kf(LOG2E), kf(MAGIA));
    let nf = m.suma(F::R(w), kf(-MAGIA));
    let r = m.fma(F::R(nf), kf(-LN2_1), F::R(a));
    let r = m.fma(F::R(nf), kf(-LN2_2), F::R(r));
    let n = m.entera(w, E::K(MAGIA.to_bits().wrapping_neg()));
    (r, n)
}

/// e^r, o e^-r con `negado` (|r| <= ln2/2): 1 + r + r^2 P(r).
pub(crate) fn e_r<M: Cuentas>(m: &mut M, r: M::V, z: M::V, uno: M::V, negado: bool) -> M::V {
    let p = if negado {
        // P(-r): Horner con el producto negado en cada paso.
        let mut p = m.fma(F::R(r), kf(-EXPE[0]), kf(EXPE[1]));
        for &k in &EXPE[2..] {
            p = m.fma(F::Menos(p), F::R(r), kf(k));
        }
        p
    } else {
        horner(m, r, &EXPE)
    };
    let y = m.fma(F::R(p), F::R(z), if negado { F::Menos(r) } else { F::R(r) });
    m.suma(F::R(y), F::R(uno))
}
