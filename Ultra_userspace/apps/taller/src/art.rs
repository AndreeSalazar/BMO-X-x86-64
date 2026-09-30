//! **THE ART OF TITAN++** -- the logo, inside F1: the night the canvas sits
//! in, and the splash when the workshop opens (`PLAN_TALLER` 8.8).
//!
//! ```text
//!    arte/titan.bin    the logo as palette + PackBits (TLG1), made ONCE by
//!                      docs/arte/titan_a_logo.py from docs/arte/titan.jpg
//!    decode            into a borrowed block: one byte per pixel
//!    backdrop          the sky, built ONCE: night gradient, two soft
//!                      nebulas (blue, violet) and coloured stars -- the
//!                      logo's colours, and nothing drawn behind the graph
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
    // Offset 8 (`dibujo`, the rows above the lettering) is not read: the
    // sky stopped drawing the logo on 30-09. The generator still writes it.
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
    Some(Logo { w: w as i32, h: h as i32, pal, idx: out })
}

impl Logo<'_> {
    fn at(&self, x: i32, y: i32) -> Color {
        let (x, y) = (x.clamp(0, self.w - 1), y.clamp(0, self.h - 1));
        self.pal[self.idx[(y * self.w + x) as usize] as usize]
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

/// Where the canvas is: the part of the window that is not EXPLORER, title
/// or panel.
pub struct Sky {
    pub w: i32,
    pub h: i32,
    pub area: (i32, i32, i32, i32),
}

/// A soft cloud of light: `c` at the center fading to nothing at radius `r`
/// (the square of a falloff, so it has no edge), `k` thousandths strong.
fn nebula(px: &mut [u32], w: i32, h: i32, (cx, cy, r): (i32, i32, i32), c: Color, k: u32) {
    let r2 = (r * r) as u64;
    for y in (cy - r).max(0)..(cy + r).min(h) {
        for x in (cx - r).max(0)..(cx + r).min(w) {
            let d2 = ((x - cx) * (x - cx) + (y - cy) * (y - cy)) as u64;
            if d2 < r2 {
                let t = r2 - d2;
                let fall = (t * t * 1000 / (r2 * r2)) as u32;
                let at = (y * w + x) as usize;
                px[at] = add(px[at], dim(c, fall * k / 1000));
            }
        }
    }
}

/// The night of TITAN++, into `px` (w*h): a gradient, two nebulas in the
/// logo's blue and violet, and stars of three colours. SIMPLE on purpose (the
/// owner, 30-09): the logo is the ENTRANCE; behind the graph there is only
/// sky, so nothing competes with the nodes. Built once; `Canvas::blit` puts
/// it back every frame.
pub fn backdrop(px: &mut [u32], sky: &Sky) {
    let (w, h) = (sky.w, sky.h);
    if px.len() < (w * h) as usize {
        return;
    }
    for y in 0..h {
        // From a blue night at the top to almost black at the bottom.
        let c = mezclar(0x0002_030A, 0x0006_0A20, y as u32, h as u32);
        px[(y * w) as usize..((y + 1) * w) as usize].fill(c);
    }
    let (ax, ay, aw, ah) = sky.area;
    // Blue low on the left, violet high on the right: the ring of the logo,
    // spread across the canvas.
    nebula(px, w, h, (ax + aw / 4, ay + ah * 3 / 4, ah * 3 / 5), 0x001A_2F8C, 420);
    nebula(px, w, h, (ax + aw * 4 / 5, ay + ah / 4, ah / 2), 0x0042_1F8A, 380);
    let mut r = Stars(0x7171_2929);
    for _ in 0..520 {
        let (x, y) = ((r.next() % w as u32) as i32, (r.next() % h as u32) as i32);
        let b = 40 + r.next() % 170;
        // Most are cold white; some blue, a few violet -- the logo's three.
        let star = match r.next() % 10 {
            0..=6 => (b * 200 / 255) << 16 | (b * 220 / 255) << 8 | b,
            7 | 8 => (b * 110 / 255) << 16 | (b * 160 / 255) << 8 | b,
            _ => (b * 180 / 255) << 16 | (b * 130 / 255) << 8 | b,
        };
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

