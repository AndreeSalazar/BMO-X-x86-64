//! **K. Un dibujo que el navegador no pintaria igual** (MAQUETA 2, 04-10).
//!
//! La regla de MAQUETA es el navegador: el mismo `.maqueta` se abre alli y se
//! compara. Un `<svg>` tiene dos trampas en las que el navegador hace OTRA
//! cosa que BMO-X sin que nadie lo vea:
//!
//! ```text
//!    sin `fill`               el navegador rellena de NEGRO; BMO-X no rellena
//!    pluma sin `round`        el navegador corta las puntas en recto y las
//!                             esquinas en pico; la pluma de la casa es redonda
//! ```
//!
//! Las dos se piden DICHAS, y las dos son una linea en la regla.

use bmo_maqueta_diag::Error;
use bmo_maqueta_layout::{Frame, Laid};

pub fn check(laid: &Laid, out: &mut Vec<Error>) {
    for f in laid.all() {
        // Por su nombre: el veredicto no depende del padre para esto.
        if f.tag.name() == "svg" {
            un_dibujo(f, out);
        }
    }
}

fn un_dibujo(f: &Frame, out: &mut Vec<Error>) {
    let s = &f.style;
    if f.view_box.is_none() {
        out.push(Error::new(
            f.span,
            "este `<svg>` no tiene `viewBox`",
            "sin el no se sabe en que coordenadas estan sus caminos, y el navegador \
             y BMO-X los pondrian en sitios distintos.",
            "`viewBox=\"0 0 24 24\"` (o las del dibujo).",
        ));
    }
    if !s.fill_said {
        out.push(Error::new(
            f.span,
            "este `<svg>` no dice su `fill`",
            "sin `fill` el navegador RELLENA DE NEGRO cada camino, y BMO-X no rellena \
             nada: la regla mentiria sin que se viera.",
            "`fill: none` (solo trazo) o `fill: #RRGGBB`.",
        ));
    }
    if s.stroke.is_some() && !(s.round_cap && s.round_join) {
        out.push(Error::new(
            f.span,
            "la pluma de este `<svg>` no es redonda",
            "la pluma de la casa es REDONDA (puntas y esquinas); sin decirlo, el \
             navegador corta en recto y en pico, y las dos fotos no se parecerian.",
            "`stroke-linecap: round; stroke-linejoin: round`.",
        ));
    }
}
