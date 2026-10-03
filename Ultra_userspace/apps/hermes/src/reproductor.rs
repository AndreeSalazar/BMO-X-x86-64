//! **EL REPRODUCTOR**, la barra de abajo de la maqueta, a lo ancho: la
//! portada y el nombre de lo que suena, azar / anterior / pausa / siguiente,
//! la vuelta del bucle, el ecualizador chico y el volumen.
//!
//! *** Todo de verdad: cada boton se le PIDE al escritorio, que es quien
//! toca (`desktop::pide`: `fondo <n>`, `fondo pausa`, `fondo volumen <n>`,
//! los mismos mandos que la PASTILLA), y las barras son el medidor del
//! MAESTRO. Sin "me gusta": no hay a quien contarselo.

use crate::canvas::Canvas;
use crate::onda::{color_pieza, portada, reloj, vuelta_ms};
use crate::piezas::{flecha, negrita_fit, redonda, trazo, ancho_txt, txt, txt_cabe};
use crate::pintar::{ancho, dentro, nivel, suelo, Vista, BLANCO, GRIS, LIMA, LINEA, NEGRO, REPRO, TENUE, TEXTO};
use bmo_dibujo::{mezclar, Lienzo};
use bmo_fondo::PIEZAS;

/// Lo que se puede tocar en la barra.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mando {
    Azar,
    Anterior,
    PausaOSigue,
    Siguiente,
    /// 0..=100.
    Volumen(u32),
}

fn centro() -> (i32, i32) {
    (ancho() / 2, suelo() + 24)
}

fn caja_boton(m: Mando) -> (i32, i32, i32, i32) {
    let (cx, cy) = centro();
    let dx = match m {
        Mando::Azar => -96,
        Mando::Anterior => -50,
        Mando::Siguiente => 50,
        _ => 0,
    };
    (cx + dx - 16, cy - 16, 32, 32)
}

/// El volumen: de donde a donde va la regla.
fn regla() -> (i32, i32) {
    let x1 = ancho() - 26;
    (x1 - 140, x1)
}

/// **Que hay debajo de un clic** en la barra.
pub fn golpe(x: i32, y: i32) -> Option<Mando> {
    if y < suelo() {
        return None;
    }
    for m in [Mando::Azar, Mando::Anterior, Mando::PausaOSigue, Mando::Siguiente] {
        if dentro(x, y, caja_boton(m)) {
            return Some(m);
        }
    }
    let (r0, r1) = regla();
    if dentro(x, y, (r0 - 8, suelo() + 20, r1 - r0 + 16, 26)) {
        return Some(Mando::Volumen((((x - r0).clamp(0, r1 - r0) * 100) / (r1 - r0)) as u32));
    }
    None
}

pub fn pintar(cv: &mut Canvas, v: &Vista) {
    let y0 = suelo();
    cv.rect(0, y0, ancho(), REPRO, NEGRO);
    cv.rect(0, y0, ancho(), 1, LINEA);
    let k = v.pedida.unwrap_or(0) % PIEZAS.len();
    let suena = v.sonando && v.pedida.is_some() && !v.pausada;
    let color = if v.pedida.is_some() { color_pieza(k) } else { LIMA };
    let (cx, cy) = centro();
    let encima = |m: Mando| v.puntero.map_or(false, |(px, py)| dentro(px, py, caja_boton(m)));

    // La vuelta del bucle es mas corta si la ventana es estrecha.
    let media = if ancho() < 1100 { 130 } else { 190 };
    let (p0, p1) = (cx - media, cx + media);
    // ** Lo que suena: portada, nombre y de donde viene, sin pisar el reloj.
    let tope = (cx - 96 - 16 - 24).min(p0 - 60);
    portada(cv, 14, y0 + 11, 44, k, suena, v.ms);
    if v.pedida.is_some() {
        negrita_fit(cv, 70, y0 + 14, PIEZAS[k].nombre.as_bytes(), BLANCO, tope - 70);
        let mut d = [0u8; 8];
        let nd = crate::fmt_num(PIEZAS[k].bpm as u64, &mut d);
        let fin = txt(cv, 70, y0 + 34, b"la ONDA - ", TENUE);
        let fin2 = txt(cv, 70 + fin, y0 + 34, &d[..nd], TENUE);
        txt_cabe(cv, 70 + fin + fin2, y0 + 34, b" pulsos", TENUE, tope - 70 - fin - fin2);
    } else {
        negrita_fit(cv, 70, y0 + 14, b"Nada suena", TEXTO, tope - 70);
        txt_cabe(cv, 70, y0 + 34, b"elige una pieza en la ONDA", TENUE, tope - 70);
    }

    // ** Los botones.
    let tinta = |m: Mando| if encima(m) { BLANCO } else { GRIS };
    // Azar: dos flechas que se cruzan.
    let (ax, ay) = (cx - 96, cy);
    let c = tinta(Mando::Azar);
    trazo(cv, (ax - 8, ay - 5), (ax + 6, ay + 5), c);
    trazo(cv, (ax - 8, ay + 5), (ax + 6, ay - 5), c);
    flecha(cv, ax + 5, ay - 5, 4, 3, 1, c);
    flecha(cv, ax + 5, ay + 5, 4, 3, 1, c);
    // Anterior y siguiente: la raya y el triangulo.
    let c = tinta(Mando::Anterior);
    cv.rect(cx - 58, cy - 6, 2, 13, c);
    flecha(cv, cx - 44, cy, 11, 6, -1, c);
    let c = tinta(Mando::Siguiente);
    flecha(cv, cx + 44, cy, 11, 6, 1, c);
    cv.rect(cx + 56, cy - 6, 2, 13, c);
    // El grande: un circulo blanco con la pausa o el triangulo.
    let r = if encima(Mando::PausaOSigue) { 19 } else { 18 };
    cv.disc(cx, cy, r, BLANCO);
    if suena {
        cv.rect(cx - 6, cy - 7, 4, 15, NEGRO);
        cv.rect(cx + 2, cy - 7, 4, 15, NEGRO);
    } else {
        flecha(cv, cx - 4, cy, 12, 7, 1, NEGRO);
    }

    // ** La vuelta del bucle, debajo de los botones.
    let total = vuelta_ms(PIEZAS[k].bpm);
    let va = if v.pedida.is_some() && v.sonando { v.ms.wrapping_sub(v.pedida_desde) % total } else { 0 };
    let py = y0 + 52;
    let mut b1 = [0u8; 8];
    let mut b2 = [0u8; 8];
    let (n1, n2) = (reloj(va, &mut b1), reloj(total, &mut b2));
    if v.pedida.is_some() {
        txt(cv, p0 - 10 - ancho_txt(&b1[..n1]), py - 8, &b1[..n1], TENUE);
        txt(cv, p1 + 10, py - 8, &b2[..n2], TENUE);
    }
    redonda(cv, p0, py - 2, p1 - p0, 4, 2, LINEA);
    let lleno = ((p1 - p0) as u64 * va as u64 / total.max(1) as u64) as i32;
    redonda(cv, p0, py - 2, lleno.max(4), 4, 2, color);

    // ** El ecualizador chico, si cabe entre la vuelta y el volumen.
    let (r0, r1) = regla();
    let (e0, e1) = (p1 + 70, r0 - 40);
    if e1 - e0 >= 60 {
        let barras = ((e1 - e0) / 7).min(18);
        let fuerza = if suena { (nivel(v.rms[0]) + nivel(v.rms[1])) / 2 } else { 0 };
        for b in 0..barras {
            let baila = crate::mates::onda(v.ms + b as u32 * 83, 230 + (b as u32 % 5) * 41);
            let a = if suena { (fuerza * 30 / 256 * (128 + baila / 2) / 256).clamp(2, 34) } else { 2 };
            // Cuadritos, como la maqueta.
            let mut y = y0 + 50;
            let mut hecho = 0;
            while hecho < a {
                cv.rect(e0 + b * 7, y - 3, 5, 3, mezclar(color, NEGRO, 120 + (hecho * 4).min(136) as u32, 256));
                y -= 4;
                hecho += 4;
            }
        }
    }

    // ** El volumen: el altavoz y su regla.
    let (sx, sy) = (r0 - 24, y0 + 32);
    cv.rect(sx - 9, sy - 3, 5, 7, GRIS);
    flecha(cv, sx + 3, sy, 8, 7, -1, GRIS);
    crate::piezas::arco(cv, sx + 6, sy, 8, -40, 40, GRIS);
    let x_vol = r0 + (r1 - r0) * v.volumen.min(100) as i32 / 100;
    redonda(cv, r0, sy - 2, r1 - r0, 5, 2, LINEA);
    redonda(cv, r0, sy - 2, (x_vol - r0).max(5), 5, 2, LIMA);
    cv.disc(x_vol, sy, 7, BLANCO);
}
