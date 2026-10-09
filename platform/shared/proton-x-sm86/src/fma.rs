//! **LA FFMA DE LA 3060, EXACTA** (DL10 de `docs/plan/PLAN_LAS_LIBRERIAS.md`,
//! 09-10): `a * b + c` con UN redondeo, en sus cuatro modos (`.RN`, `.RM`,
//! `.RP`, `.RZ`), sin `libm` ni un intrinseco: lo que el simulador necesita
//! para correr la division exacta (`cociente.rs`), que vive de ella.
//!
//! ```text
//!    el producto   dos f32 (24 bits cada uno) multiplicados en f64 (53):
//!                  EXACTO, y sin desbordar (de 2^-298 a 2^256)
//!    la suma       f64, con su error exacto (TwoSum de Knuth): s + t es
//!                  a * b + c sin perder un bit
//!    el redondeo   a IMPAR en f64 (si t no es 0 y s es par, el vecino de s
//!                  del lado de t) y despues a f32 en el modo pedido: con dos
//!                  bits de sobra, redondear a impar y luego a f32 es
//!                  redondear UNA vez (Boldo y Melquiond, 2008)
//! ```
//!
//! Lo que no es un numero, como IEEE: un NaN en cualquiera, `inf * 0` o
//! `inf - inf` dan NaN (el canonico de la 3060, 0x7FFFFFFF: la carga de un NaN
//! no se promete); un infinito sigue siendo infinito.

pub use bmo_sm86::codifica::Redondeo;

/// **`a * b + c`**, sus bits, con un redondeo en `modo`.
pub fn ffma(a: u32, b: u32, c: u32, modo: Redondeo) -> u32 {
    let (fa, fb, fc) = (f32::from_bits(a), f32::from_bits(b), f32::from_bits(c));
    if !fa.is_finite() || !fb.is_finite() || !fc.is_finite() {
        // En f64 dan lo mismo que IEEE en f32: NaN, o un infinito exacto.
        let r = (fa as f64) * (fb as f64) + (fc as f64);
        return if r.is_nan() { 0x7FFF_FFFF } else { (r as f32).to_bits() };
    }
    let p = fa as f64 * fb as f64;
    let q = fc as f64;
    let s = p + q;
    // TwoSum: el error exacto de la suma (s + t == p + q).
    let bb = s - p;
    let t = (p - (s - bb)) + (q - bb);
    if s == 0.0 && t == 0.0 {
        // Un cero exacto: el de los dos si son ceros del mismo signo; si no,
        // +0, salvo hacia abajo (IEEE 754, 6.3).
        let negativo = if p == 0.0 && q == 0.0 && p.is_sign_negative() == q.is_sign_negative() { p.is_sign_negative() } else { modo == Redondeo::Abajo };
        return if negativo { 0x8000_0000 } else { 0 };
    }
    // A impar: el vecino de s del lado de t, si s es par.
    let mut v = s;
    if t != 0.0 && v.to_bits() & 1 == 0 {
        let crece = (t > 0.0) == (v > 0.0);
        v = f64::from_bits(if crece { v.to_bits() + 1 } else { v.to_bits() - 1 });
    }
    a_f32(v, modo)
}

/// Un f64 a f32 en `modo` (el del mas cercano es el `as` de Rust: IEEE, con
/// subnormales y el infinito al desbordar).
pub fn a_f32(v: f64, modo: Redondeo) -> u32 {
    let r = v as f32;
    let rb = r.to_bits();
    if r as f64 == v || v.is_nan() {
        return rb;
    }
    // r es el mas cercano: queda por encima o por debajo de v.
    let encima = r as f64 > v;
    match modo {
        Redondeo::Cercano => rb,
        Redondeo::Abajo if encima => hacia(rb, false),
        Redondeo::Arriba if !encima => hacia(rb, true),
        Redondeo::Cero if encima == (v > 0.0) => hacia(rb, v < 0.0),
        _ => rb,
    }
}

/// El f32 vecino de `x` hacia arriba (mas infinito) o hacia abajo.
fn hacia(x: u32, arriba: bool) -> u32 {
    let negativo = x & 0x8000_0000 != 0;
    match (arriba, negativo) {
        // de -0 hacia arriba no se llega aqui: -0 nunca queda por debajo
        (true, false) | (false, true) => x + 1,
        (true, true) | (false, false) => x - 1,
    }
}

#[cfg(test)]
mod pruebas {
    extern crate std;

    use super::*;

    /// Un generador de bits, el de siempre de la casa.
    struct Azar(u64);

    impl Azar {
        fn bits(&mut self) -> u32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 32) as u32
        }

        /// Un f32 con un exponente cerca de `centro` (y a veces cualquiera).
        fn cerca(&mut self, centro: i32, ancho: i32) -> u32 {
            let e = (centro + (self.bits() % (2 * ancho as u32 + 1)) as i32 - ancho).clamp(0, 254) as u32;
            (self.bits() & 0x8000_0000) | e << 23 | (self.bits() & 0x7F_FFFF)
        }
    }

    /// ** AL MAS CERCANO, la de Rust (`mul_add`: la `fmaf` de la libreria del
    /// sistema, redondeo correcto), en bits al azar de todas las clases.
    #[test]
    fn al_mas_cercano_es_la_fma_de_ieee() {
        let mut z = Azar(0x5eed);
        let especiales = [0u32, 0x8000_0000, 1, 0x8000_0001, 0x007F_FFFF, 0x0080_0000, 0x7F7F_FFFF, 0xFF7F_FFFF, 0x7F80_0000, 0xFF80_0000, 0x7FC0_0000, 0x3F80_0000, 0xBF80_0000];
        let mut casos = std::vec::Vec::new();
        for &a in &especiales {
            for &b in &especiales {
                for &c in &especiales {
                    casos.push((a, b, c));
                }
            }
        }
        for _ in 0..300_000 {
            let e = (z.bits() % 254) as i32;
            casos.push((z.bits(), z.bits(), z.bits()));
            // el producto y la suma del mismo orden: donde se cancela
            casos.push((z.cerca(e, 3), z.cerca(127, 3), z.cerca(e, 3)));
            casos.push((z.cerca(20, 20), z.cerca(20, 20), z.cerca(0, 5)));
        }
        for (a, b, c) in casos {
            let (fa, fb, fc) = (f32::from_bits(a), f32::from_bits(b), f32::from_bits(c));
            let quiere = fa.mul_add(fb, fc);
            let da = f32::from_bits(ffma(a, b, c, Redondeo::Cercano));
            assert!(da.to_bits() == quiere.to_bits() || (da.is_nan() && quiere.is_nan()), "{a:#x} * {b:#x} + {c:#x}: {da:?} y no {quiere:?}");
        }
    }

    /// Un valor EXACTO: un entero por una potencia de dos.
    type Exacto = (i128, i32);

    /// El de un f32 finito.
    fn valor(x: u32) -> Exacto {
        let e = (x >> 23 & 0xFF) as i32;
        let m = (x & 0x7F_FFFF) as i128 | if e == 0 { 0 } else { 1 << 23 };
        (if x >> 31 == 1 { -m } else { m }, e.max(1) - 150)
    }

    /// Los dos a la misma potencia, la menor (si cabe en un i128).
    fn alinear(x: Exacto, y: Exacto) -> Option<(i128, i128)> {
        let base = x.1.min(y.1);
        let (dx, dy) = (x.1 - base, y.1 - base);
        if dx > 70 || dy > 70 {
            return None;
        }
        Some((x.0 << dx, y.0 << dy))
    }

    /// `a * b + c`, exacto.
    fn exacto(a: u32, b: u32, c: u32) -> Option<Exacto> {
        let (va, vb) = (valor(a), valor(b));
        let p = (va.0 * vb.0, va.1 + vb.1);
        let (x, y) = alinear(p, valor(c))?;
        Some((x + y, p.1.min(valor(c).1)))
    }

    fn compara(x: Exacto, y: Exacto) -> Option<core::cmp::Ordering> {
        alinear(x, y).map(|(a, b)| a.cmp(&b))
    }

    /// ** LOS TRES DIRIGIDOS: abajo y arriba rodean el valor exacto (contado
    /// con enteros) y son vecinos -- o el mismo, si es exacto --; hacia el
    /// cero es uno de ellos, el de menor magnitud; y el mas cercano, uno de
    /// los dos.
    #[test]
    fn los_redondeos_dirigidos_dan_su_vecino() {
        use core::cmp::Ordering::*;
        let mut z = Azar(0xd1e5);
        let mut vistos = 0;
        for _ in 0..300_000 {
            let e = 40 + (z.bits() % 60) as i32;
            // el producto cerca de c (donde se cancela), o lejos
            let (a, b) = (z.cerca(e, 6), z.cerca(127, 6));
            let c = if z.bits() & 1 == 0 { z.cerca(e, 4) } else { z.cerca(e - 20, 20) };
            let Some(v) = exacto(a, b, c) else { continue };
            let [rn, rm, rp, rz] = [Redondeo::Cercano, Redondeo::Abajo, Redondeo::Arriba, Redondeo::Cero].map(|m| ffma(a, b, c, m));
            let (Some(om), Some(op)) = (compara(valor(rm), v), compara(valor(rp), v)) else { continue };
            assert!(om != Greater && op != Less, "{a:#x} {b:#x} {c:#x}: abajo {rm:#x} y arriba {rp:#x} no rodean el valor");
            if om == Equal || op == Equal {
                assert!(om == Equal && op == Equal && rn == rm && rz == rm, "{a:#x} {b:#x} {c:#x}: exacto, y los cuatro iguales ({rn:#x} {rm:#x} {rp:#x} {rz:#x})");
            } else {
                let arriba = |x: u32| if x == 0x8000_0000 { 1 } else if x >> 31 == 0 { x + 1 } else { x - 1 };
                assert!(rp == arriba(rm) || (rm == 0x8000_0001 && rp == 0x8000_0000), "{a:#x} {b:#x} {c:#x}: {rm:#x} y {rp:#x} no son vecinos");
                let positivo = v.0 > 0;
                assert_eq!(rz, if positivo { rm } else { rp }, "{a:#x} {b:#x} {c:#x}: hacia el cero");
                assert!(rn == rm || rn == rp, "{a:#x} {b:#x} {c:#x}: el mas cercano {rn:#x}");
            }
            vistos += 1;
        }
        assert!(vistos > 200_000, "solo {vistos}");
    }
}
