//! Matrices 4x4 como `[f32; 16]` en **orden de columnas**: `m[c * 4 + f]` es la fila `f`
//! de la columna `c`. Es el mismo orden en que se suben a la GPU, y en HLSL se usa
//! `mul(M, v)` (vector columna). Convencion de mano izquierda, profundidad 0..1.

use crate::num::{coseno, cruz, normalizar, punto, resta, seno, tangente, V3};

pub type Mat4 = [f32; 16];

pub const IDENTIDAD: Mat4 = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// `a * b` (primero se aplica `b`, luego `a`).
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut r = [0.0; 16];
    for c in 0..4 {
        for f in 0..4 {
            r[c * 4 + f] =
                a[f] * b[c * 4] + a[4 + f] * b[c * 4 + 1] + a[8 + f] * b[c * 4 + 2] + a[12 + f] * b[c * 4 + 3];
        }
    }
    r
}

/// `m * v` con `v` como vector columna.
pub fn transformar(m: &Mat4, v: [f32; 4]) -> [f32; 4] {
    let mut r = [0.0; 4];
    for (f, salida) in r.iter_mut().enumerate() {
        *salida = m[f] * v[0] + m[4 + f] * v[1] + m[8 + f] * v[2] + m[12 + f] * v[3];
    }
    r
}

/// Parte 3x3 de `m` aplicada a una direccion (lo que hace `(float3x3)World` en HLSL).
pub fn transformar_dir(m: &Mat4, d: V3) -> V3 {
    let r = transformar(m, [d[0], d[1], d[2], 0.0]);
    [r[0], r[1], r[2]]
}

/// Rotacion alrededor de Y. Con `a = pi/2`, +X pasa a -Z.
pub fn rotacion_y(a: f32) -> Mat4 {
    let (s, c) = (seno(a), coseno(a));
    [
        c, 0.0, -s, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        s, 0.0, c, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]
}

/// Rotacion alrededor de X. Con `a = pi/2`, +Y pasa a +Z.
pub fn rotacion_x(a: f32) -> Mat4 {
    let (s, c) = (seno(a), coseno(a));
    [
        1.0, 0.0, 0.0, 0.0, //
        0.0, c, s, 0.0, //
        0.0, -s, c, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]
}

/// Camara de mano izquierda: `ojo` pasa al origen y `objetivo` queda sobre +Z.
pub fn vista(ojo: V3, objetivo: V3, arriba: V3) -> Mat4 {
    let f = normalizar(resta(objetivo, ojo));
    let s = normalizar(cruz(arriba, f));
    let u = cruz(f, s);
    [
        s[0], u[0], f[0], 0.0, //
        s[1], u[1], f[1], 0.0, //
        s[2], u[2], f[2], 0.0, //
        -punto(s, ojo), -punto(u, ojo), -punto(f, ojo), 1.0,
    ]
}

/// Perspectiva de mano izquierda: z = `cerca` -> profundidad 0, z = `lejos` -> 1, y w = z.
pub fn perspectiva(fov_y: f32, aspecto: f32, cerca: f32, lejos: f32) -> Mat4 {
    let h = 1.0 / tangente(fov_y * 0.5);
    let w = h / aspecto;
    let r = lejos / (lejos - cerca);
    [
        w, 0.0, 0.0, 0.0, //
        0.0, h, 0.0, 0.0, //
        0.0, 0.0, r, 1.0, //
        0.0, 0.0, -r * cerca, 0.0,
    ]
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::num::{raiz, MEDIO_PI};

    fn cerca_de(a: [f32; 4], b: [f32; 4]) -> bool {
        a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn identidad_es_neutra() {
        let m = rotacion_y(0.3);
        assert_eq!(mul(&IDENTIDAD, &m), m);
        assert_eq!(mul(&m, &IDENTIDAD), m);
    }

    #[test]
    fn rotaciones_de_90_grados() {
        assert!(cerca_de(transformar(&rotacion_y(MEDIO_PI), [1.0, 0.0, 0.0, 1.0]), [0.0, 0.0, -1.0, 1.0]));
        assert!(cerca_de(transformar(&rotacion_x(MEDIO_PI), [0.0, 1.0, 0.0, 1.0]), [0.0, 0.0, 1.0, 1.0]));
    }

    #[test]
    fn orden_de_mul() {
        // mul(a, b) aplica primero b: rotar X 90 grados y luego Y 90 grados lleva +Y -> +Z -> +X
        let m = mul(&rotacion_y(MEDIO_PI), &rotacion_x(MEDIO_PI));
        assert!(cerca_de(transformar(&m, [0.0, 1.0, 0.0, 1.0]), [1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn vista_lleva_ojo_al_origen_y_objetivo_a_z() {
        let ojo = [0.0, 1.5, -5.0];
        let v = vista(ojo, [0.0; 3], [0.0, 1.0, 0.0]);
        assert!(cerca_de(transformar(&v, [ojo[0], ojo[1], ojo[2], 1.0]), [0.0, 0.0, 0.0, 1.0]));
        let d = raiz(1.5 * 1.5 + 5.0 * 5.0);
        assert!(cerca_de(transformar(&v, [0.0, 0.0, 0.0, 1.0]), [0.0, 0.0, d, 1.0]));
    }

    #[test]
    fn perspectiva_profundidad_0_a_1() {
        let p = perspectiva(1.0, 16.0 / 9.0, 0.1, 100.0);
        let c = transformar(&p, [0.0, 0.0, 0.1, 1.0]);
        assert!((c[2] / c[3]).abs() < 1e-6 && (c[3] - 0.1).abs() < 1e-7);
        let l = transformar(&p, [0.0, 0.0, 100.0, 1.0]);
        assert!((l[2] / l[3] - 1.0).abs() < 1e-6);
    }
}
