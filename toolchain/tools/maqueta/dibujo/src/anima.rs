//! **S7: animar un dibujo** -- `<animate>`, `<animateTransform>` y los
//! `@keyframes` de CSS, convertidos AL COMPILAR en pasos.
//!
//! El aparato no sabe de SVG ni de tiempos de animacion: recibe PASOS -- el
//! dibujo entero, ya aplanado, en unos instantes -- y mezcla dos vecinos
//! punto a punto (`bmo_pinta::pieza_entre`). Por eso aqui se eligen los
//! instantes con cuidado:
//!
//! ```text
//!    los de la animacion   cada `keyTimes` / fotograma, en cada vuelta
//!                          dentro del ciclo
//!    un giro               un paso cada 7,5 grados: la mezcla en linea recta
//!                          de dos puntos de un giro corta por dentro
//!    una curva de tiempo   ocho pasos por tramo (`ease`, `keySplines`)
//!    discreto              dos pasos pegados: el salto
//! ```
//!
//! Y todos los pasos tienen que tener la MISMA forma -- las mismas figuras,
//! con los mismos puntos. Si animar un `d` o un discontinuo cambia cuantos
//! puntos salen, es error al compilar, diciendo cual.

use std::collections::HashMap;

use crate::css::Decl;
use crate::escena::Ajuste;
use crate::figuras;
use crate::geo::{self, Matriz};
use crate::xml::Elemento;
use crate::{color, propiedades, Falla, Figura, Herencia, Svg, Tinta};
use bmo_maqueta_diag::Error;

/// El ciclo mas largo.
pub const CICLO_MAX_MS: f64 = 20_000.0;
/// Los pasos que caben en un dibujo.
pub const PASOS_MAX: usize = 240;
/// Grados entre dos pasos de un giro.
const GIRO_PASO: f64 = 7.5;
/// Pasos por tramo de una curva de tiempo que no es recta.
const CURVA_PASOS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub enum Que {
    /// Un atributo de geometria (`r`, `d`...) o una propiedad (`fill`...).
    Atributo(String),
    /// `animateTransform` de este `type`.
    Transform(String),
    /// Un `@keyframes` de CSS: `(propiedad, [(donde, valor)])`.
    Fotogramas(Vec<(String, Vec<(f64, String)>)>),
}

/// **La curva de tiempo de un tramo.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curva {
    Recta,
    /// `cubic-bezier`, `ease`..., `keySplines`.
    Bezier([f64; 4]),
    /// `steps(n)`: `n` saltos; con `true`, el primero al empezar el tramo.
    Saltos(usize, bool),
}

impl Curva {
    fn en(&self, u: f64) -> f64 {
        match *self {
            Curva::Recta => u,
            Curva::Bezier(c) => bezier(c, u),
            Curva::Saltos(n, empieza) => (((u * n as f64).floor() + if empieza { 1.0 } else { 0.0 }) / n as f64).min(1.0),
        }
    }
}

/// **Una animacion leida.**
#[derive(Clone, Debug, PartialEq)]
pub struct Animacion {
    /// El elemento que anima (su posicion, que es su nombre aqui).
    pub objetivo: usize,
    pub que: Que,
    pub valores: Vec<String>,
    pub tiempos: Vec<f64>,
    /// Una curva por tramo (`keySplines`), o UNA para todos (la de CSS).
    pub curvas: Vec<Curva>,
    pub discreto: bool,
    pub inicio_ms: f64,
    pub dura_ms: f64,
    /// `None` = para siempre.
    pub veces: Option<f64>,
    pub congela: bool,
    pub suma: bool,
    pub alterna: bool,
    pub pos: usize,
}

/// **Los pasos de un dibujo animado.**
#[derive(Clone, Debug, PartialEq)]
pub struct Pasos {
    /// Lo que dura un ciclo (o la animacion entera si no se repite).
    pub ciclo_ms: u32,
    pub repite: bool,
    /// El instante de cada paso, de 0 al ciclo.
    pub tiempos: Vec<u32>,
    /// Las figuras de cada paso, todas con la misma forma.
    pub figuras: Vec<Vec<Figura>>,
}

fn tiempo(t: &str) -> Option<f64> {
    let t = t.trim();
    if let Some(n) = t.strip_suffix("ms") {
        return n.trim().parse().ok();
    }
    if let Some(n) = t.strip_suffix("min") {
        return n.trim().parse::<f64>().ok().map(|v| v * 60_000.0);
    }
    if let Some(n) = t.strip_suffix('s') {
        return n.trim().parse::<f64>().ok().map(|v| v * 1000.0);
    }
    t.parse::<f64>().ok().map(|v| v * 1000.0)
}

fn curva_css(t: &str) -> Option<Curva> {
    Some(match t.trim() {
        "linear" => Curva::Recta,
        "ease" => Curva::Bezier([0.25, 0.1, 0.25, 1.0]),
        "ease-in" => Curva::Bezier([0.42, 0.0, 1.0, 1.0]),
        "ease-out" => Curva::Bezier([0.0, 0.0, 0.58, 1.0]),
        "ease-in-out" => Curva::Bezier([0.42, 0.0, 0.58, 1.0]),
        "step-start" => Curva::Saltos(1, true),
        "step-end" => Curva::Saltos(1, false),
        c if c.starts_with("steps(") => {
            let dentro = c.strip_prefix("steps(")?.strip_suffix(')')?;
            let mut partes = dentro.split(',').map(str::trim);
            let n: usize = partes.next()?.parse().ok().filter(|n| *n > 0)?;
            let empieza = match partes.next() {
                None | Some("end" | "jump-end") => false,
                Some("start" | "jump-start") => true,
                _ => return None,
            };
            Curva::Saltos(n, empieza)
        }
        c => {
            let v = geo::numeros(c.strip_prefix("cubic-bezier(")?.strip_suffix(')')?)?;
            if v.len() != 4 {
                return None;
            }
            Curva::Bezier([v[0], v[1], v[2], v[3]])
        }
    })
}

/// `cubic-bezier` evaluada: el avance en `x` (0..=1).
fn bezier(c: [f64; 4], x: f64) -> f64 {
    let b = |a: f64, b: f64, s: f64| 3.0 * a * s * (1.0 - s) * (1.0 - s) + 3.0 * b * s * s * (1.0 - s) + s * s * s;
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..40 {
        let m = (lo + hi) / 2.0;
        if b(c[0], c[2], m) < x {
            lo = m;
        } else {
            hi = m;
        }
    }
    b(c[1], c[3], (lo + hi) / 2.0)
}

// ---------------------------------------------------------------------------
// Leer
// ---------------------------------------------------------------------------

/// **Las animaciones de un dibujo**, juzgadas.
pub fn leer(svg: &Svg) -> (Vec<Animacion>, Vec<Falla>) {
    let mut out = Vec::new();
    let mut fallas = Vec::new();
    fn ir(svg: &Svg, el: &Elemento, padre: Option<&Elemento>, out: &mut Vec<Animacion>, fallas: &mut Vec<Falla>) {
        let mal = |fallas: &mut Vec<Falla>, que: String| fallas.push(Falla::en(el.pos, el.nombre.len() + 1, &que, "una animacion que no se entiende entera no se pinta a medias.", "ver LA_MAQUETA_EXIGE.md, 2e (S7)."));
        match (el.nombre.as_str(), padre) {
            ("animate" | "animateTransform", Some(p)) => match smil(el, p) {
                Ok(a) => out.push(a),
                Err(m) => mal(fallas, m),
            },
            _ => {}
        }
        // Una animacion de CSS: lo que diga la cascada del elemento.
        if crate::escena::FIGURAS.contains(&el.nombre.as_str()) || matches!(el.nombre.as_str(), "g" | "use" | "a") {
            let mut tiradas = Vec::new();
            let e = propiedades::estado(el, &propiedades::Estado::inicial(), &svg.hoja, &[], &mut tiradas);
            if let Some(a) = &e.animacion {
                match css(svg, el, a) {
                    Ok(Some(a)) => out.push(a),
                    Ok(None) => {}
                    Err(m) => mal(fallas, m),
                }
            }
        }
        for h in &el.hijos {
            ir(svg, h, Some(el), out, fallas);
        }
    }
    ir(svg, &svg.raiz, None, &mut out, &mut fallas);
    (out, fallas)
}

fn smil(el: &Elemento, padre: &Elemento) -> Result<Animacion, String> {
    let que = match el.nombre.as_str() {
        "animateTransform" => Que::Transform(el.attr("type").unwrap_or("translate").to_string()),
        _ => Que::Atributo(el.attr("attributeName").ok_or("un `<animate>` sin `attributeName`")?.to_string()),
    };
    if let Que::Transform(t) = &que {
        if !matches!(t.as_str(), "translate" | "scale" | "rotate" | "skewX" | "skewY") {
            return Err(format!("`animateTransform type=\"{t}\"` no esta en la lista"));
        }
    }
    let valores: Vec<String> = match (el.attr("values"), el.attr("from"), el.attr("to"), el.attr("by")) {
        (Some(v), ..) => v.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        (None, Some(f), Some(t), _) => vec![f.to_string(), t.to_string()],
        (None, f, None, Some(b)) => {
            let base = f.map(str::to_string).or_else(|| match &que {
                Que::Atributo(n) => padre.attr(n).map(str::to_string),
                _ => None,
            });
            let base = base.ok_or("`by` sin `from` ni el atributo en su elemento")?;
            let suma = plantilla_suma(&base, b).ok_or("`by` solo suma numeros")?;
            vec![base, suma]
        }
        (None, None, Some(t), _) => {
            let base = match &que {
                Que::Atributo(n) => padre.attr(n).map(str::to_string),
                Que::Transform(_) => Some(String::new()),
                _ => None,
            };
            vec![base.ok_or("`to` sin `from`: el valor de partida tiene que estar escrito")?, t.to_string()]
        }
        _ => return Err("una animacion sin `values` ni `from`/`to`".to_string()),
    };
    if valores.is_empty() {
        return Err("una animacion sin valores".to_string());
    }
    let dura_ms = el.attr("dur").and_then(tiempo).filter(|d| *d > 0.0).ok_or("una animacion sin `dur` (o con `dur` de cero)")?;
    let inicio_ms = match el.attr("begin") {
        None => 0.0,
        Some(b) => tiempo(b).ok_or_else(|| format!("`begin=\"{b}\"`: solo un tiempo (`0s`, `1.5s`); empezar con un clic o con otra animacion no esta"))?,
    };
    let veces = match el.attr("repeatCount") {
        Some("indefinite") => None,
        Some(n) => Some(n.parse::<f64>().map_err(|_| format!("`repeatCount=\"{n}\"`"))?),
        None => match el.attr("repeatDur") {
            Some("indefinite") => None,
            Some(d) => Some(tiempo(d).ok_or("`repeatDur`")? / dura_ms),
            None => Some(1.0),
        },
    };
    let n = valores.len();
    let tiempos = match el.attr("keyTimes") {
        Some(k) => {
            let v: Vec<f64> = k.split(';').map(|s| s.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|_| "`keyTimes` no son numeros")?;
            if v.len() != n {
                return Err("`keyTimes` y `values` no tienen los mismos".to_string());
            }
            v
        }
        None if n == 1 => vec![0.0],
        None => (0..n).map(|k| k as f64 / (n - 1) as f64).collect(),
    };
    let modo = el.attr("calcMode").unwrap_or("linear");
    let curvas = match modo {
        "linear" | "discrete" => vec![Curva::Recta; n.saturating_sub(1)],
        "spline" => {
            let ks = el.attr("keySplines").ok_or("`calcMode=\"spline\"` sin `keySplines`")?;
            let v: Vec<Curva> = ks
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| geo::numeros(s).filter(|v| v.len() == 4).map(|v| Curva::Bezier([v[0], v[1], v[2], v[3]])))
                .collect::<Option<_>>()
                .ok_or("`keySplines` no se sabe leer")?;
            if v.len() != n - 1 {
                return Err("`keySplines` tiene que tener una curva por tramo".to_string());
            }
            v
        }
        m => return Err(format!("`calcMode=\"{m}\"` no esta en la lista (`linear`, `discrete`, `spline`)")),
    };
    Ok(Animacion {
        objetivo: padre.pos,
        que,
        valores,
        tiempos,
        curvas,
        discreto: modo == "discrete",
        inicio_ms,
        dura_ms,
        veces,
        congela: el.attr("fill") == Some("freeze"),
        suma: el.attr("additive") == Some("sum"),
        alterna: false,
        pos: el.pos,
    })
}

/// `animation` de CSS sobre `el`: lo que juntan sus declaraciones.
fn css(svg: &Svg, el: &Elemento, decls: &str) -> Result<Option<Animacion>, String> {
    let (mut nombre, mut dura, mut retraso, mut veces, mut alterna, mut curva, mut congela) = (None, None, 0.0, Some(1.0), false, Curva::Bezier([0.25, 0.1, 0.25, 1.0]), false);
    for d in decls.split(';').filter(|d| !d.is_empty()) {
        let (p, v) = d.split_once(':').unwrap_or((d, ""));
        let partes = |v: &str| -> Vec<String> {
            let mut out = Vec::new();
            let mut nivel = 0;
            let mut cur = String::new();
            for c in v.chars() {
                match c {
                    '(' => nivel += 1,
                    ')' => nivel -= 1,
                    _ => {}
                }
                if c.is_whitespace() && nivel == 0 {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                } else {
                    cur.push(c);
                }
            }
            if !cur.is_empty() {
                out.push(cur);
            }
            out
        };
        let mut uno = |tok: &str, p: &str| -> Result<(), String> {
            match (p, tok) {
                (_, "infinite") => veces = None,
                (_, "normal" | "running" | "none" | "backwards") => {}
                (_, "alternate") => alterna = true,
                (_, "reverse" | "alternate-reverse") => return Err(format!("`animation-direction: {tok}` no esta (`normal`, `alternate`)")),
                (_, "forwards" | "both") => congela = true,
                (_, t) if curva_css(t).is_some() && (p == "animation" || p == "animation-timing-function") => curva = curva_css(t).unwrap(),
                (_, t) if t.starts_with("steps(") => return Err(format!("`{t}`: `steps(n)`, `steps(n, start)` o `steps(n, end)`")),
                ("animation-delay", t) => retraso = tiempo(t).ok_or(format!("`{t}`"))?,
                ("animation-duration", t) => dura = Some(tiempo(t).ok_or(format!("`{t}`"))?),
                ("animation-iteration-count", t) => veces = Some(t.parse::<f64>().map_err(|_| format!("`{t}`"))?),
                ("animation-name", t) => nombre = Some(t.to_string()),
                ("animation", t) if tiempo(t).is_some() && t.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '.' || c == '-') => {
                    if dura.is_none() {
                        dura = tiempo(t);
                    } else {
                        retraso = tiempo(t).unwrap();
                    }
                }
                ("animation", t) if t.parse::<f64>().is_ok() => veces = Some(t.parse().unwrap()),
                ("animation", t) => nombre = Some(t.to_string()),
                _ => {}
            }
            Ok(())
        };
        for tok in partes(v) {
            uno(&tok, p.trim())?;
        }
    }
    let Some(nombre) = nombre else { return Ok(None) };
    if nombre == "none" {
        return Ok(None);
    }
    let dura = dura.filter(|d| *d > 0.0).ok_or_else(|| format!("la animacion `{nombre}` no dice cuanto dura"))?;
    let f = svg.hoja.fotogramas.iter().rev().find(|f| f.nombre == nombre).ok_or_else(|| format!("no hay `@keyframes {nombre}` en el dibujo"))?;
    // Cada propiedad con sus fotogramas.
    let mut props: Vec<(String, Vec<(f64, String)>)> = Vec::new();
    for (donde, decls) in &f.pasos {
        for Decl { prop, valor, .. } in decls {
            if prop.starts_with("animation") {
                continue;
            }
            match props.iter_mut().find(|(p, _)| p == prop) {
                Some((_, v)) => v.push((*donde, valor.clone())),
                None => props.push((prop.clone(), vec![(*donde, valor.clone())])),
            }
        }
    }
    let _ = el;
    Ok(Some(Animacion {
        objetivo: el.pos,
        que: Que::Fotogramas(props),
        valores: Vec::new(),
        tiempos: Vec::new(),
        curvas: vec![curva],
        discreto: false,
        inicio_ms: retraso,
        dura_ms: dura,
        veces,
        congela,
        suma: false,
        alterna,
        pos: el.pos,
    }))
}

// ---------------------------------------------------------------------------
// Mezclar valores
// ---------------------------------------------------------------------------

/// Un valor partido en numeros y lo de entre medias.
fn trocear(v: &str) -> (Vec<String>, Vec<f64>) {
    let b = v.as_bytes();
    let (mut textos, mut nums) = (vec![String::new()], Vec::new());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let empieza_num = c.is_ascii_digit() || ((c == b'-' || c == b'+' || c == b'.') && b.get(i + 1).is_some_and(|d| d.is_ascii_digit() || *d == b'.'));
        let antes_letra = i > 0 && (b[i - 1].is_ascii_alphabetic() && b[i - 1] != b'e' || b[i - 1] == b'#');
        if empieza_num && !antes_letra || (empieza_num && i > 0 && b[i - 1].is_ascii_alphabetic() && !textos.last().unwrap().ends_with('#') && !textos.last().unwrap().chars().rev().take_while(|c| c.is_ascii_alphanumeric()).any(|c| c.is_ascii_digit())) {
            let mut n = geo::Numeros::de(&v[i..]);
            if let Some(x) = n.numero() {
                nums.push(x);
                textos.push(String::new());
                i += n.i;
                continue;
            }
        }
        textos.last_mut().unwrap().push(c as char);
        i += 1;
    }
    (textos, nums)
}

fn plantilla_suma(a: &str, b: &str) -> Option<String> {
    let (ta, na) = trocear(a);
    let (_, nb) = trocear(b);
    if na.len() != nb.len() {
        return None;
    }
    Some(coser(&ta, &na.iter().zip(&nb).map(|(x, y)| x + y).collect::<Vec<_>>()))
}

fn coser(textos: &[String], nums: &[f64]) -> String {
    let mut s = textos[0].clone();
    for (k, n) in nums.iter().enumerate() {
        s.push_str(&format!("{}", (n * 1e6).round() / 1e6));
        s.push_str(&textos[k + 1]);
    }
    s
}

/// **Un valor a medio camino** entre `a` y `b` (`u` de 0 a 1): colores por
/// canal, y todo lo demas numero a numero si tienen la misma forma.
fn mezcla(a: &str, b: &str, u: f64) -> Option<String> {
    if let (Some(ca), Some(cb)) = (color::leer(a), color::leer(b)) {
        let canal = |s: u32| (((ca.0 >> s & 255) as f64) * (1.0 - u) + ((cb.0 >> s & 255) as f64) * u).round() as u32;
        let al = ca.1 * (1.0 - u) + cb.1 * u;
        return Some(format!("rgba({},{},{},{al})", canal(16), canal(8), canal(0)));
    }
    let (ta, na) = trocear(a);
    let (tb, nb) = trocear(b);
    if ta != tb || na.len() != nb.len() {
        return None;
    }
    Some(coser(&ta, &na.iter().zip(&nb).map(|(x, y)| x + (y - x) * u).collect::<Vec<_>>()))
}

/// Lo neutro de un `transform` de CSS (`rotate(360deg)` -> `rotate(0deg)`).
fn neutro(t: &str) -> String {
    let (textos, nums) = trocear(t);
    let mut out = Vec::new();
    let mut funcion = String::new();
    for (k, n) in nums.iter().enumerate() {
        if let Some(f) = textos[k].rsplit(|c: char| c == ')' || c.is_whitespace()).next().and_then(|s| s.split('(').next()).filter(|s| !s.is_empty()) {
            funcion = f.trim().to_string();
        }
        let _ = n;
        out.push(if funcion.starts_with("scale") { 1.0 } else { 0.0 });
    }
    coser(&textos, &out)
}

// ---------------------------------------------------------------------------
// Evaluar en un instante
// ---------------------------------------------------------------------------

impl Animacion {
    /// Si se repite para siempre.
    pub fn eterna(&self) -> bool {
        self.veces.is_none()
    }
    /// Lo que dura una vuelta entera (con `alternate`, ida y vuelta).
    fn vuelta(&self) -> f64 {
        if self.alterna {
            self.dura_ms * 2.0
        } else {
            self.dura_ms
        }
    }
    /// El avance (0..=1) a los `t` ms, o `None` si no actua.
    fn avance(&self, t: f64) -> Option<f64> {
        let mut local = t - self.inicio_ms;
        match self.veces {
            None => local = local.rem_euclid(self.vuelta()),
            Some(v) => {
                if local < 0.0 {
                    return None;
                }
                let total = self.dura_ms * v;
                if local >= total {
                    if !self.congela {
                        return None;
                    }
                    let fin = v.fract();
                    let vuelta = if fin == 0.0 { v - 1.0 } else { v.floor() };
                    let p = if fin == 0.0 { 1.0 } else { fin };
                    return Some(if self.alterna && vuelta as u64 % 2 == 1 { 1.0 - p } else { p });
                }
                local = local.rem_euclid(self.vuelta());
            }
        }
        let p = local / self.dura_ms;
        Some(if p > 1.0 { 2.0 - p } else { p.min(1.0) })
    }

    /// El tramo y su avance curvado, en unos tiempos.
    fn tramo(tiempos: &[f64], curvas: &[Curva], discreto: bool, p: f64) -> (usize, f64) {
        if tiempos.len() < 2 {
            return (0, 0.0);
        }
        let mut k = 0;
        while k + 2 < tiempos.len() && p >= tiempos[k + 1] {
            k += 1;
        }
        if discreto {
            return if p >= tiempos[k + 1] { (k + 1, 0.0) } else { (k, 0.0) };
        }
        let largo = (tiempos[k + 1] - tiempos[k]).max(1e-12);
        let u = ((p - tiempos[k]) / largo).clamp(0.0, 1.0);
        let c = curvas.get(k).or(curvas.first()).copied().unwrap_or(Curva::Recta);
        (k, c.en(u))
    }

    /// **Lo que pone en su elemento a los `t` ms.**
    fn ajustar(&self, t: f64, aj: &mut Ajuste) {
        let Some(p) = self.avance(t) else { return };
        match &self.que {
            Que::Atributo(n) | Que::Transform(n) => {
                let (k, u) = Self::tramo(&self.tiempos, &self.curvas, self.discreto, p);
                let a = &self.valores[k.min(self.valores.len() - 1)];
                let b = &self.valores[(k + 1).min(self.valores.len() - 1)];
                let v = mezcla(a, b, u).unwrap_or_else(|| if u < 0.5 { a.clone() } else { b.clone() });
                if let Que::Transform(tipo) = &self.que {
                    let m = geo::transform(&format!("{tipo}({v})")).unwrap_or(Matriz::UNO);
                    aj.matriz = Some(match aj.matriz {
                        Some((previa, _)) => (previa.por(&m), self.suma),
                        None => (m, self.suma),
                    });
                } else if propiedades::es_presentacion(n) {
                    aj.decls.push(Decl { prop: n.clone(), valor: v, importante: true });
                } else {
                    aj.attrs.push((n.clone(), v));
                }
            }
            Que::Fotogramas(props) => {
                for (prop, pasos) in props {
                    let mut pasos = pasos.clone();
                    // Lo que falta en 0% o 100%: lo neutro de un giro, o el
                    // fotograma mas cercano.
                    if pasos.first().is_some_and(|p| p.0 > 0.0) {
                        let v = if prop == "transform" { neutro(&pasos[0].1) } else { pasos[0].1.clone() };
                        pasos.insert(0, (0.0, v));
                    }
                    if pasos.last().is_some_and(|p| p.0 < 1.0) {
                        let ultimo = pasos.last().unwrap();
                        let v = if prop == "transform" { neutro(&ultimo.1) } else { ultimo.1.clone() };
                        pasos.push((1.0, v));
                    }
                    let tiempos: Vec<f64> = pasos.iter().map(|p| p.0).collect();
                    let (k, u) = Self::tramo(&tiempos, &self.curvas, false, p);
                    let (a, b) = (&pasos[k].1, &pasos[(k + 1).min(pasos.len() - 1)].1);
                    let v = mezcla(a, b, u).unwrap_or_else(|| if u < 0.5 { a.clone() } else { b.clone() });
                    aj.decls.push(Decl { prop: prop.clone(), valor: v, importante: true });
                }
            }
        }
    }

    /// Los instantes de su vuelta (0..=1) donde hace falta un paso.
    fn instantes(&self) -> Vec<f64> {
        let mut v = Vec::new();
        let (tiempos, giros): (Vec<f64>, Vec<f64>) = match &self.que {
            Que::Fotogramas(props) => {
                let mut ts: Vec<f64> = vec![0.0, 1.0];
                let mut giro: f64 = 0.0;
                for (prop, pasos) in props {
                    ts.extend(pasos.iter().map(|p| p.0));
                    if prop == "transform" {
                        let ns: Vec<Vec<f64>> = pasos.iter().map(|p| trocear(&p.1).1).collect();
                        for w in ns.windows(2) {
                            giro = giro.max(w[0].iter().zip(&w[1]).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max));
                        }
                        if let Some(n) = ns.first() {
                            giro = giro.max(n.iter().map(|x| x.abs()).fold(0.0, f64::max));
                        }
                    }
                }
                ts.sort_by(f64::total_cmp);
                ts.dedup();
                let g = vec![giro; ts.len()];
                (ts, g)
            }
            Que::Transform(t) if t == "rotate" || t.starts_with("skew") => {
                let ns: Vec<f64> = self.valores.iter().map(|v| trocear(v).1.first().copied().unwrap_or(0.0)).collect();
                let g = ns.windows(2).map(|w| (w[1] - w[0]).abs()).chain(std::iter::once(0.0)).collect();
                (self.tiempos.clone(), g)
            }
            _ => (self.tiempos.clone(), vec![0.0; self.tiempos.len()]),
        };
        let curvo = self.curvas.iter().any(|c| matches!(c, Curva::Bezier(_)));
        let saltos = self.curvas.iter().find_map(|c| match c {
            Curva::Saltos(n, _) => Some(*n),
            _ => None,
        });
        // Un milisegundo, en fraccion de la animacion: el ancho de un salto.
        let ms = 1.0 / self.dura_ms;
        for k in 0..tiempos.len() {
            v.push(tiempos[k]);
            if self.discreto && k > 0 {
                v.push((tiempos[k] - ms).max(0.0));
            }
            if k + 1 < tiempos.len() && !self.discreto {
                let largo = tiempos[k + 1] - tiempos[k];
                if let Some(n) = saltos {
                    for j in 1..=n {
                        let t = tiempos[k] + largo * j as f64 / n as f64;
                        v.push((t - ms).max(tiempos[k]));
                        v.push(t);
                    }
                    continue;
                }
                let por_giro = (giros[k] / GIRO_PASO).ceil() as usize;
                let n = por_giro.max(if curvo { CURVA_PASOS } else { 1 });
                for j in 1..n {
                    v.push(tiempos[k] + largo * j as f64 / n as f64);
                }
            }
        }
        v
    }
}

fn mcm(a: u64, b: u64) -> u64 {
    fn mcd(a: u64, b: u64) -> u64 {
        if b == 0 { a } else { mcd(b, a % b) }
    }
    a / mcd(a, b) * b
}

/// **Los pasos de un dibujo animado** en una caja. `None` si no anima.
pub fn pasos(svg: &Svg, h: &Herencia, caja: (f64, f64, f64, f64)) -> Option<Result<Pasos, Vec<Error>>> {
    let an = &svg.animaciones;
    if an.is_empty() {
        return None;
    }
    let falla = |pos: usize, m: String| -> Vec<Error> { vec![svg.error(&Falla::en(pos, 1, &m, "un dibujo anima en UN ciclo, con los mismos puntos en cada paso: es lo que el aparato sabe mezclar.", "ver LA_MAQUETA_EXIGE.md, 2e (S7)."))] };
    let eternas = an.iter().filter(|a| a.eterna()).count();
    if eternas != 0 && eternas != an.len() {
        return Some(Err(falla(an[0].pos, "este dibujo mezcla animaciones que se repiten para siempre con otras que acaban: no tiene UN ciclo".to_string())));
    }
    let repite = eternas > 0;
    let ciclo = if repite {
        let mut c = 1u64;
        for a in an {
            c = mcm(c, a.vuelta().round().max(1.0) as u64);
        }
        c as f64
    } else {
        an.iter().map(|a| a.inicio_ms + a.dura_ms * a.veces.unwrap_or(1.0)).fold(0.0, f64::max)
    };
    if ciclo > CICLO_MAX_MS {
        return Some(Err(falla(an[0].pos, format!("el ciclo de este dibujo dura {:.1} s, y caben {} s", ciclo / 1000.0, CICLO_MAX_MS / 1000.0))));
    }
    // Los instantes, de todas, en el ciclo.
    let mut ts: Vec<f64> = vec![0.0, ciclo];
    for a in an {
        let vuelta = a.vuelta();
        let ins = a.instantes();
        let vueltas = if repite { (ciclo / vuelta).round() as i64 } else { a.veces.unwrap_or(1.0).ceil() as i64 };
        for r in -1..=vueltas {
            for &i in &ins {
                for mitad in [0.0, 1.0] {
                    if mitad == 1.0 && !a.alterna {
                        continue;
                    }
                    let local = if mitad == 0.0 { i * a.dura_ms } else { a.dura_ms + (1.0 - i) * a.dura_ms };
                    let t = a.inicio_ms + r as f64 * vuelta + local;
                    // El final de una vuelta que vuelve a empezar es un SALTO:
                    // un paso 1 ms antes, con el valor del final.
                    let salto = if i >= 1.0 && !a.alterna { vec![t - 1.0, t] } else { vec![t] };
                    for t in salto {
                        let t = if repite { t.rem_euclid(ciclo) } else { t };
                        if (0.0..=ciclo).contains(&t) {
                            ts.push(t);
                        }
                    }
                }
            }
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    if ts.len() > PASOS_MAX {
        return Some(Err(falla(an[0].pos, format!("este dibujo pide {} pasos y caben {PASOS_MAX}", ts.len()))));
    }
    // La escena en cada paso.
    let mut escenas = Vec::new();
    let mut fallas = Vec::new();
    for &t0 in &ts {
        // El ultimo paso es el final del ciclo, justo antes de volver a 0.
        let t = if repite && t0 >= ciclo { ciclo - 0.001 } else { t0 };
        let mut ajustes: HashMap<usize, Ajuste> = HashMap::new();
        for a in an {
            a.ajustar(t, ajustes.entry(a.objetivo).or_default());
        }
        let (piezas, mut f) = figuras::piezas_en(svg, h, caja, &|el: &Elemento| ajustes.get(&el.pos).cloned().unwrap_or_default());
        fallas.append(&mut f);
        escenas.push(piezas);
    }
    if !fallas.is_empty() {
        return Some(Err(figuras::errores(svg, &fallas)));
    }
    // La misma forma en todos: las piezas y sus curvas.
    let forma = |p: &crate::escena::Pieza| -> Vec<Vec<u8>> {
        p.contorno.subs.iter().map(|s| s.segs.iter().map(|g| match g { geo::Seg::Linea(_) => 0, geo::Seg::Cuadratica(..) => 1, geo::Seg::Cubica(..) => 2 }).collect()).collect()
    };
    for e in &escenas[1..] {
        if e.len() != escenas[0].len() {
            return Some(Err(falla(an[0].pos, "en un paso de la animacion aparecen o desaparecen figuras (un `r` que llega a 0, un `display`): los pasos no tienen la misma forma".to_string())));
        }
        for (a, b) in escenas[0].iter().zip(e) {
            if forma(a) != forma(b) {
                return Some(Err(falla(b.pos, format!("{} cambia de forma entre pasos (sus curvas no son las mismas)", b.de))));
            }
        }
    }
    // Los tramos: el maximo de cada curva en todos los pasos.
    let mut n = figuras::conteos(&escenas[0]);
    for e in &escenas[1..] {
        for (k, c) in figuras::conteos(e).into_iter().enumerate() {
            for (s, sub) in c.into_iter().enumerate() {
                for (j, v) in sub.into_iter().enumerate() {
                    n[k][s][j] = n[k][s][j].max(v);
                }
            }
        }
    }
    let mut figs = Vec::new();
    for e in &escenas {
        let (f, fa) = figuras::aplanar(svg, e, &n, false);
        if !fa.is_empty() {
            return Some(Err(figuras::errores(svg, &fa)));
        }
        figs.push(f);
    }
    let firma = |f: &Figura| -> (Vec<usize>, bool, usize, u8) {
        let t = match &f.tinta {
            Tinta::Liso(_) => (0, 0),
            Tinta::Lineal { paradas, .. } => (1, paradas.len()),
            Tinta::Radial { paradas, .. } => (2, paradas.len()),
        };
        (f.caminos.iter().map(Vec::len).collect(), f.pluma > 0.0, t.1, t.0)
    };
    for paso in &figs[1..] {
        if paso.len() != figs[0].len() {
            return Some(Err(falla(an[0].pos, "los pasos de la animacion no tienen las mismas figuras".to_string())));
        }
        for (a, b) in figs[0].iter().zip(paso) {
            if firma(a) != firma(b) {
                return Some(Err(falla(an[0].pos, format!("{} no tiene los mismos puntos en todos los pasos (un discontinuo o una pluma que cambia de forma)", b.de))));
            }
        }
    }
    Some(Ok(Pasos { ciclo_ms: ciclo.round() as u32, repite, tiempos: ts.iter().map(|t| t.round() as u32).collect(), figuras: figs }))
}
