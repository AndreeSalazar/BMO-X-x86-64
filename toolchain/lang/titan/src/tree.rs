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
//!    level 3   `if` / `else`: a body splits in two; `true`, `false`, the
//!              comparisons and `and` `or` `not`: a value that is YES or NO
//!    level 4   `while`, `for NAME in range(...)`, `break`, `continue`: a
//!              body that REPEATS
//!    level 5   `fn f(a: int) -> int` and `return`: a function that takes
//!              values and gives one back -- and a call is a value
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
    /// `(a: int, b: text)` (level 5).
    pub params: Vec<Param>,
    /// `-> int`: what it gives back; `None`, nothing (level 5).
    pub ret: Option<Ty>,
    pub body: Vec<Stmt>,
}

/// The classes of value a parameter or a result can say, in level 5. `f32`,
/// `dec` and the tables arrive with the types (level 6): they are TYPES, not
/// words, and spend no ceiling (TITAN_MAESTRO 14.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Int,
    Text,
    Bool,
}

impl Ty {
    pub fn name(self) -> &'static str {
        match self {
            Ty::Int => "int",
            Ty::Text => "text",
            Ty::Bool => "bool",
        }
    }
}

/// `n: int`: a value the caller gives. It does not change (to lend one so the
/// function changes it is `mut`, level 7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Ty,
    pub line: usize,
    pub col: usize,
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
    /// `if COND` and its block, and maybe `else` and its block. An
    /// `else if` is an `else` whose block is one `if` (level 3).
    If(If),
    /// `while COND` and its block (level 4).
    While(While),
    /// `for NAME in range(TO)` or `range(FROM, TO)` and its block (level 4).
    For(For),
    /// `break`: out of the innermost loop (level 4).
    Break { line: usize, col: usize },
    /// `continue`: to the next turn of the innermost loop (level 4).
    Continue { line: usize, col: usize },
    /// `return` or `return VALUE` (level 5).
    Return { value: Option<Expr>, line: usize, col: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct While {
    pub cond: Expr,
    pub line: usize,
    pub col: usize,
    pub body: Vec<Stmt>,
}

/// `for i in range(from, to)`: `i` takes `from`, `from + 1` ... `to - 1`, a
/// NEW value each turn -- it is not a `mut`, and nobody else may change it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct For {
    pub var: String,
    pub var_at: (usize, usize),
    /// `range(n)` is `range(0, n)`.
    pub from: Expr,
    pub to: Expr,
    pub line: usize,
    pub col: usize,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct If {
    pub cond: Expr,
    pub line: usize,
    pub col: usize,
    pub then: Vec<Stmt>,
    /// Empty when there is no `else`.
    pub other: Vec<Stmt>,
    /// Where the `else` is (line, column); (0, 0) when there is none.
    pub else_at: (usize, usize),
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
    /// `true` / `false` (level 3).
    Bool { value: bool, line: usize, col: usize },
    /// `a + b`: `op` is one of `+ - * / %`, a comparison `== != < <= > >=`
    /// or `and` / `or` (level 3).
    Bin { op: &'static str, left: Box<Expr>, right: Box<Expr>, line: usize, col: usize },
    /// `-a`.
    Neg { value: Box<Expr>, line: usize, col: usize },
    /// `not a` (level 3).
    Not { value: Box<Expr>, line: usize, col: usize },
    /// `doble(3)`: what a function gives back (level 5).
    Call { callee: String, args: Vec<Expr>, line: usize, col: usize },
}

impl Expr {
    pub fn at(&self) -> (usize, usize) {
        match self {
            Expr::Int { line, col, .. }
            | Expr::Text { line, col, .. }
            | Expr::Name { line, col, .. }
            | Expr::Bool { line, col, .. }
            | Expr::Bin { line, col, .. }
            | Expr::Neg { line, col, .. }
            | Expr::Not { line, col, .. }
            | Expr::Call { line, col, .. } => (*line, *col),
        }
    }

    /// As it would be written again: for `titan arbol`.
    pub fn show(&self) -> String {
        match self {
            Expr::Int { value, .. } => value.to_string(),
            Expr::Text { value, .. } => format!("{:?}", value),
            Expr::Name { name, .. } => name.clone(),
            Expr::Bool { value, .. } => value.to_string(),
            Expr::Bin { op, left, right, .. } => format!("({} {} {})", left.show(), op, right.show()),
            Expr::Neg { value, .. } => format!("-{}", value.show()),
            Expr::Not { value, .. } => format!("not {}", value.show()),
            Expr::Call { callee, args, .. } => {
                let a: Vec<String> = args.iter().map(Expr::show).collect();
                format!("{}({})", callee, a.join(", "))
            }
        }
    }
}

impl Stmt {
    pub fn line(&self) -> usize {
        match self {
            Stmt::Call(c) => c.line,
            Stmt::Let(l) | Stmt::Set(l) => l.line,
            Stmt::If(i) => i.line,
            Stmt::While(w) => w.line,
            Stmt::For(f) => f.line,
            Stmt::Break { line, .. } | Stmt::Continue { line, .. } | Stmt::Return { line, .. } => *line,
        }
    }
}

impl Program {
    /// The tree as text, for `titan arbol`: what the frontend understood.
    pub fn show(&self) -> String {
        let mut s = format!("mod {}  \"{}\"\n", self.module, self.purpose);
        for f in &self.functions {
            let params: Vec<String> = f.params.iter().map(|p| format!("{}: {}", p.name, p.ty.name())).collect();
            let ret = f.ret.map(|t| format!(" -> {}", t.name())).unwrap_or_default();
            s += &format!("{:<44}linea {}\n", format!("  fn {}({}){}", f.name, params.join(", "), ret), f.line);
            show_body(&f.body, 1, &mut s);
        }
        s
    }
}

fn show_body(body: &[Stmt], depth: usize, s: &mut String) {
    let pad = "    ".repeat(depth);
    for st in body {
        let text = match st {
            Stmt::Call(c) => {
                let args: Vec<String> = c.args.iter().map(Expr::show).collect();
                format!("{}{}({})", pad, c.callee, args.join(", "))
            }
            Stmt::Let(l) => format!("{}let {}{} = {}", pad, if l.mutable { "mut " } else { "" }, l.name, l.value.show()),
            Stmt::Set(l) => format!("{}{} = {}", pad, l.name, l.value.show()),
            Stmt::If(i) => format!("{}if {}", pad, i.cond.show()),
            Stmt::While(w) => format!("{}while {}", pad, w.cond.show()),
            Stmt::For(f) => format!("{}for {} in range({}, {})", pad, f.var, f.from.show(), f.to.show()),
            Stmt::Break { .. } => format!("{}break", pad),
            Stmt::Continue { .. } => format!("{}continue", pad),
            Stmt::Return { value: Some(v), .. } => format!("{}return {}", pad, v.show()),
            Stmt::Return { value: None, .. } => format!("{}return", pad),
        };
        *s += &format!("{:<44}linea {}\n", text, st.line());
        match st {
            Stmt::If(i) => {
                show_body(&i.then, depth + 1, s);
                if !i.other.is_empty() {
                    *s += &format!("{}else\n", pad);
                    show_body(&i.other, depth + 1, s);
                }
            }
            Stmt::While(w) => show_body(&w.body, depth + 1, s),
            Stmt::For(f) => show_body(&f.body, depth + 1, s),
            _ => {}
        }
    }
}
