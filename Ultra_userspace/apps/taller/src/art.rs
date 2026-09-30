//! **THE ART OF TITAN++** -- the logo, inside F1: the night the canvas sits
//! in, and the splash when the workshop opens (`PLAN_TALLER` 8.8).
//!
//! ```text
//!    arte/titan.bin    the logo as palette + PackBits (TLG1), made ONCE by
//!                      docs/arte/titan_a_logo.py from docs/arte/titan.jpg
//!    decode            into a borrowed block: one byte per pixel
//!    backdrop          the sky, built ONCE: night gradient, stars, and the
//!                      planet and the centaur, dim, behind the graph
//!    splash            the whole logo, fading in and out over the workshop
//! ```
//!
//! No JPEG decoder: the expensive part (scaling, 128 colours, the black made
//! exact) happened on the PC. What is left is fifteen lines of PackBits and a
//! palette lookup.
//!
//! [consumo] the sky is drawn ONCE into its own buffer; every frame after
//! that is a copy, the same bytes `clear` used to write. The splash wakes
//! every 16 ms for under two seconds, and a click or a key ends it.
//!
//! The bytes are not trusted to be what the generator wrote: a bad header, a
//! run that overflows, a palette index past the end -- any of them and there
//! is no art, and F1 works exactly as before. Decoration never stops the
//! workshop.

use crate::canvas::Canvas;
use bmo_dibujo::{mezclar, Color};

/// The logo, as `docs/arte/titan_a_logo.py` wrote it.
pub static TITAN: &[u8] = include_bytes!("../arte/titan.bin");

const HEADER: usize = 12;

/// The logo, decoded: one palette index per pixel.
pub struct Logo<'a> {
    pub w: i32,
    pub h: i32,
    /// Rows that are the DRAWING; the lettering is below.
    pub art_h: i32,
    pal: [Color; 256],
    idx: &'a [u8],
}

fn u16_at(b: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]) as usize)
}

/// Width and height the header says, to size the block before decoding.
pub fn size(src: &[u8]) -> Option<(usize, usize)> {
    if src.get(..4)? != b"TLG1" {
        return None;
    }
    let (w, h) = (u16_at(src, 4)?, u16_at(src, 6)?);
    (w > 0 && h > 0 && w <= 1024 && h <= 1024).then_some((w, h))
}

/// Decodes `src` into `out` (at least w*h bytes). `None` at the first thing
/// that is not what the format says.
pub fn decode<'a>(src: &[u8], out: &'a mut [u8]) -> Option<Logo<'a>> {
    let (w, h) = size(src)?;
    let art_h = u16_at(src, 8)?.min(h);
    let n = u16_at(src, 10)?;
    if n == 0 || n > 256 {
        return None;
    }
    let mut pal = [0; 256];
    let colours = src.get(HEADER..HEADER + 3 * n)?;
    for (i, rgb) in colours.chunks_exact(3).enumerate() {
        pal[i] = (rgb[0] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[2] as u32;
    }
    let total = w * h;
    let out = out.get_mut(..total)?;
    let mut data = src.get(HEADER + 3 * n..)?.iter();
    let mut k = 0;
    while k < total {
        let op = *data.next()? as usize;
        if op < 128 {
            for _ in 0..=op {
                *out.get_mut(k)? = *data.next()?;
                k += 1;
            }
        } else {
            let v = *data.next()?;
            let end = k + op - 126;
            out.get_mut(k..end)?.fill(v);
            k = end;
        }
    }
    if out.iter().any(|&i| i as usize >= n) {
        return None;
    }
    Some(Logo { w: w as i32, h: h as i32, art_h: art_h as i32, pal, idx: out })
}

impl Logo<'_> {
    fn at(&self, x: i32, y: i32) -> Color {
        let (x, y) = (x.clamp(0, self.w - 1), y.clamp(0, self.h - 1));
        self.pal[self.idx[(y * self.w + x) as usize] as usize]
    }

    /// The colour at (fx, fy) in 1/256 of a pixel, between its four
    /// neighbours: what makes a glow scaled up stay a glow and not squares.
    fn sample(&self, fx: i32, fy: i32) -> Color {
        let (x, y, ax, ay) = (fx >> 8, fy >> 8, (fx & 255) as u32, (fy & 255) as u32);
        let top = mezclar(self.at(x + 1, y), self.at(x, y), ax, 256);
        let bottom = mezclar(self.at(x + 1, y + 1), self.at(x, y + 1), ax, 256);
        mezclar(bottom, top, ay, 256)
    }
}

/// `a` + `b` per channel, saturating: light added to light.
fn add(a: Color, b: Color) -> Color {
    let ch = |s: u32| ((a >> s & 255) + (b >> s & 255)).min(255) << s;
    ch(16) | ch(8) | ch(0)
}

/// `c` at `k` thousandths of its brightness.
fn dim(c: Color, k: u32) -> Color {
    let ch = |s: u32| ((c >> s & 255) * k / 1000) << s;
    ch(16) | ch(8) | ch(0)
}

/// A small generator of numbers: the stars are the same every time.
struct Stars(u32);

impl Stars {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

/// Where the canvas is and how much of the logo's drawing goes behind it.
pub struct Sky {
    pub w: i32,
    pub h: i32,
    /// The canvas: the part of the window that is not EXPLORER, title or panel.
    pub area: (i32, i32, i32, i32),
}

/// The night of TITAN++, into `px` (w*h): a gradient, the stars, and the
/// drawing of the logo -- planet, ring, centaur -- dim and centered behind the
/// graph. Built once; `Canvas::blit` puts it back every frame.
pub fn backdrop(px: &mut [u32], sky: &Sky, logo: Option<&Logo>) {
    let (w, h) = (sky.w, sky.h);
    if px.len() < (w * h) as usize {
        return;
    }
    for y in 0..h {
        // From a blue night at the top to almost black at the bottom.
        let c = mezclar(0x0002_030A, 0x0006_0A20, y as u32, h as u32);
        px[(y * w) as usize..((y + 1) * w) as usize].fill(c);
    }
    let mut r = Stars(0x7171_2929);
    for _ in 0..460 {
        let (x, y) = ((r.next() % w as u32) as i32, (r.next() % h as u32) as i32);
        let b = 40 + r.next() % 170;
        // Cold white: a touch more blue than red.
        let star = (b * 200 / 255) << 16 | (b * 220 / 255) << 8 | b;
        let at = (y * w + x) as usize;
        px[at] = add(px[at], star);
        if r.next() % 11 == 0 {
            // A bright one: a small cross of light around it.
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (sx, sy) = (x + dx, y + dy);
                if sx >= 0 && sy >= 0 && sx < w && sy < h {
                    let at = (sy * w + sx) as usize;
                    px[at] = add(px[at], dim(star, 450));
                }
            }
        }
    }
    let Some(logo) = logo else { return };
    // The drawing fills the canvas's height; the lettering stays out.
    let (ax, ay, aw, ah) = sky.area;
    let tall = ah - 8;
    let wide = logo.w * tall / logo.art_h.max(1);
    let (x0, y0) = (ax + (aw - wide) / 2, ay + (ah - tall) / 2);
    let step = logo.art_h * 256 / tall.max(1);
    for dy in 0..tall {
        let y = y0 + dy;
        if y < 0 || y >= h {
            continue;
        }
        for dx in 0..wide {
            let x = x0 + dx;
            if x < 0 || x >= w {
                continue;
            }
            let c = logo.sample(dx * step, dy * step);
            if c != 0 {
                let at = (y * w + x) as usize;
                // Dim: it is the room, not the subject.
                px[at] = add(px[at], dim(c, 300));
            }
        }
    }
}

/// The splash at `a` thousandths: the workshop veiled toward black, and the
/// whole logo lit on top, centered.
pub fn splash(c: &mut Canvas, logo: &Logo, a: u32) {
    let a = a.min(1000);
    if a == 1000 {
        // Fully covered: black is cheaper written than blended.
        c.clear(0);
    } else {
        c.veil(0, a, 1000);
    }
    let (x0, y0) = ((c.w - logo.w) / 2, (c.h - logo.h) / 2);
    for y in 0..logo.h {
        for x in 0..logo.w {
            let col = logo.at(x, y);
            if col != 0 {
                c.light(x0 + x, y0 + y, dim(col, a));
            }
        }
    }
}

/// How far into the splash `t` ms is: in, hold, out. `None` once it is over.
pub fn splash_at(t: u32) -> Option<u32> {
    const IN: u32 = 350;
    const HOLD: u32 = 1100;
    const OUT: u32 = 500;
    match t {
        t if t < IN => Some(t * 1000 / IN),
        t if t < IN + HOLD => Some(1000),
        t if t < IN + HOLD + OUT => Some(1000 - (t - IN - HOLD) * 1000 / OUT),
        _ => None,
    }
}

