//! **LAS CUENTAS DE UNA RECETA** (DL13 de `docs/plan/PLAN_LAS_LIBRERIAS.md`,
//! 09-10, del propietario: *"la casa pasa a f32"*): las Mate de SERIES --
//! seno, coseno, tangente, exp2, log2, los arcos y los hiperbolicos -- dejan
//! sus cuentas de f64 y se escriben UNA vez, como una RECETA de cuentas de
//! f32 que cada tarjeta repite instruccion a instruccion. Los mismos bits
//! por construccion: no hay dos versiones de una funcion que puedan
//! separarse, hay una receta y dos que la corren.
//!
//! ```text
//!    la receta       una funcion generica sobre `Cuentas` (trigo.rs,
//!                    exponencial.rs, arcos.rs, hiperbolicas.rs): solo las
//!                    cuentas de abajo, sin saltos -- lo que elige, lo elige
//!                    con `elige` --
//!    la casa         `Casa`: cada cuenta, hecha (los bits de un f32 o de un
//!                    entero en un u32). Es lo que corre `Mate::aplicar`, y
//!                    con el el interprete, la CPU como tarjeta y `nativo`
//!    una tarjeta     la suya graba la receta y la traduce, una cuenta a una
//!                    instruccion (la de la 3060: `proton-x-sm86/src/series.rs`)
//! ```
//!
//! Las cuentas son las que una GPU hace EXACTAS, con un solo redondeo al
//! mas cercano: FFMA (a * b + c, UN redondeo), FMUL, FADD; comparar dos
//! floats; elegir; y las de enteros (sumar, and/or/xor, desplazar). Nada
//! de inversos ni raices de una unidad especial (la de una GPU es una
//! APROXIMACION que la casa no sabe repetir): un inverso o una raiz, cuando
//! hace falta, es Newton con estas mismas cuentas.
//!
//! Los subnormales se conservan (sin FTZ), como en la CPU. Un NaN es un NaN:
//! sus bits de carga no se prometen (la GPU da el suyo), y las recetas lo
//! cuidan -- lo que sale de los bits de un NaN no decide nunca un resultado
//! que no sea NaN --.

use crate::fma::fma;
use crate::mates::Mate;

/// **Un operando de float**: un valor, negado, en valor absoluto, o una
/// constante (sus bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum F<V> {
    R(V),
    Menos(V),
    Abs(V),
    K(u32),
}

/// **Un operando de entero**: un valor, negado (modulo 2^32), o una
/// constante.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum E<V> {
    R(V),
    Menos(V),
    K(u32),
}

/// Una constante de float.
pub const fn kf<V>(x: f32) -> F<V> {
    F::K(x.to_bits())
}

/// **Como se comparan dos floats** (FSETP) o dos enteros (ISETP). Con un
/// NaN, las de floats dan FALSO salvo `Neu` (distinto, o desordenado).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Como {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    Neu,
}

/// **Las de bits** (LOP3 de dos entradas).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bits {
    Y,
    O,
    Ox,
}

/// **Un desplazamiento** de 0 a 31.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corre {
    /// `a << n`.
    Izquierda,
    /// `a >> n`, entrando ceros.
    Derecha,
    /// `a >> n`, entrando el signo.
    DerechaConSigno,
}

/// **Las cuentas de una receta.** `V` es un registro de 32 bits (un f32 o un
/// entero, por sus bits); `P`, un predicado.
pub trait Cuentas {
    type V: Copy;
    type P: Copy;
    /// Una constante en un registro (MOV).
    fn k(&mut self, bits: u32) -> Self::V;
    /// `a * b + c`, con UN redondeo al mas cercano (FFMA).
    fn fma(&mut self, a: F<Self::V>, b: F<Self::V>, c: F<Self::V>) -> Self::V;
    /// `a * b` (FMUL).
    fn mul(&mut self, a: F<Self::V>, b: F<Self::V>) -> Self::V;
    /// `a + b` (FADD).
    fn suma(&mut self, a: F<Self::V>, b: F<Self::V>) -> Self::V;
    /// Dos floats comparados (FSETP).
    fn compara(&mut self, como: Como, a: F<Self::V>, b: F<Self::V>) -> Self::P;
    /// Dos enteros con signo comparados (ISETP).
    fn compara_entero(&mut self, como: Como, a: Self::V, b: E<Self::V>) -> Self::P;
    /// `p ? a : b`, los bits tal cual (SEL).
    fn elige(&mut self, p: Self::P, a: E<Self::V>, b: E<Self::V>) -> Self::V;
    /// `a + b`, enteros modulo 2^32 (IADD3).
    fn entera(&mut self, a: Self::V, b: E<Self::V>) -> Self::V;
    /// `a * b + c`, enteros modulo 2^32 (IMAD).
    fn imad(&mut self, a: Self::V, b: E<Self::V>, c: Self::V) -> Self::V;
    /// `a & b`, `a | b`, `a ^ b` (LOP3).
    fn bits(&mut self, que: Bits, a: Self::V, b: E<Self::V>) -> Self::V;
    /// `a` desplazado `n` (0..31) bits (SHF).
    fn corre(&mut self, como: Corre, a: Self::V, n: u32) -> Self::V;
}

/// **La casa**: cada cuenta, hecha. Un valor son los bits de un f32 (o de
/// un entero); un predicado, un booleano.
pub struct Casa;

fn de_f(x: F<u32>) -> f32 {
    f32::from_bits(match x {
        F::R(v) | F::K(v) => v,
        F::Menos(v) => v ^ 0x8000_0000,
        F::Abs(v) => v & 0x7FFF_FFFF,
    })
}

fn de_e(x: E<u32>) -> u32 {
    match x {
        E::R(v) | E::K(v) => v,
        E::Menos(v) => v.wrapping_neg(),
    }
}

impl Cuentas for Casa {
    type V = u32;
    type P = bool;

    fn k(&mut self, bits: u32) -> u32 {
        bits
    }

    fn fma(&mut self, a: F<u32>, b: F<u32>, c: F<u32>) -> u32 {
        fma(de_f(a), de_f(b), de_f(c)).to_bits()
    }

    fn mul(&mut self, a: F<u32>, b: F<u32>) -> u32 {
        (de_f(a) * de_f(b)).to_bits()
    }

    fn suma(&mut self, a: F<u32>, b: F<u32>) -> u32 {
        (de_f(a) + de_f(b)).to_bits()
    }

    fn compara(&mut self, como: Como, a: F<u32>, b: F<u32>) -> bool {
        let (x, y) = (de_f(a), de_f(b));
        match como {
            Como::Lt => x < y,
            Como::Le => x <= y,
            Como::Gt => x > y,
            Como::Ge => x >= y,
            Como::Eq => x == y,
            Como::Ne => x < y || x > y,
            Como::Neu => x != y,
        }
    }

    fn compara_entero(&mut self, como: Como, a: u32, b: E<u32>) -> bool {
        let (x, y) = (a as i32, de_e(b) as i32);
        match como {
            Como::Lt => x < y,
            Como::Le => x <= y,
            Como::Gt => x > y,
            Como::Ge => x >= y,
            Como::Eq => x == y,
            Como::Ne | Como::Neu => x != y,
        }
    }

    fn elige(&mut self, p: bool, a: E<u32>, b: E<u32>) -> u32 {
        if p {
            de_e(a)
        } else {
            de_e(b)
        }
    }

    fn entera(&mut self, a: u32, b: E<u32>) -> u32 {
        a.wrapping_add(de_e(b))
    }

    fn imad(&mut self, a: u32, b: E<u32>, c: u32) -> u32 {
        a.wrapping_mul(de_e(b)).wrapping_add(c)
    }

    fn bits(&mut self, que: Bits, a: u32, b: E<u32>) -> u32 {
        let b = de_e(b);
        match que {
            Bits::Y => a & b,
            Bits::O => a | b,
            Bits::Ox => a ^ b,
        }
    }

    fn corre(&mut self, como: Corre, a: u32, n: u32) -> u32 {
        let n = n & 31;
        match como {
            Corre::Izquierda => a << n,
            Corre::Derecha => a >> n,
            Corre::DerechaConSigno => ((a as i32) >> n) as u32,
        }
    }
}

// -- Lo que comparten las recetas ----------------------------------------------

/// `M = 1.5 * 2^23`: `(x + M) - M` es el entero mas cercano a `x` (el empate
/// al par) si |x| < 2^22, y los bits bajos de `x + M` son ese entero (modulo
/// 2^22, en complemento a dos).
pub(crate) const MAGIA: f32 = 12_582_912.0;
/// 1.0 en sus bits (tambien `127 << 23`: el exponente de 2^0).
pub(crate) const UNO: u32 = 0x3F80_0000;
pub(crate) const SIGNO: u32 = 0x8000_0000;
/// Un NaN (el de la casa, sin carga: los bits de un NaN no se prometen).
pub(crate) const NAN: u32 = 0x7FC0_0000;
pub(crate) const INFINITO: u32 = 0x7F80_0000;

/// **Horner con FFMA**: `((c[0] * z + c[1]) * z + ...) + c[n-1]`, el primer
/// paso con la constante como multiplicando.
pub(crate) fn horner<M: Cuentas>(m: &mut M, z: M::V, c: &[f32]) -> M::V {
    let mut p = m.fma(F::R(z), kf(c[0]), kf(c[1]));
    for &k in &c[2..] {
        p = m.fma(F::R(p), F::R(z), kf(k));
    }
    p
}

/// **El entero `n` (|n| < 2^22) como f32**: los bits de `M + n`, menos M --
/// sin conversion de una unidad especial.
pub(crate) fn a_float<M: Cuentas>(m: &mut M, n: M::V) -> M::V {
    let b = m.entera(n, E::K(MAGIA.to_bits()));
    m.suma(F::R(b), kf(-MAGIA))
}

/// **2^n como f32**, para -126 <= n <= 127: `(n + 127) << 23`, con UN IMAD
/// (`n * 2^23 + los bits de 1.0`).
pub(crate) fn potencia<M: Cuentas>(m: &mut M, n: M::V, uno: M::V) -> M::V {
    m.imad(n, E::K(1 << 23), uno)
}

/// **`v` con el signo de `x`** (`v` sin signo): `v ^ (x & signo)`.
pub(crate) fn con_signo<M: Cuentas>(m: &mut M, v: M::V, x: M::V) -> M::V {
    let s = m.bits(Bits::Y, x, E::K(SIGNO));
    m.bits(Bits::Ox, v, E::R(s))
}

/// **El inverso de `y`** (|y| normal, entre 2^-125 y 2^125), con el signo de
/// `y`: la semilla de los bits (`0x7EF311C3 - |y|`), `n` pasos de Newton con
/// FFMA (`e = 1 - |y| r`, `r += r e`), y el signo.
pub(crate) fn inverso<M: Cuentas>(m: &mut M, y: M::V, uno: M::V, n: usize) -> M::V {
    let a = m.bits(Bits::Y, y, E::K(!SIGNO));
    let r = inverso_positivo(m, a, uno, n);
    con_signo(m, r, y)
}

/// El inverso de `a` > 0 (normal, de 2^-125 a 2^125): la semilla y `n`
/// pasos de Newton.
pub(crate) fn inverso_positivo<M: Cuentas>(m: &mut M, a: M::V, uno: M::V, n: usize) -> M::V {
    // 0x7EF311C3 - a = !a + 0x7EF311C4 (la resta, sin negar un registro).
    let na = m.bits(Bits::Ox, a, E::K(u32::MAX));
    let mut r = m.entera(na, E::K(0x7EF3_11C4));
    for _ in 0..n {
        let e = m.fma(F::Menos(a), F::R(r), F::R(uno));
        r = m.fma(F::R(r), F::R(e), F::R(r));
    }
    r
}

/// **`num / den`** con `den` > 0 (normal, de 2^-125 a 2^125): el inverso con
/// `n` pasos, el cociente, y una correccion con su residuo exacto (la FFMA
/// hace `num - den q` sin redondear el producto).
pub(crate) fn cociente<M: Cuentas>(m: &mut M, num: F<M::V>, den: M::V, uno: M::V, n: usize) -> M::V {
    let r = inverso_positivo(m, den, uno, n);
    let q = m.mul(num, F::R(r));
    let e = m.fma(F::Menos(den), F::R(q), num);
    m.fma(F::R(r), F::R(e), F::R(q))
}

/// **La receta de una Mate de series**, o `None` si `f` no es de series (las
/// exactas de `mates.rs` no son recetas: son lo que son).
pub fn receta<M: Cuentas>(m: &mut M, f: Mate, x: M::V) -> Option<M::V> {
    use crate::{arcos, exponencial, hiperbolicas, trigo};
    Some(match f {
        Mate::Sin => trigo::seno_o_coseno(m, x, false),
        Mate::Cos => trigo::seno_o_coseno(m, x, true),
        Mate::Tan => trigo::tangente(m, x),
        Mate::Exp2 => exponencial::exp2(m, x),
        Mate::Log2 => exponencial::log2(m, x),
        Mate::Atan => arcos::arcotangente(m, x),
        Mate::Asin => arcos::arcoseno_o_arcocoseno(m, x, false),
        Mate::Acos => arcos::arcoseno_o_arcocoseno(m, x, true),
        Mate::Senh => hiperbolicas::seno_h(m, x),
        Mate::Cosh => hiperbolicas::coseno_h(m, x),
        Mate::Tanh => hiperbolicas::tangente_h(m, x),
        _ => return None,
    })
}

/// **Si `f` es de series** (una receta).
pub fn es_de_series(f: Mate) -> bool {
    matches!(f, Mate::Sin | Mate::Cos | Mate::Tan | Mate::Exp2 | Mate::Log2 | Mate::Atan | Mate::Asin | Mate::Acos | Mate::Senh | Mate::Cosh | Mate::Tanh)
}

/// **Una Mate de series en la casa**: los bits de su receta.
pub fn en_la_casa(f: Mate, x: f32) -> Option<f32> {
    receta(&mut Casa, f, x.to_bits()).map(f32::from_bits)
}
