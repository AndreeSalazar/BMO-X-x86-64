//! `check` -- the names of FUNCTIONS: is there a `main`, is every function
//! defined once, does every call go somewhere, does none of them come back to
//! itself (T0053, `endless`), and no `let` takes a function's name (T0055).
//! It knows the tree and nothing of text.
//!
//! The names of VALUES -- has `area` got a value at this line, can it change
//! -- are not judged here: that is the checker's (`juez.rs`), on the IR,
//! because it is the same question the borrow checker asks.
//!
//! There are three kinds of callee: the library's (`print`, `len`, and --
//! E1, `PLAN_LA_ENTRADA` -- `lee`), a `fn`
//! of the file, and -- level 8 -- a CASE of an `enum`: `Circulo(2.0)` builds a
//! value, it does not run anything. And the `match` is judged here too: every
//! arm a case of ONE enum, none twice, and ALL of them (T0078, T0079).

use crate::ir::Director;
use crate::message::{Code, Message};
use crate::tree::{Arm, Expr, Mode, Program, Stmt, Ty};

/// What the library gives: `print` (level 0), `len` (6), and `lee` -- the
/// line typed on the program's own console (E1, `docs/plan/PLAN_LA_ENTRADA.md`)
/// -- and the lists and maps of level 13 (`docs/plan/PLAN_LISTAS_Y_MAPAS.md`).
const LIBRARY: [&str; 11] = ["print", "len", "lee", "numero", "byte", "push", "pop", "put", "get", "has", "remove"];

/// The library's fn of lists and maps (level 13): how many values each takes,
/// whether it CHANGES its first one (then it goes lent: `push(mut l, x)`), and
/// whether it gives a value back.
const COLLECTIONS: [(&str, usize, bool, bool); 6] = [
    ("push", 2, true, false),
    ("pop", 1, true, true),
    ("put", 3, true, false),
    ("remove", 2, true, false),
    ("get", 2, false, true),
    ("has", 2, false, true),
];

fn collection(name: &str) -> Option<(usize, bool, bool)> {
    COLLECTIONS.iter().find(|c| c.0 == name).map(|c| (c.1, c.2, c.3))
}

/// `mut nave.carga` or `mut t[i]`: a PART of a value lent -- what the library
/// of lists and maps accepts as its first value (level 13). The `mut x` it
/// starts from, if it is one.
pub(crate) fn lent_root(e: &Expr) -> Option<&Expr> {
    match e {
        Expr::Lend { .. } => Some(e),
        Expr::Field { base, .. } | Expr::Index { base, .. } => lent_root(base),
        _ => None,
    }
}

/// The indexes inside a lent part (`mut t[i + 1].x`): values like any other.
fn lent_indexes<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    match e {
        Expr::Field { base, .. } => lent_indexes(base, out),
        Expr::Index { base, index, .. } => {
            lent_indexes(base, out);
            out.push(index);
        }
        _ => {}
    }
}

/// The fn a trait promises, by its name (level 10): what `area(f)` calls.
fn method<'p>(p: &'p Program, name: &str) -> Option<&'p crate::tree::Sig> {
    p.traits.iter().flat_map(|t| &t.methods).find(|s| s.name == name)
}

/// A name that can be CALLED: the library's, a fn, or a trait's fn.
fn callable(p: &Program, name: &str) -> bool {
    LIBRARY.contains(&name) || p.functions.iter().any(|g| g.name == name) || method(p, name).is_some()
}

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
    enums(p)?;
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
                    modes(p, &c.args, &c.callee)?;
                    // The values themselves: `modes` looked at them as values
                    // given; a lent one has no call inside (a lent PART has
                    // its indexes, level 13).
                    for a in &c.args {
                        if lent_root(a).is_some() {
                            lent_indexes(a, &mut exprs);
                        } else {
                            exprs.push(a);
                        }
                    }
                }
                // `let x = pop(mut l)` (level 13): `pop` may stand alone as
                // the value of a `let` or an `=` -- it changes `l` AND gives.
                Stmt::Let(l) | Stmt::Set(l) if matches!(&l.value, Expr::Call { callee, .. } if callee == "pop") => {
                    let Expr::Call { callee, args, line, col } = &l.value else { unreachable!("the guard") };
                    if matches!(st, Stmt::Let(_)) {
                        not_a_case(p, &l.name, l.line, l.col)?;
                    }
                    target(p, callee, args.len(), *line, *col, true)?;
                    modes(p, args, callee)?;
                    for a in args {
                        lent_indexes(a, &mut exprs);
                    }
                }
                Stmt::Let(l) => {
                    not_a_case(p, &l.name, l.line, l.col)?;
                    if callable(p, &l.name) {
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
                    not_a_case(p, &fo.var, fo.var_at.0, fo.var_at.1)?;
                    if callable(p, &fo.var) {
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
                Stmt::Match { value, arms, line, col } => {
                    arms_cover(p, arms, *line, *col)?;
                    // `match pop(mut l)` (level 13): pop may also stand alone
                    // as the value a match looks at
                    match value {
                        Expr::Call { callee, args, line, col } if callee == "pop" => {
                            target(p, callee, args.len(), *line, *col, true)?;
                            modes(p, args, callee)?;
                            for a in args {
                                lent_indexes(a, &mut exprs);
                            }
                        }
                        _ => exprs.push(value),
                    }
                }
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
                    if callee == "pop" {
                        return Err(Message::new(
                            Code::Result,
                            line,
                            col,
                            "`pop` va solo en su linea, o como el valor de un `let` o de un `match`",
                            "`pop` CAMBIA la lista y a la vez da lo que quito: dentro de otra cuenta, quien lee no veria que la lista cambia ahi",
                            "let ultimo = pop(mut lista)",
                        ));
                    }
                    target(p, callee, n, line, col, true)?;
                }
                records(p, e)?;
                lends_only_in_calls(p, e)?;
                bare_cases(p, e)?;
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
        not_a_case(p, &a.name, a.line, a.col)?;
        let twice = f.params[..i].iter().any(|b| b.name == a.name);
        let taken = callable(p, &a.name);
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
        Expr::Int { .. } | Expr::Text { .. } | Expr::Name { .. } | Expr::Bool { .. } | Expr::Dec { .. } | Expr::Lend { .. } => {}
        Expr::Round { value, .. } => calls_in(value, out),
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
        Expr::Map { items, .. } => {
            for (k, v) in items {
                calls_in(k, out);
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
        Expr::Int { .. } | Expr::Text { .. } | Expr::Name { .. } | Expr::Bool { .. } | Expr::Dec { .. } | Expr::Lend { .. } => Ok(()),
        Expr::Round { value, .. } => records(p, value),
        Expr::Map { items, .. } => items.iter().try_for_each(|(k, v)| records(p, k).and_then(|_| records(p, v))),
    }
}

/// ** HOW EACH VALUE GOES TO A CALL (level 7), said at BOTH ends: a parameter
/// `mut t` is called `f(mut t)`, a `take t`, `f(take t)`, and a plain one with
/// the plain value. The reader of the CALL sees what can happen to `t` without
/// opening the function -- C's `f(&t)` only says "maybe". T0077 otherwise, and
/// for a `mut x` / `take x` anywhere but as the value of a call.
fn modes(p: &Program, args: &[Expr], callee: &str) -> Result<(), Message> {
    let params = p.functions.iter().find(|g| g.name == callee).map(|g| &g.params).or_else(|| method(p, callee).map(|s| &s.params));
    // `push(mut l, x)` (level 13): the library's fn that change their first
    // value say so at the call, as any fn does.
    let lib_mut = |i: usize| i == 0 && collection(callee).is_some_and(|c| c.1);
    for (i, a) in args.iter().enumerate() {
        let want = params.and_then(|ps| ps.get(i)).map(|x| x.mode).unwrap_or(if lib_mut(i) { Mode::Mut } else { Mode::Copy });
        // `push(mut nave.carga, x)` (13): a PART of a value, lent to the
        // library -- the `mut` is said at its start
        let a = match lent_root(a) {
            Some(root) if lib_mut(i) && params.is_none() => root,
            _ => a,
        };
        let (got, at) = match a {
            Expr::Lend { mode, line, col, .. } => (*mode, (*line, *col)),
            other => (Mode::Copy, other.at()),
        };
        if want != got {
            let pname = params.and_then(|ps| ps.get(i)).map(|x| x.name.as_str()).unwrap_or(if lib_mut(i) { "lista o mapa" } else { "?" });
            let how = match (want, a) {
                (Mode::Copy, Expr::Lend { name, .. }) => format!("{}(... {} ...): sin `{}`", callee, name, got.word()),
                (_, Expr::Name { name, .. }) => format!("{}(... {} {} ...)", callee, want.word(), name),
                _ => format!("`{} x` va con el nombre de un valor: {}({} tabla)", want.word(), callee, want.word()),
            };
            return Err(Message::new(
                Code::Mode,
                at.0,
                at.1,
                &match want {
                    Mode::Copy => format!("`{}` recibe `{}` como copia, y aqui se le {}", callee, pname, if got == Mode::Mut { "presta para cambiarlo" } else { "entrega" }),
                    Mode::Mut => format!("`{}` cambia su `{}`: hay que PRESTARSELO con `mut`", callee, pname),
                    Mode::Take => format!("`{}` se QUEDA su `{}`: hay que entregarselo con `take`", callee, pname),
                },
                "lo que le puede pasar a un valor se dice en los DOS lados, en la fn y en la llamada: quien lee la llamada lo ve sin abrir la fn",
                &how,
            ));
        }
        if !matches!(a, Expr::Lend { .. }) {
            lends_only_in_calls(p, a)?;
        }
    }
    Ok(())
}

/// A `mut x` / `take x` that is not the value of a call is T0077; and every
/// call inside a value has its modes checked.
fn lends_only_in_calls(p: &Program, e: &Expr) -> Result<(), Message> {
    match e {
        Expr::Lend { mode, name, line, col } => Err(Message::new(
            Code::Mode,
            *line,
            *col,
            &format!("`{} {}` solo va como valor de una llamada", mode.word(), name),
            "prestar o entregar es algo que se le hace a una LLAMADA: f(mut t). Fuera de una, no hay a quien",
            &format!("escribe el nombre solo: {}", name),
        )),
        Expr::Call { callee, args, .. } => modes(p, args, callee),
        Expr::Bin { left, right, .. } => {
            lends_only_in_calls(p, left)?;
            lends_only_in_calls(p, right)
        }
        Expr::Neg { value, .. } | Expr::Not { value, .. } | Expr::Repeat { item: value, .. } | Expr::Field { base: value, .. } | Expr::Round { value, .. } => lends_only_in_calls(p, value),
        Expr::Index { base, index, .. } => {
            lends_only_in_calls(p, base)?;
            lends_only_in_calls(p, index)
        }
        Expr::Table { items, .. } => items.iter().try_for_each(|i| lends_only_in_calls(p, i)),
        Expr::Record { fields, .. } => fields.iter().try_for_each(|(_, v)| lends_only_in_calls(p, v)),
        Expr::Map { items, .. } => items.iter().try_for_each(|(k, v)| lends_only_in_calls(p, k).and_then(|_| lends_only_in_calls(p, v))),
        Expr::Int { .. } | Expr::Text { .. } | Expr::Name { .. } | Expr::Bool { .. } | Expr::Dec { .. } => Ok(()),
    }
}

/// ** THE ENUMS (level 8). One name each, never a function's or a type's;
/// every case named ONCE in the whole file -- so a bare `Nada` says which
/// enum it is, with no `Forma::` in front; the values each case carries of a
/// type that exists; and none that contains itself (a value, not a pointer).
fn enums(p: &Program) -> Result<(), Message> {
    let builtin = ["int", "byte", "text", "bool", "dec"];
    for (i, e) in p.enums.iter().enumerate() {
        let clash = p.enums[..i].iter().any(|o| o.name == e.name) || p.types.iter().any(|t| t.name == e.name) || p.functions.iter().any(|f| f.name == e.name) || builtin.contains(&e.name.as_str());
        if clash {
            return Err(Message::new(Code::Taken, e.line, e.col, &format!("`{}` ya es el nombre de otra cosa", e.name), "un nombre dice UNA cosa", "llama al enum de otra forma"));
        }
        for (k, c) in e.cases.iter().enumerate() {
            let other_case = e.cases[..k].iter().any(|d| d.name == c.name) || p.enums[..i].iter().flat_map(|o| &o.cases).any(|d| d.name == c.name);
            let other = p.functions.iter().any(|f| f.name == c.name) || p.types.iter().any(|t| t.name == c.name) || p.enums.iter().any(|o| o.name == c.name) || LIBRARY.contains(&c.name.as_str()) || builtin.contains(&c.name.as_str());
            if other_case || other {
                return Err(Message::new(
                    Code::Taken,
                    c.line,
                    c.col,
                    &if other_case { format!("el caso `{}` ya esta en otro sitio", c.name) } else { format!("`{}` ya es el nombre de otra cosa", c.name) },
                    "un caso se escribe SOLO, sin el enum delante (`Nada`, no `Forma::Nada`): por eso su nombre es unico en todo el fichero",
                    "llama al caso de otra forma",
                ));
            }
            for f in &c.fields {
                known_ty(p, f, c.line, c.col)?;
            }
        }
    }
    for e in &p.enums {
        let mut seen = Vec::new();
        let inside = e.cases.iter().find(|c| c.fields.iter().any(|f| {
            seen.clear();
            holds_name(p, f, &e.name, &mut seen)
        }));
        if let Some(c) = inside {
            return Err(Message::new(
                Code::Case,
                c.line,
                c.col,
                &format!("`enum {}` se contiene a si mismo", e.name),
                &format!("su caso `{}` lleva otro `{}` dentro, que lleva otro, y otro: un valor sin fin", c.name, e.name),
                "un valor de TITAN++ guarda valores, no punteros: saca ese dato del caso",
            ));
        }
    }
    Ok(())
}

/// Does `t` hold a value of the type or enum `target`, directly or inside?
fn holds_name(p: &Program, t: &Ty, target: &str, seen: &mut Vec<String>) -> bool {
    match t {
        Ty::Named(n) if n == target => true,
        Ty::Named(n) if !seen.contains(n) => {
            seen.push(n.clone());
            p.types.iter().find(|d| &d.name == n).is_some_and(|d| d.fields.iter().any(|f| holds_name(p, &f.ty, target, seen)))
                || p.enums.iter().find(|d| &d.name == n).is_some_and(|d| d.cases.iter().flat_map(|c| &c.fields).any(|f| holds_name(p, f, target, seen)))
        }
        Ty::Table(inner, _) => holds_name(p, inner, target, seen),
        _ => false,
    }
}

/// A value's name that is a case's: T0055 -- `let Nada = 3` would make
/// `Nada` two things.
fn not_a_case(p: &Program, name: &str, line: usize, col: usize) -> Result<(), Message> {
    match p.case(name) {
        Some((e, _)) => Err(Message::new(
            Code::Taken,
            line,
            col,
            &format!("`{}` ya es un caso de `enum {}`", name, p.enums[e].name),
            &format!("un nombre dice UNA cosa: si fuera las dos, quien lee `{}` no sabria si es el caso o el valor", name),
            &format!("llama al valor de otra forma: {}", name.to_lowercase()),
        )),
        None => Ok(()),
    }
}

/// T0068 for a case: it carries `want` values and is given `got`.
fn carries(p: &Program, e: usize, v: usize, got: usize, line: usize, col: usize) -> Message {
    let case = &p.enums[e].cases[v];
    let tys: Vec<String> = case.fields.iter().map(Ty::name).collect();
    let n = case.fields.len();
    Message::new(
        Code::Args,
        line,
        col,
        &format!("`{}` lleva {} valor{}, y aqui se le {} {}", case.name, n, if n == 1 { "" } else { "es" }, if got == 1 { "da" } else { "dan" }, got),
        &format!("su linea {} dice: {}{}", case.line, case.name, if tys.is_empty() { String::new() } else { format!("({})", tys.join(", ")) }),
        &if n == 0 { case.name.clone() } else { format!("{}({})", case.name, vec!["..."; n].join(", ")) },
    )
}

/// A case that carries values, written bare (`Circulo` with no parentheses),
/// is a value missing its parts: T0068.
fn bare_cases(p: &Program, e: &Expr) -> Result<(), Message> {
    match e {
        Expr::Name { name, line, col } => match p.case(name) {
            Some((k, _)) if crate::prelude::is_opcion(&p.enums[k].name) => Err(library_case(name, *line, *col)),
            Some((k, v)) if !p.enums[k].cases[v].fields.is_empty() => Err(carries(p, k, v, 0, *line, *col)),
            _ => Ok(()),
        },
        Expr::Map { items, .. } => items.iter().try_for_each(|(k, v)| bare_cases(p, k).and_then(|_| bare_cases(p, v))),
        Expr::Bin { left, right, .. } | Expr::Index { base: left, index: right, .. } => {
            bare_cases(p, left)?;
            bare_cases(p, right)
        }
        Expr::Neg { value, .. } | Expr::Not { value, .. } | Expr::Repeat { item: value, .. } | Expr::Field { base: value, .. } | Expr::Round { value, .. } => bare_cases(p, value),
        Expr::Table { items, .. } | Expr::Call { args: items, .. } => items.iter().try_for_each(|i| bare_cases(p, i)),
        Expr::Record { fields, .. } => fields.iter().try_for_each(|(_, v)| bare_cases(p, v)),
        Expr::Int { .. } | Expr::Text { .. } | Expr::Bool { .. } | Expr::Dec { .. } | Expr::Lend { .. } => Ok(()),
    }
}

/// T0079: `Hay(3)` or `NoHay` written by the program -- only `get` and `pop`
/// make them (level 13).
fn library_case(name: &str, line: usize, col: usize) -> Message {
    Message::new(
        Code::Case,
        line,
        col,
        &format!("`{}` lo da la biblioteca, no se escribe", name),
        "`Hay(v)` y `NoHay` dicen si `get` o `pop` encontraron algo: un `Hay` escrito a mano diria que hay algo sin haberlo buscado",
        "para un valor que puede faltar, devuelve lo que da get o pop; para mirarlo, match get(m, k)",
    )
}

/// ** THE ARMS OF A `match` (level 8): each one a case of the SAME enum,
/// none twice, each taking as many names as its case carries values -- and
/// ALL the cases there. A case left out is T0078: the day someone adds
/// `Triangulo` to `Forma`, every `match` that does not say what to do with it
/// stops compiling and says WHERE. C's `switch` lets it fall through in
/// silence; a `_` would too, so TITAN++ has none.
fn arms_cover(p: &Program, arms: &[Arm], line: usize, col: usize) -> Result<(), Message> {
    let mut of: Option<usize> = None;
    for (i, a) in arms.iter().enumerate() {
        let Some((e, v)) = p.case(&a.case) else {
            let near = p.enums.iter().flat_map(|d| &d.cases).map(|c| c.name.as_str()).min_by_key(|k| distance(k, &a.case)).filter(|k| distance(k, &a.case) <= 2);
            return Err(Message::new(
                Code::Case,
                a.line,
                a.col,
                &format!("`{}` no es un caso de ningun `enum`", a.case),
                "cada rama de un `match` empieza por un caso: el nombre de uno de los que el `enum` dice",
                &match near {
                    Some(k) => format!("quisiste decir `{}`?", k),
                    None => "escribe uno de los casos del enum".to_string(),
                },
            ));
        };
        let def = &p.enums[e];
        match of {
            Some(first) if first != e => {
                return Err(Message::new(
                    Code::Case,
                    a.line,
                    a.col,
                    &format!("`{}` es de `enum {}`, y este `match` es de `enum {}`", a.case, def.name, p.enums[first].name),
                    "un `match` mira UN valor, y un valor es de un solo enum",
                    &format!("escribe los casos de `enum {}`: {}", p.enums[first].name, p.enums[first].cases.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")),
                ));
            }
            _ => of = Some(e),
        }
        if let Some(prev) = arms[..i].iter().find(|b| b.case == a.case) {
            return Err(Message::new(
                Code::Case,
                a.line,
                a.col,
                &format!("el caso `{}` esta dos veces en este `match`", a.case),
                &format!("la primera rama esta en la linea {}: esta no correria nunca", prev.line),
                "junta las dos en una",
            ));
        }
        let case = &def.cases[v];
        if a.binds.len() != case.fields.len() {
            let n = case.fields.len();
            let names = ["a", "b", "c", "d", "e", "f"];
            return Err(Message::new(
                Code::Case,
                a.line,
                a.col,
                &format!("`{}` lleva {} valor{}, y esta rama le pone {} nombre{}", case.name, n, if n == 1 { "" } else { "es" }, a.binds.len(), if a.binds.len() == 1 { "" } else { "s" }),
                &format!("su linea {} dice: {}{}", case.line, case.name, if n == 0 { String::new() } else { format!("({})", case.fields.iter().map(Ty::name).collect::<Vec<_>>().join(", ")) }),
                &if n == 0 { case.name.clone() } else { format!("{}({})", case.name, (0..n).map(|k| names.get(k).copied().unwrap_or("x")).collect::<Vec<_>>().join(", ")) },
            ));
        }
        for (k, (name, l, c)) in a.binds.iter().enumerate() {
            not_a_case(p, name, *l, *c)?;
            if a.binds[..k].iter().any(|b| &b.0 == name) || LIBRARY.contains(&name.as_str()) || p.functions.iter().any(|g| &g.name == name) {
                return Err(Message::new(Code::Taken, *l, *c, &format!("`{}` ya es el nombre de otra cosa", name), "un nombre dice UNA cosa: cada valor del caso, el suyo", "llama a este valor de otra forma"));
            }
        }
    }
    let e = of.expect("parse: a match has an arm");
    let def = &p.enums[e];
    let missing: Vec<&str> = def.cases.iter().filter(|c| !arms.iter().any(|a| a.case == c.name)).map(|c| c.name.as_str()).collect();
    if !missing.is_empty() {
        let first = def.cases.iter().find(|c| c.name == missing[0]).expect("listed above");
        let pattern = if first.fields.is_empty() { first.name.clone() } else { format!("{}({})", first.name, vec!["x"; first.fields.len()].join(", ")) };
        return Err(Message::new(
            Code::Missing,
            line,
            col,
            &if missing.len() == 1 { format!("este `match` no dice que hacer con `{}`", missing[0]) } else { format!("este `match` no dice que hacer con {}", missing.iter().map(|m| format!("`{}`", m)).collect::<Vec<_>>().join(", ")) },
            &format!("`enum {}` (linea {}) tiene {} casos, y un `match` los cubre TODOS: el que falta caeria donde nadie lo penso", def.name, def.line, def.cases.len()),
            &format!("agrega su rama:
             {}
                 ...", pattern),
        ));
    }
    Ok(())
}

fn unknown_type(p: &Program, name: &str, line: usize, col: usize) -> Message {
    let near = p.types.iter().map(|t| t.name.as_str()).chain(p.enums.iter().map(|e| e.name.as_str())).min_by_key(|k| distance(k, name)).filter(|k| distance(k, name) <= 2);
    Message::new(
        Code::Unknown,
        line,
        col,
        &format!("el tipo `{}` no existe", name),
        "los tipos son int, text, bool, dec, las tablas [T; n] y los `type` y `enum` de este fichero",
        &match near {
            Some(k) => format!("quisiste decir `{}`?", k),
            None => format!("declaralo arriba: type {}\n             x: dec", name),
        },
    )
}

/// Every named type a `Ty` mentions exists.
fn known_ty(p: &Program, t: &Ty, line: usize, col: usize) -> Result<(), Message> {
    match t {
        Ty::Named(n) if !p.types.iter().any(|d| &d.name == n) && !p.enums.iter().any(|d| &d.name == n) && !p.traits.iter().any(|d| &d.name == n) => Err(unknown_type(p, n, line, col)),
        Ty::Table(inner, _) | Ty::List(inner) | Ty::Opt(inner) => known_ty(p, inner, line, col),
        Ty::Map(k, v) => known_ty(p, k, line, col).and_then(|_| known_ty(p, v, line, col)),
        _ => Ok(()),
    }
}

/// The `type`s of the file: one name each, never a function's; every field
/// once, of a type that exists; and none that contains itself (a value that
/// holds itself would never end: TITAN++ keeps values, not pointers).
fn types(p: &Program) -> Result<(), Message> {
    for (i, t) in p.types.iter().enumerate() {
        if p.types[..i].iter().any(|u| u.name == t.name) || p.functions.iter().any(|f| f.name == t.name) || ["int", "byte", "text", "bool", "dec"].contains(&t.name.as_str()) {
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
    for t in &p.types {
        if let Some(f) = t.fields.iter().find(|f| holds_name(p, &f.ty, &t.name, &mut Vec::new())) {
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
    // ** LB7b: the DIRECTOR -- the resolver lets `director.x` through only
    // in a module whose header says `use director` (and the Titan.toml,
    // `screen`).
    if let Some(d) = Director::of(callee) {
        let example = match d {
            Director::Lamina => "if not director.lamina(18)",
            Director::Publica => "director.publica(f, n, posiciones, colores)",
            Director::Espera => "director.espera(16)",
            Director::Ventana => "if not director.ventana(320, 200)",
            Director::Pixel => "director.pixel(x, y, 255 * 65536)",
            Director::Rect => "director.rect(10, 10, 100, 50, 65280)",
            Director::Fila => "director.fila(y, pixeles)",
            Director::Presenta => "director.presenta()",
            Director::Toma => "if director.toma()",
            Director::Fichero => "if director.fichero(\"datos/ejemplo.lam\")",
            Director::Guarda => "if director.guarda(\"datos/resolucion.txt\", \"1280 720\")",
            Director::Crea => "if director.crea(\"datos/foto.bic\")",
            Director::Escribe => "director.escribe(66)",
            Director::Cierra => "if not director.cierra()",
            Director::Medida => "let n = director.medida()",
            Director::Byte => "let b = director.byte(i)",
            Director::Evento => "let que = director.evento()",
            Director::Codigo => "let tecla = director.codigo()",
            Director::RatonX => "let x = director.raton_x()",
            Director::RatonY => "let y = director.raton_y()",
            Director::Botones => "let b = director.botones()",
            Director::SeVe => "if director.se_ve()",
            Director::Letra => "x = director.letra(x, y, 65, 1, 16777215)",
            Director::Texto => "x = director.texto(8, 8, \"hola\", 1, 16777215)",
        };
        if n != d.takes() {
            return Err(Message::new(Code::Args, line, col, &format!("`{}` pide {} valor{}, y aqui se le {} {}", callee, d.takes(), if d.takes() == 1 { "" } else { "es" }, if n == 1 { "da" } else { "dan" }, n), "es del DIRECTOR: la lamina de VERRANO (LB7b) y la ventana (F1)", example));
        }
        if as_value && !d.gives() {
            return Err(Message::new(Code::Result, line, col, &format!("`{}` no devuelve nada, y aqui se usa como un valor", callee), if d == Director::Espera { "duerme hasta el siguiente fotograma y ya: no hay nada que guardar" } else if d == Director::Escribe { "escribe un byte y ya: si todo entro lo dice `director.cierra()`" } else { "pinta en la ventana y ya: no hay nada que guardar" }, example));
        }
        return Ok(());
    }
    if let Some(what) = callee.strip_prefix("director.") {
        let names: Vec<&str> = Director::ALL.iter().map(|d| d.name().trim_start_matches("director.")).collect();
        let near = names.iter().copied().min_by_key(|k| distance(k, what)).filter(|k| distance(k, what) <= 2);
        return Err(Message::new(
            Code::Unknown,
            line,
            col,
            &format!("`director` no tiene `{}`", what),
            &format!("lo que el director sabe hoy: {} (la lamina de VERRANO y la ventana)", names.join(", ")),
            &match near {
                Some(k) => format!("quisiste decir `director.{}`?", k),
                None => "if not director.lamina(18)".to_string(),
            },
        ));
    }
    if let Some((e, _)) = p.case(callee).filter(|(e, _)| crate::prelude::is_opcion(&p.enums[*e].name)) {
        let _ = e;
        return Err(library_case(callee, line, col));
    }
    if let Some((want, _, gives)) = collection(callee).filter(|_| !p.functions.iter().any(|g| g.name == callee)) {
        // The lists and maps of level 13: their values, and what they give.
        let example = match callee {
            "push" => "push(mut nombres, \"ana\")",
            "pop" => "let ultimo = pop(mut nombres)",
            "put" => "put(mut stock, \"pan\", 3)",
            "remove" => "remove(mut stock, \"pan\")",
            "get" => "match get(stock, \"pan\")",
            _ => "if has(stock, \"pan\")",
        };
        if n != want {
            return Err(Message::new(Code::Args, line, col, &format!("`{}` pide {} valor{}, y aqui se le {} {}", callee, want, if want == 1 { "" } else { "es" }, if n == 1 { "da" } else { "dan" }, n), "es de la biblioteca de listas y mapas (nivel 13)", example));
        }
        if as_value && !gives {
            return Err(Message::new(Code::Result, line, col, &format!("`{}` no devuelve nada, y aqui se usa como un valor", callee), "cambia la lista o el mapa que se le presta: no hay nada que guardar", example));
        }
        if !as_value && gives && callee != "pop" {
            return Err(Message::new(Code::Result, line, col, &format!("`{}(...)` da un valor, y aqui nadie lo mira", callee), "lo que dice se perderia nada mas llegar", example));
        }
        return Ok(());
    }
    if let Some((e, v)) = p.case(callee) {
        let case = &p.enums[e].cases[v];
        if case.fields.len() != n {
            return Err(carries(p, e, v, n, line, col));
        }
        if !as_value {
            return Err(Message::new(
                Code::Result,
                line,
                col,
                &format!("`{}(...)` es un valor, y aqui esta solo en su linea", callee),
                &format!("un caso de `enum {}` no hace nada: CONSTRUYE un valor, y un valor que nadie guarda se pierde", p.enums[e].name),
                &format!("guardalo: let forma = {}(...)", callee),
            ));
        }
        return Ok(());
    }
    if let Some(s) = method(p, callee) {
        // A trait's fn (level 10): as many values as it promises.
        if s.params.len() != n {
            let want: Vec<String> = s.params.iter().map(|a| format!("{}: {}", a.name, a.ty.name())).collect();
            return Err(Message::new(Code::Args, line, col, &format!("`{}` pide {} valores, y aqui se le dan {}", callee, s.params.len(), n), &format!("su trait lo dice en la linea {}: fn {}({})", s.line, callee, want.join(", ")), &format!("{}({})", callee, s.params.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "))));
        }
        if as_value && s.ret.is_none() {
            return Err(Message::new(Code::Result, line, col, &format!("`{}` no devuelve nada, y aqui se usa como un valor", callee), &format!("su trait no dice `->` en la linea {}", s.line), &format!("llamala en su propia linea: {}(...)", callee)));
        }
        return Ok(());
    }
    let own = p.functions.iter().find(|g| g.name == callee);
    if own.is_none() && !LIBRARY.contains(&callee) {
        let known = LIBRARY.iter().copied().chain(p.functions.iter().filter(|g| !g.name.contains('<')).map(|g| g.name.as_str())).chain(p.traits.iter().flat_map(|t| t.methods.iter().map(|s| s.name.as_str())));
        let near = known.min_by_key(|k| distance(k, callee)).filter(|k| distance(k, callee) <= 3);
        return Err(Message::new(
            Code::Unknown,
            line,
            col,
            &format!("`{}` no existe", callee),
"no es una `fn` de este fichero, ni un caso de sus `enum`, ni de la biblioteca (la biblioteca, hoy, es `print`, `len`, `lee`, `numero` y las de listas y mapas: `push`, `pop`, `put`, `get`, `has`, `remove`)",
            &match near {
                Some(k) => format!("quisiste decir `{}`?", k),
                None => format!("define `fn {}()` en este fichero, o usa `print`", callee),
            },
        ));
    }
    let Some(g) = own else {
        if callee == "lee" {
            // `lee()`: the line typed on the program's own console, as a
            // text. Nothing in, a text out -- and a line read must be kept.
            if n != 0 {
                return Err(Message::new(Code::Args, line, col, &format!("`lee` no pide valores, y aqui se le dan {}", n), "`lee` trae la linea que se teclea en la consola del programa: no hay nada que darle", "let nombre = lee()"));
            }
            if !as_value {
                return Err(Message::new(Code::Result, line, col, "`lee()` trae una linea, y aqui nadie la guarda", "lo que se teclea se perderia nada mas llegar", "guardala: let linea = lee()"));
            }
            return Ok(());
        }
        if callee == "numero" {
            // `numero(t)`: the case of the prelude -- Es(n) or NoEs.
            if n != 1 {
                return Err(Message::new(Code::Args, line, col, &format!("`numero` pide 1 valor, y aqui se le dan {}", n), "`numero` dice si UN texto es un numero entero", "match numero(linea)"));
            }
            if !as_value {
                return Err(Message::new(Code::Result, line, col, "`numero(...)` da un caso, y aqui nadie lo mira", "Es(n) o NoEs: lo que dice se perderia", "match numero(linea)"));
            }
            return Ok(());
        }
        if callee == "byte" {
            // `byte(x)` (TA1): an int as a byte -- 0 to 255, or a NO.
            if n != 1 {
                return Err(Message::new(Code::Args, line, col, &format!("`byte` pide 1 valor, y aqui se le dan {}", n), "`byte` vuelve UN numero entero un byte (de 0 a 255)", "byte(200)"));
            }
            if !as_value {
                return Err(Message::new(Code::Result, line, col, "`byte(...)` da un byte, y aqui nadie lo guarda", "el byte se perderia nada mas hacerse", "guardalo: let b = byte(200)"));
            }
            return Ok(());
        }
        if callee == "len" {
            // `len(tabla)`: how many cells; one value in, an `int` out.
            if n != 1 {
                return Err(Message::new(Code::Args, line, col, &format!("`len` pide 1 valor, y aqui se le dan {}", n), "`len` dice cuantas celdas tiene UNA tabla, lista o mapa", "len(planetas)"));
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
            Stmt::Match { arms, .. } => {
                for a in arms {
                    flat(&a.body, out);
                }
            }
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
            Stmt::Match { value, .. } => exprs.push(value),
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
