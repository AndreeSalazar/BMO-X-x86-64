//! **La 3060 contra el muestreador de la CPU** (P3b4c.8, 29-09). Las 96
//! muestras de `docs/metal/tex_cuda/SALIDA.TXT` son la 3060 MISMA
//! muestreando (CUDA, `tex2DLod`, lo midio el propietario en su Windows):
//! Point/Linear x Wrap/Mirror/Clamp/Border x 12 puntos de una 4x4 RGBA8
//! conocida. `bmo_proton_x::textura` es el que dibuja hoy cuando un PSO
//! muestrea; esto dice cuanto se parece a la tarjeta.

use bmo_proton_x::textura::{Direccion, Filtro, Muestreador, Textura};

const SALIDA: &str = include_str!("../../../../docs/metal/tex_cuda/SALIDA.TXT");

fn texeles() -> Vec<u32> {
    let mut t = Vec::new();
    for y in 0..4u32 {
        for x in 0..4u32 {
            t.push(x * 60 | (y * 60) << 8 | ((x + y) * 20) << 16 | 255 << 24);
        }
    }
    t
}

struct Muestra {
    linea: String,
    m: Muestreador,
    u: f32,
    v: f32,
    bits: [u32; 4],
}

fn muestras() -> Vec<Muestra> {
    SALIDA
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split_whitespace().collect();
            let filtro = if c[0] == "Point" { Filtro::Punto } else { Filtro::Lineal };
            let d = match c[1] {
                "Wrap" => Direccion::Repetir,
                "Mirror" => Direccion::Espejo,
                "Clamp" => Direccion::Sujetar,
                _ => Direccion::Borde,
            };
            let hex = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
            Muestra { linea: l.to_string(), m: Muestreador { filtro, u: d, v: d, borde: [0.0, 0.5, 1.0, 1.0] }, u: c[2].parse().unwrap(), v: c[3].parse().unwrap(), bits: [hex(c[8]), hex(c[9]), hex(c[10]), hex(c[11])] }
        })
        .collect()
}

#[test]
fn la_cpu_contra_la_3060() {
    let t = texeles();
    let tx = Textura::rgba(&t, 4, 4, false);
    let ms = muestras();
    assert_eq!(ms.len(), 96, "las 96 de la 3060");
    let (mut iguales, mut peor) = (0, 0f32);
    for s in &ms {
        let c = tx.muestrear(&s.m, s.u, s.v);
        let bits = c.map(f32::to_bits);
        if bits == s.bits {
            iguales += 1;
        } else {
            let d = (0..4).map(|k| (c[k] - f32::from_bits(s.bits[k])).abs()).fold(0f32, f32::max);
            peor = peor.max(d);
            eprintln!("DISTINTA  {}\n   cpu   {:?}", s.linea, bits.map(|b| format!("{b:#010x}")));
        }
    }
    eprintln!("la CPU iguala a la 3060 en {iguales} de 96; la peor, {peor} ({} de 255)", peor * 255.0);
    // El 29-09 la version de antes igualaba 60 (la peor, 1/255); con el
    // modelo medido (pesos de 8 bits, texeles de 16, borde cuantizado), las 96.
    assert_eq!(iguales, 96, "la CPU tiene que muestrear COMO la 3060, bit a bit");
}
