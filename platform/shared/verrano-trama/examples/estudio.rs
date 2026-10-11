//! `cargo run --release -p bmo-verrano-trama --example estudio -- SALIDA.ppm [ancho] [alto] [muestras]`:
//! la caja de Cornell de VERRANO ESTUDIO, con TODOS los nucleos de la CPU
//! (cada uno, sus filas), a un PPM.

use bmo_verrano_trama::estudio::{caja_de_cornell, pelicula, Escena};
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let salida = a.get(1).cloned().unwrap_or_else(|| "estudio.ppm".into());
    let ancho: u32 = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(640);
    let alto: u32 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(480);
    let muestras: u32 = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(256);
    let (objetos, luz, camara) = caja_de_cornell();
    let e = Escena { objetos: &objetos, luz, camara };
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
