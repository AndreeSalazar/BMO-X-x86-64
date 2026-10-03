//! **LOS CAMINOS DE LA MAQUETA**: el atributo `d` de un `<path>` de SVG,
//! leido aqui y dibujado con la pluma de la casa.
//!
//! El gato hucha y los iconos de las maquetas son trazos de SVG escritos a
//! mano (por nosotros: `docs/arte/`). Copiarlos a ojo en pixeles es lo que
//! los hacia distintos; leerlos tal cual los hace IGUALES. Es un lector, no
//! un navegador: `M L H V C S Q T Z` (y sus minusculas). Los arcos `A` no
//! se usan en las maquetas y no se leen.
//!
//! Los puntos salen en 1/64 de unidad del `viewBox`; quien dibuja los
//! escala, los gira (la pata que saluda) y los pone donde caen.

use alloc::vec::Vec;

/// Un subcamino: sus puntos y si se cierra (`Z`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sub {
    pub puntos: Vec<(i32, i32)>,
    pub cerrado: bool,
}

struct Lector<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lector<'_> {
    fn blancos(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] == b' ' || self.s[self.i] == b',' || self.s[self.i] == b'\n' || self.s[self.i] == b'\t') {
            self.i += 1;
        }
    }

    /// Un numero, en 1/64. `None` si lo que sigue no es un numero.
    fn numero(&mut self) -> Option<i32> {
        self.blancos();
        let ini = self.i;
        let mut neg = false;
        if self.i < self.s.len() && (self.s[self.i] == b'-' || self.s[self.i] == b'+') {
            neg = self.s[self.i] == b'-';
            self.i += 1;
        }
        let mut entero: i64 = 0;
        let mut hay = false;
        while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
            entero = entero * 10 + (self.s[self.i] - b'0') as i64;
            self.i += 1;
            hay = true;
        }
        let (mut frac, mut div) = (0i64, 1i64);
        if self.i < self.s.len() && self.s[self.i] == b'.' {
            self.i += 1;
            while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                if div < 1_000_000 {
                    frac = frac * 10 + (self.s[self.i] - b'0') as i64;
                    div *= 10;
                }
                self.i += 1;
                hay = true;
            }
        }
        if !hay {
            self.i = ini;
            return None;
        }
        let v = entero * 64 + (frac * 64 + div / 2) / div;
        Some(if neg { -v } else { v } as i32)
    }

    fn orden(&mut self) -> Option<u8> {
        self.blancos();
        let c = *self.s.get(self.i)?;
        if c.is_ascii_alphabetic() {
            self.i += 1;
            Some(c)
        } else {
            None
        }
    }
}

/// Cuantos tramos rectos para una curva cuyo poligono de control mide `l`
/// (1/64): uno cada ~3 unidades, entre 4 y 48.
fn tramos(l: i64) -> i32 {
    ((l / (64 * 3)) as i32).clamp(4, 48)
}

fn largo(a: (i32, i32), b: (i32, i32)) -> i64 {
    let (dx, dy) = ((b.0 - a.0) as i64, (b.1 - a.1) as i64);
    // Por arriba, sin raiz: basta para contar tramos.
    dx.abs().max(dy.abs()) + dx.abs().min(dy.abs()) / 2
}

fn cubica(p: &mut Vec<(i32, i32)>, a: (i32, i32), b: (i32, i32), c: (i32, i32), d: (i32, i32)) {
    let n = tramos(largo(a, b) + largo(b, c) + largo(c, d)) as i64;
    for k in 1..=n {
        let (t, u) = (k, n - k);
        let f = |a: i32, b: i32, c: i32, d: i32| {
            ((u * u * u * a as i64 + 3 * u * u * t * b as i64 + 3 * u * t * t * c as i64 + t * t * t * d as i64) / (n * n * n)) as i32
        };
        p.push((f(a.0, b.0, c.0, d.0), f(a.1, b.1, c.1, d.1)));
    }
}

fn cuadratica(p: &mut Vec<(i32, i32)>, a: (i32, i32), b: (i32, i32), c: (i32, i32)) {
    let n = tramos(largo(a, b) + largo(b, c)) as i64;
    for k in 1..=n {
        let (t, u) = (k, n - k);
        let f = |a: i32, b: i32, c: i32| ((u * u * a as i64 + 2 * u * t * b as i64 + t * t * c as i64) / (n * n)) as i32;
        p.push((f(a.0, b.0, c.0), f(a.1, b.1, c.1)));
    }
}

/// **Lee** un camino de SVG. Lo que no entiende lo deja (y lo que leyo hasta
/// ahi vale): una maqueta rota no tumba la app.
pub fn camino(d: &str) -> Vec<Sub> {
    let mut l = Lector { s: d.as_bytes(), i: 0 };
    let mut subs: Vec<Sub> = Vec::new();
    let mut p: Vec<(i32, i32)> = Vec::new();
    let mut cur = (0, 0);
    let mut inicio = (0, 0);
    // El ultimo punto de control, para S y T.
    let mut control: Option<(u8, (i32, i32))> = None;
    let mut orden = b'M';
    let cerrar = |subs: &mut Vec<Sub>, p: &mut Vec<(i32, i32)>, cerrado: bool| {
        if p.len() > 1 || (p.len() == 1 && cerrado) {
            subs.push(Sub { puntos: core::mem::take(p), cerrado });
        } else {
            p.clear();
        }
    };
    loop {
        if let Some(o) = l.orden() {
            orden = o;
            if o == b'Z' || o == b'z' {
                cerrar(&mut subs, &mut p, true);
                cur = inicio;
                control = None;
                continue;
            }
        } else if l.i >= l.s.len() {
            break;
        }
        let rel = orden.is_ascii_lowercase();
        let base = if rel { cur } else { (0, 0) };
        let mut lee = |l: &mut Lector| l.numero();
        let par = |l: &mut Lector, lee: &mut dyn FnMut(&mut Lector) -> Option<i32>| -> Option<(i32, i32)> {
            let x = lee(l)?;
            let y = lee(l)?;
            Some((base.0 + x, base.1 + y))
        };
        let antes = l.i;
        match orden.to_ascii_uppercase() {
            b'M' => {
                let Some(q) = par(&mut l, &mut lee) else { break };
                cerrar(&mut subs, &mut p, false);
                p.push(q);
                cur = q;
                inicio = q;
                // Lo que sigue a una M son L.
                orden = if rel { b'l' } else { b'L' };
                control = None;
            }
            b'L' => {
                let Some(q) = par(&mut l, &mut lee) else { break };
                if p.is_empty() {
                    p.push(cur);
                }
                p.push(q);
                cur = q;
                control = None;
            }
            b'H' => {
                let Some(x) = lee(&mut l) else { break };
                let q = (if rel { cur.0 + x } else { x }, cur.1);
                if p.is_empty() {
                    p.push(cur);
                }
                p.push(q);
                cur = q;
                control = None;
            }
            b'V' => {
                let Some(y) = lee(&mut l) else { break };
                let q = (cur.0, if rel { cur.1 + y } else { y });
                if p.is_empty() {
                    p.push(cur);
                }
                p.push(q);
                cur = q;
                control = None;
            }
            b'C' => {
                let (Some(b), Some(c), Some(d)) = (par(&mut l, &mut lee), par(&mut l, &mut lee), par(&mut l, &mut lee)) else { break };
                if p.is_empty() {
                    p.push(cur);
                }
                cubica(&mut p, cur, b, c, d);
                control = Some((b'C', c));
                cur = d;
            }
            b'S' => {
                let (Some(c), Some(d)) = (par(&mut l, &mut lee), par(&mut l, &mut lee)) else { break };
                let b = match control {
                    Some((b'C', k)) => (2 * cur.0 - k.0, 2 * cur.1 - k.1),
                    _ => cur,
                };
                if p.is_empty() {
                    p.push(cur);
                }
                cubica(&mut p, cur, b, c, d);
                control = Some((b'C', c));
                cur = d;
            }
            b'Q' => {
                let (Some(b), Some(c)) = (par(&mut l, &mut lee), par(&mut l, &mut lee)) else { break };
                if p.is_empty() {
                    p.push(cur);
                }
                cuadratica(&mut p, cur, b, c);
                control = Some((b'Q', b));
                cur = c;
            }
            b'T' => {
                let Some(c) = par(&mut l, &mut lee) else { break };
                let b = match control {
                    Some((b'Q', k)) => (2 * cur.0 - k.0, 2 * cur.1 - k.1),
                    _ => cur,
                };
                if p.is_empty() {
                    p.push(cur);
                }
                cuadratica(&mut p, cur, b, c);
                control = Some((b'Q', b));
                cur = c;
            }
            _ => break,
        }
        if l.i == antes {
            break;
        }
    }
    cerrar(&mut subs, &mut p, false);
    subs
}

/// **Rellena** unos subcaminos (par-impar), suavizado: cuatro sub-filas por
/// pixel y, en cada una, la parte exacta de cada pixel que el tramo cubre.
/// Los puntos, en 1/64 de pixel.
pub fn rellenar(subs: &[Vec<(i32, i32)>], mut pon: impl FnMut(i32, i32, u8)) {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for s in subs {
        for &(x, y) in s {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 > x1 {
        return;
    }
    let (px0, py0) = (x0.div_euclid(64), y0.div_euclid(64));
    let (px1, py1) = (x1.div_euclid(64) + 1, y1.div_euclid(64) + 1);
    let w = (px1 - px0) as usize;
    let mut acum = Vec::new();
    acum.resize(w, 0u32);
    let mut cortes: Vec<i32> = Vec::new();
    for py in py0..py1 {
        acum.iter_mut().for_each(|a| *a = 0);
        for sub in 0..4 {
            let y = py * 64 + 8 + sub * 16;
            cortes.clear();
            for s in subs {
                let n = s.len();
                if n < 2 {
                    continue;
                }
                for k in 0..n {
                    let (a, b) = (s[k], s[(k + 1) % n]);
                    if (a.1 <= y) != (b.1 <= y) {
                        let x = a.0 as i64 + (b.0 - a.0) as i64 * (y - a.1) as i64 / (b.1 - a.1) as i64;
                        cortes.push(x as i32);
                    }
                }
            }
            cortes.sort_unstable();
            for par in cortes.chunks_exact(2) {
                let (a, b) = (par[0].max(px0 * 64), par[1].min(px1 * 64));
                if a >= b {
                    continue;
                }
                let (ca, cb) = (a.div_euclid(64), (b - 1).div_euclid(64));
                for c in ca..=cb {
                    let ini = a.max(c * 64);
                    let fin = b.min(c * 64 + 64);
                    acum[(c - px0) as usize] += (fin - ini) as u32;
                }
            }
        }
        for (i, &a) in acum.iter().enumerate() {
            if a != 0 {
                pon(px0 + i as i32, py, (a * 255 / 256).min(255) as u8);
            }
        }
    }
}
