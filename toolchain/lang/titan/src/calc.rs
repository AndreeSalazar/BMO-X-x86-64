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
use crate::tree::Ty;
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
    for f in &out.functions {
        classes(f, m)?;
    }
    // ** The run goes in a thread of its OWN, with a big stack: a recursion
    // of DEPTH calls is a rule of the language (T0066), never a stack that
    // bursts inside the compiler. The memory is only reserved, not used.
    let r = std::thread::scope(|sc| {
        std::thread::Builder::new()
            .stack_size(STACK)
            .spawn_scoped(sc, || {
                let mut r = Run { m, steps: 0, depth: 0, flat: Vec::new(), seen: m.functions.iter().map(|f| vec![false; f.blocks.len()]).collect(), last_turn: (0, 0) };
                r.call(m.entry, Vec::new(), (m.functions[m.entry].line, 1)).map(|_| r)
            })
            .expect("a thread for the run")
            .join()
            .expect("the run does not panic")
    });
    let r = r?;
    for (f, seen) in out.functions.iter_mut().zip(&r.seen) {
        for (b, &s) in f.blocks.iter_mut().zip(seen) {
            b.dead = !s;
        }
    }
    out.flat = Some(r.flat);
    Ok(out)
}

/// How far the calculation goes before it says the program does not end
/// (T0066). A million steps is a table of multiplication a thousand times
/// over; a `while true` with no `break` reaches it in a blink.
pub const STEPS: u64 = 1_000_000;

/// How deep calls may nest while running (level 5): a recursion that goes
/// further is said, not a stack that bursts inside the compiler.
pub const DEPTH: usize = 10_000;

/// The stack of the thread the run goes in: room for DEPTH calls, even in a
/// debug build.
const STACK: usize = 512 << 20;

/// The class a type of the text says.
fn of_ty(t: Ty) -> Class {
    match t {
        Ty::Int => Class::Int,
        Ty::Text => Class::Text,
        Ty::Bool => Class::Bool,
    }
}

/// T0071: a value of the wrong class where a parameter or a result says one.
fn wrong(at: At, want: Class, got: Class, what: &str, how: &str) -> Message {
    Message::new(Code::WrongType, at.0, at.1, &format!("aqui va {}, y llega {}", want.name(), got.name()), what, how)
}

/// The first pass: the class of every local and every value, in EVERY block.
/// The blocks are in reading order, and a local that dies (`Drop`) forgets
/// its class: the next one with that name is another value.
fn classes(f: &Function, m: &Module) -> Result<(), Message> {
    let mut known: Vec<Option<Class>> = vec![None; f.locals.len()];
    for &(l, t) in &f.params {
        known[l] = Some(of_ty(t));
    }
    for b in &f.blocks {
        for op in &b.ops {
            match op {
                Op::Let { local, value, at, .. } => {
                    let c = class(value, &known, m)?;
                    // The hidden two of a `for` (`#i`, `#fin`): `range`
                    // counts with numbers.
                    if f.locals[*local].name.starts_with('#') && c != Class::Int {
                        return Err(Message::new(
                            Code::Mixed,
                            at.0,
                            at.1,
                            &format!("`range` cuenta con numeros, y aqui hay {}", c.name()),
                            "un `for` da vueltas de un numero al siguiente: un texto o un si-o-no no tienen siguiente",
                            "for i in range(10)  o  for i in range(1, 11)",
                        ));
                    }
                    known[*local] = Some(c);
                }
                Op::Set { local, value, at } => {
                    let c = class(value, &known, m)?;
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
                        class(p, &known, m)?;
                    }
                }
                Op::Call { func, args, at } => args_fit(*func, args, *at, &known, m)?,
                Op::Drop { local, .. } => known[*local] = None,
            }
        }
        if let End::Return(Some(v)) = &b.end {
            let got = class(v, &known, m)?;
            let want = of_ty(f.ret.expect("check: a return with a value is in a fn with ->"));
            if got != want {
                return Err(wrong(v.at(), want, got, &format!("`fn {}` promete `-> {}` en su linea {}", f.name, f.ret.map(|t| t.name()).unwrap_or(""), f.line), "devuelve un valor de esa clase, o cambia lo que promete la primera linea"));
            }
        }
        if let End::Branch { cond, at, .. } = &b.end {
            let c = class(cond, &known, m)?;
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
        Value::Call(_, args, _) => format!("f({})", args.iter().map(|a| written(a, f)).collect::<Vec<_>>().join(", ")),
    }
}

/// The values given to a call, each against the class its parameter says.
fn args_fit(func: usize, args: &[Value], at: At, known: &[Option<Class>], m: &Module) -> Result<(), Message> {
    let g = &m.functions[func];
    for (a, &(l, t)) in args.iter().zip(&g.params) {
        let got = class(a, known, m)?;
        if got != of_ty(t) {
            return Err(wrong(
                a.at(),
                of_ty(t),
                got,
                &format!("`{}` pide `{}: {}` (su linea {})", g.name, g.locals[l].name, t.name(), g.line),
                "TITAN++ no convierte solo: dale un valor de esa clase",
            ));
        }
    }
    let _ = at;
    Ok(())
}

/// The class of a value, or the NO that says why it has none.
pub fn class(v: &Value, known: &[Option<Class>], m: &Module) -> Result<Class, Message> {
    Ok(match v {
        Value::Call(func, args, at) => {
            args_fit(*func, args, *at, known, m)?;
            of_ty(m.functions[*func].ret.expect("check: a call used as a value gives one back"))
        }
        Value::Int(..) => Class::Int,
        Value::Text(..) => Class::Text,
        Value::Bool(..) => Class::Bool,
        Value::Local(l, _) => known[*l].expect("juez: every local read has a value"),
        Value::Neg(inner, at) => match class(inner, known, m)? {
            Class::Int => Class::Int,
            c => return Err(Message::new(Code::Mixed, at.0, at.1, &format!("{} no tiene signo", c.name()), &format!("aqui hay {} con un `-` delante", c.name()), "el `-` va delante de un numero")),
        },
        Value::Not(inner, at) => match class(inner, known, m)? {
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
            let (a, b) = (class(l, known, m)?, class(r, known, m)?);
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

/// ** THE SECOND PASS: THE PROGRAM, RUN WHEN COMPILING.
///
/// Until something comes from outside (the keyboard, a file), every value of
/// a program is known before it runs -- with loops too: a `for` of ten turns
/// is ten known turns. So the calculation does not fold each line once (a
/// line inside a loop has a different value at every turn): it RUNS the
/// program, from `main`, through every call, every `if` and every turn, and
/// keeps what it writes. That list (`Module::flat`) is what the `.bex` does.
///
/// ```text
///    a block never entered   is DEAD (`Block::dead`): no byte, no door
///    a value that overflows, /0, a division not whole
///                            a NO at its line, as in levels 1-3 -- but only
///                            if the program really gets there
///    STEPS steps and still going
///                            T0066: a loop that does not end (or not in a
///                            million steps), said at its line
/// ```
///
/// [!] The day something comes from outside, part of the program can no longer
/// be run here, and that part goes to the machine as real code (E1 of the
/// emitter, TITAN_MAESTRO 7.3). This is the floor, E0, made whole.
struct Run<'m> {
    m: &'m Module,
    steps: u64,
    /// How many calls are open right now.
    depth: usize,
    flat: Vec<Op>,
    /// Which blocks of which function some run entered.
    seen: Vec<Vec<bool>>,
    /// The last question a loop asked: where T0066 points.
    last_turn: At,
}

impl Run<'_> {
    fn tick(&mut self) -> Result<(), Message> {
        self.steps += 1;
        if self.steps > STEPS {
            let at = self.last_turn;
            return Err(Message::new(
                Code::NoEnd,
                at.0,
                at.1,
                &format!("este bucle sigue dando vueltas despues de {} pasos", STEPS),
                "nada de este programa viene de fuera todavia, asi que se corre entero al compilar; y aqui no acaba: o le falta su `break`, o su condicion nunca se hace false",
                "revisa que la condicion cambie dentro del bucle (n = n + 1) o pon un `break`; un bucle de juego que espera al teclado llega cuando haya entrada",
            ));
        }
        Ok(())
    }

    /// Runs `func` with these values; what it gives back, if anything.
    fn call(&mut self, func: usize, args: Vec<Const>, at: At) -> Result<Option<Const>, Message> {
        let m = self.m;
        let f = &m.functions[func];
        if self.depth >= DEPTH {
            return Err(Message::new(
                Code::NoEnd,
                at.0,
                at.1,
                &format!("las llamadas se anidan mas de {} veces", DEPTH),
                &format!("`{}` se sigue llamando sin llegar a su caso de parada: o no lo tiene, o el valor que se le pasa no se acerca a el", f.name),
                "revisa el `if` que la para y que cada llamada se acerque a el (cuenta(n - 1) hasta n == 0)",
            ));
        }
        self.depth += 1;
        let mut known: Vec<Option<Const>> = vec![None; f.locals.len()];
        for (&(l, _), a) in f.params.iter().zip(args) {
            known[l] = Some(a);
        }
        let mut b = 0;
        let result = loop {
            self.seen[func][b] = true;
            for op in &f.blocks[b].ops {
                self.tick()?;
                match op {
                    Op::Let { local, value, .. } | Op::Set { local, value, .. } => known[*local] = Some(self.ev(value, &known)?),
                    Op::Write { parts, at } => {
                        let mut out = Vec::with_capacity(parts.len());
                        for p in parts {
                            let c = self.ev(p, &known)?;
                            out.push(constant(&c, p.at()));
                        }
                        self.flat.push(Op::Write { parts: out, at: *at });
                    }
                    Op::Call { func, args, at } => {
                        let vals = args.iter().map(|a| self.ev(a, &known)).collect::<Result<Vec<_>, _>>()?;
                        self.call(*func, vals, *at)?;
                    }
                    Op::Drop { local, .. } => known[*local] = None,
                }
            }
            self.tick()?;
            b = match &f.blocks[b].end {
                End::Return(v) => break match v {
                    Some(v) => Some(self.ev(v, &known)?),
                    None => None,
                },
                End::Jump(t) => *t,
                End::Branch { cond, then, other, at } => {
                    if *then < b || *other < b || f.blocks.iter().skip(b).any(|x| x.end.targets().contains(&b)) {
                        self.last_turn = *at;
                    }
                    if matches!(self.ev(cond, &known)?, Const::Bool(true)) {
                        *then
                    } else {
                        *other
                    }
                }
            };
        };
        self.depth -= 1;
        Ok(result)
    }

    /// A value, calculated -- running the calls inside it.
    fn ev(&mut self, v: &Value, known: &[Option<Const>]) -> Result<Const, Message> {
        Ok(match v {
            Value::Int(n, _) => Const::Int(*n),
            Value::Text(t, _) => Const::Text(t.clone()),
            Value::Bool(b, _) => Const::Bool(*b),
            Value::Local(l, _) => known[*l].clone().expect("juez: every local read has a value"),
            Value::Call(func, args, at) => {
                let vals = args.iter().map(|a| self.ev(a, known)).collect::<Result<Vec<_>, _>>()?;
                self.call(*func, vals, *at)?.expect("check: a call used as a value gives one back")
            }
            Value::Neg(inner, at) => match self.ev(inner, known)? {
                Const::Int(n) => n.checked_neg().map(Const::Int).ok_or_else(|| overflow(*at, &format!("-({})", n)))?,
                other => return Err(unclassed(*at, &other)),
            },
            Value::Not(inner, at) => match self.ev(inner, known)? {
                Const::Bool(b) => Const::Bool(!b),
                other => return Err(unclassed(*at, &other)),
            },
            // `and` / `or` stop as soon as they know.
            Value::Bin(op @ ("and" | "or"), l, r, at) => match self.ev(l, known)? {
                Const::Bool(a) if (*op == "and") != a => Const::Bool(a),
                Const::Bool(_) => match self.ev(r, known)? {
                    b @ Const::Bool(_) => b,
                    other => return Err(unclassed(*at, &other)),
                },
                other => return Err(unclassed(*at, &other)),
            },
            Value::Bin(op, l, r, at) => {
                let (a, b) = (self.ev(l, known)?, self.ev(r, known)?);
                binop(op, a, b, *at)?
            }
        })
    }
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

/// Two values and an operator: comparisons, arithmetic, texts joined.
fn binop(op: &str, a: Const, b: Const, at: At) -> Result<Const, Message> {
    match (op, &a, &b) {
        ("==", _, _) => Ok(Const::Bool(a == b)),
        ("!=", _, _) => Ok(Const::Bool(a != b)),
        ("<", Const::Int(x), Const::Int(y)) => Ok(Const::Bool(x < y)),
        ("<=", Const::Int(x), Const::Int(y)) => Ok(Const::Bool(x <= y)),
        (">", Const::Int(x), Const::Int(y)) => Ok(Const::Bool(x > y)),
        (">=", Const::Int(x), Const::Int(y)) => Ok(Const::Bool(x >= y)),
        (_, Const::Int(x), Const::Int(y)) => int(op, *x, *y, at),
        ("+", Const::Text(x), Const::Text(y)) => Ok(Const::Text(format!("{}{}", x, y))),
        _ => Err(unclassed(at, &a)),
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
        m.flat
            .as_ref()
            .expect("calc runs the program")
            .iter()
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
    fn loops_are_run_turn_by_turn_when_compiling() {
        let src = "mod main \"x\"\nfn main()\n    for i in range(1, 4)\n        print(i, \" x 3 = \", i * 3)\n    let mut n = 10\n    while n > 0\n        n = n - 4\n        if n < 5\n            continue\n        print(\"n \", n)\n    print(\"fin \", n)\n";
        assert_eq!(printed(src), ["1 x 3 = 3", "2 x 3 = 6", "3 x 3 = 9", "n 6", "fin -2"]);
    }

    #[test]
    fn a_loop_that_does_not_end_is_said_at_its_line() {
        let e = run("mod main \"x\"\nfn main()\n    let mut n = 0\n    while n >= 0\n        n = n + 1\n").unwrap_err();
        assert_eq!((e.code, e.line), (Code::NoEnd, 4));
        // A `while true` with its `break` ends.
        assert_eq!(printed("mod main \"x\"\nfn main()\n    let mut n = 1\n    while true\n        n = n * 2\n        if n > 100\n            break\n    print(n)\n"), ["128"]);
        // `range` counts with numbers.
        assert_eq!(run("mod main \"x\"\nfn main()\n    for i in range(\"tres\")\n        print(i)\n").unwrap_err().code, Code::Mixed);
    }

    #[test]
    fn calls_carry_values_and_recursion_runs_with_its_stop() {
        let src = "mod main \"x\"\nfn fact(n: int) -> int\n    if n <= 1\n        return 1\n    return n * fact(n - 1)\nfn par(n: int) -> bool\n    return n % 2 == 0\nfn main()\n    print(fact(10), \" \", par(fact(3)))\n";
        assert_eq!(printed(src), ["3628800 true"]);
        // A deep but finite recursion runs: 5000 calls nested.
        assert_eq!(printed("mod main \"x\"\nfn suma(n: int) -> int\n    if n == 0\n        return 0\n    return n + suma(n - 1)\nfn main()\n    print(suma(5000))\n"), ["12502500"]);
        // One that never stops is said, not a compiler that bursts.
        assert_eq!(run("mod main \"x\"\nfn f(n: int) -> int\n    return f(n + 1)\nfn main()\n    print(f(0))\n").unwrap_err().code, Code::NoEnd);
        // The class of each value given is the one its parameter says.
        assert_eq!(run("mod main \"x\"\nfn f(n: int) -> int\n    return n\nfn main()\n    print(f(true))\n").unwrap_err().code, Code::WrongType);
        assert_eq!(run("mod main \"x\"\nfn f(n: int) -> text\n    return n\nfn main()\n    print(f(1))\n").unwrap_err().code, Code::WrongType);
    }

    #[test]
    fn an_inexact_division_says_the_quotient_and_the_rest() {
        let e = run("mod main \"x\"\nfn main()\n    print(7 / 2)\n").unwrap_err();
        assert!(e.why.contains("da 3 y sobra 1"), "{}", e.why);
    }
}
