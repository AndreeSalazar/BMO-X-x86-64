//! **THE SPACE TAB** -- the same graph, and every node as a STAR of its own
//! (the owner, 04-10: "nodos dinamicos unicos y divertidos, representantes del
//! espacio, como el centauro del espacio").
//!
//! ```text
//!    the package (Titan.toml)   THE CENTAUR: a constellation. Its four legs
//!                               are BMO-X, its torso and bow are TITAN++ --
//!                               TITAN_MAESTRO 6b.6, drawn
//!    a module                   a PLANET of its own: its colour, its ring and
//!                               its turning come from its NAME, so no two are
//!                               alike and each is always the same; its moons
//!                               are what it uses (its cables down)
//!    the 3060                   a PULSAR: beams that turn
//!    the DIRECTOR               a STATION, with its panels and its light
//!    a dependency               an ASTEROID
//!    a node in fault            a red SUPERNOVA around it
//!    a cable                    a road of light, and the comet that goes down
//! ```
//!
//! ** Nothing here decides anything: the nodes sit where the `[layout]` puts
//! them (so a click, a drag and the EXPLORER work the same on both tabs), and
//! what is drawn comes from the graph. It only MOVES while the window is seen
//! and someone touched it in the last 20 s (`flow_ms`, the house's rule: if it
//! does nothing, it spends nothing); at rest, the sky stands still.
//!
//! Integer arithmetic only: `core` has no `sin`, so a quarter of a sine wave
//! lives in a table of 17 numbers.

use crate::canvas::Canvas;
use crate::view::{node_rect, Buf, Scene, ACCENT, BAD, BAR, BG, BLUE, DIM, EDGE, GOOD, INK, LEFT, PANEL, TITLE, TOP, VIOLET};
use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_titan_contrato::{Graph, Node, NodeId, NodeKind};

/// The two tabs of the canvas.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Graph,
    Space,
}

const TAB_Y: i32 = TOP + 8;
const TAB_H: i32 = 20;
const TABS: [(Tab, &[u8], i32); 2] = [(Tab::Graph, b"GRAFO", 64), (Tab::Space, b"ESPACIO", 80)];

/// The tab under (x, y), if the pointer is on the strip.
pub fn tab_at(x: i32, y: i32) -> Option<Tab> {
    if !(TAB_Y..TAB_Y + TAB_H).contains(&y) {
        return None;
    }
    let mut tx = LEFT + 12;
    for (tab, _, w) in TABS {
        if (tx..tx + w).contains(&x) {
            return Some(tab);
        }
        tx += w + 6;
    }
    None
}

/// The strip of tabs, drawn on top of either canvas.
pub fn tabs(c: &mut Canvas, now: Tab) {
    let mut tx = LEFT + 12;
    for (tab, label, w) in TABS {
        let on = tab == now;
        c.rect(tx, TAB_Y, w, TAB_H, if on { 0x0010_1A46 } else { BAR });
        c.frame(tx, TAB_Y, w, TAB_H, 1, if on { ACCENT } else { EDGE });
        if on {
            c.gradient(tx + 1, TAB_Y + TAB_H - 2, w - 2, 2, BLUE, VIOLET);
        }
        let lw = label.len() as i32 * 8;
        c.text(tx + (w - lw) / 2, TAB_Y + 2, label, if on { INK } else { DIM }, 1);
        tx += w + 6;
    }
    c.text(tx + 4, TAB_Y + 2, b"[t]", DIM, 1);
}

// -- A sine without floats -------------------------------------------------

/// sin, in thousandths, for a quarter turn in 16 steps.
const QUARTER: [i32; 17] = [0, 98, 195, 290, 383, 471, 556, 634, 707, 773, 831, 882, 924, 957, 981, 995, 1000];

/// sin(a), a in 64ths of a turn, in thousandths.
fn sin64(a: i32) -> i32 {
    let a = a.rem_euclid(64);
    match a / 16 {
        0 => QUARTER[a as usize],
        1 => QUARTER[(32 - a) as usize],
        2 => -QUARTER[(a - 32) as usize],
        _ => -QUARTER[(64 - a) as usize],
    }
}

fn cos64(a: i32) -> i32 {
    sin64(a + 16)
}

/// FNV-1a: the same name, the same planet, every time and on every machine.
fn hash(name: &[u8]) -> u32 {
    name.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

/// The planets' colours: the logo's blues and violets, and a few warm ones so
/// a big package does not read as one colour.
const HUES: [Color; 8] = [0x003D_6BFF, 0x008C_52FF, 0x0036_C4D8, 0x00E0_5AA8, 0x0052_E0A0, 0x00F0_A848, 0x0070_D6FF, 0x00B4_6CF0];

/// A ROUND halo of light from radius `r` out to `r + spread`, fading: the
/// canvas's `glow` is square, made for the boxes of the graph tab.
fn halo(c: &mut Canvas, x: i32, y: i32, r: i32, spread: i32, color: Color, strength: u32) {
    let outer = r + spread;
    let (r2, o2) = (r * r, outer * outer);
    for dy in -outer..=outer {
        for dx in -outer..=outer {
            let d2 = dx * dx + dy * dy;
            if d2 > r2 && d2 <= o2 {
                let part = ((o2 - d2) as u32 * strength) / ((o2 - r2).max(1) as u32 * 100);
                if part > 0 {
                    c.blend(x + dx, y + dy, color, part, 4);
                }
            }
        }
    }
}

fn white(v: u32) -> Color {
    let v = v.min(255);
    v << 16 | v << 8 | v
}

/// Where a node's star sits: the middle of its box.
fn centre(cam: &crate::view::Camera, n: &Node) -> (i32, i32, i32) {
    let (x, y, w, h) = node_rect(cam, n);
    (x + w / 2, y + h / 2 - 6 * cam.zoom / 1000, cam.zoom)
}

/// One whole frame of the SPACE tab. The EXPLORER is drawn after, on its own.
pub fn draw(c: &mut Canvas, sc: &Scene) {
    let (g, cam) = (sc.graph, sc.cam);
    match sc.sky {
        Some(px) => c.blit(px),
        None => c.clear(BG),
    }
    // The clock: it only runs while F1 is lively; at rest, a still sky.
    let t = sc.flow_ms.unwrap_or(0) as i32;
    roads(c, g, cam, t, sc.flow_ms.is_some());
    for (i, n) in g.nodes().iter().enumerate() {
        let id = NodeId(i as u8);
        let (x, y, zoom) = centre(cam, n);
        if x < LEFT - 80 || y < TOP - 80 || x > c.w + 80 || y > c.h - PANEL + 80 {
            continue;
        }
        let r = (26 * zoom / 1000).max(6);
        if sc.faults.get().contains(&id) {
            supernova(c, x, y, r, t);
        }
        if sc.selected == Some(id) {
            orbit(c, x, y, r + 14, r / 3 + 6, ACCENT, 64);
        }
        match n.kind {
            NodeKind::Root => centaur(c, x, y, (150 * zoom / 1000).max(40), t),
            NodeKind::Gpu => pulsar(c, x, y, r, t),
            NodeKind::Director => station(c, x, y, r, t),
            NodeKind::Dependency => asteroid(c, x, y, r, hash(n.name.as_bytes())),
            NodeKind::Module => {
                let moons = g.edges().iter().filter(|e| e.from == id).count() as i32;
                planet(c, x, y, r, n.name.as_bytes(), moons, t);
            }
        }
        let label = n.name.as_bytes();
        let below = if n.kind == NodeKind::Root { (80 * zoom / 1000).max(22) } else { r + 12 };
        let lw = label.len() as i32 * 8;
        let ink = if sc.faults.get().contains(&id) { BAD } else if sc.selected == Some(id) { ACCENT } else { INK };
        c.text(x - lw / 2, y + below, label, ink, 1);
    }
    legend(c, g);
    crate::view::title(c, sc);
}

/// The cables as roads of light: dotted, faint, and -- while lively -- a comet
/// going down each one, the direction of every `mod` and `use`.
fn roads(c: &mut Canvas, g: &Graph, cam: &crate::view::Camera, t: i32, lively: bool) {
    for (k, e) in g.edges().iter().enumerate() {
        let (Some(a), Some(b)) = (g.node(e.from), g.node(e.to)) else { continue };
        let ((ax, ay, _), (bx, by, _)) = (centre(cam, a), centre(cam, b));
        let (dx, dy) = (bx - ax, by - ay);
        let len = (dx.abs() + dy.abs()).max(1);
        let steps = len / 9;
        for s in 0..=steps {
            let (x, y) = (ax + dx * s / steps.max(1), ay + dy * s / steps.max(1));
            if y > TOP + TAB_H + 10 && y < c.h - PANEL {
                c.blend(x, y, VIOLET, 1, 2);
            }
        }
        if lively {
            let u = (t + k as i32 * 431).rem_euclid(2000);
            for tail in 0..10 {
                let v = u - tail * 18;
                if v < 0 {
                    continue;
                }
                let (x, y) = (ax + dx * v / 2000, ay + dy * v / 2000);
                if y > TOP + TAB_H + 10 && y < c.h - PANEL {
                    c.light(x, y, white(240 - tail as u32 * 22));
                }
            }
        }
    }
}

/// An ellipse of light: `rx` wide, `ry` tall.
fn orbit(c: &mut Canvas, x: i32, y: i32, rx: i32, ry: i32, color: Color, steps: i32) {
    for a in 0..steps {
        let k = a * 64 / steps;
        c.blend(x + rx * cos64(k) / 1000, y + ry * sin64(k) / 1000, color, 2, 3);
    }
}

/// A planet of its own: hue, ring, size and turning from its name.
fn planet(c: &mut Canvas, x: i32, y: i32, r: i32, name: &[u8], moons: i32, t: i32) {
    let h = hash(name);
    let hue = HUES[(h % HUES.len() as u32) as usize];
    let r = r * (85 + (h >> 8) as i32 % 30) / 100;
    let ringed = (h >> 16) & 1 == 1;
    let tilt = 3 + ((h >> 20) % 3) as i32;
    if ringed {
        // The back half of the ring, behind the planet.
        for a in 32..64 {
            c.blend(x + (r * 17 / 10) * cos64(a) / 1000, y + (r * 17 / 10) * sin64(a) / 1000 / tilt, mezclar(hue, INK, 1, 2), 1, 2);
        }
    }
    halo(c, x, y, r, 9, hue, 160);
    c.disc(x, y, r, mezclar(hue, BG, 2, 5));
    c.disc(x - r / 6, y - r / 6, r * 5 / 6, hue);
    // Bands: the turning, slow, at its own speed.
    let spin = t / (60 + (h % 40) as i32);
    for b in 0..3 {
        let by = y - r / 2 + b * r / 2 + (sin64(spin + b * 11) * r / 8000);
        let half = r * 7 / 10;
        for bx in -half..half {
            c.blend(x + bx, by, mezclar(hue, BG, 1, 2), 1, 3);
        }
    }
    // The light on its face.
    c.disc(x - r / 3, y - r / 3, (r / 3).max(2), mezclar(hue, INK, 1, 2));
    if ringed {
        for a in 0..32 {
            c.blend(x + (r * 17 / 10) * cos64(a) / 1000, y + (r * 17 / 10) * sin64(a) / 1000 / tilt, mezclar(hue, INK, 2, 3), 2, 3);
        }
    }
    // Its moons: what it uses, turning around it.
    for m in 0..moons.min(6) {
        let a = t / (25 + m * 9) + m * 64 / moons.max(1);
        let (rx, ry) = (r + 9 + m * 4, (r + 9 + m * 4) / 2);
        let (mx, my) = (x + rx * cos64(a) / 1000, y + ry * sin64(a) / 1000);
        c.disc(mx, my, 2, mezclar(INK, hue, 1, 3));
        c.light(mx - 1, my - 1, white(120));
    }
}

/// THE CENTAUR: the package. Stars and lines in a 100 x 100 box; the four
/// legs below are BMO-X, the torso and the bow above are TITAN++.
fn centaur(c: &mut Canvas, x: i32, y: i32, size: i32, t: i32) {
    const STARS: [(i32, i32); 24] = [
        (50, 6),  // 0 head
        (50, 16), // 1 neck
        (47, 26), // 2 shoulder
        (46, 40), // 3 waist
        (32, 24), // 4 hand on the bow
        (24, 8),  // 5 bow, top
        (20, 26), // 6 bow, middle
        (24, 42), // 7 bow, bottom
        (8, 25),  // 8 arrow tip
        (46, 50), // 9 chest
        (80, 50), // 10 back
        (76, 62), // 11 back belly
        (50, 62), // 12 front belly
        (44, 74), // 13 front knee
        (42, 90), // 14 front hoof
        (56, 76), // 15 front knee 2
        (58, 92), // 16 front hoof 2
        (72, 76), // 17 back knee
        (68, 92), // 18 back hoof
        (82, 74), // 19 back knee 2
        (88, 90), // 20 back hoof 2
        (88, 54), // 21 tail
        (96, 64), // 22 tail end
        (58, 30), // 23 the other arm, back
    ];
    const TORSO: [(usize, usize); 11] = [(0, 1), (1, 2), (2, 3), (2, 4), (4, 6), (5, 6), (6, 7), (6, 8), (2, 23), (3, 9), (4, 8)];
    const LEGS: [(usize, usize); 13] = [
        (9, 10),
        (10, 11),
        (11, 12),
        (12, 9),
        (12, 13),
        (13, 14),
        (12, 15),
        (15, 16),
        (11, 17),
        (17, 18),
        (11, 19),
        (19, 20),
        (10, 21),
    ];
    let at = |(sx, sy): (i32, i32)| (x - size / 2 + sx * size / 100, y - size / 2 + sy * size / 100);
    halo(c, x, y, size / 4, size / 3, VIOLET, 90);
    // TITAN++ is the torso (blue to violet, the logo's ring), BMO-X the legs.
    for (a, b) in TORSO {
        c.line(at(STARS[a]), at(STARS[b]), mezclar(BLUE, VIOLET, 1, 2));
    }
    for (a, b) in LEGS {
        c.line(at(STARS[a]), at(STARS[b]), TITLE);
    }
    c.line(at(STARS[21]), at(STARS[22]), DIM);
    for (k, &s) in STARS.iter().enumerate() {
        let (sx, sy) = at(s);
        // Each star twinkles on its own phase.
        let tw = 170 + sin64(t / 40 + k as i32 * 7) * 85 / 1000;
        let big = matches!(k, 0 | 2 | 6 | 10 | 12);
        if big {
            halo(c, sx, sy, 1, 4, ACCENT, 200);
            c.disc(sx, sy, 2, white(tw as u32));
        } else {
            c.light(sx, sy, white(tw as u32));
            c.light(sx + 1, sy, white(tw as u32 / 2));
            c.light(sx, sy + 1, white(tw as u32 / 2));
        }
    }
}

/// THE 3060: a pulsar, two beams that turn.
fn pulsar(c: &mut Canvas, x: i32, y: i32, r: i32, t: i32) {
    let a = t / 30;
    let len = r * 2;
    for side in [0, 32] {
        let (ex, ey) = (x + len * cos64(a + side) / 1000, y + len * sin64(a + side) / 1000);
        let steps = 16;
        for s in 0..steps {
            let (px, py) = (x + (ex - x) * s / steps, y + (ey - y) * s / steps);
            c.light(px, py, mezclar(GOOD, BG, s as u32, steps as u32));
        }
    }
    halo(c, x, y, r / 3, r, GOOD, 220);
    c.disc(x, y, r / 3 + 2, INK);
    orbit(c, x, y, r, r / 3, GOOD, 48);
}

/// THE DIRECTOR: a station with its two panels and a light that blinks.
fn station(c: &mut Canvas, x: i32, y: i32, r: i32, t: i32) {
    let (bw, bh) = (r, r * 2 / 3);
    for side in [-1, 1] {
        let px = x + side * (bw / 2 + r / 2) - r / 2;
        c.rect(px, y - r / 4, r, r / 2, 0x0023_407E);
        c.frame(px, y - r / 4, r, r / 2, 1, BLUE);
        c.rect(px + r / 2, y - r / 4, 1, r / 2, BLUE);
    }
    c.rect(x - bw / 2, y - bh / 2, bw, bh, 0x0030_3A5E);
    c.frame(x - bw / 2, y - bh / 2, bw, bh, 1, TITLE);
    c.rect(x - 1, y - bh / 2 - r / 3, 2, r / 3, DIM);
    if (t / 500) % 2 == 0 {
        halo(c, x, y - bh / 2 - r / 3, 1, 5, BAD, 260);
        c.disc(x, y - bh / 2 - r / 3, 2, BAD);
    }
}

/// A dependency: an asteroid, its shape from its name.
fn asteroid(c: &mut Canvas, x: i32, y: i32, r: i32, h: u32) {
    for k in 0..5 {
        let a = (h >> (k * 5)) as i32 % 64;
        let d = r / 3;
        c.disc(x + d * cos64(a) / 1000, y + d * sin64(a) / 1000, r / 2, mezclar(DIM, BG, k as u32, 6));
    }
}

/// A node in fault: a red supernova that breathes.
fn supernova(c: &mut Canvas, x: i32, y: i32, r: i32, t: i32) {
    let grow = 8 + (sin64(t / 30) + 1000) * 10 / 2000;
    halo(c, x, y, r, grow + 10, BAD, 260);
    orbit(c, x, y, r + grow, r + grow, BAD, 56);
}

/// The bottom: what each star means.
fn legend(c: &mut Canvas, g: &Graph) {
    let top = c.h - PANEL;
    c.rect(LEFT, top, c.w - LEFT, PANEL, BG);
    crate::view::ring_line(c, LEFT, top, c.w - LEFT);
    let x = LEFT + 16;
    let mut t = Buf::new();
    t.s("ESPACIO  ").num(g.nodes().len() as u32).s(" astros, ").num(g.edges().len() as u32).s(" rutas de luz");
    c.text(x, top + 10, t.get(), TITLE, 1);
    let rows: [(&[u8], Color); 4] = [
        (b"el CENTAURO: el paquete. Las cuatro patas son BMO-X; el torso y el arco, TITAN++", VIOLET),
        (b"cada PLANETA es un modulo: su color, su anillo y su giro salen de su nombre; sus lunas, de lo que usa", BLUE),
        (b"el PULSAR es la 3060, la ESTACION es el DIRECTOR; en rojo, una SUPERNOVA: un modulo en fallo", GOOD),
        (b"[t] o la solapa: GRAFO / ESPACIO.  Se mueve solo mientras lo miras y lo tocas.", DIM),
    ];
    for (k, (line, color)) in rows.iter().enumerate() {
        c.disc(x + 3, top + 40 + k as i32 * 20, 3, *color);
        c.text_fit(x + 14, top + 33 + k as i32 * 20, line, if k == 3 { DIM } else { INK }, c.w - x - 30);
    }
}
