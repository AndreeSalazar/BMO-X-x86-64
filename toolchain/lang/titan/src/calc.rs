//! `calc` -- what is known when compiling is calculated when compiling.
//!
//! C++'s `constexpr` (TITAN_MAESTRO 2.1), without the keyword: in level 1
//! every value is known before the program runs -- there is nothing to read
//! from outside yet -- so the whole calculation happens HERE, and the `.bex`
//! carries the results. Exact, or a NO with its four parts:
//!
//! ```text
//!    T0060  the result does not fit in 64 bits: overflow is an ERROR, never a
//!           wrap (INTI's rule 1, and Ada's)
//!    T0061  a division (or %) by zero
//!    T0062  a division that is not whole: 7 / 2 is NOT 3. The exact decimals
//!           (dec, COBOL's) arrive with the types; until then, `%` gives the
//!           rest. Nothing is ever rounded in silence (2b.1)
//!    T0063  a text with a number: "a" + 1 is not "a1" (JavaScript's most
//!           famous fault, 14.13). Texts add to texts; print takes both
//!    T0064  a `mut` that would change KIND: `n = "hola"` after `let mut n = 0`
//!           (level 2: a value changes, its class never does)
//!    T0065  a YES/NO was asked and something else came: `if vidas` with a
//!           number, `not "a"`, `3 and true` (level 3)
//! ```
//!
//! It runs AFTER the checker (`juez.rs`): every local it reads has a value by
//! then, so a missing one here is a bug of this crate, not of the program.
//!
//! ** LEVEL 3: TWO PASSES, and the difference is the whole point.
//!
//! ```text
//!    CLASSES   every block, the ones that will never run too: a number, a
//!              text or a yes/no, and whether they fit (T0063 T0064 T0065).
//!              A line that is wrong in KIND is wrong wherever it sits
//!    VALUES    only the way the program really goes: every `if` is decided
//!              HERE (nothing comes from outside yet), the other side is
//!              DEAD (`Block::dead`) and leaves no bytes. A division by zero
//!              in a dead block is not an error: `if d != 0` guarding
//!              `10 / d` is exactly what an `if` is for
//! ```
//!
//! And `and` / `or` stop as soon as they know (`d != 0 and 10 / d > 1` does
//! not divide when `d` is 0), for the same reason.

use crate::ir::{At, End, Function, Module, Op, Value};
use crate::message::{Code, Message};

/// A value, calculated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Const {
    Int(i64),
    Text(String),
    Bool(bool),
}

impl Const {
    /// As `print` writes it.
    pub fn show(&self) -> String {
        match self {
            Const::Int(n) => n.to_string(),
            Const::Text(t) => t.clone(),
            Const::Bool(b) => b.to_string(),
        }
    }

    fn class(&self) -> Class {
        match self {
            Const::Int(_) => Class::Int,
            Const::Text(_) => Class::Text,
            Const::Bool(_) => Class::Bool,
        }
    }
}

/// What a value IS, without knowing which one: the first pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Int,
    Text,
    Bool,
}

impl Class {
    fn name(self) -> &'static str {
        match self {
            Class::Int => "un numero",
            Class::Text => "un texto",
            Class::Bool => "un si-o-no (true / false)",
        }
    }
}

/// The module with every value calculated: each `Let`, each part of each
/// `Write` and each condition of the blocks that run becomes `Value::Int`,
/// `Value::Text` or `Value::Bool`; the blocks that never run are `dead`.
pub fn fold(m: &Module) -> Result<Module, Message> {
    let mut out = m.clone();
    for f in &mut out.functions {
        classes(f)?;
        values(f)?;
    }
    Ok(out)
}

/// The first pass: the class of every local and every value, in EVERY block.
/// The blocks are in reading order, and a local that dies (`Drop`) forgets
/// its class: the next one with that name is another value.
fn classes(f: &Function) -> Result<(), Message> {
    let mut known: Vec<Option<Class>> = vec![None; f.locals.len()];
    for b in &f.blocks {
        for op in &b.ops {
            match op {
                Op::Let { local, value, .. } => known[*local] = Some(class(value, &known)?),
                Op::Set { local, value, at } => {
                    let c = class(value, &known)?;
                    // ** A `mut` changes its VALUE, never its kind: a number
                    // stays a number (Python lets `x = 5` become `x = "hola"`;
                    // the checker could not say what x is).
                    if let Some(old) = known[*local] {
                        if old != c {
                            return Err(Message::new(
                                Code::Retype,
                                at.0,
                                at.1,
                                &format!("aqui `{}` pasaria de {} a {}", f.locals[*local].name, old.name(), c.name()),
                                "un `mut` cambia de valor, no de clase: el que lee tiene que saber siempre que es",
                                "dale un valor de la misma clase, o usa otro nombre para el nuevo",
                            ));
                        }
                    }
                }
                Op::Write { parts, .. } => {
                    for p in parts {
                        class(p, &known)?;
                    }
                }
                Op::Call { .. } => {}
                Op::Drop { local, .. } => known[*local] = None,
            }
        }
        if let End::Branch { cond, at, .. } = &b.end {
            let c = class(cond, &known)?;
            if c != Class::Bool {
                return Err(Message::new(
                    Code::NotBool,
                    at.0,
                    at.1,
                    &format!("un `if` pregunta SI o NO, y aqui hay {}", c.name()),
                    "TITAN++ no adivina que quiere decir un numero o un texto como condicion (en C, 0 es NO; en Python, \"\" tambien)",
                    &format!("di la pregunta entera: if {} > 0   o   if {} == ...", written(cond, f), written(cond, f)),
                ));
            }
        }
    }
    Ok(())
}

/// A value as the author wrote it, with the NAMES (the IR numbers them):
/// for the COMO of a message.
fn written(v: &Value, f: &Function) -> String {
    match v {
        Value::Int(n, _) => n.to_string(),
        Value::Text(t, _) => format!("{:?}", t),
        Value::Bool(b, _) => b.to_string(),
        Value::Local(l, _) => f.locals[*l].name.clone(),
        Value::Bin(op, l, r, _) => format!("{} {} {}", written(l, f), op, written(r, f)),
        Value::Neg(v, _) => format!("-{}", written(v, f)),
        Value::Not(v, _) => format!("not {}", written(v, f)),
    }
}

/// The class of a value, or the NO that says why it has none.
pub fn class(v: &Value, known: &[Option<Class>]) -> Result<Class, Message> {
    Ok(match v {
        Value::Int(..) => Class::Int,
        Value::Text(..) => Class::Text,
        Value::Bool(..) => Class::Bool,
        Value::Local(l, _) => known[*l].expect("juez: every local read has a value"),
        Value::Neg(inner, at) => match class(inner, known)? {
            Class::Int => Class::Int,
            c => return Err(Message::new(Code::Mixed, at.0, at.1, &format!("{} no tiene signo", c.name()), &format!("aqui hay {} con un `-` delante", c.name()), "el `-` va delante de un numero")),
        },
        Value::Not(inner, at) => match class(inner, known)? {
            Class::Bool => Class::Bool,
            c => {
                return Err(Message::new(
                    Code::NotBool,
                    at.0,
                    at.1,
                    &format!("`not` da la vuelta a un si-o-no, y aqui hay {}", c.name()),
                    "`not` cambia true por false y false por true; un numero o un texto no tienen vuelta",
                    "pregunta primero: not (vidas > 0)",
                ))
            }
        },
        Value::Bin(op, l, r, at) => {
            let (a, b) = (class(l, known)?, class(r, known)?);
            match (*op, a, b) {
                ("+" | "-" | "*" | "/" | "%", Class::Int, Class::Int) => Class::Int,
                ("+", Class::Text, Class::Text) => Class::Text,
                ("==" | "!=", x, y) if x == y => Class::Bool,
                ("<" | "<=" | ">" | ">=", Class::Int, Class::Int) => Class::Bool,
                ("and" | "or", Class::Bool, Class::Bool) => Class::Bool,
                ("and" | "or", x, y) => {
                    let odd = if x != Class::Bool { x } else { y };
                    return Err(Message::new(
                        Code::NotBool,
                        at.0,
                        at.1,
                        &format!("`{}` une dos si-o-no, y aqui hay {}", op, odd.name()),
                        "cada lado de `and` / `or` es una pregunta entera: `vidas and escudo` no dice que se pregunta",
                        &format!("compara cada lado: vidas > 0 {} escudo > 0", op),
                    ));
                }
                ("<" | "<=" | ">" | ">=", Class::Text, Class::Text) => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("dos textos no se ordenan con `{}`", op),
                        "el orden de los textos depende del idioma (la n y la enie, las mayusculas); hoy un texto solo se compara con `==` y `!=`",
                        "compara si son iguales: a == b",
                    ))
                }
                ("-" | "*" | "/" | "%", Class::Text, Class::Text) => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("dos textos no se pueden `{}`", op),
                        "con textos solo hay `+`: ponerlos uno detras del otro",
                        "\"a\" + \"b\"",
                    ))
                }
                ("==" | "!=", x, y) => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("{} y {} no se comparan", x.name(), y.name()),
                        "nunca son iguales, y TITAN++ no convierte solo: 1 == \"1\" seria una pregunta con trampa",
                        "compara cosas de la misma clase",
                    ))
                }
                (_, x, y) => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("{} y {} no se pueden `{}`", x.name(), y.name(), op),
                        "TITAN++ no convierte solo: \"a\" + 1 no es \"a1\" (ni 1 + \"1\" es 2, ni true + 1 es 2)",
                        "para mostrarlos juntos, separalos con comas: print(\"total: \", n)",
                    ))
                }
            }
        }
    })
}

/// The second pass: the values, along the ONE way the program goes. Every
/// condition is known here, so every `if` is decided; the live blocks form a
/// single path, in reading order, and the rest are marked dead.
fn values(f: &mut Function) -> Result<(), Message> {
    let mut known: Vec<Option<Const>> = vec![None; f.locals.len()];
    let mut live = vec![false; f.blocks.len()];
    live[0] = true;
    for i in 0..f.blocks.len() {
        if !live[i] {
            f.blocks[i].dead = true;
            continue;
        }
        let b = &mut f.blocks[i];
        for op in &mut b.ops {
            match op {
                Op::Let { local, value, .. } | Op::Set { local, value, .. } => {
                    let c = eval(value, &known)?;
                    *value = constant(&c, value.at());
                    known[*local] = Some(c);
                }
                Op::Write { parts, .. } => {
                    for p in parts.iter_mut() {
                        let c = eval(p, &known)?;
                        *p = constant(&c, p.at());
                    }
                }
                Op::Call { .. } => {}
                Op::Drop { local, .. } => known[*local] = None,
            }
        }
        match &mut b.end {
            End::Return => {}
            End::Jump(t) => live[*t] = true,
            End::Branch { cond, then, other, .. } => {
                let yes = matches!(eval(cond, &known)?, Const::Bool(true));
                *cond = Value::Bool(yes, cond.at());
                live[if yes { *then } else { *other }] = true;
            }
        }
    }
    Ok(())
}

fn constant(c: &Const, at: At) -> Value {
    match c {
        Const::Int(n) => Value::Int(*n, at),
        Const::Text(t) => Value::Text(t.clone(), at),
        Const::Bool(b) => Value::Bool(*b, at),
    }
}

/// A kind that the first pass should have stopped: said, never invented.
fn unclassed(at: At, a: &Const) -> Message {
    Message::new(Code::Mixed, at.0, at.1, &format!("aqui no cabe {}", a.class().name()), "el calculo encontro una clase que la primera pasada no vio", "esto es un fallo del compilador: avisa con este programa")
}

pub fn eval(v: &Value, known: &[Option<Const>]) -> Result<Const, Message> {
    match v {
        Value::Int(n, _) => Ok(Const::Int(*n)),
        Value::Text(t, _) => Ok(Const::Text(t.clone())),
        Value::Bool(b, _) => Ok(Const::Bool(*b)),
        Value::Local(l, _) => Ok(known[*l].clone().expect("juez: every local read has a value")),
        Value::Neg(inner, at) => match eval(inner, known)? {
            Const::Int(n) => n.checked_neg().map(Const::Int).ok_or_else(|| overflow(*at, &format!("-({})", n))),
            other => Err(unclassed(*at, &other)),
        },
        Value::Not(inner, at) => match eval(inner, known)? {
            Const::Bool(b) => Ok(Const::Bool(!b)),
            other => Err(unclassed(*at, &other)),
        },
        // `and` / `or` stop as soon as they know: the right side is not
        // calculated when the left already decided.
        Value::Bin(op @ ("and" | "or"), l, r, at) => match eval(l, known)? {
            Const::Bool(a) if (*op == "and") != a => Ok(Const::Bool(a)),
            Const::Bool(_) => match eval(r, known)? {
                b @ Const::Bool(_) => Ok(b),
                other => Err(unclassed(*at, &other)),
            },
            other => Err(unclassed(*at, &other)),
        },
        Value::Bin(op, l, r, at) => {
            let (a, b) = (eval(l, known)?, eval(r, known)?);
            match (*op, &a, &b) {
                ("==", _, _) => Ok(Const::Bool(a == b)),
                ("!=", _, _) => Ok(Const::Bool(a != b)),
                ("<", Const::Int(x), Const::Int(y)) => Ok(Const::Bool(x < y)),
                ("<=", Const::Int(x), Const::Int(y)) => Ok(Const::Bool(x <= y)),
                (">", Const::Int(x), Const::Int(y)) => Ok(Const::Bool(x > y)),
                (">=", Const::Int(x), Const::Int(y)) => Ok(Const::Bool(x >= y)),
                (_, Const::Int(x), Const::Int(y)) => int(op, *x, *y, *at),
                ("+", Const::Text(x), Const::Text(y)) => Ok(Const::Text(format!("{}{}", x, y))),
                _ => Err(unclassed(*at, &a)),
            }
        }
    }
}

fn overflow(at: At, what: &str) -> Message {
    Message::new(
        Code::Overflow,
        at.0,
        at.1,
        &format!("{} no cabe en un numero", what),
        "un numero entero de TITAN++ ocupa 64 bits, y desbordar es un error, no una vuelta a empezar",
        "usa numeros mas chicos; los mas grandes y exactos llegan con los tipos",
    )
}

fn int(op: &str, x: i64, y: i64, at: At) -> Result<Const, Message> {
    let what = format!("{} {} {}", x, op, y);
    let r = match op {
        "+" => x.checked_add(y),
        "-" => x.checked_sub(y),
        "*" => x.checked_mul(y),
        "/" | "%" if y == 0 => {
            return Err(Message::new(
                Code::DivZero,
                at.0,
                at.1,
                &format!("{} divide entre cero", what),
                "entre cero no hay numero que valga: ni infinito, ni cero",
                "comprueba el divisor antes: if d != 0  (y el otro lado del if no se calcula)",
            ))
        }
        "/" if x % y != 0 => {
            return Err(Message::new(
                Code::Inexact,
                at.0,
                at.1,
                &format!("{} no da un numero entero", what),
                &format!("da {} y sobra {}: TITAN++ no redondea a escondidas (el dinero no se redondea solo)", x / y, x % y),
                &format!("el resto es {} % {}; los decimales exactos llegan con los tipos", x, y),
            ))
        }
        "/" => x.checked_div(y),
        "%" => x.checked_rem(y),
        _ => None,
    };
    r.map(Const::Int).ok_or_else(|| overflow(at, &what))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str) -> Result<Module, Message> {
        let m = crate::ir::lower(&crate::compile(src).unwrap());
        crate::juez::judge(&m)?;
        fold(&m)
    }

    fn printed(src: &str) -> Vec<String> {
        let m = run(src).unwrap();
        m.functions[m.entry]
            .blocks
            .iter()
            .filter(|b| !b.dead)
            .flat_map(|b| &b.ops)
            .filter_map(|op| match op {
                Op::Write { parts, .. } => Some(
                    parts
                        .iter()
                        .map(|p| match p {
                            Value::Int(n, _) => n.to_string(),
                            Value::Text(t, _) => t.clone(),
                            Value::Bool(b, _) => b.to_string(),
                            other => panic!("not folded: {:?}", other),
                        })
                        .collect(),
                ),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn it_calculates_with_the_school_precedence() {
        assert_eq!(printed("mod main \"x\"\nfn main()\n    let a = 2 + 3 * 4\n    print(a, \" \", (2 + 3) * 4, \" \", -a % 5, \" \", 12 / 4)\n"), ["14 20 -4 3"]);
        assert_eq!(printed("mod main \"x\"\nfn main()\n    let s = \"ho\" + \"la\"\n    print(s, \"!\")\n"), ["hola!"]);
    }

    #[test]
    fn each_no_of_the_calculation_with_its_code() {
        let code = |src: &str| run(src).unwrap_err().code;
        assert_eq!(code("mod main \"x\"\nfn main()\n    print(9223372036854775807 + 1)\n"), Code::Overflow);
        assert_eq!(code("mod main \"x\"\nfn main()\n    print(-9223372036854775807 - 2)\n"), Code::Overflow);
        assert_eq!(code("mod main \"x\"\nfn main()\n    let z = 0\n    print(5 % z)\n"), Code::DivZero);
        assert_eq!(code("mod main \"x\"\nfn main()\n    print(7 / 2)\n"), Code::Inexact);
        assert_eq!(code("mod main \"x\"\nfn main()\n    print(\"a\" + 1)\n"), Code::Mixed);
        assert_eq!(code("mod main \"x\"\nfn main()\n    print(\"a\" * \"b\")\n"), Code::Mixed);
    }

    #[test]
    fn every_if_is_decided_when_compiling_and_the_other_side_is_dead() {
        let src = "mod main \"x\"\nfn main()\n    let v = 3\n    if v > 5\n        print(\"mucho\")\n    else if v > 1\n        print(\"algo\")\n    else\n        print(\"poco\")\n    print(v == 3, \" \", not (v < 0) and v != 4)\n";
        assert_eq!(printed(src), ["algo", "true true"]);
        let m = run(src).unwrap();
        assert_eq!(m.functions[0].blocks.iter().filter(|b| b.dead).count(), 2, "{}", m.show());
    }

    #[test]
    fn a_dead_side_is_not_calculated_but_its_classes_are_judged() {
        // The guard works: 10 / d is never calculated when d is 0.
        assert_eq!(printed("mod main \"x\"\nfn main()\n    let d = 0\n    if d != 0\n        print(10 / d)\n    else\n        print(\"nada\")\n    print(d != 0 and 10 / d > 1)\n"), ["nada", "false"]);
        // A NO of CLASS is a NO wherever it sits.
        let e = run("mod main \"x\"\nfn main()\n    if false\n        print(\"a\" + 1)\n").unwrap_err();
        assert_eq!(e.code, Code::Mixed);
    }

    #[test]
    fn a_condition_is_a_yes_or_no_and_nothing_else() {
        let code = |src: &str| run(src).unwrap_err().code;
        assert_eq!(code("mod main \"x\"\nfn main()\n    let vidas = 3\n    if vidas\n        print(\"a\")\n"), Code::NotBool);
        assert_eq!(code("mod main \"x\"\nfn main()\n    print(not 3)\n"), Code::NotBool);
        assert_eq!(code("mod main \"x\"\nfn main()\n    print(1 > 0 and 2)\n"), Code::NotBool);
        assert_eq!(code("mod main \"x\"\nfn main()\n    print(1 == \"1\")\n"), Code::Mixed);
        assert_eq!(code("mod main \"x\"\nfn main()\n    print(\"a\" < \"b\")\n"), Code::Mixed);
        assert_eq!(code("mod main \"x\"\nfn main()\n    let mut ok = true\n    ok = 1\n"), Code::Retype);
    }

    #[test]
    fn an_inexact_division_says_the_quotient_and_the_rest() {
        let e = run("mod main \"x\"\nfn main()\n    print(7 / 2)\n").unwrap_err();
        assert!(e.why.contains("da 3 y sobra 1"), "{}", e.why);
    }
}
