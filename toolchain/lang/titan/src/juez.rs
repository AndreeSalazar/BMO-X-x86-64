//! `juez` -- THE CHECKER: the borrow checker of TITAN++, first step.
//!
//! TITAN_MAESTRO 6 and 6b. Inspired by Rust's, not copied: it reasons on THIS
//! crate's IR (L1, the lesson of rustc's MIR), function by function (L2: with
//! model 2 there are no lifetimes to infer), and every NO it says carries the
//! four parts (L4: in rustc, explaining the NO is 42 % of the checker).
//!
//! ```text
//!    the idea, whole        every LOCAL, at every point of the function,
//!                           is in ONE state; every operation is judged
//!                           against the state it finds, and moves it
//!
//!    the states, by level   Unborn ----let----> Alive          level 1 (HERE)
//!                           Alive  ----x = v--> Alive  if mut   level 2 (HERE)
//!                           Alive  ---take x--> Given           level 7 (paso A)
//!                           Alive  ---mut x---> Lent ... back   level 7 (paso B)
//!                           Alive  --offer/gpu--> LentOut       U1: the KERNEL
//!                                                               and the 3060
//! ```
//!
//! ** What it judges TODAY (levels 1 and 2), each with its program that
//! breaks it in `ejemplos/nivelN/`:
//!
//! ```text
//!    T0054  a local read before any `let` gave it a value
//!    T0055  a second `let` for a name that already has one (one name, one
//!           value: TITAN++ has no shadowing -- "una sola forma")
//!    T0056  `x = v` on a local that is not `mut`
//!    T0057  a `mut` that never changes: the promise is broken (level 2)
//!    T0058  a local read after the block it was born in closed (level 3)
//! ```
//!
//! ** Level 3 makes it a WALK, not a list: an `if` splits the body into
//! blocks, and at the block after it two ways meet. What is certain there is
//! what is certain on BOTH ways (`meet`): that is the step that turns a list
//! of checks into a borrow checker.
//!
//! ** THE TWO JUDGES (6b). This judge sees INSIDE the program, before it runs,
//! once: its NO is final -- the program never exists. What it cannot see --
//! what other programs hold, what the kernel grants -- is the KERNEL's, at
//! every door, always. Between them travels the CERTIFICATE
//! (`bmo_titan_contrato::certificate`): what this program uses of BMO-X and
//! from which line, so that when the kernel says NO, it says WHERE. This file
//! fills the inside half; the certificate is written from the same IR, after
//! this judge said yes.

use crate::check::distance;
use crate::ir::{At, End, Function, Module, Op};
use crate::message::{Code, Message};

/// The state of one local at one point. Grows by level (see the header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// No `let` reached it yet.
    Unborn,
    /// It has its value since `since`; `mutable` if `let mut`, and `changed`
    /// once a `x = ...` changed it (a `mut` that never changes is a NO).
    Alive { since: usize, at: At, mutable: bool, changed: bool },
    /// It was born at line `born` inside a block, and that block -- the `if`
    /// or `else` at `scope` -- closed: it is gone (level 3, T0058).
    Dead { born: usize, scope: At },
}

/// Where two ways meet (after an `if`), what is certain on BOTH. A value
/// from before the `if` is alive on both sides, and changed if EITHER side
/// changed it; what was born inside one side died when that side closed.
fn meet(a: State, b: State) -> State {
    match (a, b) {
        (State::Alive { since, at, mutable, changed: x }, State::Alive { changed: y, .. }) => State::Alive { since, at, mutable, changed: x || y },
        (d @ State::Dead { .. }, _) | (_, d @ State::Dead { .. }) => d,
        _ => State::Unborn,
    }
}

/// The whole module, function by function. The first NO stops it, like the
/// rest of the frontend: one right message is worth ten that follow from it.
pub fn judge(m: &Module) -> Result<(), Message> {
    for f in &m.functions {
        judge_fn(f)?;
    }
    Ok(())
}

/// ** THE WALK (TITAN_MAESTRO 6.8, step 3): one state per block, the state at
/// its entrance being the meeting of every way in. The blocks are in reading
/// order and every jump goes down (`ir.rs`), so walking them in order meets
/// every way into a block before the block: one pass is the fixed point. The
/// day `while` brings a jump UP (level 4), this loop repeats until nothing
/// changes -- and nothing else here moves.
fn judge_fn(f: &Function) -> Result<(), Message> {
    let mut entry: Vec<Option<Vec<State>>> = vec![None; f.blocks.len()];
    entry[0] = Some(vec![State::Unborn; f.locals.len()]);
    for (i, b) in f.blocks.iter().enumerate() {
        // No way leads here: nothing in it can run, nothing in it is judged
        // twice. (Today every block has a way in; this is the honest default.)
        let Some(mut state) = entry[i].take() else { continue };
        for op in &b.ops {
            step(f, op, &mut state)?;
        }
        match &b.end {
            End::Return => never_changed(f, &state)?,
            End::Branch { cond, .. } => {
                let mut reads = Vec::new();
                cond.reads(&mut reads);
                for (l, at) in reads {
                    usable(f, &state, l, at)?;
                }
            }
            End::Jump(_) => {}
        }
        for t in b.end.targets() {
            entry[t] = Some(match entry[t].take() {
                None => state.clone(),
                Some(other) => other.iter().zip(&state).map(|(a, b)| meet(*a, *b)).collect(),
            });
        }
    }
    Ok(())
}

/// A read of `l` at `at`: it must be alive.
fn usable(f: &Function, state: &[State], l: usize, at: At) -> Result<(), Message> {
    match state[l] {
        State::Alive { .. } => Ok(()),
        State::Unborn => Err(no_value(f, l, at)),
        State::Dead { born, scope } => Err(gone(f, l, at, born, scope)),
    }
}

/// One operation, judged against the state it finds, and the state it leaves.
fn step(f: &Function, op: &Op, state: &mut [State]) -> Result<(), Message> {
    // Reads come BEFORE the definition they feed (L5 of 6.8: what is only
    // read is evaluated before a change begins): in `n = n + 1` the `n` on
    // the right is judged as it was.
    let mut reads = Vec::new();
    let define = match op {
        Op::Let { local, value, mutable, at } => {
            value.reads(&mut reads);
            Some((*local, *at, Some(*mutable)))
        }
        Op::Set { local, value, at } => {
            value.reads(&mut reads);
            Some((*local, *at, None))
        }
        Op::Write { parts, .. } => {
            for p in parts {
                p.reads(&mut reads);
            }
            None
        }
        Op::Call { .. } => None,
        Op::Drop { local, at } => {
            // The block closes: a `mut` born in it had its whole life to
            // change, and that life is over.
            if let State::Alive { mutable: true, changed: false, since, at: born } = state[*local] {
                return Err(never(f, *local, since, born));
            }
            if let State::Alive { since, .. } = state[*local] {
                state[*local] = State::Dead { born: since, scope: *at };
            }
            return Ok(());
        }
    };
    for (l, at) in reads {
        usable(f, state, l, at)?;
    }
    let Some((l, at, how)) = define else { return Ok(()) };
    let name = &f.locals[l].name;
    state[l] = match (state[l], how) {
        // `let` / `let mut`: it is born -- also where a block-scoped one of
        // the same name died before (they never live at the same time).
        (State::Unborn | State::Dead { .. }, Some(mutable)) => State::Alive { since: at.0, at, mutable, changed: false },
        (State::Alive { since, .. }, Some(_)) => {
            return Err(Message::new(
                Code::Taken,
                at.0,
                at.1,
                &format!("`{}` ya tiene valor", name),
                &format!("se lo dio el `let` de la linea {}: un nombre, un valor (TITAN++ no tapa un nombre con otro)", since),
                &format!("si tiene que cambiar, `let mut {}` arriba y aqui `{} = ...`", name, name),
            ))
        }
        // `x = ...`
        (State::Unborn, None) => return Err(no_value(f, l, at)),
        (State::Dead { born, scope }, None) => return Err(gone(f, l, at, born, scope)),
        (State::Alive { since, mutable: false, .. }, None) => {
            return Err(Message::new(
                Code::NotMut,
                at.0,
                at.1,
                &format!("`{}` no se puede cambiar", name),
                &format!("su `let` de la linea {} no dice `mut`: un valor sin `mut` no cambia nunca", since),
                &format!("si tiene que cambiar, declaralo asi en la linea {}: let mut {} = ...", since, name),
            ))
        }
        (State::Alive { since, at: born, mutable: true, .. }, None) => State::Alive { since, at: born, mutable: true, changed: true },
    };
    Ok(())
}

/// ** A `mut` that never changes is a NO, not a warning: `mut` is a promise
/// to whoever reads ("this one moves"), and a promise nobody keeps makes
/// every `mut` worth less. Rust only warns; TITAN++ says it, because the
/// reader -- and later the borrow checker -- trusts the word.
fn never_changed(f: &Function, state: &[State]) -> Result<(), Message> {
    for (l, st) in state.iter().enumerate() {
        if let State::Alive { since, at, mutable: true, changed: false } = *st {
            return Err(never(f, l, since, at));
        }
    }
    Ok(())
}

fn never(f: &Function, l: usize, since: usize, at: At) -> Message {
    let name = &f.locals[l].name;
    Message::new(
        Code::NeverChanged,
        since,
        at.1,
        &format!("`{}` dice `mut` y no cambia nunca", name),
        &format!("en `fn {}()` ninguna linea le da otro valor: el `mut` promete algo que no pasa", f.name),
        &format!("quita el `mut`: let {} = ...", name),
    )
}

/// T0058: it was born inside a block that already closed.
fn gone(f: &Function, l: usize, at: At, born: usize, scope: At) -> Message {
    let name = &f.locals[l].name;
    Message::new(
        Code::Gone,
        at.0,
        at.1,
        &format!("`{}` ya no existe aqui", name),
        &format!(
            "nacio en la linea {}, dentro del bloque que abre la linea {}, y murio al cerrarse: lo que nace en un bloque vive en su bloque",
            born, scope.0
        ),
        &format!("si lo necesitas despues, dale valor ANTES del bloque (let mut {} = ...) y dentro solo cambialo: {} = ...", name, name),
    )
}

/// T0054, with the best "why" there is: a `let` further down (the reading
/// order), or a name that is close to one that has a value.
fn no_value(f: &Function, l: usize, at: At) -> Message {
    let name = &f.locals[l].name;
    let later = f.blocks.iter().flat_map(|b| &b.ops).find_map(|op| match op {
        Op::Let { local, at: a, .. } if *local == l && *a > at => Some(a.0),
        _ => None,
    });
    let why = match later {
        Some(line) => format!("su `let` esta en la linea {}, mas abajo: un cuerpo se lee de arriba abajo", line),
        None => "ningun `let` de esta funcion le da valor".to_string(),
    };
    let near = f
        .blocks
        .iter()
        .flat_map(|b| &b.ops)
        .filter_map(|op| match op {
            Op::Let { local, .. } if *local != l => Some(f.locals[*local].name.as_str()),
            _ => None,
        })
        .min_by_key(|k| distance(k, name))
        .filter(|k| distance(k, name) <= 2);
    let how = match (later, near) {
        (Some(_), _) => format!("sube el `let {} = ...` por encima de esta linea", name),
        (None, Some(k)) => format!("quisiste decir `{}`?", k),
        (None, None) => format!("dale valor antes: let {} = ...", name),
    };
    Message::new(Code::NoValue, at.0, at.1, &format!("`{}` no tiene valor aqui", name), &why, &how)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(src: &str) -> Result<(), Message> {
        judge(&crate::ir::lower(&crate::compile(src).unwrap()))
    }

    #[test]
    fn a_value_used_after_its_let_is_fine() {
        assert!(verdict("mod main \"x\"\nfn main()\n    let a = 3\n    let b = a * 2\n    print(b)\n").is_ok());
    }

    #[test]
    fn read_before_let_says_where_the_let_is() {
        let e = verdict("mod main \"x\"\nfn main()\n    print(area)\n    let area = 12\n").unwrap_err();
        assert_eq!((e.code, e.line), (Code::NoValue, 3));
        assert!(e.why.contains("linea 4"), "{}", e.why);
        let e = verdict("mod main \"x\"\nfn main()\n    let ancho = 3\n    print(anhco)\n").unwrap_err();
        assert_eq!(e.how, "quisiste decir `ancho`?");
    }

    #[test]
    fn a_let_reading_itself_is_judged_before_it_defines() {
        let e = verdict("mod main \"x\"\nfn main()\n    let x = x + 1\n").unwrap_err();
        assert_eq!(e.code, Code::NoValue);
    }

    #[test]
    fn one_name_one_value_and_without_mut_it_does_not_change() {
        let e = verdict("mod main \"x\"\nfn main()\n    let a = 1\n    let a = 2\n").unwrap_err();
        assert_eq!((e.code, e.line), (Code::Taken, 4));
        let e = verdict("mod main \"x\"\nfn main()\n    let a = 1\n    a = 2\n").unwrap_err();
        assert_eq!((e.code, e.line), (Code::NotMut, 4));
        assert!(e.how.contains("let mut a"), "{}", e.how);
    }

    #[test]
    fn a_mut_changes_and_a_mut_that_never_changes_is_a_no() {
        assert!(verdict("mod main \"x\"\nfn main()\n    let mut n = 0\n    n = n + 1\n    print(n)\n").is_ok());
        let e = verdict("mod main \"x\"\nfn main()\n    let mut n = 0\n    print(n)\n").unwrap_err();
        assert_eq!((e.code, e.line), (Code::NeverChanged, 3));
    }

    #[test]
    fn what_is_born_in_a_block_dies_with_it() {
        let e = verdict("mod main \"x\"\nfn main()\n    if true\n        let r = 1\n        print(r)\n    print(r)\n").unwrap_err();
        assert_eq!((e.code, e.line), (Code::Gone, 6));
        assert!(e.why.contains("linea 4") && e.why.contains("linea 3"), "{}", e.why);
        // Two sides, the same name: they never live at the same time.
        assert!(verdict("mod main \"x\"\nfn main()\n    if true\n        let r = 1\n        print(r)\n    else\n        let r = 2\n        print(r)\n").is_ok());
    }

    #[test]
    fn where_two_ways_meet_a_change_on_either_counts() {
        assert!(verdict("mod main \"x\"\nfn main()\n    let mut n = 0\n    if n == 0\n        n = 5\n    print(n)\n").is_ok());
        // A `mut` born in a block that never changes there: said when it dies.
        let e = verdict("mod main \"x\"\nfn main()\n    if true\n        let mut k = 1\n        print(k)\n").unwrap_err();
        assert_eq!((e.code, e.line), (Code::NeverChanged, 4));
        // The condition is a read like any other.
        let e = verdict("mod main \"x\"\nfn main()\n    if vidas > 0\n        print(\"a\")\n").unwrap_err();
        assert_eq!(e.code, Code::NoValue);
    }

    #[test]
    fn each_function_has_its_own_names() {
        assert!(verdict("mod main \"x\"\nfn main()\n    let a = 1\n    otra()\nfn otra()\n    let a = 2\n    print(a)\n").is_ok());
    }
}
