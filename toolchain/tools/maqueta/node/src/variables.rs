//! **Las variables de CSS, resueltas AL COMPILAR** (MAQUETA 2, 04-10).
//!
//! Las maquetas de las apps escriben los colores una vez, en `:root`, y los
//! usan con `var(--oro)` -- en `docs/arte/` son cien valores. Sin esto, cada
//! uno habria que reescribirlo a mano al pasar la maqueta a MAQUETA, y ese es
//! el sitio exacto donde dos copias de un color empiezan a no coincidir.
//!
//! ```text
//!    :root { --oro: #FFD45E; --hueco: 12px }
//!    .cifra { color: var(--oro); padding: var(--hueco) }
//!    .nota  { color: var(--tinta, #E6EDF6) }      con lo de reserva
//! ```
//!
//! ## Por que solo `:root`
//!
//! En CSS una variable se HEREDA: `.panel { --c: red }` vale para todo lo que
//! cuelga de `.panel`. Aqui no hay herencia -- una pieza no sabe que tiene
//! padre (L7) --, asi que una variable definida en otro sitio que no sea
//! `:root` significaria una cosa en el navegador y otra en el Ryzen. Se
//! rechaza, con el motivo. En `:root` las dos lecturas coinciden siempre: es
//! UNA tabla de constantes.
//!
//! ## Como
//!
//! Antes de leer ninguna regla: se sacan los bloques `:root`, se guarda cada
//! `--nombre` con sus TOKENS, y cada `var(--nombre)` del resto se sustituye
//! por ellos. Lo que sale es la misma lista de tokens que si el autor hubiera
//! escrito el valor a mano, y el resto del padre no se entera de que existian
//! variables. Es una sustitucion, no un interprete: no hay `calc()`.

use std::collections::HashMap;

use bmo_maqueta_diag::Error;
use bmo_maqueta_lex::{Kind, Token};

use crate::markup::span_of;

/// Lo mas hondo que se sigue una variable que usa otra. Mas es un ciclo.
const HONDO: usize = 8;

/// **Resuelve las variables**: devuelve los tokens de estilo sin los bloques
/// `:root` y con cada `var(...)` sustituido.
pub fn resolver(src: &[u8], toks: &[Token], datos: &mut Vec<(String, u32)>, errors: &mut Vec<Error>) -> Vec<Token> {
    let mut tabla: HashMap<Vec<u8>, Vec<Token>> = HashMap::new();
    let mut resto = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        let es_root = toks[i].kind == Kind::Colon
            && toks.get(i + 1).is_some_and(|t| t.kind == Kind::Ident && t.text(src) == b"root")
            && toks.get(i + 2).is_some_and(|t| t.kind == Kind::LBrace);
        if es_root {
            i = raiz(src, toks, i + 3, &mut tabla, errors);
            continue;
        }
        // Una variable definida FUERA de `:root`: `--c:` donde va una
        // declaracion.
        if toks[i].kind == Kind::Ident
            && toks[i].text(src).starts_with(b"--")
            && toks.get(i + 1).is_some_and(|t| t.kind == Kind::Colon)
        {
            errors.push(Error::new(
                span_of(&toks[i]),
                &format!("`{}` solo se puede definir en `:root`", String::from_utf8_lossy(toks[i].text(src))),
                "en CSS una variable se HEREDA a todo lo que cuelga de la caja donde se \
                 define, y en MAQUETA no hay herencia (una pieza no sabe que tiene padre, \
                 L7). Fuera de `:root` se leeria distinto en el navegador y en el Ryzen.",
                "definirla en `:root { --nombre: valor }`, que es una tabla de \
                 constantes para todo el fichero.",
            ));
            while i < toks.len() && !matches!(toks[i].kind, Kind::Semi | Kind::RBrace) {
                i += 1;
            }
            continue;
        }
        resto.push(toks[i]);
        i += 1;
    }
    colores_de_dato(src, &resto, &tabla, datos, errors);
    let mut fuera = Vec::with_capacity(resto.len());
    sustituir(src, &resto, &tabla, 0, &mut fuera, errors);
    fuera
}

/// ** LOS COLORES QUE LLEGAN AL EJECUTAR (H1, 04-10): `--dato-color: #4DE38F`.
///
/// El valor es la MUESTRA: lo que se juzga, lo que sale en la foto y en el
/// navegador. El emisor la reconoce por su valor en lo que pinta y pone el
/// dato en su sitio -- y para que reconocer no sea adivinar, la muestra NO
/// puede salir en el fichero de ninguna otra forma (ni escrita a mano ni en
/// otra variable). Si sale, se rechaza y se pide otra.
fn colores_de_dato(src: &[u8], resto: &[Token], tabla: &HashMap<Vec<u8>, Vec<Token>>, datos: &mut Vec<(String, u32)>, errors: &mut Vec<Error>) {
    let mut nombres: Vec<&Vec<u8>> = tabla.keys().filter(|k| k.starts_with(b"--dato-")).collect();
    nombres.sort();
    for n in nombres {
        let valor = &tabla[n];
        let nombre = String::from_utf8_lossy(&n[7..]).into_owned();
        let es_color = valor.len() == 1 && valor[0].kind == Kind::Color && valor[0].text(src).len() == 6;
        let campo_ok = nombre.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
            && nombre.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !es_color || !campo_ok {
            errors.push(Error::new(
                span_of(valor.first().unwrap_or(&resto[0])),
                &format!("`--dato-{nombre}` tiene que ser un color `#RRGGBB` con nombre en minusculas"),
                "una variable `--dato-*` es un COLOR que llega al ejecutar; su valor es \
                 la muestra, y su nombre sale como campo de Rust.",
                "por ejemplo `--dato-color: #4DE38F`.",
            ));
            continue;
        }
        let hex = valor[0].text(src).to_ascii_uppercase();
        let otra = |t: &&Token| t.kind == Kind::Color && t.text(src).to_ascii_uppercase() == hex;
        let choca = resto.iter().find(otra).or_else(|| tabla.iter().filter(|(k, _)| *k != n).flat_map(|(_, v)| v.iter()).find(otra));
        if let Some(t) = choca {
            errors.push(Error::new(
                span_of(t),
                &format!("`#{}` es la muestra de `--dato-{nombre}` y sale aqui tambien", String::from_utf8_lossy(&hex)),
                "el emisor reconoce el dato por su muestra: si el mismo color sale de \
                 otra forma, no sabria cual de los dos llega al ejecutar.",
                "dar a la muestra un color que no use nadie mas (uno cercano vale).",
            ));
            continue;
        }
        let v = String::from_utf8_lossy(&hex).chars().fold(0u32, |a, c| (a << 4) | c.to_digit(16).unwrap_or(0));
        datos.push((nombre, v));
    }
}

/// Lee un bloque `:root { ... }` desde el token siguiente a `{`. Devuelve
/// donde sigue (despues del `}`).
fn raiz(src: &[u8], toks: &[Token], mut i: usize, tabla: &mut HashMap<Vec<u8>, Vec<Token>>, errors: &mut Vec<Error>) -> usize {
    loop {
        let Some(t) = toks.get(i) else { return i };
        match t.kind {
            Kind::RBrace => return i + 1,
            Kind::Semi => i += 1,
            Kind::Ident if t.text(src).starts_with(b"--") && toks.get(i + 1).is_some_and(|c| c.kind == Kind::Colon) => {
                let nombre = t.text(src).to_vec();
                let mut j = i + 2;
                let mut valor = Vec::new();
                while let Some(v) = toks.get(j) {
                    if matches!(v.kind, Kind::Semi | Kind::RBrace) {
                        break;
                    }
                    valor.push(*v);
                    j += 1;
                }
                if valor.is_empty() {
                    errors.push(Error::new(
                        span_of(t),
                        &format!("`{}` no tiene valor", String::from_utf8_lossy(&nombre)),
                        "una variable es un nombre y lo que vale.",
                        "por ejemplo `--oro: #FFD45E`.",
                    ));
                }
                tabla.insert(nombre, valor);
                i = j;
            }
            _ => {
                errors.push(Error::new(
                    span_of(t),
                    "en `:root` solo van variables",
                    "`:root` es la tabla de constantes del fichero (`--nombre: valor`). \
                     Estilar la raiz se hace con su etiqueta, `maqueta`, como cualquier \
                     otra caja.",
                    "mover la declaracion a una regla `maqueta { ... }`.",
                ));
                while i < toks.len() && !matches!(toks[i].kind, Kind::Semi | Kind::RBrace) {
                    i += 1;
                }
            }
        }
    }
}

/// Copia `toks` a `fuera` cambiando cada `var(--x)` (o `var(--x, reserva)`)
/// por el valor de `--x`.
fn sustituir(src: &[u8], toks: &[Token], tabla: &HashMap<Vec<u8>, Vec<Token>>, hondo: usize, fuera: &mut Vec<Token>, errors: &mut Vec<Error>) {
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i];
        let es_var = t.kind == Kind::Ident && t.text(src) == b"var" && toks.get(i + 1).is_some_and(|p| p.kind == Kind::LParen);
        if !es_var {
            fuera.push(t);
            i += 1;
            continue;
        }
        // Hasta el `)` que cierra este `var(`, contando los de dentro.
        let mut j = i + 2;
        let mut nivel = 1;
        while let Some(u) = toks.get(j) {
            match u.kind {
                Kind::LParen => nivel += 1,
                Kind::RParen => {
                    nivel -= 1;
                    if nivel == 0 {
                        break;
                    }
                }
                Kind::Semi | Kind::RBrace => break,
                _ => {}
            }
            j += 1;
        }
        let dentro = &toks[i + 2..j.min(toks.len())];
        let cerrado = toks.get(j).is_some_and(|u| u.kind == Kind::RParen);
        i = if cerrado { j + 1 } else { j };
        let Some(nombre) = dentro.first().filter(|n| n.kind == Kind::Ident && n.text(src).starts_with(b"--")) else {
            errors.push(Error::new(
                span_of(&t),
                "`var(...)` quiere el nombre de una variable",
                "dentro va `--nombre`, y si se quiere, una coma y lo de reserva.",
                "por ejemplo `var(--oro)` o `var(--oro, #FFD45E)`.",
            ));
            continue;
        };
        if !cerrado {
            errors.push(Error::new(span_of(&t), "falta el `)` de `var(`", "cada `(` se cierra.", "cerrar con `)`."));
        }
        let reserva = match dentro.get(1) {
            Some(c) if c.kind == Kind::Comma => Some(&dentro[2..]),
            _ => None,
        };
        let valor = match (tabla.get(nombre.text(src)), reserva) {
            (Some(v), _) => v.as_slice(),
            (None, Some(r)) => r,
            (None, None) => {
                errors.push(Error::new(
                    span_of(nombre),
                    &format!("la variable `{}` no esta definida", String::from_utf8_lossy(nombre.text(src))),
                    "solo existen las que se definen en `:root`, en este mismo fichero.",
                    "definirla en `:root { --nombre: valor }`, o dar lo de reserva: \
                     `var(--nombre, #RRGGBB)`.",
                ));
                continue;
            }
        };
        if hondo >= HONDO {
            errors.push(Error::new(
                span_of(nombre),
                &format!("`{}` se usa a si misma", String::from_utf8_lossy(nombre.text(src))),
                "una variable que (por otras) acaba en si misma no tiene valor.",
                "romper el ciclo: que alguna diga su valor de verdad.",
            ));
            continue;
        }
        sustituir(src, valor, tabla, hondo + 1, fuera, errors);
    }
}
