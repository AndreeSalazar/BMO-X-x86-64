//! ** DL13 (09-10): las Mate de SERIES como recetas de f32 -- su error contra
//! el f64 de la `std` del anfitrion (en ULP, sobre una rejilla densa de cada
//! dominio y valores al azar), lo que tienen que dar EXACTO (los ceros con
//! su signo, 2^n de un entero, log2 de una potencia de dos, los infinitos),
//! las simetrias, y la FMA de la casa contra la `fmaf` de la `std`.
//!
//! El error de TODOS los f32 lo mide `todos_los_f32` (ignorada: con
//! `cargo test --release -p bmo-prometeo -- --ignored`, unos minutos por
//! funcion); lo que dio el 09-10 esta en `LIMITES` y en E8d de
//! `docs/plan/PLAN_LA_LENGUA_DE_LA_3060.md`.

extern crate std;

use alloc::vec::Vec;

use crate::cuentas::{en_la_casa, es_de_series};
use crate::fma::fma;
use crate::mates::Mate;

/// La distancia en ULP de dos f32 (por su orden: cruza el cero).
fn ulps(a: f32, b: f32) -> u64 {
    if a.is_nan() && b.is_nan() {
        return 0;
    }
    if a.is_nan() || b.is_nan() {
        return u64::MAX;
    }
    let o = |x: f32| {
        let i = x.to_bits() as i32;
        if i < 0 {
            i32::MIN as i64 - i as i64
        } else {
            i as i64
        }
    };
    (o(a) - o(b)).unsigned_abs()
}

/// La de la `std`, en f64 y redondeada a f32.
fn referencia(f: Mate, x: f32) -> f32 {
    let d = x as f64;
    (match f {
        Mate::Sin => d.sin(),
        Mate::Cos => d.cos(),
        Mate::Tan => d.tan(),
        Mate::Exp2 => d.exp2(),
        Mate::Log2 => d.log2(),
        Mate::Atan => d.atan(),
        Mate::Asin => d.asin(),
        Mate::Acos => d.acos(),
        Mate::Senh => d.sinh(),
        Mate::Cosh => d.cosh(),
        Mate::Tanh => d.tanh(),
        _ => unreachable!(),
    }) as f32
}

fn casa(f: Mate, x: f32) -> f32 {
    en_la_casa(f, x).unwrap()
}

/// **Lo que se promete de cada una**: el peor error en ULP con |x| hasta
/// `hasta`, medido sobre TODOS los f32 el 09-10 (`todos_los_f32`). El seno,
/// el coseno y la tangente, en dos tramos: hasta 105615 (donde la reduccion
/// de CUDA es la suya) y hasta el f32 de antes de 1.5 * 2^22 (desde ahi, por
/// vueltas, como una GPU: D3D no pide nada).
const LIMITES: [(Mate, f32, u64); 14] = [
    (Mate::Sin, 105_615.0, 1),
    (Mate::Sin, 6_291_455.5, 3),
    (Mate::Cos, 105_615.0, 2),
    (Mate::Cos, 6_291_455.5, 3),
    (Mate::Tan, 105_615.0, 3),
    (Mate::Tan, 6_291_455.5, 5),
    (Mate::Exp2, f32::INFINITY, 1),
    (Mate::Log2, f32::INFINITY, 1),
    (Mate::Atan, f32::INFINITY, 2),
    (Mate::Asin, f32::INFINITY, 2),
    (Mate::Acos, f32::INFINITY, 1),
    (Mate::Senh, f32::INFINITY, 2),
    (Mate::Cosh, f32::INFINITY, 1),
    (Mate::Tanh, f32::INFINITY, 1),
];

/// Una rejilla de cada dominio y sus vecinos, y valores al azar de todo f32.
fn muestras(f: Mate) -> Vec<f32> {
    let (desde, hasta) = match f {
        Mate::Sin | Mate::Cos | Mate::Tan => (-1000.0f32, 1000.0f32),
        Mate::Exp2 => (-152.0, 130.0),
        Mate::Log2 => (0.0, 64.0),
        Mate::Atan => (-50.0, 50.0),
        Mate::Asin | Mate::Acos => (-1.0, 1.0),
        _ => (-95.0, 95.0),
    };
    let mut v = Vec::new();
    let n = 200_000;
    for k in 0..=n {
        let x = desde + (hasta - desde) * (k as f32 / n as f32);
        v.extend([x, f32::from_bits(x.to_bits().wrapping_add(1)), f32::from_bits(x.to_bits().wrapping_sub(1))]);
    }
    let mut z = 0x1234_5678u32;
    for _ in 0..200_000 {
        z = z.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        v.push(f32::from_bits(z ^ z >> 15));
    }
    v
}

/// *** EL ERROR de cada una, en ULP, dentro de lo prometido.
#[test]
fn el_error_de_cada_una_dentro_de_lo_prometido() {
    for (f, hasta, max) in LIMITES {
        let mut peor = (0u64, 0.0f32);
        for x in muestras(f) {
            if x.abs() > hasta {
                continue;
            }
            let u = ulps(casa(f, x), referencia(f, x));
            if u > peor.0 {
                peor = (u, x);
            }
        }
        std::eprintln!("{:?} (|x| <= {:e}): el peor, {} ULP en {:e}", f, hasta, peor.0, peor.1);
        assert!(peor.0 <= max, "{:?}({:e}): {} ULP, y se prometen {}", f, peor.1, peor.0, max);
    }
}

/// ** LO EXACTO: los ceros con su signo, las potencias de dos, los
/// infinitos y lo de fuera del dominio.
#[test]
fn lo_que_tiene_que_ser_exacto() {
    let inf = f32::INFINITY;
    for f in [Mate::Sin, Mate::Tan, Mate::Atan, Mate::Asin, Mate::Senh, Mate::Tanh] {
        assert_eq!(casa(f, 0.0).to_bits(), 0, "{:?}(+0)", f);
        assert_eq!(casa(f, -0.0).to_bits(), 0x8000_0000, "{:?}(-0)", f);
    }
    assert_eq!(casa(Mate::Cos, 0.0), 1.0);
    assert_eq!(casa(Mate::Cosh, 0.0), 1.0);
    assert_eq!(casa(Mate::Acos, 1.0), 0.0);
    for n in -149..128 {
        assert_eq!(casa(Mate::Exp2, n as f32), f32::from_bits(if n < -126 { 1 << (n + 149) } else { ((n + 127) as u32) << 23 }), "2^{}", n);
    }
    for k in 0..254u32 {
        let x = f32::from_bits((k + 1) << 23);
        assert_eq!(casa(Mate::Log2, x), k as f32 - 126.0, "log2(2^{})", k as i32 - 126);
    }
    for e in 0..23 {
        assert_eq!(casa(Mate::Log2, f32::from_bits(1 << e)), e as f32 - 149.0, "log2 de un subnormal");
    }
    assert_eq!(casa(Mate::Exp2, inf), inf);
    assert_eq!(casa(Mate::Exp2, -inf), 0.0);
    assert_eq!(casa(Mate::Exp2, 128.0), inf);
    assert_eq!(casa(Mate::Exp2, -150.0), 0.0);
    assert_eq!(casa(Mate::Log2, 0.0), -inf);
    assert_eq!(casa(Mate::Log2, -0.0), -inf);
    assert_eq!(casa(Mate::Log2, inf), inf);
    assert_eq!(casa(Mate::Atan, inf), core::f32::consts::FRAC_PI_2);
    assert_eq!(casa(Mate::Atan, -inf), -core::f32::consts::FRAC_PI_2);
    assert_eq!(casa(Mate::Tanh, inf), 1.0);
    assert_eq!(casa(Mate::Tanh, -inf), -1.0);
    assert_eq!(casa(Mate::Tanh, 20.0), 1.0);
    assert_eq!(casa(Mate::Cosh, inf), inf);
    assert_eq!(casa(Mate::Cosh, -inf), inf);
    assert_eq!(casa(Mate::Senh, inf), inf);
    assert_eq!(casa(Mate::Senh, -inf), -inf);
    assert_eq!(casa(Mate::Cosh, 90.0), inf);
    for f in [Mate::Sin, Mate::Cos, Mate::Tan] {
        assert!(casa(f, inf).is_nan() && casa(f, -inf).is_nan(), "{:?}(inf)", f);
    }
    for f in [Mate::Asin, Mate::Acos] {
        assert!(casa(f, 1.0000001).is_nan() && casa(f, -2.0).is_nan() && casa(f, inf).is_nan(), "{:?} fuera de [-1, 1]", f);
    }
    for x in [-1.0f32, -1e-30, -inf] {
        assert!(casa(Mate::Log2, x).is_nan(), "log2({:e})", x);
    }
    for f in LIMITES.map(|l| l.0) {
        assert!(es_de_series(f));
        assert!(casa(f, f32::NAN).is_nan() && casa(f, -f32::NAN).is_nan(), "{:?}(NaN)", f);
    }
}

/// ** Las impares son impares y las pares, pares: bit a bit.
#[test]
fn las_simetrias() {
    for x in muestras(Mate::Sin).into_iter().step_by(3) {
        if x.is_nan() {
            continue;
        }
        for f in [Mate::Sin, Mate::Tan, Mate::Atan, Mate::Asin, Mate::Senh, Mate::Tanh] {
            let (a, b) = (casa(f, x), casa(f, -x));
            assert!(a.to_bits() == (b.to_bits() ^ 0x8000_0000) || (a.is_nan() && b.is_nan()), "{:?}({:e}) = {:e} y {:?}(-x) = {:e}", f, x, a, f, b);
        }
        for f in [Mate::Cos, Mate::Cosh] {
            let (a, b) = (casa(f, x), casa(f, -x));
            assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()), "{:?}({:e}) = {:e} y {:?}(-x) = {:e}", f, x, a, f, b);
        }
    }
}

/// ** `Mate::aplicar` es la receta.
#[test]
fn aplicar_es_la_receta() {
    for f in LIMITES.map(|l| l.0) {
        for x in muestras(f).into_iter().step_by(97) {
            assert_eq!(f.aplicar(x.to_bits()), casa(f, x).to_bits(), "{:?}({:e})", f, x);
        }
    }
}

/// *** LA FMA DE LA CASA es la `fmaf` de la `std` (la del anfitrion): con
/// sus bordes y cuatro millones al azar, los exponentes cerca para que se
/// cancelen.
#[test]
fn la_fma_de_la_casa_es_la_de_ieee() {
    let especiales = [0.0f32, -0.0, 1.0, -1.0, f32::MIN_POSITIVE, f32::from_bits(1), f32::from_bits(0x8000_0001), f32::MAX, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, 1.5, 3.0, 0.1, 1e-30, 1e30, 16_777_217.0];
    let mut casos: Vec<(f32, f32, f32)> = Vec::new();
    for &a in &especiales {
        for &b in &especiales {
            for &c in &especiales {
                casos.push((a, b, c));
            }
        }
    }
    let mut z = 0xC0FF_EEu32;
    let mut sig = || {
        z = z.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        z ^ z >> 15
    };
    for _ in 0..4_000_000 {
        let (a, b) = (sig(), sig());
        let e = ((a >> 23 & 0xFF) + (b >> 23 & 0xFF)).saturating_sub(127).min(254);
        let c = sig() & 0x807F_FFFF | e << 23;
        casos.push((f32::from_bits(a), f32::from_bits(b), f32::from_bits(c)));
    }
    for (a, b, c) in casos {
        let (x, y) = (fma(a, b, c), a.mul_add(b, c));
        assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "fma({:e}, {:e}, {:e}): la casa {:#010x}, la std {:#010x}", a, b, c, x.to_bits(), y.to_bits());
    }
}

/// TODOS los f32 de cada una, contra la `std`: ignorada (minutos, en
/// `--release`). Lo que dio el 09-10, en `LIMITES`.
#[test]
#[ignore]
fn todos_los_f32() {
    for (f, hasta, max) in LIMITES {
        let hilos: Vec<_> = (0..8u32)
            .map(|t| {
                std::thread::spawn(move || {
                    let (mut peor, mut b) = ((0u64, 0u32), t);
                    loop {
                        let x = f32::from_bits(b);
                        if !(x.abs() > hasta) {
                            let u = ulps(casa(f, x), referencia(f, x));
                            if u > peor.0 {
                                peor = (u, b);
                            }
                        }
                        match b.checked_add(8) {
                            Some(c) => b = c,
                            None => break peor,
                        }
                    }
                })
            })
            .collect();
        let peor = hilos.into_iter().map(|h| h.join().unwrap()).max().unwrap();
        std::eprintln!("{:?} (|x| <= {:e}): el peor de todos, {} ULP en {:e}", f, hasta, peor.0, f32::from_bits(peor.1));
        assert!(peor.0 <= max, "{:?}: {} ULP en {:#010x}", f, peor.0, peor.1);
    }
}
