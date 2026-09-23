//! **La aritmetica del oraculo: LA DEFINICION.**
//!
//! Todo lo de coma flotante que no es una suma, una resta, un producto o una
//! division. En `no_std` no hay `sqrt`, ni `floor`, ni `mul_add` (viven en la
//! biblioteca estandar, que las pide a la del sistema), asi que se escriben
//! aqui -- y eso es una VENTAJA: esta es la definicion con la que el emisor de
//! x86-64 (S4) se va a comparar BIT A BIT. Si cada uno tuviera la suya, la
//! prueba diferencial compararia dos opiniones.
//!
//! Lo que SPIR-V deja abierto y aqui se CIERRA (el emisor tiene que hacer lo
//! mismo, y se dice en cada funcion):
//!
//! - `min`/`max` con un NaN: devuelven el SEGUNDO operando (`x < y ? x : y`).
//! - `inverse_sqrt` es `1 / sqrt(x)`, dos redondeos; nunca una aproximacion.
//! - `mix(x, y, a)` es `x * (1 - a) + y * a`, en ese orden, sin fusionar.
//! - la conversion de flotante a entero SATURA (NaN -> 0).
//!
//! Las que redondean una vez (`sqrt`, `fma`, `floor`...) son EXACTAS: el
//! banco las compara contra la biblioteca estandar en millones de casos.
//!
//! [consumo]  NADA   funciones puras

/// `|x|`: borra el signo, tambien de un NaN.
pub fn abs(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0x7FFF_FFFF)
}

/// `-x` de SPIR-V (`OpFNegate`): voltea el signo; no es `0 - x`.
pub fn neg(x: f32) -> f32 {
    f32::from_bits(x.to_bits() ^ 0x8000_0000)
}

/// Hacia cero.
pub fn trunc(x: f32) -> f32 {
    let b = x.to_bits();
    let e = ((b >> 23) & 0xFF) as i32 - 127;
    if e >= 23 {
        return x; // ya es entero, o infinito, o NaN
    }
    if e < 0 {
        return f32::from_bits(b & 0x8000_0000); // +-0
    }
    let mascara = 0x007F_FFFFu32 >> e;
    f32::from_bits(b & !mascara)
}

/// Hacia menos infinito.
pub fn floor(x: f32) -> f32 {
    let t = trunc(x);
    if x < t {
        t - 1.0
    } else {
        t
    }
}

/// Hacia mas infinito.
pub fn ceil(x: f32) -> f32 {
    let t = trunc(x);
    if x > t {
        t + 1.0
    } else {
        t
    }
}

/// `x - floor(x)`, como lo define GLSL.
pub fn fract(x: f32) -> f32 {
    x - floor(x)
}

/// La raiz entera por defecto de `n`.
fn isqrt(n: u64) -> u64 {
    let mut x = n;
    let mut r = 0u64;
    let mut bit = 1u64 << 62;
    while bit > n {
        bit >>= 2;
    }
    while bit != 0 {
        if x >= r + bit {
            x -= r + bit;
            r = (r >> 1) + bit;
        } else {
            r >>= 1;
        }
        bit >>= 2;
    }
    r
}

/// Raiz cuadrada, redondeada al mas cercano (exacta como la de IEEE 754).
pub fn sqrt(x: f32) -> f32 {
    let b = x.to_bits();
    if x.is_nan() {
        return x;
    }
    if b & 0x7FFF_FFFF == 0 {
        return x; // +-0
    }
    if b >> 31 != 0 {
        return f32::from_bits(0x7FC0_0000); // negativo: NaN
    }
    if x.is_infinite() {
        return x;
    }
    let mut e = ((b >> 23) & 0xFF) as i32;
    let mut m = b & 0x007F_FFFF;
    if e == 0 {
        // Subnormal: se normaliza.
        while m & 0x0080_0000 == 0 {
            m <<= 1;
            e -= 1;
        }
        e += 1;
    } else {
        m |= 0x0080_0000;
    }
    e -= 127;
    // x = m * 2^(e-23). Con exponente par: sqrt(x) = sqrt(m * 2^23) * 2^(e/2 - 23).
    if e & 1 != 0 {
        m <<= 1;
        e -= 1;
    }
    let n = (m as u64) << 23;
    let mut r = isqrt(n);
    // Nunca hay empate (r + 0.5)^2 = r^2 + r + 0.25 no es entero.
    if n - r * r > r {
        r += 1;
    }
    let mut exp = e / 2 + 127;
    if r >= 1 << 24 {
        r >>= 1;
        exp += 1;
    }
    f32::from_bits(((exp as u32) << 23) | (r as u32 & 0x007F_FFFF))
}

/// `1 / sqrt(x)`: dos redondeos. Ver la cabecera.
pub fn inverse_sqrt(x: f32) -> f32 {
    1.0 / sqrt(x)
}

/// `a * b + c` con UN solo redondeo.
///
/// El producto de dos `f32` cabe exacto en un `f64`; la suma no, y redondear
/// a `f64` y luego a `f32` puede redondear mal dos veces. Se evita con
/// "redondeo a impar": si la suma en `f64` no fue exacta, se fuerza a impar su
/// ultimo bit hacia el lado del valor exacto, y entonces el segundo redondeo
/// (a `f32`, con 29 bits de sobra) sale bien.
pub fn fma(a: f32, b: f32, c: f32) -> f32 {
    let p = a as f64 * b as f64;
    let c = c as f64;
    let s = p + c;
    if !s.is_finite() {
        return s as f32;
    }
    // El error exacto de la suma (TwoSum).
    let bb = s - p;
    let err = (p - (s - bb)) + (c - bb);
    let mut bits = s.to_bits();
    if err != 0.0 && bits & 1 == 0 {
        if (err > 0.0) == (s > 0.0) {
            bits += 1;
        } else {
            bits -= 1;
        }
    }
    f64::from_bits(bits) as f32
}

/// `x < y ? x : y`. Con un NaN devuelve `y`. Ver la cabecera.
pub fn min(x: f32, y: f32) -> f32 {
    if x < y {
        x
    } else {
        y
    }
}

/// `x > y ? x : y`. Con un NaN devuelve `y`.
pub fn max(x: f32, y: f32) -> f32 {
    if x > y {
        x
    } else {
        y
    }
}

/// `min(max(x, lo), hi)`.
pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    min(max(x, lo), hi)
}

/// `x * (1 - a) + y * a`, sin fusionar.
pub fn mix(x: f32, y: f32, a: f32) -> f32 {
    x * (1.0 - a) + y * a
}

/// `x < edge ? 0 : 1`.
pub fn step(edge: f32, x: f32) -> f32 {
    if x < edge {
        0.0
    } else {
        1.0
    }
}

/// `OpFRem`: el signo de `x`. `x - y * trunc(x / y)`.
pub fn rem(x: f32, y: f32) -> f32 {
    x - y * trunc(x / y)
}

/// `OpFMod`: el signo de `y`. `x - y * floor(x / y)`.
pub fn modulo(x: f32, y: f32) -> f32 {
    x - y * floor(x / y)
}

/// Flotante a entero sin signo, SATURANDO (NaN -> 0).
pub fn to_u32(x: f32) -> u32 {
    x as u32
}

/// Flotante a entero con signo, SATURANDO (NaN -> 0).
pub fn to_i32(x: f32) -> i32 {
    x as i32
}

// ---- S3b: las que no son una instruccion ----------------------------------
//
// Seno, coseno, exponencial, logaritmo y potencia no existen en SSE. Se
// escriben UNA vez, aqui, y las usan el oraculo Y el emisor (S4): si cada uno
// tuviera la suya, la prueba diferencial compararia dos opiniones.
//
// Se calculan en DOBLE precision --reduccion de rango + polinomio, todo con
// sumas y productos de IEEE, sin fusionar-- y se redondean a `f32` al final.
// Asi son deterministas por construccion (el emisor repite las mismas
// operaciones de `f64` en SSE2) y quedan muy por dentro de lo que Vulkan pide
// (seno/coseno: error absoluto 2^-11 en [-pi, pi]; exp/log: 3 ULP).
//
// [!] NO son "redondeo correcto" siempre: son UNA definicion, la de BMO-X.

/// El entero mas cercano (empates a par), en `f64`, para |y| < 2^51.
fn nearest(y: f64) -> f64 {
    const MAGIC: f64 = 6_755_399_441_055_744.0; // 1.5 * 2^52
    if y >= 0.0 {
        (y + MAGIC) - MAGIC
    } else {
        (y - MAGIC) + MAGIC
    }
}

/// `2^k` como `f64`, para k en el rango de los normales.
fn pow2(k: i64) -> f64 {
    f64::from_bits(((k + 1023) as u64) << 52)
}

const FRAC_PI_2_HI: f64 = 1.570_796_326_734_125_6; // los 33 bits altos de pi/2
const FRAC_PI_2_LO: f64 = 6.077_100_506_506_192e-11; // el resto
const FRAC_2_PI: f64 = 0.636_619_772_367_581_4;
const LN2_HI: f64 = 0.693_147_180_369_123_8;
const LN2_LO: f64 = 1.908_214_929_270_587_7e-10;
const INV_LN2: f64 = 1.442_695_040_888_963_4;

/// seno y coseno de `r` en [-pi/4, pi/4], por Taylor (error < 1e-15).
fn sin_cos_reducido(r: f64) -> (f64, f64) {
    let r2 = r * r;
    let s = r * (1.0 + r2 * (-1.0 / 6.0 + r2 * (1.0 / 120.0 + r2 * (-1.0 / 5040.0 + r2 * (1.0 / 362_880.0
        + r2 * (-1.0 / 39_916_800.0 + r2 * (1.0 / 6_227_020_800.0)))))));
    let c = 1.0 + r2 * (-0.5 + r2 * (1.0 / 24.0 + r2 * (-1.0 / 720.0 + r2 * (1.0 / 40_320.0
        + r2 * (-1.0 / 3_628_800.0 + r2 * (1.0 / 479_001_600.0 + r2 * (-1.0 / 87_178_291_200.0)))))));
    (s, c)
}

/// `(seno, coseno)` en doble. La reduccion usa pi/2 en dos trozos: exacta
/// hasta |x| ~ 2^20, que es mucho mas de lo que Vulkan promete.
fn sin_cos_f64(x: f64) -> (f64, f64) {
    if !x.is_finite() {
        return (f64::NAN, f64::NAN);
    }
    let k = nearest(x * FRAC_2_PI);
    let r = (x - k * FRAC_PI_2_HI) - k * FRAC_PI_2_LO;
    let (s, c) = sin_cos_reducido(r);
    match (k as i64).rem_euclid(4) {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// `e^x` en doble.
fn exp_f64(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x > 709.0 {
        return f64::INFINITY;
    }
    if x < -745.0 {
        return 0.0;
    }
    let k = nearest(x * INV_LN2);
    let r = (x - k * LN2_HI) - k * LN2_LO; // |r| <= ln2/2
    let mut p = 1.0 / 479_001_600.0; // 1/12!
    for d in [39_916_800.0, 3_628_800.0, 362_880.0, 40_320.0, 5040.0, 720.0, 120.0, 24.0, 6.0, 2.0, 1.0, 1.0] {
        p = p * r + 1.0 / d;
    }
    // p = sum r^i/i!; escalar por 2^k en dos pasos para no salirse de los normales.
    let k = k as i64;
    let medio = k / 2;
    p * pow2(medio) * pow2(k - medio)
}

/// `ln(x)` en doble, para x > 0 finito.
fn ln_f64(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return x;
    }
    let mut b = x.to_bits();
    let mut e = ((b >> 52) & 0x7FF) as i64;
    if e == 0 {
        // Subnormal en doble (no llega desde un f32, pero la funcion no lo sabe).
        let y = x * pow2(54);
        b = y.to_bits();
        e = ((b >> 52) & 0x7FF) as i64 - 54;
    }
    e -= 1023;
    let mut m = f64::from_bits((b & 0x000F_FFFF_FFFF_FFFF) | 0x3FF0_0000_0000_0000); // [1, 2)
    if m > core::f64::consts::SQRT_2 {
        m *= 0.5;
        e += 1;
    }
    // ln(m) = 2 * atanh(s), s = (m - 1) / (m + 1), |s| <= 0.1716.
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    let mut p = 1.0 / 21.0;
    for d in [19.0, 17.0, 15.0, 13.0, 11.0, 9.0, 7.0, 5.0, 3.0, 1.0] {
        p = p * s2 + 1.0 / d;
    }
    let e = e as f64;
    (2.0 * s * p + e * LN2_LO) + e * LN2_HI
}

/// Seno. `GLSL.std.450 Sin`.
pub fn sin(x: f32) -> f32 {
    sin_cos_f64(x as f64).0 as f32
}

/// Coseno. `GLSL.std.450 Cos`.
pub fn cos(x: f32) -> f32 {
    sin_cos_f64(x as f64).1 as f32
}

/// `e^x`. `GLSL.std.450 Exp`.
pub fn exp(x: f32) -> f32 {
    exp_f64(x as f64) as f32
}

/// `ln(x)`. `GLSL.std.450 Log`: x < 0 da NaN, x = 0 da -infinito.
pub fn log(x: f32) -> f32 {
    ln_f64(x as f64) as f32
}

/// `x^y = e^(y ln x)`, todo en doble y un redondeo al final. GLSL la deja
/// indefinida con x < 0 (aqui NaN) y con x = 0, y <= 0 (aqui: y < 0 da
/// infinito, y = 0 da NaN, que es lo que sale de la formula).
pub fn pow(x: f32, y: f32) -> f32 {
    exp_f64(y as f64 * ln_f64(x as f64)) as f32
}
