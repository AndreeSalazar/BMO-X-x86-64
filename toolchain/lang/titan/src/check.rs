//! `check` -- the names of FUNCTIONS: is there a `main`, is every function
//! defined once, does every call go somewhere, does none of them come back to
//! itself (T0053, `endless`), and no `let` takes a function's name (T0055).
//! It knows the tree and nothing of text.
//!
//! The names of VALUES -- has `area` got a value at this line, can it change
//! -- are not judged here: that is the checker's (`juez.rs`), on the IR,
//! because it is the same question the borrow checker asks.
//!
//! There are two kinds of callee: `print` (the library's) and a `fn` of the
//! file, called without arguments.

use crate::message::{Code, Message};
use crate::tree::{Program, Stmt};

/// What the library gives in level 0.
const LIBRARY: [&str; 1] = ["print"];

/// The distance between two names, in edits: to say "did you mean".
pub(crate) fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut prev = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let next = (row[j] + 1).min(row[j - 1] + 1).min(prev + cost);
            prev = row[j];
            row[j] = next;
        }
    }
    row[b.len()]
}

pub fn check(p: &Program) -> Result<(), Message> {
    for (i, f) in p.functions.iter().enumerate() {
        if let Some(first) = p.functions[..i].iter().find(|g| g.name == f.name) {
            return Err(Message::new(
                Code::Twice,
                f.line,
                f.col,
                &format!("`fn {}()` esta dos veces", f.name),
                &format!("la primera esta en la linea {}: una llamada no sabria a cual ir", first.line),
                "cambia el nombre de una de las dos",
            ));
        }
    }
    if !p.functions.iter().any(|f| f.name == "main") {
        return Err(Message::new(
            Code::NoMain,
            1,
            1,
            "no hay `fn main()`",
            "un programa empieza por `main`, y este fichero no la tiene",
            "fn main()\n             print(\"hola\")",
        ));
    }
    for f in &p.functions {
        for st in &f.body {
            let c = match st {
                Stmt::Call(c) => c,
                Stmt::Let(l) => {
                    if LIBRARY.contains(&l.name.as_str()) || p.functions.iter().any(|g| g.name == l.name) {
                        return Err(Message::new(
                            Code::Taken,
                            l.line,
                            l.col,
                            &format!("`{}` ya es el nombre de una funcion", l.name),
                            &format!("un nombre dice UNA cosa: si fuera las dos, `{0}()` y `{0}` se confundirian al leer", l.name),
                            &format!("llama al valor de otra forma: let {}_valor = ...", l.name),
                        ));
                    }
                    continue;
                }
                Stmt::Set(_) => continue,
            };
            let own = p.functions.iter().any(|g| g.name == c.callee);
            if own && !c.args.is_empty() {
                return Err(Message::new(
                    Code::NotYet,
                    c.line,
                    c.col,
                    "pasar valores a una `fn` propia llega en el nivel 5 (funciones con resultado)",
                    &format!("`{}` no recibe nada, y aqui se le pasan {}", c.callee, c.args.len()),
                    &format!("{}()", c.callee),
                ));
            }
            if !own && !LIBRARY.contains(&c.callee.as_str()) {
                let known = LIBRARY.iter().copied().chain(p.functions.iter().map(|g| g.name.as_str()));
                let near = known.min_by_key(|k| distance(k, &c.callee)).filter(|k| distance(k, &c.callee) <= 3);
                return Err(Message::new(
                    Code::Unknown,
                    c.line,
                    c.col,
                    &format!("`{}` no existe", c.callee),
                    "no es una `fn` de este fichero ni de la biblioteca (la biblioteca, hoy, es `print`)",
                    &match near {
                        Some(k) => format!("quisiste decir `{}`?", k),
                        None => format!("define `fn {}()` en este fichero, o usa `print`", c.callee),
                    },
                ));
            }
        }
    }
    endless(p)
}

/// ** A CALL THAT COMES BACK TO ITSELF NEVER ENDS -- in level 0.
///
/// There is no `if` yet, so nothing can decide to stop: `a -> b -> a` goes
/// round until the stack runs out, and on the machine that is a task killed
/// by a fault, not a message. Saying it here costs a walk of the calls; saying
/// it there costs a reboot to read a photo. The day `if` arrives (level 3) this
/// rule has to learn which calls are behind one -- and it says so in its COMO.
fn call_of(st: &Stmt) -> Option<&crate::tree::Call> {
    match st {
        Stmt::Call(c) => Some(c),
        _ => None,
    }
}

fn endless(p: &Program) -> Result<(), Message> {
    let own = |name: &str| p.functions.iter().position(|f| f.name == name);
    for (start, f) in p.functions.iter().enumerate() {
        // Depth-first from `start`, remembering the path: a call back to
        // `start` is the cycle, written with the names the author typed.
        let mut path = vec![start];
        let mut stack = vec![(start, 0usize)];
        let mut seen = vec![false; p.functions.len()];
        while let Some(&(at, next)) = stack.last() {
            let calls: Vec<usize> = p.functions[at].body.iter().filter_map(|st| call_of(st)).filter_map(|c| own(&c.callee)).collect();
            if next >= calls.len() {
                stack.pop();
                path.pop();
                continue;
            }
            stack.last_mut().unwrap().1 += 1;
            let to = calls[next];
            if to == start {
                let call = p.functions[at].body.iter().filter_map(|st| call_of(st)).filter(|c| own(&c.callee).is_some()).nth(next).unwrap();
                let mut names: Vec<&str> = path.iter().map(|&i| p.functions[i].name.as_str()).collect();
                names.push(&f.name);
                let what = if names.len() == 2 {
                    format!("`{}` se llama a si misma y no termina nunca", f.name)
                } else {
                    format!("las llamadas vuelven a `{}` ({}) y no terminan nunca", f.name, names.join(" -> "))
                };
                return Err(Message::new(
                    Code::Endless,
                    call.line,
                    call.col,
                    &what,
                    "en el nivel 0 no hay `if`: nada puede decidir parar, y la vuelta sigue hasta que se acaba la pila",
                    &format!("quita la llamada a `{}()` de esta linea; decidir cuando parar llega en el nivel 3", call.callee),
                ));
            }
            if !seen[to] {
                seen[to] = true;
                path.push(to);
                stack.push((to, 0));
            }
        }
    }
    Ok(())
}
