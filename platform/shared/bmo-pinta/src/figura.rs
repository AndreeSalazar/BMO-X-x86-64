//! **LA FIGURA** (MAQUETA 3, 06-10): lo que un SVG de verdad pinta.
//!
//! `docs/componente/LA_MAQUETA_EXIGE.md`, seccion 2e. El lector de SVG del
//! anfitrion (`bmo-maqueta-dibujo`) ya lo aplano TODO -- arcos, grupos,
//! giros, discontinuos, el contorno de una pluma que no es redonda -- y lo
//! que llega aqui son caminos de 1/64 de pixel con tres cosas mas que las
//! piezas de MAQUETA 2 no sabian:
//!
//! ```text
//!    la regla     `nonzero` (la de SVG) o `evenodd`: el sentido de cada
//!                 corte cuenta, no solo cuantos hay
//!    la tinta     lisa, o un degradado lineal o radial con hasta ocho
//!                 paradas, cada una con su alfa
//!    la opacidad  de la figura entera: transparencia de verdad, mezclada
//!                 con lo que ya hay debajo
//! ```
//!
//! ** Y los puntos se LEEN por un rasgo ([`Caminos`]), no por una tabla.
//! Asi una figura a medio camino entre dos pasos de una animacion (S7) se
//! pinta sin construirla: [`Mezcla`] da cada punto mezclado cuando el pintor
//! lo pide. Sin monton, como todo el pintor.

use crate::{entre, Color, Lienzo};

/// Las paradas que caben en un degradado.
pub const PARADAS: usize = 8;

/// **Una parada de un degradado**: donde (milesimas del camino del degradado)
/// y que color, con su alfa (255, opaca).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parada {
    pub en: u16,
    pub c: Color,
    pub alfa: u8,
}

/// **La tinta de una figura.** Las coordenadas, en 1/64 de pixel como los
/// caminos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tinta<'a> {
    Liso(Color),
    /// `t` = 0 en `de` y 1 en `a`; las lineas del mismo color son
    /// perpendiculares a `de -> a`. Un degradado girado, estirado o en las
    /// unidades de la caja se reduce a esto al compilar, exacto: `t` es una
    /// funcion lineal del pixel, y esta es su forma.
    Lineal { de: (i32, i32), a: (i32, i32), paradas: &'a [Parada] },
    /// `t` = |M^-1 (P - centro)| con M = [`eje_x` `eje_y`]: el circulo de
    /// radio 1 del degradado, llevado al lienzo (una elipse girada, si hace
    /// falta).
    Radial { centro: (i32, i32), eje_x: (i32, i32), eje_y: (i32, i32), paradas: &'a [Parada] },
}

impl Tinta<'_> {
    /// Las paradas (vacio en la lisa).
    pub fn paradas(&self) -> &[Parada] {
        match self {
            Tinta::Liso(_) => &[],
            Tinta::Lineal { paradas, .. } | Tinta::Radial { paradas, .. } => paradas,
        }
    }
}

/// **De donde lee el pintor los puntos.** Una tabla ([`Fijos`]) o dos
/// tablas mezcladas ([`Mezcla`]).
pub trait Caminos {
    /// Cuantos subcaminos.
    fn cuantos(&self) -> usize;
    /// Cuantos puntos tiene el subcamino `s`.
    fn largo(&self, s: usize) -> usize;
    /// El punto `k` del subcamino `s`, en 1/64 de pixel.
    fn punto(&self, s: usize, k: usize) -> (i32, i32);
    /// Si el subcamino `s` se cierra (para la pluma; un relleno los cierra
    /// todos, como SVG).
    fn cerrado(&self, s: usize) -> bool;
}

/// Los caminos de una pieza, tal cual.
#[derive(Clone, Copy)]
pub struct Fijos<'a> {
    pub caminos: &'a [&'a [(i32, i32)]],
    pub cerrados: &'a [bool],
}

impl Caminos for Fijos<'_> {
    fn cuantos(&self) -> usize {
        self.caminos.len()
    }
    fn largo(&self, s: usize) -> usize {
        self.caminos[s].len()
    }
    fn punto(&self, s: usize, k: usize) -> (i32, i32) {
        self.caminos[s][k]
    }
    fn cerrado(&self, s: usize) -> bool {
        self.cerrados.get(s).copied().unwrap_or(false)
    }
}

/// **Dos tablas mezcladas**, `p` milesimas de `a` hacia `b`: cada punto se
/// calcula cuando se pide. El compilador garantiza que las dos tienen la
/// misma forma (S7); si no la tuvieran, se lee lo que tienen en comun.
#[derive(Clone, Copy)]
pub struct Mezcla<'a> {
    pub a: Fijos<'a>,
    pub b: Fijos<'a>,
    pub p: i32,
}

impl Caminos for Mezcla<'_> {
    fn cuantos(&self) -> usize {
        self.a.cuantos().min(self.b.cuantos())
    }
    fn largo(&self, s: usize) -> usize {
        self.a.largo(s).min(self.b.largo(s))
    }
    fn punto(&self, s: usize, k: usize) -> (i32, i32) {
        let (x0, y0) = self.a.punto(s, k);
        let (x1, y1) = self.b.punto(s, k);
        (crate::entre_i(x0, x1, self.p), crate::entre_i(y0, y1, self.p))
    }
    fn cerrado(&self, s: usize) -> bool {
        self.a.cerrado(s)
    }
}

// ---------------------------------------------------------------------------
// La tinta, resuelta una vez por figura
// ---------------------------------------------------------------------------

/// Una tinta lista para preguntarle el color de cada pixel.
#[derive(Clone, Copy)]
pub struct Pincel<'a> {
    forma: Forma,
    liso: Color,
    paradas: &'a [Parada],
}

#[derive(Clone, Copy)]
enum Forma {
    Liso,
    /// `t*1000 = ((x-x0)*dx + (y-y0)*dy) * 1000 / den`
    Lineal { x0: i64, y0: i64, dx: i64, dy: i64, den: i64 },
    /// `(u, v) = M^-1 (P - c)`, con la inversa ya escrita y su determinante.
    Radial { cx: i64, cy: i64, a: i64, b: i64, c: i64, d: i64, det: i64 },
}

impl<'a> Pincel<'a> {
    pub fn de(t: &Tinta<'a>) -> Self {
        match *t {
            Tinta::Liso(c) => Pincel { forma: Forma::Liso, liso: c, paradas: &[] },
            Tinta::Lineal { de, a, paradas } => {
                let (dx, dy) = ((a.0 - de.0) as i64, (a.1 - de.1) as i64);
                let den = dx * dx + dy * dy;
                if den == 0 || paradas.is_empty() {
                    return Pincel { forma: Forma::Liso, liso: paradas.last().map_or(0, |p| p.c), paradas: &[] };
                }
                Pincel { forma: Forma::Lineal { x0: de.0 as i64, y0: de.1 as i64, dx, dy, den }, liso: 0, paradas }
            }
            Tinta::Radial { centro, eje_x, eje_y, paradas } => {
                let (ex, ey) = ((eje_x.0 as i64, eje_x.1 as i64), (eje_y.0 as i64, eje_y.1 as i64));
                let det = ex.0 * ey.1 - ey.0 * ex.1;
                if det == 0 || paradas.is_empty() {
                    return Pincel { forma: Forma::Liso, liso: paradas.last().map_or(0, |p| p.c), paradas: &[] };
                }
                // M = [[ex.0, ey.0], [ex.1, ey.1]]; M^-1 = 1/det [[ey.1, -ey.0], [-ex.1, ex.0]].
                Pincel { forma: Forma::Radial { cx: centro.0 as i64, cy: centro.1 as i64, a: ey.1, b: -ey.0, c: -ex.1, d: ex.0, det }, liso: 0, paradas }
            }
        }
    }

    /// **El color y el alfa** en el centro del pixel `(px, py)`.
    pub fn en(&self, px: i32, py: i32) -> (Color, u8) {
        let (x, y) = (px as i64 * 64 + 32, py as i64 * 64 + 32);
        let t = match self.forma {
            Forma::Liso => return (self.liso, 255),
            Forma::Lineal { x0, y0, dx, dy, den } => ((x - x0) * dx + (y - y0) * dy) * 1000 / den,
            Forma::Radial { cx, cy, a, b, c, d, det } => {
                let (ux, uy) = (x - cx, y - cy);
                let u = (a * ux + b * uy) * 1000 / det;
                let v = (c * ux + d * uy) * 1000 / det;
                crate::raiz((u * u + v * v) as u64) as i64
            }
        };
        en_paradas(self.paradas, t.clamp(0, 1000) as u32)
    }
}

/// El color de unas paradas en `t` (milesimas): `pad`, como SVG por defecto.
fn en_paradas(ps: &[Parada], t: u32) -> (Color, u8) {
    let Some(primera) = ps.first() else { return (0, 255) };
    if t <= primera.en as u32 {
        return (primera.c, primera.alfa);
    }
    for w in ps.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t <= b.en as u32 {
            let tramo = (b.en as u32).saturating_sub(a.en as u32);
            if tramo == 0 {
                return (b.c, b.alfa);
            }
            let k = (t - a.en as u32) * 256 / tramo;
            let alfa = (a.alfa as u32 * (256 - k) + b.alfa as u32 * k) / 256;
            return (entre(a.c, b.c, k), alfa as u8);
        }
    }
    let ultima = ps[ps.len() - 1];
    (ultima.c, ultima.alfa)
}

/// Pone `c` en `(x, y)` con la tinta `cubre` (0..=255) de la figura.
fn pon(l: &mut impl Lienzo, x: i32, y: i32, cubre: u32, pincel: &Pincel, alfa: u8) {
    let (c, a) = pincel.en(x, y);
    let total = cubre * a as u32 * alfa as u32 / (255 * 255);
    if total >= 255 {
        l.rect(x, y, 1, 1, c);
    } else if total > 0 {
        l.mezclar(x, y, c, total as u8);
    }
}

fn caja(cs: &impl Caminos, margen: i32) -> Option<(i32, i32, i32, i32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for s in 0..cs.cuantos() {
        for k in 0..cs.largo(s) {
            let (x, y) = cs.punto(s, k);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 > x1 {
        return None;
    }
    Some(((x0 - margen).div_euclid(64), (y0 - margen).div_euclid(64), (x1 + margen).div_euclid(64) + 1, (y1 + margen).div_euclid(64) + 1))
}

// ---------------------------------------------------------------------------
// El relleno, con su regla
// ---------------------------------------------------------------------------

/// Los cortes que caben en una sub-fila: el doble que el relleno de MAQUETA
/// 2, porque un contorno de pluma (S4) son muchas piezas pisandose.
const CORTES: usize = 256;

/// **Rellena una figura** con su regla, su tinta y su opacidad. Cuatro
/// sub-filas por pixel, y en cada una la parte EXACTA de cada pixel que la
/// figura cubre -- como `relleno`, mas el sentido de cada corte.
pub fn relleno(l: &mut impl Lienzo, cs: &impl Caminos, tinta: &Tinta, alfa: u8, par_impar: bool) {
    let Some((px0, py0, px1, py1)) = caja(cs, 0) else { return };
    let pincel = Pincel::de(tinta);
    for bloque in (px0..px1).step_by(64) {
        let fin = (bloque + 64).min(px1);
        for py in py0..py1 {
            let mut acum = [0u32; 64];
            for sub in 0..4 {
                let y = py * 64 + 8 + sub * 16;
                let mut cortes = [(0i32, 0i8); CORTES];
                let mut n = 0;
                for s in 0..cs.cuantos() {
                    let m = cs.largo(s);
                    if m < 2 {
                        continue;
                    }
                    for k in 0..m {
                        let (a, b) = (cs.punto(s, k), cs.punto(s, (k + 1) % m));
                        if (a.1 <= y) != (b.1 <= y) && n < CORTES {
                            let x = (a.0 as i64 + (b.0 - a.0) as i64 * (y - a.1) as i64 / (b.1 - a.1) as i64) as i32;
                            cortes[n] = (x, if b.1 > a.1 { 1 } else { -1 });
                            n += 1;
                        }
                    }
                }
                cortes[..n].sort_unstable_by_key(|c| c.0);
                let mut vueltas = 0i32;
                for i in 0..n {
                    vueltas += cortes[i].1 as i32;
                    let dentro = if par_impar { (i + 1) % 2 == 1 } else { vueltas != 0 };
                    if !dentro || i + 1 >= n {
                        continue;
                    }
                    let (a, b) = (cortes[i].0.max(bloque * 64), cortes[i + 1].0.min(fin * 64));
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
                if t > 0 {
                    pon(l, bloque + i as i32, py, (t * 255 / 256).min(255), &pincel, alfa);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// La pluma redonda, con tinta y opacidad
// ---------------------------------------------------------------------------

/// Los tramos que se miran de una fila antes de mirarlos todos.
const CANDIDATOS: usize = 256;

/// **Una pluma redonda** de `grosor64` por todos los subcaminos a la vez:
/// cada pixel se pinta UNA vez, con la distancia al tramo mas cercano de la
/// figura entera. Con opacidad eso importa -- dos subcaminos que se cruzan
/// no oscurecen el cruce, como no lo oscurece el navegador.
pub fn trazo(l: &mut impl Lienzo, cs: &impl Caminos, grosor64: i32, tinta: &Tinta, alfa: u8) {
    let r = (grosor64 / 2).max(16);
    let Some((px0, py0, px1, py1)) = caja(cs, r + 32) else { return };
    let pincel = Pincel::de(tinta);
    // El tramo `k` del subcamino `s` (con `k == n` el de cierre).
    let tramo = |s: usize, k: usize| -> ((i32, i32), (i32, i32)) {
        let n = cs.largo(s);
        let a = cs.punto(s, k);
        let b = if n == 1 { a } else { cs.punto(s, (k + 1) % n) };
        (a, b)
    };
    let tramos_de = |s: usize| -> usize {
        let n = cs.largo(s);
        if cs.cerrado(s) && n > 2 {
            n
        } else {
            n.saturating_sub(1).max(1)
        }
    };
    for py in py0..py1 {
        let cy = py * 64 + 32;
        // Los tramos que tocan esta fila.
        let mut cand = [(0u16, 0u16); CANDIDATOS];
        let mut nc = 0usize;
        let mut todos = false;
        'reunir: for s in 0..cs.cuantos() {
            if cs.largo(s) == 0 {
                continue;
            }
            for k in 0..tramos_de(s) {
                let (a, b) = tramo(s, k);
                if cy + r + 32 < a.1.min(b.1) || cy - r - 32 > a.1.max(b.1) {
                    continue;
                }
                if nc == CANDIDATOS || s > u16::MAX as usize || k > u16::MAX as usize {
                    todos = true;
                    break 'reunir;
                }
                cand[nc] = (s as u16, k as u16);
                nc += 1;
            }
        }
        if nc == 0 && !todos {
            continue;
        }
        for px in px0..px1 {
            let centro = (px * 64 + 32, cy);
            let mut d = i32::MAX;
            let mut mira = |a: (i32, i32), b: (i32, i32)| {
                if centro.0 + r + 32 < a.0.min(b.0) || centro.0 - r - 32 > a.0.max(b.0) || centro.1 + r + 32 < a.1.min(b.1) || centro.1 - r - 32 > a.1.max(b.1) {
                    return;
                }
                d = d.min(crate::distancia(centro, a, b));
            };
            if todos {
                for s in 0..cs.cuantos() {
                    if cs.largo(s) == 0 {
                        continue;
                    }
                    for k in 0..tramos_de(s) {
                        let (a, b) = tramo(s, k);
                        mira(a, b);
                    }
                }
            } else {
                for &(s, k) in &cand[..nc] {
                    let (a, b) = tramo(s as usize, k as usize);
                    mira(a, b);
                }
            }
            if d == i32::MAX {
                continue;
            }
            let t = (32 + r - d).clamp(0, 64) as u32;
            if t > 0 {
                pon(l, px, py, (t * 255 / 64).min(255), &pincel, alfa);
            }
        }
    }
}

/// **Pinta una figura**: pluma (`pluma > 0`, su grosor en 1/64) o relleno.
pub fn figura(l: &mut impl Lienzo, cs: &impl Caminos, pluma: i32, tinta: &Tinta, alfa: u8, par_impar: bool) {
    if alfa == 0 {
        return;
    }
    if pluma > 0 {
        trazo(l, cs, pluma, tinta, alfa);
    } else {
        relleno(l, cs, tinta, alfa, par_impar);
    }
}

/// **Una tinta a medio camino** entre `a` y `b`, `p` milesimas, con sus
/// paradas escritas en `aqui`. Si no son de la misma clase y con las mismas
/// paradas, la que toca: la de salida hasta la mitad.
pub fn tinta_entre<'a>(a: &Tinta<'a>, b: &Tinta<'a>, p: i32, aqui: &'a mut [Parada; PARADAS]) -> Tinta<'a> {
    use crate::{entre_color, entre_i};
    let pt = |x: (i32, i32), y: (i32, i32)| (entre_i(x.0, y.0, p), entre_i(x.1, y.1, p));
    let paradas = |pa: &[Parada], pb: &[Parada], aqui: &'a mut [Parada; PARADAS]| -> &'a [Parada] {
        let n = pa.len().min(pb.len()).min(PARADAS);
        for k in 0..n {
            aqui[k] = Parada {
                en: entre_i(pa[k].en as i32, pb[k].en as i32, p.clamp(0, 1000)).clamp(0, 1000) as u16,
                c: entre_color(pa[k].c, pb[k].c, p),
                alfa: entre_i(pa[k].alfa as i32, pb[k].alfa as i32, p.clamp(0, 1000)).clamp(0, 255) as u8,
            };
        }
        &aqui[..n]
    };
    match (*a, *b) {
        (Tinta::Liso(x), Tinta::Liso(y)) => Tinta::Liso(entre_color(x, y, p)),
        (Tinta::Lineal { de, a: h, paradas: pa }, Tinta::Lineal { de: de2, a: h2, paradas: pb }) if pa.len() == pb.len() => {
            Tinta::Lineal { de: pt(de, de2), a: pt(h, h2), paradas: paradas(pa, pb, aqui) }
        }
        (Tinta::Radial { centro, eje_x, eje_y, paradas: pa }, Tinta::Radial { centro: c2, eje_x: x2, eje_y: y2, paradas: pb }) if pa.len() == pb.len() => {
            Tinta::Radial { centro: pt(centro, c2), eje_x: pt(eje_x, x2), eje_y: pt(eje_y, y2), paradas: paradas(pa, pb, aqui) }
        }
        _ => {
            if p.clamp(0, 1000) >= 500 {
                *b
            } else {
                *a
            }
        }
    }
}
