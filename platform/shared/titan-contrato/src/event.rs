//! **THE EVENTS** -- what the borrow checker found, in order, for F1 to PLAY.
//!
//! rustc spends 42 % of its borrow checker EXPLAINING (measured 29-09,
//! `TITAN_MAESTRO` 6.8). F1 shows it instead: every loan is an event that
//! travels along an edge of the graph, and every NO carries its 4-part message.
//!
//! ```text
//!    Borrow   a `mut` (or read) loan BEGINS: from the caller to the callee,
//!             or to the 3060 / the DIRECTOR (U1)
//!    Return   that loan ENDS: the call returned, or `wait` came back
//!    Take     the value moved for good: the origin no longer has it
//!    Conflict two accesses that overlap: the error, drawn
//!    Denied   a module used a permission the manifest did not ask for (U2)
//! ```
//!
//! `Script::check` is this shape's own judge: a script that returns a loan
//! nobody lent, or leaves one open, or walks an edge that does not exist, is
//! refused before F1 animates a lie.

use crate::graph::{Graph, NodeId, Permission};
use crate::{Line, Name, Text};

pub const MAX_EVENTS: usize = 32;
pub const MAX_DIAGS: usize = 8;

/// How a parameter receives a value (`TITAN_MAESTRO` 6.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// No word: the caller keeps it, nobody changes it.
    Read,
    /// `mut`: lent to be changed; nobody else touches it while it lasts.
    Mut,
    /// `take`: the value now belongs to the callee.
    Take,
}

impl Mode {
    pub const fn word(self) -> &'static str {
        match self {
            Mode::Read => "read",
            Mode::Mut => "mut",
            Mode::Take => "take",
        }
    }
}

/// Where something happened: a node and a line of its source.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Place {
    pub node: NodeId,
    pub line: u16,
}

/// The 4-part message inherited from INTI: what, where, why, how to fix.
/// The texts are what the owner reads on screen: Spanish, ASCII, no tilde.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    pub what: Line,
    pub place: Place,
    pub why: Line,
    pub fix: Line,
}

impl Diagnostic {
    pub const fn new(what: &str, place: Place, why: &str, fix: &str) -> Diagnostic {
        Diagnostic { what: Text::new(what), place, why: Text::new(why), fix: Text::new(fix) }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventKind {
    Borrow { mode: Mode, from: NodeId, to: NodeId },
    Return { from: NodeId, to: NodeId },
    Take { from: NodeId, to: NodeId },
    /// `diag` is an index into the script's diagnostics.
    Conflict { first: Place, second: Place, diag: u8 },
    Denied { node: NodeId, permission: Permission, diag: u8 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Event {
    pub kind: EventKind,
    /// The value the event is about (`hero`, `buf`...).
    pub value: Name,
}

impl Event {
    pub const fn new(kind: EventKind, value: &str) -> Event {
        Event { kind, value: Text::new(value) }
    }

    const EMPTY: Event = Event::new(EventKind::Return { from: NodeId(0), to: NodeId(0) }, "");
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScriptError {
    Full,
    /// Event `i` names a node the graph does not have.
    NoSuchNode(usize),
    /// Event `i` travels `from -> to` and there is no such `use`.
    NoSuchEdge(usize),
    /// Event `i` returns a loan that was never lent.
    ReturnWithoutBorrow(usize),
    /// Event `i` lent something that is never returned.
    Unreturned(usize),
    /// Event `i` points to a diagnostic that does not exist.
    NoSuchDiag(usize),
    /// Event `i` is `Borrow` with `Mode::Take`: moving is `Take`, not a loan.
    TakeIsNotALoan(usize),
}

pub struct Script {
    events: [Event; MAX_EVENTS],
    n_events: u8,
    diags: [Diagnostic; MAX_DIAGS],
    n_diags: u8,
}

impl Default for Script {
    fn default() -> Self {
        Script::new()
    }
}

impl Script {
    pub const fn new() -> Script {
        Script {
            events: [Event::EMPTY; MAX_EVENTS],
            n_events: 0,
            diags: [Diagnostic::new("", Place { node: NodeId(0), line: 0 }, "", ""); MAX_DIAGS],
            n_diags: 0,
        }
    }

    pub fn push(&mut self, e: Event) -> Result<(), ScriptError> {
        let n = self.n_events as usize;
        if n >= MAX_EVENTS {
            return Err(ScriptError::Full);
        }
        self.events[n] = e;
        self.n_events += 1;
        Ok(())
    }

    /// Stores a diagnostic and returns the index an event uses to point at it.
    pub fn diagnose(&mut self, d: Diagnostic) -> Result<u8, ScriptError> {
        let n = self.n_diags as usize;
        if n >= MAX_DIAGS {
            return Err(ScriptError::Full);
        }
        self.diags[n] = d;
        self.n_diags += 1;
        Ok(n as u8)
    }

    pub fn events(&self) -> &[Event] {
        &self.events[..self.n_events as usize]
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diags[..self.n_diags as usize]
    }

    pub fn diagnostic(&self, i: u8) -> Option<&Diagnostic> {
        self.diagnostics().get(i as usize)
    }

    /// The script's own judge, against the graph it will be played on.
    pub fn check(&self, g: &Graph) -> Result<(), ScriptError> {
        // The loans still open, as (event index, from, to, value).
        let mut open: [Option<(usize, NodeId, NodeId, Name)>; MAX_EVENTS] = [None; MAX_EVENTS];
        let has = |id: NodeId| g.node(id).is_some();
        for (i, e) in self.events().iter().enumerate() {
            match e.kind {
                EventKind::Borrow { mode, from, to } => {
                    if mode == Mode::Take {
                        return Err(ScriptError::TakeIsNotALoan(i));
                    }
                    walk(g, i, from, to)?;
                    open[i] = Some((i, from, to, e.value));
                }
                EventKind::Return { from, to } => {
                    walk(g, i, from, to)?;
                    let slot = open.iter_mut().rev().find(|o| {
                        matches!(o, Some((_, f, t, v)) if *f == from && *t == to && *v == e.value)
                    });
                    match slot {
                        Some(s) => *s = None,
                        None => return Err(ScriptError::ReturnWithoutBorrow(i)),
                    }
                }
                EventKind::Take { from, to } => walk(g, i, from, to)?,
                EventKind::Conflict { first, second, diag } => {
                    if !has(first.node) || !has(second.node) {
                        return Err(ScriptError::NoSuchNode(i));
                    }
                    if self.diagnostic(diag).is_none() {
                        return Err(ScriptError::NoSuchDiag(i));
                    }
                }
                EventKind::Denied { node, diag, .. } => {
                    if !has(node) {
                        return Err(ScriptError::NoSuchNode(i));
                    }
                    if self.diagnostic(diag).is_none() {
                        return Err(ScriptError::NoSuchDiag(i));
                    }
                }
            }
        }
        match open.iter().flatten().next() {
            Some((i, ..)) => Err(ScriptError::Unreturned(*i)),
            None => Ok(()),
        }
    }
}

/// A loan or a move travels along a `use` edge that must exist.
fn walk(g: &Graph, i: usize, from: NodeId, to: NodeId) -> Result<(), ScriptError> {
    if g.node(from).is_none() || g.node(to).is_none() {
        return Err(ScriptError::NoSuchNode(i));
    }
    if !g.edge_between(from, to) {
        return Err(ScriptError::NoSuchEdge(i));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Lang, Node, NodeKind};

    fn two() -> (Graph, NodeId, NodeId) {
        let mut g = Graph::new();
        let a = g.add(Node::new(NodeKind::Module, Lang::Titan, "a", "", 0, 0)).unwrap();
        let b = g.add(Node::new(NodeKind::Module, Lang::Titan, "b", "", 0, 0)).unwrap();
        g.connect(a, b).unwrap();
        (g, a, b)
    }

    #[test]
    fn a_loan_that_returns_is_fine() {
        let (g, a, b) = two();
        let mut s = Script::new();
        s.push(Event::new(EventKind::Borrow { mode: Mode::Mut, from: a, to: b }, "hero")).unwrap();
        s.push(Event::new(EventKind::Return { from: a, to: b }, "hero")).unwrap();
        assert_eq!(s.check(&g), Ok(()));
    }

    #[test]
    fn a_loan_never_returned_is_named() {
        let (g, a, b) = two();
        let mut s = Script::new();
        s.push(Event::new(EventKind::Borrow { mode: Mode::Mut, from: a, to: b }, "hero")).unwrap();
        assert_eq!(s.check(&g), Err(ScriptError::Unreturned(0)));
    }

    #[test]
    fn returning_what_nobody_lent_is_refused() {
        let (g, a, b) = two();
        let mut s = Script::new();
        s.push(Event::new(EventKind::Borrow { mode: Mode::Mut, from: a, to: b }, "hero")).unwrap();
        s.push(Event::new(EventKind::Return { from: a, to: b }, "map")).unwrap();
        assert_eq!(s.check(&g), Err(ScriptError::ReturnWithoutBorrow(1)));
    }

    #[test]
    fn a_loan_must_travel_along_a_real_use() {
        let (g, a, b) = two();
        let mut s = Script::new();
        // b does not use a: the edge goes a -> b only.
        s.push(Event::new(EventKind::Take { from: b, to: a }, "bullet")).unwrap();
        assert_eq!(s.check(&g), Err(ScriptError::NoSuchEdge(0)));
    }

    #[test]
    fn take_is_not_a_loan() {
        let (g, a, b) = two();
        let mut s = Script::new();
        s.push(Event::new(EventKind::Borrow { mode: Mode::Take, from: a, to: b }, "x")).unwrap();
        assert_eq!(s.check(&g), Err(ScriptError::TakeIsNotALoan(0)));
    }

    #[test]
    fn a_conflict_needs_its_message() {
        let (g, a, b) = two();
        let mut s = Script::new();
        let here = Place { node: a, line: 3 };
        let there = Place { node: b, line: 9 };
        s.push(Event::new(EventKind::Conflict { first: here, second: there, diag: 0 }, "hero")).unwrap();
        assert_eq!(s.check(&g), Err(ScriptError::NoSuchDiag(0)));
        let d = s.diagnose(Diagnostic::new("que", here, "por que", "como")).unwrap();
        assert_eq!(d, 0);
        assert_eq!(s.check(&g), Ok(()));
    }
}
