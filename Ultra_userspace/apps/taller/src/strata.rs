//! **ESTRATOS, IN F1** -- the volume's history as UNIQUE NODES (T2 of B8,
//! `docs/plan/PLAN_LA_BANDEJA.md`; the ESTRATOS tab of the mockup
//! `docs/arte/maqueta_taller_estratos.html`).
//!
//! [consumo] NADA      draws only when F1 draws; it reads the history when the
//!                     tab opens and when the generation moved, never per frame
//!
//! ```text
//!    every version     a node with its SEAL (`astros::seal`), on a chain from
//!                      the oldest (left, top) to NOW, snaking down the canvas
//!    with a name       PERMANENT: it glows, and its name rides above it
//!    picked            a ring of light; its card in the panel below
//!    ENTER, twice      RESTABLECER = `volver(n)`: ONE new estrato that points
//!                      at that root; what lies between is NOT lost
//! ```
//!
//! ** This file knows no kernel: it is data in, pixels out, and a hit test.
//! `store.rs` fills a [`History`] from `bmo::estratos::hist_*` and asks
//! `volver`; the host's camera fills it with a sample. Same painter on both.
//!
//! [!] What the seal grows from, said plainly: the kernel's door gives WHEN,
//! WHO and the NAME of each version, not its BLAKE3. So the seed is those
//! three; two automatic versions written by the same process in the same
//! second would share a face. The day `ES_HIST_*` hands out the sum, the seed
//! is the sum and nothing else changes here.

use crate::aspecto as look;
use crate::astros::{halo, hash, orbit, seal};
use crate::canvas::Canvas;
use crate::view::{LEFT, PANEL, TOP};
use bmo_dibujo::{mezclar, Color, Lienzo};
use look::{ACCENT, BAR, BG, DIM, EDGE, INK, NEON};

/// The kernel keeps the 32 most recent versions (`fsys/estratos/historia.rs`).
pub const MAX: usize = 32;
/// What fits of a name on screen. The kernel keeps 64; a node shows less.
pub const NAME_MAX: usize = 32;
/// Two ENTERs on the same version within this long restore it.
pub const CONFIRM_MS: u32 = 4_000;

#[derive(Clone, Copy)]
pub struct Version {
    /// `INFO_FECHA`'s packing (`bmo_rtc::desempaquetar`); 0: undated.
    pub when: u64,
    /// The process that wrote it.
    pub who: u32,
    pub name: [u8; NAME_MAX],
    pub name_len: usize,
    /// It has TWO parents: two branches met here (`ES_RAMA_DOS_PADRES`).
    pub merge: bool,
}

impl Version {
    pub const EMPTY: Version = Version { when: 0, who: 0, name: [0; NAME_MAX], name_len: 0, merge: false };

    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len.min(NAME_MAX)]
    }

    /// PERMANENT: the collector never lets a named version go.
    pub fn marked(&self) -> bool {
        self.name_len > 0
    }

    /// Its seal's number: when, who and its name (see the note above).
    pub fn seed(&self) -> u32 {
        (self.when as u32 ^ (self.when >> 32) as u32).rotate_left(7) ^ self.who.wrapping_mul(0x9E37_79B9) ^ hash(self.name())
    }
}

/// What the tab shows. `versions[0]` is NOW, as the kernel keeps them.
pub struct History {
    pub versions: [Version; MAX],
    pub n: usize,
    /// The kernel stopped at its cap: there are older ones, not shown.
    pub cut: bool,
    /// ESTRATOS is not mounted: nothing to show, and the tab says why.
    pub absent: bool,
    pub picked: Option<usize>,
    /// The first ENTER on a version: (which, until when).
    pub confirm: Option<(usize, u32)>,
    /// What the last action said, for the panel.
    pub said: Option<(&'static [u8], bool)>,
}

impl History {
    pub const EMPTY: History =
        History { versions: [Version::EMPTY; MAX], n: 0, cut: false, absent: true, picked: None, confirm: None, said: None };

    pub fn shown(&self) -> &[Version] {
        &self.versions[..self.n.min(MAX)]
    }

    /// Picks the version one step older (`older`) or newer than the picked one.
    pub fn step(&mut self, older: bool) {
        if self.n == 0 {
            return;
        }
        let i = self.picked.unwrap_or(0);
        self.picked = Some(if older { (i + 1).min(self.n - 1) } else { i.saturating_sub(1) });
        self.confirm = None;
    }
}

/// ENTER on the picked version. The first one ASKS (the panel says what will
/// happen); the second, within [`CONFIRM_MS`], answers how many steps back to
/// restore. NOW is never restored: it is where the volume already is.
pub fn enter(h: &mut History, now_ms: u32) -> Option<usize> {
    let i = h.picked.filter(|&i| i > 0 && i < h.n)?;
    match h.confirm {
        Some((k, until)) if k == i && until.wrapping_sub(now_ms) < u32::MAX / 2 => {
            h.confirm = None;
            Some(i)
        }
        _ => {
            h.confirm = Some((i, now_ms.wrapping_add(CONFIRM_MS)));
            h.said = None;
            None
        }
    }
}

/// A key in the tab: the arrows walk the chain, ENTER twice restores the
/// picked version (`restore` asks the kernel), Esc takes the question back.
pub fn key(h: &mut History, k: u8, now: u32, restore: impl FnOnce(usize) -> bool) {
    match k {
        crate::KEY_LEFT => h.step(true),
        crate::KEY_RIGHT => h.step(false),
        0x1B => {
            h.confirm = None;
            h.said = None;
        }
        _ => {
            if let Some(steps) = enter(h, now) {
                let ok = restore(steps);
                h.said = Some(if ok {
                    (&b"RESTABLECIDA: un estrato nuevo; lo de en medio sigue en la historia"[..], true)
                } else {
                    (&b"NO se pudo restablecer: el volumen no cambio"[..], false)
                });
                if ok {
                    h.picked = Some(0);
                }
            }
        }
    }
}

/// The canvas the chain lives in: right of the EXPLORER, under the tabs,
/// above the panel.
fn area(c: &Canvas) -> (i32, i32, i32, i32) {
    let top = TOP + 40;
    (LEFT + 24, top, c.w - LEFT - 48, c.h - PANEL - top - 16)
}

/// Where version `i` (0 = now) sits: oldest first, snaking in rows of up to
/// eight, a little wave from its own seed so the chain does not look ruled.
fn place(c: &Canvas, n: usize, i: usize, seed: u32) -> (i32, i32) {
    let (x0, y0, w, h) = area(c);
    let per_row = 8usize;
    let rows = n.div_ceil(per_row).max(1);
    let k = n - 1 - i; // position from the oldest
    let (row, col) = (k / per_row, k % per_row);
    let col = if row % 2 == 1 { per_row - 1 - col } else { col };
    let step_x = w / per_row as i32;
    let step_y = h / rows as i32;
    let wave = (seed % 17) as i32 - 8;
    (x0 + step_x / 2 + col as i32 * step_x, y0 + step_y / 2 + row as i32 * step_y + wave)
}

/// The radius of a version's node.
const R: i32 = 22;

/// The colours of the versions: the family of ESTRATOS's neon.
fn hue(seed: u32) -> Color {
    const GREENS: [Color; 5] = [NEON, 0x002B_E6A0, 0x005C_FF6E, 0x0036_D8C4, 0x007D_FF9E];
    GREENS[(seed % GREENS.len() as u32) as usize]
}

/// The version under (x, y), if any.
pub fn hit(c: &Canvas, h: &History, x: i32, y: i32) -> Option<usize> {
    h.shown().iter().enumerate().find_map(|(i, v)| {
        let (cx, cy) = place(c, h.n, i, v.seed());
        let (dx, dy) = (x - cx, y - cy);
        (dx * dx + dy * dy <= (R + 6) * (R + 6)).then_some(i)
    })
}

/// The whole tab: the chain, its nodes, and the card of the picked one.
pub fn draw(c: &mut Canvas, h: &History, sky: Option<&[u32]>, now_ms: u32) {
    match sky {
        Some(px) => c.blit(px),
        None => c.clear(BG),
    }
    let (x0, y0, _, _) = area(c);
    if h.absent || h.n == 0 {
        let why: &[u8] = if h.absent { b"ESTRATOS no esta montado: no hay historia que mostrar" } else { b"ESTRATOS no tiene versiones todavia" };
        c.text(x0, y0 + 20, why, DIM, 1);
        panel(c, h, now_ms);
        return;
    }
    // The chain first, so the nodes sit on it.
    for i in 1..h.n {
        let a = place(c, h.n, i, h.versions[i].seed());
        let b = place(c, h.n, i - 1, h.versions[i - 1].seed());
        dotted(c, a, b, mezclar(NEON, BG, 3, 5));
    }
    for (i, v) in h.shown().iter().enumerate() {
        let (x, y) = place(c, h.n, i, v.seed());
        let s = v.seed();
        let color = hue(s);
        if v.marked() {
            halo(c, x, y, R, 12, color, 220);
        }
        if h.picked == Some(i) {
            halo(c, x, y, R + 3, 8, ACCENT, 260);
            orbit(c, x, y, R + 7, R + 7, ACCENT);
        }
        // A MERGE: the second parent comes in from the side, in the colour
        // of the branch that came in (`branches.rs`).
        if v.merge {
            let from = (x + R + 34, y - R - 26);
            dotted(c, from, (x, y), look::VIOLET);
            c.disc(from.0, from.1, 5, look::VIOLET);
            c.text(from.0 + 9, from.1 - 4, b"mezcla", look::VIOLET, 1);
        }
        c.disc(x, y, R, mezclar(color, BG, 1, 9));
        seal(c, x, y, R - 3, s, color);
        if h.confirm.map(|(k, _)| k) == Some(i) {
            halo(c, x, y, R + 8, 8, look::AMBER, 300);
            orbit(c, x, y, R + 11, R + 11, look::AMBER);
        }
        // Its number under it (v1 is the oldest the kernel kept) or NOW.
        let mut b = [0u8; 8];
        let label: &[u8] = if i == 0 { b"ahora" } else { number(b'v', (h.n - i) as u64, &mut b) };
        let lw = label.len() as i32 * 8;
        c.text(x - lw / 2, y + R + 6, label, if i == 0 { INK } else { DIM }, 1);
        if v.marked() {
            let nw = (v.name_len.min(16) as i32) * 8;
            c.text_fit(x - nw / 2, y - R - 22, v.name(), color, 16 * 8);
        }
    }
    if h.cut {
        c.text(x0, y0 - 18, b"solo las 32 mas recientes: hay mas, mas viejas", DIM, 1);
    }
    panel(c, h, now_ms);
}

/// A line of dots from `a` to `b`: the chain from parent to child.
fn dotted(c: &mut Canvas, a: (i32, i32), b: (i32, i32), color: Color) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let steps = (dx.abs().max(dy.abs()) / 6).max(1);
    for k in 0..=steps {
        c.put(a.0 + dx * k / steps, a.1 + dy * k / steps, color);
    }
}

/// `v12`: a letter and a number, into `out`.
fn number(lead: u8, mut v: u64, out: &mut [u8; 8]) -> &[u8] {
    let mut digits = [0u8; 7];
    let mut n = 0;
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 || n == digits.len() {
            break;
        }
    }
    out[0] = lead;
    for k in 0..n {
        out[1 + k] = digits[n - 1 - k];
    }
    &out[..1 + n]
}

/// The card under the canvas: what the picked version is, and what ENTER does.
fn panel(c: &mut Canvas, h: &History, now_ms: u32) {
    let (x, y, w) = (LEFT, c.h - PANEL, c.w - LEFT);
    c.rect(x, y, w, PANEL, BAR);
    c.rect(x, y, w, 1, EDGE);
    let (tx, mut ty) = (x + 12, y + 10);
    c.text(tx, ty, b"ESTRATOS", NEON, 1);
    c.text(tx + 80, ty, b"cada version, un nodo: su forma sale de cuando, quien y su nombre", DIM, 1);
    ty += 22;
    let Some(i) = h.picked.filter(|&i| i < h.n) else {
        c.text(tx, ty, b"pulsa una version, o las flechas: <- mas vieja, -> mas nueva", INK, 1);
        said(c, h, tx, ty + 22);
        return;
    };
    let v = &h.versions[i];
    let mut b = [0u8; 8];
    let k = c.text(tx, ty, if i == 0 { b"ahora" } else { number(b'v', (h.n - i) as u64, &mut b) }, INK, 1);
    let mut f = [0u8; 24];
    let when: &[u8] = match bmo_rtc::desempaquetar(v.when) {
        Some(d) => {
            let n = bmo_rtc::escribir(&d, &mut f);
            &f[..n.min(19)]
        }
        // No date is invented: 1970 would lie with more conviction.
        None => b"sin fechar",
    };
    let k = k + 16 + c.text(tx + k + 16, ty, when, DIM, 1);
    let mut p = [0u8; 8];
    let k = k + 16 + c.text(tx + k + 16, ty, b"proceso ", DIM, 1);
    c.text(tx + k, ty, number(b'#', v.who as u64, &mut p), DIM, 1);
    ty += 20;
    if v.merge {
        c.text(tx + w - 360, ty - 20, b"MEZCLA: aqui se juntaron dos ramas", look::VIOLET, 1);
    }
    if v.marked() {
        let k = c.text(tx, ty, b"PERMANENTE: ", NEON, 1);
        c.text_fit(tx + k, ty, v.name(), INK, w - k - 24);
    } else {
        c.text(tx, ty, b"automatica: sin nombre", DIM, 1);
    }
    ty += 20;
    match h.confirm {
        Some((k, until)) if k == i && until.wrapping_sub(now_ms) < u32::MAX / 2 => {
            c.text(tx, ty, b"ENTER otra vez: RESTABLECER -- un estrato nuevo con la raiz de esta; lo de en medio se queda", look::AMBER, 1);
        }
        _ if i == 0 => {
            c.text(tx, ty, b"es la de ahora", DIM, 1);
        }
        _ => {
            c.text(tx, ty, b"ENTER dos veces: RESTABLECER (vuelve sin perder nada)   Esc: nada", INK, 1);
        }
    }
    said(c, h, tx, ty + 20);
}

fn said(c: &mut Canvas, h: &History, x: i32, y: i32) {
    if let Some((text, good)) = h.said {
        c.text(x, y, text, if good { NEON } else { look::BAD }, 1);
    }
}
