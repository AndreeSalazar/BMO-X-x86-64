//! **THE LOOK** -- everything F1 decides about how it LOOKS, and nothing else.
//!
//! The owner, 04-10: *"si es para mejorar apariencia usa MAQUETA ... divide
//! bien en apariencia y la logica de TITAN++"*. So F1 is cut in two, and the
//! cut is a rule anyone can check:
//!
//! ```text
//!    LOGIC        what the package IS and what may be done to it
//!                 titan-lector (the tree, the traits, organize, wire),
//!                 titan-contrato (the graph, the script, the certificate),
//!                 the compiler, and store.rs (the glue to the disk).
//!                 NONE of it names a colour, a radius or a pixel --
//!                 titan-lector has a test that fails if it does
//!
//!    APPEARANCE   how that is DRAWN
//!                 aspecto/titan.maqueta   the colours, as MAQUETA reads them
//!                 tema_gen.rs             ... generated from it (never by hand)
//!                 aspecto.rs  (here)      the roles, the measures, and the
//!                                         soft pieces of MAQUETA 2 (bmo-pinta)
//!                 view, space, astros,    the painters: they READ the logic
//!                 guia, explorer          and decide nothing about it
//! ```
//!
//! ** WHY THE SOFT PIECES ARE MAQUETA'S AND NOT F1'S. `bmo-pinta` is the
//! painter MAQUETA 2 brought (04-10, "nitidez de matematica, como SVG"): each
//! edge pixel gets exactly the part of the shape that covers it. It is the
//! same code the desktop and the host's photo run, so a rounded card in F1 and
//! a rounded card on the desktop have the same edge, pixel for pixel. F1 only
//! writes the ADAPTER (`Soft`): its canvas answering the painter's two
//! questions -- a solid rectangle, a colour blended over a pixel.
//!
//! [!] What is NOT here yet, said so nobody looks for it: the house LETTER
//! (`bmo-letra`, proportional and smooth). A new glyph costs ~13 KiB of stack
//! (`userland/src/pantalla/verde/fina.rs` measured it) and F1's deepest frame
//! is already ~57 KiB of the 64 KiB of Ring 3. The 8x16 font stays until that
//! frame shrinks; the pieces below cost no stack worth counting.

use crate::canvas::Canvas;
use crate::tema_gen as t;
use bmo_dibujo::{mezclar, Color, Lienzo as _};

// -- THE ROLES ------------------------------------------------------------
// One name per job, read from the generated palette. The painters use these
// names; only this block knows which class of `titan.maqueta` each one is.

pub const BG: Color = t::CANVAS_FONDO;
pub const DOT: Color = t::GRID;
pub const BAR: Color = t::BAR_FONDO;
pub const BODY: Color = t::NODE_FONDO;
pub const EDGE: Color = t::NODE_BORDE;
/// The row picked in the EXPLORER.
pub const SEL: Color = t::PICKED_FONDO;
pub const INK: Color = t::INK;
pub const DIM: Color = t::INK_DIM;
pub const TITLE: Color = t::TITLE;
/// The two ends of every gradient: the logo's ring.
pub const BLUE: Color = t::BLUE;
pub const VIOLET: Color = t::VIOLET;
pub const ACCENT: Color = t::ACCENT;
pub const GOOD: Color = t::GOOD;
pub const BAD: Color = t::BAD;
pub const GREY: Color = t::GREY;
/// A cable at rest: its soft halo and its bright core.
pub const CABLE_HALO: Color = t::CABLE_BORDE;
pub const CABLE_CORE: Color = t::CABLE;
/// The classes of value and cable: one colour, ONE meaning, in every tab.
pub const CYAN: Color = t::USE;
pub const AMBER: Color = t::MUT;
pub const GOLD: Color = t::DECIDE;

// -- THE MEASURES --------------------------------------------------------

/// A node, in world pixels.
pub const NODE_W: i32 = 240;
pub const NODE_H: i32 = 104;
/// The title bar, the EXPLORER column and the bottom panel, in screen pixels.
pub const TOP: i32 = 28;
pub const LEFT: i32 = 250;
pub const PANEL: i32 = 132;
/// Zoom levels, in thousandths.
pub const LEVELS: [i32; 6] = [500, 750, 1000, 1250, 1500, 2000];
/// The roundness of each kind of box, in pixels at zoom 1000.
pub const R_NODE: i32 = 10;
pub const R_CARD: i32 = 12;
pub const R_TAB: i32 = 10;
pub const R_CHIP: i32 = 9;

// -- THE SOFT PIECES (MAQUETA 2) ----------------------------------------

/// F1's canvas, as `bmo-pinta` sees it. A wrapper and not a second `impl`
/// on `Canvas`: both traits have a `rect`, and a call that could mean either
/// is a call nobody can read.
struct Soft<'a>(&'a mut Canvas);

impl bmo_pinta::Lienzo for Soft<'_> {
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        self.0.rect(x, y, w, h, c);
    }

    fn mezclar(&mut self, x: i32, y: i32, c: Color, alfa: u8) {
        self.0.blend(x, y, c, alfa as u32, 255);
    }
}

/// A rounded box, solid, its curve smooth.
pub fn card(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, r: i32, fill: Color) {
    bmo_pinta::caja(&mut Soft(c), x, y, w, h, r, fill);
}

/// The border of a rounded box, `thick` pixels, smooth on both sides.
pub fn edge(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, r: i32, thick: i32, color: Color) {
    bmo_pinta::borde(&mut Soft(c), x, y, w, h, r, thick, color);
}

/// Light around a rounded box (`box-shadow: 0 0 Npx`): `strength` of 255 at
/// its edge, falling to nothing at `reach`. Inside is left alone.
///
/// [!] It visits every pixel of the box and its ring: keep it for ONE thing
/// at a time (the picked node, a tab), not for every node every frame.
pub fn shine(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, r: i32, reach: i32, color: Color, strength: u8) {
    bmo_pinta::resplandor(&mut Soft(c), x, y, w, h, r, reach, (strength as u32) << 24 | color & 0x00FF_FFFF);
}

/// Two colours across a rounded box: left to right, or top to bottom.
pub fn band(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, r: i32, from: Color, to: Color, vertical: bool) {
    bmo_pinta::degradado(&mut Soft(c), x, y, w, h, r, from, to, vertical);
}

/// A label in a soft pill: what a chip, a tab or a tooltip is.
pub fn pill(c: &mut Canvas, x: i32, y: i32, text: &[u8], color: Color, fill: Color) -> i32 {
    let w = text.len() as i32 * 8 + 14;
    card(c, x, y, w, 18, R_CHIP, fill);
    edge(c, x, y, w, 18, R_CHIP, 1, color);
    c.text(x + 7, y + 1, text, color, 1);
    w
}

/// A radius scaled with the zoom, never less than 2 (a 0 would be a square
/// corner that comes and goes as the wheel turns).
pub fn r_at(r: i32, zoom: i32) -> i32 {
    (r * zoom / 1000).max(2)
}

// -- THE TWO LINES EVERY PANEL SHARES -----------------------------------

/// A thin line of the ring's gradient: blue to violet and back to blue.
pub fn ring_line(c: &mut Canvas, x: i32, y: i32, w: i32) {
    c.gradient(x, y, w / 2, 1, BLUE, VIOLET);
    c.gradient(x + w / 2, y, w - w / 2, 1, VIOLET, BLUE);
}

/// A picked row of a list: a soft dark blue band with a bar of light on its
/// left.
pub fn picked_row(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    card(c, x, y, w, h, 4, SEL);
    card(c, x, y + 2, 2, h - 4, 1, ACCENT);
}

/// Half way between two colours: the hue of a header's border.
pub fn half(a: Color, b: Color) -> Color {
    mezclar(a, b, 1, 2)
}
