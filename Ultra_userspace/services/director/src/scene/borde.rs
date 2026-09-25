//! **LOS BORDES QUE CUADRAN** (2026-09-25). Pedido del propietario: *"el borde
//! no cumple, se ve como que no cuadra en pixeles, y que sea redonda la
//! ventana, muy elegante y profesional"*.
//!
//! [consumo] NADA      pinta cuando alguien pinta una ventana; ni reloj ni
//!                     animacion aqui (L6h)
//!
//! # Lo que estaba mal, medido
//!
//! 1. **La curva estaba corrida una fila.** La tabla era `[8, 5, 3, 2, 1, 1,
//!    0, 0]`; un cuarto de circulo de radio 8 es `[5, 3, 2, 1, 1, 0, 0, 0]`
//!    (pixel dentro si su CENTRO cae en el circulo). La primera fila entraba 8
//!    de golpe: una esquina ACHATADA, no redonda.
//! 2. **El borde engordaba en las curvas.** Se pintaba un redondeado del
//!    color del borde y otro del cuerpo 1 px dentro CON EL MISMO RADIO. Un
//!    anillo de grosor constante es radio `r` fuera y `r - 1` dentro.
//! 3. **La barra de titulo** seguia la curva de FUERA y se comia el borde.
//! 4. **Sin suavizado**: sin canal alfa, cada pixel de la curva era todo o
//!    nada, y eso es la escalera que se ve en una foto.
//!
//! # Lo que hay ahora
//!
//! ```text
//!    sangria(r, i)       la curva de verdad, calculada (sin tabla que corregir)
//!    relleno_r / marco   rellenos y anillos de 1 px CONSTANTE, sin suavizar:
//!                        para lo chico (nodos, la barra lateral)
//!    ventana             la ventana entera: radio 10, las cuatro esquinas
//!                        SUAVIZADAS (4 x 4 muestras por pixel), borde de 1 px
//!                        y barra de titulo con su propia curva
//!    sombra              una sombra que se DESVANECE, no dos rectangulos
//!    pastilla            campos y botones: radio chico, suavizada, sobre un
//!                        fondo que se conoce
//! ```
//!
//! El suavizado mezcla con el FONDO DEL ESCRITORIO (`background_at`), no con
//! lo que haya en la pantalla: leer y mezclar otra vez en cada repintado haria
//! que la esquina engordara un poco cada vez. Solo se tocan los pixeles de
//! cobertura parcial; los de fuera se dejan como estaban.

use bmo_userland as bmo;

use super::globo::mezcla;

/// El radio de las VENTANAS. 10 y no 12: con 12 el boton de cerrar (a 6 px
/// del borde derecho) asomaria sobre la curva.
pub(crate) const R_VENTANA: u32 = 10;
/// Lo que la sombra se desvanece, en pixeles.
const DIFUMINADO: u32 = 7;
/// Lo mas oscura que llega la sombra, sobre 256.
const SOMBRA_MAX: u32 = 150;

/// **La curva de verdad**: cuanto se mete la fila `i` (0 = la del borde) de
/// una esquina de radio `r`. Un pixel esta dentro si su centro cae en el
/// circulo: `(2r - 2j - 1)^2 + (2r - 2i - 1)^2 <= (2r)^2`, en enteros.
pub(crate) const fn sangria(r: u32, i: u32) -> u32 {
    if i >= r {
        return 0;
    }
    let dy = 2 * r - 2 * i - 1;
    let tope = 4 * r * r;
    let mut j = 0;
    while j < r {
        let dx = 2 * r - 2 * j - 1;
        if dx * dx + dy * dy <= tope {
            return j;
        }
        j += 1;
    }
    r
}

/// Esta `(x, y)` dentro del redondeado de radio `r`?
pub(crate) fn dentro(x: u32, y: u32, rx: u32, ry: u32, w: u32, h: u32, r: u32) -> bool {
    if x < rx || x >= rx + w || y < ry || y >= ry + h {
        return false;
    }
    let dy = y - ry;
    let i = if dy < r { dy } else if dy + r >= h { h - 1 - dy } else { return true };
    let s = sangria(r, i);
    x >= rx + s && x < rx + w - s
}

/// Un redondeado de radio `r`, sin suavizar.
pub(crate) fn relleno_r(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, r: u32, color: u32) {
    if r == 0 || w <= 2 * r || h <= 2 * r {
        p.rect(x, y, w, h, color);
        return;
    }
    for i in 0..r {
        let s = sangria(r, i);
        p.rect(x + s, y + i, w - 2 * s, 1, color);
        p.rect(x + s, y + h - 1 - i, w - 2 * s, 1, color);
    }
    p.rect(x, y + r, w, h - 2 * r, color);
}

/// **Un marco de 1 px CONSTANTE**: radio `r` fuera, `r - 1` dentro.
pub(crate) fn marco(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, r: u32, borde: u32, cuerpo: u32) {
    relleno_r(p, x, y, w, h, r, borde);
    if w > 2 && h > 2 {
        relleno_r(p, x + 1, y + 1, w - 2, h - 2, r.saturating_sub(1), cuerpo);
    }
}

/// Cuantas de las 16 muestras de un pixel caen dentro de un circulo de radio
/// `r` (en pixeles) con centro en `(cx, cy)`: todo en octavos de pixel.
fn cobertura(px: u32, py: u32, cx: u32, cy: u32, r: u32) -> u32 {
    let tope = (r as i64 * 8) * (r as i64 * 8);
    let mut n = 0;
    for a in 0..4i64 {
        for b in 0..4i64 {
            let dx = px as i64 * 8 + 2 * a + 1 - cx as i64 * 8;
            let dy = py as i64 * 8 + 2 * b + 1 - cy as i64 * 8;
            if dx * dx + dy * dy <= tope {
                n += 1;
            }
        }
    }
    n
}

/// Tres colores en proporcion `(fuera, borde, dentro)` de 16.
fn tres(fondo: u32, borde: u32, dentro: u32, co: u32, ci: u32) -> u32 {
    let canal = |s: u32| {
        let (f, b, d) = ((fondo >> s) & 0xFF, (borde >> s) & 0xFF, (dentro >> s) & 0xFF);
        ((f * (16 - co) + b * (co - ci) + d * ci) / 16) & 0xFF
    };
    canal(16) << 16 | canal(8) << 8 | canal(0)
}

/// **Una esquina suavizada**: el cuadrado `r x r` en `(ox, oy)`, circulo de
/// centro `(cx, cy)`; `relleno` por fila (la barra de titulo y el cuerpo no
/// son del mismo color) y el fondo por pixel.
fn esquina(
    p: &bmo::Pantalla,
    (ox, oy): (u32, u32),
    (cx, cy): (u32, u32),
    r: u32,
    borde: u32,
    relleno: &dyn Fn(u32) -> u32,
    fondo: &dyn Fn(u32, u32) -> u32,
) {
    p.marcar(ox, oy, r, r);
    for py in oy..oy + r {
        for px in ox..ox + r {
            let co = cobertura(px, py, cx, cy, r);
            if co == 0 {
                continue;
            }
            let ci = if r > 1 { cobertura(px, py, cx, cy, r - 1) } else { 0 };
            let c = if co == 16 && ci == 16 { relleno(py) } else { tres(fondo(px, py), borde, relleno(py), co, ci) };
            p.punto_ya_marcado(px, py, c);
        }
    }
}

/// **LA VENTANA**: radio [`R_VENTANA`], borde de 1 px constante y suavizado,
/// la barra de titulo (`alto_titulo` filas, color `titulo`) con su propia
/// curva, y el cuerpo. `fondo` dice que hay DETRAS de cada pixel (el
/// escritorio), para suavizar sin leer la pantalla.
pub(crate) fn ventana(
    p: &bmo::Pantalla,
    (x, y, w, h): (u32, u32, u32, u32),
    borde: u32,
    titulo: u32,
    alto_titulo: u32,
    cuerpo: u32,
    fondo: &dyn Fn(u32, u32) -> u32,
) {
    let r = R_VENTANA;
    if w <= 2 * r + 2 || h <= 2 * r + 2 || alto_titulo <= r {
        marco(p, x, y, w, h, 0, borde, cuerpo);
        return;
    }
    // Los tramos rectos: la barra, el cuerpo y las cuatro rayas del borde.
    p.rect(x + r, y + 1, w - 2 * r, r - 1, titulo);
    p.rect(x + 1, y + r, w - 2, alto_titulo - r, titulo);
    p.rect(x + 1, y + alto_titulo, w - 2, h - alto_titulo - r, cuerpo);
    p.rect(x + r, y + h - r, w - 2 * r, r - 1, cuerpo);
    p.rect(x + r, y, w - 2 * r, 1, borde);
    p.rect(x + r, y + h - 1, w - 2 * r, 1, borde);
    p.rect(x, y + r, 1, h - 2 * r, borde);
    p.rect(x + w - 1, y + r, 1, h - 2 * r, borde);
    // Las cuatro esquinas, suavizadas.
    let relleno = |py: u32| if py < y + alto_titulo { titulo } else { cuerpo };
    esquina(p, (x, y), (x + r, y + r), r, borde, &relleno, fondo);
    esquina(p, (x + w - r, y), (x + w - r, y + r), r, borde, &relleno, fondo);
    esquina(p, (x, y + h - r), (x + r, y + h - r), r, borde, &relleno, fondo);
    esquina(p, (x + w - r, y + h - r), (x + w - r, y + h - r), r, borde, &relleno, fondo);
}

/// **La sombra**, que se desvanece: desplazada 2 a la derecha y 3 abajo,
/// solo dentro de lo que el borrado limpia (`SHADOW_RIGHT` x `SHADOW_BOTTOM`
/// de mas). Se pinta ANTES que la ventana: lo que la ventana tapa, lo tapa.
pub(crate) fn sombra(
    p: &bmo::Pantalla,
    (x, y, w, h): (u32, u32, u32, u32),
    derecha: u32,
    abajo: u32,
    fondo: &dyn Fn(u32, u32) -> u32,
) {
    let r = R_VENTANA;
    let (sx, sy) = (x + 2, y + 3);
    let (sw, sh) = (w, h);
    // La distancia de un pixel al redondeado desplazado (0 = dentro).
    let distancia = |px: u32, py: u32| -> u32 {
        let dx = if px < sx + r { sx + r - px } else if px + r >= sx + sw { px + r + 1 - (sx + sw) } else { 0 };
        let dy = if py < sy + r { sy + r - py } else if py + r >= sy + sh { py + r + 1 - (sy + sh) } else { 0 };
        // |(dx, dy)| aproximado sin raiz: max + min/2 (un octogono).
        let (a, b) = if dx > dy { (dx, dy) } else { (dy, dx) };
        (a + b / 2).saturating_sub(r)
    };
    let tira = |x0: u32, y0: u32, x1: u32, y1: u32| {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        p.marcar(x0, y0, x1 - x0, y1 - y0);
        for py in y0..y1 {
            for px in x0..x1 {
                // Lo que la ventana tapa no se pinta (sus esquinas si).
                if dentro(px, py, x, y, w, h, r) {
                    continue;
                }
                let d = distancia(px, py);
                if d >= DIFUMINADO {
                    continue;
                }
                let q = DIFUMINADO - d;
                let alfa = SOMBRA_MAX * q * q / (DIFUMINADO * DIFUMINADO);
                p.punto_ya_marcado(px, py, mezcla(fondo(px, py), 0, alfa));
            }
        }
    };
    // Abajo (entera) y a la derecha (sin repetir la esquina de abajo), y las
    // dos esquinas de arriba y de la izquierda por si su curva deja hueco.
    tira(x, y + h - r, x + w + derecha, y + h + abajo);
    tira(x + w - r, y, x + w + derecha, y + h - r);
}

/// **Una pastilla**: un campo o un boton, radio `r` chico, suavizada contra
/// un `fondo` que se conoce (la banda donde vive), con borde de 1 px (o sin
/// borde si `borde == relleno`).
pub(crate) fn pastilla(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32), r: u32, relleno: u32, borde: u32, fondo: u32) {
    if w <= 2 * r || h <= 2 * r || r == 0 {
        p.rect(x, y, w, h, borde);
        p.rect(x + 1, y + 1, w.saturating_sub(2), h.saturating_sub(2), relleno);
        return;
    }
    p.rect(x + r, y, w - 2 * r, 1, borde);
    p.rect(x + r, y + h - 1, w - 2 * r, 1, borde);
    p.rect(x, y + r, 1, h - 2 * r, borde);
    p.rect(x + w - 1, y + r, 1, h - 2 * r, borde);
    p.rect(x + r, y + 1, w - 2 * r, h - 2, relleno);
    p.rect(x + 1, y + r, w - 2, h - 2 * r, relleno);
    let dentro_c = |_: u32| relleno;
    let fuera = |_: u32, _: u32| fondo;
    esquina(p, (x, y), (x + r, y + r), r, borde, &dentro_c, &fuera);
    esquina(p, (x + w - r, y), (x + w - r, y + r), r, borde, &dentro_c, &fuera);
    esquina(p, (x, y + h - r), (x + r, y + h - r), r, borde, &dentro_c, &fuera);
    esquina(p, (x + w - r, y + h - r), (x + w - r, y + h - r), r, borde, &dentro_c, &fuera);
}
