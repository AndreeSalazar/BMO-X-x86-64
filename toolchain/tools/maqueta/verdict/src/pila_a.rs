//! **Las comprobaciones de la PILA A** (MAQUETA 3, 06-10, MA2): M a S de
//! `LA_MAQUETA_EXIGE.md` 3e. Cada una caza la forma en que una propiedad de la
//! pila A se veria distinta en el navegador que en el aparato, o no haria nada.

use bmo_maqueta_cascade::{Align, Desborde, Direction, Display, Position};
use bmo_maqueta_diag::Error;
use bmo_maqueta_layout::{Frame, Laid, Rect};

/// Lo que recorta, en coordenadas de lienzo: `(x0, y0, x1, y1)`; un eje sin
/// recorte va de `i64::MIN` a `i64::MAX`.
type Corte = (i64, i64, i64, i64);

const TODO: Corte = (i64::MIN, i64::MIN, i64::MAX, i64::MAX);

/// **La caja que recorta lo de dentro de `f`**: por dentro del borde, en los
/// ejes que recorta. `None` si no recorta nada.
pub fn corte_de(f: &Frame) -> Option<Corte> {
    let [rx, ry] = [f.style.desborde[0].recorta(), f.style.desborde[1].recorta()];
    if !rx && !ry {
        return None;
    }
    let [t, r, b, l] = f.style.border_width;
    let (x0, x1) = (f.rect.x as i64 + l as i64, f.rect.right() - r as i64);
    let (y0, y1) = (f.rect.y as i64 + t as i64, f.rect.bottom() - b as i64);
    Some((if rx { x0 } else { i64::MIN }, if ry { y0 } else { i64::MIN }, if rx { x1 } else { i64::MAX }, if ry { y1 } else { i64::MAX }))
}

fn cortar(a: Corte, b: Corte) -> Corte {
    (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3))
}

fn entero(r: &Rect, c: Corte) -> bool {
    r.x as i64 >= c.0 && r.y as i64 >= c.1 && r.right() <= c.2 && r.bottom() <= c.3
}

fn toca(r: &Rect, c: Corte) -> bool {
    (r.x as i64) < c.2 && r.right() > c.0 && (r.y as i64) < c.3 && r.bottom() > c.1
}

pub fn check(laid: &Laid, out: &mut Vec<Error>) {
    andar(&laid.root, None, TODO, false, false, false, laid.canvas, out);
}

/// `padre`: la caja de arriba (para R). `corte`: lo que recortan las de
/// arriba. `sin_ancla`: dentro de un recorte que no es ancla, sin ninguna
/// posicionada entre medias (M). `en_ventana`: dentro de una que se
/// desplaza (M). `en_capa`: dentro de una caja con `z-index` (P).
#[allow(clippy::too_many_arguments)]
fn andar(f: &Frame, padre: Option<&Frame>, corte: Corte, sin_ancla: bool, en_ventana: bool, en_capa: bool, lienzo: (u32, u32), out: &mut Vec<Error>) {
    let s = &f.style;
    let posicionada = s.position != Position::Static;

    // M. Una absoluta anclada FUERA de la caja que recorta: en CSS no la
    // recortaria (su caja contenedora esta por encima), aqui si.
    if sin_ancla && s.position == Position::Absolute {
        out.push(Error::new(
            f.span,
            "esta absoluta se ancla fuera de la caja que la recorta",
            "en CSS un `overflow: hidden` solo recorta a las absolutas cuya ancla \
             esta dentro de el; esta se ancla mas arriba, asi que el navegador la \
             pintaria entera y aqui saldria cortada.",
            "`position: relative` en la caja que recorta (asi es su ancla), o sacar \
             la absoluta de ella.",
        ));
    }
    let propio = corte_de(f);
    if propio.is_some() && en_ventana {
        out.push(Error::new(
            f.span,
            "un recorte dentro de una caja que se desplaza",
            "la ventana (H7) pinta lo de dentro corrido en el aparato, y el recorte \
             se resuelve al compilar contra donde cae SIN correr.",
            "recortar fuera de la ventana, o dar sitio a lo de dentro.",
        ));
    }
    // M. Un eje `hidden` con el otro `visible`: CSS convierte el otro en
    // `auto`, y si algo se sale por ahi, el navegador lo desplazaria.
    if let Some(eje) = un_eje_que_se_desplazaria(f) {
        out.push(Error::new(
            f.span,
            &format!("con `overflow-{}: hidden` el otro eje se desplazaria, y algo se sale por el", if eje == 0 { "y" } else { "x" }),
            "en CSS, si un eje recorta y el otro es `visible`, el otro pasa a `auto`: \
             el navegador pondria una barra que aqui no hay.",
            "`overflow: hidden` en los dos ejes, o `clip` (que no desplaza).",
        ));
    }
    // El texto de PIXEL no se corta a medias: la letra 8 x 16 no sabe recortar.
    let mio = propio.map_or(corte, |p| cortar(corte, p));
    if let (Some(_), Some(r), None) = (&f.text, f.text_at, s.font_size) {
        if !entero(&r, mio) && toca(&r, mio) {
            out.push(Error::new(
                f.span,
                "este texto de pixel quedaria cortado a medias por un `overflow`",
                "la letra de pixel (8 x 16) se pinta entera o no se pinta: no sabe \
                 recortar. La letra de la casa si.",
                "darle `font-size` (la letra de la casa), sitio, o `text-overflow: ellipsis`.",
            ));
        }
    }

    // N. Los tres puntos sin recorte, o sin decir que es UNA linea.
    if s.puntos && (!s.desborde[0].recorta() || !s.una_linea) {
        out.push(Error::new(
            f.span,
            "`text-overflow: ellipsis` aqui no corta nada",
            "en CSS los puntos salen solo si la MISMA caja recorta (`overflow: \
             hidden`) y el texto no baja de linea (`white-space: nowrap`): sin \
             decirlo, el navegador lo parte en dos lineas.",
            "`overflow: hidden` y `white-space: nowrap` en esta caja, o quitar `text-overflow`.",
        ));
    }

    // O. Una zona que se pulsa y nadie puede nombrar.
    if s.cursor.map(|k| k.name()) == Some("pointer") && f.id.is_none() {
        out.push(Error::new(
            f.span,
            "`cursor: pointer` en una caja sin `id`",
            "la mano dice \"esto se pulsa\", y una zona sin nombre no la puede \
             atender nadie: seria un boton que no hace nada.",
            "darle un `id` (es su nombre en la tabla de golpes), o quitar el cursor.",
        ));
    }

    // P. La capa: solo en una posicionada, y nunca una dentro de otra.
    if s.capa.is_some() {
        if !posicionada {
            out.push(Error::new(
                f.span,
                "`z-index` en una caja estatica",
                "en CSS `z-index` no hace nada en una caja sin `position`; aqui la \
                 moveria de capa, y el navegador y el aparato pintarian distinto.",
                "`position: relative` (o `absolute`) en la caja, o quitar `z-index`.",
            ));
        }
        if en_capa {
            out.push(Error::new(
                f.span,
                "una capa dentro de otra",
                "en CSS seria un contexto de apilado dentro de otro, y su numero solo \
                 cuenta DENTRO del de arriba. Aqui la capa es un numero global.",
                "quitar uno de los dos `z-index`.",
            ));
        }
    }

    // Q. Partir filas: contra la medida PROPIA.
    if s.parte {
        let sin_medida = match s.direction {
            Direction::Row => s.width.is_none(),
            Direction::Column => s.height.is_none(),
        };
        if s.display != Display::Flex || sin_medida {
            out.push(Error::new(
                f.span,
                "`flex-wrap: wrap` necesita `display: flex` y la medida contra la que se parte",
                "las filas se parten AL COMPILAR contra la medida de la caja; sin \
                 decirla, en el navegador saldria de su padre y aqui no se sabe.",
                "`width` en una fila (o `height` en una columna).",
            ));
        }
    }
    if s.align_content.is_some() && !s.parte {
        out.push(Error::new(
            f.span,
            "`align-content` sin `flex-wrap: wrap`",
            "reparte las FILAS partidas; con una sola fila no hace nada.",
            "agregar `flex-wrap: wrap`, o quitar `align-content`.",
        ));
    }

    // R. La proporcion: con una medida sobra, y no en un hijo que se estira.
    if s.proporcion.is_some() {
        if s.width.is_some() && s.height.is_some() {
            out.push(Error::new(
                f.span,
                "`aspect-ratio` con `width` y `height` dichos",
                "con las dos medidas la proporcion no decide nada.",
                "quitar una de las tres.",
            ));
        }
        if let Some(p) = padre.filter(|p| p.style.display == Display::Flex && s.position != Position::Absolute) {
            let cruzada = if p.style.direction == Direction::Row { s.height } else { s.width };
            if cruzada.is_none() && s.align_self.unwrap_or(p.style.align) == Align::Stretch {
                out.push(Error::new(
                    f.span,
                    "`aspect-ratio` en un hijo flex que se estiraria",
                    "con `stretch` el navegador estira la medida cruzada y la proporcion \
                     pelea con ella; el resultado depende del navegador.",
                    "darle su medida cruzada, o `align-self: start` (o `center`).",
                ));
            }
        }
    }

    // S. El contorno no ocupa sitio, pero tiene que caber en el lienzo.
    if let Some((w, _)) = s.contorno {
        let d = s.contorno_aparte as i64 + w as i64;
        let (x0, y0) = (f.rect.x as i64 - d, f.rect.y as i64 - d);
        let (x1, y1) = (f.rect.right() + d, f.rect.bottom() + d);
        if x0 < 0 || y0 < 0 || x1 > lienzo.0 as i64 || y1 > lienzo.1 as i64 {
            out.push(Error::new(
                f.span,
                &format!("el contorno se sale del lienzo -- va de ({x0}, {y0}) a ({x1}, {y1})"),
                "el contorno se pinta FUERA de la caja y no ocupa sitio: lo que se \
                 sale del lienzo no se ve, en ningun sitio.",
                "dejar sitio alrededor de la caja, o un `outline-offset` negativo.",
            ));
        }
    }

    let sin_ancla_dentro = match propio {
        Some(_) => !posicionada,
        None => sin_ancla && !posicionada,
    };
    for c in &f.children {
        andar(c, Some(f), mio, sin_ancla_dentro, en_ventana || s.desplaza, en_capa || s.capa.is_some(), lienzo, out);
    }
}

/// El eje (0 = x, 1 = y) que el navegador desplazaria: el `visible` frente
/// a un `hidden`, si algo de dentro se sale por el.
fn un_eje_que_se_desplazaria(f: &Frame) -> Option<usize> {
    let d = f.style.desborde;
    let eje = match (d[0], d[1]) {
        (Desborde::Hidden, Desborde::Visible) => 1,
        (Desborde::Visible, Desborde::Hidden) => 0,
        _ => return None,
    };
    let [t, r, b, l] = f.style.border_width;
    let sale = f.children.iter().any(|c| {
        if eje == 0 {
            (c.rect.x as i64) < f.rect.x as i64 + l as i64 || c.rect.right() > f.rect.right() - r as i64
        } else {
            (c.rect.y as i64) < f.rect.y as i64 + t as i64 || c.rect.bottom() > f.rect.bottom() - b as i64
        }
    });
    sale.then_some(eje)
}
