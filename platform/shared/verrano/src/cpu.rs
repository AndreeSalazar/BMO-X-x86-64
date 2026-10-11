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
//!
//! ** V2b (11-10, MC1 de `docs/plan/PLAN_MUNDO.md`): EL RECORTE DE CERCA. Un
//! triangulo con un vertice por DETRAS del plano de cerca (z < 0 en recorte:
//! con w <= 0 la division lo daria la vuelta) se recorta contra z = 0, como
//! D3D (`DepthClipEnable`): queda un triangulo o un cuadrilatero, en abanico,
//! con el mismo sentido y el color de su vertice 0. Un suelo que pasa por
//! debajo de la camara se ve hasta el borde de la pantalla. Lo que no cruza,
//! ni se toca: los fotogramas de antes dan los mismos bits.
//!
//! ```text
//!    0  si algun z < 0: Sutherland-Hodgman contra z >= 0, cada componente
//!       de la posicion interpolada con t = za / (za - zb) en el cruce
//! ```
//!
//! [!] La 3060 de hoy no esta puesta asi: `RECORTE_Z` (`ga10x`, `raster.rs`)
//! deja la z sin recorte y la SUJETA en cada pixel. Lo que cae entre w = 0
//! y el plano de cerca ella lo pinta (con z = 0) y el juez no; es lo que
//! esta a menos de la distancia de cerca de los ojos, casi siempre fuera de
//! la pantalla. Cuantos pixeles son, lo dice el juez de `laminas` en el
//! metal (M1 de `EL_FOCO`); si son muchos, el arreglo es poner la 3060 como
//! D3D (`FRUSTUM_XYZ_CLIP`), no cambiar el juez.

use crate::dibujo::{check_draw, Draw, DrawBackend, MAX_IO};
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

/// La imagen limpia (con `fondo`; sin el, lo que habia) y, si hay
/// profundidad, su z-buffer (a 1.0 si se limpia); sin el, un dibujo con
/// profundidad no se dibuja.
fn limpiar<'a>(out: &'a mut Image, fondo: Option<u32>, depth: bool) -> Result<(&'a mut [u32], Option<&'a mut [f32]>), Error> {
    let n = (out.width * out.height) as usize;
    let Image { pixels, depth: z, .. } = out;
    if let Some(f) = fondo {
        pixels[..n].fill(f);
    }
    if !depth {
        return Ok((pixels, None));
    }
    match z.as_deref_mut() {
        Some(z) if z.len() >= n => {
            if fondo.is_some() {
                z[..n].fill(1.0);
            }
            Ok((pixels, Some(z)))
        }
        _ => Err(Error::Image),
    }
}

/// Un pixel guardado (`0xAARRGGBB`) a sus cuatro canales (la mezcla lee el
/// que esta).
fn canales(p: u32) -> [f32; 4] {
    let b = |k: u32| ((p >> k) & 0xFF) as f32 / 255.0;
    [b(16), b(8), b(0), b(24)]
}

impl Backend for Cpu {
    fn draw(&mut self, frame: &Frame, out: &mut Image) -> Result<Stats, Error> {
        check(frame, out, self.max_vertices)?;
        let (w, h) = (out.width as i64, out.height as i64);
        let (pixels, mut zb) = limpiar(out, Some(self.unorm8.pack(frame.clear)), frame.depth)?;
        let (mw, mh) = (frame.viewport.width as f32 * 0.5, frame.viewport.height as f32 * 0.5);
        let mut n = 0;
        for tri in frame.vertices.chunks_exact(3) {
            let color = self.unorm8.pack(tri[0].color);
            let (piezas, cuantas) = recorta([tri[0].position, tri[1].position, tri[2].position]);
            for pieza in &piezas[..cuantas] {
                let mut pinta = |i: usize, _: i64, _: i64, _: &[i64; 3]| {
                    pixels[i] = color;
                    true
                };
                n += self.triangulo(frame.cull, pieza, &mut pinta, (w, h), (mw, mh), zb.as_deref_mut()) as u32;
            }
        }
        Ok(Stats { triangles: n, device_us: 0, prepare_us: 0, warm: false, in_flight: false, wait_us: 0 })
    }
}

/// ** V3a: EL DIBUJO CON PROGRAMAS (las reglas, en `dibujo.rs`). Cada
/// triangulo corre el de vertice en sus tres (sin cache: es el juez, no la
/// carrera) y el de pixeles UNA vez, con lo del vertice 0 -- o, si lee su
/// POSICION, en cada pixel (regla 5) --.
impl DrawBackend for Cpu {
    fn draw_targets(&mut self, d: &Draw, out: &mut Image, others: &mut [&mut [u32]]) -> Result<Stats, Error> {
        check_draw(d, out, others, self.max_vertices)?;
        let p = d.pipeline;
        let (w, h) = (out.width as i64, out.height as i64);
        let fondo = d.clear.map(|c| self.unorm8.pack(c));
        let (pixels, mut zb) = limpiar(out, fondo, p.depth)?;
        if let Some(f) = fondo {
            for o in others[..p.targets - 1].iter_mut() {
                o[..(w * h) as usize].fill(f);
            }
        }
        let (mw, mh) = (d.viewport.width as f32 * 0.5, d.viewport.height as f32 * 0.5);
        let stride = d.vertices.stride;
        let unorm8 = self.unorm8;
        let mut n = 0;
        let mut salidas = [[[0f32; 4]; MAX_IO]; 3];
        // ** reglas 6 y 7: cada salida a su render target, con su mezcla
        let mut poner = |i: usize, colores: &[[f32; 4]; MAX_IO]| {
            for t in 0..p.targets {
                let destino: &mut u32 = if t == 0 { &mut pixels[i] } else { &mut others[t - 1][i] };
                let b = p.blend[t];
                *destino = if b.trivial() { unorm8.pack(colores[t]) } else { unorm8.pack(b.apply(colores[t], canales(*destino), p.blend_factor)) };
            }
        };
        for t in 0..d.count / 3 {
            for (j, s) in salidas.iter_mut().enumerate() {
                let v = d.vertex_at(3 * t + j);
                *s = [[0.0; 4]; MAX_IO];
                p.vertex.run(&d.vertices.data[v * stride..(v + 1) * stride], d.constants, &mut s[..p.vertex.outputs()]);
            }
            let (piezas, cuantas) = recorta([salidas[0][0], salidas[1][0], salidas[2][0]]);
            let entrada = &salidas[0][..p.pixel.inputs()];
            if !p.pixel.reads(0) {
                // regla 3: plano, del vertice 0; una vez por triangulo
                let mut colores = [[0f32; 4]; MAX_IO];
                if !p.pixel.run(entrada, d.constants, &mut colores[..p.pixel.outputs()]) {
                    continue;
                }
                for pieza in &piezas[..cuantas] {
                    let mut pinta = |i: usize, _: i64, _: i64, _: &[i64; 3]| {
                        poner(i, &colores);
                        true
                    };
                    n += self.triangulo(p.cull, pieza, &mut pinta, (w, h), (mw, mh), zb.as_deref_mut()) as u32;
                }
                continue;
            }
            // regla 5: lee SV_Position -- en CADA pixel, lo de la casa (N5.9)
            for pieza in &piezas[..cuantas] {
                let mut pinta = |i: usize, px: i64, py: i64, e: &[i64; 3]| {
                    let mut ent = [[0f32; 4]; MAX_IO];
                    ent[..entrada.len()].copy_from_slice(entrada);
                    ent[0] = posicion(pieza, px, py, e);
                    let mut colores = [[0f32; 4]; MAX_IO];
                    if !p.pixel.run(&ent[..entrada.len()], d.constants, &mut colores[..p.pixel.outputs()]) {
                        return false;
                    }
                    poner(i, &colores);
                    true
                };
                n += self.triangulo(p.cull, pieza, &mut pinta, (w, h), (mw, mh), zb.as_deref_mut()) as u32;
            }
        }
        Ok(Stats { triangles: n, device_us: 0, prepare_us: 0, warm: false, in_flight: false, wait_us: 0 })
    }
}

/// ** EL COMPUTO en el juez: los techos de D3D12 y el programa, hilo a hilo
/// (el de la casa, con su memoria compartida y sus barreras).
impl crate::dibujo::ComputeBackend for Cpu {
    fn dispatch(&mut self, d: &crate::dibujo::Dispatch, reads: &[crate::dibujo::ReadBuffer], writes: &mut [crate::dibujo::RwBuffer]) -> Result<u64, Error> {
        crate::dibujo::check_dispatch(d)?;
        Ok(d.program.dispatch(d.groups, d.constants, reads, writes))
    }
}

/// **SV_Position en el pixel** (regla 5 de `dibujo.rs`, la de la casa:
/// `bmo_proton_x::trama`, N5.9): el centro del pixel, la z lineal en la
/// pantalla y la w de RECORTE con perspectiva (la inversa de la media de las
/// 1/w) -- la de D3D, no la 1/w de GL --. `e`: los pesos de `tri`.
fn posicion(tri: &[[f32; 4]; 3], px: i64, py: i64, e: &[i64; 3]) -> [f32; 4] {
    let s = (e[0] + e[1] + e[2]) as f32;
    let (b1, b2) = (e[1] as f32 / s, e[2] as f32 / s);
    let inv_w = [1.0 / tri[0][3], 1.0 / tri[1][3], 1.0 / tri[2][3]];
    let zv = [tri[0][2] * inv_w[0], tri[1][2] * inv_w[1], tri[2][2] * inv_w[2]];
    let z = zv[0] + b1 * (zv[1] - zv[0]) + b2 * (zv[2] - zv[0]);
    let w = 1.0 / ((1.0 - b1 - b2) * inv_w[0] + b1 * inv_w[1] + b2 * inv_w[2]);
    [px as f32 + 0.5, py as f32 + 0.5, z, w]
}

/// ** V2b: el triangulo recortado contra z >= 0 (el plano de cerca), en
/// triangulos: uno si no cruza (el MISMO, sin tocar), o dos.
fn recorta(t: [[f32; 4]; 3]) -> ([[[f32; 4]; 3]; 2], usize) {
    if t.iter().all(|p| p[2] >= 0.0) {
        return ([t, t], 1);
    }
    let mut poli = [[0f32; 4]; 4];
    let mut m = 0;
    for k in 0..3 {
        let (a, b) = (t[k], t[(k + 1) % 3]);
        if a[2] >= 0.0 {
            poli[m] = a;
            m += 1;
        }
        if (a[2] >= 0.0) != (b[2] >= 0.0) {
            let s = a[2] / (a[2] - b[2]);
            let mut c = [0f32; 4];
            for i in 0..4 {
                c[i] = a[i] + s * (b[i] - a[i]);
            }
            c[2] = 0.0;
            poli[m] = c;
            m += 1;
        }
    }
    match m {
        3 => ([[poli[0], poli[1], poli[2]], [poli[0], poli[1], poli[2]]], 1),
        4 => ([[poli[0], poli[1], poli[2]], [poli[0], poli[2], poli[3]]], 2),
        _ => ([t, t], 0),
    }
}

impl Cpu {
    /// Un triangulo ya delante del plano de cerca: `true` si se dibujo (no
    /// descartado ni de area 0). `pinta(i, x, y, e)` sombrea y pinta cada
    /// pixel que cubre y pasa la profundidad (`i` su indice), y dice si
    /// quedo -- `false`: su programa lo tiro, y la profundidad no se toca --;
    /// `e` son los pesos de sus vertices (por el area), en el orden de `tri`.
    fn triangulo(&self, cull: Cull, tri: &[[f32; 4]; 3], pinta: &mut dyn FnMut(usize, i64, i64, &[i64; 3]) -> bool, (w, h): (i64, i64), (mw, mh): (f32, f32), mut zb: Option<&mut [f32]>) -> bool {
        {
            let mut x = [0i64; 3];
            let mut y = [0i64; 3];
            let mut zn = [0f64; 3];
            for k in 0..3 {
                let p = tri[k];
                let inv_w = 1.0 / p[3];
                let (nx, ny) = (p[0] * inv_w, p[1] * inv_w);
                x[k] = redondear_par((nx * mw + mw) * SUBPIXEL as f32);
                y[k] = redondear_par((-ny * mh + mh) * SUBPIXEL as f32);
                zn[k] = (p[2] * inv_w) as f64;
            }
            let mut area = arista(x[0], y[0], x[1], y[1], x[2], y[2]);
            // V2: delante es horario en la pantalla (area > 0, con y hacia abajo)
            if cull == Cull::Back && area < 0 {
                return false;
            }
            let girado = area < 0;
            if girado {
                x.swap(1, 2);
                y.swap(1, 2);
                zn.swap(1, 2);
                area = -area;
            }
            if area == 0 {
                return false;
            }
            let incluye = [top_left(x[1], y[1], x[2], y[2]), top_left(x[2], y[2], x[0], y[0]), top_left(x[0], y[0], x[1], y[1])];
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
                        let mut zp = 0.0;
                        if zb.is_some() {
                            // e[k] es el peso del vertice k (por el area)
                            zp = ((e[0] as f64 * zn[0] + e[1] as f64 * zn[1] + e[2] as f64 * zn[2]) / area as f64) as f32;
                            if !(0.0..=1.0).contains(&zp) || zb.as_deref().is_some_and(|z| zp >= z[i]) {
                                continue;
                            }
                        }
                        // el programa de pixeles corre ANTES de escribir la
                        // profundidad: uno que se tira no la toca (D3D)
                        let pesos = if girado { [e[0], e[2], e[1]] } else { e };
                        if !pinta(i, px, py, &pesos) {
                            continue;
                        }
                        if let Some(z) = zb.as_deref_mut() {
                            z[i] = zp;
                        }
                    }
                }
            }
        }
        true
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

    /// Un punto de la vista (x a la derecha, y arriba, z delante) a recorte:
    /// focal 1,5 en 4:3, cerca 0,1 y lejos 64 -- la de MUNDO --.
    fn a_recorte(x: f32, y: f32, z: f32, color: [f32; 4]) -> Vertex {
        Vertex { position: [x * 1.125, y * 1.5, (z - 0.1) * 1.0015649, z], color }
    }

    /// ** V2b: UN SUELO QUE PASA POR DEBAJO DE LA CAMARA. Dos triangulos, con
    /// dos esquinas DETRAS de ella (w < 0): recortados contra el plano de
    /// cerca, el suelo llega al borde de abajo de la pantalla y nada sube
    /// por encima del horizonte. Sin recorte, la division les da la vuelta.
    #[test]
    fn the_near_plane_cuts_a_floor_under_the_camera() {
        let c = |x: f32, z: f32| a_recorte(x, -1.0, z, VERDE);
        // horario visto desde arriba (la normal sale hacia +y)
        let suelo = vec![c(-5.0, -2.0), c(-5.0, 10.0), c(5.0, 10.0), c(-5.0, -2.0), c(5.0, 10.0), c(5.0, -2.0)];
        for cull in [Cull::None, Cull::Back] {
            let p = pinta(&suelo, true, cull);
            // abajo del todo, en el centro y en las esquinas: suelo
            for x in [0, 32, 63] {
                assert_eq!(en(&p, x, 47), 0x00FF_00, "{cull:?}: ({x}, 47)");
            }
            // el horizonte del suelo (z = 10) cae en la fila 24 + 1,5 / 10 * 24 = 27,6
            for y in 0..27 {
                assert!((0..64).all(|x| en(&p, x, y) != 0x00FF_00), "{cull:?}: la fila {y} es cielo");
            }
        }
        // todo detras de la camara: nada
        let detras: Vec<Vertex> = suelo.iter().map(|v| a_recorte(v.position[0] / 1.125, -1.0, -v.position[3] - 1.0, VERDE)).collect();
        assert!(pinta(&detras, true, Cull::None).iter().all(|&x| x != 0x00FF_00));
    }

    /// Lo que no cruza el plano de cerca no se toca: el mismo triangulo, sin
    /// partir.
    #[test]
    fn what_does_not_cross_is_left_alone() {
        let t = [[0.1, 0.2, 0.3, 1.0], [0.5, -0.2, 0.0, 1.0], [-0.4, 0.1, 0.9, 2.0]];
        let (piezas, n) = recorta(t);
        assert_eq!((n, piezas[0]), (1, t));
        // uno detras: un cuadrilatero, dos triangulos, todos con z >= 0
        let (piezas, n) = recorta([[0.0, 0.0, 0.5, 1.0], [1.0, 0.0, 0.5, 1.0], [0.0, 1.0, -0.5, -0.4]]);
        assert_eq!(n, 2);
        assert!(piezas.iter().flatten().all(|p| p[2] >= 0.0 && p[3] > 0.0));
    }

    /// ** V3a / M2 en el anfitrion: EL CUBO DE BMOX-12 CON SUS PROGRAMAS.
    /// Los 24 vertices SIN transformar, sus 36 indices y el cbuffer (la
    /// matriz, el mundo, la luz) -- lo que da D3D12 --, y los DOS programas
    /// que dxc compilo para BMOX-12, traducidos al Programa de la casa: el de
    /// vertice transforma, el de pixeles ilumina. El juez los corre y da la
    /// HUELLA de D3D12 en la 3060, fotograma a fotograma. La CPU ya no cuenta
    /// ni un vertice: solo pone la matriz.
    #[cfg(feature = "prometeo")]
    mod con_programas {
        use super::*;
        use crate::dibujo::{Buffer, Draw, DrawBackend, Pipeline, Shader};
        use bmo_prometeo::programa::Programa;

        fn programa(d: &[u8]) -> Programa {
            bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(d).unwrap()).unwrap()
        }

        pub(super) fn los_dos() -> (Programa, Programa) {
            (programa(include_bytes!("../../proton-x/prueba/cubo_vs.dxil")), programa(include_bytes!("../../proton-x/prueba/cubo_ps.dxil")))
        }

        pub(super) fn cbuffer(f: u32) -> Vec<u8> {
            let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), ANCHO as f32 / ALTO as f32);
            c.wvp.iter().chain(&c.world).chain(&c.luz).flat_map(|x| x.to_le_bytes()).collect()
        }

        pub(super) fn atributos() -> Vec<[f32; 4]> {
            bmo_cubo::vertices().iter().flat_map(|v| [[v.pos[0], v.pos[1], v.pos[2], 1.0], [v.normal[0], v.normal[1], v.normal[2], 0.0], v.color]).collect()
        }

        pub(super) fn dibuja(vs: &dyn Shader, ps: &dyn Shader, cb: &[u8], indices: &[u32], cull: Cull) -> Result<(Vec<u32>, u32), Error> {
            let datos = atributos();
            let d = Draw {
                clear: Some(FONDO_F),
                viewport: Viewport { width: ANCHO, height: ALTO },
                pipeline: Pipeline::new(vs, ps, false, cull),
                vertices: Buffer { data: &datos, stride: 3 },
                indices: Some(indices),
                constants: cb,
                count: indices.len(),
            };
            let mut px = vec![0u32; (ANCHO * ALTO) as usize];
            let mut juez = Cpu { max_vertices: 1 << 16, ..Cpu::LA_3060 };
            let s = juez.draw_with(&d, &mut Image { pixels: &mut px, width: ANCHO, height: ALTO, depth: None })?;
            Ok((px, s.triangles))
        }

        pub(super) fn indices() -> Vec<u32> {
            bmo_cubo::indices().iter().map(|&i| i as u32).collect()
        }

        #[test]
        fn the_cube_of_bmox12_with_its_own_programs_gives_the_d3d12_print() {
            let (vs, ps) = los_dos();
            for f in [0, 30, 60] {
                let (px, tris) = dibuja(&vs, &ps, &cbuffer(f), &indices(), Cull::Back).unwrap();
                assert!((3..=8).contains(&tris), "fotograma {f}: {tris} triangulos de 12 miran");
                assert_eq!(Some(huella(&px)), de_la_3060(f), "fotograma {f}: la huella de D3D12");
            }
        }

        /// Sin descarte, las caras de detras se pintan encima (no hay
        /// profundidad): la huella ya NO es la de D3D12. Y con la matriz de
        /// otro fotograma, tampoco: lo que gira es la CONSTANTE.
        #[test]
        fn the_print_depends_on_the_cull_and_on_the_constants() {
            let (vs, ps) = los_dos();
            let (px, _) = dibuja(&vs, &ps, &cbuffer(30), &indices(), Cull::None).unwrap();
            assert_ne!(Some(huella(&px)), de_la_3060(30));
            let (px, _) = dibuja(&vs, &ps, &cbuffer(31), &indices(), Cull::Back).unwrap();
            assert_ne!(Some(huella(&px)), de_la_3060(30));
        }

        /// Un programa de vertice de prueba: deja su entrada 0 tal cual (la
        /// posicion ya en recorte). Uno que no es el Programa de la casa:
        /// cualquier etapa entra por `Shader`.
        struct Paso;
        impl Shader for Paso {
            fn inputs(&self) -> usize {
                1
            }
            fn outputs(&self) -> usize {
                1
            }
            fn reads(&self, k: usize) -> bool {
                k == 0
            }
            fn run(&self, i: &[[f32; 4]], _: &[u8], o: &mut [[f32; 4]]) -> bool {
                o[0] = i[0];
                true
            }
        }

        /// Uno de pixeles de prueba: la w de su SV_Position / 4, en rojo.
        struct LaW;
        impl Shader for LaW {
            fn inputs(&self) -> usize {
                1
            }
            fn outputs(&self) -> usize {
                1
            }
            fn reads(&self, k: usize) -> bool {
                k == 0
            }
            fn run(&self, i: &[[f32; 4]], _: &[u8], o: &mut [[f32; 4]]) -> bool {
                o[0] = [i[0][3] / 4.0, 0.0, 0.0, 1.0];
                true
            }
        }

        fn de_8x8(vs: &dyn Shader, ps: &dyn Shader, datos: &[[f32; 4]], indices: &[u32]) -> Vec<u32> {
            let d = Draw {
                clear: Some([0.0; 4]),
                viewport: Viewport { width: 8, height: 8 },
                pipeline: Pipeline::new(vs, ps, false, Cull::None),
                vertices: Buffer { data: datos, stride: 1 },
                indices: Some(indices),
                constants: &[],
                count: indices.len(),
            };
            let mut px = vec![0u32; 64];
            let mut juez = Cpu { max_vertices: 64, ..Cpu::D3D10 };
            juez.draw_with(&d, &mut Image { pixels: &mut px, width: 8, height: 8, depth: None }).unwrap();
            px
        }

        /// *** Cyberpunk lee SV_Position en el de pixeles (su G-buffer, su
        /// ruido): era lo primero que la 3060 le nego. `posicion.hlsl` de la
        /// casa (dxc) en un destino de 8x8: cada pixel ve SU centro, la z del
        /// viewport y la w de recorte -- lo mismo que la prueba N5.9 de
        /// `bmo_proton_x` --.
        #[test]
        fn the_pixel_program_reads_its_sv_position() {
            let ps = programa(include_bytes!("../../proton-x/prueba/posicion.dxil"));
            assert!(ps.reads(0), "posicion.hlsl lee su SV_Position");
            let (z, w) = (0.5f32, 2.0f32);
            let cuadro: Vec<[f32; 4]> = [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].iter().map(|&(x, y)| [x * w, y * w, z * w, w]).collect();
            let px = de_8x8(&Paso, &ps, &cuadro, &[0, 1, 2, 2, 1, 3]);
            for y in 0..8 {
                for x in 0..8 {
                    let esperado = Cpu::D3D10.unorm8.pack([(x as f32 + 0.5) / 8.0, (y as f32 + 0.5) / 8.0, 0.5, 0.5]);
                    assert_eq!(px[y * 8 + x], esperado, "pixel ({x}, {y})");
                }
            }
        }

        /// La w va con PERSPECTIVA: con w distinta por vertice, la del centro
        /// de un pixel es la inversa de la media de las 1/w (el triangulo de
        /// la prueba de la casa, `la_w_de_sv_position_va_con_perspectiva`).
        #[test]
        fn the_w_of_sv_position_goes_with_perspective() {
            let tri = [[-1.0, 1.0, 0.0, 1.0], [12.0, 4.0, 0.0, 4.0], [-4.0, -12.0, 0.0, 4.0]];
            let px = de_8x8(&Paso, &LaW, &tri, &[0, 1, 2]);
            let pesos = |x: f32, y: f32| {
                let (b1, b2) = (x / 16.0, y / 16.0);
                1.0 / ((1.0 - b1 - b2) / 1.0 + b1 / 4.0 + b2 / 4.0)
            };
            for (x, y) in [(0usize, 0usize), (7, 7), (3, 5)] {
                let esperado = Cpu::D3D10.unorm8.pack([pesos(x as f32 + 0.5, y as f32 + 0.5) / 4.0, 0.0, 0.0, 1.0]);
                assert_eq!(px[y * 8 + x], esperado, "pixel ({x}, {y})");
            }
            // y no es plana: la esquina de w = 1 y la del fondo, distintas
            assert_ne!(px[0], px[63]);
        }

        /// Uno de pixeles de prueba: los mismos colores siempre (uno por
        /// render target).
        struct Fijo([[f32; 4]; 3]);
        impl Shader for Fijo {
            fn inputs(&self) -> usize {
                0
            }
            fn outputs(&self) -> usize {
                3
            }
            fn reads(&self, _: usize) -> bool {
                false
            }
            fn run(&self, _: &[[f32; 4]], _: &[u8], o: &mut [[f32; 4]]) -> bool {
                o.copy_from_slice(&self.0[..o.len()]);
                true
            }
        }

        /// El cuadro que llena 8x8, en recorte (w = 1).
        fn lleno() -> Vec<[f32; 4]> {
            [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].iter().map(|&(x, y)| [x, y, 0.5, 1.0]).collect()
        }

        /// Un dibujo del cuadro en 8x8 sobre lo que ya hay en `px` (y en
        /// `otros`), con `p` como tuberia.
        fn encima(p: Pipeline, clear: Option<[f32; 4]>, px: &mut [u32], otros: &mut [&mut [u32]]) -> Result<Stats, Error> {
            let datos = lleno();
            let d = Draw { clear, viewport: Viewport { width: 8, height: 8 }, pipeline: p, vertices: Buffer { data: &datos, stride: 1 }, indices: Some(&[0, 1, 2, 2, 1, 3]), constants: &[], count: 6 };
            let mut juez = Cpu { max_vertices: 64, ..Cpu::D3D10 };
            juez.draw_targets(&d, &mut Image { pixels: px, width: 8, height: 8, depth: None }, otros)
        }

        /// *** LA MEZCLA ES LA DE LA CASA (N5.11): cada factor y cada
        /// operacion de VERRANO dan los MISMOS bits que `bmo_proton_x::
        /// mezcla`, sobre colores que no son 0 ni 1.
        #[test]
        fn blending_is_the_house_blending() {
            use crate::dibujo::{Blend, BlendOp, Factor};
            use bmo_proton_x::mezcla as m;
            let factores = [
                (Factor::Zero, m::CERO),
                (Factor::One, m::UNO),
                (Factor::SrcColor, m::ORIGEN_COLOR),
                (Factor::InvSrcColor, m::INV_ORIGEN_COLOR),
                (Factor::SrcAlpha, m::ORIGEN_ALFA),
                (Factor::InvSrcAlpha, m::INV_ORIGEN_ALFA),
                (Factor::DestAlpha, m::DESTINO_ALFA),
                (Factor::InvDestAlpha, m::INV_DESTINO_ALFA),
                (Factor::DestColor, m::DESTINO_COLOR),
                (Factor::InvDestColor, m::INV_DESTINO_COLOR),
                (Factor::SrcAlphaSat, m::ORIGEN_ALFA_SAT),
                (Factor::BlendFactor, m::FACTOR),
                (Factor::InvBlendFactor, m::INV_FACTOR),
            ];
            let ops = [(BlendOp::Add, m::SUMAR), (BlendOp::Subtract, m::RESTAR), (BlendOp::RevSubtract, m::RESTAR_AL_REVES), (BlendOp::Min, m::MIN), (BlendOp::Max, m::MAX)];
            let (o, d, k) = ([0.9, 0.3, 0.55, 0.4], [0.2, 0.7, 0.35, 0.8], [0.6, 0.15, 0.85, 0.3]);
            let mut vistas = 0;
            for &(fo, mo) in &factores {
                for &(fd, md) in &factores {
                    for &(op, mop) in &ops {
                        for mascara in [0xF, 0b1010] {
                            let v = Blend { enable: true, src: fo, dst: fd, op, src_alpha: fd, dst_alpha: fo, op_alpha: op, write_mask: mascara };
                            let casa = m::Mezcla { encendida: true, origen: mo, destino: md, op: mop, origen_a: md, destino_a: mo, op_a: mop, mascara };
                            let (a, b) = (v.apply(o, d, k), casa.aplicar(o, d, k));
                            assert_eq!(a.map(f32::to_bits), b.map(f32::to_bits), "{fo:?} {fd:?} {op:?} {mascara:#x}");
                            vistas += 1;
                        }
                    }
                }
            }
            assert_eq!(vistas, 13 * 13 * 5 * 2);
        }

        /// ** LA LUZ QUE SE SUMA: una pasada que limpia y pinta 0,25, y otra
        /// SIN limpiar que suma 0,5 encima (One, One): lo guardado mas 0,5.
        /// Y la transparencia (SrcAlpha, InvSrcAlpha) de rojo a la mitad sobre
        /// azul; y la mascara, que deja lo que no esta.
        #[test]
        fn light_adds_up_over_two_passes() {
            use crate::dibujo::Blend;
            let mut px = vec![0u32; 64];
            let base = Fijo([[0.25, 0.25, 0.25, 1.0]; 3]);
            encima(Pipeline::new(&Paso, &base, false, Cull::None), Some([0.0, 0.0, 0.0, 1.0]), &mut px, &mut []).unwrap();
            let luz = Fijo([[0.5, 0.5, 0.5, 0.0]; 3]);
            let mut p = Pipeline::new(&Paso, &luz, false, Cull::None);
            p.blend[0] = Blend::ADD;
            encima(p, None, &mut px, &mut []).unwrap();
            // el destino es de 8 bits: guardo 0,25 como 64/255, y sobre ESO se
            // suma (lo de D3D con un render target UNORM8): 192, no 191
            let guardado = Cpu::D3D10.unorm8.convert(0.25) as f32 / 255.0;
            let esperado = Cpu::D3D10.unorm8.pack([guardado + 0.5, guardado + 0.5, guardado + 0.5, 1.0]);
            assert_eq!(esperado & 0xFF, 192);
            assert!(px.iter().all(|&c| c == esperado), "{:#x} contra {esperado:#x}", px[0]);

            let mut px = vec![0u32; 64];
            encima(Pipeline::new(&Paso, &Fijo([[0.0, 0.0, 1.0, 1.0]; 3]), false, Cull::None), Some([0.0; 4]), &mut px, &mut []).unwrap();
            let mut p = Pipeline::new(&Paso, &Fijo([[1.0, 0.0, 0.0, 0.5]; 3]), false, Cull::None);
            p.blend[0] = Blend::ALPHA;
            encima(p, None, &mut px, &mut []).unwrap();
            assert_eq!(px[27], Cpu::D3D10.unorm8.pack([0.5, 0.0, 0.5, 1.0]), "rojo a la mitad sobre azul");

            let mut p = Pipeline::new(&Paso, &Fijo([[1.0, 1.0, 1.0, 1.0]; 3]), false, Cull::None);
            p.blend[0] = Blend { write_mask: 0b0001, ..Blend::NONE };
            encima(p, None, &mut px, &mut []).unwrap();
            assert_eq!(px[27], Cpu::D3D10.unorm8.pack([1.0, 0.0, 0.5, 1.0]), "solo el rojo cambia");
        }

        /// *** EL G-BUFFER: `gbuffer.hlsl` de la casa (dxc) escribe tres
        /// render targets a la vez -- el albedo, el SITIO (de su SV_Position)
        /// y la normal --, como Cyberpunk antes de su luz. Cada salida en el
        /// suyo, pixel a pixel.
        #[test]
        fn the_gbuffer_writes_three_targets_at_once() {
            let ps = programa(include_bytes!("../../proton-x/prueba/gbuffer.dxil"));
            assert_eq!((ps.outputs(), ps.reads(0)), (3, true));
            // uno de vertice que deja la posicion y un TEXCOORD a cero
            struct DosSalidas;
            impl Shader for DosSalidas {
                fn inputs(&self) -> usize {
                    1
                }
                fn outputs(&self) -> usize {
                    2
                }
                fn reads(&self, k: usize) -> bool {
                    k == 0
                }
                fn run(&self, i: &[[f32; 4]], _: &[u8], o: &mut [[f32; 4]]) -> bool {
                    o[0] = i[0];
                    o[1] = [0.0; 4];
                    true
                }
            }
            let (mut a, mut b, mut c) = (vec![0u32; 64], vec![0u32; 64], vec![0u32; 64]);
            let mut p = Pipeline::new(&DosSalidas, &ps, false, Cull::None);
            p.targets = 3;
            encima(p, Some([0.0; 4]), &mut a, &mut [&mut b, &mut c]).unwrap();
            let u = Cpu::D3D10.unorm8;
            for y in 0..8 {
                for x in 0..8 {
                    let i = y * 8 + x;
                    assert_eq!(a[i], u.pack([1.0, 0.0, 0.0, 1.0]), "albedo ({x}, {y})");
                    assert_eq!(b[i], u.pack([(x as f32 + 0.5) / 8.0, (y as f32 + 0.5) / 8.0, 0.0, 1.0]), "sitio ({x}, {y})");
                    assert_eq!(c[i], u.pack([0.0, 0.0, 1.0, 0.5]), "normal ({x}, {y})");
                }
            }
            // mas render targets que salidas, o sin su imagen: se dice
            let mut p = Pipeline::new(&DosSalidas, &ps, false, Cull::None);
            p.targets = 4;
            assert_eq!(encima(p, None, &mut a, &mut [&mut b, &mut c]).err(), Some(Error::Shader));
            p.targets = 3;
            assert_eq!(encima(p, None, &mut a, &mut [&mut b]).err(), Some(Error::Image));
        }

        fn computo(d: &[u8]) -> Programa {
            bmo_proton_x::dxil::computo::preparar(d).unwrap().programa
        }

        fn f32s(b: &[u8], k: usize) -> [f32; 4] {
            core::array::from_fn(|c| f32::from_le_bytes(b[16 * k + 4 * c..16 * k + 4 * c + 4].try_into().unwrap()))
        }

        fn u32s(b: &[u8], k: usize) -> [u32; 4] {
            core::array::from_fn(|c| u32::from_le_bytes(b[16 * k + 4 * c..16 * k + 4 * c + 4].try_into().unwrap()))
        }

        /// *** EL COMPUTO POR VERRANO: `computo.hlsl` de la casa (dxc), dos
        /// grupos de 64 hilos. Cada hilo deja su elemento en la memoria
        /// COMPARTIDA, espera a los demas (la barrera) y escribe el de su
        /// ESPEJO por la escala, con su grupo y su indice en la w: sin la
        /// barrera o sin memoria compartida de verdad, sale otra cosa.
        #[test]
        fn compute_runs_with_shared_memory_and_a_barrier() {
            use crate::dibujo::{ComputeBackend, Dispatch, ReadBuffer, RwBuffer};
            let cs = computo(include_bytes!("../../proton-x/prueba/computo.dxil"));
            assert_eq!(crate::dibujo::Compute::threads(&cs), [64, 1, 1]);
            let entrada: Vec<u8> = (0..128u32).flat_map(|k| [k as f32, 0.5, -1.0, 2.0]).flat_map(f32::to_le_bytes).collect();
            let mut salida = vec![0u8; 128 * 16];
            let mut cb = [0u8; 16];
            cb[..4].copy_from_slice(&128u32.to_le_bytes());
            cb[4..8].copy_from_slice(&3.0f32.to_le_bytes());
            let d = Dispatch { program: &cs, groups: [2, 1, 1], constants: &cb };
            let mut juez = Cpu::LA_3060;
            let hilos = juez.dispatch(&d, &[ReadBuffer { bytes: &entrada, stride: 16 }], &mut [RwBuffer { bytes: &mut salida, stride: 16 }]).unwrap();
            assert_eq!(hilos, 128);
            for k in 0..128usize {
                let (g, gi) = (k / 64, k % 64);
                let espejo = (64 * g + 63 - gi) as f32;
                assert_eq!(f32s(&salida, k), [espejo * 3.0, 1.5, -3.0, (g * 1000 + gi) as f32], "hilo {k}");
            }
        }

        /// Lo que pidieron dos CS de Cyberpunk en el metal (06-10): los
        /// `Interlocked*` sobre la memoria COMPARTIDA y `mad` de enteros
        /// (`compartido.hlsl`). Lo que se juzga no depende del orden de los
        /// hilos: lo que quedo, que los "antes" de la suma son distintos, y
        /// que UNO solo vio el cambio sin hacer.
        #[test]
        fn compute_atomics_on_shared_memory() {
            use crate::dibujo::{ComputeBackend, Dispatch, RwBuffer};
            let cs = computo(include_bytes!("../../proton-x/prueba/compartido.dxil"));
            let mut salida = vec![0u8; 65 * 16];
            let d = Dispatch { program: &cs, groups: [1, 1, 1], constants: &[] };
            let mut juez = Cpu::LA_3060;
            juez.dispatch(&d, &[], &mut [RwBuffer { bytes: &mut salida, stride: 16 }]).unwrap();
            // w = 5 gi + 1; v = 200 - 3 gi
            let suma: u32 = (0..64).map(|gi| 5 * gi + 1).sum();
            assert_eq!(u32s(&salida, 64), [suma, 200 - 3 * 63, 33, u32::MAX], "la suma, el menor, el mayor y los bits");
            let mut antes: Vec<u32> = (0..64).map(|k| u32s(&salida, k)[2]).collect();
            antes.sort();
            antes.dedup();
            assert_eq!(antes.len(), 64, "cada hilo vio la suma de ANTES, distinta");
            assert_eq!((0..64).filter(|&k| u32s(&salida, k)[3] == 1).count(), 1, "uno solo vio el contador sin cambiar");
        }

        /// Los techos de D3D12 se dicen: sin grupos, o mas de 65535 por eje.
        #[test]
        fn a_dispatch_out_of_the_limits_is_refused() {
            use crate::dibujo::{ComputeBackend, Dispatch};
            let cs = computo(include_bytes!("../../proton-x/prueba/compartido.dxil"));
            let mut juez = Cpu::LA_3060;
            for groups in [[0, 1, 1], [65536, 1, 1], [1, 0, 1]] {
                assert_eq!(juez.dispatch(&Dispatch { program: &cs, groups, constants: &[] }, &[], &mut []).err(), Some(Error::Groups), "{groups:?}");
            }
        }

        /// Lo que no encaja se dice: un indice fuera, una de pixeles que lee
        /// mas de lo que deja la de vertice.
        #[test]
        fn what_does_not_fit_is_refused() {
            let (vs, ps) = los_dos();
            let mut fuera = indices();
            fuera[5] = 24;
            assert_eq!(dibuja(&vs, &ps, &cbuffer(0), &fuera, Cull::Back).err(), Some(Error::Vertices));
            // la de pixeles como de vertice: lee 3, y la de pixeles deja 1
            assert_eq!(dibuja(&ps, &ps, &cbuffer(0), &indices(), Cull::Back).err(), Some(Error::Shader));
        }
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
