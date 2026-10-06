//! **THE GUIDE OF ESTRATOS, in F1** -- every door that exists, how many, what
//! each one does and WHY (`docs/plan/PLAN_LAS_RAMAS.md`, R1).
//!
//! [consumo] NADA      draws only when F1 draws; the table is `.rodata`
//!
//! ```text
//!    the families      two columns: the cursor and the history (they READ),
//!                      the names, the gestures and the branches (those
//!                      two WRITE)
//!    every door        one line: its name and the start of WHAT it does
//!    picked            the panel below: WHAT, whole, and WHY, as the
//!                      contract says them
//! ```
//!
//! ** Nothing here is written by hand: `guia_estratos_gen.rs` is generated
//! from the ABI's contract by `toolchain/tools/estratos-guia/guia.py`, and the
//! build says NO when the two stop saying the same. A door added to the
//! contract is in this guide the next build; the count is never a number
//! someone typed.

use crate::aspecto as look;
use crate::canvas::Canvas;
use crate::guia_estratos_gen::{COUNT, DOORS, FAMILIES};
use crate::view::{LEFT, PANEL, TOP};
use bmo_dibujo::Lienzo;
use look::{AMBER, BAR, BG, DIM, EDGE, INK, NEON, SEL};

const LINE: i32 = 16;
const HEX: &[u8; 16] = b"0123456789ABCDEF";
/// Families 0 and 1 on the left, 2 and 3 on the right.
const SPLIT: usize = 2;

/// Where each row goes: (door index or a family header, x, y).
#[derive(Clone, Copy)]
enum Row {
    Family(usize),
    Door(usize),
}

fn rows(width: i32, mut f: impl FnMut(Row, i32, i32, i32)) {
    let top = TOP + 64;
    let col_w = (width - LEFT - 48) / 2;
    for column in 0..2 {
        let x = LEFT + 24 + column as i32 * (col_w + 16);
        let mut y = top;
        let fams = if column == 0 { 0..SPLIT } else { SPLIT..FAMILIES.len() };
        for fam in fams {
            f(Row::Family(fam), x, y, col_w);
            y += LINE + 4;
            for (i, d) in DOORS.iter().enumerate() {
                if d.family == fam {
                    f(Row::Door(i), x, y, col_w);
                    y += LINE;
                }
            }
            y += 10;
        }
    }
}

/// The door under (x, y), if any.
pub fn hit(c: &Canvas, x: i32, y: i32) -> Option<usize> {
    let mut found = None;
    rows(c.w, |r, rx, ry, w| {
        if let Row::Door(i) = r {
            if x >= rx && x < rx + w && y >= ry && y < ry + LINE {
                found = Some(i);
            }
        }
    });
    found
}

/// One door up or down the list, in the order of the contract.
pub fn step(picked: usize, down: bool) -> usize {
    if down {
        (picked + 1).min(COUNT - 1)
    } else {
        picked.saturating_sub(1)
    }
}

pub fn draw(c: &mut Canvas, picked: usize, sky: Option<&[u32]>) {
    match sky {
        Some(px) => c.blit(px),
        None => c.clear(BG),
    }
    let x0 = LEFT + 24;
    let k = c.text(x0, TOP + 40, b"ESTRATOS: ", NEON, 1);
    let mut b = [0u8; 4];
    let k = k + c.text(x0 + k, TOP + 40, digits(COUNT as u32, &mut b), INK, 1);
    let k = k + c.text(x0 + k, TOP + 40, b" puertas en ", DIM, 1);
    let k = k + c.text(x0 + k, TOP + 40, digits(FAMILIES.len() as u32, &mut b), INK, 1);
    c.text(x0 + k, TOP + 40, b" familias; cada una dice QUE hace y POR QUE, tal como lo dice el contrato", DIM, 1);
    let width = c.w;
    rows(width, |r, x, y, w| match r {
        Row::Family(f) => {
            let (name, what, writes) = FAMILIES[f];
            let n = DOORS.iter().filter(|d| d.family == f).count() as u32;
            let mut b = [0u8; 4];
            let k = c.text(x, y, name.as_bytes(), if writes { AMBER } else { NEON }, 1);
            let k = k + c.text(x + k, y, b"  ", DIM, 1);
            let k = k + c.text(x + k, y, digits(n, &mut b), INK, 1);
            c.text_fit(x + k + 8, y, what.as_bytes(), DIM, w - k - 8);
            c.rect(x, y + LINE + 1, w, 1, EDGE);
        }
        Row::Door(i) => {
            let d = &DOORS[i];
            if i == picked {
                c.rect(x - 4, y - 1, w + 4, LINE, SEL);
            }
            let writes = FAMILIES[d.family].2;
            c.text_fit(x, y, d.name.as_bytes(), if writes { AMBER } else { INK }, 136);
            c.text_fit(x + 144, y, d.what.as_bytes(), if i == picked { INK } else { DIM }, w - 148);
        }
    });
    panel(c, picked);
}

/// WHAT and WHY of the picked door, whole, wrapped.
fn panel(c: &mut Canvas, picked: usize) {
    let (x, y, w) = (LEFT, c.h - PANEL, c.w - LEFT);
    c.rect(x, y, w, PANEL, BAR);
    c.rect(x, y, w, 1, EDGE);
    let d = &DOORS[picked.min(COUNT - 1)];
    let (tx, mut ty) = (x + 12, y + 8);
    let k = c.text(tx, ty, d.name.as_bytes(), if FAMILIES[d.family].2 { AMBER } else { NEON }, 1);
    let k = k + 16 + c.text(tx + k + 16, ty, d.door.as_bytes(), DIM, 1);
    let mut h = *b" = 0x00";
    h[5] = HEX[(d.value >> 4 & 0xF) as usize];
    h[6] = HEX[(d.value & 0xF) as usize];
    c.text(tx + k, ty, if d.value < 0x100 { &h[..] } else { b"" }, DIM, 1);
    ty += LINE + 4;
    let cols = ((w - 24 - 9 * 8) / 8).max(16) as usize;
    // The labels in a column of their own, so both texts start together.
    let body = tx + 9 * 8;
    c.text(tx, ty, b"QUE", INK, 1);
    ty = wrap(c, body, ty, d.what.as_bytes(), cols, INK, 2);
    if !d.why.is_empty() {
        c.text(tx, ty, b"POR QUE", NEON, 1);
        wrap(c, body, ty, d.why.as_bytes(), cols, DIM, 3);
    }
    c.text(c.w - 24 * 8, y + 8, b"flechas: otra puerta", DIM, 1);
}

/// Up to `lines` lines of `s`, cut at spaces; the next free `y`.
fn wrap(c: &mut Canvas, x: i32, mut y: i32, mut s: &[u8], cols: usize, color: bmo_dibujo::Color, lines: usize) -> i32 {
    for n in 0..lines {
        if s.is_empty() {
            break;
        }
        let cut = if s.len() <= cols {
            s.len()
        } else {
            s[..cols].iter().rposition(|&b| b == b' ').unwrap_or(cols)
        };
        let last = n + 1 == lines && cut < s.len();
        let w = c.text(x, y, &s[..cut], color, 1);
        if last {
            c.text(x + w, y, b"...", color, 1);
        }
        s = s[cut..].strip_prefix(b" ").unwrap_or(&s[cut..]);
        y += LINE;
    }
    y
}

fn digits(mut v: u32, out: &mut [u8; 4]) -> &[u8] {
    let mut n = 0;
    let mut tmp = [0u8; 4];
    loop {
        tmp[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 || n == 4 {
            break;
        }
    }
    for k in 0..n {
        out[k] = tmp[n - 1 - k];
    }
    &out[..n]
}
