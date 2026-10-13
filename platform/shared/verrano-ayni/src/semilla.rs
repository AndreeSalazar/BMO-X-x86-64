//! **Con que nace UMA**: baldosas de muestra, hechas aqui con un dado de
//! semilla fija, y su leccion (`maestra::errores`). No es un juego: es lo
//! basico que cualquier juego tiene -- cielo, degradado, borde, textura,
//! rayas finas y sombra -- para que la red no empiece adivinando.

use alloc::vec::Vec;

use crate::maestra;
use crate::pacha::{rasgos, Baldosa, Extra, ENTRADAS, LADO, PIX};
use crate::uma::{Dado, Uma};

/// Lecciones al nacer: `MUESTRAS` baldosas distintas, vistas `VUELTAS`
/// veces. Las baldosas y su leccion se hacen UNA vez (es lo caro); repasarlas
/// es solo la red. Unos 26 ms en el anfitrion (`--release`), una vez al
/// arrancar.
pub const MUESTRAS: usize = 4_000;
pub const VUELTAS: u32 = 10;
pub const PASOS_DE_NACER: u32 = MUESTRAS as u32 * VUELTAS;
/// El paso al nacer (mas largo que en vivo: no hay juego que torcer).
pub const PASO_DE_NACER: f32 = 0.03;

/// Las clases de baldosa de muestra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    Plana,
    Degradado,
    Borde,
    Textura,
    Rayas,
}

pub const CLASES: [Clase; 5] = [Clase::Plana, Clase::Degradado, Clase::Borde, Clase::Textura, Clase::Rayas];

/// **Una baldosa de muestra** de la clase `c`. `a_lo_ancho`: si cambia a lo
/// ancho (un borde vertical) o a lo alto.
pub fn baldosa(d: &mut Dado, c: Clase, a_lo_ancho: bool) -> Baldosa {
    let base = d.real() * 0.9;
    let amp = 0.05 + d.real() * 0.6;
    let corte = 2 + (d.siguiente() % (LADO as u32 - 4)) as usize;
    let periodo = 1 + (d.siguiente() % 3) as usize;
    let mut b = [0.0; PIX];
    for y in 0..LADO {
        for x in 0..LADO {
            let t = if a_lo_ancho { x } else { y };
            let v = match c {
                Clase::Plana => base,
                Clase::Degradado => base + amp * 0.5 * t as f32 / LADO as f32,
                Clase::Borde => base + if t >= corte { amp } else { 0.0 },
                Clase::Textura => base + amp * d.real(),
                Clase::Rayas => base + if (t / periodo) % 2 == 0 { amp } else { 0.0 },
            };
            b[y * LADO + x] = v.clamp(0.0, 1.0);
        }
    }
    b
}

/// Las lecciones de muestra: `(rasgos, errores)` de `n` baldosas.
pub fn lecciones(n: usize) -> Vec<([f32; ENTRADAS], [f32; 4])> {
    let mut d = Dado(0x5041_4348); // "PACH"
    (0..n)
        .map(|i| {
            let c = CLASES[(d.siguiente() % CLASES.len() as u32) as usize];
            let b = baldosa(&mut d, c, i % 2 == 0);
            (rasgos(&b, Some(&b), &Extra::default()).entrada, maestra::errores(&b))
        })
        .collect()
}

/// **Mostrar** `pasos` lecciones: las de [`lecciones`], dando vueltas.
pub fn mostrar(u: &mut Uma, pasos: u32) {
    let paso = u.paso;
    u.paso = PASO_DE_NACER;
    let l = lecciones(MUESTRAS.min(pasos as usize).max(1));
    for i in 0..pasos as usize {
        let (x, e) = &l[i % l.len()];
        u.aprender(x, e);
    }
    u.paso = paso;
}
