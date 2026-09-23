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
