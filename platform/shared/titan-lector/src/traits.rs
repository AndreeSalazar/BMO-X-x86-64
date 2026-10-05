//! **THE TRAITS OF A MODULE** -- what its BODY does, at a glance: how many
//! values it names, how many of them change, how many times it writes on the
//! console, whom it calls. F1 draws each node by these, so a node is not a
//! box with a name: it is what the module DOES, and it changes shape the
//! moment the file is saved (the owner, 04-10: "cuando el programador cambia,
//! todo el nodo se cambia en tiempo real en la forma que representa").
//!
//! ```text
//!    fn       its functions            the size of the planet
//!    let      values with a name       its moons
//!    mut      values that change       its RINGS, turning (the `mut` element)
//!    changes  `x = ...` lines          how fast the rings turn
//!    writes   `print(...)`             a BEAM to the console: it SENDS
//!    calls    other calls, and         its comets
//!             `ship.f()` (level 9)
//!    ifs      `if` / `else if`, and    a DOUBLE STAR: two ways, one lit
//!             `match` (level 8)
//!    loops    `while` / `for`          a BELT of rocks that goes round
//!    returns  `return`                 its comets come back CARRYING a value
//!    types    `type` / `enum` /        a CRYSTAL: a value with facets (fields,
//!             `trait` (level 10)       the cases of an enum, what a trait can do)
//! ```
//!
//! [!] This is a QUICK READING of the lines, not the compiler: it does not
//! judge, it counts. Whether a program is right is `titan check`'s (and, with
//! T6, the compiler inside F1). It never says a module is good; it says what
//! it looks like.
//!
//! [layer] PURE: no allocator, no `unsafe`; it reads the bytes F1 already
//! fetched for the header, so it costs no extra read.

use crate::text::{lines, trim};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Traits {
    pub fns: u8,
    pub lets: u8,
    pub muts: u8,
    pub changes: u8,
    pub writes: u8,
    pub calls: u8,
    /// The decisions: each `if` and each `else if` (level 3), each `match` (8).
    pub ifs: u8,
    /// What repeats: each `while` and each `for` (level 4).
    pub loops: u8,
    /// What it gives back: each `return` (level 5).
    pub returns: u8,
    /// Its records and enums: each `type` (level 6) and `enum` (level 8).
    pub types: u8,
    /// Lines of body (not blank, not comment, not header).
    pub lines: u16,
}

impl Traits {
    pub const NONE: Traits = Traits { fns: 0, lets: 0, muts: 0, changes: 0, writes: 0, calls: 0, ifs: 0, loops: 0, returns: 0, types: 0, lines: 0 };
}

fn bump(n: &mut u8) {
    *n = n.saturating_add(1);
}

fn is_name_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

/// The name at the start of `s`, if any, and the rest after it.
fn name(s: &[u8]) -> Option<(&[u8], &[u8])> {
    if !s.first().copied().is_some_and(is_name_start) {
        return None;
    }
    let end = s.iter().position(|&c| !(c.is_ascii_alphanumeric() || c == b'_')).unwrap_or(s.len());
    Some((&s[..end], trim(&s[end..])))
}

/// Counts what a `.titan` does. The header (`mod`, `use`, at the margin
/// before the first `fn`) is not body.
pub fn scan(text: &[u8]) -> Traits {
    let mut t = Traits::NONE;
    let mut body = false;
    for (_, raw) in lines(text) {
        let line = trim(raw);
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }
        if !body && (line.starts_with(b"mod ") || line.starts_with(b"use ")) {
            continue;
        }
        body = true;
        t.lines = t.lines.saturating_add(1);
        // `pub fn`, `pub type`, `pub enum` (level 9) count as what they are,
        // and so does a `gpu fn` (level 11): it is a fn of the program.
        let line = line.strip_prefix(b"pub ").unwrap_or(line);
        let line = line.strip_prefix(b"gpu ").unwrap_or(line);
        if line.starts_with(b"fn ") {
            bump(&mut t.fns);
        } else if line.starts_with(b"type ") || line.starts_with(b"enum ") || line.starts_with(b"trait ") {
            bump(&mut t.types);
        } else if line.starts_with(b"if ") || line.starts_with(b"else if ") || line.starts_with(b"match ") {
            bump(&mut t.ifs);
        } else if line.starts_with(b"while ") || line.starts_with(b"for ") {
            bump(&mut t.loops);
        } else if line == b"return" || line.starts_with(b"return ") {
            bump(&mut t.returns);
        } else if line.starts_with(b"let mut ") {
            bump(&mut t.muts);
        } else if line.starts_with(b"let ") {
            bump(&mut t.lets);
        } else if let Some((word, rest)) = name(line) {
            // `ship.avanza()` (level 9): a call into another module.
            let into = rest.strip_prefix(b".").and_then(name).filter(|(_, r)| r.starts_with(b"("));
            if into.is_some() {
                bump(&mut t.calls);
            } else if rest.starts_with(b"(") {
                if word == b"print" {
                    bump(&mut t.writes);
                } else {
                    bump(&mut t.calls);
                }
            } else if rest.starts_with(b"=") && !rest.starts_with(b"==") {
                bump(&mut t.changes);
            }
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_counts_what_the_body_does_and_not_the_header() {
        let t = scan(b"mod main \"x\"\nuse ship\nmod rock\n\n# nada\nfn main()\n    let a = 1\n    let mut n = 0\n    n = n + a\n    print(n)\n    saluda()\nfn saluda()\n    print(\"hola\")\n");
        assert_eq!(t, Traits { fns: 2, lets: 1, muts: 1, changes: 1, writes: 2, calls: 1, ifs: 0, loops: 0, returns: 0, types: 0, lines: 8 });
    }

    #[test]
    fn every_if_and_else_if_is_a_decision_and_a_plain_else_is_not() {
        let t = scan(b"mod a \"x\"\nfn main()\n    let v = 3\n    if v > 5\n        print(\"a\")\n    else if v > 1\n        print(\"b\")\n    else\n        print(\"c\")\n");
        assert_eq!((t.ifs, t.writes, t.lets), (2, 3, 1));
    }

    #[test]
    fn every_while_and_for_is_a_loop() {
        let t = scan(b"mod a \"x\"\nfn main()\n    for i in range(3)\n        let mut n = 0\n        while n < i\n            n = n + 1\n");
        assert_eq!((t.loops, t.muts, t.changes), (2, 1, 1));
    }

    #[test]
    fn every_return_is_counted() {
        let t = scan(b"mod a \"x\"\nfn f(n: int) -> int\n    if n < 1\n        return 1\n    return n * f(n - 1)\n");
        assert_eq!((t.returns, t.calls, t.ifs), (2, 0, 1));
    }

    #[test]
    fn every_type_is_a_crystal() {
        let t = scan(b"mod a \"x\"\ntype Nave\n    x: dec\ntype Roca\n    r: int\nfn main()\n    print(1)\n");
        assert_eq!((t.types, t.fns, t.writes), (2, 1, 1));
    }

    #[test]
    fn a_trait_is_a_crystal_and_its_fn_are_fn() {
        let t = scan(b"mod a \"x\"\ntrait Forma\n    fn area(f: Forma) -> dec\ntrait Forma for Circulo\n    fn area(c: Circulo) -> dec\n        return c.r\n");
        assert_eq!((t.types, t.fns, t.returns), (2, 2, 1));
    }

    #[test]
    fn a_gpu_fn_is_a_fn() {
        let t = scan(b"mod a \"x\"\ngpu fn d(x: f32) -> f32\n    return x\npub gpu fn e(x: f32) -> f32\n    return x\n");
        assert_eq!((t.fns, t.returns), (2, 2));
    }

    #[test]
    fn what_is_pub_counts_as_what_it_is() {
        let t = scan(b"mod a \"x\"\npub type Nave\n    x: dec\npub fn f()\n    print(1)\nfn g()\n    print(2)\n");
        assert_eq!((t.types, t.fns, t.writes), (1, 2, 2));
    }

    #[test]
    fn an_enum_is_a_crystal_and_a_match_a_decision() {
        let t = scan(b"mod a \"x\"\nenum Luz\n    Verde\n    Rojo\nfn main()\n    let l = Verde\n    match l\n        Verde\n            print(1)\n        Rojo\n            print(2)\n");
        assert_eq!((t.types, t.ifs, t.writes), (1, 1, 2));
    }

    #[test]
    fn a_comparison_is_not_a_change_and_a_header_alone_is_empty() {
        assert_eq!(scan(b"mod a \"x\"\nfn main()\n    x == 1\n").changes, 0);
        assert_eq!(scan(b"mod a \"x\"\nuse b\n"), Traits::NONE);
    }

    #[test]
    fn saving_the_file_changes_the_traits() {
        let before = scan(b"mod a \"x\"\nfn main()\n    print(\"a\")\n");
        let after = scan(b"mod a \"x\"\nfn main()\n    let mut n = 0\n    n = n + 1\n    print(n)\n");
        assert_eq!((before.muts, after.muts, after.changes), (0, 1, 1));
    }
}
