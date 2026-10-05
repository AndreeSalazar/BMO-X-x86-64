//! **THE VIEW** -- the package as nodes and cables, and the checker's events
//! played on top of it (`PLAN_TALLER` section 8).
//!
//! ```text
//!    a node      a header with its language (or TITAN.TOML, GPU, SISTEMA),
//!                its name, and the line that says what it does
//!    a cable     a `use`: from the bottom of the user to the top of the used
//!    a chip      a value on a node while a loan is open (`hero mut`), or
//!                grey on the origin when it was taken (`bullet entregado`)
//!    the panel   what the current event means; on a NO, the 4-part message
//! ```
//!
//! Nothing here judges: the graph and the script come already checked.
//!
//! ** THE LOOK is the logo's (`docs/arte/titan.jpg`, `PLAN_TALLER` 8.8): a
//! blue night, electric blue going to violet, light that GLOWS instead of
//! lines that border. F1 is TITAN++'s workshop and wears TITAN++'s colours;
//! the desktop keeps the cat's (`Los colores del gato`). The colours live in
//! `aspecto/titan.maqueta` and the soft pieces are MAQUETA 2's (`aspecto.rs`).

use crate::canvas::Canvas;
use crate::iconos::Icon;
use crate::player::{duration, Player, TRAVEL_MS};
use bmo_dibujo::{mezclar, Color, Lienzo, Vertice};
use bmo_titan_contrato::{EventKind, Graph, Lang, Mode, Node, NodeId, NodeKind, Permission, Script, MAX_NODES};

// The look -- colours, measures and the soft pieces -- is `aspecto.rs`'s:
// this file draws with it and decides nothing about it.
pub use crate::aspecto::{LEFT, LEVELS, NODE_H, NODE_W, PANEL, TOP};
pub(crate) use crate::aspecto::{picked_row, ring_line, ACCENT, BAD, BAR, BG, BLUE, DIM, EDGE, GOOD, INK, TITLE, VIOLET};
use crate::aspecto::{self as look, AMBER as MUT, BODY, CABLE_CORE, CABLE_HALO as CABLE, CYAN, DOT, GREY};

/// World -> screen: `(world - cam) * zoom / 1000`, right of the EXPLORER and
/// below the title bar.
#[derive(Clone, Copy)]
pub struct Camera {
    pub x: i32,
    pub y: i32,
    pub zoom: i32,
}

/// The middle of the canvas (the part that is not EXPLORER, title or panel).
pub fn canvas_center(w: i32, h: i32) -> Vertice {
    (LEFT + (w - LEFT) / 2, TOP + (h - TOP - PANEL) / 2)
}

impl Camera {
    pub fn to_screen(&self, wx: i32, wy: i32) -> Vertice {
        ((wx - self.x) * self.zoom / 1000 + LEFT, (wy - self.y) * self.zoom / 1000 + TOP)
    }

    pub fn to_world(&self, sx: i32, sy: i32) -> Vertice {
        ((sx - LEFT) * 1000 / self.zoom + self.x, (sy - TOP) * 1000 / self.zoom + self.y)
    }

    /// The biggest level that shows the whole graph, centered.
    pub fn fit(g: &Graph, w: i32, h: i32) -> Camera {
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for n in g.nodes() {
            x0 = x0.min(n.x);
            y0 = y0.min(n.y);
            x1 = x1.max(n.x + NODE_W);
            y1 = y1.max(n.y + NODE_H);
        }
        if x0 > x1 {
            return Camera { x: 0, y: 0, zoom: 1000 };
        }
        let (area_w, area_h) = (w - LEFT - 40, h - TOP - PANEL - 40);
        let mut zoom = LEVELS[0];
        for &z in LEVELS.iter() {
            if (x1 - x0) * z / 1000 <= area_w && (y1 - y0) * z / 1000 <= area_h {
                zoom = z;
            }
        }
        Camera::centered(zoom, (x0 + x1) / 2, (y0 + y1) / 2, w, h)
    }

    /// A camera at `zoom` whose canvas center shows the world point (cx, cy).
    pub fn centered(zoom: i32, cx: i32, cy: i32, w: i32, h: i32) -> Camera {
        let (mx, my) = canvas_center(w, h);
        Camera { x: cx - (mx - LEFT) * 1000 / zoom, y: cy - (my - TOP) * 1000 / zoom, zoom }
    }
}

/// The node's box on screen: x, y, width, height.
pub fn node_rect(cam: &Camera, n: &Node) -> (i32, i32, i32, i32) {
    let (x, y) = cam.to_screen(n.x, n.y);
    (x, y, NODE_W * cam.zoom / 1000, NODE_H * cam.zoom / 1000)
}

/// The node under a screen point: the LAST drawn wins, as the eye sees it.
pub fn hit(g: &Graph, cam: &Camera, sx: i32, sy: i32) -> Option<NodeId> {
    g.nodes().iter().enumerate().rev().find_map(|(i, n)| {
        let (x, y, w, h) = node_rect(cam, n);
        (sx >= x && sx < x + w && sy >= y && sy < y + h).then_some(NodeId(i as u8))
    })
}

/// A short line of text without an allocator.
pub(crate) struct Buf {
    b: [u8; 160],
    n: usize,
}

impl Buf {
    pub(crate) fn new() -> Buf {
        Buf { b: [0; 160], n: 0 }
    }

    pub(crate) fn b(&mut self, t: &[u8]) -> &mut Buf {
        for &c in t {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
        self
    }

    pub(crate) fn s(&mut self, t: &str) -> &mut Buf {
        self.b(t.as_bytes())
    }

    pub(crate) fn num(&mut self, v: u32) -> &mut Buf {
        let mut d = [0u8; 10];
        let (mut k, mut v) = (0, v);
        loop {
            d[k] = b'0' + (v % 10) as u8;
            k += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        while k > 0 {
            k -= 1;
            self.b(&[d[k]]);
        }
        self
    }

    pub(crate) fn get(&self) -> &[u8] {
        &self.b[..self.n]
    }
}

fn name_of(g: &Graph, id: NodeId) -> &[u8] {
    g.node(id).map(|n| n.name.as_bytes()).unwrap_or(b"?")
}

/// The four points of the cable `from -> to`.
fn cable(g: &Graph, cam: &Camera, from: NodeId, to: NodeId) -> Option<[Vertice; 4]> {
    let (ax, ay, aw, ah) = node_rect(cam, g.node(from)?);
    let (bx, by, bw, _) = node_rect(cam, g.node(to)?);
    let (p0, p3) = ((ax + aw / 2, ay + ah), (bx + bw / 2, by));
    let dy = ((p3.1 - p0.1).abs() / 2).max(30 * cam.zoom / 1000);
    Some([p0, (p0.0, p0.1 + dy), (p3.0, p3.1 - dy), p3])
}

/// The point at `u` thousandths along a cable.
fn along(k: &[Vertice; 4], u: u32) -> Vertice {
    let (u, m) = (u.min(1000) as i64, 1000 - u.min(1000) as i64);
    let (w0, w1, w2, w3) = (m * m * m, 3 * m * m * u, 3 * m * u * u, u * u * u);
    let x = (w0 * k[0].0 as i64 + w1 * k[1].0 as i64 + w2 * k[2].0 as i64 + w3 * k[3].0 as i64) / 1_000_000_000;
    let y = (w0 * k[0].1 as i64 + w1 * k[1].1 as i64 + w2 * k[2].1 as i64 + w3 * k[3].1 as i64) / 1_000_000_000;
    (x as i32, y as i32)
}

/// A node's header: its gradient (left, right) and its label.
fn header_of(n: &Node) -> (Color, Color, &'static str) {
    match n.kind {
        NodeKind::Root => (0x005A_2BD0, 0x00B0_4DE8, "TITAN.TOML"),
        NodeKind::Gpu => (0x0015_7A4E, 0x002B_B38F, "GPU"),
        NodeKind::Director => (0x0023_407E, 0x003A_66B8, "SISTEMA"),
        NodeKind::Dependency => (0x003A_3F66, 0x005A_5F8A, "DEPENDENCIA"),
        NodeKind::Module => match n.lang {
            Lang::Titan => (0x002A_4BE0, 0x007A_3DF0, "TITAN"),
            Lang::Inti => (0x008A_5A12, 0x00C9_8A24, "INTI"),
            Lang::C => (0x0023_4A8C, 0x0035_70C0, "C"),
            Lang::Rust => (0x008B_2A2A, 0x00C0_442E, "RUST"),
            Lang::None => (EDGE, EDGE, ""),
        },
    }
}

fn mode_label(m: Mode) -> &'static str {
    match m {
        Mode::Read => "lee",
        Mode::Mut => "mut",
        Mode::Take => "take",
    }
}

/// Splits a line in two at a space, each part at most `max` bytes.
fn wrap(s: &[u8], max: usize) -> (&[u8], &[u8]) {
    if s.len() <= max {
        return (s, &[]);
    }
    let cut = s[..=max.min(s.len() - 1)].iter().rposition(|&c| c == b' ').unwrap_or(max);
    let rest = &s[(cut + 1).min(s.len())..];
    (&s[..cut], &rest[..rest.len().min(max)])
}

fn blink(now_ms: u32) -> bool {
    (now_ms / 250) % 2 == 0
}

/// Everything one frame of the canvas needs.
pub struct Scene<'a> {
    pub graph: &'a Graph,
    /// The checker's events, if this package has them (today: only the
    /// `asteroids` modules; the real checker is T2-T4).
    pub script: Option<&'a Script>,
    pub player: &'a Player,
    pub cam: &'a Camera,
    pub now_ms: u32,
    /// The node picked in the EXPLORER.
    pub selected: Option<NodeId>,
    /// Where the package came from, for the title bar.
    pub origin: &'a [u8],
    /// The sky, built once (`art::backdrop`); `None`: a plain night colour.
    pub sky: Option<&'a [u32]>,
    /// The clock of the white pulse on the cables; `None`: the cables are
    /// still (F1 at rest, see `main.rs`). The faults breathe on the same clock.
    pub flow_ms: Option<u32>,
    /// The nodes in fault (`faults.rs`), in the order of their path.
    pub faults: &'a crate::faults::Marks,
    /// The files the package was read from: their depth and their TRAITS,
    /// what the SPACE tabs draw each node by (`astros.rs`).
    pub files: &'a [bmo_titan_lector::FileEntry],
    /// How far the 3D sky has turned, in 1024ths of a turn (`space.rs`).
    pub turn: u32,
}

/// One whole frame of the canvas. The EXPLORER is drawn after, on its own.
pub fn draw(c: &mut Canvas, sc: &Scene) {
    let none = Script::new();
    let (g, p, cam) = (sc.graph, sc.player, sc.cam);
    let s = sc.script.unwrap_or(&none);
    match sc.sky {
        Some(px) => c.blit(px),
        None => c.clear(BG),
    }
    grid(c, cam);
    let current = s.events().get(p.index).map(|e| e.kind);
    cables(c, g, sc.files, p, cam, current, sc.flow_ms);
    let (clock, lively) = (sc.flow_ms.unwrap_or(sc.now_ms), sc.flow_ms.is_some());
    crate::faults::draw_under(c, g, cam, sc.faults, clock, lively);
    for (i, n) in g.nodes().iter().enumerate() {
        let id = NodeId(i as u8);
        if sc.selected == Some(id) {
            // Picked: a brighter, wider halo, drawn under the node.
            let (x, y, w, h) = node_rect(cam, n);
            look::shine(c, x, y, w, h, look::r_at(look::R_NODE, cam.zoom), 14, ACCENT, 170);
        }
        let seed = crate::astros::seal_seed(sc.files, id, n.name.as_bytes());
        node(c, g, cam, id, n, current, sc.now_ms, crate::astros::traits_of(sc.files, id), seed);
    }
    chips(c, g, p, cam);
    crate::faults::draw_over(c, g, cam, sc.faults, clock, lively);
    overlay(c, g, s, p, cam, sc.now_ms);
    title_bar(c, s, p, sc.script.is_some(), sc.origin);
    if sc.script.is_some() {
        panel(c, g, s, p);
    } else {
        quiet_panel(c, g, sc.files);
    }
}

fn grid(c: &mut Canvas, cam: &Camera) {
    let step = (40 * cam.zoom / 1000).max(8);
    let (ox, oy) = cam.to_screen(0, 0);
    let (sx, sy) = (LEFT + (ox - LEFT).rem_euclid(step), (oy - TOP).rem_euclid(step) + TOP);
    let mut y = sy;
    while y < c.h - PANEL {
        let mut x = sx;
        while x < c.w {
            c.put(x, y, DOT);
            x += step;
        }
        y += step;
    }
}

/// How long a pulse takes to go down a cable, and how far behind it its tail
/// reaches (thousandths of the cable).
const FLOW_MS: u32 = 1800;
const TAIL: u32 = 220;

fn cables(c: &mut Canvas, g: &Graph, files: &[bmo_titan_lector::FileEntry], p: &Player, cam: &Camera, current: Option<EventKind>, flow_ms: Option<u32>) {
    for (i, e) in g.edges().iter().enumerate() {
        let Some(k) = cable(g, cam, e.from, e.to) else { continue };
        let lent = p.loans.iter().flatten().find(|l| l.from == e.from && l.to == e.to);
        let to_gpu = g.node(e.to).map(|n| n.kind == NodeKind::Gpu).unwrap_or(false);
        let travelling = match current {
            Some(EventKind::Borrow { from, to, .. } | EventKind::Return { from, to } | EventKind::Take { from, to }) => {
                from == e.from && to == e.to
            }
            _ => false,
        };
        // At rest a cable wears the colour of its CLASS -- mod, use, 3060,
        // sistema -- fixed, the same in every tab (`astros::Cable`): the
        // colour SAYS what it is, nobody picks it. A loan in play wears its
        // own, over it.
        let class = crate::astros::cable_of(g, files, e);
        let (color, thick) = match lent {
            Some(_) if to_gpu => (GOOD, 3),
            Some(l) if l.mode == Mode::Mut => (MUT, 3),
            Some(_) => (ACCENT, 3),
            None if travelling => (INK, 2),
            None => (mezclar(class.color(), CABLE_CORE, 2, 3), class.thick() - 1),
        };
        // Every cable shines a little; a busy one shines in its own colour.
        let halo = if lent.is_some() || travelling { color } else { mezclar(class.color(), CABLE, 1, 2) };
        c.curve_glow(k, halo, color, thick, 3);
        // The arrow head: the used node is below.
        c.disc(k[3].0, k[3].1, 3, color);
        // The pulse: white light going DOWN the cable, from the one that
        // uses to the one used -- the direction of every `mod` and `use`.
        // Not on a cable that is busy with a loan: that one already speaks.
        if let (Some(t), false) = (flow_ms, lent.is_some() || travelling) {
            // Each cable on its own phase, so they do not beat together.
            let u = (t.wrapping_add(i as u32 * 397) % FLOW_MS) * 1000 / FLOW_MS;
            pulse(c, &k, u);
        }
    }
}

/// A comet at `u` thousandths along a cable: a bright head five pixels wide
/// and a tail that thins and fades behind it, added as light (white over the
/// blue cable reads as white).
fn pulse(c: &mut Canvas, k: &[Vertice; 4], u: u32) {
    const STEPS: u32 = 12;
    for s in (0..STEPS).rev() {
        let back = TAIL * s / STEPS;
        if back > u {
            continue;
        }
        let (x, y) = along(k, u - back);
        let fade = 255 * (STEPS - s) / STEPS;
        // Two pixels of radius at the head, one in the first half of the tail.
        let r: i32 = if s < 2 { 2 } else if s < STEPS / 2 { 1 } else { 0 };
        for dy in -r..=r {
            for dx in -r..=r {
                let d = dx.abs() + dy.abs();
                if d > r {
                    continue;
                }
                let v = fade * (r + 1 - d) as u32 / (r + 1) as u32;
                c.light(x + dx, y + dy, v << 16 | v << 8 | v);
            }
        }
    }
}

/// The PINS of a node, the Unreal Engine 5 way: IN on top (where the cables
/// that use it arrive), OUT below (where it pulls its own `use` from).
pub fn in_pin(cam: &Camera, n: &Node) -> Vertice {
    let (x, y, w, _) = node_rect(cam, n);
    (x + w / 2, y)
}

pub fn out_pin(cam: &Camera, n: &Node) -> Vertice {
    let (x, y, w, h) = node_rect(cam, n);
    (x + w / 2, y + h)
}

/// The OUT pin under (x, y): only a module has one (it has a header to write
/// the `use` in).
pub fn pin_at(g: &Graph, cam: &Camera, x: i32, y: i32) -> Option<NodeId> {
    let r = (9 * cam.zoom / 1000).max(7);
    g.nodes().iter().enumerate().find_map(|(i, n)| {
        let (px, py) = out_pin(cam, n);
        (n.kind == NodeKind::Module && (px - x).abs() <= r && (py - y).abs() <= r).then_some(NodeId(i as u8))
    })
}

/// A pin: a ring, filled when something is plugged in, and its little arrow.
fn pin(c: &mut Canvas, (x, y): Vertice, r: i32, color: Color, plugged: bool) {
    c.disc(x, y, r + 1, BODY);
    c.disc(x, y, r, color);
    if !plugged {
        c.disc(x, y, r - 2, BODY);
    }
    // The arrow: down, the way every `mod` and `use` goes.
    c.rect(x - 1, y + r + 1, 3, 1, color);
    c.rect(x, y + r + 2, 1, 1, color);
}

/// The cable being pulled from `from`'s OUT pin to the pointer: in the colour
/// of a `use` if it can be let go there, red and with the reason if not.
pub fn draw_wire(c: &mut Canvas, g: &Graph, cam: &Camera, from: NodeId, x: i32, y: i32, verdict: Option<Result<(), &[u8]>>) {
    let Some(n) = g.node(from) else { return };
    let a = out_pin(cam, n);
    let ok = !matches!(verdict, Some(Err(_)));
    let color = if ok { CYAN } else { BAD };
    let dy = ((y - a.1).abs() / 2).max(30);
    c.curve_glow([a, (a.0, a.1 + dy), (x, y - dy), (x, y)], mezclar(color, CABLE, 1, 2), color, 2, 3);
    c.disc(x, y, 4, color);
    if let Some(Err(why)) = verdict {
        look::pill(c, x + 12, y + 10, why, BAD, BAR);
    }
}

#[allow(clippy::too_many_arguments)]
fn node(c: &mut Canvas, g: &Graph, cam: &Camera, id: NodeId, n: &Node, current: Option<EventKind>, now_ms: u32, tr: bmo_titan_lector::Traits, seed: u32) {
    let (x, y, w, h) = node_rect(cam, n);
    if x + w < 0 || y + h < TOP || x >= c.w || y >= c.h - PANEL {
        return;
    }
    let alarm = match current {
        Some(EventKind::Conflict { first, second, .. }) => first.node == id || second.node == id,
        Some(EventKind::Denied { node, .. }) => node == id,
        _ => false,
    } && blink(now_ms);
    let (left, right, label) = header_of(n);
    let round = look::r_at(look::R_NODE, cam.zoom);
    // The node glows in its own colour (red while it is the one in trouble):
    // a soft light around a rounded card, MAQUETA 2's pieces (`aspecto.rs`).
    look::shine(c, x, y, w, h, round, 7, if alarm { BAD } else { look::half(left, right) }, if alarm { 150 } else { 70 });
    look::card(c, x, y, w, h, round, BODY);
    let head_h = (22 * cam.zoom / 1000).max(8);
    // The header: the gradient with the card's top corners, square below.
    look::band(c, x, y, w, head_h + round, round, left, right, false);
    c.rect(x, y + head_h, w, round, BODY);
    // A line of light under the header, and a thin border of the header's hue.
    c.rect(x + 1, y + head_h, w - 2, 1, mezclar(INK, right, 1, 3));
    look::edge(c, x, y, w, h, round, 1, if alarm { BAD } else { mezclar(left, EDGE, 1, 2) });
    // Its pins: IN if anything uses it, OUT if it is a module (it can pull).
    let r = (5 * cam.zoom / 1000).max(3);
    if n.kind != NodeKind::Root {
        let used = g.edges().iter().any(|e| e.to == id);
        pin(c, in_pin(cam, n), r, if used { INK } else { DIM }, used);
    }
    if n.kind == NodeKind::Module {
        let uses = g.edges().iter().any(|e| e.from == id);
        pin(c, out_pin(cam, n), r, CYAN, uses);
    }
    if cam.zoom < 750 {
        // Too small for text: only the name, if it fits.
        c.text_fit(x + 4, y + head_h + 2, n.name.as_bytes(), INK, w - 8);
        return;
    }
    let scale = if cam.zoom >= 1500 { 2 } else { 1 };
    let line = 16 * scale;
    // ** Its SEAL first, at the left of the header: its own face, from its
    // name and the sum of its bytes (`astros::seal`) -- two nodes are never
    // drawn alike, and saving the file changes it.
    let seal_r = ((head_h - 4) / 2).clamp(4, 10);
    crate::astros::seal(c, x + 6 + seal_r, y + head_h / 2, seal_r, seed, INK);
    let lx = x + 10 + 2 * seal_r;
    c.text_fit(lx, y + (head_h - 16).max(0) / 2, label.as_bytes(), INK, w - (lx - x) - 8);
    // ** Its SHAPE, in the header, right to left: where the program starts
    // (main), the 3060's chip, and what the module DOES (`iconos.rs`) --
    // print, decide, repeat, crystal, change, value, call, return. They
    // change the moment the file is saved.
    let main = n.kind == NodeKind::Module && n.name.as_bytes() == b"main";
    let shapes = [
        (Icon::Play, main),
        (Icon::Chip, n.kind == NodeKind::Gpu),
        (Icon::Printer, tr.writes > 0),
        (Icon::Decide, tr.ifs > 0),
        (Icon::Repeat, tr.loops > 0),
        (Icon::Crystal, tr.types > 0),
        (Icon::Change, tr.muts > 0),
        (Icon::Value, tr.lets > 0),
        (Icon::Call, tr.calls > 0),
        (Icon::Return, tr.returns > 0),
    ];
    let s = (head_h - 6).clamp(8, 16);
    let floor = lx + 8 + label.len() as i32 * 8;
    let mut ix = x + w - s - 8;
    for (icon, on) in shapes {
        if on && ix >= floor {
            crate::iconos::draw(c, icon, ix, y + (head_h - s) / 2, s);
            ix -= s + 4;
        }
    }
    let mut ty = y + head_h + 6;
    if scale == 2 {
        c.text(x + 8, ty, n.name.as_bytes(), INK, 2);
    } else {
        c.text_fit(x + 8, ty, n.name.as_bytes(), INK, w - 16);
    }
    ty += line + 4;
    if n.kind == NodeKind::Root {
        permissions(c, g, x + 8, ty, w - 16, current, now_ms);
        return;
    }
    let max = ((w - 16) / 8).max(1) as usize;
    let (a, b) = wrap(n.purpose.as_bytes(), max);
    c.text_fit(x + 8, ty, a, DIM, w - 16);
    if n.kind != NodeKind::Module {
        if ty + 16 < y + h - 4 {
            c.text_fit(x + 8, ty + 16, b, DIM, w - 16);
        }
        return;
    }
    // ** THE PRINTER and what it prints: the exact text when every argument
    // is a written text, its arguments as written when something is
    // calculated (`titan_lector::traits::Said`).
    ty += line;
    if tr.writes > 0 && ty + 16 <= y + h - 2 {
        crate::iconos::draw(c, Icon::Printer, x + 8, ty, 16);
        let said = tr.says.as_bytes();
        let (tx, room) = (x + 30, w - 38);
        if tr.says.exact {
            let k = c.text_fit(tx, ty, b"\"", GOOD, room);
            let k = k + c.text_fit(tx + k, ty, said, GOOD, room - k - 16);
            c.text_fit(tx + k, ty, if tr.says.cut { b"...\"" } else { b"\"" }, GOOD, room - k);
        } else {
            let k = c.text_fit(tx, ty, b"print(", DIM, room);
            let k = k + c.text_fit(tx + k, ty, said, INK, room - k - 8);
            c.text_fit(tx + k, ty, b")", DIM, room - k);
        }
    } else if ty - line + 32 < y + h - 2 {
        c.text_fit(x + 8, ty - line + 16, b, DIM, w - 16);
    }
}

/// The root node shows what the package asked for (U2).
fn permissions(c: &mut Canvas, g: &Graph, x: i32, y: i32, w: i32, current: Option<EventKind>, now_ms: u32) {
    let mut asked = Buf::new();
    for perm in Permission::ALL {
        if g.permissions.allows(perm) {
            asked.s(perm.key()).s(" ");
        }
    }
    c.text_fit(x, y, asked.get(), GOOD, w);
    // Only the permission the event names flashes: flashing the whole line
    // said "disk" was denied too (seen in the browser preview, 29-09).
    let denied = match current {
        Some(EventKind::Denied { permission, .. }) if blink(now_ms) => Some(permission),
        _ => None,
    };
    let mut px = x;
    for perm in Permission::ALL {
        if g.permissions.allows(perm) {
            continue;
        }
        let mut t = Buf::new();
        t.s(perm.key()).s(": no ");
        let left = x + w - px;
        if left <= 0 {
            break;
        }
        px += c.text_fit(px, y + 16, t.get(), if denied == Some(perm) { BAD } else { DIM }, left);
    }
}

fn chip(c: &mut Canvas, x: i32, y: i32, text: &[u8], color: Color) {
    look::pill(c, x, y, text, color, BAR);
}

/// The values on each node while a loan is open, and the taken ones.
fn chips(c: &mut Canvas, g: &Graph, p: &Player, cam: &Camera) {
    let mut stack = [0i32; MAX_NODES];
    let mut place = |c: &mut Canvas, id: NodeId, text: &[u8], color: Color| {
        let Some(n) = g.node(id) else { return };
        let (x, y, _, h) = node_rect(cam, n);
        let k = &mut stack[id.0 as usize];
        chip(c, x, y + h + 4 + *k * 22, text, color);
        *k += 1;
    };
    for l in p.loans.iter().flatten() {
        let color = if g.node(l.to).map(|n| n.kind == NodeKind::Gpu).unwrap_or(false) {
            GOOD
        } else if l.mode == Mode::Mut {
            MUT
        } else {
            ACCENT
        };
        let mut t = Buf::new();
        t.b(l.value.as_bytes()).s(" ").s(mode_label(l.mode));
        place(c, l.to, t.get(), color);
        let mut t = Buf::new();
        t.b(l.value.as_bytes()).s(" prestado");
        place(c, l.from, t.get(), DIM);
    }
    for tk in p.taken.iter().flatten() {
        let mut t = Buf::new();
        t.b(tk.value.as_bytes()).s(" entregado");
        place(c, tk.from, t.get(), GREY);
        let mut t = Buf::new();
        t.b(tk.value.as_bytes()).s(" suyo");
        place(c, tk.to, t.get(), ACCENT);
    }
}

/// The current event, moving.
fn overlay(c: &mut Canvas, g: &Graph, s: &Script, p: &Player, cam: &Camera, now_ms: u32) {
    let Some(e) = s.events().get(p.index) else { return };
    let r = (6 * cam.zoom / 1000).max(3);
    match e.kind {
        EventKind::Borrow { mode, from, to } => {
            if p.t < TRAVEL_MS {
                if let Some(k) = cable(g, cam, from, to) {
                    let (x, y) = along(&k, p.t * 1000 / TRAVEL_MS);
                    let color = if mode == Mode::Mut { MUT } else { ACCENT };
                    c.disc(x, y, r, color);
                    c.text(x + r + 4, y - 8, e.value.as_bytes(), color, 1);
                }
            }
        }
        EventKind::Return { from, to } => {
            if let Some(k) = cable(g, cam, from, to) {
                let (x, y) = along(&k, 1000 - p.progress(s));
                c.disc(x, y, r, INK);
                c.text(x + r + 4, y - 8, e.value.as_bytes(), INK, 1);
            }
        }
        EventKind::Take { from, to } => {
            if let Some(k) = cable(g, cam, from, to) {
                let (x, y) = along(&k, p.progress(s));
                chip(c, x - 20, y - 9, e.value.as_bytes(), ACCENT);
            }
        }
        EventKind::Conflict { first, second, .. } => {
            let a = g.node(first.node).map(|n| node_rect(cam, n));
            let b = g.node(second.node).map(|n| node_rect(cam, n));
            if let (Some(a), Some(b)) = (a, b) {
                let (pa, pb) = ((a.0 + a.2 / 2, a.1 + a.3 / 2), (b.0 + b.2 / 2, b.1 + b.3 / 2));
                let color = if blink(now_ms) { BAD } else { MUT };
                c.line(pa, pb, color);
                c.line((pa.0, pa.1 + 1), (pb.0, pb.1 + 1), color);
            }
        }
        EventKind::Denied { node, .. } => {
            let a = g.node(node).map(|n| node_rect(cam, n));
            let b = g.root().and_then(|r| g.node(r)).map(|n| node_rect(cam, n));
            if let (Some(a), Some(b)) = (a, b) {
                let color = if blink(now_ms) { BAD } else { MUT };
                c.line((a.0 + a.2 / 2, a.1), (b.0 + b.2 / 2, b.1 + b.3), color);
            }
        }
    }
    // The glow of the event on screen: a soft ring on the node it happens in.
    if let EventKind::Conflict { first, .. } = e.kind {
        if let Some(n) = g.node(first.node) {
            let (x, y, w, h) = node_rect(cam, n);
            for k in 1..6 {
                for dx in 0..w + 2 * k {
                    c.blend(x - k + dx, y - k, BAD, (6 - k) as u32, 8);
                    c.blend(x - k + dx, y + h + k - 1, BAD, (6 - k) as u32, 8);
                }
            }
        }
    }
}

/// The title bar alone: the SPACE tab (`space.rs`) wears the same one.
pub(crate) fn title(c: &mut Canvas, sc: &Scene) {
    let none = Script::new();
    title_bar(c, sc.script.unwrap_or(&none), sc.player, sc.script.is_some(), sc.origin);
}

fn title_bar(c: &mut Canvas, s: &Script, p: &Player, has_script: bool, origin: &[u8]) {
    let w = c.w;
    c.gradient(0, 0, w, TOP, 0x0005_0716, 0x000C_0A24);
    ring_line(c, 0, TOP - 1, w);
    // The name as the logo writes it: TITAN in white light, `++` from blue
    // to violet.
    let mut x = 10;
    x += c.text(x, 6, b"F1  ", DIM, 1);
    x += c.text(x, 6, b"TITAN", INK, 1);
    x += c.text(x, 6, b"+", BLUE, 1);
    x += c.text(x, 6, b"+", VIOLET, 1);
    x += c.text(x, 6, b"  TALLER  ", TITLE, 1);
    let used = x + c.text_fit(x, 6, origin, ACCENT, w / 3);
    tagline(c, w / 2, 6);
    let mut t = Buf::new();
    let total = s.events().len() as u32;
    if !has_script {
        t.s("sin comprobador todavia");
    } else if p.finished(s) {
        t.s("fin: ").num(total).s(" eventos  [r] repite");
    } else {
        t.s("evento ").num(p.index as u32 + 1).s("/").num(total);
        t.s(if p.playing { "  reproduciendo" } else { "  en pausa" });
    }
    let tw = t.get().len() as i32 * 8;
    c.text_fit((w - tw - 10).max(used + 20), 6, t.get(), DIM, tw);
}

/// CODE . NODES . BEYOND, centered at `cx`: the logo's line, with its dots
/// drawn as light and not as a glyph the font does not have.
fn tagline(c: &mut Canvas, cx: i32, y: i32) {
    let words: [&[u8]; 3] = [b"CODE", b"NODES", b"BEYOND"];
    let gap = 28;
    let width: i32 = words.iter().map(|w| w.len() as i32 * 8).sum::<i32>() + 2 * gap;
    let mut x = cx - width / 2;
    for (i, word) in words.iter().enumerate() {
        x += c.text(x, y, word, DIM, 1);
        if i < 2 {
            let (dx, dy) = (x + gap / 2 - 1, y + 7);
            c.rect(dx, dy, 2, 2, if i == 0 { BLUE } else { VIOLET });
            c.glow(dx, dy, 2, 2, BLUE, 2, 40);
            x += gap;
        }
    }
}

/// The bottom panel's box: right of the EXPLORER.
fn panel_box(c: &mut Canvas) -> (i32, i32, i32) {
    let (x, top, w) = (LEFT, c.h - PANEL, c.w - LEFT);
    c.rect(x, top, w, PANEL, BAR);
    ring_line(c, x, top, w);
    (x + 12, top, w - 24)
}

fn panel(c: &mut Canvas, g: &Graph, s: &Script, p: &Player) {
    let (x, top, w) = panel_box(c);
    let mut y = top + 8;
    let Some(e) = s.events().get(p.index) else {
        c.text_fit(x, y, b"El comprobador termino: los prestamos volvieron, y los dos NO quedaron dichos.", INK, w);
        help(c, x, top);
        return;
    };
    let diag = match e.kind {
        EventKind::Conflict { diag, .. } | EventKind::Denied { diag, .. } => s.diagnostic(diag),
        _ => None,
    };
    if let Some(d) = diag {
        let rows: [(&str, &[u8], Color); 4] = [
            ("QUE      ", d.what.as_bytes(), BAD),
            ("DONDE    ", b"", INK),
            ("POR QUE  ", d.why.as_bytes(), INK),
            ("COMO     ", d.fix.as_bytes(), GOOD),
        ];
        for (label, text, color) in rows {
            let lx = x + c.text(x, y, label.as_bytes(), DIM, 1);
            if text.is_empty() {
                let mut t = Buf::new();
                t.b(name_of(g, d.place.node)).s(", linea ").num(d.place.line as u32);
                c.text(lx, y, t.get(), color, 1);
            } else {
                c.text_fit(lx, y, text, color, x + w - lx);
            }
            y += 20;
        }
    } else {
        let mut t = Buf::new();
        let v = e.value.as_bytes();
        match e.kind {
            EventKind::Borrow { mode, from, to } => {
                t.s(mode_label(mode)).s(": `").b(v).s("` se presta de ").b(name_of(g, from)).s(" a ").b(name_of(g, to));
                t.s(if mode == Mode::Mut { "; nadie mas lo toca mientras dure" } else { "; nadie lo cambia mientras dure" });
            }
            EventKind::Return { from, to } => {
                t.s("vuelve: `").b(v).s("` regresa de ").b(name_of(g, to)).s(" a ").b(name_of(g, from));
                t.s(" -- el prestamo acabo");
            }
            EventKind::Take { from, to } => {
                t.s("take: `").b(v).s("` pasa a ser de ").b(name_of(g, to)).s("; ").b(name_of(g, from));
                t.s(" ya no lo tiene");
            }
            EventKind::Conflict { .. } | EventKind::Denied { .. } => {}
        }
        c.text_fit(x, y, t.get(), INK, w);
        let mut d = Buf::new();
        d.s("dura ").num(duration(&e.kind)).s(" ms");
        c.text(x, y + 22, d.get(), DIM, 1);
    }
    help(c, x, top);
}

/// A graph the sample's events do not fit (a module or a cable they name is
/// not there -- after a hang, for one): say so, instead of animating
/// something invented.
/// Without the checker's script, the panel is the CONSOLA: what the nodes
/// print, read from their text -- the exact text when every argument is a
/// written one (`print("hola mundo")` writes `hola mundo`, nothing to
/// calculate), the arguments as written when something is calculated. It is
/// the first `print` of each node, and it says so; running the whole program
/// is `titan build`'s (and, with T6, the compiler inside F1).
fn quiet_panel(c: &mut Canvas, g: &Graph, files: &[bmo_titan_lector::FileEntry]) {
    let (x, top, w) = panel_box(c);
    let k = c.text(x, top + 8, b"CONSOLA", TITLE, 1);
    c.text_fit(x + k + 12, top + 8, b"el primer print de cada nodo, leido de su texto; cambia al guardar", DIM, w - k - 12);
    let mut y = top + 30;
    let mut any = false;
    for f in files.iter().filter(|f| f.traits.writes > 0) {
        if y > top + PANEL - 44 {
            break;
        }
        any = true;
        crate::iconos::draw(c, Icon::Printer, x, y - 1, 14);
        let name = g.node(f.node).map_or(&b"?"[..], |n| n.name.as_bytes());
        let nw = c.text(x + 22, y, name, DIM, 1);
        let said = f.traits.says.as_bytes();
        let tx = x + 22 + nw + 12;
        if f.traits.says.exact {
            let k = c.text(tx, y, b"> ", GOOD, 1);
            let k = k + c.text_fit(tx + k, y, said, GOOD, w - (tx - x) - k - 24);
            if f.traits.says.cut {
                c.text(tx + k, y, b"...", GOOD, 1);
            }
        } else {
            let k = c.text(tx, y, b"print(", DIM, 1);
            let k = k + c.text_fit(tx + k, y, said, INK, w - (tx - x) - k - 24);
            c.text(tx + k, y, b")  se calcula al compilar", DIM, 1);
        }
        y += 18;
    }
    if !any {
        c.text_fit(x, y, b"ningun nodo escribe todavia: un print(\"hola\") en su codigo (doble clic en el nodo)", DIM, w);
    }
    help(c, x, top);
}

fn help(c: &mut Canvas, x: i32, top: i32) {
    c.text(
        x,
        top + PANEL - 22,
        b"2 clics en un nodo: su codigo  clic der: menu  [TAB] maestros  [+-] zoom  [0] encuadra  [t] ESPACIO  [Esc] sale",
        DIM,
        1,
    );
}
