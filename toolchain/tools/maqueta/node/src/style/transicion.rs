//! **La TRANSICION** (P3, 04-10): `transition: <tiempo> [<curva>]
//! [<espera>]`, con las curvas de CSS y sus numeros leidos en milesimas
//! exactas.

use bmo_maqueta_diag::Error;
use bmo_maqueta_lex::{Kind, Token};

use super::skip_value;
use crate::markup::span_of;
use crate::value::{Transicion, Value};

/// Un numero con signo y decimales, en MILESIMAS EXACTAS (`-.55` -> -550,
/// `.3` -> 300). Sin pasar por sesentaicuatroavos: ahi `.3` salia 297.
fn milesimas(t: &[u8]) -> Option<i32> {
    let (neg, t) = match t.split_first() {
        Some((b'-', r)) => (true, r),
        _ => (false, t),
    };
    let (ent, frac) = match t.iter().position(|&c| c == b'.') {
        Some(k) => (&t[..k], &t[k + 1..]),
        None => (t, &b""[..]),
    };
    if !ent.iter().chain(frac).all(u8::is_ascii_digit) {
        return None;
    }
    let mut v: i64 = 0;
    for &c in ent {
        v = v.checked_mul(10)?.checked_add((c - b'0') as i64)?;
    }
    v = v.checked_mul(1000)?;
    let mut paso = 100;
    for &c in frac.iter().take(3) {
        v += (c - b'0') as i64 * paso;
        paso /= 10;
    }
    // La cuarta cifra redondea.
    if frac.get(3).is_some_and(|&c| c >= b'5') {
        v += 1;
    }
    i32::try_from(if neg { -v } else { v }).ok()
}

/// `transition: [all] <tiempo> [<curva>] [<retraso>]` -- UNA para toda la
/// caja. El tiempo en `ms` o `s`; la curva, las de CSS (`ease`, si no se dice,
/// como en CSS) o `cubic-bezier(x1, y1, x2, y2)`, con las `y` libres: por
/// encima de 1 es el REBOTE de los frameworks, y sigue siendo CSS.
pub(super) fn transicion(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>) -> Option<Value> {
    let start = *toks.get(*i)?;
    let mut tiempos: Vec<u32> = Vec::new();
    let mut curva: Option<[i32; 4]> = None;
    let mal = |t: &Token, que: &str, en: &str, errors: &mut Vec<Error>| {
        errors.push(Error::new(span_of(t), que, "una transicion es: cuanto tarda, con que curva, y cuanto espera antes de empezar.", en));
    };
    while let Some(t) = toks.get(*i).copied() {
        match t.kind {
            Kind::Semi | Kind::RBrace => break,
            Kind::Comma => {
                mal(&t, "una sola `transition` por caja", "`transition: 240ms ease-in-out` vale para todo lo que cambie de la caja.", errors);
                skip_value(toks, i);
                return None;
            }
            Kind::Number => {
                let v = milesimas(t.text(src)).filter(|v| *v >= 0);
                *i += 1;
                let u = toks.get(*i).filter(|u| u.kind == Kind::Ident).map(|u| u.text(src).to_vec());
                let ms = match (v, u.as_deref()) {
                    (Some(v), Some(b"ms")) => (v as u64 + 500) / 1000,
                    (Some(v), Some(b"s")) => v as u64,
                    _ => {
                        mal(&t, "un tiempo va en `ms` o en `s`", "por ejemplo `240ms` o `.3s`.", errors);
                        skip_value(toks, i);
                        return None;
                    }
                };
                *i += 1;
                if ms > 10_000 {
                    mal(&t, "una transicion de mas de 10 s", "eso ya no es una transicion: es una animacion, y la lleva Rust o TITAN++.", errors);
                    skip_value(toks, i);
                    return None;
                }
                tiempos.push(ms as u32);
            }
            Kind::Ident => {
                let w = t.text(src);
                *i += 1;
                if w == b"all" {
                    continue;
                }
                if let Some(c) = Transicion::curva_de(w) {
                    curva = Some(c);
                    continue;
                }
                if w == b"cubic-bezier" && toks.get(*i).is_some_and(|p| p.kind == Kind::LParen) {
                    *i += 1;
                    let mut p = [0i32; 4];
                    for (k, slot) in p.iter_mut().enumerate() {
                        if k > 0 {
                            if toks.get(*i).map(|c| c.kind) != Some(Kind::Comma) {
                                break;
                            }
                            *i += 1;
                        }
                        match toks.get(*i).filter(|n| n.kind == Kind::Number).and_then(|n| milesimas(n.text(src))) {
                            Some(v) => *slot = v,
                            None => {
                                mal(&t, "`cubic-bezier` quiere cuatro numeros", "por ejemplo `cubic-bezier(.34, 1.56, .64, 1)`.", errors);
                                skip_value(toks, i);
                                return None;
                            }
                        }
                        *i += 1;
                    }
                    if toks.get(*i).map(|c| c.kind) != Some(Kind::RParen) {
                        mal(&t, "`cubic-bezier` quiere cuatro numeros y su `)`", "por ejemplo `cubic-bezier(.34, 1.56, .64, 1)`.", errors);
                        skip_value(toks, i);
                        return None;
                    }
                    *i += 1;
                    if !(0..=1000).contains(&p[0]) || !(0..=1000).contains(&p[2]) {
                        mal(&t, "en `cubic-bezier` las `x` van de 0 a 1", "las `x` son el TIEMPO, y el tiempo no vuelve atras (CSS dice lo mismo). Las `y` si son libres.", errors);
                        skip_value(toks, i);
                        return None;
                    }
                    curva = Some([p[0], p[1].clamp(-2000, 3000), p[2], p[3].clamp(-2000, 3000)]);
                    continue;
                }
                mal(
                    &t,
                    &format!("`{}` no es parte de una transicion", String::from_utf8_lossy(w)),
                    "curvas: `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out` o `cubic-bezier(...)`. Una transicion es de la caja entera: no se nombran propiedades.",
                    errors,
                );
                skip_value(toks, i);
                return None;
            }
            _ => {
                mal(&t, "esto no va en una transicion", "por ejemplo `240ms ease-in-out`.", errors);
                skip_value(toks, i);
                return None;
            }
        }
    }
    match tiempos.as_slice() {
        [ms] => Some(Value::Transicion(Transicion { ms: *ms, retraso: 0, curva: curva.unwrap_or([250, 100, 250, 1000]) })),
        [ms, r] => Some(Value::Transicion(Transicion { ms: *ms, retraso: *r, curva: curva.unwrap_or([250, 100, 250, 1000]) })),
        _ => {
            mal(&start, "una transicion dice cuanto tarda (y, si quiere, cuanto espera)", "`240ms ease-in-out`, o `240ms ease-in-out 80ms` con espera.", errors);
            None
        }
    }
}

