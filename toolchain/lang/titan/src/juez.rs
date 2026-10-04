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
//! ```
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
use crate::ir::{At, Function, Module, Op};
use crate::message::{Code, Message};

/// The state of one local at one point. Grows by level (see the header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// No `let` reached it yet.
    Unborn,
    /// It has its value since `since`; `mutable` if `let mut`, and `changed`
    /// once a `x = ...` changed it (a `mut` that never changes is a NO).
    Alive { since: usize, at: At, mutable: bool, changed: bool },
}

/// The whole module, function by function. The first NO stops it, like the
/// rest of the frontend: one right message is worth ten that follow from it.
pub fn judge(m: &Module) -> Result<(), Message> {
    for f in &m.functions {
        judge_fn(f)?;
    }
    Ok(())
}

fn judge_fn(f: &Function) -> Result<(), Message> {
    let mut state = vec![State::Unborn; f.locals.len()];
    // One block today; when blocks are many, this walk becomes the fixed point
    // over the graph (6.8, step 3), and `state` one per block.
    for b in &f.blocks {
        for op in &b.ops {
            // Reads come BEFORE the definition they feed (L5 of 6.8: what is
            // only read is evaluated before a change begins): in `n = n + 1`
            // the `n` on the right is judged as it was.
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
            };
            for (l, at) in reads {
                if state[l] == State::Unborn {
                    return Err(no_value(f, l, at));
                }
            }
            let Some((l, at, how)) = define else { continue };
            let name = &f.locals[l].name;
            state[l] = match (state[l], how) {
                // `let` / `let mut`: it is born.
                (State::Unborn, Some(mutable)) => State::Alive { since: at.0, at, mutable, changed: false },
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
        }
    }
    // ** A `mut` that never changes is a NO, not a warning: `mut` is a promise
    // to whoever reads ("this one moves"), and a promise nobody keeps makes
    // every `mut` worth less. Rust only warns; TITAN++ says it, because the
    // reader -- and later the borrow checker -- trusts the word.
    for (l, st) in state.iter().enumerate() {
        if let State::Alive { since, at, mutable: true, changed: false } = *st {
            let name = &f.locals[l].name;
            return Err(Message::new(
                Code::NeverChanged,
                since,
                at.1,
                &format!("`{}` dice `mut` y no cambia nunca", name),
                &format!("en `fn {}()` ninguna linea le da otro valor: el `mut` promete algo que no pasa", f.name),
                &format!("quita el `mut`: let {} = ...", name),
            ));
        }
    }
    Ok(())
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
    fn each_function_has_its_own_names() {
        assert!(verdict("mod main \"x\"\nfn main()\n    let a = 1\n    otra()\nfn otra()\n    let a = 2\n    print(a)\n").is_ok());
    }
}
