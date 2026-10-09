//! `ir` -- THE IR OF TITAN++: what a program DOES, with no machine in it.
//!
//! TITAN_MAESTRO 6.8, L1: check on an IR of its OWN, with a graph of blocks,
//! not on the tree (the lesson of rustc). And the house forbids an IR shared
//! between languages (`toolchain/lang/cobol/cobol.md`: "un cerebro
//! compartido"), so this is not INTI's: INTI has its `ir/` and goes on alone,
//! toward precision; TITAN++ has this one, toward building.
//!
//! ```text
//!    tree   what was WRITTEN: names, lines, texts as the author typed them
//!    ir     what it DOES: functions by number, LOCALS by number, blocks,
//!           and what each operation reads, defines and asks of the system
//! ```
//!
//! ** NO MACHINE. This file never says "register", "syscall" or "x86": the
//! emitter (`emisor-x86_64/`, its own crate) is the only one that may. A test
//! below reads this file and fails if it names one.
//!
//! ** WHO READS IT, in order:
//!
//! ```text
//!    juez    the borrow checker (`juez.rs`): every local, at every point,
//!            is born, alive, (later) lent or given away -- and each use is
//!            judged against that. On THIS IR, function by function
//!    calc    what is known when compiling is calculated when compiling
//!            (`calc.rs`): exact, or a NO (overflow, /0, a division that
//!            is not whole, a text added to a number)
//!    emitter the bytes
//! ```
//!
//! Levels 1 and 2 kept a function as ONE block. Level 3 splits it: an `if`
//! ENDS its block with a `Branch`, each side is a block of its own, and both
//! `Jump` to the block that follows. The blocks go in the order a reader
//! meets them, and every jump goes DOWN (no loops until level 4): whoever
//! walks them in order has seen every way into a block before entering it.
//!
//! ```text
//!    b0   ... the lines before the if
//!         si (vidas > 0) -> b1, sino -> b2
//!    b1   ... the if's block           muere %3   (what was born in it)
//!         salta b3
//!    b2   ... the else's block
//!         salta b3
//!    b3   ... the lines after
//! ```
//!
//! ** `Drop`: where a block-scoped value DIES -- what rustc's MIR calls
//! `StorageDead`. Something born inside an `if` lives in its block and dies
//! when the block closes; the checker reads the `Drop` and says so if anyone
//! reaches for it later (T0058). Today it is a line of the IR; with `take`
//! (level 7) it is the point where the value is given back.

use crate::tree::{EnumDef, Expr, Mode, Program, Stmt, TraitDef, Ty, TypeDef};

mod biblioteca;
mod muestra;

/// A whole module, ready to emit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// The header (U3): it travels to the `.bex` manifest.
    pub name: String,
    pub purpose: String,
    pub functions: Vec<Function>,
    /// Where the program starts: the index of `main` in `functions`.
    pub entry: usize,
    /// The `type`s of the file, as written (level 6).
    pub types: Vec<TypeDef>,
    /// The `enum`s of the file, as written (level 8).
    pub enums: Vec<EnumDef>,
    /// The `trait`s (level 10), and which type keeps which: (trait, type).
    pub traits: Vec<TraitDef>,
    pub impls: Vec<(String, String)>,
    /// What the package's `Titan.toml` asks for (U2): it travels to the
    /// `.bex` manifest. Set by `lower_package`; nothing for a file alone.
    pub permissions: bmo_titan_contrato::Permissions,
    /// The package's map of lines: every `At` here is package-wide, and this
    /// turns it back into its file and its own line (level 11, G2+).
    pub sources: crate::paquete::Sources,
    /// Set by the calculation (`calc.rs`, level 4): the WHOLE program, run
    /// when compiling -- what it writes, in order, every value already a
    /// constant. Until something comes from outside, this is all the program
    /// does, and it is what the emitter writes. `None` before `calc`.
    pub flat: Option<Vec<Op>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    /// The line of its `fn`, so a NO from below can still point at the text.
    pub line: usize,
    /// Every name the body uses for a value, numbered in order of first use.
    /// The parameters come first, in their order.
    pub locals: Vec<Local>,
    /// Its parameters: the local each one is, and its class (level 5).
    pub params: Vec<(usize, Ty)>,
    /// How each parameter arrives: copied, lent (`mut`) or given (`take`)
    /// (level 7), in the order of `params`.
    pub modes: Vec<Mode>,
    /// What it gives back, if anything (level 5).
    pub ret: Option<Ty>,
    pub blocks: Vec<Block>,
    /// A `gpu fn` (level 11): it counts in f32, one cell per thread.
    pub gpu: bool,
    /// LB5: the WORK of one cell of a `gpu fn`, counted when compiling
    /// (`gpu::obra`): with every `range` written, it is the same in every
    /// cell. 0 for a fn of the CPU. The writer's battery is measured with it.
    pub obra: u64,
    /// A fn of a TRAIT (level 10): it has no body of its own, and a call to
    /// it runs the fn of the type of its first value -- (type, function).
    /// `None` for every fn with a body.
    pub dispatch: Option<Vec<(String, usize)>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub ops: Vec<Op>,
    pub end: End,
    /// Set by the calculation (`calc.rs`): no run of the program reaches
    /// this block -- the `if` that leads here was decided the other way when
    /// compiling. The emitter writes no byte for it, and the certificate
    /// names no door from it: what never runs asks for nothing.
    pub dead: bool,
}

/// Where in the text: line and column, as an editor counts.
pub type At = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// The local gets its value: `let x = ...`; `mutable` if `let mut`; `ty`
    /// if the type is declared (`let x: dec(7, 2) = ...`, level 7).
    Let { local: usize, value: Value, mutable: bool, ty: Option<Ty>, at: At },
    /// The local is given a new value: `x = ...` (needs `mut`, level 2).
    Set { local: usize, value: Value, at: At },
    /// Write these values on the console, one after another, and end the
    /// line: `print(...)`. The one door of BMO-X level 1 uses.
    Write { parts: Vec<Value>, at: At },
    /// Call the function with this index, with these values (its result, if
    /// it has one, is dropped: a call on its own line).
    Call { func: usize, args: Vec<Value>, at: At },
    /// The block that gave birth to this local closes here: it dies. `at` is
    /// the `if` (or `else`) whose block closes.
    Drop { local: usize, at: At },
    /// A PART of the local changes: `a[i] = v`, `nave.x = v` (level 6). Like
    /// `Set`, it needs `mut`.
    SetAt { local: usize, path: Vec<PathStep>, value: Value, at: At },
    /// A call to the DIRECTOR on its own line (LB7b): `director.espera(16)`,
    /// or one whose yes/no nobody keeps.
    Director { what: Director, args: Vec<Value>, at: At },
}

/// One step into a value, in the IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStep {
    Index(Value),
    Field(String, At),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Int(i64, At),
    Text(String, At),
    /// What the local holds at that point.
    Local(usize, At),
    /// `true` / `false`.
    Bool(bool, At),
    /// `+ - * / %`, `== != < <= > >=`, `and` `or`.
    Bin(&'static str, Box<Value>, Box<Value>, At),
    Neg(Box<Value>, At),
    Not(Box<Value>, At),
    /// What the function with this index gives back for these values (5).
    Call(usize, Vec<Value>, At),
    /// An exact decimal: the digits and how many are decimals (6).
    Dec(i64, u32, At),
    /// `[a, b, c]` (6).
    Table(Vec<Value>, At),
    /// `[v; n]` (6).
    Repeat(Box<Value>, usize, At),
    /// `a[i]` (6).
    Index(Box<Value>, Box<Value>, At),
    /// `nave.x` (6).
    Field(Box<Value>, String, At),
    /// A record of the type with this index, its fields IN THE ORDER the
    /// `type` declares them (6).
    Record(usize, Vec<Value>, At),
    /// `len(a)`: how many cells (6).
    Len(Box<Value>, At),
    /// The local itself, lent (`mut`) or given (`take`) to a call (7).
    Lend(Mode, usize, At),
    /// `round(x, n)`: the rounding WRITTEN (7).
    Round(Box<Value>, u32, At),
    /// An f32, by its bits (level 11): every number written inside a `gpu
    /// fn`, and what `let xs: [f32; n] = ...` turns a `dec` into.
    F32(u32, At),
    /// The case `v` of the enum `e`, carrying these values (8): `Circulo(2.0)`,
    /// `Nada`.
    Variant(usize, usize, Vec<Value>, At),
    /// Is this value the case `v` of the enum `e`? A yes/no: the question
    /// each arm of a `match` asks (8).
    Is(Box<Value>, usize, usize, At),
    /// The value number `k` the case `v` of the enum `e` carries: what a
    /// `Circulo(r)` of a `match` names `r` (8). Only read where `Is` said yes.
    Payload(Box<Value>, usize, usize, usize, At),
    /// `numero(t)`: the case of `enum Numero` (the prelude, `prelude.rs`) --
    /// `Es(n)` if the text is a whole number, `NoEs` if not. The enum's index.
    Number(Box<Value>, usize, At),
    /// `lee()`: the line typed on the program's own console, as a text (E1,
    /// `docs/plan/PLAN_LA_ENTRADA.md`). Known only WHEN IT RUNS: a module
    /// that reads is not run when compiling (`calc.rs`), it is emitted.
    Read(At),
    /// What the library of lists and maps does to its values (level 13,
    /// `docs/plan/PLAN_LISTAS_Y_MAPAS.md`). The ones that CHANGE a list or a
    /// map come as `l = push(l, x)`: an `Op::Set`, so the checker already
    /// knows `l` needs `mut` and that it changed.
    Lib(Lib, Vec<Value>, At),
    /// What the DIRECTOR answers (LB7b): `director.lamina(18)`, a yes/no
    /// known only when the program runs.
    Director(Director, Vec<Value>, At),
    /// `{"ana": 3}`: a map written, its keys and values in order (13).
    Map(Vec<(Value, Value)>, At),
}

/// The library of lists and maps (level 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lib {
    /// The list with one more at its end: `push(mut l, x)`.
    Push,
    /// Its last, as a case -- `Hay(x)` or `NoHay`: what `pop` gives.
    Last,
    /// The list without its last: what `pop` leaves (empty stays empty).
    DropLast,
    /// The map with this key leading to this value: `put(mut m, k, v)`.
    Put,
    /// The map without this key: `remove(mut m, k)`.
    Remove,
    /// `get(m, k)`: `Hay(v)` or `NoHay`.
    Get,
    /// `has(m, k)`: is the key there?
    Has,
    /// Turn `i` of a `for x in ...`: the cell of a table or a list, the KEY
    /// of a map (in the order they went in, D4).
    Turn,
}

/// ** THE DIRECTOR (LB7b of `docs/plan/PLAN_LAS_LIBRERIAS.md`, 09-10): what
/// a program with `use director` -- and `screen` in its Titan.toml (DL7) --
/// asks of the screen of BMO-X. VERRANO's LAMINA
/// (`platform/shared/verrano/src/lamina.rs`), the way INTI's runtime writes
/// it (`verrano.inti`): the program counts the vertices of each frame and
/// publishes them; the desktop draws them, and nobody waits for anybody.
/// Who composes, and when, is known only WHEN IT RUNS: a program that talks
/// to the director is emitted (E1), never run when compiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Director {
    /// `director.lamina(capacidad)`: the block for `capacidad` vertices per
    /// slot, its header written, OFFERED to whoever launched the program.
    /// `true` if someone took it; `false` if nobody composes (launched from
    /// the shell), or if the program already has its lamina.
    Lamina,
    /// `director.publica(fotograma, vertices, posiciones, colores)`: the first
    /// `vertices` vertices of the tables (four f32 of position and four of
    /// color each), in the slot nobody is reading, with its seal; and the
    /// sequence LAST -- that is the publishing. `false` if there is no
    /// lamina, or they are not whole triangles, or they do not fit.
    Publica,
    /// `director.espera(ms)`: sleep until the next frame.
    Espera,
}

impl Director {
    pub const ALL: [Director; 3] = [Director::Lamina, Director::Publica, Director::Espera];

    /// Its whole name, as a program writes it.
    pub fn name(self) -> &'static str {
        match self {
            Director::Lamina => "director.lamina",
            Director::Publica => "director.publica",
            Director::Espera => "director.espera",
        }
    }

    /// The director's fn with this whole name.
    pub fn of(name: &str) -> Option<Director> {
        Director::ALL.into_iter().find(|d| d.name() == name)
    }

    /// How many values it takes.
    pub fn takes(self) -> usize {
        match self {
            Director::Lamina | Director::Espera => 1,
            Director::Publica => 4,
        }
    }

    /// Whether it gives a value back: a yes/no.
    pub fn gives(self) -> bool {
        !matches!(self, Director::Espera)
    }
}

impl Lib {
    pub fn name(self) -> &'static str {
        match self {
            Lib::Push => "push",
            Lib::Last => "ultimo",
            Lib::DropLast => "sin_ultimo",
            Lib::Put => "put",
            Lib::Remove => "remove",
            Lib::Get => "get",
            Lib::Has => "has",
            Lib::Turn => "vuelta",
        }
    }
}

impl Value {
    pub fn at(&self) -> At {
        match self {
            Value::Int(_, a)
            | Value::Text(_, a)
            | Value::Bool(_, a)
            | Value::Local(_, a)
            | Value::Bin(_, _, _, a)
            | Value::Neg(_, a)
            | Value::Not(_, a)
            | Value::Call(_, _, a)
            | Value::Dec(_, _, a)
            | Value::Table(_, a)
            | Value::Repeat(_, _, a)
            | Value::Index(_, _, a)
            | Value::Field(_, _, a)
            | Value::Record(_, _, a)
            | Value::Len(_, a)
            | Value::Lend(_, _, a)
            | Value::Round(_, _, a)
            | Value::F32(_, a)
            | Value::Variant(_, _, _, a)
            | Value::Is(_, _, _, a)
            | Value::Payload(_, _, _, _, a)
            | Value::Number(_, _, a)
            | Value::Lib(_, _, a)
            | Value::Map(_, a)
            | Value::Director(_, _, a)
            | Value::Read(a) => *a,
        }
    }

    /// Every local this value reads, in reading order: what the checker
    /// judges.
    pub fn reads(&self, out: &mut Vec<(usize, At)>) {
        match self {
            Value::Int(..) | Value::Text(..) | Value::Bool(..) | Value::Dec(..) | Value::F32(..) | Value::Read(..) => {}
            Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) => {
                for i in items {
                    i.reads(out);
                }
            }
            Value::Repeat(v, _, _) | Value::Field(v, _, _) | Value::Len(v, _) | Value::Round(v, _, _) | Value::Is(v, _, _, _) | Value::Payload(v, _, _, _, _) | Value::Number(v, _, _) => v.reads(out),
            Value::Lend(_, l, a) => out.push((*l, *a)),
            Value::Index(b, i, _) => {
                b.reads(out);
                i.reads(out);
            }
            Value::Local(l, a) => out.push((*l, *a)),
            Value::Bin(_, l, r, _) => {
                l.reads(out);
                r.reads(out);
            }
            Value::Neg(v, _) | Value::Not(v, _) => v.reads(out),
            Value::Call(_, args, _) | Value::Lib(_, args, _) | Value::Director(_, args, _) => {
                for a in args {
                    a.reads(out);
                }
            }
            Value::Map(items, _) => {
                for (k, v) in items {
                    k.reads(out);
                    v.reads(out);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// Back to whoever called, with the value the function gives back if it
    /// has one. For `main`, the end of the program.
    Return(Option<Value>),
    /// On to that block.
    Jump(usize),
    /// `if`: to `then` if `cond` is true, to `other` if not. `at` is the `if`.
    Branch { cond: Value, then: usize, other: usize, at: At },
}

impl Value {
    /// Does this value come, even in part, from OUTSIDE (`lee()`, E1)?
    pub fn from_outside(&self) -> bool {
        match self {
            // What the director answers is known only when it runs (LB7b).
            Value::Read(_) | Value::Director(..) => true,
            Value::Int(..) | Value::Text(..) | Value::Bool(..) | Value::Dec(..) | Value::F32(..) | Value::Local(..) | Value::Lend(..) => false,
            Value::Bin(_, a, b, _) | Value::Index(a, b, _) => a.from_outside() || b.from_outside(),
            Value::Neg(a, _) | Value::Not(a, _) | Value::Repeat(a, _, _) | Value::Field(a, _, _) | Value::Len(a, _) | Value::Round(a, _, _) | Value::Is(a, _, _, _) | Value::Payload(a, _, _, _, _) | Value::Number(a, _, _) => a.from_outside(),
            Value::Call(_, items, _) | Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) | Value::Lib(_, items, _) => items.iter().any(Value::from_outside),
            Value::Map(items, _) => items.iter().any(|(k, v)| k.from_outside() || v.from_outside()),
        }
    }
}

impl Module {
    /// ** Does the program read from OUTSIDE anywhere (E1,
    /// `docs/plan/PLAN_LA_ENTRADA.md`)? Then it cannot be run when compiling
    /// -- what is typed is not known yet -- and it is EMITTED instead.
    pub fn reads_outside(&self) -> bool {
        let op = |o: &Op| match o {
            Op::Let { value, .. } | Op::Set { value, .. } => value.from_outside(),
            Op::Write { parts: items, .. } | Op::Call { args: items, .. } => items.iter().any(Value::from_outside),
            Op::SetAt { path, value, .. } => value.from_outside() || path.iter().any(|p| matches!(p, PathStep::Index(i) if i.from_outside())),
            Op::Drop { .. } => false,
            // Talking to the director: a program that publishes frames runs
            // (LB7b), it is not folded into what it writes.
            Op::Director { .. } => true,
        };
        let end = |e: &End| match e {
            End::Return(v) => v.as_ref().is_some_and(Value::from_outside),
            End::Branch { cond, .. } => cond.from_outside(),
            End::Jump(_) => false,
        };
        self.functions.iter().flat_map(|f| &f.blocks).any(|b| b.ops.iter().any(op) || end(&b.end))
    }
}

impl End {
    /// The blocks this end can go to.
    pub fn targets(&self) -> Vec<usize> {
        match self {
            End::Return(_) => Vec::new(),
            End::Jump(b) => vec![*b],
            End::Branch { then, other, .. } => vec![*then, *other],
        }
    }
}

fn local_of(locals: &mut Vec<Local>, name: &str) -> usize {
    match locals.iter().position(|l| l.name == name) {
        Some(i) => i,
        None => {
            locals.push(Local { name: name.to_string() });
            locals.len() - 1
        }
    }
}

/// The numbers written inside a `gpu fn`, made f32 (level 11): the exact
/// decimal, rounded ONCE to the nearest f32 -- the declaration `gpu fn` says
/// where the rounding is, as `dec(7, 2)` says the digits.
fn to_f32(v: &mut Value) {
    match v {
        Value::Int(n, at) => *v = Value::F32((*n as f32).to_bits(), *at),
        Value::Dec(d, s, at) => *v = Value::F32(crate::tree::show_dec(*d, *s).parse::<f32>().unwrap_or(f32::NAN).to_bits(), *at),
        Value::Bin(_, l, r, _) => {
            to_f32(l);
            to_f32(r);
        }
        Value::Neg(x, _) | Value::Not(x, _) => to_f32(x),
        // LB5: a call to another gpu fn, with numbers written in it.
        Value::Call(_, args, _) => args.iter_mut().for_each(to_f32),
        // LB6: a record of a gpu fn that draws, written field by field.
        Value::Record(_, items, _) => items.iter_mut().for_each(to_f32),
        _ => {}
    }
}

/// The function a call goes to: a fn of the program, or -- after them, in
/// order -- the fn of a trait (level 10).
fn func_index(p: &Program, name: &str) -> usize {
    p.functions.iter().position(|f| f.name == name).unwrap_or_else(|| {
        let k = p.traits.iter().flat_map(|t| &t.methods).position(|s| s.name == name).expect("check: every call goes somewhere");
        p.functions.len() + k
    })
}

fn value(e: &Expr, locals: &mut Vec<Local>, p: &Program) -> Value {
    let value = |e: &Expr, locals: &mut Vec<Local>| value(e, locals, p);
    match e {
        Expr::Int { value, line, col } => Value::Int(*value, (*line, *col)),
        Expr::Text { value, line, col } => Value::Text(value.clone(), (*line, *col)),
        // `Nada`: a case with nothing to carry is a value by its name (8).
        Expr::Name { name, line, col } => match p.case(name) {
            Some((e, v)) => Value::Variant(e, v, Vec::new(), (*line, *col)),
            None => Value::Local(local_of(locals, name), (*line, *col)),
        },
        Expr::Bool { value, line, col } => Value::Bool(*value, (*line, *col)),
        Expr::Not { value: v, line, col } => Value::Not(Box::new(value(v, locals)), (*line, *col)),
        Expr::Bin { op, left, right, line, col } => {
            Value::Bin(*op, Box::new(value(left, locals)), Box::new(value(right, locals)), (*line, *col))
        }
        Expr::Neg { value: v, line, col } => Value::Neg(Box::new(value(v, locals)), (*line, *col)),
        Expr::Call { callee, line, col, .. } if callee == "lee" => Value::Read((*line, *col)),
        // LB7b: what the director answers (the checker saw `use director`).
        Expr::Call { callee, args, line, col } if Director::of(callee).is_some() => {
            let d = Director::of(callee).expect("the guard");
            Value::Director(d, args.iter().map(|a| value(a, locals)).collect(), (*line, *col))
        }
        Expr::Call { callee, args, line, col } if callee == "numero" => {
            let e = p.enums.iter().position(|e| e.name == crate::prelude::NUMERO).expect("prelude: numero brings its enum");
            Value::Number(Box::new(value(&args[0], locals)), e, (*line, *col))
        }
        Expr::Call { callee, args, line, col } if callee == "len" => Value::Len(Box::new(value(&args[0], locals)), (*line, *col)),
        // `get(m, k)` / `has(m, k)` (level 13): they read, and change nothing.
        Expr::Call { callee, args, line, col } if (callee == "get" || callee == "has") && !p.functions.iter().any(|f| &f.name == callee) => {
            let lib = if callee == "get" { Lib::Get } else { Lib::Has };
            Value::Lib(lib, args.iter().map(|a| value(a, locals)).collect(), (*line, *col))
        }
        Expr::Map { items, line, col } => Value::Map(items.iter().map(|(k, v)| (value(k, locals), value(v, locals))).collect(), (*line, *col)),
        Expr::Call { callee, args, line, col } if p.case(callee).is_some() => {
            let (e, v) = p.case(callee).expect("the guard");
            Value::Variant(e, v, args.iter().map(|a| value(a, locals)).collect(), (*line, *col))
        }
        Expr::Call { callee, args, line, col } => {
            let func = func_index(p, callee);
            Value::Call(func, args.iter().map(|a| value(a, locals)).collect(), (*line, *col))
        }
        Expr::Dec { digits, scale, line, col } => Value::Dec(*digits, *scale, (*line, *col)),
        Expr::Lend { mode, name, line, col } => Value::Lend(*mode, local_of(locals, name), (*line, *col)),
        Expr::Round { value: v, digits, line, col } => Value::Round(Box::new(value(v, locals)), *digits, (*line, *col)),
        Expr::Table { items, line, col } => Value::Table(items.iter().map(|i| value(i, locals)).collect(), (*line, *col)),
        Expr::Repeat { item, count, line, col } => Value::Repeat(Box::new(value(item, locals)), *count, (*line, *col)),
        Expr::Index { base, index, line, col } => Value::Index(Box::new(value(base, locals)), Box::new(value(index, locals)), (*line, *col)),
        Expr::Field { base, name, line, col } => Value::Field(Box::new(value(base, locals)), name.clone(), (*line, *col)),
        Expr::Record { name, fields, line, col } => {
            let t = p.types.iter().position(|t| &t.name == name).expect("check: the type exists");
            // In the order the `type` declares: the author may write them in any.
            let ordered = p.types[t]
                .fields
                .iter()
                .map(|f| value(&fields.iter().find(|(k, _)| k == &f.name).expect("check: every field given").1, locals))
                .collect();
            Value::Record(t, ordered, (*line, *col))
        }
    }
}

/// A loop being lowered: where `break` and `continue` go, and how deep the
/// scopes were when its body opened (what a jump out of it must close).
struct Loop {
    /// Blocks that end in `break`: patched to the exit once it exists.
    breaks: Vec<usize>,
    /// Blocks that end in `continue`: patched to the next turn.
    conts: Vec<usize>,
    /// `scopes.len()` when the body's scope opened.
    depth: usize,
}

/// Lowers one function: its blocks grow as `if`s split them and loops turn.
struct Lowering<'p> {
    p: &'p Program,
    locals: Vec<Local>,
    blocks: Vec<Block>,
    /// The open blocks of the TEXT (the fn's body, an `if`, a loop): who opened
    /// it, and what was born in it so far. The fn's own body has no opener and
    /// closes with no `Drop`.
    scopes: Vec<(Option<At>, Vec<usize>)>,
    loops: Vec<Loop>,
    /// A number for each `for`, so its hidden locals never meet another's.
    hidden: usize,
}

impl Lowering<'_> {
    fn open(&mut self) -> usize {
        self.blocks.push(Block { ops: Vec::new(), end: End::Return(None), dead: false });
        self.blocks.len() - 1
    }

    fn index(&self, name: &str) -> usize {
        func_index(self.p, name)
    }

    fn open_scope(&mut self, opener: At) {
        self.scopes.push((Some(opener), Vec::new()));
    }

    /// The scope closes at the end of block `at`: what was born in it dies.
    fn close_scope(&mut self, at: usize) {
        let (opener, born) = self.scopes.pop().expect("a scope was opened");
        if let Some(opener) = opener {
            for local in born {
                self.blocks[at].ops.push(Op::Drop { local, at: opener });
            }
        }
    }

    fn born(&mut self, local: usize) {
        self.scopes.last_mut().expect("the fn's scope").1.push(local);
    }

    /// `break` / `continue` at the end of `at`: every scope of the loop's body
    /// (and deeper) closes on THIS path too, innermost first.
    fn leave(&mut self, at: usize) {
        let depth = self.loops.last().expect("parse: a jump is inside a loop").depth;
        for k in (depth..self.scopes.len()).rev() {
            if let Some(opener) = self.scopes[k].0 {
                for &local in &self.scopes[k].1.clone() {
                    self.blocks[at].ops.push(Op::Drop { local, at: opener });
                }
            }
        }
    }

    /// Lowers `body` in the current scope, starting in block `at`; returns
    /// the block where it ends.
    fn stmts(&mut self, body: &[Stmt], mut at: usize) -> usize {
        for st in body {
            match st {
                // `let x = pop(mut l)` (level 13): x gets the last of `l` as a
                // case, and then `l` loses it -- two lines of the IR.
                Stmt::Let(l) | Stmt::Set(l) if matches!(&l.value, Expr::Call { callee, .. } if callee == "pop") && !self.p.functions.iter().any(|f| f.name == "pop") => {
                    let (last, lent, here) = self.popped(&l.value).expect("check: pop takes `mut` and a list");
                    let local = local_of(&mut self.locals, &l.name);
                    if matches!(st, Stmt::Let(_)) {
                        self.born(local);
                        self.blocks[at].ops.push(Op::Let { local, value: last, mutable: l.mutable, ty: l.ty.clone(), at: (l.line, l.col) });
                    } else {
                        self.blocks[at].ops.push(Op::Set { local, value: last, at: (l.line, l.col) });
                    }
                    self.drop_last(at, lent, here);
                }
                Stmt::Let(l) => {
                    let v = value(&l.value, &mut self.locals, self.p);
                    let local = local_of(&mut self.locals, &l.name);
                    self.born(local);
                    self.blocks[at].ops.push(Op::Let { local, value: v, mutable: l.mutable, ty: l.ty.clone(), at: (l.line, l.col) });
                }
                Stmt::Set(l) => {
                    let v = value(&l.value, &mut self.locals, self.p);
                    let local = local_of(&mut self.locals, &l.name);
                    self.blocks[at].ops.push(Op::Set { local, value: v, at: (l.line, l.col) });
                }
                // `push(mut l, x)`, `put(mut m, k, v)`, `remove(mut m, k)` and
                // `pop(mut l)` alone (level 13): `l = push(l, x)` -- the
                // list CHANGES, as with any `mut` lent to a fn
                // (`ir/biblioteca.rs`).
                Stmt::Call(c) if matches!(c.callee.as_str(), "push" | "put" | "remove" | "pop") && !self.p.functions.iter().any(|f| f.name == c.callee) => self.mutator(c, at),
                // LB7b: the director, on its own line.
                Stmt::Call(c) if Director::of(&c.callee).is_some() => {
                    let what = Director::of(&c.callee).expect("the guard");
                    let args = c.args.iter().map(|a| value(a, &mut self.locals, self.p)).collect();
                    self.blocks[at].ops.push(Op::Director { what, args, at: (c.line, c.col) });
                }
                Stmt::Call(c) if c.callee == "print" => {
                    let parts = c.args.iter().map(|a| value(a, &mut self.locals, self.p)).collect();
                    self.blocks[at].ops.push(Op::Write { parts, at: (c.line, c.col) });
                }
                Stmt::Call(c) => {
                    let func = self.index(&c.callee);
                    let args = c.args.iter().map(|a| value(a, &mut self.locals, self.p)).collect();
                    self.blocks[at].ops.push(Op::Call { func, args, at: (c.line, c.col) });
                }
                Stmt::SetAt { name, path, value: v, line, col } => {
                    let v = value(v, &mut self.locals, self.p);
                    let path = path
                        .iter()
                        .map(|st| match st {
                            crate::tree::Step::Index(i) => PathStep::Index(value(i, &mut self.locals, self.p)),
                            crate::tree::Step::Field(f, l, c) => PathStep::Field(f.clone(), (*l, *c)),
                        })
                        .collect();
                    let local = local_of(&mut self.locals, name);
                    self.blocks[at].ops.push(Op::SetAt { local, path, value: v, at: (*line, *col) });
                }
                Stmt::Return { value: v, .. } => {
                    let v = v.as_ref().map(|e| value(e, &mut self.locals, self.p));
                    self.blocks[at].end = End::Return(v);
                    // What follows a `return` is never reached.
                    at = self.open();
                }
                Stmt::If(i) => {
                    let cond = value(&i.cond, &mut self.locals, self.p);
                    let here = (i.line, i.col);
                    let then = self.open();
                    self.open_scope(here);
                    let then_end = self.stmts(&i.then, then);
                    self.close_scope(then_end);
                    let (other, other_end) = if i.other.is_empty() {
                        (None, None)
                    } else {
                        let b = self.open();
                        self.open_scope(i.else_at);
                        let e = self.stmts(&i.other, b);
                        self.close_scope(e);
                        (Some(b), Some(e))
                    };
                    let join = self.open();
                    self.blocks[then_end].end = End::Jump(join);
                    if let Some(e) = other_end {
                        self.blocks[e].end = End::Jump(join);
                    }
                    self.blocks[at].end = End::Branch { cond, then, other: other.unwrap_or(join), at: here };
                    at = join;
                }
                Stmt::While(w) => {
                    let here = (w.line, w.col);
                    let head = self.open();
                    self.blocks[at].end = End::Jump(head);
                    let cond = value(&w.cond, &mut self.locals, self.p);
                    let body = self.open();
                    self.loops.push(Loop { breaks: Vec::new(), conts: Vec::new(), depth: self.scopes.len() });
                    self.open_scope(here);
                    let end = self.stmts(&w.body, body);
                    self.close_scope(end);
                    let lp = self.loops.pop().expect("pushed above");
                    // The jump UP: the next turn starts at the question.
                    self.blocks[end].end = End::Jump(head);
                    let exit = self.open();
                    self.blocks[head].end = End::Branch { cond, then: body, other: exit, at: here };
                    for b in lp.breaks {
                        self.blocks[b].end = End::Jump(exit);
                    }
                    for b in lp.conts {
                        self.blocks[b].end = End::Jump(head);
                    }
                    at = exit;
                }
                Stmt::For(f) => {
                    let here = (f.line, f.col);
                    // Two hidden locals, named so no program can write them:
                    // where the count stops (read ONCE, before the first
                    // turn), and the count itself. Over a table (level 6), a
                    // third: the table, read once too.
                    self.hidden += 1;
                    let (fin_name, count_name, table_name) = (format!("#fin{}", self.hidden), format!("#i{}", self.hidden), format!("#t{}", self.hidden));
                    let over = f.over.as_ref().map(|t| value(t, &mut self.locals, self.p));
                    let (to, from) = match &over {
                        Some(_) => (Value::Int(0, here), Value::Int(0, here)),
                        None => (value(&f.to, &mut self.locals, self.p), value(&f.from, &mut self.locals, self.p)),
                    };
                    // The loop's own scope: the hidden ones die at its exit.
                    self.open_scope(here);
                    let table = over.map(|t| {
                        let l = local_of(&mut self.locals, &table_name);
                        self.born(l);
                        self.blocks[at].ops.push(Op::Let { local: l, value: t, mutable: false, ty: None, at: here });
                        l
                    });
                    let fin = local_of(&mut self.locals, &fin_name);
                    let count = local_of(&mut self.locals, &count_name);
                    self.born(fin);
                    self.born(count);
                    let to = match table {
                        Some(t) => Value::Len(Box::new(Value::Local(t, here)), here),
                        None => to,
                    };
                    let (to_at, from_at) = if table.is_some() { (here, here) } else { (f.to.at(), f.from.at()) };
                    self.blocks[at].ops.push(Op::Let { local: fin, value: to, mutable: false, ty: None, at: to_at });
                    self.blocks[at].ops.push(Op::Let { local: count, value: from, mutable: true, ty: None, at: from_at });
                    let head = self.open();
                    self.blocks[at].end = End::Jump(head);
                    let body = self.open();
                    self.loops.push(Loop { breaks: Vec::new(), conts: Vec::new(), depth: self.scopes.len() });
                    // The body's scope: `i` is born at every turn, from the
                    // count, and dies at the end of it.
                    self.open_scope(here);
                    let var = local_of(&mut self.locals, &f.var);
                    self.born(var);
                    let turn = match table {
                        // a cell of a table or a list, a key of a map (13)
                        Some(t) => Value::Lib(Lib::Turn, vec![Value::Local(t, f.var_at), Value::Local(count, f.var_at)], f.var_at),
                        None => Value::Local(count, f.var_at),
                    };
                    self.blocks[body].ops.push(Op::Let { local: var, value: turn, mutable: false, ty: None, at: f.var_at });
                    let end = self.stmts(&f.body, body);
                    self.close_scope(end);
                    let lp = self.loops.pop().expect("pushed above");
                    let step = self.open();
                    self.blocks[end].end = End::Jump(step);
                    let next = Value::Bin("+", Box::new(Value::Local(count, here)), Box::new(Value::Int(1, here)), here);
                    self.blocks[step].ops.push(Op::Set { local: count, value: next, at: here });
                    // The jump UP.
                    self.blocks[step].end = End::Jump(head);
                    let exit = self.open();
                    let cond = Value::Bin("<", Box::new(Value::Local(count, here)), Box::new(Value::Local(fin, here)), here);
                    self.blocks[head].end = End::Branch { cond, then: body, other: exit, at: here };
                    for b in lp.breaks {
                        self.blocks[b].end = End::Jump(exit);
                    }
                    for b in lp.conts {
                        self.blocks[b].end = End::Jump(step);
                    }
                    self.close_scope(exit);
                    at = exit;
                }
                Stmt::Match { value: v, arms, line, col } => {
                    // ** THE MATCH (level 8): the value is read ONCE into a
                    // hidden local, and each arm asks "is it this case?" in
                    // order. The LAST arm asks nothing: `check` proved the
                    // arms cover every case, so when the others said no, it
                    // can only be this one.
                    //
                    //    b0   %m = forma            (its type: the enum, `calc` judges it)
                    //         si %m es Circulo -> b1, sino -> b2
                    //    b1   r = dato 0 de %m      Circulo(r) and its block
                    //         salta b3
                    //    b2   ... Nada              the last: no question
                    //         salta b3
                    //    b3   muere %m
                    let here = (*line, *col);
                    self.hidden += 1;
                    // `match pop(mut l)` (13): the last, and then `l` loses it
                    let popped = self.popped(v);
                    let (v, popped) = match popped {
                        Some((last, lent, here)) => (last, Some((lent, here))),
                        None => (value(v, &mut self.locals, self.p), None),
                    };
                    let enum_name = self.p.case(&arms[0].case).map(|(e, _)| self.p.enums[e].name.clone()).expect("check: every arm is a case");
                    self.open_scope(here);
                    let m = local_of(&mut self.locals, &format!("#m{}", self.hidden));
                    self.born(m);
                    let vat = v.at();
                    // `Opcion` (level 13) is one enum for every kind of value:
                    // its class is the one the value brings (`Opcion[int]`).
                    let ty = if crate::prelude::is_opcion(&enum_name) { None } else { Some(Ty::Named(enum_name)) };
                    self.blocks[at].ops.push(Op::Let { local: m, value: v, mutable: false, ty, at: vat });
                    if let Some((lent, here)) = popped {
                        self.drop_last(at, lent, here);
                    }
                    let mut ends = Vec::new();
                    for (k, arm) in arms.iter().enumerate() {
                        let (e, c) = self.p.case(&arm.case).expect("check: every arm is a case");
                        let arm_at = (arm.line, arm.col);
                        let body = if k + 1 == arms.len() {
                            at
                        } else {
                            let b = self.open();
                            self.blocks[at].end = End::Branch { cond: Value::Is(Box::new(Value::Local(m, arm_at)), e, c, arm_at), then: b, other: b, at: arm_at };
                            b
                        };
                        self.open_scope(arm_at);
                        for (j, (name, l, cl)) in arm.binds.iter().enumerate() {
                            let local = local_of(&mut self.locals, name);
                            self.born(local);
                            let data = Value::Payload(Box::new(Value::Local(m, (*l, *cl))), e, c, j, (*l, *cl));
                            self.blocks[body].ops.push(Op::Let { local, value: data, mutable: false, ty: None, at: (*l, *cl) });
                        }
                        let end = self.stmts(&arm.body, body);
                        self.close_scope(end);
                        ends.push(end);
                        if k + 1 < arms.len() {
                            // The "no" of this arm's question: where the next one asks.
                            let next = self.open();
                            if let End::Branch { other, .. } = &mut self.blocks[at].end {
                                *other = next;
                            }
                            at = next;
                        }
                    }
                    let join = self.open();
                    for end in ends {
                        self.blocks[end].end = End::Jump(join);
                    }
                    self.close_scope(join);
                    at = join;
                }
                Stmt::Break { .. } | Stmt::Continue { .. } => {
                    self.leave(at);
                    let lp = self.loops.last_mut().expect("parse: a jump is inside a loop");
                    if matches!(st, Stmt::Break { .. }) {
                        lp.breaks.push(at);
                    } else {
                        lp.conts.push(at);
                    }
                    // What follows a jump in the same block is never reached:
                    // it goes in a block of its own, with no way in.
                    at = self.open();
                }
            }
        }
        at
    }
}

/// From a tree whose FUNCTION names are checked to the IR. It does not judge
/// values: a name used before its `let` is still a local here, and the
/// checker says what is wrong with it.
pub fn lower(p: &Program) -> Module {
    let functions = p
        .functions
        .iter()
        .map(|f| {
            let mut l = Lowering { p, locals: Vec::new(), blocks: Vec::new(), scopes: vec![(None, Vec::new())], loops: Vec::new(), hidden: 0 };
            let params = f.params.iter().map(|a| (local_of(&mut l.locals, &a.name), a.ty.clone())).collect();
            let first = l.open();
            l.stmts(&f.body, first);
            let mut blocks = l.blocks;
            if f.gpu {
                // ** Inside a `gpu fn` the 3060 counts in f32: every number
                // written there IS one (`2.0`, `0.5`, `1`).
                for b in &mut blocks {
                    for op in &mut b.ops {
                        match op {
                            Op::Let { value, .. } | Op::Set { value, .. } => to_f32(value),
                            // LB6: a field of a record of a gpu fn that draws.
                            Op::SetAt { value, .. } => to_f32(value),
                            _ => {}
                        }
                    }
                    match &mut b.end {
                        End::Return(Some(v)) => to_f32(v),
                        End::Branch { cond, .. } => to_f32(cond),
                        _ => {}
                    }
                }
            }
            let obra = if f.gpu { crate::gpu::obra(p, f) } else { 0 };
            Function { name: f.name.clone(), line: f.line, locals: l.locals, params, modes: f.params.iter().map(|a| a.mode).collect(), ret: f.ret.clone(), blocks, gpu: f.gpu, obra, dispatch: None }
        })
        .collect::<Vec<_>>();
    // ** The fn of each trait (level 10): no body, and a table -- for each
    // type that keeps the trait, the fn that is ITS way (`area<Circulo>`).
    let mut functions = functions;
    for t in &p.traits {
        for s in &t.methods {
            let table = p
                .impls
                .iter()
                .filter(|i| i.trait_name == t.name)
                .map(|i| {
                    let name = crate::comportamiento::instance(&s.name, &i.ty);
                    (i.ty.name(), p.functions.iter().position(|f| f.name == name).expect("comportamiento: every fn of a trait ... for is a fn"))
                })
                .collect();
            let locals = s.params.iter().map(|a| Local { name: a.name.clone() }).collect();
            functions.push(Function {
                name: s.name.clone(),
                line: s.line,
                locals,
                params: s.params.iter().enumerate().map(|(k, a)| (k, a.ty.clone())).collect(),
                modes: s.params.iter().map(|a| a.mode).collect(),
                ret: s.ret.clone(),
                blocks: vec![Block { ops: Vec::new(), end: End::Return(None), dead: false }],
                gpu: false,
                obra: 0,
                dispatch: Some(table),
            });
        }
    }
    Module {
        name: p.module.clone(),
        purpose: p.purpose.clone(),
        functions,
        entry: p.functions.iter().position(|f| f.name == "main").expect("check: there is a main"),
        types: p.types.clone(),
        enums: p.enums.clone(),
        traits: p.traits.clone(),
        impls: p.impls.iter().map(|i| (i.trait_name.clone(), i.ty.name())).collect(),
        permissions: bmo_titan_contrato::Permissions::NONE,
        sources: crate::paquete::Sources::default(),
        flat: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ir(src: &str) -> Module {
        lower(&crate::compile(src).unwrap())
    }

    #[test]
    fn print_is_one_write_and_a_call_is_an_index() {
        let m = ir("mod main \"x\"\nfn main()\n    saluda()\n    print(\"hola\", \" otra vez\")\nfn saluda()\n    print(\"hola desde saluda\")\n");
        assert_eq!(m.entry, 0);
        let ops = &m.functions[0].blocks[0].ops;
        assert!(matches!(ops[0], Op::Call { func: 1, .. }));
        assert!(matches!(&ops[1], Op::Write { parts, .. } if parts.len() == 2));
        assert!(m.show().contains("llama   f1 (saluda)"), "{}", m.show());
    }

    #[test]
    fn a_name_is_a_numbered_local_and_its_reads_are_listed() {
        let m = ir("mod main \"x\"\nfn main()\n    let ancho = 3\n    let area = ancho * 4\n    print(area)\n");
        let f = &m.functions[0];
        assert_eq!(f.locals.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["ancho", "area"]);
        let Op::Let { local: 1, value, .. } = &f.blocks[0].ops[1] else { panic!("{:?}", f.blocks[0].ops[1]) };
        let mut r = Vec::new();
        value.reads(&mut r);
        assert_eq!(r, [(0, (4, 16))]);
        assert!(m.show().contains("%1 = (%0 * 4)"), "{}", m.show());
    }

    #[test]
    fn an_if_splits_the_body_into_blocks_that_only_jump_down() {
        let m = ir("mod main \"x\"\nfn main()\n    let v = 3\n    if v > 0\n        let r = \"si\"\n        print(r)\n    else\n        print(\"no\")\n    print(\"fin\")\n");
        let f = &m.functions[0];
        assert_eq!(f.blocks.len(), 4, "{}", m.show());
        assert!(matches!(f.blocks[0].end, End::Branch { then: 1, other: 2, .. }));
        assert_eq!((f.blocks[1].end.clone(), f.blocks[2].end.clone()), (End::Jump(3), End::Jump(3)));
        // What was born in the if's block dies when it closes.
        assert!(matches!(f.blocks[1].ops.last(), Some(Op::Drop { local: 1, at: (4, 5) })), "{}", m.show());
        for (i, b) in f.blocks.iter().enumerate() {
            assert!(b.end.targets().iter().all(|&t| t > i), "b{} jumps up", i);
        }
        assert!(m.show().contains("si (%0 > 0) -> b1, sino -> b2"), "{}", m.show());
    }

    #[test]
    fn main_need_not_be_first() {
        assert_eq!(ir("mod main \"x\"\nfn otra()\n    print(\"a\")\nfn main()\n    otra()\n").entry, 1);
    }

    /// ** The frontier is the tree of folders, and this test is its guard
    /// (the same rule as INTI's `agnostico.rs`): the IR says what a program
    /// does, never on which machine.
    #[test]
    fn the_ir_names_no_machine() {
        let src = include_str!("ir.rs");
        let code: String = src.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
        let code = code.split("#[cfg(test)]").next().unwrap().to_lowercase();
        for word in ["x86", "rax", "rdi", "syscall", "registro", "register", "invoke"] {
            assert!(!code.contains(word), "ir.rs names a machine: `{}`", word);
        }
    }
}
