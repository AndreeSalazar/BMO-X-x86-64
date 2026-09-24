//! **Window management without letting go of the keyboard**: Alt+Tab, and
//! CTRL (2026-09-22, it was Super for one afternoon): Ctrl+arrows snaps,
//! Ctrl+Shift+arrows moves, Ctrl+Tab changes the focus mode, Ctrl+F goes full
//! screen, Ctrl+Enter brings Ejecutar, Ctrl+B the side bar, Ctrl+T the tiling.
//! Alt belongs to the app -- except Alt+Tab, Alt+F4 and Alt+Enter, which are
//! already in everyone's fingers.
//!
//! [consumo] NADA      no corre en reposo: lo llama el bucle SOLO si hubo una
//!                     tecla o el raton se movio. Sin entrada, no se entra
//!                     aqui (L6h)
//!
//! These are served BEFORE anything asks about focus, and that is the point --
//! a shortcut that only works once you are already in the window you want is
//! not a shortcut.

use bmo_userland as bmo;

use super::Key;
use crate::desktop::{Desktop, Ventana};
use crate::scene::{self, paint_status, acento};
use crate::{erase_window, uncover};

pub(crate) fn on_key(
    dsk: &mut Desktop,
    p: &bmo::Pantalla,
    c: u8,
    alt_alone: bool,
    m: u8,
) -> Key {
// ** LA TECLA DEL GESTOR ES CTRL (2026-09-22). Fue Super una tarde; el
// propietario la cambio: *"BMO-X va a vivir como el estilo de Windows"*. Ctrl
// ya era del escritorio --`keys::app::del_escritorio` no se lo da a ninguna
// app--, asi que no se le quita nada a nadie.
//
// [!] Ctrl SIN Alt: en castellano `Ctrl+Alt` ES AltGr (la arroba, la
// almohadilla, los corchetes), y un atajo que saltara ahi se comeria esos
// caracteres.
//
// [!] Y las letras llegan COCIDAS: el kernel convierte Ctrl+letra en su codigo
// de control (Ctrl+B = 0x02, Ctrl+T = 0x14; ver `keyboard::feed_full`). Por eso
// se compara con el codigo y no con la letra. Lo que eso impide, dicho: Ctrl+M
// es el mismo byte que Enter y Ctrl+I el mismo que Tab, asi que el modo del foco
// va con Ctrl+Tab. Y Ctrl+W ya borra una palabra en Ejecutar: cerrar es Ctrl+Q.
let ctrl = m & bmo::MOD_CTRL != 0 && m & bmo::MOD_ALT == 0;
// ** IMPR PANT: la captura (2026-09-22). La primera, porque es la unica tecla
// que tiene que funcionar este donde este el foco -- se pulsa para guardar lo
// que se ve, no para hablarle a una ventana. Con Alt, solo la de delante.
// ** Y MIENTRAS SE RECORTA todas las teclas son del recorte: ESC cancela y
// las demas se tiran, para que no escriban detras del rectangulo.
if crate::desktop::captura::recortando() {
    if c == 0x1B {
        crate::desktop::captura::cancelar(dsk, p);
    }
    return Key::Taken;
}
// ** EL RECORTE: Ctrl+Shift+S (el Win+Shift+S de Windows, con la tecla del
// gestor de aqui) o Shift+Impr Pant. Ctrl+S llega cocida como 0x13.
if (ctrl && c == 0x13 && m & bmo::MOD_SHIFT != 0)
    || (c == crate::desktop::captura::TECLA_IMPR && m & bmo::MOD_SHIFT != 0)
{
    crate::desktop::captura::empezar(dsk);
    return Key::Taken;
}
if c == crate::desktop::captura::TECLA_IMPR {
    crate::desktop::captura::tomar(dsk, p, m & bmo::MOD_ALT != 0);
    return Key::Taken;
}
if alt_alone && c == 0x09 {
    if m & bmo::MOD_SHIFT != 0 {
        dsk.win.focus.conmutar_atras();
    } else {
        dsk.win.focus.conmutar();
    }
    scene::switcher::paint(
        &p,
        dsk.win.focus.lista(),
        dsk.win.focus.pointed_index(),
        dsk.win.focus.modo().name(),
    );
    dsk.win.switcher_painted = true;
    return Key::Taken;
}
// -- Alt+M: cambiar el MODO del foco --
//
// Sin una tecla, los tres modos son decoracion: `Fijo` y
// `Puntero` existirian sin forma de llegar a ellos. Va con Alt
// por lo mismo que el Tab --`Alt` solo no produce caracter en
// ninguna distribucion, `Ctrl+Alt` SI (es AltGr)-- y se anuncia
// en la propia ventanita, que es donde se lee el modo.
// ** CTRL+TAB desde el 22-09: Alt+Tab elige ventana y Ctrl+Tab elige COMO la
// sigue el foco -- la misma tecla para las dos preguntas del foco.
if ctrl && c == 0x09 {
    dsk.win.focus.poner_modo(dsk.win.focus.modo().next());
    if dsk.win.switcher_painted {
        scene::switcher::paint(
            &p,
            dsk.win.focus.lista(),
            dsk.win.focus.pointed_index(),
            dsk.win.focus.modo().name(),
        );
    } else if dsk.win.visible {
        // Cambiarlo sin el conmutador abierto tambien tiene que
        // verse: un modo que cambia en silencio se descubre
        // cuando el teclado ya se fue a otra ventana.
        paint_status(&p, &dsk.run_box, dsk.win.focus.modo().nombre_largo(), acento());
    }
    return Key::Taken;
}
// -- ** ALT+ENTER: PANTALLA COMPLETA, Y EN TIEMPO REAL --
//
// No es maximizar. Maximizar deja la barra a la vista a proposito --lo dice
// `toggle_maximized`-- y esto se la come: sin marco, sin botones, el panel
// entero y la superficie centrada. Es la configuracion de *pantalla
// completa* de un juego, puesta y quitada sin relanzar nada.
//
// ** Y SOLO PARA LAS APPS. Una ventana del sistema a pantalla completa
// taparia la barra y dejaria al propietario sin sitio donde volver, que es el
// mismo motivo por el que el maximizado la respeta. Aqui la salida es la
// MISMA tecla con la que se entro, y eso hace el gesto simetrico.
//
// Va con `Alt` por lo mismo que el Tab y la M, y por una razon mas: es el
// atajo que ya esta en los dedos de cualquiera que haya jugado a algo.
// ** CTRL+B: LA BARRA LATERAL, fuera o dentro (HUD 3). Cambia el area util y
// la rejilla, asi que se repinta el escritorio entero, y las ventanas que la
// columna pisaria se corren (`fit` ya lee el tope nuevo).
if ctrl && c == 0x02 {
    scene::lateral::alternar();
    crate::desktop::lateral_cambio(dsk, &p, "panel");
    return Key::Taken;
}
// ** CTRL+T: EL MOSAICO, puesto o quitado (HUD 4). Se dice en la linea de
// estado, porque un modo que cambia en silencio se descubre tarde.
if ctrl && c == 0x14 {
    crate::desktop::mosaico::alternar(dsk, &p);
    let dice = if crate::desktop::mosaico::encendido() {
        "mosaico: las ventanas se reparten la pantalla (Ctrl+T lo quita)"
    } else {
        "mosaico quitado: las ventanas se quedan donde estan"
    };
    paint_status(&p, &dsk.run_box, dice, acento());
    return Key::Taken;
}
// ** CTRL+ENTER: EJECUTAR, delante y con el teclado. Es el "abre la terminal"
// de Hyprland, y aqui la terminal es la casa. (Ctrl+M y Ctrl+J llegan con el
// mismo byte y hacen lo mismo: no hay forma de distinguirlos, y no hace falta.)
if ctrl && (c == 0x0D || c == 0x0A) {
    if !dsk.win.visible {
        dsk.win.visible = true;
    }
    dsk.win.focus.open(Ventana::Run);
    dsk.win.focus.clic_en(Ventana::Run);
    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
    paint_status(&p, &dsk.run_box, "listo", acento());
    dsk.win.top_before = Ventana::Run;
    dsk.win.taskbar_dirty = true;
    return Key::Taken;
}
// ** Y CTRL+F es el mismo gesto que Alt+Enter: pantalla completa (la F de
// Hyprland). Se quedan los dos: uno para los dedos de los juegos, otro para
// los del gestor. SOBRE UNA APP: sin app marcada la tecla pasa, y en la caja
// de Ejecutar Ctrl+F es BUSCAR (24-09, `keys::editor`).
if (alt_alone && (c == 0x0D || c == 0x0A)) || (ctrl && c == 0x06) {
    if let Some(Ventana::App(i)) = dsk.win.focus.pointed_at() {
        if let Some((_viejo, completa)) = dsk.table.pantalla_completa(i as usize, p) {
            if completa {
                // ** EL NEGRO ES DEL DIRECTOR, no de la app. Lo que sobra
                // alrededor de una superficie de 960x600 en un panel de 1920
                // no es de nadie, y dejarlo con lo que hubiera debajo seria
                // mostrar trozos del escritorio alrededor del juego.
                p.rect(0, 0, p.ancho, p.alto, 0);
                p.vaciar();
            } else {
                // Al salir se devuelve el escritorio ENTERO: la ventana
                // tapaba la barra y los iconos, asi que repintar solo su
                // hueco dejaria media pantalla en negro. Es el mismo camino
                // que al devolver una pantalla prestada.
                crate::repintar_escritorio(p, dsk, "pantalla completa: fuera");
            }
            return Key::Taken;
        }
    }
    // Sin app marcada la tecla NO se come: un Enter a secas es lo que
    // entrega la linea de ordenes, y comerselo seria romper la terminal.
    return Key::Pass;
}
// -- ** ALT+FLECHAS: MOVER Y ENCAJAR SIN SOLTAR EL TECLADO --
//
// Alt+Tab ya elegia ventana y no podia hacer nada con ella. Esto
// cierra el gesto: se elige con Tab y se coloca con las flechas,
// sin que la mano salga del teclado.
//
// * **A secas mueve; con Shift encaja** -- media pantalla a los
// lados, el panel entero arriba, y abajo deshace el maximizado.
// Es lo que hace Windows con la tecla de la ventanita, y se
// copia el reparto a proposito: un atajo de colocar ventanas que
// no es el que ya tienes en los dedos se usa una vez.
//
// Va con `Alt` por lo mismo que el Tab y la M, y esta escrito
// dos lineas mas arriba: `Alt` solo no produce caracter en
// ninguna distribucion y `Ctrl+Alt` SI, porque es AltGr.
//
// [!] Se atiende ANTES que las flechas de las ventanas, y por eso
// no les quita nada: sin `Alt` esto no entra, y las flechas de
// Datos y el volumen de Sonido siguen llegando enteras.
// ** Y DESDE EL 2026-09-22 VA CON CTRL, y se invierte el reparto: a secas
// ENCAJA (el Win+flechas de Windows 7) y con Shift mueve. Con Alt estas flechas
// eran el ladeo de DOOM, que nunca le llegaba. En Ejecutar, Ctrl+arriba y
// Ctrl+abajo copiaban y pegaban: copiar es Ctrl+Shift+C y pegar Ctrl+V.
if ctrl && (0x80..=0x83).contains(&c) {
    use scene::chrome::Heading;
    let heading = match c {
        0x80 => Heading::Up,
        0x81 => Heading::Down,
        0x82 => Heading::Left,
        _ => Heading::Right,
    };
    let fit = m & bmo::MOD_SHIFT == 0;
    let mut moved = false;
    // -- ** SE MUEVE LA MARCADA, NO LA QUE TIENE EL FOCO --
    //
    // `focus.actual()` parece lo obvio y es justo lo que no vale:
    // **no cambia mientras conmutas**, a proposito --lo dice su
    // propia documentacion-- porque una letra escrita a mitad de
    // un Alt+Tab no puede caer en una ventana que todavia no has
    // elegido.
    //
    // Pero estas flechas se pulsan CON EL ALT PULSADO, que es
    // exactamente "a mitad de un Alt+Tab". Con `actual()`, elegir
    // CABINA con Tab y darle a la flecha moveria la ventana
    // ANTERIOR -- se veria moverse la que no es, que es peor que
    // no moverse nada.
    //
    // `pointed_at()` contesta las dos situaciones con una regla:
    // conmutando es la resaltada, y sin conmutar es la que ya
    // tiene el foco. La que se mueve es **la que estas mirando en
    // la ventanita**, y eso se puede explicar en una frase.
    match dsk.win.focus.pointed_at() {
        // Mover una app con el teclado pide traducir la tecla a su marco, y
        // eso es del paso 2c. Hoy se dice que no en vez de moverla a medias.
        Some(Ventana::App(_)) => {}
        Some(Ventana::Data) => {
            if dsk.win.data_open && !dsk.win.data.chrome.minimized {
                let (vx, vy, va, vl) = (
                    dsk.win.data.x(), dsk.win.data.y(),
                    dsk.win.data.width(), dsk.win.data.height(),
                );
                let cambio = if fit {
                    dsk.win.data.chrome.snap(&p, heading)
                } else {
                    dsk.win.data.chrome.push(&p, heading)
                };
                if cambio {
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    // Encajar CAMBIA el medida, asi que las cajas del
                    // grafo hay que recolocarlas: sin esto la ventana
                    // mide una cosa y su contenido sigue midiendo otra.
                    dsk.win.data.relayout();
                    scene::data::paint(&p, &dsk.win.data);
                    dsk.win.top_before = Ventana::Data;
                    moved = true;
                }
            }
        }
        Some(Ventana::Cabina) => {
            if dsk.win.cabina_open && !dsk.win.cabina.chrome.minimized {
                let (vx, vy, va, vl) = (
                    dsk.win.cabina.chrome.x, dsk.win.cabina.chrome.y,
                    dsk.win.cabina.chrome.width, dsk.win.cabina.chrome.height,
                );
                let cambio = if fit {
                    dsk.win.cabina.chrome.snap(&p, heading)
                } else {
                    dsk.win.cabina.chrome.push(&p, heading)
                };
                if cambio {
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    scene::cabina::paint(&p, &dsk.win.cabina);
                    dsk.win.top_before = Ventana::Cabina;
                    moved = true;
                }
            }
        }
        // ESTRUCTURA se mueve con Alt+flechas como las demas que llevan
        // `Chrome`. Nace con esto y no como deuda: la ventana del taller es
        // justo la que se quiere apartar para mirar otra cosa mientras compila.
        Some(Ventana::Estructura) => {
            if dsk.win.estructura_open && !dsk.win.estructura.chrome.minimized {
                let (vx, vy, va, vl) = (
                    dsk.win.estructura.chrome.x, dsk.win.estructura.chrome.y,
                    dsk.win.estructura.chrome.width, dsk.win.estructura.chrome.height,
                );
                let cambio = if fit {
                    dsk.win.estructura.chrome.snap(&p, heading)
                } else {
                    dsk.win.estructura.chrome.push(&p, heading)
                };
                if cambio {
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    scene::estructura::paint(&p, &dsk.win.estructura);
                    dsk.win.top_before = Ventana::Estructura;
                    moved = true;
                }
            }
        }
        Some(Ventana::Sound) => {
            if dsk.win.sound_open && !dsk.win.sound.chrome.minimized {
                let (vx, vy, va, vl) = (
                    dsk.win.sound.chrome.x, dsk.win.sound.chrome.y,
                    dsk.win.sound.chrome.width, dsk.win.sound.chrome.height,
                );
                let cambio = if fit {
                    dsk.win.sound.chrome.snap(&p, heading)
                } else {
                    dsk.win.sound.chrome.push(&p, heading)
                };
                if cambio {
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    scene::sound::paint(&p, &dsk.win.sound, &dsk.snd.panel);
                    dsk.win.top_before = Ventana::Sound;
                    moved = true;
                }
            }
        }
        // ** LA TERMINAL, que hasta el 2026-08-16 no se movia.
        //
        // Aqui decia *"Ejecutar no se mueve: es el escritorio, no
        // una ventana"*. Era la descripcion de una limitacion
        // escrita como si fuera un principio -- y ni siquiera era
        // cierta: tenia barra de titulo, sombra y esquinas
        // redondeadas como las demas, solo que no se podia agarrar.
        // El propietario lo dijo mirandola: *"me gustaria que sea
        // movible"*.
        // ** ESTA RAMA DECIA `Some(W_RUN)` Y NO ERA ESTA RAMA.
        //
        // `W_RUN` era una constante que este fichero **no importaba**, y un
        // nombre desconocido en un patron de Rust no es una constante: es una
        // VARIABLE nueva que casa con todo. Asi que con las vitales marcadas
        // --o con cualquier ventana cuya guarda fallara-- la flecha movia la
        // TERMINAL, y la linea de abajo guardaba en `top_before` el id que
        // hubiera casado en vez del de Ejecutar.
        //
        // El compilador lo estuvo diciendo todo el tiempo, en un aviso que no
        // parece lo que es: `variable W_RUN should have a snake case name`.
        //
        // ** Y CON `Ventana` ESO NO SE PUEDE ESCRIBIR. Un patron con `::`
        // nunca es un enlace: si el tipo no esta importado no compila, en vez
        // de tragarse todos los casos en silencio. La clase entera de fallo se
        // fue con las constantes sueltas.
        Some(Ventana::Run) => {
            if dsk.win.visible {
                let (vx, vy, va, vl) = (
                    dsk.run_box.x, dsk.run_box.y,
                    dsk.run_box.w(), dsk.run_box.h(),
                );
                let cambio = if fit {
                    dsk.run_box.chrome.snap(&p, heading)
                } else {
                    dsk.run_box.chrome.push(&p, heading)
                };
                if cambio {
                    // El orden importa y es distinto del de las otras
                    // tres: ahi `uncover` repinta la terminal, que no
                    // se habia movido. Aqui la que se movio ES la
                    // terminal, asi que primero se recolocan sus
                    // medidas y solo despues se borra y se repinta --
                    // al reves, `erase_window` preguntaria por el
                    // color de fondo con la geometria vieja y dejaria
                    // el rastro que este mismo fichero ya cazo tres
                    // veces.
                    dsk.run_relayout(&p);
                    erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
                    uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
                    dsk.win.top_before = Ventana::Run;
                    moved = true;
                }
            }
        }
        // [!] LAS VITALES NO SE MUEVEN CON EL TECLADO, y aqui lo pone.
        //
        // Es el mismo hueco que en el raton: F7 y F8 tienen marco, titulo y
        // un pie que anuncia "arrastra el titulo", y no las mueve nada. Antes
        // caian en un `_ => {}` donde no se distinguian de "no hay foco"; con
        // el `match` sin comodin son un caso con nombre, y el dia que se
        // arreglen no hay que buscar donde.
        Some(Ventana::Cpu) | Some(Ventana::Mem) => {}
        // Sin foco no hay a quien mover. La tecla se come igual: dejarla
        // pasar mandaria un Alt+flecha a la linea de comandos.
        None => {}
    }
    // La ventana se acaba de pintar ENCIMA del conmutador, que
    // esta en el centro. Sin esto, mover tapa la ventanita que
    // dice cual estas moviendo -- y a la segunda flecha ya no
    // sabes en cual estas. Al soltar Alt se repinta todo de abajo
    // arriba, asi que el destrozo se repara solo; lo que hay que
    // arreglar es lo que se ve MIENTRAS.
    if moved && dsk.win.switcher_painted {
        scene::switcher::paint(
            &p,
            dsk.win.focus.lista(),
            dsk.win.focus.pointed_index(),
            dsk.win.focus.modo().name(),
        );
    }
    return Key::Taken;
}
    Key::Pass
}
