//! **El mando de VITALES** (F7 / F8): abrir, cerrar y cambiar de solapa; sus
//! teclas; su raton (arrastrar, los tres botones, las solapas y las filas); y
//! la muestra de cada cuarto de segundo. La cara la pinta `scene::vitals`.
//!
//! [consumo] NADA      no corre en reposo: lo llaman la tecla, el raton y el
//!                     cuarto de segundo SOLO con la ventana abierta (L6h)
//!
//! Hasta el 02-10 eran DOS ventanas (CPU y memoria), sin raton: no se
//! arrastraban, sus botones no respondian y un clic encima se lo llevaba la
//! ventana de debajo -- su propio pie prometia "arrastra el titulo" y no lo
//! hacia. Ahora es una, con solapas, y el marco se cablea entero.

use bmo_userland as bmo;

use crate::desktop::{Desktop, Ventana};
use crate::scene;
use crate::scene::chrome::Button;
use crate::scene::vitals::{filas, Solapa, FICHAS, FILA_VACIA};
use crate::{erase_window, uncover};

/// Pintarla con lo que el escritorio ya midio.
pub(crate) fn pintar(dsk: &Desktop, p: &bmo::Pantalla) {
    scene::vitals::paint(p, &dsk.win.mem, dsk.tick.loops_per_second, dsk.tick.consumo.ultimo);
}

fn mw(dsk: &Desktop) -> Option<u64> {
    dsk.tick.consumo.ultimo.map(|m| m.mw_paquete).filter(|&v| v > 0)
}

/// **F7 (`Solapa::Cpu`) o F8 (`Solapa::Memoria`)**: abre en esa solapa; si
/// ya esta abierta EN ESA, la cierra; si esta en otra, cambia a esa.
pub(crate) fn tecla_f(dsk: &mut Desktop, p: &bmo::Pantalla, s: Solapa) {
    if dsk.win.mem_open && dsk.win.mem.solapa == s {
        cerrar(dsk, p);
        return;
    }
    dsk.win.mem.solapa = s;
    if !dsk.win.mem_open {
        dsk.win.mem_open = true;
        dsk.win.mem.chrome.minimized = false;
        dsk.win.mem.aviso_n = 0;
        let m = mw(dsk);
        dsk.win.mem.muestrear(m);
    }
    dsk.win.focus.open(Ventana::Mem);
    pintar(dsk, p);
}

/// Cerrarla y devolver lo que tapaba.
pub(crate) fn cerrar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    dsk.win.mem_open = false;
    dsk.win.focus.close(Ventana::Mem);
    let c = &dsk.win.mem.chrome;
    erase_window(p, &dsk.run_box, c.x, c.y, c.width, c.height, dsk.win.visible);
    uncover(p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
}

/// **Cada cuarto de segundo**, con la ventana abierta: una muestra para las
/// graficas, y se repinta si se ve.
pub(crate) fn cuarto(dsk: &mut Desktop, p: &bmo::Pantalla, se_ve: bool) {
    if !dsk.win.mem_open {
        return;
    }
    let m = mw(dsk);
    dsk.win.mem.muestrear(m);
    if se_ve && !dsk.win.mem.chrome.minimized {
        pintar(dsk, p);
    }
}

fn cambiar(dsk: &mut Desktop, p: &bmo::Pantalla, s: Solapa) {
    dsk.win.mem.solapa = s;
    dsk.win.mem.aviso_n = 0;
    pintar(dsk, p);
}

/// **FINALIZAR la fila elegida** de PROCESOS, como el administrador de
/// tareas. Solo lo que lanzo el escritorio (`bmo::Hijo::por_tid` solo
/// encuentra eso): no hay "matar el pid N", que seria root con otro nombre.
/// Si es una app con ventana, por el mismo camino que su X.
fn finalizar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let mut v = [FILA_VACIA; FICHAS];
    let n = filas(dsk.win.mem.orden, &mut v);
    if n == 0 {
        dsk.win.mem.decir(b"no hay ningun programa en la tabla");
        return;
    }
    let f = v[dsk.win.mem.elegido.min(n - 1)];
    let mut t = [0u8; 80];
    let mut k = 0usize;
    let mut poner = |s: &[u8], k: &mut usize| {
        for &b in s {
            if *k < t.len() {
                t[*k] = b;
                *k += 1;
            }
        }
    };
    match bmo::Hijo::por_tid(f.tid) {
        Some(h) if h.vive() => {
            let app = (0..scene::surface::MAX).find(|&i| dsk.table.get_mut(i).is_some_and(|s| s.tid == f.tid));
            let hecho = match app {
                Some(i) => {
                    crate::desktop::mouse::apps::cerrar_app(dsk, p, i);
                    true
                }
                None => h.cerrar(),
            };
            poner(if hecho { b"FINALIZADO: " } else { b"no se dejo finalizar: " }, &mut k);
        }
        Some(_) => poner(b"ya habia acabado: ", &mut k),
        None => poner(b"no lo lanzo el escritorio, no se cierra desde aqui: ", &mut k),
    }
    poner(&f.nombre()[..f.nombre().len().min(40)], &mut k);
    dsk.win.mem.decir(&t[..k]);
}

/// **Las teclas de la ventana**, con el foco en ella: `1 2 3` o Tab (y las
/// flechas a los lados) cambian de solapa; en PROCESOS, arriba y abajo
/// eligen, `O` ordena y `F` finaliza. `true` si la tecla era suya.
pub(crate) fn on_key(dsk: &mut Desktop, p: &bmo::Pantalla, c: u8) -> bool {
    if !dsk.win.mem_open || !dsk.win.focus.es_para(Ventana::Mem) {
        return false;
    }
    let s = dsk.win.mem.solapa;
    match c {
        b'1' => cambiar(dsk, p, Solapa::Cpu),
        b'2' => cambiar(dsk, p, Solapa::Memoria),
        b'3' => cambiar(dsk, p, Solapa::Procesos),
        b'\t' | 0x83 => cambiar(dsk, p, s.siguiente()),
        0x82 => cambiar(dsk, p, s.siguiente().siguiente()),
        0x80 if s == Solapa::Procesos => {
            dsk.win.mem.elegido = dsk.win.mem.elegido.saturating_sub(1);
            pintar(dsk, p);
        }
        0x81 if s == Solapa::Procesos => {
            dsk.win.mem.elegido = (dsk.win.mem.elegido + 1).min(FICHAS - 1);
            pintar(dsk, p);
        }
        b'o' | b'O' if s == Solapa::Procesos => {
            dsk.win.mem.orden = dsk.win.mem.orden.siguiente();
            dsk.win.mem.elegido = 0;
            pintar(dsk, p);
        }
        b'f' | b'F' if s == Solapa::Procesos => {
            finalizar(dsk, p);
            pintar(dsk, p);
        }
        _ => return false,
    }
    true
}

/// La rueda sobre la ventana: en PROCESOS mueve la fila elegida.
pub(crate) fn rueda(dsk: &mut Desktop, p: &bmo::Pantalla, giro: i32) {
    if dsk.win.mem.solapa != Solapa::Procesos {
        return;
    }
    let e = dsk.win.mem.elegido as i32 - giro;
    dsk.win.mem.elegido = e.clamp(0, FICHAS as i32 - 1) as usize;
    pintar(dsk, p);
}

/// **El raton**: arrastrar por el titulo, los tres botones, un clic en una
/// solapa o en una fila. `true` si el gesto era suyo.
pub(crate) fn raton(dsk: &mut Desktop, p: &bmo::Pantalla, x: u32, y: u32, button: bool, antes: bool) -> bool {
    if !dsk.win.mem_open || dsk.win.mem.chrome.minimized {
        return false;
    }
    // El arrastre, como CABINA: agarrar por el titulo con el foco aqui.
    if !button && dsk.win.mem.chrome.grabbed() {
        dsk.win.mem.chrome.release();
        return true;
    }
    if button && dsk.win.mem.chrome.grabbed() {
        let c = &dsk.win.mem.chrome;
        let viejo = (c.x, c.y, c.width, c.height);
        if dsk.win.mem.chrome.follow_pointer(p, x, y) {
            let c = &dsk.win.mem.chrome;
            scene::erase_moved(p, &dsk.run_box, viejo, (c.x, c.y, c.width, c.height), dsk.win.visible);
            uncover(p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
            pintar(dsk, p);
            dsk.win.top_before = Ventana::Mem;
        }
        return true;
    }
    let flanco = button && !antes;
    if !flanco || !dsk.win.focus.es_para(Ventana::Mem) || !dsk.win.mem.chrome.contains(x, y) {
        return false;
    }
    match dsk.win.mem.chrome.button_at(x, y) {
        // Minimizar CIERRA, como el panel del sonido: no tiene ficha en la
        // barra a la que volver; vuelve con F7 o F8.
        Some(Button::Close) | Some(Button::Minimize) => {
            cerrar(dsk, p);
            return true;
        }
        Some(Button::Maximize) => {
            let viejo = dsk.win.mem.chrome.toggle_maximized(p);
            let c = &dsk.win.mem.chrome;
            if scene::erase_resized(p, &dsk.run_box, viejo, (c.x, c.y, c.width, c.height), dsk.win.visible) {
                uncover(p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
            }
            pintar(dsk, p);
            dsk.win.top_before = Ventana::Mem;
            return true;
        }
        None => {}
    }
    if let Some(s) = dsk.win.mem.solapa_en(x, y) {
        cambiar(dsk, p, s);
        return true;
    }
    if let Some(k) = dsk.win.mem.fila_en(x, y) {
        dsk.win.mem.elegido = k;
        pintar(dsk, p);
        return true;
    }
    if dsk.win.mem.chrome.on_the_grip(x, y) {
        dsk.win.mem.chrome.grab(x, y);
        return true;
    }
    false
}
