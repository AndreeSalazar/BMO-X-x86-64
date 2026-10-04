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
use crate::tree::{Expr, Program, Stmt, Ty};

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
        params(p, f)?;
        let mut lines = Vec::new();
        flat(&f.body, &mut lines);
        for st in lines {
            // Every value the line reads: a call inside one must give back.
            let mut exprs: Vec<&Expr> = Vec::new();
            match st {
                Stmt::Call(c) => {
                    target(p, &c.callee, c.args.len(), c.line, c.col, false)?;
                    exprs.extend(&c.args);
                }
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
                    exprs.push(&l.value);
                }
                Stmt::Set(l) => exprs.push(&l.value),
                Stmt::If(i) => exprs.push(&i.cond),
                Stmt::While(w) => exprs.push(&w.cond),
                Stmt::For(fo) => {
                    if LIBRARY.contains(&fo.var.as_str()) || p.functions.iter().any(|g| g.name == fo.var) {
                        return Err(Message::new(
                            Code::Taken,
                            fo.var_at.0,
                            fo.var_at.1,
                            &format!("`{}` ya es el nombre de una funcion", fo.var),
                            &format!("un nombre dice UNA cosa: si fuera las dos, `{0}()` y `{0}` se confundirian al leer", fo.var),
                            "llama a la vuelta de otra forma: for i in range(...)",
                        ));
                    }
                    exprs.push(&fo.from);
                    exprs.push(&fo.to);
                }
                Stmt::Break { .. } | Stmt::Continue { .. } => {}
                Stmt::Return { value, line, col } => {
                    match (value, f.ret) {
                        (Some(_), None) => {
                            return Err(Message::new(
                                Code::Result,
                                *line,
                                *col,
                                &format!("`fn {}` no devuelve nada, y este `return` da un valor", f.name),
                                "una funcion dice en su primera linea si devuelve algo: `-> int`, `-> text` o `-> bool`",
                                &format!("o dilo arriba: fn {}(...) -> int, o quita el valor: return", f.name),
                            ));
                        }
                        (None, Some(t)) => {
                            return Err(Message::new(
                                Code::Result,
                                *line,
                                *col,
                                &format!("`fn {}` devuelve un {}, y este `return` no da ninguno", f.name, t.name()),
                                &format!("su primera linea promete `-> {}`: cada salida tiene que cumplirlo", t.name()),
                                &format!("return {}", match t { Ty::Int => "0", Ty::Text => "\"\"", Ty::Bool => "false" }),
                            ));
                        }
                        (Some(v), Some(_)) => exprs.push(v),
                        (None, None) => {}
                    }
                }
            }
            for e in exprs {
                let mut calls = Vec::new();
                calls_in(e, &mut calls);
                for (callee, n, line, col) in calls {
                    target(p, callee, n, line, col, true)?;
                }
            }
        }
    }
    endless(p)
}

/// A function's parameters: one name each, and never a function's name.
fn params(p: &Program, f: &crate::tree::Function) -> Result<(), Message> {
    for (i, a) in f.params.iter().enumerate() {
        let twice = f.params[..i].iter().any(|b| b.name == a.name);
        let taken = LIBRARY.contains(&a.name.as_str()) || p.functions.iter().any(|g| g.name == a.name);
        if twice || taken {
            return Err(Message::new(
                Code::Taken,
                a.line,
                a.col,
                &if twice { format!("`fn {}` tiene dos parametros `{}`", f.name, a.name) } else { format!("`{}` ya es el nombre de una funcion", a.name) },
                "un nombre dice UNA cosa: quien lee `{}` tiene que saber cual es",
                "llama al parametro de otra forma",
            ));
        }
    }
    Ok(())
}

/// Every call inside a value: (callee, how many values, line, column).
fn calls_in<'a>(e: &'a Expr, out: &mut Vec<(&'a str, usize, usize, usize)>) {
    match e {
        Expr::Call { callee, args, line, col } => {
            out.push((callee, args.len(), *line, *col));
            for a in args {
                calls_in(a, out);
            }
        }
        Expr::Bin { left, right, .. } => {
            calls_in(left, out);
            calls_in(right, out);
        }
        Expr::Neg { value, .. } | Expr::Not { value, .. } => calls_in(value, out),
        Expr::Int { .. } | Expr::Text { .. } | Expr::Name { .. } | Expr::Bool { .. } => {}
    }
}

/// A call to `callee` with `n` values: does it exist, does it take `n`, and
/// -- if it is used AS A VALUE -- does it give one back.
fn target(p: &Program, callee: &str, n: usize, line: usize, col: usize, as_value: bool) -> Result<(), Message> {
    let own = p.functions.iter().find(|g| g.name == callee);
    if own.is_none() && !LIBRARY.contains(&callee) {
        let known = LIBRARY.iter().copied().chain(p.functions.iter().map(|g| g.name.as_str()));
        let near = known.min_by_key(|k| distance(k, callee)).filter(|k| distance(k, callee) <= 3);
        return Err(Message::new(
            Code::Unknown,
            line,
            col,
            &format!("`{}` no existe", callee),
            "no es una `fn` de este fichero ni de la biblioteca (la biblioteca, hoy, es `print`)",
            &match near {
                Some(k) => format!("quisiste decir `{}`?", k),
                None => format!("define `fn {}()` en este fichero, o usa `print`", callee),
            },
        ));
    }
    let Some(g) = own else {
        // `print`: any number of values, and it gives nothing back.
        if as_value {
            return Err(Message::new(Code::Result, line, col, "`print` no devuelve nada", "escribe en la consola y ya: no hay un valor que guardar o sumar", "llamalo en su propia linea: print(...)"));
        }
        return Ok(());
    };
    if g.params.len() != n {
        let want: Vec<String> = g.params.iter().map(|a| format!("{}: {}", a.name, a.ty.name())).collect();
        return Err(Message::new(
            Code::Args,
            line,
            col,
            &format!("`{}` pide {} valor{}, y aqui se le {} {}", callee, g.params.len(), if g.params.len() == 1 { "" } else { "es" }, if n == 1 { "da" } else { "dan" }, n),
            &format!("su linea {} dice: fn {}({})", g.line, callee, want.join(", ")),
            &format!("{}({})", callee, g.params.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ")),
        ));
    }
    if as_value && g.ret.is_none() {
        return Err(Message::new(
            Code::Result,
            line,
            col,
            &format!("`{}` no devuelve nada, y aqui se usa como un valor", callee),
            &format!("su linea {} no dice `->`: hace algo, pero no da nada que guardar o sumar", g.line),
            &format!("llamala en su propia linea: {}(...), o di que devuelve: fn {}(...) -> int", callee, callee),
        ));
    }
    Ok(())
}

/// Every line of a body, the ones inside `if`, `else` and loops included, in
/// reading order (the `if` and loop lines themselves too).
fn flat<'a>(body: &'a [Stmt], out: &mut Vec<&'a Stmt>) {
    for st in body {
        out.push(st);
        match st {
            Stmt::If(i) => {
                flat(&i.then, out);
                flat(&i.other, out);
            }
            Stmt::While(w) => flat(&w.body, out),
            Stmt::For(f) => flat(&f.body, out),
            _ => {}
        }
    }
}

/// The calls of a body -- the ones behind an `if`, in a loop and inside a
/// value included: (callee, line, column), in reading order.
fn calls_of(body: &[Stmt]) -> Vec<(&str, usize, usize)> {
    let mut lines = Vec::new();
    flat(body, &mut lines);
    let mut out = Vec::new();
    for st in lines {
        let mut exprs: Vec<&Expr> = Vec::new();
        match st {
            Stmt::Call(c) => {
                out.push((c.callee.as_str(), c.line, c.col));
                exprs.extend(&c.args);
            }
            Stmt::Let(l) | Stmt::Set(l) => exprs.push(&l.value),
            Stmt::If(i) => exprs.push(&i.cond),
            Stmt::While(w) => exprs.push(&w.cond),
            Stmt::For(f) => {
                exprs.push(&f.from);
                exprs.push(&f.to);
            }
            Stmt::Return { value: Some(v), .. } => exprs.push(v),
            _ => {}
        }
        for e in exprs {
            let mut calls = Vec::new();
            calls_in(e, &mut calls);
            out.extend(calls.into_iter().map(|(c, _, l, k)| (c, l, k)));
        }
    }
    out
}



/// ** A CALL THAT COMES BACK TO ITSELF NEVER ENDS -- and an `if` does not
/// change that, until level 5.
///
/// Level 0 had no `if`, so nothing could decide to stop: `a -> b -> a` goes
/// round until the stack runs out, and on the machine that is a task killed
/// by a fault, not a message. Level 3 brings `if`, and this rule was written
/// knowing it would have to ask "is the call behind one?". The answer, said
/// precisely: IT DOES NOT MATTER YET. A `fn` takes nothing until level 5, and
/// nothing outside it changes, so every call of it decides EXACTLY as the
/// first one did: if the call back happens once, it happens always; if it
/// never happens, it is a line that does nothing. Either way, a NO -- and the
/// rule moves the day a call can carry a value (level 5). A loop (level 4)
/// does not change it either: the call back inside a `while` still starts
/// the same function from scratch.

fn endless(p: &Program) -> Result<(), Message> {
    // Level 5: only the functions that take NOTHING. One that takes a value
    // can decide differently at every call (`cuenta(n - 1)` and an `if n ==
    // 0`), and whether it ends is told by running it (`calc.rs`, T0066).
    let own = |name: &str| p.functions.iter().position(|f| f.name == name && f.params.is_empty());
    for (start, f) in p.functions.iter().enumerate() {
        if !f.params.is_empty() {
            continue;
        }
        // Depth-first from `start`, remembering the path: a call back to
        // `start` is the cycle, written with the names the author typed.
        let mut path = vec![start];
        let mut stack = vec![(start, 0usize)];
        let mut seen = vec![false; p.functions.len()];
        while let Some(&(at, next)) = stack.last() {
            let calls: Vec<usize> = calls_of(&p.functions[at].body).into_iter().filter_map(|c| own(c.0)).collect();
            if next >= calls.len() {
                stack.pop();
                path.pop();
                continue;
            }
            stack.last_mut().unwrap().1 += 1;
            let to = calls[next];
            if to == start {
                let (callee, line, col) = calls_of(&p.functions[at].body).into_iter().filter(|c| own(c.0).is_some()).nth(next).unwrap();
                let mut names: Vec<&str> = path.iter().map(|&i| p.functions[i].name.as_str()).collect();
                names.push(&f.name);
                let what = if names.len() == 2 {
                    format!("`{}` se llama a si misma y no termina nunca", f.name)
                } else {
                    format!("las llamadas vuelven a `{}` ({}) y no terminan nunca", f.name, names.join(" -> "))
                };
                return Err(Message::new(
                    Code::Endless,
                    line,
                    col,
                    &what,
                    "ninguna de estas `fn` recibe nada: cada vuelta decide igual que la primera, asi que ni un `if` la para -- si vuelve una vez, vuelve siempre",
                    &format!("quita la llamada a `{}()` de esta linea; para que pare sola, dale un valor que cambie (fn cuenta(n: int)) y un `if` que la corte", callee),
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
