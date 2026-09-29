//! **ESTRUCTURA** -- F1, the workshop, as its own app (`sys/taller.bex`).
//!
//! [consumo] LATIDO    while something moves (an animation, a drag) it wakes
//!                     every 16 ms; when still, every 100 ms to read the
//!                     mailbox; and it does not draw at all if nobody sees it
//!                     (the VIEW byte, R-APP8)
//!
//! `docs/plan/PLAN_ESTRUCTURA.md` section 8 and `docs/maestro/TITAN_MAESTRO.md`:
//! F1 is a node editor for TITAN++, where the manifest is the main node,
//! EVERYTHING is a node, and the borrow checker is ANIMATED (the owner,
//! 29-09). This is B1-B3 of that base:
//!
//! ```text
//!    B1  its own .bex: the DIRECTOR launches it on F1 and composes its window
//!    B2  the canvas: the graph of `asteroids`, drag a node or the canvas,
//!        zoom with + and -
//!    B3  the checker's events played on it: the `mut` loan that travels and
//!        returns, the `take` that moves, the conflict and the permission NO
//! ```
//!
//! The graph and the script come from `bmo-titan-contrato::sample` until the
//! real front exists (T0 waits for the owner's grammar). This file knows
//! nothing about borrowing: it shows what the contract says.

#![no_std]
#![no_main]

mod canvas;
mod player;
mod view;
mod window;

use bmo_titan_contrato::{sample, NodeId};
use bmo_userland as bmo;
use canvas::Canvas;
use player::Player;
use view::{Camera, LEVELS};
use window::{Input, Window};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 760;

/// Left mouse button, in the event's and the pointer's button byte.
const LEFT: u8 = 1;

fn say(s: &str) {
    bmo::consola(s);
}

/// What the left button is holding.
#[derive(Clone, Copy)]
enum Drag {
    None,
    /// A node, grabbed at this offset from its corner (world pixels).
    Node(NodeId, i32, i32),
    /// The canvas itself, grabbed at this screen point with this camera.
    Canvas(i32, i32, Camera),
}

/// Milliseconds since boot, from the TSC.
struct Clock {
    hz: u64,
}

impl Clock {
    fn now_ms(&self) -> u32 {
        ((bmo::ciclos() as u128 * 1000 / self.hz.max(1) as u128) & 0xFFFF_FFFF) as u32
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let (mut graph, script) = sample::asteroids();
    // The contract's own judge, before a single pixel: F1 does not animate a
    // script that lends what nobody has or walks a `use` that is not there.
    if script.check(&graph).is_err() {
        say("ESTRUCTURA: NO -- el guion del ejemplo no cuadra con su grafo\n");
        bmo::salir();
    }
    let Some(win) = Window::open(WIDTH, HEIGHT) else {
        say("ESTRUCTURA: NO -- sin ventana (no hay memoria, o nadie me lanzo)\n");
        bmo::salir();
    };
    let mut canvas = Canvas::new(win.px, win.w, win.h);
    let mut cam = Camera::fit(&graph, WIDTH as i32, HEIGHT as i32);
    let mut player = Player::new();
    let mut drag = Drag::None;
    let clock = Clock { hz: bmo::info(bmo::INFO_TSC_HZ) };
    let mut last = clock.now_ms();
    let mut dirty = true;
    let mut last_view = u8::MAX;
    say("ESTRUCTURA: F1 abierto -- asteroids (ejemplo) con el comprobador animado\n");

    loop {
        let now = clock.now_ms();
        let dt = now.wrapping_sub(last).min(250);
        last = now;

        while let Some(input) = win.next() {
            match input {
                Input::Mouse { x, y, buttons, down } => {
                    if down && buttons & LEFT != 0 {
                        drag = match view::hit(&graph, &cam, x, y) {
                            Some(id) => {
                                let (wx, wy) = cam.to_world(x, y);
                                let n = graph.node(id).map(|n| (n.x, n.y)).unwrap_or((wx, wy));
                                Drag::Node(id, wx - n.0, wy - n.1)
                            }
                            None => Drag::Canvas(x, y, cam),
                        };
                    } else if !down {
                        drag = Drag::None;
                    }
                }
                Input::Char(c) => {
                    dirty |= key(c, &mut player, &mut cam, &graph, &script);
                }
            }
        }

        let ptr = win.pointer();
        if ptr.buttons & LEFT == 0 {
            // Released outside the window: the release event never came.
            drag = Drag::None;
        }
        if ptr.inside {
            match drag {
                Drag::Node(id, ox, oy) => {
                    let (wx, wy) = cam.to_world(ptr.x, ptr.y);
                    if let Some(n) = graph.node_mut(id) {
                        n.x = wx - ox;
                        n.y = wy - oy;
                    }
                    dirty = true;
                }
                Drag::Canvas(sx, sy, from) => {
                    cam.x = from.x - (ptr.x - sx) * 1000 / from.zoom;
                    cam.y = from.y - (ptr.y - sy) * 1000 / from.zoom;
                    dirty = true;
                }
                Drag::None => {}
            }
        }
        if player.advance(dt, &script) {
            dirty = true;
        }
        // Coming back into view needs a fresh frame even if nothing moved.
        if ptr.view != last_view {
            last_view = ptr.view;
            dirty = true;
        }
        let seen = ptr.view as u64 == bmo::SUP_VISTA_SE_VE;
        if dirty && seen {
            view::draw(&mut canvas, &graph, &script, &player, &cam, now);
            win.present();
            dirty = false;
        }
        let moving = player.playing || !matches!(drag, Drag::None);
        let nap_ms: u64 = if moving && seen { 16 } else { 100 };
        bmo::wait(0, 0, nap_ms * 1_000_000);
    }
}

/// A letter. `true` if the frame must be drawn again.
fn key(c: u8, p: &mut Player, cam: &mut Camera, g: &bmo_titan_contrato::Graph, s: &bmo_titan_contrato::Script) -> bool {
    match c {
        b' ' => {
            if p.finished(s) {
                p.restart();
            } else {
                p.playing = !p.playing;
            }
        }
        b'n' | b'N' => {
            p.playing = false;
            p.step(s);
        }
        b'r' | b'R' => p.restart(),
        b'+' | b'=' | b'-' => {
            let i = LEVELS.iter().position(|&z| z == cam.zoom).unwrap_or(2);
            let j = if c == b'-' { i.saturating_sub(1) } else { (i + 1).min(LEVELS.len() - 1) };
            // Keep the center of the canvas where it is.
            let (cx, cy) = cam.to_world(WIDTH as i32 / 2, view::TOP + (HEIGHT as i32 - view::TOP - view::PANEL) / 2);
            *cam = Camera::centered(LEVELS[j], cx, cy, WIDTH as i32, HEIGHT as i32);
        }
        b'0' => *cam = Camera::fit(g, WIDTH as i32, HEIGHT as i32),
        0x1B => {
            say("ESTRUCTURA: cerrado con Esc\n");
            bmo::salir();
        }
        _ => return false,
    }
    true
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    say("ESTRUCTURA: panico\n");
    if let Some(s) = info.message().as_str() {
        say(s);
        say("\n");
    }
    bmo::salir();
}
