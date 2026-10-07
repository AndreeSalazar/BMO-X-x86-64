//! # MAQUETA -- EL LECTOR DE SVG (MAQUETA 3, 06-10)
//!
//! generacion: abuelo -- lee un dibujo; no sabe de cajas ni de quien lo lee
//!
//! `docs/componente/LA_MAQUETA_EXIGE.md`, seccion 2e, y los escalones S1 a
//! S7 de `docs/plan/PLAN_MAQUETA_3.md`. El propietario: *"es para tener mi
//! BMO-X con TODO el SVG en internet y animar todo"*.
//!
//! ```text
//!    leer_dentro    el SVG escrito en una maqueta (`<svg>...</svg>`)
//!    leer_fichero   un fichero de internet (`<svg src="logo.svg"/>`)
//!    figuras        el dibujo APLANADO en una caja: caminos de pixel con
//!                   su tinta, su opacidad y su regla, para `bmo-pinta`
//!    pasos          (S7) lo mismo en cada paso de su animacion
//! ```
//!
//! ## La regla, la de siempre
//!
//! Un navegador ignora lo que no entiende; esto lo RECHAZA. Un SVG con
//! `<text>`, `<filter>`, `<mask>`... no se pinta a medias: el error dice
//! TODO lo que el fichero tiene y no se puede, de una vez, con su linea.
//!
//! ## Por que vive aparte de `node`
//!
//! El SVG no es MAQUETA: tiene su XML, su CSS, sus unidades y su herencia
//! (un `fill` en un `<g>` SI llega a sus hijos -- en un dibujo eso es la
//! norma, no L7). Meterlo en el padre seria darle al padre otro idioma.
//! El padre solo le pasa el texto y recoge un `Dibujo`.

#![forbid(unsafe_code)]

pub mod anima;
pub mod color;
pub mod css;
pub mod degradado;
pub mod escena;
pub mod figuras;
pub mod geo;
pub mod pluma;
pub mod propiedades;
pub mod xml;

use std::collections::HashMap;
use std::sync::Arc;

use bmo_maqueta_diag::{Error, Span};

pub use pluma::{Esquina, Punta};
pub use xml::Elemento;

/// Un rechazo del lector: donde (en el texto que se le dio) y las tres
/// partes de siempre.
#[derive(Clone, Debug, PartialEq)]
pub struct Falla {
    pub pos: usize,
    pub largo: usize,
    pub que: String,
    pub por_que: String,
    pub en_su_lugar: String,
}

impl Falla {
    pub fn en(pos: usize, largo: usize, que: &str, por_que: &str, en_su_lugar: &str) -> Falla {
        Falla { pos, largo, que: que.to_string(), por_que: por_que.to_string(), en_su_lugar: en_su_lugar.to_string() }
    }
}

/// Una parada de un degradado, ya resuelta.
#[derive(Clone, Debug, PartialEq)]
pub struct Parada {
    /// 0..=1.
    pub en: f64,
    pub c: u32,
    /// 0..=1.
    pub alfa: f64,
}

/// **La tinta de una figura**, en pixeles (la forma de `bmo_pinta::Tinta`).
#[derive(Clone, Debug, PartialEq)]
pub enum Tinta {
    Liso(u32),
    Lineal { de: geo::P, a: geo::P, paradas: Vec<Parada> },
    Radial { centro: geo::P, eje_x: geo::P, eje_y: geo::P, paradas: Vec<Parada> },
}

/// **Una figura aplanada**: lo que pinta `bmo_pinta::Pieza::Figura`, en
/// pixeles (con decimales; el emisor los lleva a 1/64).
#[derive(Clone, Debug, PartialEq)]
pub struct Figura {
    pub caminos: Vec<Vec<geo::P>>,
    pub cerrados: Vec<bool>,
    /// El grosor de la pluma redonda en pixeles; 0, relleno.
    pub pluma: f64,
    pub tinta: Tinta,
    /// 0..=1.
    pub alfa: f64,
    pub par_impar: bool,
    /// De que elemento sale, para el comentario del codigo generado.
    pub de: String,
}

impl Figura {
    /// Si es lo que MAQUETA 2 ya sabia pintar: pluma redonda o relleno
    /// par-impar, lisa y opaca.
    pub fn de_siempre(&self) -> Option<u32> {
        match self.tinta {
            Tinta::Liso(c) if self.alfa >= 1.0 && (self.pluma > 0.0 || self.par_impar) => Some(c),
            _ => None,
        }
    }
}

/// **Lo que hereda el dibujo de la regla de su `<svg>`** en la maqueta.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Herencia {
    /// `Some(None)` = `fill: none`.
    pub fill: Option<Option<u32>>,
    pub stroke: Option<Option<u32>>,
    /// En unidades del dibujo.
    pub stroke_width: Option<f64>,
    pub punta: Option<Punta>,
    pub esquina: Option<Esquina>,
}

impl Herencia {
    pub fn estado(&self) -> propiedades::Estado {
        use propiedades::{Largo, Pinta};
        let mut e = propiedades::Estado::inicial();
        let pinta = |c: Option<u32>| c.map_or(Pinta::Nada, |c| Pinta::Color(c, 1.0));
        if let Some(f) = self.fill {
            e.fill = pinta(f);
        }
        if let Some(s) = self.stroke {
            e.stroke = pinta(s);
        }
        if let Some(w) = self.stroke_width {
            e.stroke_width = Largo::U(w);
        }
        if let Some(p) = self.punta {
            e.punta = p;
        }
        if let Some(q) = self.esquina {
            e.esquina = q;
        }
        e
    }
}

/// De donde salio el texto, para poner los errores en su sitio.
#[derive(Clone, Debug)]
enum Origen {
    /// Dentro de la maqueta: las posiciones son del fichero de la maqueta.
    Dentro,
    /// Un fichero aparte: el error se cuelga del `<svg src>` (`ancla`) y
    /// dice la linea del fichero.
    Fichero { nombre: String, ancla: Span },
}

/// **Un dibujo leido y juzgado.**
#[derive(Clone, Debug)]
pub struct Svg {
    pub raiz: Elemento,
    pub hoja: css::Hoja,
    pub vista: Option<[f64; 4]>,
    pub aspecto: String,
    /// La medida que dice el fichero (`width`/`height` de la raiz), en px.
    pub medida: Option<(f64, f64)>,
    /// Las animaciones (S7), ya leidas.
    pub animaciones: Vec<anima::Animacion>,
    fuente: Arc<Vec<u8>>,
    origen: Origen,
}

/// **Un dibujo** como lo lleva el arbol de MAQUETA: compartido, y dos son
/// iguales si son el mismo.
#[derive(Clone, Debug)]
pub struct Dibujo(pub Arc<Svg>);

impl PartialEq for Dibujo {
    fn eq(&self, o: &Self) -> bool {
        Arc::ptr_eq(&self.0, &o.0)
    }
}
impl Eq for Dibujo {}

impl std::ops::Deref for Dibujo {
    type Target = Svg;
    fn deref(&self) -> &Svg {
        &self.0
    }
}

fn linea_col(s: &[u8], pos: usize) -> (u32, u32) {
    let pos = pos.min(s.len());
    let antes = &s[..pos];
    let linea = antes.iter().filter(|&&b| b == b'\n').count() as u32 + 1;
    let col = (pos - antes.iter().rposition(|&b| b == b'\n').map_or(0, |k| k + 1)) as u32 + 1;
    (linea, col)
}

impl Svg {
    /// La linea (del fichero de donde salio) de una posicion.
    pub fn linea_de(&self, pos: usize) -> u32 {
        linea_col(&self.fuente, pos).0
    }

    /// Los elementos con `id`, para `use` y `url(#...)`.
    pub fn ids(&self) -> HashMap<String, &Elemento> {
        fn ir<'a>(e: &'a Elemento, m: &mut HashMap<String, &'a Elemento>) {
            if let Some(i) = e.attr("id") {
                m.entry(i.to_string()).or_insert(e);
            }
            for h in &e.hijos {
                ir(h, m);
            }
        }
        let mut m = HashMap::new();
        ir(&self.raiz, &mut m);
        m
    }

    /// **Un rechazo, en la forma de MAQUETA.**
    pub fn error(&self, f: &Falla) -> Error {
        match &self.origen {
            Origen::Dentro => {
                let (l, c) = linea_col(&self.fuente, f.pos);
                Error::new(Span::new(f.pos, f.largo.max(1), l, c), &f.que, &f.por_que, &f.en_su_lugar)
            }
            Origen::Fichero { nombre, ancla } => {
                let (l, c) = linea_col(&self.fuente, f.pos);
                Error::new(*ancla, &format!("{nombre}:{l}:{c}: {}", f.que), &f.por_que, &f.en_su_lugar)
            }
        }
    }

    /// La caja del dibujo en sus unidades: el `viewBox`, o la medida.
    pub fn vista_o_medida(&self) -> Option<[f64; 4]> {
        self.vista.or(self.medida.map(|(w, h)| [0.0, 0.0, w, h]))
    }

    /// Como va el dibujo a la caja `(x, y, w, h)` del lienzo.
    pub fn encaje(&self, caja: (f64, f64, f64, f64)) -> geo::Matriz {
        match self.vista_o_medida() {
            Some(vb) if vb[2] > 0.0 && vb[3] > 0.0 => escena::encaje(vb, &self.aspecto, caja),
            _ => geo::Matriz::mover(caja.0, caja.1),
        }
    }
}

// ---------------------------------------------------------------------------
// La lista cerrada
// ---------------------------------------------------------------------------

/// Lo que no se pinta y por que, con lo que se puede hacer.
fn rechazo(n: &str) -> Option<(&'static str, &'static str)> {
    Some(match n {
        "text" | "tspan" | "textPath" => ("un texto de SVG pide SU letra, que BMO-X no tiene: la casa pinta la suya", "pasar el texto a caminos en el editor (Inkscape: Trayecto > Objeto a trayecto), o ponerlo como `<span>` de la maqueta"),
        "filter" => ("un filtro compone la figura aparte y la emborrona o la mezcla: eso es de la 3060 (VERRANO), no de la maqueta", "quitar el filtro; un brillo se hace con `box-shadow` en la caja"),
        "mask" => ("una mascara compone aparte y mezcla con otra imagen: eso es de la 3060", "quitar la mascara, o aplanarla en el editor"),
        "pattern" => ("un patron repite una figura por dentro de otra: el pintor no lo hace", "dibujar las figuras sueltas"),
        "image" => ("una imagen dentro de un SVG no se lee aqui", "ponerla como `<imagen src>` de la maqueta"),
        "foreignObject" => ("HTML dentro de un SVG no es un dibujo", "sacarlo a la maqueta"),
        "script" => ("un dibujo no ejecuta nada", "quitar el `<script>`"),
        "marker" => ("las flechas de `marker` se pintan en cada vertice; el pintor no lo hace", "dibujar la flecha como figura"),
        "switch" => ("`switch` elige segun el navegador; aqui no hay navegador", "dejar la rama que se quiere"),
        "svg" => ("un `<svg>` dentro de otro no esta en la lista", "usar un `<g>` con su `transform`, o un `<symbol>` con `<use>`"),
        "animateMotion" | "set" | "animateColor" => ("esta animacion no esta en la lista de S7", "`<animate>`, `<animateTransform>` o `@keyframes`"),
        _ if n.starts_with("fe") => ("un efecto de filtro es de la 3060", "quitar el filtro"),
        _ => return None,
    })
}

const ELEMENTOS: &[&str] = &[
    "g", "a", "defs", "symbol", "use", "clipPath", "path", "circle", "ellipse", "rect", "line", "polyline", "polygon", "linearGradient", "radialGradient", "stop", "style",
    "title", "desc", "metadata", "animate", "animateTransform",
];

fn atributo_conocido(n: &str) -> bool {
    propiedades::es_presentacion(n)
        || n.starts_with("xmlns")
        || n.starts_with("xml:")
        || n.starts_with("data-")
        || n.starts_with("aria-")
        || n.starts_with("sodipodi:")
        || n.starts_with("inkscape:")
        || matches!(
            n,
            "id" | "class" | "style" | "version" | "baseProfile" | "role" | "focusable" | "tabindex" | "lang" | "x" | "y" | "width" | "height"
                | "viewBox" | "preserveAspectRatio" | "transform" | "d" | "cx" | "cy" | "r" | "rx" | "ry" | "x1" | "y1" | "x2" | "y2" | "points"
                | "pathLength" | "href" | "xlink:href" | "xlink:title" | "target" | "gradientUnits" | "gradientTransform" | "spreadMethod" | "fx" | "fy"
                | "fr" | "offset" | "attributeName" | "attributeType" | "from" | "to" | "by" | "values" | "keyTimes" | "keySplines" | "calcMode"
                | "dur" | "begin" | "end" | "repeatCount" | "repeatDur" | "restart" | "additive" | "accumulate" | "type" | "media" | "refX" | "refY"
                | "clipPathUnits"
        )
}

fn validar(svg_txt: &[u8], el: &Elemento, raiz: bool, dentro: bool, fallas: &mut Vec<Falla>) {
    let n = el.nombre.as_str();
    if !raiz {
        if n.contains(':') || matches!(n, "title" | "desc" | "metadata") {
            return;
        }
        if let Some((por_que, en_su_lugar)) = rechazo(n) {
            fallas.push(Falla::en(el.pos, n.len() + 1, &format!("`<{n}>` no se puede pintar"), por_que, en_su_lugar));
            return;
        }
        if !ELEMENTOS.contains(&n) {
            fallas.push(Falla::en(el.pos, n.len() + 1, &format!("`<{n}>` no es una etiqueta del dibujo"), "la lista del dibujo esta CERRADA (LA_MAQUETA_EXIGE.md, 2e): lo que no esta, no se pinta a medias.", "las figuras (`path`, `circle`, `rect`...), `g`, `use`, los degradados y las animaciones."));
            return;
        }
        if n == "style" && dentro {
            fallas.push(Falla::en(el.pos, 6, "un `<style>` dentro del dibujo de una maqueta", "la maqueta tiene UN `<style>`, el suyo; el de un dibujo solo vale en un fichero `<svg src>`.", "los colores como atributos (`fill=\"#...\"`), o el dibujo en su fichero."));
        }
    }
    for a in &el.atributos {
        if !atributo_conocido(&a.nombre) {
            fallas.push(Falla::en(a.pos, a.nombre.len(), &format!("el atributo `{}` no esta en la lista del dibujo", a.nombre), "un atributo que se lee y no se hace es una linea que miente.", "ver LA_MAQUETA_EXIGE.md, 2e."));
            continue;
        }
        let malo = |fallas: &mut Vec<Falla>, que: String| fallas.push(Falla::en(a.pos, a.largo, &que, "el valor no se sabe leer: un navegador lo ignoraria en silencio y pintaria otra cosa.", "revisarlo en el editor."));
        match a.nombre.as_str() {
            "d" if n == "path" => {
                if let Err(k) = geo::camino(&a.valor) {
                    malo(fallas, format!("este `d` deja de leerse en el caracter {k}"));
                }
            }
            "points" => {
                if geo::numeros(&a.valor).is_none() {
                    malo(fallas, "estos `points` no son numeros".to_string());
                }
            }
            "transform" if !n.ends_with("Gradient") => {
                if let Err(t) = geo::transform(&a.valor) {
                    malo(fallas, format!("`transform`: `{t}` no se sabe leer"));
                }
            }
            "style" => {
                let mut e = propiedades::Estado::inicial();
                for d in css::declaraciones(&a.valor) {
                    if let Err(m) = propiedades::aplicar(&mut e, &d.prop, &d.valor) {
                        malo(fallas, m);
                    }
                }
            }
            p if propiedades::es_presentacion(p) && n != "animate" => {
                let mut e = propiedades::Estado::inicial();
                if let Err(m) = propiedades::aplicar(&mut e, p, &a.valor) {
                    malo(fallas, m);
                }
            }
            _ => {}
        }
    }
    let _ = svg_txt;
    for h in &el.hijos {
        validar(svg_txt, h, false, dentro, fallas);
    }
}

fn hoja_de(raiz: &Elemento, fallas: &mut Vec<Falla>) -> css::Hoja {
    let mut h = css::Hoja::default();
    fn ir(e: &Elemento, h: &mut css::Hoja, fallas: &mut Vec<Falla>) {
        if e.nombre == "style" {
            let (mut una, malos) = css::hoja(&e.texto);
            for m in malos {
                fallas.push(Falla::en(e.pos, 6, &format!("en el `<style>` del dibujo: `{m}` no se puede pintar"), "solo selectores de una pieza (`.clase`, `#id`, `etiqueta`) y `@keyframes`: uno de varios pisos o con `:` pinta algo que aqui no se calcula.", "una clase por figura (lo que exportan los editores)."));
            }
            h.reglas.append(&mut una.reglas);
            h.fotogramas.append(&mut una.fotogramas);
        }
        for k in &e.hijos {
            ir(k, h, fallas);
        }
    }
    ir(raiz, &mut h, fallas);
    // Los valores de las reglas, juzgados ya.
    for r in &h.reglas {
        let mut e = propiedades::Estado::inicial();
        for d in &r.decls {
            if let Err(m) = propiedades::aplicar(&mut e, &d.prop, &d.valor) {
                fallas.push(Falla::en(raiz.pos, 1, &format!("en el `<style>` del dibujo: {m}"), "lo que el pintor no hace, no se pinta a medias.", "ver LA_MAQUETA_EXIGE.md, 2e."));
            }
        }
    }
    h
}

fn terminar(raiz: Elemento, vista: Option<[f64; 4]>, aspecto: String, medida: Option<(f64, f64)>, fuente: Arc<Vec<u8>>, origen: Origen, mut fallas: Vec<Falla>, dentro: bool) -> Result<Svg, Vec<Error>> {
    validar(&fuente, &raiz, true, dentro, &mut fallas);
    let hoja = hoja_de(&raiz, &mut fallas);
    let mut svg = Svg { raiz, hoja, vista, aspecto, medida, animaciones: Vec::new(), fuente, origen };
    let (animaciones, mut fa) = anima::leer(&svg);
    fallas.append(&mut fa);
    svg.animaciones = animaciones;
    if fallas.is_empty() {
        Ok(svg)
    } else {
        Err(fallas.iter().map(|f| svg.error(f)).collect())
    }
}

/// **Lee el SVG escrito en una maqueta**: `fuente[ini..fin]` es lo de
/// DENTRO del `<svg>`; su `viewBox` y su `preserveAspectRatio` los leyo la
/// maqueta. Los errores, con sus posiciones en la maqueta.
pub fn leer_dentro(fuente: &[u8], ini: usize, fin: usize, vista: Option<[f64; 4]>, aspecto: &str) -> Result<Svg, Vec<Error>> {
    let mut fallas = Vec::new();
    let hijos = xml::leer(fuente, ini, fin, &mut fallas);
    let raiz = Elemento { nombre: "svg".to_string(), atributos: Vec::new(), hijos, texto: String::new(), pos: ini, largo: fin - ini, lugar: (1, 1) };
    terminar(raiz, vista, aspecto.to_string(), None, Arc::new(fuente.to_vec()), Origen::Dentro, fallas, true)
}

/// **Lee un fichero SVG.** `ancla` es el `<svg src>` de la maqueta: ahi se
/// cuelgan sus errores, con la linea del fichero en el titulo.
pub fn leer_fichero(nombre: &str, texto: &[u8], ancla: Span) -> Result<Svg, Vec<Error>> {
    let mut fallas = Vec::new();
    let elementos = xml::leer(texto, 0, texto.len(), &mut fallas);
    let origen = Origen::Fichero { nombre: nombre.to_string(), ancla };
    let fuente = Arc::new(texto.to_vec());
    let raiz = match elementos.into_iter().find(|e| e.nombre == "svg" || e.nombre.ends_with(":svg")) {
        Some(r) => r,
        None => {
            let s = Svg { raiz: Elemento { nombre: "svg".into(), atributos: Vec::new(), hijos: Vec::new(), texto: String::new(), pos: 0, largo: 0, lugar: (1, 1) }, hoja: Default::default(), vista: None, aspecto: String::new(), medida: None, animaciones: Vec::new(), fuente, origen };
            fallas.push(Falla::en(0, 1, "este fichero no tiene un `<svg>` de raiz", "un dibujo empieza por `<svg ...>`.", "exportarlo de nuevo como SVG."));
            return Err(fallas.iter().map(|f| s.error(f)).collect());
        }
    };
    let vista = raiz.attr("viewBox").and_then(geo::numeros).filter(|v| v.len() == 4 && v[2] > 0.0 && v[3] > 0.0).map(|v| [v[0], v[1], v[2], v[3]]);
    let medida_de = |n: &str| raiz.attr(n).and_then(propiedades::largo).and_then(|l| match l {
        propiedades::Largo::U(v) if v > 0.0 => Some(v),
        _ => None,
    });
    let medida = match (medida_de("width"), medida_de("height")) {
        (Some(w), Some(h)) => Some((w, h)),
        (Some(w), None) => vista.map(|v| (w, w * v[3] / v[2])),
        (None, Some(h)) => vista.map(|v| (h * v[2] / v[3], h)),
        (None, None) => None,
    };
    let aspecto = raiz.attr("preserveAspectRatio").unwrap_or("").to_string();
    if vista.is_none() && medida.is_none() {
        fallas.push(Falla::en(raiz.pos, 4, "este SVG no dice ni su `viewBox` ni su medida", "sin ninguno de los dos no se sabe en que coordenadas estan sus figuras.", "exportarlo con `viewBox`."));
    }
    terminar(raiz, vista, aspecto, medida, fuente, origen, fallas, false)
}

/// **El dibujo aplanado** en la caja `(x, y, w, h)` del lienzo (pixeles).
/// Los errores son los que dependen de lo que hereda (un `url(#...)` roto,
/// una opacidad de grupo): el veredicto los dice.
pub fn figuras(svg: &Svg, h: &Herencia, caja: (f64, f64, f64, f64)) -> (Vec<Figura>, Vec<Error>) {
    figuras::en(svg, h, caja, &|_| escena::Ajuste::default(), None, true)
}

/// **Los pasos de su animacion** (S7). `None` si no anima.
pub fn pasos(svg: &Svg, h: &Herencia, caja: (f64, f64, f64, f64)) -> Option<Result<anima::Pasos, Vec<Error>>> {
    anima::pasos(svg, h, caja)
}
