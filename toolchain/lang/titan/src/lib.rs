//! # TITAN++ -- el lenguaje para CONSTRUIR sobre BMO-X
//!
//! Frontend: de texto a arbol. El **porque** de cada decision esta en
//! `docs/maestro/TITAN_MAESTRO.md`; **lo que se escribe**, en `GRAMATICA.md`,
//! al lado de este fichero.
//!
//! ## Los modulos, y por que son estos
//!
//! Cortados como INTI: por **lo que cada pieza puede decir sin nombrar a las
//! demas**, no por las fases de un libro.
//!
//! ```text
//!    message   el mensaje de 4 partes y sus codigos. No sabe que existe TITAN++
//!    words     las 25 palabras con su NIVEL, y el TITAN guardian (su prueba)
//!    indent    la regla del margen: lo unico con estado del barrido
//!    lex       de texto a piezas. No conoce la gramatica
//!    tree      la forma de un programa. Cero decisiones
//!    parse     la gramatica del nivel de hoy; lo de arriba dice en que nivel llega
//!    check     los nombres: `main`, una vez cada fn, que cada llamada exista
//!              y que ninguna vuelva sobre si misma (T0053)
//!    ir        lo que el programa HACE, sin maquina: la IR PROPIA (T3)
//!    juez      EL BORROW CHECKER, sobre la IR: cada local, en cada punto, en
//!              UN estado; hoy nace y vive (nivel 1), luego se presta y se
//!              entrega (nivel 7) y lo presta el kernel (U1)
//!    calc      lo que se sabe al compilar se calcula al compilar: exacto, o NO
//!    manifest  lo que el `.bex` dira de si mismo (lo escribe el frontend)
//! ```
//!
//! ## Lo que hay hoy: el NIVEL 0 de la escalera (TITAN_MAESTRO 14.14)
//!
//! ```text
//!    mod main "saluda"
//!
//!    fn main()
//!        print("hola")
//! ```
//!
//! Entra el texto, sale el arbol o UN mensaje de 4 partes, y del arbol la IR.
//! **Este crate no emite bytes, y no puede**: los bytes son del emisor
//! (`emisor-x86_64/`, T3), que es el unico que nombra una maquina y el que
//! tiene la orden `titan`. El primer `.bex` escribe en la consola (decidido el
//! 30-09). Cada nivel entra el dia que su banco (`ejemplos/nivelN/`) pasa
//! entero.

pub mod calc;
pub mod check;
pub mod indent;
pub mod ir;
pub mod juez;
pub mod manifest;
pub mod lex;
pub mod message;
pub mod parse;
pub mod tree;
pub mod words;

pub use message::{Code, Message};
pub use tree::Program;

/// El frontend entero: texto -> arbol comprobado, o el primer NO.
pub fn compile(src: &str) -> Result<Program, Message> {
    let tokens = lex::lex(src)?;
    let program = parse::parse(&tokens)?;
    check::check(&program)?;
    Ok(program)
}

/// Texto -> IR juzgada y calculada: lo que recibe el emisor. Despues del
/// arbol, EL JUEZ (`juez.rs`) dice si cada valor existe y puede lo que se le
/// pide; despues, `calc.rs` lo calcula. Cada paso, su primer NO.
pub fn lower(src: &str) -> Result<ir::Module, Message> {
    let m = ir::lower(&compile(src)?);
    juez::judge(&m)?;
    calc::fold(&m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hola_is_a_tree_with_one_call() {
        let p = compile("mod main \"saluda\"\n\nfn main()\n    print(\"hola\")\n").unwrap();
        assert_eq!((p.module.as_str(), p.purpose.as_str()), ("main", "saluda"));
        assert_eq!(p.functions.len(), 1);
        let tree::Stmt::Call(c) = &p.functions[0].body[0] else { panic!() };
        assert_eq!(c.callee, "print");
        assert_eq!(c.args.len(), 1);
    }

    #[test]
    fn a_word_of_a_higher_level_says_which_level() {
        let e = compile("mod main \"x\"\nfn main()\n    match x\n").unwrap_err();
        assert_eq!(e.code, Code::NotYet);
        assert_eq!(e.what, "`match` llega en el nivel 8 (casos con datos)");
        let e = compile("mod main \"x\"\nfn f(x: f32)\n    print(1)\nfn main()\n    f(1)\n").unwrap_err();
        assert_eq!(e.what, "el tipo `f32` llega en el nivel 11 (la 3060)");
        let e = compile("mod main \"x\"\nfn f(take n: int)\n    print(n)\nfn main()\n    f(1)\n").unwrap_err();
        assert_eq!(e.what, "prestar o entregar un parametro llega en el nivel 7 (prestar y entregar)");
    }

    #[test]
    fn a_misspelled_call_gets_its_did_you_mean() {
        let e = compile("mod main \"x\"\nfn main()\n    pritn(\"hola\")\n").unwrap_err();
        assert_eq!((e.code, e.how.as_str()), (Code::Unknown, "quisiste decir `print`?"));
    }
}
