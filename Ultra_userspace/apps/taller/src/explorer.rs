//! **THE EXPLORER** -- the left column: the library, the files of the package
//! on screen as the tree the CODE declares, and its problems, straight from
//! ESTRATOS (`PLAN_TALLER` 8.6 and 8.7).
//!
//! ```text
//!    EXPLORER
//!    ESTRATOS gen 1234         where it comes from, and at which commit
//!    BIBLIOTECA                titan/biblioteca.toml: click to open one
//!      > asteroids
//!    ARCHIVOS                  the tree of `mod`, not of the folders
//!      Titan.toml
//!      | main.titan            click: that node lights up in the canvas
//!      | | physics.titan       drag onto another file (or its node): it
//!      | | | collide.titan       hangs there; green = it can, red = why not
//!    PROBLEMAS                 what the reader could not follow, and why
//!    ...
//!    EN DISCO                  where the selected file REALLY lives
//! ```
//!
//! The disk may be in any order: the PARENT says where its child is (`mod x
//! in "..."`), and this column shows the hierarchy the parents declare. The
//! last line says where the bytes are, so the order on screen never hides
//! the order on disk.
//!
//! Inspired by the explorers of Windows and VS Code for their SIMPLICITY
//! (the owner, 18-08: "me inspiro... pero no soy Windows"), not copied: there
//! are no buttons that do nothing. ONE function lays the rows out (`rows`), and
//! drawing, clicking and dropping all walk it: a click always lands on the row
//! that was drawn there.

use crate::canvas::Canvas;
use crate::store::{Origin, Store};
use crate::view::{self, Buf, Camera, ACCENT, BAD, BAR, BG, DIM, EDGE, GOOD, INK, LEFT, TITLE, TOP};
use bmo_dibujo::{Color, Lienzo};
use bmo_titan_contrato::NodeId;
use bmo_titan_lector::hang::{self, HangError};

const ROW: i32 = 18;
const PAD: i32 = 10;
/// One step of the tree.
const STEP: i32 = 12;
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
            // At a space (which goes), or just after a `/` (which stays on
            // the first line, as a path is read).
            match rest[..CHARS].iter().rposition(|&c| c == b' ' || c == b'/') {
                Some(i) if rest[i] == b'/' => i + 1,
                Some(i) if i > 0 => i,
                _ => CHARS,
            }
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

/// Where a file row's name starts: one step per level of the declared tree.
fn indent(depth: u8) -> i32 {
    PAD + 4 + depth as i32 * STEP
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
            let lit = selected == Some(f.node);
            if lit {
                c.rect(0, y - 1, LEFT - 1, ROW, EDGE);
            }
            // The guides of the tree: one thin line per level above it.
            for level in 1..=f.depth as i32 {
                c.rect(PAD + 4 + (level - 1) * STEP + 3, y - 1, 1, ROW, EDGE);
            }
            let path = f.path.as_bytes();
            let name = path.rsplit(|&b| b == b'/').next().unwrap_or(path);
            let x = indent(f.depth);
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
    footer(c, store, selected);
}

/// The bottom of the column: what the last drop did, and where the selected
/// file's bytes really are.
fn footer(c: &mut Canvas, store: &Store, selected: Option<NodeId>) {
    let file = selected.and_then(|id| store.loaded.file_of(id));
    let pkg = store.packages().get(store.chosen);
    let mut disk = Buf::new();
    if let (Some(f), Some(p)) = (file, pkg) {
        disk.b(p.1.as_bytes()).s("/").b(f.path.as_bytes());
    }
    let note_lines = store.note.map(|(n, _)| pieces(n.as_bytes()).take(2).count()).unwrap_or(0);
    let disk_lines = match disk.get() {
        [] => 0,
        d => 1 + pieces(d).take(2).count(),
    };
    if note_lines + disk_lines == 0 {
        return;
    }
    // As tall as what it says: counted first, so nothing falls off the bottom.
    let mut y = c.h - 8 - (note_lines + disk_lines) as i32 * ROW;
    c.rect(0, y - 8, LEFT - 1, c.h - y + 8, BG);
    c.rect(0, y - 8, LEFT - 1, 1, EDGE);
    if let Some((note, done)) = &store.note {
        let ok = *done;
        for line in pieces(note.as_bytes()).take(2) {
            c.text(PAD, y, line, if ok { GOOD } else { BAD }, 1);
            y += ROW;
        }
    }
    if disk_lines > 0 {
        c.text(PAD, y, b"EN DISCO", DIM, 1);
        y += ROW;
        for line in pieces(disk.get()).take(2) {
            c.text(PAD, y, line, INK, 1);
            y += ROW;
        }
    }
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

/// Where a dragged file would land: a file row of the column, or a node of the
/// canvas -- the same module either way.
pub fn drop_target(store: &Store, cam: &Camera, x: i32, y: i32) -> Option<NodeId> {
    if x < LEFT {
        match click(store, x, y) {
            Some(Click::File(id)) => Some(id),
            _ => None,
        }
    } else if y > TOP {
        view::hit(&store.loaded.graph, cam, x, y)
    } else {
        None
    }
}

/// A file being dragged: its target lit green (it can hang there) or red
/// (with why not), and the name following the pointer. Drawn last, on top.
pub fn draw_drag(c: &mut Canvas, store: &Store, cam: &Camera, node: NodeId, x: i32, y: i32) {
    let target = drop_target(store, cam, x, y);
    let verdict = target.map(|t| (t, store.plan(node, t)));
    let mut why = None;
    if let Some((t, v)) = verdict {
        let color = match v {
            Ok(_) => GOOD,
            Err(HangError::Same) => EDGE,
            Err(e) => {
                why = hang::note(Err(e), name(store, node), name(store, t));
                BAD
            }
        };
        if let Some((rx, ry, rw, rh)) = row_or_node(store, cam, t, x) {
            c.frame(rx, ry, rw, rh, 2, color);
        }
    }
    // The ghost: the file's name on a chip, next to the pointer.
    let label = store.loaded.file_of(node).map(|f| f.path.as_bytes()).unwrap_or(b"");
    let label = label.rsplit(|&b| b == b'/').next().unwrap_or(label);
    let w = label.len() as i32 * 8 + 12;
    c.rect(x + 12, y + 10, w, ROW + 2, BAR);
    c.frame(x + 12, y + 10, w, ROW + 2, 1, ACCENT);
    c.text(x + 18, y + 12, label, ACCENT, 1);
    if let Some(line) = why {
        let tw = line.len() as i32 * 8 + 12;
        c.rect(x + 12, y + 12 + ROW, tw, ROW + 2, BG);
        c.text(x + 18, y + 14 + ROW, line.as_bytes(), BAD, 1);
    }
}

fn name(store: &Store, id: NodeId) -> &[u8] {
    store.loaded.graph.node(id).map(|n| n.name.as_bytes()).unwrap_or(b"?")
}

/// The box to light for a target: its row when the pointer is in the column,
/// its node when it is on the canvas.
fn row_or_node(store: &Store, cam: &Camera, t: NodeId, x: i32) -> Option<(i32, i32, i32, i32)> {
    if x >= LEFT {
        let n = store.loaded.graph.node(t)?;
        let (nx, ny, nw, nh) = view::node_rect(cam, n);
        return Some((nx - 3, ny - 3, nw + 6, nh + 6));
    }
    let mut found = None;
    rows(store, |y, h, row| {
        if let Row::File(i) = row {
            if store.loaded.files().get(i).map(|f| f.node) == Some(t) {
                found = Some((1, y - 2, LEFT - 3, h + 2));
            }
        }
    });
    found
}
