//! `manifest` -- what the `.bex` says about itself, for whoever only has the
//! `.bex` (the same question INTI answers in `src/manifiesto`).
//!
//! ** Written by the FRONTEND, not the emitter: it is a statement about the
//! MODULE -- its name, what it does (U3), what it asks for (U2) -- and the
//! emitter has no business knowing what a module is. It receives the text and
//! puts it in its annex without reading it.
//!
//! ** Written by hand and not with a serializer, so the order never moves: two
//! builds of the same source give the same file byte by byte, or "this is the
//! `.bex` I audited" can no longer be said. For the same reason `fuente` is
//! the file NAME and never the path of the machine that compiled it.

use crate::ir::Module;
use crate::words::LEVEL_NOW;

/// The TOML that goes in the annex.
pub fn manifest(m: &Module, source_name: &str) -> String {
    let mut t = String::new();
    t.push_str("# Lo que este binario declara sobre si mismo.\n");
    t.push_str("# Lo escribio el compilador de TITAN++; no se edita a mano.\n\n");
    t.push_str("[modulo]\n");
    t.push_str("lenguaje = \"titan\"\n");
    t.push_str(&format!("nivel = {}\n", LEVEL_NOW));
    t.push_str(&format!("nombre = {}\n", quoted(&m.name)));
    t.push_str(&format!("que_hace = {}\n", quoted(&m.purpose)));
    t.push_str(&format!("fuente = {}\n", quoted(source_name)));
    t.push_str("\n[permissions]\n");
    // U2, from the first `.bex`: the section is there and says what it asks.
    // Level 0 asks nothing -- writing on the console of one's own task is not
    // a permission -- and saying "nothing" is not the same as not saying.
    t.push_str("# el nivel 0 no pide ninguno: escribir en la consola de la propia tarea no es un permiso\n");
    t
}

/// A TOML basic string: the purpose is the author's text and may carry `"`.
fn quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_names_the_language_the_module_and_the_file_never_the_path() {
        let m = crate::lower("mod main \"dice \\\"hola\\\"\"\nfn main()\n    print(\"hola\")\n").unwrap();
        let t = manifest(&m, "hola.titan");
        assert!(t.contains("lenguaje = \"titan\"\n"), "{}", t);
        assert!(t.contains("nombre = \"main\"\n"), "{}", t);
        assert!(t.contains("que_hace = \"dice \\\"hola\\\"\"\n"), "{}", t);
        assert!(t.contains("fuente = \"hola.titan\"\n") && t.contains("[permissions]"), "{}", t);
        assert_eq!(t, manifest(&m, "hola.titan"), "the same source, the same bytes");
    }
}
