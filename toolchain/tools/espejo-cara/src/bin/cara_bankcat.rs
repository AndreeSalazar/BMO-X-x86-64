//! **BANK CAT, pintado en el anfitrion** por SU codigo (`#[path]` a los
//! ficheros de la app, no una copia), en las escenas de la maqueta.
//!
//! `cara-bankcat <carpeta>` deja `cartera.png`, `rico.png`, `triste.png`,
//! `libro.png`, `cajero.png`, `reglas.png` y `minimo.png`.

extern crate alloc;

#[path = "../../../../../Ultra_userspace/apps/taller/src/canvas.rs"]
#[allow(dead_code)]
mod canvas;
#[path = "../../../../../Ultra_userspace/apps/ludoteca/src/mates.rs"]
#[allow(dead_code)]
mod mates;
#[path = "../../../../../Ultra_userspace/apps/hermes/src/piezas.rs"]
#[allow(dead_code)]
mod piezas;
#[path = "../../../../../Ultra_userspace/apps/bankcat/src/gato.rs"]
#[allow(dead_code)]
mod gato;
#[path = "../../../../../Ultra_userspace/apps/bankcat/src/pintar.rs"]
#[allow(dead_code)]
mod pintar;
#[path = "../../../../../Ultra_userspace/apps/bankcat/src/tinta.rs"]
#[allow(dead_code)]
mod tinta;

use bmo_bankcat::Estado;
use bmo_espejo_cara::{png, Imagen};
use pintar::{Asiento, Vista};

/// El `fmt_num` de `main.rs` de la app.
pub fn fmt_num(v: u64, out: &mut [u8]) -> usize {
    let s = v.to_string();
    let n = s.len().min(out.len());
    out[..n].copy_from_slice(&s.as_bytes()[..n]);
    n
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    // El libro de la maqueta: el saldo de bienvenida y lo que paso despues.
    let libro = vec![
        Asiento { cambio: 5000, estado: Estado::Hecho, saldo: 130000 },
        Asiento { cambio: -5997, estado: Estado::Hecho, saldo: 124003 },
        Asiento { cambio: 0, estado: Estado::SinSaldo, saldo: 124003 },
    ];
    let escenas: [(&str, u32, u32, usize, gato::Humor, u32, &[u8], Option<i64>, &[Asiento]); 8] = [
        // La de la foto de la maqueta: recien abierta, 1.250,00 y el libro vacio.
        ("cartera", 1240, 677, 0, gato::Humor::Quieto, 5000, b"\"Miau. Cada centimo, apuntado dos veces.\"", Some(125000), &[]),
        ("rico", 1240, 677, 0, gato::Humor::Rico, 250, b"\"Purr... huele a interes compuesto.\"", Some(124003), &libro),
        ("triste", 1240, 677, 0, gato::Humor::Triste, 400, b"\"NO: no hay saldo. El gato no fia.\"", Some(124003), &libro),
        ("libro", 1240, 677, 3, gato::Humor::Quieto, 5000, b"", Some(124003), &libro),
        ("cajero", 1240, 677, 4, gato::Humor::Quieto, 5000, b"", Some(124003), &libro),
        ("reglas", 1240, 677, 5, gato::Humor::Quieto, 5000, b"", Some(124003), &libro),
        ("mover", 1240, 677, 1, gato::Humor::Quieto, 5000, b"", Some(124003), &libro),
        ("minimo", 900, 600, 0, gato::Humor::Quieto, 5000, b"\"Miau.\"", None, &[]),
    ];
    for (nombre, w, h, sec, humor, t, frase, saldo, libro) in escenas {
        pintar::medir(w, h);
        let (w, h) = (w as usize, h as usize);
        let mut px = vec![0u32; w * h];
        let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
        let ms = 100_000u32;
        let v = Vista { sec, ms, desde_sec: 90_000, puntero: None, saldo, libro, humor, humor_desde: ms - t, frase, esperando: 0, aviso: b"" };
        pintar::pintar(&mut cv, &v);
        let im = Imagen { ancho: w, alto: h, px: px.iter().map(|p| p & 0x00FF_FFFF).collect() };
        let ruta = format!("{out}/{nombre}.png");
        if let Err(e) = png::escribir(&ruta, &im) {
            eprintln!("NO: {e}");
            std::process::exit(1);
        }
    }
    println!("ok: BANK CAT en {out}");
}
