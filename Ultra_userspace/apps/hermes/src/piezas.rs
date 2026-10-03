//! **Las piezas de la cara**, las de la maqueta (`docs/arte/maqueta_hermes.html`):
//! cajas con las esquinas REDONDAS (y suavizadas: el borde de la curva se
//! mezcla con el fondo, no escalona), la negrita de los nombres, los rotulos
//! espaciados de las tarjetas, las caras redondas de la lista y las pildoras
//! de estado.
//!
//! Todo con el lienzo del TALLER y la letra del escritorio (8 x 16): nada de
//! fuera, ni una fuente nueva.

use crate::canvas::Canvas;
use bmo_dibujo::{mezclar, Color, Lienzo};

/// Raiz cuadrada entera (por abajo).
fn raiz(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Lo que se come la esquina en la fila `j` de una caja de radio `r`, en
/// 1/256 de pixel.
fn mordisco(r: i32, j: i32) -> i32 {
    // En medios pixeles: el centro de la fila j esta a (2r - 2j - 1)/2 del
    // centro del circulo.
    let d = (2 * r - 2 * j - 1) as i64;
    let dentro = (4 * r as i64 * r as i64 - d * d).max(0) as u64;
    r * 256 - (raiz(dentro * 65_536) / 2) as i32
}

/// **Una caja con las esquinas redondas**, rellena de `c`. El pixel donde
/// cae la curva se MEZCLA con lo que hay debajo, segun cuanto lo cubre.
pub fn redonda(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, r: i32, c: Color) {
    if w <= 0 || h <= 0 {
        return;
    }
    let r = r.min(w / 2).min(h / 2).max(0);
    for j in 0..h {
        let fila = if j < r { j } else if j >= h - r { h - 1 - j } else { r };
        if fila >= r {
            cv.rect(x, y + j, w, 1, c);
            continue;
        }
        let m = mordisco(r, fila);
        let (lleno, parte) = (m >> 8, (256 - (m & 255)) as u32);
        cv.rect(x + lleno + 1, y + j, w - 2 * lleno - 2, 1, c);
        cv.blend(x + lleno, y + j, c, parte, 256);
        cv.blend(x + w - 1 - lleno, y + j, c, parte, 256);
    }
}

/// **Una tarjeta**: caja redonda con su borde de un pixel.
pub fn caja(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, r: i32, relleno: Color, borde: Color) {
    redonda(cv, x, y, w, h, r, borde);
    redonda(cv, x + 1, y + 1, w - 2, h - 2, (r - 1).max(0), relleno);
}

/// **Negrita**: la letra dos veces, un pixel corrida. Devuelve el ancho.
pub fn negrita(cv: &mut Canvas, x: i32, y: i32, s: &[u8], c: Color) -> i32 {
    cv.text(x, y, s, c, 1);
    cv.text(x + 1, y, s, c, 1) + 1
}

/// Negrita recortada a `max` pixeles.
pub fn negrita_fit(cv: &mut Canvas, x: i32, y: i32, s: &[u8], c: Color, max: i32) -> i32 {
    let cabe = ((max - 1) / 8).max(0) as usize;
    negrita(cv, x, y, &s[..s.len().min(cabe)], c)
}

/// **Un rotulo** de tarjeta: MAYUSCULAS espaciadas, como `LA CONEXION`.
pub fn rotulo(cv: &mut Canvas, x: i32, y: i32, s: &[u8], c: Color) -> i32 {
    let mut cx = x;
    for &b in s {
        cv.text(cx, y, &[b.to_ascii_uppercase()], c, 1);
        cx += 10;
    }
    cx - x
}

/// **Una cara redonda** con su letra, como los amigos de la maqueta.
pub fn cara(cv: &mut Canvas, cx: i32, cy: i32, r: i32, c: Color, letra: u8, fondo: Color) {
    cv.disc(cx, cy, r, c);
    let tinta = mezclar(fondo, c, 220, 256);
    cv.text(cx - 4, cy - 8, &[letra], tinta, 1);
    cv.text(cx - 3, cy - 8, &[letra], tinta, 1);
}

/// El punto de estado de una cara (abajo a la derecha), con su aro.
pub fn punto(cv: &mut Canvas, cx: i32, cy: i32, c: Color, aro: Color) {
    cv.disc(cx, cy, 5, aro);
    cv.disc(cx, cy, 3, c);
}

/// **Una pildora** de estado (`LAN directa`, `RED`, `NINGUNA`): texto en su
/// color sobre su color apagado. Se pinta hacia la IZQUIERDA desde `der`;
/// devuelve donde empieza.
pub fn pildora(cv: &mut Canvas, der: i32, y: i32, s: &[u8], c: Color, fondo: Color) -> i32 {
    let w = s.len() as i32 * 8 + 18;
    let x = der - w;
    caja(cv, x, y, w, 22, 11, mezclar(c, fondo, 34, 256), mezclar(c, fondo, 200, 256));
    negrita(cv, x + 9, y + 3, s, c);
    x
}

/// **Las alas** de HERMES, chicas: el sello de arriba del riel. Cada ala es
/// un borde de arriba que sube a la punta y un borde de abajo en tres
/// festones, como las de la maqueta.
pub fn alas(cv: &mut Canvas, cx: i32, cy: i32, c: Color) {
    for lado in [-1i32, 1] {
        let p = |dx: i32, dy: i32| (cx + lado * dx, cy + dy);
        let borde = [p(2, 1), p(8, -3), p(17, -6)];
        let festones = [p(17, -6), p(14, 0), p(11, -1), p(8, 3), p(5, 2), p(2, 5)];
        for g in 0..2 {
            for t in borde.windows(2) {
                cv.line((t[0].0, t[0].1 + g), (t[1].0, t[1].1 + g), c);
            }
        }
        for t in festones.windows(2) {
            cv.line(t[0], t[1], c);
        }
    }
    cv.disc(cx, cy + 2, 2, c);
}

/// **Un trazo de dos pixeles**, como las lineas de los iconos de la maqueta.
pub fn trazo(cv: &mut Canvas, a: (i32, i32), b: (i32, i32), c: Color) {
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        cv.line((a.0 + dx, a.1 + dy), (b.0 + dx, b.1 + dy), c);
    }
}

/// **Un arco** de radio `r` y dos pixeles de grueso, de `a0` a `a1` (vueltas
/// de 256, 0 a la derecha, creciendo hacia abajo).
pub fn arco(cv: &mut Canvas, cx: i32, cy: i32, r: i32, a0: i32, a1: i32, c: Color) {
    use crate::mates::{coseno, seno};
    let pasos = ((a1 - a0).abs() * r / 24).max(8);
    for k in 0..=pasos {
        let a = a0 + (a1 - a0) * k / pasos;
        for rr in [r, r - 1] {
            cv.put(cx + coseno(a) * rr / 256, cy + seno(a) * rr / 256, c);
        }
    }
}

/// **Un triangulo relleno** que apunta a la derecha (`dir` = 1) o a la
/// izquierda (-1), de alto `2h` y punta a `w` del lomo.
pub fn flecha(cv: &mut Canvas, x: i32, cy: i32, w: i32, h: i32, dir: i32, c: Color) {
    for j in -h..=h {
        let largo = w * (h - j.abs()) / h.max(1);
        if dir > 0 {
            cv.rect(x, cy + j, largo, 1, c);
        } else {
            cv.rect(x - largo, cy + j, largo, 1, c);
        }
    }
}
