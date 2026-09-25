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
    // ** Y la ventana que lo toma, se ENCIENDE (el destello de `brillo`).
    match ahora {
        Some(v) => crate::desktop::brillo::encender(v),
        None => crate::desktop::brillo::apagar(),
    }
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

// == *** EL ORDEN DE APILADO: una pregunta y un recorrido (2026-09-23) ======
//
// Visto en el Ryzen a las 00:06: el refresco del panel de Sonido pinto su fondo
// ENCIMA de CABINA (un rectangulo oscuro que se comio el principio de sus
// lineas) y los iconos D y N salieron encima del panel. Las dos cosas tenian la
// misma raiz: **el escritorio solo sabia cual estaba ARRIBA** (`top_before`), y
// "arriba" no contesta "me tapa alguien?" cuando hay tres ventanas. Ejecutar
// estaba delante, CABINA en medio, Sonido detras: Ejecutar no toca al panel, y
// el panel se creyo libre.
//
// El orden ya existia: es la lista del foco, y `seguir` (arriba) repinta con
// ella de atras hacia delante cada vez que el foco cambia -- asi que despues de
// cada cambio LO QUE SE VE es esa lista. Faltaba preguntarle.

/// Donde se ve una ventana, si se ve. Una app a pantalla completa es la
/// pantalla entera.
///
/// ** Las apps contestaban `None` ("su superficie se compone despues y queda
/// encima siempre"), y el Ryzen lo desmintio el 23-09 a las 01:07: el panel de
/// Sonido, DETRAS de DOOM, pintaba su medidor y sus numeros ENCIMA de DOOM. Una
/// app se repega cuando entrega fotograma, no cuando el panel acaba de pintar:
/// entre medias, lo de debajo se ve encima.
pub(crate) fn caja(dsk: &Desktop, v: Ventana) -> Option<(u32, u32, u32, u32)> {
    let de = |c: &crate::scene::chrome::Chrome| (!c.minimized).then_some((c.x, c.y, c.width, c.height));
    if !dsk.win.abierta(v) {
        return None;
    }
    match v {
        Ventana::Run => Some((dsk.run_box.x, dsk.run_box.y, dsk.run_box.w(), dsk.run_box.h())),
        Ventana::Data => de(&dsk.win.data.chrome),
        Ventana::Cabina => de(&dsk.win.cabina.chrome),
        Ventana::Estructura => de(&dsk.win.estructura.chrome),
        Ventana::Cpu => de(&dsk.win.cpu.chrome),
        Ventana::Mem => de(&dsk.win.mem.chrome),
        Ventana::Sound => de(&dsk.win.sound.chrome),
        Ventana::App(i) => dsk.table.get(i as usize).and_then(|s| {
            let c = &s.chrome;
            if c.minimized {
                None
            } else if c.is_fullscreen() {
                Some((0, 0, u32::MAX >> 1, u32::MAX >> 1))
            } else {
                Some((c.x, c.y, c.width, c.height))
            }
        }),
    }
}

fn se_pisan((x, y, w, h): (u32, u32, u32, u32), (a, b, c, d): (u32, u32, u32, u32)) -> bool {
    x < a + c && a < x + w && y < b + d && b < y + h
}

/// **Alguna ventana del sistema DELANTE de `v` la pisa?** Entonces lo que `v`
/// pinte por su cuenta (un refresco, un medidor) caeria ENCIMA de ella.
///
/// Delante = antes en la lista del foco. Si `v` no esta en la lista no se sabe
/// su sitio, y se contesta lo que no rompe nada: tapada si cualquier otra la
/// pisa. Pintar de menos se arregla al traerla delante; pintar de mas se come
/// una ventana ajena.
pub(crate) fn tapada(dsk: &Desktop, v: Ventana) -> bool {
    let Some(mia) = caja(dsk, v) else { return false };
    let lista = dsk.win.focus.lista();
    let hasta = lista.iter().position(|&id| Ventana::de_id(id) == Some(v));
    let delante = &lista[..hasta.unwrap_or(lista.len())];
    delante
        .iter()
        .filter_map(|&id| Ventana::de_id(id))
        .filter(|&o| o != v)
        .filter_map(|o| caja(dsk, o))
        .any(|otra| se_pisan(mia, otra))
}

/// Las ventanas del sistema **de atras hacia delante**: el orden en que hay que
/// pintarlas para que la de delante quede encima. Las que no estan en la lista
/// del foco van primero (detras de todo): no hay dato para ponerlas delante.
pub(crate) fn de_atras_adelante(dsk: &Desktop) -> ([Ventana; Ventana::TODAS.len()], usize) {
    let mut orden = [Ventana::Run; Ventana::TODAS.len()];
    let mut n = 0;
    let lista = dsk.win.focus.lista();
    let en_lista = |v: Ventana| lista.iter().any(|&id| Ventana::de_id(id) == Some(v));
    for v in Ventana::TODAS {
        if !en_lista(v) {
            orden[n] = v;
            n += 1;
        }
    }
    for &id in lista.iter().rev() {
        // Las apps no entran: su marco lo repinta la mesa de superficies.
        if let Some(v) = Ventana::de_id(id).filter(|v| !matches!(v, Ventana::App(_))) {
            if n < orden.len() {
                orden[n] = v;
                n += 1;
            }
        }
    }
    (orden, n)
}
