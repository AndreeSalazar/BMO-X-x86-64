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
//!                  and calls to another `gpu fn`, written in line
//!    NOT           print (the 3060 has no console), tables, texts, records,
//!                  calls to the CPU, `while` -- each with its own NO, T0090
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
//! ** THE CALLS (08-10, LB5): a `gpu fn` may call another `gpu fn` -- never
//! a fn of the CPU, and never itself, directly or through another -- and the
//! writer (`bmo-titan-prometeo`) writes it IN LINE: a thread has no stack to
//! call with, and a gpu fn is pure, so in line it gives the same bits. Its
//! work is the callee's, counted where it is called.

use crate::message::{Code, Message};
use crate::tree::{Expr, Function, Program, Step, Stmt, Ty, TypeDef};
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
        let forma = signature(p, f)?;
        sin_ciclo(p, f, &mut vec![f.name.as_str()], &mut Vec::new())?;
        body(p, f, &f.body, 0, forma)?;
    }
    Ok(())
}

/// ** LA FORMA DE UNA gpu fn, por su FIRMA (LB6; DL6 tomada como recomienda el
/// plan: los tipos no gastan techo -- del propietario confirmarla).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forma {
    /// Una CELDA (nivel 11): valores f32 o bool, un resultado f32 o bool.
    Celda,
    /// La de VERTICE: recibe UN registro -- cada campo, un atributo del
    /// vertice -- y devuelve otro con su `posicion` (4 f32, donde cae en la
    /// pantalla: SV_Position) en el campo `posicion`.
    Vertice { posicion: usize },
    /// La de PIXEL: recibe lo que deja una de vertice (un registro con su
    /// `posicion`, que es de la 3060 y no se lee) y devuelve su COLOR, un
    /// registro de 4 f32.
    Pixel { posicion: usize },
}

/// El campo de lo que deja una gpu fn de vertice que dice donde cae.
pub const POSICION: &str = "posicion";

/// Los componentes de un campo de dibujo: 1 si es un f32; n si es un
/// registro de n f32 (de 1 a 4); `None` si es otra cosa.
pub fn componentes(tipos: &[TypeDef], ty: &Ty) -> Option<usize> {
    match ty {
        Ty::F32 => Some(1),
        Ty::Named(n) => {
            let t = tipos.iter().find(|t| &t.name == n)?;
            (!t.fields.is_empty() && t.fields.len() <= 4 && t.fields.iter().all(|c| c.ty == Ty::F32)).then_some(t.fields.len())
        }
        _ => None,
    }
}

/// Los campos de un registro de dibujo -- su nombre y sus componentes, en
/// orden: cada uno un ELEMENTO de la tuberia --, o `None` si no lo es.
pub fn campos(tipos: &[TypeDef], ty: &Ty) -> Option<Vec<(String, usize)>> {
    let Ty::Named(n) = ty else { return None };
    let t = tipos.iter().find(|t| &t.name == n)?;
    if t.fields.is_empty() || t.fields.len() > 8 {
        return None;
    }
    t.fields.iter().map(|c| componentes(tipos, &c.ty).map(|k| (c.name.clone(), k))).collect()
}

/// **La forma de una gpu fn, por su firma** (`params`: los tipos de sus
/// valores; `ret`: el de su resultado). Lo que no es de vertice ni de pixel
/// es una celda, y la celda dice sus propios NO.
pub fn forma(tipos: &[TypeDef], params: &[&Ty], ret: Option<&Ty>) -> Forma {
    let ([a], Some(b)) = (params, ret) else { return Forma::Celda };
    let (Some(entra), Some(sale)) = (campos(tipos, a), campos(tipos, b)) else { return Forma::Celda };
    if let Some(p) = sale.iter().position(|(n, k)| n == POSICION && *k == 4) {
        return Forma::Vertice { posicion: p };
    }
    match (entra.iter().position(|(n, k)| n == POSICION && *k == 4), componentes(tipos, b)) {
        (Some(p), Some(4)) => Forma::Pixel { posicion: p },
        _ => Forma::Celda,
    }
}

/// La forma de la gpu fn `f` del programa.
fn forma_de(p: &Program, f: &Function) -> Forma {
    let params: Vec<&Ty> = f.params.iter().map(|a| &a.ty).collect();
    forma(&p.types, &params, f.ret.as_ref())
}

/// ** LB7b: the director is the CPU's -- a thread of the 3060 counts one
/// cell; it does not publish frames.
fn no_director(callee: &str, line: usize, col: usize) -> Message {
    no(line, col, &format!("`{}()` dentro de una gpu fn", callee), "el director -- la lamina de VERRANO -- es de la CPU: un hilo de la 3060 cuenta una celda, no publica fotogramas", "cuenta los vertices en la gpu fn y publicalos desde la CPU: director.publica(f, n, posiciones, colores)")
}

/// The `gpu fn` called `name`, if there is one.
fn gpu_fn<'p>(p: &'p Program, name: &str) -> Option<&'p Function> {
    p.functions.iter().find(|g| g.gpu && g.name == name)
}

/// The calls written in these statements: (who, line, column).
fn llamadas(stmts: &[Stmt]) -> Vec<(&str, usize, usize)> {
    fn en<'a>(e: &'a Expr, out: &mut Vec<(&'a str, usize, usize)>) {
        match e {
            Expr::Call { callee, args, line, col } => {
                out.push((callee.as_str(), *line, *col));
                args.iter().for_each(|a| en(a, out));
            }
            Expr::Bin { left, right, .. } => {
                en(left, out);
                en(right, out);
            }
            Expr::Neg { value, .. } | Expr::Not { value, .. } => en(value, out),
            _ => {}
        }
    }
    fn todas<'a>(stmts: &'a [Stmt], out: &mut Vec<(&'a str, usize, usize)>) {
        for st in stmts {
            match st {
                Stmt::Let(l) | Stmt::Set(l) => en(&l.value, out),
                Stmt::Return { value: Some(v), .. } => en(v, out),
                Stmt::If(i) => {
                    en(&i.cond, out);
                    todas(&i.then, out);
                    todas(&i.other, out);
                }
                Stmt::For(fo) => todas(&fo.body, out),
                Stmt::While(w) => {
                    en(&w.cond, out);
                    todas(&w.body, out);
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    todas(stmts, &mut out);
    out
}

/// ** A gpu fn that calls itself, directly or through another, would never
/// end being written IN LINE (and a thread has no stack to call with): its
/// NO, at the call that closes the circle. `pila`: the path from the gpu fn
/// being judged; `vistas`: the ones already known to close no circle.
fn sin_ciclo<'p>(p: &'p Program, f: &'p Function, pila: &mut Vec<&'p str>, vistas: &mut Vec<&'p str>) -> Result<(), Message> {
    for (name, line, col) in llamadas(&f.body) {
        let Some(g) = gpu_fn(p, name) else { continue };
        if pila.contains(&g.name.as_str()) {
            let mut camino: Vec<&str> = pila.iter().skip_while(|n| **n != g.name).copied().collect();
            camino.push(&g.name);
            return Err(no(
                line,
                col,
                &format!("una gpu fn que se llama a si misma: {}", camino.join(" -> ")),
                "una gpu fn se escribe EN LINEA dentro de quien la llama, y una que se llama a si misma no acabaria nunca de escribirse: un hilo de la GPU no tiene pila para llamadas",
                "escribelo con un `for` de vueltas escritas",
            ));
        }
        if vistas.contains(&g.name.as_str()) {
            continue;
        }
        pila.push(&g.name);
        sin_ciclo(p, g, pila, vistas)?;
        pila.pop();
        vistas.push(&g.name);
    }
    Ok(())
}

/// ** THE WORK OF ONE CELL (LB5): every operation written (`+ - * /`, a
/// comparison, `and`, `or`, `not`, the sign) and every `let`, assignment,
/// `return`, `if`, `break` and `continue` is 1 -- 09-10: but a general
/// division, [`OBRA_DIVISION`] --; a `for` is its turns times
/// (2 + its body) -- counting and asking -- plus 2. With N written (DL4) it is
/// known before running, for every cell. 65536 is a thread that ends soon
/// on the 3060 and on the CPU, and that each card's simulator runs well
/// inside its budget (4M instructions each: measured, the 3060 needs at
/// most ~4 per unit and the CPU ~12, `pruebas_bucles.rs` of the writer); a
/// gpu fn of the bench does tens.
pub const OBRA_MAXIMA: u64 = 1 << 16;

/// ** THE WORK OF A GENERAL DIVISION (09-10, DL10 of
/// `docs/plan/PLAN_LAS_LIBRERIAS.md`): 24, not 1. On the 3060 the EXACT
/// division is a whole calculation -- from ~30 instructions to ~90 when an
/// operand or the quotient leaves its window (`cociente.rs` of its emitter)
/// --: counted as 1, a cell of 65536 would overrun its simulator. By a
/// WRITTEN power of two (`/ 2.0`, `/ 0.25`) it is a multiplication
/// ([`inverso_exacto`]), and it is 1.
pub const OBRA_DIVISION: u64 = 24;

/// **The EXACT inverse of `b`** (its bits), if `b` is a power of two (with
/// its sign) whose inverse is an f32 too: then `x / b` and `x * inverse` are
/// the same real number and round the same, for every `x`. The writer makes
/// such a division a multiplication; the work counts it as one.
pub fn inverso_exacto(b: u32) -> Option<u32> {
    let v = f32::from_bits(b);
    if !v.is_normal() || b & 0x007F_FFFF != 0 {
        return None;
    }
    let r = 1.0f32 / v;
    (r.is_normal() && (r * v) == 1.0).then(|| r.to_bits())
}

/// A divisor WRITTEN as a number (`2.0`, `4`) with an exact inverse: the
/// same f32 the IR gives it (`ir::to_f32`, rounded once).
fn por_su_inverso(e: &Expr) -> bool {
    let v = match e {
        Expr::Int { value, .. } => *value as f32,
        Expr::Dec { digits, scale, .. } => crate::tree::show_dec(*digits, *scale).parse::<f32>().unwrap_or(f32::NAN),
        _ => return false,
    };
    inverso_exacto(v.to_bits()).is_some()
}

/// The ends of a `range` inside a gpu fn: the counter is f32, and the f32
/// counts every integer exactly up to 2^24 -- beyond it, adding 1 changes
/// nothing, and the loop would not end.
pub const RANGO_MAXIMO: i64 = 1 << 24;

/// Loops inside loops, at most: the house Programa keeps 32 structures open,
/// and each loop may go inside its `Si`.
pub const HONDO_MAXIMO: usize = 8;

/// **The work of one cell** of a gpu fn that [`check`] accepted (LB5): the
/// same count as its rule, for the writer's battery.
pub fn obra(p: &Program, f: &Function) -> u64 {
    body(p, f, &f.body, 0, forma_de(p, f)).unwrap_or(u64::MAX)
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

fn signature(p: &Program, f: &Function) -> Result<Forma, Message> {
    let forma = forma_de(p, f);
    if forma != Forma::Celda {
        // Una que DIBUJA (LB6): su registro llega como una COPIA, como una celda.
        let a = &f.params[0];
        if a.mode.word() != "" {
            return Err(no(a.line, a.col, &format!("`{} {}` en una gpu fn", a.mode.word(), a.name), "cada hilo tiene su COPIA de lo que le llega: prestar o entregar es de la CPU", &format!("{}: {}, y devuelve el registro nuevo", a.name, a.ty.name())));
        }
        return Ok(forma);
    }
    if f.params.is_empty() {
        return Err(no(f.line, f.col, &format!("`gpu fn {}` no recibe nada", f.name), "una gpu fn trabaja UNA celda: sin valores no hay celda, y cada hilo haria lo mismo", &format!("gpu fn {}(x: f32) -> f32", f.name)));
    }
    for a in &f.params {
        if let Ty::Named(t) = &a.ty {
            return Err(no(a.line, a.col, &format!("`{}: {}` en una gpu fn", a.name, t), "un hilo de una celda recibe f32 o bool. Para DIBUJAR (LB6) una gpu fn recibe UN registro y devuelve otro: la de vertice devuelve uno con su `posicion` (un registro de 4 f32); la de pixel recibe ese y devuelve su color (un registro de 4 f32). Cada campo es un f32 o un registro de 1 a 4 f32", &format!("gpu fn vertice(v: {}) -> {}  -- con `posicion` en lo que devuelve", t, t)));
        }
        if !matches!(a.ty, Ty::F32 | Ty::Bool) {
            return Err(no(a.line, a.col, &format!("`{}: {}` en una gpu fn", a.name, a.ty.name()), "un hilo de la 3060 recibe UNA celda: un f32 o un bool. Las tablas se le dan al LLAMARLA, y cada hilo toma la suya", &format!("{}: f32  -- y llamala con la tabla: {}(xs)", a.name, f.name)));
        }
        if a.mode.word() != "" {
            return Err(no(a.line, a.col, &format!("`{} {}` en una gpu fn", a.mode.word(), a.name), "cada hilo tiene su COPIA de su celda: prestar o entregar es de la CPU, y el prestamo a la 3060 (U1) llega con el lanzamiento", &format!("{}: f32, y devuelve el resultado", a.name)));
        }
    }
    match &f.ret {
        Some(Ty::F32 | Ty::Bool) => Ok(Forma::Celda),
        Some(t) => Err(no(f.line, f.col, &format!("`gpu fn {}` devuelve un {}", f.name, t.name()), "cada hilo devuelve UNA celda: un f32 o un bool", &format!("gpu fn {}(...) -> f32", f.name))),
        None => Err(no(f.line, f.col, &format!("`gpu fn {}` no devuelve nada", f.name), "un hilo que no devuelve nada no hizo nada: su celda del resultado es todo lo que deja (la 3060 no tiene consola)", &format!("gpu fn {}(...) -> f32, y return ...", f.name))),
    }
}

/// The statements of a gpu fn, against the rules above: their WORK, or the
/// first NO. `hondo`: the loops they are inside.
fn body(p: &Program, f: &Function, stmts: &[Stmt], hondo: usize, forma: Forma) -> Result<u64, Message> {
    let mut obra: u64 = 0;
    for st in stmts {
        let (cost, at) = match st {
            Stmt::Let(l) | Stmt::Set(l) => {
                if let Some(t) = &l.ty {
                    // Una que dibuja (LB6) guarda tambien sus registros de f32.
                    let de_dibujo = forma != Forma::Celda && (componentes(&p.types, t).is_some() || campos(&p.types, t).is_some());
                    if !matches!(t, Ty::F32 | Ty::Bool) && !de_dibujo {
                        return Err(no(l.line, l.col, &format!("`{}: {}` dentro de una gpu fn", l.name, t.name()), "dentro de una gpu fn solo hay f32 y bool", &format!("let {}: f32 = ...", l.name)));
                    }
                }
                (1u64.saturating_add(expr(p, &l.value, hondo, forma)?), None)
            }
            Stmt::If(i) => (1u64.saturating_add(expr(p, &i.cond, hondo, forma)?).saturating_add(body(p, f, &i.then, hondo, forma)?).saturating_add(body(p, f, &i.other, hondo, forma)?), None),
            Stmt::Return { value: Some(v), .. } => (1u64.saturating_add(expr(p, v, hondo, forma)?), None),
            Stmt::Return { line, col, value: None } => return Err(no(*line, *col, "un `return` sin valor en una gpu fn", "cada hilo devuelve su celda", "return x")),
            Stmt::Call(c) if c.callee.starts_with("director.") => return Err(no_director(&c.callee, c.line, c.col)),
            Stmt::Call(c) if c.callee == "print" => return Err(no(c.line, c.col, "`print` dentro de una gpu fn", "la 3060 no tiene consola: miles de hilos escribiendo a la vez no dirian nada que se pueda leer", "devuelve el valor, y escribelo en la CPU: print(round(x, 2))")),
            Stmt::Call(c) if gpu_fn(p, &c.callee).is_some() => return Err(no(c.line, c.col, &format!("`{}()` sola, sin usar lo que devuelve, en una gpu fn", c.callee), "una gpu fn es pura: no cambia nada fuera de ella, asi que llamarla sin usar su resultado no hace nada", &format!("let y = {}(...)", c.callee))),
            Stmt::Call(c) => return Err(no(c.line, c.col, &format!("`{}()` dentro de una gpu fn", c.callee), "una gpu fn solo llama a otra gpu fn, que se escribe EN LINEA en ella: lo de la CPU no corre en un hilo de la GPU", "escribe el calculo aqui mismo, o en otra gpu fn")),
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
                let inside = body(p, f, &fo.body, hondo + 1, forma)?;
                let turns = (to as i128 - from as i128).max(0) as u64;
                let cost = turns.saturating_mul(inside.saturating_add(2)).saturating_add(2);
                if cost > OBRA_MAXIMA {
                    return Err(too_much(fo.line, fo.col, cost));
                }
                (cost, Some((fo.line, fo.col)))
            }
            Stmt::Match { line, col, .. } => return Err(no(*line, *col, "un `match` dentro de una gpu fn", "dentro de una gpu fn solo hay f32 y bool: no hay enum que mirar", "decide con if / else")),
            // ** LB6: una que DIBUJA cambia un campo de un registro suyo
            // (`w.color = c`, con `let mut w = v`): sus f32, y los demas igual.
            Stmt::SetAt { path, value, .. } if forma != Forma::Celda && path.iter().all(|s| matches!(s, Step::Field(..))) => (1u64.saturating_add(expr(p, value, hondo, forma)?), None),
            Stmt::SetAt { line, col, .. } => return Err(no(*line, *col, "una parte de un valor cambia dentro de una gpu fn", "dentro de una gpu fn no hay tablas, y una celda no tiene partes: cada hilo tiene la suya (solo la que DIBUJA tiene registros, y cambia sus campos)", "devuelve el valor nuevo")),
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
        // ** LB5: a call to another gpu fn, IN LINE. Its work is counted where
        // it is called -- inside a loop, once per turn.
        let calls = "mod main \"x\"\ngpu fn cuadrado(x: f32) -> f32\n    return x * x\ngpu fn ocho(x: f32) -> f32\n    let mut r = x\n    for i in range(3)\n        r = cuadrado(r)\n    return r\nfn main()\n    print(1)\n";
        let m = lower(calls, Some(PIDE)).unwrap_or_else(|e| panic!("{:?}", e));
        let obra = |name: &str| m.functions.iter().find(|f| f.name == name).unwrap().obra;
        // cuadrado: return (1) and `*` (1). ocho: let (1), 3 turns of (2 + the
        // assignment (1) + the call (1) + cuadrado (2)), the for (2), return (1).
        assert_eq!(obra("cuadrado"), 2);
        assert_eq!(obra("ocho"), 1 + 3 * (2 + 1 + 1 + 2) + 2 + 1);
        // It calls itself: directly, and through another -- at the call that
        // closes the circle.
        let it = no("gpu fn f(x: f32) -> f32\n    return f(x) + 1.0\nfn main()\n    print(1)\n");
        assert_eq!((it.code, it.line, it.col), (Code::GpuBody, 3, 12), "{:?}", it);
        assert!(it.what.contains("f -> f"), "{}", it.what);
        let round = no("gpu fn a(x: f32) -> f32\n    return b(x)\ngpu fn b(x: f32) -> f32\n    let y = a(x)\n    return y\nfn main()\n    print(1)\n");
        assert_eq!((round.code, round.line, round.col), (Code::GpuBody, 5, 13), "{:?}", round);
        assert!(round.what.contains("a -> b -> a"), "{}", round.what);
        // A fn of the CPU, from a gpu fn; and a call alone, whose result nobody uses.
        let cpu = no("fn doble(x: int) -> int\n    return x * 2\ngpu fn f(x: f32) -> f32\n    return doble(x)\nfn main()\n    print(doble(1))\n");
        assert_eq!((cpu.code, cpu.line, cpu.col), (Code::GpuBody, 5, 12), "{:?}", cpu);
        let alone = no("gpu fn g(x: f32) -> f32\n    return x\ngpu fn f(x: f32) -> f32\n    g(x)\n    return x\nfn main()\n    print(1)\n");
        assert_eq!((alone.code, alone.line, alone.col), (Code::GpuBody, 5, 5), "{:?}", alone);
        // The work of a call inside loops: a callee whose work fits, called
        // in a loop of 10000 turns, does not.
        let big = no("gpu fn g(x: f32) -> f32\n    return x * x * x\ngpu fn f(x: f32) -> f32\n    let mut r = x\n    for i in range(10000)\n        r = g(r)\n    return r\nfn main()\n    print(1)\n");
        assert_eq!((big.code, big.line, big.col), (Code::NoEnd, 6, 5), "{:?}", big);
        // A huge N does not overflow the count.
        let huge = no("gpu fn f(x: f32) -> f32\n    let mut r = x\n    for i in range(-16777216, 16777216)\n        for j in range(-16777216, 16777216)\n            for k in range(-16777216, 16777216)\n                for l in range(-16777216, 16777216)\n                    r = r + x\n    return r\nfn main()\n    print(1)\n");
        assert_eq!(huge.code, Code::NoEnd, "{:?}", huge);
    }

    /// ** DL10 (09-10): a GENERAL division is a whole calculation on the
    /// 3060 (the exact one), so its work is `OBRA_DIVISION`; by a WRITTEN
    /// power of two it is a multiplication, and 1 -- as the writer writes it.
    #[test]
    fn a_general_division_weighs_its_calculation() {
        use super::{inverso_exacto, OBRA_DIVISION};
        let src = "mod main \"x\"\ngpu fn d(x: f32, y: f32) -> f32\n    return x / 3.0 + x / 2.0 + x / 4 + x / y + x / -2.0\nfn main()\n    print(1)\n";
        let m = lower(src, Some(PIDE)).unwrap_or_else(|e| panic!("{:?}", e));
        // return (1), four `+` (4), `/ 2.0` and `/ 4` (1 each), `/ 3.0`, `/ y`
        // and `/ -2.0` -- the sign makes it no longer a written number -- (24
        // each), and the sign (1).
        assert_eq!(m.functions.iter().find(|f| f.name == "d").unwrap().obra, 1 + 4 + 2 + 3 * OBRA_DIVISION + 1);
        // In a loop, it counts once per turn: 3000 turns of (2 + 1 + 24) go
        // over the top, where 3000 turns of a product fit.
        let much = no("gpu fn f(x: f32) -> f32\n    let mut r = x\n    for i in range(3000)\n        r = r / 3.0\n    return r\nfn main()\n    print(1)\n");
        assert_eq!((much.code, much.line, much.col), (Code::NoEnd, 4, 5), "{:?}", much);
        assert!(much.what.contains("81002"), "{:?}", much);
        lower("mod main \"x\"\ngpu fn f(x: f32) -> f32\n    let mut r = x\n    for i in range(3000)\n        r = r * 3.0\n    return r\nfn main()\n    print(1)\n", Some(PIDE)).unwrap();
        // The powers of two with an exact inverse, and the ones without.
        for c in [2.0f32, 0.5, -4.0, 1024.0, 1.0, 2.0f32.powi(126)] {
            assert!(inverso_exacto(c.to_bits()).is_some(), "{c}");
        }
        for c in [3.0f32, 0.1, 0.0, -0.0, f32::INFINITY, f32::NAN, 1.0e-45, 2.0f32.powi(127)] {
            assert_eq!(inverso_exacto(c.to_bits()), None, "{c}");
        }
    }

    /// ** LB6, DL6: a gpu fn DRAWS by its signature -- one record in, one
    /// out with its `posicion` (vertex); that one in, a colour of four f32
    /// out (pixel) -- and each way out of it has its NO, where it is written.
    #[test]
    fn a_gpu_fn_draws_by_its_signature() {
        use super::{forma, Forma};
        let tipos = "type Cuatro\n    x: f32\n    y: f32\n    z: f32\n    w: f32\ntype Vertice\n    posicion: Cuatro\n    color: Cuatro\n";
        let ok = format!("mod main \"x\"\n{}gpu fn vertice(v: Vertice) -> Vertice\n    let mut w = v\n    w.color.x = v.color.x * 0.5\n    return w\ngpu fn pixel(v: Vertice) -> Cuatro\n    return v.color\nfn main()\n    print(1)\n", tipos);
        let m = lower(&ok, Some(PIDE)).unwrap_or_else(|e| panic!("{:?}", e));
        let f = |n: &str| m.functions.iter().find(|f| f.name == n).unwrap();
        let de = |n: &str| forma(&m.types, &f(n).params.iter().map(|(_, t)| t).collect::<Vec<_>>(), f(n).ret.as_ref());
        assert_eq!((de("vertice"), de("pixel")), (Forma::Vertice { posicion: 0 }, Forma::Pixel { posicion: 0 }));
        // Its work: taking a field is no operation. The `let` (1), the field
        // that changes (1, and its `*`), the `return` (1).
        assert_eq!(f("vertice").obra, 1 + 2 + 1);
        let at = |src: String| {
            let n = no(&format!("{}{}fn main()\n    print(1)\n", tipos, src));
            (n.code, n.line, n.col)
        };
        // The pixel reads its position: it is the card's (SV_Position).
        assert_eq!(at("gpu fn pixel(v: Vertice) -> Cuatro\n    let p = v.posicion\n    return v.color\n".into()), (Code::GpuBody, 11, 15));
        // Its record LENT: each thread has its copy.
        assert_eq!(at("gpu fn vertice(mut v: Vertice) -> Vertice\n    return v\n".into()), (Code::GpuBody, 10, 20));
        // A record in a fn of cells; and a `posicion` of three: not a vertex.
        assert_eq!(at("gpu fn f(v: Vertice) -> f32\n    return v.color.x\n".into()), (Code::GpuBody, 10, 10));
        let tres = no("type Tres\n    x: f32\n    y: f32\n    z: f32\ntype V\n    posicion: Tres\ngpu fn vertice(v: V) -> V\n    return v\nfn main()\n    print(1)\n");
        assert_eq!((tres.code, tres.line, tres.col), (Code::GpuBody, 8, 16), "{:?}", tres);
        // One that draws is a stage of the pipeline: nobody calls it.
        assert_eq!(at("gpu fn vertice(v: Vertice) -> Vertice\n    return v\ngpu fn otro(v: Vertice) -> Vertice\n    let w = vertice(v)\n    return w\n".into()), (Code::GpuBody, 13, 13));
        // Two records are not compared: field by field, by IEEE.
        assert_eq!(at("gpu fn vertice(v: Vertice) -> Vertice\n    if v.color != v.posicion\n        return v\n    return v\n".into()), (Code::GpuBody, 11, 16));
    }
}

/// An expression of a gpu fn: its operations (its WORK), or its NO.
fn expr(p: &Program, e: &Expr, hondo: usize, forma: Forma) -> Result<u64, Message> {
    match e {
        Expr::Int { .. } | Expr::Dec { .. } | Expr::Name { .. } | Expr::Bool { .. } => Ok(0),
        Expr::Bin { op, left, right, .. } => {
            let propia = if *op == "/" && !por_su_inverso(right) { OBRA_DIVISION } else { 1 };
            Ok(propia.saturating_add(expr(p, left, hondo, forma)?).saturating_add(expr(p, right, hondo, forma)?))
        }
        Expr::Neg { value, .. } | Expr::Not { value, .. } => Ok(1u64.saturating_add(expr(p, value, hondo, forma)?)),
        // ** LB6: una que DIBUJA lee los campos de sus registros y los hace
        // -- elegir registros no cuesta: no es una operacion.
        Expr::Field { name, line, col, .. } if matches!(forma, Forma::Pixel { .. }) && name == POSICION => Err(no(*line, *col, &format!("`.{}` en una gpu fn de pixel", POSICION), "la posicion es de la 3060: con ella sabe QUE pixel pinta (SV_Position). Al de pixel le llegan los demas campos de lo que dejo el de vertice", "lee los otros campos: v.color")),
        Expr::Field { base, .. } if forma != Forma::Celda => expr(p, base, hondo, forma),
        Expr::Record { fields, .. } if forma != Forma::Celda => {
            let mut cost = 0u64;
            for (_, v) in fields {
                cost = cost.saturating_add(expr(p, v, hondo, forma)?);
            }
            Ok(cost)
        }
        Expr::Call { callee, line, col, .. } if gpu_fn(p, callee).is_some_and(|g| forma_de(p, g) != Forma::Celda) => Err(no(*line, *col, &format!("`{}()`: una gpu fn que DIBUJA no se llama", callee), "es una etapa de la tuberia de la 3060: la de vertice corre una vez por vertice y la de pixel una por pixel, y las pone a dibujar VERRANO (LB7), no otra fn", "llama a una gpu fn de celdas")),
        Expr::Text { line, col, .. } => Err(no(*line, *col, "un texto dentro de una gpu fn", "la 3060 cuenta numeros: un texto no tiene celda en ella", "deja los textos a la CPU")),
        // ** Another gpu fn (LB5): written in line, its work is its whole
        // body, here -- inside this one's loops, if it is.
        Expr::Call { callee, args, .. } if gpu_fn(p, callee).is_some() => {
            let g = gpu_fn(p, callee).expect("just seen");
            let mut cost = 1u64;
            for a in args {
                cost = cost.saturating_add(expr(p, a, hondo, forma)?);
            }
            Ok(cost.saturating_add(body(p, g, &g.body, hondo, forma_de(p, g))?))
        }
        Expr::Call { callee, line, col, .. } if callee.starts_with("director.") => Err(no_director(callee, *line, *col)),
        // A name nobody declared is the checker's NO (T0051), after this one.
        Expr::Call { callee, args, .. } if !p.functions.iter().any(|g| g.name == *callee) => {
            let mut cost = 1u64;
            for a in args {
                cost = cost.saturating_add(expr(p, a, hondo, forma)?);
            }
            Ok(cost)
        }
        Expr::Call { callee, line, col, .. } => Err(no(*line, *col, &format!("`{}()` dentro de una gpu fn", callee), &format!("una gpu fn solo llama a otra gpu fn, que se escribe EN LINEA en ella: `{}` es de la CPU, y lo de la CPU no corre en un hilo de la GPU", callee), "escribe el calculo aqui mismo, o en otra gpu fn")),
        Expr::Round { line, col, .. } => Err(no(*line, *col, "`round` dentro de una gpu fn", "round vuelve un f32 `dec`, y el dec es de la CPU: se redondea al volver", "devuelve el f32 y redondealo en la CPU: round(x, 2)")),
        Expr::Table { line, col, .. } | Expr::Repeat { line, col, .. } | Expr::Index { line, col, .. } => Err(no(*line, *col, "una tabla dentro de una gpu fn", "cada hilo tiene UNA celda: la tabla se le da al llamarla", "gpu fn f(x: f32) -> f32, y f(tabla)")),
        Expr::Field { line, col, .. } | Expr::Record { line, col, .. } => Err(no(*line, *col, "un registro dentro de una gpu fn", "dentro de una gpu fn solo hay f32 y bool", "pasa cada campo como su propio valor")),
        Expr::Lend { line, col, .. } => Err(no(*line, *col, "prestar dentro de una gpu fn", "cada hilo tiene su COPIA: una gpu fn recibe valores, no prestamos", "pasa el valor, y usa lo que devuelve")),
        Expr::Map { line, col, .. } => Err(no(*line, *col, "un mapa dentro de una gpu fn", "cada hilo tiene UNA celda: un mapa vive en la CPU", "busca en el mapa en la CPU y pasa el valor")),
    }
}
