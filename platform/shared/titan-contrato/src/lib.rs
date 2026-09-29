//! **TITAN++ CONTRACT** -- what the compiler says and what F1 draws, as data.
//!
//! [layer]  PURE   no hardware, no allocator, no `unsafe`: fixed arrays only
//! [cost]   DATA   a shape read differently by two sides is a graph that lies
//!                 or an animation of a loan that never happened
//!
//! # Why this crate exists (2026-09-29)
//!
//! The owner, on F1: *"aislar el frontend y el backend por completo"*. So there
//! are three pieces that do not know each other, and this is the only thing
//! they share:
//!
//! ```text
//!    titan-front   text -> tree -> types -> checker    WRITES a Graph, a
//!                                                      Script of events and
//!                                                      Diagnostics
//!    titan-back    own IR -> emisor-x86_64 / SPIR-V    (does not need this)
//!    F1            the node editor (sys/taller.bex)    READS them: draws the
//!                                                      graph, PLAYS the events
//! ```
//!
//! It is the LSP idea (editor and language talk by messages) and the house
//! idea of `bmo-puerta-red`: one FORM, two readers, no shared brain.
//!
//! # The three shapes
//!
//! ```text
//!    graph   a TITAN++ package IS a graph: modules are nodes and every `use`
//!            is an edge that only goes DOWN (a cycle is refused on insert).
//!            The manifest is the ROOT node; the 3060 and the DIRECTOR are
//!            nodes too, because loans travel to them (U1)
//!    event   what the borrow checker found, in order: a `mut` loan that
//!            begins and returns, a value TAKEN for good, a CONFLICT, a
//!            permission DENIED. F1 animates them one by one
//!    diag    the 4-part message: what, where, why, how to fix
//! ```
//!
//! Until the real front exists (T0 needs the owner's grammar), `sample` builds
//! one package by hand so F1 can be seen working in the Ryzen first.
//!
//! See `docs/maestro/TITAN_MAESTRO.md` (U1, U3, 6.9) and
//! `docs/plan/PLAN_TALLER.md` section 8.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(test)]
extern crate std;

pub mod event;
pub mod graph;
pub mod sample;

pub use event::{Diagnostic, Event, EventKind, Mode, Place, Script, ScriptError, MAX_DIAGS, MAX_EVENTS};
pub use graph::{Edge, Graph, GraphError, Lang, Node, NodeId, NodeKind, Permission, Permissions, MAX_EDGES, MAX_NODES};

/// A short piece of ASCII text in a fixed buffer: names and one-line texts
/// travel without an allocator. Longer input is CUT at `N`, and any byte that
/// is not printable ASCII becomes `?` -- the screen font of BMO-X is Latin-1
/// by bytes, and a UTF-8 sequence would draw as garbage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Text<const N: usize> {
    bytes: [u8; N],
    len: u8,
}

impl<const N: usize> Text<N> {
    pub const fn new(s: &str) -> Self {
        assert!(N <= 255, "Text<N> keeps its length in a u8");
        let b = s.as_bytes();
        let mut bytes = [0u8; N];
        let mut i = 0;
        while i < b.len() && i < N {
            let c = b[i];
            bytes[i] = if c >= 0x20 && c < 0x7F { c } else { b'?' };
            i += 1;
        }
        Text { bytes, len: i as u8 }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A node or value name.
pub type Name = Text<24>;
/// One line of a diagnostic or a module's "what it does".
pub type Line = Text<72>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_keeps_ascii_and_cuts_at_n() {
        let t: Text<4> = Text::new("hero-of-time");
        assert_eq!(t.as_bytes(), b"hero");
        assert_eq!(t.len(), 4);
    }

    #[test]
    fn text_turns_what_the_font_cannot_draw_into_a_question_mark() {
        // "n" with tilde is two bytes in UTF-8: both become '?', never garbage.
        let t: Text<8> = Text::new("a\u{f1}o");
        assert_eq!(t.as_bytes(), b"a??o");
    }

    #[test]
    fn empty_text_is_empty() {
        let t: Name = Text::new("");
        assert!(t.is_empty());
    }
}
