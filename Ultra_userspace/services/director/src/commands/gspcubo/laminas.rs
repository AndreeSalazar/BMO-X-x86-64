//! **LA LAMINA EN SU VENTANA** (Q0a3 de `docs/plan/EL_FOCO.md`, 10-10): si la
//! app que publica una lamina de VERRANO tiene ventana, cada fotograma NUEVO
//! de su lamina lo dibuja la 3060 en un bloque del escritorio de la medida de
//! esa ventana, y la ventana se compone desde ahi (`Surface::fuente`). El
//! cubo de TITAN++ deja de verse solo con `gpu verrano banco inti`: gira en
//! su ventana, como cualquier app.
//!
//! [carril]  AMARILLO  manda dibujos a la 3060 por VERRANO; si algo falla, se
//!                     dice UNA vez y la ventana se queda con lo que pinte la
//!                     app (nada se cuelga)
//! [consumo] LATE      un dibujo por publicacion de la app; sin publicacion
//!                     nueva se mira UNA palabra (`Lamina::leer_si_nueva`)
//!
//! ** EL DIRECTOR HABLA VERRANO (1c de EL_FOCO): aqui no se nombra la 3060
//! -- ni un registro, ni una orden --: un `Frame` a una `Image`
//! (`Backend::draw`) en el `Aparato`, la puerta. La `Image` ES el bloque que
//! se compone (`Opciones::en_la_imagen`): la 3060 escribe ahi, y nada se
//! copia de vuelta.
//!
//! [!] Una ventana de ancho que no sea multiplo de 32 (filas de 128 bytes) o
//! de mas de 1280x720 no se dibuja aqui: se dice y la app sigue con lo suyo.

use bmo_userland as bmo;
use bmo_verrano::lamina::{Lamina, Leido};
use bmo_verrano::{Backend, Frame, Image, Vertex, Viewport};

use super::sm86 as destino;
use crate::desktop::Desktop;

const MAX_VERTICES: usize = 3 * bmo_cubo::tanda::CABEN;
/// El bloque, para la ventana mas grande que se dibuja aqui: 1280x720.
const PIXELES_MAX: usize = 1280 * 720;

struct Estado {
    /// La puerta, abierta UNA vez (la primera lamina con ventana).
    aparato: Option<destino::Aparato<'static>>,
    /// No se pudo (abrir o dibujar): se dijo, y no se reintenta.
    imposible: bool,
    /// El bloque donde dibuja la 3060 y desde donde se compone.
    bloque: Option<bmo::Memoria>,
    /// De que app es lo leido, y su ultima secuencia.
    tid: u32,
    secuencia: u32,
    v: [Vertex; MAX_VERTICES],
}

static mut ESTADO: Estado = Estado { aparato: None, imposible: false, bloque: None, tid: 0, secuencia: 0, v: [Vertex { position: [0.0; 4], color: [0.0; 4] }; MAX_VERTICES] };

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo, y esto solo lo toca `vuelta`.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

fn decir(e: &mut Estado, que: &str) {
    e.imposible = true;
    bmo::consola("[Q0a3] la lamina NO va a su ventana: ");
    bmo::consola(que);
    bmo::consola("\n");
}

/// **Cada vuelta del escritorio**: si hay una lamina con ventana y un
/// fotograma nuevo, lo dibuja la 3060 en el bloque y la ventana pasa a
/// componerse de ahi. `true` si dibujo (hay que pintar).
pub(crate) fn vuelta(dsk: &mut Desktop, p: &bmo::Pantalla) -> bool {
    let e = estado();
    if e.imposible {
        return false;
    }
    let Some(t) = dsk.table.lamina() else {
        e.tid = 0;
        return false;
    };
    let Some(i) = dsk.table.ventana_de(t.tid) else { return false };
    if dsk.table.minimizada(i) || crate::commands::gspcomputo::la_3060_lista().is_none() {
        return false;
    }
    let Some((w, h)) = dsk.table.medida_de(i) else { return false };
    if t.tid != e.tid {
        e.tid = t.tid;
        e.secuencia = 0;
    }
    // SAFETY: `tomar_prestado_de` mapeo `bytes` desde `base` en este proceso y
    // la lamina vive hasta que su app muere (`reap_dead`, en este hilo). Se
    // ve como palabras ATOMICAS: la app las escribe a la vez.
    let palabras = unsafe { core::slice::from_raw_parts(t.base as *const core::sync::atomic::AtomicU32, (t.bytes / 4) as usize) };
    let Ok(l) = Lamina::abrir(palabras) else { return false };
    let (s, leido) = l.leer_si_nueva(e.secuencia, &mut e.v);
    e.secuencia = s;
    let Leido::Fotograma { vertices: n, .. } = leido else { return false };
    if w % 32 != 0 || (w as usize) * (h as usize) > PIXELES_MAX {
        decir(e, "su ventana no mide un multiplo de 32 de ancho o pasa de 1280x720");
        return false;
    }
    if e.bloque.is_none() {
        e.bloque = bmo::Memoria::residente(4 * PIXELES_MAX as u64);
        if e.bloque.is_none() {
            decir(e, "sin memoria para el bloque");
            return false;
        }
    }
    if e.aparato.is_none() {
        let Some(m) = bmo::Memoria::residente(destino::CAJA) else {
            decir(e, "sin memoria para el paquete");
            return false;
        };
        // SAFETY: residente y OLVIDADA (no se devuelve nunca: el aparato vive
        // lo que el escritorio), `CAJA` bytes de este proceso, y solo la usa
        // este aparato.
        let caja: &'static mut [u8] = unsafe { core::slice::from_raw_parts_mut(m.base() as *mut u8, destino::CAJA as usize) };
        core::mem::forget(m);
        let op = destino::Opciones { en_la_imagen: true, ligero: true, ..destino::Opciones::default() };
        match destino::abrir(dsk, p, caja, op) {
            Ok((mut a, _)) => {
                a.leer = false;
                e.aparato = Some(a);
            }
            Err(_) => {
                decir(e, "la puerta no se abrio (lo dice la salida)");
                return false;
            }
        }
    }
    let (Some(a), Some(b)) = (e.aparato.as_mut(), e.bloque.as_ref()) else { return false };
    let va = b.base() as u64;
    // SAFETY: el bloque residente mide `PIXELES_MAX` pixeles y `w * h` cabe
    // (comprobado arriba); solo lo escriben la 3060 (dentro de `draw`) y nadie
    // mas, y la ventana lo lee al componer, en este mismo hilo.
    let pixeles = unsafe { core::slice::from_raw_parts_mut(va as *mut u32, (w * h) as usize) };
    let frame = Frame { clear: bmo_cubo::FONDO_F, vertices: &e.v[..n], viewport: Viewport { width: w, height: h } };
    match a.draw(&frame, &mut Image { pixels: pixeles, width: w, height: h }) {
        Ok(_) => {
            dsk.table.poner_fuente(i, va, s);
            true
        }
        Err(_) => {
            decir(e, "la 3060 no dibujo el fotograma (VERRANO dijo que no)");
            false
        }
    }
}
