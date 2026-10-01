//! **LA ENTRADA** -- el gato de BMO-X aparece con glitch y la LUDOTECA se
//! enciende detras (01-10: *"futurista elegante como el gato en BMO-X, con
//! glitch en la entrada"*).
//!
//! ```text
//!       0 -  450 ms   el gato baja por lineas, partido en rosa y azul
//!     450 - 1100 ms   el glitch se calma y LUDOTECA se escribe debajo
//!    1100 - 1500 ms   la ventana sale de la oscuridad (lo hace quien llama)
//! ```
//!
//! Cualquier tecla o clic la corta: nadie espera a una animacion.

use crate::canvas::Canvas;
use crate::gato::{EYES, HEIGHT, STROKE, WIDTH};
use crate::mates::azar;
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
    // LUDOTECA, letra a letra, con su temblor.
    if t > 450 {
        let palabra = b"LUDOTECA";
        let n = (((t - 450) * 8 / 400) as usize).min(palabra.len());
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
            let sub = b"tus juegos, de todas las tiendas";
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
