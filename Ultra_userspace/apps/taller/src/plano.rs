//! **THE ASSEMBLY BLUEPRINT** -- F1's instrument when the mission desktop is
//! behind (HM6e of `docs/plan/PLAN_EL_HUD.md`, 07-10): *"nodos = modulos con
//! puertos"*.
//!
//! [consumo] NADA      drawn with the GRAPH, when the GRAPH is drawn (L6h)
//!
//! The GRAPH already WAS that -- a node per module, its pins on top and below
//! and the cables of every `mod` and `use` (`view.rs`) --, so the instrument
//! does not redo it: it FRAMES it as a blueprint, with the HUD's pieces.
//!
//! ```text
//!    PLANO DE ENSAMBLAJE  //  F1   MODULOS 6  CABLES 7  3060 1  FALLOS 0
//!    +-      -+
//!     main.tt        every module between the eye's L corners
//!    +-      -+      (the 3060 one in gold, the failing one in red)
//! ```
//!
//! [!] In the 8x16 letter, not the house one: F1 has no stack for it (see
//! `aspecto.rs`). For the same reason the backdrop is asked ONCE, on opening
//! (`preguntar`, from `_start`), and not on the deepest frame of painting.

use core::sync::atomic::{AtomicBool, Ordering};

use bmo_dibujo::{Color, Lienzo as _};
use bmo_titan_contrato::NodeKind;
use bmo_userland as bmo;

use crate::canvas::Canvas;
use crate::view::{node_rect, Scene, LEFT, TOP};

/// The mission palette: the desktop's, the SAME generated file.
#[path = "../../../services/director/src/scene/tema_gen.rs"]
#[allow(dead_code)]
mod tema;
use tema::{MISION_BORDE as BORDE, MISION_CUIDADO as ORO, MISION_FONDO as FONDO, MISION_NOGO as NOGO, MISION_OJO as OJO, MISION_TENUE as TENUE, MISION_TINTA as TINTA};

static MISION: AtomicBool = AtomicBool::new(false);

/// **Is the mission desktop behind?** Reads `sys/director.cfg` by the
/// desktop's rule (`bmo_config::es_mision`). Called ONCE, on opening.
pub fn preguntar() {
    let mut b = [0u8; 4096];
    let n = bmo::Archivo::leer_de(b"sys/director.cfg").ok().map(|a| a.read(&mut b[..(a.size() as usize).min(4096)]));
    MISION.store(bmo_config::es_mision(n.map(|n| &b[..n])), Ordering::Relaxed);
}

/// What [`preguntar`] answered.
pub fn es_mision() -> bool {
    MISION.load(Ordering::Relaxed)
}

/// The L of each corner.
const L: i32 = 10;

/// The four L corners around `(x, y, w, h)`, `d` pixels outside.
fn esquinas(c: &mut Canvas, (x, y, w, h): (i32, i32, i32, i32), d: i32, color: Color) {
    let (x0, y0, x1, y1) = (x - d, y - d, x + w + d, y + h + d);
    for (ex, ey, vx, vy) in [(x0, y0, x0, y0), (x1 - L, y0, x1 - 2, y0), (x0, y1 - 2, x0, y1 - L), (x1 - L, y1 - 2, x1 - 2, y1 - L)] {
        c.rect(ex, ey, L, 2, color);
        c.rect(vx, vy, 2, L, color);
    }
}

/// A short number in decimal.
fn num(v: usize, b: &mut [u8; 8]) -> &[u8] {
    let (mut i, mut v) = (b.len(), v);
    loop {
        i -= 1;
        b[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 || i == 0 {
            break;
        }
    }
    &b[i..]
}

/// **Frames the GRAPH** as a blueprint: each module's corners and the plate.
pub fn draw(c: &mut Canvas, sc: &Scene) {
    let g = sc.graph;
    let fallan = sc.faults.get();
    for (i, n) in g.nodes().iter().enumerate() {
        let id = bmo_titan_contrato::NodeId(i as u8);
        let color = if fallan.contains(&id) {
            NOGO
        } else if n.kind == NodeKind::Gpu {
            ORO
        } else {
            OJO
        };
        esquinas(c, node_rect(sc.cam, n), 6, color);
    }
    // -- the plate: one line, top right of the canvas (the tabs are on the
    // left, and `[t]` at the very right) --
    let modulos = g.nodes().iter().filter(|n| n.kind == NodeKind::Module).count();
    let gpu = g.nodes().iter().filter(|n| n.kind == NodeKind::Gpu).count();
    let lecturas = [
        (&b"MODULOS "[..], modulos, TINTA),
        (b"CABLES ", g.edges().len(), TINTA),
        (b"3060 ", gpu, if gpu > 0 { ORO } else { TENUE }),
        (b"FALLOS ", fallan.len(), if fallan.is_empty() { TENUE } else { NOGO }),
    ];
    let titulo: &[u8] = b"PLANO DE ENSAMBLAJE  //  F1";
    let gw = bmo::GLIFO_ANCHO as i32;
    let mut b = [0u8; 8];
    let largo: i32 = lecturas.iter().map(|(r, v, _)| (r.len() + num(*v, &mut b).len()) as i32 * gw + 20).sum();
    let w = 24 + titulo.len() as i32 * gw + 28 + largo;
    let (x, y, h) = ((c.w - w - 56).max(LEFT + 260), TOP + 8, 28);
    c.rect(x, y, w, h, BORDE);
    c.rect(x + 1, y + 1, w - 2, h - 2, FONDO);
    esquinas(c, (x, y, w, h), 0, OJO);
    let ty = y + (h - bmo::GLIFO_ALTO as i32) / 2;
    let mut cx = x + 12 + c.text(x + 12, ty, titulo, OJO, 1) + 28;
    for (rotulo, v, tinta) in lecturas {
        cx += c.text(cx, ty, rotulo, TENUE, 1);
        cx += c.text(cx, ty, num(v, &mut b), tinta, 1) + 20;
    }
}
