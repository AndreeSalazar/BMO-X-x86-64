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
//!    ir     what it DOES: functions by number, blocks, and what each one
//!           asks of the system. Zero names to look up, zero decisions left
//! ```
//!
//! ** NO MACHINE. This file never says "register", "syscall" or "x86": the
//! emitter (`emisor-x86_64/`, its own crate) is the only one that may. A test
//! below reads this file and fails if it names one.
//!
//! Level 0 is tiny on purpose: a function is ONE block (there is no `if` to
//! split it), and a block writes texts and calls functions. The shape is
//! already the one the borrow checker will walk (T4): a block, its operations
//! in order, and how it ends.

use crate::tree::Program;

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
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub ops: Vec<Op>,
    pub end: End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Write this text on the console. Known when compiling: it is the bytes,
    /// already joined and with its end of line.
    Write(String),
    /// Call the function with this index.
    Call(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// Back to whoever called. For `main`, the end of the program.
    Return,
}

/// From a CHECKED tree to the IR. It cannot fail: `check` already said that
/// `main` is there and that every call goes somewhere, so a name that is not
/// found here is a bug of this file, not of the program.
pub fn lower(p: &Program) -> Module {
    let index = |name: &str| p.functions.iter().position(|f| f.name == name);
    let functions = p
        .functions
        .iter()
        .map(|f| {
            let ops = f
                .body
                .iter()
                .map(|c| match c.callee.as_str() {
                    // `print("a", "b")` writes the texts one after the other and
                    // ends the line: ONE write, because that is what it means.
                    // The kernel flushes a console line at its `\n`, so two
                    // writes would mean the same and cost twice.
                    "print" => Op::Write(c.args.concat() + "\n"),
                    own => Op::Call(index(own).expect("check: every call goes somewhere")),
                })
                .collect();
            Function { name: f.name.clone(), line: f.line, blocks: vec![Block { ops, end: End::Return }] }
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
            s += &format!("f{} {}\n", i, f.name);
            for (j, b) in f.blocks.iter().enumerate() {
                s += &format!("  b{}\n", j);
                for op in &b.ops {
                    s += &match op {
                        Op::Write(t) => format!("    escribe {:?}\n", t),
                        Op::Call(k) => format!("    llama   f{} ({})\n", k, self.functions[*k].name),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_is_one_write_with_its_end_of_line_and_a_call_is_an_index() {
        let p = crate::compile(
            "mod main \"x\"\nfn main()\n    saluda()\n    print(\"hola\", \" otra vez\")\nfn saluda()\n    print(\"hola desde saluda\")\n",
        )
        .unwrap();
        let m = lower(&p);
        assert_eq!(m.entry, 0);
        assert_eq!(m.functions[0].blocks[0].ops, [Op::Call(1), Op::Write("hola otra vez\n".into())]);
        assert_eq!(m.functions[1].blocks[0].ops, [Op::Write("hola desde saluda\n".into())]);
        assert!(m.show().contains("llama   f1 (saluda)"), "{}", m.show());
    }

    #[test]
    fn main_need_not_be_first() {
        let p = crate::compile("mod main \"x\"\nfn otra()\n    print(\"a\")\nfn main()\n    otra()\n").unwrap();
        assert_eq!(lower(&p).entry, 1);
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
