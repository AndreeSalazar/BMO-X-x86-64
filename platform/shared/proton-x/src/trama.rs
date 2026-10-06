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
//!    +  el STENCIL (05-10, `stencil.rs`): su prueba con la de profundidad,
//!       la cara por el giro, y la operacion que toca a cada resultado
//! ```
//!
//! **Un atributo igual en los tres vertices es ESE valor**, sin cuentas: la
//! interpolacion `a0 + b1 (a1 - a0) + b2 (a2 - a0)` lo da exacto, y un pixel
//! de una cara plana ve lo mismo que su vertice. Y como el sombreador de
//! pixeles solo depende de lo que entra (y del cbuffer), si entra lo mismo que
//! en el pixel anterior se reusa su color: es la misma cuenta, no un atajo.
//!
//! **El recorte** (05-10): un triangulo que cruza el plano cercano (z < 0)
//! o el lejano (z > w) se RECORTA contra ellos, como en D3D, y lo que queda
//! (hasta cinco lados, en abanico) se pinta en su sitio; antes se dejaba
//! entero sin pintar (en 3D, de cerca, faltaba suelo). Los atributos, en
//! linea recta en el espacio de recorte: la perspectiva sale igual. Contra
//! x e y no hace falta (la tijera corta), salvo lo que pasa de una BANDA de
//! guarda de 64 veces la pantalla, para que las cuentas enteras no se
//! desborden. N5.16b (05-10): con `DepthClipEnable = FALSE` (el bit
//! [`SIN_RECORTE_Z`] del descarte) NO se recorta contra esos dos planos: la
//! Z se SUJETA al rango del viewport antes de la prueba, como en D3D12.

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
    /// D3D12_CULL_MODE: 1 ninguna, 2 las de delante, 3 las de detras; con
    /// el bit [`SIN_RECORTE_Z`] si el PSO apaga el recorte en z.
    pub descarte: u32,
    /// `FrontCounterClockwise`: la cara de delante es la antihoraria.
    pub antihorario: bool,
    /// La prueba de profundidad del PSO (`DepthEnable`), o `None`.
    pub profundidad: Option<Profundidad>,
    /// N5.11: la mezcla de cada render target y el factor de mezcla.
    pub mezcla: crate::mezcla::Mezclas,
    /// 03-10: el de pixeles escribe SV_Depth: la prueba de profundidad va
    /// DESPUES de el, con la suya (en `colores[PROFUNDIDAD][0]`).
    pub z_del_sombreador: bool,
    /// 05-10: el stencil del PSO (`StencilEnable`) con las referencias de
    /// la lista, o `None`. Sin `Destino::stencil` no se hace (D3D: sin
    /// plano de stencil la prueba pasa y no se escribe nada).
    pub stencil: Option<crate::stencil::Stencil>,
}

/// **`DepthClipEnable = FALSE`** (N5.16b, 05-10), un bit de
/// [`Reglas::descarte`]: sin recorte contra el plano cercano ni el lejano, y
/// la Z sujeta a `[zmin, zmax]` del viewport antes de la prueba (D3D12). Va
/// en el descarte y no en un campo nuevo para que quien mire el modo de
/// descarte entero (la puerta de la 3060, que recorta siempre) lo vea y
/// diga que no, en vez de recortar callado.
pub const SIN_RECORTE_Z: u32 = 0x100;

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
/// orden de bytes de su formato. N5.12: `pixeles` VACIO es un dibujo de solo
/// profundidad (las sombras): `ancho` y `alto` son los de la Z.
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
    /// N5.16 (05-10): el render target 0 es de FLOAT: `pixeles` lleva cuatro
    /// palabras por texel (los bits de r, g, b y a en f32), y cada color se
    /// mezcla en float y se cuantiza a ESTE formato (DXGI) al escribirlo,
    /// para que valga lo que en la GPU (`formato_ia::cuantizar`). N5.16b:
    /// si `pixeles` mide UNA palabra por texel (un R32_FLOAT, como lo guarda
    /// la casa), solo el r; lo que no trae se lee (0, 0, 1), como su formato.
    pub flotante: Option<u32>,
    /// 05-10: el plano de stencil del DSV (un byte por texel, del mismo
    /// `ancho * alto`), o `None`: un D32 sin stencil, o sin DSV.
    pub stencil: Option<&'a mut [u8]>,
}

/// **Otro render target** del mismo dibujo (N5.8): sus pixeles y su orden
/// de bytes. `None` es una ranura sin vista: lo que se escribe ahi se
/// pierde, como en D3D con un RTV nulo.
pub struct Otro<'a> {
    pub pixeles: Option<&'a mut [u32]>,
    pub bgra: bool,
    /// N5.16: como [`Destino::flotante`], el de este.
    pub flotante: Option<u32>,
}

/// Cuantos render targets puede escribir un dibujo (D3D12: 8).
pub const OBJETIVOS: usize = 8;
/// Lo que el de pixeles le da a la trama: un color por render target y,
/// detras, su SV_Depth (en el canal 0 de `colores[PROFUNDIDAD]`) y (05-10)
/// su SV_StencilRef (los bits del canal 0 de `colores[REFERENCIA]`).
pub const SALIDAS: usize = OBJETIVOS + 2;
pub const PROFUNDIDAD: usize = OBJETIVOS;
pub const REFERENCIA: usize = OBJETIVOS + 1;

/// Lo que paso.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cuenta {
    pub dibujados: u32,
    pub descartados: u32,
    /// 05-10: triangulos que hubo que RECORTAR (cruzaban un plano).
    pub recortados: u32,
    pub pixeles: u64,
    /// Cuantas veces corrio de verdad el sombreador de pixeles.
    pub sombreados: u64,
    /// Pixeles cubiertos que la prueba de profundidad dejo sin pintar.
    pub tapados: u64,
    /// N5.7: pixeles que el sombreador TIRO (`discard`, `clip`): pasaron la
    /// prueba de profundidad y no escribieron nada.
    pub tirados: u64,
    /// E2.7 (05-10): pixeles que PASARON la prueba de profundidad (y la de
    /// stencil, desde el 05-10) y que el sombreador no tiro, escriban
    /// color o no: lo que cuenta una consulta de OCLUSION de D3D12.
    pub pasan: u64,
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

/// La banda de guarda: x e y, dentro de +-64 w (64 veces la pantalla).
const BANDA: f32 = 64.0;

/// La distancia firmada de `p` a cada plano de recorte (dentro: >= 0): el
/// cercano, el lejano y los cuatro de la banda.
fn planos(p: &[f32; 4]) -> [f32; 6] {
    let [x, y, z, w] = *p;
    [z, w - z, BANDA * w - x, BANDA * w + x, BANDA * w - y, BANDA * w + y]
}

/// El punto de `a` a `b` en `t`: la posicion y cada atributo, en linea recta.
fn entre(a: &Sombreado, b: &Sombreado, t: f32) -> Sombreado {
    let l = |x: f32, y: f32| x + t * (y - x);
    Sombreado {
        pos: core::array::from_fn(|k| l(a.pos[k], b.pos[k])),
        atributos: a.atributos.iter().zip(&b.atributos).map(|(x, y)| core::array::from_fn(|k| l(x[k], y[k]))).collect(),
    }
}

/// **Recortar un triangulo** (Sutherland-Hodgman): el poligono que queda
/// dentro de todos los planos desde `desde` (0; 2 sin el cercano ni el
/// lejano, N5.16b), en el orden de sus vertices (vacio si nada).
fn recortar(v: [&Sombreado; 3], desde: usize) -> Vec<Sombreado> {
    let mut poli: Vec<Sombreado> = v.iter().map(|&s| s.clone()).collect();
    for plano in desde..6 {
        if poli.len() < 3 {
            return Vec::new();
        }
        let mut sale = Vec::with_capacity(poli.len() + 1);
        for i in 0..poli.len() {
            let (a, b) = (&poli[i], &poli[(i + 1) % poli.len()]);
            let (da, db) = (planos(&a.pos)[plano], planos(&b.pos)[plano]);
            if da >= 0.0 {
                sale.push(a.clone());
            }
            if (da >= 0.0) != (db >= 0.0) {
                sale.push(entre(a, b, da / (da - db)));
            }
        }
        poli = sale;
    }
    poli
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

/// Lo contrario de [`empaquetar`]: los cuatro canales de un pixel guardado
/// (N5.11: la mezcla lee el que esta).
pub fn desempaquetar(p: u32, bgra: bool) -> [f32; 4] {
    let b = |k: u32| ((p >> (8 * k)) & 0xFF) as f32 / 255.0;
    if bgra {
        [b(2), b(1), b(0), b(3)]
    } else {
        [b(0), b(1), b(2), b(3)]
    }
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

/// **Un triangulo ya en pantalla** (E2.5, 05-10, sacado del bucle de
/// [`dibujar`]): lo que hace falta para cubrir, probar y sombrear un pixel
/// suyo. `crate::cuadros` lo usa igual para los pixeles de sus cuadros
/// (los ayudantes, FUERA del triangulo: los mismos pesos, extrapolados).
pub struct Tri<'v> {
    /// Los vertices en subpixeles (con el area positiva), 1/w y su z.
    x: [i64; 3],
    y: [i64; 3],
    inv_w: [f32; 3],
    zv: [f32; 3],
    v: [&'v Sombreado; 3],
    constante: Vec<[bool; 4]>,
    plano: bool,
    incluye: [bool; 3],
    zmin: f32,
    zmax: f32,
}

impl Tri<'_> {
    /// Las tres aristas en el centro del pixel (dentro: todas >= 0).
    pub fn aristas(&self, px: i64, py: i64) -> [i64; 3] {
        let (x, y) = (self.x, self.y);
        let (cx, cy) = (px * SUBPIXEL + SUBPIXEL / 2, py * SUBPIXEL + SUBPIXEL / 2);
        [arista(x[1], y[1], x[2], y[2], cx, cy), arista(x[2], y[2], x[0], y[0], cx, cy), arista(x[0], y[0], x[1], y[1], cx, cy)]
    }

    /// Si el pixel es del triangulo (la regla top-left en los bordes).
    pub fn cubre(&self, e: &[i64; 3]) -> bool {
        (0..3).all(|k| e[k] > 0 || (e[k] == 0 && self.incluye[k]))
    }

    /// La z del pixel para la prueba: lineal en pantalla (los pesos de las
    /// aristas, sin w), al rango del viewport.
    pub fn z(&self, e: &[i64; 3]) -> f32 {
        let (zv, zmin, zmax) = (self.zv, self.zmin, self.zmax);
        let s = (e[0] + e[1] + e[2]) as f32;
        let (b1, b2) = (e[1] as f32 / s, e[2] as f32 / s);
        (zv[0] + b1 * (zv[1] - zv[0]) + b2 * (zv[2] - zv[0])).clamp(zmin.min(zmax), zmin.max(zmax))
    }

    /// **Lo que entra al sombreador** en el pixel: los atributos que
    /// cambian, interpolados CON perspectiva, sobre `entrada` (que empieza
    /// con los del vertice 0, [`Tri::de_partida`]: los que no cambian ya
    /// estan), y SV_Position.
    pub fn entrada(&self, e: &[i64; 3], px: i64, py: i64, posicion: Option<usize>, entrada: &mut [[f32; 4]]) {
        let (v, inv_w, zv) = (self.v, self.inv_w, self.zv);
        if !self.plano {
            // Con perspectiva: los pesos de pantalla sobre w.
            let b = [e[0] as f32 * inv_w[0], e[1] as f32 * inv_w[1], e[2] as f32 * inv_w[2]];
            let s = b[0] + b[1] + b[2];
            let (b1, b2) = (b[1] / s, b[2] / s);
            for (a, fija) in self.constante.iter().enumerate() {
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
    }

    /// Las entradas del vertice 0 (de las que parte [`Tri::entrada`]).
    pub fn de_partida(&self) -> &[[f32; 4]] {
        &self.v[0].atributos[..self.constante.len()]
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
pub fn dibujar(reglas: &Reglas, vertices: &[Sombreado], tris: &[[usize; 3]], destino: &mut Destino, posicion: Option<usize>, ps: impl FnMut(&[[f32; 4]], &mut [[f32; 4]; SALIDAS]) -> bool) -> Cuenta {
    dibujar_con(reglas, Efectos::default(), vertices, tris, destino, posicion, ps)
}

/// **Lo que un sombreador de pixeles hace ADEMAS de su color** (05-10): lo
/// que cambia el orden de la trama.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Efectos {
    /// Lee o escribe UAV: corre en CADA pixel cubierto, uno a uno y en el
    /// orden de la trama (sin la memoria del ultimo, que se saltaria
    /// escrituras), y la profundidad se prueba DESPUES de el (lo de D3D sin
    /// `[earlydepthstencil]`: un pixel tapado tambien escribe sus UAV).
    pub uav: bool,
    /// `[earlydepthstencil]`: la profundidad se prueba y se ESCRIBE antes
    /// de correrlo (un `discard` ya no la deshace), con UAV o sin ellos.
    pub temprana: bool,
    /// 05-10: escribe SV_StencilRef: la referencia de stencil es la SUYA
    /// (`colores[REFERENCIA]`, sus 8 bits bajos), asi que el stencil (y con
    /// el la profundidad) se prueba DESPUES de el, como con UAV.
    pub referencia: bool,
}

/// [`dibujar`] con los [`Efectos`] de su sombreador de pixeles.
#[allow(clippy::too_many_arguments)]
pub fn dibujar_con(reglas: &Reglas, efectos: Efectos, vertices: &[Sombreado], tris: &[[usize; 3]], destino: &mut Destino, posicion: Option<usize>, ps: impl FnMut(&[[f32; 4]], &mut [[f32; 4]; SALIDAS]) -> bool) -> Cuenta {
    dibujar_todo(reglas, efectos, vertices, tris, destino, posicion, ps, None)
}

/// E2.5 (05-10): [`dibujar_con`] con un sombreador de pixeles que usa las
/// OLAS: los pixeles de cada triangulo que llegan al sombreador (con las
/// mismas pruebas de antes, y sus escrituras tempranas) se sombrean juntos,
/// en cuadros de 2x2 con sus AYUDANTES y en olas (`crate::cuadros`), y
/// luego cada uno se pone como siempre ([`poner_pixel`]: las pruebas de
/// despues, el stencil, la profundidad, la cuenta y la mezcla).
#[allow(clippy::too_many_arguments)]
pub fn dibujar_en_olas(reglas: &Reglas, efectos: Efectos, vertices: &[Sombreado], tris: &[[usize; 3]], destino: &mut Destino, posicion: Option<usize>, olas: crate::cuadros::Olas) -> Cuenta {
    dibujar_todo(reglas, efectos, vertices, tris, destino, posicion, |_, _| false, Some(olas))
}

/// Lo que dio el de pixeles: sus colores, y ya empaquetados (`None`: lo tiro).
type Salida = Option<([[f32; 4]; SALIDAS], [u32; OBJETIVOS])>;

/// **Lo que el bucle sabe de un pixel al llegar al de pixeles** (E2.5): su
/// indice, la operacion de stencil si no paso (`fallo`), y su profundidad
/// para escribir o para probar despues (`tarde`).
struct Pendiente {
    i: usize,
    fallo: Option<u8>,
    z_nueva: Option<u32>,
    z_tarde: Option<f32>,
}

/// Lo de todo el dibujo que hace falta para poner un pixel.
struct Comun<'r> {
    reglas: &'r Reglas,
    temprana: bool,
    /// El stencil se prueba despues del de pixeles (`tarde` y, 05-10, con
    /// SV_StencilRef tambien con SV_Depth), con la referencia que diga el.
    tarde: bool,
    prueba: Option<Profundidad>,
    texeles: usize,
    n_rt: usize,
    bgra: [bool; OBJETIVOS],
    referencia: bool,
}

#[allow(clippy::too_many_arguments)]
fn dibujar_todo(reglas: &Reglas, efectos: Efectos, vertices: &[Sombreado], tris: &[[usize; 3]], destino: &mut Destino, posicion: Option<usize>, mut ps: impl FnMut(&[[f32; 4]], &mut [[f32; 4]; SALIDAS]) -> bool, mut olas: Option<crate::cuadros::Olas>) -> Cuenta {
    let mut cuenta = Cuenta::default();
    // La profundidad, despues del sombreador: la suya (SV_Depth) o la de la
    // trama con UAV (`tarde`).
    let tarde = (efectos.uav || efectos.referencia) && !efectos.temprana && !reglas.z_del_sombreador;
    let st_tarde = tarde || efectos.referencia;
    let [vx, vy, vw, vh, zmin, zmax] = reglas.viewport;
    let texeles = destino.ancho as usize * destino.alto as usize;
    let prueba = reglas.profundidad.filter(|_| destino.z.as_ref().is_some_and(|z| z.len() >= texeles));
    let plantilla = reglas.stencil.filter(|_| destino.stencil.as_ref().is_some_and(|s| s.len() >= texeles));
    let (mw, mh) = (vw * 0.5, vh * 0.5);
    let (ox, oy) = (vx + mw, vy + mh);
    // El rectangulo donde se puede pintar: viewport, tijera y destino.
    let x0 = (vx.max(0.0) as i64).max(reglas.tijera[0] as i64).max(0);
    let y0 = (vy.max(0.0) as i64).max(reglas.tijera[1] as i64).max(0);
    let x1 = ((vx + vw) as i64).min(reglas.tijera[2] as i64).min(destino.ancho as i64) - 1;
    let y1 = ((vy + vh) as i64).min(reglas.tijera[3] as i64).min(destino.alto as i64) - 1;
    let ancho = destino.ancho as i64;
    // La memoria del sombreador de pixeles: lo ultimo que entro y lo que dio.
    // Con los colores SIN mezclar (N5.11: la mezcla depende del pixel que ya
    // esta, la memoria no) y ya empaquetados para los que no mezclan.
    let mut ultima: Option<(Vec<[f32; 4]>, Salida)> = None;
    // Cuantos render targets se pintan, y el orden de bytes de cada uno.
    let n_rt = (1 + destino.otros.len()).min(OBJETIVOS);
    let mut bgra = [false; OBJETIVOS];
    bgra[0] = destino.bgra;
    for (k, o) in destino.otros.iter().take(OBJETIVOS - 1).enumerate() {
        bgra[k + 1] = o.bgra;
    }
    let comun = Comun { reglas, temprana: efectos.temprana, tarde: st_tarde, prueba, texeles, n_rt, bgra, referencia: efectos.referencia };
    // E2.5: con olas, los pixeles que llegan al sombreador, para despues.
    let mut pendientes: Vec<(Pendiente, i64, i64, Vec<[f32; 4]>)> = Vec::new();
    let mut entrada: Vec<[f32; 4]> = Vec::new();
    // El recorte: los triangulos que cruzan un plano se cambian, EN SU
    // SITIO, por el abanico de lo que queda; sus vertices nuevos van detras
    // de los de siempre (`extra`).
    let n = vertices.len();
    let mut extra: Vec<Sombreado> = Vec::new();
    let mut lista: Vec<[usize; 3]> = Vec::with_capacity(tris.len());
    // N5.16b: con DepthClipEnable = FALSE, los planos de z no cuentan (la
    // banda de x e y si: sigue dejando fuera lo de detras del ojo).
    let desde = if reglas.descarte & SIN_RECORTE_Z != 0 { 2 } else { 0 };
    for t in tris {
        let Some(v) = t.iter().map(|&i| vertices.get(i)).collect::<Option<Vec<_>>>() else {
            cuenta.descartados += 1;
            continue;
        };
        if v.iter().any(|v| !v.pos.iter().all(|c| c.is_finite())) {
            cuenta.descartados += 1;
            continue;
        }
        if v.iter().all(|v| planos(&v.pos)[desde..].iter().all(|&d| d >= 0.0) && v.pos[3] > 0.0) {
            lista.push(*t);
            continue;
        }
        cuenta.recortados += 1;
        let poli = recortar([v[0], v[1], v[2]], desde);
        if poli.len() < 3 {
            cuenta.descartados += 1;
            continue;
        }
        let base = n + extra.len();
        let k = poli.len();
        extra.extend(poli);
        lista.extend((1..k - 1).map(|i| [base, base + i, base + i + 1]));
    }
    let vertice = |i: usize| if i < n { &vertices[i] } else { &extra[i - n] };
    for t in &lista {
        let v = [vertice(t[0]), vertice(t[1]), vertice(t[2])];
        // Recortado, w >= z >= 0; w = 0 es el ojo mismo: nada que pintar.
        if v.iter().any(|v| v.pos[3] <= 0.0) {
            cuenta.descartados += 1;
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
        let fuera = match reglas.descarte & !SIN_RECORTE_Z {
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
        // 05-10: la cara de stencil de este triangulo, por su giro.
        let cara = plantilla.map(|s| if delante { s.delante } else { s.detras });
        // La profundidad de cada vertice, ya en el rango del viewport (sin
        // recorte en z puede salirse: la prueba la sujeta, abajo).
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
        let tri = Tri { x, y, inv_w, zv, v, constante, plano, incluye, zmin, zmax };
        let c = SUBPIXEL / 2;
        let (min_x, max_x) = (x[0].min(x[1]).min(x[2]), x[0].max(x[1]).max(x[2]));
        let (min_y, max_y) = (y[0].min(y[1]).min(y[2]), y[0].max(y[1]).max(y[2]));
        let px0 = (min_x - c + SUBPIXEL - 1).div_euclid(SUBPIXEL).max(x0);
        let px1 = (max_x - c).div_euclid(SUBPIXEL).min(x1);
        let py0 = (min_y - c + SUBPIXEL - 1).div_euclid(SUBPIXEL).max(y0);
        let py1 = (max_y - c).div_euclid(SUBPIXEL).min(y1);
        for py in py0..=py1 {
            for px in px0..=px1 {
                let e = tri.aristas(px, py);
                if !tri.cubre(&e) {
                    continue;
                }
                let i = (py * ancho + px) as usize;
                let mut z_nueva = None;
                let mut z_tarde = None;
                // 05-10: el stencil, con la profundidad, y los dos ANTES del
                // de pixeles (`fallo`: la operacion de stencil de un pixel que
                // NO se pinta) -- salvo con UAV y sin [earlydepthstencil]
                // (`tarde`): entonces el de pixeles corre en todos y los dos se
                // prueban DESPUES, como en D3D.
                let mut fallo = match (cara, destino.stencil.as_deref()) {
                    (Some(c), Some(s)) if !st_tarde && !c.prueba(s[i]) => Some(c.falla),
                    _ => None,
                };
                if let (None, Some(p), Some(zs)) = (fallo, prueba.filter(|_| !reglas.z_del_sombreador), destino.z.as_deref_mut()) {
                    let z = tri.z(&e);
                    if tarde {
                        z_tarde = Some(z);
                    } else if !p.pasa(z, f32::from_bits(zs[i])) {
                        fallo = Some(cara.map_or(crate::stencil::KEEP, |c| c.falla_z));
                    } else if p.escribir && efectos.temprana {
                        zs[i] = z.to_bits();
                    } else if p.escribir {
                        z_nueva = Some(z.to_bits());
                    }
                }
                // [earlydepthstencil]: el stencil se ESCRIBE ya, como la
                // profundidad de arriba (un `discard` ya no lo deshace), y lo
                // que no pasa no corre el de pixeles.
                if efectos.temprana {
                    if let (Some(c), Some(s)) = (cara, destino.stencil.as_deref_mut()) {
                        s[i] = c.aplicar(fallo.unwrap_or(c.pasa), s[i]);
                    }
                    if fallo.is_some() {
                        cuenta.tapados += 1;
                        continue;
                    }
                }
                match fallo {
                    // Su operacion de stencil vale solo si el de pixeles no
                    // lo tira (ver `stencil.rs`): si cambiaria algo, se corre.
                    Some(op) => {
                        cuenta.tapados += 1;
                        if !cara.is_some_and(|c| c.cambia(op)) {
                            continue;
                        }
                    }
                    None => cuenta.pixeles += 1,
                }
                tri.entrada(&e, px, py, posicion, &mut entrada);
                let pendiente = Pendiente { i, fallo, z_nueva, z_tarde };
                // E2.5: con olas, se sombrea con sus vecinos, despues.
                if olas.is_some() {
                    pendientes.push((pendiente, px, py, entrada.clone()));
                    continue;
                }
                let pixel = match &ultima {
                    Some((antes, p)) if !efectos.uav && antes.len() == entrada.len() && antes.iter().zip(&entrada).all(|(a, b)| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())) => *p,
                    _ => {
                        cuenta.sombreados += 1;
                        let mut colores = [[0.0f32; 4]; SALIDAS];
                        let p = ps(&entrada, &mut colores).then(|| (colores, core::array::from_fn(|k| if k < n_rt { empaquetar(colores[k], bgra[k]) } else { 0 })));
                        ultima = Some((entrada.clone(), p));
                        p
                    }
                };
                poner_pixel(&comun, destino, &mut cuenta, cara, pendiente, pixel);
            }
        }
        // E2.5: los del triangulo, en cuadros y olas, y cada uno a su sitio.
        if let Some(o) = olas.as_mut() {
            // Uno que no paso el stencil corre solo para saber si lo tira
            // (su operacion de stencil cambiaria algo): en la ola, AYUDANTE
            // (D3D ni lo correria: ni cuenta en las olas ni escribe UAV).
            let reales: Vec<(i64, i64, &[[f32; 4]], bool)> = pendientes.iter().map(|(p, px, py, e)| (*px, *py, e.as_slice(), p.fallo.is_some())).collect();
            let hechos = crate::cuadros::sombrear(&tri, &reales, posicion, &mut **o);
            for ((pendiente, ..), hecho) in pendientes.drain(..).zip(hechos) {
                cuenta.sombreados += 1;
                let pixel = hecho.map(|colores| (colores, core::array::from_fn(|k| if k < n_rt { empaquetar(colores[k], bgra[k]) } else { 0 })));
                poner_pixel(&comun, destino, &mut cuenta, cara, pendiente, pixel);
            }
        }
    }
    cuenta
}

/// **Poner un pixel ya sombreado** (sacado del bucle de [`dibujar_con`] el
/// 05-10, E2.5, para que un pixel de una ola se ponga igual): lo que va
/// DESPUES del de pixeles, en el orden de D3D -- el stencil de un pixel que
/// no paso, las pruebas de despues (`tarde`, SV_Depth), la cuenta de los
/// que pasan, la profundidad, el stencil y la mezcla en cada render target.
fn poner_pixel(k: &Comun, destino: &mut Destino, cuenta: &mut Cuenta, cara: Option<crate::stencil::Cara>, p: Pendiente, pixel: Salida) {
    let Pendiente { i, fallo, mut z_nueva, z_tarde } = p;
    let (reglas, prueba, n_rt, bgra) = (k.reglas, k.prueba, k.n_rt, k.bgra);
    let (zmin, zmax) = (reglas.viewport[4], reglas.viewport[5]);
    let mezclas = reglas.mezcla;
    // Palabras por texel de un render target de float (N5.16b): cuatro, o
    // una (un R32F, como lo guarda la casa); lo dice lo que mide.
    let palabras = |n: usize| if n >= 4 * k.texeles { 4 } else { 1 };
    let Some((colores, pixel)) = pixel else {
        if fallo.is_none() {
            cuenta.tirados += 1;
        }
        return;
    };
    // SV_StencilRef: la referencia de ESTE pixel, para la prueba y REPLACE.
    let cara = cara.map(|c| if k.referencia { crate::stencil::Cara { referencia: colores[REFERENCIA][0].to_bits() as u8, ..c } } else { c });
    if let (Some(op), Some(c), Some(s)) = (fallo, cara, destino.stencil.as_deref_mut()) {
        s[i] = c.aplicar(op, s[i]);
        return;
    }
    // `tarde` (UAV sin [earlydepthstencil], o SV_StencilRef): el stencil, ahora.
    if let (true, Some(c), Some(s)) = (k.tarde, cara, destino.stencil.as_deref_mut()) {
        if !c.prueba(s[i]) {
            cuenta.tapados += 1;
            s[i] = c.aplicar(c.falla, s[i]);
            return;
        }
    }
    // SV_Depth: la prueba, ahora, con la Z del sombreador (D3D la
    // recorta al rango del viewport); con UAV, con la de la trama.
    let z_despues = if reglas.z_del_sombreador { Some(colores[PROFUNDIDAD][0].clamp(zmin.min(zmax), zmin.max(zmax))) } else { z_tarde };
    if let (Some(z), Some(p), Some(zs)) = (z_despues, prueba, destino.z.as_deref_mut()) {
        if !p.pasa(z, f32::from_bits(zs[i])) {
            cuenta.tapados += 1;
            if let (Some(c), Some(s)) = (cara, destino.stencil.as_deref_mut()) {
                s[i] = c.aplicar(c.falla_z, s[i]);
            }
            return;
        }
        if p.escribir {
            z_nueva = Some(z.to_bits());
        }
    }
    cuenta.pasan += 1;
    if let (Some(z), Some(zs)) = (z_nueva, destino.z.as_deref_mut()) {
        zs[i] = z;
    }
    // Con [earlydepthstencil] ya se escribio arriba.
    if let (false, Some(c), Some(s)) = (k.temprana, cara, destino.stencil.as_deref_mut()) {
        s[i] = c.aplicar(c.pasa, s[i]);
    }
    // El render target `k`: el pixel nuevo, o mezclado con el que esta.
    let poner = |k: usize, p: &mut u32| {
        let m = &mezclas.rt[k];
        *p = if m.trivial() { pixel[k] } else { empaquetar(m.aplicar(colores[k], desempaquetar(*p, bgra[k]), mezclas.factor), bgra[k]) };
    };
    // N5.16: el de un render target de float, en float: mezclado
    // con el que esta y cuantizado a su formato. N5.16b: con una
    // palabra por texel (R32F), el r; lo demas se lee (0, 0, 1).
    // 05-10: uno de ENTEROS (R32_UINT, RGBA16_SINT...) guarda los bits del
    // sombreador saturados a su canal (`formato_ia::de_entero`), SIN mezcla
    // (D3D no mezcla enteros) y con su mascara de escritura.
    // D2.7 (06-10): con OTRA vista (`bufer::con_vista`), lo que hay se ve en
    // ella y lo que sale vuelve a lo guardado: en cuatro floats, por los
    // bytes del elemento; en una palabra (un RGBA8 o un R32 de la casa,
    // guardado 0), la palabra ES los bytes de la memoria.
    let poner_f = |k: usize, f: u32, t: &mut [u32]| {
        let (g, otra) = if f & crate::bufer::CUATRO_FLOATS != 0 { crate::bufer::guardado_y_vista(f) } else { (f, None) };
        let v = otra.unwrap_or(g);
        let entero = crate::formato_ia::de_entero(v, colores[k]);
        let m = crate::mezcla::Mezcla { encendida: mezclas.rt[k].encendida && entero.is_none(), ..mezclas.rt[k] };
        let d: [f32; 4] = match (otra, t.len()) {
            (Some(o), 1) => crate::formato_ia::leer(o, &t[0].to_le_bytes()),
            (Some(o), 4) => crate::bufer::a_la_vista(g, o, [t[0], t[1], t[2], t[3]]).map(f32::from_bits),
            _ => core::array::from_fn(|c| t.get(c).map_or(if c == 3 { 1.0 } else { 0.0 }, |&w| f32::from_bits(w))),
        };
        let o = entero.unwrap_or(colores[k]);
        let c = if m.trivial() { o } else { m.aplicar(o, d, mezclas.factor) };
        let c = if entero.is_some() { c } else { crate::formato_ia::cuantizar(v, c) };
        match (otra, t.len()) {
            (Some(o), 1) => {
                if let Some(b) = crate::formato_ia::empaquetar(o, c.map(f32::to_bits), entero.is_some()) {
                    t[0] = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                }
            }
            (Some(o), 4) => {
                if let Some(q) = crate::bufer::de_la_vista(g, o, c.map(f32::to_bits)) {
                    for (w, x) in t.iter_mut().zip(q) {
                        *w = x.to_bits();
                    }
                }
            }
            _ => {
                for (w, x) in t.iter_mut().zip(c) {
                    *w = x.to_bits();
                }
            }
        }
    };
    // N5.12: sin render target (solo profundidad), `pixeles` va vacio.
    match destino.flotante {
        Some(f) => {
            let w = palabras(destino.pixeles.len());
            if let Some(t) = destino.pixeles.get_mut(w * i..w * i + w) {
                poner_f(0, f, t);
            }
        }
        None => {
            if let Some(p) = destino.pixeles.get_mut(i) {
                poner(0, p);
            }
        }
    }
    for (k, o) in destino.otros.iter_mut().take(n_rt - 1).enumerate() {
        match (o.flotante, o.pixeles.as_deref_mut()) {
            (Some(f), Some(p)) => {
                let w = palabras(p.len());
                if let Some(t) = p.get_mut(w * i..w * i + w) {
                    poner_f(k + 1, f, t);
                }
            }
            (None, Some(p)) => {
                if let Some(p) = p.get_mut(i) {
                    poner(k + 1, p);
                }
            }
            _ => {}
        }
    }
}

