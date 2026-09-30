//! **THE FAULTS** -- a node that is wrong LOOKS wrong, and when there are
//! several, the canvas takes you from one to the next (`PLAN_TALLER` 8.9).
//!
//! ```text
//!    one fault      a thick red border, a red halo that BREATHES, a wave that
//!                   leaves the node every so often, and an ERROR badge that
//!                   floats above it
//!    several        numbered (ERROR 1/3, 2/3, ...) and joined in order by a
//!                   red path of MARCHING dashes: the way from one to the next.
//!                   [e] takes the camera to the next one
//! ```
//!
//! Inspired by the node editors of Houdini and Blender, where a node in error
//! is red and you find it at a glance -- not copied: theirs are still, this
//! one moves, because a fault that moves is a fault that is not missed.
//!
//! WHERE a fault comes from, and on which node it lands:
//!
//! ```text
//!    the reader's problems      MissingModule -> the parent that declares it
//!                               UnknownUse, NoPermission -> the one that uses
//!                               Duplicate -> that name; Cycle -> both ends
//!                               NoMain, BadManifest... -> the root
//!    the checker's NO on show   Conflict -> both nodes; Denied -> that node
//! ```
//!
//! A problem that names no node that exists (a header that does not parse, a
//! file whose name does not agree) stays in the EXPLORER's PROBLEMAS: inventing
//! a node for it would point at the wrong place.
//!
//! [consumo] it moves only while the cables' pulse does (seen, and touched in
//! the last 20 s); at rest the red stays, still.

use crate::canvas::Canvas;
use crate::view::{node_rect, Camera, BAD, INK};
use bmo_dibujo::{Color, Lienzo};
use bmo_titan_contrato::{EventKind, Graph, NodeId, MAX_NODES};
use bmo_titan_lector::{Loaded, ProblemKind};

/// The nodes in fault, in the order the path visits them (top to bottom,
/// left to right: the order the eye reads a graph).
#[derive(Clone, Copy)]
pub struct Marks {
    ids: [NodeId; MAX_NODES],
    n: usize,
}

impl Marks {
    pub fn get(&self) -> &[NodeId] {
        &self.ids[..self.n]
    }

    fn add(&mut self, id: NodeId) {
        if self.n < MAX_NODES && !self.get().contains(&id) {
            self.ids[self.n] = id;
            self.n += 1;
        }
    }

    /// The fault after `from` in the path, or the first.
    pub fn next_after(&self, from: Option<NodeId>) -> Option<NodeId> {
        let at = from.and_then(|f| self.get().iter().position(|&m| m == f));
        match at {
            Some(i) => self.get().get((i + 1) % self.n).copied(),
            None => self.get().first().copied(),
        }
    }
}

/// The faults of a package, and of the checker's event on show (if any).
pub fn collect(l: &Loaded, current: Option<EventKind>) -> Marks {
    let g = &l.graph;
    let mut m = Marks { ids: [NodeId(0); MAX_NODES], n: 0 };
    let find = |name: &[u8]| g.find(name);
    for p in l.problems() {
        let (a, b) = (p.a.as_bytes(), p.b.as_bytes());
        match p.kind {
            ProblemKind::MissingModule => find(b).into_iter().for_each(|id| m.add(id)),
            ProblemKind::UnknownUse | ProblemKind::NoPermission(_) | ProblemKind::Duplicate => {
                find(a).into_iter().for_each(|id| m.add(id))
            }
            ProblemKind::Cycle => {
                find(a).into_iter().for_each(|id| m.add(id));
                find(b).into_iter().for_each(|id| m.add(id));
            }
            ProblemKind::NoManifest | ProblemKind::BadManifest(_) | ProblemKind::NoMain | ProblemKind::Full => {
                g.root().into_iter().for_each(|id| m.add(id))
            }
            ProblemKind::BadHeader(_) | ProblemKind::NameMismatch | ProblemKind::TooBig => {}
        }
    }
    match current {
        Some(EventKind::Conflict { first, second, .. }) => {
            m.add(first.node);
            m.add(second.node);
        }
        Some(EventKind::Denied { node, .. }) => m.add(node),
        _ => {}
    }
    // The order of the path: top to bottom, then left to right.
    let key = |id: &NodeId| g.node(*id).map(|n| (n.y, n.x)).unwrap_or((i32::MAX, i32::MAX));
    m.ids[..m.n].sort_unstable_by_key(key);
    m
}

/// 0..=1000 and back, once every `period` ms: a breath.
fn breath(now: u32, period: u32) -> u32 {
    let t = now % period;
    let half = period / 2;
    if t < half { t * 1000 / half } else { (period - t) * 1000 / half }
}

fn centre(g: &Graph, cam: &Camera, id: NodeId) -> Option<(i32, i32)> {
    let (x, y, w, h) = node_rect(cam, g.node(id)?);
    Some((x + w / 2, y + h / 2))
}

/// UNDER the nodes: the marching path between faults, their breathing halo
/// and the waves. `lively`: false draws it all still.
pub fn draw_under(c: &mut Canvas, g: &Graph, cam: &Camera, marks: &Marks, now: u32, lively: bool) {
    let ids = marks.get();
    let t = if lively { now } else { 0 };
    // The path: dashes of 10, gaps of 6, marching toward the next fault.
    for pair in ids.windows(2) {
        let (Some(a), Some(b)) = (centre(g, cam, pair[0]), centre(g, cam, pair[1])) else { continue };
        let (dx, dy) = ((b.0 - a.0) as i64, (b.1 - a.1) as i64);
        let len = isqrt(dx * dx + dy * dy);
        if len == 0 {
            continue;
        }
        let shift = (t / 30) as i64 % 16;
        for s in 0..len {
            if (s + 16 - shift) % 16 >= 10 {
                continue;
            }
            let (x, y) = ((a.0 as i64 + dx * s / len) as i32, (a.1 as i64 + dy * s / len) as i32);
            for o in -1..=1 {
                c.blend(x + o, y, BAD, 70, 100);
                c.blend(x, y + o, BAD, 70, 100);
            }
        }
    }
    let b = if lively { breath(now, 1000) } else { 600 };
    for (i, &id) in ids.iter().enumerate() {
        let Some(n) = g.node(id) else { continue };
        let (x, y, w, h) = node_rect(cam, n);
        c.glow(x, y, w, h, BAD, 12, 25 + 45 * b / 1000);
        if lively {
            // The wave: a ring that leaves the node and fades as it goes. Each
            // fault on its own phase, like the cables.
            let t = (now + i as u32 * 470) % 1400;
            let off = 4 + (t * 30 / 1400) as i32;
            let part = 70 * (1400 - t) / 1400;
            ring(c, x - off, y - off, w + 2 * off, h + 2 * off, part);
        }
    }
}

/// ON TOP of the nodes: the thick border and the ERROR badge.
pub fn draw_over(c: &mut Canvas, g: &Graph, cam: &Camera, marks: &Marks, now: u32, lively: bool) {
    let ids = marks.get();
    for (i, &id) in ids.iter().enumerate() {
        let Some(n) = g.node(id) else { continue };
        let (x, y, w, h) = node_rect(cam, n);
        c.frame(x - 2, y - 2, w + 4, h + 4, 3, BAD);
        // The badge floats: two pixels up and down, on its own phase.
        let bob = if lively { breath(now + i as u32 * 250, 900) as i32 * 4 / 1000 - 2 } else { 0 };
        let mut label = [0u8; 16];
        let text = badge_text(&mut label, i + 1, ids.len());
        let bw = text.len() as i32 * 8 + 14;
        // Astride the top-right corner of its own header: half out, half on it,
        // so it never covers the node above (they sit ~20 px apart).
        let (bx, by) = (x + w - bw - 6, y - 10 + bob);
        c.rect(bx, by, bw, 20, BAD);
        c.frame(bx, by, bw, 20, 1, mix_white(BAD));
        // Bold: the same text twice, one pixel apart.
        c.text(bx + 7, by + 2, text, INK, 1);
        c.text(bx + 8, by + 2, text, INK, 1);
    }
}

/// `ERROR`, or `ERROR 2/3` when there are several.
fn badge_text(buf: &mut [u8; 16], k: usize, of: usize) -> &[u8] {
    let head = b"ERROR";
    buf[..5].copy_from_slice(head);
    if of < 2 {
        return &buf[..5];
    }
    let mut n = 5;
    let mut put = |b: u8, n: &mut usize| {
        if *n < buf.len() {
            buf[*n] = b;
            *n += 1;
        }
    };
    put(b' ', &mut n);
    for d in digits(k) {
        put(d, &mut n);
    }
    put(b'/', &mut n);
    for d in digits(of) {
        put(d, &mut n);
    }
    &buf[..n]
}

/// The decimal digits of a small number (faults fit in two).
fn digits(v: usize) -> impl Iterator<Item = u8> {
    let (hi, lo) = ((v / 10 % 10) as u8, (v % 10) as u8);
    (hi > 0).then_some(b'0' + hi).into_iter().chain(core::iter::once(b'0' + lo))
}

/// The integer square root: `core` has no `sqrt` without `std`.
fn isqrt(v: i64) -> i64 {
    if v <= 0 {
        return 0;
    }
    let mut x = v;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + v / x) / 2;
    }
    x
}

fn mix_white(c: Color) -> Color {
    bmo_dibujo::mezclar(0x00FF_FFFF, c, 1, 3)
}

/// A one-pixel ring of red around a box, `part` percent over what is there.
fn ring(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, part: u32) {
    for i in x..x + w {
        c.blend(i, y, BAD, part, 100);
        c.blend(i, y + h - 1, BAD, part, 100);
    }
    for j in y + 1..y + h - 1 {
        c.blend(x, j, BAD, part, 100);
        c.blend(x + w - 1, j, BAD, part, 100);
    }
}
