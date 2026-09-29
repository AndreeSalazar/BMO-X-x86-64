//! **THE CANVAS** -- drawing into our own surface with the house's rasterizer.
//!
//! Clipping, lines and curves are `bmo-dibujo`'s (the same rules the kernel
//! and the desktop use); the only thing written here is how to reach OUR
//! pixels (`Lienzo::rect_dentro` and `put`) and the letters, whose bits come
//! from `bmo::glyph_bits` -- the same font as the desktop, not a copy.

use bmo_dibujo::{curva, linea, mezclar, Color, Lienzo, Recorte, Vertice};
use bmo_userland as bmo;

pub struct Canvas {
    px: *mut u32,
    pub w: i32,
    pub h: i32,
}

impl Canvas {
    pub fn new(px: *mut u32, w: u32, h: u32) -> Canvas {
        Canvas { px, w: w as i32, h: h as i32 }
    }

    /// One pixel, if it falls inside.
    pub fn put(&mut self, x: i32, y: i32, c: Color) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            // SAFETY: (x, y) was just checked against the surface size, and
            // `px` points to `w * h` pixels of our own block.
            unsafe { *self.px.add((y * self.w + x) as usize) = c };
        }
    }

    fn get(&self, x: i32, y: i32) -> Color {
        // SAFETY: only called with coordinates already checked by `blend`.
        unsafe { *self.px.add((y * self.w + x) as usize) }
    }

    /// `c` over what is there, `part` out of `total`.
    pub fn blend(&mut self, x: i32, y: i32, c: Color, part: u32, total: u32) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            let under = self.get(x, y);
            self.put(x, y, mezclar(c, under, part, total));
        }
    }

    pub fn clear(&mut self, c: Color) {
        let r = self.recorte();
        self.rect_dentro(r, c);
    }

    /// The border of a box, `t` pixels thick.
    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, t: i32, c: Color) {
        self.rect(x, y, w, t, c);
        self.rect(x, y + h - t, w, t, c);
        self.rect(x, y, t, h, c);
        self.rect(x + w - t, y, t, h, c);
    }

    pub fn line(&mut self, a: Vertice, b: Vertice, c: Color) {
        let r = self.recorte();
        linea(&r, a.0, a.1, b.0, b.1, |x, y| self.put(x, y, c));
    }

    /// A cable: a cubic curve, `thick` pixels wide.
    pub fn curve(&mut self, a: Vertice, b: Vertice, c: Vertice, d: Vertice, color: Color, thick: i32) {
        let r = self.recorte();
        for k in 0..thick.max(1) {
            let o = k - thick / 2;
            curva(&r, (a.0 + o, a.1), (b.0 + o, b.1), (c.0 + o, c.1), (d.0 + o, d.1), |x, y| self.put(x, y, color));
        }
    }

    pub fn disc(&mut self, cx: i32, cy: i32, r: i32, c: Color) {
        for dy in -r..=r {
            let mut dx = 0;
            while (dx + 1) * (dx + 1) + dy * dy <= r * r {
                dx += 1;
            }
            self.rect(cx - dx, cy + dy, 2 * dx + 1, 1, c);
        }
    }

    /// Text, from the desktop's font, `scale` times bigger. Returns the width.
    /// Bytes the font does not have leave a gap, never garbage.
    pub fn text(&mut self, x: i32, y: i32, s: &[u8], c: Color, scale: i32) -> i32 {
        let gw = bmo::GLIFO_ANCHO as i32 * scale;
        for (i, &ch) in s.iter().enumerate() {
            let Some(rows) = bmo::glyph_bits(ch) else { continue };
            let gx = x + i as i32 * gw;
            for (row, &bits) in rows.iter().enumerate() {
                if bits == 0 {
                    continue;
                }
                for col in 0..8 {
                    if bits & (0x80 >> col) != 0 {
                        self.rect(gx + col * scale, y + row as i32 * scale, scale, scale, c);
                    }
                }
            }
        }
        s.len() as i32 * gw
    }

    /// Text cut to `max_w` pixels, so a long line never spills out of a node.
    pub fn text_fit(&mut self, x: i32, y: i32, s: &[u8], c: Color, max_w: i32) -> i32 {
        let fits = (max_w / bmo::GLIFO_ANCHO as i32).max(0) as usize;
        self.text(x, y, &s[..s.len().min(fits)], c, 1)
    }
}

impl Lienzo for Canvas {
    fn recorte(&self) -> Recorte {
        Recorte::nuevo(0, 0, self.w, self.h)
    }

    fn rect_dentro(&mut self, r: Recorte, color: Color) {
        for y in r.y0..r.y1 {
            // SAFETY: `r` is already inside the surface (that is the contract
            // of `rect_dentro`: `Lienzo::rect` clipped it before calling).
            let row = unsafe { self.px.add((y * self.w) as usize) };
            for x in r.x0..r.x1 {
                // SAFETY: as above, x in [x0, x1) is inside the row.
                unsafe { *row.add(x as usize) = color };
            }
        }
    }
}
