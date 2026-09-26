//! **`bmo-bsf-x86-64`** -- fabrica un BSF de unos `.spv`, y lo muestra.
//!
//! ```text
//!   bmo-bsf-x86-64 fabricar mandelbrot.spv [nombre=otro.spv ...] -o sombras.bsf
//!   bmo-bsf-x86-64 ver sombras.bsf
//! ```
//!
//! Al fabricar paga todas las capas, la profunda incluida: relee cada SPIR-V
//! del fichero ya escrito, compara su interfaz con la tabla y re-emite el
//! codigo. Lo que sale de aqui ya se comprobo como lo comprobaria el mas
//! desconfiado de los consumidores.
//!
//! Aqui solo hay argumentos y ficheros: el formato vive en la biblioteca.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use bmo_bsf::*;
use bmo_bsf_x86_64::*;
use bmo_spirv_front::read;
use bmo_spirv_x86_64::{emit, tables_words};

fn uso() -> ExitCode {
    eprintln!(
        "uso:
  bmo-bsf-x86-64 fabricar <a.spv> [nombre=<b.spv> ...] -o <salida.bsf>
  bmo-bsf-x86-64 ver <fichero.bsf>

  El nombre de un modulo es el del fichero sin extension, o el que se diga
  con nombre=ruta. Es como lo pide el programa."
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("fabricar") => fabricar(&args[1..]),
        Some("ver") if args.len() == 2 => ver(Path::new(&args[1])),
        _ => uso(),
    }
}

struct Pieza {
    nombre: String,
    spirv: Vec<u8>,
    hechos: Facts,
    code: Vec<u8>,
    program: bmo_spirv_x86_64::Program,
}

fn fabricar(args: &[String]) -> ExitCode {
    let mut entradas: Vec<(String, PathBuf)> = Vec::new();
    let mut salida: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-o" {
            let Some(o) = args.get(i + 1) else { return uso() };
            salida = Some(PathBuf::from(o));
            i += 2;
            continue;
        }
        let (nombre, ruta) = match args[i].split_once('=') {
            Some((n, r)) => (n.to_string(), PathBuf::from(r)),
            None => {
                let r = PathBuf::from(&args[i]);
                (r.file_stem().unwrap_or_default().to_string_lossy().into_owned(), r)
            }
        };
        entradas.push((nombre, ruta));
        i += 1;
    }
    let Some(salida) = salida else { return uso() };
    if entradas.is_empty() {
        return uso();
    }

    // Traducir cada uno. El orden de los modulos es el de la linea de
    // ordenes, y el formato no mete nada mas: mismos argumentos, mismos bytes.
    let mut piezas = Vec::new();
    for (nombre, ruta) in &entradas {
        let spirv = match std::fs::read(ruta) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[X] {}: {}", ruta.display(), e);
                return ExitCode::FAILURE;
            }
        };
        let mut ids = vec![0u32; ids_de(&spirv)];
        let m = match read(&spirv, &mut ids) {
            Ok(m) => m,
            Err(e) => return fallo(ruta, e),
        };
        let hechos = match facts(&m) {
            Ok(h) => h,
            Err(e) => return fallo(ruta, e),
        };
        let mut tablas = vec![0u32; tables_words(&m)];
        let mut code = vec![0u8; 1 << 20];
        let program = match emit(&m, &mut tablas, &mut code) {
            Ok(p) => p,
            Err(e) => return fallo(ruta, e),
        };
        code.truncate(program.code_len);
        piezas.push(Pieza { nombre: nombre.clone(), spirv: spirv.clone(), hechos, code, program });
    }

    let mut ranuras = vec![[0xFFu8; MAX_BINDINGS]; piezas.len()];
    let mut objetivos = Vec::new();
    for (p, s) in piezas.iter().zip(ranuras.iter_mut()) {
        match x86_64_target(&p.program, &p.code, p.hechos.bindings(), s) {
            Ok(t) => objetivos.push([t]),
            Err(f) => {
                eprintln!("[X] {}: {}", p.nombre, f);
                return ExitCode::FAILURE;
            }
        }
    }
    let modulos: Vec<ModuleIn> = piezas
        .iter()
        .zip(&objetivos)
        .map(|(p, t)| ModuleIn {
            model: p.hechos.model,
            name: p.nombre.as_bytes(),
            local_size: p.hechos.local_size,
            capabilities: p.hechos.capabilities,
            caps_high: p.hechos.caps_high,
            spirv: &p.spirv,
            bindings: p.hechos.bindings(),
            targets: t,
        })
        .collect();
    let n = match size(&modulos) {
        Ok(n) => n,
        Err(f) => {
            eprintln!("[X] {}", f);
            return ExitCode::FAILURE;
        }
    };
    let mut bsf = vec![0u8; n];
    if let Err(f) = write(&modulos, &mut bsf) {
        eprintln!("[X] {}", f);
        return ExitCode::FAILURE;
    }
    if let Err(f) = todas_las_capas(&bsf) {
        eprintln!("[X] el BSF recien escrito no pasa: {}", f);
        return ExitCode::FAILURE;
    }
    if let Err(e) = std::fs::write(&salida, &bsf) {
        eprintln!("[X] {}: {}", salida.display(), e);
        return ExitCode::FAILURE;
    }
    println!("[OK] {} -- {} modulos, {} bytes, las cinco capas", salida.display(), piezas.len(), n);
    ExitCode::SUCCESS
}

/// Las cinco capas, la profunda y la reproduccion incluidas.
fn todas_las_capas(bytes: &[u8]) -> Result<Bsf<'_>, Fault> {
    let bsf = Bsf::parse(bytes)?;
    bsf.verify_all()?;
    for i in 0..bsf.module_count() {
        let spirv = bsf.module(i).spirv()?;
        let mut ids = vec![0u32; ids_de(spirv)];
        bsf.deep(i, &mut ids)?;
        let mut tablas = vec![0u32; 2 * ids.len()];
        let mut code = vec![0u8; 1 << 20];
        reproducir(&bsf, i, &mut ids, &mut tablas, &mut code)?;
    }
    Ok(bsf)
}

/// La tabla de ids que pide el lector: `bound` de la cabecera, con un techo.
fn ids_de(spirv: &[u8]) -> usize {
    let bound = spirv.get(12..16).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0);
    (bound as usize).clamp(1, 1 << 22)
}

fn fallo(ruta: &Path, e: bmo_spirv_front::Error) -> ExitCode {
    eprintln!("[X] {}: {}", ruta.display(), e);
    ExitCode::FAILURE
}

fn ver(ruta: &Path) -> ExitCode {
    let bytes = match std::fs::read(ruta) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[X] {}: {}", ruta.display(), e);
            return ExitCode::FAILURE;
        }
    };
    let bsf = match todas_las_capas(&bytes) {
        Ok(b) => b,
        Err(f) => {
            eprintln!("[X] {}: {}", ruta.display(), f);
            return ExitCode::FAILURE;
        }
    };
    println!("{}: BSF v{}, {} bytes, {} modulos -- pasa las cinco capas", ruta.display(), VERSION, bytes.len(), bsf.module_count());
    for m in bsf.modules() {
        let ls = m.local_size();
        println!(
            "  {}  etapa {}  LocalSize {}x{}x{}  SPIR-V {} bytes  hash {}",
            String::from_utf8_lossy(m.name()),
            m.model(),
            ls[0],
            ls[1],
            ls[2],
            m.spirv_len(),
            corto(m.spirv_hash())
        );
        for b in m.bindings() {
            let acceso = match b.access {
                0 => "nada",
                READS => "lee",
                WRITES => "escribe",
                _ => "lee+escribe",
            };
            let clase = if b.storage { "storage" } else { "uniform" };
            let forma = if b.stride > 0 { format!("{} + n*{}", b.base_bytes, b.stride) } else { format!("{}", b.base_bytes) };
            println!("    set {} binding {}  {:7}  {:11}  bytes {}", b.set, b.binding, clase, acceso, forma);
        }
        for t in m.targets() {
            println!(
                "    objetivo {}/{}  {}  codigo {} bytes  init +{} main +{}  marco {} palabras  ranuras {:?}  hash {}",
                t.kind(),
                t.abi(),
                String::from_utf8_lossy(t.emitter()),
                t.code_len(),
                t.init(),
                t.main(),
                t.frame_words(),
                t.slots(),
                corto(t.code_hash())
            );
        }
    }
    ExitCode::SUCCESS
}

fn corto(h: &[u8; 32]) -> String {
    h[..6].iter().map(|b| format!("{:02x}", b)).collect()
}
