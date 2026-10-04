//! `message` -- the four-part message, and the codes that do not change.
//!
//! The message IS the main interface of a language: a programmer reads more
//! errors than documentation. So it can be tested alone -- this module knows
//! nothing of a lexer or a parser. It formats four parts:
//!
//! ```text
//!    QUE       one sentence, in Spanish, no compiler jargon
//!    DONDE     file, line, and THE LINE with a finger under the spot
//!    POR QUE   what was there, with the names the author wrote
//!    COMO      what to write instead
//! ```
//!
//! The same four as INTI (`toolchain/lang/inti/src/aviso`) and as the F1
//! panel (QUE / DONDE / POR QUE / COMO): one shape for every NO in the house.

use std::fmt::Write;

/// A stable code. The number never changes meaning: a test pins them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    NoHeader,
    Tab,
    BadIndent,
    OpenText,
    StrayChar,
    BadEscape,
    Expected,
    EmptyBody,
    NotYet,
    NoMain,
    Unknown,
    Twice,
    Endless,
}

impl Code {
    pub const ALL: [Code; 13] = [
        Code::NoHeader,
        Code::Tab,
        Code::BadIndent,
        Code::OpenText,
        Code::StrayChar,
        Code::BadEscape,
        Code::Expected,
        Code::EmptyBody,
        Code::NotYet,
        Code::NoMain,
        Code::Unknown,
        Code::Twice,
        Code::Endless,
    ];

    pub fn number(self) -> u16 {
        match self {
            Code::NoHeader => 1,
            Code::Tab => 10,
            Code::BadIndent => 12,
            Code::OpenText => 20,
            Code::StrayChar => 21,
            Code::BadEscape => 22,
            Code::Expected => 30,
            Code::EmptyBody => 31,
            Code::NotYet => 40,
            Code::NoMain => 50,
            Code::Unknown => 51,
            Code::Twice => 52,
            Code::Endless => 53,
        }
    }

    /// `T0012`: what a person searches for, and what the bench expects.
    pub fn label(self) -> String {
        format!("T{:04}", self.number())
    }
}

/// One NO, with its four parts and where it happened (1-based, as an editor
/// counts: the person reading is who counts here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub code: Code,
    pub what: String,
    pub line: usize,
    pub col: usize,
    pub why: String,
    pub how: String,
}

impl Message {
    pub fn new(code: Code, line: usize, col: usize, what: &str, why: &str, how: &str) -> Message {
        Message { code, what: what.into(), line, col, why: why.into(), how: how.into() }
    }

    /// The four parts, with the source line and a caret under the spot.
    pub fn render(&self, file: &str, src: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "{}  {}", self.code.label(), file);
        let _ = writeln!(out, "QUE      {}", self.what);
        let _ = writeln!(out, "DONDE    linea {}, columna {}", self.line, self.col);
        if let Some(text) = src.lines().nth(self.line.saturating_sub(1)) {
            let _ = writeln!(out, "         | {}", text);
            let _ = writeln!(out, "         | {}^", " ".repeat(self.col.saturating_sub(1)));
        }
        let _ = writeln!(out, "POR QUE  {}", self.why);
        let _ = writeln!(out, "COMO     {}", self.how);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_has_its_own_number_and_they_do_not_move() {
        let numbers: Vec<u16> = Code::ALL.iter().map(|c| c.number()).collect();
        assert_eq!(numbers, [1, 10, 12, 20, 21, 22, 30, 31, 40, 50, 51, 52, 53]);
        assert_eq!(Code::BadIndent.label(), "T0012");
    }

    #[test]
    fn the_four_parts_are_all_there_with_the_finger_under_the_spot() {
        let m = Message::new(Code::NotYet, 2, 5, "que", "por que", "como");
        let r = m.render("hola.titan", "mod main \"x\"\n    let x = 1\n");
        assert!(r.contains("QUE      que") && r.contains("POR QUE  por que") && r.contains("COMO     como"));
        assert!(r.contains("|     let x = 1\n") && r.contains("|     ^\n"), "{}", r);
    }
}
