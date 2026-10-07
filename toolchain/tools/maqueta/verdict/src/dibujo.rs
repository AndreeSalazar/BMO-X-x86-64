//! **K. Un dibujo que el navegador no pintaria igual** (MAQUETA 2, 04-10;
//! MAQUETA 3, 06-10).
//!
//! La regla de MAQUETA es el navegador: el mismo `.maqueta` se abre alli y se
//! compara. Hasta MAQUETA 3, K pedia el `fill` dicho y la pluma `round`: el
//! pintor no rellenaba por defecto ni sabia otra pluma, y el navegador si.
//! Ya sabe (`docs/componente/LA_MAQUETA_EXIGE.md`, 2e). Lo que queda de K:
//!
//! ```text
//!    un viewBox    el suyo, o el de su fichero: sin el no se sabe en que
//!                  coordenadas esta el dibujo
//!    una medida    la de su regla o la de su fichero: sin ninguna el
//!                  navegador le da 300 x 150 por su cuenta
//!    el dibujo     lo que el lector de SVG dice al pintarlo EN su caja y con
//!                  lo que hereda de su regla: un `url(#...)` roto, una
//!                  opacidad de grupo, una animacion sin ciclo
//! ```
//!
//! **Y una tercera, de cualquier caja (escalon 1)**: un borde DISTINTO por
//! lado en una caja redonda, con resplandor o con degradado. Las piezas
//! suaves pintan un anillo de grosor constante, y un navegador cose cuatro
//! lados de grosores distintos alrededor de la curva -- otro dibujo.

use bmo_maqueta_diag::Error;
use bmo_maqueta_layout::{Frame, Laid};

pub fn check(laid: &Laid, out: &mut Vec<Error>) {
    for f in laid.all() {
        // Por su nombre: el veredicto no depende del padre para esto.
        if f.tag.name() == "svg" {
            un_dibujo(f, out);
        }
        let s = &f.style;
        let suave = s.border_radius > 0 || s.shadow.is_some() || s.gradient.is_some();
        if suave && s.borde_uniforme().is_none() {
            out.push(Error::new(
                f.span,
                "esta caja suave tiene un borde distinto por lado",
                "con `border-radius`, `box-shadow` o un degradado la caja se pinta con las piezas suaves, que dibujan UN anillo de grosor y color constantes. Cuatro lados distintos alrededor de una curva los cose el navegador a su manera, y la foto de BMO-X no se pareceria.",
                "el mismo borde en los cuatro lados (`border: 1px solid #...`), o la caja sin radio, sombra ni degradado.",
            ));
        }
    }
}

fn un_dibujo(f: &Frame, out: &mut Vec<Error>) {
    // Sin dibujo: un `<svg src>` cuyo fichero no se leyo, y eso ya se dijo.
    let Some(d) = &f.dibujo else { return };
    if d.vista_o_medida().is_none() {
        out.push(Error::new(
            f.span,
            "este `<svg>` no tiene `viewBox`",
            "sin el no se sabe en que coordenadas estan sus figuras, y el navegador \
             y BMO-X las pondrian en sitios distintos.",
            "`viewBox=\"0 0 24 24\"` (o las del dibujo).",
        ));
        return;
    }
    if f.content.w == 0 || f.content.h == 0 {
        out.push(Error::new(
            f.span,
            "este `<svg>` no tiene medida",
            "un dibujo mide lo que dice su regla (o su fichero); sin nada, el navegador \
             le da 300 x 150 por su cuenta y BMO-X no pinta nada.",
            "`width` y `height` en su regla.",
        ));
        return;
    }
    let h = bmo_maqueta_cascade::herencia(&f.style);
    let c = f.content;
    let caja = (c.x as f64, c.y as f64, c.w as f64, c.h as f64);
    let (_, e) = bmo_maqueta_dibujo::figuras(d, &h, caja);
    out.extend(e);
    if let Some(Err(e)) = bmo_maqueta_dibujo::pasos(d, &h, caja) {
        out.extend(e);
    }
}
