//! `gpu` -- WHAT A `gpu fn` MAY BE (level 11, PLAN_EL_CENTAURO G1).
//!
//! A `gpu fn` is written for ONE cell and runs as one thread of the 3060 per
//! cell (D1, FORTRAN's elemental functions). What it may hold is what a
//! thread of the 3060 does well and what the house Programa and the 3060
//! emitter can say -- nothing else (07-10: no SPIR-V in between):
//!
//! ```text
//!    its values    f32 and bool, each a COPY: a thread has its own cell
//!    its result    one f32 or bool, always: a thread that gives nothing back
//!                  is a thread that did nothing
//!    its body      let, if / else and return, with + - * / and comparisons;
//!                  and (LB5) `for i in range(N)` with N WRITTEN, `break`
//!                  and `continue`
//!    NOT           print (the 3060 has no console), tables, texts, records,
//!                  calls, `while` -- each with its own NO, T0090
//! ```
//!
//! The f32 rule of the CPU side (D2) is the calculation's (`calc.rs`, T0091),
//! where the classes are; the permission (U2) is the package's: a `gpu fn`
//! needs the `Titan.toml` to ask for `gpu`, like `use gpu` (T0088).
//!
//! ** THE LOOPS (08-10, LB5 of `docs/plan/PLAN_LAS_LIBRERIAS.md`, with DL4
//! taken as the plan recommends -- the owner's to confirm): only `for i in
//! range(N)` (or `range(A, B)`) with its ends WRITTEN as integers. Then the
//! turns are known when compiling, and a thread ENDS BY CONSTRUCTION, in any
//! cell -- also in the ones typed when running (LB4), which the calculation
//! never sees. So the WORK of a cell is known too ([`OBRA_MAXIMA`]), and too
//! much is T0066 at its `for`: never a compiler, a card or a program that
//! hangs. The counter is f32, like every number inside: exact up to 2^24.
//! A `while` waits for E7 seen on the metal (DL4).
//!
//! [!] Calls between `gpu fn` are not refused for ever: they wait for the GPU
//! writer (`bmo-titan-prometeo`, PROMETEO) to write them IN LINE (LB5), and
//! enter with their own example. Said here so nobody takes the NO for a
//! decision.

use crate::message::{Code, Message};
use crate::tree::{Expr, Function, Program, Stmt, Ty};
use bmo_titan_contrato::{Permission, Permissions};

fn no(line: usize, col: usize, what: &str, why: &str, how: &str) -> Message {
    Message::new(Code::GpuBody, line, col, what, why, how)
}

/// Every `gpu fn` of the program, against the rules above and its permission.
pub fn check(p: &Program, permissions: Permissions) -> Result<(), Message> {
    for f in p.functions.iter().filter(|f| f.gpu) {
        if !permissions.allows(Permission::Gpu) {
            return Err(Message::new(
                Code::NoPermission,
                f.line,
                f.col,
                &format!("`gpu fn {}` y el Titan.toml no pide `gpu`", f.name),
                "una gpu fn corre en la 3060, y lo que un programa usa de BMO-X lo PIDE su manifiesto (U2): el kernel concede, y el certificado dice desde que linea",
                "pidelo en el Titan.toml del paquete:\n             [permissions]\n             gpu = \"compute\"",
            ));
        }
        signature(f)?;
        body(f, &f.body, 0)?;
    }
    Ok(())
}

/// ** THE WORK OF ONE CELL (LB5): every operation written (`+ - * /`, a
/// comparison, `and`, `or`, `not`, the sign) and every `let`, assignment,
/// `return`, `if`, `break` and `continue` is 1; a `for` is its turns times
/// (2 + its body) -- counting and asking -- plus 2. With N written (DL4) it is
/// known before running, for every cell. 65536 is a thread that ends soon
/// on the 3060 and on the CPU, and that each card's simulator runs well
/// inside its budget (4M instructions each: measured, the 3060 needs at
/// most ~4 per unit and the CPU ~12, `pruebas_bucles.rs` of the writer); a
/// gpu fn of the bench does tens.
pub const OBRA_MAXIMA: u64 = 1 << 16;

/// The ends of a `range` inside a gpu fn: the counter is f32, and the f32
/// counts every integer exactly up to 2^24 -- beyond it, adding 1 changes
/// nothing, and the loop would not end.
pub const RANGO_MAXIMO: i64 = 1 << 24;

/// Loops inside loops, at most: the house Programa keeps 32 structures open,
/// and each loop may go inside its `Si`.
pub const HONDO_MAXIMO: usize = 8;

/// **The work of one cell** of a gpu fn that [`check`] accepted (LB5): the
/// same count as its rule, for the writer's battery.
pub fn obra(f: &Function) -> u64 {
    body(f, &f.body, 0).unwrap_or(u64::MAX)
}

/// An integer WRITTEN: `10`, `-3`.
fn written(e: &Expr) -> Option<i64> {
    match e {
        Expr::Int { value, .. } => Some(*value),
        Expr::Neg { value, .. } => match **value {
            Expr::Int { value, .. } => value.checked_neg(),
            _ => None,
        },
        _ => None,
    }
}

/// The NO of a work too big for one thread (T0066, at its `for`).
fn too_much(line: usize, col: usize, obra: u64) -> Message {
    Message::new(
        Code::NoEnd,
        line,
        col,
        &format!("este bucle hace {} operaciones por celda", if obra == u64::MAX { "mas de 2^64".to_string() } else { obra.to_string() }),
        &format!("un hilo de la GPU tiene que acabar pronto: una gpu fn hace como mucho {} por celda -- cada vuelta cuenta lo que se escribe en ella --, y asi acaba en la 3060, en la CPU al correr y en el simulador de cada tarjeta", OBRA_MAXIMA),
        "menos vueltas, o mas celdas: el bucle grande de una gpu fn es la TABLA, un hilo por celda",
    )
}

fn signature(f: &Function) -> Result<(), Message> {
    if f.params.is_empty() {
        return Err(no(f.line, f.col, &format!("`gpu fn {}` no recibe nada", f.name), "una gpu fn trabaja UNA celda: sin valores no hay celda, y cada hilo haria lo mismo", &format!("gpu fn {}(x: f32) -> f32", f.name)));
    }
    for a in &f.params {
        if !matches!(a.ty, Ty::F32 | Ty::Bool) {
            return Err(no(a.line, a.col, &format!("`{}: {}` en una gpu fn", a.name, a.ty.name()), "un hilo de la 3060 recibe UNA celda: un f32 o un bool. Las tablas se le dan al LLAMARLA, y cada hilo toma la suya", &format!("{}: f32  -- y llamala con la tabla: {}(xs)", a.name, f.name)));
        }
        if a.mode.word() != "" {
            return Err(no(a.line, a.col, &format!("`{} {}` en una gpu fn", a.mode.word(), a.name), "cada hilo tiene su COPIA de su celda: prestar o entregar es de la CPU, y el prestamo a la 3060 (U1) llega con el lanzamiento", &format!("{}: f32, y devuelve el resultado", a.name)));
        }
    }
    match &f.ret {
        Some(Ty::F32 | Ty::Bool) => Ok(()),
        Some(t) => Err(no(f.line, f.col, &format!("`gpu fn {}` devuelve un {}", f.name, t.name()), "cada hilo devuelve UNA celda: un f32 o un bool", &format!("gpu fn {}(...) -> f32", f.name))),
        None => Err(no(f.line, f.col, &format!("`gpu fn {}` no devuelve nada", f.name), "un hilo que no devuelve nada no hizo nada: su celda del resultado es todo lo que deja (la 3060 no tiene consola)", &format!("gpu fn {}(...) -> f32, y return ...", f.name))),
    }
}

/// The statements of a gpu fn, against the rules above: their WORK, or the
/// first NO. `hondo`: the loops they are inside.
fn body(f: &Function, stmts: &[Stmt], hondo: usize) -> Result<u64, Message> {
    let mut obra: u64 = 0;
    for st in stmts {
        let (cost, at) = match st {
            Stmt::Let(l) | Stmt::Set(l) => {
                if let Some(t) = &l.ty {
                    if !matches!(t, Ty::F32 | Ty::Bool) {
                        return Err(no(l.line, l.col, &format!("`{}: {}` dentro de una gpu fn", l.name, t.name()), "dentro de una gpu fn solo hay f32 y bool", &format!("let {}: f32 = ...", l.name)));
                    }
                }
                (1 + expr(&l.value)?, None)
            }
            Stmt::If(i) => (1u64.saturating_add(expr(&i.cond)?).saturating_add(body(f, &i.then, hondo)?).saturating_add(body(f, &i.other, hondo)?), None),
            Stmt::Return { value: Some(v), .. } => (1 + expr(v)?, None),
            Stmt::Return { line, col, value: None } => return Err(no(*line, *col, "un `return` sin valor en una gpu fn", "cada hilo devuelve su celda", "return x")),
            Stmt::Call(c) if c.callee == "print" => return Err(no(c.line, c.col, "`print` dentro de una gpu fn", "la 3060 no tiene consola: miles de hilos escribiendo a la vez no dirian nada que se pueda leer", "devuelve el valor, y escribelo en la CPU: print(round(x, 2))")),
            Stmt::Call(c) => return Err(no(c.line, c.col, &format!("`{}()` dentro de una gpu fn", c.callee), "una gpu fn no llama a nada todavia: lo que corre en la 3060 se escribe entero en ella (las llamadas entre gpu fn llegan con el escritor de la 3060, IL1)", "escribe el calculo aqui mismo")),
            Stmt::While(w) => return Err(no(w.line, w.col, "un `while` dentro de una gpu fn", "un hilo de la GPU tiene que acabar, y de un `while` no se sabe al compilar cuantas vueltas da: dentro de una gpu fn solo hay `for i in range(N)` con N escrito, que acaba por construccion (DL4 de PLAN_LAS_LIBRERIAS; el `while` espera a E7 visto en el metal)", "for i in range(10)  -- y `break` para salir antes")),
            Stmt::For(fo) => {
                if fo.over.is_some() {
                    return Err(no(fo.line, fo.col, &format!("`for {} in ...` sobre una tabla dentro de una gpu fn", fo.var), "dentro de una gpu fn no hay tablas: cada hilo tiene UNA celda", "for i in range(10)"));
                }
                let (Some(from), Some(to)) = (written(&fo.from), written(&fo.to)) else {
                    let (l, c) = if written(&fo.from).is_none() { fo.from.at() } else { fo.to.at() };
                    return Err(no(l, c, "un `range` sin sus extremos escritos, en una gpu fn", "las vueltas del bucle de una gpu fn se saben al compilar (DL4): asi acaba por construccion en cualquier celda, tambien en las que llegan al correr", "range(10)  o  range(1, 11): enteros escritos"));
                };
                if from.unsigned_abs() > RANGO_MAXIMO as u64 || to.unsigned_abs() > RANGO_MAXIMO as u64 {
                    let (l, c) = if from.unsigned_abs() > RANGO_MAXIMO as u64 { fo.from.at() } else { fo.to.at() };
                    return Err(no(l, c, &format!("un `range` con un extremo mas alla de {}, en una gpu fn", RANGO_MAXIMO), &format!("dentro de una gpu fn el contador es f32, y el f32 cuenta exacto los enteros hasta 2^24 = {}: mas alla, sumar 1 no cambia nada y el bucle no acabaria", RANGO_MAXIMO), &format!("un rango entre -{} y {}", RANGO_MAXIMO, RANGO_MAXIMO)));
                }
                if hondo >= HONDO_MAXIMO {
                    return Err(no(fo.line, fo.col, &format!("mas de {} bucles uno dentro de otro, en una gpu fn", HONDO_MAXIMO), &format!("el Programa de la casa guarda {} estructuras abiertas, y cada bucle de una gpu fn puede ir dentro de su `si`", 32), "saca el calculo de dentro a otra gpu fn, o junta dos bucles en uno"));
                }
                let inside = body(f, &fo.body, hondo + 1)?;
                let turns = (to as i128 - from as i128).max(0) as u64;
                let cost = turns.saturating_mul(inside.saturating_add(2)).saturating_add(2);
                if cost > OBRA_MAXIMA {
                    return Err(too_much(fo.line, fo.col, cost));
                }
                (cost, Some((fo.line, fo.col)))
            }
            Stmt::Match { line, col, .. } => return Err(no(*line, *col, "un `match` dentro de una gpu fn", "dentro de una gpu fn solo hay f32 y bool: no hay enum que mirar", "decide con if / else")),
            Stmt::SetAt { line, col, .. } => return Err(no(*line, *col, "una parte de un valor cambia dentro de una gpu fn", "dentro de una gpu fn no hay tablas ni registros: cada hilo tiene su celda", "devuelve el valor nuevo")),
            // Inside a `for`: the parser said so (T0067), and a `while` is a NO above.
            Stmt::Break { .. } | Stmt::Continue { .. } => (1, None),
        };
        obra = obra.saturating_add(cost);
        if obra > OBRA_MAXIMA {
            // The statement that went over: its `for`, or the gpu fn.
            let (l, c) = at.unwrap_or((f.line, f.col));
            return Err(too_much(l, c, obra));
        }
    }
    Ok(obra)
}

#[cfg(test)]
mod tests {
    use crate::message::Code;

    const PIDE: &str = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";

    fn lower(main: &str, toml: Option<&str>) -> Result<crate::ir::Module, crate::Message> {
        crate::lower_package("src/main.titan", main, &mut |p| if p == "Titan.toml" { toml.map(String::from) } else { None })
    }

    fn code(body: &str) -> Code {
        lower(&format!("mod main \"x\"\n{}", body), Some(PIDE)).unwrap_err().code
    }

    #[test]
    fn every_rule_of_a_gpu_fn_has_its_no() {
        let ok = "mod main \"x\"\ngpu fn d(x: f32) -> f32\n    return x * 2.0\nfn main()\n    let xs: [f32; 2] = [1.0, 2.5]\n    let r = d(xs)\n    print(round(r[1], 1))\n";
        assert!(lower(ok, Some(PIDE)).is_ok());
        // Without its permission (U2), like `use gpu`.
        assert_eq!(lower(ok, None).unwrap_err().code, Code::NoPermission);
        // A table, a mut value, no result.
        assert_eq!(code("gpu fn d(x: [f32; 2]) -> f32\n    return 1.0\nfn main()\n    print(1)\n"), Code::GpuBody);
        assert_eq!(code("gpu fn d(mut x: f32) -> f32\n    return x\nfn main()\n    print(1)\n"), Code::GpuBody);
        assert_eq!(code("gpu fn d(x: f32)\n    let y = x\nfn main()\n    print(1)\n"), Code::GpuBody);
        // f32 with a dec does not mix, not even inside: the literal is f32 there.
        assert_eq!(code("fn main()\n    let xs: [f32; 1] = [1.0]\n    let y = round(xs[0] * 2.0, 1)\n    print(y)\n"), Code::F32Cpu);
        // Tables of different lengths: no thread for the missing cell.
        assert_eq!(code("gpu fn s(a: f32, b: f32) -> f32\n    return a + b\nfn main()\n    let a: [f32; 2] = [1.0, 2.0]\n    let b: [f32; 3] = [1.0, 2.0, 3.0]\n    let c = s(a, b)\n    print(round(c[0], 1))\n"), Code::WrongType);
        // D4: nothing becomes f32 without a declared type.
        assert_eq!(code("gpu fn d(x: f32) -> f32\n    return x\nfn main()\n    let r = d(1.5)\n    print(round(r, 1))\n"), Code::WrongType);
        // An f32 is not printed on the CPU.
        assert_eq!(code("fn main()\n    let xs: [f32; 1] = [1.0]\n    print(xs)\n"), Code::F32Cpu);
    }

    fn no(body: &str) -> crate::Message {
        lower(&format!("mod main \"x\"\n{}", body), Some(PIDE)).unwrap_err()
    }

    /// ** LB5, DL4: `for i in range(N)` with N WRITTEN, with `break`,
    /// `continue` and `return` inside, enters; and each way out of "it ends by
    /// construction, soon" has its NO, where it is written.
    #[test]
    fn a_gpu_fn_loops_only_where_its_turns_are_written() {
        let ok = "mod main \"x\"\ngpu fn f(x: f32) -> f32\n    let mut r = 0.0\n    for i in range(-3, 3)\n        if x > i\n            continue\n        if r > 10.0\n            break\n        if r < -10.0\n            return r\n        r = r + x * i\n    return r\nfn main()\n    print(1)\n";
        let m = lower(ok, Some(PIDE)).unwrap_or_else(|e| panic!("{:?}", e));
        // The work of a cell: 1 (let) + 6 turns * (2 + 3 + 3 + 4 + 3) + 2 (the
        // for) + 1 (return) -- the same in every cell. The third `if` is 4:
        // the `if`, `<`, the sign of `-10.0` and its `return`.
        assert_eq!(m.functions.iter().find(|f| f.name == "f").unwrap().obra, 1 + 6 * (2 + 3 + 3 + 4 + 3) + 2 + 1);
        // A `while`: its end is not known when compiling (DL4).
        let w = no("gpu fn f(x: f32) -> f32\n    let mut r = x\n    while r > 1.0\n        r = r * 0.5\n    return r\nfn main()\n    print(1)\n");
        assert_eq!((w.code, w.line, w.col), (Code::GpuBody, 4, 5), "{:?}", w);
        assert!(w.why.contains("DL4") && w.how.contains("range"), "{}", w.why);
        // N not written: a value, not a number in the text.
        let n = no("gpu fn f(x: f32) -> f32\n    let mut r = x\n    let n = 3\n    for i in range(n)\n        r = r * x\n    return r\nfn main()\n    print(1)\n");
        assert_eq!((n.code, n.line, n.col), (Code::GpuBody, 5, 20), "{:?}", n);
        // Beyond 2^24 the f32 counter would not move.
        let big = no("gpu fn f(x: f32) -> f32\n    let mut r = x\n    for i in range(16777216, 16777218)\n        r = r * x\n    return r\nfn main()\n    print(1)\n");
        assert_eq!((big.code, big.line, big.col), (Code::GpuBody, 4, 30), "{:?}", big);
        assert!(lower("mod main \"x\"\ngpu fn f(x: f32) -> f32\n    let mut r = x\n    for i in range(16777214, 16777216)\n        r = r * x\n    return r\nfn main()\n    print(1)\n", Some(PIDE)).is_ok(), "2^24 itself is exact");
        // Nine loops deep.
        let mut deep = String::from("gpu fn f(x: f32) -> f32\n    let mut r = x\n");
        for k in 0..9 {
            deep.push_str(&format!("{}for i{} in range(1)\n", "    ".repeat(k + 1), k));
        }
        deep.push_str(&format!("{}r = r * x\n    return r\nfn main()\n    print(1)\n", "    ".repeat(10)));
        let d = no(&deep);
        assert_eq!((d.code, d.line), (Code::GpuBody, 4 + 8), "{:?}", d);
        // ** T0066: too much work for one thread, at its `for` -- never a
        // compiler, a card or a program that hangs.
        let much = no("gpu fn f(x: f32) -> f32\n    let mut r = x\n    for i in range(100000)\n        r = r * x\n    return r\nfn main()\n    print(1)\n");
        assert_eq!((much.code, much.line, much.col), (Code::NoEnd, 4, 5), "{:?}", much);
        // 100000 turns of (2 + `r = r * x`, 2), and the `for` itself.
        assert!(much.what.contains("400002") && much.why.contains("65536"), "{:?}", much);
        // Two loops, each under the top, together over it: the second one.
        let two = no("gpu fn f(x: f32) -> f32\n    let mut r = x\n    for i in range(10000)\n        r = r * x\n    for j in range(10000)\n        r = r + x\n    return r\nfn main()\n    print(1)\n");
        assert_eq!((two.code, two.line, two.col), (Code::NoEnd, 6, 5), "{:?}", two);
        // Inside each other: the turns multiply. The inner one alone fits
        // (4002); the outer one, a thousand of them, does not.
        let nest = no("gpu fn f(x: f32) -> f32\n    let mut r = x\n    for i in range(1000)\n        for j in range(1000)\n            r = r + x\n    return r\nfn main()\n    print(1)\n");
        assert_eq!((nest.code, nest.line, nest.col), (Code::NoEnd, 4, 5), "{:?}", nest);
        // A huge N does not overflow the count.
        let huge = no("gpu fn f(x: f32) -> f32\n    let mut r = x\n    for i in range(-16777216, 16777216)\n        for j in range(-16777216, 16777216)\n            for k in range(-16777216, 16777216)\n                for l in range(-16777216, 16777216)\n                    r = r + x\n    return r\nfn main()\n    print(1)\n");
        assert_eq!(huge.code, Code::NoEnd, "{:?}", huge);
    }
}

/// An expression of a gpu fn: its operations (its WORK), or its NO.
fn expr(e: &Expr) -> Result<u64, Message> {
    match e {
        Expr::Int { .. } | Expr::Dec { .. } | Expr::Name { .. } | Expr::Bool { .. } => Ok(0),
        Expr::Bin { left, right, .. } => Ok(1u64.saturating_add(expr(left)?).saturating_add(expr(right)?)),
        Expr::Neg { value, .. } | Expr::Not { value, .. } => Ok(1u64.saturating_add(expr(value)?)),
        Expr::Text { line, col, .. } => Err(no(*line, *col, "un texto dentro de una gpu fn", "la 3060 cuenta numeros: un texto no tiene celda en ella", "deja los textos a la CPU")),
        Expr::Call { callee, line, col, .. } => Err(no(*line, *col, &format!("`{}()` dentro de una gpu fn", callee), "una gpu fn no llama a nada todavia (las llamadas entre gpu fn llegan con G2)", "escribe el calculo aqui mismo")),
        Expr::Round { line, col, .. } => Err(no(*line, *col, "`round` dentro de una gpu fn", "round vuelve un f32 `dec`, y el dec es de la CPU: se redondea al volver", "devuelve el f32 y redondealo en la CPU: round(x, 2)")),
        Expr::Table { line, col, .. } | Expr::Repeat { line, col, .. } | Expr::Index { line, col, .. } => Err(no(*line, *col, "una tabla dentro de una gpu fn", "cada hilo tiene UNA celda: la tabla se le da al llamarla", "gpu fn f(x: f32) -> f32, y f(tabla)")),
        Expr::Field { line, col, .. } | Expr::Record { line, col, .. } => Err(no(*line, *col, "un registro dentro de una gpu fn", "dentro de una gpu fn solo hay f32 y bool", "pasa cada campo como su propio valor")),
        Expr::Lend { line, col, .. } => Err(no(*line, *col, "prestar dentro de una gpu fn", "una gpu fn no llama a nada", "escribe el calculo aqui mismo")),
        Expr::Map { line, col, .. } => Err(no(*line, *col, "un mapa dentro de una gpu fn", "cada hilo tiene UNA celda: un mapa vive en la CPU", "busca en el mapa en la CPU y pasa el valor")),
    }
}
