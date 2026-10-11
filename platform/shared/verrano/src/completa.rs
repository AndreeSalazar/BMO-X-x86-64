//! **COMPLETAR, LA CUENTA** (VE5, 11-10): una pasada del filtro "a trous"
//! guiado por la superficie de cada pixel -- la idea de SVGF --, para UN
//! pixel. Es la cuenta que hacen ESTUDIO VIVO en la CPU (`bmo-verrano-trama`,
//! `Vivo::alisa`) y el JUEZ del programa de la 3060 (`bmo-gpu-ga10x`,
//! `trabajos/completa.rs`): una sola, escrita una vez.
//!
//! [carril]  VERDE     cuentas puras; sin monton, sin `unsafe`
//! [cuesta]  NADA      una cuenta mal da una imagen mal, nada mas
//!
//! Lo que cuesta en la CPU: 25 vecinos por pixel y pasada.
//!
//! ```text
//!    el nucleo     5 x 5 de la B3-spline (1 4 6 4 1 / 16), los vecinos a
//!                  2^paso
//!    la guia       un vecino de OTRO material (el color difiere 0,05) no
//!                  cuenta; de otra normal, coseno^32; de otra distancia,
//!                  1 / (1 + (24 dz / 2^paso)^2)
//!    la luz        el freno de SVGF: m = tolerancia (lp + lq) / 2 + 0,001,
//!                  y el peso m^2 / (m^2 + (lp - lq)^2)
//! ```

/// Un vector de tres.
pub type V3 = [f32; 3];

/// **La guia de un pixel**: lo que ve primero su centro -- la normal, la
/// distancia y el color de su material --.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Guia {
    pub n: V3,
    pub z: f32,
    pub albedo: V3,
}

/// Cuanto ruido se perdona al completar, con UNA muestra por pixel (la luz
/// relativa: 2 es el doble de la media de los dos); con n, entre raiz(n).
pub const FRENO: f32 = 2.0;

/// El nucleo de 5 de la B3-spline (el de "a trous": 1 4 6 4 1, / 16).
pub const NUCLEO: [f32; 5] = [1.0 / 16.0, 1.0 / 4.0, 3.0 / 8.0, 1.0 / 4.0, 1.0 / 16.0];

/// La luz de un color (los pesos de Rec. 709).
pub fn luz(v: &V3) -> f32 {
    0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2]
}

/// **El peso entre dos pixeles**, como (arriba, abajo) -- la division se
/// deja para el final, UNA por vecino --: alto si son la MISMA superficie
/// (casi la misma normal, casi la misma distancia, el mismo material), cero
/// si no. `lejos`: 24 entre lo lejos que estan (a trous: 1, 2, 4...).
#[inline(always)]
pub fn peso(p: &Guia, q: &Guia, lejos: f32) -> (f32, f32) {
    // lo mas barato primero: otro material, otra superficie
    if (p.albedo[0] - q.albedo[0]).abs() >= 0.05 || (p.albedo[1] - q.albedo[1]).abs() >= 0.05 || (p.albedo[2] - q.albedo[2]).abs() >= 0.05 {
        return (0.0, 1.0);
    }
    let mut wn = (p.n[0] * q.n[0] + p.n[1] * q.n[1] + p.n[2] * q.n[2]).max(0.0);
    for _ in 0..5 {
        wn *= wn; // coseno^32: un borde de 15 grados ya casi no pasa
    }
    let dz = (p.z - q.z).abs() * lejos;
    (wn, 1.0 + dz * dz)
}

/// **Una pasada, un pixel**: el `(x, y)` de una imagen de `ancho x alto`,
/// con los vecinos a `2^paso`. La guia y la luz de cada pixel se piden por
/// su indice (`y * ancho + x`): asi la leen igual un `Vec` de la CPU y la
/// RAM que escribio la 3060.
#[inline(always)]
pub fn alisa_pixel(x: i32, y: i32, ancho: i32, alto: i32, paso: u32, tolerancia: f32, guia: impl Fn(usize) -> Guia, c: impl Fn(usize) -> V3) -> V3 {
    let s = 1i32 << paso;
    let lejos = 24.0 / s as f32;
    let i = (y * ancho + x) as usize;
    let (gp, cp) = (guia(i), c(i));
    let lp = luz(&cp);
    // el del centro, con su peso entero
    let mut pesos = NUCLEO[2] * NUCLEO[2];
    let mut suma = [cp[0] * pesos, cp[1] * pesos, cp[2] * pesos];
    for (j, kj) in NUCLEO.iter().enumerate() {
        let qy = y + (j as i32 - 2) * s;
        if qy < 0 || qy >= alto {
            continue;
        }
        for (k, kk) in NUCLEO.iter().enumerate() {
            let qx = x + (k as i32 - 2) * s;
            if qx < 0 || qx >= ancho || (j == 2 && k == 2) {
                continue;
            }
            let q = (qy * ancho + qx) as usize;
            let (arriba, abajo) = peso(&gp, &guia(q), lejos);
            if arriba == 0.0 {
                continue; // otra superficie: ni se mira su luz
            }
            // 1 / (1 + dl^2), con dl = |lp - lq| / m, es
            // m^2 / (m^2 + (lp - lq)^2): sin dividir dos veces
            let cq = c(q);
            let lq = luz(&cq);
            let m = tolerancia * (lp + lq) * 0.5 + 1e-3;
            let (m2, d) = (m * m, lp - lq);
            let wq = kj * kk * arriba * m2 / (abajo * (m2 + d * d));
            suma = [suma[0] + cq[0] * wq, suma[1] + cq[1] * wq, suma[2] + cq[2] * wq];
            pesos += wq;
        }
    }
    let inv = 1.0 / pesos;
    [suma[0] * inv, suma[1] * inv, suma[2] * inv]
}

// ---------------------------------------------------------------------------
// EL BLOQUE DE COMPLETAR: como lo lee y escribe la 3060
// ---------------------------------------------------------------------------

/// Bytes por pixel del bloque de COMPLETAR: la luz A (16), la guia (32) y
/// la luz B (16), en tres zonas seguidas:
///
/// ```text
///    0          la luz A    r, g, b, nada              (4 f32)
///    16 n       la guia     normal, distancia, color   (8 f32)
///                           del material, nada
///    48 n       la luz B    r, g, b, nada              (4 f32)
/// ```
///
/// Una pasada lee una luz y escribe la otra. Es el bloque que el DIRECTOR
/// (o una app) presta a la 3060 (`bmo_gpu_ga10x::completa`).
pub const BYTES_POR_PIXEL: u64 = 64;

/// Donde empieza cada zona, en f32 desde el bloque (`n` pixeles).
pub const fn luz_a(_n: usize) -> usize {
    0
}
pub const fn guia(n: usize) -> usize {
    4 * n
}
pub const fn luz_b(n: usize) -> usize {
    12 * n
}

/// La guia del pixel `i` en el bloque.
pub fn guia_del_bloque(bloque: &[f32], n: usize, i: usize) -> Guia {
    let b = &bloque[guia(n) + 8 * i..guia(n) + 8 * i + 7];
    Guia { n: [b[0], b[1], b[2]], z: b[3], albedo: [b[4], b[5], b[6]] }
}

/// Pone la guia del pixel `i` en el bloque.
pub fn pon_guia(bloque: &mut [f32], n: usize, i: usize, g: &Guia) {
    let k = guia(n) + 8 * i;
    bloque[k..k + 8].copy_from_slice(&[g.n[0], g.n[1], g.n[2], g.z, g.albedo[0], g.albedo[1], g.albedo[2], 0.0]);
}

/// La luz del pixel `i` (la A, o la B con `de_b`).
pub fn luz_del_bloque(bloque: &[f32], n: usize, de_b: bool, i: usize) -> V3 {
    let k = if de_b { luz_b(n) } else { luz_a(n) } + 4 * i;
    [bloque[k], bloque[k + 1], bloque[k + 2]]
}

/// Pone la luz del pixel `i` (la A, o la B con `de_b`).
pub fn pon_luz(bloque: &mut [f32], n: usize, de_b: bool, i: usize, c: V3) {
    let k = if de_b { luz_b(n) } else { luz_a(n) } + 4 * i;
    bloque[k..k + 4].copy_from_slice(&[c[0], c[1], c[2], 0.0]);
}

/// **EL JUEZ DE LA 3060**: lo que tiene que salir en `(x, y)` de una
/// pasada que lee la luz A (o la B con `de_b`) del bloque -- la misma cuenta
/// que [`alisa_pixel`], leyendo como lee el programa --.
#[allow(clippy::too_many_arguments)]
pub fn del_bloque(bloque: &[f32], ancho: u32, alto: u32, paso: u32, de_b: bool, tolerancia: f32, x: u32, y: u32) -> V3 {
    let n = (ancho * alto) as usize;
    alisa_pixel(x as i32, y as i32, ancho as i32, alto as i32, paso, tolerancia, |i| guia_del_bloque(bloque, n, i), |i| luz_del_bloque(bloque, n, de_b, i))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    /// Dos superficies -- una pared roja a la izquierda, el suelo gris a la
    /// derecha, con la distancia en rampa -- y una luz con GRANO, en la A.
    fn bloque(ancho: u32, alto: u32) -> Vec<f32> {
        let n = (ancho * alto) as usize;
        let mut b = std::vec![0.0f32; 16 * n];
        let mut s = 0x2545_F491u32;
        for y in 0..alto {
            for x in 0..ancho {
                let i = (y * ancho + x) as usize;
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                let l = 0.5 + 1.5 * ((s >> 8) as f32 / (1u32 << 24) as f32);
                let pared = x < ancho / 2;
                let base = if pared { [0.65, 0.05, 0.05] } else { [0.73, 0.73, 0.73] };
                pon_luz(&mut b, n, false, i, [base[0] * l, base[1] * l, base[2] * l]);
                pon_luz(&mut b, n, true, i, [1.0; 3]);
                pon_guia(&mut b, n, i, &Guia { n: if pared { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }, z: 2.0 + y as f32 * 0.01, albedo: base });
            }
        }
        b
    }

    /// *** El juez de la 3060 ES la cuenta de ESTUDIO VIVO (bit a bit), una
    /// pasada quita el grano de la pared, la pared y el suelo no se mezclan,
    /// y la pasada de B lee la B.
    #[test]
    fn the_block_judge_is_the_live_studio_sum() {
        let (w, h) = (48u32, 32u32);
        let b = bloque(w, h);
        let n = (w * h) as usize;
        let (mut antes, mut despues) = (0.0f32, 0.0f32);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let j = del_bloque(&b, w, h, 1, false, 2.0, x, y);
                let guias: Vec<Guia> = (0..n).map(|k| guia_del_bloque(&b, n, k)).collect();
                let luces: Vec<V3> = (0..n).map(|k| luz_del_bloque(&b, n, false, k)).collect();
                if x % 7 == 0 {
                    assert_eq!(j, alisa_pixel(x as i32, y as i32, 48, 32, 1, 2.0, |k| guias[k], |k| luces[k]));
                }
                // el grano: lo que se aparta de la media de la pared (la luz
                // va de 0,5 a 2: su media, 1,25)
                if x < 20 && y > 2 && y < 29 {
                    antes += (luces[i][0] - 0.8125).abs();
                    despues += (j[0] - 0.8125).abs();
                }
            }
        }
        assert!(despues < 0.6 * antes, "el grano: antes {antes}, despues {despues}");
        let (pared, suelo) = (del_bloque(&b, w, h, 2, false, 2.0, 23, 16), del_bloque(&b, w, h, 2, false, 2.0, 24, 16));
        assert!(pared[0] > 5.0 * pared[1], "la pared sigue roja: {pared:?}");
        assert!((suelo[0] - suelo[1]).abs() < 1e-6, "el suelo sigue gris: {suelo:?}");
        let uno = del_bloque(&b, w, h, 0, true, 0.5, 10, 10);
        assert!(uno.iter().all(|v| (v - 1.0).abs() < 1e-6), "la B, todo a 1: {uno:?}");
    }
}
