//! **Las piezas de la cara**, las de la maqueta (`docs/arte/maqueta_hermes.html`):
//! cajas con las esquinas REDONDAS (y suavizadas: el borde de la curva se
//! mezcla con el fondo, no escalona), la negrita de los nombres, los rotulos
//! espaciados de las tarjetas, las caras redondas de la lista y las pildoras
//! de estado.
//!
//! Todo con el lienzo del TALLER y, desde el 03-10, la LETRA de las maquetas
//! (`bmo-letra`: proporcional, suave, trazos de la casa). La de 8 x 16 queda
//! para lo que en la maqueta es letra de pixel (`t_grande`).

use crate::canvas::Canvas;
use bmo_dibujo::{mezclar, Color, Lienzo};

/// Raiz cuadrada entera (por abajo).
pub fn raiz(n: u64) -> u64 {
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

/// **Negrita**: la de la maqueta (`font-weight: 600`, 14 px), en la caja de
/// 16 de las cuentas de siempre. Devuelve el ancho.
pub fn negrita(cv: &mut Canvas, x: i32, y: i32, s: &[u8], c: Color) -> i32 {
    texto(cv, x, y, 16, s, c, NEGRITA)
}

/// Negrita recortada a `max` pixeles (con tres puntos si no cabe).
pub fn negrita_fit(cv: &mut Canvas, x: i32, y: i32, s: &[u8], c: Color, max: i32) -> i32 {
    texto_cabe(cv, x, y, 16, s, c, NEGRITA, max)
}

/// **Un rotulo** de tarjeta: MAYUSCULAS de 11 espaciadas (`letter-spacing:
/// .14em`), como `LA CONEXION`.
pub fn rotulo(cv: &mut Canvas, x: i32, y: i32, s: &[u8], c: Color) -> i32 {
    texto(cv, x, y, 16, s, c, ROTULO)
}

/// La letra del cuerpo (`font-size: 14px`), la negrita y el rotulo.
pub const T: Estilo = Estilo::normal(14);
/// La de las notas de debajo de un nombre (`.t2`, 12 px).
pub const T2: Estilo = Estilo::normal(12);
pub const NEGRITA: Estilo = Estilo::negrita(14);
pub const MEDIA: Estilo = Estilo::media(14);
pub const ROTULO: Estilo = Estilo::normal(11).espaciado(140).mayusculas();

/// **Texto del cuerpo** en la caja de 16 de siempre (lo que era
/// `cv.text(.., 1)`, ya con la letra de la maqueta). Devuelve el ancho.
pub fn txt(cv: &mut Canvas, x: i32, y: i32, s: &[u8], c: Color) -> i32 {
    texto(cv, x, y, 16, s, c, T)
}

/// Texto del cuerpo recortado a `max` pixeles, con tres puntos.
pub fn txt_cabe(cv: &mut Canvas, x: i32, y: i32, s: &[u8], c: Color, max: i32) -> i32 {
    texto_cabe(cv, x, y, 16, s, c, T, max)
}

/// Lo que mide un texto del cuerpo.
pub fn ancho_txt(s: &[u8]) -> i32 {
    medir(s, T)
}

/// **La letra de PIXEL**, grande (los rotulos de la maqueta en Silkscreen):
/// la de 8 x 16 del escritorio, a `escala`.
pub fn t_grande(cv: &mut Canvas, x: i32, y: i32, s: &[u8], c: Color, escala: i32) -> i32 {
    cv.text(x, y, s, c, escala)
}

/// **Una cara redonda** con su letra, como los amigos de la maqueta
/// (`.avatar`: negrita de 13, centrada).
pub fn cara(cv: &mut Canvas, cx: i32, cy: i32, r: i32, c: Color, letra: u8, fondo: Color) {
    cv.disc(cx, cy, r, c);
    let tinta = mezclar(fondo, c, 220, 256);
    let e = Estilo::negrita(13);
    let w = medir(&[letra], e);
    texto(cv, cx - w / 2, cy - 9, 18, &[letra], tinta, e);
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
    // `.chip` de la maqueta: 11 px, `font-weight: 600`, 9 de relleno.
    let e = Estilo::negrita(11);
    let w = medir(s, e) + 20;
    let x = der - w;
    caja(cv, x, y, w, 22, 11, mezclar(c, fondo, 34, 256), mezclar(c, fondo, 200, 256));
    texto(cv, x + 10, y + 3, 16, s, c, e);
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

// ---------------------------------------------------------------------------
// LA LETRA DE LAS MAQUETAS (03-10): `bmo-letra`, con su cache, y lo que hace
// falta para escribir como escribe el CSS de la maqueta.
// ---------------------------------------------------------------------------

pub use bmo_letra::Estilo;
use bmo_letra::Fuente;

/// La cache de glifos de la app. La app es UN hilo: solo se toca desde el
/// bucle que pinta.
struct Cache(core::cell::UnsafeCell<bmo_letra::Letra>);
// SAFETY: la app tiene un solo hilo; nadie mas llega a esta cache.
unsafe impl Sync for Cache {}
static LETRA: Cache = Cache(core::cell::UnsafeCell::new(bmo_letra::Letra::nueva()));

fn letra() -> &'static mut bmo_letra::Letra {
    // SAFETY: un hilo, y ninguna funcion de aqui se llama a si misma con la
    // referencia viva (cada una la toma y la suelta).
    unsafe { &mut *LETRA.0.get() }
}

/// Lo que mide un texto.
pub fn medir(s: &[u8], e: Estilo) -> i32 {
    letra().medir(s, e)
}

/// **Escribe** con la base en `base`. Devuelve el ancho.
pub fn escribir(cv: &mut Canvas, x: i32, base: i32, s: &[u8], c: Color, e: Estilo) -> i32 {
    letra().escribir(s, e, x, base, |x, y, a| cv.blend(x, y, c, a as u32, 255))
}

/// **Escribe en una caja de texto del CSS**: `y` es lo alto de la caja y
/// `alto` su `line-height`, como los mide el navegador en la maqueta.
pub fn texto(cv: &mut Canvas, x: i32, y: i32, alto: i32, s: &[u8], c: Color, e: Estilo) -> i32 {
    escribir(cv, x, y + bmo_letra::base_en_caja(e.px, alto), s, c, e)
}

/// Como [`texto`], recortado a `max` pixeles con tres puntos.
pub fn texto_cabe(cv: &mut Canvas, x: i32, y: i32, alto: i32, s: &[u8], c: Color, e: Estilo, max: i32) -> i32 {
    let base = y + bmo_letra::base_en_caja(e.px, alto);
    letra().escribir_cabe(s, e, x, base, max, |x, y, a| cv.blend(x, y, c, a as u32, 255))
}

/// Como [`texto`], pero ACABA en `der` (los numeros de la derecha).
pub fn texto_der(cv: &mut Canvas, der: i32, y: i32, alto: i32, s: &[u8], c: Color, e: Estilo) -> i32 {
    let w = medir(s, e);
    texto(cv, der - w, y, alto, s, c, e);
    der - w
}

/// **Parte un parrafo** en lineas de `ancho` pixeles, por los espacios.
pub fn partir(s: &[u8], e: Estilo, ancho: i32) -> alloc::vec::Vec<&[u8]> {
    let mut v = alloc::vec::Vec::new();
    let mut resto = s;
    while !resto.is_empty() {
        if medir(resto, e) <= ancho {
            v.push(resto);
            break;
        }
        // El corte mas largo que cabe, en un espacio.
        let mut corte = 0;
        for (k, &b) in resto.iter().enumerate() {
            if b == b' ' {
                if k > 0 && medir(&resto[..k], e) > ancho {
                    break;
                }
                corte = k;
            }
        }
        if corte == 0 {
            corte = resto.iter().position(|&b| b == b' ').unwrap_or(resto.len());
        }
        v.push(&resto[..corte]);
        resto = &resto[corte..];
        while resto.first() == Some(&b' ') {
            resto = &resto[1..];
        }
    }
    v
}

/// **Un parrafo** que empieza en `y`, de lineas de `alto`; como mucho
/// `max_lineas`. Devuelve cuantas escribio.
pub fn parrafo(cv: &mut Canvas, x: i32, y: i32, ancho: i32, alto: i32, s: &[u8], c: Color, e: Estilo, max_lineas: usize) -> usize {
    let lineas = partir(s, e, ancho);
    let n = lineas.len().min(max_lineas);
    for (k, l) in lineas.iter().take(n).enumerate() {
        texto(cv, x, y + k as i32 * alto, alto, l, c, e);
    }
    n
}

/// **Una sombra suave** (el `box-shadow: 0 0 Npx` del CSS) alrededor de una
/// caja redonda: `fuerza` de 256 pegada al borde, y nada a `difusa` pixeles.
pub fn sombra(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, r: i32, c: Color, difusa: i32, fuerza: u32) {
    let d = difusa.max(1);
    let (cx2, cy2) = (2 * x + w, 2 * y + h);
    for py in y - d..y + h + d {
        for px in x - d..x + w + d {
            // La distancia (en medios pixeles) al borde de la caja redonda.
            let qx = ((2 * px + 1 - cx2).abs() - w + 2 * r).max(0);
            let qy = ((2 * py + 1 - cy2).abs() - h + 2 * r).max(0);
            let fuera = raiz((qx * qx + qy * qy) as u64) as i32 - 2 * r;
            if fuera <= 0 {
                continue;
            }
            let t = (2 * d - fuera).max(0) as u32;
            if t == 0 {
                continue;
            }
            let a = fuerza * t * t / (4 * (d * d) as u32);
            cv.blend(px, py, c, a, 256);
        }
    }
}

/// **Un camino de SVG** de la maqueta, trazado con la pluma de la casa:
/// `poner` lleva cada punto (en 1/64 de unidad del `viewBox`) a 1/64 de
/// pixel de la ventana (ahi va la escala, el sitio y, si la hay, el giro).
pub fn camino(cv: &mut Canvas, d: &str, grosor64: i32, c: Color, poner: impl Fn((i32, i32)) -> (i32, i32)) {
    for sub in bmo_letra::svg::camino(d) {
        let p: alloc::vec::Vec<(i32, i32)> = sub.puntos.iter().map(|&q| poner(q)).collect();
        bmo_letra::pluma(&p, grosor64, sub.cerrado, |x, y, a| cv.blend(x, y, c, a as u32, 255));
    }
}

/// **Un camino de SVG relleno** (la pajarita, la gota).
pub fn camino_relleno(cv: &mut Canvas, d: &str, c: Color, poner: impl Fn((i32, i32)) -> (i32, i32)) {
    let subs: alloc::vec::Vec<alloc::vec::Vec<(i32, i32)>> =
        bmo_letra::svg::camino(d).into_iter().map(|s| s.puntos.iter().map(|&q| poner(q)).collect()).collect();
    bmo_letra::svg::rellenar(&subs, |x, y, a| cv.blend(x, y, c, a as u32, 255));
}

/// Lo que casi siempre es `poner`: escala `k` (pixeles por unidad, en
/// 1/64) y el origen en `(x, y)`.
pub fn en(x: i32, y: i32, k64: i32) -> impl Fn((i32, i32)) -> (i32, i32) {
    move |(px, py)| (x * 64 + px * k64 / 64, y * 64 + py * k64 / 64)
}

/// **Un icono de la maqueta** (viewBox de 24, trazo de 2, puntas
/// redondas), de `lado` pixeles con su esquina en `(x, y)`.
pub fn icono_svg(cv: &mut Canvas, x: i32, y: i32, lado: i32, d: &str, c: Color) {
    let k64 = lado * 64 / 24;
    camino(cv, d, 2 * k64, c, en(x, y, k64));
}
