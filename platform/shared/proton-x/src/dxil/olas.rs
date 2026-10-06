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
//!              8 en 8 cuadros, de un mismo triangulo (`crate::cuadros`)
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
//! abajo (`crate::cuadros`):
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

use super::estructura::{bits, literal};
use super::programa::{Compilador, NoPrograma, Op, Reg, Valor};

/// **Los carriles de una ola**: 32, los de un warp de la 3060.
pub const CARRILES: u32 = 32;

const DERIV_PRIMERA: i64 = 83;
const DERIV_ULTIMA: i64 = 86;
const ES_PRIMER_CARRIL: i64 = 110;
const INDICE_DE_CARRIL: i64 = 111;
const CARRILES_DE_OLA: i64 = 112;
const ALGUNO_CIERTO: i64 = 113;
const TODOS_CIERTOS: i64 = 114;
const TODOS_IGUALES: i64 = 115;
const PAPELETA: i64 = 116;
const LEER_CARRIL: i64 = 117;
const LEER_PRIMERO: i64 = 118;
const OP_DE_OLA: i64 = 119;
const BITS_DE_OLA: i64 = 120;
const PREFIJO: i64 = 121;
const LEER_DEL_CUADRO: i64 = 122;
const CRUZA_EL_CUADRO: i64 = 123;
const CUENTA_DE_BITS: i64 = 135;
const CUENTA_DE_PREFIJO: i64 = 136;

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

/// **La operacion de D3D `op` si es de olas o una derivada** (`None` si
/// no): una [`Op::Ola`] (las derivadas tambien, D4.4), o -- `WaveGetLaneCount`
/// -- una constante. `nombre` es el de la funcion (`dx.op.waveActiveOp.f32`): su
/// sobrecarga dice si son floats.
pub(super) fn de(c: &mut Compilador, op: i64, args: &[usize], nombre: &str) -> Option<Result<Valor, NoPrograma>> {
    if !(DERIV_PRIMERA..=DERIV_ULTIMA).contains(&op) && !(ES_PRIMER_CARRIL..=CRUZA_EL_CUADRO).contains(&op) && !(CUENTA_DE_BITS..=CUENTA_DE_PREFIJO).contains(&op) {
        return None;
    }
    Some(compilar(c, op, args, nombre))
}

fn compilar(c: &mut Compilador, op: i64, args: &[usize], nombre: &str) -> Result<Valor, NoPrograma> {
    let arg = |k: usize| args.get(k).copied().ok_or(NoPrograma::Forma("una operacion de olas con menos argumentos"));
    let float = nombre.ends_with(".f32");
    if [".f16", ".f64", ".i16", ".i64"].iter().any(|s| nombre.ends_with(s)) {
        return Err(NoPrograma::Forma("una operacion de olas de 16 o 64 bits: todavia no"));
    }
    // Una constante i8 (la operacion, el signo); `undef`, 0.
    let chico = |c: &Compilador, k: usize| -> Result<u8, NoPrograma> {
        match c.valores.get(arg(k)?) {
            Some(Valor::Entero(v)) if (0..4).contains(v) => Ok(*v as u8),
            Some(Valor::Indefinido) => Ok(0),
            _ => Err(NoPrograma::Forma("una operacion de olas con su clase calculada")),
        }
    };
    let num = |c: &Compilador, k: usize| -> Result<Numero, NoPrograma> {
        Ok(if float {
            Numero::Float
        } else if chico(c, k)? == 1 {
            Numero::SinSigno
        } else {
            Numero::Entero
        })
    };
    let que = match op {
        // D4.4: las derivadas, de verdad (83 y 84 gruesas, 85 y 86 finas; las
        // pares, las de y). El numero de carriles, constante.
        DERIV_PRIMERA..=DERIV_ULTIMA => Ola::Derivada { y: (op - DERIV_PRIMERA) % 2 == 1, fina: op >= DERIV_PRIMERA + 2, muestra: false },
        CARRILES_DE_OLA => return literal(c, CARRILES).map(Valor::Bits),
        ES_PRIMER_CARRIL => Ola::EsPrimero,
        INDICE_DE_CARRIL => Ola::Indice,
        ALGUNO_CIERTO => Ola::AlgunoCierto,
        TODOS_CIERTOS => Ola::TodosCiertos,
        TODOS_IGUALES => Ola::TodosIguales { float },
        PAPELETA => Ola::Papeleta,
        LEER_CARRIL => Ola::LeerCarril,
        LEER_PRIMERO => Ola::LeerPrimero,
        OP_DE_OLA => Ola::Activa { op: chico(c, 2)?, num: num(c, 3)? },
        BITS_DE_OLA => Ola::Bits { op: chico(c, 2)? },
        PREFIJO => Ola::Prefijo { op: chico(c, 2)? & 1, num: num(c, 3)? },
        LEER_DEL_CUADRO => Ola::Cuadro,
        CRUZA_EL_CUADRO => match chico(c, 2)? {
            k @ 0..=2 => Ola::Cruza(k + 1),
            _ => return Err(NoPrograma::Forma("un QuadOp que no es X, Y ni la diagonal")),
        },
        CUENTA_DE_BITS => Ola::CuentaBits,
        CUENTA_DE_PREFIJO => Ola::PrefijoBits,
        _ => return Err(NoPrograma::OperacionD3d(op)),
    };
    // El valor (las que no lo llevan, cualquiera: 0) y el segundo operando.
    let a = match args.get(1) {
        Some(&k) => bits(c, k)?,
        None => literal(c, 0)?,
    };
    let b = match que {
        Ola::LeerCarril | Ola::Cuadro => bits(c, arg(2)?)?,
        _ => a,
    };
    let d = c.registro(0.0)?;
    for _ in 1..anchura(que) {
        c.registro(0.0)?;
    }
    c.ops.push(Op::Ola { d, a, b, que });
    // Lo que devuelve: un booleano, un uint4, un entero, o del tipo del valor.
    Ok(match que {
        Ola::EsPrimero | Ola::AlgunoCierto | Ola::TodosCiertos | Ola::TodosIguales { .. } => Valor::Bool(d),
        Ola::Papeleta => Valor::CuatroEnteros(d),
        Ola::Indice | Ola::CuentaBits | Ola::PrefijoBits => Valor::Bits(d),
        _ if float => Valor::Float(d),
        _ if nombre.ends_with(".i1") => Valor::Bool(d),
        _ => Valor::Bits(d),
    })
}

/// **D4.4: los gradientes de un muestreo** con la mip por derivadas: en
/// `g..g+4`, `ddx(u)`, `ddx(v)`, `ddy(u)` y `ddy(v)`, gruesas (una mip para
/// todo el cuadro, como las GPU). Los usa `textura::Textura::lambda`.
pub fn gradientes(ops: &mut alloc::vec::Vec<Op>, g: Reg, u: Reg, v: Reg) {
    for (k, (a, y)) in [(u, false), (v, false), (u, true), (v, true)].into_iter().enumerate() {
        ops.push(Op::Ola { d: g + k as Reg, a, b: a, que: Ola::Derivada { y, fina: false, muestra: true } });
    }
}

/// **D4.4: el bloque de un muestreo con la mip por gradientes** (ver
/// `Lectura::Gradientes`), y su primer registro: los cuatro gradientes de
/// (u, v) = (`co[0]`, `co[1]`) -- los que da el sombreador (`grad`: ddx u,
/// ddx v, ddy u, ddy v) o los de su cuadro (`olas::gradientes`) -- y detras,
/// copiado, cada uno de `resto` (el sesgo, el clamp, la referencia; uno que
/// no viene, 0).
pub(super) fn bloque(c: &mut Compilador, co: [Reg; 4], grad: Option<[usize; 4]>, resto: &[Option<usize>]) -> Result<Reg, NoPrograma> {
    let g = c.registro(0.0)?;
    for _ in 1..4 + resto.len() {
        c.registro(0.0)?;
    }
    match grad {
        Some(ids) => {
            for (k, id) in ids.into_iter().enumerate() {
                let a = bits(c, id)?;
                c.ops.push(Op::Copia { d: g + k as Reg, a });
            }
        }
        None => gradientes(&mut c.ops, g, co[0], co[1]),
    }
    for (k, id) in resto.iter().enumerate() {
        let a = match id {
            Some(id) => bits(c, *id)?,
            None => literal(c, 0)?,
        };
        c.ops.push(Op::Copia { d: g + 4 + k as Reg, a });
    }
    Ok(g)
}
