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

use crate::ir::{At, End, Function, Lib, Module, Op, PathStep, Value};
use crate::message::{Code, Message};
use crate::tree::{show_dec, EnumDef, TraitDef, Ty, TypeDef};

// The two halves cut out on 05-10 (L6a): what each value IS, and the
// arithmetic. They read everything here (`use super::*`); this reads them.
mod clase;
mod coleccion;
mod numero;
pub use clase::class;
use clase::*;
use coleccion::*;
use numero::*;

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
    /// A list (level 13): the type of what it holds -- so `push(mut l, 2)`
    /// into a `[dec]` keeps a decimal -- and its cells.
    List(Ty, Vec<Const>),
    /// A map (level 13): the types of its keys and values, and its entries
    /// in the order they went in (D4).
    Map(Ty, Ty, Vec<(Const, Const)>),
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
            Const::Table(items) | Const::List(_, items) => format!("[{}]", items.iter().map(|i| i.show_in(types, true)).collect::<Vec<_>>().join(", ")),
            Const::Map(_, _, items) => format!("{{{}}}", items.iter().map(|(k, v)| format!("{}: {}", k.show_in(types, true), v.show_in(types, true))).collect::<Vec<_>>().join(", ")),
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
    /// `[T]`, a list (level 13).
    List(Box<Class>),
    /// `{K: V}`, a map (level 13).
    Map(Box<Class>, Box<Class>),
    /// `Opcion[T]`: `Hay(v)` or `NoHay`, what `get` and `pop` give (13).
    Opt(Box<Class>),
    /// What `[]` and `{}` hold: nothing yet, so ANY class -- the type said
    /// where they go decides (`let l: [int] = []`). Never the class of a
    /// local: a list with nothing that says what it is, is a NO.
    Any,
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
            Class::List(c) => format!("una lista [{}]", c.short(types)),
            Class::Map(k, v) => format!("un mapa {{{}: {}}}", k.short(types), v.short(types)),
            Class::Opt(c) => format!("un caso Opcion[{}] (Hay o NoHay)", c.short(types)),
            Class::Any => "algo sin decir".into(),
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
            Class::List(c) => format!("[{}]", c.short(types)),
            Class::Map(k, v) => format!("{{{}: {}}}", k.short(types), v.short(types)),
            Class::Opt(c) => format!("Opcion[{}]", c.short(types)),
            Class::Any => "?".into(),
        }
    }

    fn number(&self) -> bool {
        matches!(self, Class::Int | Class::Dec)
    }
}

/// The class a type of the text says.
pub fn of_ty(t: &Ty, types: Defs) -> Class {
    match t {
        Ty::Int => Class::Int,
        Ty::Text => Class::Text,
        Ty::Bool => Class::Bool,
        Ty::Dec | Ty::DecP(..) => Class::Dec,
        Ty::F32 => Class::F32,
        Ty::Table(inner, n) => Class::Table(Box::new(of_ty(inner, types)), *n),
        Ty::List(inner) => Class::List(Box::new(of_ty(inner, types))),
        Ty::Map(k, v) => Class::Map(Box::new(of_ty(k, types)), Box::new(of_ty(v, types))),
        Ty::Opt(inner) => Class::Opt(Box::new(of_ty(inner, types))),
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
    want == got || (*want == Class::Dec && *got == Class::Int) || fits_collection(want, got)
}

/// T0071: a value of the wrong class where something says one.
fn wrong(at: At, want: &Class, got: &Class, types: Defs, what: &str, how: &str) -> Message {
    Message::new(Code::WrongType, at.0, at.1, &format!("aqui va {}, y llega {}", want.name(types), got.name(types)), what, how)
}

/// ** WHO RUNS A `gpu fn` (level 11, G3 of PLAN_EL_CENTAURO), said without
/// naming a machine or a format: given the cells of each value -- an f32 by
/// its bits, a bool as 0 or 1 --, the cells of the result. The emitter's
/// side gives one that writes the gpu fn as SM86 (the house Programa, the
/// 3060 emitter and its judge) and runs it in the 3060 SIMULATOR; without one, the calculation runs each thread itself, with f32 of
/// single precision. The two benches compare the same `# sale:` lines, so
/// the day they disagreed, one of them would say it.
pub trait Device: Send {
    fn run(&mut self, m: &Module, func: usize, cells: Vec<Vec<u32>>) -> Result<Vec<u32>, DeviceNo>;
}

/// ** WHY A `Device` DID NOT RUN A `gpu fn` -- and the two are not the same
/// thing (LB1 of `docs/plan/PLAN_LAS_LIBRERIAS.md`, 08-10):
///
/// ```text
///    Limit     what the device's library does NOT KNOW YET, and says so on
///              purpose: a documented limit (today, a general division on the
///              3060: LI2g). That is the PROGRAM's NO, said where it is
///              written -- its line and column are the module's, like `at`
///    Failure   the writer, its judge or the oracle went wrong: never the
///              program's, and it says so
/// ```
///
/// Before this, both came out as "a failure of the writer ... tell us with
/// this program", at the line of the CALL: a known limit read as a bug.
///
/// 09-10 (DL10): the 3060 divides EXACTLY now, and no card of `titan` has a
/// limit; the road stays for the one that will (the toy card of the
/// writer's tests says its own).
#[derive(Debug)]
pub enum DeviceNo {
    Limit(Message),
    Failure(String),
}

impl std::fmt::Display for DeviceNo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceNo::Limit(m) => write!(f, "linea {}, columna {}: {} -- {}", m.line, m.col, m.what, m.why),
            DeviceNo::Failure(why) => f.write_str(why),
        }
    }
}

/// The module RUN: its classes judged in every block, then the program run
/// from `main`. `Module::flat` is what it writes; the blocks no run entered
/// are `dead`.
pub fn fold(m: &Module) -> Result<Module, Message> {
    fold_with(m, None)
}

/// `fold`, with who runs the `gpu fn` (level 11).
pub fn fold_with(m: &Module, device: Option<&mut dyn Device>) -> Result<Module, Message> {
    let mut out = unfolded(m)?;
    // ** E1 (`docs/plan/PLAN_LA_ENTRADA.md`): a program that reads from
    // outside is not RUN here -- what will be typed is not known yet. Its
    // classes are judged (above) and so are its loans (`juez.rs`, before);
    // the rest is the emitter's: every block stays, `flat` stays empty.
    if m.reads_outside() {
        return Ok(out);
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

/// The module with its classes judged and NOTHING run: every block alive,
/// `flat` empty -- what E1 emits. `fold_with` runs from here; the oracle of
/// E1 (`emisor-x86_64/tests/e1.rs`) starts here too, so that a NO the
/// calculation finds by running is found again by the machine.
pub fn unfolded(m: &Module) -> Result<Module, Message> {
    // A trait's fn has no body: each type's fn is judged as its own.
    for f in m.functions.iter().filter(|f| f.dispatch.is_none()) {
        classes(f, m)?;
    }
    Ok(m.clone())
}

/// ** A `gpu fn` run by the CALCULATION on given cells (level 11): an f32 by
/// its bits, a bool as 0 or 1 -- the same cells a `Device` gets. It is the
/// reference the oracle (the simulated 3060) is measured against at every build, cell by
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

/// ** A `gpu fn` that DRAWS (LB6), run by the CALCULATION on its cells: each
/// cell is its ELEMENTS -- one per field of what it receives, its f32 by
/// their bits in up to four components --, and gives back the elements of
/// what it leaves (the one of vertex: one per field; the one of pixel: its
/// colour, one). The reference its cards are measured against.
pub fn run_gpu_dibujo(m: &Module, func: usize, celdas: &[Vec<[u32; 4]>]) -> Result<Vec<Vec<[u32; 4]>>, Message> {
    let f = &m.functions[func];
    let ty_of = |name: &str| m.types.iter().position(|t| t.name == name);
    // A value of a field from its element: an f32, or a record of f32.
    let field = |ty: &Ty, e: &[u32; 4]| -> Const {
        match ty {
            Ty::Named(n) => {
                let k = ty_of(n).expect("gpu: the type exists");
                Const::Record(k, (0..m.types[k].fields.len()).map(|c| Const::F32(e[c])).collect())
            }
            _ => Const::F32(e[0]),
        }
    };
    let (Some((_, Ty::Named(entra))), Some(Ty::Named(sale))) = (f.params.first(), f.ret.as_ref()) else {
        return Err(Message::new(Code::GpuBody, f.line, 1, "una gpu fn que dibuja sin sus registros", "gpu.rs tenia que haberlo dicho", "--"));
    };
    let entra = ty_of(entra).expect("gpu: the type exists");
    let mut r = Run { m, steps: 0, depth: 0, flat: Vec::new(), seen: m.functions.iter().map(|f| vec![false; f.blocks.len()]).collect(), last_turn: (0, 0), lenient: 0, device: None };
    let mut out = Vec::with_capacity(celdas.len());
    for e in celdas {
        let fields = m.types[entra].fields.iter().enumerate().map(|(k, c)| field(&c.ty, &e[k])).collect();
        let (result, _) = r.call(func, vec![Const::Record(entra, fields)], (f.line, 1))?;
        // What it leaves, by elements: a record of records (vertex), or one
        // record of f32 (pixel: the colour).
        let bits = |c: &Const| if let Const::F32(b) = c { *b } else { 0 };
        let elemento = |c: &Const| -> [u32; 4] {
            let mut x = [0u32; 4];
            match c {
                Const::Record(_, items) => items.iter().take(4).enumerate().for_each(|(i, v)| x[i] = bits(v)),
                v => x[0] = bits(v),
            }
            x
        };
        let sale_ty = ty_of(sale).expect("gpu: the type exists");
        let es_color = m.types[sale_ty].fields.iter().all(|c| c.ty == Ty::F32);
        out.push(match result {
            Some(c @ Const::Record(..)) if es_color => vec![elemento(&c)],
            Some(Const::Record(_, items)) => items.iter().map(elemento).collect(),
            _ => Vec::new(),
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
                            // `l = [1, 2]` into a list: of the list's own type (13).
                            (Some(old), v) if decl[*local].is_none() => like(old, v, m.defs(), *at)?,
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
        let out = device.run(m, func, cells).map_err(|no| match no {
            // A limit the library says on purpose: the program's NO, where it
            // is written (the caller locates it in its file, like any other).
            DeviceNo::Limit(said) => said,
            // A failure names no card (L05): it may be any of them (LB3).
            DeviceNo::Failure(why) => Message::new(
                Code::GpuBody,
                at.0,
                at.1,
                &format!("la GPU no pudo correr `{}`", m.functions[func].name),
                &format!("es un fallo de su tarjeta -- su emisor, su juez o su simulador -- o del oraculo, no del programa: {}", why),
                "avisa con este programa",
            ),
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
            Value::Number(inner, e, _) => match self.ev(inner, known)? {
                Const::Text(t) => match crate::prelude::parse(&t) {
                    Some(n) => Const::Variant(*e, 0, vec![Const::Int(n)]),
                    None => Const::Variant(*e, 1, Vec::new()),
                },
                _ => unreachable!("classes: numero reads a text"),
            },
            Value::Read(_) => unreachable!("calc: a module that reads is emitted, never run when compiling (fold_with)"),
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
                Const::Table(items) | Const::List(_, items) => Const::Int(items.len() as i64),
                Const::Map(_, _, items) => Const::Int(items.len() as i64),
                _ => unreachable!("classes: only a table, a list or a map has cells"),
            },
            Value::Lib(lib, args, at) => self.lib(*lib, args, *at, known)?,
            Value::Map(items, at) => {
                let mut vals = Vec::with_capacity(items.len());
                for (k, v) in items {
                    vals.push((self.ev(k, known)?, self.ev(v, known)?));
                }
                map_written(vals, types, *at)?
            }
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
        (c, Some(t @ (Ty::List(_) | Ty::Map(..) | Ty::Opt(_)))) => fit_collection(c, t, types, at),
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
    let (Const::Table(items) | Const::List(_, items), Const::Int(i)) = (table, idx) else { unreachable!("classes: a table and an int") };
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
        // A cell of a list (13): what goes in takes the list's own type.
        (Const::List(t, items), Err((Const::Int(i), iat))) => {
            if *i < 0 || *i as usize >= items.len() {
                return Err(outside(*i, items.len(), *iat));
            }
            let v = if rest.is_empty() { fit_into(v, Some(&*t), types, at)? } else { v };
            set_in(&mut items[*i as usize], rest, v, types, at)
        }
        (Const::Record(t, items), Ok(name)) => {
            let k = types[*t].fields.iter().position(|f| f.name == *name).expect("classes: the field exists");
            set_in(&mut items[k], rest, v, types, at)
        }
        _ => unreachable!("classes: every step fits its value"),
    }
}

#[cfg(test)]
mod pruebas;

/// The test the law L06 names (`toolchain/tools/titan-leyes/LEYES.txt`): it
/// stays HERE, where the law looks for it.
#[cfg(test)]
mod tests {
    use super::pruebas::{printed, run};
    use super::*;

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
}
