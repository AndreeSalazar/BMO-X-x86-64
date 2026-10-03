//! **EL GATO HUCHA** -- el de la maqueta (`docs/arte/maqueta_bankcat.html`):
//! el gato de BMO-X hecho maneki-neko, con la RANURA de la hucha en la
//! cabeza, monoculo de banquero, pajarita, una pata que llama a la suerte y la
//! moneda CAB en la otra.
//!
//! ```text
//!    quieto     saluda despacio y parpadea de vez en cuando
//!    RICO       entra dinero: monedas en los ojos, caen monedas en la
//!               ranura, salta el monoculo y saluda deprisa
//!    TRISTE     sale dinero (o no hay): ojos caidos, una lagrima, y la pata
//!               se queda quieta
//! ```
//!
//! Dibujado con trazos de dos pixeles sobre una caja de 260 x 290 (la del
//! SVG de la maqueta), desde `(x, y)`.

use crate::canvas::Canvas;
use crate::mates::{coseno, fase, seno};
use crate::piezas::{arco, redonda, trazo};
use bmo_dibujo::{mezclar, Color, Lienzo};

pub const ANCHO: i32 = 260;
pub const ALTO: i32 = 290;

const CIAN: Color = 0x005E_F2E6;
const ORO: Color = 0x00FF_D45E;
const ORO2: Color = 0x00C9_8A1B;
const ROSA: Color = 0x00FF_2E88;
const AZUL: Color = 0x003D_A5FF;
const NEGRO: Color = 0x0005_060A;
const TINTA_ORO: Color = 0x006B_4300;

/// Como esta el gato, y desde cuando.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Humor {
    Quieto,
    Rico,
    Triste,
}

/// Lo que dura un humor antes de volver a quieto.
pub const DURA_MS: u32 = 1400;

/// Una elipse de dos pixeles.
fn elipse(cv: &mut Canvas, cx: i32, cy: i32, rx: i32, ry: i32, c: Color) {
    let pasos = (rx + ry) * 3;
    for k in 0..pasos {
        let a = k * 256 / pasos;
        let (x, y) = (cx + coseno(a) * rx / 256, cy + seno(a) * ry / 256);
        cv.put(x, y, c);
        cv.put(x, y + 1, c);
        cv.put(x + 1, y, c);
    }
}

/// Una moneda CAB: oro, su canto y su nombre.
fn moneda(cv: &mut Canvas, cx: i32, cy: i32, r: i32, letra: &[u8]) {
    cv.disc(cx, cy, r, ORO2);
    cv.disc(cx, cy, r - 2, ORO);
    cv.disc(cx - r / 3, cy - r / 3, (r / 4).max(1), mezclar(0x00FF_F6CF, ORO, 160, 256));
    if r >= 16 {
        arco(cv, cx, cy, r - 6, 0, 256, ORO2);
    }
    let w = letra.len() as i32 * 8;
    cv.text(cx - w / 2, cy - 8, letra, TINTA_ORO, 1);
    cv.text(cx - w / 2 + 1, cy - 8, letra, TINTA_ORO, 1);
}

/// **El gato entero.** `humor` y `t` (los ms que lleva en ese humor) mandan
/// en los ojos, la pata, el monoculo, las monedas y la lagrima.
pub fn pintar(cv: &mut Canvas, x: i32, y: i32, ms: u32, humor: Humor, t: u32) {
    let p = |dx: i32, dy: i32| (x + dx, y + dy);
    let c = CIAN;

    // La cola, que se mece.
    let m = seno(fase(ms, 3000)) * 6 / 256;
    arco(cv, x + 214 + m / 2, y + 226, 24, -40, 150, c);
    arco(cv, x + 220 + m, y + 200, 10, 64, 230, c);

    // El cuerpo y las patas de abajo.
    trazo(cv, p(84, 168), p(72, 214), c);
    trazo(cv, p(72, 214), p(84, 260), c);
    trazo(cv, p(176, 168), p(188, 214), c);
    trazo(cv, p(188, 214), p(176, 260), c);
    trazo(cv, p(84, 260), p(176, 260), c);
    trazo(cv, p(110, 260), p(110, 238), c);
    trazo(cv, p(150, 260), p(150, 238), c);

    // La cabeza y las orejas.
    elipse(cv, x + 130, y + 120, 60, 50, c);
    trazo(cv, p(82, 90), p(74, 50), c);
    trazo(cv, p(74, 50), p(108, 74), c);
    trazo(cv, p(178, 90), p(186, 50), c);
    trazo(cv, p(186, 50), p(152, 74), c);

    // ** LA RANURA de la hucha, y las monedas que caen en ella (rico).
    redonda(cv, x + 112, y + 64, 36, 10, 5, ORO);
    redonda(cv, x + 115, y + 66, 30, 6, 3, NEGRO);
    if humor == Humor::Rico {
        for k in 0..3u32 {
            let tk = t.saturating_sub(k * 160);
            if tk > 0 && tk < 600 {
                let cy = y - 30 + (tk as i32 * 98 / 600);
                let r = 11 - (tk as i32 * 7 / 600);
                moneda(cv, x + 130, cy, r.max(3), b"");
            }
        }
    }

    // Los ojos: normales (y parpadean), de MONEDA (rico) o caidos (triste).
    match humor {
        Humor::Rico => {
            moneda(cv, x + 105, y + 114, 11, b"C");
            moneda(cv, x + 155, y + 114, 11, b"C");
        }
        Humor::Triste => {
            trazo(cv, p(94, 108), p(114, 114), c);
            trazo(cv, p(166, 108), p(146, 114), c);
            arco(cv, x + 105, y + 118, 7, 20, 108, c);
            arco(cv, x + 155, y + 118, 7, 20, 108, c);
            // La lagrima, que cae y se apaga.
            if t < 1000 {
                let ly = y + 124 + t as i32 * 50 / 1000;
                let a = 256 - t * 256 / 1000;
                cv.disc(x + 112, ly, 3, mezclar(AZUL, NEGRO, a, 256));
                cv.rect(x + 111, ly - 5, 2, 3, mezclar(AZUL, NEGRO, a, 256));
            }
        }
        Humor::Quieto => {
            let alto = if ms % 4000 < 140 { 1 } else { 5 };
            redonda(cv, x + 94, y + 112 + (5 - alto) / 2, 22, alto, 2, c);
            redonda(cv, x + 144, y + 112 + (5 - alto) / 2, 22, alto, 2, c);
        }
    }

    // El monoculo, con su cadena; salta cuando entra dinero.
    let salto = if humor == Humor::Rico && t < 500 { (seno((t * 128 / 500) as i32) * 14 / 256).max(0) } else { 0 };
    arco(cv, x + 155, y + 114 - salto, 16, 0, 256, ORO);
    arco(cv, x + 155, y + 114 - salto, 15, 0, 256, ORO);
    for k in 0..8 {
        let (a, b) = (p(170 + k, 122 + k * 5 - salto), p(171 + k, 124 + k * 5 - salto));
        if k % 2 == 0 {
            cv.line(a, b, ORO2);
        }
    }

    // El hocico, la sonrisa y los bigotes.
    trazo(cv, p(124, 134), p(130, 139), c);
    trazo(cv, p(130, 139), p(136, 134), c);
    trazo(cv, p(130, 139), p(130, 144), c);
    if humor == Humor::Triste {
        arco(cv, x + 130, y + 156, 7, 150, 234, c);
    } else {
        arco(cv, x + 130, y + 142, 8, 30, 98, c);
    }
    for (a, b) in [((80, 136), (54, 132)), ((80, 144), (54, 150)), ((180, 136), (206, 132)), ((180, 144), (206, 150))] {
        cv.line(p(a.0, a.1), p(b.0, b.1), c);
    }

    // La pajarita.
    for j in -9..=9i32 {
        let largo = 16 * (j.abs() + 3) / 12;
        cv.rect(x + 130 - largo, y + 172 + j, largo, 1, ROSA);
        cv.rect(x + 130, y + 172 + j, largo, 1, ROSA);
    }
    cv.disc(x + 130, y + 172, 4, ROSA);

    // ** LA PATA QUE LLAMA: gira desde el hombro; deprisa si entra dinero,
    // quieta y caida si sale.
    let (hx, hy) = (x + 84, y + 176);
    let angulo = match humor {
        Humor::Triste => 150,
        Humor::Rico => 166 + seno(fase(ms, 450)) * 16 / 256,
        Humor::Quieto => 166 + seno(fase(ms, 1600)) * 14 / 256,
    };
    let (px, py) = (hx + coseno(angulo) * 56 / 256, hy + seno(angulo) * 56 / 256);
    // El brazo es una manga de dos bordes, no un palo.
    let (nx, ny) = (-seno(angulo) * 6 / 256, coseno(angulo) * 6 / 256);
    trazo(cv, (hx + nx, hy + ny), (px + nx, py + ny), c);
    trazo(cv, (hx - nx, hy - ny), (px - nx, py - ny), c);
    // La mano: redonda, con sus tres deditos y las almohadillas.
    let (mx, my) = (px + coseno(angulo) * 8 / 256, py + seno(angulo) * 8 / 256);
    cv.disc(mx, my, 11, NEGRO);
    arco(cv, mx, my, 11, 0, 256, c);
    for k in -1..=1 {
        let a = angulo + k * 26;
        let (dx, dy) = (mx + coseno(a) * 8 / 256, my + seno(a) * 8 / 256);
        cv.disc(dx, dy, 2, ROSA);
    }
    cv.disc(mx - coseno(angulo) * 2 / 256, my - seno(angulo) * 2 / 256, 3, ROSA);

    // La otra pata, con la moneda CAB.
    trazo(cv, p(178, 172), p(190, 190), c);
    moneda(cv, x + 194, y + 210, 22, b"CAB");
}
