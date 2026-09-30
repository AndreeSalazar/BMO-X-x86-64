//! `titan` -- the tool (TITAN_MAESTRO 4.1). Today, level 0:
//!
//! ```text
//!    titan check FICHERO.titan    bien, o el mensaje de 4 partes (sale con 1)
//!    titan arbol FICHERO.titan    el arbol que entendio el frontend
//! ```
//!
//! `build` and `run` come with the emitter (T3).

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(order), Some(file)) = (args.first(), args.get(1)) else {
        eprintln!("uso: titan check FICHERO.titan | titan arbol FICHERO.titan");
        return ExitCode::from(2);
    };
    let src = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("titan: no pude leer {}: {}", file, e);
            return ExitCode::from(2);
        }
    };
    match (order.as_str(), bmo_titan_front::compile(&src)) {
        (_, Err(m)) => {
            print!("{}", m.render(file, &src));
            ExitCode::from(1)
        }
        ("check", Ok(p)) => {
            let calls: usize = p.functions.iter().map(|f| f.body.len()).sum();
            let plural = if calls == 1 { "llamada" } else { "llamadas" };
            println!("bien  {}  -- mod {}, {} fn, {} {}", file, p.module, p.functions.len(), calls, plural);
            ExitCode::SUCCESS
        }
        ("arbol", Ok(p)) => {
            print!("{}", p.show());
            ExitCode::SUCCESS
        }
        (other, Ok(_)) => {
            eprintln!("titan: no conozco `{}` (check, arbol)", other);
            ExitCode::from(2)
        }
    }
}
