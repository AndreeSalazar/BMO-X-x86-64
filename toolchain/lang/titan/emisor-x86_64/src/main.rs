//! `titan` -- la herramienta (TITAN_MAESTRO 4.1). Hoy, el nivel 0:
//!
//! ```text
//!    titan check FICHERO.titan              bien, o el mensaje de 4 partes
//!    titan arbol FICHERO.titan              el arbol que entendio el frontend
//!    titan ir    FICHERO.titan              lo que recibe el emisor
//!    titan build FICHERO.titan [-o X.bex]   el .bex, que ya paso el gate
//! ```
//!
//! ** Vive en el crate del EMISOR desde T3 (2026-10-04) por el mismo motivo
//! que `inti`: `build` produce bytes de UNA maquina, y el frontend tiene
//! prohibido nombrar ninguna.
//!
//! [!] **Esto no ejecuta nada.** Escribe un `.bex`, y quien lo ejecuta es el
//! kernel: `run titan/hola.bex` en la consola (F12). `titan run` no existe en
//! el anfitrion (TITAN_MAESTRO 4.1): llega con T6, dentro de F1, y aun alli
//! sera el ESCRITORIO quien lance, no el taller.
//!
//! Salidas: 0 bien, 1 el fuente tiene un NO (el mensaje de 4 partes), 2 la
//! orden esta mal o el disco no deja.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USE: &str = "uso: titan check|arbol|ir FICHERO.titan\n     titan build FICHERO.titan [-o SALIDA.bex]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(order), Some(file)) = (args.first(), args.get(1)) else {
        eprintln!("{}", USE);
        return ExitCode::from(2);
    };
    let out = match args.get(2).map(String::as_str) {
        None => None,
        Some("-o") if order == "build" => match args.get(3) {
            Some(p) if args.len() == 4 => Some(PathBuf::from(p)),
            _ => return fail("`-o` pide una ruta detras, y nada mas"),
        },
        Some(other) => return fail(&format!("no conozco `{}` aqui\n{}", other, USE)),
    };
    let src = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => return fail(&format!("no pude leer {}: {}", file, e)),
    };

    match order.as_str() {
        "check" | "arbol" | "ir" => {
            let p = match bmo_titan_front::compile(&src) {
                Ok(p) => p,
                Err(m) => return no(&m, file, &src, false),
            };
            match order.as_str() {
                "check" => {
                    let calls: usize = p.functions.iter().map(|f| f.body.len()).sum();
                    let plural = if calls == 1 { "llamada" } else { "llamadas" };
                    println!("bien  {}  -- mod {}, {} fn, {} {}", file, p.module, p.functions.len(), calls, plural);
                }
                "arbol" => print!("{}", p.show()),
                _ => print!("{}", bmo_titan_front::ir::lower(&p).show()),
            }
            ExitCode::SUCCESS
        }
        "build" => {
            // El NOMBRE del fichero va al manifiesto, nunca la ruta de esta
            // maquina: dos builds del mismo fuente dan los mismos bytes.
            let name = Path::new(file).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let bex = match bmo_titan_x86_64::build(&src, &name) {
                Ok(b) => b,
                Err(bmo_titan_x86_64::Failure::Source(m)) => return no(&m, file, &src, true),
                Err(bmo_titan_x86_64::Failure::Gate(why)) => {
                    // No es un fallo del programa: es de este compilador.
                    return fail(&format!("el .bex no paso el gate ({}): es un fallo del compilador, no de {}", why, file));
                }
            };
            let dst = out.unwrap_or_else(|| Path::new(file).with_extension("bex"));
            if let Err(e) = std::fs::write(&dst, &bex) {
                return fail(&format!("no pude escribir {}: {}", dst.display(), e));
            }
            println!("ok: {} bytes -> {}", bex.len(), dst.display());
            ExitCode::SUCCESS
        }
        other => fail(&format!("no conozco `{}` (check, arbol, ir, build)", other)),
    }
}

fn no(m: &bmo_titan_front::Message, file: &str, src: &str, building: bool) -> ExitCode {
    print!("{}", m.render(file, src));
    if building {
        println!("no se ha escrito nada.");
    }
    ExitCode::from(1)
}

fn fail(why: &str) -> ExitCode {
    eprintln!("titan: {}", why);
    ExitCode::from(2)
}
