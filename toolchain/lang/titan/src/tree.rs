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
//!    level 6   the TYPES: `dec` (exact decimal, no float), tables `[int; 3]`
//!              with `a[i]` and `for x in a`, and `type Nave` with fields
//!    level 7   lend and give: `fn f(mut t: [int; 5])` called `f(mut t)`, and
//!              `take`; and COBOL's precision: `dec(7, 2)`, `let x: T = v`,
//!              and `round(x, 2)` -- the rounding is WRITTEN
//!    level 8   `enum` with cases that carry data, and `match`, which has to
//!              cover EVERY case (no `_` to hide one)
//!    level 9   a PACKAGE: the header says the children (`mod ship`) and the
//!              connections (`use ship`); `pub` says what is seen from
//!              outside; `ship.avanza()` calls into another module
//!    level 10  `trait Forma` says what a value KNOWS how to do (its fn, no
//!              body); `trait Forma for Circulo` is how Circulo does it; and
//!              `fn mide(f: Forma)` takes ANY value that does it
//! ```

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// The header: `mod NAME "purpose"` (U3).
    pub module: String,
    pub purpose: String,
    pub functions: Vec<Function>,
    /// `type Nave` and its fields (level 6).
    pub types: Vec<TypeDef>,
    /// `enum Forma` and its cases (level 8).
    pub enums: Vec<EnumDef>,
    /// `use ship, gpu`: the modules this one talks to (level 9).
    pub uses: Vec<Use>,
    /// `mod ship, rock` / `mod rules in "x/r.titan"`: its children (level 9).
    pub children: Vec<Child>,
    /// `trait Forma` and the fn it promises (level 10).
    pub traits: Vec<TraitDef>,
    /// `trait Forma for Circulo` and how Circulo keeps it (level 10).
    pub impls: Vec<Impl>,
}

/// `trait Forma` and, below it, what a value of it KNOWS how to do: each fn
/// with no body, its first value of the trait itself (level 10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDef {
    pub name: String,
    pub public: bool,
    pub line: usize,
    pub col: usize,
    pub methods: Vec<Sig>,
}

/// A fn without a body: what a trait promises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sig {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<Ty>,
    pub line: usize,
    pub col: usize,
}

/// `trait Forma for Circulo` and its fn: how the type keeps the trait (10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Impl {
    pub trait_name: String,
    pub ty: Ty,
    pub line: usize,
    pub col: usize,
    pub functions: Vec<Function>,
}

/// One name of a `use` line (level 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    pub name: String,
    pub line: usize,
    pub col: usize,
}

/// One child of a `mod` line of the header, and where it lives if the line
/// says (`mod rules in "reglas/juego.titan"`), the path from the package (9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Child {
    pub name: String,
    pub path: Option<String>,
    pub line: usize,
    pub col: usize,
}

/// `enum Forma` and, below it, one case per line: `Circulo(dec)`, `Nada`
/// (level 8). A case carries the values its parentheses say, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    pub name: String,
    /// `pub enum`: other modules may name it (level 9).
    pub public: bool,
    pub line: usize,
    pub col: usize,
    pub cases: Vec<Case>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    pub name: String,
    pub fields: Vec<Ty>,
    pub line: usize,
    pub col: usize,
}

/// `type Nave` and, below it, one field per line: `x: dec` (level 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDef {
    pub name: String,
    /// `pub type`: other modules may name it (level 9).
    pub public: bool,
    pub line: usize,
    pub col: usize,
    pub fields: Vec<Param>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    /// `pub fn`: other modules may call it (level 9).
    pub public: bool,
    pub line: usize,
    pub col: usize,
    /// `(a: int, b: text)` (level 5).
    pub params: Vec<Param>,
    /// `-> int`: what it gives back; `None`, nothing (level 5).
    pub ret: Option<Ty>,
    pub body: Vec<Stmt>,
}

/// The type of a parameter, a result or a field. Types are not words and
/// spend no ceiling (TITAN_MAESTRO 14.2).
///
/// ** `dec` and not `f32` on the CPU (the owner, 04-10: "evita la float,
/// siempre decimal"). A `dec` is an exact decimal: an integer and how many of
/// its digits are decimals, so `0.1 + 0.2` is `0.3` and money never rounds by
/// itself. The x86-64 DOES have floats in hardware; what it lacks is base 10,
/// and that is why a float gets `0.1` wrong. `f32` is the 3060's (`gpu fn`,
/// level 11), and it says so if asked for before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Int,
    Text,
    Bool,
    /// The exact decimal (level 6).
    Dec,
    /// `dec(7, 2)`: an exact decimal with its digits DECLARED -- 7 in all, 2
    /// of them decimals, COBOL's `PIC 9(5)V99` (level 7). A value that does not
    /// fit is a NO, never a silent cut.
    DecP(u32, u32),
    /// `[int; 3]`: a table of exactly that many (level 6).
    Table(Box<Ty>, usize),
    /// A `type` of the file, by name (level 6).
    Named(String),
}

impl Ty {
    pub fn name(&self) -> String {
        match self {
            Ty::Int => "int".into(),
            Ty::Text => "text".into(),
            Ty::Bool => "bool".into(),
            Ty::Dec => "dec".into(),
            Ty::DecP(p, sc) => format!("dec({}, {})", p, sc),
            Ty::Table(t, n) => format!("[{}; {}]", t.name(), n),
            Ty::Named(n) => n.clone(),
        }
    }
}

/// `n: int`: a value the caller gives -- copied, and only read. `mut n: T`:
/// LENT, the function changes the caller's own; `take n: T`: GIVEN, the caller
/// no longer has it (level 7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Ty,
    pub mode: Mode,
    pub line: usize,
    pub col: usize,
}

/// How a value goes to a function (level 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// A copy: the caller keeps its own, untouched.
    Copy,
    /// Lent to be changed: `mut`. The caller sees the changes when it returns.
    Mut,
    /// Given for good: `take`. The caller no longer has it.
    Take,
}

impl Mode {
    pub fn word(self) -> &'static str {
        match self {
            Mode::Copy => "",
            Mode::Mut => "mut",
            Mode::Take => "take",
        }
    }
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
    /// `a[i] = v`, `nave.x = v`, `a[i].x = v`: a part of a value changes
    /// (level 6). The whole value needs `mut`, as for `a = v`.
    SetAt { name: String, path: Vec<Step>, value: Expr, line: usize, col: usize },
    /// `match VALUE` and one arm per case (level 8).
    Match { value: Expr, arms: Vec<Arm>, line: usize, col: usize },
}

/// `Circulo(r)` and its block: the case, the names its data take in the
/// block, and the block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arm {
    pub case: String,
    pub binds: Vec<(String, usize, usize)>,
    pub body: Vec<Stmt>,
    pub line: usize,
    pub col: usize,
}

/// One step into a value: a cell of a table or a field of a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Index(Expr),
    Field(String, usize, usize),
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
    /// `for x in tabla` (level 6): the turns are its cells, and `from`/`to`
    /// say nothing.
    pub over: Option<Expr>,
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

/// `let [mut] NAME [: TYPE] = VALUE` (and `NAME = VALUE`, the same shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Let {
    pub name: String,
    /// `let precio: dec(7, 2) = ...`: the type DECLARED, as COBOL declares
    /// every field (level 7). Every value it ever takes must fit it.
    pub ty: Option<Ty>,
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
    /// `12.50`: an exact decimal -- the digits as an integer, and how many of
    /// them are decimals (level 6). Never a float.
    Dec { digits: i64, scale: u32, line: usize, col: usize },
    /// `[1, 2, 3]` (level 6).
    Table { items: Vec<Expr>, line: usize, col: usize },
    /// `[0; 10]`: ten cells of the same value (level 6).
    Repeat { item: Box<Expr>, count: usize, line: usize, col: usize },
    /// `a[i]` (level 6).
    Index { base: Box<Expr>, index: Box<Expr>, line: usize, col: usize },
    /// `nave.x` (level 6).
    Field { base: Box<Expr>, name: String, line: usize, col: usize },
    /// `Nave { x: 1.0, fuel: 12.50 }` (level 6).
    Record { name: String, fields: Vec<(String, Expr)>, line: usize, col: usize },
    /// `mut t` / `take t` as a value given to a call (level 7): the local
    /// itself is lent or given, not a copy of it.
    Lend { mode: Mode, name: String, line: usize, col: usize },
    /// `round(x, 2)`: the rounding WRITTEN, COBOL's `ROUNDED` (level 7).
    Round { value: Box<Expr>, digits: u32, line: usize, col: usize },
}

/// A decimal as it is written: `1250` with scale 2 is `12.50`.
pub fn show_dec(digits: i64, scale: u32) -> String {
    let neg = digits < 0;
    let abs = digits.unsigned_abs().to_string();
    let s = if scale == 0 {
        abs
    } else {
        let padded = format!("{:0>width$}", abs, width = scale as usize + 1);
        let cut = padded.len() - scale as usize;
        format!("{}.{}", &padded[..cut], &padded[cut..])
    };
    if neg { format!("-{}", s) } else { s }
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
            | Expr::Call { line, col, .. }
            | Expr::Dec { line, col, .. }
            | Expr::Table { line, col, .. }
            | Expr::Repeat { line, col, .. }
            | Expr::Index { line, col, .. }
            | Expr::Field { line, col, .. }
            | Expr::Record { line, col, .. }
            | Expr::Lend { line, col, .. }
            | Expr::Round { line, col, .. } => (*line, *col),
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
            Expr::Dec { digits, scale, .. } => show_dec(*digits, *scale),
            Expr::Table { items, .. } => format!("[{}]", items.iter().map(Expr::show).collect::<Vec<_>>().join(", ")),
            Expr::Repeat { item, count, .. } => format!("[{}; {}]", item.show(), count),
            Expr::Index { base, index, .. } => format!("{}[{}]", base.show(), index.show()),
            Expr::Field { base, name, .. } => format!("{}.{}", base.show(), name),
            Expr::Record { name, fields, .. } => {
                format!("{} {{ {} }}", name, fields.iter().map(|(k, v)| format!("{}: {}", k, v.show())).collect::<Vec<_>>().join(", "))
            }
            Expr::Lend { mode, name, .. } => format!("{} {}", mode.word(), name),
            Expr::Round { value, digits, .. } => format!("round({}, {})", value.show(), digits),
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
            Stmt::Break { line, .. } | Stmt::Continue { line, .. } | Stmt::Return { line, .. } | Stmt::SetAt { line, .. } | Stmt::Match { line, .. } => *line,
        }
    }
}

impl Program {
    /// The case called `name`: (its enum, its place in it). Case names are
    /// unique in the whole file (`check`), so a bare `Nada` says which (8).
    pub fn case(&self, name: &str) -> Option<(usize, usize)> {
        self.enums.iter().enumerate().find_map(|(e, d)| d.cases.iter().position(|c| c.name == name).map(|v| (e, v)))
    }

    /// The tree as text, for `titan arbol`: what the frontend understood.
    pub fn show(&self) -> String {
        let mut s = format!("mod {}  \"{}\"\n", self.module, self.purpose);
        for e in &self.enums {
            s += &format!("{:<43} linea {}\n", format!("  enum {}", e.name), e.line);
            for c in &e.cases {
                let f: Vec<String> = c.fields.iter().map(Ty::name).collect();
                s += &format!("{:<43} linea {}\n", format!("    {}{}", c.name, if f.is_empty() { String::new() } else { format!("({})", f.join(", ")) }), c.line);
            }
        }
        for t in &self.types {
            s += &format!("{:<43} linea {}\n", format!("  type {}", t.name), t.line);
            for fl in &t.fields {
                s += &format!("{:<43} linea {}\n", format!("    {}: {}", fl.name, fl.ty.name()), fl.line);
            }
        }
        for f in &self.functions {
            let params: Vec<String> = f.params.iter().map(|p| format!("{}{}{}: {}", p.mode.word(), if p.mode == Mode::Copy { "" } else { " " }, p.name, p.ty.name())).collect();
            let ret = f.ret.as_ref().map(|t| format!(" -> {}", t.name())).unwrap_or_default();
            s += &format!("{:<43} linea {}\n", format!("  fn {}({}){}", f.name, params.join(", "), ret), f.line);
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
            Stmt::For(f) => match &f.over {
                Some(t) => format!("{}for {} in {}", pad, f.var, t.show()),
                None => format!("{}for {} in range({}, {})", pad, f.var, f.from.show(), f.to.show()),
            },
            Stmt::Match { value, .. } => format!("{}match {}", pad, value.show()),
            Stmt::SetAt { name, path, value, .. } => format!("{}{}{} = {}", pad, name, path.iter().map(|st| match st {
                Step::Index(e) => format!("[{}]", e.show()),
                Step::Field(f, _, _) => format!(".{}", f),
            }).collect::<String>(), value.show()),
            Stmt::Break { .. } => format!("{}break", pad),
            Stmt::Continue { .. } => format!("{}continue", pad),
            Stmt::Return { value: Some(v), .. } => format!("{}return {}", pad, v.show()),
            Stmt::Return { value: None, .. } => format!("{}return", pad),
        };
        *s += &format!("{:<43} linea {}\n", text, st.line());
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
            Stmt::Match { arms, .. } => {
                for a in arms {
                    let b: Vec<&str> = a.binds.iter().map(|x| x.0.as_str()).collect();
                    *s += &format!("{}    {}{}\n", pad, a.case, if b.is_empty() { String::new() } else { format!("({})", b.join(", ")) });
                    show_body(&a.body, depth + 2, s);
                }
            }
            _ => {}
        }
    }
}
