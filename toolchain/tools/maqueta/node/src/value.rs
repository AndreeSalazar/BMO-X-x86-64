//! **The closed lists, as types.**
//!
//! This file is the contract of `LA_MAQUETA_EXIGE.md` sections 2, 3 and 4
//! turned into Rust enums, and that is not a formality:
//!
//! > There is no `Tag` value for `h1`. There is no `Prop` value for
//! > `z-index`. **The father cannot represent them.**
//!
//! So rejecting them is not an opinion this generation holds -- it is a naming
//! failure, which is structural. Opinions live in `verdict/`. The difference is
//! real: the father rejects the *unnameable*, the great-grandson rejects the
//! *unwise* (text that does not fit, a box that escapes its parent).

use bmo_maqueta_diag::{Error, Span};

/// The tags. `<style>` is not here: the lexer eats it and its contents
/// become rules, so it never reaches the tree.
///
/// ** `<svg>` and `<path>` (MAQUETA 2, 04-10): a drawing is MATHS, not
/// pixels. The path is the SVG `d` of the maqueta, the compiler flattens its
/// curves on the host, exactly, and the device only inks segments. Being
/// real SVG, the browser draws the same file: the ruler still works.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tag {
    Maqueta,
    Div,
    Span,
    Island,
    Svg,
    Path,
    /// `<usa src="fila.maqueta"/>` (04-10): una PIEZA, otra maqueta compilada
    /// sola y puesta aqui. Para esta maqueta es una caja hoja de la medida que
    /// la pieza calculo; dentro no se ve nada de ella.
    Usa,
    /// `<imagen src="gato.qoi"/>` o `<imagen dato="miniatura"/>` (H4, 04-10):
    /// pixeles, de un fichero (embebidos al compilar) o del aparato al correr.
    /// Mide lo que mide la imagen; es una hoja.
    Imagen,
}

impl Tag {
    pub fn from_name(b: &[u8]) -> Option<Tag> {
        match b {
            b"maqueta" => Some(Tag::Maqueta),
            b"div" => Some(Tag::Div),
            b"span" => Some(Tag::Span),
            b"island" => Some(Tag::Island),
            b"svg" => Some(Tag::Svg),
            b"path" => Some(Tag::Path),
            b"usa" => Some(Tag::Usa),
            b"imagen" => Some(Tag::Imagen),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Tag::Maqueta => "maqueta",
            Tag::Div => "div",
            Tag::Span => "span",
            Tag::Island => "island",
            Tag::Svg => "svg",
            Tag::Path => "path",
            Tag::Usa => "usa",
            Tag::Imagen => "imagen",
        }
    }

    /// An island is filled by another process, so nothing of ours goes inside.
    /// A span holds text and no boxes. See `LA_MAQUETA_EXIGE.md` section 2.
    pub fn takes_boxes(self) -> bool {
        matches!(self, Tag::Maqueta | Tag::Div)
    }

    /// Only `<svg>` takes `<path>`s, and a `<path>` goes nowhere else.
    pub fn takes_paths(self) -> bool {
        matches!(self, Tag::Svg)
    }
}

/// The properties. The first sixteen were counted from what `scene/` does;
/// the rest (MAQUETA 2, 04-10) from what the app maquetas actually use --
/// `docs/arte/maqueta_bankcat.html` and `maqueta_hermes.html`, measured.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prop {
    // the box
    Width,
    Height,
    Padding,
    // the paint
    BackgroundColor,
    Color,
    BorderWidth,
    BorderColor,
    BorderRadius,
    // the placement
    Display,
    FlexDirection,
    Gap,
    JustifyContent,
    AlignItems,
    // the absolute placement
    Position,
    Left,
    Top,
    /// H5 (04-10): desde el borde derecho / de abajo de su ancla.
    Right,
    Bottom,
    // the letter (MAQUETA 2): measured on the host with `bmo-letra`
    FontSize,
    FontWeight,
    LetterSpacing,
    LineHeight,
    TextTransform,
    /// H3 (04-10): `normal` = un PARRAFO, partido en lineas al compilar.
    WhiteSpace,
    /// H7 (04-10): `auto` = la caja se DESPLAZA (lo de dentro pasa de su alto).
    OverflowY,
    // the finish (MAQUETA 2)
    BoxShadow,
    BackgroundImage,
    // the drawing (MAQUETA 2): on `<svg>`, for its `<path>`s
    Stroke,
    StrokeWidth,
    Fill,
    /// Solo `round`: la pluma de la casa es redonda, y el navegador tiene que
    /// dibujar con la misma o la regla mentiria.
    StrokeLinecap,
    StrokeLinejoin,
    // the sides (MAQUETA 2, escalon 1): a padding and a border per side,
    // because the maquetas draw a row's separator as `border-bottom`.
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    BorderTopWidth,
    BorderRightWidth,
    BorderBottomWidth,
    BorderLeftWidth,
    BorderTopColor,
    BorderRightColor,
    BorderBottomColor,
    BorderLeftColor,
    // the motion (P3, 04-10): how this box goes from one state to another.
    Transition,
}

/// The four sides, in CSS order (top, right, bottom, left).
pub const LADOS: [&str; 4] = ["top", "right", "bottom", "left"];

impl Prop {
    /// The padding of side `k` (CSS order).
    pub fn padding_de(k: usize) -> Prop {
        [Prop::PaddingTop, Prop::PaddingRight, Prop::PaddingBottom, Prop::PaddingLeft][k]
    }

    /// The border width of side `k`.
    pub fn grosor_de(k: usize) -> Prop {
        [Prop::BorderTopWidth, Prop::BorderRightWidth, Prop::BorderBottomWidth, Prop::BorderLeftWidth][k]
    }

    /// The border colour of side `k`.
    pub fn color_de(k: usize) -> Prop {
        [Prop::BorderTopColor, Prop::BorderRightColor, Prop::BorderBottomColor, Prop::BorderLeftColor][k]
    }

    /// Which side a per-side property speaks of, if it is one.
    pub fn lado(self) -> Option<usize> {
        (0..4).find(|&k| self == Prop::padding_de(k) || self == Prop::grosor_de(k) || self == Prop::color_de(k))
    }

    pub fn from_name(b: &[u8]) -> Option<Prop> {
        Some(match b {
            b"width" => Prop::Width,
            b"height" => Prop::Height,
            b"padding" => Prop::Padding,
            b"background-color" => Prop::BackgroundColor,
            b"color" => Prop::Color,
            b"border-width" => Prop::BorderWidth,
            b"border-color" => Prop::BorderColor,
            b"border-radius" => Prop::BorderRadius,
            b"display" => Prop::Display,
            b"flex-direction" => Prop::FlexDirection,
            b"gap" => Prop::Gap,
            b"justify-content" => Prop::JustifyContent,
            b"align-items" => Prop::AlignItems,
            b"position" => Prop::Position,
            b"left" => Prop::Left,
            b"top" => Prop::Top,
            b"right" => Prop::Right,
            b"bottom" => Prop::Bottom,
            b"font-size" => Prop::FontSize,
            b"font-weight" => Prop::FontWeight,
            b"letter-spacing" => Prop::LetterSpacing,
            b"line-height" => Prop::LineHeight,
            b"text-transform" => Prop::TextTransform,
            b"white-space" => Prop::WhiteSpace,
            b"overflow-y" => Prop::OverflowY,
            b"box-shadow" => Prop::BoxShadow,
            b"background-image" => Prop::BackgroundImage,
            b"stroke" => Prop::Stroke,
            b"stroke-width" => Prop::StrokeWidth,
            b"fill" => Prop::Fill,
            b"stroke-linecap" => Prop::StrokeLinecap,
            b"stroke-linejoin" => Prop::StrokeLinejoin,
            b"padding-top" => Prop::PaddingTop,
            b"padding-right" => Prop::PaddingRight,
            b"padding-bottom" => Prop::PaddingBottom,
            b"padding-left" => Prop::PaddingLeft,
            b"border-top-width" => Prop::BorderTopWidth,
            b"border-right-width" => Prop::BorderRightWidth,
            b"border-bottom-width" => Prop::BorderBottomWidth,
            b"border-left-width" => Prop::BorderLeftWidth,
            b"border-top-color" => Prop::BorderTopColor,
            b"border-right-color" => Prop::BorderRightColor,
            b"border-bottom-color" => Prop::BorderBottomColor,
            b"border-left-color" => Prop::BorderLeftColor,
            b"transition" => Prop::Transition,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Prop::Width => "width",
            Prop::Height => "height",
            Prop::Padding => "padding",
            Prop::BackgroundColor => "background-color",
            Prop::Color => "color",
            Prop::BorderWidth => "border-width",
            Prop::BorderColor => "border-color",
            Prop::BorderRadius => "border-radius",
            Prop::Display => "display",
            Prop::FlexDirection => "flex-direction",
            Prop::Gap => "gap",
            Prop::JustifyContent => "justify-content",
            Prop::AlignItems => "align-items",
            Prop::Position => "position",
            Prop::Left => "left",
            Prop::Top => "top",
            Prop::Right => "right",
            Prop::Bottom => "bottom",
            Prop::FontSize => "font-size",
            Prop::FontWeight => "font-weight",
            Prop::LetterSpacing => "letter-spacing",
            Prop::LineHeight => "line-height",
            Prop::TextTransform => "text-transform",
            Prop::WhiteSpace => "white-space",
            Prop::OverflowY => "overflow-y",
            Prop::BoxShadow => "box-shadow",
            Prop::BackgroundImage => "background-image",
            Prop::Stroke => "stroke",
            Prop::StrokeWidth => "stroke-width",
            Prop::Fill => "fill",
            Prop::StrokeLinecap => "stroke-linecap",
            Prop::StrokeLinejoin => "stroke-linejoin",
            Prop::PaddingTop => "padding-top",
            Prop::PaddingRight => "padding-right",
            Prop::PaddingBottom => "padding-bottom",
            Prop::PaddingLeft => "padding-left",
            Prop::BorderTopWidth => "border-top-width",
            Prop::BorderRightWidth => "border-right-width",
            Prop::BorderBottomWidth => "border-bottom-width",
            Prop::BorderLeftWidth => "border-left-width",
            Prop::BorderTopColor => "border-top-color",
            Prop::BorderRightColor => "border-right-color",
            Prop::BorderBottomColor => "border-bottom-color",
            Prop::BorderLeftColor => "border-left-color",
            Prop::Transition => "transition",
        }
    }

    /// Does this property only change how a box LOOKS, never where it is?
    ///
    /// ** This is what makes `:hover` admissible. A hover rule may only touch
    /// these four, and therefore **cannot move a single box** -- so the layout is
    /// still computed exactly once, and `layout/` never learns that hover exists.
    /// Let hover set a `width` and the whole model breaks: you would need a
    /// second layout per state, in the aparato, at frame time.
    pub fn es_pintura(self) -> bool {
        matches!(
            self,
            Prop::BackgroundColor
                | Prop::Color
                | Prop::BorderColor
                | Prop::BorderRadius
                | Prop::BoxShadow
                | Prop::BackgroundImage
                | Prop::Stroke
                | Prop::Fill
                | Prop::BorderTopColor
                | Prop::BorderRightColor
                | Prop::BorderBottomColor
                | Prop::BorderLeftColor
        )
    }

    /// What shape of value this property accepts. Knowing that is naming, which
    /// is this generation's whole job.
    pub fn shape(self) -> Shape {
        match self {
            Prop::Width
            | Prop::Height
            | Prop::BorderRadius
            | Prop::Gap
            | Prop::Left
            | Prop::Top
            | Prop::Right
            | Prop::Bottom
            | Prop::FontSize
            | Prop::LineHeight
            | Prop::PaddingTop
            | Prop::PaddingRight
            | Prop::PaddingBottom
            | Prop::PaddingLeft
            | Prop::BorderTopWidth
            | Prop::BorderRightWidth
            | Prop::BorderBottomWidth
            | Prop::BorderLeftWidth => Shape::OnePx,
            Prop::FontWeight => Shape::Weight,
            Prop::LetterSpacing => Shape::Em,
            Prop::TextTransform => Shape::Words(&[Keyword::Uppercase, Keyword::None]),
            Prop::WhiteSpace => Shape::Words(&[Keyword::Normal, Keyword::Nowrap]),
            Prop::OverflowY => Shape::Words(&[Keyword::Auto, Keyword::Visible]),
            Prop::BoxShadow => Shape::Shadow,
            Prop::BackgroundImage => Shape::Gradient,
            Prop::BackgroundColor
            | Prop::BorderColor
            | Prop::BorderTopColor
            | Prop::BorderRightColor
            | Prop::BorderBottomColor
            | Prop::BorderLeftColor => Shape::ColorOrClear,
            Prop::Stroke | Prop::Fill => Shape::ColorOrNone,
            Prop::StrokeWidth => Shape::Fine,
            Prop::StrokeLinecap | Prop::StrokeLinejoin => Shape::Words(&[Keyword::Round]),
            Prop::Padding | Prop::BorderWidth => Shape::OneToFourPx,
            Prop::Transition => Shape::Transition,
            Prop::Color => Shape::Color,
            Prop::Display => Shape::Words(&[Keyword::Block, Keyword::Flex]),
            Prop::FlexDirection => Shape::Words(&[Keyword::Row, Keyword::Column]),
            Prop::JustifyContent => Shape::Words(&[
                Keyword::Start,
                Keyword::Center,
                Keyword::End,
                Keyword::SpaceBetween,
            ]),
            Prop::AlignItems => Shape::Words(&[
                Keyword::Stretch,
                Keyword::Start,
                Keyword::Center,
                Keyword::End,
            ]),
            Prop::Position => Shape::Words(&[Keyword::Absolute, Keyword::Relative]),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    OnePx,
    /// One to four lengths, as CSS reads them: `a` (all), `a b` (vertical,
    /// horizontal), `a b c` (top, horizontal, bottom), `a b c d`.
    OneToFourPx,
    Color,
    /// A colour, or `transparent` (paint nothing).
    ColorOrClear,
    Words(&'static [Keyword]),
    /// `400`, `500`, `600`, `700` (or `normal`, `bold`).
    Weight,
    /// `.14em` (or `0`): thousandths of the font size.
    Em,
    /// `0 0 14px #RRGGBBAA`: only the glow around a box, no offset.
    Shadow,
    /// `linear-gradient(90deg, #RRGGBB, #RRGGBB)`, or `180deg`: two colours.
    Gradient,
    /// A colour, or `none`.
    ColorOrNone,
    /// A number that may carry a decimal (`1.5`), in `viewBox` units.
    Fine,
    /// `240ms ease-in-out`, `.3s cubic-bezier(.34,1.56,.64,1) 50ms`.
    Transition,
}

/// **Como va una caja de un estado a otro**: cuanto tarda, cuanto espera y
/// con que curva (los cuatro puntos de un `cubic-bezier` de CSS, en
/// milesimas: `x1 y1 x2 y2`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Transicion {
    pub ms: u32,
    pub retraso: u32,
    pub curva: [i32; 4],
}

impl Transicion {
    /// Las curvas con nombre de CSS, con sus puntos de la especificacion.
    pub fn curva_de(nombre: &[u8]) -> Option<[i32; 4]> {
        Some(match nombre {
            b"linear" => [0, 0, 1000, 1000],
            b"ease" => [250, 100, 250, 1000],
            b"ease-in" => [420, 0, 1000, 1000],
            b"ease-out" => [0, 0, 580, 1000],
            b"ease-in-out" => [420, 0, 580, 1000],
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keyword {
    Block,
    Flex,
    Row,
    Column,
    Start,
    Center,
    End,
    SpaceBetween,
    Stretch,
    Absolute,
    /// H5 (04-10): no se mueve; es el ANCLA de sus absolutas.
    Relative,
    Uppercase,
    None,
    Round,
    /// `white-space: normal` (H3): el texto se parte en lineas.
    Normal,
    /// `white-space: nowrap`: una sola linea (lo de siempre en MAQUETA).
    Nowrap,
    /// `overflow-y: auto` (H7): se desplaza.
    Auto,
    /// `overflow-y: visible`: lo de siempre (no se desplaza).
    Visible,
    /// `border-radius: 50%` (H6): la mitad del lado corto -- un circulo en
    /// una caja cuadrada. El unico porcentaje que hay.
    Mitad,
}

impl Keyword {
    pub fn from_name(b: &[u8]) -> Option<Keyword> {
        Some(match b {
            b"block" => Keyword::Block,
            b"flex" => Keyword::Flex,
            b"row" => Keyword::Row,
            b"column" => Keyword::Column,
            b"start" => Keyword::Start,
            b"center" => Keyword::Center,
            b"end" => Keyword::End,
            b"space-between" => Keyword::SpaceBetween,
            b"stretch" => Keyword::Stretch,
            b"absolute" => Keyword::Absolute,
            b"relative" => Keyword::Relative,
            b"normal" => Keyword::Normal,
            b"nowrap" => Keyword::Nowrap,
            b"auto" => Keyword::Auto,
            b"visible" => Keyword::Visible,
            b"uppercase" => Keyword::Uppercase,
            b"none" => Keyword::None,
            b"round" => Keyword::Round,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Keyword::Block => "block",
            Keyword::Flex => "flex",
            Keyword::Row => "row",
            Keyword::Column => "column",
            Keyword::Start => "start",
            Keyword::Center => "center",
            Keyword::End => "end",
            Keyword::SpaceBetween => "space-between",
            Keyword::Stretch => "stretch",
            Keyword::Absolute => "absolute",
            Keyword::Relative => "relative",
            Keyword::Normal => "normal",
            Keyword::Nowrap => "nowrap",
            Keyword::Auto => "auto",
            Keyword::Visible => "visible",
            Keyword::Mitad => "50%",
            Keyword::Uppercase => "uppercase",
            Keyword::None => "none",
            Keyword::Round => "round",
        }
    }
}

/// A resolved value. Everything is an integer number of pixels or a packed
/// `0x00RRGGBB`, because that is what BMO-X draws with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Value {
    Px(u32),
    /// top, right, bottom, left -- CSS order, so the browser preview agrees.
    Px4([u32; 4]),
    Color(u32),
    Word(Keyword),
    /// `font-weight`: 400 to 700.
    Weight(u16),
    /// Thousandths of an em.
    Em(i32),
    /// The glow: how far it reaches, and `0xAARRGGBB` (alpha = strength).
    Shadow { reach: u32, argb: u32 },
    /// Two colours, left to right (`90deg`) or top to bottom (`180deg`).
    Gradient { vertical: bool, from: u32, to: u32 },
    /// `none` for `stroke` and `fill`.
    Nothing,
    /// A fine number, in 1/64 units.
    Fine(u32),
    Transicion(Transicion),
}

// ------------------------------------------------------------------------
//  The rejection table
// ------------------------------------------------------------------------

/// Real CSS that MAQUETA does not have, each with **why** and **instead**.
///
/// * This table is the difference between a compiler and a toy. "unknown
/// property `box-shadow`" tells the author nothing; naming the reason and the
/// way out is what makes rejecting better than ignoring. Without it, the
/// inversion this project is built on -- *a compiler rejects what it does not
/// understand* -- just leaves people stuck.
pub fn known_rejection(name: &[u8]) -> Option<(&'static str, &'static str)> {
    Some(match name {
        b"opacity" | b"filter" | b"backdrop-filter" => (
            "transparentar una caja ENTERA obliga a pintarla aparte y mezclarla \
             encima, en el aparato. La mezcla que si hay es la del borde (letra, \
             esquina, trazo) y la de `box-shadow`.",
            "elegir el color ya mezclado: `#RRGGBB` resuelto al escribirlo.",
        ),
        b"z-index" => (
            "no hay capas: las cajas se pintan en el orden en que estan escritas.",
            "mover la caja en el fichero. El orden del texto ES el orden de pintado.",
        ),
        b"overflow" | b"overflow-x" | b"overflow-y" => (
            "nada se recorta ni se desplaza: una caja que no cabe es un ERROR del \
             veredicto, no un caso a manejar en ejecucion.",
            "dar sitio a la caja, o repartir con `display:flex` y `gap`.",
        ),
        b"float" | b"clear" => (
            "el flotado existe para rodear texto con imagenes, y aqui no hay ninguna \
             de las dos cosas.",
            "`display:flex` con `flex-direction:row`.",
        ),
        b"font-family" | b"font" => (
            "hay UNA letra, la de la casa (`bmo-letra`): trazos propios, sin \
             fuentes de nadie. Elegir familia seria traer otra.",
            "`font-size`, `font-weight`, `letter-spacing`, `line-height` y \
             `text-transform`: lo que de verdad cambia una letra.",
        ),
        b"text-align" => (
            "no esta implementada, y no es gratis: alinear texto es colocar una caja \
             dentro de otra, o sea maquetacion.",
            "meter el texto en su `<span>` y colocarlo con `justify-content`.",
        ),
        b"animation" | b"transform" | b"@keyframes" => (
            "en el aparato no se maqueta NADA: cada estado se maqueta entero al \
             compilar y se juzga. Una animacion por fotogramas o una transformacion \
             libre pedirian maquetar en ejecucion.",
            "estados y transiciones: `@estado abierta { ... }` y `transition: 240ms \
             ease` en la caja. Lo que vive de verdad (un juego, un grafico) es Rust \
             o TITAN++.",
        ),
        b"margin" | b"margin-top" | b"margin-left" | b"margin-right" | b"margin-bottom" => (
            "los margenes verticales de CSS se FUNDEN entre hermanos (dos de 10px pegados dan 10, no 20), y MAQUETA no va a implementar esa regla. Aceptarla sin fundirlos haria que el fichero se viera distinto en el navegador que en el Ryzen, que es justo lo que el guardian de la cascada existe para impedir.",
            "`gap` dentro de un `display:flex`, o `padding` en el contenedor. Las dos cubren todos los casos contados en `scene/`.",
        ),
        b"grid" | b"grid-template-columns" | b"grid-template-rows" | b"grid-area" => (
            "`grid` no esta: `flex` cubre todo lo que hace `scene/` hoy, contado.",
            "`display:flex` anidado. Una rejilla de N columnas son N `<div>` en una \
             fila, y filas en una columna.",
        ),
        b"inherit" | b"initial" | b"unset" => (
            "no hay herencia: en MAQUETA una pieza no sabe que tiene padre (L7), y \
             eso es lo que mantiene al padre sin conocer a sus ancestros.",
            "declarar el valor donde hace falta. Con estilos de ambito corto, \
             repetirlo cuesta menos que la regla que lo evitaba.",
        ),
        _ => return None,
    })
}

// ------------------------------------------------------------------------
//  Errors this file produces
// ------------------------------------------------------------------------

pub fn unknown_tag(span: Span, name: &[u8]) -> Error {
    let n = String::from_utf8_lossy(name).into_owned();
    let promises: &[&str] = &[
        "h1", "h2", "h3", "p", "button", "a", "ul", "li", "table", "img", "input", "form",
    ];
    if promises.contains(&n.as_str()) {
        return Error::new(
            span,
            &format!("etiqueta no soportada -- `<{n}>`"),
            "esa etiqueta PROMETE semantica que MAQUETA no tiene (papel, foco, \
             navegacion, tipografia). Aceptarla y no honrarla seria la mentira que \
             este compilador existe para evitar.",
            "`<div>` o `<span>`: son las dos unicas etiquetas de HTML que no \
             prometen nada, que es justo lo que hay aqui.",
        );
    }
    Error::new(
        span,
        &format!("etiqueta no soportada -- `<{n}>`"),
        "la lista de etiquetas esta CERRADA, y lo que no esta en ella no compila.",
        "`<maqueta>`, `<div>`, `<span>`, `<island>`, `<svg>`, `<path>` o \
         `<style>`. La lista entera esta en la seccion 2 de `LA_MAQUETA_EXIGE.md`.",
    )
}

pub fn unknown_prop(span: Span, name: &[u8]) -> Error {
    let n = String::from_utf8_lossy(name).into_owned();
    if let Some((why, instead)) = known_rejection(name) {
        return Error::new(span, &format!("propiedad no soportada -- `{n}`"), why, instead);
    }
    Error::new(
        span,
        &format!("propiedad no soportada -- `{n}`"),
        "la lista de propiedades esta CERRADA: contadas sobre lo que el escritorio \
         y las maquetas de las apps hacen de verdad.",
        "la lista entera esta en la seccion 3 de `LA_MAQUETA_EXIGE.md`. Agregar una \
         empieza por anadirla ahi.",
    )
}
