//! `inti-hex` -- ver `lib.rs`.
//!
//! ```text
//!    inti-hex volcar FICHERO.inti [-o SALIDA.hex]
//!    inti-hex leer   FICHERO.hex  [-o SALIDA.bin]
//!    inti-hex fagraw CUERPO.hex   [--entrada 0x0] [-o PAQUETE.hex] [--x86 INICIO.X86]
//! ```

use std::path::PathBuf;

fn uso() -> ! {
    eprintln!("uso: inti-hex volcar FICHERO.inti [-o SALIDA.hex]");
    eprintln!("     inti-hex leer   FICHERO.hex  [-o SALIDA.bin]");
    eprintln!("     inti-hex fagraw CUERPO.hex   [--entrada 0x0] [-o PAQUETE.hex] [--x86 INICIO.X86]");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (orden, fichero) = match (args.first(), args.get(1)) {
        (Some(o), Some(f)) => (o.as_str(), PathBuf::from(f)),
        _ => uso(),
    };
    let opcion = |nombre: &str| args.iter().position(|a| a == nombre).and_then(|i| args.get(i + 1)).cloned();
    let salida = opcion("-o");
    let texto = std::fs::read_to_string(&fichero).unwrap_or_else(|e| {
        eprintln!("no puedo leer {}: {e}", fichero.display());
        std::process::exit(1)
    });
    let nombre = fichero.display().to_string();
    let escribe = |bytes: &[u8], por_defecto: &str| match &salida {
        Some(p) => {
            std::fs::write(p, bytes).unwrap_or_else(|e| {
                eprintln!("no puedo escribir {p}: {e}");
                std::process::exit(1)
            });
            eprintln!("ok: {} bytes -> {p}", bytes.len());
        }
        None if por_defecto == "texto" => print!("{}", String::from_utf8_lossy(bytes)),
        None => eprintln!("ok: {} bytes (sin -o no se escriben)", bytes.len()),
    };
    match orden {
        "volcar" => {
            let c = bmo_inti_x86_64::cadena::compilar(&texto, &nombre, &bmo_mods::Roots::find()).unwrap_or_else(|f| {
                eprintln!("{f}");
                std::process::exit(1)
            });
            escribe(bmo_inti_hex::volcar(&c, &nombre).as_bytes(), "texto");
        }
        "leer" => match bmo_inti_hex::leer(&texto) {
            Ok(b) => escribe(&b, "bin"),
            Err(e) => {
                eprintln!("{nombre}, {e}");
                std::process::exit(1)
            }
        },
        "fagraw" => {
            let entrada = opcion("--entrada").map(|e| u32::from_str_radix(e.trim_start_matches("0x"), 16).unwrap_or_else(|_| uso())).unwrap_or(0);
            let cuerpo = bmo_inti_hex::leer(&texto).unwrap_or_else(|e| {
                eprintln!("{nombre}, {e}");
                std::process::exit(1)
            });
            let p = bmo_inti_hex::fagraw(&cuerpo, entrada).unwrap_or_else(|e| {
                eprintln!("{nombre}: {e}");
                std::process::exit(1)
            });
            if let Some(x86) = opcion("--x86") {
                std::fs::write(&x86, &p).unwrap_or_else(|e| {
                    eprintln!("no puedo escribir {x86}: {e}");
                    std::process::exit(1)
                });
                eprintln!("ok: {} bytes -> {x86}", p.len());
            }
            escribe(bmo_inti_hex::fagraw_hex(&p, &nombre).as_bytes(), "texto");
        }
        _ => uso(),
    }
}
