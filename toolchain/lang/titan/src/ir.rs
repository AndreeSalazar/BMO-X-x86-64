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

use crate::tree::{Expr, Program, Stmt};

/// A whole module, ready to emit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// The header (U3): it travels to the `.bex` manifest.
    pub name: String,
    pub purpose: String,
    pub functions: Vec<Function>,
    /// Where the program starts: the index of `main` in `functions`.
    pub entry: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    /// The line of its `fn`, so a NO from below can still point at the text.
    pub line: usize,
    /// Every name the body uses for a value, numbered in order of first use.
    pub locals: Vec<Local>,
    pub blocks: Vec<Block>,
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
    /// The local gets its value: `let x = ...`; `mutable` if `let mut`.
    Let { local: usize, value: Value, mutable: bool, at: At },
    /// The local is given a new value: `x = ...` (needs `mut`, level 2).
    Set { local: usize, value: Value, at: At },
    /// Write these values on the console, one after another, and end the
    /// line: `print(...)`. The one door of BMO-X level 1 uses.
    Write { parts: Vec<Value>, at: At },
    /// Call the function with this index.
    Call { func: usize, at: At },
    /// The block that gave birth to this local closes here: it dies. `at` is
    /// the `if` (or `else`) whose block closes.
    Drop { local: usize, at: At },
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
}

impl Value {
    pub fn at(&self) -> At {
        match self {
            Value::Int(_, a) | Value::Text(_, a) | Value::Bool(_, a) | Value::Local(_, a) | Value::Bin(_, _, _, a) | Value::Neg(_, a) | Value::Not(_, a) => *a,
        }
    }

    /// Every local this value reads, in reading order: what the checker
    /// judges.
    pub fn reads(&self, out: &mut Vec<(usize, At)>) {
        match self {
            Value::Int(..) | Value::Text(..) | Value::Bool(..) => {}
            Value::Local(l, a) => out.push((*l, *a)),
            Value::Bin(_, l, r, _) => {
                l.reads(out);
                r.reads(out);
            }
            Value::Neg(v, _) | Value::Not(v, _) => v.reads(out),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// Back to whoever called. For `main`, the end of the program.
    Return,
    /// On to that block.
    Jump(usize),
    /// `if`: to `then` if `cond` is true, to `other` if not. `at` is the `if`.
    Branch { cond: Value, then: usize, other: usize, at: At },
}

impl End {
    /// The blocks this end can go to.
    pub fn targets(&self) -> Vec<usize> {
        match self {
            End::Return => Vec::new(),
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

fn value(e: &Expr, locals: &mut Vec<Local>) -> Value {
    match e {
        Expr::Int { value, line, col } => Value::Int(*value, (*line, *col)),
        Expr::Text { value, line, col } => Value::Text(value.clone(), (*line, *col)),
        Expr::Name { name, line, col } => Value::Local(local_of(locals, name), (*line, *col)),
        Expr::Bool { value, line, col } => Value::Bool(*value, (*line, *col)),
        Expr::Not { value: v, line, col } => Value::Not(Box::new(value(v, locals)), (*line, *col)),
        Expr::Bin { op, left, right, line, col } => {
            Value::Bin(*op, Box::new(value(left, locals)), Box::new(value(right, locals)), (*line, *col))
        }
        Expr::Neg { value: v, line, col } => Value::Neg(Box::new(value(v, locals)), (*line, *col)),
    }
}

/// Lowers one function: its blocks grow as `if`s split them.
struct Lowering<'p> {
    p: &'p Program,
    locals: Vec<Local>,
    blocks: Vec<Block>,
}

impl Lowering<'_> {
    fn open(&mut self) -> usize {
        self.blocks.push(Block { ops: Vec::new(), end: End::Return, dead: false });
        self.blocks.len() - 1
    }

    fn index(&self, name: &str) -> usize {
        self.p.functions.iter().position(|f| f.name == name).expect("check: every call goes somewhere")
    }

    /// Lowers `body` starting in block `at`; returns the block where it ends.
    /// `scope` is the `if`/`else` that opened it (None for the fn's own body):
    /// what is born directly in it dies when it closes.
    fn body(&mut self, body: &[Stmt], mut at: usize, scope: Option<At>) -> usize {
        let mut born = Vec::new();
        for st in body {
            match st {
                Stmt::Let(l) => {
                    let v = value(&l.value, &mut self.locals);
                    let local = local_of(&mut self.locals, &l.name);
                    born.push(local);
                    self.blocks[at].ops.push(Op::Let { local, value: v, mutable: l.mutable, at: (l.line, l.col) });
                }
                Stmt::Set(l) => {
                    let v = value(&l.value, &mut self.locals);
                    let local = local_of(&mut self.locals, &l.name);
                    self.blocks[at].ops.push(Op::Set { local, value: v, at: (l.line, l.col) });
                }
                Stmt::Call(c) if c.callee == "print" => {
                    let parts = c.args.iter().map(|a| value(a, &mut self.locals)).collect();
                    self.blocks[at].ops.push(Op::Write { parts, at: (c.line, c.col) });
                }
                Stmt::Call(c) => {
                    let func = self.index(&c.callee);
                    self.blocks[at].ops.push(Op::Call { func, at: (c.line, c.col) });
                }
                Stmt::If(i) => {
                    let cond = value(&i.cond, &mut self.locals);
                    let here = (i.line, i.col);
                    let then = self.open();
                    let then_end = self.body(&i.then, then, Some(here));
                    let (other, other_end) = if i.other.is_empty() {
                        (None, None)
                    } else {
                        let b = self.open();
                        (Some(b), Some(self.body(&i.other, b, Some(i.else_at))))
                    };
                    let join = self.open();
                    self.blocks[then_end].end = End::Jump(join);
                    if let Some(e) = other_end {
                        self.blocks[e].end = End::Jump(join);
                    }
                    self.blocks[at].end = End::Branch { cond, then, other: other.unwrap_or(join), at: here };
                    at = join;
                }
            }
        }
        if let Some(scope) = scope {
            for local in born {
                self.blocks[at].ops.push(Op::Drop { local, at: scope });
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
            let mut l = Lowering { p, locals: Vec::new(), blocks: Vec::new() };
            let first = l.open();
            l.body(&f.body, first, None);
            Function { name: f.name.clone(), line: f.line, locals: l.locals, blocks: l.blocks }
        })
        .collect();
    Module {
        name: p.module.clone(),
        purpose: p.purpose.clone(),
        functions,
        entry: p.functions.iter().position(|f| f.name == "main").expect("check: there is a main"),
    }
}

impl Module {
    /// The IR as text, for `titan ir`: what the emitter will receive.
    pub fn show(&self) -> String {
        let mut s = format!("mod {}  \"{}\"   entra por f{}\n", self.name, self.purpose, self.entry);
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
                        Op::Call { func, .. } => format!("    llama   f{} ({})\n", func, self.functions[*func].name),
                        Op::Drop { local, at } => format!("    muere   %{} ({}, al cerrarse el bloque de la linea {})\n", local, f.locals[*local].name, at.0),
                    };
                }
                s += &match &b.end {
                    End::Return => "    vuelve\n".to_string(),
                    End::Jump(t) => format!("    salta   b{}\n", t),
                    End::Branch { cond: c @ Value::Bool(..), then, other, .. } => format!("    si {} -> b{}, sino -> b{}   (decidido al compilar)\n", show(c), then, other),
                    End::Branch { cond, then, other, .. } => format!("    si {} -> b{}, sino -> b{}\n", show(cond), then, other),
                };
            }
        }
        s
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
