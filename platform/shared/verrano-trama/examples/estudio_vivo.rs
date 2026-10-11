//! `cargo run --release -p bmo-verrano-trama --example estudio_vivo -- TIRA.ppm`:
//! ESTUDIO VIVO, la luz que se recuerda. La memoria de la luz se educa con
//! la camara en un sitio; la camara se MUEVE; y se comparan, en los
//! fotogramas 1, 4, 16 y 64 despues de moverse, la imagen SIN memoria
//! (arriba), CON ella (en medio) y con ella y COMPLETADA (VE5, abajo: el
//! filtro que quita el grano guiado por la superficie). Cada fotograma
//! calcula UN CUARTO de los pixeles, repartido entre todos los nucleos, y
//! completar tambien se reparte; dice cuanto tarda cada cosa.

use bmo_verrano_trama::estudio::{caja_de_cornell, pelicula, Camara, Escena};
use bmo_verrano_trama::estudio::V3;
use bmo_verrano_trama::vivo::{Memoria, Parte, Vivo};
use std::time::Instant;

const W: u32 = 320;
const H: u32 = 240;

/// Un fotograma con todos los nucleos: cada uno, un trozo de filas.
fn fotograma(v: &mut Vivo, e: &Escena, nucleos: u32) -> f32 {
    let t0 = Instant::now();
    let trozos: Vec<_> = std::thread::scope(|s| {
        let v = &*v;
        let hilos: Vec<_> = (0..nucleos).map(|k| s.spawn(move || v.calcula(e, k * H / nucleos, (k + 1) * H / nucleos, Parte::Cuarto))).collect();
        hilos.into_iter().map(|h| h.join().unwrap()).collect()
    });
    v.junta(trozos);
    t0.elapsed().as_secs_f32() * 1000.0
}

/// COMPLETAR con todos los nucleos: cada pasada, un trozo de filas cada
/// uno, y se espera a todos antes de la siguiente (lo mismo que hace la app).
fn completa(v: &Vivo, pasos: u32, nucleos: u32) -> (Vec<V3>, f32) {
    let t0 = Instant::now();
    let n = (W * H) as usize;
    let (mut c, mut sig) = (vec![[0.0; 3]; n], vec![[0.0; 3]; n]);
    let filas = (H + nucleos - 1) / nucleos;
    std::thread::scope(|s| {
        for (k, trozo) in c.chunks_mut((filas * W) as usize).enumerate() {
            let y0 = k as u32 * filas;
            s.spawn(move || v.separa(y0, y0 + filas, trozo));
        }
    });
    let mut t = v.tolerancia();
    for paso in 0..pasos {
        let lee = &c;
        std::thread::scope(|s| {
            for (k, trozo) in sig.chunks_mut((filas * W) as usize).enumerate() {
                let y0 = k as u32 * filas;
                s.spawn(move || v.alisa(paso, t, lee, y0, y0 + filas, trozo));
            }
        });
        std::mem::swap(&mut c, &mut sig);
        t *= 0.5;
    }
    std::thread::scope(|s| {
        for (k, trozo) in c.chunks_mut((filas * W) as usize).enumerate() {
            let y0 = k as u32 * filas;
            s.spawn(move || v.vuelve(y0, y0 + filas, trozo));
        }
    });
    (c, t0.elapsed().as_secs_f32() * 1000.0)
}

fn main() {
    let salida = std::env::args().nth(1).unwrap_or_else(|| "estudio_vivo.ppm".into());
    let nucleos = std::thread::available_parallelism().map_or(4, |n| n.get()) as u32;
    let (objetos, luz, camara) = caja_de_cornell();
    let antes = Escena { objetos: &objetos, luz, camara, mallas: &[] };
    let despues = Escena { objetos: &objetos, luz, camara: Camara { ojo: [0.7, 1.25, 3.1], mira: [0.0, 0.85, 0.0], ..camara }, mallas: &[] };
    // educar la memoria con la camara de ANTES
    let mut con = Vivo::nuevo(W, H, Some(Memoria::nueva(18, 0.05, 8)));
    for _ in 0..64 {
        fotograma(&mut con, &antes, nucleos);
    }
    eprintln!("memoria educada: {} celdas", con.memoria.as_ref().unwrap().llenas());
    // la camara se mueve: la imagen se olvida, la memoria NO
    con.olvida_imagen();
    con.prepara_guia(&despues);
    let mut sin = Vivo::nuevo(W, H, None);
    sin.fotograma = con.fotograma;
    let mut tira = vec![0u32; (4 * W * 3 * H) as usize];
    let (mut ms_sin, mut ms_con, mut ms_completa) = (0.0, 0.0, 0.0);
    let mut col = 0;
    for f in 1..=64u32 {
        ms_sin += fotograma(&mut sin, &despues, nucleos);
        ms_con += fotograma(&mut con, &despues, nucleos);
        let (completada, ms) = completa(&con, 3, nucleos);
        ms_completa += ms;
        if [1, 4, 16, 64].contains(&f) {
            for (fila, img) in [(0, sin.imagen()), (1, con.imagen()), (2, completada)] {
                for (i, l) in img.iter().enumerate() {
                    let (x, y) = (i as u32 % W, i as u32 / W);
                    tira[((fila * H + y) * 4 * W + col * W + x) as usize] = pelicula(*l, 1.0);
                }
            }
            col += 1;
        }
    }
    eprintln!("{W}x{H}, un cuarto de los pixeles por fotograma, {nucleos} nucleos: sin memoria {:.1} ms/fotograma, con memoria {:.1} ms/fotograma; completar (3 pasadas) {:.1} ms", ms_sin / 64.0, ms_con / 64.0, ms_completa / 64.0);
    let mut b = format!("P6 {} {} 255\n", 4 * W, 3 * H).into_bytes();
    for p in &tira {
        b.extend([(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
    }
    std::fs::write(&salida, b).unwrap();
    eprintln!("-> {salida}");
}
