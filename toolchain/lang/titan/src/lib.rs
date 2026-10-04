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

pub mod check;
pub mod indent;
pub mod ir;
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

/// Texto -> IR: lo que recibe el emisor. El mismo camino que `compile`, y un
/// paso mas que no puede fallar.
pub fn lower(src: &str) -> Result<ir::Module, Message> {
    compile(src).map(|p| ir::lower(&p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hola_is_a_tree_with_one_call() {
        let p = compile("mod main \"saluda\"\n\nfn main()\n    print(\"hola\")\n").unwrap();
        assert_eq!((p.module.as_str(), p.purpose.as_str()), ("main", "saluda"));
        assert_eq!(p.functions.len(), 1);
        assert_eq!(p.functions[0].body[0].callee, "print");
        assert_eq!(p.functions[0].body[0].args, ["hola"]);
    }

    #[test]
    fn a_word_of_a_higher_level_says_which_level() {
        let e = compile("mod main \"x\"\nfn main()\n    let x = 1\n").unwrap_err();
        assert_eq!(e.code, Code::NotYet);
        assert_eq!(e.what, "`let` llega en el nivel 1 (calcular)");
        let e = compile("mod main \"x\"\nfn main()\n    while x\n").unwrap_err();
        assert_eq!(e.what, "`while` llega en el nivel 4 (repetir)");
    }

    #[test]
    fn a_misspelled_call_gets_its_did_you_mean() {
        let e = compile("mod main \"x\"\nfn main()\n    pritn(\"hola\")\n").unwrap_err();
        assert_eq!((e.code, e.how.as_str()), (Code::Unknown, "quisiste decir `print`?"));
    }
}
