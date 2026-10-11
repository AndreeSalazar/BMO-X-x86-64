//! `cargo run --release -p bmo-verrano-trama --example estudio -- SALIDA.ppm [ancho] [alto] [muestras] [cornell|materiales|mallas|piel]`:
//! la caja de Cornell de VERRANO ESTUDIO (o la sala de materiales, VE4a, la
//! de mallas, VE3, o la de piel y niebla, VE4b),
//! con TODOS los nucleos de la CPU (cada uno, sus filas), a un PPM.

use bmo_verrano_trama::estudio::{caja_de_cornell, pelicula, sala_de_mallas, sala_de_materiales, sala_de_piel, Escena};
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let salida = a.get(1).cloned().unwrap_or_else(|| "estudio.ppm".into());
    let ancho: u32 = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(640);
    let alto: u32 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(480);
    let muestras: u32 = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(256);
    let (cornell, sala, mallas, piel) = (caja_de_cornell(), sala_de_materiales(), sala_de_mallas(), sala_de_piel());
    let e = match a.get(5).map(String::as_str) {
        Some("materiales") => Escena { objetos: &sala.0, luz: sala.1, camara: sala.2, mallas: &[], niebla: None },
        Some("piel") => Escena { objetos: &piel.0, luz: piel.1, camara: piel.2, mallas: &[], niebla: Some(piel.3) },
        Some("mallas") => Escena { objetos: &mallas.0, luz: mallas.1, camara: mallas.2, mallas: &mallas.3, niebla: None },
        _ => Escena { objetos: &cornell.0, luz: cornell.1, camara: cornell.2, mallas: &[], niebla: None },
    };
    let nucleos = std::thread::available_parallelism().map_or(4, |n| n.get());
    let t0 = Instant::now();
    let mut px = vec![0u32; (ancho * alto) as usize];
    // cada nucleo, sus filas (una de cada `nucleos`): un pixel no mira a otro
    let mut repartos: Vec<Vec<(u32, &mut [u32])>> = (0..nucleos).map(|_| Vec::new()).collect();
    for (y, fila) in px.chunks_mut(ancho as usize).enumerate() {
        repartos[y % nucleos].push((y as u32, fila));
    }
    std::thread::scope(|s| {
        for filas in repartos {
            let e = &e;
            s.spawn(move || {
                for (y, fila) in filas {
                    for (x, p) in fila.iter_mut().enumerate() {
                        *p = pelicula(e.pixel(x as u32, y, ancho, alto, muestras), 1.0);
                    }
                }
            });
        }
    });
    let mut b = format!("P6 {ancho} {alto} 255\n").into_bytes();
    for p in &px {
        b.extend([(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
    }
    std::fs::write(&salida, b).unwrap();
    eprintln!("{ancho}x{alto}, {muestras} muestras por pixel, {nucleos} nucleos: {:.1} s -> {salida}", t0.elapsed().as_secs_f32());
}
