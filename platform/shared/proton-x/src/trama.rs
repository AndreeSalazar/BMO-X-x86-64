//! **La trama: triangulos a pixeles, con las reglas de D3D** (P3b3, 27-09).
//!
//! Lo que en Windows hace el hardware entre el sombreador de vertices y el de
//! pixeles, en la CPU y sin un `unsafe`. Las reglas son las cuatro que el
//! estudio D3D hallo MIDIENDO la 3060 (`bmo_cubo`, el juez), mas lo que el
//! juez no necesitaba y un programa D3D12 si pide:
//!
//! ```text
//!    1  x / w, y / w; viewport  nx * (ancho/2) + (x0 + ancho/2)
//!                               -ny * (alto/2) + (y0 + alto/2)
//!    2  ajuste a 1/256 de pixel, empates al PAR
//!    3  cobertura en el CENTRO del pixel (x + 0.5), regla top-left
//!    4  el color a 8 bits: saturar, por 255, redondear
//!    +  descarte (ninguno, delante, detras) y que cara es la de delante
//!    +  el rectangulo: viewport, tijera y destino
//!    +  los atributos del sombreador, interpolados CON PERSPECTIVA
//! ```
//!
//! **Un atributo igual en los tres vertices es ESE valor**, sin cuentas: la
//! interpolacion `a0 + b1 (a1 - a0) + b2 (a2 - a0)` lo da exacto, y un pixel
//! de una cara plana ve lo mismo que su vertice. Y como el sombreador de
//! pixeles solo depende de lo que entra (y del cbuffer), si entra lo mismo que
//! en el pixel anterior se reusa su color: es la misma cuenta, no un atajo.
//!
//! Lo que falta se CUENTA ([`Cuenta::sin_recortar`]): un triangulo que cruza
//! el plano cercano (w <= 0) o sale de la profundidad (z < 0, z > w) no se
//! recorta todavia; se deja entero sin pintar y se dice.

use alloc::vec::Vec;

/// Subpixeles por pixel (D3D10+).
const SUBPIXEL: i64 = 256;

/// Un vertice ya sombreado: su posicion de RECORTE y lo que le pasa al
/// sombreador de pixeles, ya en el orden de SU firma de entrada.
#[derive(Debug, Clone, PartialEq)]
pub struct Sombreado {
    pub pos: [f32; 4],
    pub atributos: Vec<[f32; 4]>,
}

/// Como se dibuja: el viewport de D3D12 (x, y, ancho, alto, zmin, zmax), la
/// tijera (izquierda, arriba, derecha, abajo) y el descarte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reglas {
    pub viewport: [f32; 6],
    pub tijera: [i32; 4],
    /// D3D12_CULL_MODE: 1 ninguna, 2 las de delante, 3 las de detras.
    pub descarte: u32,
    /// `FrontCounterClockwise`: la cara de delante es la antihoraria.
    pub antihorario: bool,
}

/// Donde se pinta: `ancho * alto` pixeles de 32 bits, fila 0 arriba, en el
/// orden de bytes de su formato.
pub struct Destino<'a> {
    pub pixeles: &'a mut [u32],
    pub ancho: u32,
    pub alto: u32,
    /// `B8G8R8A8` (si no, `R8G8B8A8`).
    pub bgra: bool,
}

/// Lo que paso.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cuenta {
    pub dibujados: u32,
    pub descartados: u32,
    pub sin_recortar: u32,
    pub pixeles: u64,
    /// Cuantas veces corrio de verdad el sombreador de pixeles.
    pub sombreados: u64,
}

/// Redondeo con empates al PAR (el de `bmo_cubo::num::redondear_par`).
pub fn redondear_par(x: f32) -> i64 {
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

/// Arista a->b con el interior a la derecha: "top" es horizontal hacia +x,
/// "left" sube.
fn top_left(ax: i64, ay: i64, bx: i64, by: i64) -> bool {
    let (dx, dy) = (bx - ax, by - ay);
    (dy == 0 && dx > 0) || dy < 0
}

/// Un canal a UNORM8: saturar (NaN es 0), por 255 y redondear.
pub fn unorm8(x: f32) -> u32 {
    let s = if x > 0.0 {
        if x < 1.0 {
            x
        } else {
            1.0
        }
    } else {
        0.0
    };
    (s * 255.0 + 0.5) as u32
}

/// Un color en el orden de bytes del destino.
pub fn empaquetar(c: [f32; 4], bgra: bool) -> u32 {
    let [r, g, b, a] = [unorm8(c[0]), unorm8(c[1]), unorm8(c[2]), unorm8(c[3])];
    if bgra {
        a << 24 | r << 16 | g << 8 | b
    } else {
        a << 24 | b << 16 | g << 8 | r
    }
}

/// **Dibujar triangulos** (cada tres indices de `tris`, uno) sobre `destino`,
/// con `ps` como sombreador de pixeles: recibe los atributos interpolados y
/// devuelve el color (r, g, b, a).
pub fn dibujar(reglas: &Reglas, vertices: &[Sombreado], tris: &[[usize; 3]], destino: &mut Destino, mut ps: impl FnMut(&[[f32; 4]]) -> [f32; 4]) -> Cuenta {
    let mut cuenta = Cuenta::default();
    let [vx, vy, vw, vh, _, _] = reglas.viewport;
    let (mw, mh) = (vw * 0.5, vh * 0.5);
    let (ox, oy) = (vx + mw, vy + mh);
    // El rectangulo donde se puede pintar: viewport, tijera y destino.
    let x0 = (vx.max(0.0) as i64).max(reglas.tijera[0] as i64).max(0);
    let y0 = (vy.max(0.0) as i64).max(reglas.tijera[1] as i64).max(0);
    let x1 = ((vx + vw) as i64).min(reglas.tijera[2] as i64).min(destino.ancho as i64) - 1;
    let y1 = ((vy + vh) as i64).min(reglas.tijera[3] as i64).min(destino.alto as i64) - 1;
    let ancho = destino.ancho as i64;
    // La memoria del sombreador de pixeles: lo ultimo que entro y lo que dio.
    let mut ultima: Option<(Vec<[f32; 4]>, u32)> = None;
    let mut entrada: Vec<[f32; 4]> = Vec::new();
    for t in tris {
        let Some(v) = t.iter().map(|&i| vertices.get(i)).collect::<Option<Vec<_>>>() else {
            cuenta.descartados += 1;
            continue;
        };
        if v.iter().any(|v| {
            let [x, y, z, w] = v.pos;
            !(w > 0.0 && x.is_finite() && y.is_finite() && z >= 0.0 && z <= w && w.is_finite())
        }) {
            cuenta.sin_recortar += 1;
            continue;
        }
        // 1-2: a pantalla y al subpixel.
        let mut x = [0i64; 3];
        let mut y = [0i64; 3];
        let mut inv_w = [0f32; 3];
        for k in 0..3 {
            let p = v[k].pos;
            inv_w[k] = 1.0 / p[3];
            let (nx, ny) = (p[0] * inv_w[k], p[1] * inv_w[k]);
            x[k] = redondear_par((nx * mw + ox) * SUBPIXEL as f32);
            y[k] = redondear_par((-ny * mh + oy) * SUBPIXEL as f32);
        }
        let area = arista(x[0], y[0], x[1], y[1], x[2], y[2]);
        // En pantalla (y hacia abajo), area > 0 es sentido HORARIO.
        let delante = if reglas.antihorario { area < 0 } else { area > 0 };
        let fuera = match reglas.descarte {
            2 => delante,
            3 => !delante,
            _ => false,
        };
        if fuera || area == 0 {
            cuenta.descartados += 1;
            continue;
        }
        // Con el area positiva, los tres en el orden en que la cobertura mira.
        let mut o = [0usize, 1, 2];
        if area < 0 {
            o = [0, 2, 1];
            x.swap(1, 2);
            y.swap(1, 2);
            inv_w.swap(1, 2);
        }
        let v = [v[o[0]], v[o[1]], v[o[2]]];
        cuenta.dibujados += 1;
        let n = v[0].atributos.len().min(v[1].atributos.len()).min(v[2].atributos.len());
        // Que componentes cambian dentro del triangulo.
        let constante: Vec<[bool; 4]> = (0..n)
            .map(|a| core::array::from_fn(|c| v[0].atributos[a][c].to_bits() == v[1].atributos[a][c].to_bits() && v[0].atributos[a][c].to_bits() == v[2].atributos[a][c].to_bits()))
            .collect();
        let plano = constante.iter().all(|c| c.iter().all(|&b| b));
        entrada.clear();
        entrada.extend(v[0].atributos[..n].iter().copied());
        let incluye = [top_left(x[1], y[1], x[2], y[2]), top_left(x[2], y[2], x[0], y[0]), top_left(x[0], y[0], x[1], y[1])];
        let c = SUBPIXEL / 2;
        let (min_x, max_x) = (x[0].min(x[1]).min(x[2]), x[0].max(x[1]).max(x[2]));
        let (min_y, max_y) = (y[0].min(y[1]).min(y[2]), y[0].max(y[1]).max(y[2]));
        let px0 = (min_x - c + SUBPIXEL - 1).div_euclid(SUBPIXEL).max(x0);
        let px1 = (max_x - c).div_euclid(SUBPIXEL).min(x1);
        let py0 = (min_y - c + SUBPIXEL - 1).div_euclid(SUBPIXEL).max(y0);
        let py1 = (max_y - c).div_euclid(SUBPIXEL).min(y1);
        for py in py0..=py1 {
            for px in px0..=px1 {
                let (cx, cy) = (px * SUBPIXEL + c, py * SUBPIXEL + c);
                let e = [
                    arista(x[1], y[1], x[2], y[2], cx, cy),
                    arista(x[2], y[2], x[0], y[0], cx, cy),
                    arista(x[0], y[0], x[1], y[1], cx, cy),
                ];
                if !(0..3).all(|k| e[k] > 0 || (e[k] == 0 && incluye[k])) {
                    continue;
                }
                cuenta.pixeles += 1;
                if !plano {
                    // Con perspectiva: los pesos de pantalla sobre w.
                    let b = [e[0] as f32 * inv_w[0], e[1] as f32 * inv_w[1], e[2] as f32 * inv_w[2]];
                    let s = b[0] + b[1] + b[2];
                    let (b1, b2) = (b[1] / s, b[2] / s);
                    for (a, fija) in constante.iter().enumerate() {
                        for k in 0..4 {
                            if !fija[k] {
                                let a0 = v[0].atributos[a][k];
                                entrada[a][k] = a0 + b1 * (v[1].atributos[a][k] - a0) + b2 * (v[2].atributos[a][k] - a0);
                            }
                        }
                    }
                }
                let pixel = match &ultima {
                    Some((antes, p)) if antes.len() == entrada.len() && antes.iter().zip(&entrada).all(|(a, b)| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())) => *p,
                    _ => {
                        cuenta.sombreados += 1;
                        let p = empaquetar(ps(&entrada), destino.bgra);
                        ultima = Some((entrada.clone(), p));
                        p
                    }
                };
                destino.pixeles[(py * ancho + px) as usize] = pixel;
            }
        }
    }
    cuenta
}
