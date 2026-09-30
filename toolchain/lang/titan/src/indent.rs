//! `indent` -- the rule of the margin, apart from everything else.
//!
//! The only part of the lexer with STATE (a stack of open margins), and the
//! exact place where languages with blocks by indentation break. Apart, the
//! whole rule reads at once and is tested without a lexer: in go widths, out
//! come "in", "same" or "out N".
//!
//! ```text
//!    one level .......... FOUR spaces, exactly   (the same as INTI)
//!    a tab .............. T0010, the lexer refuses it before it gets here
//!    not a multiple of 4,
//!    two levels at once,
//!    or back to a margin
//!    nobody opened ...... T0012
//! ```

use crate::message::{Code, Message};

pub const WIDTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Same,
    In,
    /// Closes this many levels.
    Out(usize),
}

#[derive(Debug, Clone)]
pub struct Indenter {
    /// Open margins, always growing. The 0 is there from the start and never
    /// leaves: it is the margin of the file.
    levels: Vec<usize>,
}

impl Default for Indenter {
    fn default() -> Self {
        Indenter { levels: vec![0] }
    }
}

impl Indenter {
    /// The margin of a line that carries something.
    pub fn line(&mut self, width: usize, line: usize) -> Result<Change, Message> {
        let now = *self.levels.last().unwrap_or(&0);
        let bad = |why: String| {
            Message::new(
                Code::BadIndent,
                line,
                width + 1,
                "la sangria no cuadra",
                &why,
                "sangra de 4 en 4 espacios, un nivel cada vez, y vuelve solo a un margen que ya se abrio",
            )
        };
        if !width.is_multiple_of(WIDTH) {
            return Err(bad(format!("esta linea empieza con {} espacios, y un nivel son {}", width, WIDTH)));
        }
        if width == now {
            return Ok(Change::Same);
        }
        if width > now {
            if width != now + WIDTH {
                return Err(bad(format!("salta de {} a {} espacios: una sangria entra UN nivel", now, width)));
            }
            self.levels.push(width);
            return Ok(Change::In);
        }
        let mut out = 0;
        while *self.levels.last().unwrap_or(&0) > width {
            self.levels.pop();
            out += 1;
        }
        if *self.levels.last().unwrap_or(&0) != width {
            return Err(bad(format!("vuelve a {} espacios, y ningun bloque se abrio ahi", width)));
        }
        Ok(Change::Out(out))
    }

    /// The levels still open at the end of the file.
    pub fn open(&self) -> usize {
        self.levels.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_same_out_and_out_of_several() {
        let mut i = Indenter::default();
        assert_eq!(i.line(0, 1).unwrap(), Change::Same);
        assert_eq!(i.line(4, 2).unwrap(), Change::In);
        assert_eq!(i.line(8, 3).unwrap(), Change::In);
        assert_eq!(i.line(8, 4).unwrap(), Change::Same);
        assert_eq!(i.line(0, 5).unwrap(), Change::Out(2));
        assert_eq!(i.open(), 0);
    }

    #[test]
    fn what_does_not_add_up_is_t0012_with_its_line() {
        let mut i = Indenter::default();
        assert_eq!(i.line(3, 7).unwrap_err().code, Code::BadIndent);
        assert_eq!(i.line(8, 8).unwrap_err().line, 8);
        let mut j = Indenter::default();
        j.line(4, 1).unwrap();
        j.line(8, 2).unwrap();
        assert!(j.line(2, 3).is_err());
    }
}
