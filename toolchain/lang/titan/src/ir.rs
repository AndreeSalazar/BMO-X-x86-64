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
    /// The case `v` of the enum `e`, carrying these values (8): `Circulo(2.0)`,
    /// `Nada`.
    Variant(usize, usize, Vec<Value>, At),
    /// Is this value the case `v` of the enum `e`? A yes/no: the question
    /// each arm of a `match` asks (8).
    Is(Box<Value>, usize, usize, At),
    /// The value number `k` the case `v` of the enum `e` carries: what a
    /// `Circulo(r)` of a `match` names `r` (8). Only read where `Is` said yes.
    Payload(Box<Value>, usize, usize, usize, At),
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
            | Value::Variant(_, _, _, a)
            | Value::Is(_, _, _, a)
            | Value::Payload(_, _, _, _, a) => *a,
        }
    }

    /// Every local this value reads, in reading order: what the checker
    /// judges.
    pub fn reads(&self, out: &mut Vec<(usize, At)>) {
        match self {
            Value::Int(..) | Value::Text(..) | Value::Bool(..) | Value::Dec(..) => {}
            Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) => {
                for i in items {
                    i.reads(out);
                }
            }
            Value::Repeat(v, _, _) | Value::Field(v, _, _) | Value::Len(v, _) | Value::Round(v, _, _) | Value::Is(v, _, _, _) | Value::Payload(v, _, _, _, _) => v.reads(out),
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
            Value::Call(_, args, _) => {
                for a in args {
                    a.reads(out);
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
        Expr::Call { callee, args, line, col } if callee == "len" => Value::Len(Box::new(value(&args[0], locals)), (*line, *col)),
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
                        Some(t) => Value::Index(Box::new(Value::Local(t, f.var_at)), Box::new(Value::Local(count, f.var_at)), f.var_at),
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
                    let v = value(v, &mut self.locals, self.p);
                    let enum_name = self.p.case(&arms[0].case).map(|(e, _)| self.p.enums[e].name.clone()).expect("check: every arm is a case");
                    self.open_scope(here);
                    let m = local_of(&mut self.locals, &format!("#m{}", self.hidden));
                    self.born(m);
                    let vat = v.at();
                    self.blocks[at].ops.push(Op::Let { local: m, value: v, mutable: false, ty: Some(Ty::Named(enum_name)), at: vat });
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
            Function { name: f.name.clone(), line: f.line, locals: l.locals, params, modes: f.params.iter().map(|a| a.mode).collect(), ret: f.ret.clone(), blocks: l.blocks, dispatch: None }
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
        flat: None,
    }
}

impl Module {
    /// The IR as text, for `titan ir`: what the emitter will receive.
    pub fn show(&self) -> String {
        let mut s = format!("mod {}  \"{}\"   entra por f{}\n", self.name, self.purpose, self.entry);
        let tail = match &self.flat {
            Some(flat) => {
                let mut t = format!("\nlo que escribe, CORRIDO al compilar ({} escrituras): es lo que hace el .bex\n", flat.len());
                for op in flat {
                    if let Op::Write { parts, at } = op {
                        let p: Vec<String> = parts.iter().map(show).collect();
                        t += &format!("    linea {:<4} escribe {}\n", at.0, p.join(", "));
                    }
                }
                t
            }
            None => String::new(),
        };
        for (i, f) in self.functions.iter().enumerate() {
            let locals: Vec<String> = f.locals.iter().enumerate().map(|(k, l)| format!("%{}={}", k, l.name)).collect();
            s += &format!("f{} {}   {}\n", i, f.name, locals.join(" "));
            for (j, b) in f.blocks.iter().enumerate() {
                s += &format!("  b{}{}\n", j, if b.dead { "   (muerto: ninguna ejecucion llega aqui, y no deja bytes)" } else { "" });
                for op in &b.ops {
                    s += &match op {
                        Op::Let { local, value, mutable: true, .. } => format!("    %{} = {}   (mut)\n", local, show(value)),
                        Op::Let { local, value, .. } => format!("    %{} = {}\n", local, show(value)),
                        Op::Set { local, value, .. } => format!("    %{} := {}\n", local, show(value)),
                        Op::Write { parts, .. } => {
                            let p: Vec<String> = parts.iter().map(show).collect();
                            format!("    escribe {}\n", p.join(", "))
                        }
                        Op::Call { func, args, .. } => {
                            let a: Vec<String> = args.iter().map(show).collect();
                            format!("    llama   f{} ({})({})\n", func, self.functions[*func].name, a.join(", "))
                        }
                        Op::Drop { local, at } => format!("    muere   %{} ({}, al cerrarse el bloque de la linea {})\n", local, f.locals[*local].name, at.0),
                        Op::SetAt { local, path, value, .. } => {
                            let p: String = path
                                .iter()
                                .map(|st| match st {
                                    PathStep::Index(i) => format!("[{}]", show(i)),
                                    PathStep::Field(n, _) => format!(".{}", n),
                                })
                                .collect();
                            format!("    %{}{} := {}\n", local, p, show(value))
                        }
                    };
                }
                s += &match &b.end {
                    End::Return(None) => "    vuelve\n".to_string(),
                    End::Return(Some(v)) => format!("    vuelve con {}\n", show(v)),
                    End::Jump(t) => format!("    salta   b{}\n", t),
                    End::Branch { cond, then, other, .. } => format!("    si {} -> b{}, sino -> b{}\n", show(cond), then, other),
                };
            }
        }
        s + &tail
    }
}

fn show(v: &Value) -> String {
    match v {
        Value::Int(n, _) => n.to_string(),
        Value::Text(t, _) => format!("{:?}", t),
        Value::Bool(b, _) => b.to_string(),
        Value::Local(l, _) => format!("%{}", l),
        Value::Bin(op, l, r, _) => format!("({} {} {})", show(l), op, show(r)),
        Value::Neg(v, _) => format!("-{}", show(v)),
        Value::Not(v, _) => format!("not {}", show(v)),
        Value::Call(f, args, _) => {
            let a: Vec<String> = args.iter().map(show).collect();
            format!("f{}({})", f, a.join(", "))
        }
        Value::Dec(d, sc, _) => crate::tree::show_dec(*d, *sc),
        Value::Table(items, _) => format!("[{}]", items.iter().map(show).collect::<Vec<_>>().join(", ")),
        Value::Repeat(v, n, _) => format!("[{}; {}]", show(v), n),
        Value::Index(b, i, _) => format!("{}[{}]", show(b), show(i)),
        Value::Field(b, n, _) => format!("{}.{}", show(b), n),
        Value::Record(t, items, _) => format!("T{} {{ {} }}", t, items.iter().map(show).collect::<Vec<_>>().join(", ")),
        Value::Len(v, _) => format!("len({})", show(v)),
        Value::Lend(m, l, _) => format!("{} %{}", m.word(), l),
        Value::Round(v, n, _) => format!("round({}, {})", show(v), n),
        Value::Variant(e, v, args, _) if args.is_empty() => format!("E{}.{}", e, v),
        Value::Variant(e, v, args, _) => format!("E{}.{}({})", e, v, args.iter().map(show).collect::<Vec<_>>().join(", ")),
        Value::Is(x, e, v, _) => format!("{} es E{}.{}", show(x), e, v),
        Value::Payload(x, e, v, k, _) => format!("dato {} de {} (E{}.{})", k, show(x), e, v),
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
