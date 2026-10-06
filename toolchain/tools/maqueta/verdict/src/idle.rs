//! **Hay algo escrito que no hace nada?**
//!
//! Ninguna de estas tres rompe la imagen. Son errores igualmente, y por la regla
//! que ordena el proyecto entero: *nada que compile y no haga lo que dice*.
//!
//! Una linea aceptada que no se honra es la mentira que envejece sin avisar --
//! la misma clase que `INFO_ES_ESCRIBIBLE => 0`, un valor puesto por prudencia
//! que tres meses despues era falso y nadie se entero.

use bmo_maqueta_cascade::{Display, Position};
use bmo_maqueta_diag::Error;
use bmo_maqueta_layout::{Frame, Laid};

pub fn check(laid: &Laid, out: &mut Vec<Error>) {
    for f in laid.all() {
        gap_sin_flex(f, out);
        absoluta_sin_sitio(f, out);
        relativa_que_se_mueve(f, out);
        texto_sin_color(f, out);
        alineado_que_no_alinea(f, out);
    }
}

/// L. (MAQUETA 3, 06-10) Un `text-align` que no puede hacer lo que dice.
///
/// Dos casos, y en los dos el navegador tampoco lo haria -- la diferencia es
/// que aqui se dice:
///
/// - en una caja `flex`, el texto es un elemento flex ANONIMO de su misma
///   medida, asi que no hay sitio dentro donde moverlo. Lo coloca
///   `justify-content`;
/// - en un PARRAFO (`white-space:normal`), cada linea mide distinto y
///   pediria su propia x. Todavia no: se escribe aqui en vez de alinear solo
///   la primera.
fn alineado_que_no_alinea(f: &Frame, out: &mut Vec<Error>) {
    use bmo_maqueta_cascade::TextAlign;
    let s = &f.style;
    if s.text_align == TextAlign::Left || f.text.is_none() {
        return;
    }
    if s.display == Display::Flex {
        out.push(Error::new(
            f.span,
            "`text-align` en una caja `flex` no hace nada",
            "en un contenedor flex el texto es un elemento ANONIMO de su misma \
             medida: no queda sitio dentro donde alinearlo. En el navegador \
             tampoco haria nada.",
            "`justify-content:center` (o `end`) en esta caja, o quitar el `display:flex`.",
        ));
    } else if s.parrafo {
        out.push(Error::new(
            f.span,
            "un parrafo solo se alinea a la izquierda, todavia",
            "cada linea de un parrafo mide distinto, y centrarlo pide una x por \
             linea. Alinear solo la caja entera dejaria las lineas cortas donde \
             el navegador no las pone.",
            "`text-align:left`, o un `<span>` por linea.",
        ));
    }
}

/// G. `gap` en una caja que no reparte nada.
fn gap_sin_flex(f: &Frame, out: &mut Vec<Error>) {
    if f.style.gap == 0 || f.style.display == Display::Flex {
        return;
    }
    out.push(Error::new(
        f.span,
        &format!("`gap:{}px` aqui no hace nada", f.style.gap),
        "`gap` solo separa elementos de un contenedor flex, y esta caja es `block`. \
         En un navegador tampoco haria nada -- la diferencia es que alli no te lo \
         dice nadie y aqui si.",
        "agregar `display:flex`, o quitar el `gap`.",
    ));
}

/// H. `position:absolute` sin decir donde.
fn absoluta_sin_sitio(f: &Frame, out: &mut Vec<Error>) {
    if f.style.position != Position::Absolute {
        return;
    }
    let s = &f.style;
    if (s.left.is_some() || s.right.is_some()) && (s.top.is_some() || s.bottom.is_some()) {
        return;
    }
    out.push(Error::new(
        f.span,
        "una caja absoluta tiene que decir donde va en los dos ejes",
        "salirse del flujo es renunciar a que alguien te coloque. Sin las dos \
         coordenadas la caja cae en el 0 que puso el compilador, que no es una \
         decision de nadie.",
        "`left` o `right`, y `top` o `bottom`, en pixeles, contra su ancla: la \
         caja `position:relative` (o absoluta) mas cercana por arriba, o el lienzo.",
    ));
}

/// I. (H5, 04-10) Una `relative` que pide moverse.
///
/// En CSS, `relative` con `top`/`left` corre la caja SIN mover a sus vecinas:
/// se pinta en un sitio y ocupa otro. Aqui `relative` solo hace de ANCLA de sus
/// absolutas, y una caja que se pinta donde no esta es justo lo que el
/// veredicto existe para no tener.
fn relativa_que_se_mueve(f: &Frame, out: &mut Vec<Error>) {
    let s = &f.style;
    if s.position != Position::Relative || (s.left.is_none() && s.top.is_none() && s.right.is_none() && s.bottom.is_none()) {
        return;
    }
    out.push(Error::new(
        f.span,
        "una caja `relative` no se corre con `top`/`left`/`right`/`bottom`",
        "aqui `position:relative` solo es el ANCLA de las absolutas de dentro. \
         Correrla la pintaria en un sitio y la dejaria ocupando otro.",
        "quitar las coordenadas; si tenia que ir en otro sitio, moverla en el \
         flujo (`padding`, `gap`) o hacerla absoluta.",
    ));
}

/// F. Texto que nadie ha coloreado.
///
/// Consecuencia directa de no tener herencia: sin un color heredado del padre y
/// sin uno por defecto --que seria herencia de ninguna parte-- un texto sin
/// `color` no tiene ninguno. Es el precio del trato, y se cobra aqui en vez de
/// pintarse de un color que nadie eligio.
fn texto_sin_color(f: &Frame, out: &mut Vec<Error>) {
    let Some(t) = &f.text else { return };
    if t.is_empty() || f.style.color.is_some() {
        return;
    }
    out.push(Error::new(
        f.span,
        "este texto no tiene color",
        "MAQUETA no hereda, y no hay color por defecto: seria herencia de ninguna \
         parte -- un valor que nadie escribio, con pinta de intencional, que envejece \
         hacia la mentira. Sin `color` no hay con que pintar estas letras.",
        "una clase de la paleta -- `class=\"ink\"` o `class=\"ink-dim\"`, que estan en \
         `toolchain/tools/maqueta/tema/tema.maqueta` -- o un `color:#RRGGBB` propio.",
    ));
}
