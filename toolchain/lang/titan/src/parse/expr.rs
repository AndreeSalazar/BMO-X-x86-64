//! `parse::expr` -- the EXPRESSIONS: from `or`, the weakest, down to a
//! value -- a number, a text, a name, a call, a case, a table, a record.
//!
//! Cut out of `parse.rs` on 05-10 (it had reached 1.230 lines, and the census
//! of modules, L6a, says no new file passes 1.000). Only the place changed:
//! the precedence is still the school's (`not` over `and` over `or`, `*` over
//! `+`), and `round` and the decimals are still checked where they are
//! written.

use super::*;

impl<'a> Parser<'a> {
    /// `a or b`: the weakest binding first.
    pub(super) fn expr(&mut self) -> Result<Expr, Message> {
        let mut left = self.and()?;
        while self.peek().kind == Kind::Word("or") {
            let tok = self.next();
            let right = self.and()?;
            left = Expr::Bin { op: "or", left: Box::new(left), right: Box::new(right), line: tok.line, col: tok.col };
        }
        Ok(left)
    }

    pub(super) fn and(&mut self) -> Result<Expr, Message> {
        let mut left = self.not()?;
        while self.peek().kind == Kind::Word("and") {
            let tok = self.next();
            let right = self.not()?;
            left = Expr::Bin { op: "and", left: Box::new(left), right: Box::new(right), line: tok.line, col: tok.col };
        }
        Ok(left)
    }

    pub(super) fn not(&mut self) -> Result<Expr, Message> {
        if self.peek().kind == Kind::Word("not") && LEVEL_NOW >= 3 {
            let tok = self.next();
            let value = self.not()?;
            return Ok(Expr::Not { value: Box::new(value), line: tok.line, col: tok.col });
        }
        self.compare()
    }

    /// `a < b`, and only ONE: `a < b < c` is a NO that says how.
    pub(super) fn compare(&mut self) -> Result<Expr, Message> {
        let left = self.sum()?;
        let Kind::Sym(op @ ("==" | "!=" | "<" | "<=" | ">" | ">=")) = self.peek().kind else { return Ok(left) };
        if LEVEL_NOW < 3 {
            return Ok(left);
        }
        let tok = self.next();
        let right = self.sum()?;
        if let Kind::Sym(second @ ("==" | "!=" | "<" | "<=" | ">" | ">=")) = self.peek().kind {
            let at = self.next();
            let third = self.sum().map(|e| e.show()).unwrap_or_else(|_| "...".into());
            let (l, r) = (left.show(), right.show());
            return Err(self.expected(
                at,
                "el final de la comparacion",
                &format!("se compara de dos en dos, y se dice cual: {} {} {} and {} {} {}", l, op, r, r, second, third),
            ));
        }
        Ok(Expr::Bin { op, left: Box::new(left), right: Box::new(right), line: tok.line, col: tok.col })
    }

    /// `a + b - c`.
    pub(super) fn sum(&mut self) -> Result<Expr, Message> {
        let mut left = self.term()?;
        while let Kind::Sym(op @ ("+" | "-")) = self.peek().kind {
            let tok = self.next();
            let right = self.term()?;
            left = Expr::Bin { op, left: Box::new(left), right: Box::new(right), line: tok.line, col: tok.col };
        }
        Ok(left)
    }

    pub(super) fn term(&mut self) -> Result<Expr, Message> {
        let mut left = self.unary()?;
        while let Kind::Sym(op @ ("*" | "/" | "%")) = self.peek().kind {
            let tok = self.next();
            let right = self.unary()?;
            left = Expr::Bin { op, left: Box::new(left), right: Box::new(right), line: tok.line, col: tok.col };
        }
        Ok(left)
    }

    pub(super) fn unary(&mut self) -> Result<Expr, Message> {
        if self.peek().kind == Kind::Sym("-") {
            let tok = self.next();
            let value = self.unary()?;
            return Ok(Expr::Neg { value: Box::new(value), line: tok.line, col: tok.col });
        }
        let mut e = self.primary()?;
        // `a[i]`, `nave.x`, `a[i].x` (level 6).
        while LEVEL_NOW >= 6 && matches!(self.peek().kind, Kind::Sym("[") | Kind::Sym(".")) {
            let tok = self.next();
            if tok.kind == Kind::Sym("[") {
                let index = self.expr()?;
                let close = self.next();
                if close.kind != Kind::Sym("]") {
                    return Err(self.expected(close, "`]`", "planetas[0]"));
                }
                e = Expr::Index { base: Box::new(e), index: Box::new(index), line: tok.line, col: tok.col };
            } else {
                let f = self.next();
                let Kind::Name(name) = &f.kind else {
                    return Err(self.expected(f, "el nombre de un campo", "nave.x"));
                };
                e = Expr::Field { base: Box::new(e), name: name.clone(), line: f.line, col: f.col };
            }
        }
        Ok(e)
    }

    pub(super) fn primary(&mut self) -> Result<Expr, Message> {
        let tok = self.next();
        // `ship.avanza(...)` and `ship.Nave { ... }`: into another module
        // (level 9), read as ONE name from here on. A bare `forma.Nada` stays
        // a field here; the package (`paquete.rs`) knows `forma` is a module.
        let joined;
        let kind = match &tok.kind {
            Kind::Name(m) if self.qualified().is_some() && matches!(self.ahead(2).kind, Kind::Sym("(") | Kind::Sym("{")) => {
                let inner = self.qualified().expect("the guard");
                self.next();
                self.next();
                joined = Kind::Name(format!("{}.{}", m, inner));
                &joined
            }
            k => k,
        };
        match kind {
            Kind::Sym("(") => {
                let e = self.expr()?;
                let close = self.next();
                if close.kind != Kind::Sym(")") {
                    return Err(self.ladder(close).unwrap_or_else(|| self.expected(close, "`)`", "(3 + 4) * 2")));
                }
                Ok(e)
            }
            Kind::Number(n) if !n.contains('.') => match n.parse::<i64>() {
                Ok(value) => Ok(Expr::Int { value, line: tok.line, col: tok.col }),
                Err(_) => Err(Message::new(
                    Code::Overflow,
                    tok.line,
                    tok.col,
                    &format!("{} no cabe en un numero", n),
                    "un numero entero de TITAN++ ocupa 64 bits: va de -9223372036854775808 a 9223372036854775807",
                    "usa un numero mas chico; los numeros mas grandes y exactos llegan con los tipos",
                )),
            },
            Kind::Number(n) if LEVEL_NOW >= 6 => self.dec(n, tok),
            Kind::Sym("[") if LEVEL_NOW >= 6 => {
                if self.peek().kind == Kind::Sym("]") {
                    // `[]`: a list with nothing yet (level 13); its type says
                    // what it will hold: let nombres: [text] = []
                    if LEVEL_NOW >= 13 {
                        self.next();
                        return Ok(Expr::Table { items: Vec::new(), line: tok.line, col: tok.col });
                    }
                    let close = self.peek();
                    return Err(self.expected(close, "las celdas de la tabla", "[1, 2, 3]  o  [0; 10]: una tabla vacia no dice de que es"));
                }
                let first = self.expr()?;
                if self.peek().kind == Kind::Sym(";") {
                    self.next();
                    let count = self.next();
                    let n = match &count.kind {
                        Kind::Number(c) if !c.contains('.') => c.parse::<usize>().ok().filter(|&k| k > 0),
                        _ => None,
                    };
                    let Some(n) = n else {
                        return Err(self.expected(count, "cuantas celdas: un numero entero mayor que 0", "[0; 10]"));
                    };
                    let close = self.next();
                    if close.kind != Kind::Sym("]") {
                        return Err(self.expected(close, "`]`", "[0; 10]"));
                    }
                    return Ok(Expr::Repeat { item: Box::new(first), count: n, line: tok.line, col: tok.col });
                }
                let mut items = vec![first];
                loop {
                    let sep = self.next();
                    match sep.kind {
                        Kind::Sym(",") => items.push(self.expr()?),
                        Kind::Sym("]") => break,
                        _ => return Err(self.expected(sep, "`,` o `]`", "[1, 2, 3]")),
                    }
                }
                Ok(Expr::Table { items, line: tok.line, col: tok.col })
            }
            Kind::Name(n) if self.peek().kind == Kind::Sym("{") && LEVEL_NOW >= 6 => {
                // `Nave { x: 1.0, fuel: 12.50 }`
                self.next();
                let mut fields = Vec::new();
                if self.peek().kind != Kind::Sym("}") {
                    loop {
                        let f = self.next();
                        let Kind::Name(fname) = &f.kind else {
                            return Err(self.expected(f, "el nombre de un campo", &format!("{} {{ x: 1.0 }}", n)));
                        };
                        let colon = self.next();
                        if colon.kind != Kind::Sym(":") {
                            return Err(self.expected(colon, "`:` y el valor", &format!("{} {{ {}: 1.0 }}", n, fname)));
                        }
                        fields.push((fname.clone(), self.expr()?));
                        let sep = self.next();
                        match sep.kind {
                            Kind::Sym(",") => continue,
                            Kind::Sym("}") => break,
                            _ => return Err(self.expected(sep, "`,` o `}`", &format!("{} {{ x: 1.0, y: 2.0 }}", n))),
                        }
                    }
                } else {
                    self.next();
                }
                Ok(Expr::Record { name: n.clone(), fields, line: tok.line, col: tok.col })
            }
            // `{"ana": 3, "bo": 5}` or `{}`: a MAP written (level 13).
            Kind::Sym("{") if LEVEL_NOW >= 13 => {
                let mut items = Vec::new();
                if self.peek().kind == Kind::Sym("}") {
                    self.next();
                } else {
                    loop {
                        let key = self.expr()?;
                        let colon = self.next();
                        if colon.kind != Kind::Sym(":") {
                            return Err(self.expected(colon, "`:` y el valor de esa clave", "{\"ana\": 3}"));
                        }
                        items.push((key, self.expr()?));
                        let sep = self.next();
                        match sep.kind {
                            Kind::Sym(",") => continue,
                            Kind::Sym("}") => break,
                            _ => return Err(self.expected(sep, "`,` o `}`", "{\"ana\": 3, \"bo\": 5}")),
                        }
                    }
                }
                Ok(Expr::Map { items, line: tok.line, col: tok.col })
            }
            Kind::Text(t) => Ok(Expr::Text { value: t.clone(), line: tok.line, col: tok.col }),
            Kind::Word(w @ ("true" | "false")) if LEVEL_NOW >= 3 => Ok(Expr::Bool { value: *w == "true", line: tok.line, col: tok.col }),
            Kind::Name(n) if self.peek().kind == Kind::Sym("(") && LEVEL_NOW >= 5 => {
                self.next();
                let mut args = Vec::new();
                if self.peek().kind == Kind::Sym(")") {
                    self.next();
                } else {
                    loop {
                        args.push(self.expr()?);
                        let sep = self.next();
                        match sep.kind {
                            Kind::Sym(",") => continue,
                            Kind::Sym(")") => break,
                            _ => return Err(self.ladder(sep).unwrap_or_else(|| self.expected(sep, "`,` o `)`", &format!("{}(a, b)", n)))),
                        }
                    }
                }
                if n == "round" && LEVEL_NOW >= 7 {
                    return self.round(tok, args);
                }
                Ok(Expr::Call { callee: n.clone(), args, line: tok.line, col: tok.col })
            }
            // `mut t` / `take t` as a value given to a call (level 7).
            Kind::Word(w @ ("mut" | "take")) if LEVEL_NOW >= 7 => {
                let name_tok = self.next();
                let Kind::Name(name) = &name_tok.kind else {
                    return Err(self.expected(name_tok, &format!("el nombre de lo que se {}", if *w == "mut" { "presta" } else { "entrega" }), &format!("ordena({} tabla)", w)));
                };
                let mode = if *w == "mut" { Mode::Mut } else { Mode::Take };
                Ok(Expr::Lend { mode, name: name.clone(), line: tok.line, col: tok.col })
            }
            Kind::Name(n) => Ok(Expr::Name { name: n.clone(), line: tok.line, col: tok.col }),
            _ => Err(self.ladder(tok).unwrap_or_else(|| self.expected(tok, "un valor: un numero, un texto o un nombre", "let area = 3 * 4"))),
        }
    }
}

impl Parser<'_> {
    /// `round(x, 2)`: two values, and the second a whole number of decimals
    /// written right there -- how much is rounded is never calculated.
    pub(super) fn round(&self, tok: &Token, mut args: Vec<Expr>) -> Result<Expr, Message> {
        if args.len() != 2 {
            return Err(Message::new(Code::Args, tok.line, tok.col, &format!("`round` pide 2 valores, y aqui se le dan {}", args.len()), "el numero, y con cuantos decimales queda", "round(total / 3, 2)"));
        }
        let digits = match &args[1] {
            Expr::Int { value, .. } if (0..=18).contains(value) => *value as u32,
            other => {
                let (l, c) = other.at();
                return Err(Message::new(
                    Code::WrongType,
                    l,
                    c,
                    "el segundo valor de `round` va escrito: cuantos decimales, de 0 a 18",
                    "cuanto se redondea se DICE en el texto, no se calcula: quien lee tiene que verlo",
                    "round(total, 2)",
                ));
            }
        };
        let value = args.swap_remove(0);
        Ok(Expr::Round { value: Box::new(value), digits, line: tok.line, col: tok.col })
    }

    /// `12.50` -> 1250 with scale 2: an exact decimal, never a float. A
    /// number without a dot stays an `int`.
    pub(super) fn dec(&self, n: &str, tok: &Token) -> Result<Expr, Message> {
        let too_big = || {
            Message::new(
                Code::Overflow,
                tok.line,
                tok.col,
                &format!("{} no cabe en un numero", n),
                "un `int` o un `dec` de TITAN++ ocupa 64 bits (sus cifras, con los decimales dentro), y desbordar es un error",
                "usa un numero mas chico, o menos decimales",
            )
        };
        let Some((whole, frac)) = n.split_once('.') else {
            return n.parse::<i64>().map(|value| Expr::Int { value, line: tok.line, col: tok.col }).map_err(|_| too_big());
        };
        if whole.is_empty() || frac.is_empty() || frac.contains('.') {
            return Err(self.expected(tok, "un numero: 12 o 12.50", "un decimal lleva cifras a los dos lados de un solo punto"));
        }
        let scale = frac.len() as u32;
        let digits = format!("{}{}", whole, frac).parse::<i64>().map_err(|_| too_big())?;
        if scale > 18 {
            return Err(too_big());
        }
        Ok(Expr::Dec { digits, scale, line: tok.line, col: tok.col })
    }
}
