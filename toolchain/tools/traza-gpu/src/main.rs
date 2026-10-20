//! `traza-gpu` -- ver `lib.rs`.
//!
//! ```text
//!    traza-gpu receta  TRAZA.txt            hasta que arranca el booter
//!    traza-gpu todo    TRAZA.txt            todos los accesos a la BAR0
//!    traza-gpu compara BUENA.txt MALA.txt [--lecturas]
//! ```

fn uso() -> ! {
    eprintln!("uso: traza-gpu receta TRAZA.txt | todo TRAZA.txt | compara A.txt B.txt [--lecturas]");
    std::process::exit(2)
}

fn leer(p: &str) -> Vec<bmo_traza_gpu::Acceso> {
    let t = std::fs::read_to_string(p).unwrap_or_else(|e| {
        eprintln!("no puedo leer {p}: {e}");
        std::process::exit(1)
    });
    bmo_traza_gpu::accesos(&t).unwrap_or_else(|e| {
        eprintln!("{p}: {e}");
        std::process::exit(1)
    })
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match (a.first().map(String::as_str), a.get(1), a.get(2)) {
        (Some(o @ ("receta" | "todo")), Some(p), _) => {
            let t = leer(p);
            let (r, llego) = if o == "receta" { bmo_traza_gpu::receta(&t) } else { (&t[..], false) };
            let t0 = r.first().map_or(0, |x| x.us);
            for x in r {
                println!("{}", bmo_traza_gpu::fila(x, t0));
            }
            if o == "receta" {
                eprintln!("{} accesos; {}", r.len(), if llego { "hasta que el SEC2 arranca (el booter)" } else { "la traza no llega a arrancar el booter" });
            }
        }
        (Some("compara"), Some(p), Some(q)) => {
            let (ta, tb) = (leer(p), leer(q));
            let (ra, rb) = (bmo_traza_gpu::receta(&ta).0, bmo_traza_gpu::receta(&tb).0);
            match bmo_traza_gpu::primera_diferencia(ra, rb, a.iter().any(|x| x == "--lecturas")) {
                None => println!("iguales: las mismas escrituras del arranque, en el mismo orden, hasta el booter"),
                Some((i, x, y)) => {
                    println!("la primera diferencia, en el acceso {i} del arranque:");
                    let ver = |z: Option<bmo_traza_gpu::Acceso>| z.map_or("(se acaba)".to_string(), |z| bmo_traza_gpu::fila(&z, 0));
                    println!("  {p}: {}", ver(x));
                    println!("  {q}: {}", ver(y));
                }
            }
        }
        _ => uso(),
    }
}
