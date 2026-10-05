//! **THE TAB** -- the master nodes, the Houdini and Blender way
//! (`docs/plan/PLAN_TALLER.md` 8.15; the owner, 05-10: "las TAB elegante ...
//! opciones totales de NODOS maestro ya hechos como ejemplos y porque, como
//! tutoriales y pruebas").
//!
//! ```text
//!    TAB         opens over the GRAPH (and the SKY): a soft box, a search
//!                field on top; Esc or TAB close it
//!    the list    on the left, by FAMILY (the level: the TAB, in order, IS
//!                the course); each row, the node and what it does
//!    the card    on the right, the picked node: its astro ALIVE, its why,
//!                the words it brings and its proof (`# sale:`)
//!    typing      filters by name, family, why or word: "match", "dinero"
//!    Enter       puts it in the package, where the mouse was
//! ```
//!
//! Only the LOOK and the keys live here: what a master IS, the filter and
//! the placing are `bmo_titan_lector::maestros` (logic, no colour), and the
//! masters themselves are the bench of TITAN++ (`maestros_gen.rs`).
//!
//! [!] Stack: the table is a `static` (`.rodata`); this keeps the query, the
//! pick and one row array of ~40 bytes. `pila.py --ring3` measures it.

use crate::aspecto::{self as look, ACCENT, AMBER, BG, CYAN, DIM, EDGE, GOLD, GOOD, INK, LEFT, LILAC, PANEL, TAB_BOX, TAB_EDGE, TAB_PICK, TAB_ROW, TITLE, TOP, VIOLET};
use crate::astros;
use crate::canvas::Canvas;
use crate::guia::lines;
use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_titan_contrato::Permission;
use bmo_titan_lector::maestros::{filter, Master, COUNT, FAMILIES, MASTERS};

const QUERY: usize = 24;
const ROW: i32 = 18;
const LIST_W: i32 = 330;

/// The TAB while it is open.
pub struct Palette {
    query: [u8; QUERY],
    len: usize,
    /// Which of the FILTERED masters is picked.
    picked: usize,
    /// Where the node goes, in world pixels: where the mouse was.
    pub at: (i32, i32),
}

/// What a key did.
pub enum Act {
    Stay,
    Close,
    /// Place this master (its index in `MASTERS`).
    Place(usize),
}

impl Palette {
    pub fn open(at: (i32, i32)) -> Palette {
        Palette { query: [0; QUERY], len: 0, picked: 0, at }
    }

    fn query(&self) -> &[u8] {
        &self.query[..self.len]
    }

    /// The masters the query names, in the order of the course.
    fn shown(&self, out: &mut [u8; COUNT]) -> usize {
        filter(self.query(), out)
    }

    pub fn key(&mut self, c: u8, up: u8, down: u8) -> Act {
        let mut idx = [0u8; COUNT];
        let n = self.shown(&mut idx);
        match c {
            0x1B | b'\t' => return Act::Close,
            b'\r' | b'\n' => return if n > 0 { Act::Place(idx[self.picked.min(n - 1)] as usize) } else { Act::Stay },
            0x08 | 0x7F => {
                self.len = self.len.saturating_sub(1);
                self.picked = 0;
            }
            k if k == up => self.picked = self.picked.saturating_sub(1),
            k if k == down => self.picked = (self.picked + 1).min(n.saturating_sub(1)),
            0x20..=0x7E if self.len < QUERY => {
                self.query[self.len] = c;
                self.len += 1;
                self.picked = 0;
            }
            _ => {}
        }
        Act::Stay
    }

    /// A click: on a row it picks that node (`true`: the TAB stays); outside
    /// the box it is `false` and the TAB closes.
    pub fn click(&mut self, c_w: i32, c_h: i32, x: i32, y: i32) -> bool {
        let (bx, by, bw, bh) = frame(c_w, c_h);
        if x < bx || x >= bx + bw || y < by || y >= by + bh {
            return false;
        }
        let mut idx = [0u8; COUNT];
        let n = self.shown(&mut idx);
        let mut rows = [Row::Family(0); COUNT + 16];
        let k = rows_of(&idx[..n], &mut rows);
        let (lx, ly, _, lh) = list_area(bx, by, bh);
        if x >= lx && x < lx + LIST_W && y >= ly {
            let first = first_row(&rows[..k], self.picked, lh);
            if let Some(Row::Node(p)) = rows.get(first + ((y - ly) / ROW) as usize).filter(|_| ((y - ly) / ROW) * ROW < lh) {
                self.picked = *p as usize;
            }
        }
        true
    }
}

/// A row of the list: a family's title or a node (its place in the filtered list).
#[derive(Clone, Copy)]
enum Row {
    Family(u8),
    Node(u8),
}

fn rows_of(idx: &[u8], out: &mut [Row]) -> usize {
    let mut k = 0;
    let mut last = u8::MAX;
    for (p, &i) in idx.iter().enumerate() {
        let lv = MASTERS[i as usize].level;
        if lv != last && k < out.len() {
            out[k] = Row::Family(lv);
            k += 1;
            last = lv;
        }
        if k < out.len() {
            out[k] = Row::Node(p as u8);
            k += 1;
        }
    }
    k
}

/// The first row on screen: the picked one always shows, with two below it.
fn first_row(rows: &[Row], picked: usize, list_h: i32) -> usize {
    let visible = (list_h / ROW).max(1) as usize;
    let at = rows.iter().position(|r| matches!(r, Row::Node(p) if *p as usize == picked)).unwrap_or(0);
    (at + 3).saturating_sub(visible).min(rows.len().saturating_sub(visible))
}

/// The box: centred over the canvas, under the title bar, above the panel.
fn frame(w: i32, h: i32) -> (i32, i32, i32, i32) {
    let (aw, ah) = (w - LEFT, h - TOP - PANEL);
    let (bw, bh) = (aw.min(840) - 40, ah.min(520) - 30);
    (LEFT + (aw - bw) / 2, TOP + (ah - bh) / 2, bw, bh)
}

fn list_area(bx: i32, by: i32, bh: i32) -> (i32, i32, i32, i32) {
    (bx + 12, by + 50, LIST_W, bh - 50 - 30)
}

/// The colour of a word: its class, the same as its cable and its astro.
fn word_color(w: &str) -> Color {
    match w {
        "mut" | "take" => AMBER,
        "if" | "else" | "and" | "or" | "not" | "match" => GOLD,
        "for" | "while" | "break" | "continue" => LILAC,
        "use" | "mod" | "pub" => CYAN,
        "gpu" | "f32" => GOOD,
        "enum" | "type" | "trait" | "dec" => mezclar(VIOLET, INK, 1, 3),
        _ => ACCENT,
    }
}

fn num(v: usize, out: &mut [u8; 3]) -> &[u8] {
    if v >= 10 {
        *out = [b'0' + (v / 10 % 10) as u8, b'0' + (v % 10) as u8, 0];
        &out[..2]
    } else {
        out[0] = b'0' + v as u8;
        &out[..1]
    }
}

pub fn draw(c: &mut Canvas, p: &Palette, t: i32) {
    let (bx, by, bw, bh) = frame(c.w, c.h);
    // The night behind it dims; the box floats with a little light.
    c.veil(BG, 1, 3);
    look::shine(c, bx, by, bw, bh, look::R_BOX, 14, VIOLET, 70);
    look::card(c, bx, by, bw, bh, look::R_BOX, TAB_BOX);
    look::edge(c, bx, by, bw, bh, look::R_BOX, 1, TAB_EDGE);

    // -- the search field ------------------------------------------------
    let mut idx = [0u8; COUNT];
    let n = p.shown(&mut idx);
    let w = look::pill(c, bx + 12, by + 14, b"TAB", ACCENT, mezclar(ACCENT, TAB_BOX, 1, 6));
    let (fx, fw) = (bx + 12 + w + 8, bw - w - 156);
    look::card(c, fx, by + 10, fw, 26, 8, mezclar(TAB_BOX, BG, 1, 2));
    look::edge(c, fx, by + 10, fw, 26, 8, 1, if p.len > 0 { ACCENT } else { EDGE });
    if p.len == 0 {
        c.text_fit(fx + 10, by + 15, b"escribe: un nombre, una palabra (match), un porque", DIM, fw - 20);
    } else {
        let tw = c.text(fx + 10, by + 15, p.query(), INK, 1);
        c.rect(fx + 11 + tw, by + 14, 2, 18, ACCENT);
    }
    let mut a = [0u8; 3];
    let mut b = [0u8; 3];
    let cx = bx + bw - 120;
    let k = c.text(cx, by + 15, num(n, &mut a), if n > 0 { INK } else { DIM }, 1);
    let k = k + c.text(cx + k, by + 15, b" de ", DIM, 1);
    c.text(cx + k, by + 15, num(COUNT, &mut b), DIM, 1);
    look::ring_line(c, bx + 12, by + 44, bw - 24);

    // -- the list, by family ---------------------------------------------
    let (lx, ly, lw, lh) = list_area(bx, by, bh);
    let mut rows = [Row::Family(0); COUNT + 16];
    let k = rows_of(&idx[..n], &mut rows);
    let first = first_row(&rows[..k], p.picked, lh);
    if n == 0 {
        c.text(lx + 4, ly + 4, b"nada se llama asi", DIM, 1);
    }
    for (r, row) in rows[first..k].iter().enumerate() {
        let y = ly + r as i32 * ROW;
        if y + ROW > ly + lh {
            break;
        }
        match *row {
            Row::Family(lv) => {
                let mut d = [0u8; 3];
                c.text(lx + 2, y + 1, num(lv as usize, &mut d), ACCENT, 1);
                c.text_fit(lx + 26, y + 1, FAMILIES.get(lv as usize).map_or(&b"?"[..], |f| f.as_bytes()), TITLE, lw - 30);
            }
            Row::Node(q) => {
                if q as usize == p.picked {
                    look::card(c, lx + 14, y, lw - 14, ROW, 4, TAB_PICK);
                    look::card(c, lx + 14, y + 3, 2, ROW - 6, 1, ACCENT);
                }
                let m = &MASTERS[idx[q as usize] as usize];
                let nw = c.text(lx + 24, y + 1, m.name.as_bytes(), TAB_ROW, 1);
                c.text_fit(lx + 32 + nw, y + 1, m.says.as_bytes(), DIM, lw - 46 - nw);
            }
        }
    }
    c.rect(lx + lw + 10, ly, 1, lh, EDGE);

    // -- the card of the picked node ---------------------------------------
    if n > 0 {
        card(c, &MASTERS[idx[p.picked.min(n - 1)] as usize], lx + lw + 24, ly, bx + bw - 12 - (lx + lw + 24), lh, t);
    }

    // -- what the keys do ------------------------------------------------
    c.text_fit(bx + 14, by + bh - 22, b"flechas eligen   Enter lo pone donde estaba el raton   Esc o TAB cierran", DIM, bw - 28);
}

/// One master, the way ELEMENTOS shows a figure: alive, with its why.
fn card(c: &mut Canvas, m: &Master, x: i32, y: i32, w: i32, h: i32, t: i32) {
    let chars = (w / 8) as usize;
    // Its astro, from what its body does: the same figure it will have in the sky.
    let tr = bmo_titan_lector::traits::scan(m.source.as_bytes());
    // Low enough that what leaves it (its `print` packets) stays in the card.
    astros::planet(c, x + 58, y + 86, 24, m.name.as_bytes(), tr, t);
    let tx = x + 140;
    let big = m.name.len() as i32 * 16 <= w - 140;
    c.text(tx, y + 8, &m.name.as_bytes()[..m.name.len().min(((w - 140) / 8) as usize)], INK, if big { 2 } else { 1 });
    let mut d = [0u8; 3];
    let k = c.text(tx, y + 46, b"nivel ", DIM, 1);
    let k = k + c.text(tx + k, y + 46, num(m.level as usize, &mut d), ACCENT, 1);
    c.text_fit(tx + k + 8, y + 46, m.family().as_bytes(), TITLE, w - 148 - k);
    lines(m.says.as_bytes(), ((w - 140) / 8) as usize, 2, |i, l| {
        c.text(tx, y + 68 + i as i32 * 16, l, INK, 1);
    });

    let mut cy = y + 140;
    c.text(x, cy, b"POR QUE", TITLE, 1);
    cy += 20;
    let mut used = 0;
    lines(m.why.as_bytes(), chars, 3, |i, l| {
        c.text(x, cy + i as i32 * 16, l, INK, 1);
        used = i + 1;
    });
    cy += used as i32 * 16 + 12;

    c.text(x, cy, b"TRAE", TITLE, 1);
    let mut px = x + 48;
    for word in m.words {
        let ww = word.len() as i32 * 8 + 14;
        if px + ww > x + w {
            break;
        }
        let col = word_color(word);
        px += look::pill(c, px, cy - 1, word.as_bytes(), col, mezclar(col, TAB_BOX, 1, 6)) + 6;
    }
    cy += 28;
    for perm in Permission::ALL {
        if m.asks.allows(perm) {
            let k = c.text(x, cy, b"PIDE ", AMBER, 1);
            c.text(x + k, cy, perm.key().as_bytes(), AMBER, 1);
            cy += 20;
            break;
        }
    }

    // Level 12: what the bench TYPES for it -- the proof depends on it.
    if !m.typed.is_empty() {
        let k = c.text(x, cy, b"TECLEA ", CYAN, 1);
        let mut px = x + k;
        for t in m.typed {
            if px + t.len() as i32 * 8 + 24 > x + w {
                c.text(px, cy, b"...", DIM, 1);
                break;
            }
            px += look::pill(c, px, cy - 1, t.as_bytes(), CYAN, mezclar(CYAN, TAB_BOX, 1, 6)) + 6;
        }
        cy += 24;
    }

    // Its proof: what it prints, checked by both benches on every build.
    c.text(x, cy, b"PRUEBA", TITLE, 1);
    c.text(x + 64, cy, b"# sale:  lo que escribe al correr", DIM, 1);
    cy += 20;
    let room = ((y + h - cy) / 16).max(1) as usize;
    for (i, line) in m.out.iter().enumerate() {
        if i + 1 == room && m.out.len() > room {
            let mut d = [0u8; 3];
            let k = c.text(x, cy, b"... y ", DIM, 1);
            let k = k + c.text(x + k, cy, num(m.out.len() - i, &mut d), DIM, 1);
            c.text(x + k, cy, b" lineas mas", DIM, 1);
            break;
        }
        c.text_fit(x, cy, line.as_bytes(), GOOD, w);
        cy += 16;
    }
}
