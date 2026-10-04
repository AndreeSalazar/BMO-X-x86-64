//! `tree` -- the shape of a program. Zero decisions: `parse` builds it,
//! `check` judges it, and nothing here knows either of them.
//!
//! Each level of the ladder (TITAN_MAESTRO 14.14) grows this file by what it
//! brings, and nothing else:
//!
//! ```text
//!    level 0   a module that says what it does, functions, calls with texts
//!    level 1   `let`: a name for a value, and values that are calculated
//!    level 2   `mut`: a value that changes -- `let mut n = 0`, `n = n + 1`
//! ```

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
    pub body: Vec<Stmt>,
}

/// One line of a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `print("hola")`, `greet()`.
    Call(Call),
    /// `let area = 3 * 4`.
    Let(Let),
    /// `area = 5`: written now, judged by the checker (`juez.rs`) -- a value
    /// without `mut` does not change, and `mut` arrives in level 2.
    Set(Let),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub callee: String,
    pub line: usize,
    pub col: usize,
    pub args: Vec<Expr>,
}

/// `let [mut] NAME = VALUE` (and `NAME = VALUE`, the same shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Let {
    pub name: String,
    /// `let mut`: it may change later. Always false for `NAME = VALUE`.
    pub mutable: bool,
    pub line: usize,
    pub col: usize,
    pub value: Expr,
}

/// A value, with where it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int { value: i64, line: usize, col: usize },
    Text { value: String, line: usize, col: usize },
    Name { name: String, line: usize, col: usize },
    /// `a + b`: `op` is one of `+ - * / %`.
    Bin { op: char, left: Box<Expr>, right: Box<Expr>, line: usize, col: usize },
    /// `-a`.
    Neg { value: Box<Expr>, line: usize, col: usize },
}

impl Expr {
    pub fn at(&self) -> (usize, usize) {
        match self {
            Expr::Int { line, col, .. }
            | Expr::Text { line, col, .. }
            | Expr::Name { line, col, .. }
            | Expr::Bin { line, col, .. }
            | Expr::Neg { line, col, .. } => (*line, *col),
        }
    }

    /// As it would be written again: for `titan arbol`.
    pub fn show(&self) -> String {
        match self {
            Expr::Int { value, .. } => value.to_string(),
            Expr::Text { value, .. } => format!("{:?}", value),
            Expr::Name { name, .. } => name.clone(),
            Expr::Bin { op, left, right, .. } => format!("({} {} {})", left.show(), op, right.show()),
            Expr::Neg { value, .. } => format!("-{}", value.show()),
        }
    }
}

impl Stmt {
    pub fn line(&self) -> usize {
        match self {
            Stmt::Call(c) => c.line,
            Stmt::Let(l) | Stmt::Set(l) => l.line,
        }
    }
}

impl Program {
    /// The tree as text, for `titan arbol`: what the frontend understood.
    pub fn show(&self) -> String {
        let mut s = format!("mod {}  \"{}\"\n", self.module, self.purpose);
        for f in &self.functions {
            s += &format!("{:<44}linea {}\n", format!("  fn {}()", f.name), f.line);
            for st in &f.body {
                let text = match st {
                    Stmt::Call(c) => {
                        let args: Vec<String> = c.args.iter().map(Expr::show).collect();
                        format!("    {}({})", c.callee, args.join(", "))
                    }
                    Stmt::Let(l) => format!("    let {}{} = {}", if l.mutable { "mut " } else { "" }, l.name, l.value.show()),
                    Stmt::Set(l) => format!("    {} = {}", l.name, l.value.show()),
                };
                s += &format!("{:<44}linea {}\n", text, st.line());
            }
        }
        s
    }
}
