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
const LIBRARY: [&str; 2] = ["print", "len"];

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
    types(p)?;
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
                    if let Some(t) = &fo.over {
                        exprs.push(t);
                    }
                }
                Stmt::Break { .. } | Stmt::Continue { .. } => {}
                Stmt::SetAt { path, value, .. } => {
                    for st in path {
                        if let crate::tree::Step::Index(i) = st {
                            exprs.push(i);
                        }
                    }
                    exprs.push(value);
                }
                Stmt::Return { value, line, col } => {
                    match (value, &f.ret) {
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
                                &format!("return {}", match t { Ty::Int => "0", Ty::Text => "\"\"", Ty::Bool => "false", Ty::Dec => "0.0", _ => "..." }),
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
                records(p, e)?;
            }
        }
    }
    endless(p)
}

/// A function's parameters: one name each, never a function's name, and of
/// a type that exists.
fn params(p: &Program, f: &crate::tree::Function) -> Result<(), Message> {
    if let Some(t) = &f.ret {
        known_ty(p, t, f.line, f.col)?;
    }
    for (i, a) in f.params.iter().enumerate() {
        known_ty(p, &a.ty, a.line, a.col)?;
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
        Expr::Int { .. } | Expr::Text { .. } | Expr::Name { .. } | Expr::Bool { .. } | Expr::Dec { .. } => {}
        Expr::Table { items, .. } => {
            for i in items {
                calls_in(i, out);
            }
        }
        Expr::Repeat { item, .. } | Expr::Field { base: item, .. } => calls_in(item, out),
        Expr::Index { base, index, .. } => {
            calls_in(base, out);
            calls_in(index, out);
        }
        Expr::Record { fields, .. } => {
            for (_, v) in fields {
                calls_in(v, out);
            }
        }
    }
}

/// Every `Nave { ... }` inside a value: the type exists, and it names each
/// of its fields exactly once (T0073).
fn records(p: &Program, e: &Expr) -> Result<(), Message> {
    match e {
        Expr::Record { name, fields, line, col } => {
            let Some(t) = p.types.iter().find(|t| &t.name == name) else {
                return Err(unknown_type(p, name, *line, *col));
            };
            for (i, (k, v)) in fields.iter().enumerate() {
                if !t.fields.iter().any(|f| &f.name == k) {
                    return Err(Message::new(
                        Code::Field,
                        *line,
                        *col,
                        &format!("`{}` no tiene un campo `{}`", name, k),
                        &format!("los campos de `type {}` (linea {}) son: {}", name, t.line, t.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")),
                        "escribe uno de esos, o agregalo al `type`",
                    ));
                }
                if fields[..i].iter().any(|(j, _)| j == k) {
                    return Err(Message::new(Code::Field, *line, *col, &format!("el campo `{}` esta dos veces", k), "un campo, un valor", "deja uno"));
                }
                records(p, v)?;
            }
            if let Some(missing) = t.fields.iter().find(|f| !fields.iter().any(|(k, _)| k == &f.name)) {
                return Err(Message::new(
                    Code::Field,
                    *line,
                    *col,
                    &format!("a `{}` le falta el campo `{}`", name, missing.name),
                    "un registro nace ENTERO: un campo sin valor seria un valor que nadie dio (TITAN++ no tiene null)",
                    &format!("dale su valor: {}: ...", missing.name),
                ));
            }
            Ok(())
        }
        Expr::Bin { left, right, .. } => {
            records(p, left)?;
            records(p, right)
        }
        Expr::Neg { value, .. } | Expr::Not { value, .. } | Expr::Repeat { item: value, .. } | Expr::Field { base: value, .. } => records(p, value),
        Expr::Index { base, index, .. } => {
            records(p, base)?;
            records(p, index)
        }
        Expr::Table { items, .. } | Expr::Call { args: items, .. } => items.iter().try_for_each(|i| records(p, i)),
        Expr::Int { .. } | Expr::Text { .. } | Expr::Name { .. } | Expr::Bool { .. } | Expr::Dec { .. } => Ok(()),
    }
}

fn unknown_type(p: &Program, name: &str, line: usize, col: usize) -> Message {
    let near = p.types.iter().map(|t| t.name.as_str()).min_by_key(|k| distance(k, name)).filter(|k| distance(k, name) <= 2);
    Message::new(
        Code::Unknown,
        line,
        col,
        &format!("el tipo `{}` no existe", name),
        "los tipos son int, text, bool, dec, las tablas [T; n] y los `type` de este fichero",
        &match near {
            Some(k) => format!("quisiste decir `{}`?", k),
            None => format!("declaralo arriba: type {}\n             x: dec", name),
        },
    )
}

/// Every named type a `Ty` mentions exists.
fn known_ty(p: &Program, t: &Ty, line: usize, col: usize) -> Result<(), Message> {
    match t {
        Ty::Named(n) if !p.types.iter().any(|d| &d.name == n) => Err(unknown_type(p, n, line, col)),
        Ty::Table(inner, _) => known_ty(p, inner, line, col),
        _ => Ok(()),
    }
}

/// The `type`s of the file: one name each, never a function's; every field
/// once, of a type that exists; and none that contains itself (a value that
/// holds itself would never end: TITAN++ keeps values, not pointers).
fn types(p: &Program) -> Result<(), Message> {
    for (i, t) in p.types.iter().enumerate() {
        if p.types[..i].iter().any(|u| u.name == t.name) || p.functions.iter().any(|f| f.name == t.name) || ["int", "text", "bool", "dec"].contains(&t.name.as_str()) {
            return Err(Message::new(Code::Taken, t.line, t.col, &format!("`{}` ya es el nombre de otra cosa", t.name), "un nombre dice UNA cosa", "llama al tipo de otra forma"));
        }
        for (k, f) in t.fields.iter().enumerate() {
            if t.fields[..k].iter().any(|g| g.name == f.name) {
                return Err(Message::new(Code::Field, f.line, f.col, &format!("`type {}` tiene dos campos `{}`", t.name, f.name), "un campo, un nombre", "llama a uno de otra forma"));
            }
            known_ty(p, &f.ty, f.line, f.col)?;
        }
    }
    // A type that contains itself, directly or through others.
    fn holds(p: &Program, t: &Ty, target: &str, seen: &mut Vec<String>) -> bool {
        match t {
            Ty::Named(n) if n == target => true,
            Ty::Named(n) if !seen.contains(n) => {
                seen.push(n.clone());
                p.types.iter().find(|d| &d.name == n).is_some_and(|d| d.fields.iter().any(|f| holds(p, &f.ty, target, seen)))
            }
            Ty::Table(inner, _) => holds(p, inner, target, seen),
            _ => false,
        }
    }
    for t in &p.types {
        if let Some(f) = t.fields.iter().find(|f| holds(p, &f.ty, &t.name, &mut Vec::new())) {
            return Err(Message::new(
                Code::Field,
                f.line,
                f.col,
                &format!("`type {}` se contiene a si mismo", t.name),
                &format!("su campo `{}` lleva otro `{}` dentro, que lleva otro, y otro: un valor sin fin", f.name, t.name),
                "un registro guarda valores, no punteros: saca ese campo, o guarda solo lo que hace falta de el",
            ));
        }
    }
    Ok(())
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
            "no es una `fn` de este fichero ni de la biblioteca (la biblioteca, hoy, es `print` y `len`)",
            &match near {
                Some(k) => format!("quisiste decir `{}`?", k),
                None => format!("define `fn {}()` en este fichero, o usa `print`", callee),
            },
        ));
    }
    let Some(g) = own else {
        if callee == "len" {
            // `len(tabla)`: how many cells; one value in, an `int` out.
            if n != 1 {
                return Err(Message::new(Code::Args, line, col, &format!("`len` pide 1 valor, y aqui se le dan {}", n), "`len` dice cuantas celdas tiene UNA tabla", "len(planetas)"));
            }
            return Ok(());
        }
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
                exprs.extend(f.over.as_ref());
            }
            Stmt::Return { value: Some(v), .. } => exprs.push(v),
            Stmt::SetAt { path, value, .. } => {
                for st in path {
                    if let crate::tree::Step::Index(i) = st {
                        exprs.push(i);
                    }
                }
                exprs.push(value);
            }
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
