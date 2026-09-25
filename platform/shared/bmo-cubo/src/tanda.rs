//! **LA TANDA DE LA 3060 (X5)** -- el cubo de un fotograma tal como se le da
//! al rasterizador de la 3060 dibujando SIN Windows. PROPIO de BMO-X (como
//! [`crate::referencia`]): el juez no se toca.
//!
//! [carril]  VERDE     aritmetica pura: las mismas cuentas que el juez
//!
//! # Que se le da, y que hace la 3060
//!
//! ```text
//!    la CPU (con las cuentas del juez)     la 3060 (su hardware)
//!    wvp * pos: la posicion de RECORTE     x / w, el viewport, el ajuste a 1/256
//!    que triangulos miran a la camara      que pixeles cubre cada uno (top-left)
//!    el color de la cara, en float         pasarlo a 8 bits en el ROP (regla 4)
//! ```
//!
//! La posicion va ANTES de dividir por w: la division, el viewport y el
//! ajuste al subpixel los hace el hardware, que es lo que las reglas 1..3 del
//! juez describen. Si la 3060 da la huella de D3D12, esas reglas son las de SU
//! silicio y no las del driver de Windows.
//!
//! # Por que no hace falta un depth buffer (todavia)
//!
//! El cubo es CONVEXO: con las caras traseras fuera, cada pixel lo cubre a lo
//! sumo UNA cara (y la regla top-left reparte las aristas compartidas). El
//! test de profundidad del juez no cambia ningun pixel -- lo prueba
//! `sin_profundidad_es_el_juez` en la vuelta entera. El depth buffer vuelve a
//! hacer falta con dos objetos que se tapan (X5b).
//!
//! Las caras traseras las descarta la CPU con la MISMA cuenta que el juez
//! (area en subpixeles <= 0): el culling por hardware es otro metodo sin
//! probar, y va con el depth buffer.

use crate::mat::{transformar, transformar_dir};
use crate::num::redondear_par;
use crate::{constantes, iluminar, indices, vertices, NUM_INDICES, NUM_VERTICES};

/// Lo mas que se le da a la 3060 de una vez. Un cubo muestra a lo sumo tres
/// caras (6 triangulos); 8 deja margen a una cara de canto.
pub const CABEN: usize = 8;

/// Un triangulo de la tanda.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tri {
    /// Los tres vertices en coordenadas de RECORTE (x, y, z, w), en el orden
    /// de los indices.
    pub clip: [[f32; 4]; 3],
    /// El color de la cara ya iluminado (r, g, b, a), antes de pasar a 8 bits.
    pub color: [f32; 4],
}

/// Los triangulos que miran a la camara, en el orden de los indices.
#[derive(Clone, Copy, Debug)]
pub struct Tanda {
    pub tris: [Tri; CABEN],
    pub n: usize,
}

impl Tanda {
    pub fn tris(&self) -> &[Tri] {
        &self.tris[..self.n]
    }
}

const SUBPIXEL: f32 = 256.0;

fn arista(ax: i64, ay: i64, bx: i64, by: i64, px: i64, py: i64) -> i64 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// **La tanda del fotograma `f`** en un destino de `ancho` x `alto`. `None`
/// si miran a la camara mas de [`CABEN`] triangulos (no pasa en la vuelta:
/// lo prueba `caben_en_la_vuelta_entera`).
pub fn de_fotograma(f: u32, ancho: u32, alto: u32) -> Option<Tanda> {
    let c = constantes(crate::angulo_de_fotograma(f), ancho as f32 / alto as f32);
    let vs = vertices();
    let is = indices();
    let (medio_w, medio_h) = (ancho as f32 * 0.5, alto as f32 * 0.5);
    let mut clip = [[0f32; 4]; NUM_VERTICES];
    let mut sx = [0i64; NUM_VERTICES];
    let mut sy = [0i64; NUM_VERTICES];
    let mut detras = [false; NUM_VERTICES];
    for (i, v) in vs.iter().enumerate() {
        // Las cuentas del juez (`dibujar_por_cpu_con` con `Reglas::D3D10`),
        // una por una: solo para decidir que triangulos miran a la camara.
        let p = transformar(&c.wvp, [v.pos[0], v.pos[1], v.pos[2], 1.0]);
        clip[i] = p;
        detras[i] = p[3] <= 0.0;
        let inv_w = 1.0 / p[3];
        let (nx, ny) = (p[0] * inv_w, p[1] * inv_w);
        sx[i] = redondear_par((nx * medio_w + medio_w) * SUBPIXEL);
        sy[i] = redondear_par((-ny * medio_h + medio_h) * SUBPIXEL);
    }
    let vacio = Tri { clip: [[0.0; 4]; 3], color: [0.0; 4] };
    let mut t = Tanda { tris: [vacio; CABEN], n: 0 };
    for k in 0..NUM_INDICES / 3 {
        let i = [is[k * 3] as usize, is[k * 3 + 1] as usize, is[k * 3 + 2] as usize];
        if i.iter().any(|&j| detras[j]) {
            continue;
        }
        if arista(sx[i[0]], sy[i[0]], sx[i[1]], sy[i[1]], sx[i[2]], sy[i[2]]) <= 0 {
            continue;
        }
        if t.n == CABEN {
            return None;
        }
        let normal = transformar_dir(&c.world, vs[i[0]].normal);
        t.tris[t.n] = Tri { clip: [clip[i[0]], clip[i[1]], clip[i[2]]], color: iluminar(vs[i[0]].color, normal, c.luz) };
        t.n += 1;
    }
    Some(t)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::{dibujar_por_cpu, empaquetar, FONDO};
    use std::vec;
    use std::vec::Vec;

    /// La cobertura del juez (centro del pixel, top-left), SIN profundidad:
    /// cada triangulo de la tanda pinta sus pixeles, en orden. Es la cuenta
    /// de `dibujar_por_cpu` quitando el z-buffer.
    fn sin_profundidad(f: u32, w: u32, h: u32) -> Vec<u32> {
        let mut px = vec![FONDO; (w * h) as usize];
        let t = de_fotograma(f, w, h).expect("caben");
        let (mw, mh) = (w as f32 * 0.5, h as f32 * 0.5);
        for tri in t.tris() {
            let mut x = [0i64; 3];
            let mut y = [0i64; 3];
            for k in 0..3 {
                let p = tri.clip[k];
                let inv_w = 1.0 / p[3];
                x[k] = redondear_par((p[0] * inv_w * mw + mw) * 256.0);
                y[k] = redondear_par((-(p[1] * inv_w) * mh + mh) * 256.0);
            }
            let top_left = |a: usize, b: usize| {
                let (dx, dy) = (x[b] - x[a], y[b] - y[a]);
                (dy == 0 && dx > 0) || dy < 0
            };
            let incluye = [top_left(1, 2), top_left(2, 0), top_left(0, 1)];
            let color = empaquetar(tri.color);
            for py in 0..h as i64 {
                for pxx in 0..w as i64 {
                    let (cx, cy) = (pxx * 256 + 128, py * 256 + 128);
                    let e = [
                        arista(x[1], y[1], x[2], y[2], cx, cy),
                        arista(x[2], y[2], x[0], y[0], cx, cy),
                        arista(x[0], y[0], x[1], y[1], cx, cy),
                    ];
                    if (0..3).all(|k| e[k] > 0 || (e[k] == 0 && incluye[k])) {
                        px[(py * w as i64 + pxx) as usize] = color;
                    }
                }
            }
        }
        px
    }

    fn juez(f: u32, w: u32, h: u32) -> Vec<u32> {
        let mut px = vec![0u32; (w * h) as usize];
        dibujar_por_cpu(crate::angulo_de_fotograma(f), w, h, &mut px);
        px
    }

    /// *** SIN DEPTH BUFFER, LA MISMA IMAGEN: en la medida de las capturas, en
    /// los tres fotogramas que tienen huella de D3D12 (y por tanto la huella).
    #[test]
    fn sin_profundidad_da_las_huellas_de_la_3060() {
        use crate::referencia::{de_la_3060, huella, ALTO, ANCHO, HUELLAS};
        for (f, _) in HUELLAS {
            let px = sin_profundidad(f, ANCHO, ALTO);
            assert_eq!(Some(huella(&px)), de_la_3060(f), "fotograma {f}");
        }
    }

    /// Y en la vuelta entera, pixel a pixel contra el juez (a 320x180: la
    /// convexidad no depende de la medida).
    #[test]
    fn sin_profundidad_es_el_juez() {
        for f in (0..360).step_by(3) {
            assert_eq!(sin_profundidad(f, 320, 180), juez(f, 320, 180), "fotograma {f}");
        }
    }

    #[test]
    fn caben_en_la_vuelta_entera() {
        let mut mas = 0;
        for f in 0..360 {
            let t = de_fotograma(f, 1280, 720).expect("caben");
            assert!(t.n % 2 == 0 && t.n >= 2, "fotograma {f}: {} triangulos", t.n);
            mas = mas.max(t.n);
        }
        assert!(mas <= 6, "un cubo muestra a lo sumo tres caras: {mas}");
    }

    #[test]
    fn los_colores_son_los_del_juez() {
        // El fotograma 0: el centro de la pantalla es la cara verde de frente.
        let t = de_fotograma(0, 1280, 720).unwrap();
        let centro = juez(0, 1280, 720)[360 * 1280 + 640];
        assert!(t.tris().iter().any(|tri| empaquetar(tri.color) == centro));
        assert!(t.tris().iter().all(|tri| tri.color[3] == 1.0));
    }
}
