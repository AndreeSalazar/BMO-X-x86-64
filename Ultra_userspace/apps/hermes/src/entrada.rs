//! **LA ENTRADA** -- el gato de BMO-X con ALAS, y su glitch (H5 de
//! `PLAN_HERMES.md`: *"entrada con alas"*; la maqueta la tiene).
//!
//! ```text
//!       0 -  450 ms   el gato baja por lineas, partido en rosa y azul
//!     250 - 1100 ms   las alas se abren pluma a pluma y aletean
//!     450 - 1100 ms   el glitch se calma y HERMES se escribe debajo
//!    1100 - 1500 ms   la ventana sale de la oscuridad (lo hace quien llama)
//! ```
//!
//! Hermes, el mensajero, lleva alas: aqui las lleva el gato. Cualquier tecla
//! o clic la corta: nadie espera a una animacion.

use crate::canvas::Canvas;
use crate::gato::{EYES, HEIGHT, STROKE, WIDTH};
use crate::mates::{azar, coseno, seno};
use bmo_dibujo::{mezclar, Color, Lienzo};

pub const DURA: u32 = 1500;
/// Desde aqui se ve la ventana, y la entrada solo la oscurece.
pub const FUNDIDO: u32 = 1100;

const NEGRO: Color = 0x0005_060A;
const BLANCO: Color = 0x00F2_F7F9;
const CIAN: Color = 0x005E_F2E6;
const ROSA: Color = 0x00FF_2E88;
const AZUL: Color = 0x003D_A5FF;

fn bit(m: &[u8], fx: u32, fy: u32) -> bool {
    let i = (fy * WIDTH + fx) as usize;
    m[i / 8] >> (i % 8) & 1 == 1
}

/// El gato a escala 2 en `(x, y)`, con el salto de cada banda y su color.
fn gato(cv: &mut Canvas, x: i32, y: i32, filas: u32, salto: &dyn Fn(u32) -> i32, trazo: Color, ojos: Color) {
    for fy in 0..filas.min(HEIGHT) {
        let dx = salto(fy);
        for fx in 0..WIDTH {
            let c = if bit(&EYES, fx, fy) {
                ojos
            } else if bit(&STROKE, fx, fy) {
                trazo
            } else {
                continue;
            };
            cv.rect(x + fx as i32 * 2 + dx, y + fy as i32 * 2, 2, 2, c);
        }
    }
}

/// **Un ala**: plumas que salen del hombro `(hx, hy)` en abanico hacia el lado
/// `lado` (-1 izquierda, +1 derecha). `abre` de 0 a 256 (cuantas plumas hay),
/// `bate` el angulo del aleteo, en vueltas de 256.
pub fn ala(cv: &mut Canvas, hx: i32, hy: i32, lado: i32, abre: i32, bate: i32, color: Color, escala: i32) {
    const PLUMAS: i32 = 9;
    // De arriba y fuera (-45 grados, la mas larga) a un poco hacia abajo (la
    // mas corta): un ala abierta a cada lado del lomo, con las puntas fuera
    // de la cabeza. El aleteo las mueve a todas a la vez.
    let pluma = |k: i32| -> (i32, i32) {
        let ang = -32 + k * 5 + bate;
        let largo = (160 - k * 10) * escala / 2;
        (hx + lado * coseno(ang) * largo / 256, hy + seno(ang) * largo / 256)
    };
    let cuantas = (PLUMAS * abre.clamp(0, 256) / 256).max(0);
    // La membrana: un abanico tenue entre la primera pluma y la ultima abierta.
    if cuantas > 1 {
        let tenue = mezclar(color, NEGRO, 60, 256);
        for j in 0..=(cuantas - 1) * 6 {
            let k6 = j; // en sextos de pluma
            let (a, b) = (pluma(k6 / 6), pluma((k6 / 6 + 1).min(PLUMAS - 1)));
            let f = k6 % 6;
            let p = (a.0 + (b.0 - a.0) * f / 6, a.1 + (b.1 - a.1) * f / 6);
            cv.line((hx, hy), p, tenue);
        }
    }
    for k in 0..cuantas {
        let (px, py) = pluma(k);
        let grosor = ((3 - k / 3) * escala / 2).max(1);
        for g in 0..grosor {
            cv.line((hx, hy + g), (px, py + g), color);
        }
        // La punta, un poco mas clara.
        cv.rect(px - escala, py - escala, 2 * escala, 2 * escala, mezclar(BLANCO, color, 120, 256));
    }
}

/// **Un fotograma de la entrada** a `t` ms de abrir.
pub fn pintar(cv: &mut Canvas, t: u32) {
    cv.clear(NEGRO);
    let (gw, gh) = (WIDTH as i32 * 2, HEIGHT as i32 * 2);
    let x = (cv.w - gw) / 2;
    let y = (cv.h - gh) / 2 - 50;
    // Lo que queda de glitch: entero al principio, nada a los 1100 ms.
    let fuerza = (FUNDIDO.saturating_sub(t) * 256 / FUNDIDO) as i32;
    let filas = (t * HEIGHT / 450).min(HEIGHT);
    let tic = t / 45;
    let banda = move |fy: u32| -> i32 {
        let r = azar((fy / 10) ^ (tic << 8));
        let salto = if fuerza > 0 && r % 7 == 0 { ((r >> 8) % 24) as i32 - 12 } else { 0 };
        salto * fuerza / 256
    };
    // ** LAS ALAS, detras del gato: se abren desde los 250 ms y aletean dos
    // veces por segundo, cada vez menos, hasta quedarse abiertas.
    if t > 250 {
        let abre = ((t - 250) * 256 / 500).min(256) as i32;
        let calma = (DURA.saturating_sub(t) * 256 / DURA) as i32;
        let bate = seno((t as i32 * 256 / 500) % 256) * (6 + calma / 20) / 256;
        // Los hombros, en el lomo: los dos costados del cuerpo, a media altura.
        let (hy, hl, hr) = (y + gh * 60 / 100, x + gw * 37 / 100, x + gw * 79 / 100);
        if fuerza > 20 {
            let d = 2 + fuerza * 6 / 256;
            ala(cv, hl - d, hy, -1, abre, bate, ROSA, 2);
            ala(cv, hr + d, hy, 1, abre, bate, AZUL, 2);
        }
        let color = mezclar(CIAN, NEGRO, abre as u32, 256);
        ala(cv, hl, hy, -1, abre, bate, color, 2);
        ala(cv, hr, hy, 1, abre, bate, color, 2);
    }
    if fuerza > 20 {
        let d = 2 + fuerza * 6 / 256;
        gato(cv, x - d, y, filas, &banda, ROSA, ROSA);
        gato(cv, x + d, y, filas, &banda, AZUL, AZUL);
    }
    let ojos = if t > 450 { CIAN } else { mezclar(CIAN, NEGRO, t * 256 / 450, 256) };
    gato(cv, x, y, filas, &banda, BLANCO, ojos);
    // La linea que escanea mientras baja.
    if filas < HEIGHT {
        cv.rect(x - 20, y + filas as i32 * 2, gw + 40, 2, CIAN);
    }
    // HERMES, letra a letra, con su temblor.
    if t > 450 {
        let palabra = b"HERMES";
        let n = (((t - 450) * 6 / 360) as usize).min(palabra.len());
        let escala = 4;
        let ancho = palabra.len() as i32 * 8 * escala;
        let tx = (cv.w - ancho) / 2;
        let ty = y + gh + 30;
        if fuerza > 0 {
            let j = (azar(tic) % 5) as i32 - 2;
            cv.text(tx - 3 + j, ty, &palabra[..n], ROSA, escala);
            cv.text(tx + 3 - j, ty, &palabra[..n], AZUL, escala);
        }
        cv.text(tx, ty, &palabra[..n], BLANCO, escala);
        if n == palabra.len() {
            let sub = b"dos BMO-X que se hablan, sin servidor de nadie";
            cv.text((cv.w - sub.len() as i32 * 8) / 2, ty + 80, sub, mezclar(CIAN, NEGRO, 180, 256), 1);
        }
    }
}

/// Oscurece la ventana ya pintada mientras sale de la entrada.
pub fn fundido(cv: &mut Canvas, t: u32) {
    if t < DURA {
        let falta = (DURA - t) * 256 / (DURA - FUNDIDO);
        cv.veil(NEGRO, falta.min(256), 256);
    }
}
