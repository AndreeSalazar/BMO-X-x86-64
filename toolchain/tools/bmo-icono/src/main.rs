//! `bmo-icono ENTRADA.(svg|png|jpg) -o SALIDA.bico [--lado N]`
//!
//! La cara de una app, lista para `bmo-pack -r icono=SALIDA.bico`. Ver la
//! cabecera de la libreria.

use std::process::ExitCode;

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut entrada = None;
    let mut salida = None;
    let mut lado = bmo_icono::LADO;
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "-o" => {
                salida = a.get(i + 1).cloned();
                i += 1;
            }
            "--lado" => {
                lado = match a.get(i + 1).and_then(|n| n.parse().ok()) {
                    Some(n) => n,
                    None => return mal("`--lado` pide un numero"),
                };
                i += 1;
            }
            x if entrada.is_none() => entrada = Some(x.to_string()),
            x => return mal(&format!("no conozco `{x}`")),
        }
        i += 1;
    }
    let (Some(entrada), Some(salida)) = (entrada, salida) else {
        return mal("uso: bmo-icono ENTRADA.(svg|png|jpg) -o SALIDA.bico [--lado N]");
    };
    let bytes = match std::fs::read(&entrada) {
        Ok(b) => b,
        Err(e) => return mal(&format!("no pude leer {entrada}: {e}")),
    };
    match bmo_icono::icono(&bytes, lado) {
        Ok((bico, saltado)) => {
            for s in &saltado {
                eprintln!("[!] {entrada}: <{s}> no se sabe pintar y no sale en el icono");
            }
            if let Err(e) = std::fs::write(&salida, &bico) {
                return mal(&format!("no pude escribir {salida}: {e}"));
            }
            println!("  {entrada} -> {salida}  ({lado} x {lado}, {} B)", bico.len());
            ExitCode::SUCCESS
        }
        Err(e) => mal(&format!("{entrada}: {e}")),
    }
}

fn mal(m: &str) -> ExitCode {
    eprintln!("[X] {m}");
    ExitCode::FAILURE
}
