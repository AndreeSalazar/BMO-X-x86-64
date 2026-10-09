//! **LA FMA EXACTA DE f32, sin `libm`** (DL13, 09-10): `a * b + c` con UN
//! redondeo al mas cercano (el empate al par), como la FFMA de una GPU y la
//! `fmaf` de IEEE 754 -- con subnormales, ceros con signo, infinitos y NaN --.
//! Ring 3 no tiene `libm` (`f32::mul_add` es de `std`): esta es la de la casa,
//! la que corren las recetas de `cuentas.rs`.
//!
//! ```text
//!    el producto   a * b en f64: 24 + 24 bits caben en 53, EXACTO (de
//!                  2^-298 a 2^256: ni se pierde ni se sale)
//!    la suma       p + c en f64, redondeada A LO IMPAR: el resto de TwoSum
//!                  (Knuth) dice, exacto, si la suma redondeo; si lo hizo y
//!                  el ultimo bit salio par, un ULP hacia el resto
//!    a f32         al mas cercano: con 53 >= 24 + 2 bits, redondear a lo
//!                  impar y despues al mas cercano es redondear UNA vez
//!                  (Boldo y Melquiond, 2008) -- tambien a un subnormal de
//!                  f32, que en f64 es normal --
//! ```
//!
//! Se prueba contra la `fmaf` de la `std` del anfitrion y contra la FFMA del
//! simulador de cada tarjeta (otra cuenta, de enteros): tres que no comparten
//! nada, y los mismos bits.

/// `a * b + c`, con UN redondeo al mas cercano.
pub fn fma(a: f32, b: f32, c: f32) -> f32 {
    let p = a as f64 * b as f64;
    let c = c as f64;
    let s = p + c;
    if !s.is_finite() {
        // Un infinito o un NaN: el de IEEE (inf - inf, 0 * inf, un NaN).
        return s as f32;
    }
    // TwoSum: s + resto = p + c, exacto.
    let v = s - p;
    let resto = (p - (s - v)) + (c - v);
    if resto != 0.0 && s.to_bits() & 1 == 0 {
        // A lo impar: un ULP hacia el resto (s no es 0: con resto, p + c no
        // lo es).
        let hacia_arriba = (resto > 0.0) == (s > 0.0);
        let b = s.to_bits();
        return f64::from_bits(if hacia_arriba { b + 1 } else { b - 1 }) as f32;
    }
    s as f32
}
