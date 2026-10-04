//! # TITAN++ para x86-64 -- de la IR a bytes, y al `.bex`
//!
//! T3 de `docs/maestro/TITAN_MAESTRO.md` (seccion 12), con los niveles 0 y 1:
//!
//! ```text
//!    bmo-titan-front      texto -> arbol -> IR       no nombra ninguna maquina
//!    bmo-titan-x86-64     IR -> bytes -> .bex        nombra x86-64 en cada linea
//!    (esto)
//! ```
//!
//! ## Lo que sale, entero
//!
//! ```text
//!    _start   call main            el arranque: lo primero del codigo,
//!             EXIT                 la entrada es el byte 0
//!    f0       escribe / call fK    una funcion: su bloque, en orden
//!             ret
//!    f1 ...
//! ```
//!
//! ** Los textos viajan como INMEDIATOS dentro del codigo, de ocho en ocho
//! bytes por la puerta (`bmo_lower::console::write_const`): sin constantes,
//! sin reubicaciones, sin monton. Es exactamente la secuencia que
//! `tools/hello-bex` corrio en el metal, y por eso el primer `.bex` de TITAN++
//! no estrena NADA del cargador: si no sale el `hola`, el fallo es de aqui.
//!
//! ## Lo que NO hace, dicho por delante
//!
//! - **No es el emisor de INTI ni lo enlaza.** Cada lenguaje, su IR y su emisor.
//! - **No pasa por `bmo-enlazar` todavia.** TITAN_MAESTRO T3 dice "un `.bo`";
//!   el nivel 0 no llama a nada de fuera, asi que un objeto no tendria que
//!   resolver ningun simbolo. El `.bo` llega el dia que TITAN++ llame a INTI.
//! - **No hay asignador de registros, ni valores**: el nivel 0 no tiene ninguno.

use bmo_abi::bef2;
use bmo_lower::{console, task};
use bmo_titan_front::ir::{End, Module, Op, Value};
use bmo_titan_front::Message;

/// Lo que sale de emitir un modulo.
#[derive(Debug, Clone)]
pub struct Emitted {
    pub code: Vec<u8>,
    /// Donde empieza cada funcion, en el orden de la IR.
    pub starts: Vec<usize>,
}

/// `call rel32` con el destino por parchear: devuelve donde va el campo.
fn call_rel32(code: &mut Vec<u8>) -> usize {
    code.push(0xE8);
    let field = code.len();
    code.extend_from_slice(&[0; 4]);
    field
}

fn ret(code: &mut Vec<u8>) {
    code.push(0xC3);
}

/// IR -> bytes. La IR llega JUZGADA y CALCULADA (`juez.rs`, `calc.rs`): en el
/// nivel 1 cada valor ya es una constante, y un `let` no deja bytes -- su valor
/// ya esta dentro de los textos que se escriben. Lo unico que no puede pasar
/// es una parte sin calcular: se dice (`Unfolded`) en vez de inventarle bytes.
pub fn emit(m: &Module) -> Result<Emitted, String> {
    let mut code = Vec::new();
    // (campo rel32, funcion destino): se resuelven AL FINAL, porque una
    // funcion puede llamar a otra que esta mas abajo.
    let mut calls: Vec<(usize, usize)> = Vec::new();

    // -- El arranque: llamar a main y salir. `EXIT` no vuelve; `task::exit`
    // deja detras la red de seguridad (`pause`/`jmp`) por si algun dia volviera.
    calls.push((call_rel32(&mut code), m.entry));
    task::exit(&mut code);

    let mut starts = Vec::with_capacity(m.functions.len());
    for f in &m.functions {
        starts.push(code.len());
        for b in &f.blocks {
            for op in &b.ops {
                match op {
                    Op::Write { parts, at } => {
                        let mut text = String::new();
                        for p in parts {
                            match p {
                                Value::Int(n, _) => text.push_str(&n.to_string()),
                                Value::Text(t, _) => text.push_str(t),
                                _ => return Err(format!("linea {}: una parte de print llego sin calcular", at.0)),
                            }
                        }
                        // `print` ends its line: ONE write, because the kernel
                        // flushes a console line at its `\n`.
                        text.push('\n');
                        console::write_const(&mut code, text.as_bytes());
                    }
                    // Already inside the texts that use it (calc.rs).
                    Op::Let { .. } | Op::Set { .. } => {}
                    Op::Call { func, .. } => calls.push((call_rel32(&mut code), *func)),
                }
            }
            match b.end {
                End::Return => ret(&mut code),
            }
        }
    }

    for (field, k) in calls {
        let rel = starts[k] as i64 - (field as i64 + 4);
        code[field..field + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }
    Ok(Emitted { code, starts })
}

/// Bytes + manifiesto -> el `.bex`, que ya paso el gate.
///
/// ** `exige_manifiesto` y no solo `verify`: es estrictamente mas fuerte (llama
/// a `verify` primero) y TITAN++ se obliga, como INTI, a no escribir NUNCA un
/// binario mudo. Si el cableado del manifiesto se rompe, no sale fichero.
pub fn package(e: &Emitted, manifest: &str) -> Result<Vec<u8>, String> {
    let mut b = bef2::Escritor::ejecutable();
    b.codigo(e.code.clone()).entrada(0);
    b.anexo(bef2::ANEXO_MANIFIESTO, manifest.as_bytes().to_vec());
    let bytes = b.construir().map_err(String::from)?;
    match bmo_verify::declaracion::exige_manifiesto(&bytes) {
        bmo_verify::Verdict::Ok => Ok(bytes),
        bmo_verify::Verdict::Rejected(why) => Err(why.join("; ")),
    }
}

/// Por que no salio un `.bex`.
#[derive(Debug)]
pub enum Failure {
    /// El NO del frontend: el mensaje de 4 partes.
    Source(Message),
    /// El gate dijo que no. Es un fallo de ESTE compilador, nunca del programa.
    Gate(String),
}

/// La cadena entera: texto -> `.bex`. `source_name` es el NOMBRE del fichero
/// (va al manifiesto), no su ruta.
pub fn build(src: &str, source_name: &str) -> Result<Vec<u8>, Failure> {
    let m = bmo_titan_front::lower(src).map_err(Failure::Source)?;
    let manifest = bmo_titan_front::manifest::manifest(&m, source_name);
    let e = emit(&m).map_err(Failure::Gate)?;
    package(&e, &manifest).map_err(Failure::Gate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_calls_main_even_when_main_is_not_first() {
        let m = bmo_titan_front::lower("mod main \"x\"\nfn otra()\n    print(\"a\")\nfn main()\n    otra()\n").unwrap();
        let e = emit(&m).unwrap();
        // call rel32 at byte 0: its target is the start of main (f1).
        assert_eq!(e.code[0], 0xE8);
        let rel = i32::from_le_bytes(e.code[1..5].try_into().unwrap()) as i64;
        assert_eq!((5 + rel) as usize, e.starts[1]);
    }

    #[test]
    fn every_function_ends_in_ret() {
        let m = bmo_titan_front::lower("mod main \"x\"\nfn main()\n    print(\"a\")\n").unwrap();
        let e = emit(&m).unwrap();
        assert_eq!(*e.code.last().unwrap(), 0xC3);
    }

    #[test]
    fn a_built_bex_carries_its_manifest_and_passes_the_gate() {
        let bex = build("mod main \"saluda\"\nfn main()\n    print(\"hola\")\n", "hola.titan").unwrap();
        assert!(bmo_verify::verify(&bex).is_ok());
        let t = bmo_verify::declaracion::manifiesto(&bex).expect("the manifest annex");
        let t = core::str::from_utf8(t).unwrap();
        assert!(t.contains("lenguaje = \"titan\"") && t.contains("fuente = \"hola.titan\""), "{}", t);
    }

    /// ** THE TWO JUDGES, end to end: the compiler certifies, the kernel's
    /// judge reads the certificate out of the `.bex` -- and a `.bex` whose
    /// certificate was changed after compiling (it now claims the NET, which
    /// its manifest never asked for) is caught, with the line it names.
    #[test]
    fn a_forged_certificate_is_caught_by_the_kernel_s_judge() {
        use bmo_titan_contrato::certificate::{judge, Certificate, Door, Use, Verdict};
        use bmo_titan_contrato::Permissions;
        let m = bmo_titan_front::lower("mod main \"x\"\nfn main()\n    print(\"hola\")\n").unwrap();
        let honest = bmo_titan_front::manifest::manifest(&m, "x.titan");
        let e = emit(&m).unwrap();
        let read = |bex: &[u8]| Certificate::read(bmo_verify::declaracion::manifiesto(bex).unwrap()).unwrap();
        let good = package(&e, &honest).unwrap();
        assert_eq!(judge(&read(&good), Permissions::NONE, Permissions::NONE), Verdict::Agrees);
        let forged = honest.replace("console = [3]", "console = [3]\nnet = [3]");
        assert_ne!(forged, honest);
        let bad = package(&e, &forged).unwrap();
        assert_eq!(judge(&read(&bad), Permissions::NONE, Permissions::NONE), Verdict::Unasked(Use { door: Door::Net, line: 3 }));
    }

    #[test]
    fn a_source_with_a_no_writes_nothing() {
        match build("mod main \"x\"\nfn otra()\n    print(\"a\")\n", "x.titan") {
            Err(Failure::Source(m)) => assert_eq!(m.code.label(), "T0050"),
            other => panic!("expected the NO of the frontend, got {:?}", other.map(|b| b.len())),
        }
    }
}
