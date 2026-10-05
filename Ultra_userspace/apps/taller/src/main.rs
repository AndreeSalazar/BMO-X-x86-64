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
//!    L5  the EXPLORER organizes the DISK, VS Code style: every folder and
//!        file of the package, in the owner's order (not A to Z), folds, new
//!        file / folder, rename, remove, move -- each `.titan` carrying its
//!        module along (`titan-lector::organize`, `PLAN_TALLER` 8.10)
//! ```
//!
//! The checker's events still come from `bmo-titan-contrato::sample`, and only
//! for a graph that has the modules they name: a package without them shows NO
//! events rather than invented ones (T0 waits for the owner's grammar). This
//! file knows nothing about borrowing: it shows what the contract says.

#![no_std]
#![no_main]

mod art;
mod aspecto;
mod astros;
mod canvas;
mod editor;
mod explorer;
mod faults;
mod guia;
mod guia_estratos_gen;
mod iconos;
mod player;
mod space;
mod store;
mod strata;
mod strata_guide;
mod tab;
mod tema_gen;
mod view;
mod window;

use bmo_titan_contrato::{sample, Graph, NodeId, Script};
use bmo_userland as bmo;
use canvas::Canvas;
use explorer::{Click, Drop, Edit, EditKind, Entry, Menu, Ui, Zone};
use player::Player;
use space::Tab;
use store::{Origin, Store};
use view::{Camera, Scene, LEVELS, NODE_H, NODE_W};
use window::{Input, Window};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 760;

/// Left mouse button, in the event's and the pointer's button byte.
const BUTTON: u8 = 1;
/// The right one: the EXPLORER's menu.
const RIGHT_BUTTON: u8 = 2;
/// Two clicks on the same row within this long: rename it (VS Code).
const DOUBLE_MS: u32 = 450;
/// A first Supr waits this long for the second one.
const CONFIRM_MS: u32 = 3_000;

// The cooked codes of the kernel's keyboard map (`<bmo/entrada.h>`).
const KEY_UP: u8 = 0x80;
const KEY_DOWN: u8 = 0x81;
const KEY_LEFT: u8 = 0x82;
const KEY_RIGHT: u8 = 0x83;
const KEY_SUPR: u8 = 0x86;
const KEY_F2: u8 = 0x8A;

/// The editor's block: the biggest file of a node it opens (`editor.rs`).
const EDIT_CAP: usize = 16 * 1024;

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
    /// A declared file of the EXPLORER (no disk tree), grabbed at this point.
    File(NodeId, i32, i32),
    /// An item of the disk tree, grabbed at this screen point.
    Item(usize, i32, i32),
    /// The 3D sky, grabbed at this x with this turn: dragging turns it.
    Turn(i32, u32),
    /// A cable pulled from this node's OUT pin (UE5 style): let go on a node,
    /// it is a `use` written.
    Wire(NodeId),
}

/// Far enough from where the button went down to be a drag and not a click:
/// a sloppy click must never rewrite a file.
fn dragged(x0: i32, y0: i32, x: i32, y: i32) -> bool {
    (x - x0).abs() + (y - y0).abs() > 4
}

/// Something of the EXPLORER let go at (x, y). A declared file (no disk
/// tree) hangs under the file or node there; a disk item is placed, moved,
/// declared or hung, by `explorer::drop_at`.
fn drop_file(store: &mut Store, ui: &Ui, cam: &Camera, grab: Drag, x: i32, y: i32) -> bool {
    match grab {
        // A cable let go on a node: the `use` is written (or the note says
        // why not); let go on nothing, it just vanishes.
        Drag::Wire(from) => match view::hit(&store.loaded.graph, cam, x, y) {
            Some(to) => {
                store.wire(from, to);
                true
            }
            None => true,
        },
        Drag::File(id, x0, y0) if dragged(x0, y0, x, y) => match explorer::drop_target(store, ui, cam, x, y) {
            Some(t) if t != id => {
                store.hang(id, t);
                true
            }
            _ => false,
        },
        Drag::Item(i, x0, y0) if dragged(x0, y0, x, y) => {
            match explorer::drop_at(store, ui, cam, i, x, y) {
                Some(Drop::Before(t)) => store.place(i, t, false),
                Some(Drop::After(t)) => store.place(i, t, true),
                Some(Drop::Into(f)) => store.move_into(i, f),
                Some(Drop::Node(n)) => match explorer::node_of(store, i) {
                    Some(child) if child == n => return false,
                    Some(child) => store.hang(child, n),
                    None => store.declare(i, n),
                },
                None => return false,
            }
            true
        }
        _ => false,
    }
}

/// Starts typing a new name in the folder of the pick (VS Code: the folder
/// itself if a folder is picked). A folded folder opens to show the box --
/// on screen only: what is folded on disk is the owner's, not the box's.
fn begin_new(store: &mut Store, ui: &mut Ui, folder_kind: bool) {
    let Some(t) = store.tree.as_deref_mut() else { return };
    let folder = t.folder_for(ui.picked);
    if let Some(f) = folder.filter(|&f| t.is_folded(f)) {
        t.toggle(f);
    }
    let kind = if folder_kind { EditKind::NewFolder(folder) } else { EditKind::NewFile(folder) };
    ui.edit = Some(Edit::new(kind, b""));
    ui.menu = None;
}

/// Starts renaming `i`, with its name in the box.
fn begin_rename(store: &Store, ui: &mut Ui, i: usize) {
    if let Some(t) = store.tree.as_deref() {
        ui.edit = Some(Edit::new(EditKind::Rename(i), t.name(i)));
        ui.menu = None;
    }
}

/// The first Supr asks; the second, within `CONFIRM_MS`, removes.
fn ask_remove(store: &mut Store, ui: &mut Ui, i: usize, now: u32) {
    match ui.confirm {
        Some((j, until)) if j == i && now.wrapping_sub(until) > u32::MAX / 2 => {
            ui.confirm = None;
            store.remove(i);
        }
        _ => {
            ui.confirm = Some((i, now.wrapping_add(CONFIRM_MS)));
            let name = store.tree.as_deref().map(|t| bmo_titan_lector::Say::new().t(b"Supr otra vez para quitar ").t(t.name(i)).done());
            if let Some(line) = name {
                store.say(line, Some(bmo_titan_contrato::Line::new("no se pierde: vuelve en F12 lo trae")), false);
            }
        }
    }
}

/// What a key does while a name is being typed. Esc lets it go; Enter makes
/// it; nothing typed here reaches the canvas.
fn typing(c: u8, store: &mut Store, ui: &mut Ui) {
    let Some(mut e) = ui.edit else { return };
    match c {
        0x1B => {
            ui.edit = None;
            return;
        }
        0x08 | 0x7F => e.len = e.len.saturating_sub(1),
        b'\r' | b'\n' => {
            ui.edit = None;
            let name = e.get();
            match e.kind {
                EditKind::NewFile(folder) => {
                    // Picked a module? The new `.titan` is born declared by it.
                    let declarer = ui.picked.and_then(|p| explorer::node_of(store, p));
                    store.create(folder, name, false, declarer);
                }
                EditKind::NewFolder(folder) => store.create(folder, name, true, None),
                EditKind::Rename(i) => store.rename(i, name),
            }
            return;
        }
        0x20..=0x7E => e.push(c),
        _ => {}
    }
    ui.edit = Some(e);
}

/// An entry of the right button's menu, on the item it was opened on. `Some`:
/// the item whose code to open in the editor (`main` owns the editor).
fn menu_entry(e: Entry, item: Option<usize>, store: &mut Store, ui: &mut Ui, now: u32) -> Option<usize> {
    ui.picked = item.or(ui.picked);
    match (e, item) {
        (Entry::Edit, Some(i)) => return Some(i),
        (Entry::NewFile, _) => begin_new(store, ui, false),
        (Entry::NewFolder, _) => begin_new(store, ui, true),
        (Entry::Rename, Some(i)) => begin_rename(store, ui, i),
        (Entry::Remove, Some(i)) => ask_remove(store, ui, i, now),
        _ => {}
    }
    None
}

/// **The code of a node, in the editor** (`editor.rs`): the file `rel` of the
/// package, read into the editor's block (asked once, kept: F1 opens it
/// again and again). The one open before is saved first.
fn open_code(store: &mut Store, mem: &mut Option<bmo::Memoria>, open: &mut Option<editor::Editor>, rel: bmo_titan_lector::Path) {
    close_code(store, mem, open);
    if mem.is_none() {
        *mem = bmo::Memoria::request(EDIT_CAP as u64);
    }
    let Some(m) = mem.as_ref() else {
        store.say(bmo_titan_contrato::Line::new("no hay memoria para el editor"), None, false);
        return;
    };
    let file = store.loaded.files().iter().find(|f| f.path.as_bytes() == rel.as_bytes());
    let name = file.and_then(|f| store.loaded.graph.node(f.node)).map(|n| n.name).unwrap_or(bmo_titan_contrato::Text::new("?"));
    match store.read_text(rel.as_bytes(), m, EDIT_CAP) {
        Some(n) => *open = Some(editor::Editor::open(rel, name, n)),
        None => store.say(bmo_titan_contrato::Line::new("no pude leerlo en ESTRATOS, o no cabe en el editor"), None, false),
    }
}

/// Saves what the editor has (if anything changed) and closes it.
fn close_code(store: &mut Store, mem: &Option<bmo::Memoria>, open: &mut Option<editor::Editor>) {
    if let (Some(e), Some(m)) = (open.as_mut(), mem.as_ref()) {
        if e.dirty && store.save_text(e.rel.as_bytes(), m, e.len) {
            e.saved();
        }
    }
    *open = None;
}

/// The code of disk item `i`, if a node comes from it.
fn code_of(store: &Store, i: usize) -> Option<bmo_titan_lector::Path> {
    store.tree.as_deref().and_then(|t| t.path_of(i)).filter(|_| explorer::has_code(store, i))
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
    let Some(mut store) = Store::open() else {
        say("TALLER: NO -- sin memoria para la tienda del paquete\n");
        bmo::salir();
    };
    let mut shown = Shown::of(&store.loaded.graph);
    // The ESTRATOS tab (`strata.rs`): the history, in a block of its own, and
    // the generation it was read at.
    let mut history = store::history_block();
    let mut history_gen = u64::MAX;
    // The door picked in ESTRATOS's GUIDE (`strata_guide.rs`).
    let mut door = 0usize;
    let mut canvas = Canvas::new(win.px, win.w, win.h);
    let mut cam = fit(&store.loaded.graph);
    let mut drag = Drag::None;
    let mut ui = Ui::new();
    // GRAFO or ESPACIO: the same nodes, two ways of seeing them (`space.rs`).
    let mut tab = Tab::Graph;
    // How far the 3D sky has turned (1024ths): it turns by itself while F1 is
    // lively, and by hand when the sky is dragged.
    let mut turn: u32 = 96;
    // The last click on a disk row: (when, which), for the double click.
    let mut last_click: (u32, Option<usize>) = (0, None);
    // The TAB of master nodes, while it is open (`tab.rs`, PLAN_TALLER 8.15),
    // and the node it just placed: picked when the next beat reads it back.
    let mut palette: Option<tab::Palette> = None;
    let mut placed: Option<bmo_titan_contrato::Name> = None;
    // The code of a node, open in the editor, and the block it lives in.
    let mut code: Option<editor::Editor> = None;
    let mut code_mem: Option<bmo::Memoria> = None;
    // The last click on a node of the GRAPH: (when, which), for the double click.
    let mut last_node: (u32, Option<NodeId>) = (0, None);
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
        // The pick of the disk tree survives by PATH, for the same reason.
        let keep_item = ui.picked.and_then(|i| store.tree.as_deref().and_then(|t| t.path_of(i)));
        if store.refresh() {
            shown = Shown::of(&store.loaded.graph);
            shown.selected = keep.and_then(|n| store.loaded.graph.find(n.as_bytes()));
            if let Some(n) = placed.take() {
                shown.selected = store.loaded.graph.find(n.as_bytes()).or(shown.selected);
            }
            ui.picked = keep_item.and_then(|p| store.tree.as_deref().and_then(|t| t.find(p.as_bytes())));
            // Indices of the old tree mean nothing in the new one.
            if ui.edit.take().is_some() {
                store.say(bmo_titan_contrato::Line::new("ESTRATOS cambio mientras escribias: no se guardo"), None, false);
            }
            ui.menu = None;
            ui.confirm = None;
            drag = Drag::None;
            dirty = true;
        }
        // The ESTRATOS tab reads the history when it opens and when the
        // generation moved -- never per frame: `hist_releer` reads the disk.
        if tab == Tab::Strata {
            if let Some(h) = history.as_deref_mut() {
                let g = store::generation();
                if g != history_gen {
                    store::read_history(h);
                    history_gen = g;
                    dirty = true;
                }
                dirty |= h.confirm.is_some();
            }
        }
        if let Some((_, until)) = ui.confirm {
            if now.wrapping_sub(until) < u32::MAX / 2 {
                ui.confirm = None;
                store.note = None;
            }
        }
        // The caret blinks and a doomed row flashes: draw while they are there.
        dirty |= ui.edit.is_some() || ui.confirm.is_some() || code.is_some();

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
                // The editor takes every key while it is open: Esc saves and
                // closes it (and does not close F1).
                Input::Char(c) if code.is_some() => {
                    let mut close = false;
                    if let (Some(e), Some(m)) = (code.as_mut(), code_mem.as_ref()) {
                        close = matches!(e.key(store::text_block(m, EDIT_CAP), c, now), editor::Act::Close);
                    }
                    if close {
                        close_code(&mut store, &code_mem, &mut code);
                    }
                    dirty = true;
                }
                // The TAB is on top of everything while it is open: every key
                // is its own (Esc closes IT, not F1), and a click outside it
                // only closes it.
                Input::Char(c) if palette.is_some() => {
                    if let Some(p) = palette.as_mut() {
                        match p.key(c, KEY_UP, KEY_DOWN) {
                            tab::Act::Stay => {}
                            tab::Act::Close => palette = None,
                            tab::Act::Place(i) => {
                                let (x, y) = p.at;
                                placed = store.place_master(i, x, y);
                                palette = None;
                            }
                        }
                    }
                    dirty = true;
                }
                Input::Mouse { x, y, buttons, down: true } if palette.is_some() => {
                    let stays = buttons & BUTTON != 0 && palette.as_mut().is_some_and(|p| p.click(WIDTH as i32, HEIGHT as i32, x, y));
                    if !stays {
                        palette = None;
                    }
                    dirty = true;
                }
                Input::Mouse { .. } if palette.is_some() => {}
                // The menu is on top of everything: a click is ITS click, and
                // anywhere else only closes it.
                Input::Mouse { down: true, .. } if ui.menu.is_some() => {
                    if let (Some(m), Input::Mouse { x, y, buttons, .. }) = (ui.menu.take(), input) {
                        if buttons & BUTTON != 0 {
                            if let Some(e) = m.hit(x, y) {
                                if let Some(rel) = menu_entry(e, m.item, &mut store, &mut ui, now).and_then(|i| code_of(&store, i)) {
                                    open_code(&mut store, &mut code_mem, &mut code, rel);
                                }
                            }
                        }
                    }
                    dirty = true;
                }
                Input::Mouse { x, y, buttons, down: true } if buttons & RIGHT_BUTTON != 0 && x < view::LEFT => {
                    let item = match explorer::click(&store, &ui, x, y) {
                        Some(Click::Item(i, _)) => Some(i),
                        _ => None,
                    };
                    if store.tree.as_deref().is_some_and(|t| !t.is_empty()) {
                        ui.picked = item.or(ui.picked);
                        let code = item.is_some_and(|i| explorer::has_code(&store, i));
                        ui.menu = Some(Menu { x: (x + 2).min(view::LEFT - 40), y, item, code });
                        dirty = true;
                    }
                }
                // The right button on a NODE of the GRAPH: the same menu as its
                // file in the EXPLORER, with `Editar codigo` first.
                Input::Mouse { x, y, buttons, down: true } if buttons & RIGHT_BUTTON != 0 && tab == Tab::Graph => {
                    let (ex, ey, ew, eh) = editor::frame(WIDTH as i32, HEIGHT as i32);
                    let on_editor = code.is_some() && x >= ex && x < ex + ew && y >= ey && y < ey + eh;
                    if let Some(id) = view::hit(&store.loaded.graph, &cam, x, y).filter(|_| !on_editor) {
                        let item = explorer::item_of(&store, id);
                        shown.selected = Some(id);
                        if item.is_some() {
                            ui.picked = item;
                            let code = item.is_some_and(|i| explorer::has_code(&store, i));
                            ui.menu = Some(Menu { x: x.min(WIDTH as i32 - 200), y: y.min(HEIGHT as i32 - 140), item, code });
                        }
                        dirty = true;
                    }
                }
                Input::Mouse { x, y, buttons, down: true } if buttons & BUTTON != 0 => {
                    // A click away from the box makes the name, as VS Code does.
                    if ui.edit.is_some() && !matches!(explorer::click(&store, &ui, x, y), Some(Click::Typing)) {
                        typing(b'\r', &mut store, &mut ui);
                        dirty = true;
                    }
                    if x < view::LEFT {
                        // The EXPLORER: a row can be dragged, nothing else.
                        drag = Drag::None;
                        match explorer::click(&store, &ui, x, y) {
                            Some(Click::Package(i)) => {
                                close_code(&mut store, &code_mem, &mut code);
                                store.choose(i);
                                shown = Shown::of(&store.loaded.graph);
                                cam = fit(&store.loaded.graph);
                                ui = Ui::new();
                            }
                            Some(Click::File(id)) => {
                                shown.selected = Some(id);
                                cam = look_at(&store.loaded.graph, id, cam.zoom).unwrap_or(cam);
                                drag = Drag::File(id, x, y);
                            }
                            Some(Click::Item(i, Zone::Arrow)) => {
                                ui.picked = Some(i);
                                store.toggle(i);
                            }
                            Some(Click::Item(i, Zone::Body)) => {
                                let double = last_click.1 == Some(i) && now.wrapping_sub(last_click.0) < DOUBLE_MS;
                                last_click = (now, Some(i));
                                ui.picked = Some(i);
                                ui.confirm = None;
                                if double {
                                    begin_rename(&store, &mut ui, i);
                                } else {
                                    drag = Drag::Item(i, x, y);
                                }
                                // A module lights its node, and the canvas goes to it.
                                if let Some(id) = explorer::node_of(&store, i) {
                                    shown.selected = Some(id);
                                    cam = look_at(&store.loaded.graph, id, cam.zoom).unwrap_or(cam);
                                }
                            }
                            Some(Click::NewFile) => begin_new(&mut store, &mut ui, false),
                            Some(Click::NewFolder) => begin_new(&mut store, &mut ui, true),
                            Some(Click::Typing) => {}
                            None => continue,
                        }
                        dirty = true;
                        continue;
                    }
                    if let Some(t) = space::tab_at(tab, x, y) {
                        tab = t;
                        dirty = true;
                        continue;
                    }
                    match tab {
                        Tab::Graph => {}
                        // In the sky a click picks the nearest star, and
                        // dragging turns the sky: the nodes stay where the
                        // [layout] puts them.
                        Tab::Sky => {
                            let look = space::Look { graph: &store.loaded.graph, files: store.loaded.files(), cam: &cam, turn };
                            if let Some(id) = space::hit_sky(&look, WIDTH as i32, HEIGHT as i32, x, y) {
                                shown.selected = Some(id);
                                ui.picked = explorer::item_of(&store, id);
                            }
                            drag = Drag::Turn(x, turn);
                            dirty = true;
                            continue;
                        }
                        // The history: a click picks a version.
                        Tab::Strata => {
                            if let Some(h) = history.as_deref_mut() {
                                h.picked = strata::hit(&canvas, h, x, y).or(h.picked);
                                h.confirm = None;
                            }
                            dirty = true;
                            continue;
                        }
                        Tab::StrataGuide => {
                            door = strata_guide::hit(&canvas, x, y).unwrap_or(door);
                            dirty = true;
                            continue;
                        }
                        // The pages are to read.
                        _ => continue,
                    }
                    let g = &store.loaded.graph;
                    // An OUT pin first: pulling from it is a cable, not a move.
                    if let Some(from) = view::pin_at(g, &cam, x, y) {
                        shown.selected = Some(from);
                        drag = Drag::Wire(from);
                        dirty = true;
                        continue;
                    }
                    // A click inside the editor is the editor's.
                    if code.is_some() {
                        let (ex, ey, ew, eh) = editor::frame(WIDTH as i32, HEIGHT as i32);
                        if x >= ex && x < ex + ew && y >= ey && y < ey + eh {
                            continue;
                        }
                    }
                    let hit = view::hit(g, &cam, x, y);
                    // Two clicks on a node: its code, in the editor.
                    if let Some(id) = hit.filter(|&id| last_node.1 == Some(id) && now.wrapping_sub(last_node.0) < DOUBLE_MS) {
                        last_node = (0, None);
                        if let Some(rel) = store.loaded.file_of(id).map(|f| f.path) {
                            open_code(&mut store, &mut code_mem, &mut code, rel);
                        }
                        dirty = true;
                        continue;
                    }
                    last_node = (now, hit);
                    let g = &store.loaded.graph;
                    drag = match hit {
                        Some(id) => {
                            // Picking a node in the canvas lights its file on the left.
                            shown.selected = Some(id);
                            ui.picked = explorer::item_of(&store, id);
                            dirty = true;
                            let (wx, wy) = cam.to_world(x, y);
                            let n = g.node(id).map(|n| (n.x, n.y)).unwrap_or((wx, wy));
                            Drag::Node(id, wx - n.0, wy - n.1)
                        }
                        None => Drag::Canvas(x, y, cam),
                    };
                }
                Input::Mouse { x, y, down: false, .. } => {
                    dirty |= drop_file(&mut store, &ui, &cam, drag, x, y) || matches!(drag, Drag::File(..) | Drag::Item(..));
                    drag = Drag::None;
                }
                Input::Mouse { .. } => {}
                // While a name is typed, every key is the box's.
                Input::Char(c) if ui.edit.is_some() => {
                    typing(c, &mut store, &mut ui);
                    dirty = true;
                }
                // ESTRATOS's GUIDE: the arrows walk the doors.
                Input::Char(k @ (KEY_UP | KEY_DOWN)) if tab == Tab::StrataGuide => {
                    door = strata_guide::step(door, k == KEY_DOWN);
                    dirty = true;
                }
                // The ESTRATOS tab: the arrows walk the chain, ENTER twice
                // restores the picked version, Esc takes the question back.
                Input::Char(k @ (KEY_LEFT | KEY_RIGHT | b'\r' | b'\n' | 0x1B)) if tab == Tab::Strata => {
                    if let Some(h) = history.as_deref_mut() {
                        match k {
                            KEY_LEFT => h.step(true),
                            KEY_RIGHT => h.step(false),
                            0x1B => {
                                h.confirm = None;
                                h.said = None;
                            }
                            _ => {
                                if let Some(steps) = strata::enter(h, now) {
                                    let ok = store::restore(steps);
                                    h.said = Some(if ok {
                                        (&b"RESTABLECIDA: un estrato nuevo; lo de en medio sigue en la historia"[..], true)
                                    } else {
                                        (&b"NO se pudo restablecer: el volumen no cambio"[..], false)
                                    });
                                    if ok {
                                        h.picked = Some(0);
                                    }
                                }
                            }
                        }
                    }
                    dirty = true;
                }
                // TAB, over the GRAPH or the SKY: the master nodes (Houdini).
                // The node will go where the mouse is now, centred on it.
                Input::Char(b'\t') if matches!(tab, Tab::Graph | Tab::Sky) => {
                    let p = win.pointer();
                    let on_canvas = tab == Tab::Graph && p.inside && p.x >= view::LEFT && p.y >= view::TOP && p.y < HEIGHT as i32 - view::PANEL;
                    let (sx, sy) = if on_canvas { (p.x, p.y) } else { view::canvas_center(WIDTH as i32, HEIGHT as i32) };
                    let (wx, wy) = cam.to_world(sx, sy);
                    palette = Some(tab::Palette::open((wx - NODE_W / 2, wy - NODE_H / 2)));
                    ui.menu = None;
                    dirty = true;
                }
                Input::Char(0x1B) if ui.menu.is_some() => {
                    ui.menu = None;
                    dirty = true;
                }
                Input::Char(0x1B) if ui.confirm.is_some() => {
                    ui.confirm = None;
                    store.note = None;
                    dirty = true;
                }
                Input::Char(KEY_SUPR) => {
                    if let Some(i) = ui.picked {
                        ask_remove(&mut store, &mut ui, i, now);
                        dirty = true;
                    }
                }
                Input::Char(KEY_F2) => {
                    if let Some(i) = ui.picked {
                        begin_rename(&store, &mut ui, i);
                        dirty = true;
                    }
                }
                Input::Char(k @ (KEY_UP | KEY_DOWN)) => {
                    if let Some(i) = explorer::next_row(&store, ui.picked, k == KEY_DOWN) {
                        ui.picked = Some(i);
                        if let Some(id) = explorer::node_of(&store, i) {
                            shown.selected = Some(id);
                        }
                        dirty = true;
                    }
                }
                Input::Char(k @ (KEY_LEFT | KEY_RIGHT | b'\r' | b'\n')) if ui.picked.is_some() && store.tree.is_some() => {
                    // Left folds (or goes up to the folder), right opens, Enter flips.
                    let i = ui.picked.unwrap_or(0);
                    let (folder, folded, parent) = match store.tree.as_deref() {
                        Some(t) => (t.is_folder(i), t.is_folded(i), t.parent(i)),
                        None => (false, false, None),
                    };
                    match k {
                        KEY_LEFT if folder && !folded => store.toggle(i),
                        KEY_LEFT => ui.picked = parent.or(ui.picked),
                        KEY_RIGHT if folder && folded => store.toggle(i),
                        b'\r' | b'\n' if folder => store.toggle(i),
                        _ => {}
                    }
                    dirty = true;
                }
                Input::Char(b't' | b'T') => {
                    tab = tab.next();
                    dirty = true;
                }
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

        // The editor saves on its own once the keys stop: the node changes
        // the next beat, while the owner looks at it.
        if code.as_ref().is_some_and(|e| e.due(now)) {
            if let (Some(e), Some(m)) = (code.as_mut(), code_mem.as_ref()) {
                if store.save_text(e.rel.as_bytes(), m, e.len) {
                    e.saved();
                } else {
                    e.last_key = now;
                }
            }
            dirty = true;
        }

        let ptr = win.pointer();
        if ptr.buttons & BUTTON == 0 {
            // The release event never came: let go where the pointer is (if
            // it is still ours), or nowhere.
            if ptr.inside {
                dirty |= drop_file(&mut store, &ui, &cam, drag, ptr.x, ptr.y);
            }
            dirty |= matches!(drag, Drag::File(..) | Drag::Item(..) | Drag::Wire(..));
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
                Drag::Turn(sx, from) => {
                    turn = from.wrapping_add((ptr.x - sx) as u32 * 2);
                    dirty = true;
                }
                // The ghost follows the pointer.
                Drag::File(..) | Drag::Item(..) | Drag::Wire(..) => dirty = true,
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
        // The sky turns by itself while lively: one turn every 48 s.
        if flowing && tab == Tab::Sky && !matches!(drag, Drag::Turn(..)) {
            turn = turn.wrapping_add(dt * 1024 / 48_000);
        }
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
                files: store.loaded.files(),
                turn,
            };
            let covered = splash && veil == Some(1000);
            if !covered {
                match tab {
                    Tab::Graph => view::draw(&mut canvas, &scene),
                    Tab::Strata => {
                        match history.as_deref() {
                            Some(h) => strata::draw(&mut canvas, h, sky, now),
                            None => canvas.clear(aspecto::BG),
                        }
                        view::title(&mut canvas, &scene);
                    }
                    Tab::StrataGuide => {
                        strata_guide::draw(&mut canvas, door, sky);
                        view::title(&mut canvas, &scene);
                    }
                    space_tab => space::draw(&mut canvas, &scene, space_tab),
                }
                space::tabs(&mut canvas, tab);
                explorer::draw(&mut canvas, &store, &ui, shown.selected, now);
                match drag {
                    Drag::File(id, x0, y0) if ptr.inside && dragged(x0, y0, ptr.x, ptr.y) => {
                        explorer::draw_drag(&mut canvas, &store, &ui, &cam, id, ptr.x, ptr.y);
                    }
                    Drag::Item(i, x0, y0) if ptr.inside && dragged(x0, y0, ptr.x, ptr.y) => {
                        explorer::draw_drag_item(&mut canvas, &store, &ui, &cam, i, ptr.x, ptr.y);
                    }
                    Drag::Wire(from) if ptr.inside && tab == Tab::Graph => {
                        // Over a node: green if it can be let go, red with why.
                        let target = view::hit(&store.loaded.graph, &cam, ptr.x, ptr.y);
                        let verdict = target.map(|to| {
                            let r = store.wire_plan(from, to);
                            let name = |id| store.loaded.graph.node(id).map(|n| n.name).unwrap_or(bmo_titan_contrato::Text::new("?"));
                            r.map_err(|e| bmo_titan_lector::wire::note(Err(e), name(from).as_bytes(), name(to).as_bytes()))
                        });
                        let why = verdict.as_ref().map(|v| v.as_ref().map(|_| ()).map_err(|l| l.as_ref().map_or(&b""[..], |l| l.as_bytes())));
                        view::draw_wire(&mut canvas, &store.loaded.graph, &cam, from, ptr.x, ptr.y, why);
                    }
                    _ => {}
                }
            }
            if let (false, Some(e), Some(m)) = (covered, code.as_mut(), code_mem.as_ref()) {
                editor::draw(&mut canvas, e, store::text_block(m, EDIT_CAP), now);
            }
            if let (false, Some(p)) = (covered, palette.as_ref()) {
                tab::draw(&mut canvas, p, flowing.then_some(now).unwrap_or(0) as i32);
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
        // Esc closes F1 -- unless a name is being typed or a menu is open:
        // those take it first (see the loop).
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
