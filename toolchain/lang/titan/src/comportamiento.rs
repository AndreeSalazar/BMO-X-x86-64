//! `comportamiento` -- WHAT A VALUE KNOWS HOW TO DO (level 10, `trait`).
//!
//! TITAN_MAESTRO 14.13: no inheritance -- composition and `trait`; generics
//! "with messages that are read": a generic says what it lacks in ONE
//! sentence, never a wall of template errors. And no new words: there is no
//! `impl` and no `self` among the 25, so:
//!
//! ```text
//!    trait Forma                     what a Forma KNOWS how to do: fn with
//!        fn area(f: Forma) -> dec    no body, the FIRST value of the trait
//!
//!    trait Forma for Circulo         how Circulo does it (`for` is already
//!        fn area(c: Circulo) -> dec  a word): the same fn, with a body
//!            return 3.14 * c.r * c.r
//!
//!    fn mide(f: Forma) -> dec        ANY value that does it
//!        return area(f)              a plain call: the value's type picks
//! ```
//!
//! ** A GENERIC IS JUDGED ONCE, AGAINST ITS TRAIT (Rust's way, not C++'s
//! templates): inside `mide`, `f` is "a Forma" and may only do what Forma
//! promises. Whoever calls `mide(roca)` with a Roca that does not keep Forma
//! hears it at the call, in one sentence (T0085) -- never from inside `mide`.
//!
//! This file checks the traits and their `trait ... for`, and turns each fn
//! of a `trait Forma for Circulo` into a plain fn of the program called
//! `area<Circulo>` (a name no one can write). Which one a call `area(f)`
//! runs is decided by the type of its first value: the IR gives each fn of a
//! trait a table of them (`ir.rs`, `dispatch`), and today the calculation,
//! which knows every value, picks from it. The day a value comes from outside
//! (E1), each call is decided by its type when compiling -- zero cost, as C++
//! promises; nothing here changes for that.
//!
//! ```text
//!    T0085  a value whose type does not keep the trait a parameter asks for
//!    T0086  a `trait ... for` that does not keep its trait: a fn missing,
//!           one too many, another signature, twice for one type
//!    T0087  a trait where only a parameter may have one: a trait is "any
//!           value that does this", so it is the type of a PARAMETER (and of
//!           the first value of its own fn), never of a `let`, a field or a
//!           result
//! ```

use crate::message::{Code, Message};
use crate::tree::{Function, Program, Stmt, Ty};

/// A fn's name without the module it comes from: `forma.area` -> `area`.
fn short(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// The name of the fn that is `method` of `ty`: `area<Circulo>`.
pub fn instance(method: &str, ty: &Ty) -> String {
    format!("{}<{}>", method, ty.name())
}

/// The traits checked, and every `trait ... for` turned into plain fn of
/// the program (`area<Circulo>`), appended to its functions.
pub fn expand(p: &mut Program) -> Result<(), Message> {
    traits(p)?;
    places(p)?;
    let mut extra = Vec::new();
    for (k, i) in p.impls.iter().enumerate() {
        let Some(t) = p.traits.iter().find(|t| t.name == i.trait_name) else {
            let near = p.traits.iter().map(|t| t.name.as_str()).min_by_key(|n| crate::check::distance(n, &i.trait_name)).filter(|n| crate::check::distance(n, &i.trait_name) <= 2);
            return Err(Message::new(
                Code::Unknown,
                i.line,
                i.col,
                &format!("no hay un `trait {}`", i.trait_name),
                "un `trait ... for` cumple un trait que existe: el que dice que fn hay que saber",
                &match near {
                    Some(n) => format!("quisiste decir `{}`?", n),
                    None => format!("declaralo: trait {}\n             fn ...(x: {}) -> ...", i.trait_name, i.trait_name),
                },
            ));
        };
        match &i.ty {
            Ty::Int | Ty::Text | Ty::Bool | Ty::Dec => {}
            Ty::Named(n) if p.types.iter().any(|d| &d.name == n) || p.enums.iter().any(|d| &d.name == n) => {}
            Ty::Named(n) if p.traits.iter().any(|d| &d.name == n) => return Err(place(i.line, i.col, n, "el tipo de un `trait ... for`")),
            Ty::Named(n) => {
                return Err(Message::new(Code::Unknown, i.line, i.col, &format!("el tipo `{}` no existe", n), "un `trait ... for` dice como CUMPLE el trait un tipo: un `type`, un `enum`, int, dec, text o bool", &format!("declaralo arriba: type {}\n             x: dec", n)))
            }
            other => {
                return Err(Message::new(
                    Code::BadImpl,
                    i.line,
                    i.col,
                    &format!("`trait {} for {}`: un trait lo cumple un TIPO con nombre", i.trait_name, other.name()),
                    "una tabla o un dec(p, s) es una forma de guardar, no un tipo con comportamiento propio",
                    "envuelvelo en un type: type Tabla\n             celdas: [int; 3]",
                ))
            }
        }
        if let Some(first) = p.impls[..k].iter().find(|j| j.trait_name == i.trait_name && j.ty == i.ty) {
            return Err(Message::new(
                Code::Twice,
                i.line,
                i.col,
                &format!("`trait {} for {}` esta dos veces", i.trait_name, i.ty.name()),
                &format!("la primera esta en la linea {}: una llamada no sabria cual correr", first.line),
                "deja una",
            ));
        }
        for g in &i.functions {
            if !t.methods.iter().any(|s| short(&s.name) == g.name) {
                return Err(Message::new(
                    Code::BadImpl,
                    g.line,
                    g.col,
                    &format!("`trait {}` no tiene una `fn {}`", t.name, g.name),
                    &format!("lo que el trait promete (linea {}): {}", t.line, t.methods.iter().map(|s| short(&s.name)).collect::<Vec<_>>().join(", ")),
                    "una fn propia del tipo va fuera del `trait ... for`, como cualquier otra fn",
                ));
            }
        }
        for s in &t.methods {
            let Some(g) = i.functions.iter().find(|g| g.name == short(&s.name)) else {
                return Err(Message::new(
                    Code::BadImpl,
                    i.line,
                    i.col,
                    &format!("`trait {} for {}` no tiene `fn {}`", t.name, i.ty.name(), short(&s.name)),
                    &format!("el trait la promete en su linea {}: quien llame a `{}` con un {} no tendria que correr", s.line, short(&s.name), i.ty.name()),
                    &format!("agregala: fn {}({}: {}{}) ...", short(&s.name), s.params[0].name, i.ty.name(), s.params[1..].iter().map(|a| format!(", {}: {}", a.name, a.ty.name())).collect::<String>()),
                ));
            };
            same_signature(t.name.as_str(), s, g, &i.ty)?;
            let mut f = g.clone();
            f.name = instance(&s.name, &i.ty);
            extra.push(f);
        }
    }
    p.functions.extend(extra);
    Ok(())
}

/// The `trait`s themselves: one name each, never a type's or a function's;
/// each fn named once in the whole program (a call `area(f)` must say ONE
/// thing), and with its first value of the trait itself.
fn traits(p: &Program) -> Result<(), Message> {
    for (k, t) in p.traits.iter().enumerate() {
        let clash = p.traits[..k].iter().any(|o| o.name == t.name) || p.types.iter().any(|d| d.name == t.name) || p.enums.iter().any(|d| d.name == t.name) || p.functions.iter().any(|f| f.name == t.name);
        if clash {
            return Err(Message::new(Code::Taken, t.line, t.col, &format!("`{}` ya es el nombre de otra cosa", t.name), "un nombre dice UNA cosa", "llama al trait de otra forma"));
        }
        for (j, s) in t.methods.iter().enumerate() {
            let twice = t.methods[..j].iter().any(|o| o.name == s.name) || p.traits[..k].iter().flat_map(|o| &o.methods).any(|o| o.name == s.name);
            let other = p.functions.iter().any(|f| f.name == s.name) || p.case(&s.name).is_some() || ["print", "len", "round"].contains(&s.name.as_str());
            if twice || other {
                return Err(Message::new(
                    Code::Taken,
                    s.line,
                    s.col,
                    &format!("`{}` ya es el nombre de otra cosa", s.name),
                    "una llamada `area(f)` dice UNA fn: si hubiera dos con ese nombre, quien lee no sabria cual (TITAN++ no sobrecarga)",
                    "llama a la fn de otra forma",
                ));
            }
            let receiver = s.params.first().is_some_and(|a| a.ty == Ty::Named(t.name.clone()));
            if !receiver {
                return Err(Message::new(
                    Code::BadImpl,
                    s.line,
                    s.col,
                    &format!("`fn {}` de `trait {}` no recibe un {}", short(&s.name), t.name, t.name),
                    "la fn de un trait la cumple cada tipo a su manera, y lo que dice QUE manera es el tipo de su primer valor",
                    &format!("fn {}(x: {}{}) ...", short(&s.name), t.name, s.params.iter().skip(1).map(|a| format!(", {}: {}", a.name, a.ty.name())).collect::<String>()),
                ));
            }
        }
    }
    Ok(())
}

/// A `trait ... for` fn against what its trait promises: as many values,
/// each passed the same way and of the same type (the first, of the type
/// that keeps the trait), and the same result.
fn same_signature(trait_name: &str, s: &crate::tree::Sig, g: &Function, ty: &Ty) -> Result<(), Message> {
    let promised = |s: &crate::tree::Sig| {
        let ps: Vec<String> = s.params.iter().map(|a| format!("{}{}: {}", if a.mode.word().is_empty() { String::new() } else { format!("{} ", a.mode.word()) }, a.name, a.ty.name())).collect();
        format!("fn {}({}){}", short(&s.name), ps.join(", "), s.ret.as_ref().map(|t| format!(" -> {}", t.name())).unwrap_or_default())
    };
    let ok = s.params.len() == g.params.len()
        && s.params.iter().zip(&g.params).enumerate().all(|(k, (a, b))| a.mode == b.mode && if k == 0 { &b.ty == ty } else { a.ty == b.ty })
        && s.ret == g.ret;
    if ok {
        return Ok(());
    }
    Err(Message::new(
        Code::BadImpl,
        g.line,
        g.col,
        &format!("`fn {}` no es la que promete `trait {}`", g.name, trait_name),
        &format!("el trait dice (linea {}): {} -- con un {} donde dice {}", s.line, promised(s), ty.name(), trait_name),
        "los mismos valores, pasados igual (copia, mut o take) y del mismo tipo, y el mismo resultado",
    ))
}

fn place(line: usize, col: usize, name: &str, here: &str) -> Message {
    Message::new(
        Code::TraitPlace,
        line,
        col,
        &format!("`{}` es un trait, y aqui va {}", name, here),
        "un trait dice QUE sabe hacer un valor, no cual es: por eso es el tipo de un PARAMETRO (`fn mide(f: Forma)`), que recibe cualquier valor que lo cumpla, y nunca de un `let`, un campo o un resultado",
        "usa el tipo de verdad (Circulo), o recibelo como parametro: fn usa(f: Forma)",
    )
}

/// Where a trait may be a type: the whole type of a parameter, and the
/// first value of its own fn. Anywhere else, T0087.
fn places(p: &Program) -> Result<(), Message> {
    let is_trait = |n: &str| p.traits.iter().any(|t| t.name == n);
    // A type with a trait INSIDE it (or being one, if `whole` is false).
    let inside = |t: &Ty, whole: bool| -> Option<String> {
        fn walk(t: &Ty, top: bool, whole: bool, is_trait: &dyn Fn(&str) -> bool) -> Option<String> {
            match t {
                Ty::Named(n) if is_trait(n) && !(top && whole) => Some(n.clone()),
                Ty::Table(inner, _) => walk(inner, false, whole, is_trait),
                _ => None,
            }
        }
        walk(t, true, whole, &is_trait)
    };
    let functions = p.functions.iter().chain(p.impls.iter().flat_map(|i| &i.functions));
    for f in functions {
        for a in &f.params {
            if let Some(n) = inside(&a.ty, true) {
                return Err(place(a.line, a.col, &n, "dentro de una tabla"));
            }
        }
        if let Some(n) = f.ret.as_ref().and_then(|t| inside(t, false)) {
            return Err(place(f.line, f.col, &n, &format!("como resultado de `fn {}`", f.name)));
        }
        let mut lets = Vec::new();
        typed_lets(&f.body, &mut lets);
        for (t, line, col) in lets {
            if let Some(n) = inside(t, false) {
                return Err(place(line, col, &n, "como tipo de un `let`"));
            }
        }
    }
    for t in &p.types {
        for fl in &t.fields {
            if let Some(n) = inside(&fl.ty, false) {
                return Err(place(fl.line, fl.col, &n, &format!("como campo de `type {}`", t.name)));
            }
        }
    }
    for e in &p.enums {
        for c in &e.cases {
            if let Some(n) = c.fields.iter().find_map(|t| inside(t, false)) {
                return Err(place(c.line, c.col, &n, &format!("como dato del caso `{}`", c.name)));
            }
        }
    }
    for t in &p.traits {
        for s in &t.methods {
            if let Some(n) = s.params.iter().skip(1).find_map(|a| inside(&a.ty, false)) {
                return Err(place(s.line, s.col, &n, &format!("en `fn {}` mas alla de su primer valor", short(&s.name))));
            }
            if let Some(n) = s.ret.as_ref().and_then(|r| inside(r, false)) {
                return Err(place(s.line, s.col, &n, &format!("como resultado de `fn {}`", short(&s.name))));
            }
        }
    }
    Ok(())
}

/// Every `let x: T` of a body, with where it is.
fn typed_lets<'a>(body: &'a [Stmt], out: &mut Vec<(&'a Ty, usize, usize)>) {
    for st in body {
        match st {
            Stmt::Let(l) => {
                if let Some(t) = &l.ty {
                    out.push((t, l.line, l.col));
                }
            }
            Stmt::If(i) => {
                typed_lets(&i.then, out);
                typed_lets(&i.other, out);
            }
            Stmt::While(w) => typed_lets(&w.body, out),
            Stmt::For(f) => typed_lets(&f.body, out),
            Stmt::Match { arms, .. } => {
                for a in arms {
                    typed_lets(&a.body, out);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::message::Code;

    const FORMA: &str = "mod main \"x\"\ntrait Forma\n    fn area(f: Forma) -> dec\ntype Circulo\n    r: dec\n";
    const CUMPLE: &str = "trait Forma for Circulo\n    fn area(c: Circulo) -> dec\n        return 3 * c.r * c.r\n";

    fn code(body: &str) -> Code {
        crate::lower(&format!("{}{}", FORMA, body)).unwrap_err().code
    }

    #[test]
    fn an_enum_and_an_int_keep_a_trait_too() {
        let src = "mod main \"x\"\ntrait Doble\n    fn doble(x: Doble) -> int\nenum Luz\n    Verde\n    Rojo\ntrait Doble for int\n    fn doble(n: int) -> int\n        return n * 2\ntrait Doble for Luz\n    fn doble(l: Luz) -> int\n        match l\n            Verde\n                return 2\n            Rojo\n                return 4\nfn suma(a: Doble, b: Doble) -> int\n    return doble(a) + doble(b)\nfn main()\n    print(suma(21, Rojo))\n";
        let m = crate::lower(src).unwrap();
        assert!(format!("{:?}", m.flat).contains("\"46\""), "{:?}", m.flat);
    }

    #[test]
    fn every_rule_of_a_trait_has_its_no() {
        // A fn of a trait whose first value is not of the trait.
        assert_eq!(crate::lower("mod main \"x\"\ntrait Forma\n    fn area(n: int) -> dec\nfn main()\n    print(1)\n").unwrap_err().code, Code::BadImpl);
        // Twice for one type; a fn the trait does not promise; another signature.
        assert_eq!(code(&format!("{}{}fn main()\n    print(1)\n", CUMPLE, CUMPLE)), Code::Twice);
        assert_eq!(code("trait Forma for Circulo\n    fn area(c: Circulo) -> dec\n        return c.r\n    fn lado(c: Circulo) -> dec\n        return c.r\nfn main()\n    print(1)\n"), Code::BadImpl);
        assert_eq!(code("trait Forma for Circulo\n    fn area(c: Circulo) -> int\n        return 1\nfn main()\n    print(1)\n"), Code::BadImpl);
        // A trait is the type of a parameter, and only of a whole one.
        assert_eq!(code(&format!("{}fn f(t: [Forma; 2])\n    print(1)\nfn main()\n    print(1)\n", CUMPLE)), Code::TraitPlace);
        assert_eq!(code(&format!("{}fn f(c: Circulo) -> Forma\n    return c\nfn main()\n    print(1)\n", CUMPLE)), Code::TraitPlace);
        // Inside a generic fn, only what the trait promises.
        assert_eq!(code(&format!("{}fn f(x: Forma)\n    print(x.r)\nfn main()\n    f(Circulo {{ r: 1.0 }})\n", CUMPLE)), Code::Field);
        // A fn of a trait takes its name: no fn or value may.
        assert_eq!(code(&format!("{}fn area()\n    print(1)\nfn main()\n    print(1)\n", CUMPLE)), Code::Taken);
        assert_eq!(code(&format!("{}fn main()\n    let area = 1\n    print(area)\n", CUMPLE)), Code::Taken);
        // `pub` goes on the trait, not on how a type keeps it.
        assert_eq!(code("pub trait Forma for Circulo\n    fn area(c: Circulo) -> dec\n        return c.r\nfn main()\n    print(1)\n"), Code::Expected);
    }
}
