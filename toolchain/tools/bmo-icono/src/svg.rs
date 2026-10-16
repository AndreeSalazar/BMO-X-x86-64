//! **SVG, LO QUE UN ICONO USA**: `<path>`, `<rect>`, `<circle>`, `<ellipse>`,
//! `<line>`, `<polyline>`, `<polygon>` dentro de `<g>` con `transform`, el
//! relleno y el trazo (color, `opacity`, `fill-rule`), y los degradados
//! `<linearGradient>` y `<radialGradient>` (con su `href`). Es lo que traen
//! los iconos de Lucide, Fluent, Material o lo que exporta Inkscape para un
//! icono.
//!
//! Lo que NO: `<text>`, `<image>`, `<use>`, mascaras, recortes, filtros y las
//! hojas `<style>`. No se adivina: lo que no se sabe pintar NO SE PINTA, y la
//! lista de lo saltado sale en `Dibujo::saltado` para que la herramienta lo
//! diga.

use crate::ruta::{self, Matriz, P};

/// Un color en RGBA de 0 a 1 (sin premultiplicar).
pub type Rgba = [f64; 4];

#[derive(Clone, Debug)]
pub enum Pintura {
    Nada,
    Color(Rgba),
    /// Un degradado por su indice en `Dibujo::degradados`.
    Degradado(usize),
    /// `url(#id)` aun sin resolver (los `<defs>` pueden ir al final).
    Ref(String),
}

#[derive(Clone, Debug)]
pub struct Degradado {
    pub id: String,
    pub radial: bool,
    /// `objectBoundingBox` (lo de siempre): los numeros son de la CAJA de la
    /// forma (0 a 1). Si no, son del dibujo.
    pub caja: bool,
    /// Lineal: x1 y1 x2 y2. Radial: cx cy r fx fy.
    pub p: [f64; 5],
    pub paradas: Vec<(f64, Rgba)>,
    pub matriz: Matriz,
    pub href: Option<String>,
    /// Que numeros dijo el fichero (los demas se heredan del `href`).
    dichos: [bool; 5],
}

#[derive(Clone, Debug)]
pub struct Forma {
    pub contornos: Vec<(Vec<P>, bool)>,
    /// De las coordenadas de la forma a las del dibujo.
    pub matriz: Matriz,
    pub relleno: Pintura,
    pub trazo: Pintura,
    pub ancho: f64,
    pub par_impar: bool,
    pub opacidad_relleno: f64,
    pub opacidad_trazo: f64,
}

pub struct Dibujo {
    /// `viewBox`: x, y, ancho, alto.
    pub vista: [f64; 4],
    pub formas: Vec<Forma>,
    pub degradados: Vec<Degradado>,
    /// Las etiquetas que se saltaron por no saber pintarlas.
    pub saltado: Vec<String>,
}

/// Lo que se hereda de padre a hijo.
#[derive(Clone)]
struct Estilo {
    relleno: Pintura,
    trazo: Pintura,
    ancho: f64,
    par_impar: bool,
    opacidad: f64,
    opacidad_relleno: f64,
    opacidad_trazo: f64,
    color: Rgba,
    matriz: Matriz,
    /// Dentro de `<defs>`, de un degradado o de lo que no se pinta.
    callado: bool,
}

impl Estilo {
    fn raiz() -> Self {
        Estilo {
            relleno: Pintura::Color([0.0, 0.0, 0.0, 1.0]),
            trazo: Pintura::Nada,
            ancho: 1.0,
            par_impar: false,
            opacidad: 1.0,
            opacidad_relleno: 1.0,
            opacidad_trazo: 1.0,
            color: [0.0, 0.0, 0.0, 1.0],
            matriz: Matriz::UNO,
            callado: false,
        }
    }
}

/// Una etiqueta leida: su nombre, sus atributos y si se cierra sola.
struct Etiqueta<'a> {
    nombre: &'a str,
    attrs: Vec<(&'a str, String)>,
    sola: bool,
}

impl Etiqueta<'_> {
    fn attr(&self, n: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| *k == n).map(|(_, v)| v.as_str())
    }
    fn num(&self, n: &str, d: f64) -> f64 {
        self.attr(n).map(|v| largo(v, d)).unwrap_or(d)
    }
}

/// Un largo: `12`, `12px`, `50%` (como fraccion: 0,5).
fn largo(v: &str, d: f64) -> f64 {
    let v = v.trim();
    if let Some(p) = v.strip_suffix('%') {
        return ruta::numeros(p).first().map(|n| n / 100.0).unwrap_or(d);
    }
    ruta::numeros(v).first().copied().unwrap_or(d)
}

/// Las entidades de XML que un atributo puede traer.
fn sin_entidades(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

fn etiqueta(t: &str) -> Option<Etiqueta<'_>> {
    let t = t.trim();
    let (t, sola) = match t.strip_suffix('/') {
        Some(r) => (r, true),
        None => (t, false),
    };
    let fin_nombre = t.find(|c: char| c.is_whitespace()).unwrap_or(t.len());
    let nombre = &t[..fin_nombre];
    let mut attrs = Vec::new();
    let b = t.as_bytes();
    let mut i = fin_nombre;
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let ini = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let clave = &t[ini..i];
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            continue;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let q = b[i];
        if q != b'"' && q != b'\'' {
            return None;
        }
        let desde = i + 1;
        let hasta = desde + t[desde..].find(q as char)?;
        attrs.push((clave, sin_entidades(&t[desde..hasta])));
        i = hasta + 1;
    }
    Some(Etiqueta { nombre, attrs, sola })
}

/// **Un color de CSS**: `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()` y
/// los nombres de siempre. `None` = no se entiende.
pub fn color(v: &str) -> Option<Rgba> {
    let v = v.trim().to_ascii_lowercase();
    if let Some(h) = v.strip_prefix('#') {
        let d = |s: &str| u8::from_str_radix(s, 16).ok().map(|n| n as f64 / 255.0);
        return match h.len() {
            3 | 4 => {
                let c: Vec<f64> = h.chars().filter_map(|c| d(&format!("{c}{c}"))).collect();
                (c.len() == h.len()).then(|| [c[0], c[1], c[2], *c.get(3).unwrap_or(&1.0)])
            }
            6 | 8 => {
                let c: Vec<f64> = (0..h.len() / 2).filter_map(|i| d(&h[i * 2..i * 2 + 2])).collect();
                (c.len() == h.len() / 2).then(|| [c[0], c[1], c[2], *c.get(3).unwrap_or(&1.0)])
            }
            _ => None,
        };
    }
    if v.starts_with("rgb") {
        let dentro = &v[v.find('(')? + 1..v.find(')')?];
        let partes: Vec<&str> = dentro.split(|c| c == ',' || c == ' ' || c == '/').filter(|s| !s.is_empty()).collect();
        if partes.len() < 3 {
            return None;
        }
        let canal = |s: &str| if s.ends_with('%') { largo(s, 0.0) } else { largo(s, 0.0) / 255.0 };
        let a = partes.get(3).map(|s| largo(s, 1.0)).unwrap_or(1.0);
        return Some([canal(partes[0]), canal(partes[1]), canal(partes[2]), a]);
    }
    let rgb = match v.as_str() {
        "black" => 0x000000,
        "white" => 0xFFFFFF,
        "red" => 0xFF0000,
        "lime" => 0x00FF00,
        "green" => 0x008000,
        "blue" => 0x0000FF,
        "yellow" => 0xFFFF00,
        "cyan" | "aqua" => 0x00FFFF,
        "magenta" | "fuchsia" => 0xFF00FF,
        "gray" | "grey" => 0x808080,
        "silver" => 0xC0C0C0,
        "maroon" => 0x800000,
        "olive" => 0x808000,
        "navy" => 0x000080,
        "purple" => 0x800080,
        "teal" => 0x008080,
        "orange" => 0xFFA500,
        "gold" => 0xFFD700,
        "pink" => 0xFFC0CB,
        "brown" => 0xA52A2A,
        "darkgray" | "darkgrey" => 0xA9A9A9,
        "lightgray" | "lightgrey" => 0xD3D3D3,
        "transparent" => return Some([0.0, 0.0, 0.0, 0.0]),
        _ => return None,
    };
    Some([((rgb >> 16) & 0xFF) as f64 / 255.0, ((rgb >> 8) & 0xFF) as f64 / 255.0, (rgb & 0xFF) as f64 / 255.0, 1.0])
}

fn pintura(v: &str, actual: Rgba) -> Option<Pintura> {
    let v = v.trim();
    if v == "none" {
        return Some(Pintura::Nada);
    }
    if v == "currentColor" {
        return Some(Pintura::Color(actual));
    }
    if let Some(r) = v.strip_prefix("url(") {
        let id = r.split(')').next()?.trim().trim_matches(|c| c == '\'' || c == '"').trim_start_matches('#');
        return Some(Pintura::Ref(id.to_string()));
    }
    color(v).map(Pintura::Color)
}

/// Aplica UNA propiedad (de atributo o de `style`).
fn propiedad(e: &mut Estilo, k: &str, v: &str) {
    match k {
        "fill" => {
            if let Some(p) = pintura(v, e.color) {
                e.relleno = p;
            }
        }
        "stroke" => {
            if let Some(p) = pintura(v, e.color) {
                e.trazo = p;
            }
        }
        "color" => {
            if let Some(c) = color(v) {
                e.color = c;
            }
        }
        "stroke-width" => e.ancho = largo(v, e.ancho),
        "fill-rule" => e.par_impar = v.trim() == "evenodd",
        "opacity" => e.opacidad *= largo(v, 1.0).clamp(0.0, 1.0),
        "fill-opacity" => e.opacidad_relleno = largo(v, 1.0).clamp(0.0, 1.0),
        "stroke-opacity" => e.opacidad_trazo = largo(v, 1.0).clamp(0.0, 1.0),
        "display" | "visibility" if v.trim() == "none" || v.trim() == "hidden" => e.callado = true,
        _ => {}
    }
}

fn estilo_de(padre: &Estilo, t: &Etiqueta) -> Estilo {
    let mut e = padre.clone();
    // `color` primero: `currentColor` en el mismo elemento lo usa.
    if let Some(c) = t.attr("color").and_then(color) {
        e.color = c;
    }
    for (k, v) in &t.attrs {
        propiedad(&mut e, k, v);
    }
    if let Some(s) = t.attr("style") {
        for decl in s.split(';') {
            if let Some((k, v)) = decl.split_once(':') {
                propiedad(&mut e, k.trim(), v.trim());
            }
        }
    }
    if let Some(tr) = t.attr("transform") {
        e.matriz = padre.matriz.por(&ruta::transformacion(tr));
    }
    e
}

/// La geometria de una etiqueta de forma; `None` si no es una.
fn geometria(t: &Etiqueta) -> Option<Vec<(Vec<P>, bool)>> {
    Some(match t.nombre {
        "path" => ruta::camino(t.attr("d").unwrap_or("")),
        "rect" => {
            let (rx, ry) = match (t.attr("rx"), t.attr("ry")) {
                (Some(a), Some(b)) => (largo(a, 0.0), largo(b, 0.0)),
                (Some(a), None) | (None, Some(a)) => (largo(a, 0.0), largo(a, 0.0)),
                _ => (0.0, 0.0),
            };
            vec![(ruta::rectangulo(t.num("x", 0.0), t.num("y", 0.0), t.num("width", 0.0), t.num("height", 0.0), rx, ry), true)]
        }
        "circle" => {
            let r = t.num("r", 0.0);
            vec![(ruta::elipse(t.num("cx", 0.0), t.num("cy", 0.0), r, r), true)]
        }
        "ellipse" => vec![(ruta::elipse(t.num("cx", 0.0), t.num("cy", 0.0), t.num("rx", 0.0), t.num("ry", 0.0)), true)],
        "line" => vec![(vec![(t.num("x1", 0.0), t.num("y1", 0.0)), (t.num("x2", 0.0), t.num("y2", 0.0))], false)],
        "polyline" | "polygon" => {
            let n = ruta::numeros(t.attr("points").unwrap_or(""));
            let pts: Vec<P> = n.chunks_exact(2).map(|c| (c[0], c[1])).collect();
            vec![(pts, t.nombre == "polygon")]
        }
        _ => return None,
    })
}

fn degradado_de(t: &Etiqueta) -> Degradado {
    let radial = t.nombre == "radialGradient";
    let nombres: [&str; 5] = if radial { ["cx", "cy", "r", "fx", "fy"] } else { ["x1", "y1", "x2", "y2", ""] };
    let base: [f64; 5] = if radial { [0.5, 0.5, 0.5, f64::NAN, f64::NAN] } else { [0.0, 0.0, 1.0, 0.0, 0.0] };
    let mut p = base;
    let mut dichos = [false; 5];
    for i in 0..5 {
        if let Some(v) = t.attr(nombres[i]).filter(|_| !nombres[i].is_empty()) {
            p[i] = largo(v, base[i]);
            dichos[i] = true;
        }
    }
    Degradado {
        id: t.attr("id").unwrap_or("").to_string(),
        radial,
        caja: t.attr("gradientUnits") != Some("userSpaceOnUse"),
        p,
        paradas: Vec::new(),
        matriz: t.attr("gradientTransform").map(ruta::transformacion).unwrap_or(Matriz::UNO),
        href: t.attr("href").or(t.attr("xlink:href")).map(|h| h.trim_start_matches('#').to_string()),
        dichos,
    }
}

/// **Lee un SVG.** `Err` solo si no es un SVG; lo que trae y no se sabe
/// pintar va a `saltado`.
pub fn leer(texto: &str) -> Result<Dibujo, String> {
    let mut d = Dibujo { vista: [0.0, 0.0, 24.0, 24.0], formas: Vec::new(), degradados: Vec::new(), saltado: Vec::new() };
    let mut pila: Vec<Estilo> = vec![Estilo::raiz()];
    let mut nombres: Vec<String> = Vec::new();
    let mut visto_svg = false;
    // El degradado que se esta leyendo (sus `<stop>`).
    let mut abierto: Option<usize> = None;
    let mut resto = texto;
    while let Some(i) = resto.find('<') {
        resto = &resto[i + 1..];
        if let Some(r) = resto.strip_prefix("!--") {
            resto = r.find("-->").map(|f| &r[f + 3..]).unwrap_or("");
            continue;
        }
        if resto.starts_with("![CDATA[") {
            resto = resto.find("]]>").map(|f| &resto[f + 3..]).unwrap_or("");
            continue;
        }
        let Some(f) = resto.find('>') else { break };
        let cuerpo = &resto[..f];
        resto = &resto[f + 1..];
        if cuerpo.starts_with('?') || cuerpo.starts_with('!') {
            continue;
        }
        if let Some(cierre) = cuerpo.strip_prefix('/') {
            let n = cierre.trim();
            if n == "linearGradient" || n == "radialGradient" {
                abierto = None;
            }
            if nombres.last().map(String::as_str) == Some(n) {
                nombres.pop();
                pila.pop();
            }
            continue;
        }
        let Some(t) = etiqueta(cuerpo) else {
            return Err(format!("una etiqueta que no se entiende: <{}>", cuerpo.chars().take(40).collect::<String>()));
        };
        let padre = pila.last().cloned().unwrap_or_else(Estilo::raiz);
        let mut e = estilo_de(&padre, &t);
        match t.nombre {
            "svg" => {
                if !visto_svg {
                    visto_svg = true;
                    let vb = ruta::numeros(t.attr("viewBox").unwrap_or(""));
                    d.vista = if vb.len() == 4 && vb[2] > 0.0 && vb[3] > 0.0 {
                        [vb[0], vb[1], vb[2], vb[3]]
                    } else {
                        [0.0, 0.0, t.num("width", 24.0), t.num("height", 24.0)]
                    };
                }
            }
            "g" | "a" | "switch" => {}
            "defs" | "symbol" | "clipPath" | "mask" | "pattern" | "marker" | "title" | "desc" | "metadata" | "style" => e.callado = true,
            "linearGradient" | "radialGradient" => {
                e.callado = true;
                d.degradados.push(degradado_de(&t));
                abierto = (!t.sola).then_some(d.degradados.len() - 1);
            }
            "stop" => {
                if let Some(g) = abierto {
                    let mut c = t.attr("stop-color").and_then(color).unwrap_or([0.0, 0.0, 0.0, 1.0]);
                    let mut a = t.attr("stop-opacity").map(|v| largo(v, 1.0)).unwrap_or(1.0);
                    for decl in t.attr("style").unwrap_or("").split(';') {
                        match decl.split_once(':').map(|(k, v)| (k.trim(), v.trim())) {
                            Some(("stop-color", v)) => c = color(v).unwrap_or(c),
                            Some(("stop-opacity", v)) => a = largo(v, 1.0),
                            _ => {}
                        }
                    }
                    c[3] *= a.clamp(0.0, 1.0);
                    let previo = d.degradados[g].paradas.last().map(|p| p.0).unwrap_or(0.0);
                    let en = t.attr("offset").map(|v| largo(v, 0.0)).unwrap_or(0.0).clamp(0.0, 1.0).max(previo);
                    d.degradados[g].paradas.push((en, c));
                }
            }
            n => match geometria(&t) {
                Some(contornos) if !e.callado => d.formas.push(Forma {
                    contornos,
                    matriz: e.matriz,
                    relleno: e.relleno.clone(),
                    trazo: e.trazo.clone(),
                    ancho: e.ancho,
                    par_impar: e.par_impar,
                    opacidad_relleno: e.opacidad * e.opacidad_relleno,
                    opacidad_trazo: e.opacidad * e.opacidad_trazo,
                }),
                Some(_) => {}
                None => {
                    if !e.callado && !d.saltado.iter().any(|s| s == n) {
                        d.saltado.push(n.to_string());
                    }
                    e.callado = true;
                }
            },
        }
        if !t.sola {
            nombres.push(t.nombre.to_string());
            pila.push(e);
        }
    }
    if !visto_svg {
        return Err("no es un SVG: no hay <svg>".into());
    }
    resolver(&mut d);
    Ok(d)
}

/// Los `href` de los degradados (paradas y numeros heredados) y los
/// `url(#id)` de las formas.
fn resolver(d: &mut Dibujo) {
    for _ in 0..4 {
        for i in 0..d.degradados.len() {
            let Some(h) = d.degradados[i].href.clone() else { continue };
            let Some(j) = d.degradados.iter().position(|g| g.id == h) else { continue };
            if j == i {
                continue;
            }
            let otro = d.degradados[j].clone();
            let g = &mut d.degradados[i];
            if g.paradas.is_empty() {
                g.paradas = otro.paradas.clone();
            }
            if g.radial == otro.radial {
                for k in 0..5 {
                    if !g.dichos[k] && otro.dichos[k] {
                        g.p[k] = otro.p[k];
                        g.dichos[k] = true;
                    }
                }
            }
        }
    }
    let busca = |id: &str, degradados: &[Degradado]| match degradados.iter().position(|g| g.id == id) {
        Some(i) if !degradados[i].paradas.is_empty() => Pintura::Degradado(i),
        // Un degradado que no existe o no tiene paradas no pinta (SVG 1.1, 13.2.4).
        _ => Pintura::Nada,
    };
    for f in &mut d.formas {
        if let Pintura::Ref(id) = &f.relleno {
            f.relleno = busca(id, &d.degradados);
        }
        if let Pintura::Ref(id) = &f.trazo {
            f.trazo = busca(id, &d.degradados);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn colores_de_css() {
        assert_eq!(color("#fff"), Some([1.0, 1.0, 1.0, 1.0]));
        assert_eq!(color("#FF000080").map(|c| (c[0], (c[3] * 255.0).round())), Some((1.0, 128.0)));
        assert_eq!(color("rgb(255, 0, 0)"), Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(color("orange").map(|c| c[0]), Some(1.0));
        assert_eq!(color("chartreuse-ish"), None);
    }

    #[test]
    fn hereda_del_grupo_y_el_estilo_manda() {
        let d = leer(r##"<svg viewBox="0 0 10 10"><g fill="red" transform="translate(1 2)"><rect width="2" height="2" style="fill:#00f"/><circle r="1"/></g></svg>"##).unwrap();
        assert_eq!(d.formas.len(), 2);
        assert!(matches!(d.formas[0].relleno, Pintura::Color(c) if c == [0.0, 0.0, 1.0, 1.0]));
        assert!(matches!(d.formas[1].relleno, Pintura::Color(c) if c == [1.0, 0.0, 0.0, 1.0]));
        assert_eq!(d.formas[1].matriz.aplica((0.0, 0.0)), (1.0, 2.0));
    }

    #[test]
    fn el_degradado_de_despues_y_su_href() {
        let d = leer(
            r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="url(#b)"/>
            <defs><linearGradient id="a"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient>
            <linearGradient id="b" href="#a" x2="0" y2="1"/></defs></svg>"##,
        )
        .unwrap();
        let Pintura::Degradado(i) = d.formas[0].relleno else { panic!("sin degradado") };
        assert_eq!(d.degradados[i].paradas.len(), 2);
        assert_eq!(d.degradados[i].p[..4], [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn lo_que_no_sabe_lo_dice() {
        let d = leer(r##"<svg viewBox="0 0 4 4"><text>hola</text><image href="x.png"/><rect width="1" height="1"/></svg>"##).unwrap();
        assert_eq!(d.saltado, vec!["text".to_string(), "image".to_string()]);
        assert_eq!(d.formas.len(), 1);
    }
}
