//! # TITAN++ para x86-64 -- de la IR a bytes, y al `.bex`
//!
//! T3 de `docs/maestro/TITAN_MAESTRO.md` (seccion 12), con los niveles 0 a 3:
//!
//! ```text
//!    bmo-titan-front      texto -> arbol -> IR       no nombra ninguna maquina
//!    bmo-titan-x86-64     IR -> bytes -> .bex        nombra x86-64 en cada linea
//!    (esto)
//! ```
//!
//! ## Lo que sale, entero (nivel 4, 2026-10-04)
//!
//! ```text
//!    escribe "..."     lo que el programa escribe, en orden: el calculo
//!    escribe "..."     (`calc.rs`) ya lo CORRIO entero al compilar --
//!    ...               cada llamada, cada `if`, cada vuelta de cada bucle
//!    EXIT              y el `.bex` hace exactamente eso
//! ```
//!
//! ** Hasta el nivel 3 salian funciones con `call`/`ret` y bloques con
//! `jmp`. Con bucles eso ya no alcanza: una linea dentro de un `for` escribe
//! otra cosa en cada vuelta, y no hay un valor por linea que emitir. Mientras
//! nada venga de fuera, el programa ENTERO se sabe al compilar, y lo que se
//! emite es su resultado (`Module::flat`). El camino de bloques (`emit_blocks`)
//! se queda: es el de E1, el dia que una parte del programa tenga que correr de
//! verdad en la maquina (TITAN_MAESTRO 7.3).
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
//! - **No hay asignador de registros, ni valores**: hasta el nivel 3 todo valor
//!   se sabe al compilar (`calc.rs`), asi que no hay ninguno que guardar.
//!
//! ## Los bloques (nivel 3)
//!
//! Un `if` parte la funcion en bloques. Al emisor le llegan DECIDIDOS: la
//! condicion ya es `true` o `false`, y el lado que no corre viene marcado
//! `dead` -- **no deja ni un byte**. Lo que queda es un camino:
//!
//! ```text
//!    Branch(true/false)   un `jmp` al lado que corre, o nada si es el siguiente
//!    Jump                 un `jmp`, o nada si el destino es el siguiente
//!    bloque muerto        nada: ni bytes, ni puerta en el certificado
//! ```
//!
//! ** Un `jmp` que nadie salta no se escribe: con los muertos fuera, casi
//! todos los saltos caen al bloque de al lado. El nivel 4 (`while`) traera el
//! primer salto HACIA ARRIBA, y la misma lista de parches lo resuelve.

mod e1;

use bmo_abi::bef2;
use bmo_lower::{console, task};
use bmo_titan_front::ir::{Module, Op, Value};
use bmo_titan_front::Message;

/// Lo que sale de emitir un modulo.
#[derive(Debug, Clone)]
pub struct Emitted {
    pub code: Vec<u8>,
    /// Donde empieza cada funcion, en el orden de la IR.
    pub starts: Vec<usize>,
}

/// IR -> bytes. La IR llega JUZGADA y CALCULADA (`juez.rs`, `calc.rs`): en el
/// nivel 1 cada valor ya es una constante, y un `let` no deja bytes -- su valor
/// ya esta dentro de los textos que se escriben. Lo unico que no puede pasar
/// es una parte sin calcular: se dice (`Unfolded`) en vez de inventarle bytes.
pub fn emit(m: &Module) -> Result<Emitted, String> {
    match &m.flat {
        Some(flat) => emit_flat(flat),
        // E1 (`docs/plan/PLAN_LA_ENTRADA.md`): the program reads from outside,
        // so it was not run when compiling -- it is emitted to run (`e1.rs`).
        None => e1::emit(m),
    }
}

/// Una escritura ya calculada, como texto: lo que `print` deja en la consola.
fn text_of(parts: &[Value], at: (usize, usize)) -> Result<String, String> {
    let mut text = String::new();
    for p in parts {
        match p {
            Value::Int(n, _) => text.push_str(&n.to_string()),
            Value::Text(t, _) => text.push_str(t),
            Value::Bool(b, _) => text.push_str(if *b { "true" } else { "false" }),
            _ => return Err(format!("linea {}: una parte de print llego sin calcular", at.0)),
        }
    }
    // `print` ends its line: ONE write, because the kernel flushes a console
    // line at its `\n`.
    text.push('\n');
    Ok(text)
}

/// E0 entero: lo que el programa escribe, y EXIT.
fn emit_flat(flat: &[Op]) -> Result<Emitted, String> {
    let mut code = Vec::new();
    for op in flat {
        if let Op::Write { parts, at } = op {
            console::write_const(&mut code, text_of(parts, *at)?.as_bytes());
        }
    }
    task::exit(&mut code);
    Ok(Emitted { code, starts: vec![0] })
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
    build_package(source_name, src, &mut |_| None)
}

/// A PACKAGE to a `.bex` (level 9): the root file (its path from the package
/// and its text), and `read` for the files its `mod`s name. The manifest
/// names the root.
pub fn build_package(root: &str, src: &str, read: &mut dyn FnMut(&str) -> Option<String>) -> Result<Vec<u8>, Failure> {
    let source_name = root.rsplit('/').next().unwrap_or(root);
    // ** Nivel 11 (G3): las celdas de cada gpu fn las calcula el ORACULO de
    // spirv, sobre el SPIR-V que escribe `bmo-titan-spirv`; el .bex lleva
    // esos resultados.
    let mut oracle = bmo_titan_spirv::Oracle::default();
    let m = bmo_titan_front::lower_package_with(root, src, read, Some(&mut oracle)).map_err(Failure::Source)?;
    // (G2): y TODAS, tambien las que ninguna ejecucion llamo, escritas y
    // juzgadas. Si una no pasa, el fallo es del ESCRITOR: no hay .bex.
    bmo_titan_spirv::kernels(&m).map_err(Failure::Gate)?;
    let manifest = bmo_titan_front::manifest::manifest(&m, source_name);
    let e = emit(&m).map_err(Failure::Gate)?;
    package(&e, &manifest).map_err(Failure::Gate)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ** El programa ENTERO, corrido al compilar: las llamadas y las vueltas
    /// ya no dejan `call` ni `jmp`, solo lo que escriben, en su orden.
    #[test]
    fn calls_and_loops_leave_only_what_they_write() {
        let m = bmo_titan_front::lower("mod main \"x\"\nfn otra()\n    print(\"a\")\nfn main()\n    for i in range(2)\n        otra()\n    print(\"fin\")\n").unwrap();
        let e = emit(&m).unwrap();
        let has = |w: &[u8]| e.code.windows(w.len()).filter(|x| *x == w).count();
        assert_ne!(e.code[0], 0xE8, "no call: the program starts by writing");
        assert_eq!(has(b"a\n"), 2, "two turns, two writes");
        assert_eq!(has(b"fin\n"), 1);
    }

    #[test]
    fn every_program_ends_in_exit() {
        let m = bmo_titan_front::lower("mod main \"x\"\nfn main()\n    print(\"a\")\n").unwrap();
        let e = emit(&m).unwrap();
        let mut exit = Vec::new();
        task::exit(&mut exit);
        assert!(e.code.ends_with(&exit));
    }

    /// ** U2 (level 11, G0): what the package's Titan.toml asks for travels
    /// to the `.bex` manifest, where `titan juez` and the kernel read it.
    #[test]
    fn a_package_carries_the_permissions_its_titan_toml_asks_for() {
        let files = [("src/main.titan", "mod main \"x\"\nuse gpu\n\nfn main()\n    print(1)\n"), ("Titan.toml", "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\nnet = false\n")];
        let bex = build_package(files[0].0, files[0].1, &mut |p| files.iter().find(|f| f.0 == p).map(|f| f.1.to_string())).unwrap();
        let t = core::str::from_utf8(bmo_verify::declaracion::manifiesto(&bex).unwrap()).unwrap().to_string();
        assert!(t.contains("[permissions]") && t.contains("gpu = true") && !t.contains("net ="), "{}", t);
    }

    /// ** U2 and U1's first half (level 11): a call to a `gpu fn` opens the
    /// GPU's door in the certificate, at its line, and the kernel's judge on
    /// the PC agrees only when `gpu` is asked AND granted.
    #[test]
    fn a_gpu_call_is_named_in_the_certificate_and_judged() {
        use bmo_titan_contrato::certificate::{judge, Certificate, Door, Verdict};
        use bmo_titan_contrato::{Permission, Permissions};
        let main = "mod main \"x\"\n\ngpu fn d(x: f32) -> f32\n    return x * 2.0\n\nfn main()\n    let xs: [f32; 2] = [1.0, 2.0]\n    let r = d(xs)\n    print(round(r[0], 1))\n";
        let files = [("src/main.titan", main), ("Titan.toml", "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n")];
        let bex = build_package(files[0].0, files[0].1, &mut |p| files.iter().find(|f| f.0 == p).map(|f| f.1.to_string())).unwrap();
        let cert = Certificate::read(bmo_verify::declaracion::manifiesto(&bex).unwrap()).unwrap();
        assert_eq!(cert.line_of(Door::Gpu), Some(8));
        let asked = Permissions::NONE.with(Permission::Gpu);
        assert_eq!(judge(&cert, asked, asked), Verdict::Agrees);
        assert!(matches!(judge(&cert, asked, Permissions::NONE), Verdict::Ungranted(u) if u.door == Door::Gpu && u.line == 8));
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

    /// ** What never runs leaves no byte: the dead side of a decided `if`
    /// is not in the code -- its text is nowhere in the `.bex`.
    #[test]
    fn the_dead_side_of_an_if_leaves_no_byte() {
        let src = "mod main \"x\"\nfn main()\n    if 2 > 1\n        print(\"VIVO\")\n    else\n        print(\"MUERTO\")\n";
        let e = emit(&bmo_titan_front::lower(src).unwrap()).unwrap();
        let has = |w: &[u8]| e.code.windows(w.len()).any(|x| x == w);
        // write_const carries texts eight bytes at a time: "VIVO\n" fits in one.
        assert!(has(b"VIVO\n"), "the live side is there");
        assert!(!has(b"MUERTO"[..4].as_ref()), "the dead side left bytes");
    }

    #[test]
    fn a_source_with_a_no_writes_nothing() {
        match build("mod main \"x\"\nfn otra()\n    print(\"a\")\n", "x.titan") {
            Err(Failure::Source(m)) => assert_eq!(m.code.label(), "T0050"),
            other => panic!("expected the NO of the frontend, got {:?}", other.map(|b| b.len())),
        }
    }
}
