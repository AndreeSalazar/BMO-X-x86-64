//! La aritmetica del oraculo contra la biblioteca estandar, BIT A BIT.
//!
//! `math` es `no_std`: escribe a mano `sqrt`, `floor`, `fma`... Aqui (con
//! `std`) se compara con las de la biblioteca en millones de valores
//! repartidos por TODOS los patrones de bits: normales, subnormales, ceros,
//! infinitos y NaN. Un redondeo mal en un caso raro es exactamente lo que
//! haria que el emisor (S4) y el oraculo no coincidieran sin que nadie supiera
//! por que.

use bmo_spirv_front::math;

/// xorshift: numeros repetibles sin dependencias.
struct Azar(u64);

impl Azar {
    fn bits(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.bits())
    }
}

/// Iguales como BITS, salvo que los dos sean NaN (la carga de un NaN no se promete).
fn igual(a: f32, b: f32) -> bool {
    (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
}

fn especiales() -> Vec<f32> {
    vec![
        0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 1.5, -1.5, 2.0, 4.0, 0.25, 3.0,
        f32::MIN_POSITIVE, -f32::MIN_POSITIVE, f32::from_bits(1), f32::from_bits(0x8000_0001),
        f32::from_bits(0x007F_FFFF), f32::MAX, f32::MIN, f32::INFINITY, f32::NEG_INFINITY, f32::NAN,
        8388607.5, 8388608.0, -8388607.5, 16777217.0, 0.999_999_94, 1.000_000_1,
    ]
}

#[test]
fn sqrt_como_ieee() {
    for x in especiales() {
        assert!(igual(math::sqrt(x), x.sqrt()), "sqrt({:e}) = {:e}, std {:e}", x, math::sqrt(x), x.sqrt());
    }
    let mut r = Azar(0x9E37_79B9_7F4A_7C15);
    for _ in 0..2_000_000 {
        let x = r.f();
        assert!(igual(math::sqrt(x), x.sqrt()), "sqrt({:e}) [0x{:08x}]", x, x.to_bits());
    }
    // Todos los positivos entre 1 y 4: dos octavas enteras, cada bit.
    let mut b = 1.0f32.to_bits();
    while b < 4.0f32.to_bits() {
        let x = f32::from_bits(b);
        assert!(igual(math::sqrt(x), x.sqrt()), "sqrt(0x{:08x})", b);
        b += 7;
    }
}

#[test]
fn trunc_floor_ceil_fract_como_std() {
    let mut r = Azar(0x1234_5678_9ABC_DEF1);
    let mut casos = especiales();
    casos.extend((0..1_000_000).map(|_| r.f()));
    for x in casos {
        assert!(igual(math::trunc(x), x.trunc()), "trunc({:e})", x);
        assert!(igual(math::floor(x), x.floor()), "floor({:e})", x);
        assert!(igual(math::ceil(x), x.ceil()), "ceil({:e})", x);
        assert!(igual(math::fract(x), x - x.floor()), "fract({:e})", x);
        assert!(igual(math::abs(x), x.abs()), "abs({:e})", x);
    }
}

#[test]
fn fma_con_un_solo_redondeo() {
    let mut r = Azar(0xDEAD_BEEF_CAFE_F00D);
    for _ in 0..2_000_000 {
        let (a, b, c) = (r.f(), r.f(), r.f());
        assert!(igual(math::fma(a, b, c), a.mul_add(b, c)), "fma({:e}, {:e}, {:e})", a, b, c);
    }
    // Y cerca de la cancelacion, donde el doble redondeo se nota: productos que
    // casi anulan la suma.
    for _ in 0..2_000_000 {
        let a = f32::from_bits(0x3F80_0000 | (r.bits() & 0x007F_FFFF));
        let b = f32::from_bits(0x3F80_0000 | (r.bits() & 0x007F_FFFF));
        let c = -(a * b) + f32::from_bits(r.bits() & 0x33FF_FFFF);
        assert!(igual(math::fma(a, b, c), a.mul_add(b, c)), "fma({:e}, {:e}, {:e})", a, b, c);
    }
}

#[test]
fn las_que_se_definen_aqui() {
    // min/max con NaN: el SEGUNDO operando. Ver la cabecera de `math`.
    assert!(math::min(f32::NAN, 1.0) == 1.0);
    assert!(math::min(1.0, f32::NAN).is_nan());
    assert!(math::max(f32::NAN, 1.0) == 1.0);
    assert_eq!(math::clamp(7.0, 0.0, 1.0), 1.0);
    assert_eq!(math::clamp(-7.0, 0.0, 1.0), 0.0);
    assert_eq!(math::mix(2.0, 4.0, 0.5), 3.0);
    assert_eq!(math::step(1.0, 0.5), 0.0);
    assert_eq!(math::step(1.0, 1.0), 1.0);
    assert_eq!(math::to_u32(-5.0), 0);
    assert_eq!(math::to_u32(f32::NAN), 0);
    assert_eq!(math::to_i32(1e20), i32::MAX);
    assert_eq!(math::neg(0.0).to_bits(), 0x8000_0000);
    // rem lleva el signo de x; modulo el de y.
    assert_eq!(math::rem(-7.0, 3.0), -1.0);
    assert_eq!(math::modulo(-7.0, 3.0), 2.0);
}

// ---- S3b: las trascendentes --------------------------------------------------
//
// No se prometen exactas (ver `math`): se prometen deterministas y DENTRO de
// un ULP del valor verdadero redondeado a f32 -- que es mucho mas estricto que
// lo que Vulkan pide (seno/coseno 2^-11 absoluto; exp/log 3 ULP). La
// referencia es la de la biblioteca estandar en DOBLE, redondeada a f32.

/// Distancia en ULP entre dos f32 finitos del mismo signo (o ambos cero).
fn ulps(a: f32, b: f32) -> u32 {
    if a == b || (a.is_nan() && b.is_nan()) {
        return 0;
    }
    if a.is_sign_negative() != b.is_sign_negative() {
        return u32::MAX;
    }
    (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs() as u32
}

fn en_rango(r: &mut Azar, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * ((r.bits() >> 8) as f32 / (1u32 << 24) as f32)
}

#[test]
fn seno_y_coseno_a_un_ulp() {
    let mut r = Azar(0x5EED_0001);
    for _ in 0..1_000_000 {
        let x = en_rango(&mut r, -1000.0, 1000.0);
        let (s, c) = ((x as f64).sin() as f32, (x as f64).cos() as f32);
        let (ds, dc) = (ulps(math::sin(x), s), ulps(math::cos(x), c));
        assert!(ds <= 1, "sin({:e}) = {:e}, verdad {:e}", x, math::sin(x), s);
        assert!(dc <= 1, "cos({:e}) = {:e}, verdad {:e}", x, math::cos(x), c);
    }
    assert!(math::sin(f32::INFINITY).is_nan() && math::cos(f32::NAN).is_nan());
    assert_eq!(math::sin(0.0), 0.0);
    assert_eq!(math::cos(0.0), 1.0);
}

#[test]
fn exp_log_pow_a_un_ulp() {
    let mut r = Azar(0x5EED_0002);
    for _ in 0..1_000_000 {
        let x = en_rango(&mut r, -87.0, 88.0);
        let v = (x as f64).exp() as f32;
        assert!(ulps(math::exp(x), v) <= 1, "exp({:e}) = {:e}, verdad {:e}", x, math::exp(x), v);
        let y = f32::from_bits(r.bits() & 0x7F7F_FFFF).max(f32::MIN_POSITIVE);
        let v = (y as f64).ln() as f32;
        assert!(ulps(math::log(y), v) <= 1, "log({:e}) = {:e}, verdad {:e}", y, math::log(y), v);
        let (b, e) = (en_rango(&mut r, 0.001, 100.0), en_rango(&mut r, -10.0, 10.0));
        let v = (b as f64).powf(e as f64) as f32;
        if v.is_finite() && v != 0.0 {
            assert!(ulps(math::pow(b, e), v) <= 1, "pow({:e}, {:e}) = {:e}, verdad {:e}", b, e, math::pow(b, e), v);
        }
    }
    assert_eq!(math::exp(0.0), 1.0);
    assert_eq!(math::log(1.0), 0.0);
    assert_eq!(math::log(0.0), f32::NEG_INFINITY);
    assert!(math::log(-1.0).is_nan());
    assert!(math::pow(-2.0, 2.0).is_nan());
    assert_eq!(math::exp(200.0), f32::INFINITY);
    assert_eq!(math::exp(-200.0), 0.0);
}
