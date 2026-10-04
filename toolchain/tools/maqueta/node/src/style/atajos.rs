//! **Los ATAJOS** (escalon 1, 04-10): `background`, `border` y
//! `border-<lado>` se expanden aqui en sus propiedades largas, como hace un
//! navegador. La cascada solo ve largas.

use bmo_maqueta_diag::Error;
use bmo_maqueta_lex::{Kind, Token};

use super::{at_value_start, color, gradient, measure, skip_value};
use crate::markup::span_of;
use crate::value::{self, Prop, Value};

/// Los atajos que se expanden en propiedades largas.
#[derive(Clone, Copy)]
pub(super) enum Atajo {
    Background,
    /// `border` (los cuatro lados) o `border-<lado>` (uno).
    Border(Option<usize>),
}

impl Atajo {
    pub(super) fn de(nombre: &[u8]) -> Option<Atajo> {
        Some(match nombre {
            b"background" => Atajo::Background,
            b"border" => Atajo::Border(None),
            b"border-top" => Atajo::Border(Some(0)),
            b"border-right" => Atajo::Border(Some(1)),
            b"border-bottom" => Atajo::Border(Some(2)),
            b"border-left" => Atajo::Border(Some(3)),
            _ => return None,
        })
    }

    pub(super) fn leer(self, src: &[u8], toks: &[Token], i: &mut usize, span: bmo_maqueta_diag::Span, errors: &mut Vec<Error>) -> Vec<(Prop, Value)> {
        match self {
            Atajo::Background => fondo(src, toks, i, span, errors),
            Atajo::Border(lado) => borde(src, toks, i, lado, span, errors),
        }
    }
}

/// `background: #RRGGBB | transparent | none | linear-gradient(...)`.
///
/// Como en CSS, el atajo pone las DOS largas: un color quita el degradado
/// que hubiera, y un degradado quita el color.
fn fondo(src: &[u8], toks: &[Token], i: &mut usize, span: bmo_maqueta_diag::Span, errors: &mut Vec<Error>) -> Vec<(Prop, Value)> {
    let Some(t) = toks.get(*i).copied() else { return Vec::new() };
    let mut v = Vec::new();
    if t.kind == Kind::Ident && (t.text(src) == b"transparent" || t.text(src) == b"none") {
        *i += 1;
        v.push((Prop::BackgroundColor, Value::Nothing));
        v.push((Prop::BackgroundImage, Value::Nothing));
    } else if t.kind == Kind::Ident && t.text(src) == b"linear-gradient" {
        if let Some(g) = gradient(src, toks, i, errors) {
            v.push((Prop::BackgroundColor, Value::Nothing));
            v.push((Prop::BackgroundImage, g));
        }
    } else if let Some(c) = color(src, toks, i, Prop::BackgroundColor, errors) {
        v.push((Prop::BackgroundColor, Value::Color(c)));
        v.push((Prop::BackgroundImage, Value::Nothing));
    } else {
        return Vec::new();
    }
    if at_value_start(toks, *i) {
        errors.push(Error::new(
            span,
            "`background` lleva UNA cosa: un color, un degradado, o `none`",
            "las capas de fondo de CSS (imagen encima de color, posicion, repeticion) piden pintar varias veces la misma caja, y una caja aqui es una pieza.",
            "elegir una: el color ya mezclado, o el degradado.",
        ));
        skip_value(toks, i);
        return Vec::new();
    }
    v
}

/// `border[-lado]: <grosor> solid <color>`, o `none`, o `0`.
///
/// ** Los tres son OBLIGATORIOS, y no por capricho: en CSS el que falta
/// toma su valor de serie -- grosor `medium` (3 px), estilo `none` (no se
/// ve) y color `currentColor` --, y ninguno de los tres es algo que el
/// autor haya visto escribir. Sin uno, el navegador dibujaria otra cosa.
fn borde(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    lado: Option<usize>,
    span: bmo_maqueta_diag::Span,
    errors: &mut Vec<Error>,
) -> Vec<(Prop, Value)> {
    let nombre = match lado {
        None => "border".to_string(),
        Some(k) => format!("border-{}", value::LADOS[k]),
    };
    let lados: Vec<usize> = match lado {
        None => vec![0, 1, 2, 3],
        Some(k) => vec![k],
    };
    let grosores = |w: u32| -> Vec<(Prop, Value)> {
        match lado {
            None => vec![(Prop::BorderWidth, Value::Px4([w; 4]))],
            Some(k) => vec![(Prop::grosor_de(k), Value::Px(w))],
        }
    };
    let (mut grosor, mut solido, mut color_v): (Option<u32>, bool, Option<Value>) = (None, false, None);
    let mut nada = false;
    while let Some(t) = toks.get(*i).copied() {
        if matches!(t.kind, Kind::Semi | Kind::RBrace) {
            break;
        }
        match t.kind {
            Kind::Number => match measure(src, toks, i, Prop::BorderWidth, errors) {
                Some(w) => grosor = Some(w),
                None => return Vec::new(),
            },
            Kind::Color => match color(src, toks, i, Prop::BorderColor, errors) {
                Some(c) => color_v = Some(Value::Color(c)),
                None => return Vec::new(),
            },
            Kind::Ident => {
                let w = t.text(src);
                match w {
                    b"solid" => {
                        solido = true;
                        *i += 1;
                    }
                    b"none" => {
                        nada = true;
                        *i += 1;
                    }
                    b"transparent" => {
                        color_v = Some(Value::Nothing);
                        *i += 1;
                    }
                    b"dashed" | b"dotted" | b"double" | b"groove" | b"ridge" | b"inset" | b"outset" => {
                        errors.push(Error::new(
                            span_of(&t),
                            &format!("estilo de borde no soportado -- `{}`", String::from_utf8_lossy(w)),
                            "el borde de la casa es una raya llena; las rayas a trozos se dibujarian distinto en cada esquina que en el navegador.",
                            "`solid`.",
                        ));
                        skip_value(toks, i);
                        return Vec::new();
                    }
                    b"currentColor" | b"currentcolor" => {
                        errors.push(Error::new(
                            span_of(&t),
                            "`currentColor` no existe aqui",
                            "el color de la letra de una caja no siempre esta dicho (no hay herencia ni color de serie), y un borde no puede depender de algo que quiza no existe.",
                            "escribir el color: `#RRGGBB` o `var(--nombre)`.",
                        ));
                        skip_value(toks, i);
                        return Vec::new();
                    }
                    _ => match color(src, toks, i, Prop::BorderColor, errors) {
                        Some(c) => color_v = Some(Value::Color(c)),
                        None => return Vec::new(),
                    },
                }
            }
            _ => {
                errors.push(Error::new(
                    span_of(&t),
                    &format!("`{nombre}` no entiende {}", t.kind.name()),
                    "un borde es grosor, estilo y color.",
                    "por ejemplo `1px solid #2B2250`.",
                ));
                skip_value(toks, i);
                return Vec::new();
            }
        }
    }
    // `none` o `0` a secas: sin borde.
    if nada || (grosor == Some(0) && !solido && color_v.is_none()) {
        return grosores(0);
    }
    let falta = match (grosor, solido, color_v) {
        (None, _, _) => Some("el grosor (`1px`): sin el, el navegador pone `medium`, 3 px"),
        (_, false, _) => Some("`solid`: sin el, el navegador pone `none` y el borde no se ve"),
        (_, _, None) => Some("el color: sin el, el navegador usa el de la letra, que aqui puede no estar dicho"),
        _ => None,
    };
    if let Some(que) = falta {
        errors.push(Error::new(
            span,
            &format!("a `{nombre}` le falta {que}"),
            "en un atajo de CSS, lo que no se escribe toma un valor de serie que nadie vio escribir, y la maqueta se veria distinta en el navegador.",
            "escribir los tres: `1px solid #2B2250`.",
        ));
        return Vec::new();
    }
    let mut v = grosores(grosor.unwrap_or(0));
    let c = color_v.unwrap_or(Value::Nothing);
    match lado {
        None => v.push((Prop::BorderColor, c)),
        Some(_) => {
            for &k in &lados {
                v.push((Prop::color_de(k), c));
            }
        }
    }
    v
}

