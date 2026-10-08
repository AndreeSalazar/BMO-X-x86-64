//! `titan` -- la herramienta (TITAN_MAESTRO 4.1). Hoy, el nivel 0:
//!
//! ```text
//!    titan check FICHERO.titan              bien, o el mensaje de 4 partes: lo
//!                                           MISMO que diria build (LB1)
//!    titan arbol FICHERO.titan              el arbol que entendio el frontend
//!    titan ir    FICHERO.titan              lo que recibe el emisor
//!    titan build FICHERO.titan [-o X.bex]   el .bex, que ya paso el gate
//!    titan juez  X.bex [--concede a,b]      EL JUEZ DEL KERNEL, en el PC: lee
//!                                           el certificado del .bex y lo
//!                                           compara con lo pedido y lo dado
//! ```
//!
//!    titan sm86 FICHERO.titan [-o CARPETA]  cada gpu fn, como el SASS de la
//!                                           3060 que el juez acepto (nivel 11)
//!
//! Desde el nivel 9 el FICHERO es la raiz de un paquete: `titan check
//! flota/src/main.titan` sigue sus `mod` desde `flota/`, como F1.
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

const USE: &str = "uso: titan check|arbol|ir FICHERO.titan\n     titan build FICHERO.titan [-o SALIDA.bex]\n     titan sm86 FICHERO.titan [-o CARPETA]\n     titan juez X.bex [--concede screen,input,...]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(order), Some(file)) = (args.first(), args.get(1)) else {
        eprintln!("{}", USE);
        return ExitCode::from(2);
    };
    if order == "juez" {
        return kernel_judge(file, &args[2..]);
    }
    let out = match args.get(2).map(String::as_str) {
        None => None,
        Some("-o") if order == "build" || order == "sm86" => match args.get(3) {
            Some(p) if args.len() == 4 => Some(PathBuf::from(p)),
            _ => return fail("`-o` pide una ruta detras, y nada mas"),
        },
        Some(other) => return fail(&format!("no conozco `{}` aqui\n{}", other, USE)),
    };
    let src = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => return fail(&format!("no pude leer {}: {}", file, e)),
    };
    // ** EL PAQUETE (nivel 9): sus rutas empiezan en la carpeta de encima de
    // la del fichero (`asteroids/` para `asteroids/src/main.titan`), como en
    // F1. Un fichero que no dice `mod hijo` es un paquete de un modulo.
    let (dir, root) = package_of(file);
    let mut read = |p: &str| std::fs::read_to_string(dir.join(p)).ok();

    match order.as_str() {
        "check" | "arbol" | "ir" => {
            let p = match bmo_titan_front::compile_package(&root, &src, &mut read) {
                Ok(p) => p,
                Err(m) => return no(&m, &dir, &root, &src, false),
            };
            match order.as_str() {
                "check" => {
                    // ** `check` runs what `build` runs, without writing
                    // (LB1, 08-10): the whole frontend, the 3060's writer and
                    // its judge, and E1 -- so the two say the same thing by
                    // construction. It used to stop at the frontend and say
                    // `bien` to what `build` then refused.
                    match bmo_titan_x86_64::check_package(&root, &src, &mut read) {
                        Ok(()) => {}
                        Err(bmo_titan_x86_64::Failure::Source(m)) => return no(&m, &dir, &root, &src, false),
                        Err(bmo_titan_x86_64::Failure::Gate(why)) => return fail(&compiler_failure(&why, file)),
                    }
                    let calls: usize = p.functions.iter().map(|f| lines(&f.body)).sum();
                    let plural = if calls == 1 { "linea" } else { "lineas" };
                    println!("bien  {}  -- mod {}, {} fn, {} {}", file, p.module, p.functions.len(), calls, plural);
                }
                "arbol" => print!("{}", p.show()),
                _ => match bmo_titan_front::lower_package(&root, &src, &mut read) {
                    Ok(m) => print!("{}", m.show()),
                    Err(m) => return no(&m, &dir, &root, &src, false),
                },
            }
            ExitCode::SUCCESS
        }
        "build" => {
            // El NOMBRE del fichero va al manifiesto, nunca la ruta de esta
            // maquina: dos builds del mismo fuente dan los mismos bytes.
            let bex = match bmo_titan_x86_64::build_package(&root, &src, &mut read) {
                Ok(b) => b,
                Err(bmo_titan_x86_64::Failure::Source(m)) => return no(&m, &dir, &root, &src, true),
                Err(bmo_titan_x86_64::Failure::Gate(why)) => {
                    // No es un fallo del programa: es de este compilador.
                    return fail(&compiler_failure(&why, file));
                }
            };
            let dst = out.unwrap_or_else(|| Path::new(file).with_extension("bex"));
            if let Err(e) = std::fs::write(&dst, &bex) {
                return fail(&format!("no pude escribir {}: {}", dst.display(), e));
            }
            println!("ok: {} bytes -> {}", bex.len(), dst.display());
            ExitCode::SUCCESS
        }
        "sm86" => {
            // Nivel 11 (07-10, sin SPIR-V): cada gpu fn como el SASS de la 3060
            // -- el Programa de la casa, el emisor de SM86 y el juez --, ya
            // juzgado y comprobado. Junto al fuente, o en la carpeta de `-o`.
            // Desde LB3 (08-10), por PROMETEO: esta orden pide UNA tarjeta, la
            // 3060, porque su salida es su SASS. Desde el mismo dia, por su
            // APARATO exacto: el SASS sm_86 que deja es SOLO para la RTX 3060
            // 12G, y lo dice.
            let m = match bmo_titan_front::lower_package(&root, &src, &mut read) {
                Ok(m) => m,
                Err(m) => return no(&m, &dir, &root, &src, true),
            };
            let tarjeta = &bmo_tarjeta_rtx3060_12g::RTX_3060_12G;
            let ficha = bmo_prometeo::Tarjeta::ficha(tarjeta);
            let kernels = match bmo_titan_prometeo::kernels(&m, &[tarjeta]) {
                Ok(k) => k,
                // Lo que la libreria de la 3060 todavia no sabe: el NO del
                // programa, en su fichero y su linea (LB1).
                Err(bmo_titan_front::calc::DeviceNo::Limit(said)) => return no(&m.sources.locate(said), &dir, &root, &src, true),
                Err(bmo_titan_front::calc::DeviceNo::Failure(why)) => return fail(&format!("{} -- es del escritor de {} o de su juez, no de {}", why, ficha.nombre, file)),
            };
            if kernels.is_empty() {
                println!("{}: no tiene ninguna gpu fn", file);
                return ExitCode::SUCCESS;
            }
            let carpeta = out.unwrap_or_else(|| Path::new(file).parent().map(Path::to_path_buf).unwrap_or_default());
            // `-o CARPETA` la crea si no esta (08-10: antes decia "no pude
            // escribir ... No such file or directory").
            if let Err(e) = std::fs::create_dir_all(&carpeta) {
                return fail(&format!("no pude crear {}: {}", carpeta.display(), e));
            }
            for k in kernels {
                let dst = carpeta.join(format!("{}.sass", k.name.replace('.', "_")));
                if let Err(e) = std::fs::write(&dst, k.bytes()) {
                    return fail(&format!("no pude escribir {}: {}", dst.display(), e));
                }
                println!("ok: gpu fn {} -> {} ({} SOLO para {}: {} instrucciones, {} registros; su juez dijo que si)", k.name, dst.display(), ficha.lengua, ficha.aparato.modelo, k.viaje.instrucciones, k.viaje.registros);
            }
            ExitCode::SUCCESS
        }
        other => fail(&format!("no conozco `{}` (check, arbol, ir, build, sm86)", other)),
    }
}

/// **What the kernel's load gate will do (J2), done on the PC**: the
/// certificate of the `.bex`, against the `[permissions]` it asked for and
/// what the process would be granted (`--concede`). It never grants: it says
/// whether the three agree, and if not, WHICH door and WHICH line.
fn kernel_judge(file: &str, rest: &[String]) -> ExitCode {
    use bmo_titan_contrato::certificate::{judge, Certificate, Verdict};
    use bmo_titan_contrato::{Permission, Permissions};
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(e) => return fail(&format!("no pude leer {}: {}", file, e)),
    };
    if !bmo_verify::verify(&bytes).is_ok() {
        return fail(&format!("{} no pasa el gate: no es un .bex", file));
    }
    let Some(text) = bmo_verify::declaracion::manifiesto(&bytes) else {
        println!("{}: no trae manifiesto -- no es de TITAN++; el kernel lo juzga solo por sus capabilities, como a todos", file);
        return ExitCode::SUCCESS;
    };
    let cert = match Certificate::read(text) {
        Ok(c) => c,
        Err(e) => {
            println!("{}: el certificado no se lee ({:?}): no nombra nada, y el kernel juzga igual", file, e);
            return ExitCode::from(1);
        }
    };
    // What it asked for: the [permissions] of its own manifest.
    let mut asked = Permissions::NONE;
    let mut inside = false;
    for line in String::from_utf8_lossy(text).lines() {
        let l = line.trim();
        if l.starts_with('[') {
            inside = l == "[permissions]";
        } else if inside && !l.starts_with('#') {
            if let Some((k, v)) = l.split_once('=') {
                if let Some(p) = Permission::ALL.into_iter().find(|p| p.key() == k.trim()) {
                    if v.trim() != "false" {
                        asked = asked.with(p);
                    }
                }
            }
        }
    }
    let mut granted = Permissions::NONE;
    if let [flag, list] = rest {
        if flag == "--concede" {
            for k in list.split(',') {
                match Permission::ALL.into_iter().find(|p| p.key() == k.trim()) {
                    Some(p) => granted = granted.with(p),
                    None => return fail(&format!("`{}` no es un permiso (screen, input, sound, gpu, disk, net)", k)),
                }
            }
        }
    }
    for u in cert.uses() {
        println!("  usa {:<8} desde la linea {}", u.door.key(), u.line);
    }
    match judge(&cert, asked, granted) {
        Verdict::Agrees => {
            println!("de acuerdo: lo que dice que usa, lo que pidio y lo que se le da cuadran");
            ExitCode::SUCCESS
        }
        Verdict::Unasked(u) => {
            println!("NO: dice que usa `{}` (linea {}) y su manifiesto no lo pidio: este .bex no salio de un compilador honesto", u.door.key(), u.line);
            ExitCode::from(1)
        }
        Verdict::Ungranted(u) => {
            println!("NO: pidio `{}` y no se le concede: la puerta dira que no en la linea {}", u.door.key(), u.line);
            ExitCode::from(1)
        }
    }
}

/// Where a package starts, and the root's path from there: the folder above
/// the file's (`asteroids/` for `asteroids/src/main.titan`).
fn package_of(file: &str) -> (PathBuf, String) {
    let full = std::fs::canonicalize(file).unwrap_or_else(|_| PathBuf::from(file));
    let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match full.parent() {
        Some(here) => match (here.parent(), here.file_name()) {
            (Some(up), Some(d)) => (up.to_path_buf(), format!("{}/{}", d.to_string_lossy(), name)),
            _ => (here.to_path_buf(), name),
        },
        None => (PathBuf::from("."), name),
    }
}

/// The NO, drawn on the file of the package it is in.
fn no(m: &bmo_titan_front::Message, dir: &Path, root: &str, src: &str, building: bool) -> ExitCode {
    let file = m.file.clone().unwrap_or_else(|| root.to_string());
    let text = if file == root { src.to_string() } else { std::fs::read_to_string(dir.join(&file)).unwrap_or_default() };
    print!("{}", m.render(&file, &text));
    if building {
        println!("no se ha escrito nada.");
    }
    ExitCode::from(1)
}

fn fail(why: &str) -> ExitCode {
    eprintln!("titan: {}", why);
    ExitCode::from(2)
}

/// A failure of THIS compiler -- the gate, the 3060's writer or its judge, E1
/// --, said as one: it is never the program's (a known limit is a NO of the
/// program, with its four parts, and never comes here).
fn compiler_failure(why: &str, file: &str) -> String {
    format!("{} -- es un fallo del compilador, no de {}: avisa con este programa", why, file)
}

/// The lines of a body, the ones inside `if` and `else` included.
fn lines(body: &[bmo_titan_front::tree::Stmt]) -> usize {
    body.iter()
        .map(|st| match st {
            bmo_titan_front::tree::Stmt::If(i) => 1 + lines(&i.then) + lines(&i.other),
            _ => 1,
        })
        .sum()
}
