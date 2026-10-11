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
//!
//! ** V2 (11-10): con `Frame::cull` las caras de detras (antihorarias en la
//! pantalla) no se dibujan; con `Frame::depth`, el z-buffer de D3D:
//!
//! ```text
//!    5  z = z / w de cada vertice, interpolada en el centro del pixel con
//!       los mismos pesos de la cobertura (las aristas): la z de despues de
//!       dividir es lineal en la pantalla
//!    6  fuera de [0, 1], no se pinta (el recorte de profundidad)
//!    7  se pinta si z < la del z-buffer (LESS), y se apunta; el z-buffer
//!       empieza en 1.0
//! ```

use crate::{check, Backend, Cull, Error, Frame, Image, Stats, Unorm8};

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
        let Image { pixels, depth, .. } = out;
        pixels[..(w * h) as usize].fill(fondo);
        // V2: el z-buffer, a 1.0; sin el, un fotograma con profundidad no se dibuja.
        let mut zb: Option<&mut [f32]> = None;
        if frame.depth {
            match depth.as_deref_mut() {
                Some(z) if z.len() >= (w * h) as usize => {
                    z[..(w * h) as usize].fill(1.0);
                    zb = Some(z);
                }
                _ => return Err(Error::Image),
            }
        }
        let (mw, mh) = (frame.viewport.width as f32 * 0.5, frame.viewport.height as f32 * 0.5);
        let mut n = 0;
        for tri in frame.vertices.chunks_exact(3) {
            let mut x = [0i64; 3];
            let mut y = [0i64; 3];
            let mut zn = [0f64; 3];
            for k in 0..3 {
                let p = tri[k].position;
                let inv_w = 1.0 / p[3];
                let (nx, ny) = (p[0] * inv_w, p[1] * inv_w);
                x[k] = redondear_par((nx * mw + mw) * SUBPIXEL as f32);
                y[k] = redondear_par((-ny * mh + mh) * SUBPIXEL as f32);
                zn[k] = (p[2] * inv_w) as f64;
            }
            let mut area = arista(x[0], y[0], x[1], y[1], x[2], y[2]);
            // V2: delante es horario en la pantalla (area > 0, con y hacia abajo)
            if frame.cull == Cull::Back && area < 0 {
                continue;
            }
            if area < 0 {
                x.swap(1, 2);
                y.swap(1, 2);
                zn.swap(1, 2);
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
                        let i = (py * w + px) as usize;
                        if let Some(z) = zb.as_deref_mut() {
                            // e[k] es el peso del vertice k (por el area)
                            let zp = ((e[0] as f64 * zn[0] + e[1] as f64 * zn[1] + e[2] as f64 * zn[2]) / area as f64) as f32;
                            if !(0.0..=1.0).contains(&zp) || zp >= z[i] {
                                continue;
                            }
                            z[i] = zp;
                        }
                        pixels[i] = color;
                    }
                }
            }
        }
        Ok(Stats { triangles: n, device_us: 0, prepare_us: 0, warm: false, in_flight: false, wait_us: 0 })
    }
}

/// **El juicio de un fotograma ajeno** (10-10, Q0a3 de `docs/plan/EL_FOCO.md`):
/// cuantos pixeles de lo que dejo otro backend (la 3060, en la ventana de una
/// app) no son los que dibuja el juez.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Juicio {
    /// Los pixeles mirados (`ancho * alto`).
    pub pixeles: usize,
    /// Los que no son los del juez.
    pub distintos: usize,
    /// El primero de esos: `(x, y, visto, juez)`.
    pub primero: Option<(u32, u32, u32, u32)>,
}

impl Cpu {
    /// **Juzgar lo que dibujo OTRO**: el juez dibuja `frame` en `hoja` y se
    /// compara, pixel a pixel, con `visto`. A cualquier medida: la huella de
    /// D3D12 solo existe a 1280x720, y este juez da esas huellas
    /// (`da_las_huellas_de_d3d12`), asi que a otra medida es el que queda.
    ///
    /// [!] Un recuento, no un si o un no: la 3060 tiene pixeles que el juez
    /// aun no explica (`bmo_cubo::referencia::SIN_EXPLICAR`, unos pocos en
    /// algunos fotogramas). Quien juzga dice cuantos.
    ///
    /// V2: con `frame.depth`, el juez lleva su z-buffer en `profundidad`.
    pub fn juzgar(mut self, frame: &Frame, visto: &[u32], hoja: &mut [u32], profundidad: Option<&mut [f32]>) -> Result<Juicio, Error> {
        let (w, h) = (frame.viewport.width, frame.viewport.height);
        let n = (w as usize) * (h as usize);
        if visto.len() < n || hoja.len() < n {
            return Err(Error::Image);
        }
        self.draw(frame, &mut Image { pixels: &mut hoja[..n], width: w, height: h, depth: profundidad })?;
        let mut j = Juicio { pixeles: n, distintos: 0, primero: None };
        for (i, (&v, &c)) in visto[..n].iter().zip(&hoja[..n]).enumerate() {
            if v != c {
                j.distintos += 1;
                if j.primero.is_none() {
                    j.primero = Some(((i % w as usize) as u32, (i / w as usize) as u32, v, c));
                }
            }
        }
        Ok(j)
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
        let frame = Frame { clear: FONDO_F, vertices: &v, viewport: Viewport { width: ANCHO, height: ALTO } , depth: false, cull: Cull::None };
        let mut px = vec![0u32; (ANCHO * ALTO) as usize];
        let mut img = Image { pixels: &mut px, width: ANCHO, height: ALTO , depth: None };
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

    /// ** El juicio, a una medida que NO es la de D3D12: lo que dibujo el
    /// propio juez sale igual; un pixel tocado sale, y en su sitio; y una
    /// imagen corta no se juzga.
    #[test]
    fn el_juicio_a_otra_medida() {
        let (w, h) = (640u32, 360u32);
        let v = vertices(30);
        let frame = Frame { clear: FONDO_F, vertices: &v, viewport: Viewport { width: w, height: h } , depth: false, cull: Cull::None };
        let n = (w * h) as usize;
        let mut visto = vec![0u32; n];
        let mut b = Cpu::LA_3060;
        b.max_vertices = v.len();
        b.draw(&frame, &mut Image { pixels: &mut visto, width: w, height: h , depth: None }).unwrap();
        let fondo = Unorm8::Truncate12.pack(FONDO_F);
        assert!(visto.iter().any(|&p| p != fondo), "el cubo tiene que verse a esta medida");
        let mut hoja = vec![0u32; n];
        let j = b.juzgar(&frame, &visto, &mut hoja, None).unwrap();
        assert_eq!(j, Juicio { pixeles: n, distintos: 0, primero: None });
        let i = 200 * w as usize + 321;
        let bueno = visto[i];
        visto[i] ^= 0x00FF_FFFF;
        let j = b.juzgar(&frame, &visto, &mut hoja, None).unwrap();
        assert_eq!((j.distintos, j.primero), (1, Some((321, 200, bueno ^ 0x00FF_FFFF, bueno))));
        assert_eq!(b.juzgar(&frame, &visto[..n - 1], &mut hoja, None), Err(Error::Image));
    }

    // -- V2: la profundidad y el descarte -----------------------------------

    /// Un cuadrado de `x0..x1` por `y0..y1` (coordenadas de recorte, w = 1) a
    /// la profundidad `z`: dos triangulos, de DELANTE (horarios en la pantalla).
    fn cuadrado(x0: f32, y0: f32, x1: f32, y1: f32, z: f32, color: [f32; 4]) -> Vec<Vertex> {
        let v = |x: f32, y: f32| Vertex { position: [x, y, z, 1.0], color };
        // abajo-izquierda, arriba-izquierda, arriba-derecha: horario en la pantalla
        vec![v(x0, y0), v(x0, y1), v(x1, y1), v(x0, y0), v(x1, y1), v(x1, y0)]
    }

    const ROJO: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
    const VERDE: [f32; 4] = [0.0, 1.0, 0.0, 1.0];

    fn pinta(v: &[Vertex], depth: bool, cull: Cull) -> Vec<u32> {
        let (w, h) = (64u32, 48u32);
        let frame = Frame { clear: FONDO_F, vertices: v, viewport: Viewport { width: w, height: h }, depth, cull };
        let mut px = vec![0u32; (w * h) as usize];
        let mut z = vec![0f32; (w * h) as usize];
        let mut b = Cpu { max_vertices: 4096, ..Cpu::LA_3060 };
        b.draw(&frame, &mut Image { pixels: &mut px, width: w, height: h, depth: Some(&mut z) }).unwrap();
        px
    }

    fn en(px: &[u32], x: usize, y: usize) -> u32 {
        px[y * 64 + x] & 0x00FF_FFFF
    }

    /// ** V2: lo de DELANTE tapa lo de detras, se dibuje en el orden que se
    /// dibuje; sin profundidad, como V0, gana el ultimo.
    #[test]
    fn depth_hides_what_is_behind_in_any_order() {
        let cerca = cuadrado(-0.5, -0.5, 0.25, 0.5, 0.25, ROJO);
        let lejos = cuadrado(-0.25, -0.25, 0.75, 0.75, 0.75, VERDE);
        let a: Vec<Vertex> = cerca.iter().chain(&lejos).copied().collect();
        let b: Vec<Vertex> = lejos.iter().chain(&cerca).copied().collect();
        let (pa, pb) = (pinta(&a, true, Cull::None), pinta(&b, true, Cull::None));
        assert_eq!(pa, pb, "con profundidad, el orden no importa");
        // el cruce (x = 0 en recorte = pixel 32; y = 0 = pixel 24): rojo
        assert_eq!(en(&pa, 30, 22), 0xFF_0000, "delante, el rojo");
        assert_eq!(en(&pa, 44, 14), 0x00_FF00, "solo el verde");
        // sin profundidad, el ultimo gana
        assert_eq!(en(&pinta(&a, false, Cull::None), 30, 22), 0x00_FF00);
        assert_eq!(en(&pinta(&b, false, Cull::None), 30, 22), 0xFF_0000);
    }

    /// ** V2: las caras de detras (antihorarias en la pantalla) fuera con
    /// `Cull::Back`; sin el, al reves se dibuja igual (V0).
    #[test]
    fn back_faces_are_culled() {
        let delante = cuadrado(-0.5, -0.5, 0.5, 0.5, 0.5, ROJO);
        let mut detras = delante.clone();
        for t in detras.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
        assert_eq!(en(&pinta(&delante, false, Cull::Back), 32, 24), 0xFF_0000, "la de delante");
        let vacio = pinta(&detras, false, Cull::Back);
        assert!(vacio.iter().all(|&p| p == vacio[0]), "la de detras no se dibuja");
        assert_eq!(en(&pinta(&detras, false, Cull::None), 32, 24), 0xFF_0000, "sin descarte, si");
    }

    /// ** V2: el recorte de profundidad -- fuera de [0, 1] no se pinta -- y el
    /// MENOR que: a 1.0 (lo que vale el z-buffer limpio) tampoco.
    #[test]
    fn depth_clips_outside_zero_one() {
        for z in [1.5, -0.25, 1.0] {
            let p = pinta(&cuadrado(-0.5, -0.5, 0.5, 0.5, z, ROJO), true, Cull::None);
            assert!(p.iter().all(|&x| x == p[0]) && en(&p, 32, 24) != 0xFF_0000, "z = {z}");
        }
        assert_eq!(en(&pinta(&cuadrado(-0.5, -0.5, 0.5, 0.5, 0.0, ROJO), true, Cull::None), 32, 24), 0xFF_0000, "z = 0 si");
    }

    /// Profundidad sin z-buffer, en la CPU: no se dibuja (Error::Image).
    #[test]
    fn depth_without_a_buffer_is_refused() {
        let v = cuadrado(-0.5, -0.5, 0.5, 0.5, 0.5, ROJO);
        let frame = Frame { clear: FONDO_F, vertices: &v, viewport: Viewport { width: 8, height: 8 }, depth: true, cull: Cull::None };
        let mut px = vec![0u32; 64];
        let mut b = Cpu::LA_3060;
        assert_eq!(b.draw(&frame, &mut Image { pixels: &mut px, width: 8, height: 8, depth: None }), Err(Error::Image));
    }

    /// Un cubo de lado `l` en (cx, cy, cz) (la camara mira +z), girado `a`
    /// radianes en y, en perspectiva (cerca 1, lejos 10): sus 12 triangulos,
    /// cada cara de un color.
    fn cubo(cx: f32, cy: f32, cz: f32, l: f32, a: f32, tono: f32) -> Vec<Vertex> {
        let (s, c) = (a.sin(), a.cos());
        let esquina = |i: usize| {
            let (x, y, z) = (if i & 1 == 0 { -l } else { l } / 2.0, if i & 2 == 0 { -l } else { l } / 2.0, if i & 4 == 0 { -l } else { l } / 2.0);
            let (xr, zr) = (x * c + z * s, -x * s + z * c);
            let (xc, yc, zc) = (xr + cx, y + cy, zr + cz);
            [xc, yc, (zc - 1.0) * 10.0 / 9.0, zc]
        };
        let caras = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]];
        let mut v = Vec::new();
        for (k, q) in caras.iter().enumerate() {
            let color = [tono, k as f32 / 6.0, 1.0 - k as f32 / 6.0, 1.0];
            for i in [0, 1, 2, 0, 2, 3] {
                v.push(Vertex { position: esquina(q[i]), color });
            }
        }
        v
    }

    /// ** M1 de PLAN_VERRANO: DOS CUBOS QUE SE TAPAN, en perspectiva y con
    /// todas sus caras. Con profundidad, da igual cual se dibuje primero (y
    /// cual cara): se ve lo de delante. Sin ella, se ve el orden.
    #[test]
    fn two_cubes_hide_each_other() {
        let a = cubo(-0.4, 0.0, 4.0, 1.4, 0.5, 0.2);
        let b = cubo(0.3, 0.2, 6.0, 1.6, -0.3, 0.9);
        let ab: Vec<Vertex> = a.iter().chain(&b).copied().collect();
        let mut ba: Vec<Vertex> = b.iter().chain(&a).copied().collect();
        let con = pinta(&ab, true, Cull::None);
        assert_eq!(con, pinta(&ba, true, Cull::None), "con profundidad, el orden de los cubos no importa");
        ba.reverse();
        assert_eq!(con, pinta(&ba, true, Cull::None), "ni el de los vertices al reves");
        assert_ne!(pinta(&ab, false, Cull::None), pinta(&b.iter().chain(&a).copied().collect::<Vec<_>>(), false, Cull::None), "sin ella, si");
        // los dos se ven: cada uno por el rojo de sus caras
        let rojo = |tono: f32| Unorm8::Truncate12.convert(tono);
        let de = |tono: f32| con.iter().filter(|&&p| p >> 16 & 0xFF == rojo(tono)).count();
        assert!(de(0.2) > 100 && de(0.9) > 20, "a (delante) {} y b (detras, casi tapado) {} pixeles", de(0.2), de(0.9));
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
            let frame = Frame { clear: FONDO_F, vertices: &v, viewport: Viewport { width: ANCHO, height: ALTO } , depth: false, cull: Cull::None };
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
        assert_eq!(Frame { clear: FONDO_F, vertices: &[], viewport: vp , depth: false, cull: Cull::None }.cover(), None);
        let detras = [Vertex { position: [0.0, 0.0, 0.5, -1.0], color: [1.0; 4] }; 3];
        assert_eq!(Frame { clear: FONDO_F, vertices: &detras, viewport: vp , depth: false, cull: Cull::None }.cover(), None);
    }

    #[test]
    fn comprueba_lo_que_le_dan() {
        let v = vertices(0);
        let mut px = vec![0u32; 16];
        let mut img = Image { pixels: &mut px, width: 4, height: 4 , depth: None };
        let mut cpu = Cpu::D3D10;
        let malo = Frame { clear: FONDO_F, vertices: &v[..4], viewport: Viewport { width: 4, height: 4 } , depth: false, cull: Cull::None };
        assert_eq!(cpu.draw(&malo, &mut img), Err(Error::Vertices));
        let otro = Frame { clear: FONDO_F, vertices: &v[..3], viewport: Viewport { width: 8, height: 4 } , depth: false, cull: Cull::None };
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
