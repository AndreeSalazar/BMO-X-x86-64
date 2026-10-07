//! **Where does each box go?** Top-down, and this is where boxes finally see
//! each other -- which is the son's product being *related*, so it is the
//! grandson's whole reason to exist.
//!
//! ## The two rules that answer "what is an undeclared size"
//!
//! The son recorded `None` rather than inventing a number, because the answer
//! depends on the neighbours. Here are the neighbours, so here is the answer:
//!
//! ```text
//!    block child, no width      -> the container's content width   (fills)
//!    flex item,   no main size  -> what its content needs          (shrinks)
//!    flex item,   no cross size -> the container's, if `stretch`
//! ```
//!
//! All three are CSS's, and that is the point: the answers had to be *somebody's*
//! and they had to be the browser's.
//!
//! ## Free space is signed, and stays signed
//!
//! `content - used` goes negative the moment something does not fit, and a box
//! centred inside a smaller one lands at a negative coordinate -- in a browser
//! too. Clamping it to zero here would hide an overflow that `verdict/` is
//! supposed to catch, so positions are `i32` and nothing saturates.
//!
//! It is the same trap the rasterizer wrote down: with the wrong width an
//! overflow flips the SIGN and the shape comes out inside out.

use bmo_maqueta_cascade::{Align, AlignContent, Direction, Display, Justify, Position, Styled, TextAlign};

use crate::measure::{alto, ancho, content_size, filas, frame, outer_size, por_proporcion};
use crate::{Frame, Rect};

/// Lay one box out inside the border box its parent decided for it. La raiz
/// es su propia ancla (el lienzo).
pub fn place(b: &Styled, border: Rect) -> Frame {
    colocar(b, border, border)
}

/// Como [`place`], con el ANCLA de las absolutas de dentro (H5): la caja de
/// relleno de la `position:relative` mas cercana por arriba, o el lienzo.
fn colocar(b: &Styled, border: Rect, ancla: Rect) -> Frame {
    let inset = b.style.border_width[3] as i32 + b.style.padding[3] as i32;
    let inset_top = b.style.border_width[0] as i32 + b.style.padding[0] as i32;
    let (fw, fh) = frame(b);
    let content = Rect {
        x: border.x + inset,
        y: border.y + inset_top,
        w: border.w.saturating_sub(fw),
        h: border.h.saturating_sub(fh),
    };

    let flow: Vec<&Styled> = b
        .children
        .iter()
        .filter(|c| c.style.position != Position::Absolute)
        .collect();
    // Una caja POSICIONADA (`relative`, o `absolute`) es el ancla de lo de
    // dentro: su caja de RELLENO, como en CSS (dentro del borde, con el
    // `padding`).
    let ancla = if b.style.position != Position::Static {
        let [t, r, bo, l] = b.style.border_width;
        Rect { x: border.x + l as i32, y: border.y + t as i32, w: border.w.saturating_sub(l + r), h: border.h.saturating_sub(t + bo) }
    } else {
        ancla
    };

    // Un `<svg>` es una hoja: sus `<path>` caen todos en su caja y su
    // `viewBox` dice el resto (MAQUETA 2).
    let mut placed: Vec<Frame> = if b.tag == bmo_maqueta_node::Tag::Svg {
        b.children.iter().map(|c| colocar(c, content, ancla)).collect()
    } else {
        match b.style.display {
            Display::Flex => flex(b, &flow, content, ancla),
            Display::Block => block(&flow, content, ancla),
        }
    };

    // * Absolutely positioned boxes are placed against their ANCHOR, as in
    // CSS: the padding box of the nearest `position:relative` ancestor (H5,
    // 04-10), or the canvas if there is none -- which was the only case before
    // `relative` existed. `right` and `bottom` count from the far edges.
    for c in b.children.iter().filter(|c| c.style.position == Position::Absolute && b.tag != bmo_maqueta_node::Tag::Svg) {
        let o = outer_size(c);
        let s = &c.style;
        // Con signo desde MA2: la insignia que se sale de la esquina.
        let x = match (s.left, s.right) {
            (Some(l), _) => ancla.x + l,
            (None, Some(r)) => ancla.x + ancla.w as i32 - r - o.w as i32,
            (None, None) => ancla.x,
        };
        let y = match (s.top, s.bottom) {
            (Some(t), _) => ancla.y + t,
            (None, Some(bo)) => ancla.y + ancla.h as i32 - bo - o.h as i32,
            (None, None) => ancla.y,
        };
        placed.push(colocar(c, Rect { x, y, w: o.w, h: o.h }, ancla));
    }

    // `border-radius: 50%` (H6): ahora que la caja tiene medida.
    let mut style = b.style;
    let mut hover = b.hover;
    if style.radio_mitad {
        style.border_radius = border.w.min(border.h) / 2;
    }
    if let Some(h) = hover.as_mut().filter(|h| h.radio_mitad) {
        h.border_radius = border.w.min(border.h) / 2;
    }

    // `text-overflow: ellipsis` (MA2): con la caja ya medida, la linea que no
    // cabe se corta AL COMPILAR. Solo con el recorte en la misma caja y en una
    // linea, como en CSS (lo demas lo dice el veredicto, N).
    let text = match &b.text {
        Some(t) if style.puntos && style.desborde[0].recorta() && !style.parrafo => Some(crate::measure::con_puntos(&style, t, content.w)),
        otro => otro.clone(),
    };
    let text_at = text.as_deref().map(|t| donde_el_texto(b, t, content));

    Frame {
        tag: b.tag,
        id: b.id.clone(),
        island: b.island.clone(),
        text,
        dibujo: b.dibujo.clone(),
        src: b.src.clone(),
        repite: b.repite,
        hueco: b.hueco.clone(),
        imagen: None,
        style,
        hover,
        rect: border,
        content,
        text_at,
        children: placed,
        span: b.span,
    }
}

/// Where the glyphs start.
///
/// * In a `block` box the text sits at the top left of the content, which is
/// what a browser does for left-to-right text. In a `flex` box the text is an
/// **anonymous flex item** -- a real CSS concept, not an invention -- so
/// `justify-content` and `align-items` move it, and that is how a label gets
/// centred in its button.
///
/// This is here and not in the emitter on purpose: centring is arithmetic, and
/// arithmetic in a consumer is arithmetic nobody checks. `calc.rs` writes
/// `bx + CALC_BTN/2 - GLIFO_ANCHO/2` by hand, once per label.
fn donde_el_texto(b: &Styled, t: &str, content: Rect) -> Rect {
    let (w, h) = crate::measure::texto(&b.style, t);
    if b.style.display != Display::Flex {
        // `text-align` (MAQUETA 3): el texto ya se midio, asi que colocarlo
        // dentro de su caja es una resta. Con signo, como todo aqui: un texto
        // que no cabe y se centra cae a la izquierda del contenido, como en el
        // navegador, y el veredicto (B) lo caza.
        let libre = content.w as i64 - w as i64;
        let dx = match b.style.text_align {
            TextAlign::Left => 0,
            TextAlign::Center => libre / 2,
            TextAlign::Right => libre,
        };
        return Rect { x: (content.x as i64 + dx) as i32, y: content.y, w, h };
    }
    let (libre_main, libre_cross, horizontal) = if b.style.direction == Direction::Row {
        (content.w as i64 - w as i64, content.h as i64 - h as i64, true)
    } else {
        (content.h as i64 - h as i64, content.w as i64 - w as i64, false)
    };
    let main = match b.style.justify {
        Justify::Start | Justify::SpaceBetween => 0,
        Justify::Center => libre_main / 2,
        Justify::End => libre_main,
    };
    let cross = match b.style.align {
        Align::Stretch | Align::Start => 0,
        Align::Center => libre_cross / 2,
        Align::End => libre_cross,
    };
    let (dx, dy) = if horizontal { (main, cross) } else { (cross, main) };
    Rect {
        x: (content.x as i64 + dx) as i32,
        y: (content.y as i64 + dy) as i32,
        w,
        h,
    }
}

/// Children stack downwards, each filling the width unless it named one.
fn block(flow: &[&Styled], content: Rect, ancla: Rect) -> Vec<Frame> {
    let mut y = content.y;
    let mut out = Vec::with_capacity(flow.len());
    for c in flow {
        let (fw, _) = frame(c);
        // Sin `width` llena a su padre -- y sus cotas (MAQUETA 3) acotan lo que
        // llena, como en CSS: `max-width` es justo eso.
        let w = match c.style.width {
            Some(_) => content_size(c).w + fw,
            None => ancho(&c.style, content.w.saturating_sub(fw)) + fw,
        };
        // `aspect-ratio` (MA2): sin ancho ni alto dichos, el alto sale del
        // ancho que LLENA, como en CSS.
        let h = match (c.style.width, c.style.height, c.style.proporcion) {
            (None, None, Some((pa, pb))) => alto(&c.style, por_proporcion(w - fw, pb, pa)) + frame(c).1,
            _ => outer_size(c).h,
        };
        out.push(colocar(c, Rect { x: content.x, y, w, h }, ancla));
        y += h as i32;
    }
    out
}

/// ** `flex-wrap: wrap` (MA2): las filas se parten contra el eje principal
/// del contenido, y cada fila se coloca como una caja flex de una linea, en
/// su franja. Las franjas se reparten con `align-content` (`normal` =
/// `stretch`: lo que sobra se reparte entre las filas, como en CSS).
fn flex(b: &Styled, flow: &[&Styled], content: Rect, ancla: Rect) -> Vec<Frame> {
    if !b.style.parte || flow.is_empty() {
        return linea(b, flow, content, ancla);
    }
    let row = b.style.direction == Direction::Row;
    let gap = b.style.gap;
    let outers: Vec<_> = flow.iter().map(|c| outer_size(c)).collect();
    let largos: Vec<u32> = outers.iter().map(|s| if row { s.w } else { s.h }).collect();
    let fs = filas(&largos, if row { content.w } else { content.h }, gap);
    let mut cruces: Vec<u32> = fs.iter().map(|f| outers[f.clone()].iter().map(|s| if row { s.h } else { s.w }).max().unwrap_or(0)).collect();
    let sitio = if row { content.h } else { content.w } as i64;
    let usado: i64 = cruces.iter().map(|&c| c as i64).sum::<i64>() + gap as i64 * (fs.len() as i64 - 1);
    let libre = sitio - usado;
    let n = fs.len() as i64;
    let (mut pos, entre) = match b.style.align_content.unwrap_or_default() {
        AlignContent::Stretch => {
            if libre > 0 {
                for c in cruces.iter_mut() {
                    *c += (libre / n) as u32;
                }
            }
            (0, 0)
        }
        AlignContent::Start => (0, 0),
        AlignContent::Center => (libre / 2, 0),
        AlignContent::End => (libre, 0),
        AlignContent::SpaceBetween => (0, if n > 1 && libre > 0 { libre / (n - 1) } else { 0 }),
    };
    let mut out = Vec::with_capacity(flow.len());
    for (f, c) in fs.iter().zip(&cruces) {
        let franja = if row {
            Rect { x: content.x, y: content.y + pos as i32, w: content.w, h: *c }
        } else {
            Rect { x: content.x + pos as i32, y: content.y, w: *c, h: content.h }
        };
        out.extend(linea(b, &flow[f.clone()], franja, ancla));
        pos += *c as i64 + gap as i64 + entre;
    }
    out
}

/// Una linea flex: los hijos en fila (o columna) dentro de `content`.
fn linea(b: &Styled, flow: &[&Styled], content: Rect, ancla: Rect) -> Vec<Frame> {
    if flow.is_empty() {
        return Vec::new();
    }
    let row = b.style.direction == Direction::Row;
    let gap = b.style.gap;

    let outers: Vec<_> = flow.iter().map(|c| outer_size(c)).collect();
    let used: u32 = outers
        .iter()
        .map(|s| if row { s.w } else { s.h })
        .sum::<u32>()
        + gap * (flow.len() as u32 - 1);

    let room = if row { content.w } else { content.h };
    let free = room as i64 - used as i64;

    // Signed on purpose: `Center` with more content than room is a negative
    // offset in a browser too, and pretending otherwise would hide the overflow.
    let (lead, between) = match b.style.justify {
        Justify::Start => (0i64, 0i64),
        Justify::Center => (free / 2, 0),
        Justify::End => (free, 0),
        Justify::SpaceBetween => {
            if flow.len() > 1 && free > 0 {
                (0, free / (flow.len() as i64 - 1))
            } else {
                (0, 0)
            }
        }
    };

    let mut main = if row { content.x } else { content.y } as i64 + lead;
    let mut out = Vec::with_capacity(flow.len());

    for (c, o) in flow.iter().zip(&outers) {
        let (fw, fh) = frame(c);
        // `align-self` (MA2): el `align-items` de este hijo.
        let al = c.style.align_self.unwrap_or(b.style.align);
        let (main_len, cross_len) = if row {
            let cross = match (c.style.height, al) {
                (Some(_), _) => content_size(c).h + fh,
                (None, Align::Stretch) => alto(&c.style, content.h.saturating_sub(fh)) + fh,
                (None, _) => o.h,
            };
            (o.w, cross)
        } else {
            let cross = match (c.style.width, al) {
                (Some(_), _) => content_size(c).w + fw,
                (None, Align::Stretch) => ancho(&c.style, content.w.saturating_sub(fw)) + fw,
                (None, _) => o.w,
            };
            (o.h, cross)
        };

        let room_cross = if row { content.h } else { content.w };
        let cross_start = if row { content.y } else { content.x } as i64;
        let cross_pos = match al {
            Align::Stretch | Align::Start => cross_start,
            Align::Center => cross_start + (room_cross as i64 - cross_len as i64) / 2,
            Align::End => cross_start + room_cross as i64 - cross_len as i64,
        };

        let r = if row {
            Rect {
                x: main as i32,
                y: cross_pos as i32,
                w: main_len,
                h: cross_len,
            }
        } else {
            Rect {
                x: cross_pos as i32,
                y: main as i32,
                w: cross_len,
                h: main_len,
            }
        };
        out.push(colocar(c, r, ancla));
        main += main_len as i64 + gap as i64 + between;
    }
    out
}

/// The size the root ends up with: what it declared, or what its tree needs.
pub fn canvas_of(root: &Styled) -> (u32, u32) {
    match root.canvas {
        Some((w, h)) => (w, h),
        None => {
            let c = content_size(root);
            let (fw, fh) = frame(root);
            (c.w + fw, c.h + fh)
        }
    }
}
