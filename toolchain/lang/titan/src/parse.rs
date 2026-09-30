//! `parse` -- the grammar of level 0 (GRAMATICA.md). It does not know if a
//! name exists: that is `check`.
//!
//! ```text
//!    file     :=  header  function*
//!    header   :=  `mod` NAME TEXT NEWLINE
//!    function :=  `fn` NAME `(` `)` NEWLINE INDENT call+ DEDENT
//!    call     :=  NAME `(` [ TEXT { `,` TEXT } ] `)` NEWLINE
//! ```
//!
//! ** What makes it a LADDER and not a wall: anything that belongs to a level
//! above (one of the 25 words, a number, a `=`, a parameter, a `->`) is not
//! "unexpected" -- it is T0040, and the message says which level brings it.
//! The frontend grows by moving that line, not by rewriting this file.

use crate::lex::{Kind, Token};
use crate::message::{Code, Message};
use crate::tree::{Call, Function, Program};
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
                (level > LEVEL_NOW).then(|| not_yet(tok, &format!("`{}`", w), level, "por ahora, solo llamadas con textos: print(\"...\")"))
            }
            Kind::Number(_) => Some(not_yet(tok, "un numero", 1, "por ahora, un numero va dentro de un texto: print(\"12\")")),
            Kind::Sym("=") => Some(not_yet(tok, "dar un valor con `=`", 1, "por ahora, solo llamadas: print(\"...\")")),
            Kind::Sym("+" | "-" | "*" | "/" | "%") => Some(not_yet(tok, "calcular", 1, "por ahora, varios textos se separan con comas: print(\"a\", \"b\")")),
            Kind::Sym("->") => Some(not_yet(tok, "una funcion que devuelve algo", 5, "por ahora, `fn nombre()` sin `->`")),
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
        let end = self.next();
        if end.kind != Kind::Newline {
            return Err(self.ladder(end).unwrap_or_else(|| self.expected(end, "el final de la linea", &format!("fn {}() y el cuerpo debajo, sangrado", name))));
        }
        if self.peek().kind != Kind::Indent {
            return Err(Message::new(
                Code::EmptyBody,
                fn_tok.line,
                fn_tok.col,
                &format!("`fn {}()` no tiene cuerpo", name),
                "debajo de una funcion va su cuerpo, sangrado cuatro espacios",
                &format!("fn {}()\n             print(\"hola\")", name),
            ));
        }
        self.next();
        let mut body = Vec::new();
        while self.peek().kind != Kind::Dedent && self.peek().kind != Kind::End {
            body.push(self.call()?);
        }
        self.next();
        Ok(Function { name: name.clone(), line: fn_tok.line, col: fn_tok.col, body })
    }

    fn call(&mut self) -> Result<Call, Message> {
        let tok = self.next();
        if let Some(m) = self.ladder(tok) {
            return Err(m);
        }
        let Kind::Name(callee) = &tok.kind else {
            return Err(match tok.kind {
                Kind::Word("fn") => self.expected(tok, "una llamada", "una `fn` va arriba del todo, sin sangria"),
                Kind::Indent => Message::new(
                    Code::BadIndent,
                    tok.line,
                    tok.col,
                    "aqui no se abre ningun bloque",
                    "esta linea va mas sangrada que la de arriba, y nada de arriba abre un bloque",
                    "ponla al mismo margen que la linea anterior",
                ),
                _ => self.expected(tok, "una llamada, como print(\"hola\")", "print(\"hola\")"),
            });
        };
        let open = self.next();
        if open.kind != Kind::Sym("(") {
            return Err(self.ladder(open).unwrap_or_else(|| self.expected(open, "`(` despues del nombre", &format!("{}(\"...\")", callee))));
        }
        let mut args = Vec::new();
        loop {
            let a = self.next();
            match &a.kind {
                Kind::Sym(")") if args.is_empty() => break,
                Kind::Text(t) => args.push(t.clone()),
                Kind::Name(_) => return Err(not_yet(a, "usar un nombre como valor", 1, "por ahora, lo que se pasa es un texto: \"...\"")),
                _ => return Err(self.ladder(a).unwrap_or_else(|| self.expected(a, "un texto", &format!("{}(\"hola\")", callee)))),
            }
            let sep = self.next();
            match sep.kind {
                Kind::Sym(",") => continue,
                Kind::Sym(")") => break,
                _ => return Err(self.ladder(sep).unwrap_or_else(|| self.expected(sep, "`,` o `)`", &format!("{}(\"a\", \"b\")", callee)))),
            }
        }
        let end = self.next();
        if end.kind != Kind::Newline {
            return Err(self.ladder(end).unwrap_or_else(|| self.expected(end, "el final de la linea", "una llamada por linea")));
        }
        Ok(Call { callee: callee.clone(), line: tok.line, col: tok.col, args })
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
