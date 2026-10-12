//! **SIN SOLAPE** (12-10): la mezcla de la 3060 lee, en el sombreador, lo
//! que habia en el destino ANTES del lote (`bmo_gpu_ga10x::mezcla`). Si dos
//! triangulos del mismo lote pintan el mismo pixel, el segundo tendria que
//! leer lo que dejo el primero -- el ROP lo ordena, el sombreador no --. Asi
//! que la puerta solo deja mezclar un lote donde ningun pixel se pinta dos
//! veces, y eso se mira aqui, con las posiciones que da la CPU:
//!
//! ```text
//!    las posiciones  el de vertice de la CPU (el interprete de la casa) en
//!                    cada vertice del lote, por el viewport, a pixeles
//!    dos triangulos  no se pisan si sus cajas (un pixel mas a cada lado) no se
//!                    tocan, o si comparten un LADO (dos vertices iguales,
//!                    bit a bit) y sus otros vertices caen a los dos lados:
//!                    la regla de arriba-izquierda da cada pixel del lado a
//!                    uno solo (un cuadro de dos triangulos, la pantalla
//!                    completa, los cuadros de la interfaz)
//!    lo demas        se pisan (o no se sabe): por la CPU
//! ```
//!
//! Un triangulo sin area no pinta nada y no cuenta. Un vertice detras de la
//! camara (w <= 0) o que no es un numero, no se sabe: por la CPU.

use alloc::vec::Vec;

use bmo_proton_x::lote::Lote;

/// Lo mas que se mira (por parejas): mas, por la CPU.
pub const MAX_TRIANGULOS: usize = 256;

/// Por que un lote que mezcla no va a la 3060.
pub const SE_PISAN: &str = "la mezcla con triangulos que se pisan (el pixel de uno lee lo que dejo el otro): por la CPU";
pub const MUCHOS: &str = "la mezcla en un lote de mas de 256 triangulos (la puerta no mira si se pisan): por la CPU";
pub const NO_SE_SABE: &str = "la mezcla con un vertice detras de la camara o que no es un numero: por la CPU";

/// Un triangulo en pixeles, y su caja.
struct Tri {
    v: [[f64; 2]; 3],
    caja: [f64; 4],
}

/// De que lado de la recta `a -> b` cae `p` (el signo del producto).
fn lado(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Si comparten un lado y quedan a sus dos lados.
fn lado_comun(x: &Tri, y: &Tri) -> bool {
    for i in 0..3 {
        let (a, b, c) = (x.v[i], x.v[(i + 1) % 3], x.v[(i + 2) % 3]);
        for j in 0..3 {
            let (p, q, r) = (y.v[j], y.v[(j + 1) % 3], y.v[(j + 2) % 3]);
            if (a == p && b == q) || (a == q && b == p) {
                let (s1, s2) = (lado(a, b, c), lado(a, b, r));
                return s1 * s2 < 0.0;
            }
        }
    }
    false
}

/// **Si ningun pixel del lote se pinta dos veces**, o por que no se puede
/// decir. Solo lo pide un lote que mezcla.
pub fn sin_solape(l: &Lote) -> Result<(), &'static str> {
    let en = l.enlace;
    let n = l.ids.len() / 3;
    if n > MAX_TRIANGULOS {
        return Err(MUCHOS);
    }
    if n <= 1 {
        return Ok(());
    }
    let [x0, y0, ancho, alto, _, _] = l.reglas.viewport;
    let mut regs = Vec::new();
    let mut vistos: Vec<(u32, [f64; 2])> = Vec::new();
    let mut posicion = |id: u32| -> Result<[f64; 2], &'static str> {
        if let Some(&(_, p)) = vistos.iter().find(|x| x.0 == id) {
            return Ok(p);
        }
        let ent: Vec<[f32; 4]> = en.desde_ia.iter().map(|&f| bmo_proton_x::lote::entrada(l, f, id, 0)).collect();
        let mut salidas = alloc::vec![[0f32; 4]; en.vs.salidas.max(1)];
        en.vs.correr(&ent, l.cb, &mut salidas, &mut regs);
        let [x, y, _, w] = salidas.get(en.posicion as usize).copied().unwrap_or([f32::NAN; 4]);
        if !(w > 0.0) || !x.is_finite() || !y.is_finite() || !w.is_finite() {
            return Err(NO_SE_SABE);
        }
        let (x, y, w) = (x as f64, y as f64, w as f64);
        let p = [x0 as f64 + (x / w * 0.5 + 0.5) * ancho as f64, y0 as f64 + (0.5 - y / w * 0.5) * alto as f64];
        vistos.push((id, p));
        Ok(p)
    };
    let mut tris = Vec::with_capacity(n);
    for t in l.ids.chunks_exact(3) {
        let v = [posicion(t[0])?, posicion(t[1])?, posicion(t[2])?];
        if lado(v[0], v[1], v[2]) == 0.0 {
            continue;
        }
        let caja = [
            v.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min) - 1.0,
            v.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min) - 1.0,
            v.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max) + 1.0,
            v.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max) + 1.0,
        ];
        tris.push(Tri { v, caja });
    }
    for i in 0..tris.len() {
        for j in i + 1..tris.len() {
            let (a, b) = (&tris[i], &tris[j]);
            let separadas = a.caja[2] <= b.caja[0] || b.caja[2] <= a.caja[0] || a.caja[3] <= b.caja[1] || b.caja[3] <= a.caja[1];
            if !separadas && !lado_comun(a, b) {
                return Err(SE_PISAN);
            }
        }
    }
    Ok(())
}
