//! **LOS ICONOS VIVOS** -- uno por tienda, cada uno con SU animacion.
//!
//! [consumo] NADA      no corre solo: pinta un fotograma cuando la ventana se
//!                     ve y quien llama ya esta pintando (L6h)
//!
//! El propietario (01-10): *"que todos tengan animaciones unicas"*. Cada
//! tienda se reconoce por su COLOR y por un gesto que la evoca; ningun logo
//! se copia (son marcas de otros):
//!
//! ```text
//!    todas      cuatro puntos en orbita          gog        anillo que gira
//!    steam      tres hilos de vapor que suben    epic       esquirlas que flotan
//!    ubisoft    una espiral que se dibuja        ea         un pulso que corre
//!    battlenet  ondas hexagonales                microsoft  cuatro colores en orbita
//!    rockstar   estrella que late y destella     amazon     una caja que se abre
//!    itch       tres puntos que saltan           humble     un ecualizador
//!    libre      una rama que crece
//! ```

use crate::canvas::Canvas;
use crate::mates::{coseno, entre, fase, onda, seno};
use bmo_dibujo::{mezclar, triangulo, Color, Lienzo};

/// Cual de los trece.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Gesto {
    Todas,
    Gog,
    Steam,
    Epic,
    Ubisoft,
    Ea,
    Battlenet,
    Microsoft,
    Rockstar,
    Amazon,
    Itch,
    Humble,
    Libre,
}

fn punto(cv: &mut Canvas, x: i32, y: i32, g: i32, c: Color) {
    cv.rect(x - g / 2, y - g / 2, g, g, c);
}

fn tri(cv: &mut Canvas, a: (i32, i32), b: (i32, i32), d: (i32, i32), c: Color) {
    let r = cv.recorte();
    triangulo(&r, a, b, d, |y, x0, x1| cv.rect(x0, y, x1 - x0, 1, c));
}

/// Una linea de `g` pixeles de grueso.
fn trazo(cv: &mut Canvas, a: (i32, i32), b: (i32, i32), g: i32, c: Color) {
    let pasos = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
    for k in 0..=pasos {
        punto(cv, a.0 + (b.0 - a.0) * k / pasos, a.1 + (b.1 - a.1) * k / pasos, g, c);
    }
}

/// Un punto del circulo de radio `r` en el angulo `a` (vuelta = 256).
fn en(cx: i32, cy: i32, r: i32, a: i32) -> (i32, i32) {
    (cx + coseno(a) * r / 256, cy + seno(a) * r / 256)
}

/// Un hexagono de radio `r`.
fn hexagono(cv: &mut Canvas, cx: i32, cy: i32, r: i32, g: i32, c: Color) {
    for k in 0..6 {
        let a = en(cx, cy, r, k * 256 / 6 + 64);
        let b = en(cx, cy, r, (k + 1) * 256 / 6 + 64);
        trazo(cv, a, b, g, c);
    }
}

/// **Pinta el gesto** centrado en `(cx, cy)`, unos 30 px, en `c` sobre `fondo`.
pub fn pintar(cv: &mut Canvas, gesto: Gesto, cx: i32, cy: i32, ms: u32, c: Color, fondo: Color) {
    let suave = |k: i32| mezclar(c, fondo, k.clamp(0, 256) as u32, 256);
    match gesto {
        Gesto::Todas => {
            let f = fase(ms, 4000);
            for k in 0..4 {
                let (x, y) = en(cx, cy, 10, f + k * 64);
                cv.disc(x, y, 3, c);
            }
        }
        Gesto::Gog => {
            cv.disc(cx, cy, 5, c);
            cv.disc(cx, cy, 3, fondo);
            let giro = fase(ms, 3000);
            let mut a = 0;
            while a < 256 {
                if ((a + giro) / 16) % 2 == 0 {
                    let (x, y) = en(cx, cy, 12, a);
                    punto(cv, x, y, 2, c);
                }
                a += 3;
            }
        }
        Gesto::Steam => {
            for k in 0..3 {
                let x0 = cx - 8 + k * 8;
                let sube = fase(ms + k as u32 * 700, 2200);
                for y in -12..=12 {
                    let x = x0 + seno(y * 14 + sube * 2) * 3 / 256;
                    // Se enciende al subir y se apaga arriba.
                    let vida = 256 - ((y + 12) * 256 / 24 - sube).abs() * 2;
                    if vida > 0 {
                        punto(cv, x, cy + y, 2, suave(vida));
                    }
                }
            }
        }
        Gesto::Epic => {
            let d1 = seno(fase(ms, 2600)) * 3 / 256;
            let d2 = seno(fase(ms + 900, 2600)) * 3 / 256;
            tri(cv, (cx - 11, cy + 10 + d1), (cx - 2, cy - 11 + d1), (cx + 2, cy + 4 + d1), c);
            tri(cv, (cx + 1, cy + 11 + d2), (cx + 10, cy - 4 + d2), (cx + 12, cy + 10 + d2), suave(170));
        }
        Gesto::Ubisoft => {
            let hasta = fase(ms, 3000) * 2;
            for t in 0..hasta.min(256) {
                let r = 2 + t * 11 / 256;
                let (x, y) = en(cx, cy, r, t * 3);
                punto(cv, x, y, 2, c);
            }
        }
        Gesto::Ea => {
            let p = entre(-14, 14, fase(ms, 1200));
            for x in -13..=13 {
                let d = (x - p).abs();
                let alto = if d < 6 { (6 - d) * 2 * if (x - p) % 2 == 0 { 1 } else { -1 } } else { 0 };
                punto(cv, cx + x, cy - alto, 2, if d < 6 { c } else { suave(120) });
            }
        }
        Gesto::Battlenet => {
            hexagono(cv, cx, cy, 6, 2, c);
            let f = fase(ms, 2400);
            hexagono(cv, cx, cy, 6 + f * 9 / 256, 1, suave(256 - f));
        }
        Gesto::Microsoft => {
            // Cuatro colores en orbita alrededor de un nucleo que late.
            let giro = fase(ms, 7000);
            for (k, col) in [0x00F2_5022u32, 0x007F_BA00, 0x0000_A4EF, 0x00FF_B900].into_iter().enumerate() {
                let (x, y) = en(cx, cy, 9, giro + k as i32 * 64);
                cv.disc(x, y, 4, col);
            }
            cv.disc(cx, cy, 1 + onda(ms, 2400) * 2 / 256, c);
        }
        Gesto::Rockstar => {
            // La estrella que gira y late, y sus rayos que destellan.
            let destello = onda(ms, 1600);
            for k in 0..8 {
                let a = en(cx, cy, 13, k * 32);
                let b = en(cx, cy, 15, k * 32);
                trazo(cv, a, b, 1, suave(60 + destello * 196 / 256));
            }
            let giro = seno(fase(ms, 3200)) * 18 / 256;
            let r = 10 + onda(ms, 3200) / 128;
            let mut p = [(0, 0); 10];
            for (k, q) in p.iter_mut().enumerate() {
                *q = en(cx, cy, if k % 2 == 0 { r } else { r * 2 / 5 }, giro + k as i32 * 256 / 10 - 64);
            }
            for k in 0..10 {
                tri(cv, (cx, cy), p[k], p[(k + 1) % 10], c);
            }
        }
        Gesto::Amazon => {
            // La caja que se abre y deja salir una luz.
            cv.rect(cx - 9, cy - 3, 18, 13, c);
            cv.rect(cx - 9, cy - 3, 18, 1, suave(120));
            cv.rect(cx, cy - 3, 1, 13, suave(120));
            let f = fase(ms, 3400);
            let abre = if (90..200).contains(&f) { 4 } else { 0 };
            trazo(cv, (cx - 9, cy - 5 - abre), (cx + 1, cy - 5), 2, suave(200));
            if f > 120 {
                let sube = (f - 120) * 12 / 136;
                cv.disc(cx, cy - 6 - sube, 2, suave(256 - (f - 120) * 2));
            }
        }
        Gesto::Itch => {
            for k in 0..3 {
                let s = seno(fase(ms + k as u32 * 160, 1100)).max(0);
                cv.disc(cx - 9 + k * 9, cy + 4 - s * 8 / 256, 3, c);
            }
        }
        Gesto::Humble => {
            for k in 0..3 {
                let h = 6 + onda(ms + k as u32 * 220, 1300) * 16 / 256;
                cv.rect(cx - 10 + k * 8, cy + 11 - h, 5, h, c);
            }
        }
        Gesto::Libre => {
            let t = (fase(ms, 3400) * 2).min(256);
            let alto = 22 * t / 256;
            trazo(cv, (cx, cy + 12), (cx, cy + 12 - alto), 2, c);
            if t > 120 {
                let k = (t - 120) * 9 / 136;
                trazo(cv, (cx, cy + 4), (cx - k, cy - k / 2), 2, c);
                cv.disc(cx - k, cy - k / 2, 2, c);
            }
            if t > 180 {
                let k = (t - 180) * 8 / 76;
                trazo(cv, (cx, cy), (cx + k, cy - k), 2, c);
                cv.disc(cx + k, cy - k, 2, c);
            }
        }
    }
}
