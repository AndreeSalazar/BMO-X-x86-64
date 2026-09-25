//! **LA CAJA COMO UN EXPLORADOR: que hace cada boton** (2026-09-25).
//!
//! [consumo] NADA      corre con un clic
//!
//! La cara (y donde cae cada boton) es de `scene::caja`; esto decide. Nada de
//! aqui es un camino nuevo: cada boton es una TECLA que ya existia, o una
//! orden escrita en el campo y Enter -- como hacen F1..F10 en
//! `keys::editor` --, para que no haya dos versiones de la orden mas usada.
//!
//! ```text
//!    <  >     el historial (flecha arriba / abajo)
//!    ^        subir la salida una pagina (RePag)
//!    @        repetir la ultima orden
//!    Buscar   Ctrl+F: buscar en la salida
//!    [>]      Ejecutar lo que haya en el campo (Enter)
//!    info ... ayuda   escribir esa orden y Enter
//! ```

use bmo_userland as bmo;

use crate::desktop::keys::editor::on_key;
use crate::desktop::Desktop;
use crate::scene::caja::{boton_en, Boton, ORDENES};

/// Las teclas que ya existian (`keys::editor`).
const ARRIBA: u8 = 0x80;
const ABAJO: u8 = 0x81;
const REPAG: u8 = 0x87;
const CTRL_F: u8 = 0x06;

/// **Un clic en la caja.** `true` si cayo en un boton (y se hizo lo suyo).
pub(crate) fn clic(dsk: &mut Desktop, p: &bmo::Pantalla, x: u32, y: u32) -> bool {
    let Some(b) = boton_en(&dsk.run_box, x, y) else { return false };
    match b {
        Boton::Atras => {
            let _ = on_key(dsk, p, ARRIBA);
        }
        Boton::Adelante => {
            let _ = on_key(dsk, p, ABAJO);
        }
        Boton::Subir => {
            let _ = on_key(dsk, p, REPAG);
        }
        Boton::Repetir => {
            // La ultima del historial, al campo, y Enter.
            dsk.field.n = 0;
            dsk.field.cur = 0;
            let _ = on_key(dsk, p, ARRIBA);
            if dsk.field.n > 0 {
                let _ = on_key(dsk, p, b'\r');
            }
        }
        Boton::Buscar => {
            let _ = on_key(dsk, p, CTRL_F);
        }
        Boton::Orden(k) => {
            let orden = ORDENES[k].2;
            if !orden.is_empty() {
                let n = orden.len().min(dsk.field.path.len());
                dsk.field.path[..n].copy_from_slice(&orden[..n]);
                dsk.field.n = n;
                dsk.field.cur = n;
            }
            // Una ruta de .bex en el campo pediria prestar la pantalla, y eso
            // solo lo puede el bucle de `main`: desde un clic, se deja
            // escrita y el Enter del teclado la lanza.
            let _ = on_key(dsk, p, b'\r');
        }
    }
    dsk.tick.repaint_field = true;
    true
}
