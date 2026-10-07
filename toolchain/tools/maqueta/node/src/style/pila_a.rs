//! **Los valores de la PILA A** (MAQUETA 3, 06-10, MA2): los que no son una
//! medida de siempre. `LA_MAQUETA_EXIGE.md` 3e.
//!
//! ```text
//!    con signo    `-4px`, `12px`, `0`      left/top/right/bottom, outline-offset
//!    cuenta       `30`                     z-index
//!    proporcion   `16 / 9`, `16/9`, `1`    aspect-ratio
//!    contorno     `2px solid #RRGGBB`      outline (o `none`, `0`)
//! ```

use bmo_maqueta_diag::Error;
use bmo_maqueta_lex::{Kind, Token};

use super::{color, skip_value};
use crate::markup::span_of;
use crate::value::{Prop, Value};

/// Un entero de pixeles, con signo. `None` (y el error dicho) si no lo es.
pub(super) fn con_signo(src: &[u8], toks: &[Token], i: &mut usize, prop: Prop, errors: &mut Vec<Error>) -> Option<i32> {
    let t = *toks.get(*i)?;
    let mal = |errors: &mut Vec<Error>, t: &Token, que: &str, porque: &str| {
        errors.push(Error::new(span_of(t), &format!("`{}` {que}", prop.name()), porque, "por ejemplo `-4px`, `12px` o `0`."));
    };
    let texto = t.text(src);
    let (neg, cifras) = match texto.first() {
        Some(b'-') => (true, &texto[1..]),
        _ => (false, texto),
    };
    if t.kind != Kind::Number || cifras.is_empty() || !cifras.iter().all(u8::is_ascii_digit) {
        mal(errors, &t, "quiere pixeles ENTEROS", "una caja cae en pixel entero: medio pixel es un borde borroso.");
        skip_value(toks, i);
        return None;
    }
    let mut n: i64 = 0;
    for &c in cifras {
        n = n * 10 + (c - b'0') as i64;
        if n > i32::MAX as i64 {
            mal(errors, &t, "no cabe", "las distancias son enteros de 32 bits.");
            skip_value(toks, i);
            return None;
        }
    }
    *i += 1;
    match toks.get(*i) {
        Some(u) if u.kind == Kind::Ident && u.text(src) == b"px" => *i += 1,
        _ if n == 0 => {}
        Some(u) if u.kind == Kind::Ident || u.kind == Kind::Pct => {
            mal(errors, u, "solo va en `px`", "un porcentaje o un `em` piden medir contra un contenedor que una pieza no conoce (L7).");
            skip_value(toks, i);
            return None;
        }
        _ => {
            mal(errors, &t, "lleva su `px`", "las medidas llevan `px` siempre, menos el cero.");
            skip_value(toks, i);
            return None;
        }
    }
    Some(if neg { -(n as i32) } else { n as i32 })
}

/// Un entero sin unidad y sin signo (`z-index`).
pub(super) fn cuenta(src: &[u8], toks: &[Token], i: &mut usize, prop: Prop, errors: &mut Vec<Error>) -> Option<u32> {
    let t = *toks.get(*i)?;
    let txt = t.text(src);
    if t.kind == Kind::Number && !txt.is_empty() && txt.iter().all(u8::is_ascii_digit) && txt.len() <= 6 {
        *i += 1;
        return std::str::from_utf8(txt).ok()?.parse().ok();
    }
    errors.push(Error::new(
        span_of(&t),
        &format!("`{}` quiere un entero desde 0", prop.name()),
        "la capa es un numero GLOBAL y ordenado. Una capa negativa en CSS se pinta \
         DEBAJO del fondo de su padre, y aqui no hay contexto de apilado que lo haga.",
        "por ejemplo `z-index: 5`, y mover la caja en el fichero si tiene que ir debajo.",
    ));
    skip_value(toks, i);
    None
}

/// `A / B`, `A/B` o `A` (= `A / 1`), enteros desde 1.
pub(super) fn proporcion(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>) -> Option<Value> {
    let entero = |t: Option<&Token>| -> Option<u32> {
        let t = t?;
        let x = t.text(src);
        (t.kind == Kind::Number && !x.is_empty() && x.iter().all(u8::is_ascii_digit) && x.len() <= 5)
            .then(|| std::str::from_utf8(x).ok()?.parse().ok())
            .flatten()
            .filter(|&n: &u32| n > 0)
    };
    let inicio = *toks.get(*i)?;
    if let Some(a) = entero(toks.get(*i)) {
        let barra = toks.get(*i + 1).is_some_and(|t| t.text(src) == b"/");
        if !barra {
            *i += 1;
            return Some(Value::Ratio(a, 1));
        }
        if let Some(b) = entero(toks.get(*i + 2)) {
            *i += 3;
            return Some(Value::Ratio(a, b));
        }
    }
    errors.push(Error::new(
        span_of(&inicio),
        "`aspect-ratio` quiere `A / B` o `N`",
        "la proporcion de la caja propia, en enteros: con decimales el alto no \
         caeria en pixel entero dos veces igual.",
        "por ejemplo `16 / 9` o `1`.",
    ));
    skip_value(toks, i);
    None
}

/// `outline: Npx solid #RRGGBB`, `none` o `0`. El orden de las tres partes
/// da igual, como en CSS; el estilo solo puede ser `solid`.
pub(super) fn contorno(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>) -> Option<Value> {
    let inicio = *toks.get(*i)?;
    let (mut w, mut c, mut solido) = (None, None, false);
    while let Some(t) = toks.get(*i).copied() {
        if matches!(t.kind, Kind::Semi | Kind::RBrace) {
            break;
        }
        match (t.kind, t.text(src)) {
            (Kind::Ident, b"none") => {
                *i += 1;
                return Some(Value::Outline { w: 0, color: 0 });
            }
            (Kind::Ident, b"solid") => {
                solido = true;
                *i += 1;
            }
            (Kind::Number, _) => w = Some(super::measure(src, toks, i, Prop::Outline, errors)?),
            (Kind::Color, _) => c = Some(color(src, toks, i, Prop::Outline, errors)?),
            _ => {
                errors.push(Error::new(
                    span_of(&t),
                    &format!("`outline` no sabe que es `{}`", String::from_utf8_lossy(t.text(src))),
                    "el contorno de la casa es una linea lisa: `dashed`, `dotted` o \
                     `double` se pintarian con otra pluma que la del navegador.",
                    "`outline: 2px solid #5EF2E6`, o `none`.",
                ));
                skip_value(toks, i);
                return None;
            }
        }
    }
    match (w, c, solido) {
        (Some(0), _, _) => Some(Value::Outline { w: 0, color: 0 }),
        (Some(w), Some(color), true) => Some(Value::Outline { w, color }),
        _ => {
            errors.push(Error::new(
                span_of(&inicio),
                "a `outline` le falta algo: va `Npx solid #RRGGBB`",
                "sin color, el navegador usa el de la letra, que aqui puede no estar \
                 dicho; sin grosor, el `medium` de cada navegador.",
                "las tres partes: `2px solid #5EF2E6`.",
            ));
            None
        }
    }
}
