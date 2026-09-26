//! **EL BACKEND CPU: EL JUEZ.** Dibuja un [`Frame`] con las reglas que el
//! estudio D3D hallo midiendo (`bmo_cubo`, `Reglas::D3D10`) y la conversion a
//! 8 bits que se le pida: con [`Unorm8::Truncate12`] da lo que da la 3060.
//! Es el backend contra el que se mide cualquier otro.
//!
//! [carril]  VERDE     aritmetica pura
//!
//! ```text
//!    1  x / w, y / w; viewport x * (ancho/2) + ancho/2, -y * (alto/2) + alto/2
//!    2  ajuste a 1/256 de pixel, empates al PAR
//!    3  cobertura en el CENTRO del pixel, regla top-left
//!    4  el color del vertice 0 (todos iguales en una cara), a 8 bits
//! ```
//!
//! Sin profundidad ni culling (V0): un triangulo al reves se da la vuelta y
//! se dibuja; con area 0 no cubre nada.

use crate::{check, Backend, Error, Frame, Image, Stats, Unorm8};

/// Subpixeles por pixel (D3D10+).
const SUBPIXEL: i64 = 256;

/// El backend CPU.
#[derive(Clone, Copy, Debug)]
pub struct Cpu {
    pub unorm8: Unorm8,
    /// Lo mas que acepta (la 3060 de V0 toma 24).
    pub max_vertices: usize,
}

impl Cpu {
    /// El juez con la regla 4 de la 3060.
    pub const LA_3060: Cpu = Cpu { unorm8: Unorm8::Truncate12, max_vertices: 24 };
    /// El juez de D3D10, con el redondeo exacto.
    pub const D3D10: Cpu = Cpu { unorm8: Unorm8::Exact, max_vertices: 24 };
}

/// Redondeo con empates al PAR (lo mismo que `bmo_cubo::num::redondear_par`).
fn redondear_par(x: f32) -> i64 {
    let t = x as i64;
    let piso = if (t as f32) > x { t - 1 } else { t };
    let resto = x - piso as f32;
    if resto > 0.5 || (resto == 0.5 && piso % 2 != 0) {
        piso + 1
    } else {
        piso
    }
}

fn arista(ax: i64, ay: i64, bx: i64, by: i64, px: i64, py: i64) -> i64 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

fn top_left(ax: i64, ay: i64, bx: i64, by: i64) -> bool {
    let (dx, dy) = (bx - ax, by - ay);
    (dy == 0 && dx > 0) || dy < 0
}

impl Backend for Cpu {
    fn draw(&mut self, frame: &Frame, out: &mut Image) -> Result<Stats, Error> {
        check(frame, out, self.max_vertices)?;
        let (w, h) = (out.width as i64, out.height as i64);
        let fondo = self.unorm8.pack(frame.clear);
        out.pixels[..(w * h) as usize].fill(fondo);
        let (mw, mh) = (frame.viewport.width as f32 * 0.5, frame.viewport.height as f32 * 0.5);
        let mut n = 0;
        for tri in frame.vertices.chunks_exact(3) {
            let mut x = [0i64; 3];
            let mut y = [0i64; 3];
            for k in 0..3 {
                let p = tri[k].position;
                let inv_w = 1.0 / p[3];
                let (nx, ny) = (p[0] * inv_w, p[1] * inv_w);
                x[k] = redondear_par((nx * mw + mw) * SUBPIXEL as f32);
                y[k] = redondear_par((-ny * mh + mh) * SUBPIXEL as f32);
            }
            let mut area = arista(x[0], y[0], x[1], y[1], x[2], y[2]);
            if area < 0 {
                x.swap(1, 2);
                y.swap(1, 2);
                area = -area;
            }
            if area == 0 {
                continue;
            }
            n += 1;
            let incluye = [top_left(x[1], y[1], x[2], y[2]), top_left(x[2], y[2], x[0], y[0]), top_left(x[0], y[0], x[1], y[1])];
            let color = self.unorm8.pack(tri[0].color);
            let c = SUBPIXEL / 2;
            let (min_x, max_x) = (x[0].min(x[1]).min(x[2]), x[0].max(x[1]).max(x[2]));
            let (min_y, max_y) = (y[0].min(y[1]).min(y[2]), y[0].max(y[1]).max(y[2]));
            let px0 = (min_x - c + SUBPIXEL - 1).div_euclid(SUBPIXEL).max(0);
            let px1 = (max_x - c).div_euclid(SUBPIXEL).min(w - 1);
            let py0 = (min_y - c + SUBPIXEL - 1).div_euclid(SUBPIXEL).max(0);
            let py1 = (max_y - c).div_euclid(SUBPIXEL).min(h - 1);
            for py in py0..=py1 {
                for px in px0..=px1 {
                    let (cx, cy) = (px * SUBPIXEL + c, py * SUBPIXEL + c);
                    let e = [
                        arista(x[1], y[1], x[2], y[2], cx, cy),
                        arista(x[2], y[2], x[0], y[0], cx, cy),
                        arista(x[0], y[0], x[1], y[1], cx, cy),
                    ];
                    if (0..3).all(|k| e[k] > 0 || (e[k] == 0 && incluye[k])) {
                        out.pixels[(py * w + px) as usize] = color;
                    }
                }
            }
        }
        Ok(Stats { triangles: n, device_us: 0, prepare_us: 0, warm: false, in_flight: false, wait_us: 0 })
    }
}

#[cfg(test)]
mod pruebas {
    extern crate std;

    use super::*;
    use crate::{Vertex, Viewport};
    use bmo_cubo::referencia::{como_la_3060, de_la_3060, huella, ALTO, ANCHO, HUELLAS_BMO_X, SIN_EXPLICAR};
    use bmo_cubo::{tanda, FONDO_F};
    use std::vec;
    use std::vec::Vec;

    /// El fotograma `f` del cubo como lo arma la app: la tanda del juez en
    /// vertices de VERRANO.
    fn vertices(f: u32) -> Vec<Vertex> {
        let t = tanda::de_fotograma(f, ANCHO, ALTO).unwrap();
        t.tris().iter().flat_map(|tri| tri.clip.iter().map(move |&p| Vertex { position: p, color: tri.color })).collect()
    }

    fn dibujar(cpu: Cpu, f: u32) -> Vec<u32> {
        let v = vertices(f);
        let frame = Frame { clear: FONDO_F, vertices: &v, viewport: Viewport { width: ANCHO, height: ALTO } };
        let mut px = vec![0u32; (ANCHO * ALTO) as usize];
        let mut img = Image { pixels: &mut px, width: ANCHO, height: ALTO };
        let mut b = cpu;
        b.draw(&frame, &mut img).unwrap();
        px
    }

    /// *** EL BACKEND CPU DE VERRANO DA LAS HUELLAS DE D3D12 EN LA 3060.
    #[test]
    fn da_las_huellas_de_d3d12() {
        for f in [0, 30, 60] {
            assert_eq!(Some(huella(&dibujar(Cpu::D3D10, f))), de_la_3060(f), "fotograma {f}");
            assert_eq!(Some(huella(&dibujar(Cpu::LA_3060, f))), de_la_3060(f), "fotograma {f}, con la regla 4");
        }
    }

    /// Y con la regla 4, el modelo de la 3060 (salvo lo que el juez no
    /// explica, que no es de la cuenta: se dice aparte).
    #[test]
    fn da_el_modelo_de_la_3060() {
        let mut m = vec![0u32; (ANCHO * ALTO) as usize];
        for f in [23, 32, 102] {
            let px = dibujar(Cpu::LA_3060, f);
            como_la_3060(f, &mut m);
            let malos: Vec<usize> = (0..m.len()).filter(|&i| px[i] != m[i]).collect();
            let sin_explicar: Vec<usize> = SIN_EXPLICAR.iter().filter(|s| s.0 == f).map(|s| (s.2 * ANCHO + s.1) as usize).collect();
            assert_eq!(malos, sin_explicar, "fotograma {f}");
        }
        assert_eq!(HUELLAS_BMO_X[0].0, 32);
    }

    /// *** V1c: la limpieza RECORTADA da el MISMO fotograma. Lo que no es
    /// fondo cae dentro de `Frame::cover`, y una imagen que solo limpia la
    /// caja de antes unida a la de ahora (lo que hace `coopera` en la 3060)
    /// es, fotograma a fotograma, la limpiada entera. Uno de cada 7 del giro
    /// (en el anfitrion se miraron los 360: iguales, margen minimo 2 px, un
    /// 14 % de la ventana de media) y el 30 al final, como el banco.
    #[test]
    fn la_limpieza_recortada_da_lo_mismo() {
        let n = (ANCHO * ALTO) as usize;
        let mut inc = vec![0u32; n];
        let mut antes: Option<crate::Rect> = None;
        for f in (0..360).step_by(7).chain([30]) {
            let v = vertices(f);
            let frame = Frame { clear: FONDO_F, vertices: &v, viewport: Viewport { width: ANCHO, height: ALTO } };
            let px = dibujar(Cpu::LA_3060, f);
            let fondo = Unorm8::Truncate12.pack(FONDO_F);
            let c = frame.cover().unwrap();
            for (i, &p) in px.iter().enumerate() {
                let (x, y) = (i as u32 % ANCHO, i as u32 / ANCHO);
                assert!(p == fondo || (c.x0..c.x1).contains(&x) && (c.y0..c.y1).contains(&y), "fotograma {f}: ({x}, {y}) fuera de {c:?}");
            }
            let r = antes.map_or(crate::Rect::full(frame.viewport), |a| a.union(c));
            antes = Some(c);
            for y in r.y0..r.y1 {
                inc[(y * ANCHO + r.x0) as usize..(y * ANCHO + r.x1) as usize].fill(fondo);
            }
            for (d, &p) in inc.iter_mut().zip(&px) {
                if p != fondo {
                    *d = p;
                }
            }
            assert!(inc == px, "fotograma {f}: la recortada no es la entera");
        }
        // Sin vertices, o uno que no se proyecta: la imagen entera.
        let vp = Viewport { width: ANCHO, height: ALTO };
        assert_eq!(Frame { clear: FONDO_F, vertices: &[], viewport: vp }.cover(), None);
        let detras = [Vertex { position: [0.0, 0.0, 0.5, -1.0], color: [1.0; 4] }; 3];
        assert_eq!(Frame { clear: FONDO_F, vertices: &detras, viewport: vp }.cover(), None);
    }

    #[test]
    fn comprueba_lo_que_le_dan() {
        let v = vertices(0);
        let mut px = vec![0u32; 16];
        let mut img = Image { pixels: &mut px, width: 4, height: 4 };
        let mut cpu = Cpu::D3D10;
        let malo = Frame { clear: FONDO_F, vertices: &v[..4], viewport: Viewport { width: 4, height: 4 } };
        assert_eq!(cpu.draw(&malo, &mut img), Err(Error::Vertices));
        let otro = Frame { clear: FONDO_F, vertices: &v[..3], viewport: Viewport { width: 8, height: 4 } };
        assert_eq!(cpu.draw(&otro, &mut img), Err(Error::Image));
    }

    #[test]
    fn la_regla_4() {
        // La cara amarilla del 32: 0x22 exacto, 0x21 truncando a 12 bits.
        let x = 34.49 / 255.0;
        assert_eq!(Unorm8::Exact.convert(x), 34);
        assert_eq!(Unorm8::Exact.convert(1.0), 255);
        assert_eq!(Unorm8::Truncate12.convert(1.0), 255);
        assert_eq!(Unorm8::Truncate12.pack(FONDO_F), 0xFF10_1018);
    }
}
