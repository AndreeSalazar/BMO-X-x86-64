//! `parse` -- the grammar of the levels done (GRAMATICA.md). It does not know
//! if a name exists: that is `check`, and whether it has a value at that line
//! is the checker's (`juez.rs`).
//!
//! ```text
//!    file     :=  header  function*
//!    header   :=  `mod` NAME TEXT NEWLINE
//!    function :=  `fn` NAME `(` [ param { `,` param } ] `)` [ `->` type ] block
//!    param    :=  NAME `:` type                                     (level 5)
//!    type     :=  `int` | `text` | `bool`     (f32, dec, tables: level 6)
//!    block    :=  NEWLINE INDENT stmt+ DEDENT
//!    stmt     :=  call | `let` [`mut`] NAME `=` expr | NAME `=` expr   NEWLINE
//!              |  `if` expr block [ `else` ( block | if ) ]          (level 3)
//!              |  `while` expr block                                (level 4)
//!              |  `for` NAME `in` `range` `(` expr [`,` expr] `)` block
//!              |  `break` | `continue`        (inside a loop: T0067)
//!              |  `return` [ expr ]                                 (level 5)
//!    call     :=  NAME `(` [ expr { `,` expr } ] `)`
//!    expr     :=  and { `or` and }                                  (level 3)
//!    and      :=  not { `and` not }
//!    not      :=  `not` not | compare
//!    compare  :=  sum [ (`==` | `!=` | `<` | `<=` | `>` | `>=`) sum ]
//!    sum      :=  term { (`+` | `-`) term }
//!    term     :=  unary { (`*` | `/` | `%`) unary }
//!    unary    :=  `-` unary | NUMBER | TEXT | NAME | `true` | `false` | `(` expr `)`
//!              |  NAME `(` [ expr { `,` expr } ] `)`     a call is a value (5)
//! ```
//!
//! ** One comparison at a time: `a < b < c` is a NO (T0030) and says how to
//! write it (`a < b and b < c`). Python reads it one way, C another; TITAN++
//! makes the author say which. And `not` binds looser than a comparison:
//! `not vidas > 0` is `not (vidas > 0)`, the only reading that makes sense.
//!
//! A NUMBER is a whole number in level 1. With a `.` it is a decimal, and the
//! exact decimals (`dec`, COBOL's) arrive with the types.
//!
//! ** What makes it a LADDER and not a wall: anything that belongs to a level
//! above (one of the 25 words, a number, a `=`, a parameter, a `->`) is not
//! "unexpected" -- it is T0040, and the message says which level brings it.
//! The frontend grows by moving that line, not by rewriting this file.

use crate::lex::{Kind, Token};
use crate::message::{Code, Message};
use crate::tree::{Call, Expr, For, Function, If, Let, Mode, Param, Program, Step, Stmt, Ty, TypeDef, While};
use crate::words::{self, LEVEL_NOW};

struct Parser<'a> {
    t: &'a [Token],
    at: usize,
    /// How many loops enclose the statement being read: `break` and
    /// `continue` need one (T0067).
    loops: usize,
}

/// "`x` llega en el nivel N (what it is)": the NO of the ladder.
fn not_yet(tok: &Token, what: &str, level: u8, how: &str) -> Message {
    Message::new(
        Code::NotYet,
        tok.line,
        tok.col,
        &format!("{} llega en el nivel {} ({})", what, level, words::level_name(level)),
        &format!("este frontend va por el nivel {}: la escalera se sube de uno en uno (TITAN_MAESTRO 14.14)", LEVEL_NOW),
        how,
    )
}

fn show(k: &Kind) -> String {
    match k {
        Kind::Word(w) => format!("la palabra `{}`", w),
        Kind::Name(n) => format!("el nombre `{}`", n),
        Kind::Number(n) => format!("el numero `{}`", n),
        Kind::Text(_) => "un texto".into(),
        Kind::Sym(s) => format!("`{}`", s),
        Kind::Newline => "el final de la linea".into(),
        Kind::Indent => "una sangria".into(),
        Kind::Dedent => "el final del bloque".into(),
        Kind::End => "el final del fichero".into(),
    }
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &'a Token {
        &self.t[self.at.min(self.t.len() - 1)]
    }

    fn next(&mut self) -> &'a Token {
        let tok = self.peek();
        self.at += 1;
        tok
    }

    fn expected(&self, tok: &Token, want: &str, how: &str) -> Message {
        Message::new(
            Code::Expected,
            tok.line,
            tok.col,
            &format!("aqui iba {}", want),
            &format!("hay {}", show(&tok.kind)),
            how,
        )
    }

    /// The NO for a piece of a higher level, or `None` if it is not one.
    fn ladder(&self, tok: &Token) -> Option<Message> {
        match &tok.kind {
            Kind::Word(w) => {
                let level = words::find(w).map(|x| x.level).unwrap_or(0);
                (level > LEVEL_NOW).then(|| not_yet(tok, &format!("`{}`", w), level, "por ahora: llamadas, `let`, `let mut`, `if` / `else`, `while`, `for` y `fn` con `return`"))
            }
            Kind::Number(n) if n.contains('.') && LEVEL_NOW < 6 => Some(not_yet(tok, "un decimal", 6, "por ahora, numeros enteros: los decimales EXACTOS (dec) llegan con los tipos")),
            Kind::Sym("->") if LEVEL_NOW < 5 => Some(not_yet(tok, "una funcion que devuelve algo", 5, "por ahora, `fn nombre()` sin `->`")),
            Kind::Sym("==" | "!=" | "<" | "<=" | ">" | ">=") if LEVEL_NOW < 3 => Some(not_yet(tok, "comparar", 3, "por ahora, se calcula con + - * / %")),
            _ => None,
        }
    }

    fn header(&mut self) -> Result<(String, String), Message> {
        let tok = self.next();
        let no_header = |tok: &Token, why: &str| {
            Message::new(
                Code::NoHeader,
                tok.line,
                tok.col,
                "falta la cabecera del modulo",
                why,
                "empieza el fichero con: mod main \"que hace este programa\"",
            )
        };
        if tok.kind == Kind::Indent {
            return Err(no_header(tok, "la primera linea va sangrada: la cabecera va pegada al margen"));
        }
        if tok.kind != Kind::Word("mod") {
            return Err(no_header(tok, &format!("la primera linea empieza con {}, y un modulo dice primero quien es y que hace (U3)", show(&tok.kind))));
        }
        let name = match &self.next().kind {
            Kind::Name(n) => n.clone(),
            _ => return Err(no_header(tok, "despues de `mod` va el NOMBRE del modulo")),
        };
        let purpose = match &self.next().kind {
            Kind::Text(p) => p.clone(),
            _ => return Err(no_header(tok, &format!("`mod {}` no dice que hace: un modulo lo dice en su primera linea (U3)", name))),
        };
        let end = self.next();
        if end.kind != Kind::Newline {
            return Err(self.expected(end, "el final de la cabecera", "la cabecera es una linea sola: mod nombre \"que hace\""));
        }
        Ok((name, purpose))
    }

    fn function(&mut self) -> Result<Function, Message> {
        let fn_tok = self.next();
        let name_tok = self.next();
        let Kind::Name(name) = &name_tok.kind else {
            return Err(self.expected(name_tok, "el nombre de la funcion", "fn saluda()"));
        };
        let open = self.next();
        if open.kind != Kind::Sym("(") {
            return Err(self.expected(open, "`(`", &format!("fn {}()", name)));
        }
        let mut params: Vec<Param> = Vec::new();
        if self.peek().kind != Kind::Sym(")") {
            loop {
                let mut p = self.next();
                // `mut n: T` / `take n: T` (level 7): lent or given.
                let mode = match p.kind {
                    Kind::Word("mut") if LEVEL_NOW >= 7 => Mode::Mut,
                    Kind::Word("take") if LEVEL_NOW >= 7 => Mode::Take,
                    Kind::Word("mut" | "take") => return Err(not_yet(p, "prestar o entregar un parametro", 7, &format!("por ahora, `{0}(n: int)`: el valor llega y no cambia", name))),
                    _ => Mode::Copy,
                };
                if mode != Mode::Copy {
                    p = self.next();
                }
                let Kind::Name(pname) = &p.kind else {
                    return Err(self.ladder(p).unwrap_or_else(|| self.expected(p, "el nombre de un parametro", &format!("fn {}(n: int)", name))));
                };
                if LEVEL_NOW < 5 {
                    return Err(not_yet(p, "una funcion con parametros", 5, &format!("por ahora, fn {}()", name)));
                }
                let colon = self.next();
                if colon.kind != Kind::Sym(":") {
                    return Err(self.expected(colon, "`:` y el tipo", &format!("fn {}({}: int)", name, pname)));
                }
                let ty = self.ty(&format!("fn {}({}: int)", name, pname))?;
                params.push(Param { name: pname.clone(), ty, mode, line: p.line, col: p.col });
                let sep = self.next();
                match sep.kind {
                    Kind::Sym(",") => continue,
                    Kind::Sym(")") => break,
                    _ => return Err(self.expected(sep, "`,` o `)`", &format!("fn {}(a: int, b: int)", name))),
                }
            }
        } else {
            self.next();
        }
        let mut ret = None;
        if self.peek().kind == Kind::Sym("->") && LEVEL_NOW >= 5 {
            self.next();
            ret = Some(self.ty(&format!("fn {}() -> int", name))?);
        }
        let end = self.peek();
        if end.kind != Kind::Newline {
            return Err(self.ladder(end).unwrap_or_else(|| self.expected(end, "el final de la linea", &format!("fn {}() y el cuerpo debajo, sangrado", name))));
        }
        let _ = fn_tok;
        let body = self.block(fn_tok, &format!("`fn {}()`", name), &format!("fn {}()\n             print(\"hola\")", name))?;
        Ok(Function { name: name.clone(), line: fn_tok.line, col: fn_tok.col, params, ret, body })
    }

    /// A type: `int`, `text`, `bool`, `dec`, `[T; n]` or the name of a
    /// `type` of the file (level 6). `f32` is the 3060's, and says so.
    fn ty(&mut self, example: &str) -> Result<Ty, Message> {
        let t = self.next();
        match &t.kind {
            Kind::Name(n) if n == "int" => Ok(Ty::Int),
            Kind::Name(n) if n == "text" => Ok(Ty::Text),
            Kind::Name(n) if n == "bool" => Ok(Ty::Bool),
            Kind::Name(n) if n == "dec" && LEVEL_NOW >= 7 && self.peek().kind == Kind::Sym("(") => {
                // `dec(7, 2)`: COBOL's PIC 9(5)V99 -- the digits DECLARED.
                self.next();
                let num = |p: &mut Self| -> Option<u32> {
                    let t = p.next();
                    match &t.kind {
                        Kind::Number(c) if !c.contains('.') => c.parse::<u32>().ok(),
                        _ => None,
                    }
                };
                let digits = num(self);
                let comma = self.next().kind == Kind::Sym(",");
                let scale = num(self);
                let close = self.next().kind == Kind::Sym(")");
                match (digits, comma, scale, close) {
                    (Some(d), true, Some(sc), true) if (1..=18).contains(&d) && sc <= d => Ok(Ty::DecP(d, sc)),
                    _ => Err(self.expected(t, "dec(cifras, decimales): de 1 a 18 cifras, y los decimales dentro de ellas", "dec(7, 2): hasta 99999.99")),
                }
            }
            Kind::Name(n) if n == "dec" && LEVEL_NOW >= 6 => Ok(Ty::Dec),
            Kind::Name(n) if n == "f32" || n == "f64" => Err(not_yet(
                t,
                &format!("el tipo `{}`", n),
                11,
                "en la CPU TITAN++ cuenta EXACTO con `dec` (sin float: 0.1 + 0.2 es 0.3); el float es de la 3060, con `gpu fn`",
            )),
            Kind::Sym("[") if LEVEL_NOW >= 6 => {
                let inner = self.ty(example)?;
                let semi = self.next();
                if semi.kind != Kind::Sym(";") {
                    return Err(self.expected(semi, "`;` y cuantas celdas", "[int; 3]"));
                }
                let count = self.next();
                let n = match &count.kind {
                    Kind::Number(c) if !c.contains('.') => c.parse::<usize>().ok().filter(|&k| k > 0),
                    _ => None,
                };
                let Some(n) = n else {
                    return Err(self.expected(count, "cuantas celdas: un numero entero mayor que 0", "[int; 3]"));
                };
                let close = self.next();
                if close.kind != Kind::Sym("]") {
                    return Err(self.expected(close, "`]`", "[int; 3]"));
                }
                Ok(Ty::Table(Box::new(inner), n))
            }
            Kind::Name(n) if LEVEL_NOW >= 6 => Ok(Ty::Named(n.clone())),
            _ => Err(self.expected(t, "un tipo: int, text, bool, dec, [int; 3] o el nombre de un `type`", example)),
        }
    }

    /// `type NAME` and its fields, one per line, sangrados (level 6).
    fn typedef(&mut self) -> Result<TypeDef, Message> {
        let tok = self.next();
        let name_tok = self.next();
        let Kind::Name(name) = &name_tok.kind else {
            return Err(self.expected(name_tok, "el nombre del tipo", "type Nave"));
        };
        let end = self.next();
        if end.kind != Kind::Newline {
            return Err(self.expected(end, "el final de la linea", &format!("type {}\n             x: dec", name)));
        }
        if self.peek().kind != Kind::Indent {
            return Err(Message::new(
                Code::EmptyBody,
                tok.line,
                tok.col,
                &format!("`type {}` no tiene campos", name),
                "debajo de un `type` van sus campos, uno por linea y sangrados: nombre: tipo",
                &format!("type {}\n             x: dec", name),
            ));
        }
        self.next();
        let mut fields = Vec::new();
        while self.peek().kind != Kind::Dedent && self.peek().kind != Kind::End {
            let f = self.next();
            let Kind::Name(fname) = &f.kind else {
                return Err(self.expected(f, "el nombre de un campo", "x: dec"));
            };
            let colon = self.next();
            if colon.kind != Kind::Sym(":") {
                return Err(self.expected(colon, "`:` y el tipo", &format!("{}: dec", fname)));
            }
            let ty = self.ty(&format!("{}: dec", fname))?;
            self.end_of_line()?;
            fields.push(Param { name: fname.clone(), ty, mode: Mode::Copy, line: f.line, col: f.col });
        }
        self.next();
        Ok(TypeDef { name: name.clone(), line: tok.line, col: tok.col, fields })
    }

    /// NEWLINE INDENT stmt+ DEDENT: the body of whatever `opener` opens (a
    /// `fn`, an `if`, an `else`). `what` names it in the NO of an empty one.
    fn block(&mut self, opener: &Token, what: &str, example: &str) -> Result<Vec<Stmt>, Message> {
        let end = self.next();
        if end.kind != Kind::Newline {
            return Err(self.ladder(end).unwrap_or_else(|| self.expected(end, "el final de la linea", "lo de dentro va debajo, sangrado")));
        }
        if self.peek().kind != Kind::Indent {
            return Err(Message::new(
                Code::EmptyBody,
                opener.line,
                opener.col,
                &format!("{} no tiene cuerpo", what),
                "debajo va su cuerpo, sangrado cuatro espacios mas",
                example,
            ));
        }
        self.next();
        let mut body = Vec::new();
        while self.peek().kind != Kind::Dedent && self.peek().kind != Kind::End {
            body.push(self.statement()?);
        }
        self.next();
        Ok(body)
    }

    /// `if COND` and its block, then `else` + block or `else if ...`. The
    /// `if` token is already taken.
    fn if_statement(&mut self, if_tok: &Token) -> Result<Stmt, Message> {
        let cond = self.expr()?;
        if self.peek().kind == Kind::Sym("=") {
            let eq = self.peek();
            return Err(self.expected(eq, "una condicion", "para comparar se escribe `==`: `=` da un valor, `==` pregunta si es igual"));
        }
        let then = self.block(if_tok, "este `if`", "if vidas > 0\n             print(\"sigue\")")?;
        let mut other = Vec::new();
        let mut else_at = (0, 0);
        if self.peek().kind == Kind::Word("else") {
            let else_tok = self.next();
            else_at = (else_tok.line, else_tok.col);
            if self.peek().kind == Kind::Word("if") {
                let tok = self.next();
                other.push(self.if_statement(tok)?);
            } else {
                other = self.block(else_tok, "este `else`", "else\n             print(\"si no\")")?;
            }
        }
        Ok(Stmt::If(If { cond, line: if_tok.line, col: if_tok.col, then, other, else_at }))
    }

    /// `while COND` and its block; the `while` token is already taken.
    fn while_statement(&mut self, tok: &Token) -> Result<Stmt, Message> {
        let cond = self.expr()?;
        if self.peek().kind == Kind::Sym("=") {
            let eq = self.peek();
            return Err(self.expected(eq, "una condicion", "para comparar se escribe `==`: `=` da un valor, `==` pregunta si es igual"));
        }
        self.loops += 1;
        let body = self.block(tok, "este `while`", "while vidas > 0\n             vidas = vidas - 1");
        self.loops -= 1;
        Ok(Stmt::While(While { cond, line: tok.line, col: tok.col, body: body? }))
    }

    /// `for NAME in range(TO)` / `range(FROM, TO)` and its block.
    fn for_statement(&mut self, tok: &Token) -> Result<Stmt, Message> {
        let name_tok = self.next();
        let Kind::Name(var) = &name_tok.kind else {
            return Err(self.ladder(name_tok).unwrap_or_else(|| self.expected(name_tok, "el nombre de la vuelta", "for i in range(10)")));
        };
        let in_tok = self.next();
        if in_tok.kind != Kind::Word("in") {
            return Err(self.expected(in_tok, "`in`", &format!("for {} in range(10)", var)));
        }
        let is_range = self.peek().kind == Kind::Name("range".into()) && self.t.get(self.at + 1).map(|t| &t.kind) == Some(&Kind::Sym("("));
        if !is_range {
            if LEVEL_NOW < 6 {
                let what = self.peek();
                return Err(not_yet(what, "recorrer una tabla con `for`", 6, &format!("por ahora, los numeros: for {} in range(10)", var)));
            }
            // `for x in tabla` (level 6): its cells, one per turn.
            let table = self.expr()?;
            self.loops += 1;
            let body = self.block(tok, "este `for`", &format!("for {} in tabla\n             print({})", var, var));
            self.loops -= 1;
            let zero = Expr::Int { value: 0, line: tok.line, col: tok.col };
            return Ok(Stmt::For(For { var: var.clone(), var_at: (name_tok.line, name_tok.col), from: zero.clone(), to: zero, over: Some(table), line: tok.line, col: tok.col, body: body? }));
        }
        let what = self.next();
        self.next();
        let first = self.expr()?;
        let sep = self.next();
        let (from, to) = match sep.kind {
            Kind::Sym(")") => (Expr::Int { value: 0, line: what.line, col: what.col }, first),
            Kind::Sym(",") => {
                let second = self.expr()?;
                let close = self.next();
                if close.kind != Kind::Sym(")") {
                    return Err(self.expected(close, "`)`", "range(desde, hasta)"));
                }
                (first, second)
            }
            _ => return Err(self.expected(sep, "`,` o `)`", "range(10)  o  range(1, 11)")),
        };
        self.loops += 1;
        let body = self.block(tok, "este `for`", &format!("for {} in range(10)\n             print({})", var, var));
        self.loops -= 1;
        Ok(Stmt::For(For { var: var.clone(), var_at: (name_tok.line, name_tok.col), from, to, over: None, line: tok.line, col: tok.col, body: body? }))
    }

    /// `break` / `continue`: only inside a loop (T0067).
    fn jump_statement(&mut self, tok: &Token) -> Result<Stmt, Message> {
        let word = if tok.kind == Kind::Word("break") { "break" } else { "continue" };
        if self.loops == 0 {
            return Err(Message::new(
                Code::OutsideLoop,
                tok.line,
                tok.col,
                &format!("`{}` fuera de un bucle", word),
                &format!("`{}` {} la vuelta de un `while` o de un `for`, y esta linea no esta dentro de ninguno", word, if word == "break" { "corta" } else { "salta a la siguiente" }),
                "ponlo dentro del bloque de un `while` o un `for`; para acabar una funcion antes, `return` llega en el nivel 5",
            ));
        }
        self.end_of_line()?;
        Ok(if word == "break" { Stmt::Break { line: tok.line, col: tok.col } } else { Stmt::Continue { line: tok.line, col: tok.col } })
    }

    /// One line of a body: a call, a `let`, `name = value`, an `if` or a loop.
    fn statement(&mut self) -> Result<Stmt, Message> {
        let tok = self.next();
        if tok.kind == Kind::Word("if") && LEVEL_NOW >= 3 {
            return self.if_statement(tok);
        }
        if tok.kind == Kind::Word("return") && LEVEL_NOW >= 5 {
            let value = if self.peek().kind == Kind::Newline { None } else { Some(self.expr()?) };
            self.end_of_line()?;
            return Ok(Stmt::Return { value, line: tok.line, col: tok.col });
        }
        if LEVEL_NOW >= 4 {
            match tok.kind {
                Kind::Word("while") => return self.while_statement(tok),
                Kind::Word("for") => return self.for_statement(tok),
                Kind::Word("break" | "continue") => return self.jump_statement(tok),
                _ => {}
            }
        }
        if tok.kind == Kind::Word("else") && LEVEL_NOW >= 3 {
            return Err(self.expected(tok, "una llamada, un `let` o un `if`", "un `else` va justo debajo del bloque de su `if`, al mismo margen que el `if`"));
        }
        if tok.kind == Kind::Word("let") {
            let mutable = self.peek().kind == Kind::Word("mut");
            if mutable {
                self.next();
            }
            let name_tok = self.next();
            let Kind::Name(name) = &name_tok.kind else {
                return Err(self.ladder(name_tok).unwrap_or_else(|| self.expected(name_tok, "el nombre del valor", "let area = 3 * 4")));
            };
            // `let precio: dec(7, 2) = ...` (level 7): the type DECLARED.
            let mut ty = None;
            if self.peek().kind == Kind::Sym(":") && LEVEL_NOW >= 7 {
                self.next();
                ty = Some(self.ty(&format!("let {}: dec(7, 2) = 0.00", name))?);
            }
            let eq = self.next();
            if eq.kind != Kind::Sym("=") {
                return Err(self.ladder(eq).unwrap_or_else(|| self.expected(eq, "`=`", &format!("let {} = 3 * 4", name))));
            }
            let value = self.expr()?;
            self.end_of_line()?;
            return Ok(Stmt::Let(Let { name: name.clone(), ty, mutable, line: name_tok.line, col: name_tok.col, value }));
        }
        if let Some(m) = self.ladder(tok) {
            return Err(m);
        }
        let Kind::Name(callee) = &tok.kind else {
            return Err(match tok.kind {
                Kind::Word("fn") => self.expected(tok, "una llamada, un `let` o un `if`", "una `fn` va arriba del todo, sin sangria"),
                Kind::Indent => Message::new(
                    Code::BadIndent,
                    tok.line,
                    tok.col,
                    "aqui no se abre ningun bloque",
                    "esta linea va mas sangrada que la de arriba, y nada de arriba abre un bloque",
                    "ponla al mismo margen que la linea anterior",
                ),
                _ => self.expected(tok, "una llamada, un `let` o un `if`", "print(\"hola\")  o  let area = 3 * 4"),
            });
        };
        let mut open = self.next();
        if matches!(open.kind, Kind::Sym("[") | Kind::Sym(".")) && LEVEL_NOW >= 6 {
            // `a[i] = v`, `nave.x = v`: a part of a value changes.
            let mut path = Vec::new();
            loop {
                match open.kind {
                    Kind::Sym("[") => {
                        let i = self.expr()?;
                        let close = self.next();
                        if close.kind != Kind::Sym("]") {
                            return Err(self.expected(close, "`]`", &format!("{}[0] = ...", callee)));
                        }
                        path.push(Step::Index(i));
                    }
                    Kind::Sym(".") => {
                        let f = self.next();
                        let Kind::Name(fname) = &f.kind else {
                            return Err(self.expected(f, "el nombre de un campo", &format!("{}.x = ...", callee)));
                        };
                        path.push(Step::Field(fname.clone(), f.line, f.col));
                    }
                    Kind::Sym("=") => break,
                    _ => return Err(self.expected(open, "`=`", &format!("{}[0] = 5", callee))),
                }
                open = self.next();
            }
            let value = self.expr()?;
            self.end_of_line()?;
            return Ok(Stmt::SetAt { name: callee.clone(), path, value, line: tok.line, col: tok.col });
        }
        if open.kind == Kind::Sym("=") {
            // `name = value`: the grammar knows it; whether it may change is
            // the checker's (`juez.rs`, T0056).
            let value = self.expr()?;
            self.end_of_line()?;
            return Ok(Stmt::Set(Let { name: callee.clone(), ty: None, mutable: false, line: tok.line, col: tok.col, value }));
        }
        if open.kind != Kind::Sym("(") {
            return Err(self.ladder(open).unwrap_or_else(|| self.expected(open, "`(` despues del nombre", &format!("{}(\"...\")", callee))));
        }
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
                    _ => return Err(self.ladder(sep).unwrap_or_else(|| self.expected(sep, "`,` o `)`", &format!("{}(\"a\", b)", callee)))),
                }
            }
        }
        self.end_of_line()?;
        Ok(Stmt::Call(Call { callee: callee.clone(), line: tok.line, col: tok.col, args }))
    }

    fn end_of_line(&mut self) -> Result<(), Message> {
        let end = self.next();
        if end.kind != Kind::Newline {
            return Err(self.ladder(end).unwrap_or_else(|| self.expected(end, "el final de la linea", "una cosa por linea")));
        }
        Ok(())
    }

    /// `a or b`: the weakest binding first.
    fn expr(&mut self) -> Result<Expr, Message> {
        let mut left = self.and()?;
        while self.peek().kind == Kind::Word("or") {
            let tok = self.next();
            let right = self.and()?;
            left = Expr::Bin { op: "or", left: Box::new(left), right: Box::new(right), line: tok.line, col: tok.col };
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, Message> {
        let mut left = self.not()?;
        while self.peek().kind == Kind::Word("and") {
            let tok = self.next();
            let right = self.not()?;
            left = Expr::Bin { op: "and", left: Box::new(left), right: Box::new(right), line: tok.line, col: tok.col };
        }
        Ok(left)
    }

    fn not(&mut self) -> Result<Expr, Message> {
        if self.peek().kind == Kind::Word("not") && LEVEL_NOW >= 3 {
            let tok = self.next();
            let value = self.not()?;
            return Ok(Expr::Not { value: Box::new(value), line: tok.line, col: tok.col });
        }
        self.compare()
    }

    /// `a < b`, and only ONE: `a < b < c` is a NO that says how.
    fn compare(&mut self) -> Result<Expr, Message> {
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
    fn sum(&mut self) -> Result<Expr, Message> {
        let mut left = self.term()?;
        while let Kind::Sym(op @ ("+" | "-")) = self.peek().kind {
            let tok = self.next();
            let right = self.term()?;
            left = Expr::Bin { op, left: Box::new(left), right: Box::new(right), line: tok.line, col: tok.col };
        }
        Ok(left)
    }

    fn term(&mut self) -> Result<Expr, Message> {
        let mut left = self.unary()?;
        while let Kind::Sym(op @ ("*" | "/" | "%")) = self.peek().kind {
            let tok = self.next();
            let right = self.unary()?;
            left = Expr::Bin { op, left: Box::new(left), right: Box::new(right), line: tok.line, col: tok.col };
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, Message> {
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

    fn primary(&mut self) -> Result<Expr, Message> {
        let tok = self.next();
        match &tok.kind {
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
    fn round(&self, tok: &Token, mut args: Vec<Expr>) -> Result<Expr, Message> {
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
    fn dec(&self, n: &str, tok: &Token) -> Result<Expr, Message> {
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

pub fn parse(tokens: &[Token]) -> Result<Program, Message> {
    let mut p = Parser { t: tokens, at: 0, loops: 0 };
    let (module, purpose) = p.header()?;
    let mut functions = Vec::new();
    let mut types = Vec::new();
    loop {
        let tok = p.peek();
        match &tok.kind {
            Kind::End => break,
            Kind::Word("fn") => functions.push(p.function()?),
            Kind::Word("type") if LEVEL_NOW >= 6 => types.push(p.typedef()?),
            Kind::Indent => {
                return Err(Message::new(
                    Code::BadIndent,
                    tok.line,
                    tok.col,
                    "aqui no se abre ningun bloque",
                    "esta linea va sangrada y no esta dentro de ninguna `fn`",
                    "quita la sangria, o ponla debajo de su `fn`",
                ))
            }
            _ => {
                return Err(p.ladder(tok).unwrap_or_else(|| {
                    p.expected(tok, "una `fn` o un `type`", "arriba del todo solo van funciones y tipos: fn main() y su cuerpo debajo")
                }))
            }
        }
    }
    Ok(Program { module, purpose, functions, types })
}
