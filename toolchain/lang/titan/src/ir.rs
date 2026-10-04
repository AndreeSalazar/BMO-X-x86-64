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
//! Level 1 keeps a function as ONE block (there is no `if` to split it):
//! `let` defines a local, `print` writes, a call calls. The shape is already
//! the one the checker walks when blocks are many.

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Int(i64, At),
    Text(String, At),
    /// What the local holds at that point.
    Local(usize, At),
    /// `+ - * / %`.
    Bin(char, Box<Value>, Box<Value>, At),
    Neg(Box<Value>, At),
}

impl Value {
    pub fn at(&self) -> At {
        match self {
            Value::Int(_, a) | Value::Text(_, a) | Value::Local(_, a) | Value::Bin(_, _, _, a) | Value::Neg(_, a) => *a,
        }
    }

    /// Every local this value reads, in reading order: what the checker
    /// judges.
    pub fn reads(&self, out: &mut Vec<(usize, At)>) {
        match self {
            Value::Int(..) | Value::Text(..) => {}
            Value::Local(l, a) => out.push((*l, *a)),
            Value::Bin(_, l, r, _) => {
                l.reads(out);
                r.reads(out);
            }
            Value::Neg(v, _) => v.reads(out),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// Back to whoever called. For `main`, the end of the program.
    Return,
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
        Expr::Bin { op, left, right, line, col } => {
            Value::Bin(*op, Box::new(value(left, locals)), Box::new(value(right, locals)), (*line, *col))
        }
        Expr::Neg { value: v, line, col } => Value::Neg(Box::new(value(v, locals)), (*line, *col)),
    }
}

/// From a tree whose FUNCTION names are checked to the IR. It does not judge
/// values: a name used before its `let` is still a local here, and the
/// checker says what is wrong with it.
pub fn lower(p: &Program) -> Module {
    let index = |name: &str| p.functions.iter().position(|f| f.name == name);
    let functions = p
        .functions
        .iter()
        .map(|f| {
            let mut locals = Vec::new();
            let ops = f
                .body
                .iter()
                .map(|st| match st {
                    Stmt::Let(l) => {
                        let v = value(&l.value, &mut locals);
                        Op::Let { local: local_of(&mut locals, &l.name), value: v, mutable: l.mutable, at: (l.line, l.col) }
                    }
                    Stmt::Set(l) => {
                        let v = value(&l.value, &mut locals);
                        Op::Set { local: local_of(&mut locals, &l.name), value: v, at: (l.line, l.col) }
                    }
                    Stmt::Call(c) if c.callee == "print" => {
                        Op::Write { parts: c.args.iter().map(|a| value(a, &mut locals)).collect(), at: (c.line, c.col) }
                    }
                    Stmt::Call(c) => Op::Call { func: index(&c.callee).expect("check: every call goes somewhere"), at: (c.line, c.col) },
                })
                .collect();
            Function { name: f.name.clone(), line: f.line, locals, blocks: vec![Block { ops, end: End::Return }] }
        })
        .collect();
    Module {
        name: p.module.clone(),
        purpose: p.purpose.clone(),
        functions,
        entry: index("main").expect("check: there is a main"),
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
                s += &format!("  b{}\n", j);
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
                    };
                }
                s += match b.end {
                    End::Return => "    vuelve\n",
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
        Value::Local(l, _) => format!("%{}", l),
        Value::Bin(op, l, r, _) => format!("({} {} {})", show(l), op, show(r)),
        Value::Neg(v, _) => format!("-{}", show(v)),
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
