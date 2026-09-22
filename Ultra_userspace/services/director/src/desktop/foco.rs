//! **EL BORDE DE FOCO: a donde van las teclas, sin leer nada** (HUD 2,
//! 2026-09-22).
//!
//! [consumo] NADA      una comparacion por vuelta; solo pinta cuando el foco
//!                     CAMBIA, y eso lo cambia una mano (L6h)
//!
//! El motivo, uno: **saber de un vistazo donde cae lo que escribes.** Es el
//! `col.active_border` de Hyprland. Hasta hoy el foco solo se veia en la ficha
//! de la barra (subrayada) y en que la tecla llegaba -- o sea, despues de
//! escribir en la ventana equivocada.
//!
//! # Por que aqui y no en cada sitio que cambia el foco
//!
//! El foco lo cambian el raton, Alt+Tab, las F, cerrar una ventana, una app que
//! nace... y cada uno pinta SU ventana nueva. Ninguno repinta la VIEJA, que se
//! quedaria con el borde encendido: dos ventanas con foco en la pantalla, y
//! una de las dos miente. Aqui se mira UNA vez por vuelta si el foco es otro,
//! y se pone el borde a todas a la vez.
//!
//! # El orden en que se repintan
//!
//! Las del sistema, **de la mas vieja a la de delante** (la lista del foco al
//! reves): la que se pinta la ultima queda encima, y esa es la que tiene el
//! foco. Las apps no se pintan aqui: su marco lo repinta la mesa de superficies
//! en la vuelta (`repaint_all`), y sus pixeles van encima de todo de todas
//! formas.

use bmo_userland as bmo;

use crate::desktop::{Desktop, Ventana};
use crate::scene::surface::MAX;

/// **Si el foco cambio, el borde se muda.** Lo llama el bucle antes de componer.
pub(crate) fn seguir(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let ahora = dsk.win.focus.actual().filter(|&v| dsk.win.abierta(v));
    if ahora == dsk.win.foco_pintado {
        return;
    }
    dsk.win.foco_pintado = ahora;
    let es = |v: Ventana| ahora == Some(v);

    dsk.run_box.chrome.foco = es(Ventana::Run);
    dsk.win.data.chrome.foco = es(Ventana::Data);
    dsk.win.cabina.chrome.foco = es(Ventana::Cabina);
    dsk.win.estructura.chrome.foco = es(Ventana::Estructura);
    dsk.win.cpu.chrome.foco = es(Ventana::Cpu);
    dsk.win.mem.chrome.foco = es(Ventana::Mem);
    dsk.win.sound.chrome.foco = es(Ventana::Sound);
    for i in 0..MAX {
        if let Some(s) = dsk.table.get_mut(i) {
            let f = es(Ventana::App(i as u8));
            if s.chrome.foco != f {
                s.chrome.foco = f;
                s.repaint_all();
            }
        }
    }

    // Copia de la lista: pintar necesita `dsk` entero.
    let mut orden = [0u8; 16];
    let lista = dsk.win.focus.lista();
    let n = lista.len().min(orden.len());
    orden[..n].copy_from_slice(&lista[..n]);
    for &id in orden[..n].iter().rev() {
        if let Some(v) = Ventana::de_id(id) {
            crate::desktop::paint::pintar_ventana(dsk, p, v);
        }
    }
    dsk.win.taskbar_dirty = true;
}
