//! `calc::numero` -- THE ARITHMETIC: ints that never wrap, and the exact
//! decimal (`dec`), as COBOL counts and INTI traps.
//!
//! Cut out of `calc.rs` on 05-10 with `clase.rs` (the census of modules, L6a:
//! no new file passes 1.000 lines). Only the place changed: an overflow is
//! T0060, a division by zero T0061, a division that does not end T0062 --
//! never a wrap, never a cut in silence.

use super::*;

// ===================================================================
//  THE ARITHMETIC: ints, and the exact decimal
// ===================================================================

pub(super) fn pow10(k: u32) -> i128 {
    10i128.pow(k)
}

/// `n` as a decimal of `scale` decimals (13 with 2 is 1300).
pub(super) fn to_dec(n: i64, scale: u32, at: At) -> Result<Const, Message> {
    let d = n as i128 * pow10(scale);
    i64::try_from(d).map(|d| Const::Dec(d, scale)).map_err(|_| overflow(at, &n.to_string()))
}

/// A number with at least `scale` decimals (an int becomes a decimal).
pub(super) fn rescale(c: Const, scale: u32, at: At) -> Result<Const, Message> {
    match c {
        Const::Int(n) => to_dec(n, scale, at),
        Const::Dec(d, s) if s >= scale => Ok(Const::Dec(d, s)),
        Const::Dec(d, s) => {
            let v = d as i128 * pow10(scale - s);
            i64::try_from(v).map(|v| Const::Dec(v, scale)).map_err(|_| overflow(at, &show_dec(d, s)))
        }
        other => Ok(other),
    }
}

/// Both numbers as (digits, scale) with the SAME scale, in 128 bits.
pub(super) fn align(a: &Const, b: &Const) -> (i128, i128, u32) {
    let (da, sa) = parts(a);
    let (db, sb) = parts(b);
    let s = sa.max(sb);
    (da * pow10(s - sa), db * pow10(s - sb), s)
}

pub(super) fn parts(c: &Const) -> (i128, u32) {
    match c {
        Const::Int(n) => (*n as i128, 0),
        Const::Dec(d, s) => (*d as i128, *s),
        _ => unreachable!("classes: a number"),
    }
}

/// Removes trailing zero decimals while the scale is above `keep`.
pub(super) fn trim(mut d: i128, mut s: u32, keep: u32) -> (i128, u32) {
    while s > keep && d % 10 == 0 {
        d /= 10;
        s -= 1;
    }
    (d, s)
}

pub(super) fn dec_result(d: i128, s: u32, at: At, what: &str) -> Result<Const, Message> {
    i64::try_from(d).ok().filter(|_| s <= SCALE).map(|d| Const::Dec(d, s)).ok_or_else(|| overflow(at, what))
}

/// The four operations with at least one `dec`: exact, or a NO.
pub(super) fn decimal(op: &str, a: Const, b: Const, at: At, lenient: bool) -> Result<Const, Message> {
    let what = format!("{} {} {}", a.show(NONE), op, b.show(NONE));
    let ((da, sa), (db, sb)) = (parts(&a), parts(&b));
    match op {
        "+" | "-" => {
            let (x, y, s) = align(&a, &b);
            let r = if op == "+" { x + y } else { x - y };
            dec_result(r, s, at, &what)
        }
        // Multiplying adds the decimals (COBOL's rule): 12.50 * 2 = 25.00.
        "*" => {
            let (r, s) = trim(da * db, sa + sb, sa.max(sb));
            dec_result(r, s, at, &what)
        }
        "/" => {
            if db == 0 {
                return Err(Message::new(Code::DivZero, at.0, at.1, &format!("{} divide entre cero", what), "entre cero no hay numero que valga: ni infinito, ni cero", "comprueba el divisor antes: if d != 0.0"));
            }
            // a / b = (da / 10^sa) / (db / 10^sb) = da * 10^sb / (db * 10^sa).
            // The fewest decimals (at least those of the two) that make it
            // EXACT -- or a NO: 1.0 / 3 never ends, and is not cut in silence.
            //
            // ** By LONG DIVISION, one decimal per turn (05-10): `num * 10^s`
            // went past 128 bits with big numbers and the compiler burst
            // instead of saying T0060 -- a random test of E1 found it. The
            // quotient only grows, so once it no longer fits a `dec` it never
            // will: that is T0060. E1 divides the same way (`h_div`).
            let (num, den) = (da * pow10(sb), db * pow10(sa));
            let neg = (num < 0) != (den < 0);
            let (n, d) = (num.unsigned_abs(), den.unsigned_abs());
            let (mut q, mut r) = (n / d, n % d);
            let signed = |q: u128| if neg { -(q as i128) } else { q as i128 };
            let s0 = sa.max(sb);
            let mut s = 0;
            loop {
                if s >= s0 && r == 0 {
                    return dec_result(signed(q), s, at, &what);
                }
                if s == SCALE {
                    break;
                }
                if q > i64::MAX as u128 + 1 {
                    return Err(overflow(at, &what));
                }
                s += 1;
                let t = r * 10;
                q = q * 10 + t / d;
                r = t % d;
            }
            if lenient {
                // Inside `round`: carried to SCALE decimals (cut, not
                // rounded), and `round` decides where it ends. Cutting at 18
                // never changes a rounding to fewer decimals.
                return dec_result(signed(q), SCALE, at, &what);
            }
            Err(Message::new(
                Code::Inexact,
                at.0,
                at.1,
                &format!("{} no da un decimal exacto", what),
                &format!("sus decimales no acaban en {} cifras (como 1 / 3 = 0.333...): TITAN++ no corta un numero a escondidas", SCALE),
                "si hay que redondear, se ESCRIBE: round(a / b, 2) -- el redondeo de COBOL (ROUNDED), visible",
            ))
        }
        _ => Err(unclassed(at, &a)),
    }
}

/// Two values and an operator: comparisons, arithmetic, texts joined.
pub(super) fn binop(op: &str, a: Const, b: Const, at: At, lenient: bool) -> Result<Const, Message> {
    // ** Two f32 (level 11): IEEE single precision, as the 3060 counts --
    // each operation rounded once, no fused steps.
    if let (Const::F32(x), Const::F32(y)) = (&a, &b) {
        let (x, y) = (f32::from_bits(*x), f32::from_bits(*y));
        return Ok(match op {
            "+" => Const::F32((x + y).to_bits()),
            "-" => Const::F32((x - y).to_bits()),
            "*" => Const::F32((x * y).to_bits()),
            "/" => Const::F32((x / y).to_bits()),
            "==" => Const::Bool(x == y),
            "!=" => Const::Bool(x != y),
            "<" => Const::Bool(x < y),
            "<=" => Const::Bool(x <= y),
            ">" => Const::Bool(x > y),
            ">=" => Const::Bool(x >= y),
            _ => return Err(unclassed(at, &a)),
        });
    }
    let numbers = matches!(a, Const::Int(_) | Const::Dec(..)) && matches!(b, Const::Int(_) | Const::Dec(..));
    let dec = numbers && (matches!(a, Const::Dec(..)) || matches!(b, Const::Dec(..)));
    if numbers && matches!(op, "==" | "!=" | "<" | "<=" | ">" | ">=") {
        let (x, y, _) = align(&a, &b);
        return Ok(Const::Bool(match op {
            "==" => x == y,
            "!=" => x != y,
            "<" => x < y,
            "<=" => x <= y,
            ">" => x > y,
            _ => x >= y,
        }));
    }
    if dec {
        return decimal(op, a, b, at, lenient);
    }
    // Inside `round`, 7 / 2 between ints is the exact 3.5, to be rounded.
    if lenient && op == "/" {
        if let (Const::Int(x), Const::Int(y)) = (&a, &b) {
            if *y != 0 && x.wrapping_rem(*y) != 0 {
                return decimal(op, a, b, at, lenient);
            }
        }
    }
    match (op, &a, &b) {
        ("==", _, _) => Ok(Const::Bool(same(&a, &b))),
        ("!=", _, _) => Ok(Const::Bool(!same(&a, &b))),
        (_, Const::Int(x), Const::Int(y)) => int(op, *x, *y, at),
        ("+", Const::Text(x), Const::Text(y)) => Ok(Const::Text(format!("{}{}", x, y))),
        _ => Err(unclassed(at, &a)),
    }
}

/// Equal VALUES: 12.50 and 12.5 are the same number; tables and records,
/// cell by cell.
pub(super) fn same(a: &Const, b: &Const) -> bool {
    match (a, b) {
        (Const::Int(_) | Const::Dec(..), Const::Int(_) | Const::Dec(..)) => {
            let (x, y, _) = align(a, b);
            x == y
        }
        (Const::Table(x) | Const::List(_, x), Const::Table(y) | Const::List(_, y)) | (Const::Record(_, x), Const::Record(_, y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        // Two maps with the same keys, each leading to the same value -- in
        // any order: {"a": 1, "b": 2} == {"b": 2, "a": 1} (level 13).
        (Const::Map(_, _, x), Const::Map(_, _, y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(j, w)| same(k, j) && same(v, w))),
        // The same case, carrying the same values: Circulo(2.0) == Circulo(2.00).
        (Const::Variant(e, v, x), Const::Variant(f, w, y)) => e == f && v == w && x.iter().zip(y).all(|(p, q)| same(p, q)),
        _ => a == b,
    }
}

pub(super) fn constant(c: &Const, at: At) -> Value {
    match c {
        Const::Int(n) => Value::Int(*n, at),
        Const::Text(t) => Value::Text(t.clone(), at),
        Const::Bool(b) => Value::Bool(*b, at),
        Const::Dec(d, s) => Value::Dec(*d, *s, at),
        other => Value::Text(other.show(NONE), at),
    }
}

/// A kind that the first pass should have stopped: said, never invented.
pub(super) fn unclassed(at: At, a: &Const) -> Message {
    let _ = constant;
    Message::new(Code::Mixed, at.0, at.1, &format!("aqui no cabe {}", a.show(NONE)), "el calculo encontro una clase que la primera pasada no vio", "esto es un fallo del compilador: avisa con este programa")
}

/// T0060 for `byte(x)` (TA1): the int does not fit in a byte.
pub(crate) fn byte_no_cabe(at: At, n: i64) -> Message {
    Message::new(
        Code::Overflow,
        at.0,
        at.1,
        &format!("byte({}) no cabe en un byte", n),
        "un byte va de 0 a 255, y salirse es un error, no una vuelta a empezar",
        "comprueba antes: if x >= 0 and x <= 255  ->  byte(x)",
    )
}

pub(super) fn overflow(at: At, what: &str) -> Message {
    Message::new(
        Code::Overflow,
        at.0,
        at.1,
        &format!("{} no cabe en un numero", what),
        "un numero entero de TITAN++ ocupa 64 bits, y desbordar es un error, no una vuelta a empezar",
        "usa numeros mas chicos: un `int` y las cifras de un `dec` caben en 64 bits",
    )
}

pub(super) fn int(op: &str, x: i64, y: i64, at: At) -> Result<Const, Message> {
    let what = format!("{} {} {}", x, op, y);
    let r = match op {
        "+" => x.checked_add(y),
        "-" => x.checked_sub(y),
        "*" => x.checked_mul(y),
        "/" | "%" if y == 0 => {
            return Err(Message::new(
                Code::DivZero,
                at.0,
                at.1,
                &format!("{} divide entre cero", what),
                "entre cero no hay numero que valga: ni infinito, ni cero",
                "comprueba el divisor antes: if d != 0  (y el otro lado del if no se calcula)",
            ))
        }
        // wrapping_rem: i64::MIN % -1 es 0 (y `%` de Rust revienta ahi)
        "/" if x.wrapping_rem(y) != 0 => {
            return Err(Message::new(
                Code::Inexact,
                at.0,
                at.1,
                &format!("{} no da un numero entero", what),
                &format!("da {} y sobra {}: TITAN++ no redondea a escondidas (el dinero no se redondea solo)", x / y, x % y),
                &format!("el resto es {} % {}; para el resultado con decimales, un `dec`: {}.0 / {} da el exacto", x, y, x, y),
            ))
        }
        "/" => x.checked_div(y),
        "%" => Some(x.wrapping_rem(y)),
        _ => None,
    };
    r.map(Const::Int).ok_or_else(|| overflow(at, &what))
}
