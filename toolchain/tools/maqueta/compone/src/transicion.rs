//! **LA TRANSICION** (P3, 04-10): de un estado a otro, interpolando cajas YA
//! maquetadas. Aqui no se maqueta nada: las dos maquetaciones salieron de la
//! cadena y del veredicto, y esto solo mezcla numeros.
//!
//! ```text
//!    cada caja      su `transition` (la del estado de LLEGADA, como en CSS):
//!                   cuanto tarda, cuanto espera y con que curva
//!    lo que se      el sitio y la medida (rect, contenido, donde va el
//!    mueve          texto), los colores, el radio, el borde, el resplandor
//!    la letra       su talla y su interlineado crecen suaves (el pintor
//!                   dibuja cualquier talla); el peso salta a mitad
//!    lo que salta   lo que no se puede mezclar -- un color que aparece o
//!                   desaparece -- cambia a MITAD de su transicion, que es
//!                   lo que hace CSS con lo discreto
//! ```
//!
//! Las cajas se emparejan por su CAMINO de spans desde la raiz, no por
//! orden: una caja que en un estado es absoluta cambia de sitio en la lista
//! de hijos, y una pieza puesta dos veces tiene los mismos spans dentro.
//!
//! El rebote (`cubic-bezier` con `y` por encima de 1) se pasa de largo en
//! el SITIO y la MEDIDA; en los colores no (un color no se pasa de largo).

use std::collections::HashMap;

use bmo_maqueta_cascade::Style;
use bmo_maqueta_layout::{Frame, Laid, Rect};

/// Lo que tarda la transicion ENTERA de `a` a `b`: la caja que mas tarde
/// acaba (espera incluida).
pub fn duracion(b: &Laid) -> u32 {
    b.all().iter().filter_map(|f| f.style.transicion).map(|t| t.ms + t.retraso).max().unwrap_or(0)
}

/// **La maquetacion a los `ms` de empezar a ir de `a` a `b`.**
pub fn en(a: &Laid, b: &Laid, ms: u32) -> Laid {
    let mut destino: HashMap<Vec<usize>, &Frame> = HashMap::new();
    indexar(&b.root, &mut Vec::new(), &mut destino);
    let (wa, ha) = a.canvas;
    let (wb, hb) = b.canvas;
    // El lienzo, con la caja raiz.
    let p = avance(&b.root.style, ms).clamp(0, 1000);
    let canvas = (mezcla_u(wa, wb, p), mezcla_u(ha, hb, p));
    let mut root = mezclar(&a.root, &destino, &mut Vec::new(), ms);
    root.rect.w = canvas.0;
    root.rect.h = canvas.1;
    Laid { root, canvas }
}

fn indexar<'a>(f: &'a Frame, camino: &mut Vec<usize>, out: &mut HashMap<Vec<usize>, &'a Frame>) {
    camino.push(f.span.start);
    out.insert(camino.clone(), f);
    for c in &f.children {
        indexar(c, camino, out);
    }
    camino.pop();
}

/// Cuanto ha avanzado una caja (milesimas, puede pasar de 1000).
fn avance(s: &Style, ms: u32) -> i32 {
    match s.transicion {
        None => {
            if ms == 0 {
                0
            } else {
                1000
            }
        }
        Some(t) => {
            if ms <= t.retraso {
                return 0;
            }
            if t.ms == 0 {
                return 1000;
            }
            let x = ((ms - t.retraso) as u64 * 1000 / t.ms as u64).min(1000) as i32;
            bmo_pinta::curva(t.curva, x)
        }
    }
}

fn mezclar(a: &Frame, destino: &HashMap<Vec<usize>, &Frame>, camino: &mut Vec<usize>, ms: u32) -> Frame {
    camino.push(a.span.start);
    let Some(b) = destino.get(camino.as_slice()).copied() else {
        // Una caja que solo esta en un estado no puede pasar: misma marca,
        // mismo arbol. Si llega, se queda como esta.
        camino.pop();
        return a.clone();
    };
    let p = avance(&b.style, ms);
    let pc = p.clamp(0, 1000);
    let mut f = if pc >= 500 { b.clone() } else { a.clone() };
    f.rect = rect(a.rect, b.rect, p);
    f.content = rect(a.content, b.content, p);
    f.text_at = match (a.text_at, b.text_at) {
        (Some(x), Some(y)) => Some(rect(x, y, p)),
        (x, y) => if pc >= 500 { y } else { x },
    };
    f.style = estilo(&a.style, &b.style, p);
    f.children = a.children.iter().map(|c| mezclar(c, destino, camino, ms)).collect();
    camino.pop();
    f
}

fn mezcla_i(a: i32, b: i32, p: i32) -> i32 {
    a + ((b as i64 - a as i64) * p as i64 / 1000) as i32
}

fn mezcla_u(a: u32, b: u32, p: i32) -> u32 {
    mezcla_i(a as i32, b as i32, p).max(0) as u32
}

fn rect(a: Rect, b: Rect, p: i32) -> Rect {
    Rect { x: mezcla_i(a.x, b.x, p), y: mezcla_i(a.y, b.y, p), w: mezcla_u(a.w, b.w, p), h: mezcla_u(a.h, b.h, p) }
}

/// Un color de `0x00RRGGBB` (o `0xAARRGGBB`), canal a canal.
fn color(a: u32, b: u32, p: i32) -> u32 {
    let p = p.clamp(0, 1000) as u32;
    let c = |s: u32| (((a >> s & 255) * (1000 - p) + (b >> s & 255) * p + 500) / 1000) << s;
    c(24) | c(16) | c(8) | c(0)
}

fn opcion(a: Option<u32>, b: Option<u32>, p: i32) -> Option<u32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(color(x, y, p)),
        (x, y) => if p >= 500 { y } else { x },
    }
}

fn estilo(a: &Style, b: &Style, p: i32) -> Style {
    let pc = p.clamp(0, 1000);
    let mut s = if pc >= 500 { *b } else { *a };
    s.background = opcion(a.background, b.background, pc);
    s.color = opcion(a.color, b.color, pc);
    for k in 0..4 {
        s.border_color[k] = opcion(a.border_color[k], b.border_color[k], pc);
        s.border_width[k] = mezcla_u(a.border_width[k], b.border_width[k], pc);
    }
    s.border_radius = mezcla_u(a.border_radius, b.border_radius, p);
    s.shadow = match (a.shadow, b.shadow) {
        (Some((ra, ca)), Some((rb, cb))) => Some((mezcla_u(ra, rb, p), color(ca, cb, pc))),
        // Un resplandor que aparece crece desde nada (alcance y fuerza 0).
        (None, Some((rb, cb))) => Some((mezcla_u(0, rb, p), color(cb & 0x00FF_FFFF, cb, pc))),
        (Some((ra, ca)), None) => Some((mezcla_u(ra, 0, p), color(ca, ca & 0x00FF_FFFF, pc))).filter(|_| pc < 1000),
        (None, None) => None,
    };
    s.gradient = match (a.gradient, b.gradient) {
        (Some((da, aa, va)), Some((db, ab, _))) => Some((color(da, db, pc), color(aa, ab, pc), va)),
        (x, y) => if pc >= 500 { y } else { x },
    };
    // La letra crece suave: el pintor dibuja cualquier talla. Con el rebote
    // tambien se pasa un poco, como la caja que la lleva.
    let talla = |x: Option<u32>, y: Option<u32>| match (x, y) {
        (Some(x), Some(y)) => Some(mezcla_u(x, y, p).clamp(4, 200)),
        (x, y) => if pc >= 500 { y } else { x },
    };
    s.font_size = talla(a.font_size, b.font_size);
    s.line_height = talla(a.line_height, b.line_height);
    s.stroke = opcion(a.stroke, b.stroke, pc);
    s.fill = opcion(a.fill, b.fill, pc);
    s
}
