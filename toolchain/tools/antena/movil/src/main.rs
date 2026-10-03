//! `antena` -- la ANTENA en Rust (AO0). Las mismas ordenes que `antena.py`:
//!
//! ```text
//!     antena --carpeta <dir> --permitir <IP de BMO-X> [--escuchar <IP>] [--puerto 7117] [--nombre honor]
//! ```

use bmo_antena_movil::{servir, Ajuste};
use std::net::{IpAddr, TcpListener};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let valor = |nombre: &str| args.iter().position(|a| a == nombre).and_then(|i| args.get(i + 1)).cloned();
    let (Some(carpeta), Some(permitir)) = (valor("--carpeta"), valor("--permitir")) else {
        eprintln!("uso: antena --carpeta <dir> --permitir <IP> [--escuchar <IP>] [--puerto 7117] [--nombre antena]");
        return ExitCode::from(2);
    };
    let Ok(permitir) = permitir.parse::<IpAddr>() else {
        eprintln!("antena: --permitir no es una IP");
        return ExitCode::from(2);
    };
    let puerto = valor("--puerto").and_then(|p| p.parse::<u16>().ok()).unwrap_or(bmo_antena::PUERTO);
    let escuchar = valor("--escuchar").unwrap_or_else(|| "0.0.0.0".into());
    let a = Ajuste {
        carpeta: carpeta.into(),
        permitir,
        nombre: valor("--nombre").unwrap_or_else(|| "antena".into()),
        ffmpeg: std::env::var("ANTENA_FFMPEG").unwrap_or_else(|_| "ffmpeg".into()),
    };
    if !a.carpeta.is_dir() {
        eprintln!("antena: no existe la carpeta");
        return ExitCode::from(1);
    }
    let escucha = match TcpListener::bind((escuchar.as_str(), puerto)) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("antena: no puedo escuchar en el {puerto}: {e} (otra antena ya escucha?)");
            return ExitCode::from(1);
        }
    };
    let lista = bmo_antena_movil::carpeta::catalogo(&a.carpeta);
    let videos = lista.iter().filter(|(i, _)| i.starts_with('v')).count();
    println!("antena (Rust): escuchando en el puerto {puerto}, {videos} videos y {} paginas en la carpeta", lista.len() - videos);
    match servir(&escucha, &a, None) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("antena: {e}");
            ExitCode::from(1)
        }
    }
}
