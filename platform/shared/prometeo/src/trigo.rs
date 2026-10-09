//! **SENO, COSENO Y TANGENTE en f32** (DL13, 09-10): recetas de `cuentas.rs`
//! -- la casa las corre, cada tarjeta las repite --.
//!
//! ```text
//!    la reduccion   j = el entero mas cercano a x * 2/pi (con la MAGIA: sus
//!                   bits bajos son el cuadrante), y r = x - j pi/2 con pi/2
//!                   en TRES trozos por FFMA (la de CUDA): el producto no se
//!                   redondea, y r sale a una millonesima de ULP de lo que es
//!    los polinomios en [-pi/4, pi/4], por Horner con FFMA: el seno y el
//!                   coseno de CUDA; la tangente, la de Cephes, y en los
//!                   cuadrantes impares -1/tan(r) (Newton, `cuentas::inverso`)
//!    el cuadrante   el par elige seno o coseno; el bit 1, el signo. La
//!                   tangente da UNA vuelta mas: lejos, 2/pi en f32 deja un j
//!                   de mas o de menos, y su polinomio solo vale hasta pi/4
//!    lo grande      desde |x| = 1.5 * 2^22 la MAGIA no da el entero: x se
//!                   lleva antes a vueltas, como hace una GPU (x / 2pi, lo
//!                   de despues de la coma, por 2pi). D3D no pide nada ahi
//!                   (su margen es de -100pi a 100pi); la receta da un
//!                   numero de [-1, 1] que se mueve con x, nunca un infinito
//!    el signo       sobre |x|, y el de x al final: seno y tangente
//!                   impares, coseno par, por construccion (sin(-0) = -0)
//!    NaN e inf      NaN
//! ```
//!
//! El error, medido sobre TODOS los f32 contra el f64 de la `std` del
//! anfitrion, lo dicen las pruebas (`pruebas_series.rs`).

use crate::cuentas::{con_signo, inverso, kf, Bits, Como, Corre, Cuentas, E, F, MAGIA, SIGNO, UNO};

/// 2/pi.
const DOS_PI: f32 = 0.636_619_77;
/// pi/2 en tres trozos (los de CUDA): -PIO2_1 - PIO2_2 - PIO2_3.
const PIO2_1: f32 = 1.570_796_01;
const PIO2_2: f32 = 3.139_164_73e-7;
const PIO2_3: f32 = 5.390_302_53e-15;
/// 1/(2pi) y 2pi, para lo grande.
const INV_2PI: f32 = 0.159_154_94;
const DOS_PI_ENTERO: f32 = 6.283_185_5;
/// Desde aqui, lo grande: |x * 2/pi| < 2^22 con margen.
const GRANDE: f32 = 6_291_456.0;
const DOS_23: f32 = 8_388_608.0;

/// sin(r) = r + r^3 (S0 s^3 + S1 s^2 + S2 s + S3), s = r^2 (CUDA).
const SENO: [f32; 4] = [2.865_679_56e-6, -1.985_599_23e-4, 8.333_385_92e-3, -1.666_666_72e-1];
/// cos(r) = 1 + s (...) (CUDA).
const COSENO: [f32; 5] = [2.446_770_67e-5, -1.388_772_97e-3, 4.166_665_67e-2, -0.5, 1.0];
/// tan(r) = r + r^3 P(s) (Cephes `tanf`).
const TANGENTE: [f32; 6] = [9.385_401_855_43e-3, 3.119_922_326_97e-3, 2.443_013_545_25e-2, 5.341_128_070_05e-2, 1.333_879_940_85e-1, 3.333_315_685_48e-1];

/// `r` y los bits del cuadrante (los bajos de `w`), de `x` >= 0 (o NaN).
fn reduce<M: Cuentas>(m: &mut M, x: M::V) -> (M::V, M::V) {
    // Lo grande, a vueltas: u = x / 2pi; f = u - su entero (u mismo si ya lo
    // es); x = f 2pi. Con un NaN o un infinito, NaN.
    let u = m.mul(F::R(x), kf(INV_2PI));
    let w = m.suma(F::R(u), kf(MAGIA));
    let j = m.suma(F::R(w), kf(-MAGIA));
    let chico = m.compara(Como::Lt, F::R(u), kf(DOS_23));
    let j = m.elige(chico, E::R(j), E::R(u));
    let f = m.suma(F::R(u), F::Menos(j));
    let vueltas = m.mul(F::R(f), kf(DOS_PI_ENTERO));
    let cabe = m.compara(Como::Lt, F::R(x), kf(GRANDE));
    let x = m.elige(cabe, E::R(x), E::R(vueltas));
    // j = rint(x 2/pi), y r = x - j pi/2.
    let w = m.fma(F::R(x), kf(DOS_PI), kf(MAGIA));
    let j = m.suma(F::R(w), kf(-MAGIA));
    let r = m.fma(F::R(j), kf(-PIO2_1), F::R(x));
    let r = m.fma(F::R(j), kf(-PIO2_2), F::R(r));
    let r = m.fma(F::R(j), kf(-PIO2_3), F::R(r));
    (r, w)
}

/// **Una vuelta mas** (la tangente): con |x| de millones, 2/pi en f32 (a
/// 2^-25) puede dar un j de mas o de menos, y |r| pasa de pi/4 -- el
/// polinomio de la tangente solo vale hasta ahi --. j1 = rint(r 2/pi) (-1, 0
/// o 1), r -= j1 pi/2, y el cuadrante suma j1 (los bits bajos de la MAGIA son
/// ceros: w + w1 tiene abajo j + j1).
fn otra_vuelta<M: Cuentas>(m: &mut M, r: M::V, w: M::V) -> (M::V, M::V) {
    let w1 = m.fma(F::R(r), kf(DOS_PI), kf(MAGIA));
    let j1 = m.suma(F::R(w1), kf(-MAGIA));
    let r = m.fma(F::R(j1), kf(-PIO2_1), F::R(r));
    let r = m.fma(F::R(j1), kf(-PIO2_2), F::R(r));
    let r = m.fma(F::R(j1), kf(-PIO2_3), F::R(r));
    let w = m.entera(w, E::R(w1));
    (r, w)
}

/// El seno de r (|r| <= pi/4), con s = r^2.
fn seno_r<M: Cuentas>(m: &mut M, r: M::V, s: M::V) -> M::V {
    let p = crate::cuentas::horner(m, s, &SENO);
    let t = m.mul(F::R(s), F::R(r));
    m.fma(F::R(p), F::R(t), F::R(r))
}

/// El coseno de r, con s = r^2.
fn coseno_r<M: Cuentas>(m: &mut M, s: M::V) -> M::V {
    crate::cuentas::horner(m, s, &COSENO)
}

/// **sin(x)**, o **cos(x)** con `coseno` (sin(x + pi/2): el cuadrante mas
/// uno). Sobre |x|, y el signo de x al final en el seno: impar y par por
/// construccion, tambien en los ceros (sin(-0) = -0) y en lo grande.
pub fn seno_o_coseno<M: Cuentas>(m: &mut M, x: M::V, coseno: bool) -> M::V {
    let a = m.bits(Bits::Y, x, E::K(!SIGNO));
    let (r, w) = reduce(m, a);
    let s = m.mul(F::R(r), F::R(r));
    let vs = seno_r(m, r, s);
    let vc = coseno_r(m, s);
    let q = if coseno { m.entera(w, E::K(1)) } else { w };
    // El cuadrante impar, el coseno; el bit 1, el signo.
    let par = m.corre(Corre::Izquierda, q, 31);
    let impar = m.compara_entero(Como::Lt, par, E::K(0));
    let v = m.elige(impar, E::R(vc), E::R(vs));
    let b = m.corre(Corre::Izquierda, q, 30);
    let signo = m.bits(Bits::Y, b, E::K(SIGNO));
    let v = m.bits(Bits::Ox, v, E::R(signo));
    if coseno {
        v
    } else {
        con_signo(m, v, x)
    }
}

/// **tan(x)**: tan(r) en los cuadrantes pares, -1/tan(r) en los impares;
/// sobre |x|, y el signo de x al final (impar por construccion).
pub fn tangente<M: Cuentas>(m: &mut M, x: M::V) -> M::V {
    let a = m.bits(Bits::Y, x, E::K(!SIGNO));
    let (r, w) = reduce(m, a);
    let (r, w) = otra_vuelta(m, r, w);
    let s = m.mul(F::R(r), F::R(r));
    let p = crate::cuentas::horner(m, s, &TANGENTE);
    let t = m.mul(F::R(s), F::R(r));
    let y = m.fma(F::R(p), F::R(t), F::R(r));
    let uno = m.k(UNO);
    let inv = inverso(m, y, uno, 4);
    // El impar: -1/y, el signo por el bit 0 del cuadrante en el 31.
    let par = m.corre(Corre::Izquierda, w, 31);
    let impar = m.compara_entero(Como::Lt, par, E::K(0));
    let v = m.elige(impar, E::R(inv), E::R(y));
    let v = m.bits(Bits::Ox, v, E::R(par));
    con_signo(m, v, x)
}
