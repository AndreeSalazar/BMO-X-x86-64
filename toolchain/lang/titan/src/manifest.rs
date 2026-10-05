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
//!
//! ** And it carries the CERTIFICATE (J1, TITAN_MAESTRO 6b.3): every door of
//! BMO-X the program goes through and the line that opens it, taken from the
//! IR that EL JUEZ already judged. The kernel's load gate reads it with the
//! same code (`bmo_titan_contrato::certificate`) and compares; it never
//! GRANTS by it -- it NAMES by it.

use crate::ir::{End, Module, Op, Value};
use crate::words::LEVEL_NOW;
use bmo_titan_contrato::{Certificate, Door};

/// The certificate of a judged module: the doors, in source order.
pub fn certificate(m: &Module) -> Certificate {
    let mut uses: Vec<(usize, Door)> = Vec::new();
    // What the program really does (`Module::flat`, run when compiling): a
    // line that never runs opens no door, and a line inside a loop opens it
    // ONCE in the certificate, however many turns it writes.
    let ops: Vec<&Op> = match &m.flat {
        Some(flat) => flat.iter().collect(),
        None => m.functions.iter().flat_map(|f| f.blocks.iter().filter(|b| !b.dead).flat_map(|b| &b.ops)).collect(),
    };
    for op in ops {
        if let Op::Write { at, .. } = op {
            // `print`: the console of the program's own task.
            uses.push((at.0, Door::Console));
        }
    }
    // ** The 3060 (level 11): every call to a `gpu fn` from a block that
    // runs opens the door of the GPU, at its line. A call in a block no run
    // reaches asks for nothing.
    for f in m.functions.iter().filter(|f| !f.gpu) {
        for b in f.blocks.iter().filter(|b| !b.dead) {
            let mut values: Vec<&Value> = Vec::new();
            for op in &b.ops {
                match op {
                    Op::Let { value, .. } | Op::Set { value, .. } | Op::SetAt { value, .. } => values.push(value),
                    Op::Write { parts, .. } => values.extend(parts),
                    Op::Call { func, args, at } => {
                        if m.functions[*func].gpu {
                            uses.push((at.0, Door::Gpu));
                        }
                        values.extend(args);
                    }
                    Op::Drop { .. } => {}
                }
            }
            match &b.end {
                End::Return(Some(v)) => values.push(v),
                End::Branch { cond, .. } => values.push(cond),
                _ => {}
            }
            for v in values {
                gpu_calls(v, m, &mut uses);
            }
        }
    }
    uses.sort_by_key(|u| u.0);
    uses.dedup();
    let mut c = Certificate::new();
    for (line, door) in uses {
        c.add(door, line.min(u16::MAX as usize) as u16);
    }
    c
}

/// The doors opened INSIDE a value: a call to a `gpu fn` (the GPU's), and
/// `lee()` (E1: the program's own console, the same door `print` writes by).
fn gpu_calls(v: &Value, m: &Module, out: &mut Vec<(usize, Door)>) {
    match v {
        Value::Read(at) => out.push((at.0, Door::Console)),
        Value::Call(f, args, at) => {
            if m.functions[*f].gpu {
                out.push((at.0, Door::Gpu));
            }
            args.iter().for_each(|a| gpu_calls(a, m, out));
        }
        Value::Bin(_, a, b, _) | Value::Index(a, b, _) => {
            gpu_calls(a, m, out);
            gpu_calls(b, m, out);
        }
        Value::Neg(a, _) | Value::Not(a, _) | Value::Repeat(a, _, _) | Value::Field(a, _, _) | Value::Len(a, _) | Value::Round(a, _, _) | Value::Is(a, _, _, _) | Value::Payload(a, _, _, _, _) => gpu_calls(a, m, out),
        Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) => items.iter().for_each(|a| gpu_calls(a, m, out)),
        _ => {}
    }
}

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
    t.push_str("# escribir en la consola de la propia tarea no es un permiso; lo demas lo pide el Titan.toml del paquete\n");
    // What the package asks for, in the contract's order: the same build,
    // the same bytes.
    for p in bmo_titan_contrato::Permission::ALL {
        if m.permissions.allows(p) {
            t.push_str(&format!("{} = true\n", p.key()));
        }
    }
    let mut buf = [0u8; 4096];
    if let Ok(n) = certificate(m).write(&mut buf) {
        t.push('\n');
        t.push_str(core::str::from_utf8(&buf[..n]).unwrap_or(""));
    }
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

    #[test]
    fn a_print_that_never_runs_opens_no_door() {
        let m = crate::lower("mod main \"x\"\nfn main()\n    if false\n        print(\"nunca\")\n    print(\"si\")\n").unwrap();
        assert!(manifest(&m, "x.titan").contains("console = [5]"), "{}", manifest(&m, "x.titan"));
    }

    #[test]
    fn the_certificate_names_every_print_line_and_reads_back() {
        let m = crate::lower("mod main \"x\"\nfn main()\n    let a = 2\n    print(a)\n    otra()\nfn otra()\n    print(\"b\")\n").unwrap();
        let t = manifest(&m, "x.titan");
        assert!(t.contains("[certificado]") && t.contains("console = [4, 7]"), "{}", t);
        let c = Certificate::read(t.as_bytes()).unwrap();
        assert_eq!(c.line_of(Door::Console), Some(4));
        // The console needs nothing: the kernel's judge has nothing to add.
        use bmo_titan_contrato::{certificate::judge, Permissions, Verdict};
        assert_eq!(judge(&c, Permissions::NONE, Permissions::NONE), Verdict::Agrees);
    }
}
