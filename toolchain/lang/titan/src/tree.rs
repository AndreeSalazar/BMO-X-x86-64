//! `tree` -- the shape of a program. Zero decisions: `parse` builds it,
//! `check` judges it, and nothing here knows either of them.
//!
//! Level 0 is small on purpose: a module that says what it does, functions,
//! and calls with texts. Each level of the ladder (TITAN_MAESTRO 14.14) will
//! grow this file by what it brings, and nothing else.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// The header: `mod NAME "purpose"` (U3).
    pub module: String,
    pub purpose: String,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub line: usize,
    pub col: usize,
    pub body: Vec<Call>,
}

/// A call on its own line: `print("hola")`, `greet()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub callee: String,
    pub line: usize,
    pub col: usize,
    pub args: Vec<String>,
}

impl Program {
    /// The tree as text, for `titan arbol`: what the frontend understood.
    pub fn show(&self) -> String {
        let mut s = format!("mod {}  \"{}\"\n", self.module, self.purpose);
        for f in &self.functions {
            s += &format!("{:<44}linea {}\n", format!("  fn {}()", f.name), f.line);
            for c in &f.body {
                let args: Vec<String> = c.args.iter().map(|a| format!("{:?}", a)).collect();
                s += &format!("{:<44}linea {}\n", format!("    {}({})", c.callee, args.join(", ")), c.line);
            }
        }
        s
    }
}
