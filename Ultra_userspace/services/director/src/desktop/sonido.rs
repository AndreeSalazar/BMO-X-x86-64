//! **EL MANDO DEL SONIDO en el escritorio**: abrir y cerrar el panel, sus
//! teclas, su raton y el refresco del medidor.
//!
//! [consumo] LATE      `latido` mira el medidor 20 veces por segundo mientras
//!                     suena algo y 4 en silencio; `toca`, que se pregunta en
//!                     cada vuelta, no cruza ni una puerta: compara el reloj
//!                     con una cifra apuntada (L6h)
//!
//! La cara la pinta `scene::sound`; esto la MUEVE. Van separadas por lo mismo
//! que en el resto del escritorio: pintar no decide nada, y lo que decide no
//! pinta.
//!
//! # *** UNA SOLA PUERTA PARA ABRIR Y CERRAR
//!
//! F10, ESC, el aspa y el indicador de la barra abren y cierran lo mismo, y
//! entran TODOS por [`abrir_o_cerrar`]. Es la regla que la barra ya escribio
//! para CABINA: *"si las dos puertas dejaran la ventana en estados distintos,
//! el que la abre con el raton veria otra cosa que el que la abre con la
//! tecla, y una de las dos estaria mal sin que nadie pudiera decir cual"*.

use bmo_userland as bmo;

use crate::desktop::{Desktop, Ventana};
use crate::scene;
use crate::scene::chrome::Button;
use crate::{erase_window, uncover};

const DB: i32 = 256;

/// **Abrir o cerrar el panel del maestro.** `desde_la_barra` lo coloca
/// debajo del indicador, que es de donde se pidio.
///
/// Ya no toma ni devuelve ningun aparato: el mando no es exclusivo (ver la
/// cabecera de `scene::sound`). Abrirlo con DOOM sonando no calla a DOOM.
pub(crate) fn abrir_o_cerrar(dsk: &mut Desktop, p: &bmo::Pantalla, abrir: bool, desde_la_barra: bool) {
    dsk.win.sound_open = abrir;
    dsk.snd.panel.arrastrando = false;
    // Lo que estuviera a medio escribir no sobrevive a cerrar y abrir.
    dsk.snd.panel.escrito_n = 0;
    if abrir {
        if desde_la_barra {
            dsk.win.sound.junto_a_la_barra(p);
        }
        dsk.win.sound.chrome.minimized = false;
        dsk.snd.panel.olvidar();
        // Que el primer refresco no espere: el medidor tiene que salir ya.
        dsk.snd.panel.proximo = 0;
        dsk.win.focus.open(Ventana::Sound);
        if desde_la_barra {
            dsk.win.focus.clic_en(Ventana::Sound);
        }
        scene::sound::paint(p, &dsk.win.sound, &dsk.snd.panel);
        dsk.win.top_before = if dsk.win.focus.es_para(Ventana::Sound) { Ventana::Sound } else { Ventana::Run };
        if dsk.win.top_before == Ventana::Run {
            uncover(p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        }
    } else {
        dsk.win.focus.close(Ventana::Sound);
        erase_window(
            p, &dsk.run_box, dsk.win.sound.chrome.x, dsk.win.sound.chrome.y,
            dsk.win.sound.chrome.width, dsk.win.sound.chrome.height, dsk.win.visible,
        );
        dsk.win.top_before = Ventana::Run;
        uncover(p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
        // Si habia ventanas debajo, vuelven a verse.
        if dsk.win.data_open {
            scene::data::paint(p, &dsk.win.data);
        }
        if dsk.win.cabina_open {
            scene::cabina::paint(p, &dsk.win.cabina);
        }
    }
}

/// **Mover el fader**: se redondea al grano, se manda, y se refresca ya para
/// que la mano vea el numero nuevo sin esperar al siguiente latido.
fn mover(dsk: &mut Desktop, p: &bmo::Pantalla, db: i32) {
    let db = scene::sound::redondear(db);
    if dsk.snd.panel.enviado == Some(db) {
        return;
    }
    // El kernel dice NO a quien no tiene la pantalla, con el motivo en CABINA.
    // El escritorio la tiene; si aun asi contestara que no, el panel lo
    // muestra solo: el fader se queda donde el kernel dice que esta.
    if bmo::audio_mando(bmo::AUDIO_MANDO_FADER, db as i64).is_some() {
        dsk.snd.panel.enviado = Some(db);
    }
    refrescar_ya(dsk, p);
}

fn callar(dsk: &mut Desktop, p: &bmo::Pantalla, si: bool) {
    let _ = bmo::audio_mando(bmo::AUDIO_MANDO_MUDO, si as i64);
    refrescar_ya(dsk, p);
}

fn refrescar_ya(dsk: &mut Desktop, p: &bmo::Pantalla) {
    dsk.snd.panel.proximo = 0;
    latido(dsk, p);
}

/// **Hay dB a medio escribir?** Lo pregunta el ESC de `keys::windows`: con algo
/// escrito, ESC deja de escribir; sin nada, cierra el panel.
pub(crate) fn escribiendo(dsk: &Desktop) -> bool {
    dsk.win.sound_open && dsk.snd.panel.escrito_n > 0
}

/// **Las teclas del panel**, solo con el foco en el. Devuelve si la tomo.
///
/// ```text
///    flecha arriba / derecha    +1 dB        RePag   +6 dB
///    flecha abajo / izquierda   -1 dB        AvPag   -6 dB
///    un NUMERO y Enter          esos dB      M       mudo
/// ```
///
/// ** EL NUMERO (2026-09-22). El propietario: *"ponlo como control de
/// numeros"*. Se teclea `40`, `-12`, `0` y Enter, y el fader va ALLI -- con la
/// misma rampa que el raton, por el mismo `mover`. Retroceso borra una letra y
/// ESC deja lo escrito sin cerrar el panel. Lo que pase del recorrido se queda
/// en el borde, y el numero de la columna dice donde quedo.
///
/// Con el foco en Ejecutar, una `m` es una letra que el propietario esta
/// escribiendo: por eso la guarda del foco va primero, como en el klog.
pub(crate) fn on_key(dsk: &mut Desktop, p: &bmo::Pantalla, c: u8) -> bool {
    if !(dsk.win.sound_open && dsk.win.focus.es_para(Ventana::Sound)) {
        return false;
    }
    if escribir(dsk, p, c) {
        return true;
    }
    let l = scene::sound::leer();
    let f = l.fader_efectivo();
    match c {
        0x80 | 0x83 => mover(dsk, p, f + DB),
        0x81 | 0x82 => mover(dsk, p, f - DB),
        0x87 => mover(dsk, p, f + 6 * DB),
        0x88 => mover(dsk, p, f - 6 * DB),
        b'm' | b'M' => callar(dsk, p, !l.mudo),
        _ => return false,
    }
    true
}

/// **La entrada de numeros.** Devuelve si la tecla era suya.
fn escribir(dsk: &mut Desktop, p: &bmo::Pantalla, c: u8) -> bool {
    let n = dsk.snd.panel.escrito_n;
    match c {
        b'0'..=b'9' | b'-' => {
            // El signo solo delante; y tres letras son el tope (`-60`).
            if (c == b'-' && n > 0) || n >= dsk.snd.panel.escrito.len() {
                return true;
            }
            dsk.snd.panel.escrito[n] = c;
            dsk.snd.panel.escrito_n = n + 1;
        }
        0x08 if n > 0 => {
            dsk.snd.panel.escrito_n = n - 1;
            if n == 1 {
                dejar(dsk, p);
                return true;
            }
        }
        0x1B if n > 0 => {
            dejar(dsk, p);
            return true;
        }
        0x0D | 0x0A if n > 0 => {
            let (neg, cifras) = match dsk.snd.panel.escrito[0] {
                b'-' => (true, &dsk.snd.panel.escrito[1..n]),
                _ => (false, &dsk.snd.panel.escrito[..n]),
            };
            let mut v = 0i32;
            for &d in cifras {
                v = v * 10 + (d - b'0') as i32;
            }
            let hay_cifras = !cifras.is_empty();
            dejar(dsk, p);
            if hay_cifras {
                mover(dsk, p, if neg { -v * DB } else { v * DB });
            }
            return true;
        }
        _ => return false,
    }
    scene::sound::escrito(p, &dsk.win.sound, &dsk.snd.panel);
    true
}

/// Se deja de escribir: la linea del aviso vuelve a lo suyo.
fn dejar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    dsk.snd.panel.escrito_n = 0;
    dsk.snd.panel.olvidar_cabecera();
    refrescar_ya(dsk, p);
}

/// **El raton sobre el panel**: los tres botones del marco, el MUDO y el
/// fader. Devuelve si se quedo el gesto. El arrastre de la barra de titulo
/// sigue en `mouse::ventanas`, con el de las demas ventanas.
///
/// ** El fader se sigue AUNQUE el puntero se salga de su columna: una mano que
/// sube deprisa se pasa de lado, y un fader que se suelta solo porque te
/// desviaste tres pixeles es un fader que pelea contigo.
pub(crate) fn raton(dsk: &mut Desktop, p: &bmo::Pantalla, x: u32, y: u32, button: bool, antes: bool) -> bool {
    if !dsk.win.sound_open || dsk.win.sound.chrome.minimized {
        dsk.snd.panel.arrastrando = false;
        return false;
    }
    let s = scene::sound::sitio(&dsk.win.sound);
    if dsk.snd.panel.arrastrando {
        if !button {
            dsk.snd.panel.arrastrando = false;
        } else {
            mover(dsk, p, s.db_en(y));
        }
        return true;
    }
    let flanco = button && !antes;
    if !flanco || !dsk.win.focus.es_para(Ventana::Sound) || !dsk.win.sound.chrome.contains(x, y) {
        return false;
    }
    match dsk.win.sound.chrome.button_at(x, y) {
        // ** Minimizar CIERRA. Esta ventana no tiene ficha en la barra: su
        // ficha es el indicador, que la vuelve a abrir con un clic. Un
        // "minimizado" sin sitio al que volver seria una ventana perdida.
        Some(Button::Close) | Some(Button::Minimize) => {
            abrir_o_cerrar(dsk, p, false, false);
            return true;
        }
        Some(Button::Maximize) => {
            let (vx, vy, va, vl) = dsk.win.sound.chrome.toggle_maximized(p);
            erase_window(p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
            uncover(p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
            dsk.snd.panel.olvidar();
            scene::sound::paint(p, &dsk.win.sound, &dsk.snd.panel);
            dsk.win.top_before = Ventana::Sound;
            return true;
        }
        None => {}
    }
    if s.en_el_mudo(x, y) {
        let mudo = scene::sound::leer().mudo;
        callar(dsk, p, !mudo);
        return true;
    }
    if s.en_el_fader(x, y) {
        dsk.snd.panel.arrastrando = true;
        mover(dsk, p, s.db_en(y));
        return true;
    }
    false
}

/// **Hace falta pintar por el sonido?** Se pregunta en cada vuelta del bucle,
/// y por eso NO cruza ninguna puerta: mira el reloj contra una cifra.
///
/// Solo pide fotograma mientras SUENA. En silencio contesta que no y el
/// indicador se pone al dia con el cuarto de segundo que el escritorio ya
/// pinta: meter el sonido en `actividad` reiniciaria el reposo del bucle, que
/// es exactamente el fallo W4b del plan de vatios.
pub(crate) fn toca(dsk: &Desktop) -> bool {
    dsk.snd.panel.activo && bmo::ciclos() >= dsk.snd.panel.proximo
}

/// **El refresco**: lee el kernel, apunta la marca de pico y la luz, y pinta el
/// indicador de la barra y las partes vivas del panel que CAMBIARON.
pub(crate) fn latido(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let ahora = bmo::ciclos();
    if ahora < dsk.snd.panel.proximo {
        return;
    }
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    let l = scene::sound::leer();
    dsk.snd.panel.observar(&l, ahora, hz);
    dsk.snd.panel.proximo = ahora + if dsk.snd.panel.activo { hz / 20 } else { hz / 4 };
    scene::sound::barra(p, &l);
    if dsk.win.sound_open && !dsk.win.sound.chrome.minimized && !tapada(dsk) {
        scene::sound::vivo(p, &dsk.win.sound, &l, &mut dsk.snd.panel);
    }
}

/// **Alguna ventana delante tapa el panel?** Entonces el refresco no pinta: se
/// pintaria ENCIMA de ella. Vuelve a verse al traerlo delante.
///
/// ** Miraba SOLO la de arriba, y el 23-09 00:06 en el Ryzen eso pinto el fondo
/// del panel encima de CABINA: Ejecutar arriba (sin tocarlo), CABINA en medio
/// (tocandolo). La pregunta es de todas las de delante, y vive en `foco`.
fn tapada(dsk: &Desktop) -> bool {
    crate::desktop::foco::tapada(dsk, Ventana::Sound)
}
