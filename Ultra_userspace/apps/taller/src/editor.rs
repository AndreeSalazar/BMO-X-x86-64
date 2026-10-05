//! **THE CODE OF A NODE, EDITED IN PLACE** (the owner, 05-10: "cuando le doy
//! click a los nodos en grafos tengan click derecho o doble click para
//! modificar escrituras en tiempo real").
//!
//! ```text
//!    double click on a node      its file opens in a panel on the right;
//!    (or right click > Editar)   the GRAPH stays visible on the left
//!    typing                      the text changes; Enter keeps the indent
//!                                of the line (TITAN++ blocks go by it), TAB
//!                                puts 4 spaces (a tab is T0010)
//!    stop typing                 ~1 s later it SAVES on its own: ESTRATOS
//!                                takes a version, F1 reads it back, and the
//!                                node changes -- its printer, its shapes
//!    Esc                         saves what is left and closes
//! ```
//!
//! ** THE TEXT IS THE TRUTH, still: this edits the FILE, not the node. Every
//! save is a version of ESTRATOS (`vuelve 1` in F12 undoes it), and the node
//! is what the next beat reads from that file -- the same path as a `renombra`
//! from F12 or a cable pulled in the GRAPH.
//!
//! Only the look and the keys live here, and no memory: the text is a buffer
//! `main.rs` borrows from the kernel (a block, not the 64 KiB stack), handed
//! in on every call.

use crate::aspecto::{self as look, ACCENT, AMBER, BG, CYAN, DIM, EDGE, GOLD, GOOD, INK, LEFT, LILAC, PANEL, TAB_BOX, TAB_EDGE, TITLE, TOP, VIOLET};
use crate::canvas::Canvas;
use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_titan_contrato::Name;
use bmo_titan_lector::Path;

/// It saves on its own this long after the last key.
pub const QUIET_MS: u32 = 900;
const LINE: i32 = 16;
/// The panel's width; the GRAPH keeps the rest.
const WIDTH: i32 = 560;

/// The editor while it is open. The text itself is not here (see the top).
pub struct Editor {
    /// The file, from the package's folder: `src/main.titan`.
    pub rel: Path,
    /// Whose file it is: the node's name, for the title.
    pub name: Name,
    pub len: usize,
    cursor: usize,
    /// The first line on screen.
    top: usize,
    /// Changed since the last save.
    pub dirty: bool,
    /// When the last key came: it saves `QUIET_MS` after it.
    pub last_key: u32,
    /// How many saves this opening made (each one a version of ESTRATOS).
    pub saves: u32,
}

/// What a key asks of `main.rs`.
pub enum Act {
    Stay,
    /// Save what is there, and close.
    Close,
}

/// The keys the kernel cooks (`<bmo/entrada.h>`).
pub struct Keys {
    pub up: u8,
    pub down: u8,
    pub left: u8,
    pub right: u8,
    pub home: u8,
    pub end: u8,
    pub supr: u8,
}

pub const KEYS: Keys = Keys { up: 0x80, down: 0x81, left: 0x82, right: 0x83, home: 0x84, end: 0x85, supr: 0x86 };

impl Editor {
    pub fn open(rel: Path, name: Name, len: usize) -> Editor {
        Editor { rel, name, len, cursor: 0, top: 0, dirty: false, last_key: 0, saves: 0 }
    }

    /// Where the line of `at` starts.
    fn line_start(buf: &[u8], at: usize) -> usize {
        buf[..at].iter().rposition(|&c| c == b'\n').map_or(0, |i| i + 1)
    }

    /// Where the line that starts at `start` ends (its `\n`, or the end).
    fn line_end(&self, buf: &[u8], start: usize) -> usize {
        buf[start..self.len].iter().position(|&c| c == b'\n').map_or(self.len, |i| start + i)
    }

    fn line_of(&self, buf: &[u8]) -> usize {
        buf[..self.cursor].iter().filter(|&&c| c == b'\n').count()
    }

    fn insert(&mut self, buf: &mut [u8], bytes: &[u8]) {
        let n = bytes.len();
        if self.len + n > buf.len() {
            return;
        }
        buf.copy_within(self.cursor..self.len, self.cursor + n);
        buf[self.cursor..self.cursor + n].copy_from_slice(bytes);
        self.len += n;
        self.cursor += n;
        self.dirty = true;
    }

    fn remove(&mut self, buf: &mut [u8], at: usize) {
        if at >= self.len {
            return;
        }
        buf.copy_within(at + 1..self.len, at);
        self.len -= 1;
        self.dirty = true;
    }

    /// The cursor to column `col` of the line that starts at `start`.
    fn to_column(&mut self, buf: &[u8], start: usize, col: usize) {
        self.cursor = (start + col).min(self.line_end(buf, start));
    }

    pub fn key(&mut self, buf: &mut [u8], c: u8, now: u32) -> Act {
        let k = &KEYS;
        let start = Self::line_start(buf, self.cursor);
        let col = self.cursor - start;
        match c {
            0x1B => return Act::Close,
            b'\r' | b'\n' => {
                // The indent of this line goes on to the next one.
                let indent = buf[start..self.cursor].iter().take_while(|&&c| c == b' ').count().min(32);
                let mut nl = [b' '; 33];
                nl[0] = b'\n';
                self.insert(buf, &nl[..1 + indent]);
            }
            b'\t' => self.insert(buf, b"    "),
            0x08 | 0x7F => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.remove(buf, self.cursor);
                }
            }
            x if x == k.supr => self.remove(buf, self.cursor),
            x if x == k.left => self.cursor = self.cursor.saturating_sub(1),
            x if x == k.right => self.cursor = (self.cursor + 1).min(self.len),
            x if x == k.home => self.cursor = start,
            x if x == k.end => self.cursor = self.line_end(buf, start),
            x if x == k.up => {
                if start > 0 {
                    let prev = Self::line_start(buf, start - 1);
                    self.to_column(buf, prev, col);
                }
            }
            x if x == k.down => {
                let end = self.line_end(buf, start);
                if end < self.len {
                    self.to_column(buf, end + 1, col);
                }
            }
            0x20..=0x7E => self.insert(buf, &[c]),
            _ => return Act::Stay,
        }
        self.last_key = now;
        Act::Stay
    }

    /// Time to save on its own?
    pub fn due(&self, now: u32) -> bool {
        self.dirty && now.wrapping_sub(self.last_key) >= QUIET_MS
    }

    /// Saved: the next keys start a new version.
    pub fn saved(&mut self) {
        self.dirty = false;
        self.saves += 1;
    }
}

/// The panel: on the right of the canvas, under the tabs.
pub fn frame(w: i32, h: i32) -> (i32, i32, i32, i32) {
    let pw = WIDTH.min(w - LEFT - 40);
    (w - pw - 12, TOP + 34, pw, h - PANEL - TOP - 44)
}

/// The colour of each byte of a line, the way TITAN++ reads it: words of the
/// language, texts, numbers, comments, and `print` -- the printer's green.
fn paint_line(line: &[u8], out: &mut [Color; 160]) {
    let n = line.len().min(out.len());
    let mut i = 0;
    while i < n {
        let c = line[i];
        if c == b'#' {
            out[i..n].fill(DIM);
            return;
        }
        if c == b'"' {
            let mut j = i + 1;
            while j < n && line[j] != b'"' {
                j += if line[j] == b'\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(n);
            out[i..end].fill(GOOD);
            i = end;
            continue;
        }
        if c.is_ascii_digit() {
            let end = i + line[i..n].iter().position(|b| !(b.is_ascii_digit() || *b == b'.')).unwrap_or(n - i);
            out[i..end].fill(AMBER);
            i = end;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let end = i + line[i..n].iter().position(|b| !(b.is_ascii_alphanumeric() || *b == b'_')).unwrap_or(n - i);
            let color = match &line[i..end] {
                b"fn" | b"return" | b"mod" | b"use" | b"pub" => ACCENT,
                b"let" | b"type" | b"enum" | b"trait" | b"dec" | b"int" | b"text" | b"bool" | b"f32" => mezclar(VIOLET, INK, 1, 3),
                b"mut" | b"take" => AMBER,
                b"if" | b"else" | b"match" | b"and" | b"or" | b"not" | b"true" | b"false" => GOLD,
                b"for" | b"in" | b"while" | b"break" | b"continue" | b"range" => LILAC,
                b"print" => GOOD,
                b"gpu" => GOOD,
                _ => INK,
            };
            out[i..end].fill(color);
            i = end;
            continue;
        }
        out[i] = if b"()[]{},.:=+-*/<>!%".contains(&c) { CYAN } else { INK };
        i += 1;
    }
}

pub fn draw(c: &mut Canvas, e: &mut Editor, buf: &[u8], now: u32) {
    let (x, y, w, h) = frame(c.w, c.h);
    look::shine(c, x, y, w, h, look::R_BOX, 12, VIOLET, 60);
    look::card(c, x, y, w, h, look::R_BOX, TAB_BOX);
    look::edge(c, x, y, w, h, look::R_BOX, 1, TAB_EDGE);

    // -- the title: whose file, which file, and whether it is saved ---------
    c.text(x + 14, y + 10, e.name.as_bytes(), INK, 1);
    let k = e.name.len() as i32 * 8;
    c.text_fit(x + 24 + k, y + 10, e.rel.as_bytes(), DIM, w - 200 - k);
    let (state, col): (&[u8], Color) = if e.dirty { (b"escribiendo...", AMBER) } else if e.saves > 0 { (b"guardado", GOOD) } else { (b"abierto", DIM) };
    look::pill(c, x + w - 14 - (state.len() as i32 * 8 + 14), y + 8, state, col, mezclar(col, TAB_BOX, 1, 6));
    look::ring_line(c, x + 12, y + 32, w - 24);

    // -- the text --------------------------------------------------------
    let (tx, ty) = (x + 48, y + 42);
    let rows = ((h - 42 - 30) / LINE).max(1) as usize;
    let cols = ((w - 48 - 14) / 8).max(1) as usize;
    let cur_line = e.line_of(buf);
    if cur_line < e.top {
        e.top = cur_line;
    } else if cur_line >= e.top + rows {
        e.top = cur_line + 1 - rows;
    }
    let cur_col = e.cursor - Editor::line_start(buf, e.cursor);
    let left = (cur_col + 1).saturating_sub(cols);
    let mut colors = [INK; 160];
    let mut start = 0;
    let mut line = 0;
    while start <= e.len && line < e.top + rows {
        let end = e.line_end(buf, start);
        if line >= e.top {
            let ly = ty + (line - e.top) as i32 * LINE;
            if line == cur_line {
                look::card(c, x + 8, ly - 1, w - 16, LINE + 1, 3, mezclar(ACCENT, TAB_BOX, 1, 8));
            }
            // the line number
            let mut d = [b' '; 4];
            let mut v = line + 1;
            for slot in d.iter_mut().rev() {
                *slot = b'0' + (v % 10) as u8;
                v /= 10;
                if v == 0 {
                    break;
                }
            }
            c.text(x + 10, ly, &d, if line == cur_line { TITLE } else { mezclar(DIM, BG, 2, 3) }, 1);
            let text = &buf[start..end];
            paint_line(text, &mut colors);
            for (i, &b) in text.iter().enumerate().skip(left).take(cols) {
                c.text(tx + (i - left) as i32 * 8, ly, &[b], colors[i.min(159)], 1);
            }
            if line == cur_line && (now / 500) % 2 == 0 {
                c.rect(tx + (cur_col - left) as i32 * 8, ly, 2, LINE, ACCENT);
            }
        }
        if end >= e.len {
            break;
        }
        start = end + 1;
        line += 1;
    }
    c.rect(x + 44, ty - 4, 1, rows as i32 * LINE + 4, EDGE);

    // -- what the keys do ------------------------------------------------
    c.text_fit(x + 14, y + h - 22, b"se guarda solo al parar de escribir   Esc guarda y cierra", DIM, w - 28);
}
