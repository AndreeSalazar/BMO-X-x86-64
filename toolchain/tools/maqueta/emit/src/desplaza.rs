//! **LO QUE SE DESPLAZA** (H7, 04-10): `overflow-y: auto`.
//!
//! Una caja con `height` y `overflow-y: auto` es una VENTANA: lo de dentro
//! se maqueta y se juzga ENTERO al compilar, aunque pase de su alto. En el
//! aparato no se maqueta nada -- se mueve UN numero:
//!
//! ```text
//!    1. se limpia la ventana con su fondo (liso: lo exige el veredicto, J)
//!    2. lo de dentro se pinta corrido `desde` pixeles hacia arriba,
//!       recortado a la ventana
//!    3. encima, la barra: su largo es lo que se ve de lo que hay
//! ```
//!
//! La foto del anfitrion (`foto.rs`) y el codigo generado (`rust.rs`) salen
//! de AQUI: lo de dentro, la ventana, la medida y la barra son una cuenta.

use bmo_maqueta_layout::{Frame, Laid, Rect};

use crate::orden::{lista, Estado, Orden, Trazo};

/// Las cajas que se desplazan, en orden de pintado.
pub fn cajas(l: &Laid) -> Vec<&Frame> {
    l.all().into_iter().filter(|f| f.style.desplaza).collect()
}

/// La maquetacion SIN lo de dentro de las cajas que se desplazan: eso se
/// pinta aparte, recortado.
pub fn podar(l: &Laid) -> Laid {
    fn ir(f: &mut Frame) {
        if f.style.desplaza {
            f.children.clear();
        }
        f.children.iter_mut().for_each(ir);
    }
    let mut p = l.clone();
    ir(&mut p.root);
    p
}

/// **La ventana**: la caja por dentro del borde (con su `padding`), como
/// recorta CSS.
pub fn ventana(f: &Frame) -> Rect {
    let [t, r, b, l] = f.style.border_width;
    Rect { x: f.rect.x + l as i32, y: f.rect.y + t as i32, w: f.rect.w.saturating_sub(l + r), h: f.rect.h.saturating_sub(t + b) }
}

/// El radio con el que se limpia la ventana: el de la caja (sin borde, la
/// ventana ES la caja), menos el borde si lo hay.
pub fn radio(f: &Frame) -> i32 {
    (f.style.border_radius as i32 - f.style.border_width[0] as i32).max(0)
}

/// **Lo que mide todo lo de dentro**, desde arriba de la ventana: hasta el
/// fondo de lo mas bajo, mas el `padding` de abajo. Nunca menos que la
/// ventana.
pub fn total(f: &Frame) -> u32 {
    let v = ventana(f);
    let fondo = f.children.iter().map(|c| c.rect.bottom()).max().unwrap_or(v.y as i64);
    ((fondo - v.y as i64).max(0) as u32 + f.style.padding[2]).max(v.h)
}

/// Lo mas que se puede bajar.
pub fn maximo(f: &Frame) -> u32 {
    total(f) - ventana(f).h
}

/// **Lo de dentro**, en reposo y como piezas (un `Rect` es una caja de radio
/// 0: los mismos pixeles, y se deja recortar).
pub fn contenido(f: &Frame, lienzo: (u32, u32)) -> Vec<Orden> {
    f.children
        .iter()
        .flat_map(|c| lista(&Laid { root: c.clone(), canvas: lienzo }))
        .filter(|o| o.estado == Estado::Reposo)
        .map(|mut o| {
            if let Trazo::Rect { r, color } = o.trazo {
                o.trazo = Trazo::Caja { r, radio: 0, color };
            }
            o
        })
        .collect()
}

/// El ancho de la barra, y lo que se separa del borde.
pub const BARRA: u32 = 4;
const ORILLA: u32 = 3;

/// **La barra a `desde`**: su caja. `None` si cabe todo (no hay que
/// desplazar, y no se pinta). Su largo es lo que se ve de lo que hay, nunca
/// menos de 18 px.
pub fn barra(f: &Frame, desde: u32) -> Option<Rect> {
    let (v, t) = (ventana(f), total(f));
    if t <= v.h || v.h < 2 * ORILLA + 18 {
        return None;
    }
    let pista = v.h - 2 * ORILLA;
    let largo = (pista * v.h / t).max(18);
    let y = v.y + ORILLA as i32 + (desde.min(t - v.h) * (pista - largo) / (t - v.h)) as i32;
    Some(Rect { x: v.x + v.w as i32 - (BARRA + ORILLA) as i32, y, w: BARRA, h: largo })
}

/// El color de la barra: el fondo de la ventana, aclarado.
pub fn color_barra(f: &Frame) -> u32 {
    let fondo = f.style.background.unwrap_or(0);
    bmo_pinta::entre_color(fondo, 0x00FF_FFFF, 260)
}
