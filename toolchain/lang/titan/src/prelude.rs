//! `prelude` -- what the library brings with it: the case `numero(t)` gives
//! (`docs/plan/PLAN_LA_ENTRADA.md`, R6; D3 of the owner, 05-10).
//!
//! ```text
//!    numero("42")    Es(42)     the text IS a whole number
//!    numero("hola")  NoEs       it is not -- and the `match` has to say
//!                               what then: no null, no exception, no 0
//!                               invented (like `pago`, level 8)
//! ```
//!
//! The enum `Numero` is added to a program only when it calls `numero`, so a
//! program that does not use it may name its own `Numero`, `Es` or `NoEs`.
//! One that does use it and also names them is a NO (T0055): two things with
//! one name.
//!
//! What `numero` accepts, the same when compiling (`calc.rs`) and when
//! running (`emisor-x86_64`, E1): the text without the blanks around it, an
//! optional `+` or `-`, and one digit or more -- that fit in 64 bits.
//!
//! ** And `enum Opcion` (level 13, `docs/plan/PLAN_LISTAS_Y_MAPAS.md`, D3):
//! what `get(m, k)` and `pop(mut l)` give -- `Hay(v)` if there is one,
//! `NoHay` if not. ONE enum for every kind of value: its `Hay` carries the
//! value of the list or the map it came from, so its class is
//! `Opcion[int]`, `Opcion[text]`... (`Class::Opt`), never the enum's own.
//! The `int` written in its case below is only a place: nobody reads it. Only
//! the library makes them; a program that writes `Hay(3)` is told so.

use crate::message::{Code, Message};
use crate::tree::{Case, EnumDef, Program, Ty};

pub const NUMERO: &str = "Numero";
pub const ES: &str = "Es";
pub const NO_ES: &str = "NoEs";
pub const OPCION: &str = "Opcion";
pub const HAY: &str = "Hay";
pub const NO_HAY: &str = "NoHay";

/// Is the enum `name` the prelude's `Opcion` (level 13)?
pub fn is_opcion(name: &str) -> bool {
    name.rsplit('.').next() == Some(OPCION)
}

/// Adds `enum Numero` if the program calls `numero`, and `enum Opcion` if
/// it calls `get` or `pop`, or names `Opcion[T]`.
pub fn add(p: &mut Program) -> Result<(), Message> {
    // The tree, as text, names every call: `callee: "numero"` is one.
    let text = format!("{:?}", p);
    // ... and a program that writes `Hay(3)` or `NoHay` without its own enum
    // of them: so it hears that only the library makes them (T0079), not
    // that they do not exist
    let written = (text.contains("callee: \"Hay\"") || text.contains("name: \"NoHay\"")) && p.case(HAY).is_none() && p.case(NO_HAY).is_none();
    if text.contains("callee: \"get\"") || text.contains("callee: \"pop\"") || text.contains("Opt(") || written {
        add_opcion(p)?;
    }
    if !text.contains("callee: \"numero\"") {
        return Ok(());
    }
    let taken = p.enums.iter().any(|e| e.name == NUMERO) || p.types.iter().any(|t| t.name == NUMERO) || p.case(ES).is_some() || p.case(NO_ES).is_some();
    if taken {
        return Err(Message::new(
            Code::Taken,
            1,
            1,
            "este programa usa `numero(...)` y tambien nombra `Numero`, `Es` o `NoEs`",
            "esos tres nombres son los del caso que trae `numero`: dos cosas con un nombre no se distinguen",
            "cambia el nombre de tu `enum` o de sus casos",
        ));
    }
    let case = |name: &str, fields: Vec<Ty>| Case { name: name.into(), fields, line: 0, col: 0 };
    p.enums.push(EnumDef { name: NUMERO.into(), public: true, line: 0, col: 0, cases: vec![case(ES, vec![Ty::Int]), case(NO_ES, Vec::new())] });
    Ok(())
}

fn add_opcion(p: &mut Program) -> Result<(), Message> {
    let taken = p.enums.iter().any(|e| e.name == OPCION) || p.types.iter().any(|t| t.name == OPCION) || p.case(HAY).is_some() || p.case(NO_HAY).is_some();
    if taken {
        return Err(Message::new(
            Code::Taken,
            1,
            1,
            "este programa usa `get` o `pop` y tambien nombra `Opcion`, `Hay` o `NoHay`",
            "esos tres nombres son los del caso que dan `get` y `pop`: dos cosas con un nombre no se distinguen",
            "cambia el nombre de tu `enum` o de sus casos",
        ));
    }
    let case = |name: &str, fields: Vec<Ty>| Case { name: name.into(), fields, line: 0, col: 0 };
    p.enums.push(EnumDef { name: OPCION.into(), public: true, line: 0, col: 0, cases: vec![case(HAY, vec![Ty::Int]), case(NO_HAY, Vec::new())] });
    Ok(())
}

/// What `numero(t)` reads in `t`: the whole number, if it is one.
pub fn parse(t: &str) -> Option<i64> {
    let t = t.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\r' || c == '\n');
    let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn numero_reads_a_whole_number_and_nothing_else() {
        assert_eq!(parse("42"), Some(42));
        assert_eq!(parse("  -7 "), Some(-7));
        assert_eq!(parse("+3"), Some(3));
        assert_eq!(parse("-9223372036854775808"), Some(i64::MIN));
        for no in ["", " ", "-", "+", "4 2", "12.5", "doce", "9223372036854775808", "1e3", "--1", "0x10"] {
            assert_eq!(parse(no), None, "{no:?}");
        }
    }
}
