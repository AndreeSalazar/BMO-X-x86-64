//! `espejo-cara`: comparar una app con su maqueta, y generar su tinta. Ver
//! `lib.rs` y `ESPEJO_CARA.md`.

use bmo_espejo_cara::{comparar, mapa, png, por_ciento, tinta, COLUMNAS, FILAS};

fn uso() -> ! {
    eprintln!("uso: espejo-cara comparar <maqueta.png> <app.png> [--mapa diff.png]");
    eprintln!("     espejo-cara tinta <maqueta.html> [--comprobar tinta.rs]");
    std::process::exit(2)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("comparar") if a.len() >= 3 => {
            let (m, ap) = match (png::leer(&a[1]), png::leer(&a[2])) {
                (Ok(m), Ok(ap)) => (m, ap),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("NO: {e}");
                    std::process::exit(1)
                }
            };
            if (m.ancho, m.alto) != (ap.ancho, ap.alto) {
                println!("[!] medidas distintas: maqueta {}x{}, app {}x{} -- se compara lo comun", m.ancho, m.alto, ap.ancho, ap.alto);
            }
            let inf = comparar(&m, &ap);
            println!("ESPEJO DE CARA  {} contra {}", a[1], a[2]);
            println!("  igual       {}   (pixel a pixel, tolerancia {})", por_ciento(inf.igual), bmo_espejo_cara::TOLERANCIA);
            println!("  parecido    {}   (con un pixel de holgura)", por_ciento(inf.parecido));
            println!("  las zonas peores (rejilla {COLUMNAS} x {FILAS}, x y de su esquina):");
            for &(c, f, p) in inf.zonas.iter().take(6) {
                println!("    x {:4}  y {:4}   {}", c * inf.ancho / COLUMNAS, f * inf.alto / FILAS, por_ciento(p));
            }
            if let Some(k) = a.iter().position(|s| s == "--mapa") {
                if let Some(ruta) = a.get(k + 1) {
                    match png::escribir(ruta, &mapa(&ap, &inf)) {
                        Ok(()) => println!("  el mapa: {ruta}"),
                        Err(e) => eprintln!("NO: {e}"),
                    }
                }
            }
        }
        Some("tinta") if a.len() >= 2 => {
            let html = std::fs::read_to_string(&a[1]).unwrap_or_else(|e| {
                eprintln!("NO: {}: {e}", a[1]);
                std::process::exit(1)
            });
            let nombre = std::path::Path::new(&a[1]).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let t = tinta(&html, &nombre);
            if let Some(k) = a.iter().position(|s| s == "--comprobar") {
                let ruta = a.get(k + 1).cloned().unwrap_or_else(|| uso());
                let hay = std::fs::read_to_string(&ruta).unwrap_or_default();
                if hay != t {
                    eprintln!("NO: {ruta} no es la tinta de {} -- regenerar con `espejo-cara tinta {} > {ruta}`", a[1], a[1]);
                    std::process::exit(1);
                }
                println!("ok: {ruta} es la tinta de {}", a[1]);
            } else {
                print!("{t}");
            }
        }
        _ => uso(),
    }
}
