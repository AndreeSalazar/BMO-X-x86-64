//! **Tokens -> a tree of named boxes.** Answers: *what is each piece?*
//!
//! ## The enforcement is the struct, not the discipline
//!
//! `Node` has **no parent field**, and that is the whole reason inheritance,
//! descendant selectors and `%` are impossible rather than merely forbidden. A
//! node knows its children -- they are part of what it *is* -- and knows nothing
//! above or beside it. L7: *the father does not know he has brothers*.
//!
//! Anyone tempted later to add `parent: Option<&Node>` should read this first:
//! that one field silently unlocks every feature the contract rejects, and
//! nothing would fail to compile.

use crate::value::{self, Tag};
use crate::Node;
use bmo_maqueta_diag::{Error, Span};
use bmo_maqueta_lex::{Kind, Token};

pub fn span_of(t: &Token) -> Span {
    Span::new(t.start, t.len, t.line, t.col)
}

fn text_of(src: &[u8], t: &Token) -> Vec<u8> {
    t.text(src).to_vec()
}

/// Build the tree. Errors accumulate; parsing carries on wherever the shape
/// still makes sense, because five compilations for five typos is the thing a
/// compiler is supposed to spare you.
pub fn parse(src: &[u8], toks: &[Token], errors: &mut Vec<Error>) -> Option<Node> {
    let mut stack: Vec<Node> = Vec::new();
    let mut roots: Vec<Node> = Vec::new();
    let mut i = 0usize;

    while i < toks.len() {
        let t = &toks[i];
        match t.kind {
            Kind::Lt => {
                i += 1;
                if let Some((mut node, self_closed)) = open_tag(src, toks, &mut i, errors) {
                    if node.tag == Tag::Svg {
                        // ** MAQUETA 3: lo de dentro de un `<svg>` es SVG y lo
                        // lee SU lector, entero, desde el texto.
                        dibujo(src, toks, &mut i, &mut node, self_closed, errors);
                        attach(&mut stack, &mut roots, node, errors);
                    } else if self_closed {
                        attach(&mut stack, &mut roots, node, errors);
                    } else {
                        stack.push(node);
                    }
                }
            }
            Kind::LtSlash => {
                i += 1;
                close_tag(src, toks, &mut i, &mut stack, &mut roots, errors);
            }
            Kind::Text => {
                let raw = text_of(src, t);
                if !raw.iter().all(|b| b.is_ascii_whitespace()) {
                    add_text(&mut stack, &raw, span_of(t), errors);
                }
                i += 1;
            }
            Kind::NonAscii => {
                errors.push(non_ascii(span_of(t)));
                i += 1;
            }
            Kind::StyleOpen | Kind::StyleClose => {
                // lib.rs lifts the style block out before we get here; seeing
                // one means there were two, and that error is reported there.
                i += 1;
            }
            _ => {
                errors.push(Error::new(
                    span_of(t),
                    &format!("aqui no puede ir {}", t.kind.name()),
                    "fuera de una etiqueta solo hay texto y otras etiquetas.",
                    "revisar si falta un `<` o sobra un caracter.",
                ));
                i += 1;
            }
        }
    }

    for open in stack.iter().rev() {
        errors.push(Error::new(
            open.span,
            &format!("`<{}>` se abrio y no se cerro", open.tag.name()),
            "MAQUETA no cierra etiquetas por su cuenta. Un navegador si, y esa \
             reparacion silenciosa es exactamente lo que L7 prohibe aqui: obligaria \
             al lexer a consultar el arbol.",
            &format!("escribir `</{}>` donde corresponda.", open.tag.name()),
        ));
    }

    if roots.is_empty() {
        return None;
    }
    if roots.len() > 1 {
        for extra in &roots[1..] {
            errors.push(Error::new(
                extra.span,
                "sobra una etiqueta en la raiz",
                "un fichero es UN componente, asi que tiene una sola raiz.",
                "envolver todo en el `<maqueta>` de arriba.",
            ));
        }
    }
    let root = roots.remove(0);
    if root.tag != Tag::Maqueta {
        errors.push(Error::new(
            root.span,
            &format!("la raiz es `<{}>` y tiene que ser `<maqueta>`", root.tag.name()),
            "`<maqueta>` es lo que declara el lienzo, y sin el no hay contra que \
             medir si algo se sale.",
            "envolver el contenido en `<maqueta> ... </maqueta>`.",
        ));
    }
    Some(root)
}

/// `<name attr="v" ...>` or `.../>`. Returns the node and whether it closed
/// itself. The cursor is left after the `>`.
fn open_tag(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    errors: &mut Vec<Error>,
) -> Option<(Node, bool)> {
    let name_tok = toks.get(*i)?;
    if name_tok.kind != Kind::Ident {
        errors.push(Error::new(
            span_of(name_tok),
            "falta el nombre de la etiqueta",
            "despues de `<` va un nombre.",
            "por ejemplo `<div>`.",
        ));
        return None;
    }
    let raw = text_of(src, name_tok);
    let tag = match Tag::from_name(&raw) {
        Some(t) => t,
        None => {
            errors.push(value::unknown_tag(span_of(name_tok), &raw));
            // Keep walking to the `>` so one bad tag does not cascade.
            skip_to_tag_end(toks, i);
            return None;
        }
    };
    *i += 1;

    let mut node = Node::new(tag, span_of(name_tok));
    while let Some(t) = toks.get(*i) {
        match t.kind {
            Kind::Gt => {
                *i += 1;
                return Some((node, false));
            }
            Kind::SlashGt => {
                *i += 1;
                return Some((node, true));
            }
            Kind::Ident => {
                attribute(src, toks, i, &mut node, errors);
            }
            _ => {
                errors.push(Error::new(
                    span_of(t),
                    &format!("dentro de la etiqueta no puede ir {}", t.kind.name()),
                    "dentro de `<...>` solo hay nombres de atributo, `=` y valores \
                     entre comillas.",
                    "revisar las comillas del atributo anterior.",
                ));
                *i += 1;
            }
        }
    }
    errors.push(Error::new(
        node.span,
        &format!("`<{}` se quedo sin cerrar el `>`", tag.name()),
        "la etiqueta empieza y el fichero se acaba.",
        "cerrar con `>` o con `/>`.",
    ));
    Some((node, true))
}

fn attribute(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    node: &mut Node,
    errors: &mut Vec<Error>,
) {
    let name_tok = toks[*i];
    let name = text_of(src, &name_tok);
    *i += 1;

    if toks.get(*i).map(|t| t.kind) != Some(Kind::Eq) {
        errors.push(Error::new(
            span_of(&name_tok),
            &format!(
                "el atributo `{}` no tiene valor",
                String::from_utf8_lossy(&name)
            ),
            "en MAQUETA todo atributo lleva valor. Los atributos sueltos son una \
             comodidad de HTML que aqui solo serviria para escribir erratas que \
             compilan.",
            "escribirlo como `nombre=\"valor\"`.",
        ));
        return;
    }
    *i += 1;

    let val_tok = match toks.get(*i) {
        Some(t) if t.kind == Kind::Str => *t,
        Some(t) => {
            errors.push(Error::new(
                span_of(t),
                "el valor del atributo tiene que ir entre comillas",
                "sin comillas no se sabe donde acaba el valor.",
                "por ejemplo `class=\"pad\"`.",
            ));
            *i += 1;
            return;
        }
        None => return,
    };
    let val = text_of(src, &val_tok);
    let vspan = span_of(&val_tok);
    *i += 1;

    match name.as_slice() {
        b"class" => {
            for word in val.split(|b| b.is_ascii_whitespace()).filter(|w| !w.is_empty()) {
                if !is_name(word) {
                    errors.push(bad_name(vspan, word, "una clase"));
                    continue;
                }
                node.classes.push(String::from_utf8_lossy(word).into_owned());
            }
        }
        b"id" => {
            if !is_name(&val) {
                errors.push(bad_name(vspan, &val, "un id"));
            } else {
                node.id = Some(String::from_utf8_lossy(&val).into_owned());
            }
        }
        b"nombre" => {
            if node.tag != Tag::Island {
                errors.push(Error::new(
                    span_of(&name_tok),
                    "`nombre` es solo de `<island>`",
                    "el nombre es como el proceso de fuera encuentra su rect. En \
                     cualquier otra caja no lo lee nadie.",
                    "usar `id` si lo que hace falta es la tabla de golpeo.",
                ));
            } else if !is_name(&val) {
                errors.push(bad_name(vspan, &val, "el nombre de una isla"));
            } else {
                node.island = Some(String::from_utf8_lossy(&val).into_owned());
            }
        }
        b"ancho" | b"alto" => {
            if node.tag != Tag::Maqueta {
                errors.push(Error::new(
                    span_of(&name_tok),
                    &format!(
                        "`{}` es solo de `<maqueta>`",
                        String::from_utf8_lossy(&name)
                    ),
                    "la medida de una caja se declara en el estilo, no en el marcado: \
                     mezclarlos daria dos sitios donde buscar el mismo numero.",
                    "`width` y `height` en el bloque `<style>`.",
                ));
            } else {
                match parse_u32(&val) {
                    Some(n) if name == b"ancho" => node.width = Some(n),
                    Some(n) => node.height = Some(n),
                    None => errors.push(Error::new(
                        vspan,
                        "esto no es un numero de pixeles",
                        "`ancho` y `alto` son enteros, sin unidad y sin signo.",
                        "por ejemplo `ancho=\"322\"`.",
                    )),
                }
            }
        }
        b"viewBox" if node.tag == Tag::Svg => {
            let t = String::from_utf8_lossy(&val).into_owned();
            match vista(&t) {
                Some(_) => node.svg_vista = Some(t),
                None => errors.push(Error::new(
                    vspan,
                    "un `viewBox` son cuatro numeros: x, y, ancho y alto",
                    "son las coordenadas propias del dibujo; el compilador las lleva a la \
                     caja del `<svg>`. El ancho y el alto, mayores que cero.",
                    "por ejemplo `viewBox=\"0 0 24 24\"`.",
                )),
            }
        }
        b"preserveAspectRatio" if node.tag == Tag::Svg => {
            let t = String::from_utf8_lossy(&val).into_owned();
            let mut p = t.split_whitespace();
            let alinea = p.next().unwrap_or("");
            let ok = (alinea == "none" || ["xMin", "xMid", "xMax"].iter().any(|x| ["YMin", "YMid", "YMax"].iter().any(|y| alinea == format!("{x}{y}"))))
                && p.next().is_none_or(|m| m == "meet" || m == "slice")
                && p.next().is_none();
            if ok {
                node.svg_aspecto = Some(t);
            } else {
                errors.push(Error::new(
                    vspan,
                    "este `preserveAspectRatio` no se sabe leer",
                    "es como cae el `viewBox` en la caja, y el navegador y BMO-X tienen que \
                     leerlo igual.",
                    "`none`, o `xMidYMid` (o `xMinYMin`...) con `meet` o `slice`.",
                ));
            }
        }
        b"src" if node.tag == Tag::Svg => {
            let ok = val.ends_with(b".svg")
                && !val.starts_with(b"/")
                && val.iter().all(|&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/'))
                && !val.windows(2).any(|w| w == b"..");
            if ok {
                node.src = Some(String::from_utf8_lossy(&val).into_owned());
            } else {
                errors.push(Error::new(
                    vspan,
                    "`src` de un `<svg>` es un `.svg` de esta carpeta o de debajo",
                    "lo lee el compilador con el lector de SVG y su dibujo va DENTRO del \
                     codigo generado: sin `..` ni `/` delante, se sabe siempre de donde sale.",
                    "por ejemplo `<svg class=\"logo\" src=\"arte/logo.svg\"/>`.",
                ));
            }
        }
        b"src" if node.tag == Tag::Imagen => {
            let ok = (val.ends_with(b".qoi") || val.ends_with(b".bmp") || val.ends_with(b".png"))
                && !val.starts_with(b"/")
                && val.iter().all(|&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/'))
                && !val.windows(2).any(|w| w == b"..");
            if ok {
                node.src = Some(String::from_utf8_lossy(&val).into_owned());
            } else {
                errors.push(Error::new(
                    vspan,
                    "`src` de una imagen es un `.qoi`, `.bmp` o `.png` de esta carpeta o de debajo",
                    "la lee el compilador (con `bmo-imagen`, el mismo lector del aparato) y \
                     sus pixeles van DENTRO del codigo generado: sin `..` ni `/` delante, \
                     se sabe siempre de donde salen.",
                    "por ejemplo `<imagen src=\"iconos/gato.qoi\"/>`.",
                ));
            }
        }
        b"dato" if node.tag == Tag::Imagen => {
            let s = String::from_utf8_lossy(&val).into_owned();
            if crate::hueco(&format!("{{{s}}}")).is_some_and(|(n, _)| n == s) {
                node.hueco = Some(s);
            } else {
                errors.push(Error::new(
                    vspan,
                    "`dato` es el nombre del campo, en minusculas",
                    "sale como campo de Rust en `Datos`: los pixeles que llegan al ejecutar.",
                    "por ejemplo `<imagen dato=\"miniatura\"/>`.",
                ));
            }
        }
        b"src" if node.tag == Tag::Usa => {
            let ok = val.ends_with(b".maqueta")
                && !val.starts_with(b"/")
                && val.iter().all(|&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/'))
                && !val.windows(2).any(|w| w == b"..");
            if ok {
                node.src = Some(String::from_utf8_lossy(&val).into_owned());
            } else {
                errors.push(Error::new(
                    vspan,
                    "`src` es el camino de una pieza: `fila.maqueta`",
                    "relativo a ESTE fichero, sin `..` ni `/` delante: una maqueta solo \
                     compone piezas de su carpeta o de debajo, y asi se sabe siempre de \
                     donde sale lo que se pinta.",
                    "por ejemplo `src=\"piezas/fila.maqueta\"`.",
                ));
            }
        }
        // ** LA LISTA (P2): la pieza, hasta `repite` veces en columna.
        b"repite" | b"entre" | b"columnas" if node.tag == Tag::Usa => {
            let es_repite = name == b"repite";
            let es_columnas = name == b"columnas";
            let n = parse_u32(&val);
            let ok = match n {
                Some(n) if es_repite => (1..=crate::MAX_REPITE).contains(&n),
                Some(n) if es_columnas => (1..=16).contains(&n),
                Some(n) => n <= 256,
                None => false,
            };
            if !ok {
                errors.push(Error::new(
                    vspan,
                    if es_repite {
                        "`repite` es cuantas filas caben como mucho: de 1 a 64"
                    } else if es_columnas {
                        "`columnas` es cuantas piezas van por fila de la rejilla: de 1 a 16"
                    } else {
                        "`entre` son los pixeles entre fila y fila: de 0 a 256"
                    },
                    "la lista se maqueta y se juzga con TODAS sus filas (lo peor que \
                     puede pasar); cuantas hay de verdad lo dice el aparato al correr.",
                    "por ejemplo `<usa src=\"fila.maqueta\" repite=\"8\" entre=\"4\"/>`.",
                ));
            } else {
                let r = node.repite.get_or_insert(crate::Repite { veces: 0, entre: 0, columnas: 1 });
                if es_repite {
                    r.veces = n.unwrap_or(0);
                } else if es_columnas {
                    r.columnas = n.unwrap_or(1);
                } else {
                    r.entre = n.unwrap_or(0);
                }
            }
        }
        other => {
            errors.push(Error::new(
                span_of(&name_tok),
                &format!(
                    "atributo no soportado -- `{}`",
                    String::from_utf8_lossy(other)
                ),
                "la lista de atributos esta CERRADA. Un atributo que se acepta y no \
                 se lee es una linea que parece hacer algo y no hace nada.",
                "`class`, `id`, `nombre` (solo en `<island>`), `ancho`/`alto` \
                 (solo en `<maqueta>`), `viewBox`, `preserveAspectRatio` y `src` (en \
                 `<svg>`), `src`, `repite`, `entre` y `columnas` (en `<usa>`), y `src` y \
                 `dato` (en `<imagen>`).",
            ));
        }
    }
}

fn close_tag(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    stack: &mut Vec<Node>,
    roots: &mut Vec<Node>,
    errors: &mut Vec<Error>,
) {
    let name_tok = match toks.get(*i) {
        Some(t) if t.kind == Kind::Ident => *t,
        _ => {
            errors.push(Error::new(
                toks.get(*i).map(span_of).unwrap_or(Span::new(0, 0, 1, 1)),
                "falta el nombre en la etiqueta de cierre",
                "despues de `</` va el nombre de lo que se cierra.",
                "por ejemplo `</div>`.",
            ));
            return;
        }
    };
    let raw = text_of(src, &name_tok);
    *i += 1;
    if toks.get(*i).map(|t| t.kind) == Some(Kind::Gt) {
        *i += 1;
    }

    let open = match stack.pop() {
        Some(n) => n,
        None => {
            errors.push(Error::new(
                span_of(&name_tok),
                &format!("`</{}>` cierra algo que no esta abierto", String::from_utf8_lossy(&raw)),
                "no hay ninguna etiqueta abierta en este punto.",
                "borrar el cierre, o abrir la etiqueta antes.",
            ));
            return;
        }
    };
    if Tag::from_name(&raw) != Some(open.tag) {
        errors.push(Error::new(
            span_of(&name_tok),
            &format!(
                "se cierra `</{}>` pero lo abierto es `<{}>`",
                String::from_utf8_lossy(&raw),
                open.tag.name()
            ),
            "MAQUETA no reordena etiquetas mal anidadas. Un navegador si, y hacerlo \
             obliga al lexer a mirar el arbol -- lo que L7a prohibe.",
            &format!("cerrar `</{}>` aqui.", open.tag.name()),
        ));
    }
    attach(stack, roots, open, errors);
}

/// Hang a finished node on its parent, or on the root list if there is none.
fn attach(stack: &mut [Node], roots: &mut Vec<Node>, node: Node, errors: &mut Vec<Error>) {
    if node.tag == Tag::Usa && node.repite.is_some_and(|r| r.veces == 0) {
        errors.push(Error::new(
            node.span,
            "este `<usa>` dice `entre` o `columnas` pero no `repite`",
            "`entre` es el hueco entre las filas de una lista; sin `repite` no hay \
             filas.",
            "`<usa src=\"fila.maqueta\" repite=\"8\" entre=\"4\"/>`.",
        ));
    }
    if node.tag == Tag::Imagen && node.src.is_none() && node.hueco.is_none() {
        errors.push(Error::new(
            node.span,
            "esta `<imagen>` no dice de donde salen sus pixeles",
            "o de un fichero (`src`, se embebe al compilar) o del aparato al correr \
             (`dato`, un campo de `Datos`).",
            "`<imagen src=\"gato.qoi\"/>` o `<imagen dato=\"miniatura\" src=\"muestra.qoi\"/>`.",
        ));
    }
    if node.tag == Tag::Usa && node.src.is_none() {
        errors.push(Error::new(
            node.span,
            "este `<usa>` no dice que pieza usa",
            "`<usa>` pone aqui otra maqueta, y sin `src` no hay cual.",
            "`<usa src=\"fila.maqueta\"/>`.",
        ));
    }
    match stack.last_mut() {
        Some(parent) => {
            // Lo de dentro de un `<svg>` lo lee el lector de SVG; un
            // `<path>` que llega aqui esta FUERA de un dibujo.
            if node.tag == Tag::Path {
                errors.push(Error::new(
                    node.span,
                    &format!("`<path>` no puede ir dentro de `<{}>`", parent.tag.name()),
                    "un `<path>` solo tiene sentido dentro de su `<svg>`: es quien dice su \
                     `viewBox` y en que caja cae.",
                    "ponerlo dentro de un `<svg viewBox=\"...\">`.",
                ));
                return;
            }
            if !parent.tag.takes_boxes() {
                errors.push(Error::new(
                    node.span,
                    &format!("`<{}>` no puede llevar cajas dentro", parent.tag.name()),
                    match parent.tag {
                        Tag::Island => {
                            "una isla la rellena OTRO proceso: lo que hubiera dentro \
                             no lo pintaria nadie."
                        }
                        Tag::Usa => {
                            "lo de dentro de un `<usa>` es la pieza, y la pieza se \
                             escribe en SU fichero: aqui no se pinta nada mas."
                        }
                        Tag::Imagen => "una imagen son sus pixeles: no lleva nada dentro.",
                        _ => {
                            "un `<span>` lleva texto. Meter cajas dentro es flujo en \
                             linea, y eso no esta implementado."
                        }
                    },
                    "sacar la caja fuera.",
                ));
                return;
            }
            if parent.text.is_some() {
                errors.push(mixed(node.span));
                return;
            }
            parent.children.push(node);
        }
        None => roots.push(node),
    }
}

fn add_text(stack: &mut [Node], raw: &[u8], span: Span, errors: &mut Vec<Error>) {
    let node = match stack.last_mut() {
        Some(n) => n,
        None => {
            errors.push(Error::new(
                span,
                "hay texto fuera de la raiz",
                "todo lo que se pinta va dentro de `<maqueta>`.",
                "meter el texto en un `<div>` o un `<span>`.",
            ));
            return;
        }
    };
    if !node.children.is_empty() {
        errors.push(mixed(span));
        return;
    }
    if node.tag == Tag::Imagen {
        errors.push(Error::new(
            span,
            "una `<imagen>` no lleva texto",
            "son sus pixeles; el texto va en su propio `<span>`.",
            "dejarla vacia: `<imagen src=\"gato.qoi\"/>`.",
        ));
        return;
    }
    if node.tag == Tag::Usa {
        errors.push(Error::new(
            span,
            "un `<usa>` no lleva texto",
            "el texto de una pieza va en el fichero de la pieza.",
            "dejarlo vacio: `<usa src=\"fila.maqueta\"/>`.",
        ));
        return;
    }
    if node.tag == Tag::Island {
        errors.push(Error::new(
            span,
            "una isla no lleva texto",
            "la rellena otro proceso; lo que se escriba aqui no lo pinta nadie.",
            "dejarla vacia: `<island nombre=\"...\"></island>`.",
        ));
        return;
    }
    let s = String::from_utf8_lossy(raw).trim().to_string();
    // ** UN HUECO (H1): `{nombre|muestra}` es TODO el texto de su caja.
    if s.contains('{') || s.contains('}') {
        match crate::hueco(&s) {
            Some((nombre, muestra)) if node.text.is_none() => {
                node.hueco = Some(nombre.to_string());
                node.text = Some(muestra.to_string());
            }
            _ => errors.push(Error::new(
                span,
                "un hueco de datos es `{nombre}` o `{nombre|muestra}`, y es TODO el texto de su caja",
                "el texto que llega al ejecutar va en su propia caja, con su nombre \
                 en minusculas (sale como campo de Rust). La muestra es lo que se \
                 maqueta, se juzga y sale en la foto.",
                "por ejemplo `<span class=\"nombre\">{nombre|Ana Lopez}</span>`.",
            )),
        }
        return;
    }
    match &mut node.text {
        Some(t) => {
            t.push(' ');
            t.push_str(&s);
        }
        None => node.text = Some(s),
    }
}

fn mixed(span: Span) -> Error {
    Error::new(
        span,
        "no se pueden mezclar texto y cajas en la misma etiqueta",
        "mezclarlos es flujo en linea, que es la parte cara de un motor de \
         maquetacion y no esta implementada. Aceptarlo a medias daria un texto \
         colocado de una forma que nadie escribio.",
        "meter el texto en su propio `<span>`, hermano de las cajas.",
    )
}

fn non_ascii(span: Span) -> Error {
    Error::new(
        span,
        "byte fuera de ASCII",
        "las fuentes de BMO-X son ASCII, y no por estetica: una sola letra acentuada \
         en un literal llego a hacer crecer un `.bex` de 512 bytes a 492.032. Las \
         cadenas de pantalla son castellano SIN tilde.",
        "escribir el texto sin tildes ni enes con virgulilla.",
    )
}

fn bad_name(span: Span, raw: &[u8], what: &str) -> Error {
    Error::new(
        span,
        &format!("`{}` no vale como {what}", String::from_utf8_lossy(raw)),
        "un nombre empieza por letra o `_` y sigue con letras, cifras, `-` o `_`.",
        "por ejemplo `tecla-op`.",
    )
}

/// Los cuatro numeros de un `viewBox`, con el ancho y el alto mayores que 0.
pub fn vista(t: &str) -> Option<[f64; 4]> {
    let v: Vec<f64> = t.split(|c: char| c.is_ascii_whitespace() || c == ',').filter(|w| !w.is_empty()).map(|w| w.parse::<f64>().ok()).collect::<Option<_>>()?;
    match v.as_slice() {
        [a, b, c, d] if *c > 0.0 && *d > 0.0 && v.iter().all(|x| x.is_finite()) => Some([*a, *b, *c, *d]),
        _ => None,
    }
}

/// **Un `<svg>`** (MAQUETA 3): lo de dentro, desde el TEXTO, al lector de
/// SVG. Los tokens que el lexer saco de dentro se saltan: el lexer de la
/// maqueta no sabe SVG, y su lector si.
fn dibujo(src: &[u8], toks: &[Token], i: &mut usize, node: &mut Node, self_closed: bool, errors: &mut Vec<Error>) {
    let (ini, fin) = if self_closed {
        (0, 0)
    } else {
        let ini = toks.get(i.wrapping_sub(1)).map_or(src.len(), |t| t.start + t.len);
        let mut hondo = 0usize;
        let mut j = *i;
        let mut fin = None;
        while j < toks.len() {
            let t = &toks[j];
            let es_svg = toks.get(j + 1).is_some_and(|n| n.kind == Kind::Ident && n.text(src) == b"svg");
            match t.kind {
                Kind::Lt if es_svg => hondo += 1,
                Kind::LtSlash if es_svg => {
                    if hondo == 0 {
                        fin = Some(j);
                        break;
                    }
                    hondo -= 1;
                }
                Kind::NonAscii => errors.push(non_ascii(span_of(t))),
                _ => {}
            }
            j += 1;
        }
        let Some(k) = fin else {
            errors.push(Error::new(node.span, "`<svg>` se abrio y no se cerro", "un dibujo acaba en su `</svg>`.", "escribir `</svg>`."));
            *i = toks.len();
            return;
        };
        let fin = toks[k].start;
        *i = k + 2;
        if toks.get(*i).map(|t| t.kind) == Some(Kind::Gt) {
            *i += 1;
        }
        (ini, fin)
    };
    let hay_dentro = src[ini..fin].iter().any(|b| !b.is_ascii_whitespace());
    if node.src.is_some() {
        if hay_dentro {
            errors.push(Error::new(
                node.span,
                "este `<svg>` dice `src` y ademas trae dibujo dentro",
                "o el dibujo esta en su fichero, o esta aqui; con los dos no se sabe cual se pinta.",
                "`<svg src=\"logo.svg\"/>` vacio, o el dibujo dentro sin `src`.",
            ));
        }
        if node.svg_vista.is_some() || node.svg_aspecto.is_some() {
            errors.push(Error::new(
                node.span,
                "un `<svg src>` no dice su `viewBox` ni su `preserveAspectRatio`",
                "los dice su fichero, como en el navegador.",
                "quitarlos de aqui.",
            ));
        }
        return;
    }
    let v = node.svg_vista.as_deref().and_then(vista);
    match bmo_maqueta_dibujo::leer_dentro(src, ini, fin, v, node.svg_aspecto.as_deref().unwrap_or("")) {
        Ok(s) => node.dibujo = Some(bmo_maqueta_dibujo::Dibujo(std::sync::Arc::new(s))),
        Err(e) => errors.extend(e),
    }
}

fn skip_to_tag_end(toks: &[Token], i: &mut usize) {
    while let Some(t) = toks.get(*i) {
        *i += 1;
        if t.kind == Kind::Gt || t.kind == Kind::SlashGt {
            return;
        }
    }
}

fn is_name(b: &[u8]) -> bool {
    !b.is_empty()
        && (b[0].is_ascii_alphabetic() || b[0] == b'_')
        && b.iter().all(|&c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn parse_u32(b: &[u8]) -> Option<u32> {
    if b.is_empty() || !b.iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut n: u32 = 0;
    for &c in b {
        n = n.checked_mul(10)?.checked_add((c - b'0') as u32)?;
    }
    Some(n)
}
