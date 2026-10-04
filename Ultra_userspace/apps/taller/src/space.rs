//! **THE TABS, AND THE SKY IN 3D** -- the same package, seen as space (the
//! owner, 04-10: "en VS Code es 2D, pero en 3D lo esconde ... muy creativo,
//! solo que ya no se ve como 2D").
//!
//! ```text
//!    GRAFO     the node editor (view.rs): what the code declares, flat
//!    ESPACIO   CIELO       the package in 3D: depth is how deep a module
//!                          sits in the declared tree; it turns slowly, and
//!                          dragging the sky turns it by hand
//!              ELEMENTOS   every element, animated, with what it represents
//!                          and why (guia.rs)
//!              GUIA        the whys, in order (guia.rs)
//! ```
//!
//! ** The TEXT is still the truth (PLAN_TALLER 8.3): the sky is a way of
//! seeing it. Each star is drawn from what the code IS (`astros.rs`): its
//! kind, its name and the TRAITS of its body -- save the file and, at the next
//! beat of ESTRATOS, the star changes.
//!
//! ** 3D with integers: `core` has no floats to spare and no `sin`. A point
//! (x, y, z) turns around the vertical axis by `turn` (1024ths of a turn),
//! tilts toward the eye, and is divided by its distance -- perspective, the
//! oldest trick there is. The far ones are drawn first (painter's order).
//!
//! It only MOVES while F1 is seen and touched (`flow_ms`, the house's rule:
//! if it does nothing, it spends nothing). At rest, the sky stands still.

use crate::astros::{self, cable, cable_of, centaur, halo, hash, pulsar, station, supernova, traits_of};
use crate::canvas::Canvas;
use crate::aspecto as look;
use crate::view::{Buf, Camera, Scene, ACCENT, BAD, BAR, BG, BLUE, DIM, EDGE, INK, LEFT, NODE_H, NODE_W, PANEL, TITLE, TOP, VIOLET};
use bmo_titan_contrato::Graph;
use bmo_titan_lector::FileEntry;
use bmo_dibujo::{mezclar, Lienzo};
use bmo_titan_contrato::{NodeId, NodeKind, MAX_NODES};

/// Where F1 is looking.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Graph,
    Sky,
    Elements,
    Guide,
}

impl Tab {
    pub fn is_space(self) -> bool {
        self != Tab::Graph
    }
    /// `t` walks them all.
    pub fn next(self) -> Tab {
        match self {
            Tab::Graph => Tab::Sky,
            Tab::Sky => Tab::Elements,
            Tab::Elements => Tab::Guide,
            Tab::Guide => Tab::Graph,
        }
    }
}

const TAB_Y: i32 = TOP + 8;
const TAB_H: i32 = 20;
/// The first strip, and -- inside ESPACIO -- the second one.
const MAIN: [(Tab, &[u8], i32); 2] = [(Tab::Graph, b"GRAFO", 64), (Tab::Sky, b"ESPACIO", 80)];
const SUB: [(Tab, &[u8], i32); 3] = [(Tab::Sky, b"CIELO 3D", 80), (Tab::Elements, b"ELEMENTOS", 88), (Tab::Guide, b"GUIA", 56)];
const SUB_X: i32 = LEFT + 12 + 64 + 6 + 80 + 26;

/// Every tab on screen: (tab, label, x, width).
fn strips(now: Tab, mut f: impl FnMut(Tab, &[u8], i32, i32, bool)) {
    let mut x = LEFT + 12;
    for (tab, label, w) in MAIN {
        f(tab, label, x, w, tab == now || (tab == Tab::Sky && now.is_space()));
        x += w + 6;
    }
    if now.is_space() {
        let mut x = SUB_X;
        for (tab, label, w) in SUB {
            f(tab, label, x, w, tab == now);
            x += w + 4;
        }
    }
}

/// The tab under (x, y), if the pointer is on a strip.
pub fn tab_at(now: Tab, x: i32, y: i32) -> Option<Tab> {
    if !(TAB_Y..TAB_Y + TAB_H).contains(&y) {
        return None;
    }
    let mut hit = None;
    strips(now, |tab, _, tx, w, _| {
        if (tx..tx + w).contains(&x) {
            // ESPACIO from GRAFO opens the sky; inside ESPACIO it stays.
            hit = Some(if tab == Tab::Sky && now.is_space() && x < SUB_X { now } else { tab });
        }
    });
    hit
}

/// The strips, drawn on top of whatever tab is showing.
pub fn tabs(c: &mut Canvas, now: Tab) {
    strips(now, |_, label, x, w, on| {
        // A soft tab (MAQUETA 2's pieces, `aspecto.rs`): the one that is on
        // glows a little and wears the ring's gradient.
        if on {
            look::shine(c, x, TAB_Y, w, TAB_H, look::R_TAB, 6, ACCENT, 90);
            look::band(c, x, TAB_Y, w, TAB_H, look::R_TAB, look::SEL, mezclar(VIOLET, BG, 1, 3), false);
        } else {
            look::card(c, x, TAB_Y, w, TAB_H, look::R_TAB, BAR);
        }
        look::edge(c, x, TAB_Y, w, TAB_H, look::R_TAB, 1, if on { ACCENT } else { EDGE });
        let lw = label.len() as i32 * 8;
        c.text(x + (w - lw) / 2, TAB_Y + 2, label, if on { INK } else { DIM }, 1);
    });
    if now.is_space() {
        c.rect(SUB_X - 14, TAB_Y + TAB_H / 2, 10, 1, EDGE);
    }
    c.text(c.w - 4 * 8 - 12, TAB_Y + 2, b"[t]", DIM, 1);
}

/// The whole canvas for the tab: the sky, or the pages of `guia.rs`.
pub fn draw(c: &mut Canvas, sc: &Scene, tab: Tab) {
    match sc.sky {
        Some(px) => c.blit(px),
        None => c.clear(BG),
    }
    let t = sc.flow_ms.unwrap_or(0) as i32;
    match tab {
        Tab::Elements => crate::guia::elements(c, t),
        Tab::Guide => crate::guia::guide(c),
        _ => sky(c, sc, t),
    }
    crate::view::title(c, sc);
}

// -- The sky in 3D -------------------------------------------------------------

/// A node, placed in space and projected.
#[derive(Clone, Copy)]
struct Star {
    id: NodeId,
    x: i32,
    y: i32,
    /// Distance from the eye, for the painter's order.
    z: i32,
    /// Perspective, in thousandths: near is bigger.
    s: i32,
}

/// The sine of `a` 1024ths of a turn, in thousandths: the 64-step table with
/// the step in between, so turning slowly does not jump.
fn sin1024(a: i32) -> i32 {
    let a = a.rem_euclid(1024);
    let (i, f) = (a / 16, a % 16);
    let (p, q) = (astros::sin64(i), astros::sin64(i + 1));
    p + (q - p) * f / 16
}

fn cos1024(a: i32) -> i32 {
    sin1024(a + 256)
}

/// How far toward the eye the sky leans (1024ths of a turn): a little, so
/// what is deeper reads as further away and not as lower.
const TILT: i32 = 110;
/// The distance of the eye: the strength of the perspective.
const EYE: i32 = 1300;
/// How deep each step of the declared tree goes.
const DEPTH: i32 = 240;

/// What the sky needs to place its stars: the graph, the files (for the
/// depth), the camera's zoom and how far it has turned.
pub struct Look<'a> {
    pub graph: &'a Graph,
    pub files: &'a [FileEntry],
    pub cam: &'a Camera,
    pub turn: u32,
}

/// Every star of the package, in space and on screen, far ones first.
fn stars(sc: &Look, w: i32, h: i32) -> ([Star; MAX_NODES], usize) {
    let g = sc.graph;
    let n = g.nodes().len().min(MAX_NODES);
    let (mut cx, mut cy) = (0i64, 0i64);
    for node in &g.nodes()[..n] {
        cx += (node.x + NODE_W / 2) as i64;
        cy += (node.y + NODE_H / 2) as i64;
    }
    let (cx, cy) = ((cx / n.max(1) as i64) as i32, (cy / n.max(1) as i64) as i32);
    let mid = (LEFT + (w - LEFT) / 2, TOP + TAB_H + (h - TOP - TAB_H - PANEL) / 2 + 10);
    let (ca, sa, cb, sb) = (cos1024(sc.turn as i32), sin1024(sc.turn as i32), cos1024(TILT), sin1024(TILT));
    let empty = Star { id: NodeId(0), x: 0, y: 0, z: 0, s: 1000 };
    let mut out = [empty; MAX_NODES];
    for (i, node) in g.nodes()[..n].iter().enumerate() {
        let id = NodeId(i as u8);
        // The 3060 and the DIRECTOR are below everything: they are the metal.
        let depth = match node.kind {
            NodeKind::Gpu | NodeKind::Director => 3,
            NodeKind::Root => 0,
            _ => sc.files.iter().find(|f| f.node == id).map_or(2, |f| f.depth as i32),
        };
        let (x, y, z) = (node.x + NODE_W / 2 - cx, node.y + NODE_H / 2 - cy, depth * DEPTH - DEPTH * 3 / 2);
        let (x1, z1) = ((x * ca + z * sa) / 1000, (z * ca - x * sa) / 1000);
        let (y2, z2) = ((y * cb - z1 * sb) / 1000, (y * sb + z1 * cb) / 1000);
        let s = (EYE * 1000 / (EYE + z2).max(200)).clamp(300, 3000);
        let zoom = sc.cam.zoom;
        out[i] = Star { id, x: mid.0 + x1 * s / 1000 * zoom / 1000, y: mid.1 + y2 * s / 1000 * zoom / 1000, z: z2, s };
    }
    // Painter's order: the far ones first (a handful of nodes: insertion sort).
    for i in 1..n {
        let mut j = i;
        while j > 0 && out[j - 1].z < out[j].z {
            out.swap(j - 1, j);
            j -= 1;
        }
    }
    (out, n)
}

/// The node under (x, y) in the sky: the NEAREST to the eye that is close.
pub fn hit_sky(sc: &Look, w: i32, h: i32, x: i32, y: i32) -> Option<NodeId> {
    let (list, n) = stars(sc, w, h);
    list[..n].iter().rev().find(|s| {
        let r = (40 * s.s / 1000 * sc.cam.zoom / 1000).max(14);
        (s.x - x).abs() < r && (s.y - y).abs() < r
    }).map(|s| s.id)
}

/// A floor of light under the package: what makes the depth readable.
fn floor(c: &mut Canvas, sc: &Look, w: i32, h: i32) {
    let mid = (LEFT + (w - LEFT) / 2, TOP + TAB_H + (h - TOP - TAB_H - PANEL) / 2 + 10);
    let (ca, sa, cb, sb) = (cos1024(sc.turn as i32), sin1024(sc.turn as i32), cos1024(TILT), sin1024(TILT));
    let zoom = sc.cam.zoom;
    let put = |x: i32, z: i32| {
        let y = 330;
        let (x1, z1) = ((x * ca + z * sa) / 1000, (z * ca - x * sa) / 1000);
        let (y2, z2) = ((y * cb - z1 * sb) / 1000, (y * sb + z1 * cb) / 1000);
        let s = (EYE * 1000 / (EYE + z2).max(200)).clamp(300, 3000);
        (mid.0 + x1 * s / 1000 * zoom / 1000, mid.1 + y2 * s / 1000 * zoom / 1000)
    };
    let line_color = mezclar(EDGE, BLUE, 1, 3);
    let span = 720;
    for k in -6..=6 {
        let v = k * 120;
        for (a, b) in [(put(v, -span), put(v, span)), (put(-span, v), put(span, v))] {
            let steps = ((b.0 - a.0).abs() + (b.1 - a.1).abs()).max(1) / 4;
            for s in 0..=steps {
                let (x, y) = (a.0 + (b.0 - a.0) * s / steps.max(1), a.1 + (b.1 - a.1) * s / steps.max(1));
                if x > LEFT && y > TOP + TAB_H + 8 && y < h - PANEL {
                    c.blend(x, y, line_color, 2, 3);
                }
            }
        }
    }
}

fn sky(c: &mut Canvas, sc: &Scene, t: i32) {
    let (w, h) = (c.w, c.h);
    let look = Look { graph: sc.graph, files: sc.files, cam: sc.cam, turn: sc.turn };
    floor(c, &look, w, h);
    let g = sc.graph;
    let (list, n) = stars(&look, w, h);
    let at = |id: NodeId| list[..n].iter().find(|s| s.id == id).copied();
    let lively = sc.flow_ms.is_some();
    // The cables first, behind every star, each with its class's colour; the
    // far ones fainter.
    for (k, e) in g.edges().iter().enumerate() {
        let (Some(a), Some(b)) = (at(e.from), at(e.to)) else { continue };
        let fade = (((a.s + b.s) / 2 - 500) / 250).clamp(1, 4) as u32;
        cable(c, (a.x, a.y), (b.x, b.y), cable_of(g, sc.files, e), t + k as i32 * 431, lively, fade);
    }
    for s in &list[..n] {
        let Some(node) = g.node(s.id) else { continue };
        let r = (24 * s.s / 1000 * sc.cam.zoom / 1000).max(5);
        if sc.faults.get().contains(&s.id) {
            supernova(c, s.x, s.y, r, t);
        }
        if sc.selected == Some(s.id) {
            halo(c, s.x, s.y, r + 10, 4, ACCENT, 260);
        }
        match node.kind {
            NodeKind::Root => centaur(c, s.x, s.y, (150 * s.s / 1000 * sc.cam.zoom / 1000).max(40), t),
            NodeKind::Gpu => pulsar(c, s.x, s.y, r, t),
            NodeKind::Director => station(c, s.x, s.y, r, t),
            NodeKind::Dependency => astros::asteroid(c, s.x, s.y, r, hash(node.name.as_bytes())),
            NodeKind::Module => astros::planet(c, s.x, s.y, r, node.name.as_bytes(), traits_of(sc.files, s.id), t),
        }
    }
    // The names in a second pass, over every star: a near planet must not
    // hide the name of a far one.
    for s in &list[..n] {
        let Some(node) = g.node(s.id) else { continue };
        let r = (24 * s.s / 1000 * sc.cam.zoom / 1000).max(5);
        let label = node.name.as_bytes();
        let below = if node.kind == NodeKind::Root { 80 * s.s / 1000 * sc.cam.zoom / 1000 } else { r * 2 + 8 };
        let ink = if sc.faults.get().contains(&s.id) { BAD } else if sc.selected == Some(s.id) { ACCENT } else { INK };
        let (lx, ly) = (s.x - label.len() as i32 * 4, s.y + below);
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            c.text(lx + dx, ly + dy, label, BG, 1);
        }
        c.text(lx, ly, label, ink, 1);
    }
    legend(c, sc);
}

/// The bottom of the sky: the picked star in words, and the cables' colours.
fn legend(c: &mut Canvas, sc: &Scene) {
    let g = sc.graph;
    let top = c.h - PANEL;
    c.rect(LEFT, top, c.w - LEFT, PANEL, BG);
    crate::view::ring_line(c, LEFT, top, c.w - LEFT);
    let x = LEFT + 16;
    let mut t = Buf::new();
    t.s("CIELO 3D  ").num(g.nodes().len() as u32).s(" astros, ").num(g.edges().len() as u32).s(" cables.  ");
    t.s("arrastra el cielo: gira   clic en un astro: su fichero   [t] siguiente solapa");
    c.text_fit(x, top + 10, t.get(), TITLE, c.w - x - 16);
    // The picked one, read out of its traits: what the star SAYS, in words.
    let mut line = Buf::new();
    match sc.selected.and_then(|id| g.node(id).map(|n| (id, n))) {
        Some((id, n)) if n.kind == NodeKind::Module => {
            let tr = traits_of(sc.files, id);
            line.b(n.name.as_bytes()).s(":  ");
            if tr.lines == 0 {
                line.s("protoplaneta -- todavia no tiene cuerpo");
            } else {
                line.num(tr.fns as u32).s(" fn (medida)  ").num(tr.lets as u32).s(" let (lunas)  ");
                line.num(tr.muts as u32).s(" mut (anillos, ").num(tr.changes as u32).s(" cambios)  ");
                line.num(tr.writes as u32).s(" print (paquetes)  ").num(tr.calls as u32).s(" llamadas (cometas)  ");
                line.num(tr.ifs as u32).s(" if (doble)");
            }
        }
        Some((_, n)) => {
            line.b(n.name.as_bytes()).s(match n.kind {
                NodeKind::Root => ":  el CENTAURO -- el paquete: las patas son BMO-X, el torso TITAN++",
                NodeKind::Gpu => ":  el PULSAR -- la 3060: lo que se le presta vuelve en el wait",
                NodeKind::Director => ":  la ESTACION -- el sistema: el kernel, el segundo juez",
                _ => ":  un ASTEROIDE -- una dependencia de fuera",
            });
        }
        None => {
            line.s("elige un astro (aqui o en el EXPLORER) y aqui se lee lo que es");
        }
    }
    c.text_fit(x, top + 36, line.get(), INK, c.w - x - 16);
    // The cables: fixed colours, the same in every tab.
    let mut cx = x;
    for kind in [astros::Cable::Mod, astros::Cable::Use, astros::Cable::Gpu, astros::Cable::System] {
        cable(c, (cx, top + 72), (cx + 40, top + 72), kind, 0, false, 4);
        cx += 48;
        let label = match kind {
            astros::Cable::Mod => &b"mod: lo declara (lazo fuerte)"[..],
            astros::Cable::Use => b"use: depende de el",
            astros::Cable::Gpu => b"prestamo a la 3060",
            astros::Cable::System => b"puerta del sistema",
        };
        cx += c.text(cx, top + 64, label, DIM, 1) + 24;
    }
}
