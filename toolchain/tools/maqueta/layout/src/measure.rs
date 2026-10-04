//! **How big does this box want to be?** Bottom-up, and it asks nobody above.
//!
//! Two sizes per box, and the difference is the CSS box model:
//!
//! ```text
//!    content    what `width`/`height` name -- CSS's default `content-box`
//!    outer      content + padding + border on all four sides
//! ```
//!
//! `content-box` is the confusing one of the two, and it is chosen anyway,
//! because it is what a browser does when nobody says otherwise. Fidelity of the
//! preview beats convenience of the author -- that trade is the whole reason the
//! tags are HTML's in the first place.

use bmo_maqueta_cascade::{Direction, Display, Position, Style, Styled};
use bmo_maqueta_node::Tag;

use crate::{GLIFO_ALTO, GLIFO_ANCHO};

/// **La letra de una caja**, si dijo `font-size` (MAQUETA 2): la de la casa,
/// la MISMA que la pinta en el aparato. Sin `font-size`, la de 8 x 16.
pub fn estilo(s: &Style) -> Option<bmo_letra::Estilo> {
    let px = u8::try_from(s.font_size?).ok()?;
    let e = match s.font_weight {
        500 => bmo_letra::Estilo::media(px),
        600 | 700 => bmo_letra::Estilo::negrita(px),
        _ => bmo_letra::Estilo::normal(px),
    };
    let e = e.espaciado(s.letter_spacing);
    Some(if s.uppercase { e.mayusculas() } else { e })
}

thread_local! {
    /// La cache de la letra del compilador. Medir es rasterizar una vez cada
    /// glifo: con la cache, cada letra se mide UNA vez por compilacion.
    static LETRA: core::cell::RefCell<bmo_letra::Letra> = core::cell::RefCell::new(bmo_letra::Letra::nueva());
}

/// **Lo que mide el texto de una caja**: con la letra de la casa (su ancho,
/// medido glifo a glifo, y la altura de linea: `line-height` o la normal), o
/// con la de 8 x 16 (`len * 8`, 16) si la caja no dijo `font-size`.
///
/// ** Antes esto era SOLO `len * 8`, y su cabecera decia que esa aritmetica
/// era "el cimiento" del compilador. Lo sigue siendo, con otra forma: la letra
/// nueva tambien se mide en el ANFITRION, al compilar, y con el mismo codigo
/// que la pinta. El aparato no mide nada.
pub fn texto(s: &Style, t: &str) -> (u32, u32) {
    match estilo(s) {
        Some(e) => {
            use bmo_letra::Fuente;
            let h = s.line_height.unwrap_or(bmo_letra::alto_normal(e.px) as u32);
            let ls = lineas(s, t);
            let w = ls.iter().map(|l| LETRA.with(|f| f.borrow_mut().medir(l.as_bytes(), e)).max(0) as u32).max().unwrap_or(0);
            (w, h * ls.len().max(1) as u32)
        }
        None => (t.len() as u32 * GLIFO_ANCHO, GLIFO_ALTO),
    }
}

/// **Las lineas de un texto** (H3). Un parrafo (`white-space: normal`, con
/// `width` y `font-size`) se parte por los espacios en lineas que caben en
/// su `width`, con la MISMA letra que lo pinta: la ultima palabra que no cabe
/// baja a la siguiente, como en el navegador. Cualquier otro texto es UNA
/// linea. Lo usan la maquetacion (cuanto mide) y el emisor (que pinta).
///
/// [!] Una palabra mas larga que el ancho se queda entera en su linea, y el
/// veredicto (B) la caza: no se parten palabras.
pub fn lineas(s: &Style, t: &str) -> Vec<String> {
    let (Some(e), Some(ancho), true) = (estilo(s), s.width, s.parrafo) else {
        return vec![t.to_string()];
    };
    use bmo_letra::Fuente;
    let mide = |x: &str| LETRA.with(|f| f.borrow_mut().medir(x.as_bytes(), e)).max(0) as u32;
    let mut out: Vec<String> = Vec::new();
    let mut linea = String::new();
    for palabra in t.split_whitespace() {
        let prueba = if linea.is_empty() { palabra.to_string() } else { format!("{linea} {palabra}") };
        if linea.is_empty() || mide(&prueba) <= ancho {
            linea = prueba;
        } else {
            out.push(std::mem::take(&mut linea));
            linea = palabra.to_string();
        }
    }
    if !linea.is_empty() || out.is_empty() {
        out.push(linea);
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

/// Padding plus border, horizontally and vertically.
pub fn frame(b: &Styled) -> (u32, u32) {
    let p = b.style.padding;
    let d = b.style.border_width;
    (p[1] + p[3] + d[1] + d[3], p[0] + p[2] + d[0] + d[2])
}

/// The content size this box settles on: what it declared, or what it needs.
pub fn content_size(b: &Styled) -> Size {
    let want = intrinsic(b);
    Size {
        w: b.style.width.unwrap_or(want.w),
        h: b.style.height.unwrap_or(want.h),
    }
}

/// The border box: what the parent has to make room for.
pub fn outer_size(b: &Styled) -> Size {
    let c = content_size(b);
    let (fw, fh) = frame(b);
    Size {
        w: c.w + fw,
        h: c.h + fh,
    }
}

/// What the box needs if nothing constrains it -- CSS's `max-content`.
fn intrinsic(b: &Styled) -> Size {
    if let Some(t) = &b.text {
        // * The measurement that makes this whole compiler possible: text is
        // measured HERE, on the host, so the device never lays anything out.
        // See `texto`.
        let (w, h) = texto(&b.style, t);
        return Size { w, h };
    }
    // A drawing is as big as it says: its paths do not push.
    if b.tag == Tag::Svg {
        return Size { w: 0, h: 0 };
    }

    // Absolutely positioned children are out of the flow, so they contribute
    // nothing to what their parent needs -- same as CSS.
    let flow: Vec<&Styled> = b
        .children
        .iter()
        .filter(|c| c.style.position != Position::Absolute)
        .collect();

    if flow.is_empty() {
        return Size { w: 0, h: 0 };
    }

    let sizes: Vec<Size> = flow.iter().map(|c| outer_size(c)).collect();
    let gaps = b.style.gap * (flow.len() as u32 - 1);

    match (b.style.display, b.style.direction) {
        (Display::Flex, Direction::Row) => Size {
            w: sizes.iter().map(|s| s.w).sum::<u32>() + gaps,
            h: sizes.iter().map(|s| s.h).max().unwrap_or(0),
        },
        (Display::Flex, Direction::Column) => Size {
            w: sizes.iter().map(|s| s.w).max().unwrap_or(0),
            h: sizes.iter().map(|s| s.h).sum::<u32>() + gaps,
        },
        // Block: children stack, each as wide as it needs. `gap` does nothing
        // here, exactly as in CSS, and `verdict/` is where saying so belongs.
        (Display::Block, _) => Size {
            w: sizes.iter().map(|s| s.w).max().unwrap_or(0),
            h: sizes.iter().map(|s| s.h).sum::<u32>(),
        },
    }
}
