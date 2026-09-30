//! `check` -- the names: is there a `main`, is every function defined once,
//! and does every call go somewhere. It knows the tree and nothing of text.
//!
//! In level 0 there are only two kinds of callee: `print` (the library's) and
//! a `fn` of the file, called without arguments.

use crate::message::{Code, Message};
use crate::tree::Program;

/// What the library gives in level 0.
const LIBRARY: [&str; 1] = ["print"];

/// The distance between two names, in edits: to say "did you mean".
fn distance(a: &str, b: &str) -> usize {
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
        for c in &f.body {
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
                    "no es una `fn` de este fichero ni de la biblioteca (en el nivel 0, la biblioteca es `print`)",
                    &match near {
                        Some(k) => format!("quisiste decir `{}`?", k),
                        None => format!("define `fn {}()` en este fichero, o usa `print`", c.callee),
                    },
                ));
            }
        }
    }
    Ok(())
}
