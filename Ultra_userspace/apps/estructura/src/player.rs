//! **THE PLAYER** -- walks the checker's script one event at a time, with time.
//!
//! It does not decide anything about borrowing: the script says what happened
//! (`bmo-titan-contrato`, already judged by `Script::check`) and this only
//! keeps WHEN each event is shown and what is still open while it plays:
//!
//! ```text
//!    loans   a `mut`/read loan that began and has not returned: its cable
//!            stays lit and its value shows on both nodes
//!    taken   a value moved for good: grey on the origin, owned by the target
//! ```
//!
//! It STOPS at the end of the script instead of looping: an animation nobody
//! asked to repeat is a window that spends forever (EFICIENCIA, R21).

use bmo_titan_contrato::{EventKind, Mode, Name, NodeId, Script, MAX_EVENTS};

/// How long each kind of event is on screen, in milliseconds.
pub const TRAVEL_MS: u32 = 700;
const HOLD_MS: u32 = 500;
const BACK_MS: u32 = 700;
const TAKE_MS: u32 = 1000;
const ALERT_MS: u32 = 3200;

#[derive(Clone, Copy)]
pub struct Loan {
    pub from: NodeId,
    pub to: NodeId,
    pub mode: Mode,
    pub value: Name,
}

#[derive(Clone, Copy)]
pub struct Taken {
    pub from: NodeId,
    pub to: NodeId,
    pub value: Name,
}

pub struct Player {
    /// The event on screen now.
    pub index: usize,
    /// Milliseconds into it.
    pub t: u32,
    pub playing: bool,
    pub loans: [Option<Loan>; MAX_EVENTS],
    pub taken: [Option<Taken>; MAX_EVENTS],
}

pub fn duration(kind: &EventKind) -> u32 {
    match kind {
        EventKind::Borrow { .. } => TRAVEL_MS + HOLD_MS,
        EventKind::Return { .. } => BACK_MS,
        EventKind::Take { .. } => TAKE_MS,
        EventKind::Conflict { .. } | EventKind::Denied { .. } => ALERT_MS,
    }
}

impl Player {
    pub fn new() -> Player {
        Player { index: 0, t: 0, playing: true, loans: [None; MAX_EVENTS], taken: [None; MAX_EVENTS] }
    }

    pub fn finished(&self, s: &Script) -> bool {
        self.index >= s.events().len()
    }

    pub fn restart(&mut self) {
        *self = Player::new();
    }

    /// Moves time forward. `true` if anything on screen changed.
    pub fn advance(&mut self, dt: u32, s: &Script) -> bool {
        if !self.playing || self.finished(s) {
            return false;
        }
        self.t += dt;
        while let Some(e) = s.events().get(self.index) {
            let d = duration(&e.kind);
            if self.t < d {
                break;
            }
            self.t -= d;
            self.complete(s);
        }
        if self.finished(s) {
            self.playing = false;
            self.t = 0;
        }
        true
    }

    /// Ends the current event now (the `n` key).
    pub fn step(&mut self, s: &Script) {
        if !self.finished(s) {
            self.t = 0;
            self.complete(s);
        }
    }

    /// The current event leaves its mark and the next one starts.
    fn complete(&mut self, s: &Script) {
        let Some(e) = s.events().get(self.index) else { return };
        match e.kind {
            EventKind::Borrow { mode, from, to } => {
                if let Some(slot) = self.loans.iter_mut().find(|l| l.is_none()) {
                    *slot = Some(Loan { from, to, mode, value: e.value });
                }
            }
            EventKind::Return { from, to } => {
                if let Some(slot) = self
                    .loans
                    .iter_mut()
                    .rev()
                    .find(|l| matches!(l, Some(x) if x.from == from && x.to == to && x.value == e.value))
                {
                    *slot = None;
                }
            }
            EventKind::Take { from, to } => {
                if let Some(slot) = self.taken.iter_mut().find(|l| l.is_none()) {
                    *slot = Some(Taken { from, to, value: e.value });
                }
            }
            EventKind::Conflict { .. } | EventKind::Denied { .. } => {}
        }
        self.index += 1;
    }

    /// How far into the current event, from 0 to 1000.
    pub fn progress(&self, s: &Script) -> u32 {
        match s.events().get(self.index) {
            Some(e) => (self.t.min(duration(&e.kind)) * 1000) / duration(&e.kind).max(1),
            None => 1000,
        }
    }
}
