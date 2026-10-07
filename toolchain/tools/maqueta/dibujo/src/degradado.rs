//! **Los degradados** (S5): `linearGradient` y `radialGradient`, con su
//! `href` a otro, sus dos clases de unidades, su `gradientTransform` y sus
//! paradas (hasta ocho, como el pintor).
//!
//! Lo que sale es la forma del pintor (`bmo_pinta::Tinta`), en pixeles, y
//! EXACTA para cualquier transformacion afin: un degradado lineal es una
//! funcion lineal del pixel (se da por el punto donde vale 0 y el gradiente
//! perpendicular a sus lineas), y uno radial es un circulo llevado por una
//! matriz (su centro y sus dos ejes).

use std::collections::HashMap;

use crate::css::Hoja;
use crate::geo::{Matriz, P};
use crate::propiedades::{self, largo, Estado, Largo};
use crate::xml::Elemento;
use crate::{Parada, Tinta};

/// Las paradas que caben (las del pintor).
pub const PARADAS: usize = 8;

/// Un atributo del degradado o de los que hereda por `href`.
fn heredado<'a>(ids: &HashMap<String, &'a Elemento>, el: &'a Elemento, n: &str) -> Option<&'a str> {
    let mut e = el;
    for _ in 0..8 {
        if let Some(v) = e.attr(n) {
            return Some(v);
        }
        let h = e.attr("href").or_else(|| e.attr("xlink:href"))?.strip_prefix('#')?;
        e = ids.get(h)?;
    }
    None
}

/// Las paradas, del primero de la cadena `href` que tenga.
fn paradas_de<'a>(ids: &HashMap<String, &'a Elemento>, el: &'a Elemento) -> Vec<&'a Elemento> {
    let mut e = el;
    for _ in 0..8 {
        let ps: Vec<&Elemento> = e.hijos.iter().filter(|h| h.nombre == "stop").collect();
        if !ps.is_empty() {
            return ps;
        }
        let Some(h) = e.attr("href").or_else(|| e.attr("xlink:href")).and_then(|h| h.strip_prefix('#')).and_then(|h| ids.get(h)) else {
            break;
        };
        e = h;
    }
    Vec::new()
}

/// **La tinta de `url(#id)`** para una figura de caja `caja` (en sus
/// unidades) que va al lienzo con `m`, en el viewport `(vw, vh)`.
#[allow(clippy::too_many_arguments)]
pub fn tinta(ids: &HashMap<String, &Elemento>, hoja: &Hoja, id: &str, caja: Option<(f64, f64, f64, f64)>, m: &Matriz, vw: f64, vh: f64, color: (u32, f64)) -> Result<Option<Tinta>, String> {
    let Some(el) = ids.get(id).copied() else {
        return Err(format!("`url(#{id})` no apunta a ningun degradado del dibujo"));
    };
    let lineal = match el.nombre.as_str() {
        "linearGradient" => true,
        "radialGradient" => false,
        otro => return Err(format!("`url(#{id})` apunta a un `<{otro}>`: solo se pinta con degradados")),
    };
    if heredado(ids, el, "spreadMethod").is_some_and(|s| s != "pad") {
        return Err(format!("el degradado `#{id}` se repite (`spreadMethod`): el pintor solo extiende la ultima parada (`pad`)"));
    }
    // Las paradas, con su cascada (un `stop` puede tener clase o `style`).
    let mut base = Estado::inicial();
    base.color = color;
    let mut ps = Vec::new();
    let mut antes = 0.0f64;
    for s in paradas_de(ids, el) {
        let mut tiradas = Vec::new();
        let e = propiedades::estado(s, &base, hoja, &[], &mut tiradas);
        let en = match s.attr("offset").map(|o| o.trim()) {
            Some(o) => match o.strip_suffix('%') {
                Some(p) => p.trim().parse::<f64>().map(|v| v / 100.0).unwrap_or(0.0),
                None => o.parse::<f64>().unwrap_or(0.0),
            },
            None => 0.0,
        }
        .clamp(0.0, 1.0)
        .max(antes);
        antes = en;
        ps.push(Parada { en, c: e.stop_color.0, alfa: e.stop_color.1 * e.stop_opacity });
    }
    if ps.is_empty() {
        return Ok(None);
    }
    if ps.len() == 1 {
        return Ok(Some(Tinta::Liso(ps[0].c)).map(|t| if ps[0].alfa < 1.0 { Tinta::Lineal { de: (0.0, 0.0), a: (1.0, 0.0), paradas: ps.clone() } } else { t }));
    }
    if ps.len() > PARADAS {
        return Err(format!("el degradado `#{id}` tiene {} paradas y el pintor lleva hasta {PARADAS}", ps.len()));
    }
    let caja_obj = heredado(ids, el, "gradientUnits").is_none_or(|u| u != "userSpaceOnUse");
    let (bx, by, bw, bh) = match (caja_obj, caja) {
        (true, Some(c)) if c.2 > 0.0 && c.3 > 0.0 => c,
        // Una figura sin area (una raya recta) con `objectBoundingBox`: el
        // navegador no la pinta.
        (true, _) => return Ok(None),
        (false, _) => (0.0, 0.0, vw, vh),
    };
    let leer = |n: &str, defecto: &str, refm: f64| -> Result<f64, String> {
        let v = heredado(ids, el, n).unwrap_or(defecto);
        let l = largo(v).ok_or_else(|| format!("`{n}=\"{v}\"` del degradado `#{id}` no se sabe leer"))?;
        Ok(match (caja_obj, l) {
            (true, Largo::Pc(p)) => p / 100.0,
            (true, Largo::U(u)) => u,
            (false, l) => l.en(refm),
        })
    };
    let diag = ((vw * vw + vh * vh) / 2.0).sqrt();
    let g = match heredado(ids, el, "gradientTransform") {
        Some(t) => crate::geo::transform(t).map_err(|t| format!("`gradientTransform` del degradado `#{id}`: `{t}` no se sabe leer"))?,
        None => Matriz::UNO,
    };
    let unidades = if caja_obj { Matriz::mover(bx, by).por(&Matriz::escala(bw, bh)) } else { Matriz::UNO };
    let total = m.por(&unidades).por(&g);
    if lineal {
        let p1 = (leer("x1", "0%", vw)?, leer("y1", "0%", vh)?);
        let p2 = (leer("x2", "100%", vw)?, leer("y2", "0%", vh)?);
        let v = (p2.0 - p1.0, p2.1 - p1.1);
        let vv = v.0 * v.0 + v.1 * v.1;
        let Some(inv) = total.inversa() else { return Ok(Some(Tinta::Liso(ps[ps.len() - 1].c))) };
        if vv == 0.0 {
            return Ok(Some(Tinta::Liso(ps[ps.len() - 1].c)));
        }
        // w = A^-T v / |v|^2: el gradiente de t en el lienzo.
        let w = ((inv.a * v.0 + inv.b * v.1) / vv, (inv.c * v.0 + inv.d * v.1) / vv);
        let ww = w.0 * w.0 + w.1 * w.1;
        let de: P = total.punto(p1);
        Ok(Some(Tinta::Lineal { de, a: (de.0 + w.0 / ww, de.1 + w.1 / ww), paradas: ps }))
    } else {
        let c = (leer("cx", "50%", vw)?, leer("cy", "50%", vh)?);
        let r = leer("r", "50%", diag)?;
        let f = (heredado(ids, el, "fx").map_or(Ok(c.0), |_| leer("fx", "50%", vw))?, heredado(ids, el, "fy").map_or(Ok(c.1), |_| leer("fy", "50%", vh))?);
        if (f.0 - c.0).abs() > 1e-6 || (f.1 - c.1).abs() > 1e-6 || heredado(ids, el, "fr").is_some_and(|v| largo(v).is_some_and(|l| l.en(1.0) != 0.0)) {
            return Err(format!("el degradado radial `#{id}` tiene el foco fuera del centro (`fx`/`fy`/`fr`): el pintor lleva circulos con centro, no conos"));
        }
        if r <= 0.0 {
            return Ok(Some(Tinta::Liso(ps[ps.len() - 1].c)));
        }
        Ok(Some(Tinta::Radial { centro: total.punto(c), eje_x: (total.a * r, total.b * r), eje_y: (total.c * r, total.d * r), paradas: ps }))
    }
}
