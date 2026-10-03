//! **EL GATO HUCHA** -- el de la maqueta (`docs/arte/maqueta_bankcat.html`),
//! TRAZO A TRAZO: los caminos de abajo son los `d` de su SVG, leidos por
//! `bmo_letra::svg` y trazados con la pluma suave de la casa. Antes era un
//! dibujo a ojo en pixeles de dos; ahora es el mismo gato (03-10).
//!
//! ```text
//!    quieto     saluda despacio (1,6 s) y parpadea cada 4 s
//!    RICO       entra dinero: monedas en los ojos, caen tres monedas en la
//!               ranura, salta el monoculo y saluda deprisa (0,45 s)
//!    TRISTE     sale dinero (o no hay): ojos caidos, una lagrima, y la pata
//!               se queda caida
//! ```
//!
//! Las mismas animaciones que el CSS de la maqueta (`saluda`, `parpadea`,
//! `salta`, `cola`, `cae`, `gota`), con sus tiempos. La caja es la del SVG:
//! 260 x 290, desde `(x, y)`, un pixel por unidad.

use crate::canvas::Canvas;
use crate::piezas::{camino, camino_relleno, raiz, Estilo};
use alloc::vec::Vec;
use bmo_dibujo::{mezclar, Color};
use bmo_letra::{coseno, seno};

pub const ANCHO: i32 = 260;
pub const ALTO: i32 = 290;

const CIAN: Color = 0x005E_F2E6;
const ORO: Color = 0x00FF_D45E;
const ORO2: Color = 0x00C9_8A1B;
const ROSA: Color = 0x00FF_2E88;
const AZUL: Color = 0x003D_A5FF;
const NEGRO: Color = 0x0005_060A;
const TINTA_OJO: Color = 0x008A_5A00;
const TINTA_ORO: Color = 0x006B_4300;

/// Como esta el gato, y desde cuando.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Humor {
    Quieto,
    Rico,
    Triste,
}

/// Lo que dura un humor antes de volver a quieto (el `setTimeout` de 1400).
pub const DURA_MS: u32 = 1400;

// -- Los caminos del SVG de la maqueta, tal cual -------------------------------
const COLA: &str = "M196 250c40 0 52-44 26-58-14-8-26 6-16 16";
const CUERPO: &str = "M80 168c-14 30-14 70 4 92h96c18-22 18-62 4-92";
const PATAS: &str = "M110 260v-24M150 260v-24";
const CABEZA: &str = "M70 120c0-30 22-48 60-48s60 18 60 48-24 50-60 50-60-20-60-50z";
const OREJAS: &str = "M80 92l-6-42 34 26M180 92l6-42-34 26";
const OJOS_TRISTES: &str = "M94 110l20 6M166 110l-20 6M98 122q7 6 14 0M148 122q7 6 14 0";
const CADENA: &str = "M170 120c10 14 8 30 0 44";
const HOCICO: &str = "M124 134l6 5 6-5M130 139v5M122 148q8 6 16 0";
const BIGOTES: &str = "M80 136l-26-4M80 144l-26 6M180 136l26-4M180 144l26 6";
const PAJARITA: &str = "M130 172l-16-9v18zM130 172l16-9v18z";
const PATA: &str = "M84 176c-20-10-34-30-36-50-1-12 6-20 16-19 10 1 14 11 10 20-2 5-6 8-10 9";
const DEDOS: &str = "M50 112l3 5M58 106l1 6M67 108l-2 5";
const OTRA_PATA: &str = "M178 172c14 4 20 18 14 28";

/// Un pixel por unidad: el trazo de 3 del CSS son 192/64.
const T3: i32 = 3 * 64;

/// `ease-in-out` del CSS, de 0 a 256.
fn suave(t: i32) -> i32 {
    let t = t.clamp(0, 256);
    t * t * (768 - 2 * t) / 65536
}

/// Lo que va de `a` a `b` grados con la curva `ease-in-out`, ida y vuelta en
/// `periodo` ms (el `0%, 100% {a} 50% {b}` de la maqueta).
fn vaiven(ms: u32, periodo: u32, a: i32, b: i32) -> i32 {
    let f = ((ms % periodo) as u64 * 512 / periodo as u64) as i32;
    let t = if f < 256 { suave(f) } else { suave(512 - f) };
    a + (b - a) * t / 256
}

/// Pone un punto del SVG (1/64 de unidad) en la ventana, girado `grados`
/// alrededor de `(px, py)` (unidades) y subido `alto` (1/64).
fn girado(x: i32, y: i32, px: i32, py: i32, grados: i32, alto: i32) -> impl Fn((i32, i32)) -> (i32, i32) {
    let (c, s) = (coseno(grados) as i64, seno(grados) as i64);
    move |(qx, qy)| {
        let (dx, dy) = ((qx - px * 64) as i64, (qy - py * 64) as i64);
        let rx = (dx * c - dy * s) / 32768;
        let ry = (dx * s + dy * c) / 32768;
        (x * 64 + px * 64 + rx as i32, y * 64 + py * 64 + ry as i32 - alto)
    }
}

/// Un trazo con rayas (el `stroke-dasharray: 3 3` de la cadena).
fn rayado(cv: &mut Canvas, d: &str, grosor64: i32, c: Color, poner: impl Fn((i32, i32)) -> (i32, i32)) {
    for sub in bmo_letra::svg::camino(d) {
        let p: Vec<(i32, i32)> = sub.puntos.iter().map(|&q| poner(q)).collect();
        // Se anda el camino y se corta cada 3 px: dentro, fuera, dentro...
        let mut tramo: Vec<(i32, i32)> = Vec::new();
        let (mut andado, mut dentro) = (0i64, true);
        for w in p.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = ((b.0 - a.0) as i64, (b.1 - a.1) as i64);
            let largo = raiz((dx * dx + dy * dy) as u64) as i64;
            let mut hecho = 0i64;
            while hecho < largo {
                let falta = 3 * 64 - andado;
                let paso = falta.min(largo - hecho);
                let q = |k: i64| (a.0 + (dx * k / largo.max(1)) as i32, a.1 + (dy * k / largo.max(1)) as i32);
                if dentro {
                    if tramo.is_empty() {
                        tramo.push(q(hecho));
                    }
                    tramo.push(q(hecho + paso));
                }
                hecho += paso;
                andado += paso;
                if andado >= 3 * 64 {
                    andado = 0;
                    if dentro && tramo.len() > 1 {
                        bmo_letra::pluma(&tramo, grosor64, false, |x, y, a| cv.blend(x, y, c, a as u32, 255));
                    }
                    tramo.clear();
                    dentro = !dentro;
                }
            }
        }
        if dentro && tramo.len() > 1 {
            bmo_letra::pluma(&tramo, grosor64, false, |x, y, a| cv.blend(x, y, c, a as u32, 255));
        }
    }
}

/// Un texto centrado en `cx` con su base en `base`.
fn centrado(cv: &mut Canvas, cx: i32, base: i32, s: &[u8], c: Color, e: Estilo) {
    let w = crate::piezas::medir(s, e);
    crate::piezas::escribir(cv, cx - w / 2, base, s, c, e);
}

/// Una moneda: oro con su canto (`stroke: oro2`) y, si es la grande, su aro
/// de dentro y su nombre.
fn moneda(cv: &mut Canvas, cx: i32, cy: i32, r: i32) {
    cv.disc(cx, cy, r + 1, ORO2);
    cv.disc(cx, cy, r - 1, ORO);
}

/// **El gato entero.** `humor` y `t` (los ms que lleva en ese humor) mandan
/// en los ojos, la pata, el monoculo, las monedas y la lagrima.
pub fn pintar(cv: &mut Canvas, x: i32, y: i32, ms: u32, humor: Humor, t: u32) {
    let aqui = girado(x, y, 0, 0, 0, 0);

    // La cola, que se mece de 0 a 8 grados en 3 s.
    let g = vaiven(ms, 3000, 0, 8);
    camino(cv, COLA, T3, CIAN, girado(x, y, 196, 250, g, 0));

    // El cuerpo, las patas de abajo, la cabeza y las orejas.
    for d in [CUERPO, PATAS, CABEZA, OREJAS] {
        camino(cv, d, T3, CIAN, &aqui);
    }

    // ** LA RANURA de la hucha (rect 114,66 32x6 rx 3, borde oro de 2).
    crate::piezas::redonda(cv, x + 113, y + 65, 34, 8, 4, ORO);
    crate::piezas::redonda(cv, x + 115, y + 67, 30, 4, 2, NEGRO);

    // Los ojos.
    match humor {
        Humor::Rico => {
            for cx in [105, 155] {
                moneda(cv, x + cx, y + 114, 11);
                centrado(cv, x + cx, y + 119, b"C", TINTA_OJO, Estilo::negrita(13));
            }
        }
        Humor::Triste => camino(cv, OJOS_TRISTES, T3, CIAN, &aqui),
        Humor::Quieto => {
            // `parpadea`: 4 s; del 94 % al 96 % se cierra a un decimo y al
            // 100 % vuelve.
            let f = (ms % 4000) as i32;
            let alto = if f < 3760 {
                5 * 64
            } else if f < 3840 {
                5 * 64 - (f - 3760) * (45 * 64 / 10) / 80
            } else {
                64 / 2 + (f - 3840) * (45 * 64 / 10) / 160
            };
            // Cada ojo es la raya de 22 x 5 con sus puntas redondas: una
            // pluma de `alto` de gruesa.
            for ox in [94, 144] {
                let cy = (y + 114) * 64 + 32;
                let r = alto / 2;
                let raya = [((x + ox) * 64 + r, cy), ((x + ox + 22) * 64 - r, cy)];
                bmo_letra::pluma(&raya, alto, false, |px, py, a| cv.blend(px, py, CIAN, a as u32, 255));
            }
        }
    }

    // El monoculo (circulo de 16, trazo 3) con su cadena rayada: `salta`
    // cuando entra dinero (0,5 s: sube 14 y gira -12 grados al 40 %).
    let (alto, giro) = if humor == Humor::Rico && t < 500 {
        let k = t as i32 * 256 / 500;
        let u = if k < 102 { suave(k * 256 / 102) } else { 256 - suave((k - 102) * 256 / 154) };
        (14 * 64 * u / 256, -12 * u / 256)
    } else {
        (0, 0)
    };
    let mono = girado(x, y, 156, 112, giro, alto);
    let aro: Vec<(i32, i32)> = (0..=48).map(|k| mono((155 * 64 + 16 * coseno(k * 15) / 512, 114 * 64 - 16 * seno(k * 15) / 512))).collect();
    bmo_letra::pluma(&aro, T3, true, |px, py, a| cv.blend(px, py, ORO, a as u32, 255));
    rayado(cv, CADENA, 2 * 64, ORO2, &mono);

    // El hocico, la sonrisa y los bigotes.
    camino(cv, HOCICO, T3, CIAN, &aqui);
    camino(cv, BIGOTES, T3, CIAN, &aqui);

    // La pajarita, rellena, y su nudo.
    camino_relleno(cv, PAJARITA, ROSA, &aqui);
    cv.disc(x + 130, y + 172, 4, ROSA);

    // ** LA PATA QUE LLAMA: `saluda` de -14 a 16 grados desde el hombro
    // (84, 176), en 1,6 s; deprisa (0,45 s) si entra dinero; caida a 18
    // grados si sale.
    let g = match humor {
        Humor::Triste => 18,
        Humor::Rico => vaiven(ms, 450, -14, 16),
        Humor::Quieto => vaiven(ms, 1600, -14, 16),
    };
    let pata = girado(x, y, 84, 176, g, 0);
    camino(cv, PATA, T3, CIAN, &pata);
    camino(cv, DEDOS, T3, CIAN, &pata);

    // La otra pata, con la moneda CAB (r 22, aro de dentro r 16).
    camino(cv, OTRA_PATA, T3, CIAN, &aqui);
    moneda(cv, x + 194, y + 210, 22);
    let aro: Vec<(i32, i32)> = (0..=48).map(|k| ((x + 194) * 64 + 16 * coseno(k * 15) / 512, (y + 210) * 64 - 16 * seno(k * 15) / 512)).collect();
    bmo_letra::pluma(&aro, 96, true, |px, py, a| cv.blend(px, py, ORO2, a as u32, 255));
    centrado(cv, x + 194, y + 215, b"CAB", TINTA_ORO, Estilo::negrita(11));

    // Las monedas que caen en la ranura (`cae`, 0,7 s, tres, cada 160 ms):
    // de 30 por encima de la caja hasta la ranura, aplastandose y apagandose.
    if humor == Humor::Rico {
        for k in 0..3u32 {
            let tk = t.saturating_sub(k * 160);
            if tk == 0 || tk >= 700 {
                continue;
            }
            // cubic-bezier(.5, 0, .9, .5): empieza despacio y acelera.
            let u = (tk * 256 / 700) as i32;
            let v = u * u / 256 * 3 / 4 + u / 4;
            let top = -30 + 64 * v / 256;
            let (cx, cy) = (x + 131, y + top + 11);
            let ancho = 11 * (256 - v * 7 / 10) / 256;
            let luz = (256 - v) as u32;
            for j in -11..=11 {
                let medio = raiz((121 - j * j).max(0) as u64) as i32 * ancho / 11;
                for i in -medio..=medio {
                    let c = if j < -3 && i < 0 { mezclar(0x00FF_F1B8, ORO, 120, 256) } else { ORO };
                    cv.blend(cx + i, cy + j, c, luz, 256);
                }
            }
        }
    }

    // La lagrima (`gota`, 1 s): de (112, 120) a 175, apagandose.
    if humor == Humor::Triste && t < 1000 {
        let ly = y + 120 + t as i32 * 55 / 1000;
        let luz = 256 - t * 256 / 1000;
        for j in 0..9 {
            let medio = if j < 4 { (j + 1) * 3 / 4 } else { 3 - (j - 4) / 3 };
            for i in -medio..=medio {
                cv.blend(x + 115 + i, ly + j, AZUL, luz, 256);
            }
        }
    }
}
