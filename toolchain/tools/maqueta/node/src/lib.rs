//! # MAQUETA -- NODE, the father generation
//!
//! generacion: padre
//!
//! Tokens in; **named pieces** out. A `Node` is a box that knows what it is; a
//! `Rule` is a rule that knows what it says. Neither knows the other exists.
//!
//! ## What this generation may and may not know (L7)
//!
//! > *the father names it and composes it -- **he does not know he has
//! > brothers***
//!
//! A node knows its children, because they are part of what it *is*. It does
//! **not** know its parent or its siblings, and `Node` has no field for either.
//! That is not a style choice; it is the enforcement:
//!
//! | what CSS wants | what it would need | verdict |
//! |---|---|---|
//! | inheritance (`color` from the parent) | a parent pointer | impossible |
//! | descendant selectors (`.a .b`) | the ancestor chain | impossible |
//! | `%`, `auto` | the container's size | impossible |
//!
//! **The data structure decides the feature set.** Add `parent: Option<..>` and
//! all three become available with nothing failing to compile -- which is why
//! the absence is written down here rather than merely observed.
//!
//! ## What this generation rejects, and what it does not
//!
//! It rejects the **unnameable**: there is no `Tag` for `h1`, no `Prop` for
//! `box-shadow`, no unit but `px`. That is a naming failure, structural.
//!
//! It does *not* reject the **unwise** -- text that does not fit its box, a
//! child that escapes its parent, two rules in the wrong order. Those are
//! opinions about a finished layout, and they belong to `verdict/`.

#![forbid(unsafe_code)]

pub mod markup;
pub mod style;
pub mod value;
pub mod variables;

use bmo_maqueta_diag::{Error, Span};
use bmo_maqueta_lex::{lex, Kind, Token};

pub use value::{Keyword, Prop, Shape, Tag, Transicion, Value};

/// One box, named. **No parent, no siblings** -- see the module header.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Node {
    pub tag: Tag,
    pub classes: Vec<String>,
    /// The hit-table key. Never a styling hook -- see `style::selector`.
    pub id: Option<String>,
    /// `<island nombre="...">`: how the process outside finds its rect.
    pub island: Option<String>,
    /// Only ever set on `<maqueta>`. Absent means "the compiler works it out".
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Text content. A node has text or children, never both.
    pub text: Option<String>,
    /// `<svg>` (MAQUETA 3): el dibujo, leido y juzgado por el lector de SVG
    /// (`bmo-maqueta-dibujo`). Con `src="logo.svg"` lo pone `compone`, que es
    /// quien lee ficheros.
    pub dibujo: Option<bmo_maqueta_dibujo::Dibujo>,
    /// `viewBox` y `preserveAspectRatio` del `<svg>`, como se escribieron.
    pub svg_vista: Option<String>,
    pub svg_aspecto: Option<String>,
    /// `<usa src="...">`: la pieza que va aqui, relativa a este fichero.
    pub src: Option<String>,
    /// `<usa repite="8" entre="4">` (P2): la pieza, hasta N veces en columna.
    pub repite: Option<Repite>,
    /// `{nombre|muestra}` (H1): el texto llega al ejecutar. `text` lleva la
    /// muestra (la que se maqueta, se juzga y sale en la foto) y esto el
    /// nombre del dato.
    pub hueco: Option<String>,
    pub children: Vec<Node>,
    pub span: Span,
}

impl Node {
    pub fn new(tag: Tag, span: Span) -> Self {
        Self {
            tag,
            classes: Vec::new(),
            id: None,
            island: None,
            width: None,
            height: None,
            text: None,
            dibujo: None,
            svg_vista: None,
            svg_aspecto: None,
            src: None,
            repite: None,
            hueco: None,
            children: Vec::new(),
            span,
        }
    }
}

/// **Una LISTA** (P2, 04-10): `<usa src="fila.maqueta" repite="8" entre="4"/>`.
/// La pieza es la FILA; cuantas hay lo dice el aparato al correr, hasta
/// `veces`. Se maqueta y se juzga con TODAS (lo peor que puede pasar), en
/// columna, `entre` pixeles una de otra. Con `columnas="2"` (H6) es una
/// REJILLA: de izquierda a derecha y bajando, `entre` en los dos sentidos.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Repite {
    pub veces: u32,
    pub entre: u32,
    pub columnas: u32,
}

impl Repite {
    /// Cuantas filas de rejilla salen con todas.
    pub fn filas(&self) -> u32 {
        self.veces.div_ceil(self.columnas.max(1))
    }

    /// Lo que mide entera, con piezas de `(w, h)`.
    pub fn medida(&self, (w, h): (u32, u32)) -> (u32, u32) {
        let (c, f) = (self.columnas.max(1).min(self.veces), self.filas());
        (c * w + self.entre * (c - 1), f * h + self.entre * (f - 1))
    }

    /// Donde va la pieza `k`, relativo a la lista.
    pub fn sitio(&self, k: u32, (w, h): (u32, u32)) -> (u32, u32) {
        let c = self.columnas.max(1);
        ((k % c) * (w + self.entre), (k / c) * (h + self.entre))
    }
}

/// Lo mas largo que se deja una lista.
pub const MAX_REPITE: u32 = 64;

/// **Un hueco de datos** en un texto: `{nombre}` o `{nombre|muestra}`. El
/// nombre en minusculas (`[a-z][a-z0-9_]*`): sale como campo de Rust.
pub fn hueco(t: &str) -> Option<(&str, &str)> {
    let dentro = t.strip_prefix('{')?.strip_suffix('}')?;
    let (nombre, muestra) = match dentro.split_once('|') {
        Some((n, m)) => (n.trim(), m.trim()),
        None => (dentro.trim(), dentro.trim()),
    };
    let ok = nombre.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && nombre.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    ok.then_some((nombre, muestra))
}

/// One declaration, named and with its value resolved to integers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Decl {
    pub prop: Prop,
    pub value: Value,
    pub span: Span,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Selector {
    Tag(Tag),
    Class(String),
}

/// One rule. **Does not know that other rules exist** -- which one wins is a
/// relation between two, so it lives in `cascade/`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub decls: Vec<Decl>,
    pub span: Span,
    /// `.tecla:hover { ... }` -- applies only while the pointer is over the box.
    ///
    /// * It is a property of the RULE and not of each selector because a rule
    /// where half the selectors hover and half do not has no single meaning, and
    /// a meaning that has to be explained twice is one this compiler refuses.
    pub hover: bool,
    /// `@estado abierta { ... }` (04-10): la regla solo vale en ese estado.
    /// `None` = la del reposo, que vale en todos.
    pub estado: Option<String>,
}

/// What a `.maqueta` file is, once named: a tree and a list of rules, side by
/// side and still unrelated.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Document {
    pub root: Node,
    pub rules: Vec<Rule>,
    /// Los COLORES que llegan al ejecutar (H1): `:root { --dato-color:
    /// #4DE38F }`. El nombre sin `--dato-`, y su muestra (la que se juzga y
    /// sale en la foto). La muestra no puede salir en el fichero de otra
    /// forma: asi el emisor la reconoce sin adivinar.
    pub datos: Vec<(String, u32)>,
}

impl Document {
    /// **Los estados que declara el fichero**, en el orden en que aparecen.
    /// El reposo no esta: es el de partida, el que no tiene `@estado`.
    pub fn estados(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for r in &self.rules {
            if let Some(e) = &r.estado {
                if !v.contains(e) {
                    v.push(e.clone());
                }
            }
        }
        v
    }

    /// **El documento tal como es EN un estado**: las reglas del reposo y,
    /// en su sitio, las de ese estado. Asi la cascada no sabe que existen
    /// los estados: "gana la ultima" lo resuelve igual que siempre, y es lo
    /// que veria un navegador con ese bloque `@estado` abierto.
    pub fn en_estado(&self, estado: Option<&str>) -> Document {
        Document {
            datos: self.datos.clone(),
            root: self.root.clone(),
            rules: self
                .rules
                .iter()
                .filter(|r| r.estado.is_none() || r.estado.as_deref() == estado)
                .cloned()
                .map(|mut r| {
                    r.estado = None;
                    r
                })
                .collect(),
        }
    }
}

/// Name everything in a source file.
///
/// Every error is collected, never just the first: five typos should cost one
/// compilation, not five.
pub fn parse(src: &[u8]) -> Result<Document, Vec<Error>> {
    let toks = lex(src);
    let mut errors = Vec::new();
    let (markup_toks, style_toks) = split(&toks, &mut errors);

    // `:root` y `var(--x)` se resuelven ANTES de leer reglas: lo que llega a
    // `style::parse` es lo mismo que si el valor se hubiera escrito a mano.
    let mut datos = Vec::new();
    let style_toks = variables::resolver(src, &style_toks, &mut datos, &mut errors);
    let rules = style::parse(src, &style_toks, &mut errors);
    let root = markup::parse(src, &markup_toks, &mut errors);

    match root {
        Some(root) if errors.is_empty() => Ok(Document { root, rules, datos }),
        Some(_) => Err(errors),
        None => {
            if errors.is_empty() {
                errors.push(Error::new(
                    Span::new(0, 0, 1, 1),
                    "el fichero no tiene nada que maquetar",
                    "hace falta al menos un `<maqueta>` con algo dentro.",
                    "empezar por `<maqueta> ... </maqueta>`.",
                ));
            }
            Err(errors)
        }
    }
}

/// Cut the token stream in two: what is markup and what is style.
///
/// The lexer already told us where the boundary is (`StyleOpen`/`StyleClose`
/// are literal byte matches, not a judgement about the tree), so this is a
/// split and not a parse.
fn split(toks: &[Token], errors: &mut Vec<Error>) -> (Vec<Token>, Vec<Token>) {
    let mut markup = Vec::new();
    let mut styles = Vec::new();
    let mut seen = 0u32;
    let mut inside = false;

    for t in toks {
        match t.kind {
            Kind::StyleOpen => {
                seen += 1;
                inside = true;
                if seen == 2 {
                    errors.push(Error::new(
                        markup::span_of(t),
                        "solo puede haber un bloque `<style>`",
                        "con dos, cual gana depende del orden en que se lean -- que es \
                         justo la clase de regla invisible que MAQUETA existe para no \
                         tener.",
                        "juntar las reglas en un solo bloque.",
                    ));
                }
            }
            Kind::StyleClose => inside = false,
            _ if inside => styles.push(*t),
            _ => markup.push(*t),
        }
    }
    (markup, styles)
}
