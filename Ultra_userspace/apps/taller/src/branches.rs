//! **THE BRANCHES OF ESTRATOS, IN F1** -- R5 of `docs/plan/PLAN_LAS_RAMAS.md`:
//! every branch a node, and a MERGE you choose with your own hand.
//!
//! [consumo] NADA      draws only when F1 draws; it asks the kernel when the
//!                     tab opens, when the generation moved, and on a key
//!
//! ```text
//!    the branches      a column: each one with its SEAL, the current one lit.
//!                      ENTER twice: switch to it. N: a new one, typed
//!    M                 COUNT the merge of the picked branch into the current
//!                      one: two chains that meet in one node, with the LOCK
//!    every conflict    a SPLIT node: the left half is NOW, the right half the
//!                      branch that comes in. A, B or Q picks a whole node
//!    ENTER twice       when all are picked: MERGE -- ONE estrato, two parents
//! ```
//!
//! ** THE LOCK (`ES_RAMA_CANDADO`) does not lock the volume: it guards the
//! choice. CLOSED: counted and waiting, nothing written -- Esc and come back,
//! nothing is lost. BROKEN: something else wrote meanwhile, so what was counted
//! is about another volume, and ENTER counts again. Locking the volume instead
//! would freeze a game that saves while a person thinks.
//!
//! This file knows no kernel: every question goes through [`Volume`], which
//! `store.rs` answers with `bmo::estratos` and the host's camera with a sample.

use crate::aspecto as look;
use crate::astros::{arc, halo, hash, orbit, seal};
use crate::canvas::Canvas;
use crate::view::{LEFT, PANEL, TOP};
use crate::{KEY_DOWN, KEY_UP};
use bmo_dibujo::{mezclar, Color, Lienzo};
use look::{ACCENT, AMBER, BAD, BAR, BG, DIM, EDGE, INK, NEON, SEL, VIOLET};

/// Branches in one table (`bmo_estratos::ramas::RAMAS_MAX`).
pub const MAX: usize = 36;
pub const NAME_MAX: usize = 63;
/// The conflicts the kernel keeps for choosing (`MEZCLA_CHOQUES_MAX`).
pub const CONFLICTS: usize = 64;
/// What is kept of a conflict's path (the kernel keeps 255).
pub const PATH_MAX: usize = 96;
/// Two ENTERs within this long switch or merge.
pub const CONFIRM_MS: u32 = 4_000;

/// What F1 asks the volume. Same answers as `bmo::estratos`'s.
pub trait Volume {
    /// The name of branch `i` into `dst`: `(length, is the current one)`.
    fn branch(&mut self, i: usize, dst: &mut [u8]) -> Option<(usize, bool)>;
    fn create(&mut self, name: &[u8]) -> bool;
    fn switch(&mut self, name: &[u8]) -> bool;
    /// `(conflicts, blocks)`.
    fn count(&mut self, name: &[u8]) -> Option<(u32, u32)>;
    /// Conflict `i`'s path into `dst`: `(length, sides, pick)`.
    fn conflict(&mut self, i: usize, dst: &mut [u8]) -> Option<(usize, u8, u8)>;
    fn choose(&mut self, i: usize, pick: u8) -> bool;
    fn merge(&mut self) -> bool;
    /// `(state, conflicts, picked)`: 0 nothing counted, 1 closed, 2 broken.
    fn lock(&mut self) -> (u64, u32, u32);
}

#[derive(Clone, Copy)]
pub struct Branch {
    name: [u8; NAME_MAX],
    len: usize,
    pub current: bool,
}

impl Branch {
    const EMPTY: Branch = Branch { name: [0; NAME_MAX], len: 0, current: false };

    pub fn name(&self) -> &[u8] {
        &self.name[..self.len.min(NAME_MAX)]
    }
}

#[derive(Clone, Copy)]
pub struct Conflict {
    path: [u8; PATH_MAX],
    len: usize,
    /// bit 0: NOW has it; bit 1: the incoming branch has it.
    sides: u8,
    /// 0 not yet; 1 NOW (A), 2 the incoming one (B), 3 neither.
    pub pick: u8,
}

impl Conflict {
    const EMPTY: Conflict = Conflict { path: [0; PATH_MAX], len: 0, sides: 0, pick: 0 };

    pub fn path(&self) -> &[u8] {
        &self.path[..self.len.min(PATH_MAX)]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lock {
    None,
    Closed,
    Broken,
}

/// What the tab shows and where the person is in it.
pub struct Branches {
    pub list: [Branch; MAX],
    pub n: usize,
    pub picked: usize,
    /// The branch that comes in, while a merge is on screen.
    from: [u8; NAME_MAX],
    from_len: usize,
    /// The branch whose merge the kernel holds counted (its lock).
    counted: [u8; NAME_MAX],
    counted_len: usize,
    pub conflicts: [Conflict; CONFLICTS],
    pub nc: usize,
    /// What the kernel counted: more than [`CONFLICTS`] cannot be chosen.
    pub total: u32,
    pub blocks: u32,
    pub lock: Lock,
    pub on: usize,
    /// A new branch's name, while it is typed.
    typing: Option<([u8; NAME_MAX], usize)>,
    /// The first ENTER: (on what, until when). `s` switch, `x` merge.
    confirm: Option<(u8, u32)>,
    pub said: Option<(&'static [u8], bool)>,
}

impl Branches {
    pub const EMPTY: Branches = Branches {
        list: [Branch::EMPTY; MAX],
        n: 0,
        picked: 0,
        from: [0; NAME_MAX],
        from_len: 0,
        counted: [0; NAME_MAX],
        counted_len: 0,
        conflicts: [Conflict::EMPTY; CONFLICTS],
        nc: 0,
        total: 0,
        blocks: 0,
        lock: Lock::None,
        on: 0,
        typing: None,
        confirm: None,
        said: None,
    };

    fn merging(&self) -> bool {
        self.from_len > 0
    }

    pub fn from(&self) -> &[u8] {
        &self.from[..self.from_len]
    }

    pub fn current(&self) -> &[u8] {
        self.list[..self.n].iter().find(|b| b.current).map_or(&b"la de ahora"[..], |b| b.name())
    }

    /// Reads the branches and the lock again: when the tab opens, when the
    /// generation moved, and after every gesture.
    pub fn refresh(&mut self, v: &mut dyn Volume) {
        self.n = 0;
        while self.n < MAX {
            let b = &mut self.list[self.n];
            match v.branch(self.n, &mut b.name) {
                Some((len, current)) => {
                    b.len = len.min(NAME_MAX);
                    b.current = current;
                    self.n += 1;
                }
                None => break,
            }
        }
        self.picked = self.picked.min(self.n.saturating_sub(1));
        let (state, total, _) = v.lock();
        self.lock = match state {
            1 => Lock::Closed,
            2 => Lock::Broken,
            _ => Lock::None,
        };
        if self.lock == Lock::None {
            self.from_len = 0;
            self.nc = 0;
        } else {
            self.total = total;
            self.read_conflicts(v);
        }
    }

    fn read_conflicts(&mut self, v: &mut dyn Volume) {
        self.nc = 0;
        while self.nc < CONFLICTS {
            let c = &mut self.conflicts[self.nc];
            match v.conflict(self.nc, &mut c.path) {
                Some((len, sides, pick)) => {
                    c.len = len.min(PATH_MAX);
                    c.sides = sides;
                    c.pick = pick;
                    self.nc += 1;
                }
                None => break,
            }
        }
        self.on = self.on.min(self.nc.saturating_sub(1));
    }

    fn picked_all(&self) -> bool {
        self.total as usize <= CONFLICTS && self.conflicts[..self.nc].iter().all(|c| c.pick != 0)
    }

    /// Does this tab take key `k`? While a name is typed, every key.
    pub fn wants(&self, k: u8) -> bool {
        self.typing.is_some()
            || matches!(k, KEY_UP | KEY_DOWN | b'\r' | b'\n' | 0x1B | b'n' | b'N' | b'm' | b'M')
            || (self.merging() && matches!(k, b'a' | b'A' | b'b' | b'B' | b'q' | b'Q'))
    }

    fn twice(&mut self, what: u8, now: u32) -> bool {
        match self.confirm {
            Some((w, until)) if w == what && until.wrapping_sub(now) < u32::MAX / 2 => {
                self.confirm = None;
                true
            }
            _ => {
                self.confirm = Some((what, now.wrapping_add(CONFIRM_MS)));
                self.said = None;
                false
            }
        }
    }

    /// A key, in this tab.
    pub fn key(&mut self, k: u8, now: u32, v: &mut dyn Volume) {
        if let Some((mut name, mut len)) = self.typing {
            match k {
                0x1B => self.typing = None,
                b'\r' | b'\n' => {
                    self.typing = None;
                    let ok = len > 0 && v.create(&name[..len]);
                    self.said = Some(if ok { (&b"RAMA NUEVA en la punta de ahora: no se copio nada"[..], true) } else { (&b"NO: ese nombre ya esta, o no vale"[..], false) });
                    self.refresh(v);
                }
                0x08 | 0x7F => {
                    len = len.saturating_sub(1);
                    self.typing = Some((name, len));
                }
                0x20..=0x7E | 0xA0..=0xFF if len < NAME_MAX => {
                    name[len] = k;
                    self.typing = Some((name, len + 1));
                }
                _ => {}
            }
            return;
        }
        if self.merging() {
            self.merge_key(k, now, v);
            return;
        }
        match k {
            KEY_UP => self.picked = self.picked.saturating_sub(1),
            KEY_DOWN => self.picked = (self.picked + 1).min(self.n.saturating_sub(1)),
            b'n' | b'N' => self.typing = Some(([0; NAME_MAX], 0)),
            0x1B => {
                self.confirm = None;
                self.said = None;
            }
            b'm' | b'M' => self.count(v),
            _ => {
                let Some(b) = self.list[..self.n].get(self.picked).copied() else { return };
                if b.current {
                    self.said = Some((&b"ya es la rama actual"[..], false));
                } else if self.twice(b's', now) {
                    let ok = v.switch(b.name());
                    self.said = Some(if ok { (&b"CAMBIO DE RAMA: la de antes se queda guardada en la tabla"[..], true) } else { (&b"NO se pudo cambiar de rama"[..], false) });
                    self.refresh(v);
                }
            }
        }
    }

    /// M on the picked branch: count its merge into the current one -- or, if
    /// it is the one already counted and the lock holds, just come back to it.
    fn count(&mut self, v: &mut dyn Volume) {
        let Some(b) = self.list[..self.n].get(self.picked).copied() else {
            self.said = Some((&b"el volumen no tiene ramas: N crea la primera"[..], false));
            return;
        };
        if b.current {
            self.said = Some((&b"esa es la actual: elige OTRA rama para mezclarla aqui"[..], false));
            return;
        }
        self.from[..b.len].copy_from_slice(b.name());
        self.from_len = b.len;
        let (state, _, _) = v.lock();
        if state == 1 && &self.counted[..self.counted_len] == b.name() {
            self.lock = Lock::Closed;
            return;
        }
        match v.count(b.name()) {
            Some((total, blocks)) => {
                self.total = total;
                self.blocks = blocks;
                self.on = 0;
                self.said = None;
                self.refresh(v);
                self.from[..b.len].copy_from_slice(b.name());
                self.from_len = b.len;
                self.counted = self.from;
                self.counted_len = b.len;
            }
            None => {
                self.from_len = 0;
                self.said = Some((&b"NO se puede mezclar: sin historia comun, nada nuevo, o una carpeta muy grande"[..], false));
            }
        }
    }

    fn merge_key(&mut self, k: u8, now: u32, v: &mut dyn Volume) {
        let pick = match k {
            b'a' | b'A' => 1,
            b'b' | b'B' => 2,
            b'q' | b'Q' => 3,
            _ => 0,
        };
        match k {
            KEY_UP => self.on = self.on.saturating_sub(1),
            KEY_DOWN => self.on = (self.on + 1).min(self.nc.saturating_sub(1)),
            // Esc: back to the list. NOTHING was written, and the lock keeps
            // what was picked: M on the same branch comes back to it.
            0x1B => {
                self.from_len = 0;
                self.confirm = None;
            }
            _ if pick != 0 => {
                if self.lock == Lock::Closed && self.on < self.nc && v.choose(self.on, pick) {
                    self.conflicts[self.on].pick = pick;
                    // On to the next one still open.
                    if let Some(next) = (0..self.nc).map(|d| (self.on + 1 + d) % self.nc).find(|&i| self.conflicts[i].pick == 0) {
                        self.on = next;
                    }
                }
            }
            b'\r' | b'\n' if self.lock == Lock::Broken => {
                let from = self.from;
                let len = self.from_len;
                if let Some(i) = self.list[..self.n].iter().position(|b| b.name() == &from[..len]) {
                    self.picked = i;
                    self.nc = 0;
                    self.count(v);
                }
            }
            b'\r' | b'\n' if !self.picked_all() => {
                self.said = Some((&b"faltan choques por elegir: A, B o Q en cada uno"[..], false));
            }
            b'\r' | b'\n' => {
                if self.twice(b'x', now) {
                    let ok = v.merge();
                    self.said = Some(if ok { (&b"MEZCLADA: un estrato de DOS padres; las dos historias siguen enteras"[..], true) } else { (&b"NO se mezclo: el volumen no cambio (mira F11)"[..], false) });
                    if ok {
                        self.from_len = 0;
                    }
                    self.refresh(v);
                }
            }
            _ => {}
        }
    }

    /// A click: a branch in the column, or a conflict in the list.
    pub fn click(&mut self, c: &Canvas, x: i32, y: i32) {
        let (x0, y0, _, _) = area(c);
        if x < x0 + COLUMN {
            let i = ((y - y0 - 24) / ROW) as usize;
            if y >= y0 + 24 && i < self.n {
                self.picked = i;
                self.confirm = None;
            }
        } else if self.merging() {
            let top = conflicts_top(c);
            if y >= top {
                let i = ((y - top) / ROW) as usize;
                if i < self.nc {
                    self.on = i;
                }
            }
        }
    }
}

const COLUMN: i32 = 260;
const ROW: i32 = 22;

fn area(c: &Canvas) -> (i32, i32, i32, i32) {
    let top = TOP + 40;
    (LEFT + 24, top, c.w - LEFT - 48, c.h - PANEL - top - 16)
}

fn conflicts_top(c: &Canvas) -> i32 {
    area(c).1 + 186
}

/// The colour of NOW, and of what comes in.
const A: Color = NEON;
const B: Color = VIOLET;

/// A node split in two: the left half NOW, the right half the incoming one.
/// The picked half is bright; a side that does not have the node is hollow.
fn split(c: &mut Canvas, x: i32, y: i32, r: i32, sides: u8, pick: u8) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let (left, has, mine) = if dx < 0 { (true, sides & 1 != 0, 1) } else { (false, sides & 2 != 0, 2) };
            let base = if left { A } else { B };
            let edge = dx * dx + dy * dy > (r - 2) * (r - 2);
            let color = match (has, pick == mine, pick == 0) {
                (false, _, _) if !edge => continue,
                (_, true, _) => base,
                (_, _, true) => mezclar(base, BG, 1, 2),
                _ => mezclar(base, BG, 1, 3),
            };
            c.put(x + dx, y + dy, color);
        }
    }
    c.rect(x, y - r, 1, 2 * r + 1, BG);
    if pick == 3 {
        for k in -r / 2..=r / 2 {
            c.put(x + k, y + k, BAD);
            c.put(x + k, y - k, BAD);
        }
    }
}

/// THE LOCK, drawn: closed (the choice is safe) or broken (count again).
fn padlock(c: &mut Canvas, x: i32, y: i32, lock: Lock) {
    let color = if lock == Lock::Broken { BAD } else { A };
    c.rect(x - 9, y, 18, 14, color);
    c.rect(x - 1, y + 4, 2, 5, BG);
    // The shackle: on the body when closed; lifted and off to one side when
    // broken -- the picture of "this no longer holds".
    let (sx, sy) = if lock == Lock::Broken { (x + 5, y - 5) } else { (x, y) };
    arc(c, sx, sy, 6, 8, 32, 64, color);
    arc(c, sx, sy, 5, 7, 32, 64, color);
}

/// The whole tab. `None`: there was no memory for it, and the tab is empty.
pub fn draw(c: &mut Canvas, b: Option<&Branches>, sky: Option<&[u32]>, now: u32) {
    match sky {
        Some(px) => c.blit(px),
        None => c.clear(BG),
    }
    let Some(b) = b else { return };
    let (x0, y0, w, _) = area(c);
    // The column of branches.
    c.text(x0, y0, b"RAMAS", NEON, 1);
    if b.n == 0 {
        c.text(x0, y0 + 24, b"sin ramas: la de ahora", DIM, 1);
        c.text(x0, y0 + 24 + ROW, b"N crea la primera", DIM, 1);
    }
    for (i, br) in b.list[..b.n].iter().enumerate() {
        let y = y0 + 24 + i as i32 * ROW;
        if i == b.picked && !b.merging() {
            c.rect(x0 - 4, y - 4, COLUMN - 12, ROW - 2, SEL);
        }
        let s = hash(br.name());
        let hue = if br.current { A } else { mezclar(B, INK, 1, 3) };
        if br.current {
            halo(c, x0 + 8, y + 6, 7, 6, A, 200);
        }
        seal(c, x0 + 8, y + 6, 8, s, hue);
        let k = c.text_fit(x0 + 24, y, br.name(), if br.current { INK } else { DIM }, COLUMN - 110);
        if br.current {
            c.text(x0 + 24 + k + 8, y, b"actual", A, 1);
        }
    }
    if let Some((name, len)) = b.typing {
        let y = y0 + 24 + b.n as i32 * ROW + 6;
        c.rect(x0 - 4, y - 4, COLUMN - 12, ROW, BAR);
        let k = c.text(x0, y, b"nueva: ", AMBER, 1);
        let k = k + c.text_fit(x0 + k, y, &name[..len], INK, COLUMN - 80);
        if now / 500 % 2 == 0 {
            c.rect(x0 + k + 1, y - 1, 2, 12, INK);
        }
    }
    let rx = x0 + COLUMN + 24;
    let rw = w - COLUMN - 24;
    if !b.merging() {
        c.text(rx, y0, b"M: mezclar la rama elegida en la actual", DIM, 1);
        c.text(rx, y0 + 20, b"cada choque es un nodo partido: la mitad de ahora y la de la otra rama", DIM, 1);
        if b.lock != Lock::None {
            padlock(c, rx + 9, y0 + 50, b.lock);
            c.text(rx + 28, y0 + 52, b"hay una mezcla contada: M sobre su rama vuelve a ella", INK, 1);
        }
        panel(c, b, now);
        return;
    }
    // Two chains that meet: NOW on the left, the incoming one on the right,
    // and the node they will make, with the lock beside it.
    let (ax, bx, ty, my) = (rx + rw / 4, rx + rw * 3 / 4, y0 + 30, y0 + 110);
    let mx = (ax + bx) / 2;
    for (x, color) in [(ax, A), (bx, B)] {
        let (dx, dy) = (mx - x, my - ty);
        for k in 0..=24 {
            c.put(x + dx * k / 24, ty + dy * k / 24, mezclar(color, BG, 2, 3));
        }
    }
    seal(c, ax, ty, 14, hash(b.current()), A);
    seal(c, bx, ty, 14, hash(b.from()), B);
    let k = (b.current().len().min(24) as i32) * 4;
    c.text_fit(ax - k, ty - 32, b.current(), A, 24 * 8);
    let k = (b.from().len().min(24) as i32) * 4;
    c.text_fit(bx - k, ty - 32, b.from(), B, 24 * 8);
    let state = b.lock;
    halo(c, mx, my, 16, 10, if state == Lock::Broken { BAD } else { ACCENT }, 220);
    c.disc(mx, my, 16, mezclar(ACCENT, BG, 1, 8));
    seal(c, mx, my, 13, hash(b.current()) ^ hash(b.from()).rotate_left(11), ACCENT);
    orbit(c, mx, my, 20, 20, mezclar(ACCENT, BG, 1, 2));
    padlock(c, rx + 9, my + 24, state);
    let (msg, color): (&[u8], Color) = match state {
        Lock::Broken => (b"ROTO: el volumen cambio desde que se conto -- ENTER cuenta otra vez", BAD),
        _ => (b"CERRADO: contado, NADA escrito -- puedes irte (Esc) y volver", A),
    };
    c.text_fit(rx + 28, my + 28, msg, color, rw - 32);

    // The conflicts, each one a split node.
    let top = conflicts_top(c);
    let mut n = [0u8; 4];
    let picked = b.conflicts[..b.nc].iter().filter(|c| c.pick != 0).count() as u32;
    let k = c.text(rx, top - 22, b"CHOQUES ", INK, 1);
    let k = k + c.text(rx + k, top - 22, digits(picked, &mut n), A, 1);
    let k = k + c.text(rx + k, top - 22, b" de ", DIM, 1);
    let k = k + c.text(rx + k, top - 22, digits(b.total, &mut n), INK, 1);
    c.text(rx + k, top - 22, b" elegidos", DIM, 1);
    if b.nc == 0 {
        c.text(rx, top, b"ninguno: las dos ramas se juntan solas", A, 1);
    }
    let rows = ((c.h - PANEL - 12 - top) / ROW).max(1) as usize;
    let first = b.on.saturating_sub(rows - 1);
    for (j, cf) in b.conflicts[first..b.nc].iter().take(rows).enumerate() {
        let i = first + j;
        let y = top + j as i32 * ROW;
        if i == b.on {
            c.rect(rx - 4, y - 5, rw, ROW - 2, SEL);
            c.rect(rx - 4, y - 5, 2, ROW - 2, EDGE);
        }
        split(c, rx + 8, y + 5, 8, cf.sides, cf.pick);
        let k = 24 + c.text_fit(rx + 24, y, cf.path(), INK, rw - 220);
        let (word, color): (&[u8], Color) = match cf.pick {
            1 => (b"A: la de ahora", A),
            2 => (b"B: la que entra", B),
            3 => (b"ninguno de los dos", BAD),
            _ => (b"sin elegir", AMBER),
        };
        c.text(rx + k.max(rw - 180), y, word, color, 1);
    }
    panel(c, b, now);
}

fn digits(mut v: u32, out: &mut [u8; 4]) -> &[u8] {
    let mut n = 0;
    let mut tmp = [0u8; 4];
    loop {
        tmp[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 || n == 4 {
            break;
        }
    }
    for k in 0..n {
        out[k] = tmp[n - 1 - k];
    }
    &out[..n]
}

fn panel(c: &mut Canvas, b: &Branches, now: u32) {
    let (x, y, w) = (LEFT, c.h - PANEL, c.w - LEFT);
    c.rect(x, y, w, PANEL, BAR);
    c.rect(x, y, w, 1, EDGE);
    let (tx, mut ty) = (x + 12, y + 10);
    c.text(tx, ty, b"RAMAS", NEON, 1);
    c.text(tx + 56, ty, b"una rama es un nombre para una punta: no copia nada; mezclar junta dos por NODOS, enteros", DIM, 1);
    ty += 22;
    let waiting = |what| b.confirm.is_some_and(|(w, until)| w == what && until.wrapping_sub(now) < u32::MAX / 2);
    let help: (&[u8], Color) = if b.typing.is_some() {
        (b"escribe el nombre   ENTER: crear la rama   Esc: nada", INK)
    } else if b.merging() && waiting(b'x') {
        (b"ENTER otra vez: MEZCLAR -- UN estrato de dos padres; nada se pisa", AMBER)
    } else if b.merging() && b.lock == Lock::Broken {
        (b"ENTER: contar otra vez (lo elegido se pierde: era de otro volumen)   Esc: volver", AMBER)
    } else if b.merging() {
        (b"flechas: choque   A: el de ahora   B: el que entra   Q: ninguno   ENTER dos veces: mezclar   Esc: volver", INK)
    } else if waiting(b's') {
        (b"ENTER otra vez: CAMBIAR a esa rama -- la de ahora se queda guardada", AMBER)
    } else {
        (b"flechas: rama   ENTER dos veces: cambiar a ella   N: rama nueva   M: mezclarla en la actual", INK)
    };
    c.text_fit(tx, ty, help.0, help.1, w - 24);
    ty += 20;
    if b.merging() && b.total as usize > CONFLICTS {
        c.text(tx, ty, b"mas de 64 choques: se cuentan, pero no se pueden elegir todos todavia", BAD, 1);
        ty += 20;
    }
    if let Some((text, good)) = b.said {
        c.text_fit(tx, ty, text, if good { NEON } else { BAD }, w - 24);
    }
}
