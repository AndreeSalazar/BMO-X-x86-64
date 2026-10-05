//! `calc` -- what is known when compiling is calculated when compiling.
//!
//! C++'s `constexpr` (TITAN_MAESTRO 2.1), without the keyword: until something
//! comes from outside, every value of a program is known before it runs, so
//! the whole calculation happens HERE, and the `.bex` carries the results.
//! Exact, or a NO with its four parts:
//!
//! ```text
//!    T0060  the result does not fit in 64 bits: overflow is an ERROR, never a
//!           wrap (INTI's rule 1, and Ada's)
//!    T0061  a division (or %) by zero
//!    T0062  a division that is not exact: 7 / 2 between ints is NOT 3, and
//!           1.0 / 3 is not a decimal that ends. Nothing is ever rounded in
//!           silence (2b.1)
//!    T0063  a text with a number: "a" + 1 is not "a1" (JavaScript's most
//!           famous fault, 14.13). Texts add to texts; print takes both
//!    T0064  a `mut` that would change KIND: `n = "hola"` after `let mut n = 0`
//!           (level 2: a value changes, its class never does)
//!    T0065  a YES/NO was asked and something else came: `if vidas` with a
//!           number, `not "a"`, `3 and true` (level 3)
//!    T0066  the program does not end when run (level 4)
//!    T0071  a value of another class where a parameter, a result, a field or
//!           a table says one (level 5-6)
//!    T0072  a cell outside its table: `a[5]` of a `[int; 3]` (level 6)
//!    T0073  a field the record does not have (level 6)
//! ```
//!
//! It runs AFTER the checker (`juez.rs`): every local it reads has a value by
//! then, so a missing one here is a bug of this crate, not of the program.
//!
//! ** TWO PASSES, and the difference is the whole point.
//!
//! ```text
//!    CLASSES   every block, the ones that will never run too: what each value
//!              IS -- a number, a decimal, a text, a yes/no, a table, a record
//!              -- and whether they fit. Wrong in KIND is wrong wherever it is
//!    VALUES    the program RUN, from `main`, every call and every turn:
//!              `Module::flat` is what it writes (level 4)
//! ```
//!
//! ** LEVEL 6: `dec`, THE EXACT DECIMAL, AND NO FLOAT (the owner, 04-10:
//! "evita la float, siempre decimal"). A `dec` is an integer and how many of
//! its digits are decimals: `12.50` is 1250 with 2. Adding aligns the
//! decimals, multiplying adds them, and dividing gives the decimal that is
//! EXACT -- or a NO (T0062: `1.0 / 3` never ends, and TITAN++ does not cut it
//! in silence). An `int` joins a `dec` without losing anything (13 is 13.00);
//! the other way would lose, so it is never done alone. No float anywhere: the
//! x86-64 has floats in hardware, but in base 2, and that is why a float gets
//! `0.1` wrong. A `dec` is COBOL's number (Grace Hopper's) at the speed of an
//! integer.

use crate::ir::{At, End, Function, Module, Op, PathStep, Value};
use crate::message::{Code, Message};
use crate::tree::{show_dec, EnumDef, TraitDef, Ty, TypeDef};

/// What a value needs to be classed or shown: the `type`s and the `enum`s of
/// the file (levels 6 and 8). Indexed, it is a `type`.
#[derive(Clone, Copy)]
pub struct Defs<'a> {
    pub types: &'a [TypeDef],
    pub enums: &'a [EnumDef],
    /// The traits, and which type keeps which (level 10).
    pub traits: &'a [TraitDef],
    pub impls: &'a [(String, String)],
}

/// No `type` and no `enum`: enough to show a number.
const NONE: Defs<'static> = Defs { types: &[], enums: &[], traits: &[], impls: &[] };

impl std::ops::Index<usize> for Defs<'_> {
    type Output = TypeDef;
    fn index(&self, t: usize) -> &TypeDef {
        &self.types[t]
    }
}

impl Module {
    pub fn defs(&self) -> Defs<'_> {
        Defs { types: &self.types, enums: &self.enums, traits: &self.traits, impls: &self.impls }
    }
}

/// A value, calculated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Const {
    Int(i64),
    Text(String),
    Bool(bool),
    /// The digits and how many of them are decimals.
    Dec(i64, u32),
    Table(Vec<Const>),
    /// A record of the type with this index, its fields in declared order.
    Record(usize, Vec<Const>),
    /// The case `v` of the enum `e`, and the values it carries (level 8).
    Variant(usize, usize, Vec<Const>),
    /// An f32 by its bits (level 11): what a `gpu fn` counts with.
    F32(u32),
}

impl Const {
    /// As `print` writes it. Inside a table or a record a text goes quoted,
    /// so `["a, b"]` and `["a", "b"]` never read the same.
    pub fn show(&self, types: Defs) -> String {
        self.show_in(types, false)
    }

    fn show_in(&self, types: Defs, inside: bool) -> String {
        match self {
            Const::Int(n) => n.to_string(),
            Const::Text(t) if inside => format!("{:?}", t),
            Const::Text(t) => t.clone(),
            Const::Bool(b) => b.to_string(),
            Const::Dec(d, s) => show_dec(*d, *s),
            Const::F32(b) => format!("{}", f32::from_bits(*b)),
            Const::Table(items) => format!("[{}]", items.iter().map(|i| i.show_in(types, true)).collect::<Vec<_>>().join(", ")),
            Const::Record(t, items) => {
                let def = &types[*t];
                let fields: Vec<String> = def.fields.iter().zip(items).map(|(f, v)| format!("{}: {}", f.name, v.show_in(types, true))).collect();
                format!("{} {{ {} }}", short(&def.name), fields.join(", "))
            }
            Const::Variant(e, v, items) => {
                let case = short(&types.enums[*e].cases[*v].name);
                if items.is_empty() {
                    case.to_string()
                } else {
                    format!("{}({})", case, items.iter().map(|i| i.show_in(types, true)).collect::<Vec<_>>().join(", "))
                }
            }
        }
    }
}

/// A name as `print` writes it: without the module it comes from (level 9:
/// `nave.Nave` of the package is just `Nave` on the console).
fn short(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// What a value IS, without knowing which one: the first pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    Int,
    Text,
    Bool,
    Dec,
    Table(Box<Class>, usize),
    Record(usize),
    /// A value of the enum with this index: one of its cases (level 8).
    Enum(usize),
    /// ANY value whose type keeps the trait with this index: what a
    /// parameter `f: Forma` is inside its fn (level 10).
    Trait(usize),
    /// The 3060's number (level 11): it counts inside a `gpu fn`; outside,
    /// it is kept or passed, and `round` makes it a `dec` (D2).
    F32,
}

impl Class {
    fn name(&self, types: Defs) -> String {
        match self {
            Class::Int => "un numero (int)".into(),
            Class::Text => "un texto".into(),
            Class::Bool => "un si-o-no (true / false)".into(),
            Class::Dec => "un decimal (dec)".into(),
            Class::Table(c, n) => format!("una tabla [{}; {}]", c.short(types), n),
            Class::Record(t) => format!("un {}", types[*t].name),
            Class::Enum(e) => format!("un {}", types.enums[*e].name),
            Class::Trait(k) => format!("algo que cumple `trait {}`", types.traits[*k].name),
            Class::F32 => "un f32 (el numero de la 3060)".into(),
        }
    }

    fn short(&self, types: Defs) -> String {
        match self {
            Class::Int => "int".into(),
            Class::Text => "text".into(),
            Class::Bool => "bool".into(),
            Class::Dec => "dec".into(),
            Class::Table(c, n) => format!("[{}; {}]", c.short(types), n),
            Class::Record(t) => types[*t].name.clone(),
            Class::Enum(e) => types.enums[*e].name.clone(),
            Class::Trait(k) => types.traits[*k].name.clone(),
            Class::F32 => "f32".into(),
        }
    }

    fn number(&self) -> bool {
        matches!(self, Class::Int | Class::Dec)
    }
}

/// The class a type of the text says.
fn of_ty(t: &Ty, types: Defs) -> Class {
    match t {
        Ty::Int => Class::Int,
        Ty::Text => Class::Text,
        Ty::Bool => Class::Bool,
        Ty::Dec | Ty::DecP(..) => Class::Dec,
        Ty::F32 => Class::F32,
        Ty::Table(inner, n) => Class::Table(Box::new(of_ty(inner, types)), *n),
        Ty::Named(n) => match types.types.iter().position(|d| &d.name == n) {
            Some(t) => Class::Record(t),
            None => match types.enums.iter().position(|d| &d.name == n) {
                Some(e) => Class::Enum(e),
                None => Class::Trait(types.traits.iter().position(|d| &d.name == n).expect("check: the type exists")),
            },
        },
    }
}

/// May a value of class `got` go where `want` is said? The same class, or an
/// `int` where a `dec` goes (13 is 13.00: nothing is lost). Never the other
/// way, and never anything else.
fn fits(want: &Class, got: &Class) -> bool {
    want == got || (*want == Class::Dec && *got == Class::Int)
}

/// T0071: a value of the wrong class where something says one.
fn wrong(at: At, want: &Class, got: &Class, types: Defs, what: &str, how: &str) -> Message {
    Message::new(Code::WrongType, at.0, at.1, &format!("aqui va {}, y llega {}", want.name(types), got.name(types)), what, how)
}

/// ** WHO RUNS A `gpu fn` (level 11, G3 of PLAN_EL_CENTAURO), said without
/// naming a machine or a format: given the cells of each value -- an f32 by
/// its bits, a bool as 0 or 1 --, the cells of the result. The emitter's
/// side gives one that writes the gpu fn as SPIR-V and runs it in spirv's
/// ORACLE; without one, the calculation runs each thread itself, with f32 of
/// single precision. The two benches compare the same `# sale:` lines, so
/// the day they disagreed, one of them would say it.
pub trait Device: Send {
    fn run(&mut self, m: &Module, func: usize, cells: Vec<Vec<u32>>) -> Result<Vec<u32>, String>;
}

/// The module RUN: its classes judged in every block, then the program run
/// from `main`. `Module::flat` is what it writes; the blocks no run entered
/// are `dead`.
pub fn fold(m: &Module) -> Result<Module, Message> {
    fold_with(m, None)
}

/// `fold`, with who runs the `gpu fn` (level 11).
pub fn fold_with(m: &Module, device: Option<&mut dyn Device>) -> Result<Module, Message> {
    let mut out = m.clone();
    // A trait's fn has no body: each type's fn is judged as its own.
    for f in out.functions.iter().filter(|f| f.dispatch.is_none()) {
        classes(f, m)?;
    }
    // ** The run goes in a thread of its OWN, with a big stack: a recursion
    // of DEPTH calls is a rule of the language (T0066), never a stack that
    // bursts inside the compiler. The memory is only reserved, not used.
    let r = std::thread::scope(|sc| {
        std::thread::Builder::new()
            .stack_size(STACK)
            .spawn_scoped(sc, move || {
                let mut r = Run { m, steps: 0, depth: 0, flat: Vec::new(), seen: m.functions.iter().map(|f| vec![false; f.blocks.len()]).collect(), last_turn: (0, 0), lenient: 0, device };
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

/// ** A `gpu fn` run by the CALCULATION on given cells (level 11): an f32 by
/// its bits, a bool as 0 or 1 -- the same cells a `Device` gets. It is the
/// reference the oracle of spirv is measured against at every build, cell by
/// cell, so that nobody has to guess whether the two agree.
pub fn run_gpu(m: &Module, func: usize, cells: &[Vec<u32>]) -> Result<Vec<u32>, Message> {
    let f = &m.functions[func];
    let n = cells.first().map(|c| c.len()).unwrap_or(0);
    let mut r = Run { m, steps: 0, depth: 0, flat: Vec::new(), seen: m.functions.iter().map(|f| vec![false; f.blocks.len()]).collect(), last_turn: (0, 0), lenient: 0, device: None };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let args = f.params.iter().zip(cells).map(|((_, t), c)| if *t == Ty::Bool { Const::Bool(c[i] != 0) } else { Const::F32(c[i]) }).collect();
        let (result, _) = r.call(func, args, (f.line, 1))?;
        out.push(match result {
            Some(Const::F32(b)) => b,
            Some(Const::Bool(b)) => b as u32,
            _ => 0,
        });
        r.steps = 0;
    }
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

/// The most decimals a `dec` keeps: 18, so its digits always fit in 64 bits.
pub const SCALE: u32 = 18;

// ===================================================================
//  THE FIRST PASS: classes
// ===================================================================

/// The class of every local and every value, in EVERY block. The blocks are
/// in reading order, and a local that dies (`Drop`) forgets its class: the
/// next one with that name is another value.
fn classes(f: &Function, m: &Module) -> Result<(), Message> {
    let types = m.defs();
    let mut known: Vec<Option<Class>> = vec![None; f.locals.len()];
    for (l, t) in &f.params {
        known[*l] = Some(of_ty(t, types));
    }
    for b in &f.blocks {
        for op in &b.ops {
            // ** D2 (level 11): outside a `gpu fn`, an f32 is kept or
            // passed, never counted, compared or printed.
            if !f.gpu {
                match op {
                    Op::Let { value, .. } | Op::Set { value, .. } => cpu_f32(value, &known, m)?,
                    Op::SetAt { value, .. } => cpu_f32(value, &known, m)?,
                    Op::Call { args, .. } => args.iter().try_for_each(|a| cpu_f32(a, &known, m))?,
                    Op::Write { parts, .. } => {
                        for p in parts {
                            cpu_f32(p, &known, m)?;
                            let c = class(p, &known, m)?;
                            if holds_f32(&c, types) {
                                return Err(f32_here(p.at(), "se imprime", "un f32 es la cuenta de la 3060, con sus redondeos de base 2: lo que se muestra en la CPU es un `dec`, y el redondeo se escribe"));
                            }
                        }
                    }
                    Op::Drop { .. } => {}
                }
            }
            match op {
                Op::Let { local, value, at, ty, .. } => {
                    let mut c = class(value, &known, m)?;
                    // `let x: T = v` (level 7): the value must fit the type
                    // DECLARED, and the local is of that type from then on.
                    if let Some(t) = ty {
                        let want = of_ty(t, types);
                        // D4 (level 11): a number becomes f32 only where a
                        // type DECLARES it: `let xs: [f32; 4] = [1.0, ...]`.
                        if into_f32(&want, &c) {
                            c = want.clone();
                        }
                        if !fits(&want, &c) && f.locals[*local].name.starts_with("#m") {
                            // The hidden local of a `match` (level 8): its arms
                            // are the cases of one enum, so that is what it reads.
                            return Err(wrong(value.at(), &want, &c, types, &format!("los casos de este `match` son los de `enum {}`", want.short(types)), "dale al `match` un valor de ese enum, o escribe los casos del enum que tiene"));
                        }
                        if !fits(&want, &c) {
                            return Err(wrong(value.at(), &want, &c, types, &format!("`{}` se declaro `{}`", f.locals[*local].name, t.name()), "dale un valor de esa clase, o cambia el tipo declarado"));
                        }
                        c = want;
                    }
                    // The hidden count of a `for` (`#i`, `#fin`): `range`
                    // counts with whole numbers.
                    let name = &f.locals[*local].name;
                    if (name.starts_with("#i") || name.starts_with("#fin")) && c != Class::Int {
                        return Err(Message::new(
                            Code::Mixed,
                            at.0,
                            at.1,
                            &format!("`range` cuenta con numeros enteros, y aqui hay {}", c.name(types)),
                            "un `for` da vueltas de un entero al siguiente: un texto, un decimal o un si-o-no no tienen siguiente",
                            "for i in range(10)  o  for i in range(1, 11)",
                        ));
                    }
                    if name.starts_with("#t") && !matches!(c, Class::Table(..)) {
                        return Err(Message::new(
                            Code::Mixed,
                            at.0,
                            at.1,
                            &format!("`for ... in` recorre una tabla, y aqui hay {}", c.name(types)),
                            "las vueltas de `for x in t` son las celdas de la tabla `t`",
                            "for x in [1, 2, 3]  o  for i in range(10)",
                        ));
                    }
                    known[*local] = Some(c);
                }
                Op::Set { local, value, at } => {
                    let c = class(value, &known, m)?;
                    // ** A `mut` changes its VALUE, never its kind: a number
                    // stays a number (Python lets `x = 5` become `x = "hola"`;
                    // the checker could not say what x is).
                    if let Some(old) = &known[*local] {
                        if !fits(old, &c) {
                            return Err(Message::new(
                                Code::Retype,
                                at.0,
                                at.1,
                                &format!("aqui `{}` pasaria de {} a {}", f.locals[*local].name, old.name(types), c.name(types)),
                                "un `mut` cambia de valor, no de clase: el que lee tiene que saber siempre que es",
                                "dale un valor de la misma clase, o usa otro nombre para el nuevo",
                            ));
                        }
                    }
                }
                Op::SetAt { local, path, value, at } => {
                    let mut c = known[*local].clone().expect("juez: the local has a value");
                    for st in path {
                        c = step_class(&c, st, &known, m, *at)?;
                    }
                    let got = class(value, &known, m)?;
                    if !fits(&c, &got) {
                        return Err(wrong(value.at(), &c, &got, types, "esa parte de la tabla o del registro es de otra clase", "dale un valor de la clase de esa celda o ese campo"));
                    }
                }
                Op::Write { parts, .. } => {
                    for p in parts {
                        class(p, &known, m)?;
                    }
                }
                Op::Call { func, args, at } if m.functions[*func].gpu => {
                    gpu_call(*func, args, *at, &known, m)?;
                }
                Op::Call { func, args, at } => args_fit(*func, args, *at, &known, m)?,
                Op::Drop { local, .. } => known[*local] = None,
            }
        }
        if !f.gpu {
            match &b.end {
                End::Return(Some(v)) => cpu_f32(v, &known, m)?,
                End::Branch { cond, .. } => cpu_f32(cond, &known, m)?,
                _ => {}
            }
        }
        if let End::Return(Some(v)) = &b.end {
            let got = class(v, &known, m)?;
            let want = of_ty(f.ret.as_ref().expect("check: a return with a value is in a fn with ->"), types);
            if !fits(&want, &got) {
                return Err(wrong(v.at(), &want, &got, types, &format!("`fn {}` promete `-> {}` en su linea {}", f.name, want.short(types), f.line), "devuelve un valor de esa clase, o cambia lo que promete la primera linea"));
            }
        }
        if let End::Branch { cond, at, .. } = &b.end {
            let c = class(cond, &known, m)?;
            if c != Class::Bool {
                return Err(Message::new(
                    Code::NotBool,
                    at.0,
                    at.1,
                    &format!("un `if` pregunta SI o NO, y aqui hay {}", c.name(types)),
                    "TITAN++ no adivina que quiere decir un numero o un texto como condicion (en C, 0 es NO; en Python, \"\" tambien)",
                    &format!("di la pregunta entera: if {} > 0   o   if {} == ...", written(cond, f), written(cond, f)),
                ));
            }
        }
    }
    Ok(())
}

/// One step into a class: a cell of a table, a field of a record.
fn step_class(c: &Class, st: &PathStep, known: &[Option<Class>], m: &Module, at: At) -> Result<Class, Message> {
    match st {
        PathStep::Index(i) => {
            let ic = class(i, known, m)?;
            if ic != Class::Int {
                return Err(wrong(i.at(), &Class::Int, &ic, m.defs(), "una celda se pide con su numero: 0, 1, 2...", "a[0]"));
            }
            match c {
                Class::Table(inner, _) => Ok((**inner).clone()),
                other => Err(Message::new(Code::Mixed, at.0, at.1, &format!("{} no tiene celdas", other.name(m.defs())), "`[...]` pide una celda, y solo una tabla las tiene", "usa `[i]` sobre una tabla: [1, 2, 3][0]")),
            }
        }
        PathStep::Field(name, fat) => field_class(c, name, *fat, m),
    }
}

/// D4: may a value of class `got` become `want` by a DECLARED type? A number
/// into an f32, cell by cell into a table of f32.
fn into_f32(want: &Class, got: &Class) -> bool {
    match (want, got) {
        (Class::F32, Class::Int | Class::Dec) => true,
        (Class::Table(w, n), Class::Table(g, k)) => n == k && into_f32(w, g),
        _ => false,
    }
}

/// Is there an f32 in a value of this class (a cell, a field)?
fn holds_f32(c: &Class, d: Defs) -> bool {
    match c {
        Class::F32 => true,
        Class::Table(inner, _) => holds_f32(inner, d),
        Class::Record(t) => d.types[*t].fields.iter().any(|f| holds_f32(&of_ty(&f.ty, d), d)),
        _ => false,
    }
}

/// T0091: an f32 used on the CPU (D2).
fn f32_here(at: At, how: &str, why: &str) -> Message {
    Message::new(
        Code::F32Cpu,
        at.0,
        at.1,
        &format!("aqui un f32 {} en la CPU", how),
        why,
        "en la CPU un f32 se guarda o se pasa a otra gpu fn; para usarlo, se vuelve dec a la vista: round(x, 2)",
    )
}

/// ** D2 on the CPU: an f32 is never counted or compared here. `round(x, n)`
/// is the door out, so what it takes is not judged as a count -- only what
/// is inside it.
fn cpu_f32(v: &Value, known: &[Option<Class>], m: &Module) -> Result<(), Message> {
    match v {
        Value::Bin(op, l, r, at) => {
            if class(l, known, m)? == Class::F32 || class(r, known, m)? == Class::F32 {
                let how = if matches!(*op, "==" | "!=" | "<" | "<=" | ">" | ">=") { "se compara" } else { "se cuenta" };
                return Err(f32_here(*at, how, "la CPU cuenta con dec exacto (la regla de la casa, PLAN_EL_CENTAURO D2): el f32 cuenta dentro de una gpu fn, en la 3060"));
            }
            cpu_f32(l, known, m)?;
            cpu_f32(r, known, m)
        }
        Value::Neg(x, at) | Value::Not(x, at) => {
            if class(x, known, m)? == Class::F32 {
                return Err(f32_here(*at, "se cuenta", "la CPU cuenta con dec exacto (PLAN_EL_CENTAURO D2): el f32 cuenta en la 3060"));
            }
            cpu_f32(x, known, m)
        }
        Value::Round(x, _, _) | Value::Repeat(x, _, _) | Value::Field(x, _, _) | Value::Len(x, _) | Value::Is(x, _, _, _) | Value::Payload(x, _, _, _, _) => cpu_f32(x, known, m),
        Value::Index(b, i, _) => {
            cpu_f32(b, known, m)?;
            cpu_f32(i, known, m)
        }
        Value::Call(_, items, _) | Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) => items.iter().try_for_each(|x| cpu_f32(x, known, m)),
        _ => Ok(()),
    }
}

/// The class of a call to a `gpu fn` (level 11, D1): with values, its
/// result; with TABLES of n cells -- all of the same n -- n results, one per
/// thread.
fn gpu_call(func: usize, args: &[Value], at: At, known: &[Option<Class>], m: &Module) -> Result<Class, Message> {
    let g = &m.functions[func];
    let ret = of_ty(g.ret.as_ref().expect("gpu: a gpu fn gives a value"), m.defs());
    let got: Vec<Class> = args.iter().map(|a| class(a, known, m)).collect::<Result<_, _>>()?;
    let wants: Vec<Class> = g.params.iter().map(|(_, t)| of_ty(t, m.defs())).collect();
    if got.iter().zip(&wants).all(|(c, w)| c == w) {
        return Ok(ret);
    }
    let n = match got.first() {
        Some(Class::Table(_, n)) => *n,
        _ => 0,
    };
    let cells = got.iter().zip(&wants).all(|(c, w)| matches!(c, Class::Table(inner, k) if *k == n && **inner == *w));
    if n > 0 && cells {
        return Ok(Class::Table(Box::new(ret), n));
    }
    let want: Vec<String> = wants.iter().map(|w| w.short(m.defs())).collect();
    Err(Message::new(
        Code::WrongType,
        at.0,
        at.1,
        &format!("`{}` se aplica a valores ({}) o a tablas de esos valores, todas del mismo largo", g.name, want.join(", ")),
        &format!("aqui llega: {}", got.iter().map(|c| c.short(m.defs())).collect::<Vec<_>>().join(", ")),
        &format!("una celda de cada tabla por hilo: {}(xs, ys) con xs, ys de [f32; n]", g.name),
    ))
}

/// Does a value of class `c` keep the trait `k`? Its type has a
/// `trait ... for` of it (level 10).
fn keeps(c: &Class, k: usize, d: Defs) -> bool {
    let ty = match c {
        Class::Int => "int".to_string(),
        Class::Dec => "dec".to_string(),
        Class::Text => "text".to_string(),
        Class::Bool => "bool".to_string(),
        Class::Record(t) => d.types[*t].name.clone(),
        Class::Enum(e) => d.enums[*e].name.clone(),
        Class::F32 => "f32".to_string(),
        Class::Table(..) | Class::Trait(_) => return false,
    };
    d.impls.iter().any(|(t, i)| t == &d.traits[k].name && i == &ty)
}

/// The type a value IS, by name: what picks the fn of a trait it runs.
fn type_of(c: &Const, d: Defs) -> String {
    match c {
        Const::Int(_) => "int".into(),
        Const::Dec(..) => "dec".into(),
        Const::Text(_) => "text".into(),
        Const::Bool(_) => "bool".into(),
        Const::Record(t, _) => d.types[*t].name.clone(),
        Const::Variant(e, ..) => d.enums[*e].name.clone(),
        Const::F32(_) => "f32".into(),
        Const::Table(_) => "tabla".into(),
    }
}

/// The class of the field `name` of a value of class `c`, or T0073.
fn field_class(c: &Class, name: &str, at: At, m: &Module) -> Result<Class, Message> {
    if let Class::Trait(k) = c {
        let t = &m.traits[*k];
        return Err(Message::new(
            Code::Field,
            at.0,
            at.1,
            &format!("de algo que cumple `trait {}` no se conoce el campo `{}`", t.name, name),
            &format!("puede ser cualquier tipo que cumpla {}: de el solo se sabe lo que el trait promete (linea {}), sus fn", t.name, t.line),
            &format!("pidelo con una fn del trait: agrega `fn {}(x: {}) -> ...` a `trait {}`", name, t.name, t.name),
        ));
    }
    let Class::Record(t) = c else {
        return Err(Message::new(Code::Field, at.0, at.1, &format!("{} no tiene campos", c.name(m.defs())), "`.campo` pide un campo, y solo un registro (un `type`) los tiene", "usa `.x` sobre un registro: nave.x"));
    };
    let def = &m.defs()[*t];
    match def.fields.iter().find(|f| f.name == name) {
        Some(f) => Ok(of_ty(&f.ty, m.defs())),
        None => Err(Message::new(
            Code::Field,
            at.0,
            at.1,
            &format!("`{}` no tiene un campo `{}`", def.name, name),
            &format!("los campos de `type {}` (linea {}) son: {}", def.name, def.line, def.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")),
            "usa uno de esos, o agregalo al `type`",
        )),
    }
}

/// A value as the author wrote it, with the NAMES (the IR numbers them):
/// for the COMO of a message.
fn written(v: &Value, f: &Function) -> String {
    match v {
        Value::Int(n, _) => n.to_string(),
        Value::Text(t, _) => format!("{:?}", t),
        Value::Bool(b, _) => b.to_string(),
        Value::Dec(d, s, _) => show_dec(*d, *s),
        Value::Local(l, _) => f.locals[*l].name.clone(),
        Value::Bin(op, l, r, _) => format!("{} {} {}", written(l, f), op, written(r, f)),
        Value::Neg(v, _) => format!("-{}", written(v, f)),
        Value::Not(v, _) => format!("not {}", written(v, f)),
        Value::Field(b, n, _) => format!("{}.{}", written(b, f), n),
        Value::Index(b, i, _) => format!("{}[{}]", written(b, f), written(i, f)),
        Value::Len(v, _) => format!("len({})", written(v, f)),
        _ => "...".into(),
    }
}

/// The values given to a call, each against the class its parameter says.
fn args_fit(func: usize, args: &[Value], _at: At, known: &[Option<Class>], m: &Module) -> Result<(), Message> {
    let g = &m.functions[func];
    for ((a, (l, t)), mode) in args.iter().zip(&g.params).zip(&g.modes) {
        let got = class(a, known, m)?;
        let want = of_ty(t, m.defs());
        // ** A parameter of a TRAIT (level 10): any value whose type keeps
        // it -- said HERE, at the call, in one sentence (T0085), never from
        // inside the generic fn.
        if let Class::Trait(k) = want {
            if got == want || keeps(&got, k, m.defs()) {
                continue;
            }
            let t = &m.traits[k].name;
            let ty = got.short(m.defs());
            return Err(Message::new(
                Code::NotImpl,
                a.at().0,
                a.at().1,
                &format!("{} no cumple `trait {}`", ty, t),
                &format!("`{}` pide `{}: {}` (su linea {}): cualquier valor que sepa hacer lo que {} promete, y {} no dice como", g.name, g.locals[*l].name, t, g.line, t, ty),
                &format!("dile como: trait {} for {}, con sus fn ({})", t, ty, m.traits[k].methods.iter().map(|s| s.name.rsplit('.').next().unwrap_or(&s.name)).collect::<Vec<_>>().join(", ")),
            ));
        }
        // Lent to be changed: the SAME class, since what comes back goes in
        // the caller's own local (an int lent as a dec would come back a dec).
        let ok = if *mode == crate::tree::Mode::Mut { want == got } else { fits(&want, &got) };
        if !ok {
            return Err(wrong(
                a.at(),
                &want,
                &got,
                m.defs(),
                &format!("`{}` pide `{}: {}` (su linea {})", g.name, g.locals[*l].name, t.name(), g.line),
                "TITAN++ no convierte solo: dale un valor de esa clase",
            ));
        }
    }
    Ok(())
}

/// The class of a value, or the NO that says why it has none.
pub fn class(v: &Value, known: &[Option<Class>], m: &Module) -> Result<Class, Message> {
    let types = m.defs();
    Ok(match v {
        Value::Int(..) => Class::Int,
        Value::Text(..) => Class::Text,
        Value::Bool(..) => Class::Bool,
        Value::Dec(..) => Class::Dec,
        Value::F32(..) => Class::F32,
        Value::Local(l, _) | Value::Lend(_, l, _) => known[*l].clone().expect("juez: every local read has a value"),
        Value::Round(inner, _, at) => match class(inner, known, m)? {
            c if c.number() || c == Class::F32 => Class::Dec,
            c => return Err(Message::new(Code::Mixed, at.0, at.1, &format!("`round` redondea un numero, y aqui hay {}", c.name(types)), "solo un numero tiene decimales que redondear", "round(total / 3, 2)")),
        },
        Value::Call(func, args, at) if m.functions[*func].gpu => gpu_call(*func, args, *at, known, m)?,
        Value::Call(func, args, at) => {
            args_fit(*func, args, *at, known, m)?;
            of_ty(m.functions[*func].ret.as_ref().expect("check: a call used as a value gives one back"), types)
        }
        Value::Table(items, at) => {
            let mut c = class(&items[0], known, m)?;
            for i in &items[1..] {
                let ci = class(i, known, m)?;
                if fits(&c, &ci) {
                    continue;
                }
                if fits(&ci, &c) {
                    // [1, 2.5]: the whole table is `dec`.
                    c = ci;
                    continue;
                }
                return Err(wrong(i.at(), &c, &ci, types, "las celdas de una tabla son todas de UNA clase", &format!("o todas {}, o una tabla para cada clase", c.short(types))));
            }
            let _ = at;
            Class::Table(Box::new(c), items.len())
        }
        Value::Repeat(item, n, _) => Class::Table(Box::new(class(item, known, m)?), *n),
        Value::Index(b, i, at) => {
            let bc = class(b, known, m)?;
            step_class(&bc, &PathStep::Index((**i).clone()), known, m, *at)?
        }
        Value::Field(b, name, at) => field_class(&class(b, known, m)?, name, *at, m)?,
        Value::Record(t, items, _) => {
            let def = &types[*t];
            for (f, item) in def.fields.iter().zip(items) {
                let got = class(item, known, m)?;
                let want = of_ty(&f.ty, types);
                if !fits(&want, &got) {
                    return Err(wrong(item.at(), &want, &got, types, &format!("el campo `{}` de `type {}` es `{}` (linea {})", f.name, def.name, f.ty.name(), f.line), "dale un valor de esa clase"));
                }
            }
            Class::Record(*t)
        }
        Value::Variant(e, v, items, _) => {
            let case = &types.enums[*e].cases[*v];
            for (k, (want_ty, item)) in case.fields.iter().zip(items).enumerate() {
                let got = class(item, known, m)?;
                let want = of_ty(want_ty, types);
                if !fits(&want, &got) {
                    return Err(wrong(item.at(), &want, &got, types, &format!("el dato {} de `{}` es `{}` (linea {})", k + 1, case.name, want_ty.name(), case.line), "dale un valor de esa clase"));
                }
            }
            Class::Enum(*e)
        }
        Value::Is(inner, e, _, at) => match class(inner, known, m)? {
            Class::Enum(k) if k == *e => Class::Bool,
            other => return Err(wrong(*at, &Class::Enum(*e), &other, types, &format!("los casos de este `match` son los de `enum {}`", types.enums[*e].name), "dale al `match` un valor de ese enum")),
        },
        Value::Payload(_, e, v, k, _) => of_ty(&types.enums[*e].cases[*v].fields[*k], types),
        Value::Len(inner, at) => match class(inner, known, m)? {
            Class::Table(..) => Class::Int,
            other => {
                return Err(Message::new(Code::Mixed, at.0, at.1, &format!("`len` cuenta las celdas de una tabla, y aqui hay {}", other.name(types)), "solo una tabla tiene celdas que contar", "len([1, 2, 3])"))
            }
        },
        Value::Neg(inner, at) => match class(inner, known, m)? {
            c if c.number() || c == Class::F32 => c,
            c => return Err(Message::new(Code::Mixed, at.0, at.1, &format!("{} no tiene signo", c.name(types)), &format!("aqui hay {} con un `-` delante", c.name(types)), "el `-` va delante de un numero")),
        },
        Value::Not(inner, at) => match class(inner, known, m)? {
            Class::Bool => Class::Bool,
            c => {
                return Err(Message::new(
                    Code::NotBool,
                    at.0,
                    at.1,
                    &format!("`not` da la vuelta a un si-o-no, y aqui hay {}", c.name(types)),
                    "`not` cambia true por false y false por true; un numero o un texto no tienen vuelta",
                    "pregunta primero: not (vidas > 0)",
                ))
            }
        },
        Value::Bin(op, l, r, at) if class(l, known, m)? == Class::F32 || class(r, known, m)? == Class::F32 => {
            // ** f32 with f32 only (level 11): in the 3060 every number is
            // one; a dec or an int never becomes f32 without a declaration.
            let (a, b) = (class(l, known, m)?, class(r, known, m)?);
            if a != b {
                let other = if a == Class::F32 { b } else { a };
                return Err(Message::new(
                    Code::Mixed,
                    at.0,
                    at.1,
                    &format!("un f32 y {} no se mezclan", other.name(types)),
                    "un f32 redondea en base 2 y un dec es exacto: juntarlos callados perderia lo exacto sin decirlo",
                    "dentro de una gpu fn todo es f32; en la CPU, el dec entra a f32 por un tipo declarado: let xs: [f32; 4] = ...",
                ));
            }
            match *op {
                "+" | "-" | "*" | "/" => Class::F32,
                "==" | "!=" | "<" | "<=" | ">" | ">=" => Class::Bool,
                _ => return Err(Message::new(Code::Mixed, at.0, at.1, &format!("dos f32 no se pueden `{}`", op), "un f32 tiene + - * / y se compara; el resto `%` es de enteros", "a - b * floor(a / b) cuando llegue; hoy, ints para el resto")),
            }
        }
        Value::Bin(op, l, r, at) => {
            let (a, b) = (class(l, known, m)?, class(r, known, m)?);
            let numbers = a.number() && b.number();
            let dec = numbers && (a == Class::Dec || b == Class::Dec);
            match *op {
                "+" | "-" | "*" | "/" if numbers => {
                    if dec {
                        Class::Dec
                    } else {
                        Class::Int
                    }
                }
                "%" if a == Class::Int && b == Class::Int => Class::Int,
                "%" if numbers => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        "el resto `%` es de numeros enteros",
                        "un decimal no tiene resto: la division de un `dec` da su resultado EXACTO",
                        "divide con `/`, o usa ints para el resto",
                    ))
                }
                "+" if a == Class::Text && b == Class::Text => Class::Text,
                "==" | "!=" if a == b || numbers => Class::Bool,
                "<" | "<=" | ">" | ">=" if numbers => Class::Bool,
                "and" | "or" if a == Class::Bool && b == Class::Bool => Class::Bool,
                "and" | "or" => {
                    let odd = if a != Class::Bool { a } else { b };
                    return Err(Message::new(
                        Code::NotBool,
                        at.0,
                        at.1,
                        &format!("`{}` une dos si-o-no, y aqui hay {}", op, odd.name(types)),
                        "cada lado de `and` / `or` es una pregunta entera: `vidas and escudo` no dice que se pregunta",
                        &format!("compara cada lado: vidas > 0 {} escudo > 0", op),
                    ));
                }
                "<" | "<=" | ">" | ">=" if a == Class::Text && b == Class::Text => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("dos textos no se ordenan con `{}`", op),
                        "el orden de los textos depende del idioma (la n y la enie, las mayusculas); hoy un texto solo se compara con `==` y `!=`",
                        "compara si son iguales: a == b",
                    ))
                }
                "-" | "*" | "/" | "%" if a == Class::Text && b == Class::Text => {
                    return Err(Message::new(Code::Mixed, at.0, at.1, &format!("dos textos no se pueden `{}`", op), "con textos solo hay `+`: ponerlos uno detras del otro", "\"a\" + \"b\""))
                }
                "==" | "!=" => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("{} y {} no se comparan", a.name(types), b.name(types)),
                        "nunca son iguales, y TITAN++ no convierte solo: 1 == \"1\" seria una pregunta con trampa",
                        "compara cosas de la misma clase",
                    ))
                }
                _ => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("{} y {} no se pueden `{}`", a.name(types), b.name(types), op),
                        "TITAN++ no convierte solo: \"a\" + 1 no es \"a1\" (ni 1 + \"1\" es 2, ni true + 1 es 2)",
                        "para mostrarlos juntos, separalos con comas: print(\"total: \", n)",
                    ))
                }
            }
        }
    })
}

// ===================================================================
//  THE SECOND PASS: the program, run when compiling
// ===================================================================

/// ** THE PROGRAM, RUN WHEN COMPILING (level 4).
///
/// Until something comes from outside (the keyboard, a file), every value of
/// a program is known before it runs -- with loops and calls too. So the
/// calculation does not fold each line once (a line inside a loop has a
/// different value at every turn): it RUNS the program, from `main`, and
/// keeps what it writes. That list (`Module::flat`) is what the `.bex` does.
///
/// [!] The day something comes from outside, part of the program can no longer
/// be run here, and that part goes to the machine as real code (E1 of the
/// emitter, TITAN_MAESTRO 7.3). This is the floor, E0, made whole.
struct Run<'m, 'd> {
    m: &'m Module,
    /// Who runs a `gpu fn` (level 11), if someone does: otherwise, this.
    device: Option<&'d mut dyn Device>,
    steps: u64,
    /// How many calls are open right now.
    depth: usize,
    flat: Vec<Op>,
    /// Which blocks of which function some run entered.
    seen: Vec<Vec<bool>>,
    /// The last question a loop asked: where T0066 points.
    last_turn: At,
    /// Inside a `round(...)`: a division that does not end is carried to
    /// SCALE decimals, for `round` to cut where it SAYS (level 7).
    lenient: u32,
}

impl Run<'_, '_> {
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

    /// Runs `func` with these values: what it gives back, if anything, and
    /// how its parameters ended (what a `mut` one gives back to the caller).
    fn call(&mut self, func: usize, args: Vec<Const>, at: At) -> Result<(Option<Const>, Vec<Const>), Message> {
        let m = self.m;
        let f = &m.functions[func];
        // ** A trait's fn (level 10): the type of the first value picks
        // which fn runs. Every value is known here, so the pick is exact;
        // `classes` already proved the type keeps the trait.
        if let Some(table) = &f.dispatch {
            let ty = type_of(&args[0], m.defs());
            let target = table.iter().find(|(t, _)| *t == ty).map(|(_, k)| *k).expect("classes: the type keeps the trait");
            return self.call(target, args, at);
        }
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
        // The types DECLARED (a parameter's, a `let x: T`'s): every value the
        // local ever takes must fit it (COBOL's PIC, level 7).
        let mut decl: Vec<Option<Ty>> = vec![None; f.locals.len()];
        for ((l, t), a) in f.params.iter().zip(args) {
            known[*l] = Some(fit_into(a, Some(t), m.defs(), at)?);
            decl[*l] = Some(t.clone());
        }
        let mut b = 0;
        let result = loop {
            self.seen[func][b] = true;
            for op in &f.blocks[b].ops {
                self.tick()?;
                match op {
                    Op::Let { local, value, ty, at, .. } => {
                        let v = self.ev(value, &mut known)?;
                        known[*local] = Some(fit_into(v, ty.as_ref(), m.defs(), *at)?);
                        decl[*local] = ty.clone();
                    }
                    Op::Set { local, value, at } => {
                        let v = self.ev(value, &mut known)?;
                        // 13 into a `dec` of 2 decimals is 13.00.
                        let v = match (&known[*local], v) {
                            (Some(Const::Dec(_, s)), Const::Int(n)) if decl[*local].is_none() => to_dec(n, *s, value.at())?,
                            (_, v) => v,
                        };
                        known[*local] = Some(fit_into(v, decl[*local].as_ref(), m.defs(), *at)?);
                    }
                    Op::SetAt { local, path, value, at } => {
                        let v = self.ev(value, &mut known)?;
                        let mut steps = Vec::with_capacity(path.len());
                        for st in path {
                            steps.push(match st {
                                PathStep::Index(i) => Err((self.ev(i, &mut known)?, i.at())),
                                PathStep::Field(name, _) => Ok(name.as_str()),
                            });
                        }
                        let mut whole = known[*local].take().expect("juez: the local has a value");
                        set_in(&mut whole, &steps, v, m.defs(), *at)?;
                        known[*local] = Some(fit_into(whole, decl[*local].as_ref(), m.defs(), *at)?);
                    }
                    Op::Write { parts, at } => {
                        let mut out = Vec::with_capacity(parts.len());
                        for p in parts {
                            let c = self.ev(p, &mut known)?;
                            out.push(Value::Text(c.show(m.defs()), p.at()));
                        }
                        self.flat.push(Op::Write { parts: out, at: *at });
                    }
                    Op::Call { func, args, at } => {
                        self.call_with(*func, args, *at, &mut known)?;
                    }
                    Op::Drop { local, .. } => known[*local] = None,
                }
            }
            self.tick()?;
            b = match &f.blocks[b].end {
                End::Return(v) => {
                    break match v {
                        Some(v) => {
                            let c = self.ev(v, &mut known)?;
                            Some(fit_into(c, f.ret.as_ref(), m.defs(), v.at())?)
                        }
                        None => None,
                    }
                }
                End::Jump(t) => *t,
                End::Branch { cond, then, other, at } => {
                    if *then < b || *other < b || f.blocks.iter().skip(b).any(|x| x.end.targets().contains(&b)) {
                        self.last_turn = *at;
                    }
                    if matches!(self.ev(cond, &mut known)?, Const::Bool(true)) {
                        *then
                    } else {
                        *other
                    }
                }
            };
        };
        self.depth -= 1;
        let finals = f.params.iter().map(|(l, _)| known[*l].clone().unwrap_or(Const::Bool(false))).collect();
        Ok((result, finals))
    }

    /// A call from a frame: the values given, the call run, and then what
    /// lending and giving do to the caller's locals -- a `mut` one gets back
    /// what the function left in it; a `take` one is gone.
    fn call_with(&mut self, func: usize, args: &[Value], at: At, known: &mut Vec<Option<Const>>) -> Result<Option<Const>, Message> {
        let vals = args.iter().map(|a| self.ev(a, known)).collect::<Result<Vec<_>, _>>()?;
        // ** A `gpu fn` applied to TABLES (level 11, D1): one cell of each
        // per thread -- here, one call per cell, in order. The 3060 runs
        // them at once; the result is the same, cell by cell.
        if self.m.functions[func].gpu && self.device.is_some() {
            return self.on_device(func, vals, at).map(Some);
        }
        if self.m.functions[func].gpu && vals.iter().any(|v| matches!(v, Const::Table(_))) {
            let n = match &vals[0] {
                Const::Table(items) => items.len(),
                _ => 0,
            };
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let cell: Vec<Const> = vals.iter().map(|v| if let Const::Table(items) = v { items[i].clone() } else { v.clone() }).collect();
                out.push(self.call(func, cell, at)?.0.expect("gpu: a gpu fn gives a value"));
            }
            return Ok(Some(Const::Table(out)));
        }
        let (result, finals) = self.call(func, vals, at)?;
        for (a, back) in args.iter().zip(finals) {
            match a {
                Value::Lend(crate::tree::Mode::Mut, l, _) => known[*l] = Some(back),
                Value::Lend(crate::tree::Mode::Take, l, _) => known[*l] = None,
                _ => {}
            }
        }
        Ok(result)
    }

    /// ** A `gpu fn` run by the DEVICE (level 11, G3): its cells, by their
    /// bits, and back. A value that is not a table is one cell; tables give
    /// one cell per thread.
    fn on_device(&mut self, func: usize, vals: Vec<Const>, at: At) -> Result<Const, Message> {
        let m = self.m;
        let table = vals.iter().any(|v| matches!(v, Const::Table(_)));
        let n = vals.iter().find_map(|v| if let Const::Table(items) = v { Some(items.len()) } else { None }).unwrap_or(1);
        let bits = |c: &Const| match c {
            Const::F32(b) => *b,
            Const::Bool(b) => *b as u32,
            _ => 0,
        };
        let cells: Vec<Vec<u32>> = vals.iter().map(|v| match v {
            Const::Table(items) => items.iter().map(bits).collect(),
            one => vec![bits(one); n],
        }).collect();
        let device = self.device.as_mut().expect("the caller checked");
        let out = device.run(m, func, cells).map_err(|why| {
            Message::new(
                Code::GpuBody,
                at.0,
                at.1,
                &format!("la 3060 no pudo correr `{}`", m.functions[func].name),
                &format!("es un fallo del escritor de SPIR-V o del oraculo, no del programa: {}", why),
                "avisa con este programa",
            )
        })?;
        let ret_bool = m.functions[func].ret == Some(Ty::Bool);
        let back: Vec<Const> = out.into_iter().map(|b| if ret_bool { Const::Bool(b != 0) } else { Const::F32(b) }).collect();
        Ok(if table { Const::Table(back) } else { back.into_iter().next().unwrap_or(Const::F32(0)) })
    }

    /// A value, calculated -- running the calls inside it.
    fn ev(&mut self, v: &Value, known: &mut Vec<Option<Const>>) -> Result<Const, Message> {
        let types = self.m.defs();
        Ok(match v {
            Value::Int(n, _) => Const::Int(*n),
            Value::Text(t, _) => Const::Text(t.clone()),
            Value::Bool(b, _) => Const::Bool(*b),
            Value::Dec(d, s, _) => Const::Dec(*d, *s),
            Value::F32(b, _) => Const::F32(*b),
            Value::Local(l, _) | Value::Lend(_, l, _) => known[*l].clone().expect("juez: every local read has a value"),
            Value::Call(func, args, at) => self.call_with(*func, args, *at, known)?.expect("check: a call used as a value gives one back"),
            Value::Round(inner, n, at) => {
                self.lenient += 1;
                let c = self.ev(inner, known);
                self.lenient -= 1;
                round_to(c?, *n, *at)?
            }
            Value::Table(items, _) => {
                let vals = items.iter().map(|i| self.ev(i, known)).collect::<Result<Vec<_>, _>>()?;
                // [1, 2.5] is a table of `dec`: every cell, a decimal.
                if vals.iter().any(|c| matches!(c, Const::Dec(..))) {
                    let scale = vals.iter().map(|c| if let Const::Dec(_, s) = c { *s } else { 0 }).max().unwrap_or(0);
                    vals.into_iter().map(|c| rescale(c, scale, v.at())).collect::<Result<Vec<_>, _>>().map(Const::Table)?
                } else {
                    Const::Table(vals)
                }
            }
            Value::Repeat(item, n, _) => {
                let c = self.ev(item, known)?;
                Const::Table(vec![c; *n])
            }
            Value::Index(b, i, at) => {
                let table = self.ev(b, known)?;
                let idx = self.ev(i, known)?;
                cell(&table, &idx, *at)?.clone()
            }
            Value::Field(b, name, _) => match self.ev(b, known)? {
                Const::Record(t, items) => {
                    let k = types[t].fields.iter().position(|f| &f.name == name).expect("classes: the field exists");
                    items[k].clone()
                }
                _ => unreachable!("classes: only a record has fields"),
            },
            Value::Record(t, items, at) => {
                let mut vals = Vec::with_capacity(items.len());
                for (f, i) in types[*t].fields.iter().zip(items) {
                    let c = self.ev(i, known)?;
                    vals.push(fit_into(c, Some(&f.ty), types, *at)?);
                }
                Const::Record(*t, vals)
            }
            Value::Variant(e, v, items, at) => {
                let mut vals = Vec::with_capacity(items.len());
                for (ty, i) in types.enums[*e].cases[*v].fields.iter().zip(items) {
                    let c = self.ev(i, known)?;
                    vals.push(fit_into(c, Some(ty), types, *at)?);
                }
                Const::Variant(*e, *v, vals)
            }
            Value::Is(inner, e, v, _) => match self.ev(inner, known)? {
                Const::Variant(k, w, _) => Const::Bool(k == *e && w == *v),
                _ => unreachable!("classes: a match reads a value of its enum"),
            },
            Value::Payload(inner, _, _, k, _) => match self.ev(inner, known)? {
                Const::Variant(_, _, mut items) => items.swap_remove(*k),
                _ => unreachable!("classes: a match reads a value of its enum"),
            },
            Value::Len(inner, _) => match self.ev(inner, known)? {
                Const::Table(items) => Const::Int(items.len() as i64),
                _ => unreachable!("classes: only a table has cells"),
            },
            Value::Neg(inner, at) => match self.ev(inner, known)? {
                Const::F32(b) => Const::F32((-f32::from_bits(b)).to_bits()),
                Const::Int(n) => n.checked_neg().map(Const::Int).ok_or_else(|| overflow(*at, &format!("-({})", n)))?,
                Const::Dec(d, s) => d.checked_neg().map(|d| Const::Dec(d, s)).ok_or_else(|| overflow(*at, &format!("-({})", show_dec(d, s))))?,
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
                binop(op, a, b, *at, self.lenient > 0)?
            }
        })
    }
}

/// ** A VALUE GOING WHERE A TYPE IS SAID -- COBOL's PIC, enforced.
///
/// An `int` that goes to a `dec` becomes one (13 is 13 with no decimals). A
/// `dec(7, 2)` takes a value with AT MOST 2 decimals (padded to 2: 12.5 is
/// 12.50) and at most 5 digits before the point -- anything else is T0074,
/// COBOL's SIZE ERROR, but never ignored: COBOL cuts it in silence unless the
/// author wrote ON SIZE ERROR; TITAN++ does not compile. Tables and records
/// go cell by cell, field by field.
fn fit_into(c: Const, ty: Option<&Ty>, types: Defs, at: At) -> Result<Const, Message> {
    match (c, ty) {
        (Const::Int(n), Some(Ty::Dec)) => Ok(Const::Dec(n, 0)),
        // D4: the exact decimal, rounded ONCE to the nearest f32, where the
        // declared type says so.
        (Const::Int(n), Some(Ty::F32)) => Ok(Const::F32((n as f32).to_bits())),
        (Const::Dec(d, s), Some(Ty::F32)) => Ok(Const::F32(show_dec(d, s).parse::<f32>().unwrap_or(f32::NAN).to_bits())),
        (c @ (Const::Int(_) | Const::Dec(..)), Some(Ty::DecP(p, s))) => {
            let (d, sc) = parts(&c);
            let shown = c.show(types);
            if sc > *s {
                // More decimals than declared: only if the extra ones are 0.
                let (t, ts) = trim(d, sc, *s);
                if ts > *s {
                    return Err(Message::new(
                        Code::Size,
                        at.0,
                        at.1,
                        &format!("{} tiene mas decimales de los que caben en dec({}, {})", shown, p, s),
                        &format!("dec({}, {}) guarda {} decimales, y cortar los otros seria perder dinero en silencio", p, s, s),
                        &format!("redondea a la vista: round(..., {}), o declara mas decimales", s),
                    ));
                }
                return fit_into(Const::Dec(i64::try_from(t).unwrap_or(0), ts), ty, types, at);
            }
            let padded = d * pow10(s - sc);
            if padded.abs() >= pow10(*p) {
                return Err(Message::new(
                    Code::Size,
                    at.0,
                    at.1,
                    &format!("{} no cabe en dec({}, {}): llega hasta {}", shown, p, s, show_dec(i64::try_from(pow10(*p) - 1).unwrap_or(i64::MAX), *s)),
                    "el SIZE ERROR de COBOL: un numero mas grande que sus cifras declaradas. COBOL lo corta callado si no pones ON SIZE ERROR; TITAN++ no compila",
                    &format!("declara mas cifras: dec({}, {})", p + 1, s),
                ));
            }
            Ok(Const::Dec(i64::try_from(padded).unwrap_or(0), *s))
        }
        (Const::Table(items), Some(Ty::Table(inner, _))) => items.into_iter().map(|i| fit_into(i, Some(inner), types, at)).collect::<Result<Vec<_>, _>>().map(Const::Table),
        (Const::Record(t, items), _) => {
            let fields = &types[t].fields;
            items.into_iter().zip(fields).map(|(i, f)| fit_into(i, Some(&f.ty), types, at)).collect::<Result<Vec<_>, _>>().map(|v| Const::Record(t, v))
        }
        (Const::Variant(e, v, items), _) => {
            let fields = &types.enums[e].cases[v].fields;
            items.into_iter().zip(fields).map(|(i, f)| fit_into(i, Some(f), types, at)).collect::<Result<Vec<_>, _>>().map(|x| Const::Variant(e, v, x))
        }
        (c, _) => Ok(c),
    }
}

/// `round(x, n)`: to `n` decimals, half away from zero -- COBOL's ROUNDED
/// (2.345 -> 2.35, -2.345 -> -2.35). Written, never silent.
fn round_to(c: Const, n: u32, at: At) -> Result<Const, Message> {
    if let Const::F32(b) = c {
        return round_f32(f32::from_bits(b), n, at);
    }
    let (d, s) = parts(&c);
    if s <= n {
        return dec_result(d * pow10(n - s), n, at, &c.show(NONE));
    }
    let q = pow10(s - n);
    let (mut r, rem) = (d / q, d % q);
    if rem.abs() * 2 >= q {
        r += d.signum();
    }
    dec_result(r, n, at, &c.show(NONE))
}

/// ** `round(x, n)` of an f32 -- THE door from the 3060 to the CPU (D2).
/// The f32's value is EXACT in decimal (a binary fraction ends), so it is
/// written out whole and rounded at `n` digits half away from zero, as every
/// `round` of TITAN++. NaN and infinity are not numbers: a NO.
fn round_f32(v: f32, n: u32, at: At) -> Result<Const, Message> {
    if !v.is_finite() {
        return Err(Message::new(
            Code::Inexact,
            at.0,
            at.1,
            &format!("`round` recibe {} de la 3060, y eso no es un numero", v),
            "un f32 que dividio entre cero, o desbordo, queda en infinito o NaN: no hay decimal que lo diga",
            "comprueba el divisor dentro de la gpu fn: if d != 0.0",
        ));
    }
    // 150 decimals hold every f32 exactly (its smallest step is 2^-149).
    let s = format!("{:.150}", (v as f64).abs());
    let (whole, frac) = s.split_once('.').unwrap_or((&s, ""));
    let keep = &frac[..(n as usize).min(frac.len())];
    let up = frac.as_bytes().get(n as usize).is_some_and(|d| *d >= b'5');
    let digits: i128 = format!("{}{}", whole, keep).parse::<i128>().map_err(|_| overflow(at, &format!("round({}, {})", v, n)))?;
    let r = (digits + up as i128) * if v < 0.0 { -1 } else { 1 };
    dec_result(r, n, at, &format!("round({}, {})", v, n))
}

/// The cell `idx` of `table`, or T0072.
fn cell<'c>(table: &'c Const, idx: &Const, at: At) -> Result<&'c Const, Message> {
    let (Const::Table(items), Const::Int(i)) = (table, idx) else { unreachable!("classes: a table and an int") };
    if *i < 0 || *i as usize >= items.len() {
        return Err(outside(*i, items.len(), at));
    }
    Ok(&items[*i as usize])
}

fn outside(i: i64, len: usize, at: At) -> Message {
    Message::new(
        Code::Outside,
        at.0,
        at.1,
        &format!("la celda {} no existe: la tabla tiene {} (de la 0 a la {})", i, len, len as i64 - 1),
        "se corrio al compilar y se vio: este indice cae fuera. En C esto lee memoria de otro (el fallo mas viejo del mundo); aqui no compila",
        &format!("pide una celda de 0 a {}, o mira antes: if i < len(tabla)", len as i64 - 1),
    )
}

/// `whole` with the part at `steps` changed to `v`. A step is a cell (the
/// index, already calculated) or a field (by name).
fn set_in(whole: &mut Const, steps: &[Result<&str, (Const, At)>], v: Const, types: Defs, at: At) -> Result<(), Message> {
    let Some((first, rest)) = steps.split_first() else {
        let v = match (&*whole, v) {
            (Const::Dec(_, s), Const::Int(n)) => to_dec(n, *s, at)?,
            (Const::Dec(_, s), Const::Dec(d, ds)) => rescale(Const::Dec(d, ds), (*s).max(ds), at)?,
            (_, v) => v,
        };
        *whole = v;
        return Ok(());
    };
    match (whole, first) {
        (Const::Table(items), Err((Const::Int(i), iat))) => {
            if *i < 0 || *i as usize >= items.len() {
                return Err(outside(*i, items.len(), *iat));
            }
            set_in(&mut items[*i as usize], rest, v, types, at)
        }
        (Const::Record(t, items), Ok(name)) => {
            let k = types[*t].fields.iter().position(|f| f.name == *name).expect("classes: the field exists");
            set_in(&mut items[k], rest, v, types, at)
        }
        _ => unreachable!("classes: every step fits its value"),
    }
}

// ===================================================================
//  THE ARITHMETIC: ints, and the exact decimal
// ===================================================================

fn pow10(k: u32) -> i128 {
    10i128.pow(k)
}

/// `n` as a decimal of `scale` decimals (13 with 2 is 1300).
fn to_dec(n: i64, scale: u32, at: At) -> Result<Const, Message> {
    let d = n as i128 * pow10(scale);
    i64::try_from(d).map(|d| Const::Dec(d, scale)).map_err(|_| overflow(at, &n.to_string()))
}

/// A number with at least `scale` decimals (an int becomes a decimal).
fn rescale(c: Const, scale: u32, at: At) -> Result<Const, Message> {
    match c {
        Const::Int(n) => to_dec(n, scale, at),
        Const::Dec(d, s) if s >= scale => Ok(Const::Dec(d, s)),
        Const::Dec(d, s) => {
            let v = d as i128 * pow10(scale - s);
            i64::try_from(v).map(|v| Const::Dec(v, scale)).map_err(|_| overflow(at, &show_dec(d, s)))
        }
        other => Ok(other),
    }
}

/// Both numbers as (digits, scale) with the SAME scale, in 128 bits.
fn align(a: &Const, b: &Const) -> (i128, i128, u32) {
    let (da, sa) = parts(a);
    let (db, sb) = parts(b);
    let s = sa.max(sb);
    (da * pow10(s - sa), db * pow10(s - sb), s)
}

fn parts(c: &Const) -> (i128, u32) {
    match c {
        Const::Int(n) => (*n as i128, 0),
        Const::Dec(d, s) => (*d as i128, *s),
        _ => unreachable!("classes: a number"),
    }
}

/// Removes trailing zero decimals while the scale is above `keep`.
fn trim(mut d: i128, mut s: u32, keep: u32) -> (i128, u32) {
    while s > keep && d % 10 == 0 {
        d /= 10;
        s -= 1;
    }
    (d, s)
}

fn dec_result(d: i128, s: u32, at: At, what: &str) -> Result<Const, Message> {
    i64::try_from(d).ok().filter(|_| s <= SCALE).map(|d| Const::Dec(d, s)).ok_or_else(|| overflow(at, what))
}

/// The four operations with at least one `dec`: exact, or a NO.
fn decimal(op: &str, a: Const, b: Const, at: At, lenient: bool) -> Result<Const, Message> {
    let what = format!("{} {} {}", a.show(NONE), op, b.show(NONE));
    let ((da, sa), (db, sb)) = (parts(&a), parts(&b));
    match op {
        "+" | "-" => {
            let (x, y, s) = align(&a, &b);
            let r = if op == "+" { x + y } else { x - y };
            dec_result(r, s, at, &what)
        }
        // Multiplying adds the decimals (COBOL's rule): 12.50 * 2 = 25.00.
        "*" => {
            let (r, s) = trim(da * db, sa + sb, sa.max(sb));
            dec_result(r, s, at, &what)
        }
        "/" => {
            if db == 0 {
                return Err(Message::new(Code::DivZero, at.0, at.1, &format!("{} divide entre cero", what), "entre cero no hay numero que valga: ni infinito, ni cero", "comprueba el divisor antes: if d != 0.0"));
            }
            // a / b = (da / 10^sa) / (db / 10^sb) = da * 10^sb / (db * 10^sa).
            // The fewest decimals (at least those of the two) that make it
            // EXACT -- or a NO: 1.0 / 3 never ends, and is not cut in silence.
            let (num, den) = (da * pow10(sb), db * pow10(sa));
            for s in sa.max(sb)..=SCALE {
                let n = num * pow10(s);
                if n % den == 0 {
                    return dec_result(n / den, s, at, &what);
                }
            }
            if lenient {
                // Inside `round`: carried to SCALE decimals (cut, not
                // rounded), and `round` decides where it ends. Cutting at 18
                // never changes a rounding to fewer decimals.
                return dec_result(num * pow10(SCALE) / den, SCALE, at, &what);
            }
            Err(Message::new(
                Code::Inexact,
                at.0,
                at.1,
                &format!("{} no da un decimal exacto", what),
                &format!("sus decimales no acaban en {} cifras (como 1 / 3 = 0.333...): TITAN++ no corta un numero a escondidas", SCALE),
                "si hay que redondear, se ESCRIBE: round(a / b, 2) -- el redondeo de COBOL (ROUNDED), visible",
            ))
        }
        _ => Err(unclassed(at, &a)),
    }
}

/// Two values and an operator: comparisons, arithmetic, texts joined.
fn binop(op: &str, a: Const, b: Const, at: At, lenient: bool) -> Result<Const, Message> {
    // ** Two f32 (level 11): IEEE single precision, as the 3060 counts --
    // each operation rounded once, no fused steps.
    if let (Const::F32(x), Const::F32(y)) = (&a, &b) {
        let (x, y) = (f32::from_bits(*x), f32::from_bits(*y));
        return Ok(match op {
            "+" => Const::F32((x + y).to_bits()),
            "-" => Const::F32((x - y).to_bits()),
            "*" => Const::F32((x * y).to_bits()),
            "/" => Const::F32((x / y).to_bits()),
            "==" => Const::Bool(x == y),
            "!=" => Const::Bool(x != y),
            "<" => Const::Bool(x < y),
            "<=" => Const::Bool(x <= y),
            ">" => Const::Bool(x > y),
            ">=" => Const::Bool(x >= y),
            _ => return Err(unclassed(at, &a)),
        });
    }
    let numbers = matches!(a, Const::Int(_) | Const::Dec(..)) && matches!(b, Const::Int(_) | Const::Dec(..));
    let dec = numbers && (matches!(a, Const::Dec(..)) || matches!(b, Const::Dec(..)));
    if numbers && matches!(op, "==" | "!=" | "<" | "<=" | ">" | ">=") {
        let (x, y, _) = align(&a, &b);
        return Ok(Const::Bool(match op {
            "==" => x == y,
            "!=" => x != y,
            "<" => x < y,
            "<=" => x <= y,
            ">" => x > y,
            _ => x >= y,
        }));
    }
    if dec {
        return decimal(op, a, b, at, lenient);
    }
    // Inside `round`, 7 / 2 between ints is the exact 3.5, to be rounded.
    if lenient && op == "/" {
        if let (Const::Int(x), Const::Int(y)) = (&a, &b) {
            if *y != 0 && x % y != 0 {
                return decimal(op, a, b, at, lenient);
            }
        }
    }
    match (op, &a, &b) {
        ("==", _, _) => Ok(Const::Bool(same(&a, &b))),
        ("!=", _, _) => Ok(Const::Bool(!same(&a, &b))),
        (_, Const::Int(x), Const::Int(y)) => int(op, *x, *y, at),
        ("+", Const::Text(x), Const::Text(y)) => Ok(Const::Text(format!("{}{}", x, y))),
        _ => Err(unclassed(at, &a)),
    }
}

/// Equal VALUES: 12.50 and 12.5 are the same number; tables and records,
/// cell by cell.
fn same(a: &Const, b: &Const) -> bool {
    match (a, b) {
        (Const::Int(_) | Const::Dec(..), Const::Int(_) | Const::Dec(..)) => {
            let (x, y, _) = align(a, b);
            x == y
        }
        (Const::Table(x), Const::Table(y)) | (Const::Record(_, x), Const::Record(_, y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        // The same case, carrying the same values: Circulo(2.0) == Circulo(2.00).
        (Const::Variant(e, v, x), Const::Variant(f, w, y)) => e == f && v == w && x.iter().zip(y).all(|(p, q)| same(p, q)),
        _ => a == b,
    }
}

fn constant(c: &Const, at: At) -> Value {
    match c {
        Const::Int(n) => Value::Int(*n, at),
        Const::Text(t) => Value::Text(t.clone(), at),
        Const::Bool(b) => Value::Bool(*b, at),
        Const::Dec(d, s) => Value::Dec(*d, *s, at),
        other => Value::Text(other.show(NONE), at),
    }
}

/// A kind that the first pass should have stopped: said, never invented.
fn unclassed(at: At, a: &Const) -> Message {
    let _ = constant;
    Message::new(Code::Mixed, at.0, at.1, &format!("aqui no cabe {}", a.show(NONE)), "el calculo encontro una clase que la primera pasada no vio", "esto es un fallo del compilador: avisa con este programa")
}

fn overflow(at: At, what: &str) -> Message {
    Message::new(
        Code::Overflow,
        at.0,
        at.1,
        &format!("{} no cabe en un numero", what),
        "un numero entero de TITAN++ ocupa 64 bits, y desbordar es un error, no una vuelta a empezar",
        "usa numeros mas chicos: un `int` y las cifras de un `dec` caben en 64 bits",
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
                &format!("el resto es {} % {}; para el resultado con decimales, un `dec`: {}.0 / {} da el exacto", x, y, x, y),
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
    fn dec_is_exact_and_never_a_float() {
        assert_eq!(printed("mod main \"x\"\nfn main()\n    print(0.1 + 0.2)\n"), ["0.3"]);
        assert_eq!(printed("mod main \"x\"\nfn main()\n    let precio = 12.50\n    print(precio * 3, \" \", precio + 1, \" \", 10.00 / 4, \" \", 7.0 / 2)\n"), ["37.50 13.50 2.50 3.5"]);
        assert_eq!(printed("mod main \"x\"\nfn main()\n    print(12.50 == 12.5, \" \", 0.3 > 0.29, \" \", -1.25)\n"), ["true true -1.25"]);
        // 1 / 3 never ends: a NO, never a cut.
        assert_eq!(run("mod main \"x\"\nfn main()\n    print(1.0 / 3)\n").unwrap_err().code, Code::Inexact);
        // A decimal has no rest.
        assert_eq!(run("mod main \"x\"\nfn main()\n    print(2.5 % 2)\n").unwrap_err().code, Code::Mixed);
        // 13 into a `mut` dec of two decimals is 13.00.
        assert_eq!(printed("mod main \"x\"\nfn main()\n    let mut p = 12.50\n    p = 13\n    print(p)\n"), ["13.00"]);
    }

    #[test]
    fn tables_and_records_are_values_with_their_cells_and_fields() {
        let src = "mod main \"x\"\ntype Nave\n    x: dec\n    nombre: text\nfn main()\n    let mut t = [3, 1, 2]\n    t[0] = 9\n    let mut n = Nave { nombre: \"centauro\", x: 1.5 }\n    n.x = n.x * 2\n    print(t, \" \", len(t), \" \", t[2])\n    print(n)\n    let mut s = 0\n    for v in t\n        s = s + v\n    print(s, \" \", [0; 3])\n";
        assert_eq!(printed(src), ["[9, 1, 2] 3 2", "Nave { x: 3.0, nombre: \"centauro\" }", "12 [0, 0, 0]"]);
        // A cell outside its table: seen when compiling.
        assert_eq!(run("mod main \"x\"\nfn main()\n    let t = [1, 2, 3]\n    print(t[3])\n").unwrap_err().code, Code::Outside);
        // A field the record does not have.
        assert_eq!(run("mod main \"x\"\ntype P\n    x: int\nfn main()\n    let p = P { x: 1 }\n    print(p.y)\n").unwrap_err().code, Code::Field);
        // One class per table.
        assert_eq!(run("mod main \"x\"\nfn main()\n    print([1, \"dos\"])\n").unwrap_err().code, Code::WrongType);
    }

    #[test]
    fn mut_gives_back_the_changes_and_cobol_precision_is_enforced() {
        let src = "mod main \"x\"\nfn ordena(mut t: [int; 3])\n    for i in range(3)\n        for j in range(2 - i)\n            if t[j] > t[j + 1]\n                let m = t[j + 1]\n                t[j + 1] = t[j]\n                t[j] = m\nfn main()\n    let mut t = [3, 1, 2]\n    ordena(mut t)\n    print(t)\n";
        assert_eq!(printed(src), ["[1, 2, 3]"]);
        // dec(7, 2): padded to its decimals; COBOL's PIC.
        assert_eq!(printed("mod main \"x\"\nfn main()\n    let precio: dec(7, 2) = 12.5\n    print(precio)\n"), ["12.50"]);
        // SIZE ERROR: too many digits, or too many decimals -- never cut.
        assert_eq!(run("mod main \"x\"\nfn main()\n    let p: dec(5, 2) = 1234.5\n    print(p)\n").unwrap_err().code, Code::Size);
        assert_eq!(run("mod main \"x\"\nfn main()\n    let p: dec(7, 2) = 1.255\n    print(p)\n").unwrap_err().code, Code::Size);
        // ... unless the rounding is WRITTEN: COBOL's ROUNDED, half away from zero.
        assert_eq!(printed("mod main \"x\"\nfn main()\n    let p: dec(7, 2) = round(1.255, 2)\n    print(p, \" \", round(10.00 / 3, 2), \" \", round(-2.345, 2), \" \", round(7 / 2, 0))\n"), ["1.26 3.33 -2.35 4"]);
        // A `mut` dec(7, 2) keeps its type at every change.
        assert_eq!(run("mod main \"x\"\nfn main()\n    let mut saldo: dec(5, 2) = 900.00\n    saldo = saldo * 200\n    print(saldo)\n").unwrap_err().code, Code::Size);
    }

    /// Level 8: a case carries its data, `match` takes it out, and every
    /// rule of the cases has its NO.
    #[test]
    fn enums_carry_their_data_and_match_covers_every_case() {
        const FORMA: &str = "mod main \"x\"\nenum Forma\n    Circulo(dec)\n    Rect(dec, dec)\n    Nada\n";
        let src = format!("{}fn area(f: Forma) -> dec\n    match f\n        Circulo(r)\n            return 3 * r * r\n        Rect(a, b)\n            return a * b\n        Nada\n            return 0\nfn main()\n    for f in [Circulo(1.5), Rect(2, 2.5), Nada]\n        print(f, \" \", area(f))\n    print(Circulo(2.0) == Circulo(2), \" \", Nada == Circulo(1.0))\n", FORMA);
        assert_eq!(printed(&src), ["Circulo(1.5) 6.75", "Rect(2, 2.5) 5.0", "Nada 0", "true false"]);
        let code = |body: &str| crate::lower(&format!("{}{}", FORMA, body)).unwrap_err().code;
        // A case left out, and a case that is not one.
        assert_eq!(code("fn main()\n    let f = Nada\n    match f\n        Circulo(r)\n            print(r)\n        Nada\n            print(0)\n"), Code::Missing);
        assert_eq!(code("fn main()\n    let f = Nada\n    match f\n        Cuadrado\n            print(0)\n"), Code::Case);
        // Twice, or with the wrong number of names.
        assert_eq!(code("fn main()\n    let f = Nada\n    match f\n        Nada\n            print(0)\n        Nada\n            print(1)\n"), Code::Case);
        assert_eq!(code("fn main()\n    let f = Nada\n    match f\n        Circulo\n            print(0)\n        Rect(a, b)\n            print(a)\n        Nada\n            print(1)\n"), Code::Case);
        // A case with data, bare; a case alone on its line; a value named like a case.
        assert_eq!(code("fn main()\n    let f = Circulo\n    print(f)\n"), Code::Args);
        assert_eq!(code("fn main()\n    Circulo(1.0)\n"), Code::Result);
        assert_eq!(code("fn main()\n    let Nada = 1\n    print(Nada)\n"), Code::Taken);
        // The data of a case, and the value of a match, of their class.
        assert_eq!(code("fn main()\n    print(Circulo(\"dos\"))\n"), Code::WrongType);
        assert_eq!(code("fn main()\n    match 3\n        Circulo(r)\n            print(r)\n        Rect(a, b)\n            print(a)\n        Nada\n            print(0)\n"), Code::WrongType);
        // What an arm names lives in its arm.
        assert_eq!(code("fn main()\n    let f = Circulo(1.0)\n    match f\n        Circulo(r)\n            print(r)\n        Rect(a, b)\n            print(a)\n        Nada\n            print(0)\n    print(r)\n"), Code::Gone);
        // An enum that holds itself would never end.
        assert_eq!(crate::lower("mod main \"x\"\nenum Lista\n    Vacia\n    Uno(int, Lista)\nfn main()\n    print(1)\n").unwrap_err().code, Code::Case);
    }

    #[test]
    fn an_inexact_division_says_the_quotient_and_the_rest() {
        let e = run("mod main \"x\"\nfn main()\n    print(7 / 2)\n").unwrap_err();
        assert!(e.why.contains("da 3 y sobra 1"), "{}", e.why);
    }
}
