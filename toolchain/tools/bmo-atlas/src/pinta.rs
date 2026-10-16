//! **Las curvas a pixeles, con el borde suave.**
//!
//! Un contorno de TrueType son puntos SOBRE la curva y puntos de CONTROL; dos
//! de control seguidos llevan uno sobre la curva en medio, implicito. Cada
//! tramo es una recta o una curva cuadratica, que se parte en rectas cortas
//! (`aplanar`). Lo de dentro se decide por la regla de la VUELTA (nonzero),
//! la de TrueType.
//!
//! El borde suave sale de MEDIR cuanto de cada pixel queda dentro: 16 lineas
//! por pixel de alto, y en cada una lo de dentro se mide EXACTO a lo ancho
//! (los trozos de pixel cuentan su parte). 255 es el pixel entero dentro.

use crate::ttf::Punto;

/// Un tramo recto, ya en pixeles (y hacia ABAJO).
pub type Tramo = ((f64, f64), (f64, f64));

/// Lineas por pixel de alto.
const LINEAS: usize = 16;

/// **Los contornos en tramos rectos**, pasados por `pon` (de unidades de la
/// letra a pixeles).
pub fn aplanar(contornos: &[Vec<Punto>], pon: impl Fn(f64, f64) -> (f64, f64)) -> Vec<Tramo> {
    let mut v = Vec::new();
    for c in contornos {
        let n = c.len();
        if n < 2 {
            continue;
        }
        // Donde empezar: un punto sobre la curva; si no hay, el medio de los
        // dos primeros.
        let (inicio, desde) = match c.iter().position(|p| p.2) {
            Some(i) => ((c[i].0, c[i].1), i),
            None => (((c[0].0 + c[1].0) / 2.0, (c[0].1 + c[1].1) / 2.0), 0),
        };
        let mut actual = inicio;
        let mut control: Option<(f64, f64)> = None;
        let linea = |a: (f64, f64), b: (f64, f64), v: &mut Vec<Tramo>| v.push((pon(a.0, a.1), pon(b.0, b.1)));
        let curva = |a: (f64, f64), k: (f64, f64), b: (f64, f64), v: &mut Vec<Tramo>| {
            let (pa, pk, pb) = (pon(a.0, a.1), pon(k.0, k.1), pon(b.0, b.1));
            let largo = ((pk.0 - pa.0).hypot(pk.1 - pa.1) + (pb.0 - pk.0).hypot(pb.1 - pk.1)).max(1.0);
            let pasos = (largo / 0.5).ceil().min(64.0) as usize;
            let mut antes = pa;
            for s in 1..=pasos {
                let t = s as f64 / pasos as f64;
                let u = 1.0 - t;
                let p = (u * u * pa.0 + 2.0 * u * t * pk.0 + t * t * pb.0, u * u * pa.1 + 2.0 * u * t * pk.1 + t * t * pb.1);
                v.push((antes, p));
                antes = p;
            }
        };
        for k in 1..=n {
            let p = c[(desde + k) % n];
            let q = (p.0, p.1);
            match (p.2, control) {
                (true, None) => {
                    linea(actual, q, &mut v);
                    actual = q;
                }
                (true, Some(k0)) => {
                    curva(actual, k0, q, &mut v);
                    actual = q;
                    control = None;
                }
                (false, None) => control = Some(q),
                (false, Some(k0)) => {
                    let medio = ((k0.0 + q.0) / 2.0, (k0.1 + q.1) / 2.0);
                    curva(actual, k0, medio, &mut v);
                    actual = medio;
                    control = Some(q);
                }
            }
        }
        // Cerrar hasta el inicio (por la curva, si quedaba un control).
        match control {
            Some(k0) => curva(actual, k0, inicio, &mut v),
            None if actual != inicio => linea(actual, inicio, &mut v),
            None => {}
        }
    }
    v
}

/// **Lo que cubre cada pixel** de una caja de `ancho` x `alto` cuya
/// esquina esta en (x0, y0): 0 nada, 255 entero.
pub fn cubrir(tramos: &[Tramo], x0: f64, y0: f64, ancho: usize, alto: usize) -> Vec<u8> {
    let mut acc = vec![0f64; ancho * alto];
    let mut cruces: Vec<(f64, i32)> = Vec::new();
    for fila in 0..alto {
        for j in 0..LINEAS {
            let y = y0 + fila as f64 + (j as f64 + 0.5) / LINEAS as f64;
            cruces.clear();
            for &((ax, ay), (bx, by)) in tramos {
                let (sube, baja) = (ay <= y && by > y, by <= y && ay > y);
                if sube || baja {
                    let x = ax + (y - ay) * (bx - ax) / (by - ay);
                    cruces.push((x - x0, if sube { 1 } else { -1 }));
                }
            }
            cruces.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut vueltas = 0;
            for w in 0..cruces.len() {
                vueltas += cruces[w].1;
                if vueltas != 0 && w + 1 < cruces.len() {
                    tramo_lleno(&mut acc[fila * ancho..(fila + 1) * ancho], cruces[w].0, cruces[w + 1].0);
                }
            }
        }
    }
    acc.iter().map(|&a| (a / LINEAS as f64 * 255.0).round().clamp(0.0, 255.0) as u8).collect()
}

/// Suma a cada pixel de la fila lo que de [a, b) cae en el.
fn tramo_lleno(fila: &mut [f64], a: f64, b: f64) {
    let ancho = fila.len() as f64;
    let (a, b) = (a.clamp(0.0, ancho), b.clamp(0.0, ancho));
    if b <= a {
        return;
    }
    let (ia, ib) = (a.floor() as usize, (b.ceil() as usize).min(fila.len()));
    for (i, px) in fila.iter_mut().enumerate().take(ib).skip(ia) {
        let (l, r) = (i as f64, i as f64 + 1.0);
        *px += (b.min(r) - a.max(l)).max(0.0);
    }
}
