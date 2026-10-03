//! **La matematica de los sombreadores** (03-10, N5.6): lo que DXIL llama
//! `dx.op.unary` y la casa no sabia -- seno, coseno, tangente, exp2, log2,
//! frac, los redondeos y los medios floats; y (03-10, la sexta corrida
//! pidio la 15) los arcos y los hiperbolicos.
//!
//! [carril]  VERDE     cuentas; no toca la maquina
//! [cuesta]  DATO      una cuenta mal hecha pinta otra luz (la niebla, el
//!                     brillo, las olas salen de estas cuatro funciones)
//! [riesgo]  ESPEJO    se comparan con la `std` del anfitrion (libm) en el
//!                     banco, punto a punto, dentro de lo que pide D3D
//!                     (unos pocos ULP; D3D no pide mas a una GPU)
//! [consumo] NADA      solo cuando un sombreador las usa
//!
//! Ring 3 no tiene `libm`: aqui todo es suma y producto. Las cuentas van en
//! f64 por dentro (series cortas y exactas de sobra para un f32) y salen en
//! f32, que es lo que guarda un registro del sombreador.
//!
//! ```text
//!    DXIL  12 Cos   13 Sin   14 Tan   15 Acos   16 Asin   17 Atan
//!          18 Hcos  19 Hsin  20 Htan  21 Exp (2^x)   22 Frc   23 Log (log2)
//!          26 Round_ne (al par)  27 Round_ni (suelo)  28 Round_pi (techo)
//!          29 Round_z (truncar)   130 f32tof16   131 f16tof32
//! ```

use core::f64::consts::{FRAC_PI_2, LN_2, PI};

/// **Que funcion** (lo que guarda `Op::Mate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mate {
    Sin,
    Cos,
    Tan,
    Exp2,
    Log2,
    Frac,
    RedondoPar,
    Suelo,
    Techo,
    Trunca,
    /// Los 16 bits de abajo (un half) a float.
    F16aF32,
    /// Un float a half: sus 16 bits, como entero (sale como BITS).
    F32aF16,
    Acos,
    Asin,
    Atan,
    Cosh,
    Senh,
    Tanh,
}

impl Mate {
    /// La de la operacion `op` de DXIL, si es una de estas.
    pub fn de_dxil(op: i64) -> Option<Mate> {
        Some(match op {
            12 => Mate::Cos,
            13 => Mate::Sin,
            14 => Mate::Tan,
            15 => Mate::Acos,
            16 => Mate::Asin,
            17 => Mate::Atan,
            18 => Mate::Cosh,
            19 => Mate::Senh,
            20 => Mate::Tanh,
            21 => Mate::Exp2,
            22 => Mate::Frac,
            23 => Mate::Log2,
            26 => Mate::RedondoPar,
            27 => Mate::Suelo,
            28 => Mate::Techo,
            29 => Mate::Trunca,
            130 => Mate::F32aF16,
            131 => Mate::F16aF32,
            _ => return None,
        })
    }

    /// **Aplicarla** a los BITS de un registro (un float, o un entero en
    /// los medios floats); devuelve los bits del resultado.
    pub fn aplicar(self, a: u32) -> u32 {
        let x = f32::from_bits(a);
        match self {
            Mate::Sin => seno(x).to_bits(),
            Mate::Cos => coseno(x).to_bits(),
            Mate::Tan => tangente(x).to_bits(),
            Mate::Exp2 => exp2(x).to_bits(),
            Mate::Log2 => log2(x).to_bits(),
            Mate::Frac => frac(x).to_bits(),
            Mate::RedondoPar => redondo_par(x).to_bits(),
            Mate::Suelo => suelo(x).to_bits(),
            Mate::Techo => (-suelo(-x)).to_bits(),
            Mate::Trunca => trunca(x).to_bits(),
            Mate::F16aF32 => de_medio(a as u16).to_bits(),
            Mate::F32aF16 => a_medio(x) as u32,
            Mate::Acos => (FRAC_PI_2 - arcoseno64(x as f64)).to_bits_f32(),
            Mate::Asin => arcoseno64(x as f64).to_bits_f32(),
            Mate::Atan => arcotangente64(x as f64).to_bits_f32(),
            Mate::Cosh => {
                let e = exp64(x.abs() as f64);
                ((e + 1.0 / e) * 0.5).to_bits_f32()
            }
            Mate::Senh => senh64(x as f64).to_bits_f32(),
            Mate::Tanh => tanh64(x as f64).to_bits_f32(),
        }
    }
}

/// Lo que ya es entero (o NaN, o infinito) en un f32: |x| >= 2^23.
fn ya_entero(x: f32) -> bool {
    !(x.abs() < 8_388_608.0)
}

fn trunca(x: f32) -> f32 {
    if ya_entero(x) {
        x
    } else {
        // El signo del cero se conserva (-0.5 trunca a -0).
        f32::from_bits(((x as i32) as f32).to_bits() | (x.to_bits() & 0x8000_0000))
    }
}

/// `floor`, sin `libm`.
pub fn suelo(x: f32) -> f32 {
    let t = trunca(x);
    if t > x {
        t - 1.0
    } else {
        t
    }
}

/// `frac` de HLSL: `x - floor(x)`.
fn frac(x: f32) -> f32 {
    x - suelo(x)
}

/// Al entero mas cercano; empate, al PAR (lo de `round_ne` y del FPU).
fn redondo_par(x: f32) -> f32 {
    if ya_entero(x) {
        return x;
    }
    // En f64, `x + 0.5` es exacto: no hay sorpresa con 0.49999997.
    let d = x as f64;
    let mut r = (d + 0.5) as i64 as f64;
    if r > d + 0.5 {
        r -= 1.0;
    }
    // Empate exacto: al par.
    if r - d == 0.5 && (r as i64) % 2 != 0 {
        r -= 1.0;
    }
    f32::from_bits((r as f32).to_bits() | (x.to_bits() & 0x8000_0000))
}

/// El seno de `r` en [-pi/2, pi/2], por su serie (hasta x^15: el error, por
/// debajo de 1e-10).
fn seno_centrado(r: f64) -> f64 {
    let r2 = r * r;
    let mut t = r;
    let mut s = r;
    for k in 1..8 {
        t *= -r2 / ((2 * k) as f64 * (2 * k + 1) as f64);
        s += t;
    }
    s
}

/// `floor` de un f64 (los que llegan aqui caben en un i64).
fn suelo64(x: f64) -> f64 {
    let t = x as i64 as f64;
    if t > x {
        t - 1.0
    } else {
        t
    }
}

/// El seno de cualquier `x`: se lleva a [-pi, pi] y despues a [-pi/2, pi/2]
/// con sin(pi - r) = sin(r).
fn seno64(x: f64) -> f64 {
    let vueltas = suelo64(x / (2.0 * PI) + 0.5);
    let mut r = x - vueltas * 2.0 * PI;
    if r > FRAC_PI_2 {
        r = PI - r;
    } else if r < -FRAC_PI_2 {
        r = -PI - r;
    }
    seno_centrado(r)
}

fn seno(x: f32) -> f32 {
    if !x.is_finite() {
        return f32::NAN;
    }
    seno64(x as f64) as f32
}

fn coseno(x: f32) -> f32 {
    if !x.is_finite() {
        return f32::NAN;
    }
    seno64(x as f64 + FRAC_PI_2) as f32
}

fn tangente(x: f32) -> f32 {
    if !x.is_finite() {
        return f32::NAN;
    }
    let d = x as f64;
    (seno64(d) / seno64(d + FRAC_PI_2)) as f32
}

/// 2^x.
fn exp2(x: f32) -> f32 {
    if x.is_nan() {
        return x;
    }
    if x >= 128.0 {
        return f32::INFINITY;
    }
    if x < -150.0 {
        return 0.0;
    }
    let n = suelo(x) as i64;
    let f = (x as f64 - n as f64) * LN_2;
    // e^f con f en [0, ln 2): la serie, hasta f^14.
    let mut t = 1.0f64;
    let mut s = 1.0f64;
    for k in 1..15 {
        t *= f / k as f64;
        s += t;
    }
    // Por 2^n, armando el exponente de un f64 (n cabe de sobra).
    (s * f64::from_bits(((n + 1023) as u64) << 52)) as f32
}

/// log2(x).
fn log2(x: f32) -> f32 {
    if x.is_nan() || x < 0.0 {
        return f32::NAN;
    }
    if x == 0.0 {
        return f32::NEG_INFINITY;
    }
    if x.is_infinite() {
        return x;
    }
    // x = m * 2^e con m en [1, 2) (los subnormales, por 2^64 primero).
    let (mut d, mut e) = (x as f64, 0i64);
    if x < f32::MIN_POSITIVE {
        d *= 18_446_744_073_709_551_616.0;
        e -= 64;
    }
    let b = d.to_bits();
    e += ((b >> 52) & 0x7FF) as i64 - 1023;
    let mut m = f64::from_bits((b & 0x000F_FFFF_FFFF_FFFF) | (1023u64 << 52));
    // m en [raiz(1/2), raiz(2)): la serie converge mas deprisa.
    if m > core::f64::consts::SQRT_2 {
        m *= 0.5;
        e += 1;
    }
    // ln(m) = 2 atanh(z), z = (m - 1) / (m + 1), |z| < 0.172.
    let z = (m - 1.0) / (m + 1.0);
    let z2 = z * z;
    let (mut t, mut s) = (z, z);
    for k in 1..12 {
        t *= z2;
        s += t / (2 * k + 1) as f64;
    }
    (e as f64 + 2.0 * s / LN_2) as f32
}

/// Un f64 a los bits de su f32 (lo que guarda un registro).
trait AF32 {
    fn to_bits_f32(self) -> u32;
}
impl AF32 for f64 {
    fn to_bits_f32(self) -> u32 {
        (self as f32).to_bits()
    }
}

/// La raiz de un f64 >= 0, por Newton (sin `libm`): el exponente a la
/// mitad para empezar, y seis vueltas.
fn raiz64(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 || x.is_infinite() {
        return x;
    }
    let mut r = f64::from_bits((x.to_bits() >> 1) + (1023u64 << 51));
    for _ in 0..6 {
        r = 0.5 * (r + x / r);
    }
    r
}

/// atan(x). Lo de |x| > 1 va a 1/x (pi/2 - atan(1/x)); lo de mas de
/// tan(pi/12), a la identidad de pi/6; ahi la serie corre deprisa (x^2 <
/// 0.072).
fn arcotangente64(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    let (signo, mut a) = (if x < 0.0 { -1.0 } else { 1.0 }, x.abs());
    let mut suma = 0.0;
    if a > 1.0 {
        // pi/2 - atan(1/a); con a infinito, 1/a = 0.
        suma = FRAC_PI_2;
        a = -1.0 / a;
    }
    const RAIZ3: f64 = 1.732_050_807_568_877_2;
    const TAN_PI_12: f64 = 0.267_949_192_431_122_7;
    let (mut extra, mut b) = (0.0, a);
    if b.abs() > TAN_PI_12 {
        extra = b.signum() * PI / 6.0;
        b = (b.abs() * RAIZ3 - 1.0) / (b.abs() + RAIZ3) * b.signum();
    }
    let b2 = b * b;
    let (mut t, mut s) = (b, b);
    for k in 1..14 {
        t *= -b2;
        s += t / (2 * k + 1) as f64;
    }
    signo * (suma + extra + s)
}

/// asin(x) = atan(x / raiz(1 - x^2)); fuera de [-1, 1], NaN.
fn arcoseno64(x: f64) -> f64 {
    if x.is_nan() || x.abs() > 1.0 {
        return f64::NAN;
    }
    if x.abs() == 1.0 {
        return x * FRAC_PI_2;
    }
    arcotangente64(x / raiz64((1.0 - x) * (1.0 + x)))
}

/// e^x en f64 (la misma serie que [`exp2`]).
fn exp64(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x > 709.0 {
        return f64::INFINITY;
    }
    if x < -745.0 {
        return 0.0;
    }
    let n = suelo64(x / LN_2);
    let f = x - n * LN_2;
    let (mut t, mut s) = (1.0f64, 1.0f64);
    for k in 1..18 {
        t *= f / k as f64;
        s += t;
    }
    // 2^n en dos mitades: n llega a -1075 y un solo exponente no cabe.
    let (n1, n2) = ((n / 2.0) as i64, (n - (n / 2.0) as i64 as f64) as i64);
    s * f64::from_bits(((n1 + 1023) as u64) << 52) * f64::from_bits(((n2 + 1023) as u64) << 52)
}

/// senh(x): la serie cerca de 0 (restar dos e^x casi iguales perderia todo).
fn senh64(x: f64) -> f64 {
    if x.abs() < 0.5 {
        let x2 = x * x;
        let (mut t, mut s) = (x, x);
        for k in 1..10 {
            t *= x2 / ((2 * k) * (2 * k + 1)) as f64;
            s += t;
        }
        return s;
    }
    let e = exp64(x.abs());
    x.signum() * (e - 1.0 / e) * 0.5
}

/// tanh(x): senh / cosh, y +-1 lejos de 0.
fn tanh64(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x.abs() > 20.0 {
        return x.signum();
    }
    let e = exp64(x.abs());
    senh64(x) / ((e + 1.0 / e) * 0.5)
}

/// Un half (IEEE 754 binario16) a f32.
pub fn de_medio(h: u16) -> f32 {
    let signo = ((h >> 15) as u32) << 31;
    let exp = ((h >> 10) & 0x1F) as u32;
    let man = (h & 0x3FF) as u32;
    let bits = match (exp, man) {
        (0, 0) => signo,
        // Subnormal: man * 2^-24, con su signo.
        (0, m) => {
            let v = m as f32 * (1.0 / 16_777_216.0);
            return if signo != 0 { -v } else { v };
        }
        (31, 0) => signo | 0x7F80_0000,
        (31, m) => signo | 0x7FC0_0000 | (m << 13),
        (e, m) => signo | ((e + 112) << 23) | (m << 13),
    };
    f32::from_bits(bits)
}

/// Un f32 a half, al mas cercano (empate al par), como `f32tof16`.
pub fn a_medio(x: f32) -> u16 {
    let b = x.to_bits();
    let signo = ((b >> 16) & 0x8000) as u16;
    if x.is_nan() {
        return signo | 0x7E00;
    }
    let a = x.abs();
    if a >= 65_520.0 {
        return signo | 0x7C00;
    }
    if a < 6.103_515_6e-5 {
        // Subnormal (o cero): a / 2^-24, redondeado al par.
        let q = redondo_par(a * 16_777_216.0) as u16;
        return signo | q;
    }
    let e = ((b >> 23) & 0xFF) as i32 - 127 + 15;
    let man = b & 0x7F_FFFF;
    let mut h = ((e as u32) << 10) | (man >> 13);
    let resto = man & 0x1FFF;
    if resto > 0x1000 || (resto == 0x1000 && h & 1 != 0) {
        h += 1;
    }
    signo | h as u16
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;

    /// La diferencia en ULP entre dos f32 del mismo signo.
    fn ulp(a: f32, b: f32) -> u32 {
        (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs() as u32
    }

    #[test]
    fn seno_coseno_y_tangente_contra_la_libm() {
        let mut x = -100.0f32;
        while x < 100.0 {
            let (s, c) = (seno(x), coseno(x));
            assert!((s - x.sin()).abs() <= 2e-7_f32.max(x.sin().abs() * 3e-7), "sin {x}: {s} vs {}", x.sin());
            assert!((c - x.cos()).abs() <= 2e-7_f32.max(x.cos().abs() * 3e-7), "cos {x}: {c} vs {}", x.cos());
            if x.cos().abs() > 0.01 {
                let t = tangente(x);
                assert!((t - x.tan()).abs() <= 1e-6 * x.tan().abs().max(1.0), "tan {x}: {t} vs {}", x.tan());
            }
            x += 0.0173;
        }
        assert!(seno(f32::INFINITY).is_nan());
    }

    /// Los arcos y los hiperbolicos (03-10), contra la `std`.
    #[test]
    fn arcos_e_hiperbolicos_contra_la_libm() {
        let cerca = |a: f32, b: f32, que: &str, x: f32| assert!((a - b).abs() <= 3e-7 * b.abs().max(1.0), "{que} {x}: {a} vs {b}");
        let mut x = -1.0f32;
        while x <= 1.0 {
            cerca(f32::from_bits(Mate::Acos.aplicar(x.to_bits())), x.acos(), "acos", x);
            cerca(f32::from_bits(Mate::Asin.aplicar(x.to_bits())), x.asin(), "asin", x);
            x += 0.0071;
        }
        for x in [-1.0f32, 1.0, 0.0, -0.0, 0.999_999_9] {
            cerca(f32::from_bits(Mate::Acos.aplicar(x.to_bits())), x.acos(), "acos", x);
        }
        assert!(f32::from_bits(Mate::Asin.aplicar(1.5f32.to_bits())).is_nan());
        let mut x = -60.0f32;
        while x < 60.0 {
            cerca(f32::from_bits(Mate::Atan.aplicar(x.to_bits())), x.atan(), "atan", x);
            x += 0.0613;
        }
        cerca(f32::from_bits(Mate::Atan.aplicar(f32::INFINITY.to_bits())), core::f32::consts::FRAC_PI_2, "atan", f32::INFINITY);
        let mut x = -12.0f32;
        while x < 12.0 {
            let rel = |a: f32, b: f32, que: &str| assert!((a - b).abs() <= 3e-7 * b.abs().max(1e-30) + 1e-37, "{que} {x}: {a} vs {b}");
            rel(f32::from_bits(Mate::Cosh.aplicar(x.to_bits())), x.cosh(), "cosh");
            rel(f32::from_bits(Mate::Senh.aplicar(x.to_bits())), x.sinh(), "senh");
            rel(f32::from_bits(Mate::Tanh.aplicar(x.to_bits())), x.tanh(), "tanh");
            x += 0.0437;
        }
        assert_eq!(f32::from_bits(Mate::Tanh.aplicar(50.0f32.to_bits())), 1.0);
    }

    #[test]
    fn exp2_y_log2_contra_la_libm() {
        let mut x = -149.0f32;
        while x < 127.9 {
            let (a, b) = (exp2(x), x.exp2());
            assert!(ulp(a, b) <= 2 || (a - b).abs() < 1e-44, "exp2 {x}: {a} vs {b}");
            x += 0.371;
        }
        for x in [1e-40f32, 1e-30, 0.001, 0.5, 0.7071, 1.0, 1.5, 2.0, 3.0, 10.0, 1e10, 3e38] {
            assert!((log2(x) - x.log2()).abs() <= 2e-7 * x.log2().abs().max(1.0), "log2 {x}: {} vs {}", log2(x), x.log2());
        }
        assert_eq!(log2(1.0), 0.0);
        assert_eq!(log2(8.0), 3.0);
        assert!(log2(-1.0).is_nan());
        assert_eq!(log2(0.0), f32::NEG_INFINITY);
        assert_eq!(exp2(200.0), f32::INFINITY);
    }

    #[test]
    fn los_redondeos_y_frac() {
        for x in [-2.5f32, -1.5, -0.5, -0.49999997, 0.49999997, 0.5, 1.5, 2.5, 3.7, -3.7, 1e9, -0.0, 8_388_609.0] {
            assert_eq!(redondo_par(x), x.round_ties_even(), "round_ne {x}");
            assert_eq!(suelo(x), x.floor(), "floor {x}");
            assert_eq!(-suelo(-x), x.ceil(), "ceil {x}");
            assert_eq!(trunca(x), x.trunc(), "trunc {x}");
            assert_eq!(trunca(x).to_bits() >> 31, x.trunc().to_bits() >> 31, "el signo del cero, {x}");
        }
        assert_eq!(frac(-0.25), 0.75, "frac de HLSL: x - floor(x)");
        assert_eq!(frac(2.75), 0.75);
    }

    #[test]
    fn los_medios_floats_ida_y_vuelta() {
        // Todos los halfs: a f32 y de vuelta, el mismo (salvo el NaN, que se
        // queda NaN).
        for h in 0..=u16::MAX {
            let f = de_medio(h);
            if f.is_nan() {
                assert!(de_medio(a_medio(f)).is_nan());
                continue;
            }
            assert_eq!(a_medio(f), h, "half {h:#06x} = {f}");
        }
        assert_eq!(de_medio(0x3C00), 1.0);
        assert_eq!(de_medio(0xC000), -2.0);
        assert_eq!(a_medio(1.0 + 1.0 / 4096.0), 0x3C00, "empate: al par");
        assert_eq!(a_medio(1e6), 0x7C00, "fuera: infinito");
        assert_eq!(Mate::F32aF16.aplicar(0.5f32.to_bits()), 0x3800);
        assert_eq!(f32::from_bits(Mate::F16aF32.aplicar(0x3800)), 0.5);
    }
}
