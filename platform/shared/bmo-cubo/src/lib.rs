//! # cubo-neutro (en BMO-X: `bmo-cubo`)
//!
//! generacion: nieto -- la malla, las matrices y el juez; no sabe que es una GPU
//! capa: puro -- ni un `unsafe`, ni un aparato: lo usan el escritorio y el banco
//!
//! [carril]  VERDE     aritmetica pura sobre un bufer que le dan
//! [cuesta]  NADA      corre cuando se pide `gpu cubo` (un fotograma, unos ms)
//!
//! TRAIDO, no escrito aqui: ver `PROCEDENCIA.md` (rama, hash, y lo unico que
//! cambio: los comentarios pasados a ASCII). Lo propio de BMO-X va en
//! [`referencia`]: las huellas de las capturas de D3D12 en la 3060.
//!
//! Todo lo del cubo que NO depende de la plataforma: malla, matrices, luz y un
//! rasterizador por CPU que sirve de referencia ("el juez") para comparar contra la GPU.
//!
//! Reglas: `#![no_std]`, sin dependencias, sin asignar memoria, solo `f32` y aritmetica
//! propia ([`num`]). Es la parte que puede cruzar a BMO-X; lo que llama a Windows vive
//! en `estudio-d3d/` y no cruza.
//!
//! Convenciones (las mismas que usan los cuatro cubos D3D):
//! - Mano izquierda, camara mirando a +Z, profundidad 0..1.
//! - Triangulos frontales en sentido horario en pantalla; los traseros se descartan.
//! - Matrices en orden de columnas y `mul(M, v)` en HLSL.

#![no_std]

#[cfg(test)]
extern crate std;

pub mod mat;
pub mod num;
pub mod referencia;

use mat::{mul, perspectiva, rotacion_x, rotacion_y, transformar, transformar_dir, vista, Mat4};
use num::{normalizar, punto, redondear, redondear_par, saturar, DOS_PI, PI, V3};

/// Formato de vertice (40 bytes). Es `repr(C)` porque la GPU lo lee tal cual.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertice {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

/// Constantes por fotograma (144 bytes = 9 registros float4).
/// En D3D9 van a registros c0..c8; en D3D10/11/12 al cbuffer b0.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constantes {
    pub wvp: Mat4,
    pub world: Mat4,
    /// xyz = direccion hacia la luz ya normalizada, w = luz ambiente.
    pub luz: [f32; 4],
}

pub const NUM_VERTICES: usize = 24;
pub const NUM_INDICES: usize = 36;

/// Fondo 0xFF101018 (igual que `examples/cube_dx12.rs`).
pub const FONDO: u32 = 0xFF10_1018;
pub const FONDO_F: [f32; 4] = [16.0 / 255.0, 16.0 / 255.0, 24.0 / 255.0, 1.0];

pub const OJO: V3 = [0.0, 1.5, -5.0];
pub const FOV_Y: f32 = PI / 3.0;
pub const CERCA: f32 = 0.1;
pub const LEJOS: f32 = 100.0;
/// Direccion hacia la luz sin normalizar.
pub const LUZ: V3 = [0.4, 0.8, -0.5];
pub const AMBIENTE: f32 = 0.25;

/// Un grado por fotograma: el fotograma 360 vuelve a girar Y una vuelta completa.
pub const PASO_POR_FOTOGRAMA: f32 = DOS_PI / 360.0;

pub fn angulo_de_fotograma(n: u32) -> f32 {
    n as f32 * PASO_POR_FOTOGRAMA
}

/// 24 vertices (4 por cara, cada cara con su normal) con un color por cara.
pub fn vertices() -> [Vertice; NUM_VERTICES] {
    // (normal, u, v, color) con u x v = -normal: asi (-u+v, u+v, u-v, -u-v)
    // queda en sentido horario visto desde fuera.
    const CARAS: [(V3, V3, V3, [f32; 4]); 6] = [
        ([0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.19, 0.88, 0.19, 1.0]), // frente: verde
        ([0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.88, 0.19, 0.19, 1.0]), // atras: rojo
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.88, 0.88, 0.19, 1.0]),  // derecha: amarillo
        ([-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [0.19, 0.19, 0.88, 1.0]), // izquierda: azul
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.88, 0.19, 0.88, 1.0]),  // arriba: magenta
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.19, 0.88, 0.88, 1.0]), // abajo: cian
    ];
    const ESQUINAS: [(f32, f32); 4] = [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];

    let mut v = [Vertice { pos: [0.0; 3], normal: [0.0; 3], color: [0.0; 4] }; NUM_VERTICES];
    for (c, (n, du, dv, color)) in CARAS.iter().enumerate() {
        for (e, (a, b)) in ESQUINAS.iter().enumerate() {
            let mut pos = [0.0; 3];
            for k in 0..3 {
                pos[k] = n[k] + du[k] * a + dv[k] * b;
            }
            v[c * 4 + e] = Vertice { pos, normal: *n, color: *color };
        }
    }
    v
}

/// Dos triangulos por cara: (0,1,2) y (0,2,3).
pub fn indices() -> [u16; NUM_INDICES] {
    let mut i = [0u16; NUM_INDICES];
    for c in 0..6u16 {
        let b = c * 4;
        let t = [b, b + 1, b + 2, b, b + 2, b + 3];
        i[c as usize * 6..c as usize * 6 + 6].copy_from_slice(&t);
    }
    i
}

/// World = rotar X (0.7*angulo) y luego Y (angulo). View = camara en [`OJO`]. Projection 60 grados.
pub fn constantes(angulo: f32, aspecto: f32) -> Constantes {
    let world = mul(&rotacion_y(angulo), &rotacion_x(angulo * 0.7));
    let view = vista(OJO, [0.0; 3], [0.0, 1.0, 0.0]);
    let proj = perspectiva(FOV_Y, aspecto, CERCA, LEJOS);
    let l = normalizar(LUZ);
    Constantes { wvp: mul(&mul(&proj, &view), &world), world, luz: [l[0], l[1], l[2], AMBIENTE] }
}

/// Lo mismo que el pixel shader: `rgb * (ambiente + (1 - ambiente) * saturate(dot(n, L)))`.
pub fn iluminar(color: [f32; 4], normal_mundo: V3, luz: [f32; 4]) -> [f32; 4] {
    let d = saturar(punto(normalizar(normal_mundo), [luz[0], luz[1], luz[2]]));
    let k = luz[3] + (1.0 - luz[3]) * d;
    [color[0] * k, color[1] * k, color[2] * k, color[3]]
}

/// Float -> UNORM8 exacto: saturar, escalar por 255 y redondear al mas cercano.
/// Es la conversion ideal; D3D permite a la GPU desviarse hasta 0.6 unidades.
pub fn a_unorm8(x: f32) -> u32 {
    redondear(saturar(x) * 255.0) as u32
}

/// Float -> UNORM8 como la midio una RTX 3060: truncar a 12 bits (`floor(x * 4096)`)
/// y luego redondear a 8 bits. Legal dentro de las 0.6 unidades que permite D3D;
/// otra GPU puede hacerlo distinto.
pub fn a_unorm8_truncar_12(x: f32) -> u32 {
    let t = (saturar(x) * 4096.0) as u32; // x >= 0: truncar = floor
    (t * 255 + 2048) / 4096
}

/// `[r, g, b, a]` en 0..1 -> `0xAARRGGBB` con [`a_unorm8`].
pub fn empaquetar(c: [f32; 4]) -> u32 {
    empaquetar_con(c, a_unorm8)
}

/// `[r, g, b, a]` en 0..1 -> `0xAARRGGBB` con la conversion `unorm8` que se indique.
pub fn empaquetar_con(c: [f32; 4], unorm8: fn(f32) -> u32) -> u32 {
    (unorm8(c[3]) << 24) | (unorm8(c[0]) << 16) | (unorm8(c[1]) << 8) | unorm8(c[2])
}

/// Bits de subpixel: D3D10+ exige ajustar los vertices a una rejilla de 1/256 de pixel.
const SUBPIXEL: i64 = 256;

/// Las cuatro reglas que hacen que la CPU de los mismos pixeles que la GPU.
/// Cada una se descubrio por una diferencia medida (ver README).
#[derive(Clone, Copy)]
pub struct Reglas {
    /// Regla 1. `true`: centro del pixel en (x + 0.5, y + 0.5), como D3D10/11/12.
    /// `false`: centro en coordenadas enteras, como D3D9.
    pub centro_medio_pixel: bool,
    /// Regla 2. Al ajustar a 1/256 de pixel: `true` = empates al par (lo que hace la GPU),
    /// `false` = mitades hacia arriba.
    pub subpixel_al_par: bool,
    /// Regla 3. `true`: viewport como `ndc * (ancho/2) + ancho/2` (lo que hace la GPU).
    /// `false`: `(ndc * 0.5 + 0.5) * ancho`, que en f32 redondea distinto.
    pub viewport_escalado: bool,
    /// Regla 4. Conversion float -> UNORM8 del color final.
    pub unorm8: fn(f32) -> u32,
}

impl Reglas {
    /// El juez: reglas de D3D10+ con la conversion a 8 bits exacta.
    pub const D3D10: Reglas = Reglas { centro_medio_pixel: true, subpixel_al_par: true, viewport_escalado: true, unorm8: a_unorm8 };
    /// Como D3D9: centros de pixel en coordenadas enteras.
    pub const D3D9: Reglas = Reglas { centro_medio_pixel: false, ..Reglas::D3D10 };
}

#[derive(Clone, Copy)]
struct Triangulo {
    x: [i64; 3],
    y: [i64; 3],
    z: [f32; 3],
    area: i64,
    /// Rectangulo de pixeles a revisar (inclusive).
    px: (i64, i64),
    py: (i64, i64),
    /// En un empate (centro del pixel justo sobre la arista) la arista cuenta? Regla top-left.
    incluye: [bool; 3],
    color: u32,
    /// Posicion del centro del pixel dentro del pixel, en subpixeles (128 = +0.5, 0 = entero).
    centro: i64,
}

fn arista(ax: i64, ay: i64, bx: i64, by: i64, px: i64, py: i64) -> i64 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

impl Triangulo {
    /// Pesos baricentricos sin normalizar si el centro del pixel (px, py) esta cubierto.
    fn cubre(&self, px: i64, py: i64) -> Option<[i64; 3]> {
        let (cx, cy) = (px * SUBPIXEL + self.centro, py * SUBPIXEL + self.centro);
        let (x, y) = (self.x, self.y);
        // e[k] = arista opuesta al vertice k
        let e = [
            arista(x[1], y[1], x[2], y[2], cx, cy),
            arista(x[2], y[2], x[0], y[0], cx, cy),
            arista(x[0], y[0], x[1], y[1], cx, cy),
        ];
        for k in 0..3 {
            if e[k] < 0 || (e[k] == 0 && !self.incluye[k]) {
                return None;
            }
        }
        Some(e)
    }
}

/// Arista a->b con el interior a la derecha (sentido horario, y hacia abajo):
/// "top" = horizontal hacia +x, "left" = sube (dy < 0).
fn es_top_left(ax: i64, ay: i64, bx: i64, by: i64) -> bool {
    let (dx, dy) = (bx - ax, by - ay);
    (dy == 0 && dx > 0) || dy < 0
}

/// Rasterizador de referencia: dibuja el cubo con el `angulo` dado en `destino`
/// (`ancho * alto` pixeles `0xAARRGGBB`, fila 0 arriba).
///
/// Hace lo mismo que el pipeline de la GPU en los cubos D3D:
/// 1. "Vertex shader": `wvp * pos`, division por w, viewport, ajuste a 1/256 de pixel.
/// 2. Culling: descarta triangulos con area <= 0 (antihorarios o degenerados).
/// 3. Cobertura en el centro de cada pixel con regla top-left (sin antialiasing).
/// 4. Profundidad interpolada, test `LESS` contra 1.0, en el orden de los indices.
/// 5. "Pixel shader": [`iluminar`] con la normal de la cara y [`a_unorm8`].
///
/// No usa buffer de profundidad: para cada pixel recorre los 12 triangulos y se queda
/// con el mas cercano, que es exactamente lo que deja un z-buffer con `LESS`.
pub fn dibujar_por_cpu(angulo: f32, ancho: u32, alto: u32, destino: &mut [u32]) {
    dibujar_por_cpu_con(angulo, ancho, alto, destino, &Reglas::D3D10)
}

/// Igual que [`dibujar_por_cpu`] con las [`Reglas`] que se indiquen: sirve para apagar
/// una regla y ver cuantos pixeles dejan de coincidir con la GPU.
pub fn dibujar_por_cpu_con(angulo: f32, ancho: u32, alto: u32, destino: &mut [u32], reglas: &Reglas) {
    let (w, h) = (ancho as usize, alto as usize);
    assert!(destino.len() >= w * h, "destino mas chico que ancho*alto");
    let c = constantes(angulo, ancho as f32 / alto as f32);
    let vs = vertices();
    let is = indices();

    // 1) Vertices a pantalla en punto fijo
    let mut sx = [0i64; NUM_VERTICES];
    let mut sy = [0i64; NUM_VERTICES];
    let mut sz = [0f32; NUM_VERTICES];
    let mut detras = [false; NUM_VERTICES];
    for (i, v) in vs.iter().enumerate() {
        let clip = transformar(&c.wvp, [v.pos[0], v.pos[1], v.pos[2], 1.0]);
        detras[i] = clip[3] <= 0.0;
        let inv_w = 1.0 / clip[3];
        let (nx, ny, nz) = (clip[0] * inv_w, clip[1] * inv_w, clip[2] * inv_w);
        // Viewport (regla 3). El orden importa: con x*256 ~ 2*10^5 un f32 solo tiene 1/64 de
        // subpixel, y `(nx*0.5 + 0.5)*ancho` redondea distinto que `nx*(ancho/2) + ancho/2`.
        let (x, y) = if reglas.viewport_escalado {
            let (medio_w, medio_h) = (ancho as f32 * 0.5, alto as f32 * 0.5);
            (nx * medio_w + medio_w, -ny * medio_h + medio_h)
        } else {
            ((nx * 0.5 + 0.5) * ancho as f32, (-ny * 0.5 + 0.5) * alto as f32)
        };
        // Ajuste a 1/256 de pixel (regla 2)
        let ajustar = if reglas.subpixel_al_par { redondear_par } else { redondear };
        sx[i] = ajustar(x * SUBPIXEL as f32);
        sy[i] = ajustar(y * SUBPIXEL as f32);
        sz[i] = nz;
    }

    // 2) Preparar triangulos (culling + rectangulo + color de la cara)
    let mut tris = [None::<Triangulo>; NUM_INDICES / 3];
    for (t, tri) in tris.iter_mut().enumerate() {
        let k = [is[t * 3] as usize, is[t * 3 + 1] as usize, is[t * 3 + 2] as usize];
        if k.iter().any(|&i| detras[i]) {
            continue; // el cubo nunca cruza el plano cercano; no hace falta recortar
        }
        let x = [sx[k[0]], sx[k[1]], sx[k[2]]];
        let y = [sy[k[0]], sy[k[1]], sy[k[2]]];
        let area = arista(x[0], y[0], x[1], y[1], x[2], y[2]);
        if area <= 0 {
            continue;
        }
        let (min_x, max_x) = (x[0].min(x[1]).min(x[2]), x[0].max(x[1]).max(x[2]));
        let (min_y, max_y) = (y[0].min(y[1]).min(y[2]), y[0].max(y[1]).max(y[2]));
        // Centro de pixel (regla 1): +0.5 en D3D10+, entero en D3D9
        let centro = if reglas.centro_medio_pixel { SUBPIXEL / 2 } else { 0 };
        let px = ((min_x - centro + SUBPIXEL - 1).div_euclid(SUBPIXEL).max(0), (max_x - centro).div_euclid(SUBPIXEL).min(w as i64 - 1));
        let py = ((min_y - centro + SUBPIXEL - 1).div_euclid(SUBPIXEL).max(0), (max_y - centro).div_euclid(SUBPIXEL).min(h as i64 - 1));
        let normal = transformar_dir(&c.world, vs[k[0]].normal);
        *tri = Some(Triangulo {
            x,
            y,
            z: [sz[k[0]], sz[k[1]], sz[k[2]]],
            area,
            px,
            py,
            incluye: [
                es_top_left(x[1], y[1], x[2], y[2]),
                es_top_left(x[2], y[2], x[0], y[0]),
                es_top_left(x[0], y[0], x[1], y[1]),
            ],
            color: empaquetar_con(iluminar(vs[k[0]].color, normal, c.luz), reglas.unorm8),
            centro,
        });
    }

    // 3-5) Cada pixel: el triangulo que lo cubre con menor profundidad
    for py in 0..h {
        let fila = &mut destino[py * w..py * w + w];
        for (px, pixel) in fila.iter_mut().enumerate() {
            let (pxi, pyi) = (px as i64, py as i64);
            let mut z_min = 1.0f32;
            let mut color = FONDO;
            for t in tris.iter().flatten() {
                if pxi < t.px.0 || pxi > t.px.1 || pyi < t.py.0 || pyi > t.py.1 {
                    continue;
                }
                if let Some(e) = t.cubre(pxi, pyi) {
                    let z = (e[0] as f32 * t.z[0] + e[1] as f32 * t.z[1] + e[2] as f32 * t.z[2]) / t.area as f32;
                    if z < z_min {
                        z_min = z;
                        color = t.color;
                    }
                }
            }
            *pixel = color;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::vec;

    const W: u32 = 320;
    const H: u32 = 180;

    fn imagen(fotograma: u32) -> std::vec::Vec<u32> {
        let mut b = vec![0u32; (W * H) as usize];
        dibujar_por_cpu(angulo_de_fotograma(fotograma), W, H, &mut b);
        b
    }

    /// Color esperado de una cara con normal `n` en el fotograma 0 (World = identidad),
    /// calculado con std, sin pasar por el crate.
    fn esperado(color: [f32; 3], n: [f32; 3]) -> u32 {
        let l = [0.4f32, 0.8, -0.5];
        let largo = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        let d = ((n[0] * l[0] + n[1] * l[1] + n[2] * l[2]) / largo).clamp(0.0, 1.0);
        let k = 0.25 + 0.75 * d;
        let u = |x: f32| ((x * k).clamp(0.0, 1.0) * 255.0).round() as u32;
        0xFF00_0000 | (u(color[0]) << 16) | (u(color[1]) << 8) | u(color[2])
    }

    #[test]
    fn malla() {
        let v = vertices();
        let i = indices();
        assert_eq!(core::mem::size_of::<Vertice>(), 40);
        assert_eq!(core::mem::size_of::<Constantes>(), 144);
        assert!(i.iter().all(|&k| (k as usize) < NUM_VERTICES));
        // cada vertice esta en una esquina del cubo y su normal apunta hacia fuera
        for x in v.iter() {
            assert!(x.pos.iter().all(|c| c.abs() == 1.0));
            assert_eq!(x.pos[0] * x.normal[0] + x.pos[1] * x.normal[1] + x.pos[2] * x.normal[2], 1.0);
        }
    }

    #[test]
    fn fotograma_0_centro_es_la_cara_verde() {
        // El rayo del centro de la pantalla toca la cara frontal (z = -1) en y = 0.3
        let b = imagen(0);
        let centro = b[(H / 2 * W + W / 2) as usize];
        assert_eq!(centro, esperado([0.19, 0.88, 0.19], [0.0, 0.0, -1.0]), "centro = {centro:08X}");
    }

    #[test]
    fn fotograma_0_arriba_se_ve_la_cara_magenta() {
        // La camara esta en y = 1.5, asi que la cara de arriba (y = +1) se ve.
        // Centro de esa cara, (0, 1, 0), proyectado a pantalla:
        let c = constantes(0.0, W as f32 / H as f32);
        let p = transformar(&c.wvp, [0.0, 1.0, 0.0, 1.0]);
        let x = ((p[0] / p[3] * 0.5 + 0.5) * W as f32) as u32;
        let y = ((-p[1] / p[3] * 0.5 + 0.5) * H as f32) as u32;
        let b = imagen(0);
        assert_eq!(b[(y * W + x) as usize], esperado([0.88, 0.19, 0.88], [0.0, 1.0, 0.0]));
    }

    #[test]
    fn esquinas_son_fondo_y_es_determinista() {
        let a = imagen(30);
        assert_eq!(a[0], FONDO);
        assert_eq!(a[(W * H - 1) as usize], FONDO);
        assert_eq!(a, imagen(30));
        assert_ne!(a, imagen(31));
    }

    #[test]
    fn regla_top_left_sin_huecos_ni_dobles() {
        // Un cuadrado de 4x4 pixeles partido en dos triangulos horarios:
        // cada centro de pixel lo cubre exactamente uno.
        let s = SUBPIXEL;
        let hacer = |x: [i64; 3], y: [i64; 3]| Triangulo {
            x,
            y,
            z: [0.0; 3],
            area: arista(x[0], y[0], x[1], y[1], x[2], y[2]),
            px: (0, 3),
            py: (0, 3),
            incluye: [
                es_top_left(x[1], y[1], x[2], y[2]),
                es_top_left(x[2], y[2], x[0], y[0]),
                es_top_left(x[0], y[0], x[1], y[1]),
            ],
            color: 0,
            centro: SUBPIXEL / 2,
        };
        // Diagonal que pasa justo por centros de pixel: (0.5,0.5) -> (3.5,3.5), extendida
        let a = hacer([0, 4 * s, 4 * s], [0, 0, 4 * s]);
        let b = hacer([0, 4 * s, 0], [0, 4 * s, 4 * s]);
        assert!(a.area > 0 && b.area > 0);
        for py in 0..4 {
            for px in 0..4 {
                let n = a.cubre(px, py).is_some() as u32 + b.cubre(px, py).is_some() as u32;
                assert_eq!(n, 1, "pixel ({px},{py}) cubierto {n} veces");
            }
        }
    }
}
