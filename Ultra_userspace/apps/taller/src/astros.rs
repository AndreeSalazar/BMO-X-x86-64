//! **THE STARS** -- how each thing of a TITAN++ package is drawn in the SPACE
//! tabs: one painter per element, all fed by what the code IS (its kind, its
//! name, its TRAITS), so a node changes shape the moment its file is saved.
//!
//! ```text
//!    a module          a PLANET; its size from its `fn`, its colour from its
//!                      NAME (two never alike, each always the same)
//!      its `let`       MOONS, still on their orbits: values that do not change
//!      its `let mut`   amber RINGS that turn -- faster the more `x = ...` it has
//!      its `print`     an EMITTER: packets that leave it, toward the console
//!      its calls       COMETS that go round it
//!      no body yet     a PROTOPLANET: a cloud with a dashed edge
//!    the package       THE CENTAUR (legs BMO-X, torso and bow TITAN++)
//!    the 3060          a PULSAR       the DIRECTOR   a STATION
//!    a fault           a SUPERNOVA    a dependency   an ASTEROID
//! ```
//!
//! ** And the CABLES have a class, and the class gives the colour -- FIXED, not
//! chosen (the owner, 04-10: colours "que te limitan para no tener que pelear
//! por motivos"): the colour SAYS what the cable is, in the graph and in the
//! sky, the same everywhere.
//!
//! ```text
//!    MOD     who DECLARES whom: the strong bond      violet, braided, thick
//!    USE     whom it DEPENDS on: the connector       cyan light
//!    3060    a loan to the GPU (U1)                  green
//!    SYSTEM  a door of the kernel (the DIRECTOR)     blue
//! ```
//!
//! Integer arithmetic only: `core` has no `sin`, so a quarter of a sine wave
//! lives in a table of 17 numbers.

use crate::canvas::Canvas;
pub use crate::aspecto::{AMBER, CYAN};
use crate::aspecto::{ACCENT, BAD, BG, BLUE, DIM, GOLD, GOOD, GREY, INK, LILAC, TITLE, VIOLET};
use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_titan_contrato::{Edge, Graph, NodeKind};
use bmo_titan_lector::{FileEntry, Traits};


// -- A sine without floats -------------------------------------------------

/// sin, in thousandths, for a quarter turn in 16 steps.
const QUARTER: [i32; 17] = [0, 98, 195, 290, 383, 471, 556, 634, 707, 773, 831, 882, 924, 957, 981, 995, 1000];

/// sin(a), a in 64ths of a turn, in thousandths.
pub fn sin64(a: i32) -> i32 {
    let a = a.rem_euclid(64);
    match a / 16 {
        0 => QUARTER[a as usize],
        1 => QUARTER[(32 - a) as usize],
        2 => -QUARTER[(a - 32) as usize],
        _ => -QUARTER[(64 - a) as usize],
    }
}

pub fn cos64(a: i32) -> i32 {
    sin64(a + 16)
}

/// FNV-1a: the same name, the same planet, every time and on every machine.
pub fn hash(name: &[u8]) -> u32 {
    name.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

/// The planets' colours: the logo's blues and violets, and a few warm ones so
/// a big package does not read as one colour.
const HUES: [Color; 8] = [0x003D_6BFF, 0x008C_52FF, 0x0036_C4D8, 0x00E0_5AA8, 0x0052_E0A0, 0x00F0_A848, 0x0070_D6FF, 0x00B4_6CF0];

pub fn hue_of(name: &[u8]) -> Color {
    HUES[(hash(name) % HUES.len() as u32) as usize]
}

pub fn white(v: u32) -> Color {
    let v = v.min(255);
    v << 16 | v << 8 | v
}

/// A ROUND halo of light from radius `r` out to `r + spread`, fading (the
/// canvas's `glow` is square, made for the boxes of the graph tab).
pub fn halo(c: &mut Canvas, x: i32, y: i32, r: i32, spread: i32, color: Color, strength: u32) {
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

/// An ellipse of light: `rx` wide, `ry` tall; only the arc from `a0` to `a1`
/// (64ths of a turn), so rings can pass behind and in front of a planet.
pub fn arc(c: &mut Canvas, x: i32, y: i32, rx: i32, ry: i32, a0: i32, a1: i32, color: Color) {
    let steps = (rx.max(ry) * 2).clamp(16, 160);
    for k in 0..=steps {
        let a = a0 * 16 + (a1 - a0) * 16 * k / steps;
        let (px, py) = (x + rx * cos64(a / 16) / 1000, y + ry * sin64(a / 16) / 1000);
        c.blend(px, py, color, 2, 3);
    }
}

pub fn orbit(c: &mut Canvas, x: i32, y: i32, rx: i32, ry: i32, color: Color) {
    arc(c, x, y, rx, ry, 0, 64, color);
}

// -- The cables --------------------------------------------------------------

/// What a cable IS, and so its colour: fixed, the same in every tab.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Cable {
    /// The parent DECLARES the child (`mod`): the strong bond.
    Mod,
    /// It DEPENDS on it (`use`): the connector.
    Use,
    /// To the 3060: a loan until the `wait` (U1).
    Gpu,
    /// To the DIRECTOR: a door of the kernel.
    System,
}

impl Cable {
    pub fn color(self) -> Color {
        match self {
            Cable::Mod => VIOLET,
            Cable::Use => CYAN,
            Cable::Gpu => GOOD,
            Cable::System => BLUE,
        }
    }
    pub fn thick(self) -> i32 {
        match self {
            Cable::Mod => 3,
            _ => 2,
        }
    }
}

/// The class of a cable: the 3060 and the DIRECTOR by where it goes; between
/// modules, `mod` if the one above DECLARES the one below, else `use`.
pub fn cable_of(g: &Graph, files: &[FileEntry], e: &Edge) -> Cable {
    match g.node(e.to).map(|n| n.kind) {
        Some(NodeKind::Gpu) => Cable::Gpu,
        Some(NodeKind::Director) => Cable::System,
        _ => {
            let declared = files.iter().any(|f| f.node == e.to && f.parent == e.from && f.depth > 0);
            if declared {
                Cable::Mod
            } else {
                Cable::Use
            }
        }
    }
}

/// The number a node's SEAL grows from: its name, mixed with the sum of its
/// file's bytes (`titan_lector::package::sum`) when it came from one. Two
/// nodes never share it, and saving a file changes it.
pub fn seal_seed(files: &[FileEntry], node: bmo_titan_contrato::NodeId, name: &[u8]) -> u32 {
    let sum = files.iter().find(|f| f.node == node).map_or(0, |f| f.sum);
    hash(name) ^ sum.rotate_left(13)
}

/// **THE SEAL** of a node (`docs/plan/PLAN_LA_BANDEJA.md`, section 5, and the
/// mockup `docs/arte/maqueta_taller_estratos.html`): an uneven polygon of 5 to
/// 9 points, a dotted ring and a core, all from ONE number. The same number
/// draws the same seal on the Ryzen and in the host's camera; another number,
/// another seal.
pub fn seal(c: &mut Canvas, x: i32, y: i32, r: i32, seed: u32, color: Color) {
    let mut s = seed | 1;
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    let n = 5 + (next() % 5) as i32;
    let turn = (next() % 64) as i32;
    let mut pts = [(0i32, 0i32); 9];
    for i in 0..n {
        let a = turn + i * 64 / n + (next() % 3) as i32;
        let rr = r * (72 + (next() % 33) as i32) / 100;
        pts[i as usize] = (x + rr * cos64(a) / 1000, y + rr * sin64(a) / 1000);
    }
    for i in 0..n as usize {
        c.line(pts[i], pts[(i + 1) % n as usize], color);
    }
    // The ring: which of its 16 dots are lit is part of the number too.
    let (ring, dots) = (r * 45 / 100, next());
    if ring >= 3 {
        for k in 0..16 {
            if dots >> k & 1 == 1 {
                let a = k * 4;
                c.put(x + ring * cos64(a) / 1000, y + ring * sin64(a) / 1000, mezclar(color, BG, 2, 3));
            }
        }
    }
    c.disc(x, y, (r / 6).max(1), color);
}

/// The traits of a node's file, if it came from one.
pub fn traits_of(files: &[FileEntry], node: bmo_titan_contrato::NodeId) -> Traits {
    files.iter().find(|f| f.node == node).map_or(Traits::NONE, |f| f.traits)
}

// -- The elements ------------------------------------------------------------

/// A module: a planet built from its name and what its body does.
pub fn planet(c: &mut Canvas, x: i32, y: i32, base: i32, name: &[u8], tr: Traits, t: i32) {
    let h = hash(name);
    let hue = hue_of(name);
    if tr.lines == 0 {
        protoplanet(c, x, y, base, hue, t);
        return;
    }
    // Bigger with each `fn` (up to twice), and a little of its own.
    let r = base * (80 + (h >> 8) as i32 % 20 + (tr.fns.min(5) as i32 * 12)) / 100;
    let tilt = 3 + ((h >> 20) % 2) as i32;
    // The `mut` rings: behind first, the planet, then in front.
    let rings = tr.muts.min(3) as i32;
    for k in 0..rings {
        let rr = r * (15 + k * 3) / 10;
        arc(c, x, y, rr, rr / tilt, 32, 64, mezclar(AMBER, BG, 1, 3));
    }
    halo(c, x, y, r, 9, hue, 160);
    c.disc(x, y, r, mezclar(hue, BG, 2, 5));
    c.disc(x - r / 6, y - r / 6, r * 5 / 6, hue);
    let spin = t / (60 + (h % 40) as i32);
    for b in 0..3 {
        let by = y - r / 2 + b * r / 2 + (sin64(spin + b * 11) * r / 8000);
        let half = r * 7 / 10;
        for bx in -half..half {
            c.blend(x + bx, by, mezclar(hue, BG, 1, 2), 1, 3);
        }
    }
    c.disc(x - r / 3, y - r / 3, (r / 3).max(2), mezclar(hue, INK, 1, 2));
    // The front of the rings, and a spark that runs on each: they TURN, faster
    // the more lines change a `mut`.
    for k in 0..rings {
        let rr = r * (15 + k * 3) / 10;
        arc(c, x, y, rr, rr / tilt, 0, 32, AMBER);
        let speed = 2 + tr.changes.min(8) as i32 * 2;
        let a = t * speed / 400 + k * 21;
        let (sx, sy) = (x + rr * cos64(a) / 1000, y + rr / tilt * sin64(a) / 1000);
        if sin64(a) >= 0 {
            halo(c, sx, sy, 1, 3, AMBER, 220);
            c.disc(sx, sy, 1, INK);
        }
    }
    // The moons: its `let`, still on fixed orbits (they do not change).
    let moons = tr.lets.min(6) as i32;
    for m in 0..moons {
        let a = m * 64 / moons.max(1) + (h % 64) as i32;
        let (rx, ry) = (r + 10 + m * 3, (r + 10 + m * 3) / 2);
        let (mx, my) = (x + rx * cos64(a) / 1000, y + ry * sin64(a) / 1000);
        c.disc(mx, my, 2, mezclar(INK, hue, 1, 3));
    }
    // The comets: its calls, going round.
    for k in 0..tr.calls.min(3) as i32 {
        let a = t / (18 + k * 7) + k * 20;
        let (rx, ry) = (r + 22 + k * 5, r / 2 + 14);
        for tail in 0..6 {
            let b = a - tail;
            let (px, py) = (x + rx * cos64(b) / 1000, y + ry * sin64(b) / 1000);
            c.light(px, py, white(200 - tail as u32 * 30));
        }
        // With `return` (level 5) the call comes back CARRYING a value: a
        // bright head on the comet.
        if tr.returns > 0 {
            let (hx, hy) = (x + rx * cos64(a) / 1000, y + ry * sin64(a) / 1000);
            halo(c, hx, hy, 1, 4, GOLD, 180);
            c.disc(hx, hy, 1, INK);
        }
    }
    // The emitter: its `print`, packets leaving toward the console (up).
    if tr.writes > 0 {
        emitter(c, x, y - r, tr.writes.min(4) as i32, t);
    }
    // The crystals: its `type`s -- values with facets.
    if tr.types > 0 {
        crystal(c, x - r - 8, y - r / 2, tr.types.min(3) as i32, t);
    }
    // The belt: its `while` / `for` -- rocks that go round and round.
    if tr.loops > 0 {
        belt(c, x, y, r, tr.loops.min(3) as i32, t);
    }
    // The double stars: its `if`, each a pair -- two ways, one lit.
    if tr.ifs > 0 {
        double_star(c, x + r + 6, y + r / 2 + 4, tr.ifs.min(3) as i32, t);
    }
}

/// **A CRYSTAL**: a `type`. A record is a value with FACETS -- its fields --
/// and the crystal turns to show them, one face lit at a time.
pub fn crystal(c: &mut Canvas, x: i32, y: i32, n: i32, t: i32) {
    for k in 0..n {
        let (cx, cy) = (x - k * 10, y + k * 9);
        let a = t / 10 + k * 16;
        let w = 2 + (4 * cos64(a) / 1000).abs();
        let top = (cx, cy - 6);
        let bottom = (cx, cy + 6);
        let (l, r) = ((cx - w, cy), (cx + w, cy));
        halo(c, cx, cy, 3, 6, ACCENT, 120);
        c.line(top, l, ACCENT);
        c.line(top, r, ACCENT);
        c.line(l, bottom, mezclar(ACCENT, BG, 1, 2));
        c.line(r, bottom, mezclar(ACCENT, BG, 1, 2));
        c.line(l, r, INK);
    }
}

/// **A BELT**: what REPEATS. Rocks on a wide, flat orbit, going round and
/// round -- one ring of rocks per loop, up to three, each turning the other
/// way from the last (a loop inside a loop turns inside its own turn).
pub fn belt(c: &mut Canvas, x: i32, y: i32, r: i32, n: i32, t: i32) {
    for k in 0..n {
        let rx = r * (22 + k * 4) / 10;
        let ry = rx / 5 + 2;
        let dir = if k % 2 == 0 { 1 } else { -1 };
        for rock in 0..9 {
            let a = dir * t / (9 + k * 3) + rock * 64 / 9;
            let (px, py) = (x + rx * cos64(a) / 1000, y + ry * sin64(a) / 1000);
            // The far half is dim: the planet is in front of it.
            let near = sin64(a) >= 0;
            c.disc(px, py, if rock % 3 == 0 { 2 } else { 1 }, if near { LILAC } else { mezclar(LILAC, BG, 1, 3) });
        }
    }
}

/// **A DOUBLE STAR**: what DECIDES. Two stars turning round each other, the
/// two ways of an `if`: one shines gold (the way that runs), the other is a
/// grey ember (the dead side: the compiler decided it never runs, and it left
/// no byte in the `.bex`). One pair per decision, up to three.
pub fn double_star(c: &mut Canvas, x: i32, y: i32, n: i32, t: i32) {
    for k in 0..n {
        let (cx, cy) = (x + k * 9, y + k * 7);
        let a = t / 14 + k * 23;
        let (dx, dy) = (5 * cos64(a) / 1000, 3 * sin64(a) / 1000);
        c.disc(cx - dx, cy - dy, 1, GREY);
        halo(c, cx + dx, cy + dy, 2, 6, GOLD, 200);
        c.disc(cx + dx, cy + dy, 2, mezclar(GOLD, INK, 1, 2));
    }
}

/// Packets that leave a point and rise: what SENDS to the console.
pub fn emitter(c: &mut Canvas, x: i32, y: i32, n: i32, t: i32) {
    c.rect(x - 1, y - 6, 3, 6, TITLE);
    for k in 0..n {
        let u = (t / 6 + k * 40).rem_euclid(160);
        let py = y - 8 - u / 3;
        let px = x + sin64(u / 4 + k * 13) * 4 / 1000;
        let fade = 255 - u as u32;
        c.rect(px - 2, py - 2, 4, 4, mezclar(GOOD, BG, 255 - fade.min(255), 255));
        c.light(px, py, white(fade / 2));
    }
}

/// A module with no body yet: a cloud that has not become a planet.
pub fn protoplanet(c: &mut Canvas, x: i32, y: i32, r: i32, hue: Color, t: i32) {
    halo(c, x, y, r / 3, r, mezclar(hue, BG, 1, 2), 120);
    for a in (0..64).step_by(4) {
        let a = a + t / 120;
        c.light(x + r * cos64(a) / 1000, y + r * sin64(a) / 1000, mezclar(hue, BG, 1, 2));
    }
    for k in 0..7 {
        let a = (hash(&[k as u8]) % 64) as i32 + t / 90;
        let d = r * (k + 2) / 10;
        c.light(x + d * cos64(a) / 1000, y + d * sin64(a) / 1000, white(150));
    }
}

/// THE CENTAUR: the package. Stars and lines in a 100 x 100 box; the four
/// legs below are BMO-X, the torso and the bow above are TITAN++.
pub fn centaur(c: &mut Canvas, x: i32, y: i32, size: i32, t: i32) {
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
    const LEGS: [(usize, usize); 13] =
        [(9, 10), (10, 11), (11, 12), (12, 9), (12, 13), (13, 14), (12, 15), (15, 16), (11, 17), (17, 18), (11, 19), (19, 20), (10, 21)];
    let at = |(sx, sy): (i32, i32)| (x - size / 2 + sx * size / 100, y - size / 2 + sy * size / 100);
    halo(c, x, y, size / 4, size / 3, VIOLET, 90);
    for (a, b) in TORSO {
        c.line(at(STARS[a]), at(STARS[b]), mezclar(BLUE, VIOLET, 1, 2));
    }
    for (a, b) in LEGS {
        c.line(at(STARS[a]), at(STARS[b]), TITLE);
    }
    c.line(at(STARS[21]), at(STARS[22]), DIM);
    for (k, &s) in STARS.iter().enumerate() {
        let (sx, sy) = at(s);
        let tw = 170 + sin64(t / 40 + k as i32 * 7) * 85 / 1000;
        if matches!(k, 0 | 2 | 6 | 10 | 12) {
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
pub fn pulsar(c: &mut Canvas, x: i32, y: i32, r: i32, t: i32) {
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
    orbit(c, x, y, r, r / 3, GOOD);
}

/// THE DIRECTOR: a station with its two panels and a light that blinks.
pub fn station(c: &mut Canvas, x: i32, y: i32, r: i32, t: i32) {
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
pub fn asteroid(c: &mut Canvas, x: i32, y: i32, r: i32, h: u32) {
    for k in 0..5 {
        let a = (h >> (k * 5)) as i32 % 64;
        let d = r / 3;
        c.disc(x + d * cos64(a) / 1000, y + d * sin64(a) / 1000, r / 2, mezclar(DIM, BG, k as u32, 6));
    }
}

/// A node in fault: a red supernova that breathes.
pub fn supernova(c: &mut Canvas, x: i32, y: i32, r: i32, t: i32) {
    let grow = 8 + (sin64(t / 30) + 1000) * 10 / 2000;
    halo(c, x, y, r, grow + 10, BAD, 260);
    orbit(c, x, y, r + grow, r + grow, BAD);
}

/// A cable of its class between two points: the strong bond braided, the
/// rest as light; a comet runs down it while F1 is lively.
pub fn cable(c: &mut Canvas, a: (i32, i32), b: (i32, i32), kind: Cable, t: i32, lively: bool, fade: u32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let steps = ((dx.abs() + dy.abs()) / 3).max(2);
    let color = kind.color();
    for s in 0..=steps {
        let (x, y) = (a.0 + dx * s / steps, a.1 + dy * s / steps);
        match kind {
            // Braided: two threads that cross.
            Cable::Mod => {
                let w = sin64(s * 64 / 18) * 2 / 1000;
                c.blend(x + w, y - w, color, fade, 4);
                c.blend(x - w, y + w, color, fade, 4);
            }
            _ if s % 3 == 0 => c.blend(x, y, color, fade, 4),
            _ => {}
        }
    }
    if lively {
        let u = (t / 2).rem_euclid(1000);
        for tail in 0..8 {
            let v = u - tail * 14;
            if v < 0 {
                continue;
            }
            c.light(a.0 + dx * v / 1000, a.1 + dy * v / 1000, white(230 - tail as u32 * 25));
        }
    }
}
