//! `parse` -- the grammar of the levels done (GRAMATICA.md). It does not know
//! if a name exists: that is `check`, and whether it has a value at that line
//! is the checker's (`juez.rs`).
//!
//! ```text
//!    file     :=  header  function*
//!    header   :=  `mod` NAME TEXT NEWLINE
//!    function :=  `fn` NAME `(` `)` block
//!    block    :=  NEWLINE INDENT stmt+ DEDENT
//!    stmt     :=  call | `let` [`mut`] NAME `=` expr | NAME `=` expr   NEWLINE
//!              |  `if` expr block [ `else` ( block | if ) ]          (level 3)
//!    call     :=  NAME `(` [ expr { `,` expr } ] `)`
//!    expr     :=  and { `or` and }                                  (level 3)
//!    and      :=  not { `and` not }
//!    not      :=  `not` not | compare
//!    compare  :=  sum [ (`==` | `!=` | `<` | `<=` | `>` | `>=`) sum ]
//!    sum      :=  term { (`+` | `-`) term }
//!    term     :=  unary { (`*` | `/` | `%`) unary }
//!    unary    :=  `-` unary | NUMBER | TEXT | NAME | `true` | `false` | `(` expr `)`
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
use crate::tree::{Call, Expr, Function, If, Let, Program, Stmt};
use crate::words::{self, LEVEL_NOW};

struct Parser<'a> {
    t: &'a [Token],
    at: usize,
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
                (level > LEVEL_NOW).then(|| not_yet(tok, &format!("`{}`", w), level, "por ahora: llamadas, `let`, `let mut` e `if` / `else`"))
            }
            Kind::Number(n) if n.contains('.') => Some(not_yet(tok, "un decimal", 6, "por ahora, numeros enteros: los decimales EXACTOS (dec) llegan con los tipos")),
            Kind::Sym("->") => Some(not_yet(tok, "una funcion que devuelve algo", 5, "por ahora, `fn nombre()` sin `->`")),
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
        let close = self.next();
        if close.kind != Kind::Sym(")") {
            return Err(match close.kind {
                Kind::Name(_) => not_yet(close, "una funcion con parametros", 5, &format!("por ahora, fn {}()", name)),
                _ => self.expected(close, "`)`", &format!("fn {}()", name)),
            });
        }
        let end = self.peek();
        if end.kind != Kind::Newline {
            return Err(self.ladder(end).unwrap_or_else(|| self.expected(end, "el final de la linea", &format!("fn {}() y el cuerpo debajo, sangrado", name))));
        }
        let body = self.block(fn_tok, &format!("`fn {}()`", name), &format!("fn {}()\n             print(\"hola\")", name))?;
        Ok(Function { name: name.clone(), line: fn_tok.line, col: fn_tok.col, body })
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

    /// One line of a body: a call, a `let`, `name = value`, or an `if`.
    fn statement(&mut self) -> Result<Stmt, Message> {
        let tok = self.next();
        if tok.kind == Kind::Word("if") && LEVEL_NOW >= 3 {
            return self.if_statement(tok);
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
            let eq = self.next();
            if eq.kind != Kind::Sym("=") {
                return Err(self.ladder(eq).unwrap_or_else(|| self.expected(eq, "`=`", &format!("let {} = 3 * 4", name))));
            }
            let value = self.expr()?;
            self.end_of_line()?;
            return Ok(Stmt::Let(Let { name: name.clone(), mutable, line: name_tok.line, col: name_tok.col, value }));
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
        let open = self.next();
        if open.kind == Kind::Sym("=") {
            // `name = value`: the grammar knows it; whether it may change is
            // the checker's (`juez.rs`, T0056).
            let value = self.expr()?;
            self.end_of_line()?;
            return Ok(Stmt::Set(Let { name: callee.clone(), mutable: false, line: tok.line, col: tok.col, value }));
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
        let tok = self.next();
        match &tok.kind {
            Kind::Sym("-") => {
                let value = self.unary()?;
                Ok(Expr::Neg { value: Box::new(value), line: tok.line, col: tok.col })
            }
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
            Kind::Text(t) => Ok(Expr::Text { value: t.clone(), line: tok.line, col: tok.col }),
            Kind::Word(w @ ("true" | "false")) if LEVEL_NOW >= 3 => Ok(Expr::Bool { value: *w == "true", line: tok.line, col: tok.col }),
            Kind::Name(n) => Ok(Expr::Name { name: n.clone(), line: tok.line, col: tok.col }),
            _ => Err(self.ladder(tok).unwrap_or_else(|| self.expected(tok, "un valor: un numero, un texto o un nombre", "let area = 3 * 4"))),
        }
    }
}

pub fn parse(tokens: &[Token]) -> Result<Program, Message> {
    let mut p = Parser { t: tokens, at: 0 };
    let (module, purpose) = p.header()?;
    let mut functions = Vec::new();
    loop {
        let tok = p.peek();
        match &tok.kind {
            Kind::End => break,
            Kind::Word("fn") => functions.push(p.function()?),
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
                    p.expected(tok, "una `fn`", "arriba del todo solo van funciones: fn main() y su cuerpo debajo")
                }))
            }
        }
    }
    Ok(Program { module, purpose, functions })
}
