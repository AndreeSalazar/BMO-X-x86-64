//! **Las olas de verdad** (E2.5, 05-10; antes, desde el 03-10, "un pixel
//! por ola"): las operaciones `Wave*` y `Quad*` del modelo 6.0, y las
//! derivadas.
//!
//! Una OLA son [`CARRILES`] hilos que corren juntos (un warp de la 3060: lo
//! que la casa dice en `CheckFeatureSupport(OPTIONS1)`, WaveLaneCountMin y
//! Max, y lo que ve `WaveGetLaneCount`: el MISMO numero). Quien los agrupa:
//!
//! ```text
//!    computo   los hilos seguidos del grupo, en el orden de SV_GroupIndex:
//!              la ola k son los 32k..32k+31 (como la 3060)
//!    pixeles   cuadros de 2x2 (los de fuera del triangulo, AYUDANTES), de
//!              8 en 8 cuadros, de un mismo triangulo (los `cuadros` de PROTON-X)
//!    vertices  un hilo SOLO: una ola de 32 con un carril activo, el 0
//!    y GS      (D3D lo deja: los activos pueden ser menos que 32)
//! ```
//!
//! Lo que hace cada una lo dice [`hacer`]: sobre los carriles ACTIVOS (los
//! que llegaron a ESA operacion por el mismo camino; los ayudantes no
//! cuentan, como en D3D), en el orden de los carriles. Las `Quad*` leen el
//! carril vecino de su cuadro (`k ^ 1` en x, `k ^ 2` en y), ayudante o no.
//!
//! Lo que falta, dicho: las de 16 y 64 bits (`.f16`, `.i64`...) no compilan
//! (el modulo lo dice), ni las del modelo 6.5 (`WaveMatch`, `WaveMulti*`).
//!
//! **Las DERIVADAS** (D4.4, 05-10; hasta entonces daban 0): `ddx`, `ddy` y
//! `fwidth` son una [`Ola::Derivada`], la resta de dos carriles de SU cuadro
//! (ayudantes o no, como las `Quad*`). Con los carriles 0 1 arriba y 2 3
//! abajo (los `cuadros` de PROTON-X):
//!
//! ```text
//!    gruesa x   p1 - p0, para los cuatro      (DerivCoarseX, 83: `ddx`)
//!    gruesa y   p2 - p0, para los cuatro      (DerivCoarseY, 84: `ddy`)
//!    fina x     la de su FILA: p1 - p0 o p3 - p2          (85: `ddx_fine`)
//!    fina y     la de su COLUMNA: p2 - p0 o p3 - p1       (86: `ddy_fine`)
//! ```
//!
//! D3D deja a la gruesa usar cualquier par del cuadro; esta es la de arriba
//! a la izquierda, la de las GPU de escritorio. `dxc` traduce `ddx` y `ddy`
//! a las gruesas. Fuera de un cuadro (un vertice, un hilo de computo solo)
//! la resta es de un carril consigo mismo: 0.
//!
//! El MUESTREO con la mip por derivadas (`Sample`, `SampleBias`,
//! `SampleCmp`, `CalculateLevelOfDetail`) pide las cuatro gruesas de sus
//! (u, v) con [`gradientes`]: por eso un sombreador que muestrea tambien va
//! en cuadros. Esas llevan `muestra: true`: la 3060 las sabe hacer sola (su
//! TEX las calcula) y la puerta no las cuenta como olas del sombreador.
//!
//! En PROMETEO desde el 08-10 (LB3b): lo que HACE cada operacion de olas.
//! Leerlas del DXIL (`dx.op.wave*`, las derivadas) es del traductor de
//! PROTON-X (`platform/shared/proton-x/src/dxil/olas.rs`).

use alloc::vec::Vec;

use super::programa::{Op, Reg};

/// **Los carriles de una ola**: 32, los de un warp de la 3060.
pub const CARRILES: u32 = 32;

/// **Que hace una [`Op::Ola`]**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ola {
    /// `WaveIsFirstLane`: si es el activo mas bajo.
    EsPrimero,
    /// `WaveGetLaneIndex`.
    Indice,
    /// `WaveActiveAnyTrue`, `WaveActiveAllTrue`.
    AlgunoCierto,
    TodosCiertos,
    /// `WaveActiveAllEqual`: con floats, `==` de float (0 y -0 iguales, un
    /// NaN distinto); si no, los bits.
    TodosIguales { float: bool },
    /// `WaveActiveBallot`: un bit por activo con `a` cierto, en `d..d+4`.
    Papeleta,
    /// `WaveReadLaneAt(a, b)` y `WaveReadLaneFirst(a)`.
    LeerCarril,
    LeerPrimero,
    /// `WaveActiveSum/Product/Min/Max` (`op` 0, 1, 2, 3).
    Activa { op: u8, num: Numero },
    /// `WaveActiveBitAnd/Or/Xor` (`op` 0, 1, 2).
    Bits { op: u8 },
    /// `WavePrefixSum/Product` (`op` 0, 1): los activos de MAS ABAJO.
    Prefijo { op: u8, num: Numero },
    /// `WaveActiveCountBits`, `WavePrefixCountBits`.
    CuentaBits,
    PrefijoBits,
    /// `QuadReadLaneAt(a, b)`: el carril `b` de su cuadro.
    Cuadro,
    /// `QuadReadAcrossX` (1), `...Y` (2), `...Diagonal` (3): el carril `k ^ m`.
    Cruza(u8),
    /// D4.4: una derivada de float, `ddx` (o `ddy` si `y`), gruesa o `fina`
    /// (ver arriba). `muestra`: no la pidio el sombreador sino un muestreo,
    /// para su mip ([`gradientes`]).
    Derivada { y: bool, fina: bool, muestra: bool },
}

/// Como se cuentan los valores de [`Ola::Activa`] y [`Ola::Prefijo`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numero {
    Float,
    Entero,
    SinSigno,
}

/// Cuantos registros escribe (4 la papeleta: un `uint4`).
pub fn anchura(que: Ola) -> usize {
    if que == Ola::Papeleta {
        4
    } else {
        1
    }
}

/// **Lo que da la operacion `que` en el carril `k`**, en una ola cuyos
/// carriles activos son `activos` (bit j = carril j); `a(j)` es el valor
/// del carril j (sus bits), `b` el segundo operando del carril `k`.
pub fn hacer(que: Ola, k: usize, activos: u32, a: impl Fn(usize) -> u32, b: u32) -> [u32; 4] {
    let cierto = |x: bool| if x { u32::MAX } else { 0 };
    let lista = || (0..CARRILES as usize).filter(move |&j| activos >> j & 1 != 0);
    // El primer activo (o el mismo, si no hay ninguno: una ola de ayudantes).
    let primero = if activos == 0 { k } else { activos.trailing_zeros() as usize };
    let x = match que {
        Ola::EsPrimero => cierto(activos != 0 && k == primero),
        Ola::Indice => k as u32,
        Ola::AlgunoCierto => cierto(lista().any(|j| a(j) != 0)),
        Ola::TodosCiertos => cierto(lista().all(|j| a(j) != 0)),
        Ola::TodosIguales { float } => {
            let p = a(primero);
            cierto(lista().all(|j| if float { f32::from_bits(a(j)) == f32::from_bits(p) } else { a(j) == p }))
        }
        Ola::Papeleta => return [lista().filter(|&j| a(j) != 0).fold(0, |m, j| m | 1 << j), 0, 0, 0],
        // Un carril que no existe: D3D no lo define; el mismo.
        Ola::LeerCarril => a(if b < CARRILES { b as usize } else { k }),
        Ola::LeerPrimero => a(primero),
        Ola::Activa { op, num } => juntar(op, num, lista().map(&a)).unwrap_or_else(|| a(k)),
        Ola::Bits { op } => lista().map(&a).fold(if op == 0 { u32::MAX } else { 0 }, |s, v| match op {
            0 => s & v,
            1 => s | v,
            _ => s ^ v,
        }),
        // Sin nadie debajo, el neutro: 0 la suma, 1 el producto (de su tipo).
        Ola::Prefijo { op, num } => juntar(op, num, lista().filter(|&j| j < k).map(&a)).unwrap_or(match (op, num) {
            (0, Numero::Float) => 0.0f32.to_bits(),
            (_, Numero::Float) => 1.0f32.to_bits(),
            (0, _) => 0,
            _ => 1,
        }),
        Ola::CuentaBits => lista().filter(|&j| a(j) != 0).count() as u32,
        Ola::PrefijoBits => lista().filter(|&j| j < k && a(j) != 0).count() as u32,
        Ola::Cuadro => a((k & !3) | (b & 3) as usize),
        Ola::Cruza(m) => a(k ^ m as usize),
        // D4.4: el carril de arriba a la izquierda de su fila (x) o de su
        // columna (y); en la gruesa, del cuadro. Menos el de su derecha (o
        // el de debajo): la resta de dos floats.
        Ola::Derivada { y, fina, .. } => {
            let q = k & !3;
            let de = match (y, fina) {
                (false, true) => q + (k & 2),
                (true, true) => q + (k & 1),
                _ => q,
            };
            let paso = if y { 2 } else { 1 };
            (f32::from_bits(a(de + paso)) - f32::from_bits(a(de))).to_bits()
        }
    };
    [x, 0, 0, 0]
}

/// Suma, producto, min o max (`op` 0..3) de los valores, en ese orden
/// (con floats el orden cambia los bits: el de los carriles, de abajo a
/// arriba); `None` si no hay ninguno.
fn juntar(op: u8, num: Numero, mut v: impl Iterator<Item = u32>) -> Option<u32> {
    let p = v.next()?;
    let f = f32::from_bits;
    Some(v.fold(p, |s, x| match (num, op) {
        (Numero::Float, 0) => (f(s) + f(x)).to_bits(),
        (Numero::Float, 1) => (f(s) * f(x)).to_bits(),
        // FMin/FMax de D3D: con un NaN, el otro.
        (Numero::Float, 2) => {
            if f(s).is_nan() || f(x) < f(s) {
                x
            } else {
                s
            }
        }
        (Numero::Float, _) => {
            if f(s).is_nan() || f(x) > f(s) {
                x
            } else {
                s
            }
        }
        (_, 0) => s.wrapping_add(x),
        (_, 1) => s.wrapping_mul(x),
        (Numero::Entero, 2) => (s as i32).min(x as i32) as u32,
        (Numero::Entero, _) => (s as i32).max(x as i32) as u32,
        (_, 2) => s.min(x),
        _ => s.max(x),
    }))
}

/// **D4.4: los gradientes de un muestreo** con la mip por derivadas: en
/// `g..g+4`, `ddx(u)`, `ddx(v)`, `ddy(u)` y `ddy(v)`, gruesas (una mip para
/// todo el cuadro, como las GPU). Los usa `textura::Textura::lambda`.
pub fn gradientes(ops: &mut Vec<Op>, g: Reg, u: Reg, v: Reg) {
    for (k, (a, y)) in [(u, false), (v, false), (u, true), (v, true)].into_iter().enumerate() {
        ops.push(Op::Ola { d: g + k as Reg, a, b: a, que: Ola::Derivada { y, fina: false, muestra: true } });
    }
}
