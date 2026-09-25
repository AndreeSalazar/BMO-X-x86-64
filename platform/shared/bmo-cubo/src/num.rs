//! Aritmetica propia sobre `f32`: solo `+ - * /`, comparaciones y `to_bits`/`from_bits`.
//! Nada de libm ni de intrinsecos de la plataforma, asi el resultado es el mismo en
//! cualquier CPU con floats IEEE-754 (x86-64, ARM, o el kernel de BMO-X).

pub const PI: f32 = 3.141_592_7;
pub const DOS_PI: f32 = 6.283_185_5;
pub const MEDIO_PI: f32 = 1.570_796_4;

// 2pi partido en dos para reducir el angulo sin perder precision (Cody-Waite).
const DOS_PI_ALTO: f32 = 6.283_185_5;
const DOS_PI_BAJO: f32 = -1.748_455_5e-7;

pub fn abs(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0x7FFF_FFFF)
}

/// Redondeo al entero mas cercano (mitades hacia fuera de cero).
pub fn redondear(x: f32) -> i64 {
    if x >= 0.0 {
        (x + 0.5) as i64
    } else {
        -((-x + 0.5) as i64)
    }
}

/// Redondeo al entero mas cercano con empates al PAR (2.5 -> 2, 3.5 -> 4).
/// Es lo que hace la GPU al pasar las posiciones a punto fijo; con el redondeo
/// "mitades hacia fuera" un vertice en x*256 = 218350.5 caia en otro subpixel.
pub fn redondear_par(x: f32) -> i64 {
    let t = x as i64; // trunca hacia cero
    let piso = if (t as f32) > x { t - 1 } else { t };
    let resto = x - piso as f32; // exacto: x y piso estan a menos de 1
    if resto > 0.5 || (resto == 0.5 && piso % 2 != 0) {
        piso + 1
    } else {
        piso
    }
}

pub fn saturar(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// Raiz cuadrada: estimacion por bits + 4 pasos de Newton.
pub fn raiz(x: f32) -> f32 {
    if x <= 0.0 {
        return if x == 0.0 { 0.0 } else { f32::NAN };
    }
    let mut y = f32::from_bits((x.to_bits() >> 1) + 0x1FBD_1DF5);
    for _ in 0..4 {
        y = 0.5 * (y + x / y);
    }
    y
}

/// Lleva `x` a [-pi, pi].
fn reducir(x: f32) -> f32 {
    let k = redondear(x / DOS_PI) as f32;
    (x - k * DOS_PI_ALTO) - k * DOS_PI_BAJO
}

/// Seno con polinomio de Taylor de grado 11 en [-pi/2, pi/2].
pub fn seno(x: f32) -> f32 {
    let mut r = reducir(x);
    if r > MEDIO_PI {
        r = PI - r;
    } else if r < -MEDIO_PI {
        r = -PI - r;
    }
    let r2 = r * r;
    r * (1.0 + r2 * (-1.0 / 6.0 + r2 * (1.0 / 120.0 + r2 * (-1.0 / 5040.0 + r2 * (1.0 / 362_880.0 + r2 * (-1.0 / 39_916_800.0))))))
}

/// Coseno con polinomio de Taylor de grado 12 en [-pi/2, pi/2].
pub fn coseno(x: f32) -> f32 {
    let r = abs(reducir(x));
    let (r, signo) = if r > MEDIO_PI { (PI - r, -1.0) } else { (r, 1.0) };
    let r2 = r * r;
    signo
        * (1.0
            + r2 * (-0.5
                + r2 * (1.0 / 24.0 + r2 * (-1.0 / 720.0 + r2 * (1.0 / 40_320.0 + r2 * (-1.0 / 3_628_800.0 + r2 * (1.0 / 479_001_600.0)))))))
}

pub fn tangente(x: f32) -> f32 {
    seno(x) / coseno(x)
}

pub type V3 = [f32; 3];

pub fn resta(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn punto(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cruz(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn normalizar(a: V3) -> V3 {
    let inv = 1.0 / raiz(punto(a, a));
    [a[0] * inv, a[1] * inv, a[2] * inv]
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn seno_y_coseno_contra_std() {
        let mut peor = 0.0f32;
        let mut x = -20.0f32;
        while x < 20.0 {
            peor = peor.max((seno(x) - x.sin()).abs()).max((coseno(x) - x.cos()).abs());
            x += 0.001;
        }
        assert!(peor < 2e-6, "error maximo {peor}");
    }

    #[test]
    fn raiz_contra_std() {
        for &x in &[1e-6f32, 0.01, 0.5, 1.0, 1.05, 2.0, 27.25, 1e6] {
            let esperado = x.sqrt();
            assert!((raiz(x) - esperado).abs() <= esperado * 2e-7, "raiz({x})");
        }
        assert_eq!(raiz(0.0), 0.0);
    }

    #[test]
    fn redondeo() {
        assert_eq!(redondear(2.5), 3);
        assert_eq!(redondear(-2.5), -3);
        assert_eq!(redondear(2.49), 2);
    }

    #[test]
    fn redondeo_al_par() {
        assert_eq!(redondear_par(2.5), 2);
        assert_eq!(redondear_par(3.5), 4);
        assert_eq!(redondear_par(-2.5), -2);
        assert_eq!(redondear_par(-3.5), -4);
        assert_eq!(redondear_par(2.51), 3);
        assert_eq!(redondear_par(-0.2), 0);
        // el caso real del fotograma 60: vertice en x = 852.931641 px
        assert_eq!(redondear_par(218_350.5), 218_350);
    }
}
