//! **THE EXPLORER** -- the left column: the library, the package AS IT IS ON
//! DISK in the order the owner chose, and its problems, straight from ESTRATOS
//! (`PLAN_TALLER` 8.6, 8.7 and 8.10).
//!
//! ```text
//!    EXPLORER
//!    ESTRATOS gen 1234         where it comes from, and at which commit
//!    BIBLIOTECA                titan/biblioteca.toml: click to open one
//!      > asteroids
//!    ARCHIVOS        [+] [+]   a new file, a new folder (where the pick is)
//!      Titan.toml
//!      v src                   click the arrow: fold or open (remembered)
//!        main.titan            a module: click lights its node
//!        notas.txt             any file: the disk, not only what `mod` names
//!        > physics             a folded folder
//!    PROBLEMAS                 what the reader could not follow, and why
//!    ...
//!    EN DISCO                  where the picked item REALLY lives
//! ```
//!
//! ** The owner (04-10): "organizar a mi manera ... sin pelear con orden de A
//! hasta la Z". Drag a row onto another of the same folder: it goes before or
//! after it, and the order is saved in Titan.toml. Drag it onto a folder: it
//! moves in. Drag a `.titan` onto a NODE of the canvas: that module declares
//! it (or it hangs there, if it was declared already). The column organizes
//! the DISK; the canvas organizes the MODULES.
//!
//! Like VS Code: a double click or F2 renames (if the desktop lets F2 through:
//! F1-F10 are its own), Supr removes (twice, it asks), the right button opens
//! the menu, the arrows walk the rows. There are no buttons that do nothing.
//! ONE function lays the rows out (`rows`), and drawing, clicking and dropping
//! all walk it: a click always lands on the row that was drawn there.
//!
//! Without ESTRATOS (the sample in memory) there is no disk to show: the column
//! falls back to the tree the code declares, as before 8.10.

use crate::canvas::Canvas;
use crate::store::{Origin, Store};
use crate::view::{self, picked_row, ring_line, Buf, Camera, ACCENT, BAD, BAR, BG, BLUE, DIM, EDGE, GOOD, INK, LEFT, TITLE, TOP, VIOLET};
use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_titan_contrato::NodeId;
use bmo_titan_lector::explorer::{Tree, NAME_MAX};
use bmo_titan_lector::hang::{self, HangError};

const ROW: i32 = 18;
const PAD: i32 = 10;
/// One step of the tree.
const STEP: i32 = 12;
/// Characters that fit in the column.
const CHARS: usize = ((LEFT - 2 * PAD) / 8) as usize;

// -- The icons: 8x10, bit 7 the leftmost pixel. Drawn with the same `rect` as
// everything else; no image, no font.

/// A page with its corner folded: the outline of every file.
const PAGE: [u8; 10] = [0xF8, 0x8C, 0x8E, 0x82, 0x82, 0x82, 0x82, 0x82, 0x82, 0xFE];
/// A `T` inside the page: a `.titan`.
const TEE: [u8; 10] = [0, 0, 0, 0, 0x38, 0x10, 0x10, 0x10, 0, 0];
/// Lines inside the page: `Titan.toml`, the manifest.
const LINES: [u8; 10] = [0, 0, 0, 0, 0x38, 0, 0x38, 0, 0x30, 0];
/// A box: a package of the library.
const BOX: [u8; 10] = [0x38, 0x44, 0x82, 0xC6, 0xBA, 0x92, 0x92, 0x92, 0x54, 0x38];
/// A warning: a problem.
const WARN: [u8; 10] = [0x10, 0x28, 0x38, 0x54, 0x54, 0x92, 0x82, 0x92, 0xFE, 0];
/// A disk: where the bytes are.
const DISK: [u8; 10] = [0x7C, 0x82, 0x7C, 0x82, 0x82, 0x82, 0x82, 0x82, 0x7C, 0];
/// A folder, closed and open.
const FOLDER: [u8; 10] = [0, 0x70, 0x8F, 0x81, 0x81, 0x81, 0x81, 0x81, 0xFF, 0];
const FOLDER_OPEN: [u8; 10] = [0, 0x70, 0x8F, 0x80, 0xFF, 0x82, 0x84, 0x88, 0xF0, 0];
/// The arrows of a folder: open (down) and folded (right).
const DOWN: [u8; 10] = [0, 0, 0, 0xFE, 0x7C, 0x38, 0x10, 0, 0, 0];
const RIGHT: [u8; 10] = [0, 0x20, 0x30, 0x38, 0x3C, 0x38, 0x30, 0x20, 0, 0];
/// The two buttons of ARCHIVOS: a page with a `+`, a folder with a `+`.
const NEW_FILE: [u8; 10] = [0xF8, 0x8C, 0x8E, 0x92, 0xBA, 0x92, 0x82, 0x82, 0x82, 0xFE];
const NEW_FOLDER: [u8; 10] = [0, 0x70, 0x8F, 0x81, 0x91, 0xBB, 0x91, 0x81, 0xFF, 0];

fn icon(c: &mut Canvas, x: i32, y: i32, bits: &[u8; 10], color: Color) {
    for (row, &b) in bits.iter().enumerate() {
        for col in 0..8 {
            if b & (0x80 >> col) != 0 {
                c.rect(x + col, y + 3 + row as i32, 1, 1, color);
            }
        }
    }
}

/// Where the text of a row starts, after its icon.
const ICON_W: i32 = 12;
/// Where the two buttons of ARCHIVOS sit.
const BTN_FILE: i32 = LEFT - PAD - 30;
const BTN_FOLDER: i32 = LEFT - PAD - 12;

/// What is being typed in the column, VS Code style: a row with a box.
#[derive(Clone, Copy)]
pub enum EditKind {
    /// A new file (or folder) in this folder; `None` = the package folder.
    NewFile(Option<usize>),
    NewFolder(Option<usize>),
    Rename(usize),
}

#[derive(Clone, Copy)]
pub struct Edit {
    pub kind: EditKind,
    pub text: [u8; NAME_MAX],
    pub len: usize,
}

impl Edit {
    pub fn new(kind: EditKind, start: &[u8]) -> Edit {
        let mut e = Edit { kind, text: [0; NAME_MAX], len: 0 };
        for &c in start {
            e.push(c);
        }
        e
    }
    pub fn push(&mut self, c: u8) {
        if self.len < NAME_MAX {
            self.text[self.len] = c;
            self.len += 1;
        }
    }
    pub fn get(&self) -> &[u8] {
        &self.text[..self.len]
    }
}

/// The entries of the right button's menu.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    NewFile,
    NewFolder,
    Rename,
    Remove,
}

const MENU: [(Entry, &[u8]); 4] = [
    (Entry::NewFile, b"Nuevo archivo"),
    (Entry::NewFolder, b"Nueva carpeta"),
    (Entry::Rename, b"Renombrar   F2"),
    (Entry::Remove, b"Quitar      Supr"),
];
const MENU_W: i32 = 150;

#[derive(Clone, Copy)]
pub struct Menu {
    pub x: i32,
    pub y: i32,
    /// The item it was opened on; `None` = the empty part of the column.
    pub item: Option<usize>,
}

impl Menu {
    fn entries(&self) -> usize {
        if self.item.is_some() {
            4
        } else {
            2
        }
    }
    fn rect(&self) -> (i32, i32, i32, i32) {
        (self.x, self.y, MENU_W, self.entries() as i32 * ROW + 8)
    }
    /// The entry under (x, y), if any.
    pub fn hit(&self, x: i32, y: i32) -> Option<Entry> {
        let (mx, my, w, h) = self.rect();
        if x < mx || x >= mx + w || y < my + 4 || y >= my + h - 4 {
            return None;
        }
        MENU.get(((y - my - 4) / ROW) as usize).filter(|_| ((y - my - 4) / ROW) < self.entries() as i32).map(|e| e.0)
    }
}

/// The state of the column that is not on disk: what is picked, what is being
/// typed, the menu, and a removal waiting for its second Supr.
pub struct Ui {
    pub picked: Option<usize>,
    pub edit: Option<Edit>,
    pub menu: Option<Menu>,
    /// The item that a second Supr removes, until this millisecond.
    pub confirm: Option<(usize, u32)>,
}

impl Ui {
    pub const fn new() -> Ui {
        Ui { picked: None, edit: None, menu: None, confirm: None }
    }
}

/// Where in an item's row a click fell.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Arrow,
    Body,
}

/// What a click in the column asks for.
pub enum Click {
    Package(usize),
    /// A declared file, when there is no disk tree (the sample in memory).
    File(NodeId),
    Item(usize, Zone),
    NewFile,
    NewFolder,
    /// The row with the typing box.
    Typing,
}

#[derive(Clone, Copy)]
enum Row {
    Heading(&'static str),
    Origin,
    Package(usize),
    /// ARCHIVOS, with its two buttons when there is a disk tree.
    Files,
    File(usize),
    /// An item of the disk tree, at this depth.
    Item(usize, u8),
    /// The typing box, at this depth.
    Typing(u8),
    /// The tree did not fit: it says so.
    Short,
    /// Problem `i`, its line `part` (a message takes up to three).
    Problem(usize, usize),
    More,
}

/// The disk tree, if there is one to show.
fn tree(store: &Store) -> Option<&Tree> {
    store.tree.as_deref().filter(|t| !t.is_empty())
}

/// Walks the rows top to bottom: `f(y, height, row)`.
fn rows(store: &Store, ui: &Ui, mut f: impl FnMut(i32, i32, Row)) {
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
    put(22, Row::Files);
    match tree(store) {
        Some(t) => {
            let typing = ui.edit.map(|e| e.kind);
            if let Some(EditKind::NewFile(None) | EditKind::NewFolder(None)) = typing {
                put(ROW, Row::Typing(0));
            }
            for (i, depth) in t.rows() {
                match typing {
                    Some(EditKind::Rename(r)) if r == i => put(ROW, Row::Typing(depth)),
                    _ => put(ROW, Row::Item(i, depth)),
                }
                if let Some(EditKind::NewFile(Some(f)) | EditKind::NewFolder(Some(f))) = typing {
                    if f == i {
                        put(ROW, Row::Typing(depth + 1));
                    }
                }
            }
            if t.cut() {
                put(ROW, Row::Short);
            }
        }
        None => {
            for i in 0..store.loaded.files().len() {
                put(ROW, Row::File(i));
            }
        }
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

/// Where a row's name starts: one step per level, after room for the arrow.
fn indent(depth: u8) -> i32 {
    PAD + 14 + depth as i32 * STEP
}

/// The node of a disk item, if a `mod` declares its file.
pub fn node_of(store: &Store, i: usize) -> Option<NodeId> {
    let rel = tree(store)?.path_of(i)?;
    store.loaded.files().iter().find(|f| f.depth > 0 && f.path.as_bytes() == rel.as_bytes()).map(|f| f.node)
}

/// The disk item of a node's file (the canvas picked it: light its row).
pub fn item_of(store: &Store, node: NodeId) -> Option<usize> {
    let f = store.loaded.file_of(node)?;
    tree(store)?.find(f.path.as_bytes())
}

/// The guides of the tree: one thin line per level above a row.
fn guides(c: &mut Canvas, y: i32, depth: u8) {
    for level in 0..depth as i32 {
        c.rect(indent(level as u8) + 3, y - 1, 1, ROW, EDGE);
    }
}

pub fn draw(c: &mut Canvas, store: &Store, ui: &Ui, selected: Option<NodeId>, now_ms: u32) {
    c.rect(0, TOP, LEFT, c.h - TOP, BAR);
    // Its edge is the logo's ring, top to bottom: blue into violet.
    let tall = (c.h - TOP) as u32;
    for y in TOP..c.h {
        c.put(LEFT - 1, y, mezclar(VIOLET, BLUE, (y - TOP) as u32, tall));
    }
    let files = store.loaded.files();
    // The persistent faults (the reader's), the same the canvas marks.
    let faults = crate::faults::collect(&store.loaded, None);
    let t = tree(store);
    let doomed = ui.confirm.map(|(i, _)| i);
    rows(store, ui, |y, _h, row| match row {
        Row::Heading("EXPLORER") => {
            c.text(PAD, y, b"EXPLORER", TITLE, 1);
        }
        Row::Heading(label) => {
            c.text(PAD, y + 4, label.as_bytes(), DIM, 1);
        }
        Row::Files => {
            c.text(PAD, y + 4, b"ARCHIVOS", DIM, 1);
            if t.is_some() {
                icon(c, BTN_FILE, y + 2, &NEW_FILE, ACCENT);
                icon(c, BTN_FOLDER, y + 2, &NEW_FOLDER, ACCENT);
            }
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
                picked_row(c, 0, y - 1, LEFT - 1, ROW);
            }
            icon(c, PAD, y, &BOX, if chosen { ACCENT } else { VIOLET });
            let x = PAD + ICON_W;
            c.text_fit(x, y, store.packages()[i].0.as_bytes(), if chosen { ACCENT } else { INK }, LEFT - x - PAD);
        }
        Row::Item(i, depth) => {
            let Some(t) = t else { return };
            let node = node_of(store, i);
            let lit = ui.picked == Some(i) || (node.is_some() && node == selected);
            if lit {
                picked_row(c, 0, y - 1, LEFT - 1, ROW);
            }
            guides(c, y, depth);
            let x = indent(depth);
            let name = t.name(i);
            if t.is_folder(i) {
                let folded = t.is_folded(i);
                icon(c, x - 11, y, if folded { &RIGHT } else { &DOWN }, DIM);
                icon(c, x, y, if folded { &FOLDER } else { &FOLDER_OPEN }, if lit { ACCENT } else { VIOLET });
            } else if name.ends_with(b".titan") {
                icon(c, x, y, &PAGE, if lit { ACCENT } else { BLUE });
                icon(c, x, y, &TEE, if lit { ACCENT } else { INK });
            } else if name == b"Titan.toml" && depth == 0 {
                icon(c, x, y, &PAGE, if lit { ACCENT } else { VIOLET });
                icon(c, x, y, &LINES, if lit { ACCENT } else { INK });
            } else {
                icon(c, x, y, &PAGE, if lit { ACCENT } else { DIM });
            }
            // A module in fault in red; a `.titan` nobody declares, dim: it is
            // on disk, and not a node until a module declares it.
            let ink = match node {
                Some(n) if faults.get().contains(&n) => BAD,
                None if name.ends_with(b".titan") => DIM,
                _ if lit => ACCENT,
                _ => INK,
            };
            let ink = if doomed == Some(i) && (now_ms / 250) % 2 == 0 { BAD } else { ink };
            c.text_fit(x + ICON_W, y, name, ink, LEFT - x - ICON_W - PAD);
        }
        Row::Typing(depth) => {
            let Some(e) = ui.edit else { return };
            guides(c, y, depth);
            let x = indent(depth);
            let folder = matches!(e.kind, EditKind::NewFolder(_)) || matches!(e.kind, EditKind::Rename(i) if t.is_some_and(|t| t.is_folder(i)));
            icon(c, x, y, if folder { &FOLDER } else { &PAGE }, ACCENT);
            let bx = x + ICON_W - 2;
            c.rect(bx, y - 1, LEFT - bx - PAD + 2, ROW, BG);
            c.frame(bx, y - 1, LEFT - bx - PAD + 2, ROW, 1, ACCENT);
            let room = (LEFT - bx - PAD - 10) / 8;
            let text = e.get();
            // The end of the name is what is being typed: keep it in view.
            let shown = &text[text.len().saturating_sub(room.max(1) as usize)..];
            // `text` returns the WIDTH: the caret goes after it, not at it.
            let end = bx + 3 + c.text(bx + 3, y, shown, INK, 1);
            if (now_ms / 500) % 2 == 0 {
                c.rect(end + 1, y + 1, 2, ROW - 4, ACCENT);
            }
        }
        Row::Short => {
            c.text(PAD + 14, y, b"... el arbol no cabe entero", BAD, 1);
        }
        Row::File(i) => {
            let f = &files[i];
            let lit = selected == Some(f.node);
            if lit {
                picked_row(c, 0, y - 1, LEFT - 1, ROW);
            }
            guides(c, y, f.depth);
            let path = f.path.as_bytes();
            let name = path.rsplit(|&b| b == b'/').next().unwrap_or(path);
            let x = indent(f.depth);
            // The manifest in violet with its lines; a module in blue with its T.
            let (outline, inside) = if f.depth == 0 { (VIOLET, &LINES) } else { (BLUE, &TEE) };
            icon(c, x, y, &PAGE, if lit { ACCENT } else { outline });
            icon(c, x, y, inside, if lit { ACCENT } else { INK });
            let ink = if faults.get().contains(&f.node) { BAD } else if lit { ACCENT } else { INK };
            c.text_fit(x + ICON_W, y, name, ink, LEFT - x - ICON_W - PAD);
        }
        Row::Problem(i, part) => {
            let text = store.loaded.problems()[i].describe();
            if part == 0 {
                icon(c, PAD, y, &WARN, BAD);
            }
            if let Some(line) = pieces(text.as_bytes()).nth(part) {
                c.text(PAD + ICON_W, y, line, BAD, 1);
            };
        }
        Row::More => {
            let mut t = Buf::new();
            t.s("y ").num(store.loaded.more_problems as u32).s(" mas");
            c.text(PAD, y, t.get(), BAD, 1);
        }
    });
    footer(c, store, ui, selected);
    if let Some(m) = &ui.menu {
        menu(c, m);
    }
}

fn menu(c: &mut Canvas, m: &Menu) {
    let (x, y, w, h) = m.rect();
    c.glow(x, y, w, h, VIOLET, 6, 60);
    c.rect(x, y, w, h, BG);
    c.frame(x, y, w, h, 1, ACCENT);
    for (k, (_, label)) in MENU.iter().take(m.entries()).enumerate() {
        c.text(x + 10, y + 4 + k as i32 * ROW + 1, label, INK, 1);
    }
}

/// The bottom of the column: what the last gesture did (and what it left, or
/// why not), and where the picked item's bytes really are.
fn footer(c: &mut Canvas, store: &Store, ui: &Ui, selected: Option<NodeId>) {
    let pkg = store.packages().get(store.chosen);
    let mut disk = Buf::new();
    if let Some(p) = pkg {
        match (tree(store), ui.picked) {
            (Some(t), Some(i)) => {
                if let Some(rel) = t.path_of(i) {
                    disk.b(p.1.as_bytes()).s("/").b(rel.as_bytes());
                }
            }
            (Some(_), None) => {}
            (None, _) => {
                if let Some(f) = selected.and_then(|id| store.loaded.file_of(id)) {
                    disk.b(p.1.as_bytes()).s("/").b(f.path.as_bytes());
                }
            }
        }
    }
    let ok = store.note.map(|(_, d)| d).unwrap_or(true);
    // The note in two lines of the column at most; its second line in three.
    let note: &[u8] = store.note.as_ref().map_or(&[][..], |(n, _)| n.as_bytes());
    let detail: &[u8] = store.detail.as_ref().filter(|_| store.note.is_some()).map_or(&[][..], |d| d.as_bytes());
    let note_lines = pieces(note).take(2).count() + pieces(detail).count();
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
    ring_line(c, 0, y - 8, LEFT - 1);
    for line in pieces(note).take(2) {
        c.text(PAD, y, line, if ok { GOOD } else { BAD }, 1);
        y += ROW;
    }
    for line in pieces(detail) {
        c.text(PAD, y, line, DIM, 1);
        y += ROW;
    }
    if disk_lines > 0 {
        icon(c, PAD, y, &DISK, DIM);
        c.text(PAD + ICON_W, y, b"EN DISCO", DIM, 1);
        y += ROW;
        for line in pieces(disk.get()).take(2) {
            c.text(PAD, y, line, INK, 1);
            y += ROW;
        }
    }
}

/// What the column has at (x, y), if it is something to click.
pub fn click(store: &Store, ui: &Ui, x: i32, y: i32) -> Option<Click> {
    if x >= LEFT {
        return None;
    }
    let mut hit = None;
    rows(store, ui, |ry, h, row| {
        if y >= ry && y < ry + h {
            hit = match row {
                Row::Package(i) => Some(Click::Package(i)),
                Row::File(i) => store.loaded.files().get(i).map(|f| Click::File(f.node)),
                Row::Files if tree(store).is_some() && (BTN_FILE - 2..BTN_FILE + 10).contains(&x) => Some(Click::NewFile),
                Row::Files if tree(store).is_some() && (BTN_FOLDER - 2..BTN_FOLDER + 10).contains(&x) => Some(Click::NewFolder),
                Row::Item(i, depth) => {
                    let arrow = tree(store).is_some_and(|t| t.is_folder(i)) && x < indent(depth) && x >= indent(depth) - 13;
                    Some(Click::Item(i, if arrow { Zone::Arrow } else { Zone::Body }))
                }
                Row::Typing(_) => Some(Click::Typing),
                _ => None,
            };
        }
    });
    hit
}

/// The rows of the disk tree in the order they are drawn: for the arrows.
pub fn next_row(store: &Store, from: Option<usize>, down: bool) -> Option<usize> {
    let t = tree(store)?;
    let at = from.and_then(|f| t.rows().position(|(i, _)| i == f));
    let n = t.row_count();
    let r = match (at, down) {
        (None, _) => 0,
        (Some(a), true) => (a + 1).min(n.saturating_sub(1)),
        (Some(a), false) => a.saturating_sub(1),
    };
    t.rows().nth(r).map(|(i, _)| i)
}

/// Where a dragged DISK item would land.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Drop {
    /// Before or after this sibling: the owner's order.
    Before(usize),
    After(usize),
    /// Into this folder (`None` = the package folder).
    Into(Option<usize>),
    /// Onto this node of the canvas: declared by it, or hung under it.
    Node(NodeId),
}

pub fn drop_at(store: &Store, ui: &Ui, cam: &Camera, moving: usize, x: i32, y: i32) -> Option<Drop> {
    let t = tree(store)?;
    if x >= LEFT {
        return (y > TOP).then(|| view::hit(&store.loaded.graph, cam, x, y)).flatten().map(Drop::Node);
    }
    let mut found = None;
    rows(store, ui, |ry, h, row| {
        if y < ry || y >= ry + h {
            return;
        }
        found = match row {
            Row::Files => Some(Drop::Into(None)),
            Row::Item(i, _) if i != moving => {
                let sibling = t.parent(i) == t.parent(moving);
                let part = (y - ry) * 4 / h.max(1);
                if t.is_folder(i) {
                    match (sibling, part) {
                        (true, 0) => Some(Drop::Before(i)),
                        (true, 3) => Some(Drop::After(i)),
                        _ => Some(Drop::Into(Some(i))),
                    }
                } else if sibling {
                    Some(if part < 2 { Drop::Before(i) } else { Drop::After(i) })
                } else {
                    Some(Drop::Into(t.parent(i)))
                }
            }
            _ => None,
        };
    });
    // Into the folder it is already in: nothing to do.
    found.filter(|d| *d != Drop::Into(t.parent(moving)))
}

/// A disk item being dragged: where it would land, and its name following
/// the pointer. Drawn last, on top.
pub fn draw_drag_item(c: &mut Canvas, store: &Store, ui: &Ui, cam: &Camera, moving: usize, x: i32, y: i32) {
    let Some(t) = tree(store) else { return };
    let target = drop_at(store, ui, cam, moving, x, y);
    let mut why: Option<&[u8]> = None;
    match target {
        Some(Drop::Before(i) | Drop::After(i)) => {
            let after = matches!(target, Some(Drop::After(_)));
            rows(store, ui, |ry, h, row| {
                if let Row::Item(j, depth) = row {
                    if j == i {
                        let ly = if after { ry + h - 2 } else { ry - 2 };
                        c.rect(indent(depth) - 2, ly, LEFT - indent(depth) - PAD + 2, 2, ACCENT);
                        c.disc(indent(depth) - 2, ly + 1, 3, ACCENT);
                    }
                }
            });
        }
        Some(Drop::Into(folder)) => {
            rows(store, ui, |ry, h, row| match (row, folder) {
                (Row::Item(j, _), Some(f)) if j == f => c.frame(1, ry - 2, LEFT - 3, h + 2, 2, GOOD),
                (Row::Files, None) => c.frame(1, ry, LEFT - 3, h, 2, GOOD),
                _ => {}
            });
        }
        Some(Drop::Node(n)) => {
            let ok = match (node_of(store, moving), t.name(moving).ends_with(b".titan")) {
                (Some(child), _) => match store.plan(child, n) {
                    Ok(_) => true,
                    Err(HangError::Same) => false,
                    Err(_) => {
                        why = Some(b"ahi no se cuelga: suelta y la nota dice por que");
                        false
                    }
                },
                (None, true) => true,
                (None, false) => {
                    why = Some(b"solo un .titan se declara en un modulo");
                    false
                }
            };
            if let Some(node) = store.loaded.graph.node(n) {
                let (nx, ny, nw, nh) = view::node_rect(cam, node);
                c.frame(nx - 3, ny - 3, nw + 6, nh + 6, 2, if ok { GOOD } else { BAD });
            }
        }
        None => {}
    }
    chip(c, t.name(moving), x, y, why);
}

/// The ghost: a name on a chip next to the pointer, and why not under it.
fn chip(c: &mut Canvas, label: &[u8], x: i32, y: i32, why: Option<&[u8]>) {
    let w = label.len() as i32 * 8 + 12;
    c.rect(x + 12, y + 10, w, ROW + 2, BAR);
    c.frame(x + 12, y + 10, w, ROW + 2, 1, ACCENT);
    c.text(x + 18, y + 12, label, ACCENT, 1);
    if let Some(line) = why {
        let tw = line.len() as i32 * 8 + 12;
        c.rect(x + 12, y + 12 + ROW, tw, ROW + 2, BG);
        c.text(x + 18, y + 14 + ROW, line, BAD, 1);
    }
}

// -- The declared tree, without a disk (the sample in memory) ----------------

/// Where a dragged declared file would land: a file row of the column, or a
/// node of the canvas -- the same module either way.
pub fn drop_target(store: &Store, ui: &Ui, cam: &Camera, x: i32, y: i32) -> Option<NodeId> {
    if x < LEFT {
        match click(store, ui, x, y) {
            Some(Click::File(id)) => Some(id),
            _ => None,
        }
    } else if y > TOP {
        view::hit(&store.loaded.graph, cam, x, y)
    } else {
        None
    }
}

/// A declared file being dragged: its target lit green (it can hang there) or
/// red (with why not), and the name following the pointer.
pub fn draw_drag(c: &mut Canvas, store: &Store, ui: &Ui, cam: &Camera, node: NodeId, x: i32, y: i32) {
    let target = drop_target(store, ui, cam, x, y);
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
        if let Some((rx, ry, rw, rh)) = row_or_node(store, ui, cam, t, x) {
            c.frame(rx, ry, rw, rh, 2, color);
        }
    }
    let label = store.loaded.file_of(node).map(|f| f.path.as_bytes()).unwrap_or(b"");
    let label = label.rsplit(|&b| b == b'/').next().unwrap_or(label);
    chip(c, label, x, y, why.as_ref().map(|l| l.as_bytes()));
}

fn name(store: &Store, id: NodeId) -> &[u8] {
    store.loaded.graph.node(id).map(|n| n.name.as_bytes()).unwrap_or(b"?")
}

/// The box to light for a target: its row when the pointer is in the column,
/// its node when it is on the canvas.
fn row_or_node(store: &Store, ui: &Ui, cam: &Camera, t: NodeId, x: i32) -> Option<(i32, i32, i32, i32)> {
    if x >= LEFT {
        let n = store.loaded.graph.node(t)?;
        let (nx, ny, nw, nh) = view::node_rect(cam, n);
        return Some((nx - 3, ny - 3, nw + 6, nh + 6));
    }
    let mut found = None;
    rows(store, ui, |y, h, row| {
        if let Row::File(i) = row {
            if store.loaded.files().get(i).map(|f| f.node) == Some(t) {
                found = Some((1, y - 2, LEFT - 3, h + 2));
            }
        }
    });
    found
}
