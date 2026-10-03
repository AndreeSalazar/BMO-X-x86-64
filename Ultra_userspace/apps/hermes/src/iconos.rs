//! **Los iconos del riel**, de linea como los de la maqueta, cada uno con su
//! gesto chico. Aparte de `pintar.rs` para que este no pase el techo.

use crate::canvas::Canvas;
use crate::mates::{coseno, fase, onda, seno};
use crate::piezas::redonda;
use crate::pintar::{AMIGOS, CANAL, ENVIOS, MENSAJES, MURO, ONDA, PAGINAS, TERTULIAS};
use bmo_dibujo::{mezclar, Color, Lienzo};

/// **El icono de cada seccion**, de LINEA como los de la maqueta (bocadillo,
/// dos personas, cuadricula, pantalla, nota, globo, descarga, llave, escudo),
/// y cada uno con su gesto chico: los puntos que se escriben, la nota que
/// bota, el globo que gira, la flecha que baja...
pub(crate) fn icono(cv: &mut Canvas, i: usize, cx: i32, cy: i32, ms: u32, c: Color, bg: Color) {
    use crate::piezas::{arco, flecha, trazo};
    match i {
        // Un bocadillo, con sus tres puntos que se escriben por turno.
        MENSAJES => {
            redonda(cv, cx - 11, cy - 9, 22, 16, 5, c);
            redonda(cv, cx - 9, cy - 7, 18, 12, 3, bg);
            trazo(cv, (cx - 6, cy + 6), (cx - 8, cy + 10), c);
            trazo(cv, (cx - 8, cy + 10), (cx - 2, cy + 6), c);
            let turno = (ms / 250 % 4) as i32;
            for k in 0..turno.min(3) {
                cv.rect(cx - 6 + k * 5, cy - 2, 2, 2, c);
            }
        }
        // Dos personas: cabeza y hombros; la de delante saluda a ratos.
        TERTULIAS => {
            let bota = if onda(ms, 1400) > 200 { 1 } else { 0 };
            arco(cv, cx - 4, cy - 4 - bota, 4, 0, 256, c);
            arco(cv, cx - 4, cy + 9, 8, 128, 256, c);
            arco(cv, cx + 6, cy - 3, 3, 0, 256, c);
            arco(cv, cx + 7, cy + 8, 6, 160, 256, c);
        }
        // La cuadricula del MURO: cuatro fotos, una se enciende por turno.
        MURO => {
            let turno = (ms / 600 % 4) as i32;
            for k in 0..4 {
                let (x, y) = (cx - 10 + (k % 2) * 11, cy - 10 + (k / 2) * 11);
                redonda(cv, x, y, 9, 9, 3, c);
                if k != turno {
                    redonda(cv, x + 2, y + 2, 5, 5, 1, bg);
                }
            }
        }
        // Una pantalla con su triangulo de play, que late.
        CANAL => {
            redonda(cv, cx - 11, cy - 9, 22, 18, 4, c);
            redonda(cv, cx - 9, cy - 7, 18, 14, 2, bg);
            let w = 6 + (onda(ms, 1200) > 200) as i32;
            flecha(cv, cx - 2, cy, w, 4, 1, c);
        }
        // Una nota doble que bota.
        ONDA => {
            let b = onda(ms, 700) * 2 / 256;
            trazo(cv, (cx - 5, cy - 8 - b), (cx - 5, cy + 5 - b), c);
            trazo(cv, (cx + 6, cy - 10 - b), (cx + 6, cy + 3 - b), c);
            trazo(cv, (cx - 5, cy - 8 - b), (cx + 6, cy - 10 - b), c);
            cv.disc(cx - 8, cy + 6 - b, 3, c);
            cv.disc(cx + 3, cy + 4 - b, 3, c);
        }
        // Un globo, con su meridiano que gira.
        PAGINAS => {
            arco(cv, cx, cy, 11, 0, 256, c);
            trazo(cv, (cx - 10, cy), (cx + 9, cy), c);
            let w = (coseno(fase(ms, 2400)).abs() * 9 / 256).max(1);
            for k in 0..=24 {
                let a = k * 256 / 24;
                cv.put(cx + coseno(a) * w / 256, cy + seno(a) * 10 / 256, c);
                cv.put(cx + 1 + coseno(a) * w / 256, cy + seno(a) * 10 / 256, c);
            }
        }
        // La descarga: una flecha que baja a su bandeja.
        ENVIOS => {
            let b = (ms / 90 % 6) as i32 - 3;
            trazo(cv, (cx, cy - 10 + b), (cx, cy + 2 + b), c);
            trazo(cv, (cx - 5, cy - 3 + b), (cx, cy + 2 + b), c);
            trazo(cv, (cx + 5, cy - 3 + b), (cx, cy + 2 + b), c);
            trazo(cv, (cx - 10, cy + 5), (cx - 10, cy + 9), c);
            trazo(cv, (cx - 10, cy + 9), (cx + 9, cy + 9), c);
            trazo(cv, (cx + 9, cy + 9), (cx + 9, cy + 5), c);
        }
        // La llave de la huella, que se mece.
        AMIGOS => {
            let m = seno(fase(ms, 2000)) * 2 / 256;
            arco(cv, cx - 6, cy + m, 5, 0, 256, c);
            trazo(cv, (cx - 1, cy + m), (cx + 11, cy), c);
            trazo(cv, (cx + 6, cy), (cx + 6, cy + 4), c);
            trazo(cv, (cx + 10, cy), (cx + 10, cy + 3), c);
        }
        // El escudo de las jaulas, con su raya que sube y baja.
        _ => {
            trazo(cv, (cx - 9, cy - 9), (cx + 8, cy - 9), c);
            trazo(cv, (cx - 9, cy - 9), (cx - 9, cy + 1), c);
            trazo(cv, (cx + 8, cy - 9), (cx + 8, cy + 1), c);
            trazo(cv, (cx - 9, cy + 1), (cx, cy + 11), c);
            trazo(cv, (cx + 8, cy + 1), (cx, cy + 11), c);
            let y = cy - 6 + onda(ms, 2000) * 12 / 256;
            cv.rect(cx - 1, cy - 6, 2, 14, mezclar(c, bg, 110, 256));
            cv.rect(cx - 4, y, 8, 2, c);
        }
    }
}
