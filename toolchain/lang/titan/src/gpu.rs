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
//!    its body      let, if / else and return, with + - * / and comparisons
//!    NOT           print (the 3060 has no console), tables, texts, records,
//!                  calls, loops -- each with its own NO, T0090
//! ```
//!
//! The f32 rule of the CPU side (D2) is the calculation's (`calc.rs`, T0091),
//! where the classes are; the permission (U2) is the package's: a `gpu fn`
//! needs the `Titan.toml` to ask for `gpu`, like `use gpu` (T0088).
//!
//! [!] Loops and calls between `gpu fn` are not refused for ever: they wait
//! for the GPU writer (`bmo-titan-prometeo`, PROMETEO) to carry them (IL1 of ILLAPA), and each one enters with its own
//! example. Said here so nobody takes the NO for a decision.

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
        body(f, &f.body)?;
    }
    Ok(())
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

fn body(f: &Function, stmts: &[Stmt]) -> Result<(), Message> {
    for st in stmts {
        match st {
            Stmt::Let(l) | Stmt::Set(l) => {
                if let Some(t) = &l.ty {
                    if !matches!(t, Ty::F32 | Ty::Bool) {
                        return Err(no(l.line, l.col, &format!("`{}: {}` dentro de una gpu fn", l.name, t.name()), "dentro de una gpu fn solo hay f32 y bool", &format!("let {}: f32 = ...", l.name)));
                    }
                }
                expr(&l.value)?;
            }
            Stmt::If(i) => {
                expr(&i.cond)?;
                body(f, &i.then)?;
                body(f, &i.other)?;
            }
            Stmt::Return { value: Some(v), .. } => expr(v)?,
            Stmt::Return { line, col, value: None } => return Err(no(*line, *col, "un `return` sin valor en una gpu fn", "cada hilo devuelve su celda", "return x")),
            Stmt::Call(c) if c.callee == "print" => return Err(no(c.line, c.col, "`print` dentro de una gpu fn", "la 3060 no tiene consola: miles de hilos escribiendo a la vez no dirian nada que se pueda leer", "devuelve el valor, y escribelo en la CPU: print(round(x, 2))")),
            Stmt::Call(c) => return Err(no(c.line, c.col, &format!("`{}()` dentro de una gpu fn", c.callee), "una gpu fn no llama a nada todavia: lo que corre en la 3060 se escribe entero en ella (las llamadas entre gpu fn llegan con el escritor de la 3060, IL1)", "escribe el calculo aqui mismo")),
            Stmt::While(w) => return Err(no(w.line, w.col, "un bucle dentro de una gpu fn", "el bucle de una gpu fn ES la tabla: un hilo por celda. Los bucles dentro de un hilo llegan con el escritor de la 3060 (IL1)", "aplica la gpu fn a una tabla mas grande")),
            Stmt::For(fo) => return Err(no(fo.line, fo.col, "un bucle dentro de una gpu fn", "el bucle de una gpu fn ES la tabla: un hilo por celda. Los bucles dentro de un hilo llegan con el escritor de la 3060 (IL1)", "aplica la gpu fn a una tabla mas grande")),
            Stmt::Match { line, col, .. } => return Err(no(*line, *col, "un `match` dentro de una gpu fn", "dentro de una gpu fn solo hay f32 y bool: no hay enum que mirar", "decide con if / else")),
            Stmt::SetAt { line, col, .. } => return Err(no(*line, *col, "una parte de un valor cambia dentro de una gpu fn", "dentro de una gpu fn no hay tablas ni registros: cada hilo tiene su celda", "devuelve el valor nuevo")),
            Stmt::Break { line, col } | Stmt::Continue { line, col } => return Err(no(*line, *col, "`break` / `continue` dentro de una gpu fn", "una gpu fn no tiene bucles", "decide con if / else")),
        }
    }
    Ok(())
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
}

fn expr(e: &Expr) -> Result<(), Message> {
    match e {
        Expr::Int { .. } | Expr::Dec { .. } | Expr::Name { .. } | Expr::Bool { .. } => Ok(()),
        Expr::Bin { left, right, .. } => {
            expr(left)?;
            expr(right)
        }
        Expr::Neg { value, .. } | Expr::Not { value, .. } => expr(value),
        Expr::Text { line, col, .. } => Err(no(*line, *col, "un texto dentro de una gpu fn", "la 3060 cuenta numeros: un texto no tiene celda en ella", "deja los textos a la CPU")),
        Expr::Call { callee, line, col, .. } => Err(no(*line, *col, &format!("`{}()` dentro de una gpu fn", callee), "una gpu fn no llama a nada todavia (las llamadas entre gpu fn llegan con G2)", "escribe el calculo aqui mismo")),
        Expr::Round { line, col, .. } => Err(no(*line, *col, "`round` dentro de una gpu fn", "round vuelve un f32 `dec`, y el dec es de la CPU: se redondea al volver", "devuelve el f32 y redondealo en la CPU: round(x, 2)")),
        Expr::Table { line, col, .. } | Expr::Repeat { line, col, .. } | Expr::Index { line, col, .. } => Err(no(*line, *col, "una tabla dentro de una gpu fn", "cada hilo tiene UNA celda: la tabla se le da al llamarla", "gpu fn f(x: f32) -> f32, y f(tabla)")),
        Expr::Field { line, col, .. } | Expr::Record { line, col, .. } => Err(no(*line, *col, "un registro dentro de una gpu fn", "dentro de una gpu fn solo hay f32 y bool", "pasa cada campo como su propio valor")),
        Expr::Lend { line, col, .. } => Err(no(*line, *col, "prestar dentro de una gpu fn", "una gpu fn no llama a nada", "escribe el calculo aqui mismo")),
        Expr::Map { line, col, .. } => Err(no(*line, *col, "un mapa dentro de una gpu fn", "cada hilo tiene UNA celda: un mapa vive en la CPU", "busca en el mapa en la CPU y pasa el valor")),
    }
}
