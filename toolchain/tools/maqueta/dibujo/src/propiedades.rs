//! **El estilo de cada figura**, como lo calcula un navegador: lo que hereda
//! del padre, sus atributos de presentacion, las reglas del `<style>` por
//! especificidad, su `style="..."`, y `!important` encima de todo.
//!
//! Lo que NO pinta (la letra, `shape-rendering`...) se lee y se deja; lo
//! que pintaria OTRA cosa y el pintor no sabe (`filter`, `mask`,
//! `vector-effect: non-scaling-stroke`...) es error.

use crate::color;
use crate::css::{Decl, Hoja};
use crate::pluma::{Esquina, Punta};
use crate::xml::Elemento;

/// Una pintura: `fill` o `stroke`.
#[derive(Clone, Debug, PartialEq)]
pub enum Pinta {
    Nada,
    Color(u32, f64),
    Url(String),
    Actual,
}

/// Lo que sabe el dibujo de una figura. Los campos de arriba se heredan.
#[derive(Clone, Debug, PartialEq)]
pub struct Estado {
    pub fill: Pinta,
    pub fill_opacity: f64,
    pub par_impar: bool,
    pub stroke: Pinta,
    pub stroke_width: Largo,
    pub stroke_opacity: f64,
    pub punta: Punta,
    pub esquina: Esquina,
    pub miterlimit: f64,
    pub dasharray: Option<Vec<Largo>>,
    pub dashoffset: Largo,
    pub color: (u32, f64),
    pub visible: bool,
    pub trazo_primero: bool,
    // -- no se heredan ---------------------------------------------------
    pub opacity: f64,
    pub mostrar: bool,
    pub stop_color: (u32, f64),
    pub stop_opacity: f64,
    /// `transform` de CSS (con su origen), aparte del atributo.
    pub transform_css: Option<String>,
    pub transform_origin: Option<String>,
    pub transform_box_figura: bool,
    /// `animation` de CSS (S7), tal cual.
    pub animacion: Option<String>,
    /// `clip-path: url(#id)`: el recorte (convexo) de este elemento.
    pub recorte: Option<String>,
}

/// Una medida de SVG: numero de usuario o porcentaje del viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Largo {
    U(f64),
    Pc(f64),
}

impl Largo {
    /// En unidades del dibujo; `ref_` es la medida contra la que va el `%`.
    pub fn en(&self, ref_: f64) -> f64 {
        match *self {
            Largo::U(v) => v,
            Largo::Pc(p) => p / 100.0 * ref_,
        }
    }
}

/// Lee un largo con su unidad.
pub fn largo(t: &str) -> Option<Largo> {
    let t = t.trim();
    if let Some(n) = t.strip_suffix('%') {
        return n.trim().parse().ok().map(Largo::Pc);
    }
    let corte = t.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(t.len());
    let (n, u) = t.split_at(corte);
    let v: f64 = n.trim().parse().ok()?;
    let k = match u {
        "" | "px" => 1.0,
        "pt" => 4.0 / 3.0,
        "pc" => 16.0,
        "mm" => 96.0 / 25.4,
        "cm" => 96.0 / 2.54,
        "in" => 96.0,
        "em" => 16.0,
        "ex" => 8.0,
        _ => return None,
    };
    Some(Largo::U(v * k))
}

impl Estado {
    pub fn inicial() -> Estado {
        Estado {
            fill: Pinta::Color(0, 1.0),
            fill_opacity: 1.0,
            par_impar: false,
            stroke: Pinta::Nada,
            stroke_width: Largo::U(1.0),
            stroke_opacity: 1.0,
            punta: Punta::Recta,
            esquina: Esquina::Pico,
            miterlimit: 4.0,
            dasharray: None,
            dashoffset: Largo::U(0.0),
            color: (0, 1.0),
            visible: true,
            trazo_primero: false,
            opacity: 1.0,
            mostrar: true,
            stop_color: (0, 1.0),
            stop_opacity: 1.0,
            transform_css: None,
            transform_origin: None,
            transform_box_figura: false,
            animacion: None,
            recorte: None,
        }
    }

    /// Lo que un hijo hereda de este estado.
    pub fn para_hijo(&self) -> Estado {
        Estado {
            opacity: 1.0,
            mostrar: true,
            stop_color: (0, 1.0),
            stop_opacity: 1.0,
            transform_css: None,
            transform_origin: None,
            transform_box_figura: false,
            animacion: None,
            recorte: None,
            ..self.clone()
        }
    }
}

fn pinta(v: &str) -> Option<Pinta> {
    let v = v.trim();
    if v == "none" {
        return Some(Pinta::Nada);
    }
    if v == "currentColor" || v == "currentcolor" {
        return Some(Pinta::Actual);
    }
    if let Some(r) = v.strip_prefix("url(") {
        let cierra = r.find(')')?;
        let id = r[..cierra].trim().trim_matches(|c| c == '"' || c == '\'').strip_prefix('#')?;
        return Some(Pinta::Url(id.to_string()));
    }
    color::leer(v).map(|(c, a)| Pinta::Color(c, a))
}

fn opacidad(v: &str) -> Option<f64> {
    let v = v.trim();
    let n = match v.strip_suffix('%') {
        Some(p) => p.trim().parse::<f64>().ok()? / 100.0,
        None => v.parse().ok()?,
    };
    Some(n.clamp(0.0, 1.0))
}

/// Las propiedades que no pintan una figura y se dejan.
fn se_deja(p: &str) -> bool {
    p.starts_with("font")
        || p.starts_with("text-")
        || p.starts_with("-inkscape")
        || p.starts_with("-webkit")
        || p.starts_with("-moz")
        || p.starts_with("inline-size")
        || matches!(
            p,
            "line-height" | "letter-spacing" | "word-spacing" | "writing-mode" | "direction" | "baseline-shift" | "dominant-baseline" | "alignment-baseline"
                | "shape-rendering" | "color-rendering" | "image-rendering" | "color-interpolation" | "color-interpolation-filters" | "enable-background"
                | "isolation" | "clip-rule" | "overflow" | "white-space" | "shape-inside" | "shape-padding" | "solid-color" | "solid-opacity"
                | "flood-color" | "flood-opacity" | "lighting-color" | "cursor" | "pointer-events" | "unicode-bidi"
        )
}

/// **Aplica una propiedad.** `Err` con por que no se puede.
pub fn aplicar(e: &mut Estado, p: &str, v: &str) -> Result<(), String> {
    let v = v.trim();
    let malo = || format!("`{p}: {v}` no se sabe leer");
    if v == "inherit" {
        // Lo heredado ya esta: no hacer nada es heredar.
        return Ok(());
    }
    match p {
        "fill" => e.fill = pinta(v).ok_or_else(malo)?,
        "stroke" => e.stroke = pinta(v).ok_or_else(malo)?,
        "fill-opacity" => e.fill_opacity = opacidad(v).ok_or_else(malo)?,
        "stroke-opacity" => e.stroke_opacity = opacidad(v).ok_or_else(malo)?,
        "opacity" => e.opacity = opacidad(v).ok_or_else(malo)?,
        "fill-rule" => match v {
            "nonzero" => e.par_impar = false,
            "evenodd" => e.par_impar = true,
            _ => return Err(malo()),
        },
        "stroke-width" => e.stroke_width = largo(v).ok_or_else(malo)?,
        "stroke-linecap" => {
            e.punta = match v {
                "butt" => Punta::Recta,
                "round" => Punta::Redonda,
                "square" => Punta::Cuadrada,
                _ => return Err(malo()),
            }
        }
        "stroke-linejoin" => {
            e.esquina = match v {
                "miter" | "miter-clip" => Esquina::Pico,
                "round" => Esquina::Redonda,
                "bevel" => Esquina::Cortada,
                _ => return Err(malo()),
            }
        }
        "stroke-miterlimit" => e.miterlimit = v.parse::<f64>().ok().filter(|m| *m >= 1.0).ok_or_else(malo)?,
        "stroke-dasharray" => {
            if v == "none" {
                e.dasharray = None;
            } else {
                let partes: Option<Vec<Largo>> = v.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).map(largo).collect();
                let partes = partes.ok_or_else(malo)?;
                e.dasharray = (!partes.is_empty() && partes.iter().any(|l| l.en(1.0) > 0.0)).then_some(partes);
            }
        }
        "stroke-dashoffset" => e.dashoffset = largo(v).ok_or_else(malo)?,
        "color" => e.color = color::leer(v).ok_or_else(malo)?,
        "display" => e.mostrar = v != "none",
        "visibility" => e.visible = v == "visible",
        "stop-color" => {
            e.stop_color = if v == "currentColor" { e.color } else { color::leer(v).ok_or_else(malo)? };
        }
        "stop-opacity" => e.stop_opacity = opacidad(v).ok_or_else(malo)?,
        "paint-order" => {
            let f = v.find("fill");
            let s = v.find("stroke");
            e.trazo_primero = match (f, s) {
                (Some(f), Some(s)) => s < f,
                (None, Some(_)) => true,
                _ => false,
            };
        }
        "transform" => e.transform_css = (v != "none").then(|| v.to_string()),
        "transform-origin" => e.transform_origin = Some(v.to_string()),
        "transform-box" => e.transform_box_figura = v == "fill-box",
        "animation" | "animation-name" | "animation-duration" | "animation-delay" | "animation-iteration-count" | "animation-direction"
        | "animation-timing-function" | "animation-fill-mode" | "animation-play-state" => {
            let previo = e.animacion.take().unwrap_or_default();
            e.animacion = Some(format!("{previo};{p}:{v}"));
        }
        "vector-effect" if v == "none" => {}
        "mix-blend-mode" if v == "normal" => {}
        "filter" | "mask" | "clip-path" | "marker" | "marker-start" | "marker-mid" | "marker-end" if v == "none" => {}
        "clip-path" if v.starts_with("url(") => match pinta(v) {
            Some(Pinta::Url(id)) => e.recorte = Some(id),
            _ => return Err(malo()),
        },
        "clip-path" => return Err(format!("`clip-path: {v}`: solo `url(#...)` a un `<clipPath>` convexo")),
        "vector-effect" | "mix-blend-mode" | "filter" | "mask" | "marker" | "marker-start" | "marker-mid" | "marker-end" => {
            return Err(format!("`{p}: {v}` pide componer en el aparato (eso es de la 3060), y el pintor no lo hace"));
        }
        _ if se_deja(p) => {}
        _ => return Err(format!("la propiedad `{p}` no esta en la lista del dibujo")),
    }
    Ok(())
}

/// Si este atributo es una propiedad de presentacion (lo que `aplicar` lee).
pub fn es_presentacion(n: &str) -> bool {
    matches!(
        n,
        "fill" | "stroke" | "fill-opacity" | "stroke-opacity" | "opacity" | "fill-rule" | "stroke-width" | "stroke-linecap" | "stroke-linejoin"
            | "stroke-miterlimit" | "stroke-dasharray" | "stroke-dashoffset" | "color" | "display" | "visibility" | "stop-color" | "stop-opacity"
            | "paint-order" | "vector-effect" | "mix-blend-mode" | "filter" | "mask" | "clip-path" | "marker" | "marker-start" | "marker-mid"
            | "marker-end" | "transform-origin" | "transform-box"
    ) || se_deja(n)
}

/// **El estado de un elemento**, con su cascada entera. Los errores van a
/// `fallas` como `(pos, mensaje)`.
pub fn estado(el: &Elemento, padre: &Estado, hoja: &Hoja, extra: &[Decl], fallas: &mut Vec<(usize, String)>) -> Estado {
    let mut e = padre.para_hijo();
    // 1. Los atributos de presentacion (lo mas flojo).
    for a in &el.atributos {
        if es_presentacion(&a.nombre) {
            if let Err(m) = aplicar(&mut e, &a.nombre, &a.valor) {
                fallas.push((a.pos, m));
            }
        }
    }
    // 2. Las reglas del `<style>`, por especificidad y orden; luego `style`.
    let clases: Vec<&str> = el.attr("class").map(|c| c.split_whitespace().collect()).unwrap_or_default();
    let id = el.attr("id");
    let mut casan: Vec<(usize, &crate::css::Regla)> = hoja.reglas.iter().enumerate().filter(|(_, r)| r.selector.casa(&el.nombre, &clases, id, el.lugar)).collect();
    casan.sort_by_key(|(k, r)| (r.selector.especificidad(), *k));
    let estilo = el.attr("style").map(crate::css::declaraciones).unwrap_or_default();
    let pos_estilo = el.atributo("style").map_or(el.pos, |a| a.pos);
    for importante in [false, true] {
        for (_, r) in &casan {
            for d in r.decls.iter().filter(|d| d.importante == importante) {
                if let Err(m) = aplicar(&mut e, &d.prop, &d.valor) {
                    fallas.push((el.pos, m));
                }
            }
        }
        for d in estilo.iter().filter(|d| d.importante == importante) {
            if let Err(m) = aplicar(&mut e, &d.prop, &d.valor) {
                fallas.push((pos_estilo, m));
            }
        }
    }
    // 3. Lo que pone una animacion en este instante (S7), encima de todo.
    for d in extra {
        let _ = aplicar(&mut e, &d.prop, &d.valor);
    }
    e
}
