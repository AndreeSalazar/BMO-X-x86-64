//! **LAS CAPAS, EL RECORTE Y LAS ZONAS** (MAQUETA 3, pila A, 06-10, MA2).
//!
//! Lo que la pila A cambia de *que se dibuja*, en un solo recorrido del arbol,
//! para que los tres emisores (Rust, la CARA y la foto) lo lean del mismo sitio:
//!
//! ```text
//!    el orden      capa a capa (`z-index`), y dentro de una capa el del fichero
//!    el recorte    lo que un `overflow: hidden` de arriba deja ver de cada caja
//!                  (y el de la propia caja, para su texto y su dibujo)
//!    el puntero    `pointer-events: none` se HEREDA, como en CSS
//!    las zonas     `cursor`: que forma pide cada punto
//!    el contorno   `outline`, encima de todo
//! ```
//!
//! `LA_MAQUETA_EXIGE.md` 3e. Las reglas que hacen que esto sea lo mismo que
//! en el navegador (una capa nunca dentro de otra, la absoluta anclada dentro
//! de lo que la recorta) las pone el veredicto (M, P).

use bmo_maqueta_layout::{Frame, Laid, Rect};

use crate::orden::Trazo;

/// Lo de fuera del lienzo hasta donde se recorta un eje que NO recorta: lo
/// bastante para que nada visible quede fuera.
const LEJOS: i64 = 1 << 16;

/// Una caja con lo que la pila A le dice.
pub struct Caja<'a> {
    pub f: &'a Frame,
    /// Lo que recortan las de ARRIBA (para su fondo, su borde y su brillo).
    pub corte: Option<Rect>,
    /// Lo de arriba y lo suyo (para su texto y su dibujo, que son su contenido).
    pub corte_propio: Option<Rect>,
    /// `pointer-events: none`, suyo o heredado.
    pub sin_puntero: bool,
    pub capa: u32,
}

/// **Las cajas en orden de pintado**: capa a capa, estable.
pub fn cajas(l: &Laid) -> Vec<Caja<'_>> {
    let mut out = Vec::new();
    andar(&l.root, None, false, 0, &mut out);
    out.sort_by_key(|c| c.capa);
    out
}

fn andar<'a>(f: &'a Frame, corte: Option<(i64, i64, i64, i64)>, sin_puntero: bool, capa: u32, out: &mut Vec<Caja<'a>>) {
    let sin_puntero = f.style.sin_puntero.unwrap_or(sin_puntero);
    let capa = f.style.capa.unwrap_or(capa);
    let propio = match (corte, corte_de(f)) {
        (Some(a), Some(b)) => Some((a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3))),
        (a, b) => a.or(b),
    };
    out.push(Caja { f, corte: corte.map(a_rect), corte_propio: propio.map(a_rect), sin_puntero, capa });
    for c in &f.children {
        andar(c, propio, sin_puntero, capa, out);
    }
}

/// Por dentro del borde, en los ejes que la caja recorta.
fn corte_de(f: &Frame) -> Option<(i64, i64, i64, i64)> {
    let [rx, ry] = [f.style.desborde[0].recorta(), f.style.desborde[1].recorta()];
    if !rx && !ry {
        return None;
    }
    let [t, r, b, l] = f.style.border_width;
    Some((
        if rx { f.rect.x as i64 + l as i64 } else { -LEJOS },
        if ry { f.rect.y as i64 + t as i64 } else { -LEJOS },
        if rx { f.rect.right() - r as i64 } else { LEJOS },
        if ry { f.rect.bottom() - b as i64 } else { LEJOS },
    ))
}

fn a_rect(c: (i64, i64, i64, i64)) -> Rect {
    Rect { x: c.0 as i32, y: c.1 as i32, w: (c.2 - c.0).max(0) as u32, h: (c.3 - c.1).max(0) as u32 }
}

/// **Que hace un recorte con un trazo**: `None` si no se ve nada; si cae
/// entero dentro, el mismo trazo sin recorte; un rect, cortado exacto; lo
/// demas, con su recorte para el pintor.
pub fn recortar(t: Trazo, corte: Option<Rect>) -> Option<(Trazo, Option<Rect>)> {
    let Some(c) = corte else { return Some((t, None)) };
    let a = t.area();
    let (x0, y0) = (a.x.max(c.x), a.y.max(c.y));
    let (x1, y1) = (a.right().min(c.right()), a.bottom().min(c.bottom()));
    if x0 as i64 >= x1 || y0 as i64 >= y1 {
        return None;
    }
    if a.inside(&c) {
        return Some((t, None));
    }
    match t {
        Trazo::Rect { color, .. } => Some((Trazo::Rect { r: Rect { x: x0, y: y0, w: (x1 - x0 as i64) as u32, h: (y1 - y0 as i64) as u32 }, color }, None)),
        otro => Some((otro, Some(c))),
    }
}

/// **El contorno** (`outline`): un anillo fuera del borde que no ocupa sitio.
/// Sin radio, cuatro rects (los pixeles de CSS); con radio, el borde suave
/// que sigue la curva -- el radio mas lo que se separa.
pub fn contorno(f: &Frame) -> Vec<Trazo> {
    let Some((w, color)) = f.style.contorno else { return Vec::new() };
    let d = f.style.contorno_aparte + w as i32;
    let r = Rect { x: f.rect.x - d, y: f.rect.y - d, w: (f.rect.w as i64 + 2 * d as i64).max(0) as u32, h: (f.rect.h as i64 + 2 * d as i64).max(0) as u32 };
    let radio = f.style.border_radius as i32 + f.style.contorno_aparte;
    if f.style.border_radius > 0 && radio > 0 {
        return vec![Trazo::Borde { r, radio: (radio + w as i32) as u32, grosor: w, color }];
    }
    let lado = r.h.saturating_sub(2 * w);
    vec![
        Trazo::Rect { r: Rect { x: r.x, y: r.y, w: r.w, h: w }, color },
        Trazo::Rect { r: Rect { x: r.x, y: r.y + r.h as i32 - w as i32, w: r.w, h: w }, color },
        Trazo::Rect { r: Rect { x: r.x, y: r.y + w as i32, w, h: lado }, color },
        Trazo::Rect { r: Rect { x: r.x + r.w as i32 - w as i32, y: r.y + w as i32, w, h: lado }, color },
    ]
}

/// **Las zonas del puntero**: las cajas que dijeron `cursor` (y que el puntero
/// alcanza), en orden de pintado. La de mas encima que contiene el punto es
/// la que manda, como cuando CSS lo hereda a lo de dentro.
pub fn zonas(l: &Laid) -> Vec<(Rect, &'static str)> {
    cajas(l)
        .into_iter()
        .filter(|c| !c.sin_puntero)
        .filter_map(|c| c.f.style.cursor.map(|k| (c.f.rect, k.name())))
        .collect()
}
