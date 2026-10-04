//! # EL PINTOR -- lo que una MAQUETA pinta, con nitidez de matematica
//!
//! El propietario (04-10): *"que MAQUETA mejore con ultra nitidez matematica,
//! como SVG"*. Nitidez de matematica quiere decir esto: cada borde se calcula
//! como lo que es -- una curva, una recta, un circulo -- y cada pixel del borde
//! recibe EXACTAMENTE la parte que la figura le cubre. Ni escalera ni borron.
//!
//! ```text
//!    caja        rectangulo de esquinas redondas: la distancia de cada pixel
//!                al borde, y de ella su tinta
//!    resplandor  el `box-shadow: 0 0 Npx` de la maqueta, cayendo suave
//!    degradado   dos colores en un eje, dentro de su caja redonda
//!    letra       la de la casa (`bmo-letra`), en su caja de `line-height`
//!    trazo       una polilinea con pluma redonda (los `<path>` de SVG, ya
//!                aplanados en el anfitrion: aqui solo se entinta)
//!    relleno     el interior par-impar de unos caminos, con cuatro sub-filas
//! ```
//!
//! **Sin monton y sin `unsafe`**: lo usan el ESCRITORIO (Ring 3, sin `alloc`),
//! las apps y el compilador de MAQUETA en el anfitrion -- y los tres pintan
//! IGUAL porque ejecutan el MISMO codigo (la leccion de `bmo-dibujo`).
//!
//! Quien pinta solo tiene que saber dos cosas de su lienzo: poner un
//! rectangulo macizo y mezclar un color sobre un pixel. Ver [`Lienzo`].

#![no_std]
#![forbid(unsafe_code)]

pub use bmo_letra::{Estilo, Fuente, Peso};

/// Un color `0x00RRGGBB`.
pub type Color = u32;

/// **Lo que el pintor necesita de un lienzo.** Las coordenadas pueden caer
/// fuera: recortar es del lienzo.
pub trait Lienzo {
    /// Un rectangulo macizo.
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color);
    /// `c` sobre lo que hay en `(x, y)`, con `alfa` de 255.
    fn mezclar(&mut self, x: i32, y: i32, c: Color, alfa: u8);
}

/// Una mezcla de dos colores, `t` de 256 hacia `a`.
pub fn entre(b: Color, a: Color, t: u32) -> Color {
    let t = t.min(256);
    let c = |s: u32| (((a >> s & 255) * t + (b >> s & 255) * (256 - t)) / 256) << s;
    c(16) | c(8) | c(0)
}

/// **`c` sobre `fondo` con `alfa` de 255**: la UNICA mezcla de la casa. La
/// usan la pantalla del escritorio y la foto del anfitrion, para que el
/// borde suave salga igual en los dos -- un redondeo distinto en cada lado
/// seria un pixel distinto en cada borde.
pub fn sobre(fondo: Color, c: Color, alfa: u8) -> Color {
    match alfa {
        0 => fondo,
        255 => c,
        _ => entre(fondo, c, (alfa as u32 * 256 + 127) / 255),
    }
}

/// Raiz cuadrada entera, por abajo.
fn raiz(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// **La distancia (1/16 px, con signo: negativa dentro) del centro del
/// pixel `(px, py)` al borde de una caja de esquinas redondas.**
fn al_borde(px: i32, py: i32, x: i32, y: i32, w: i32, h: i32, r: i32) -> i32 {
    // Todo en 1/16 de pixel, respecto al centro de la caja.
    let (cx2, cy2) = (2 * x + w, 2 * y + h);
    let qx = ((2 * px + 1 - cx2).abs() * 8) - (w - 2 * r) * 8;
    let qy = ((2 * py + 1 - cy2).abs() * 8) - (h - 2 * r) * 8;
    let (mx, my) = (qx.max(0) as i64, qy.max(0) as i64);
    let fuera = raiz((mx * mx + my * my) as u64) as i32;
    fuera + qx.max(qy).min(0) - r * 16
}

/// La tinta (0..=255) de un pixel a `d` (1/16 px, con signo) del borde: una
/// rampa de un pixel centrada en el contorno.
fn tinta(d: i32) -> u32 {
    ((8 - d).clamp(0, 16) as u32 * 255) / 16
}

/// **Una caja de esquinas redondas**, maciza, con el borde de la curva
/// suavizado. `r` se recorta a la mitad del lado menor (como en CSS).
pub fn caja(l: &mut impl Lienzo, x: i32, y: i32, w: i32, h: i32, r: i32, c: Color) {
    if w <= 0 || h <= 0 {
        return;
    }
    let r = r.clamp(0, w.min(h) / 2);
    if r == 0 {
        l.rect(x, y, w, h, c);
        return;
    }
    // El centro: tres rectangulos macizos. Las cuatro esquinas: pixel a pixel.
    l.rect(x + r, y, w - 2 * r, h, c);
    l.rect(x, y + r, r, h - 2 * r, c);
    l.rect(x + w - r, y + r, r, h - 2 * r, c);
    for (ex, ey) in [(x, y), (x + w - r, y), (x, y + h - r), (x + w - r, y + h - r)] {
        for py in ey..ey + r {
            for px in ex..ex + r {
                let a = tinta(al_borde(px, py, x, y, w, h, r));
                if a >= 255 {
                    l.rect(px, py, 1, 1, c);
                } else if a > 0 {
                    l.mezclar(px, py, c, a as u8);
                }
            }
        }
    }
}

/// **Un borde** de `grosor` pixeles alrededor de una caja redonda (lo de
/// dentro no se toca): la diferencia de dos curvas, suavizada en las dos.
pub fn borde(l: &mut impl Lienzo, x: i32, y: i32, w: i32, h: i32, r: i32, grosor: i32, c: Color) {
    if w <= 0 || h <= 0 || grosor <= 0 {
        return;
    }
    let r = r.clamp(0, w.min(h) / 2);
    let ri = (r - grosor).max(0);
    let (xi, yi, wi, hi) = (x + grosor, y + grosor, w - 2 * grosor, h - 2 * grosor);
    let banda = grosor.max(r);
    for py in y..y + h {
        // En las filas de en medio solo hay borde en los laterales.
        let medio = py >= y + banda && py < y + h - banda;
        let mut px = x;
        while px < x + w {
            if medio && px == x + banda {
                px = x + w - banda;
                continue;
            }
            let fuera = tinta(al_borde(px, py, x, y, w, h, r));
            let dentro = if wi > 0 && hi > 0 { tinta(al_borde(px, py, xi, yi, wi, hi, ri)) } else { 0 };
            let a = fuera.saturating_sub(dentro);
            if a >= 255 {
                l.rect(px, py, 1, 1, c);
            } else if a > 0 {
                l.mezclar(px, py, c, a as u8);
            }
            px += 1;
        }
    }
}

/// **El resplandor** de una caja redonda (`box-shadow: 0 0 Npx`): `argb`
/// con su fuerza en el alfa, cayendo suave hasta `alcance` pixeles fuera. Lo
/// de dentro de la caja no se pinta (lo tapa la caja).
pub fn resplandor(l: &mut impl Lienzo, x: i32, y: i32, w: i32, h: i32, r: i32, alcance: i32, argb: u32) {
    let n = alcance.max(1);
    let fuerza = argb >> 24;
    let c = argb & 0x00FF_FFFF;
    let r = r.clamp(0, w.min(h) / 2);
    for py in y - n..y + h + n {
        for px in x - n..x + w + n {
            let d = al_borde(px, py, x, y, w, h, r);
            if d <= 0 {
                continue;
            }
            // Una campana: (1 - t)^2 de 0 al alcance (en 1/16 px).
            let t = (n * 16 - d).max(0) as u32;
            if t == 0 {
                continue;
            }
            let lado = (n * 16) as u32;
            let a = fuerza * t * t / (lado * lado);
            if a > 0 {
                l.mezclar(px, py, c, a.min(255) as u8);
            }
        }
    }
}

/// **Un degradado** de dos colores (de izquierda a derecha, o de arriba
/// abajo si `vertical`), dentro de su caja redonda.
pub fn degradado(l: &mut impl Lienzo, x: i32, y: i32, w: i32, h: i32, r: i32, de: Color, a: Color, vertical: bool) {
    if w <= 0 || h <= 0 {
        return;
    }
    let r = r.clamp(0, w.min(h) / 2);
    let largo = if vertical { h } else { w }.max(2) - 1;
    for py in y..y + h {
        for px in x..x + w {
            let k = if vertical { py - y } else { px - x };
            let c = entre(de, a, (k * 256 / largo) as u32);
            let en_esquina = (px < x + r || px >= x + w - r) && (py < y + r || py >= y + h - r);
            if !en_esquina {
                l.rect(px, py, 1, 1, c);
                continue;
            }
            let t = tinta(al_borde(px, py, x, y, w, h, r));
            if t >= 255 {
                l.rect(px, py, 1, 1, c);
            } else if t > 0 {
                l.mezclar(px, py, c, t as u8);
            }
        }
    }
}

/// **Letra de la casa** en una caja de texto de `alto` (su `line-height`),
/// como la pone el navegador. Devuelve el ancho.
pub fn letra(l: &mut impl Lienzo, f: &mut impl Fuente, x: i32, y: i32, alto: i32, s: &[u8], c: Color, e: Estilo) -> i32 {
    let base = y + bmo_letra::base_en_caja(e.px, alto);
    f.escribir(s, e, x, base, |px, py, a| l.mezclar(px, py, c, a))
}

/// La distancia (1/64 px) del punto `p` al segmento `a`-`b`.
fn distancia(p: (i32, i32), a: (i32, i32), b: (i32, i32)) -> i32 {
    let (px, py) = ((p.0 - a.0) as i64, (p.1 - a.1) as i64);
    let (bx, by) = ((b.0 - a.0) as i64, (b.1 - a.1) as i64);
    let l2 = bx * bx + by * by;
    let t = px * bx + py * by;
    let d2 = if l2 == 0 || t <= 0 {
        px * px + py * py
    } else if t >= l2 {
        let (qx, qy) = (px - bx, py - by);
        qx * qx + qy * qy
    } else {
        let cruz = px * by - py * bx;
        ((cruz as i128 * cruz as i128) / l2 as i128) as i64
    };
    raiz(d2 as u64) as i32
}

/// **Un trazo**: la polilinea `puntos` (1/64 de pixel, ya en el lienzo) con
/// una pluma redonda de `grosor64`. Cada pixel lleva la tinta del tramo que
/// MAS lo cubre (las juntas no se pintan dos veces), y sin monton: se recorre
/// la caja del trazo y en cada pixel se busca el tramo mas cercano.
pub fn trazo(l: &mut impl Lienzo, puntos: &[(i32, i32)], cerrado: bool, grosor64: i32, c: Color) {
    if puntos.is_empty() {
        return;
    }
    let r = (grosor64 / 2).max(16);
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &(x, y) in puntos {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let (px0, py0) = ((x0 - r - 32).div_euclid(64), (y0 - r - 32).div_euclid(64));
    let (px1, py1) = ((x1 + r + 32).div_euclid(64) + 1, (y1 + r + 32).div_euclid(64) + 1);
    let n = puntos.len();
    let tramos = if cerrado && n > 2 { n } else { n.saturating_sub(1).max(1) };
    for py in py0..py1 {
        for px in px0..px1 {
            let centro = (px * 64 + 32, py * 64 + 32);
            let mut d = i32::MAX;
            for k in 0..tramos {
                let a = puntos[k];
                let b = if n == 1 { a } else { puntos[(k + 1) % n] };
                // Lejos de la caja de este tramo: no hace falta medir.
                if centro.0 + r + 32 < a.0.min(b.0) || centro.0 - r - 32 > a.0.max(b.0) || centro.1 + r + 32 < a.1.min(b.1) || centro.1 - r - 32 > a.1.max(b.1) {
                    continue;
                }
                d = d.min(distancia(centro, a, b));
            }
            if d == i32::MAX {
                continue;
            }
            let t = (32 + r - d).clamp(0, 64) as u32;
            if t >= 64 {
                l.rect(px, py, 1, 1, c);
            } else if t > 0 {
                l.mezclar(px, py, c, (t * 255 / 64) as u8);
            }
        }
    }
}

/// Los cortes que caben en una sub-fila de un relleno.
const CORTES: usize = 64;

/// **Un relleno** par-impar de unos caminos cerrados (1/64 de pixel, ya en
/// el lienzo), suavizado: cuatro sub-filas por pixel y, en cada una, la
/// parte exacta de cada pixel que el tramo cubre. Sin monton: los cortes de
/// una sub-fila caben en 64 (una figura con mas se pinta sin los que sobran,
/// y se ve; nunca se escribe fuera).
pub fn relleno(l: &mut impl Lienzo, caminos: &[&[(i32, i32)]], c: Color) {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for s in caminos {
        for &(x, y) in s.iter() {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 > x1 {
        return;
    }
    let (px0, py0) = (x0.div_euclid(64), y0.div_euclid(64));
    let (px1, py1) = (x1.div_euclid(64) + 1, y1.div_euclid(64) + 1);
    // La tinta acumulada de una fila, de 64 en 64 pixeles.
    for bloque in (px0..px1).step_by(64) {
        let fin = (bloque + 64).min(px1);
        for py in py0..py1 {
            let mut acum = [0u32; 64];
            for sub in 0..4 {
                let y = py * 64 + 8 + sub * 16;
                let mut cortes = [0i32; CORTES];
                let mut n = 0;
                for s in caminos {
                    let m = s.len();
                    if m < 2 {
                        continue;
                    }
                    for k in 0..m {
                        let (a, b) = (s[k], s[(k + 1) % m]);
                        if (a.1 <= y) != (b.1 <= y) && n < CORTES {
                            cortes[n] = (a.0 as i64 + (b.0 - a.0) as i64 * (y - a.1) as i64 / (b.1 - a.1) as i64) as i32;
                            n += 1;
                        }
                    }
                }
                cortes[..n].sort_unstable();
                for par in cortes[..n].chunks_exact(2) {
                    let (a, b) = (par[0].max(bloque * 64), par[1].min(fin * 64));
                    if a >= b {
                        continue;
                    }
                    for col in a.div_euclid(64)..=(b - 1).div_euclid(64) {
                        let ini = a.max(col * 64);
                        let hasta = b.min(col * 64 + 64);
                        acum[(col - bloque) as usize] += (hasta - ini) as u32;
                    }
                }
            }
            for (i, &t) in acum[..(fin - bloque) as usize].iter().enumerate() {
                if t >= 256 {
                    l.rect(bloque + i as i32, py, 1, 1, c);
                } else if t > 0 {
                    l.mezclar(bloque + i as i32, py, c, (t * 255 / 256) as u8);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// LA PIEZA: lo que una cara pinta, descrito una vez
// ---------------------------------------------------------------------------

/// **Una pieza de una cara**, ya resuelta por MAQUETA: lo mismo que pinta el
/// codigo generado (emisor A), lo que viaja en un recurso CARA (emisor B) y lo
/// que pinta la foto del anfitrion. Una descripcion, tres caminos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pieza<'a> {
    Caja { x: i32, y: i32, w: i32, h: i32, r: i32, c: Color },
    Borde { x: i32, y: i32, w: i32, h: i32, r: i32, grosor: i32, c: Color },
    Resplandor { x: i32, y: i32, w: i32, h: i32, r: i32, alcance: i32, argb: u32 },
    Degradado { x: i32, y: i32, w: i32, h: i32, r: i32, de: Color, a: Color, vertical: bool },
    /// La letra de la casa: su caja (`alto` = `line-height`) y su estilo
    /// (peso 400, 500, 600 o 700; espacio en milesimas de eme).
    Letra { x: i32, y: i32, alto: i32, texto: &'a [u8], c: Color, px: u8, peso: u16, espacio: i32, mayusculas: bool },
    /// Polilineas de 1/64 px, con la pluma redonda de `grosor64`.
    Trazo { caminos: &'a [&'a [(i32, i32)]], cerrados: &'a [bool], grosor64: i32, c: Color },
    Relleno { caminos: &'a [&'a [(i32, i32)]], c: Color },
}

/// El estilo de la letra de una pieza.
pub fn estilo(px: u8, peso: u16, espacio: i32, mayusculas: bool) -> Estilo {
    let e = match peso {
        500 => Estilo::media(px),
        600 | 700 => Estilo::negrita(px),
        _ => Estilo::normal(px),
    }
    .espaciado(espacio);
    if mayusculas { e.mayusculas() } else { e }
}

/// **Un lienzo con recorte**: lo de fuera de `[x0, x1) x [y0, y1)` no se
/// toca. Es lo que deja repintar SOLO un trozo (el `pintar_en` de una cara)
/// sin mezclar dos veces el borde suave de lo que se sale.
pub struct Recortado<'l, L: Lienzo> {
    pub dentro: &'l mut L,
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl<L: Lienzo> Lienzo for Recortado<'_, L> {
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        let (a, b) = (x.max(self.x0), y.max(self.y0));
        let (c2, d) = ((x + w).min(self.x1), (y + h).min(self.y1));
        if a < c2 && b < d {
            self.dentro.rect(a, b, c2 - a, d - b, c);
        }
    }
    fn mezclar(&mut self, x: i32, y: i32, c: Color, alfa: u8) {
        if x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1 {
            self.dentro.mezclar(x, y, c, alfa);
        }
    }
}

/// Un lienzo corrido `(ox, oy)` (1/64 px para los caminos van aparte).
struct Corrido<'l, L: Lienzo> {
    l: &'l mut L,
    ox: i32,
    oy: i32,
}

impl<L: Lienzo> Lienzo for Corrido<'_, L> {
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        self.l.rect(x + self.ox, y + self.oy, w, h, c);
    }
    fn mezclar(&mut self, x: i32, y: i32, c: Color, alfa: u8) {
        self.l.mezclar(x + self.ox, y + self.oy, c, alfa);
    }
}

/// **Pinta una pieza** con su origen en `(ox, oy)`.
pub fn pieza(l: &mut impl Lienzo, f: &mut impl Fuente, p: &Pieza, ox: i32, oy: i32) {
    let mut l = Corrido { l, ox, oy };
    match *p {
        Pieza::Caja { x, y, w, h, r, c } => caja(&mut l, x, y, w, h, r, c),
        Pieza::Borde { x, y, w, h, r, grosor, c } => borde(&mut l, x, y, w, h, r, grosor, c),
        Pieza::Resplandor { x, y, w, h, r, alcance, argb } => resplandor(&mut l, x, y, w, h, r, alcance, argb),
        Pieza::Degradado { x, y, w, h, r, de, a, vertical } => degradado(&mut l, x, y, w, h, r, de, a, vertical),
        Pieza::Letra { x, y, alto, texto, c, px, peso, espacio, mayusculas } => {
            letra(&mut l, f, x, y, alto, texto, c, estilo(px, peso, espacio, mayusculas));
        }
        Pieza::Trazo { caminos, cerrados, grosor64, c } => {
            for (k, cam) in caminos.iter().enumerate() {
                trazo(&mut l, cam, cerrados.get(k).copied().unwrap_or(false), grosor64, c);
            }
        }
        Pieza::Relleno { caminos, c } => relleno(&mut l, caminos, c),
    }
}

/// La caja (en pixeles, sin el origen) que una pieza puede manchar, con lo
/// que su borde suave o su resplandor se salen.
pub fn caja_de(p: &Pieza) -> (i32, i32, i32, i32) {
    match *p {
        Pieza::Caja { x, y, w, h, .. } | Pieza::Borde { x, y, w, h, .. } | Pieza::Degradado { x, y, w, h, .. } => (x, y, w, h),
        Pieza::Resplandor { x, y, w, h, alcance, .. } => (x - alcance, y - alcance, w + 2 * alcance, h + 2 * alcance),
        // La letra puede asomar por arriba (acentos) y por abajo.
        Pieza::Letra { x, y, alto, px, .. } => (x - 2, y - px as i32 / 2, 4096, alto + px as i32),
        Pieza::Trazo { caminos, grosor64, .. } => caja_de_caminos(caminos, grosor64),
        Pieza::Relleno { caminos, .. } => caja_de_caminos(caminos, 0),
    }
}

fn caja_de_caminos(caminos: &[&[(i32, i32)]], grosor64: i32) -> (i32, i32, i32, i32) {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for c in caminos {
        for &(x, y) in c.iter() {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 > x1 {
        return (0, 0, 0, 0);
    }
    let m = grosor64 / 2 + 64;
    let (a, b) = ((x0 - m).div_euclid(64), (y0 - m).div_euclid(64));
    (a, b, (x1 + m).div_euclid(64) + 1 - a, (y1 + m).div_euclid(64) + 1 - b)
}

// ---------------------------------------------------------------------------
// UNA CARA QUE VIAJA (version 2), pintada
// ---------------------------------------------------------------------------

/// Los puntos que caben en un camino de una cara, y los subcaminos.
const PUNTOS: usize = 1024;
const SUBS: usize = 32;

/// **Pinta un trazo de una CARA** (ya comprobada por `bmo_maqueta_cara::leer`)
/// con su origen en `(ox, oy)`. Sin monton: los caminos se descodifican en
/// una tabla fija (los que no caben, no se pintan; nunca se escribe fuera).
pub fn pincelada(l: &mut impl Lienzo, f: &mut impl Fuente, p: &bmo_maqueta_cara::Pincelada, ox: i32, oy: i32) {
    use bmo_maqueta_cara as cara;
    let (x, y, w, h) = (p.x as i32, p.y as i32, p.w as i32, p.h as i32);
    let e = p.extra as i32;
    let pz = match p.clase {
        cara::CLASE_RECT => {
            l.rect(ox + x, oy + y, w, h, p.color);
            return;
        }
        // La letra de pixel la pinta quien la tiene (el escritorio): aqui no
        // hay tabla de 8 x 16.
        cara::CLASE_TEXTO => return,
        cara::CLASE_CAJA => Pieza::Caja { x, y, w, h, r: e, c: p.color },
        cara::CLASE_BORDE => Pieza::Borde { x, y, w, h, r: e & 0xFF, grosor: e >> 8, c: p.color },
        cara::CLASE_RESPLANDOR => Pieza::Resplandor { x, y, w, h, r: e & 0xFF, alcance: e >> 8, argb: p.color },
        cara::CLASE_DEGRADADO => {
            let a = match p.datos {
                [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
                _ => return,
            };
            Pieza::Degradado { x, y, w, h, r: e & 0x7FFF, de: p.color, a, vertical: e >> 15 != 0 }
        }
        cara::CLASE_LETRA => Pieza::Letra {
            x,
            y,
            alto: h,
            texto: p.texto,
            c: p.color,
            px: (e & 0x7F) as u8,
            peso: [400, 500, 600, 700][((e >> 7) & 3) as usize],
            espacio: ((e >> 10) & 63) * 10,
            mayusculas: (e >> 9) & 1 != 0,
        },
        cara::CLASE_LINEA | cara::CLASE_RELLENO => {
            let mut puntos = [(0i32, 0i32); PUNTOS];
            let mut cortes = [(0usize, 0usize, false); SUBS];
            let (mut n, mut m) = (0usize, 0usize);
            cara::subcaminos(p.datos, |cerrado, pares| {
                if m == SUBS {
                    return;
                }
                let ini = n;
                for par in pares.chunks_exact(4) {
                    if n == PUNTOS {
                        break;
                    }
                    let px = i16::from_le_bytes([par[0], par[1]]) as i32;
                    let py = i16::from_le_bytes([par[2], par[3]]) as i32;
                    puntos[n] = ((x * 64) + px * 4, (y * 64) + py * 4);
                    n += 1;
                }
                cortes[m] = (ini, n, cerrado);
                m += 1;
            });
            let mut caminos: [&[(i32, i32)]; SUBS] = [&[]; SUBS];
            let mut cerrados = [false; SUBS];
            for k in 0..m {
                caminos[k] = &puntos[cortes[k].0..cortes[k].1];
                cerrados[k] = cortes[k].2;
            }
            let pz = if p.clase == cara::CLASE_LINEA {
                Pieza::Trazo { caminos: &caminos[..m], cerrados: &cerrados[..m], grosor64: e * 4, c: p.color }
            } else {
                Pieza::Relleno { caminos: &caminos[..m], c: p.color }
            };
            pieza(l, f, &pz, ox, oy);
            return;
        }
        _ => return,
    };
    pieza(l, f, &pz, ox, oy);
}

#[cfg(test)]
mod pruebas;
