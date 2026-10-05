//! **THE ICONS** -- what each thing a module DOES looks like on its node in the
//! GRAPH (the owner, 05-10: "formas de nodos unicos que representan ... el
//! nodo de imprimir, o logo de impresora, mas facil").
//!
//! ```text
//!    print     a PRINTER, with its sheet coming out    (the console)
//!    if        a DIAMOND: one way in, two out          (decide)
//!    for       a LOOP: an arrow that comes back        (repeat)
//!    type      a CRYSTAL: a value with facets          (records, enums)
//!    let       a BOX: a value with a name              (it stays)
//!    let mut   a RING that turns                       (it changes)
//!    call      an ARROW out                            (another fn)
//!    return    an ARROW that comes back carrying       (a result)
//!    main      PLAY: where the program starts
//!    the 3060  a CHIP with its pins
//! ```
//!
//! The same meaning and the same colour as in the SKY (`astros.rs`) and on
//! the cables: a colour SAYS one thing everywhere. Integer pixels only, drawn
//! from rectangles, lines and discs -- no font, no image, no stack.

use crate::aspecto::{self as look, ACCENT, AMBER, BG, CYAN, DIM, GOLD, GOOD, INK, LILAC, VIOLET};
use crate::canvas::Canvas;
use bmo_dibujo::{mezclar, Color, Lienzo};

/// What a node can show, one icon each.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Printer,
    Decide,
    Repeat,
    Crystal,
    Value,
    Change,
    Call,
    Return,
    Play,
    Chip,
}

impl Icon {
    /// Its colour: the class of the thing, fixed.
    pub fn color(self) -> Color {
        match self {
            Icon::Printer => GOOD,
            Icon::Decide => GOLD,
            Icon::Repeat => LILAC,
            Icon::Crystal => mezclar(VIOLET, INK, 1, 3),
            Icon::Value => DIM,
            Icon::Change => AMBER,
            Icon::Call | Icon::Return => CYAN,
            Icon::Play => ACCENT,
            Icon::Chip => GOOD,
        }
    }
}

/// Draws `icon` in an `s` x `s` square whose corner is (x, y).
pub fn draw(c: &mut Canvas, icon: Icon, x: i32, y: i32, s: i32) {
    let col = icon.color();
    let s = s.max(8);
    match icon {
        Icon::Printer => printer(c, x, y, s),
        Icon::Decide => {
            let (cx, cy, r) = (x + s / 2, y + s / 2, s / 2 - 1);
            for (a, b) in [((cx, cy - r), (cx + r, cy)), ((cx + r, cy), (cx, cy + r)), ((cx, cy + r), (cx - r, cy)), ((cx - r, cy), (cx, cy - r))] {
                c.line(a, b, col);
            }
            c.disc(cx, cy, (s / 7).max(1), col);
        }
        Icon::Repeat => {
            let (cx, cy, r) = (x + s / 2, y + s / 2, s / 2 - 2);
            crate::astros::arc(c, cx, cy, r, r, 6, 58, col);
            // the head of the arrow, at the top, pointing right
            let (hx, hy) = (cx + r / 3, cy - r);
            c.line((hx - 3, hy - 3), (hx, hy), col);
            c.line((hx - 3, hy + 3), (hx, hy), col);
        }
        Icon::Crystal => {
            let (cx, cy, r) = (x + s / 2, y + s / 2, s / 2 - 1);
            let pts = [(cx, cy - r), (cx + r * 7 / 8, cy - r / 2), (cx + r * 7 / 8, cy + r / 2), (cx, cy + r), (cx - r * 7 / 8, cy + r / 2), (cx - r * 7 / 8, cy - r / 2)];
            for k in 0..6 {
                c.line(pts[k], pts[(k + 1) % 6], col);
            }
            c.line(pts[0], (cx, cy), col);
            c.line(pts[2], (cx, cy), col);
            c.line(pts[4], (cx, cy), col);
        }
        Icon::Value => {
            look::edge(c, x + 1, y + 2, s - 2, s - 4, 3, 1, col);
            c.disc(x + s / 2, y + s / 2, (s / 6).max(1), col);
        }
        Icon::Change => {
            let (cx, cy, r) = (x + s / 2, y + s / 2, s / 2 - 1);
            crate::astros::arc(c, cx, cy, r, r / 2, 0, 64, col);
            c.disc(cx, cy, (s / 5).max(2), mezclar(col, BG, 1, 2));
            c.disc(cx + r, cy, 1, INK);
        }
        Icon::Call => {
            let my = y + s / 2;
            c.rect(x + 1, my, s - 3, 2, col);
            c.line((x + s - 6, my - 4), (x + s - 2, my), col);
            c.line((x + s - 6, my + 5), (x + s - 2, my + 1), col);
        }
        Icon::Return => {
            // a hook: down the right side, back to the left, with its head
            let (r, b) = (x + s - 3, y + s - 4);
            c.rect(r, y + 2, 2, b - y - 2, col);
            c.rect(x + 3, b, r - x - 1, 2, col);
            c.line((x + 7, b - 4), (x + 3, b), col);
            c.line((x + 7, b + 5), (x + 3, b + 1), col);
            c.disc(r + 1, y + 2, 2, GOLD);
        }
        Icon::Play => {
            // pointing right: tall at the left, a point at the right
            let (w, cy) = (s * 3 / 4, y + s / 2);
            for k in 0..w {
                let half = (w - k) * (s / 2 - 1) / w;
                c.rect(x + s / 6 + k, cy - half, 1, 2 * half + 1, col);
            }
        }
        Icon::Chip => {
            look::card(c, x + 3, y + 3, s - 6, s - 6, 2, mezclar(col, BG, 1, 2));
            look::edge(c, x + 3, y + 3, s - 6, s - 6, 2, 1, col);
            let mut k = x + 5;
            while k < x + s - 4 {
                c.rect(k, y, 1, 3, col);
                c.rect(k, y + s - 3, 1, 3, col);
                k += 3;
            }
        }
    }
}

/// The PRINTER: a sheet in, the body with its light, and the sheet that comes
/// out with two lines written on it.
fn printer(c: &mut Canvas, x: i32, y: i32, s: i32) {
    let paper = mezclar(INK, BG, 7, 8);
    let body = mezclar(GOOD, BG, 1, 2);
    // the sheet going in
    c.rect(x + s / 4, y, s / 2, s / 3, paper);
    // the body
    look::card(c, x, y + s / 4, s, s / 2, 2, body);
    look::edge(c, x, y + s / 4, s, s / 2, 2, 1, GOOD);
    c.disc(x + s - 4, y + s / 4 + 4, 1, GOOD);
    c.rect(x + s / 6, y + s / 2 + 1, s * 2 / 3, 1, BG);
    // the sheet coming out, written
    let (py, ph) = (y + s / 2 + 2, s / 2 - 2);
    c.rect(x + s / 4, py, s / 2, ph, paper);
    c.rect(x + s / 4 + 2, py + ph / 3, s / 2 - 4, 1, mezclar(GOOD, BG, 2, 3));
    c.rect(x + s / 4 + 2, py + ph * 2 / 3, s / 3 - 2, 1, mezclar(GOOD, BG, 2, 3));
}
