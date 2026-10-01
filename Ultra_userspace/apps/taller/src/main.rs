//! **TALLER** -- F1, the workshop, as its own app (`sys/taller.bex`).
//!
//! [consumo] LATIDO    while something moves (an animation, a drag) it wakes
//!                     every 16 ms; the white pulse on the cables every 33 ms,
//!                     and ONLY while the window is seen and someone touched it
//!                     in the last 20 s; when still, every 100 ms to read the
//!                     mailbox and ESTRATOS's generation (one number); it
//!                     reads files only when that number moved, and it does
//!                     not draw at all if nobody sees it (the VIEW byte, R-APP8)
//!
//! `docs/plan/PLAN_TALLER.md` section 8 and `docs/maestro/TITAN_MAESTRO.md`:
//! F1 is a node editor for TITAN++, where the manifest is the main node,
//! EVERYTHING is a node, and the borrow checker is ANIMATED (the owner,
//! 29-09). This is B1-B3 of that base, and L1 on top:
//!
//! ```text
//!    B1  its own .bex: the DIRECTOR launches it on F1 and composes its window
//!    B2  the canvas: drag a node or the canvas, zoom with + and -
//!    B3  the checker's events played on it: the `mut` loan that travels and
//!        returns, the `take` that moves, the conflict and the permission NO
//!    L1  the graph READ from a package in ESTRATOS (`store.rs`, following
//!        `mod`), listed on the left (`explorer.rs`), and live: a commit from
//!        anywhere -- a `renombra` in F12, a `vuelve` -- is on screen in a beat
//!    L2  drag a file of the EXPLORER onto another file (or its node): it
//!        hangs there. Two headers are rewritten; the file does not move
//!    L3  the look of TITAN++ (`art.rs`): the logo opens the workshop, and
//!        the graph lives in a starry sky of the logo's colours
//! ```
//!
//! The checker's events still come from `bmo-titan-contrato::sample`, and only
//! for a graph that has the modules they name: a package without them shows NO
//! events rather than invented ones (T0 waits for the owner's grammar). This
//! file knows nothing about borrowing: it shows what the contract says.

#![no_std]
#![no_main]

mod art;
mod canvas;
mod explorer;
mod faults;
mod player;
mod store;
mod view;
mod window;

use bmo_titan_contrato::{sample, Graph, NodeId, Script};
use bmo_userland as bmo;
use canvas::Canvas;
use explorer::Click;
use player::Player;
use store::{Origin, Store};
use view::{Camera, Scene, LEVELS, NODE_H, NODE_W};
use window::{Input, Window};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 760;

/// Left mouse button, in the event's and the pointer's button byte.
const BUTTON: u8 = 1;

/// The pulse on the cables stops this long after the last touch: at rest, F1
/// goes back to sleeping (the house's rule -- if it does nothing, it spends
/// nothing).
const FLOW_REST_MS: u32 = 20_000;

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
    /// A file of the EXPLORER, grabbed at this screen point.
    File(NodeId, i32, i32),
}

/// Far enough from where the button went down to be a drag and not a click:
/// a sloppy click must never rewrite a file.
fn dragged(x0: i32, y0: i32, x: i32, y: i32) -> bool {
    (x - x0).abs() + (y - y0).abs() > 4
}

/// A file let go at (x, y): it hangs under the file or node there, if any.
fn drop_file(store: &mut Store, cam: &Camera, grab: Drag, x: i32, y: i32) -> bool {
    let Drag::File(id, x0, y0) = grab else { return false };
    if !dragged(x0, y0, x, y) {
        return false;
    }
    match explorer::drop_target(store, cam, x, y) {
        Some(t) if t != id => {
            store.hang(id, t);
            true
        }
        _ => false,
    }
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

/// What the canvas plays over the package's graph. Rebuilt whole on every
/// read: node ids of the old graph mean nothing in the new one.
struct Shown {
    script: Option<Script>,
    player: Player,
    selected: Option<NodeId>,
}

impl Shown {
    fn of(g: &Graph) -> Shown {
        Shown { script: sample::script_for(g), player: Player::new(), selected: None }
    }
}

/// The camera that puts node `id` in the middle of the canvas, at `zoom`.
fn look_at(g: &Graph, id: NodeId, zoom: i32) -> Option<Camera> {
    let n = g.node(id)?;
    Some(Camera::centered(zoom, n.x + NODE_W / 2, n.y + NODE_H / 2, WIDTH as i32, HEIGHT as i32))
}

fn fit(g: &Graph) -> Camera {
    Camera::fit(g, WIDTH as i32, HEIGHT as i32)
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let Some(mut win) = Window::open(WIDTH, HEIGHT) else {
        say("TALLER: NO -- sin ventana (no hay memoria, o nadie me lanzo)\n");
        bmo::salir();
    };
    // The art, made once: the logo decoded into a borrowed block, and the sky
    // built into another. Without the memory for either, F1 is the same
    // workshop in a plain night colour -- decoration never stops it.
    let logo_block = art::size(art::TITAN).and_then(|(w, h)| Some((bmo::Memoria::request((w * h) as u64)?, w * h)));
    let logo = logo_block.as_ref().and_then(|(block, n)| {
        // SAFETY: the block is ours, mapped and `n` bytes long, and it lives
        // as long as `_start` (it is never dropped: `_start` does not return).
        let out = unsafe { core::slice::from_raw_parts_mut(block.base(), *n) };
        art::decode(art::TITAN, out)
    });
    let pixels = (WIDTH * HEIGHT) as usize;
    let sky_block = bmo::Memoria::request(pixels as u64 * 4);
    let sky: Option<&[u32]> = sky_block.as_ref().map(|block| {
        // SAFETY: as above, `pixels` u32 of our own block, for all of `_start`.
        let px = unsafe { core::slice::from_raw_parts_mut(block.base() as *mut u32, pixels) };
        let area = (view::LEFT, view::TOP, WIDTH as i32 - view::LEFT, HEIGHT as i32 - view::TOP - view::PANEL);
        art::backdrop(px, &art::Sky { w: WIDTH as i32, h: HEIGHT as i32, area });
        &*px
    });
    let mut store = Store::open();
    let mut shown = Shown::of(&store.loaded.graph);
    let mut canvas = Canvas::new(win.px, win.w, win.h);
    let mut cam = fit(&store.loaded.graph);
    let mut drag = Drag::None;
    let clock = Clock { hz: bmo::info(bmo::INFO_TSC_HZ) };
    let mut last = clock.now_ms();
    let opened = last;
    let mut touched = last;
    // The splash runs until its time is up or the owner touches anything.
    let mut splash = logo.is_some();
    let mut dirty = true;
    let mut last_view = u8::MAX;
    say(match store.origin {
        Origin::Estratos(_) => "TALLER: F1 abierto -- la biblioteca de ESTRATOS, en vivo\n",
        Origin::Memory(_) => "TALLER: F1 abierto -- sin ESTRATOS, el ejemplo en memoria\n",
    });

    loop {
        let now = clock.now_ms();
        let dt = now.wrapping_sub(last).min(250);
        last = now;

        // REAL TIME: one number per beat, and the files only when it moved.
        // The selection survives by NAME: node ids of the old graph mean
        // nothing in the new one, names do.
        let keep = shown.selected.and_then(|id| store.loaded.graph.node(id)).map(|n| n.name);
        if store.refresh() {
            shown = Shown::of(&store.loaded.graph);
            shown.selected = keep.and_then(|n| store.loaded.graph.find(n.as_bytes()));
            drag = Drag::None;
            dirty = true;
        }

        let veil = if splash { art::splash_at(now.wrapping_sub(opened)) } else { None };
        splash = veil.is_some();
        dirty |= splash;

        while let Some(input) = win.next() {
            touched = now;
            if splash {
                // The first touch only ends the splash: it must not also pick
                // a node the owner has not seen yet.
                splash = false;
                dirty = true;
                continue;
            }
            match input {
                Input::Mouse { x, y, buttons, down: true } if buttons & BUTTON != 0 => {
                    if x < view::LEFT {
                        // The EXPLORER: a file can be dragged, nothing else.
                        drag = Drag::None;
                        match explorer::click(&store, x, y) {
                            Some(Click::Package(i)) => {
                                store.choose(i);
                                shown = Shown::of(&store.loaded.graph);
                                cam = fit(&store.loaded.graph);
                            }
                            Some(Click::File(id)) => {
                                shown.selected = Some(id);
                                cam = look_at(&store.loaded.graph, id, cam.zoom).unwrap_or(cam);
                                drag = Drag::File(id, x, y);
                            }
                            None => continue,
                        }
                        dirty = true;
                        continue;
                    }
                    let g = &store.loaded.graph;
                    drag = match view::hit(g, &cam, x, y) {
                        Some(id) => {
                            // Picking a node in the canvas lights its file on the left.
                            shown.selected = Some(id);
                            dirty = true;
                            let (wx, wy) = cam.to_world(x, y);
                            let n = g.node(id).map(|n| (n.x, n.y)).unwrap_or((wx, wy));
                            Drag::Node(id, wx - n.0, wy - n.1)
                        }
                        None => Drag::Canvas(x, y, cam),
                    };
                }
                Input::Mouse { x, y, down: false, .. } => {
                    dirty |= drop_file(&mut store, &cam, drag, x, y) || matches!(drag, Drag::File(..));
                    drag = Drag::None;
                }
                Input::Mouse { .. } => {}
                Input::Char(b'e' | b'E') => {
                    // The next fault of the path: select it and go there.
                    let current = shown.script.as_ref().and_then(|s| s.events().get(shown.player.index)).map(|e| e.kind);
                    let marks = faults::collect(&store.loaded, current);
                    if let Some(id) = marks.next_after(shown.selected) {
                        shown.selected = Some(id);
                        cam = look_at(&store.loaded.graph, id, cam.zoom).unwrap_or(cam);
                        dirty = true;
                    }
                }
                // TALLER keeps its size: the DIRECTOR centers it.
                Input::Resize { .. } => {}
                Input::Char(c) => {
                    dirty |= key(c, &mut shown, &mut cam, &store.loaded.graph);
                }
            }
        }

        let ptr = win.pointer();
        if ptr.buttons & BUTTON == 0 {
            // The release event never came: let go where the pointer is (if
            // it is still ours), or nowhere.
            if ptr.inside {
                dirty |= drop_file(&mut store, &cam, drag, ptr.x, ptr.y);
            }
            dirty |= matches!(drag, Drag::File(..));
            drag = Drag::None;
        }
        if ptr.inside {
            match drag {
                Drag::Node(id, ox, oy) => {
                    let (wx, wy) = cam.to_world(ptr.x, ptr.y);
                    if let Some(n) = store.loaded.graph.node_mut(id) {
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
                // The ghost follows the pointer.
                Drag::File(..) => dirty = true,
                Drag::None => {}
            }
        }
        if let Some(s) = &shown.script {
            dirty |= shown.player.advance(dt, s);
        }
        // Coming back into view needs a fresh frame even if nothing moved.
        if ptr.view != last_view {
            last_view = ptr.view;
            dirty = true;
        }
        let seen = ptr.view as u64 == bmo::SUP_VISTA_SE_VE;
        let flowing = seen && !splash && now.wrapping_sub(touched) < FLOW_REST_MS;
        dirty |= flowing;
        if dirty && seen {
            let name = store.packages().get(store.chosen).map(|p| p.0.as_bytes());
            let current = shown.script.as_ref().and_then(|s| s.events().get(shown.player.index)).map(|e| e.kind);
            let marks = faults::collect(&store.loaded, current);
            let scene = Scene {
                graph: &store.loaded.graph,
                script: shown.script.as_ref(),
                player: &shown.player,
                cam: &cam,
                now_ms: now,
                selected: shown.selected,
                origin: name.unwrap_or(b"ejemplo en memoria"),
                sky,
                flow_ms: flowing.then_some(now),
                faults: &marks,
            };
            let covered = splash && veil == Some(1000);
            if !covered {
                view::draw(&mut canvas, &scene);
                explorer::draw(&mut canvas, &store, shown.selected);
                if let Drag::File(id, x0, y0) = drag {
                    if ptr.inside && dragged(x0, y0, ptr.x, ptr.y) {
                        explorer::draw_drag(&mut canvas, &store, &cam, id, ptr.x, ptr.y);
                    }
                }
            }
            if let (true, Some(a), Some(l)) = (splash, veil, logo.as_ref()) {
                art::splash(&mut canvas, l, a);
            }
            win.present();
            dirty = false;
        }
        let moving = splash || shown.player.playing || !matches!(drag, Drag::None);
        let nap_ms: u64 = if moving && seen {
            16
        } else if flowing {
            33
        } else {
            100
        };
        bmo::wait(0, 0, nap_ms * 1_000_000);
    }
}

/// A letter. `true` if the frame must be drawn again. Without events, the
/// player's keys do nothing -- and say nothing, there is nothing to play.
fn key(c: u8, shown: &mut Shown, cam: &mut Camera, g: &Graph) -> bool {
    let p = &mut shown.player;
    match (c, &shown.script) {
        (b' ', Some(s)) => {
            if p.finished(s) {
                p.restart();
            } else {
                p.playing = !p.playing;
            }
        }
        (b'n' | b'N', Some(s)) => {
            p.playing = false;
            p.step(s);
        }
        (b'r' | b'R', Some(_)) => p.restart(),
        (b'+' | b'=' | b'-', _) => {
            let i = LEVELS.iter().position(|&z| z == cam.zoom).unwrap_or(2);
            let j = if c == b'-' { i.saturating_sub(1) } else { (i + 1).min(LEVELS.len() - 1) };
            // Keep the center of the canvas where it is.
            let (mx, my) = view::canvas_center(WIDTH as i32, HEIGHT as i32);
            let (cx, cy) = cam.to_world(mx, my);
            *cam = Camera::centered(LEVELS[j], cx, cy, WIDTH as i32, HEIGHT as i32);
        }
        (b'0', _) => *cam = fit(g),
        (0x1B, _) => {
            say("TALLER: cerrado con Esc\n");
            bmo::salir();
        }
        _ => return false,
    }
    true
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    say("TALLER: panico\n");
    if let Some(s) = info.message().as_str() {
        say(s);
        say("\n");
    }
    bmo::salir();
}
