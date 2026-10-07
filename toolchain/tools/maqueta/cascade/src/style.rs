//! **What a resolved style is, and how one declaration changes it.**
//!
//! ## Absence is not `auto`
//!
//! `width` comes out as `Option<u32>`, and `None` does **not** mean the CSS
//! keyword `auto` -- that is still refused by the father. It means *this file
//! did not say*, and deciding what an unsaid width becomes is a question about
//! a box **next to other boxes**, which is the grandson's job (`layout/`).
//!
//! Resolving it here would be this generation reaching for something it cannot
//! see. Recording the absence is the honest move, and it is what lets the
//! grandson give a block box its parent's width and a flex item its content's
//! width without either rule living in two places.
//!
//! ## * There is no default text colour, and that is deliberate
//!
//! Without inheritance, a default `color` would be inheritance from nowhere: a
//! value nobody wrote, that looks intentional, and that ages into a lie exactly
//! like `INFO_ES_ESCRIBIBLE => 0` did. So `color` is `None` until someone says
//! it, and text that never got one is a finding for `verdict/`.
//!
//! It costs one word in the markup -- `class="ink"` -- and the palette it points
//! at is `tema/tema.maqueta`. That is the Arch bargain the whole project is
//! built on: nothing implicit, and the explicit thing is one readable line.

use bmo_maqueta_node::{Decl, Keyword, Prop, Transicion, Value};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Display {
    #[default]
    Block,
    Flex,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Direction {
    #[default]
    Row,
    Column,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

/// * `Stretch` is the default because **CSS's default is `stretch`**, and this
/// was very nearly a divergence nobody would have noticed: with `Start` as the
/// default, a flex row would size its items to their content here and stretch
/// them to the container in the browser. Same family as the ordering guardian --
/// a preview that lies -- found in a different place.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Align {
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Position {
    #[default]
    Static,
    Absolute,
    /// H5 (04-10): en su sitio del flujo, y ANCLA de sus absolutas.
    Relative,
}

/// `text-align` (MAQUETA 3): where the text sits inside its `block` box.
/// `Left` is CSS's default for left-to-right text.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// `overflow-x` / `overflow-y` (MAQUETA 3, MA2). `Auto` en `y` es la caja que
/// se desplaza (H7).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Desborde {
    #[default]
    Visible,
    /// Recorta; si el otro eje es `visible`, el navegador lo DESPLAZARIA.
    Hidden,
    /// Recorta, y el otro eje se queda como esta.
    Clip,
    Auto,
}

impl Desborde {
    pub fn recorta(self) -> bool {
        matches!(self, Desborde::Hidden | Desborde::Clip)
    }
}

/// `align-content` (MA2): donde caen las filas partidas. `Stretch` es el
/// `normal` de CSS.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AlignContent {
    #[default]
    Stretch,
    Start,
    Center,
    End,
    SpaceBetween,
}

/// Every one of the sixteen properties, resolved.
///
/// `Option` means *nobody said*; everything else carries the value CSS uses when
/// nobody says, so that the browser preview and MAQUETA start from the same
/// place.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Style {
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// top, right, bottom, left.
    pub padding: [u32; 4],
    /// `None` = paint nothing. A box that is only a container is legitimate.
    pub background: Option<u32>,
    /// `None` = nobody said. See the module header: there is no default.
    pub color: Option<u32>,
    /// top, right, bottom, left (escalon 1: `border-bottom` es la raya que
    /// separa dos filas en las maquetas).
    pub border_width: [u32; 4],
    /// `None` = ese lado no se pinta (aunque ocupe su grosor).
    pub border_color: [Option<u32>; 4],
    pub border_radius: u32,
    /// `border-radius: 50%` (H6): la caja lo resuelve al maquetar, con su
    /// medida (`layout`); despues `border_radius` ya lleva el numero.
    pub radio_mitad: bool,
    pub display: Display,
    pub direction: Direction,
    pub gap: u32,
    pub justify: Justify,
    pub align: Align,
    pub position: Position,
    /// Con signo desde MAQUETA 3 (MA2): una absoluta se sale de su ancla a
    /// proposito.
    pub left: Option<i32>,
    pub top: Option<i32>,
    pub right: Option<i32>,
    pub bottom: Option<i32>,
    // -- MAQUETA 2: the letter. `font_size` None = the 8x16 pixel letter.
    pub font_size: Option<u32>,
    /// 400, 500, 600, 700 (0 = not said: 400).
    pub font_weight: u16,
    /// Thousandths of an em.
    pub letter_spacing: i32,
    pub line_height: Option<u32>,
    pub uppercase: bool,
    /// `white-space: normal` (H3): un parrafo, partido en lineas de `width`.
    pub parrafo: bool,
    /// `overflow-y: auto` (H7): lo de dentro puede pasar de su alto, y se
    /// desplaza.
    pub desplaza: bool,
    // -- MAQUETA 2: the finish.
    /// The glow: reach in px and `0xAARRGGBB`.
    pub shadow: Option<(u32, u32)>,
    /// Two colours and the axis (`true` = top to bottom).
    pub gradient: Option<(u32, u32, bool)>,
    // -- MAQUETA 2: the drawing (on `<svg>`).
    pub stroke: Option<u32>,
    /// In 1/64 of a `viewBox` unit; 0 = not said (1 unit).
    pub stroke_width: u32,
    pub fill: Option<u32>,
    /// Si el `<svg>` declaro `fill` (aunque sea `none`): sin declararlo, el
    /// navegador rellena de NEGRO, y la regla no puede mentir.
    pub fill_said: bool,
    /// `stroke-linecap` y `stroke-linejoin`, si se dijeron (MAQUETA 3: las
    /// tres puntas y las tres esquinas de SVG).
    pub linecap: Option<Keyword>,
    pub linejoin: Option<Keyword>,
    // -- P3 (04-10): como va esta caja de un estado a otro. `None` = de golpe.
    pub transicion: Option<Transicion>,
    // -- MAQUETA 3, pila A (06-10): resueltas con la medida PROPIA.
    pub text_align: TextAlign,
    /// `min-width` / `max-width` / `min-height` / `max-height`. `None` = sin
    /// cota. Las aplica el nieto (`layout::measure::sujeta`).
    pub min_w: Option<u32>,
    pub max_w: Option<u32>,
    pub min_h: Option<u32>,
    pub max_h: Option<u32>,
    // -- MAQUETA 3, pila A, segunda tanda (06-10, MA2).
    /// `overflow-x` y `overflow-y`: `[x, y]`.
    pub desborde: [Desborde; 2],
    /// `text-overflow: ellipsis`: la linea que no cabe se corta con `...`.
    pub puntos: bool,
    /// `white-space: nowrap` DICHO. Sin decirlo, en el navegador un texto que
    /// no cabe baja de linea (su `normal`); MAQUETA no deja que no quepa
    /// (B), salvo con los puntos: alli hace falta decirlo (N).
    pub una_linea: bool,
    /// `cursor`, si se dijo: declara una zona del puntero.
    pub cursor: Option<Keyword>,
    /// `pointer-events`: `Some(true)` = `none`, `Some(false)` = `auto`. Se
    /// hereda (como en CSS), y eso lo resuelve quien recorre el arbol.
    pub sin_puntero: Option<bool>,
    /// `z-index`: la CAPA.
    pub capa: Option<u32>,
    /// `flex-wrap: wrap`.
    pub parte: bool,
    /// `align-content`, si se dijo.
    pub align_content: Option<AlignContent>,
    /// `align-self`: el `align-items` de esta caja como hijo.
    pub align_self: Option<Align>,
    /// `aspect-ratio`: ancho y alto.
    pub proporcion: Option<(u32, u32)>,
    /// `outline`: grosor y color (sin contorno, `None`).
    pub contorno: Option<(u32, u32)>,
    /// `outline-offset`.
    pub contorno_aparte: i32,
}

impl Style {
    /// **El borde, si es el MISMO en los cuatro lados**: su grosor y su color.
    /// `None` si algun lado difiere -- entonces se pinta lado a lado.
    pub fn borde_uniforme(&self) -> Option<(u32, Option<u32>)> {
        let w = self.border_width[0];
        let c = self.border_color[0];
        (self.border_width.iter().all(|&x| x == w) && self.border_color.iter().all(|&x| x == c)).then_some((w, c))
    }

    /// Fold one declaration in. Later calls overwrite earlier ones, which is the
    /// whole of MAQUETA's cascade: **last wins**. What makes that safe is the
    /// guardian in `guard.rs`.
    pub fn apply(&mut self, d: &Decl) {
        match (d.prop, d.value) {
            (Prop::Width, Value::Px(n)) => self.width = Some(n),
            (Prop::Height, Value::Px(n)) => self.height = Some(n),
            (Prop::Padding, Value::Px4(v)) => self.padding = v,
            (Prop::BackgroundColor, Value::Color(c)) => self.background = Some(c),
            (Prop::BackgroundColor, Value::Nothing) => self.background = None,
            (Prop::Color, Value::Color(c)) => self.color = Some(c),
            (Prop::BorderWidth, Value::Px4(v)) => self.border_width = v,
            (Prop::BorderColor, Value::Color(c)) => self.border_color = [Some(c); 4],
            (Prop::BorderColor, Value::Nothing) => self.border_color = [None; 4],
            (p, Value::Px(n)) if p.lado().is_some() && matches!(p, Prop::PaddingTop | Prop::PaddingRight | Prop::PaddingBottom | Prop::PaddingLeft) => {
                self.padding[p.lado().unwrap_or(0)] = n
            }
            (p, Value::Px(n)) if p.lado().is_some() => self.border_width[p.lado().unwrap_or(0)] = n,
            (p, Value::Color(c)) if p.lado().is_some() => self.border_color[p.lado().unwrap_or(0)] = Some(c),
            (p, Value::Nothing) if p.lado().is_some() => self.border_color[p.lado().unwrap_or(0)] = None,
            (Prop::BorderRadius, Value::Px(n)) => {
                self.border_radius = n;
                self.radio_mitad = false;
            }
            (Prop::BorderRadius, Value::Word(Keyword::Mitad)) => self.radio_mitad = true,
            (Prop::Gap, Value::Px(n)) => self.gap = n,
            (Prop::Left, Value::Signed(n)) => self.left = Some(n),
            (Prop::Top, Value::Signed(n)) => self.top = Some(n),
            (Prop::Right, Value::Signed(n)) => self.right = Some(n),
            (Prop::Bottom, Value::Signed(n)) => self.bottom = Some(n),

            (Prop::Display, Value::Word(Keyword::Block)) => self.display = Display::Block,
            (Prop::Display, Value::Word(Keyword::Flex)) => self.display = Display::Flex,
            (Prop::FlexDirection, Value::Word(Keyword::Row)) => self.direction = Direction::Row,
            (Prop::FlexDirection, Value::Word(Keyword::Column)) => {
                self.direction = Direction::Column
            }
            (Prop::JustifyContent, Value::Word(k)) => {
                self.justify = match k {
                    Keyword::Center => Justify::Center,
                    Keyword::End => Justify::End,
                    Keyword::SpaceBetween => Justify::SpaceBetween,
                    _ => Justify::Start,
                }
            }
            (Prop::AlignItems, Value::Word(k)) => {
                self.align = match k {
                    Keyword::Center => Align::Center,
                    Keyword::End => Align::End,
                    Keyword::Start => Align::Start,
                    _ => Align::Stretch,
                }
            }
            (Prop::Position, Value::Word(Keyword::Absolute)) => {
                self.position = Position::Absolute
            }
            (Prop::Position, Value::Word(Keyword::Relative)) => {
                self.position = Position::Relative
            }
            (Prop::FontSize, Value::Px(n)) => self.font_size = Some(n),
            (Prop::FontWeight, Value::Weight(w)) => self.font_weight = w,
            (Prop::LetterSpacing, Value::Em(e)) => self.letter_spacing = e,
            (Prop::LineHeight, Value::Px(n)) => self.line_height = Some(n),
            (Prop::TextTransform, Value::Word(k)) => self.uppercase = k == Keyword::Uppercase,
            (Prop::WhiteSpace, Value::Word(k)) => {
                self.parrafo = k == Keyword::Normal;
                self.una_linea = k == Keyword::Nowrap;
            }
            (Prop::OverflowY, Value::Word(k)) => {
                self.desplaza = k == Keyword::Auto;
                self.desborde[1] = desborde(k);
            }
            (Prop::OverflowX, Value::Word(k)) => self.desborde[0] = desborde(k),
            (Prop::TextOverflow, Value::Word(k)) => self.puntos = k == Keyword::Ellipsis,
            (Prop::Cursor, Value::Word(k)) => self.cursor = Some(k),
            (Prop::PointerEvents, Value::Word(k)) => self.sin_puntero = Some(k == Keyword::None),
            (Prop::ZIndex, Value::Count(n)) => self.capa = Some(n),
            (Prop::FlexWrap, Value::Word(k)) => self.parte = k == Keyword::Wrap,
            (Prop::AlignContent, Value::Word(k)) => {
                self.align_content = Some(match k {
                    Keyword::Start => AlignContent::Start,
                    Keyword::Center => AlignContent::Center,
                    Keyword::End => AlignContent::End,
                    Keyword::SpaceBetween => AlignContent::SpaceBetween,
                    _ => AlignContent::Stretch,
                })
            }
            (Prop::AlignSelf, Value::Word(k)) => {
                self.align_self = match k {
                    Keyword::Start => Some(Align::Start),
                    Keyword::Center => Some(Align::Center),
                    Keyword::End => Some(Align::End),
                    Keyword::Stretch => Some(Align::Stretch),
                    _ => None,
                }
            }
            (Prop::AspectRatio, Value::Ratio(a, b)) => self.proporcion = Some((a, b)),
            (Prop::Outline, Value::Outline { w, color }) => self.contorno = (w > 0).then_some((w, color)),
            (Prop::OutlineOffset, Value::Signed(n)) => self.contorno_aparte = n,
            (Prop::BoxShadow, Value::Shadow { reach, argb }) => self.shadow = Some((reach, argb)),
            (Prop::BackgroundImage, Value::Gradient { vertical, from, to }) => {
                self.gradient = Some((from, to, vertical))
            }
            (Prop::BackgroundImage, Value::Nothing) => self.gradient = None,
            (Prop::Stroke, Value::Color(c)) => self.stroke = Some(c),
            (Prop::Stroke, Value::Nothing) => self.stroke = None,
            (Prop::StrokeWidth, Value::Fine(w)) => self.stroke_width = w,
            (Prop::Fill, Value::Color(c)) => {
                self.fill = Some(c);
                self.fill_said = true;
            }
            (Prop::Fill, Value::Nothing) => {
                self.fill = None;
                self.fill_said = true;
            }
            (Prop::StrokeLinecap, Value::Word(k)) => self.linecap = Some(k),
            (Prop::StrokeLinejoin, Value::Word(k)) => self.linejoin = Some(k),
            (Prop::Transition, Value::Transicion(t)) => self.transicion = Some(t),
            (Prop::TextAlign, Value::Word(k)) => {
                self.text_align = match k {
                    Keyword::Center => TextAlign::Center,
                    Keyword::Right | Keyword::End => TextAlign::Right,
                    _ => TextAlign::Left,
                }
            }
            (Prop::MinWidth, Value::Px(n)) => self.min_w = Some(n),
            (Prop::MaxWidth, Value::Px(n)) => self.max_w = Some(n),
            (Prop::MinHeight, Value::Px(n)) => self.min_h = Some(n),
            (Prop::MaxHeight, Value::Px(n)) => self.max_h = Some(n),

            // The father checked every shape before this generation saw it, so
            // no other pairing exists. Ignoring rather than panicking keeps a
            // bug in one generation from taking down the next -- and if one ever
            // arrives, `verdict/` sees a style that is simply missing a value.
            _ => {}
        }
    }
}

fn desborde(k: Keyword) -> Desborde {
    match k {
        Keyword::Hidden => Desborde::Hidden,
        Keyword::Clip => Desborde::Clip,
        Keyword::Auto => Desborde::Auto,
        _ => Desborde::Visible,
    }
}
