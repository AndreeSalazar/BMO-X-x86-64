//! **La matriz: el lector y el juez contra una carpeta de `.spv` de otros.**
//!
//! Herramienta del ANFITRION (usa `std` para leer ficheros; la biblioteca no:
//! sigue siendo `no_std` sin `alloc`, y esto la usa desde fuera). La conduce
//! `herramientas/censo_naga.py` con el banco de pruebas de Naga, que vive
//! FUERA del repo (`BMO-externo/naga-corpus`).
//!
//!     cargo run -q -p bmo-spirv-front --example censo -- <carpeta de .spv>
//!
//! Lo que dice: cuantos se leen, cuantos caben, y POR QUE no el resto,
//! ordenado por cuantas veces sale cada motivo. Es el numero que decide que
//! se amplia del subconjunto, en vez de ampliarlo a ojo.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use bmo_spirv_front::{censo, juzgar, leer, Familia, Motivo};

fn clave(m: &Motivo) -> String {
    // El motivo sin el numero de palabra, y con lo que lo distingue.
    match m {
        Motivo::FamiliaFuera { familia: Familia::Otro, codigo } => {
            format!("{}", bmo_spirv_front::fila(*codigo).map(|f| f.nombre).unwrap_or("?"))
        }
        Motivo::FamiliaFuera { familia, .. } => format!("familia: {}", familia.nombre()),
        Motivo::CapacidadFuera { capacidad } => format!("capacidad {}", capacidad),
        Motivo::ClaseFuera { clase } => format!("clase de almacenamiento {}", clase),
        Motivo::EtapaFuera { modelo } => format!("etapa {} (no es computo)", modelo),
        Motivo::ExtInstFuera { numero } => format!("GLSL.std.450 numero {} fuera", numero),
        Motivo::ExtInstLuego { numero } => format!("GLSL.std.450 numero {} (S3b)", numero),
        Motivo::ModoFuera { modo } => format!("modo de ejecucion {}", modo),
        Motivo::TipoFuera { porque } => format!("tipo: {}", porque),
        Motivo::BuiltInFuera { builtin } => format!("BuiltIn {}", builtin),
        Motivo::SinFila { codigo } => format!("lector: codigo {} sin fila", codigo),
        Motivo::TipoNoCuadra { codigo } | Motivo::IndiceFuera { codigo } | Motivo::FueraDeBloque { codigo } => {
            format!("{} ({})", m.nombre(), codigo)
        }
        otro => otro.nombre().to_string(),
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
    let mut familias = [0usize; 9];
    let mut con_familia = [0usize; 9];

    for p in &nombres {
        let nombre = p.file_stem().unwrap().to_string_lossy().to_string();
        let bytes = fs::read(p).unwrap();
        let bound = if bytes.len() >= 16 {
            u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize
        } else {
            0
        };
        let mut ids = vec![0u32; bound.clamp(1, 1 << 22)];
        let m = match leer(&bytes, &mut ids) {
            Ok(m) => m,
            Err(f) => {
                println!("  NO LEE  {:<40} {}", nombre, f);
                motivos.entry(format!("LECTOR -- {}", clave(&f.motivo))).or_default().push(nombre);
                continue;
            }
        };
        leidos += 1;
        let computo = m.entradas().iter().any(|e| e.modelo == 5);
        if computo {
            de_computo += 1;
        }
        let c = censo(&m);
        for (i, n) in c.por_familia.iter().enumerate() {
            familias[i] += n;
            if *n > 0 {
                con_familia[i] += 1;
            }
        }
        match juzgar(&m) {
            Ok(v) => {
                caben += 1;
                println!("  CABE    {:<40} {} bloques, {} funciones", nombre, v.bloques, v.funciones);
            }
            Err(f) => {
                println!("  NO      {:<40} {}", nombre, f);
                if computo {
                    de_computo_no.entry(clave(&f.motivo)).or_default().push(nombre.clone());
                }
                motivos.entry(clave(&f.motivo)).or_default().push(nombre);
            }
        }
    }

    println!();
    println!("  {} ficheros | {} se leen | {} traen una entrada de computo | {} CABEN", nombres.len(), leidos, de_computo, caben);
    println!();
    println!("  por que no (el PRIMER motivo de cada uno), de mas a menos:");
    let mut orden: Vec<_> = motivos.into_iter().collect();
    orden.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    for (motivo, quienes) in &orden {
        let muestra: Vec<_> = quienes.iter().take(3).cloned().collect();
        println!("    {:>4}  {:<60} {}", quienes.len(), motivo, muestra.join(", "));
    }
    println!();
    println!("  y solo los de COMPUTO que no caben, de mas a menos:");
    let mut orden: Vec<_> = de_computo_no.into_iter().collect();
    orden.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    for (motivo, quienes) in &orden {
        let muestra: Vec<_> = quienes.iter().take(3).cloned().collect();
        println!("    {:>4}  {:<60} {}", quienes.len(), motivo, muestra.join(", "));
    }
    println!();
    println!("  familias (ficheros que la usan / instrucciones):");
    for f in Familia::TODAS {
        println!("    {:<16} {:>4} / {:>6}", format!("{:?}", f), con_familia[f.indice()], familias[f.indice()]);
    }
    let _ = Path::new(&carpeta);
}
