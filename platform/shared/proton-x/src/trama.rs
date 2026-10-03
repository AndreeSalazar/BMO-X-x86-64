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
//!    +  la PROFUNDIDAD (P3c4): z / w al rango del viewport, LINEAL en
//!       pantalla (como D3D), la prueba antes del sombreador de pixeles, y
//!       escrita si pasa y el PSO lo pide
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
    /// La prueba de profundidad del PSO (`DepthEnable`), o `None`.
    pub profundidad: Option<Profundidad>,
}

/// La prueba de profundidad: `D3D12_COMPARISON_FUNC` (1 nunca, 2 menor,
/// 3 igual, 4 menor o igual, 5 mayor, 6 distinto, 7 mayor o igual, 8
/// siempre) y si se escribe (`DepthWriteMask` ALL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profundidad {
    pub funcion: u32,
    pub escribir: bool,
}

impl Profundidad {
    /// Si `z` (el del pixel) pasa contra `guardado` (el del bufer).
    pub fn pasa(&self, z: f32, guardado: f32) -> bool {
        match self.funcion {
            1 => false,
            2 => z < guardado,
            3 => z == guardado,
            4 => z <= guardado,
            5 => z > guardado,
            6 => z != guardado,
            7 => z >= guardado,
            _ => true,
        }
    }
}

/// Donde se pinta: `ancho * alto` pixeles de 32 bits, fila 0 arriba, en el
/// orden de bytes de su formato.
pub struct Destino<'a, 'o> {
    pub pixeles: &'a mut [u32],
    pub ancho: u32,
    pub alto: u32,
    /// `B8G8R8A8` (si no, `R8G8B8A8`).
    pub bgra: bool,
    /// El bufer de profundidad (`D32_FLOAT`: los bits de cada `f32`, como
    /// estan en memoria), del mismo `ancho * alto`, o `None`: sin el, la
    /// prueba no se hace (como en D3D sin DSV).
    pub z: Option<&'a mut [u32]>,
    /// P3b4c.9 Z1: es un back buffer de la cadena de intercambio (lo que
    /// muestra `Present`, y nada mas lo lee): el ejecutor puede ponerlo
    /// directo en la pantalla y NO en `pixeles` (lo dice `Cuenta::en_pantalla`).
    pub cadena: bool,
    /// N5.8 (03-10): los render targets 1..8 (el G-buffer): `otros[k]` es
    /// el SV_Target `k + 1`, del mismo `ancho * alto`. `pixeles` es el 0.
    pub otros: &'a mut [Otro<'o>],
}

/// **Otro render target** del mismo dibujo (N5.8): sus pixeles y su orden
/// de bytes. `None` es una ranura sin vista: lo que se escribe ahi se
/// pierde, como en D3D con un RTV nulo.
pub struct Otro<'a> {
    pub pixeles: Option<&'a mut [u32]>,
    pub bgra: bool,
}

/// Cuantos render targets puede escribir un dibujo (D3D12: 8).
pub const OBJETIVOS: usize = 8;

/// Lo que paso.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cuenta {
    pub dibujados: u32,
    pub descartados: u32,
    pub sin_recortar: u32,
    pub pixeles: u64,
    /// Cuantas veces corrio de verdad el sombreador de pixeles.
    pub sombreados: u64,
    /// Pixeles cubiertos que la prueba de profundidad dejo sin pintar.
    pub tapados: u64,
    /// N5.7: pixeles que el sombreador TIRO (`discard`, `clip`): pasaron la
    /// prueba de profundidad y no escribieron nada.
    pub tirados: u64,
    /// P3b4c.9 Z1: el dibujo quedo EN LA PANTALLA (la 3060, directo), no en
    /// `Destino::pixeles`: `Present` no tiene nada que copiar.
    pub en_pantalla: bool,
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
/// con `ps` como sombreador de pixeles: recibe los atributos interpolados,
/// pone el color (r, g, b, a) de cada render target en su SV_Target (N5.8:
/// el 0 en `pixeles`, los demas en `otros`) y dice si el pixel queda --
/// `false` lo TIRA (N5.7: ni color ni profundidad; por eso la Z se escribe
/// DESPUES de correrlo).
///
/// `posicion` (N5.9): el atributo que es SV_Position, si el sombreador lo
/// lee: en cada pixel, (x + 0.5, y + 0.5, z, w) -- el centro en pantalla,
/// la z del viewport y la w de recorte (la de D3D: w, no 1/w como en GL).
pub fn dibujar(reglas: &Reglas, vertices: &[Sombreado], tris: &[[usize; 3]], destino: &mut Destino, posicion: Option<usize>, mut ps: impl FnMut(&[[f32; 4]], &mut [[f32; 4]; OBJETIVOS]) -> bool) -> Cuenta {
    let mut cuenta = Cuenta::default();
    let [vx, vy, vw, vh, zmin, zmax] = reglas.viewport;
    let prueba = reglas.profundidad.filter(|_| destino.z.as_ref().is_some_and(|z| z.len() >= destino.pixeles.len()));
    let (mw, mh) = (vw * 0.5, vh * 0.5);
    let (ox, oy) = (vx + mw, vy + mh);
    // El rectangulo donde se puede pintar: viewport, tijera y destino.
    let x0 = (vx.max(0.0) as i64).max(reglas.tijera[0] as i64).max(0);
    let y0 = (vy.max(0.0) as i64).max(reglas.tijera[1] as i64).max(0);
    let x1 = ((vx + vw) as i64).min(reglas.tijera[2] as i64).min(destino.ancho as i64) - 1;
    let y1 = ((vy + vh) as i64).min(reglas.tijera[3] as i64).min(destino.alto as i64) - 1;
    let ancho = destino.ancho as i64;
    // La memoria del sombreador de pixeles: lo ultimo que entro y lo que dio.
    let mut ultima: Option<(Vec<[f32; 4]>, Option<[u32; OBJETIVOS]>)> = None;
    // Cuantos render targets se pintan, y el orden de bytes de cada uno.
    let n_rt = (1 + destino.otros.len()).min(OBJETIVOS);
    let mut bgra = [false; OBJETIVOS];
    bgra[0] = destino.bgra;
    for (k, o) in destino.otros.iter().take(OBJETIVOS - 1).enumerate() {
        bgra[k + 1] = o.bgra;
    }
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
        // La profundidad de cada vertice, ya en el rango del viewport.
        let zv: [f32; 3] = core::array::from_fn(|k| zmin + v[k].pos[2] * inv_w[k] * (zmax - zmin));
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
                let i = (py * ancho + px) as usize;
                let mut z_nueva = None;
                if let (Some(p), Some(zs)) = (prueba, destino.z.as_deref_mut()) {
                    // Lineal en pantalla: los pesos de las aristas, sin w.
                    let s = (e[0] + e[1] + e[2]) as f32;
                    let (b1, b2) = (e[1] as f32 / s, e[2] as f32 / s);
                    let z = (zv[0] + b1 * (zv[1] - zv[0]) + b2 * (zv[2] - zv[0])).clamp(zmin.min(zmax), zmin.max(zmax));
                    if !p.pasa(z, f32::from_bits(zs[i])) {
                        cuenta.tapados += 1;
                        continue;
                    }
                    if p.escribir {
                        z_nueva = Some(z.to_bits());
                    }
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
                if let Some(a) = posicion.filter(|&a| a < entrada.len()) {
                    let s = (e[0] + e[1] + e[2]) as f32;
                    let (b1, b2) = (e[1] as f32 / s, e[2] as f32 / s);
                    let z = zv[0] + b1 * (zv[1] - zv[0]) + b2 * (zv[2] - zv[0]);
                    let w = 1.0 / ((1.0 - b1 - b2) * inv_w[0] + b1 * inv_w[1] + b2 * inv_w[2]);
                    entrada[a] = [px as f32 + 0.5, py as f32 + 0.5, z, w];
                }
                let pixel = match &ultima {
                    Some((antes, p)) if antes.len() == entrada.len() && antes.iter().zip(&entrada).all(|(a, b)| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())) => *p,
                    _ => {
                        cuenta.sombreados += 1;
                        let mut colores = [[0.0f32; 4]; OBJETIVOS];
                        let p = ps(&entrada, &mut colores).then(|| core::array::from_fn(|k| if k < n_rt { empaquetar(colores[k], bgra[k]) } else { 0 }));
                        ultima = Some((entrada.clone(), p));
                        p
                    }
                };
                let Some(pixel) = pixel else {
                    cuenta.tirados += 1;
                    continue;
                };
                if let (Some(z), Some(zs)) = (z_nueva, destino.z.as_deref_mut()) {
                    zs[i] = z;
                }
                destino.pixeles[i] = pixel[0];
                for (k, o) in destino.otros.iter_mut().take(n_rt - 1).enumerate() {
                    if let Some(p) = o.pixeles.as_deref_mut().and_then(|p| p.get_mut(i)) {
                        *p = pixel[k + 1];
                    }
                }
            }
        }
    }
    cuenta
}
