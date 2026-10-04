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
//! ```
//!
//! It runs AFTER the checker (`juez.rs`): every local it reads has a value by
//! then, so a missing one here is a bug of this crate, not of the program.

use crate::ir::{At, Module, Op, Value};
use crate::message::{Code, Message};

/// A value, calculated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Const {
    Int(i64),
    Text(String),
}

impl Const {
    /// As `print` writes it.
    pub fn show(&self) -> String {
        match self {
            Const::Int(n) => n.to_string(),
            Const::Text(t) => t.clone(),
        }
    }
}

/// The module with every value calculated: each `Let` and each part of each
/// `Write` becomes `Value::Int` or `Value::Text`.
pub fn fold(m: &Module) -> Result<Module, Message> {
    let mut out = m.clone();
    for f in &mut out.functions {
        let mut known: Vec<Option<Const>> = vec![None; f.locals.len()];
        for b in &mut f.blocks {
            for op in &mut b.ops {
                match op {
                    Op::Let { local, value, .. } => {
                        let c = eval(value, &known)?;
                        *value = constant(&c, value.at());
                        known[*local] = Some(c);
                    }
                    Op::Set { local, value, at } => {
                        let c = eval(value, &known)?;
                        // ** A `mut` changes its VALUE, never its kind: a
                        // number stays a number (Python lets `x = 5` become
                        // `x = "hola"`; the checker could not say what x is).
                        if let Some(old) = &known[*local] {
                            if kind(old) != kind(&c) {
                                return Err(Message::new(
                                    Code::Retype,
                                    at.0,
                                    at.1,
                                    &format!("aqui `{}` pasaria de {} a {}", f.locals[*local].name, kind(old), kind(&c)),
                                    "un `mut` cambia de valor, no de clase: el que lee tiene que saber siempre que es",
                                    "dale un valor de la misma clase, o usa otro nombre para el nuevo",
                                ));
                            }
                        }
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
                }
            }
        }
    }
    Ok(out)
}

fn constant(c: &Const, at: At) -> Value {
    match c {
        Const::Int(n) => Value::Int(*n, at),
        Const::Text(t) => Value::Text(t.clone(), at),
    }
}

fn kind(c: &Const) -> &'static str {
    match c {
        Const::Int(_) => "un numero",
        Const::Text(_) => "un texto",
    }
}

pub fn eval(v: &Value, known: &[Option<Const>]) -> Result<Const, Message> {
    match v {
        Value::Int(n, _) => Ok(Const::Int(*n)),
        Value::Text(t, _) => Ok(Const::Text(t.clone())),
        Value::Local(l, _) => Ok(known[*l].clone().expect("juez: every local read has a value")),
        Value::Neg(inner, at) => match eval(inner, known)? {
            Const::Int(n) => n.checked_neg().map(Const::Int).ok_or_else(|| overflow(*at, &format!("-({})", n))),
            t => Err(Message::new(Code::Mixed, at.0, at.1, "un texto no tiene signo", &format!("aqui hay {} con un `-` delante", kind(&t)), "el `-` va delante de un numero")),
        },
        Value::Bin(op, l, r, at) => {
            let (a, b) = (eval(l, known)?, eval(r, known)?);
            match (&a, &b) {
                (Const::Int(x), Const::Int(y)) => int(*op, *x, *y, *at),
                (Const::Text(x), Const::Text(y)) if *op == '+' => Ok(Const::Text(format!("{}{}", x, y))),
                (Const::Text(_), Const::Text(_)) => Err(Message::new(
                    Code::Mixed,
                    at.0,
                    at.1,
                    &format!("dos textos no se pueden `{}`", op),
                    "con textos solo hay `+`: ponerlos uno detras del otro",
                    "\"a\" + \"b\"",
                )),
                _ => Err(Message::new(
                    Code::Mixed,
                    at.0,
                    at.1,
                    &format!("{} y {} no se pueden `{}`", kind(&a), kind(&b), op),
                    "TITAN++ no convierte solo: \"a\" + 1 no es \"a1\" (ni 1 + \"1\" es 2)",
                    "para mostrarlos juntos, separalos con comas: print(\"total: \", n)",
                )),
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

fn int(op: char, x: i64, y: i64, at: At) -> Result<Const, Message> {
    let what = format!("{} {} {}", x, op, y);
    let r = match op {
        '+' => x.checked_add(y),
        '-' => x.checked_sub(y),
        '*' => x.checked_mul(y),
        '/' | '%' if y == 0 => {
            return Err(Message::new(
                Code::DivZero,
                at.0,
                at.1,
                &format!("{} divide entre cero", what),
                "entre cero no hay numero que valga: ni infinito, ni cero",
                "comprueba el divisor (decidir con `if` llega en el nivel 3)",
            ))
        }
        '/' if x % y != 0 => {
            return Err(Message::new(
                Code::Inexact,
                at.0,
                at.1,
                &format!("{} no da un numero entero", what),
                &format!("da {} y sobra {}: TITAN++ no redondea a escondidas (el dinero no se redondea solo)", x / y, x % y),
                &format!("el resto es {} % {}; los decimales exactos llegan con los tipos", x, y),
            ))
        }
        '/' => x.checked_div(y),
        '%' => x.checked_rem(y),
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
        m.functions[m.entry].blocks[0]
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Write { parts, .. } => Some(
                    parts
                        .iter()
                        .map(|p| match p {
                            Value::Int(n, _) => n.to_string(),
                            Value::Text(t, _) => t.clone(),
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
    fn an_inexact_division_says_the_quotient_and_the_rest() {
        let e = run("mod main \"x\"\nfn main()\n    print(7 / 2)\n").unwrap_err();
        assert!(e.why.contains("da 3 y sobra 1"), "{}", e.why);
    }
}
