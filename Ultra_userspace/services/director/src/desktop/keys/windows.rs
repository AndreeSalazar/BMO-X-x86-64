//! **The five window toggles**: F7 cpu, F8 memory, F10 sound, F11 CABINA,
//! F12 data -- and the ESC that closes each one. And F1, which since
//! 2026-09-29 LAUNCHES the TALLER (`sys/taller.bex`) instead of toggling a
//! window of the DIRECTOR, and F4, which since 01-10 launches the LUDOTECA
//! (`sys/ludoteca.bex`).
//!
//! [consumo] NADA      no corre en reposo: lo llama el bucle SOLO si hubo una
//!                     tecla o el raton se movio. Sin entrada, no se entra
//!                     aqui (L6h)
//!
//! F1 is first in the list and last in the file, and both are on purpose: it is
//! the workshop, which is where you start, while F7..F12 are instruments you go
//! to. In the file it sits after F12 because that block is the one it was
//! copied from, and putting it next to its original is what makes a later
//! divergence between the two visible.
//!
//! Function keys produce no character in ANY layout, so they cannot collide
//! with typing. That is the only thing that matters in a system shortcut, and
//! it is exactly what `Ctrl+Alt` cannot offer: in Spanish it IS AltGr.

use bmo_userland as bmo;

use super::Key;
use crate::desktop::{Desktop, Ventana};
use crate::scene::{self};
use crate::{erase_window, uncover};

pub(crate) fn on_key(dsk: &mut Desktop, p: &bmo::Pantalla, c: u8, alt_alone: bool, m: u8) -> Key {
// Ctrl sin Alt (con Alt es AltGr). Ver `shortcuts`: Ctrl es la tecla del gestor.
let ctrl = m & bmo::MOD_CTRL != 0 && m & bmo::MOD_ALT == 0;
// == ALT+F4: CERRAR LO DE DELANTE ====================================
//
// Lo pidio el propietario con estas palabras: *"agregar esa ventanita para cerrar
// y todo eso como tipico Alt+F4, que es para cerrar cualquier app"*.
//
// Va ANTES que nada por lo mismo que F12: un atajo que solo funciona si ya
// estas dentro de la ventana no sirve para cerrarla.
//
// ** Y usa el MISMO camino que la X del marco --`cerrar_app`-- porque cerrar
// tiene que significar lo mismo se pida como se pida. Dos cierres distintos
// para el mismo gesto es como se llega a que uno mate el proceso y el otro no.
//
// [!] LO QUE NO ALCANZA, dicho por delante: una app a PANTALLA COMPLETA. Con
// DOOM delante el escritorio esta dormido en `lend_screen` y no lee teclas --
// no hay a quien mandarle este atajo. Para esas sigue siendo `Ctrl+Alt+Esc`,
// que vive en Ring 0 justamente porque es el unico sitio por donde pasan
// TODAS las teclas. Un atajo de escritorio no puede rescatar de algo que se
// llevo el escritorio.
// 0x8C = F4, con el mismo criterio que el 0x94 de F12 mas abajo: el codigo
// crudo, porque asi esta escrito el resto de este fichero.
//
// *** CIERRA LO QUE HAYA DELANTE, SEA LO QUE SEA (corregido el 05-09).
//
// La primera version solo cerraba `Ventana::App`, y el propietario lo probo y dijo
// *"el Alt+F4 no me funciona"*. Tenia razon, y el fallo era de esquema: el
// atajo se llamaba *cerrar cualquier app* y solo cerraba una CLASE de ventana.
// Con Ejecutar delante --que es lo que hay en cuanto no tienes una app
// abierta-- no hacia nada Y NO DECIA NADA, que es la peor de las dos mitades.
//
// ** Un atajo que a veces no hace nada y nunca lo explica se lee como roto,
// aunque este haciendo exactamente lo que se le escribio. Es la misma familia
// que lleva toda la semana: algo correcto que se entiende como otra cosa.
// == *** Y CTRL+C HACE LO MISMO (2026-09-12) ==========================
//
// Lo pidio el propietario con el problema delante: *"control + C es que eso me
// permite restaurar mi terminal y cerrar a la fuerza el app arrancado, no me
// dejo poner save por eso"*.
//
// Y el problema es real y no es de comodidad: **mientras una app tiene el
// foco, las teclas son suyas** --es el orden de `keys::app`-- asi que la caja
// de Ejecutar se queda muda y no hay donde escribir `guarda`. Alt+F4 ya
// resolvia eso desde el 05-09, pero `Ctrl+C` es el gesto que el propietario ya tiene
// en los dedos de treinta anios de terminal, y un atajo que hay que recordar
// es un atajo que no se usa.
//
// ** Mismo camino, no uno nuevo: `cerrar_app`. Tres gestos --la X, Alt+F4 y
// esto-- y UN cierre. Dos cierres distintos para el mismo gesto es como se
// llega a que uno mate el proceso y el otro no.
//
// ** Y con EJECUTAR delante no dice una frase: LIMPIA LA LINEA. Es lo que
// hace `Ctrl+C` en cualquier terminal del mundo, y es ademas lo que el propietario
// estaba intentando conseguir -- recuperar la caja para escribir otra cosa.
//
// [!] Lo que NO alcanza es lo mismo que no alcanza Alt+F4, y por el mismo
// motivo: una app que se llevo la PANTALLA deja al escritorio dormido en
// `lend_screen`, sin leer teclas. Para esas sigue siendo `Ctrl+Alt+Esc`, que
// vive en Ring 0 porque es el unico sitio por donde pasan TODAS.
//
// [!] Y llega aqui de verdad: el kernel cuece `Ctrl+C` como el byte 0x03, la
// cola cruda no reenvia teclas con modificador (`del_escritorio`) y la cocida
// tampoco reenvia los codigos de control (`keys::app::caracter`). O sea que
// **ninguna app lo ve**, y eso es a proposito: un rescate que la app pudiera
// interceptar no seria un rescate.
//
// ** Y CON SHIFT NO: Ctrl+Shift+C COPIA la linea (el de Windows Terminal). El
// kernel cuece los dos al mismo 0x03 --el codigo de control no lleva Shift--,
// asi que se distinguen por el modificador y el copiar sigue hasta el editor.
let ctrl_c = c == 0x03 && m & bmo::MOD_SHIFT == 0;

// *** Y LO PRIMERO QUE MIRA CTRL+C ES LA CORRIDA EN VUELO. (2026-09-12)
//
// El propietario lo corrigio y tenia razon: *"el control + C es para frenar en
// comando como terminal de Windows"*. Un terminal no interrumpe *la ventana de
// delante*: interrumpe **el comando que tu lanzaste**, tenga ventana o no.
//
// La diferencia no es de matiz y se ve en el caso que importa: un programa de
// CONSOLA que se cuelga --`leer.bex`, uno de COBOL-- no tiene ventana, asi que
// el foco sigue en Ejecutar. Mirando el foco, Ctrl+C le habria limpiado la
// linea y **habria dejado el programa colgado**, que es exactamente lo que el
// propietario estaba sufriendo.
//
// ** `Out::run` ya existia y ya sabia que hay una corrida esperando final: lo
// unico que le faltaba era a QUIEN. `ejecutar_en` devolvia el tid desde
// siempre y este sitio lo tiraba.
//
// Y despues de frenarlo no hay que hacer nada mas: el vigilante de `watch.rs`
// ve que ya no hay hijo, DRENA lo que dejo dicho y lo guarda en su `.txt`. O
// sea que un programa frenado deja su volcado igual que uno que acaba solo --
// que es lo que uno espera de un terminal.
if ctrl_c {
    if let Some(r) = dsk.out.run.as_ref() {
        if let Some(h) = bmo::Hijo::por_tid(r.tid) {
            if h.vive() {
                h.cerrar();
                dsk.out.grid.text(b"  ^C  frenado
");
                dsk.tick.repaint_field = true;
                return Key::Taken;
            }
        }
    }
}

// ** CTRL+Q es el mismo cierre (el `killactive` de Hyprland), por la misma
// puerta: tres gestos --la X, Alt+F4, Ctrl+Q-- y UN cierre. No Ctrl+W: en
// Ejecutar ya borra la palabra de antes, como en cualquier terminal.
if (c == 0x8C && alt_alone) || (ctrl && c == 0x11) || ctrl_c {
    match dsk.win.focus.actual() {
        // Una app: se cierra de verdad, por el MISMO camino que la X.
        Some(Ventana::App(i)) => {
            crate::desktop::mouse::apps::cerrar_app(dsk, p, i as usize);
            return Key::Taken;
        }
        // Los paneles del propio escritorio se esconden, que es lo que
        // significa cerrar para algo que no es un proceso.
        Some(Ventana::Data) => {
            dsk.win.data_open = false;
            dsk.win.focus.close(Ventana::Data);
            erase_window(p, &dsk.run_box, dsk.win.data.x(), dsk.win.data.y(),
                         dsk.win.data.width(), dsk.win.data.height(), dsk.win.visible);
            dsk.win.top_before = Ventana::Run;
            uncover(p, &dsk.run_box, &dsk.launcher, dsk.win.visible,
                    &mut dsk.out.grid, &mut dsk.tick.repaint_field);
            return Key::Taken;
        }
        // [!] Y aqui NO se cierra nada, pero SE DICE. Ejecutar es la casa: si
        // Alt+F4 la cerrara no quedaria donde escribir. Callar seria dejar al
        // propietario pensando que el atajo esta roto -- que es exactamente lo que
        // paso el 05-09.
        _ => {
            if ctrl_c {
                // La caja delante y `Ctrl+C`: se limpia lo tecleado, como en
                // cualquier terminal. No hay nada que cerrar y SI hay algo que
                // devolver -- una linea en blanco donde escribir.
                dsk.field.n = 0;
                dsk.field.cur = 0;
                dsk.out.grid.text(b"  linea limpia
");
            } else {
                dsk.out.grid.text(b"  Alt+F4 cierra la ventana de delante. Esta es la casa.
");
            }
            dsk.tick.repaint_field = true;
            return Key::Taken;
        }
    }
}

// -- F12 es del SISTEMA, no de una ventana --
//
// Se atiende ANTES de preguntar por el foco, y tiene que ser
// asi: un atajo que solo funciona si ya estas en la ventana que
// abre no sirve para abrirla -- y peor, no sirve para cerrarla,
// porque para entonces el foco ya es suyo.
//
// ESC cierra la de arriba, que es lo que hace ESC en todas
// partes. En Ejecutar ESC sigue borrando la linea: son dos
// ventanas distintas y cada una contesta lo suyo.
let toggle_data = if c == 0x94 {
    Some(!dsk.win.data_open)
} else if c == 0x1B && dsk.win.data_open && dsk.win.focus.es_para(Ventana::Data) {
    Some(false)
} else {
    None
};
if let Some(open) = toggle_data {
    dsk.win.data_open = open;
    if open {
        // Abrir es decirselo al foco y ya: en modo `Fijo` la
        // ventana aparece y NO se lleva el teclado, y quien
        // decide eso es la politica, no esta tecla.
        dsk.win.focus.open(Ventana::Data);
        // El equipo se MIDE al abrir: los numeros son de ahora, no de la
        // ultima vez que alguien miro.
        // Y el Inicio, que ademas lee la historia (01-10).
        let v = dsk.win.data.view;
        if v == scene::data::View::Equipo || v == scene::data::View::Inicio {
            dsk.win.data.ir_a(v);
        }
        scene::data::paint(&p, &dsk.win.data);
        dsk.win.top_before = if dsk.win.focus.es_para(Ventana::Data) { Ventana::Data } else { Ventana::Run };
        // En `Fijo` se ha pintado encima de una caja que sigue
        // teniendo el teclado: hay que devolverla arriba.
        if dsk.win.top_before == Ventana::Run {
            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        }
    } else {
        // Al cerrarla hay que devolver el fondo Y repintar
        // lo que tapaba: la caja de Ejecutar esta debajo.
        dsk.win.focus.close(Ventana::Data);
        erase_window(
            &p, &dsk.run_box, dsk.win.data.x(), dsk.win.data.y(),
            dsk.win.data.width(), dsk.win.data.height(), dsk.win.visible,
        );
        dsk.win.top_before = Ventana::Run;
        uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
    }
    return Key::Taken;
}

// -- F1: el TALLER (se llamo ESTRUCTURA hasta el 29-09) --
//
// Calcada de F12 y por los mismos motivos: se atiende ANTES de
// preguntar por el foco, porque un atajo que solo funciona si ya
// estas dentro de la ventana no sirve para abrirla, y peor, no
// sirve para cerrarla.
//
// ** F1 ESTABA LIBRE, y no de casualidad: `keys/app.rs` declaraba
// `SC_F1 = 0x3B` solo como frontera del rango que el escritorio
// retiene (`SC_F1..=SC_F10`), sin que nadie la usara. O sea que la
// tecla ya llegaba aqui y no habia quien la recogiera.
//
// El 0x89 es el codigo COCIDO, no el scancode: lo pone
// `ring0/dev/keyboard.rs` (`KEY_F1`), y las doce F son 0x89..0x94.
// Escribir aqui el 0x3B compilaria y no abriria nada -- son dos
// colas distintas, y esta es la cocida.
//
// ** DESDE EL 29-09 F1 LANZA UNA APP, no abre una ventana del
// DIRECTOR. `docs/plan/PLAN_TALLER.md` seccion 8: el taller es
// un editor de nodos para TITAN++ con el comprobador animado, y eso
// dentro del compositor seria el monolito que prohibe MODULAR. La
// ventana de antes (escalon 1, vista en el metal el 06-09) se retiro:
// su sitio es `sys/taller.bex` (`Ultra_userspace/apps/taller`).
//
// Se pide por `scene::abrir`, el MISMO camino que un `run` tecleado o
// un icono: consola, prestamo de pantalla y vigilante. Se cierra
// desde dentro: Esc lo lee la app, y Alt+F4 sigue valiendo.
if c == 0x89 {
    // `taller` y no `estructura`: el FAT32 de BMO-X busca por nombre 8.3, y el
    // 29-09 en el Ryzen `estructura.bex` dio "un nombre no cabe en 8.3" en
    // CABINA y F1 no abrio nada. El build ya lo comprueba (`ejemplos.ps1`).
    lanzar_o_alternar(dsk, p, b"sys/taller.bex");
    return Key::Taken;
}

// ** F4: LA LUDOTECA (01-10). El propietario: *"F4 para la LUDOTECA"*.
// Las doce estaban tomadas; F4 escribia `apps`, que se sigue tecleando. Es
// una app como el TALLER (`sys/ludoteca.bex`, `Ultra_userspace/apps/
// ludoteca`): mismo camino, mismo cierre (Esc dentro, Alt+F4 fuera). Y su
// JUGAR vuelve aqui por la consola: ver `desktop::pide`.
if c == 0x8C {
    lanzar_o_alternar(dsk, p, b"sys/ludoteca.bex");
    return Key::Taken;
}

// -- F7 y F8: VITALES, una ventana con tres solapas (02-10) --
//
// Calcadas de F11 y por los mismos motivos: se atienden ANTES de preguntar
// por el foco, porque un atajo que solo funciona si ya estas dentro de la
// ventana no sirve para abrirla. F7 abre en CPU y F8 en MEMORIA; la misma
// tecla otra vez la cierra, la otra cambia de solapa. ESC la cierra.
if c == 0x8F || c == 0x90 {
    let s = if c == 0x8F { scene::vitals::Solapa::Cpu } else { scene::vitals::Solapa::Memoria };
    crate::desktop::vitales::tecla_f(dsk, &p, s);
    return Key::Taken;
}
if c == 0x1B && dsk.win.mem_open {
    crate::desktop::vitales::cerrar(dsk, &p);
    return Key::Taken;
}

// -- F11: la consola del KERNEL --
//
// Calcada de F12 y por los mismos motivos: se atiende ANTES de
// preguntar por el foco, porque un atajo que solo funciona si ya
// estas dentro de la ventana no sirve para abrirla.
let toggle_klog = if c == 0x93 {
    Some(!dsk.win.cabina_open)
} else if c == 0x1B && dsk.win.cabina_open {
    Some(false)
} else {
    None
};
if let Some(open) = toggle_klog {
    dsk.win.cabina_open = open;
    if open {
        // Se abre SIEMPRE por lo ultimo, que es lo que se quiere
        // ver el 90% de las veces. Para ir al arranque estan
        // RePag/AvPag.
        dsk.win.cabina.from = 0;
        dsk.win.focus.open(Ventana::Cabina);
        scene::cabina::paint(&p, &dsk.win.cabina);
        dsk.win.top_before = if dsk.win.focus.es_para(Ventana::Cabina) { Ventana::Cabina } else { Ventana::Run };
        if dsk.win.top_before == Ventana::Run {
            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        }
    } else {
        dsk.win.focus.close(Ventana::Cabina);
        erase_window(
            &p, &dsk.run_box, dsk.win.cabina.chrome.x, dsk.win.cabina.chrome.y,
            dsk.win.cabina.chrome.width, dsk.win.cabina.chrome.height, dsk.win.visible,
        );
        dsk.win.top_before = Ventana::Run;
        uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        // Si Datos estaba abierta debajo, vuelve a verse.
        if dsk.win.data_open {
            scene::data::paint(&p, &dsk.win.data);
        }
    }
    return Key::Taken;
}

// -- F10: el panel del SONIDO, el maestro --
//
// Calcada de F11. Hasta el 2026-09-22 abrir y cerrar TOMABAN Y
// DEVOLVIAN el aparato, y por eso con un juego sonando la ventana
// decia "lo tiene OTRO": ahora manda sin reclamar, y abrir y cerrar
// entran por la misma puerta que el indicador de la barra
// (`desktop::sonido::abrir_o_cerrar`).
let toggle_sound = if c == 0x92 {
    Some(!dsk.win.sound_open)
} else if c == 0x1B
    && dsk.win.sound_open
    && dsk.win.focus.es_para(Ventana::Sound)
    && !crate::desktop::sonido::escribiendo(dsk)
{
    // Con dB a medio escribir, ESC deja de escribir (lo atiende el panel) y
    // no cierra: perder la ventana por arrepentirse de un numero es mucho.
    Some(false)
} else {
    None
};
if let Some(open) = toggle_sound {
    crate::desktop::sonido::abrir_o_cerrar(dsk, &p, open, false);
    return Key::Taken;
}
    Key::Pass
}

// == F1 y F4: UNA SOLA VENTANA DE CADA APP (01-10) =========================
//
// El propietario: *"F4 cuando abro normal pero se duplica; presiono F4 y no
// se cierra ni se minimiza"*. Cada F lanzaba OTRA copia. Ahora la tecla de una
// app es como la de una ventana del escritorio (F11, F12):
//
//    no esta abierta          la lanza (UNA vez: dos pulsaciones seguidas,
//                             antes de que llegue su ventana, no son dos)
//    abierta y delante        la minimiza
//    abierta, detras o minimizada   la trae delante, con el foco
//
// Cual es cual se sabe por el tid de su superficie y la tabla de programas
// del kernel (INFO_PROG_QUIEN y su nombre), no por adivinarlo.

/// Lo que se espera a que aparezca la ventana de una app recien pedida.
const ESPERA_MS: u64 = 4000;
/// (ciclos, ruta) de la ultima app pedida por una F.
static mut PEDIDA: (u64, [u8; 24]) = (0, [0; 24]);

/// La superficie de la app cuyo programa acaba en `nombre`, si esta abierta.
fn app_abierta(dsk: &Desktop, nombre: &[u8]) -> Option<usize> {
    let (huecos, n) = dsk.table.fichas();
    for &i in &huecos[..n] {
        let Some(sup) = dsk.table.get(i) else { continue };
        for k in 0..64u64 {
            let quien = bmo::info(bmo::INFO_PROG_QUIEN | (k << 8));
            if quien == 0 {
                break;
            }
            if (quien >> 16) & 0xFFFF != sup.tid as u64 {
                continue;
            }
            let mut texto = [0u8; 40];
            let t = bmo::info_texto(bmo::INFO_TXT_PROG_NOMBRE | (k << 8), &mut texto).min(texto.len());
            if texto[..t].ends_with(nombre) {
                return Some(i);
            }
        }
    }
    None
}

fn lanzar_o_alternar(dsk: &mut Desktop, p: &bmo::Pantalla, ruta: &[u8]) {
    let nombre = ruta.rsplit(|&c| c == b'/').next().unwrap_or(ruta);
    if let Some(i) = app_abierta(dsk, nombre) {
        let v = Ventana::App(i as u8);
        if !dsk.table.minimizada(i) && dsk.win.focus.actual() == Some(v) {
            crate::desktop::mouse::apps::minimizar_app(dsk, p, i);
        } else if dsk.table.traer(i, p) {
            dsk.win.focus.open(v);
            dsk.win.focus.clic_en(v);
        }
        dsk.win.taskbar_dirty = true;
        return;
    }
    // Pedida hace nada y su ventana todavia no llego: la misma pulsacion.
    let ahora = bmo::ciclos();
    let ms = bmo::info(bmo::INFO_TSC_HZ).max(1) / 1000;
    // SAFETY: el hilo del escritorio, al atender una tecla.
    let pedida = unsafe { &mut *core::ptr::addr_of_mut!(PEDIDA) };
    let misma = pedida.1[..ruta.len().min(24)] == ruta[..ruta.len().min(24)];
    if misma && ahora.wrapping_sub(pedida.0) < ESPERA_MS * ms.max(1) {
        return;
    }
    let mut r = [0u8; 24];
    r[..ruta.len().min(24)].copy_from_slice(&ruta[..ruta.len().min(24)]);
    *pedida = (ahora, r);
    scene::abrir::pedir(&[ruta]);
}
