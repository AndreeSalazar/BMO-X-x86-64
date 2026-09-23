//! **La matriz: el lector y el juez contra una carpeta de `.spv` de otros.**
//!
//! Herramienta del ANFITRION (usa `std` para leer ficheros; la biblioteca no:
//! sigue siendo `no_std` sin `alloc`, y esto la usa desde fuera). La conduce
//! `herramientas/censo_naga.py` con el banco de pruebas de Naga, que vive
//! FUERA del repo (`BMO-externo/naga-corpus`).
//!
//!     cargo run -q -p bmo-spirv-front --example census -- <carpeta de .spv>
//!
//! Lo que dice: cuantos se leen, cuantos caben, y POR QUE no el resto,
//! ordenado por cuantas veces sale cada motivo. Es el numero que decide que
//! se amplia del subconjunto, en vez de ampliarlo a ojo.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use bmo_spirv_front::table::op;
use bmo_spirv_front::{census, read, validate, workspace_words, Buffer, Family, Interpreter, Module, Reason};

/// Cuantas palabras tiene cada buffer que se le da al oraculo: de sobra para
/// que los del banco de Naga (que no traen datos) no se salgan por poco.
const PALABRAS_POR_BUFFER: usize = 1 << 14;

/// Los `(set, binding)` de todos los buffers que declara el modulo.
fn buffers_de(m: &Module) -> Vec<(u32, u32)> {
    let deco = |id: u32, d: u32| {
        m.instructions()
            .find(|i| i.opcode == op::OpDecorate && i.op(1) == id && i.op(2) == d)
            .map(|i| i.op(3))
            .unwrap_or(0)
    };
    m.instructions()
        .take_while(|i| i.opcode != op::OpFunction)
        .filter(|i| i.opcode == op::OpVariable && (i.op(3) == 2 || i.op(3) == 12))
        .map(|i| (deco(i.op(2), 34), deco(i.op(2), 33)))
        .collect()
}

/// **El oraculo sobre un modulo que cabe**: un grupo de trabajo, buffers a
/// cero. Lo que se mide es si SABE ejecutarlo; una division por cero con los
/// datos a cero es del sombreador, no un hueco del oraculo.
fn ejecutar(m: &Module) -> Result<u64, Reason> {
    let mut ws = vec![0u32; workspace_words(m)];
    let mut it = Interpreter::new(m, &mut ws).map_err(|e| e.reason)?;
    let pares = buffers_de(m);
    let mut datos: Vec<Vec<u32>> = pares.iter().map(|_| vec![0u32; PALABRAS_POR_BUFFER]).collect();
    let mut bufs: Vec<Buffer> = pares
        .iter()
        .zip(datos.iter_mut())
        .map(|(&(set, binding), d)| Buffer { set, binding, data: d })
        .collect();
    it.dispatch([1, 1, 1], &mut bufs, 200_000).map(|s| s.instructions).map_err(|t| t.reason)
}

fn clave(m: &Reason) -> String {
    // El motivo sin el numero de palabra, y con lo que lo distingue.
    match m {
        Reason::UnsupportedFamily { family: Family::Other, opcode } => {
            format!("{}", bmo_spirv_front::op_info(*opcode).map(|f| f.name).unwrap_or("?"))
        }
        Reason::UnsupportedFamily { family, .. } => format!("familia: {}", family.name()),
        Reason::UnsupportedCapability { capability } => format!("capacidad {}", capability),
        Reason::UnsupportedStorageClass { class } => format!("clase de almacenamiento {}", class),
        Reason::UnsupportedStage { model } => format!("etapa {} (no es computo)", model),
        Reason::UnsupportedGlsl { number } => format!("GLSL.std.450 numero {} fuera", number),
        Reason::GlslLater { number } => format!("GLSL.std.450 numero {} (S3b)", number),
        Reason::UnsupportedMode { mode } => format!("modo de ejecucion {}", mode),
        Reason::UnsupportedType { why } => format!("tipo: {}", why),
        Reason::UnsupportedBuiltIn { builtin } => format!("BuiltIn {}", builtin),
        Reason::UnknownOpcode { opcode } => format!("lector: codigo {} sin fila", opcode),
        Reason::TypeMismatch { opcode } | Reason::IndexOutOfRange { opcode } | Reason::OutsideBlock { opcode } => {
            format!("{} ({})", m.name(), opcode)
        }
        otro => otro.name().to_string(),
    }
}

fn main() {
    let carpeta = std::env::args().nth(1).expect("uso: censo <carpeta con .spv>");
    let mut nombres: Vec<_> = fs::read_dir(&carpeta)
        .expect("no se puede leer la carpeta")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "spv").unwrap_or(false))
        .collect();
    nombres.sort();

    let (mut leidos, mut caben, mut de_computo) = (0, 0, 0);
    let mut motivos: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // Los mismos, solo de los que traen una entrada de computo: es lo que el
    // subconjunto PROMETE aceptar, y lo que ordena que se amplia primero.
    let mut de_computo_no: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // Lo que dijo el oraculo de los que caben.
    let mut oraculo: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut familias = [0usize; 9];
    let mut con_familia = [0usize; 9];

    for p in &nombres {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let bytes = fs::read(p).unwrap();
        let bound = if bytes.len() >= 16 {
            u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize
        } else {
            0
        };
        let mut ids = vec![0u32; bound.clamp(1, 1 << 22)];
        let m = match read(&bytes, &mut ids) {
            Ok(m) => m,
            Err(f) => {
                println!("  NO LEE  {:<40} {}", name, f);
                motivos.entry(format!("LECTOR -- {}", clave(&f.reason))).or_default().push(name);
                continue;
            }
        };
        leidos += 1;
        let computo = m.entry_points().iter().any(|e| e.model == 5);
        if computo {
            de_computo += 1;
        }
        let c = census(&m);
        for (i, n) in c.per_family.iter().enumerate() {
            familias[i] += n;
            if *n > 0 {
                con_familia[i] += 1;
            }
        }
        match validate(&m) {
            Ok(v) => {
                caben += 1;
                let dijo = match ejecutar(&m) {
                    Ok(n) => format!("ejecuta ({} instrucciones)", n),
                    Err(r) => format!("PARA: {}", clave(&r)),
                };
                println!("  CABE    {:<40} {} bloques, {} funciones -- oraculo: {}", name, v.blocks, v.functions, dijo);
                let grupo = if dijo.starts_with("ejecuta") { "ejecuta".to_string() } else { dijo };
                oraculo.entry(grupo).or_default().push(name.clone());
            }
            Err(f) => {
                println!("  NO      {:<40} {}", name, f);
                if computo {
                    de_computo_no.entry(clave(&f.reason)).or_default().push(name.clone());
                }
                motivos.entry(clave(&f.reason)).or_default().push(name);
            }
        }
    }

    println!();
    println!("  {} ficheros | {} se leen | {} traen una entrada de computo | {} CABEN", nombres.len(), leidos, de_computo, caben);
    println!();
    println!("  por que no (el PRIMER motivo de cada uno), de mas a menos:");
    let mut orden: Vec<_> = motivos.into_iter().collect();
    orden.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    for (reason, quienes) in &orden {
        let muestra: Vec<_> = quienes.iter().take(3).cloned().collect();
        println!("    {:>4}  {:<60} {}", quienes.len(), reason, muestra.join(", "));
    }
    println!();
    println!("  y solo los de COMPUTO que no caben, de mas a menos:");
    let mut orden: Vec<_> = de_computo_no.into_iter().collect();
    orden.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    for (reason, quienes) in &orden {
        let muestra: Vec<_> = quienes.iter().take(3).cloned().collect();
        println!("    {:>4}  {:<60} {}", quienes.len(), reason, muestra.join(", "));
    }
    println!();
    println!("  el ORACULO sobre los que caben (un grupo, buffers a cero):");
    for (que, quienes) in &oraculo {
        let muestra: Vec<_> = quienes.iter().take(3).cloned().collect();
        println!("    {:>4}  {:<60} {}", quienes.len(), que, muestra.join(", "));
    }
    println!();
    println!("  familias (ficheros que la usan / instrucciones):");
    for f in Family::ALL {
        println!("    {:<16} {:>4} / {:>6}", format!("{:?}", f), con_familia[f.index()], familias[f.index()]);
    }
    let _ = Path::new(&carpeta);
}
