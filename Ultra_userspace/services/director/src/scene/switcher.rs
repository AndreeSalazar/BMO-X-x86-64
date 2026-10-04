//! **El conmutador de ventanas** -- la ventanita de Alt+Tab.
//!
//! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! La politica vive en `bmo_foco::focus` y **se prueba alli**; aqui solo se
//! pinta lo que esa politica ya decidio. Es el mismo reparto de siempre: quien
//! decide no dibuja, y quien dibuja no decide.
//!
//! === Por que hay que ensenarlo y no basta con conmutar ===
//!
//! Sin esta ventanita, Alt+Tab es adivinar. Con dos ventanas se aguanta; con
//! tres ya no se sabe cuantos Tabs faltan, y el resultado es pulsar de mas y
//! acabar donde no querias. Mostrar **la lista y cual esta marcada** convierte
//! un atajo de memoria en uno que se mira.
//!
//! Es lo que Eddi pidio con estas palabras: *"requiere la chica ventana que
//! hace ver todo para facilitar que prioridad le da, porque si no chocan"*.

use bmo_userland as bmo;

use super::*;
use crate::ventana::Ventana;

// ** LA CARA (04-10): la ventanita es una TARJETA de `fino.rs` -- la
// curva, la sombra y el hilo de laton --, con la letra de la casa. Era un
// marco de dos pixeles con `> ` delante de la elegida: se leia como una
// consola. Las medidas, en pixeles:
//
//    margen 24 | el rotulo (base a 30) | el filete (a 42) | las filas (de
//    34, desde 52) | el filete | el modo y la ayuda (dos lineas de 20) | 16
const SW_W: u32 = 400;
const MARGEN: u32 = 24;
const ROW_H: u32 = 34;
const FILAS_Y: u32 = 52;
const PIE_H: u32 = 20;
/// Las filas que caben; si hay mas ventanas, la ultima dice cuantas faltan.
const MAX_FILAS: usize = 10;

/// Lo que habia debajo, para repintar sin mezclar dos veces: la tarjeta con
/// su sombra, del alto que tiene con [`MAX_FILAS`].
const GUARDADO: usize = ((SW_W + 2 * fino::HALO) * (FILAS_Y + MAX_FILAS as u32 * ROW_H + 16 + 2 * PIE_H + 16 + 2 * fino::HALO)) as usize;

struct Debajo {
    px: [u32; GUARDADO],
    caja: (u32, u32, u32, u32),
    puesto: bool,
}

static mut DEBAJO: Debajo = Debajo { px: [0; GUARDADO], caja: (0, 0, 0, 0), puesto: false };

fn debajo() -> &'static mut Debajo {
    // SAFETY: el director es un solo hilo, y esto solo lo toca el conmutador.
    unsafe { &mut *core::ptr::addr_of_mut!(DEBAJO) }
}

/// **Olvida lo guardado** sin devolverlo: lo de debajo ya no es lo que se
/// guardo (se pinto una ventana encima, o se repinto todo al soltar Alt).
pub(crate) fn olvidar() {
    debajo().puesto = false;
}

/// El nombre de cada ventana.
///
/// ** LA TABLA YA NO ESTA AQUI, y ese es el arreglo. Estuvo, con un `_ =>
/// "?"` al final, y mintio dos veces: CABINA y Sonido salieron como `?` por
/// no ampliarla al nacer, y la ventana de CPU se anunciaba como "Sonido"
/// porque compartia el id 3 con ella.
///
/// El sintoma de una tabla que se queda corta es suave y por eso dura: Alt+Tab
/// funciona, conmuta bien, y solo miente en el nombre. Ahora los nombres son
/// de `Ventana`, donde el `match` es EXHAUSTIVO --sin `_`-- y una ventana
/// nueva no compila hasta que tiene el suyo. Aqui solo queda la traduccion
/// desde el id que guarda la politica, y el `?` que ya no puede pasar.
pub(crate) fn name(id: u8) -> &'static str {
    match Ventana::de_id(id) {
        Some(v) => v.nombre(),
        None => "?",
    }
}

/// El rectangulo del conmutador. **Una sola cuenta**, porque la usan dos.
///
/// `paint` y `area` la tenian copiada, con un comentario que advertia justo de
/// esto. Mientras las dos copias fueran identicas daba igual; en cuanto una
/// crece --la fila de la ayuda de las flechas-- la otra borra un rectangulo mas
/// corto que el pintado y deja una franja de la ventanita pegada en el
/// escritorio hasta el siguiente repintado.
fn run_box(p: &bmo::Pantalla, count: usize) -> (u32, u32, u32, u32) {
    let filas = count.min(MAX_FILAS) as u32;
    let height = FILAS_Y + ROW_H * filas + 16 + 2 * PIE_H + 16;
    let width = SW_W.min(p.ancho.saturating_sub(40 + 2 * fino::HALO));
    (
        (p.ancho.saturating_sub(width)) / 2,
        (p.alto.saturating_sub(height)) / 2,
        width,
        height,
    )
}

/// Pinta el conmutador centrado, con la marcada resaltada.
pub(crate) fn paint(p: &bmo::Pantalla, lista: &[u8], pointed_at: usize, modo: &str) {
    if lista.is_empty() {
        return;
    }
    let (x, y, width, height) = run_box(p, lista.len());
    guardar(p, (x, y, width, height));

    fino::tarjeta(p, x, y, width, height);
    let dentro = width - 2 * MARGEN;
    fino::rotulo(p, x + MARGEN, y + 30, b"Ventanas");
    fino::filete(p, x + MARGEN, y + 42, dentro);

    // Si no caben todas, la ventana marcada SIEMPRE se ve: la lista corre
    // para que quede dentro, y la ultima fila dice cuantas faltan.
    let caben = lista.len().min(MAX_FILAS);
    let desde = if pointed_at >= caben { pointed_at + 1 - caben } else { 0 };
    let mut fy = y + FILAS_Y;
    for (i, &v) in lista.iter().enumerate().skip(desde).take(caben) {
        if i == pointed_at {
            // La marca va de borde a borde: una a media anchura se lee como
            // "hay mas columnas" y no las hay.
            fino::marca(p, x + MARGEN - 8, fy + 2, dentro + 16, ROW_H - 4, acento());
            fino::texto(p, x + MARGEN + 18, fy, ROW_H, name(v).as_bytes(), fino::TINTA, fino::CUERPO_FIRME);
        } else {
            fino::texto(p, x + MARGEN + 18, fy, ROW_H, name(v).as_bytes(), fino::TENUE, fino::CUERPO);
        }
        fy += ROW_H;
    }
    if lista.len() > caben {
        let mut n = [0u8; 16];
        let k = cuantas_mas(&mut n, lista.len() - caben);
        let w = p.medir(&n[..k], fino::PIE) as u32;
        fino::texto(p, x + width - MARGEN - w, fy - ROW_H, ROW_H, &n[..k], fino::TENUE, fino::PIE);
    }
    fy += 8;
    fino::filete(p, x + MARGEN, fy, dentro);
    fy += 8;

    // El modo, abajo: sin esto no hay forma de saber por que el foco se
    // comporta distinto de lo que esperabas. Y con el la tecla que lo cambia:
    // un modo que se lee pero no se toca invita a pensar que esta averiado.
    let mx = x + MARGEN;
    let mx = mx + fino::texto(p, mx, fy, PIE_H, b"modo  ", fino::TENUE, fino::PIE);
    let mx = mx + fino::texto(p, mx, fy, PIE_H, modo.as_bytes(), acento(), fino::PIE);
    fino::texto(p, mx, fy, PIE_H, b"    Ctrl+Tab", fino::TENUE, fino::PIE);
    fy += PIE_H;

    // ** Las flechas se anuncian AQUI y no en el pie de cada ventana.
    //
    // Porque este es el unico momento en que la mano ya tiene el Alt pulsado:
    // se lee la frase con el dedo puesto en la tecla que hace falta. En el pie
    // de CABINA seria una linea mas que se lee una vez y se olvida, y ademas
    // habria que repetirla en las tres ventanas -- tres sitios que actualizar
    // cuando el atajo cambie.
    let hx = x + MARGEN;
    let hx = hx + fino::texto(p, hx, fy, PIE_H, b"Ctrl+flechas  ", fino::TENUE, fino::PIE);
    let hx = hx + fino::texto(p, hx, fy, PIE_H, b"encajar", fino::TINTA, fino::PIE);
    let hx = hx + fino::texto(p, hx, fy, PIE_H, b"     +Shift  ", fino::TENUE, fino::PIE);
    fino::texto(p, hx, fy, PIE_H, b"mover", fino::TINTA, fino::PIE);
}

/// `+N mas`, sin formato (no hay `alloc`). Devuelve cuantos bytes.
fn cuantas_mas(b: &mut [u8; 16], n: usize) -> usize {
    let mut k = 0;
    b[k] = b'+';
    k += 1;
    let mut d = [0u8; 8];
    let (mut m, mut j) = (n.min(9_999_999), 0);
    loop {
        d[j] = b'0' + (m % 10) as u8;
        j += 1;
        m /= 10;
        if m == 0 {
            break;
        }
    }
    while j > 0 {
        j -= 1;
        b[k] = d[j];
        k += 1;
    }
    for &c in b" mas" {
        b[k] = c;
        k += 1;
    }
    k
}

/// **Devuelve lo de debajo y guarda lo nuevo.** La tarjeta MEZCLA su borde y
/// su sombra con lo que hay: repintarla encima de si misma (cada Tab) los
/// oscureceria. Asi cada pintado empieza sobre lo que habia antes del
/// primero.
fn guardar(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32)) {
    let d = debajo();
    if d.puesto {
        let (gx, gy, gw, gh) = d.caja;
        p.marcar(gx, gy, gw, gh);
        for fy in 0..gh {
            for fx in 0..gw {
                p.punto_ya_marcado(gx + fx, gy + fy, d.px[(fy * gw + fx) as usize]);
            }
        }
    }
    let (gx, gy) = (x.saturating_sub(fino::HALO), y.saturating_sub(fino::HALO));
    let gw = (w + 2 * fino::HALO).min(p.ancho.saturating_sub(gx));
    let gh = (h + 2 * fino::HALO).min(p.alto.saturating_sub(gy));
    if (gw * gh) as usize > GUARDADO {
        d.puesto = false;
        return;
    }
    p.sincronizar_lectura();
    for fy in 0..gh {
        for fx in 0..gw {
            d.px[(fy * gw + fx) as usize] = p.read(gx + fx, gy + fy);
        }
    }
    d.caja = (gx, gy, gw, gh);
    d.puesto = true;
}

/// Que rectangulo ocupo, para poder borrarlo despues.
pub(crate) fn area(p: &bmo::Pantalla, count: usize) -> (u32, u32, u32, u32) {
    // Con la sombra: lo que se borra al soltar Alt es todo lo que se pinto.
    let (x, y, w, h) = run_box(p, count);
    let (gx, gy) = (x.saturating_sub(fino::HALO), y.saturating_sub(fino::HALO));
    (gx, gy, (w + 2 * fino::HALO).min(p.ancho.saturating_sub(gx)), (h + 2 * fino::HALO).min(p.alto.saturating_sub(gy)))
}
