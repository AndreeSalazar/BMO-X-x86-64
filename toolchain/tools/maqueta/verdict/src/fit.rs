//! **Cabe todo?**
//!
//! Las tres comprobaciones que miran los rects ya calculados y no repiten ni una
//! resta. La segunda es la que mas veces va a saltar y la que mas vale.

use bmo_maqueta_cascade::Position;
use bmo_maqueta_diag::Error;
use bmo_maqueta_layout::{Frame, Laid, Rect, GLIFO_ANCHO};

pub fn check(laid: &Laid, out: &mut Vec<Error>) {
    let canvas = Rect {
        x: 0,
        y: 0,
        w: laid.canvas.0,
        h: laid.canvas.1,
    };
    walk(&laid.root, &canvas, out);
}

fn walk(f: &Frame, canvas: &Rect, out: &mut Vec<Error>) {
    for c in &f.children {
        let (limite, quien) = if c.style.position == Position::Absolute {
            (canvas, "el lienzo")
        } else {
            (&f.content, "su padre")
        };
        if !c.rect.inside(limite) {
            out.push(fuera(c, limite, quien));
        }
        walk(c, canvas, out);
    }
    cabe_el_texto(f, out);
    el_hueco(f, out);
    el_parrafo(f, out);
    no_esta_vacia(f, out);
}

/// * E. UN PARRAFO (H3): `white-space: normal` parte el texto en lineas AL
/// COMPILAR, con la letra que lo pinta, contra el `width` de su caja. Sin
/// ancho no hay contra que partir, y sin `font-size` la letra de pixel no se
/// parte. Un dato (`{nombre}`) llega al ejecutar: no se puede partir, se corta.
fn el_parrafo(f: &Frame, out: &mut Vec<Error>) {
    if !f.style.parrafo || f.text.is_none() {
        return;
    }
    let mut falta = Vec::new();
    if f.style.width.is_none() {
        falta.push("un `width` (contra el que se parte)");
    }
    if f.style.font_size.is_none() {
        falta.push("un `font-size` (se parte con la letra de la casa)");
    }
    if f.hueco.is_some() {
        falta.push("un texto que se conozca al compilar (un dato no se parte: se corta)");
    }
    if !falta.is_empty() {
        out.push(Error::new(
            f.span,
            &format!("este parrafo necesita {}", falta.join(", ")),
            "un parrafo se parte en lineas AL COMPILAR, por los espacios, con la \
             misma letra que lo pinta: el aparato no parte nada.",
            "dar a la caja su `width` y su `font-size`.",
        ));
    }
}

/// * D. UN HUECO DE DATOS (H1): el texto llega al ejecutar, y lo que no se
/// conoce no se puede juzgar. Lo que SI se juzga es su caja -- y para que el
/// aparato pueda cortar con `...` sin maquetar nada, la caja tiene que
/// decirlo todo: su ancho (`width`), su letra (`font-size`) y que el texto
/// empieza a la izquierda. Un texto centrado se mueve con lo que mide, y lo
/// que mide no se sabe.
fn el_hueco(f: &Frame, out: &mut Vec<Error>) {
    // Solo los huecos de TEXTO: una `<imagen dato>` mide lo que dice su caja.
    let (Some(nombre), Some(_)) = (&f.hueco, &f.text) else { return };
    let mut falta = Vec::new();
    if f.style.width.is_none() {
        falta.push("un `width` (el ancho donde se corta)");
    }
    if f.style.font_size.is_none() {
        falta.push("un `font-size` (se corta con la letra de la casa)");
    }
    if f.text_at.is_some_and(|t| t.x != f.content.x) {
        falta.push("el texto a la izquierda (centrado se moveria con lo que mide)");
    }
    if !falta.is_empty() {
        out.push(Error::new(
            f.span,
            &format!("el hueco `{{{nombre}}}` necesita {}", falta.join(", ")),
            "su texto llega al ejecutar y no se pudo juzgar. Lo que se juzga es su \
             caja: el aparato escribe ahi y corta con `...` lo que no quepa, sin \
             maquetar nada.",
            "dar a la caja del hueco su `width` y su `font-size`, sin centrar el texto.",
        ));
    }
}

/// A. Una caja fuera de su sitio.
fn fuera(c: &Frame, limite: &Rect, quien: &str) -> Error {
    Error::new(
        c.span,
        &format!(
            "esta caja se sale de {quien} -- esta en ({}, {}) y mide {}x{}, y el sitio \
             va de ({}, {}) a ({}, {})",
            c.rect.x,
            c.rect.y,
            c.rect.w,
            c.rect.h,
            limite.x,
            limite.y,
            limite.right(),
            limite.bottom()
        ),
        "BMO-X no recorta ni desplaza: lo que se sale se pinta encima de lo de al \
         lado, o fuera de la pantalla. No hay `overflow` que lo tape, y por eso esto \
         es un error y no un caso a manejar en ejecucion.",
        "dar sitio al padre (mas `width`/`height`, o menos `padding`), o encoger la \
         caja. Si lo que se queria era salirse a proposito, `position:absolute`.",
    )
}

/// * B. El texto que no cabe.
///
/// Es la unica clase de fallo de este sistema que **se ve bonita en pantalla y
/// esta mal**: un navegador lo esconde reajustando lineas, y BMO-X no puede --
/// la fuente no parte palabras, asi que las letras que sobran se pintan encima
/// del borde y nadie avisa.
fn cabe_el_texto(f: &Frame, out: &mut Vec<Error>) {
    let Some(t) = &f.text else { return };
    if t.is_empty() {
        return;
    }
    // La MISMA medida que la maquetacion: la letra de la casa (si la caja
    // dijo `font-size`) o la de pixel. Medir aqui con otra regla seria un
    // juez que mira otra cosa que la que se pinta.
    let (ancho, alto) = bmo_maqueta_layout::measure::texto(&f.style, t);
    let como = if f.style.font_size.is_some() {
        "con la letra de la casa".to_string()
    } else {
        format!("{} letras x {GLIFO_ANCHO}", t.len())
    };
    if ancho > f.content.w {
        out.push(Error::new(
            f.span,
            &format!("el texto no cabe -- mide {ancho} px ({como}) y la caja da {} px", f.content.w),
            "BMO-X no parte palabras ni reajusta lineas, asi que las letras que \
             sobran se pintan por encima del borde. Un navegador lo esconde y aqui \
             se ve, y por eso es la comprobacion que mas vale: es el unico fallo que \
             queda BONITO en pantalla estando mal.",
            &format!("un `width` de {ancho} px o mas, menos `padding`, o menos texto."),
        ));
    }
    if alto > f.content.h {
        out.push(Error::new(
            f.span,
            &format!("el texto no cabe de alto -- una linea son {alto} px y la caja da {}", f.content.h),
            "la linea mide su `line-height` (o la de la letra de pixel, 16), y no se \
             escala para entrar.",
            &format!("un `height` de {alto} px o mas."),
        ));
    }
}

/// C. Una caja de medida cero.
fn no_esta_vacia(f: &Frame, out: &mut Vec<Error>) {
    if f.rect.w != 0 && f.rect.h != 0 {
        return;
    }
    out.push(Error::new(
        f.span,
        &format!("esta caja mide {}x{} y no se va a ver", f.rect.w, f.rect.h),
        "casi siempre es una propiedad que se olvido, no una intencion. Y como no \
         pinta nada ni ocupa sitio, no hay forma de darse cuenta mirando la pantalla.",
        "declarar `width` y `height`, o meterle contenido que le de medida. Si de \
         verdad no tenia que verse, borrarla.",
    ))
}
