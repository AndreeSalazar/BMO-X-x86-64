//! **Tokens -> a list of named rules.** Answers: *what does each rule say?*
//!
//! A `Rule` does not know that other rules exist, exactly as `Fila` did not know
//! it had sisters. Which rule beats which is a **relation between two rules**,
//! so it belongs one generation up, in `cascade/`. The same goes for the
//! ordering guardian of `LA_MAQUETA_EXIGE.md` section 5 (tag rules before class
//! rules, so that "last wins" and "most specific wins" agree and the browser
//! preview cannot lie): comparing two rules is not this generation's job.

use crate::value::{self, Keyword, Prop, Shape, Value};
use crate::{Decl, Rule, Selector};
use bmo_maqueta_diag::Error;
use bmo_maqueta_lex::{Kind, Token};

use crate::markup::span_of;

// Los valores que se leen aparte (04-10): los ATAJOS que se expanden en
// largas, y la TRANSICION de un estado a otro. Hijos de este modulo: ven lo
// privado de aqui (`color`, `measure`, `skip_value`...) sin abrirlo a nadie.
mod atajos;
mod transicion;

use atajos::Atajo;
use transicion::transicion;

pub fn parse(src: &[u8], toks: &[Token], errors: &mut Vec<Error>) -> Vec<Rule> {
    let mut rules = Vec::new();
    let mut i = 0usize;
    while i < toks.len() {
        if toks[i].kind == Kind::At {
            estado(src, toks, &mut i, errors, &mut rules);
            continue;
        }
        match rule(src, toks, &mut i, errors) {
            Some(r) => rules.push(r),
            None => {
                // Nothing salvageable here: skip past the next `}` so a single
                // broken rule does not turn into an error on every line after it.
                while i < toks.len() && toks[i].kind != Kind::RBrace {
                    i += 1;
                }
                i += 1;
            }
        }
    }
    rules
}

fn rule(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>) -> Option<Rule> {
    let start = *toks.get(*i)?;
    let mut selectors = Vec::new();
    let mut hovers = Vec::new();

    loop {
        let (sel, hov) = selector(src, toks, i, errors)?;
        selectors.push(sel);
        hovers.push(hov);
        match toks.get(*i).map(|t| t.kind) {
            Some(Kind::Comma) => {
                *i += 1;
            }
            Some(Kind::LBrace) => {
                *i += 1;
                break;
            }
            Some(Kind::Ident) | Some(Kind::Dot) => {
                errors.push(Error::new(
                    span_of(&toks[*i]),
                    "los selectores de descendencia no existen",
                    "`.panel .boton` obliga a una caja a conocer a sus ancestros, y en \
                     MAQUETA una pieza no sabe que tiene padre (L7). Es la misma razon \
                     por la que no hay herencia.",
                    "poner la clase directamente en la caja que se quiere estilar, o \
                     separar con `,` si lo que se queria era estilar las dos.",
                ));
                return None;
            }
            other => {
                errors.push(Error::new(
                    toks.get(*i).map(span_of).unwrap_or(span_of(&start)),
                    "falta el `{` de la regla",
                    "despues del selector va el bloque de declaraciones.",
                    &format!(
                        "escribir `{{ ... }}`{}",
                        match other {
                            Some(k) => format!(" en vez de {}", k.name()),
                            None => String::new(),
                        }
                    ),
                ));
                return None;
            }
        }
    }

    let mut decls = Vec::new();
    loop {
        match toks.get(*i).map(|t| t.kind) {
            Some(Kind::RBrace) => {
                *i += 1;
                break;
            }
            Some(Kind::Semi) => {
                *i += 1;
            }
            None => {
                errors.push(Error::new(
                    span_of(&start),
                    "la regla se abrio y no se cerro",
                    "falta el `}`.",
                    "cerrar el bloque.",
                ));
                break;
            }
            _ => declaration(src, toks, i, errors, &mut decls),
        }
    }

    let hover = hovers.first().copied().unwrap_or(false);
    if hovers.iter().any(|h| *h != hover) {
        errors.push(Error::new(
            span_of(&start),
            "esta regla mezcla selectores con `:hover` y sin el",
            "asi la regla querria decir dos cosas a la vez, y habria que explicar cual manda. Una regla es entera de reposo o entera de encima.",
            "partirla en dos reglas.",
        ));
    }
    if hover {
        for d in &decls {
            if !d.prop.es_pintura() {
                errors.push(Error::new(
                    d.span,
                    &format!("`{}` no puede ir en una regla `:hover`", d.prop.name()),
                    "una regla de `:hover` solo puede cambiar como se VE una caja, nunca donde esta. Si pudiera mover algo, la maquetacion habria que recalcularla por cada estado y dentro del aparato -- y entonces esto deja de ser un compilador y pasa a ser un motor en ejecucion.",
                    "solo `background-color`, `color`, `border-color` y `border-radius`.",
                ));
            }
        }
    }

    Some(Rule {
        selectors,
        decls,
        span: span_of(&start),
        hover,
        estado: None,
    })
}

/// **`@estado abierta { .panel { ... } }`** (04-10): las reglas de un
/// ESTADO. Cada estado se maqueta entero en el anfitrion y se juzga; pasar de
/// uno a otro es interpolar cajas ya calculadas (`transition`). Un navegador
/// no conoce `@estado` y se salta el bloque: lo que ve es el reposo, y
/// `foto.js --estado` abre el bloque para ver los demas.
fn estado(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>, rules: &mut Vec<Rule>) {
    let at = toks[*i];
    *i += 1;
    let palabra = toks.get(*i).filter(|t| t.kind == Kind::Ident).map(|t| t.text(src).to_vec());
    let nombre = toks.get(*i + 1).filter(|t| t.kind == Kind::Ident).map(|t| t.text(src).to_vec());
    let abre = toks.get(*i + 2).is_some_and(|t| t.kind == Kind::LBrace);
    let (Some(b"estado"), Some(nombre), true) = (palabra.as_deref(), nombre, abre) else {
        errors.push(Error::new(
            span_of(&at),
            "la unica regla con `@` es `@estado nombre { ... }`",
            "`@media`, `@keyframes` y las demas son de un navegador que cambia en \
             ejecucion. Aqui cada ESTADO se maqueta y se juzga al compilar.",
            "`@estado abierta { .panel { width: 320px } }`.",
        ));
        // Saltar el bloque entero, con lo que lleve dentro.
        let mut nivel = 0;
        while let Some(t) = toks.get(*i) {
            *i += 1;
            match t.kind {
                Kind::LBrace => nivel += 1,
                Kind::RBrace => {
                    nivel -= 1;
                    if nivel <= 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        return;
    };
    if nombre == b"reposo" {
        errors.push(Error::new(
            span_of(&at),
            "`reposo` es el estado de partida y no se declara",
            "las reglas de fuera de todo `@estado` SON el reposo; un bloque con ese \
             nombre diria lo mismo dos veces.",
            "sacar las reglas del bloque.",
        ));
    }
    let nombre = String::from_utf8_lossy(&nombre).into_owned();
    *i += 3;
    loop {
        match toks.get(*i).map(|t| t.kind) {
            Some(Kind::RBrace) => {
                *i += 1;
                return;
            }
            None => {
                errors.push(Error::new(span_of(&at), "el `@estado` se abrio y no se cerro", "falta su `}`.", "cerrar el bloque."));
                return;
            }
            Some(Kind::At) => {
                errors.push(Error::new(
                    span_of(&toks[*i]),
                    "un `@estado` no va dentro de otro",
                    "un estado es una foto de la maqueta entera; uno dentro de otro no \
                     tiene foto.",
                    "ponerlos uno detras de otro.",
                ));
                return;
            }
            _ => match rule(src, toks, i, errors) {
                Some(mut r) => {
                    r.estado = Some(nombre.clone());
                    rules.push(r);
                }
                None => {
                    while *i < toks.len() && toks[*i].kind != Kind::RBrace {
                        *i += 1;
                    }
                    *i += 1;
                }
            },
        }
    }
}

/// `:hover`, y nada mas. Devuelve si lo habia.
///
/// * Es la UNICA pseudo-clase, y esta admitida por una razon estructural, no por
/// utilidad: no puede tocar la maquetacion (ver el filtro de arriba), asi que la
/// maquetacion se sigue calculando UNA vez y `layout/` no llega a enterarse de
/// que el hover existe.
fn pseudo(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>) -> bool {
    if toks.get(*i).map(|t| t.kind) != Some(Kind::Colon) {
        return false;
    }
    let colon = toks[*i];
    let n = match toks.get(*i + 1) {
        Some(t) if t.kind == Kind::Ident => *t,
        _ => {
            errors.push(Error::new(
                span_of(&colon),
                "falta el nombre de la pseudo-clase",
                "solo existe una: `:hover`.",
                "por ejemplo `.tecla:hover`.",
            ));
            *i += 1;
            return false;
        }
    };
    *i += 2;
    if n.text(src) == b"hover" {
        return true;
    }
    errors.push(Error::new(
        span_of(&n),
        &format!(
            "pseudo-clase no soportada -- `:{}`",
            String::from_utf8_lossy(n.text(src))
        ),
        "la unica es `:hover`, y esta porque no puede mover nada. `:active`, `:focus` y las demas piden un estado que MAQUETA no lleva: quien lleva el estado es Rust.",
        "`:hover`, o dejarlo en manos del codigo.",
    ));
    false
}

fn selector(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    errors: &mut Vec<Error>,
) -> Option<(Selector, bool)> {
    let t = *toks.get(*i)?;
    match t.kind {
        Kind::Dot => {
            *i += 1;
            let n = toks.get(*i)?;
            if n.kind != Kind::Ident {
                errors.push(Error::new(
                    span_of(n),
                    "falta el nombre de la clase despues del `.`",
                    "un selector de clase es un punto y un nombre.",
                    "por ejemplo `.tecla`.",
                ));
                return None;
            }
            *i += 1;
            let clase = Selector::Class(String::from_utf8_lossy(n.text(src)).into_owned());
            Some((clase, pseudo(src, toks, i, errors)))
        }
        Kind::Ident => {
            let raw = t.text(src).to_vec();
            *i += 1;
            match value::Tag::from_name(&raw) {
                Some(tag) => Some((Selector::Tag(tag), pseudo(src, toks, i, errors))),
                None => {
                    errors.push(value::unknown_tag(span_of(&t), &raw));
                    None
                }
            }
        }
        Kind::Hash => {
            errors.push(Error::new(
                span_of(&t),
                "los selectores de id no existen",
                "`id` es la clave de la tabla de golpeo -- por donde se sabe que \
                 boton se pulso. Si ademas estilara, un id valdria para dos cosas y \
                 cambiar una romperia la otra.",
                "usar una clase: `.tecla`.",
            ));
            *i += 1;
            None
        }
        _ => {
            errors.push(Error::new(
                span_of(&t),
                &format!("un selector no puede empezar por {}", t.kind.name()),
                "solo hay dos formas de selector: `etiqueta` y `.clase`.",
                "por ejemplo `div` o `.tecla`.",
            ));
            *i += 1;
            None
        }
    }
}

fn declaration(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>, decls: &mut Vec<Decl>) {
    let Some(name_tok) = toks.get(*i).copied() else { return };
    if name_tok.kind != Kind::Ident {
        errors.push(Error::new(
            span_of(&name_tok),
            &format!("aqui esperaba el nombre de una propiedad, y hay {}", name_tok.kind.name()),
            "dentro de un bloque van declaraciones `nombre: valor`.",
            "revisar si falta un `;` en la linea anterior.",
        ));
        *i += 1;
        return;
    }
    let raw = name_tok.text(src).to_vec();
    *i += 1;

    // ** LOS ATAJOS (escalon 1, 04-10): `background`, `border` y
    // `border-<lado>` no son propiedades -- se EXPANDEN aqui en las largas,
    // como hace un navegador. La cascada solo ve las largas, y por eso "gana
    // la ultima" sigue siendo verdad: un atajo escrito despues pisa lo que
    // pisaria en CSS, ni mas ni menos.
    if let Some(atajo) = Atajo::de(&raw) {
        if toks.get(*i).map(|t| t.kind) != Some(Kind::Colon) {
            errors.push(Error::new(
                span_of(&name_tok),
                &format!("falta el `:` despues de `{}`", String::from_utf8_lossy(&raw)),
                "una declaracion es `nombre: valor`.",
                "agregar los dos puntos.",
            ));
            skip_value(toks, i);
            return;
        }
        *i += 1;
        let span = span_of(&name_tok);
        for (prop, value) in atajo.leer(src, toks, i, span, errors) {
            decls.push(Decl { prop, value, span });
        }
        return;
    }

    let prop = match Prop::from_name(&raw) {
        Some(p) => p,
        None => {
            errors.push(value::unknown_prop(span_of(&name_tok), &raw));
            skip_value(toks, i);
            return;
        }
    };

    if toks.get(*i).map(|t| t.kind) != Some(Kind::Colon) {
        errors.push(Error::new(
            span_of(&name_tok),
            &format!("falta el `:` despues de `{}`", prop.name()),
            "una declaracion es `nombre: valor`.",
            "agregar los dos puntos.",
        ));
        skip_value(toks, i);
        return;
    }
    *i += 1;

    if let Some(v) = read_value(src, toks, i, prop, errors) {
        decls.push(Decl { prop, value: v, span: span_of(&name_tok) });
    }
}

/// `a`, `a b`, `a b c`, `a b c d` -> top, right, bottom, left (CSS).
fn cuatro(v: &[u32]) -> [u32; 4] {
    match *v {
        [a] => [a; 4],
        [a, b] => [a, b, a, b],
        [a, b, c] => [a, b, c, b],
        [a, b, c, d] => [a, b, c, d],
        _ => [0; 4],
    }
}

fn read_value(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    prop: Prop,
    errors: &mut Vec<Error>,
) -> Option<Value> {
    // `inherit` and friends are VALUES, not properties, so the rejection table
    // keyed by property name would never have seen them. Catching them here is
    // worth the special case: they are the exact words someone reaches for when
    // they hit the missing feature, and a generic "quiere un color" would send
    // them looking in the wrong place.
    if let Some(t) = toks.get(*i) {
        if t.kind == Kind::Ident {
            let w = t.text(src);
            if w == b"inherit" || w == b"initial" || w == b"unset" {
                *i += 1;
                errors.push(Error::new(
                    span_of(t),
                    &format!("`{}` no existe", String::from_utf8_lossy(w)),
                    "no hay herencia: en MAQUETA una pieza no sabe que tiene padre \
                     (L7), asi que no hay nada de donde heredar ni a donde volver.",
                    "declarar el valor donde hace falta. Con estilos de ambito corto, \
                     repetirlo cuesta menos que la regla que lo evitaba.",
                ));
                skip_value(toks, i);
                return None;
            }
        }
    }
    match prop.shape() {
        // `border-radius: 50%` (H6): el circulo, que la caja resuelve al saber
        // su medida. Ningun otro porcentaje: ver `measure`.
        Shape::OnePx
            if prop == Prop::BorderRadius
                && toks.get(*i).is_some_and(|t| t.kind == Kind::Number && t.text(src) == b"50")
                && toks.get(*i + 1).is_some_and(|t| t.kind == Kind::Pct) =>
        {
            *i += 2;
            Some(Value::Word(Keyword::Mitad))
        }
        Shape::OnePx => measure(src, toks, i, prop, errors).map(Value::Px),
        Shape::OneToFourPx => {
            let mut v = Vec::with_capacity(4);
            v.push(measure(src, toks, i, prop, errors)?);
            while v.len() < 4 && at_value_start(toks, *i) {
                v.push(measure(src, toks, i, prop, errors)?);
            }
            if at_value_start(toks, *i) {
                errors.push(Error::new(
                    span_of(&toks[*i]),
                    &format!("`{}` acepta de uno a cuatro valores, no mas", prop.name()),
                    "uno son los cuatro lados; dos, arriba-abajo e izquierda-derecha; tres, arriba, los lados y abajo; cuatro, arriba, derecha, abajo e izquierda -- como en CSS.",
                    "quitar lo que sobra.",
                ));
                skip_value(toks, i);
            }
            Some(Value::Px4(cuatro(&v)))
        }
        Shape::Color => color(src, toks, i, prop, errors).map(Value::Color),
        Shape::ColorOrClear => {
            let t = *toks.get(*i)?;
            if t.kind == Kind::Ident && t.text(src) == b"transparent" {
                *i += 1;
                return Some(Value::Nothing);
            }
            color(src, toks, i, prop, errors).map(Value::Color)
        }
        Shape::Weight => {
            let t = *toks.get(*i)?;
            let n = match t.kind {
                Kind::Number => decimal(t.text(src)).map(|v| (v / 64) as u16),
                Kind::Ident if t.text(src) == b"normal" => Some(400),
                Kind::Ident if t.text(src) == b"bold" => Some(700),
                _ => None,
            };
            *i += 1;
            match n {
                Some(w @ (400 | 500 | 600 | 700)) => Some(Value::Weight(w)),
                _ => {
                    errors.push(Error::new(
                        span_of(&t),
                        "`font-weight` quiere 400, 500, 600 o 700",
                        "la letra de la casa tiene esos cuatro pesos de pluma; un peso \
                         que no existe se pintaria con otro y la maqueta mentiria.",
                        "`400` (normal), `500` (los nombres), `600` o `700` (negrita).",
                    ));
                    skip_value(toks, i);
                    None
                }
            }
        }
        Shape::Em => {
            let t = *toks.get(*i)?;
            if t.kind == Kind::Number {
                let v = decimal(t.text(src));
                *i += 1;
                let unidad = toks.get(*i).filter(|u| u.kind == Kind::Ident).map(|u| u.text(src).to_vec());
                match (v, unidad.as_deref()) {
                    (Some(v), Some(b"em")) => {
                        *i += 1;
                        return Some(Value::Em((v * 1000 / 64) as i32));
                    }
                    (Some(0), _) => return Some(Value::Em(0)),
                    _ => {}
                }
            }
            errors.push(Error::new(
                span_of(&t),
                &format!("`{}` quiere una medida en `em`", prop.name()),
                "el espacio entre letras crece con la letra: `.14em` de una de 11 px \
                 no es lo mismo que de una de 40.",
                "por ejemplo `.14em`, o `0`.",
            ));
            skip_value(toks, i);
            None
        }
        Shape::Shadow => shadow(src, toks, i, errors),
        Shape::Transition => transicion(src, toks, i, errors),
        Shape::Gradient => {
            if toks.get(*i).is_some_and(|t| t.kind == Kind::Ident && t.text(src) == b"none") {
                *i += 1;
                return Some(Value::Nothing);
            }
            gradient(src, toks, i, errors)
        }
        Shape::ColorOrNone => {
            let t = *toks.get(*i)?;
            if t.kind == Kind::Ident && t.text(src) == b"none" {
                *i += 1;
                return Some(Value::Nothing);
            }
            color(src, toks, i, prop, errors).map(Value::Color)
        }
        Shape::Fine => {
            let t = *toks.get(*i)?;
            let v = if t.kind == Kind::Number { decimal(t.text(src)) } else { None };
            *i += 1;
            if toks.get(*i).is_some_and(|u| u.kind == Kind::Ident && u.text(src) == b"px") {
                *i += 1;
            }
            match v {
                Some(v) => Some(Value::Fine(v)),
                None => {
                    errors.push(Error::new(
                        span_of(&t),
                        &format!("`{}` quiere un numero", prop.name()),
                        "el grosor del trazo, en unidades del `viewBox` (como en SVG).",
                        "por ejemplo `2` o `1.5`.",
                    ));
                    skip_value(toks, i);
                    None
                }
            }
        }
        Shape::Words(allowed) => {
            let t = *toks.get(*i)?;
            if t.kind == Kind::Ident {
                let raw = t.text(src).to_vec();
                if let Some(k) = Keyword::from_name(&raw) {
                    if allowed.contains(&k) {
                        *i += 1;
                        return Some(Value::Word(k));
                    }
                }
                *i += 1;
                errors.push(Error::new(
                    span_of(&t),
                    &format!(
                        "`{}` no es un valor de `{}`",
                        String::from_utf8_lossy(&raw),
                        prop.name()
                    ),
                    "cada propiedad tiene su lista cerrada de palabras.",
                    &format!("aqui van: {}.", list(allowed)),
                ));
                skip_value(toks, i);
                return None;
            }
            errors.push(Error::new(
                span_of(&t),
                &format!("`{}` quiere una palabra", prop.name()),
                "esta propiedad no lleva numero.",
                &format!("aqui van: {}.", list(allowed)),
            ));
            skip_value(toks, i);
            None
        }
    }
}

/// `#RRGGBB` (a solid colour). An eight-digit colour is only for shadows.
fn color(src: &[u8], toks: &[Token], i: &mut usize, prop: Prop, errors: &mut Vec<Error>) -> Option<u32> {
    let t = *toks.get(*i)?;
    if t.kind == Kind::Color && t.text(src).len() == 6 {
        *i += 1;
        return Some(hex(t.text(src)));
    }
    errors.push(Error::new(
        span_of(&t),
        &format!("`{}` quiere un color `#RRGGBB`", prop.name()),
        "no hay nombres de color, ni `rgb()`, ni `rgba()`: un color solido es \
         `0x00RRGGBB`. La transparencia (`#RRGGBBAA`) es solo de `box-shadow`.",
        "por ejemplo `#182434`. La paleta del sistema esta en \
         `toolchain/tools/maqueta/tema/tema.maqueta`.",
    ));
    skip_value(toks, i);
    None
}

fn hex(h: &[u8]) -> u32 {
    h.iter().fold(0u32, |n, &c| (n << 4) | (c as char).to_digit(16).unwrap_or(0))
}

/// A number with an optional decimal part, in 1/64 units (`1.5` -> 96).
fn decimal(t: &[u8]) -> Option<u32> {
    if t.first() == Some(&b'-') {
        return None;
    }
    let (ent, frac) = match t.iter().position(|&c| c == b'.') {
        Some(k) => (&t[..k], &t[k + 1..]),
        None => (t, &b""[..]),
    };
    let mut e: u64 = 0;
    for &c in ent {
        e = e.checked_mul(10)?.checked_add((c - b'0') as u64)?;
    }
    let (mut f, mut d) = (0u64, 1u64);
    for &c in frac.iter().take(6) {
        f = f * 10 + (c - b'0') as u64;
        d *= 10;
    }
    u32::try_from(e * 64 + (f * 64 + d / 2) / d).ok()
}

/// `box-shadow: 0 0 14px #5EF2E659` -- only the glow, no offset.
fn shadow(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>) -> Option<Value> {
    let start = *toks.get(*i)?;
    let mal = |errors: &mut Vec<Error>, t: &Token| {
        errors.push(Error::new(
            span_of(t),
            "`box-shadow` quiere `0 0 Npx #RRGGBBAA`",
            "solo el RESPLANDOR alrededor de la caja: sin desplazamiento (una sombra \
             corrida promete una luz que el escritorio no tiene) y un color con su \
             fuerza en las dos ultimas cifras.",
            "por ejemplo `0 0 14px #FFD45E59` (el oro al 35 %).",
        ));
    };
    for _ in 0..2 {
        match toks.get(*i) {
            Some(t) if t.kind == Kind::Number && t.text(src) == b"0" => *i += 1,
            _ => {
                mal(errors, &start);
                skip_value(toks, i);
                return None;
            }
        }
    }
    let reach = measure(src, toks, i, Prop::BoxShadow, errors)?;
    match toks.get(*i) {
        Some(t) if t.kind == Kind::Color => {
            *i += 1;
            let h = t.text(src);
            let argb = if h.len() == 8 { (hex(&h[6..]) << 24) | hex(&h[..6]) } else { 0xFF00_0000 | hex(h) };
            Some(Value::Shadow { reach, argb })
        }
        _ => {
            mal(errors, &start);
            skip_value(toks, i);
            None
        }
    }
}

/// `linear-gradient(90deg, #A, #B)` or `180deg`: two colours, one axis.
fn gradient(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>) -> Option<Value> {
    let start = *toks.get(*i)?;
    let mut leer = || -> Option<Value> {
        let f = toks.get(*i)?;
        if f.kind != Kind::Ident || f.text(src) != b"linear-gradient" {
            return None;
        }
        *i += 1;
        (toks.get(*i)?.kind == Kind::LParen).then_some(())?;
        *i += 1;
        let ang = toks.get(*i)?;
        let vertical = match ang.text(src) {
            b"90" => false,
            b"180" => true,
            _ => return None,
        };
        *i += 1;
        (toks.get(*i)?.text(src) == b"deg").then_some(())?;
        *i += 1;
        let mut c = [0u32; 2];
        for k in 0..2 {
            (toks.get(*i)?.kind == Kind::Comma).then_some(())?;
            *i += 1;
            let t = toks.get(*i)?;
            (t.kind == Kind::Color && t.text(src).len() == 6).then_some(())?;
            c[k] = hex(t.text(src));
            *i += 1;
        }
        (toks.get(*i)?.kind == Kind::RParen).then_some(())?;
        *i += 1;
        Some(Value::Gradient { vertical, from: c[0], to: c[1] })
    };
    match leer() {
        Some(v) => Some(v),
        None => {
            errors.push(Error::new(
                span_of(&start),
                "`background-image` quiere `linear-gradient(90deg, #RRGGBB, #RRGGBB)`",
                "un degradado de DOS colores en un eje: `90deg` de izquierda a derecha, \
                 `180deg` de arriba abajo. Mas paradas o angulos sueltos son un motor \
                 de pintura, no una maqueta.",
                "por ejemplo `linear-gradient(90deg, #FFD45E, #080A10)`.",
            ));
            skip_value(toks, i);
            None
        }
    }
}

/// A number followed by `px`. The only unit there is.
fn measure(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    prop: Prop,
    errors: &mut Vec<Error>,
) -> Option<u32> {
    let t = *toks.get(*i)?;
    if t.kind != Kind::Number {
        errors.push(Error::new(
            span_of(&t),
            &format!("`{}` quiere un numero de pixeles", prop.name()),
            "esta propiedad es una medida.",
            "por ejemplo `72px`.",
        ));
        skip_value(toks, i);
        return None;
    }
    let digits = t.text(src);
    if digits.first() == Some(&b'-') {
        errors.push(Error::new(
            span_of(&t),
            &format!("`{}` no puede ser negativo", prop.name()),
            "una medida de caja es una talla o una distancia hacia dentro: en \
             negativo, una caja se meteria encima de su vecina.",
            "un numero de pixeles desde cero.",
        ));
        *i += 1;
        skip_value(toks, i);
        return None;
    }
    if digits.contains(&b'.') {
        errors.push(Error::new(
            span_of(&t),
            &format!("`{}` quiere pixeles ENTEROS", prop.name()),
            "las cajas caen en pixel entero: medio pixel de caja es un borde borroso.",
            "redondear al pixel.",
        ));
        *i += 1;
        skip_value(toks, i);
        return None;
    }
    let mut n: u32 = 0;
    let mut overflow = false;
    for &c in digits {
        match n.checked_mul(10).and_then(|x| x.checked_add((c - b'0') as u32)) {
            Some(v) => n = v,
            None => overflow = true,
        }
    }
    if overflow {
        errors.push(Error::new(
            span_of(&t),
            "el numero no cabe",
            "las medidas son enteros de 32 bits sin signo.",
            "un numero de pixeles razonable para una pantalla.",
        ));
        *i += 1;
        return None;
    }
    *i += 1;

    match toks.get(*i).map(|t| (t.kind, *t)) {
        Some((Kind::Ident, u)) => {
            let unit = u.text(src);
            if unit == b"px" {
                *i += 1;
                Some(n)
            } else {
                *i += 1;
                errors.push(Error::new(
                    span_of(&u),
                    &format!("unidad no soportada -- `{}`", String::from_utf8_lossy(unit)),
                    "solo existe `px`, y en enteros. `em` y `rem` se miden contra una \
                     tipografia que aqui no se puede elegir; `vh` y `vw` contra un \
                     contenedor que una pieza no conoce (L7).",
                    "un numero exacto de pixeles.",
                ));
                None
            }
        }
        Some((Kind::Pct, u)) => {
            *i += 1;
            errors.push(Error::new(
                span_of(&u),
                "unidad no soportada -- `%`",
                "los porcentajes exigen conocer el contenedor, y en MAQUETA una pieza \
                 no sabe que tiene padre (L7). El unico que hay es `border-radius: \
                 50%`, el circulo: ese se mide contra la PROPIA caja.",
                "un pixel exacto, o `display:flex` en el padre repartiendo con `gap`.",
            ));
            None
        }
        _ if n == 0 => Some(0),
        _ => {
            errors.push(Error::new(
                span_of(&t),
                "falta la unidad",
                "las medidas llevan `px` siempre, menos el cero.",
                &format!("`{n}px`."),
            ));
            None
        }
    }
}

fn at_value_start(toks: &[Token], i: usize) -> bool {
    matches!(
        toks.get(i).map(|t| t.kind),
        Some(Kind::Number) | Some(Kind::Color) | Some(Kind::Ident)
    )
}

/// Walk to the end of the current declaration so one bad value does not turn
/// into a complaint about every token after it.
fn skip_value(toks: &[Token], i: &mut usize) {
    while let Some(t) = toks.get(*i) {
        if t.kind == Kind::Semi || t.kind == Kind::RBrace {
            return;
        }
        *i += 1;
    }
}

fn list(ks: &[Keyword]) -> String {
    ks.iter()
        .map(|k| format!("`{}`", k.name()))
        .collect::<Vec<_>>()
        .join(", ")
}
