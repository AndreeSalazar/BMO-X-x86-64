//! `cargo run --release -p bmo-verrano-trama --example completa_prueba`: lo
//! que COMPLETAR (VE5) le quita al error contra el juez (ESTUDIO entero, 256
//! muestras), con 1, 4 y 16 muestras por pixel y de 1 a 4 pasadas.

use bmo_verrano_trama::estudio::{caja_de_cornell, Escena};
use bmo_verrano_trama::vivo::{Parte, Vivo};
fn main() {
    let (o, luz, camara) = caja_de_cornell();
    let e = Escena { objetos: &o, luz, camara, mallas: &[], niebla: None };
    for (w, h) in [(32u32, 24u32), (160, 120)] {
        let juez: Vec<_> = (0..w * h).map(|i| e.pixel(i % w, i / w, w, h, 256)).collect();
        let err = |a: &[[f32; 3]]| a.iter().zip(&juez).map(|(p, q)| (0..3).map(|k| (p[k].min(1.5) - q[k].min(1.5)).abs()).sum::<f32>()).sum::<f32>() / (a.len() * 3) as f32;
        for spp in [1, 4, 16] {
            let mut v = Vivo::nuevo(w, h, None);
            v.prepara_guia(&e);
            for _ in 0..spp { v.paso(&e, Parte::Todos); }
            print!("{w}x{h} {spp:>2} spp: cruda {:.4}", err(&v.imagen()));
            for p in [1, 2, 3, 4] { print!("  a-trous{p} {:.4}", err(&v.completa(p))); }
            println!();
        }
    }
}
