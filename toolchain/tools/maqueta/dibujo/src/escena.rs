//! **La escena**: el arbol del SVG recorrido como lo pinta un navegador --
//! grupos, `use`, simbolos, transformaciones, la cascada -- hasta una lista
//! de PIEZAS: cada figura con su contorno (en sus unidades), la matriz que
//! la lleva al lienzo, y lo que pinta.
//!
//! Aqui todavia no se aplana: `figuras` lo hace despues, con los tramos que
//! le digan (los suyos, o los de todos los pasos de una animacion).

use std::collections::HashMap;

use crate::css::Decl;
use crate::geo::{self, Contorno, Matriz};
use crate::pluma::{Esquina, Punta};
use crate::propiedades::{self, largo, Estado, Largo, Pinta};
use crate::xml::Elemento;
use crate::Svg;

/// Lo que una animacion cambia de un elemento en un instante (S7).
#[derive(Clone, Debug, Default)]
pub struct Ajuste {
    /// Atributos con otro valor (`r`, `cx`, `d`...).
    pub attrs: Vec<(String, String)>,
    /// Propiedades encima de todo (`fill`, `opacity`...).
    pub decls: Vec<Decl>,
    /// `animateTransform`: la matriz, y si SUMA a la del atributo.
    pub matriz: Option<(Matriz, bool)>,
}

/// Lo que pinta una pieza: color o degradado.
#[derive(Clone, Debug, PartialEq)]
pub enum Pintura {
    Color(u32),
    Degradado(String),
}

#[derive(Clone, Debug)]
pub struct Relleno {
    pub pintura: Pintura,
    pub alfa: f64,
    pub par_impar: bool,
}

#[derive(Clone, Debug)]
pub struct Linea {
    pub pintura: Pintura,
    pub alfa: f64,
    pub ancho: f64,
    pub punta: Punta,
    pub esquina: Esquina,
    pub limite: f64,
    pub discontinuo: Option<(Vec<f64>, f64)>,
}

/// **Una figura de la escena**, sin aplanar.
#[derive(Clone, Debug)]
pub struct Pieza {
    pub contorno: Contorno,
    pub m: Matriz,
    pub relleno: Option<Relleno>,
    pub linea: Option<Linea>,
    pub linea_primero: bool,
    pub color: (u32, f64),
    /// El recorte convexo que le toca, en el lienzo (sentido positivo).
    pub recorte: Option<Vec<geo::P>>,
    pub de: String,
    pub pos: usize,
}

impl Pieza {
    fn pinturas(&self) -> usize {
        self.relleno.is_some() as usize + self.linea.is_some() as usize
    }
}

/// Las etiquetas que son figuras.
pub const FIGURAS: &[&str] = &["path", "circle", "ellipse", "rect", "line", "polyline", "polygon"];

struct Recorrido<'a> {
    svg: &'a Svg,
    ids: &'a HashMap<String, &'a Elemento>,
    ajuste: &'a dyn Fn(&Elemento) -> Ajuste,
    vw: f64,
    vh: f64,
    piezas: Vec<Pieza>,
    fallas: Vec<(usize, String)>,
    /// El recorte de lo que se esta recorriendo.
    recorte: Option<Vec<geo::P>>,
}

/// **La escena entera**: `m` lleva las unidades del dibujo al lienzo.
pub fn piezas(svg: &Svg, raiz_estado: &Estado, m: &Matriz, ajuste: &dyn Fn(&Elemento) -> Ajuste) -> (Vec<Pieza>, Vec<(usize, String)>) {
    let ids = svg.ids();
    let (vw, vh) = svg.vista.map_or((100.0, 100.0), |v| (v[2], v[3]));
    let mut r = Recorrido { svg, ids: &ids, ajuste, vw, vh, piezas: Vec::new(), fallas: Vec::new(), recorte: None };
    // La raiz: su propio estilo (atributos de presentacion del `<svg>`).
    let mut tiradas = Vec::new();
    let e = propiedades::estado(&svg.raiz, raiz_estado, &svg.hoja, &[], &mut tiradas);
    r.fallas.extend(tiradas);
    for h in &svg.raiz.hijos {
        r.elemento(h, &e, m, 0);
    }
    (r.piezas, r.fallas)
}

impl Recorrido<'_> {
    fn attr<'b>(&self, el: &'b Elemento, aj: &'b Ajuste, n: &str) -> Option<&'b str> {
        aj.attrs.iter().rev().find(|(k, _)| k == n).map(|(_, v)| v.as_str()).or_else(|| el.attr(n))
    }

    fn largo(&mut self, el: &Elemento, aj: &Ajuste, n: &str, ref_: f64) -> f64 {
        match self.attr(el, aj, n) {
            None => 0.0,
            Some(v) => match largo(v) {
                Some(l) => l.en(ref_),
                None => {
                    self.fallas.push((el.atributo(n).map_or(el.pos, |a| a.pos), format!("`{n}=\"{v}\"` no es un largo")));
                    0.0
                }
            },
        }
    }

    /// La matriz propia de un elemento: su `transform`, o el de CSS (que
    /// gana, con su origen), y lo que sume una animacion.
    fn matriz_propia(&mut self, el: &Elemento, aj: &Ajuste, e: &Estado, caja: Option<(f64, f64, f64, f64)>) -> Matriz {
        let mut m = match self.attr(el, aj, "transform") {
            Some(t) => geo::transform(t).unwrap_or_else(|t| {
                self.fallas.push((el.pos, format!("`transform`: `{t}` no se sabe leer")));
                Matriz::UNO
            }),
            None => Matriz::UNO,
        };
        if let Some(t) = &e.transform_css {
            match geo::transform_css(t) {
                Ok(f) => {
                    let (rx, ry, rw, rh) = if e.transform_box_figura { caja.unwrap_or((0.0, 0.0, 0.0, 0.0)) } else { (0.0, 0.0, self.vw, self.vh) };
                    let defecto = if e.transform_box_figura { "50% 50%" } else { "0 0" };
                    let o = origen(e.transform_origin.as_deref().unwrap_or(defecto), (rx, ry, rw, rh));
                    m = Matriz::mover(o.0, o.1).por(&f).por(&Matriz::mover(-o.0, -o.1));
                }
                Err(t) => self.fallas.push((el.pos, format!("`transform: {t}` de CSS no se sabe leer"))),
            }
        }
        match aj.matriz {
            Some((a, true)) => m.por(&a),
            Some((a, false)) => a,
            None => m,
        }
    }

    fn elemento(&mut self, el: &Elemento, padre: &Estado, m: &Matriz, hondo: usize) {
        let aj = (self.ajuste)(el);
        let n = el.nombre.as_str();
        if n.contains(':') || matches!(n, "clipPath" | "defs" | "title" | "desc" | "metadata" | "style" | "linearGradient" | "radialGradient" | "stop" | "symbol" | "animate" | "animateTransform") {
            return;
        }
        let e = propiedades::estado(el, padre, &self.svg.hoja, &aj.decls, &mut self.fallas);
        if !e.mostrar {
            return;
        }
        if hondo > 24 {
            self.fallas.push((el.pos, "los `<use>` se llaman en circulo (o demasiado hondo)".to_string()));
            return;
        }
        let antes = self.piezas.len();
        let recorte_antes = self.recorte.clone();
        match n {
            "g" | "a" => {
                let m = m.por(&self.matriz_propia(el, &aj, &e, None));
                self.recortar(el, &e, &m, None);
                for h in &el.hijos {
                    self.elemento(h, &e, &m, hondo + 1);
                }
            }
            "use" => {
                let Some(id) = self.attr(el, &aj, "href").or_else(|| self.attr(el, &aj, "xlink:href")).and_then(|h| h.strip_prefix('#')).map(str::to_string) else {
                    self.fallas.push((el.pos, "un `<use>` sin `href=\"#...\"`".to_string()));
                    return;
                };
                let Some(dest) = self.ids.get(&id).copied() else {
                    self.fallas.push((el.pos, format!("`<use href=\"#{id}\">` no apunta a nada del dibujo")));
                    return;
                };
                let (x, y) = (self.largo(el, &aj, "x", self.vw), self.largo(el, &aj, "y", self.vh));
                let mut m = m.por(&self.matriz_propia(el, &aj, &e, None)).por(&Matriz::mover(x, y));
                if dest.nombre == "symbol" {
                    if let Some(vb) = dest.attr("viewBox").and_then(geo::numeros).filter(|v| v.len() == 4 && v[2] > 0.0 && v[3] > 0.0) {
                        let w = self.attr(el, &aj, "width").and_then(largo).unwrap_or(Largo::Pc(100.0)).en(self.vw);
                        let h = self.attr(el, &aj, "height").and_then(largo).unwrap_or(Largo::Pc(100.0)).en(self.vh);
                        m = m.por(&encaje([vb[0], vb[1], vb[2], vb[3]], dest.attr("preserveAspectRatio").unwrap_or(""), (0.0, 0.0, w, h)));
                    }
                    for h in &dest.hijos {
                        self.elemento(h, &e, &m, hondo + 1);
                    }
                } else {
                    self.elemento(dest, &e, &m, hondo + 1);
                }
            }
            _ if FIGURAS.contains(&n) => self.figura(el, &aj, &e, m),
            _ => {}
        }
        self.recorte = recorte_antes;
        // ** La opacidad de un grupo o de una figura: exacta si pinta UNA
        // cosa; si pinta varias, el navegador las compone aparte y mezcla el
        // grupo entero -- eso es de la 3060, y se dice.
        if e.opacity < 1.0 {
            let pinturas: usize = self.piezas[antes..].iter().map(Pieza::pinturas).sum();
            if pinturas > 1 {
                self.fallas.push((
                    el.pos,
                    format!(
                        "`opacity` en `<{n}>`, que pinta {pinturas} cosas: el navegador las junta antes de mezclarlas y el pintor no (eso es componer aparte, de la 3060). Usar `fill-opacity` y `stroke-opacity` en cada figura"
                    ),
                ));
            }
            for p in &mut self.piezas[antes..] {
                if let Some(r) = &mut p.relleno {
                    r.alfa *= e.opacity;
                }
                if let Some(l) = &mut p.linea {
                    l.alfa *= e.opacity;
                }
            }
        }
    }

    fn contorno(&mut self, el: &Elemento, aj: &Ajuste) -> Option<Contorno> {
        let (vw, vh) = (self.vw, self.vh);
        let diag = ((vw * vw + vh * vh) / 2.0).sqrt();
        Some(match el.nombre.as_str() {
            "path" => {
                let d = self.attr(el, aj, "d").unwrap_or("").to_string();
                match geo::camino(&d) {
                    Ok(c) => c,
                    Err(_) => {
                        self.fallas.push((el.pos, "este `d` no se sabe leer".to_string()));
                        return None;
                    }
                }
            }
            "circle" => {
                let r = self.largo(el, aj, "r", diag);
                if r <= 0.0 {
                    return None;
                }
                geo::elipse(self.largo(el, aj, "cx", vw), self.largo(el, aj, "cy", vh), r, r)
            }
            "ellipse" => {
                let (mut rx, mut ry) = (self.attr(el, aj, "rx"), self.attr(el, aj, "ry"));
                if rx.is_none() || rx == Some("auto") {
                    rx = ry;
                }
                if ry.is_none() || ry == Some("auto") {
                    ry = rx;
                }
                let (rx, ry) = (rx.and_then(largo).map_or(0.0, |l| l.en(vw)), ry.and_then(largo).map_or(0.0, |l| l.en(vh)));
                if rx <= 0.0 || ry <= 0.0 {
                    return None;
                }
                geo::elipse(self.largo(el, aj, "cx", vw), self.largo(el, aj, "cy", vh), rx, ry)
            }
            "rect" => {
                let (w, h) = (self.largo(el, aj, "width", vw), self.largo(el, aj, "height", vh));
                if w <= 0.0 || h <= 0.0 {
                    return None;
                }
                let (rx, ry) = (self.attr(el, aj, "rx").and_then(largo).map(|l| l.en(vw)), self.attr(el, aj, "ry").and_then(largo).map(|l| l.en(vh)));
                let (rx, ry) = match (rx, ry) {
                    (Some(x), Some(y)) => (x, y),
                    (Some(x), None) => (x, x),
                    (None, Some(y)) => (y, y),
                    (None, None) => (0.0, 0.0),
                };
                geo::rectangulo(self.largo(el, aj, "x", vw), self.largo(el, aj, "y", vh), w, h, rx.clamp(0.0, w / 2.0), ry.clamp(0.0, h / 2.0))
            }
            "line" => {
                let a = (self.largo(el, aj, "x1", vw), self.largo(el, aj, "y1", vh));
                let b = (self.largo(el, aj, "x2", vw), self.largo(el, aj, "y2", vh));
                Contorno { subs: vec![geo::Sub { inicio: a, segs: vec![geo::Seg::Linea(b)], cerrado: false }] }
            }
            "polyline" | "polygon" => {
                let pts = self.attr(el, aj, "points").unwrap_or("").to_string();
                match geo::poligono(&pts, el.nombre == "polygon") {
                    Some(c) => c,
                    None => {
                        self.fallas.push((el.pos, "estos `points` no son pares de numeros".to_string()));
                        return None;
                    }
                }
            }
            _ => return None,
        })
    }

    fn pintura(&mut self, p: &Pinta, e: &Estado, el: &Elemento) -> Option<(Pintura, f64)> {
        match p {
            Pinta::Nada => None,
            Pinta::Color(c, a) => Some((Pintura::Color(*c), *a)),
            Pinta::Actual => Some((Pintura::Color(e.color.0), e.color.1)),
            Pinta::Url(id) => {
                if !self.ids.contains_key(id) {
                    self.fallas.push((el.pos, format!("`url(#{id})` no apunta a nada del dibujo")));
                    return None;
                }
                Some((Pintura::Degradado(id.clone()), 1.0))
            }
        }
    }

    fn figura(&mut self, el: &Elemento, aj: &Ajuste, e: &Estado, m: &Matriz) {
        let Some(contorno) = self.contorno(el, aj) else { return };
        if !e.visible {
            return;
        }
        let caja = contorno.caja_exacta();
        let m = m.por(&self.matriz_propia(el, aj, e, caja));
        self.recortar(el, e, &m, caja);
        // Una `line` no tiene dentro: no se rellena (una `polyline` si).
        let relleno = if el.nombre == "line" {
            None
        } else {
            self.pintura(&e.fill.clone(), e, el).map(|(pintura, a)| Relleno { pintura, alfa: a * e.fill_opacity, par_impar: e.par_impar })
        };
        let diag = ((self.vw * self.vw + self.vh * self.vh) / 2.0).sqrt();
        let ancho = e.stroke_width.en(diag);
        let linea = if ancho > 0.0 {
            self.pintura(&e.stroke.clone(), e, el).map(|(pintura, a)| Linea {
                pintura,
                alfa: a * e.stroke_opacity,
                ancho,
                punta: e.punta,
                esquina: e.esquina,
                limite: e.miterlimit,
                discontinuo: e.dasharray.as_ref().map(|d| (d.iter().map(|l| l.en(diag)).collect(), e.dashoffset.en(diag))),
            })
        } else {
            None
        };
        if relleno.is_none() && linea.is_none() {
            return;
        }
        let linea_n = self.svg.linea_de(el.pos);
        self.piezas.push(Pieza { contorno, m, relleno, linea, linea_primero: e.trazo_primero, color: e.color, recorte: self.recorte.clone(), de: format!("<{}> linea {linea_n}", el.nombre), pos: el.pos });
    }
}

impl Recorrido<'_> {
    /// **El `clip-path` de un elemento**, ya en el lienzo y cortado con el
    /// que venia: UNA figura convexa (un rectangulo, un circulo, un
    /// poligono convexo), que es lo que se recorta exacto al compilar.
    fn recortar(&mut self, el: &Elemento, e: &Estado, m: &Matriz, caja: Option<(f64, f64, f64, f64)>) {
        let Some(id) = &e.recorte else { return };
        let Some(cp) = self.ids.get(id).copied().filter(|c| c.nombre == "clipPath") else {
            self.fallas.push((el.pos, format!("`clip-path: url(#{id})` no apunta a un `<clipPath>` del dibujo")));
            return;
        };
        let formas: Vec<&Elemento> = cp.hijos.iter().filter(|h| FIGURAS.contains(&h.nombre.as_str()) || h.nombre == "use").collect();
        let [forma] = formas.as_slice() else {
            self.fallas.push((cp.pos, format!("el `<clipPath id=\"{id}\">` tiene {} figuras: se recorta exacto por UNA figura convexa", formas.len())));
            return;
        };
        let forma = if forma.nombre == "use" {
            match forma.attr("href").or_else(|| forma.attr("xlink:href")).and_then(|h| h.strip_prefix('#')).and_then(|h| self.ids.get(h).copied()) {
                Some(f) => f,
                None => return,
            }
        } else {
            forma
        };
        let aj = Ajuste::default();
        let Some(c) = self.contorno(forma, &aj) else { return };
        let mut mc = *m;
        if cp.attr("clipPathUnits") == Some("objectBoundingBox") {
            let (bx, by, bw, bh) = caja.unwrap_or((0.0, 0.0, 0.0, 0.0));
            mc = mc.por(&Matriz::mover(bx, by)).por(&Matriz::escala(bw, bh));
        }
        for t in [cp.attr("transform"), forma.attr("transform")].into_iter().flatten() {
            if let Ok(t) = geo::transform(t) {
                mc = mc.por(&t);
            }
        }
        let (cs, _) = geo::aplanar(&c.transformado(&mc), 0.05);
        let convexo = match cs.as_slice() {
            [uno] => geo::convexo(uno),
            _ => None,
        };
        let Some(mut nuevo) = convexo else {
            self.fallas.push((cp.pos, format!("el `<clipPath id=\"{id}\">` no es convexo: se recorta exacto por un rectangulo, un circulo o un poligono convexo")));
            return;
        };
        if let Some(previo) = &self.recorte {
            nuevo = geo::convexo(&geo::recortar(&nuevo, previo)).unwrap_or_default();
        }
        self.recorte = Some(nuevo);
    }
}

/// `transform-origin`: uno o dos valores (palabras, `%` o largos) en una caja.
fn origen(t: &str, (x, y, w, h): (f64, f64, f64, f64)) -> (f64, f64) {
    let partes: Vec<&str> = t.split_whitespace().collect();
    let uno = |v: &str, ref_: f64, base: f64| -> f64 {
        match v {
            "left" | "top" => base,
            "center" => base + ref_ / 2.0,
            "right" | "bottom" => base + ref_,
            _ => base + largo(v).map_or(0.0, |l| l.en(ref_)),
        }
    };
    match partes.as_slice() {
        [a] if *a == "top" || *a == "bottom" => (x + w / 2.0, uno(a, h, y)),
        [a] => (uno(a, w, x), y + h / 2.0),
        [a, b, ..] if matches!(*a, "top" | "bottom") || matches!(*b, "left" | "right") => (uno(b, w, x), uno(a, h, y)),
        [a, b, ..] => (uno(a, w, x), uno(b, h, y)),
        [] => (x, y),
    }
}

/// **El `viewBox` en una caja**, con `preserveAspectRatio` (por defecto
/// `xMidYMid meet`, como el navegador).
pub fn encaje(vb: [f64; 4], aspecto: &str, (x, y, w, h): (f64, f64, f64, f64)) -> Matriz {
    let (sx, sy) = (w / vb[2], h / vb[3]);
    let partes: Vec<&str> = aspecto.split_whitespace().collect();
    let alinea = partes.first().copied().unwrap_or("xMidYMid");
    if alinea == "none" {
        return Matriz::mover(x, y).por(&Matriz::escala(sx, sy)).por(&Matriz::mover(-vb[0], -vb[1]));
    }
    let s = if partes.get(1) == Some(&"slice") { sx.max(sy) } else { sx.min(sy) };
    let (libre_x, libre_y) = (w - vb[2] * s, h - vb[3] * s);
    let fx = if alinea.starts_with("xMin") { 0.0 } else if alinea.starts_with("xMax") { 1.0 } else { 0.5 };
    let fy = if alinea.ends_with("YMin") { 0.0 } else if alinea.ends_with("YMax") { 1.0 } else { 0.5 };
    Matriz::mover(x + libre_x * fx, y + libre_y * fy).por(&Matriz::escala(s, s)).por(&Matriz::mover(-vb[0], -vb[1]))
}
