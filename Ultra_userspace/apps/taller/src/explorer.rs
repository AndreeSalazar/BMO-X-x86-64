//! **THE EXPLORER** -- the left column: the library, the files of the package
//! on screen, and its problems, straight from ESTRATOS (`PLAN_TALLER` 8.6).
//!
//! ```text
//!    EXPLORER
//!    ESTRATOS gen 1234         where it comes from, and at which commit
//!    BIBLIOTECA                titan/biblioteca.toml: click to open one
//!      > asteroids
//!    ARCHIVOS                  what the reader followed, as a tree
//!      Titan.toml
//!        main.titan            click: that node lights up in the canvas
//!        physics.titan
//!          collide.titan
//!    PROBLEMAS (1)             what the reader could not follow, and why
//! ```
//!
//! Inspired by the explorers of Windows and VS Code for their SIMPLICITY
//! (the owner, 18-08: "me inspiro... pero no soy Windows"), not copied: there
//! are no buttons that do nothing, and what it lists is what the code uses --
//! not a folder listing (see `store.rs`: the kernel's cursor is F12's).
//!
//! ONE function lays the rows out (`rows`), and both drawing and clicking walk
//! it: a click always lands on the row that was drawn there.

use crate::canvas::Canvas;
use crate::store::{Origin, Store};
use crate::view::{Buf, ACCENT, BAD, BAR, DIM, EDGE, GOOD, INK, LEFT, TITLE, TOP};
use bmo_dibujo::{Color, Lienzo};
use bmo_titan_contrato::NodeId;

const ROW: i32 = 18;
const PAD: i32 = 10;
/// Characters that fit in the column.
const CHARS: usize = ((LEFT - 2 * PAD) / 8) as usize;

/// What a click in the column asks for.
pub enum Click {
    Package(usize),
    File(NodeId),
}

#[derive(Clone, Copy)]
enum Row {
    Heading(&'static str),
    Origin,
    Package(usize),
    File(usize),
    /// Problem `i`, its line `part` (a message takes up to three).
    Problem(usize, usize),
    More,
}

/// Walks the rows top to bottom: `f(y, height, row)`.
fn rows(store: &Store, mut f: impl FnMut(i32, i32, Row)) {
    let mut y = TOP + 8;
    let mut put = |h: i32, r: Row| {
        f(y, h, r);
        y += h;
    };
    put(24, Row::Heading("EXPLORER"));
    put(ROW * origin_lines(store) as i32 + 8, Row::Origin);
    put(22, Row::Heading("BIBLIOTECA"));
    for i in 0..store.packages().len() {
        put(ROW, Row::Package(i));
    }
    put(8, Row::Heading(""));
    put(22, Row::Heading("ARCHIVOS"));
    for i in 0..store.loaded.files().len() {
        put(ROW, Row::File(i));
    }
    let problems = store.loaded.problems();
    if !problems.is_empty() {
        put(8, Row::Heading(""));
        put(22, Row::Heading("PROBLEMAS"));
        for (i, p) in problems.iter().enumerate() {
            let text = p.describe();
            for part in 0..pieces(text.as_bytes()).count() {
                put(ROW, Row::Problem(i, part));
            }
            put(4, Row::Heading(""));
        }
        if store.loaded.more_problems > 0 {
            put(ROW, Row::More);
        }
    }
}

/// A text cut at spaces into lines of the column, at most three.
fn pieces(s: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut rest = s;
    core::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let cut = if rest.len() <= CHARS {
            rest.len()
        } else {
            rest[..=CHARS].iter().rposition(|&c| c == b' ').filter(|&i| i > 0).unwrap_or(CHARS)
        };
        let (line, tail) = rest.split_at(cut);
        rest = tail.strip_prefix(b" ").unwrap_or(tail);
        Some(line)
    })
    .take(3)
}

fn origin_text(store: &Store) -> (Buf, Color) {
    let mut t = Buf::new();
    match store.origin {
        Origin::Estratos(g) => {
            t.s("ESTRATOS  gen ").num(g as u32);
            (t, GOOD)
        }
        Origin::Memory(why) => {
            t.s(why);
            (t, DIM)
        }
    }
}

fn origin_lines(store: &Store) -> usize {
    pieces(origin_text(store).0.get()).count().max(1)
}

pub fn draw(c: &mut Canvas, store: &Store, selected: Option<NodeId>) {
    c.rect(0, TOP, LEFT, c.h - TOP, BAR);
    c.rect(LEFT - 1, TOP, 1, c.h - TOP, EDGE);
    let files = store.loaded.files();
    rows(store, |y, _h, row| match row {
        Row::Heading("EXPLORER") => {
            c.text(PAD, y, b"EXPLORER", TITLE, 1);
        }
        Row::Heading(label) => {
            c.text(PAD, y + 4, label.as_bytes(), DIM, 1);
        }
        Row::Origin => {
            let (t, color) = origin_text(store);
            for (k, line) in pieces(t.get()).enumerate() {
                c.text(PAD, y + k as i32 * ROW, line, color, 1);
            }
        }
        Row::Package(i) => {
            let chosen = i == store.chosen;
            if chosen {
                c.rect(0, y - 1, LEFT - 1, ROW, EDGE);
            }
            let mut t = Buf::new();
            t.s(if chosen { "> " } else { "  " }).b(store.packages()[i].0.as_bytes());
            c.text_fit(PAD, y, t.get(), if chosen { ACCENT } else { INK }, LEFT - 2 * PAD);
        }
        Row::File(i) => {
            let f = &files[i];
            let path = f.path.as_bytes();
            // A tree without folder rows: one step in per `/`, and the last name.
            let depth = path.iter().filter(|&&b| b == b'/').count() as i32;
            let name = path.rsplit(|&b| b == b'/').next().unwrap_or(path);
            let lit = selected == Some(f.node);
            if lit {
                c.rect(0, y - 1, LEFT - 1, ROW, EDGE);
            }
            let x = PAD + 8 + depth * 12;
            c.text_fit(x, y, name, if lit { ACCENT } else { INK }, LEFT - x - PAD);
        }
        Row::Problem(i, part) => {
            let text = store.loaded.problems()[i].describe();
            if let Some(line) = pieces(text.as_bytes()).nth(part) {
                c.text(PAD + if part == 0 { 0 } else { 8 }, y, line, BAD, 1);
            };
        }
        Row::More => {
            let mut t = Buf::new();
            t.s("y ").num(store.loaded.more_problems as u32).s(" mas");
            c.text(PAD, y, t.get(), BAD, 1);
        }
    });
}

/// What the column has at (x, y), if it is something to click.
pub fn click(store: &Store, x: i32, y: i32) -> Option<Click> {
    if x >= LEFT {
        return None;
    }
    let mut hit = None;
    rows(store, |ry, h, row| {
        if y >= ry && y < ry + h {
            hit = match row {
                Row::Package(i) => Some(Click::Package(i)),
                Row::File(i) => store.loaded.files().get(i).map(|f| Click::File(f.node)),
                _ => None,
            };
        }
    });
    hit
}
