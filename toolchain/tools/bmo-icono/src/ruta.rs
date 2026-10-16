//! **LA GEOMETRIA**: los numeros de un atributo, la matriz de `transform` y
//! el `d` de un `<path>` hecho POLIGONOS (las curvas y los arcos, en tramos
//! rectos tan cortos que a 32 pixeles no se ve el codo).

/// Un punto del plano.
pub type P = (f64, f64);

/// La matriz afin de SVG: `[a c e; b d f]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matriz(pub [f64; 6]);

impl Matriz {
    pub const UNO: Matriz = Matriz([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn aplica(&self, p: P) -> P {
        let m = &self.0;
        (m[0] * p.0 + m[2] * p.1 + m[4], m[1] * p.0 + m[3] * p.1 + m[5])
    }

    /// `self` despues de `o`: primero `o`, luego `self`.
    pub fn por(&self, o: &Matriz) -> Matriz {
        let (a, b) = (&self.0, &o.0);
        Matriz([
            a[0] * b[0] + a[2] * b[1],
            a[1] * b[0] + a[3] * b[1],
            a[0] * b[2] + a[2] * b[3],
            a[1] * b[2] + a[3] * b[3],
            a[0] * b[4] + a[2] * b[5] + a[4],
            a[1] * b[4] + a[3] * b[5] + a[5],
        ])
    }

    pub fn inversa(&self) -> Option<Matriz> {
        let m = &self.0;
        let det = m[0] * m[3] - m[1] * m[2];
        if det.abs() < 1e-12 {
            return None;
        }
        let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
        Some(Matriz([a, b, c, d, -(a * m[4] + c * m[5]), -(b * m[4] + d * m[5])]))
    }

    /// Cuanto agranda un largo (para el ancho del trazo).
    pub fn escala(&self) -> f64 {
        let m = &self.0;
        (m[0] * m[3] - m[1] * m[2]).abs().sqrt()
    }
}

/// **Los numeros de un texto**, como los escribe SVG: `1.5.5` son dos,
/// `-1-2` son dos, `1e-3` es uno, y comas o espacios en medio.
pub fn numeros(s: &str) -> Vec<f64> {
    let mut v = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match leer_numero(b, i) {
            Some((n, fin)) => {
                v.push(n);
                i = fin;
            }
            None => i += 1,
        }
    }
    v
}

/// Un numero desde `i` (saltando separadores); `None` si ahi no empieza uno.
fn leer_numero(b: &[u8], mut i: usize) -> Option<(f64, usize)> {
    while i < b.len() && (b[i] == b' ' || b[i] == b',' || b[i] == b'\t' || b[i] == b'\n' || b[i] == b'\r') {
        i += 1;
    }
    let ini = i;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let mut cifras = false;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        cifras = true;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            cifras = true;
        }
    }
    if !cifras {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'-' || b[j] == b'+') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            i = j;
        }
    }
    std::str::from_utf8(&b[ini..i]).ok()?.parse().ok().map(|n| (n, i))
}

/// **`transform="..."`**: `matrix`, `translate`, `scale`, `rotate`, `skewX`
/// y `skewY`, encadenadas de izquierda a derecha como manda SVG.
pub fn transformacion(s: &str) -> Matriz {
    let mut m = Matriz::UNO;
    let mut resto = s;
    while let Some(abre) = resto.find('(') {
        let nombre = resto[..abre].trim().trim_start_matches(',').trim();
        let Some(cierra) = resto[abre..].find(')') else { break };
        let n = numeros(&resto[abre + 1..abre + cierra]);
        let g = |i: usize, d: f64| n.get(i).copied().unwrap_or(d);
        let t = match nombre {
            "matrix" if n.len() == 6 => Matriz([n[0], n[1], n[2], n[3], n[4], n[5]]),
            "translate" => Matriz([1.0, 0.0, 0.0, 1.0, g(0, 0.0), g(1, 0.0)]),
            "scale" => Matriz([g(0, 1.0), 0.0, 0.0, g(1, g(0, 1.0)), 0.0, 0.0]),
            "rotate" => {
                let (s, c) = g(0, 0.0).to_radians().sin_cos();
                let (cx, cy) = (g(1, 0.0), g(2, 0.0));
                let r = Matriz([c, s, -s, c, 0.0, 0.0]);
                Matriz([1.0, 0.0, 0.0, 1.0, cx, cy]).por(&r).por(&Matriz([1.0, 0.0, 0.0, 1.0, -cx, -cy]))
            }
            "skewX" => Matriz([1.0, 0.0, g(0, 0.0).to_radians().tan(), 1.0, 0.0, 0.0]),
            "skewY" => Matriz([1.0, g(0, 0.0).to_radians().tan(), 0.0, 1.0, 0.0, 0.0]),
            _ => Matriz::UNO,
        };
        m = m.por(&t);
        resto = &resto[abre + cierra + 1..];
    }
    m
}

/// Cuantos tramos rectos por curva: a 32 pixeles, 16 sobran.
const TRAMOS: usize = 16;

/// **El `d` de un `<path>`** hecho contornos (cada uno, una lista de puntos;
/// `true` si se cerro con `Z`). Lo que no se entiende corta ahi, sin inventar.
pub fn camino(d: &str) -> Vec<(Vec<P>, bool)> {
    let b = d.as_bytes();
    let mut out: Vec<(Vec<P>, bool)> = Vec::new();
    let mut actual: Vec<P> = Vec::new();
    let (mut x, mut y) = (0.0, 0.0);
    let mut inicio = (0.0, 0.0);
    // El ultimo punto de control, para `S` y `T`.
    let mut control: Option<P> = None;
    let mut orden = 0u8;
    let mut i = 0;
    let cerrar = |actual: &mut Vec<P>, out: &mut Vec<(Vec<P>, bool)>, z: bool| {
        if actual.len() > 1 {
            out.push((std::mem::take(actual), z));
        } else {
            actual.clear();
        }
    };
    loop {
        while i < b.len() && (b[i] == b' ' || b[i] == b',' || b[i].is_ascii_whitespace()) {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        if b[i].is_ascii_alphabetic() {
            orden = b[i];
            i += 1;
            if orden == b'Z' || orden == b'z' {
                cerrar(&mut actual, &mut out, true);
                x = inicio.0;
                y = inicio.1;
                control = None;
                continue;
            }
        }
        let rel = orden.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        let cuantos = match orden.to_ascii_uppercase() {
            b'M' | b'L' | b'T' => 2,
            b'H' | b'V' => 1,
            b'S' | b'Q' => 4,
            b'C' => 6,
            b'A' => 7,
            _ => break,
        };
        let mut n = [0.0f64; 7];
        for k in 0..cuantos {
            // Los dos indicadores del arco son UNA cifra cada uno y pueden ir
            // pegados (`a1 1 0 011 1`): se leen a mano.
            if orden.to_ascii_uppercase() == b'A' && (k == 3 || k == 4) {
                while i < b.len() && (b[i] == b' ' || b[i] == b',' || b[i].is_ascii_whitespace()) {
                    i += 1;
                }
                if i < b.len() && (b[i] == b'0' || b[i] == b'1') {
                    n[k] = (b[i] - b'0') as f64;
                    i += 1;
                    continue;
                }
                return { cerrar(&mut actual, &mut out, false); out };
            }
            match leer_numero(b, i) {
                Some((v, fin)) => {
                    n[k] = v;
                    i = fin;
                }
                None => {
                    cerrar(&mut actual, &mut out, false);
                    return out;
                }
            }
        }
        let mut sig_control = None;
        match orden.to_ascii_uppercase() {
            b'M' => {
                cerrar(&mut actual, &mut out, false);
                x = ox + n[0];
                y = oy + n[1];
                inicio = (x, y);
                actual.push((x, y));
                // Las parejas que siguen a un `M` son `L`.
                orden = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                x = ox + n[0];
                y = oy + n[1];
                actual.push((x, y));
            }
            b'H' => {
                x = if rel { x + n[0] } else { n[0] };
                actual.push((x, y));
            }
            b'V' => {
                y = if rel { y + n[0] } else { n[0] };
                actual.push((x, y));
            }
            b'C' | b'S' => {
                let (c1, c2, fin) = if orden.to_ascii_uppercase() == b'C' {
                    ((ox + n[0], oy + n[1]), (ox + n[2], oy + n[3]), (ox + n[4], oy + n[5]))
                } else {
                    let c1 = match control {
                        Some((cx, cy)) => (2.0 * x - cx, 2.0 * y - cy),
                        None => (x, y),
                    };
                    (c1, (ox + n[0], oy + n[1]), (ox + n[2], oy + n[3]))
                };
                empezado(&mut actual, (x, y));
                for t in 1..=TRAMOS {
                    let t = t as f64 / TRAMOS as f64;
                    let u = 1.0 - t;
                    let (a, bb, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    actual.push((a * x + bb * c1.0 + c * c2.0 + d * fin.0, a * y + bb * c1.1 + c * c2.1 + d * fin.1));
                }
                sig_control = Some(c2);
                x = fin.0;
                y = fin.1;
            }
            b'Q' | b'T' => {
                let (c, fin) = if orden.to_ascii_uppercase() == b'Q' {
                    ((ox + n[0], oy + n[1]), (ox + n[2], oy + n[3]))
                } else {
                    let c = match control {
                        Some((cx, cy)) => (2.0 * x - cx, 2.0 * y - cy),
                        None => (x, y),
                    };
                    (c, (ox + n[0], oy + n[1]))
                };
                empezado(&mut actual, (x, y));
                for t in 1..=TRAMOS {
                    let t = t as f64 / TRAMOS as f64;
                    let u = 1.0 - t;
                    actual.push((u * u * x + 2.0 * u * t * c.0 + t * t * fin.0, u * u * y + 2.0 * u * t * c.1 + t * t * fin.1));
                }
                sig_control = Some(c);
                x = fin.0;
                y = fin.1;
            }
            _ => {
                // 'A'
                let fin = (ox + n[5], oy + n[6]);
                empezado(&mut actual, (x, y));
                arco((x, y), n[0], n[1], n[2], n[3] != 0.0, n[4] != 0.0, fin, &mut actual);
                x = fin.0;
                y = fin.1;
            }
        }
        control = sig_control;
    }
    cerrar(&mut actual, &mut out, false);
    out
}

/// Un contorno que empieza sin `M` (despues de un `Z`) arranca donde se quedo.
fn empezado(actual: &mut Vec<P>, p: P) {
    if actual.is_empty() {
        actual.push(p);
    }
}

/// **El arco de SVG** (de extremo a extremo), pasado a centro y angulos como
/// dice el apendice F.6.5 de la norma, y troceado.
#[allow(clippy::too_many_arguments)]
pub fn arco(de: P, rx: f64, ry: f64, giro: f64, grande: bool, barrido: bool, a: P, out: &mut Vec<P>) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-9 || ry < 1e-9 || (de.0 - a.0).abs() + (de.1 - a.1).abs() < 1e-12 {
        out.push(a);
        return;
    }
    let (s, c) = giro.to_radians().sin_cos();
    let (dx, dy) = ((de.0 - a.0) / 2.0, (de.1 - a.1) / 2.0);
    let (x1, y1) = (c * dx + s * dy, -s * dx + c * dy);
    let l = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if l > 1.0 {
        rx *= l.sqrt();
        ry *= l.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = (num / den).sqrt();
    if grande == barrido {
        k = -k;
    }
    let (cx1, cy1) = (k * rx * y1 / ry, -k * ry * x1 / rx);
    let (cx, cy) = (c * cx1 - s * cy1 + (de.0 + a.0) / 2.0, s * cx1 + c * cy1 + (de.1 + a.1) / 2.0);
    let angulo = |ux: f64, uy: f64, vx: f64, vy: f64| {
        let d = (ux * vx + uy * vy) / ((ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt());
        let t = d.clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 { -t } else { t }
    };
    let t1 = angulo(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = angulo((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
    if !barrido && dt > 0.0 {
        dt -= std::f64::consts::TAU;
    } else if barrido && dt < 0.0 {
        dt += std::f64::consts::TAU;
    }
    let n = ((dt.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize * TRAMOS / 2).max(4);
    for i in 1..=n {
        let t = t1 + dt * i as f64 / n as f64;
        let (st, ct) = t.sin_cos();
        out.push((cx + rx * ct * c - ry * st * s, cy + rx * ct * s + ry * st * c));
    }
}

/// **Una elipse** como contorno cerrado.
pub fn elipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<P> {
    let n = 64;
    (0..n)
        .map(|i| {
            let t = std::f64::consts::TAU * i as f64 / n as f64;
            (cx + rx * t.cos(), cy + ry * t.sin())
        })
        .collect()
}

/// **Un rectangulo** con sus esquinas redondas (`rx`, `ry`).
pub fn rectangulo(x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64) -> Vec<P> {
    let (rx, ry) = (rx.min(w / 2.0).max(0.0), ry.min(h / 2.0).max(0.0));
    if rx < 1e-9 || ry < 1e-9 {
        return vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    }
    let mut v = Vec::new();
    let esquina = |v: &mut Vec<P>, cx: f64, cy: f64, desde: f64| {
        for i in 0..=8 {
            let t = (desde + 90.0 * i as f64 / 8.0).to_radians();
            v.push((cx + rx * t.cos(), cy + ry * t.sin()));
        }
    };
    esquina(&mut v, x + w - rx, y + ry, 270.0);
    esquina(&mut v, x + w - rx, y + h - ry, 0.0);
    esquina(&mut v, x + rx, y + h - ry, 90.0);
    esquina(&mut v, x + rx, y + ry, 180.0);
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_numeros_pegados_de_svg() {
        assert_eq!(numeros("1.5.5-2,3e1 -.5"), vec![1.5, 0.5, -2.0, 30.0, -0.5]);
    }

    #[test]
    fn un_cuadrado_relativo_y_cerrado() {
        let c = camino("m2 2h4v4h-4z");
        assert_eq!(c.len(), 1);
        assert!(c[0].1);
        assert_eq!(c[0].0, vec![(2.0, 2.0), (6.0, 2.0), (6.0, 6.0), (2.0, 6.0)]);
    }

    #[test]
    fn el_arco_acaba_donde_dice() {
        let c = camino("M0 10 A10 10 0 0 1 20 10");
        let ultimo = *c[0].0.last().unwrap();
        assert!((ultimo.0 - 20.0).abs() < 1e-9 && (ultimo.1 - 10.0).abs() < 1e-9);
        // Medio circulo por ARRIBA (barrido 1 en y hacia abajo): pasa por (10, 0).
        assert!(c[0].0.iter().any(|p| (p.0 - 10.0).abs() < 0.5 && p.1.abs() < 0.5));
    }

    #[test]
    fn traslado_y_escala_encadenados() {
        let m = transformacion("translate(10 20) scale(2)");
        assert_eq!(m.aplica((1.0, 1.0)), (12.0, 22.0));
        assert_eq!(m.inversa().unwrap().aplica((12.0, 22.0)), (1.0, 1.0));
    }
}
