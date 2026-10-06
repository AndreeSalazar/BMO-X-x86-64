//! **La geometria**: puntos, la matriz de SVG, el `transform`, el camino
//! `d` (con el arco `A`, S2) y las formas como caminos (S1).
//!
//! Un contorno se guarda en CURVAS, no en puntos: una transformacion afin
//! lleva una curva de Bezier a otra curva de Bezier exacta, asi que se gira
//! antes de aplanar y no se pierde nada. Y aplanar con un numero de tramos
//! DADO (`aplanar_con`) es lo que deja animar (S7): dos pasos del mismo
//! circulo salen con los mismos puntos aunque uno sea mas grande.

pub type P = (f64, f64);

/// La matriz de SVG: `x' = a x + c y + e`, `y' = b x + d y + f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matriz {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matriz {
    pub const UNO: Matriz = Matriz { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };
    pub fn mover(x: f64, y: f64) -> Matriz {
        Matriz { e: x, f: y, ..Matriz::UNO }
    }
    pub fn escala(x: f64, y: f64) -> Matriz {
        Matriz { a: x, d: y, ..Matriz::UNO }
    }
    pub fn giro(grados: f64) -> Matriz {
        let (s, c) = grados.to_radians().sin_cos();
        Matriz { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }
    /// `self` despues de `o`: primero `o`, luego `self`.
    pub fn por(&self, o: &Matriz) -> Matriz {
        Matriz {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            e: self.a * o.e + self.c * o.f + self.e,
            f: self.b * o.e + self.d * o.f + self.f,
        }
    }
    pub fn punto(&self, p: P) -> P {
        (self.a * p.0 + self.c * p.1 + self.e, self.b * p.0 + self.d * p.1 + self.f)
    }
    /// Un vector (sin mover).
    pub fn vector(&self, p: P) -> P {
        (self.a * p.0 + self.c * p.1, self.b * p.0 + self.d * p.1)
    }
    pub fn det(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }
    pub fn inversa(&self) -> Option<Matriz> {
        let det = self.det();
        if det.abs() < 1e-12 {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Matriz { a, b, c, d, e: -(a * self.e + c * self.f), f: -(b * self.e + d * self.f) })
    }
    /// Lo mas que estira en alguna direccion.
    pub fn estira(&self) -> f64 {
        let (p, q) = (self.a * self.a + self.b * self.b, self.c * self.c + self.d * self.d);
        let r = self.a * self.c + self.b * self.d;
        let m = (p + q) / 2.0;
        (m + ((p - q) * (p - q) / 4.0 + r * r).sqrt()).sqrt()
    }
    /// Si es un parecido: giro, escala IGUAL en los dos ejes y traslado (una
    /// pluma redonda sigue siendo redonda).
    pub fn parecido(&self) -> bool {
        let (p, q) = (self.a * self.a + self.b * self.b, self.c * self.c + self.d * self.d);
        let r = self.a * self.c + self.b * self.d;
        (p - q).abs() <= 1e-6 * p.max(q).max(1e-12) && r.abs() <= 1e-6 * p.max(1e-12)
    }
}

// ---------------------------------------------------------------------------
// Los numeros
// ---------------------------------------------------------------------------

/// Lee numeros de SVG seguidos (`10-5.5.5e2,3`).
pub struct Numeros<'a> {
    s: &'a [u8],
    pub i: usize,
}

impl<'a> Numeros<'a> {
    pub fn de(t: &'a str) -> Self {
        Numeros { s: t.as_bytes(), i: 0 }
    }
    fn blancos(&mut self) {
        while self.i < self.s.len() && (self.s[self.i].is_ascii_whitespace() || self.s[self.i] == b',') {
            self.i += 1;
        }
    }
    pub fn fin(&mut self) -> bool {
        self.blancos();
        self.i >= self.s.len()
    }
    pub fn mira(&mut self) -> Option<u8> {
        self.blancos();
        self.s.get(self.i).copied()
    }
    pub fn numero(&mut self) -> Option<f64> {
        self.blancos();
        let ini = self.i;
        let s = self.s;
        let mut k = self.i;
        if k < s.len() && (s[k] == b'-' || s[k] == b'+') {
            k += 1;
        }
        let mut cifras = false;
        while k < s.len() && s[k].is_ascii_digit() {
            k += 1;
            cifras = true;
        }
        if k < s.len() && s[k] == b'.' {
            k += 1;
            while k < s.len() && s[k].is_ascii_digit() {
                k += 1;
                cifras = true;
            }
        }
        if !cifras {
            return None;
        }
        if k < s.len() && (s[k] == b'e' || s[k] == b'E') {
            let mut j = k + 1;
            if j < s.len() && (s[j] == b'-' || s[j] == b'+') {
                j += 1;
            }
            if j < s.len() && s[j].is_ascii_digit() {
                while j < s.len() && s[j].is_ascii_digit() {
                    j += 1;
                }
                k = j;
            }
        }
        self.i = k;
        std::str::from_utf8(&s[ini..k]).ok()?.parse().ok()
    }
    /// Una bandera de arco: `0` o `1`, que puede ir pegada a lo que sigue.
    pub fn bandera(&mut self) -> Option<bool> {
        self.blancos();
        let c = *self.s.get(self.i)?;
        if c == b'0' || c == b'1' {
            self.i += 1;
            Some(c == b'1')
        } else {
            None
        }
    }
}

/// Una lista de numeros (`points`, `values`...). `None` si algo no lo es.
pub fn numeros(t: &str) -> Option<Vec<f64>> {
    let mut n = Numeros::de(t);
    let mut v = Vec::new();
    while !n.fin() {
        v.push(n.numero()?);
    }
    Some(v)
}

// ---------------------------------------------------------------------------
// transform
// ---------------------------------------------------------------------------

/// **Lee un `transform`** de SVG. `Err` con lo que no entiende.
pub fn transform(t: &str) -> Result<Matriz, String> {
    let mut m = Matriz::UNO;
    let mut resto = t.trim();
    while !resto.is_empty() {
        let abre = resto.find('(').ok_or_else(|| resto.to_string())?;
        let cierra = resto.find(')').ok_or_else(|| resto.to_string())?;
        let nombre = resto[..abre].trim().trim_start_matches(',').trim();
        let v = numeros(&resto[abre + 1..cierra]).ok_or_else(|| resto[..=cierra].to_string())?;
        let paso = match (nombre, v.as_slice()) {
            ("matrix", [a, b, c, d, e, f]) => Matriz { a: *a, b: *b, c: *c, d: *d, e: *e, f: *f },
            ("translate", [x]) => Matriz::mover(*x, 0.0),
            ("translate", [x, y]) => Matriz::mover(*x, *y),
            ("scale", [s]) => Matriz::escala(*s, *s),
            ("scale", [x, y]) => Matriz::escala(*x, *y),
            ("rotate", [g]) => Matriz::giro(*g),
            ("rotate", [g, x, y]) => Matriz::mover(*x, *y).por(&Matriz::giro(*g)).por(&Matriz::mover(-x, -y)),
            ("skewX", [g]) => Matriz { c: g.to_radians().tan(), ..Matriz::UNO },
            ("skewY", [g]) => Matriz { b: g.to_radians().tan(), ..Matriz::UNO },
            _ => return Err(resto[..=cierra].to_string()),
        };
        m = m.por(&paso);
        resto = resto[cierra + 1..].trim();
    }
    Ok(m)
}

// ---------------------------------------------------------------------------
// El contorno: subcaminos de curvas
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Linea(P),
    Cuadratica(P, P),
    Cubica(P, P, P),
}

impl Seg {
    pub fn fin(&self) -> P {
        match *self {
            Seg::Linea(p) | Seg::Cuadratica(_, p) | Seg::Cubica(_, _, p) => p,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sub {
    pub inicio: P,
    pub segs: Vec<Seg>,
    pub cerrado: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Contorno {
    pub subs: Vec<Sub>,
}

impl Contorno {
    pub fn transformado(&self, m: &Matriz) -> Contorno {
        let t = |p: P| m.punto(p);
        Contorno {
            subs: self
                .subs
                .iter()
                .map(|s| Sub {
                    inicio: t(s.inicio),
                    cerrado: s.cerrado,
                    segs: s
                        .segs
                        .iter()
                        .map(|g| match *g {
                            Seg::Linea(p) => Seg::Linea(t(p)),
                            Seg::Cuadratica(a, p) => Seg::Cuadratica(t(a), t(p)),
                            Seg::Cubica(a, b, p) => Seg::Cubica(t(a), t(b), t(p)),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// La caja de sus puntos de control (basta para `objectBoundingBox` de
    /// lineas; las curvas se miden aplanadas en `caja_exacta`).
    pub fn caja_exacta(&self) -> Option<(f64, f64, f64, f64)> {
        let (cs, _) = aplanar(self, 0.01);
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for c in &cs {
            for &(x, y) in c {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
        (x0 <= x1).then_some((x0, y0, x1 - x0, y1 - y0))
    }
}

/// Cuantos tramos rectos pide una curva para no separarse mas de `tol` de
/// su camino (la cuenta de Wang).
fn tramos(seg: &Seg, desde: P, tol: f64) -> usize {
    let segunda = |a: P, b: P, c: P| ((a.0 - 2.0 * b.0 + c.0).powi(2) + (a.1 - 2.0 * b.1 + c.1).powi(2)).sqrt();
    let n = match *seg {
        Seg::Linea(_) => return 1,
        Seg::Cuadratica(a, p) => (0.25 * segunda(desde, a, p) / tol).sqrt(),
        Seg::Cubica(a, b, p) => (0.75 * segunda(desde, a, b).max(segunda(a, b, p)) / tol).sqrt(),
    };
    (n.ceil() as usize).clamp(1, 256)
}

/// **Los tramos de cada curva** de un contorno con tolerancia `tol`.
pub fn conteos(c: &Contorno, tol: f64) -> Vec<Vec<usize>> {
    c.subs
        .iter()
        .map(|s| {
            let mut cur = s.inicio;
            s.segs
                .iter()
                .map(|g| {
                    let n = tramos(g, cur, tol);
                    cur = g.fin();
                    n
                })
                .collect()
        })
        .collect()
}

/// **Aplana con los tramos DADOS** (los de [`conteos`], o el maximo de los
/// de varios pasos de una animacion).
pub fn aplanar_con(c: &Contorno, n: &[Vec<usize>]) -> (Vec<Vec<P>>, Vec<bool>) {
    let mut caminos = Vec::new();
    let mut cerrados = Vec::new();
    for (k, s) in c.subs.iter().enumerate() {
        let mut p = vec![s.inicio];
        let mut cur = s.inicio;
        for (j, g) in s.segs.iter().enumerate() {
            let m = n.get(k).and_then(|v| v.get(j)).copied().unwrap_or(1).max(1);
            match *g {
                Seg::Linea(q) => p.push(q),
                Seg::Cuadratica(a, q) => {
                    for i in 1..=m {
                        let t = i as f64 / m as f64;
                        let u = 1.0 - t;
                        p.push((u * u * cur.0 + 2.0 * u * t * a.0 + t * t * q.0, u * u * cur.1 + 2.0 * u * t * a.1 + t * t * q.1));
                    }
                }
                Seg::Cubica(a, b, q) => {
                    for i in 1..=m {
                        let t = i as f64 / m as f64;
                        let u = 1.0 - t;
                        let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                        p.push((k0 * cur.0 + k1 * a.0 + k2 * b.0 + k3 * q.0, k0 * cur.1 + k1 * a.1 + k2 * b.1 + k3 * q.1));
                    }
                }
            }
            cur = g.fin();
        }
        caminos.push(p);
        cerrados.push(s.cerrado);
    }
    (caminos, cerrados)
}

pub fn aplanar(c: &Contorno, tol: f64) -> (Vec<Vec<P>>, Vec<bool>) {
    aplanar_con(c, &conteos(c, tol))
}

// ---------------------------------------------------------------------------
// El camino `d`, con el arco (S2)
// ---------------------------------------------------------------------------

/// **Lee un `d`**. `Err(pos)` donde deja de entenderlo.
pub fn camino(d: &str) -> Result<Contorno, usize> {
    let mut n = Numeros::de(d);
    let mut subs: Vec<Sub> = Vec::new();
    let mut actual: Option<Sub> = None;
    let (mut cur, mut inicio) = ((0.0, 0.0), (0.0, 0.0));
    let mut control: Option<(u8, P)> = None;
    let mut orden = 0u8;
    let empezar = |actual: &mut Option<Sub>, subs: &mut Vec<Sub>, en: P| {
        if let Some(s) = actual.take() {
            subs.push(s);
        }
        *actual = Some(Sub { inicio: en, segs: Vec::new(), cerrado: false });
    };
    loop {
        let Some(c) = n.mira() else { break };
        if c.is_ascii_alphabetic() {
            orden = c;
            n.i += 1;
            if c == b'Z' || c == b'z' {
                if let Some(mut s) = actual.take() {
                    s.cerrado = true;
                    subs.push(s);
                }
                cur = inicio;
                control = None;
                continue;
            }
        } else if orden == 0 {
            return Err(n.i);
        }
        let rel = orden.is_ascii_lowercase();
        let base = if rel { cur } else { (0.0, 0.0) };
        let antes = n.i;
        let par = |n: &mut Numeros| -> Option<P> { Some((base.0 + n.numero()?, base.1 + n.numero()?)) };
        if actual.is_none() && !matches!(orden, b'M' | b'm') {
            empezar(&mut actual, &mut subs, cur);
        }
        match orden.to_ascii_uppercase() {
            b'M' => {
                let q = par(&mut n).ok_or(n.i)?;
                empezar(&mut actual, &mut subs, q);
                cur = q;
                inicio = q;
                orden = if rel { b'l' } else { b'L' };
                control = None;
            }
            b'L' => {
                let q = par(&mut n).ok_or(n.i)?;
                actual.as_mut().unwrap().segs.push(Seg::Linea(q));
                cur = q;
                control = None;
            }
            b'H' => {
                let x = n.numero().ok_or(n.i)?;
                let q = (if rel { cur.0 + x } else { x }, cur.1);
                actual.as_mut().unwrap().segs.push(Seg::Linea(q));
                cur = q;
                control = None;
            }
            b'V' => {
                let y = n.numero().ok_or(n.i)?;
                let q = (cur.0, if rel { cur.1 + y } else { y });
                actual.as_mut().unwrap().segs.push(Seg::Linea(q));
                cur = q;
                control = None;
            }
            b'C' => {
                let (a, b, q) = (par(&mut n).ok_or(n.i)?, par(&mut n).ok_or(n.i)?, par(&mut n).ok_or(n.i)?);
                actual.as_mut().unwrap().segs.push(Seg::Cubica(a, b, q));
                control = Some((b'C', b));
                cur = q;
            }
            b'S' => {
                let (b, q) = (par(&mut n).ok_or(n.i)?, par(&mut n).ok_or(n.i)?);
                let a = match control {
                    Some((b'C', k)) => (2.0 * cur.0 - k.0, 2.0 * cur.1 - k.1),
                    _ => cur,
                };
                actual.as_mut().unwrap().segs.push(Seg::Cubica(a, b, q));
                control = Some((b'C', b));
                cur = q;
            }
            b'Q' => {
                let (a, q) = (par(&mut n).ok_or(n.i)?, par(&mut n).ok_or(n.i)?);
                actual.as_mut().unwrap().segs.push(Seg::Cuadratica(a, q));
                control = Some((b'Q', a));
                cur = q;
            }
            b'T' => {
                let q = par(&mut n).ok_or(n.i)?;
                let a = match control {
                    Some((b'Q', k)) => (2.0 * cur.0 - k.0, 2.0 * cur.1 - k.1),
                    _ => cur,
                };
                actual.as_mut().unwrap().segs.push(Seg::Cuadratica(a, q));
                control = Some((b'Q', a));
                cur = q;
            }
            b'A' => {
                let rx = n.numero().ok_or(n.i)?;
                let ry = n.numero().ok_or(n.i)?;
                let giro = n.numero().ok_or(n.i)?;
                let grande = n.bandera().ok_or(n.i)?;
                let horario = n.bandera().ok_or(n.i)?;
                let q = par(&mut n).ok_or(n.i)?;
                arco(&mut actual.as_mut().unwrap().segs, cur, rx, ry, giro, grande, horario, q);
                cur = q;
                control = None;
            }
            _ => return Err(antes.saturating_sub(1)),
        }
        if n.i == antes {
            return Err(n.i);
        }
    }
    if let Some(s) = actual.take() {
        subs.push(s);
    }
    Ok(Contorno { subs })
}

/// **El arco de SVG** (F.6 de la norma: de los extremos al centro), en
/// cubicas de un cuarto de vuelta como mucho.
#[allow(clippy::too_many_arguments)]
pub fn arco(segs: &mut Vec<Seg>, p0: P, rx: f64, ry: f64, giro: f64, grande: bool, horario: bool, p1: P) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || p0 == p1 {
        segs.push(Seg::Linea(p1));
        return;
    }
    let (s, c) = giro.to_radians().sin_cos();
    let (dx, dy) = ((p0.0 - p1.0) / 2.0, (p0.1 - p1.1) / 2.0);
    let (x1, y1) = (c * dx + s * dy, -s * dx + c * dy);
    let l = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if l > 1.0 {
        rx *= l.sqrt();
        ry *= l.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = if den == 0.0 { 0.0 } else { (num / den).sqrt() };
    if grande == horario {
        k = -k;
    }
    let (cx1, cy1) = (k * rx * y1 / ry, -k * ry * x1 / rx);
    let (cx, cy) = (c * cx1 - s * cy1 + (p0.0 + p1.0) / 2.0, s * cx1 + c * cy1 + (p0.1 + p1.1) / 2.0);
    let angulo = |ux: f64, uy: f64, vx: f64, vy: f64| {
        let a = (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
        a
    };
    let t1 = angulo(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = angulo((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
    if !horario && dt > 0.0 {
        dt -= std::f64::consts::TAU;
    } else if horario && dt < 0.0 {
        dt += std::f64::consts::TAU;
    }
    let trozos = (dt.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let paso = dt / trozos as f64;
    let kappa = 4.0 / 3.0 * (paso / 4.0).tan();
    let punto = |t: f64| {
        let (st, ct) = t.sin_cos();
        (cx + rx * ct * c - ry * st * s, cy + rx * ct * s + ry * st * c)
    };
    let derivada = |t: f64| {
        let (st, ct) = t.sin_cos();
        (-rx * st * c - ry * ct * s, -rx * st * s + ry * ct * c)
    };
    for i in 0..trozos {
        let (ta, tb) = (t1 + paso * i as f64, t1 + paso * (i + 1) as f64);
        let (pa, pb) = (punto(ta), if i + 1 == trozos { p1 } else { punto(tb) });
        let (da, db) = (derivada(ta), derivada(tb));
        segs.push(Seg::Cubica((pa.0 + kappa * da.0, pa.1 + kappa * da.1), (pb.0 - kappa * db.0, pb.1 - kappa * db.1), pb));
    }
}

// ---------------------------------------------------------------------------
// Las formas (S1)
// ---------------------------------------------------------------------------

/// Una elipse en cuatro cubicas, empezando a la derecha y en el sentido de
/// las agujas (el de SVG).
pub fn elipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Contorno {
    let k = 0.552_284_749_830_793_6;
    let (ox, oy) = (rx * k, ry * k);
    let segs = vec![
        Seg::Cubica((cx + rx, cy + oy), (cx + ox, cy + ry), (cx, cy + ry)),
        Seg::Cubica((cx - ox, cy + ry), (cx - rx, cy + oy), (cx - rx, cy)),
        Seg::Cubica((cx - rx, cy - oy), (cx - ox, cy - ry), (cx, cy - ry)),
        Seg::Cubica((cx + ox, cy - ry), (cx + rx, cy - oy), (cx + rx, cy)),
    ];
    Contorno { subs: vec![Sub { inicio: (cx + rx, cy), segs, cerrado: true }] }
}

/// Un rectangulo, con sus esquinas redondas si `rx`/`ry` (ya resueltos como
/// SVG: uno solo vale para los dos, y no pasan de la mitad del lado).
pub fn rectangulo(x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64) -> Contorno {
    if rx <= 0.0 || ry <= 0.0 {
        let segs = vec![Seg::Linea((x + w, y)), Seg::Linea((x + w, y + h)), Seg::Linea((x, y + h))];
        return Contorno { subs: vec![Sub { inicio: (x, y), segs, cerrado: true }] };
    }
    let k = 0.552_284_749_830_793_6;
    let (ox, oy) = (rx * k, ry * k);
    let segs = vec![
        Seg::Linea((x + w - rx, y)),
        Seg::Cubica((x + w - rx + ox, y), (x + w, y + ry - oy), (x + w, y + ry)),
        Seg::Linea((x + w, y + h - ry)),
        Seg::Cubica((x + w, y + h - ry + oy), (x + w - rx + ox, y + h), (x + w - rx, y + h)),
        Seg::Linea((x + rx, y + h)),
        Seg::Cubica((x + rx - ox, y + h), (x, y + h - ry + oy), (x, y + h - ry)),
        Seg::Linea((x, y + ry)),
        Seg::Cubica((x, y + ry - oy), (x + rx - ox, y), (x + rx, y)),
    ];
    Contorno { subs: vec![Sub { inicio: (x + rx, y), segs, cerrado: true }] }
}

/// `polyline` / `polygon`: sus puntos (`None` si no son pares de numeros).
pub fn poligono(puntos: &str, cerrado: bool) -> Option<Contorno> {
    let v = numeros(puntos)?;
    if v.len() < 2 {
        return Some(Contorno::default());
    }
    let pares: Vec<P> = v.chunks_exact(2).map(|p| (p[0], p[1])).collect();
    let segs = pares[1..].iter().map(|&p| Seg::Linea(p)).collect();
    Some(Contorno { subs: vec![Sub { inicio: pares[0], segs, cerrado }] })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_arco_de_un_semicirculo_pasa_por_donde_tiene_que_pasar() {
        let c = camino("M0 0 A10 10 0 0 1 20 0").unwrap();
        let (cs, _) = aplanar(&c, 0.01);
        let ultimo = *cs[0].last().unwrap();
        assert!((ultimo.0 - 20.0).abs() < 1e-9 && ultimo.1.abs() < 1e-9);
        // Horario en SVG (y hacia abajo): el semicirculo cae por ARRIBA (y < 0).
        let medio = cs[0].iter().map(|p| p.1).fold(f64::MAX, f64::min);
        assert!((medio + 10.0).abs() < 0.05, "{medio}");
    }

    #[test]
    fn las_banderas_pegadas_se_leen() {
        let c = camino("M0 0a5 5 0 1020 0").unwrap();
        assert_eq!(c.subs[0].segs.last().unwrap().fin(), (20.0, 0.0));
    }

    #[test]
    fn un_camino_roto_dice_donde() {
        assert!(camino("M0 0 L 5").is_err());
        assert!(camino("10 10").is_err());
    }

    #[test]
    fn transform_compuesto_y_giro_con_centro() {
        let m = transform("translate(10 0) rotate(90 5 5)").unwrap();
        let p = m.punto((5.0, 0.0));
        assert!((p.0 - 20.0).abs() < 1e-9 && (p.1 - 5.0).abs() < 1e-9, "{p:?}");
        assert!(transform("perspective(3)").is_err());
        assert!(Matriz::giro(30.0).por(&Matriz::escala(2.0, 2.0)).parecido());
        assert!(!Matriz::escala(2.0, 1.0).parecido());
    }

    #[test]
    fn el_circulo_aplanado_esta_en_su_radio() {
        let (cs, _) = aplanar(&elipse(0.0, 0.0, 50.0, 50.0), 0.05);
        for &(x, y) in &cs[0] {
            assert!(((x * x + y * y).sqrt() - 50.0).abs() < 0.1);
        }
    }
}

/// **Un `transform` de CSS** (`rotate(45deg) translate(10px, 0)`): los
/// angulos con su unidad (`deg`, `rad`, `turn`, `grad`), los largos en `px`.
pub fn transform_css(t: &str) -> Result<Matriz, String> {
    let mut m = Matriz::UNO;
    let mut resto = t.trim();
    let angulo = |v: &str| -> Option<f64> {
        let v = v.trim();
        for (u, k) in [("deg", 1.0), ("grad", 0.9), ("rad", 180.0 / std::f64::consts::PI), ("turn", 360.0)] {
            if let Some(n) = v.strip_suffix(u) {
                return n.trim().parse::<f64>().ok().map(|n| n * k);
            }
        }
        v.parse().ok()
    };
    let px = |v: &str| -> Option<f64> { v.trim().strip_suffix("px").unwrap_or(v.trim()).parse().ok() };
    while !resto.is_empty() {
        let abre = resto.find('(').ok_or_else(|| resto.to_string())?;
        let cierra = resto.find(')').ok_or_else(|| resto.to_string())?;
        let nombre = resto[..abre].trim();
        let args: Vec<&str> = resto[abre + 1..cierra].split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
        let malo = || resto[..=cierra].to_string();
        let n = |k: usize| px(args.get(k).copied().unwrap_or("")).ok_or_else(malo);
        let a = |k: usize| angulo(args.get(k).copied().unwrap_or("")).ok_or_else(malo);
        let paso = match (nombre, args.len()) {
            ("matrix", 6) => Matriz { a: n(0)?, b: n(1)?, c: n(2)?, d: n(3)?, e: n(4)?, f: n(5)? },
            ("translate", 1) => Matriz::mover(n(0)?, 0.0),
            ("translate", 2) => Matriz::mover(n(0)?, n(1)?),
            ("translateX", 1) => Matriz::mover(n(0)?, 0.0),
            ("translateY", 1) => Matriz::mover(0.0, n(0)?),
            ("scale", 1) => Matriz::escala(n(0)?, n(0)?),
            ("scale", 2) => Matriz::escala(n(0)?, n(1)?),
            ("scaleX", 1) => Matriz::escala(n(0)?, 1.0),
            ("scaleY", 1) => Matriz::escala(1.0, n(0)?),
            ("rotate", 1) => Matriz::giro(a(0)?),
            ("skewX", 1) => Matriz { c: a(0)?.to_radians().tan(), ..Matriz::UNO },
            ("skewY", 1) => Matriz { b: a(0)?.to_radians().tan(), ..Matriz::UNO },
            ("skew", 1) => Matriz { c: a(0)?.to_radians().tan(), ..Matriz::UNO },
            ("skew", 2) => Matriz { c: a(0)?.to_radians().tan(), b: a(1)?.to_radians().tan(), ..Matriz::UNO },
            _ => return Err(malo()),
        };
        m = m.por(&paso);
        resto = resto[cierra + 1..].trim();
    }
    Ok(m)
}

// ---------------------------------------------------------------------------
// El recorte convexo (`clipPath` de una figura convexa)
// ---------------------------------------------------------------------------

/// El area con signo de un poligono.
pub fn area(v: &[P]) -> f64 {
    let n = v.len();
    (0..n).map(|k| v[k].0 * v[(k + 1) % n].1 - v[(k + 1) % n].0 * v[k].1).sum::<f64>() / 2.0
}

/// **Si un poligono es convexo** (todas las vueltas hacia el mismo lado),
/// y en ese caso el mismo poligono en sentido positivo y sin puntos
/// repetidos.
pub fn convexo(v: &[P]) -> Option<Vec<P>> {
    let mut p: Vec<P> = Vec::new();
    for &q in v {
        if p.last().is_none_or(|u: &P| (u.0 - q.0).abs() > 1e-9 || (u.1 - q.1).abs() > 1e-9) {
            p.push(q);
        }
    }
    while p.len() > 1 && (p[0].0 - p[p.len() - 1].0).abs() < 1e-9 && (p[0].1 - p[p.len() - 1].1).abs() < 1e-9 {
        p.pop();
    }
    if p.len() < 3 {
        return None;
    }
    let n = p.len();
    let mut signo = 0.0f64;
    for k in 0..n {
        let (a, b, c) = (p[k], p[(k + 1) % n], p[(k + 2) % n]);
        let z = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
        if z.abs() < 1e-9 {
            continue;
        }
        if signo == 0.0 {
            signo = z.signum();
        } else if z.signum() != signo {
            return None;
        }
    }
    if area(&p) < 0.0 {
        p.reverse();
    }
    Some(p)
}

/// **Sutherland-Hodgman**: lo de `sujeto` que cae dentro del convexo
/// `recorte` (en sentido positivo). Recortar cada contorno de una figura
/// por un convexo deja las vueltas de cada punto de dentro como estaban:
/// la regla (`nonzero` o `evenodd`) sigue diciendo lo mismo.
pub fn recortar(sujeto: &[P], recorte: &[P]) -> Vec<P> {
    let mut salida = sujeto.to_vec();
    let n = recorte.len();
    for k in 0..n {
        if salida.is_empty() {
            break;
        }
        let (a, b) = (recorte[k], recorte[(k + 1) % n]);
        let dentro = |p: P| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0) >= 0.0;
        let corte = |p: P, q: P| {
            let (d1, d2) = ((b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0), (b.0 - a.0) * (q.1 - a.1) - (b.1 - a.1) * (q.0 - a.0));
            let t = d1 / (d1 - d2);
            (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t)
        };
        let entrada = std::mem::take(&mut salida);
        let m = entrada.len();
        for j in 0..m {
            let (p, q) = (entrada[j], entrada[(j + 1) % m]);
            match (dentro(p), dentro(q)) {
                (true, true) => salida.push(q),
                (true, false) => salida.push(corte(p, q)),
                (false, true) => {
                    salida.push(corte(p, q));
                    salida.push(q);
                }
                (false, false) => {}
            }
        }
    }
    salida
}
