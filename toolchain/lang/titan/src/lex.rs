//! `lex` -- from text to pieces. It does not know the grammar.
//!
//! ```text
//!    a word      one of the 25 (words.rs), whatever its level: saying "not
//!                yet" is the parser's job, with the level in the message
//!    a name      a letter, then letters, digits or _
//!    a number    digits (with a `.` inside): they arrive in level 1
//!    a text      "..." on one line; inside, \"  \\  \n
//!    a symbol    the proposal of TITAN_MAESTRO 14.2
//!    and the margin: Newline, Indent, Dedent (indent.rs)
//! ```
//!
//! A blank line or a line that is only a comment carries nothing: it does not
//! move the margin. The first NO stops the scan: one message that is right is
//! worth more than ten that follow from the first.

use crate::indent::{Change, Indenter};
use crate::message::{Code, Message};
use crate::words;

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Word(&'static str),
    Name(String),
    Number(String),
    Text(String),
    Sym(&'static str),
    Newline,
    Indent,
    Dedent,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: Kind,
    pub line: usize,
    pub col: usize,
}

/// The symbols of the proposal (14.2), the long ones first.
const SYMBOLS: [&str; 23] = [
    "==", "!=", "<=", ">=", "->", "(", ")", "{", "}", "[", "]", ",", ":", ";", ".", "=", "<", ">", "+", "-", "*", "/", "%",
];

/// What C and JavaScript write with symbols, TITAN++ writes with words
/// (14.13): the message says which.
fn instead(c: char) -> Option<&'static str> {
    match c {
        '!' => Some("en TITAN++ se escribe `not`"),
        '&' => Some("en TITAN++ se escribe `and` (y no hay punteros: `mut` y `take` dicen lo que decia `&`)"),
        '|' => Some("en TITAN++ se escribe `or`"),
        _ => None,
    }
}

pub fn lex(src: &str) -> Result<Vec<Token>, Message> {
    let mut out = Vec::new();
    let mut margin = Indenter::default();
    let mut last_line = 0;
    for (i, raw) in src.lines().enumerate() {
        let line = i + 1;
        last_line = line;
        let chars: Vec<char> = raw.chars().collect();
        let lead = chars.iter().take_while(|c| **c == ' ' || **c == '\t').count();
        if let Some(t) = chars[..lead].iter().position(|c| *c == '\t') {
            return Err(Message::new(
                Code::Tab,
                line,
                t + 1,
                "hay un tabulador en la sangria",
                "un tabulador y cuatro espacios se ven igual y valdrian distinto",
                "sangra con cuatro espacios; el editor puede ponerlos al pulsar Tab",
            ));
        }
        let rest = &chars[lead..];
        if rest.is_empty() || rest[0] == '#' {
            continue;
        }
        match margin.line(lead, line)? {
            Change::Same => {}
            Change::In => out.push(Token { kind: Kind::Indent, line, col: lead + 1 }),
            Change::Out(n) => {
                for _ in 0..n {
                    out.push(Token { kind: Kind::Dedent, line, col: lead + 1 });
                }
            }
        }
        scan_line(&chars, lead, line, &mut out)?;
        out.push(Token { kind: Kind::Newline, line, col: chars.len() + 1 });
    }
    for _ in 0..margin.open() {
        out.push(Token { kind: Kind::Dedent, line: last_line + 1, col: 1 });
    }
    out.push(Token { kind: Kind::End, line: last_line + 1, col: 1 });
    Ok(out)
}

fn scan_line(chars: &[char], from: usize, line: usize, out: &mut Vec<Token>) -> Result<(), Message> {
    let mut i = from;
    while i < chars.len() {
        let c = chars[i];
        let col = i + 1;
        if c == ' ' || c == '\t' {
            i += 1;
        } else if c == '#' {
            break;
        } else if c == '"' {
            let (text, next) = text(chars, i, line)?;
            out.push(Token { kind: Kind::Text(text), line, col });
            i = next;
        } else if c.is_ascii_alphabetic() || c == '_' {
            let end = (i..chars.len()).find(|&k| !(chars[k].is_ascii_alphanumeric() || chars[k] == '_')).unwrap_or(chars.len());
            let s: String = chars[i..end].iter().collect();
            let kind = match words::find(&s) {
                Some(w) => Kind::Word(w.text),
                None => Kind::Name(s),
            };
            out.push(Token { kind, line, col });
            i = end;
        } else if c.is_ascii_digit() {
            let end = (i..chars.len()).find(|&k| !(chars[k].is_ascii_digit() || chars[k] == '.')).unwrap_or(chars.len());
            out.push(Token { kind: Kind::Number(chars[i..end].iter().collect()), line, col });
            i = end;
        } else if let Some(sym) = SYMBOLS.iter().find(|s| chars[i..].iter().take(s.len()).copied().eq(s.chars())) {
            out.push(Token { kind: Kind::Sym(sym), line, col });
            i += sym.len();
        } else {
            return Err(Message::new(
                Code::StrayChar,
                line,
                col,
                &format!("`{}` no es de TITAN++", c),
                "fuera de un texto solo valen letras y cifras inglesas, `_` y los simbolos del lenguaje",
                instead(c).unwrap_or("si es para mostrarlo, ponlo dentro de un texto: \"...\""),
            ));
        }
    }
    Ok(())
}

/// A text from the `"` at `at`: its content and where the scan continues.
fn text(chars: &[char], at: usize, line: usize) -> Result<(String, usize), Message> {
    let mut s = String::new();
    let mut i = at + 1;
    while i < chars.len() {
        match chars[i] {
            '"' => return Ok((s, i + 1)),
            '\\' => {
                let esc = chars.get(i + 1).copied();
                match esc {
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some('n') => s.push('\n'),
                    _ => {
                        return Err(Message::new(
                            Code::BadEscape,
                            line,
                            i + 1,
                            "esa `\\` no dice nada que TITAN++ conozca",
                            "dentro de un texto, `\\` solo va antes de `\"`, de `\\` o de `n`",
                            "escribe `\\\\` si querias una barra, o quita la barra",
                        ))
                    }
                }
                i += 2;
            }
            c => {
                s.push(c);
                i += 1;
            }
        }
    }
    Err(Message::new(
        Code::OpenText,
        line,
        at + 1,
        "este texto no se cierra",
        "la linea acaba antes de la `\"` que lo cierra; un texto va en una sola linea",
        "cierra el texto con `\"` antes del final de la linea",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Kind> {
        lex(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn hola_in_pieces_with_its_margin() {
        let k = kinds("mod main \"saluda\"\n\nfn main()\n    print(\"hola\")  # un comentario\n");
        assert_eq!(
            k,
            [
                Kind::Word("mod"),
                Kind::Name("main".into()),
                Kind::Text("saluda".into()),
                Kind::Newline,
                Kind::Word("fn"),
                Kind::Name("main".into()),
                Kind::Sym("("),
                Kind::Sym(")"),
                Kind::Newline,
                Kind::Indent,
                Kind::Name("print".into()),
                Kind::Sym("("),
                Kind::Text("hola".into()),
                Kind::Sym(")"),
                Kind::Newline,
                Kind::Dedent,
                Kind::End,
            ]
        );
    }

    #[test]
    fn escapes_long_symbols_and_numbers() {
        assert_eq!(kinds("\"a\\\"b\\n\"")[0], Kind::Text("a\"b\n".into()));
        assert_eq!(kinds("a -> b")[1], Kind::Sym("->"));
        assert_eq!(kinds("x <= 12.5")[2], Kind::Number("12.5".into()));
    }

    #[test]
    fn each_no_with_its_code_and_column() {
        let e = lex("\tprint(\"x\")").unwrap_err();
        assert_eq!((e.code, e.col), (Code::Tab, 1));
        let e = lex("print(\"hola)").unwrap_err();
        assert_eq!((e.code, e.col), (Code::OpenText, 7));
        let e = lex("print(\"a\\q\")").unwrap_err();
        assert_eq!(e.code, Code::BadEscape);
        let e = lex("x = 1 @ 2").unwrap_err();
        assert_eq!((e.code, e.col), (Code::StrayChar, 7));
    }
}
