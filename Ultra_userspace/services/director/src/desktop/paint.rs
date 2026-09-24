//! **Closing the frame**: drain the child, the taskbar chips, the caret, the
//! apps, the vitals, and the mouse cursor on top of everything.
//!
//! [consumo] NADA      no corre en reposo: solo cuando alguien lo pide, o en
//!                     el arranque (L6h)
//!
//! 245 lines, and they need **three names**: the desktop, the screen and how
//! many app windows died this turn. That is the smallest signature of any
//! block in the loop, and the reason is the order itself -- everything in here
//! happens once the decisions are already made.
//!
//! ## The order is the design
//!
//! - The dead windows' holes are given back BEFORE composing the live ones.
//!   Erasing afterwards would cover a window that is still there.
//! - The apps go LAST, for the same reason the cursor does: what is painted
//!   last is what ends up on top.
//! - The vitals repaint before the cursor, because anything painted after the
//!   cursor eats its save-under.
//! - `p.vaciar()` is the final instruction. The framebuffer is mapped
//!   write-combining, so without it what was painted this frame sits waiting
//!   for someone to write more -- the exact symptom seen on the Ryzen, where
//!   typing painted nothing until the mouse moved.

use bmo_userland as bmo;

use super::{BLINK, Desktop, Ventana};
use crate::scene::calc::paint_calc;
use crate::scene::output::paint_output;
use crate::scene::{self, paint_field, paint_status, acento, INK_BAD};
use crate::{erase_window, uncover};

/// **La terminal pinto: las apps que la tapan se vuelven a pegar.**
///
/// ** Visto en el Ryzen el 2026-09-12: la barra de texto de Ejecutar, con su
/// cursor, ASOMABA en medio de DOOM. El campo parpadea y la rejilla escribe
/// por su cuenta, las dos ANTES de componer las apps -- y una superficie solo
/// se repega cuando su secuencia cambia. DOOM entrega unas 58 veces por
/// segundo contra 112 vueltas del DIRECTOR: la mitad de los fotogramas se
/// quedaba el campo encima.
///
/// Cuesta una copia de la caja por parpadeo, no por vuelta. Es lo mismo que
/// ya hace quien llama a `uncover`: lo pintado debajo obliga a repintar lo de
/// encima.
fn repintar_apps_encima(dsk: &mut Desktop) {
    let (x, y, w, h) = (dsk.run_box.x, dsk.run_box.y, dsk.run_box.w(), dsk.run_box.h());
    for s in dsk.table.iter_mut() {
        let c = &s.chrome;
        let se_tocan = !c.minimized
            && c.x < x + w
            && x < c.x + c.width
            && c.y < y + h
            && y < c.y + c.height;
        if se_tocan {
            s.repaint_all();
        }
    }
}

/// **Devuelve las ventanas del sistema que un borrado destapo** (2026-09-13).
///
/// Visto en el Ryzen: mover el cubo por encima de la biblioteca dejaba la FOTO
/// de fondo donde estaba la lista. Ver `scene::perjuicio`, que es quien apunta.
///
/// El orden es el Z-order: **de atras hacia delante segun la lista del foco**
/// (`foco::de_atras_adelante`), y las apps se recomponen al final porque van
/// encima de todas. Hasta el 23-09 era "todas las que no son la de arriba, en
/// el orden del enum, y la de arriba al final": con tres ventanas pisandose, la
/// de en medio podia salir encima de la de delante.
fn devolver(dsk: &mut Desktop, p: &bmo::Pantalla) {
    if !scene::dirty::hay() {
        return;
    }
    let toca = |dsk: &Desktop, v: Ventana| -> bool {
        let caja = |c: &scene::chrome::Chrome| !c.minimized && scene::dirty::toca(c.x, c.y, c.width, c.height);
        match v {
            Ventana::Data => dsk.win.data_open && caja(&dsk.win.data.chrome),
            Ventana::Cabina => dsk.win.cabina_open && caja(&dsk.win.cabina.chrome),
            Ventana::Sound => dsk.win.sound_open && caja(&dsk.win.sound.chrome),
            Ventana::Estructura => dsk.win.estructura_open && caja(&dsk.win.estructura.chrome),
            Ventana::Run => {
                dsk.win.visible
                    && scene::dirty::toca(dsk.run_box.x, dsk.run_box.y, dsk.run_box.w(), dsk.run_box.h())
            }
            // Las vitales se repintan solas cada 15 vueltas; las apps, abajo.
            _ => false,
        }
    };
    let pintar = |dsk: &mut Desktop, v: Ventana| match v {
        Ventana::Data => scene::data::paint(p, &dsk.win.data),
        Ventana::Cabina => scene::cabina::paint(p, &dsk.win.cabina),
        Ventana::Sound => scene::sound::paint(p, &dsk.win.sound, &dsk.snd.panel),
        Ventana::Estructura => scene::estructura::paint(p, &dsk.win.estructura),
        // ** La CAJA y no `uncover`: `uncover` pinta tambien los iconos, que
        // son la capa de abajo, y aqui se esta pintando de atras hacia delante
        // -- los iconos saldrian encima de las ventanas ya devueltas.
        Ventana::Run => {
            scene::paint_run_box(p, &dsk.run_box);
            dsk.tick.repaint_field = true;
            dsk.out.grid.dirty = true;
        }
        _ => {}
    };
    let (orden, n) = super::foco::de_atras_adelante(dsk);
    let mut algo = false;
    for &v in &orden[..n] {
        if toca(dsk, v) {
            pintar(dsk, v);
            algo = true;
        }
    }
    if algo {
        for s in dsk.table.iter_mut() {
            s.repaint_all();
        }
    }
    scene::dirty::olvidar();
}

/// Everything that happens after the input has been read and understood.
/// **Pinta UNA ventana del sistema**, si esta abierta. Las apps no: sus
/// pixeles los pega `compose` y su marco la mesa de superficies.
///
/// Vivia como un cierre dentro de `keys::edges` (el Alt que se suelta), y el
/// borde de foco necesitaba lo mismo: dos copias de "como se pinta cada
/// ventana" es la lista escrita a mano que se queda corta con la ventana
/// siguiente. El `match` no lleva `_`: agregar una ventana no compila hasta
/// decir como se pinta.
pub(crate) fn pintar_ventana(dsk: &mut Desktop, p: &bmo::Pantalla, v: Ventana) {
    if !dsk.win.abierta(v) {
        return;
    }
    match v {
        Ventana::App(_) => {}
        Ventana::Cabina => scene::cabina::paint(p, &dsk.win.cabina),
        Ventana::Data => scene::data::paint(p, &dsk.win.data),
        Ventana::Estructura => scene::estructura::paint(p, &dsk.win.estructura),
        Ventana::Cpu => scene::vitals::paint(p, &dsk.win.cpu, dsk.tick.loops_per_second, dsk.tick.consumo.ultimo),
        Ventana::Mem => scene::vitals::paint(p, &dsk.win.mem, dsk.tick.loops_per_second, dsk.tick.consumo.ultimo),
        Ventana::Sound => scene::sound::paint(p, &dsk.win.sound, &dsk.snd.panel),
        Ventana::Run => uncover(
            p,
            &dsk.run_box,
            &dsk.launcher,
            dsk.win.visible,
            &mut dsk.out.grid,
            &mut dsk.tick.repaint_field,
        ),
    }
}

pub(crate) fn compose(dsk: &mut Desktop, p: &bmo::Pantalla, dead: usize) {
    // Aqui y no antes: `will_paint` no es definitivo hasta que la recogida de
    // entrada termina, y ella lo puede subir. Ver `Tick::pintados_por_segundo`.
    dsk.tick.anota_pintado();
    // ** LO QUE NO SE VE, NO SE PINTA -- y el DIRECTOR se lo aplica a SI
    // MISMO (2026-09-11). Con una ventana a pantalla completa, su mobiliario
    // --la barra, la terminal, la salida-- esta debajo. Pintarlo no solo
    // gasta: ASOMA encima del juego en el primer fotograma en que la app no
    // entregue uno nuevo, porque una superficie solo se repega cuando su
    // secuencia cambia.
    let fs = dsk.table.alguna_a_pantalla_completa();
    // -- Drenar la salida de los hijos --
    //
    // Con tope por fotograma. Un programa que escupe sin parar podria
    // quedarse con el bucle entero y congelar el cursor: es preferible que
    // la salida vaya un poco por detras a que el escritorio deje de
    // responder. Lo que no se lea ahora sigue en el anillo del kernel.
    if let Some(c) = dsk.out.console.as_ref() {
        let mut buf = [0u8; 8];
        let mut drained = 0;
        // Si el bucle acaba por FALTA de bytes y no por el tope, el anillo
        // quedo vacio. Hace falta saberlo abajo: sin eso, cancelar la espera
        // podria tirar una respuesta que todavia estaba en la cola.
        let mut vacia = false;
        while drained < 64 {
            let read_bytes = c.read(&mut buf);
            if read_bytes == 0 {
                vacia = true;
                break;
            }
            if dsk.calc.waiting {
                // Todo lo que escriba el motor es la respuesta: el
                // programa no imprime prompts a proposito.
                for &b in &buf[..read_bytes] {
                    if b == b'\n' {
                        // ** LA PRIMERA LINEA ES EL ESTADO, NO LA RESPUESTA.
                        //
                        // El motor contesta `estado \n valor \n`. Antes mandaba
                        // una sola linea y ESTE bucle daba por hecho que era un
                        // numero -- asi que un "codigo no valido" del motor se
                        // pintaba en la pantallita como si fuera una cifra.
                        //
                        // Con la tecla `$` eso deja de poder arreglarse
                        // mirando: `$12,345.67` es una respuesta BUENA y no
                        // parece un numero. Quien sabe si contesto es el motor,
                        // y por eso lo dice el en vez de adivinarlo nosotros.
                        if !dsk.calc.respondio {
                            dsk.calc.respondio = true;
                            dsk.calc.bien = dsk.resp_n > 0 && dsk.resp[0] == b'0';
                            dsk.resp_n = 0;
                            continue;
                        }
                        if dsk.resp_n > 0 {
                            dsk.calc.input = [0; 20];
                            let k = dsk.resp_n.min(dsk.calc.input.len());
                            dsk.calc.input[..k].copy_from_slice(&dsk.resp[..k]);
                            dsk.calc.n = k;
                            dsk.calc.saved_n = 0;
                            dsk.calc.op = 0;
                            dsk.calc.waiting = false;
                            dsk.calc.respondio = false;
                            // Lo que no es una cifra es una PRESENTACION (`$`)
                            // o un motivo: en los dos casos, teclear encima
                            // empieza de cero en vez de agregar al final.
                            dsk.calc.presentado = !dsk.calc.bien
                                || dsk.calc.input[..k].iter().any(|c| {
                                    !c.is_ascii_digit() && *c != b'.' && *c != b'-'
                                });
                            // * El cursor SE APARTA antes de pintar aqui.
                            //
                            // Este es el unico pintado del bucle que no
                            // dispara la ENTRADA: lo dispara el HIJO al
                            // contestar. Asi que puede caer en un fotograma
                            // con `will_paint` en falso -- o sea con el
                            // puntero todavia en pantalla y lo que hay
                            // debajo ya guardado. Pintar encima **caduca**
                            // ese guardado, y el `lift` de la vuelta
                            // siguiente devolveria los pixeles viejos
                            // encima del resultado recien escrito: un
                            // rectangulo fantasma sobre la calculadora.
                            //
                            // `lift` es idempotente --si no esta puesto no
                            // hace nada--, asi que llamarlo aqui no cuesta
                            // nada en los fotogramas que ya lo apartaron.
                            dsk.save_under.lift(&p);
                            paint_calc(&p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
                        }
                    } else if dsk.resp_n < dsk.resp.len() && b >= 0x20 {
                        dsk.resp[dsk.resp_n] = b;
                        dsk.resp_n += 1;
                    }
                }
            } else {
                dsk.out.grid.text(&buf[..read_bytes]);
            }
            drained += 1;
        }

        // ** Y SI EL MOTOR SE FUE SIN CONTESTAR, LA ESPERA SE ACABA SOLA.
        //
        // El 2026-08-18, en metal, el motor equivocado nunca mando su segunda
        // linea y la calculadora se quedo esperando **para siempre**. Con las
        // teclas suyas, eso dejo el escritorio sin teclado.
        //
        // `has_child` es exacto y no necesita reloj --que aqui no hay--: dice
        // si queda alguien escribiendo en esta consola. Se pregunta solo con el
        // anillo VACIO, porque un hijo que ya termino puede haber dejado su
        // respuesta en la cola, y cancelar entonces seria tirarla.
        if dsk.calc.waiting && vacia && !c.has_child() {
            dsk.calc.clear();
            paint_status(&p, &dsk.run_box, "el motor se fue sin contestar", INK_BAD);
            dsk.save_under.lift(&p);
            paint_calc(&p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
        }
    }
    // * Y solo en un fotograma que haya apartado el cursor. Un hijo que
    // escribe no es motivo suficiente: pintar aqui dejaria el puntero
    // enterrado bajo la rejilla y, al quitarlo, devolveria pixeles viejos
    // encima de lo recien escrito. `dirty` se queda puesto y la vuelta
    // siguiente ya empieza sabiendo que hay que pintar.
    if dsk.out.grid.dirty && dsk.tick.will_paint && !fs {
        // Se pinta solo si se ve; el contenido sigue acumulandose oculto,
        // asi que al invocar la ventana esta todo lo que paso mientras.
        //
        // * Y NO si la consola de datos esta ARRIBA. Sin este guardia, el
        // fotograma siguiente repintaria la rejilla POR DEBAJO y la
        // dibujaria encima de la ventana de datos, dejandola a trozos. La
        // salida no se pierde: `dirty` se queda puesto y se pinta entera
        // cuando esta ventana vuelva a estar arriba.
        //
        // Y es ARRIBA, no ABIERTA: con Datos abierta pero detras, la
        // rejilla se ve y tiene que seguir escribiendose.
        if dsk.win.visible && dsk.win.top_before != Ventana::Data && !dsk.win.switcher_painted && !dsk.win.nya_painted {
            paint_output(&p, &dsk.run_box, &dsk.out.grid);
            dsk.out.grid.dirty = false;
            repintar_apps_encima(dsk);
        } else if !dsk.win.visible {
            dsk.out.grid.dirty = false;
        }
    }

    // -- Las FICHAS del panel --
    //
    // Son la lista de lo que hay abierto, y la unica forma de volver a una
    // ventana minimizada con el raton. Iban en la barra de arriba; desde el
    // 2026-09-22 van en vertical en el panel de la izquierda, que las repinta
    // SOLO si la lista cambio (`lateral::fichas`).
    //
    // * Lo que las ensucia se calcula AQUI, comparando el estado con el del
    // fotograma anterior, en vez de poner `taskbar_dirty = true` en los seis
    // sitios que cambian algo. Un `sucio` que hay que acordarse de poner es
    // un `sucio` que un dia no se pone, y entonces la barra muestra un
    // estado viejo sin que nada falle -- el peor tipo de fallo de interfaz.
    let taskbar_state = (
        dsk.win.visible,
        dsk.win.top_before,
        dsk.win.data_open,
        dsk.win.data.chrome.minimized,
        dsk.win.cabina_open,
        dsk.table.estado_fichas(),
    );
    if taskbar_state != dsk.win.taskbar_state_before {
        dsk.win.taskbar_state_before = taskbar_state;
        dsk.win.taskbar_dirty = true;
    }
    if dsk.win.taskbar_dirty && dsk.tick.will_paint && !fs {
        use scene::lateral::{Ficha, MAX_FICHAS};
        let mut lista = [Ficha::VACIA; MAX_FICHAS];
        let mut n = 0usize;
        let mut pon = |f: Ficha| {
            if n < MAX_FICHAS {
                lista[n] = f;
                n += 1;
            }
        };
        pon(Ficha {
            v: Ventana::Run,
            nombre: "Ejecutar",
            color: acento(),
            activa: dsk.win.visible && dsk.win.top_before == Ventana::Run,
            minimizada: !dsk.win.visible,
        });
        // ESTRATOS solo mientras esta abierta: una ficha que se queda tras
        // cerrar la ventana promete algo que ya no esta.
        if dsk.win.data_open {
            pon(Ficha {
                v: Ventana::Data,
                nombre: "ESTRATOS",
                color: 0x0034_D399,
                activa: dsk.win.top_before == Ventana::Data,
                minimizada: dsk.win.data.chrome.minimized,
            });
        }
        // -- ** CABINA: LA UNICA FICHA QUE ESTA SIEMPRE --
        //
        // Las otras aparecen cuando su ventana existe. Esta no, y el motivo es
        // el dia que la puso: **el teclado dejo de escribir y con el se fue la
        // unica forma de diagnosticarlo**. Un panel de diagnostico al que solo
        // se llega con el aparato que puede estar roto no es un panel de
        // diagnostico. Asi que la ficha esta aunque la ventana este cerrada:
        // es la puerta, no el recordatorio.
        pon(Ficha {
            v: Ventana::Cabina,
            nombre: "CABINA",
            color: 0x00F5_9E0B,
            activa: dsk.win.cabina_open && dsk.win.top_before == Ventana::Cabina,
            minimizada: !dsk.win.cabina_open,
        });
        // -- ** LAS FICHAS DE LAS APPS (2026-09-12) --
        //
        // Una por app abierta, detras de CABINA. Sin ellas, minimizar una app
        // era perderla: DOOM minimizado no tenia ficha ni volvia con Alt+Tab.
        let (fichas, k) = dsk.table.fichas();
        for &hueco in &fichas[..k] {
            let v = Ventana::App(hueco as u8);
            pon(Ficha {
                v,
                nombre: v.nombre(),
                color: 0x0060_A5FA,
                activa: dsk.win.focus.actual() == Some(v),
                minimizada: dsk.table.minimizada(hueco),
            });
        }
        scene::lateral::fichas(&lista[..n]);
        dsk.win.taskbar_dirty = false;
    }
    // ** EL PANEL (HUD 3 y 5): lo que haya que repintar de el --entero si se
    // dio por perdido, las fichas si cambiaron, el reloj si cambio el minuto--
    // y cuatro veces por segundo la muestra de sus instrumentos.
    if dsk.tick.will_paint && !fs {
        scene::lateral::latido(&p, dsk.tick.consumo.ultimo.map(|c| c.mw_paquete), &dsk.tick.lectura_pulso());
        // Y el MAESTRO: su indicador al pie del panel y, si su ventana esta
        // abierta, su medidor. Se mira a su propio ritmo, no al del panel.
        crate::desktop::sonido::latido(dsk, &p);
    }

    // El parpadeo del cursor de escritura. Solo repinta cuando cambia de
    // estado -- repintar el campo cada vuelta seria reescribir la ruta
    // miles de veces por segundo para que se vea igual.
    //
    // * El contador se REINICIA con cada tecla (ver el manejador). Antes
    // era `frames % BLINK`, un reloj que corria solo: si te ponias a
    // escribir justo cuando tocaba apagarlo, el cursor desaparecia a mitad
    // de la palabra y no volvia hasta la siguiente vuelta entera. Un
    // cursor que se esconde mientras escribes es lo contrario de lo que
    // un cursor existe para decir.
    // ** Y AVANZA CON EL RELOJ, NO CON LAS VUELTAS (2026-09-08). `since_key`
    // contaba iteraciones del bucle; ahora cuenta CUARTOS DE SEGUNDO, medidos
    // por `Tick::pulse` con el TSC. Es el mismo movimiento que hicieron las
    // vitales veinte lineas mas abajo, y el porque entero esta en `BLINK`.
    // ** LA RED SE SONDEA SOLA mientras esta armada (2026-09-13). Hasta hoy el
    // anillo solo se vaciaba al teclar `red rx`: entre dos ordenes la tarjeta
    // llenaba sus 16 descriptores y tiraba el resto, y los contadores estaban
    // quietos. Cuatro veces por segundo son dos preguntas al kernel.
    // ** Con un ping en marcha el buzon se mira CADA vuelta: medir un tiempo de
    // ida y vuelta de 250 en 250 ms seria medir al escritorio, no a la red.
    crate::commands::red_nodo::cada_vuelta();
    if dsk.tick.quarter && bmo::info(bmo::INFO_NET_RX_ARMADO) != 0 {
        // Con pase abierto el kernel no sondea por aqui: las tramas van al
        // buzon, y se recogen sin syscall.
        bmo::red::sondear();
        // Y `red prueba` avanza aqui, un cuarto de segundo cada vez.
        crate::commands::red_pase::latir(&mut dsk.out.grid);
    }
    if dsk.tick.quarter {
        dsk.field.since_key = dsk.field.since_key.wrapping_add(1);
        if dsk.field.since_key >= BLINK {
            dsk.field.since_key = 0;
            dsk.field.caret = !dsk.field.caret;
            dsk.tick.repaint_field = true;
        }
    }
    if dsk.tick.repaint_field
        && dsk.tick.will_paint
        && !fs
        && dsk.win.visible
        && dsk.win.top_before != Ventana::Data
        && !dsk.win.switcher_painted && !dsk.win.nya_painted
    {
        paint_field(&p, &dsk.run_box, dsk.field.line(), dsk.field.cur, dsk.field.caret);
        sugerencias(dsk, &p);
        repintar_apps_encima(dsk);
    }

    // * UNA sola vez, al cerrar el primer fotograma entero. Con esto, las
    // ultimas palabras que guarda el kernel dicen DONDE murio sin tener que
    // adivinarlo:
    //
    //   "reclamo pantalla y entrada"  -> murio en el arranque o en la intro
    //   "escritorio pintado"          -> murio sin cerrar el primer cuadro
    //   "primer fotograma completo"   -> murio ya en el bucle
    //
    // Tres mensajes que ya existian mas este, y el diagnostico deja de ser
    // una teoria. Cuesta una linea en el log y se dice una vez en la vida
    // del proceso.
    // -- ** LAS APPS, COMPUESTAS --
    //
    // Va **al final** y por el mismo motivo que el cursor va detras: lo que
    // se pinta al final es lo que queda encima. Una app en su caja esta por
    // delante de las ventanas del sistema, y el unico que se le pone encima
    // es el puntero del raton.
    //
    // El hueco de las que murieron se devuelve ANTES de componer las vivas:
    // borrar despues taparia a una ventana que si esta.
    // Un borrado de esta vuelta obliga a pintar: lo destapado se devuelve abajo.
    if scene::dirty::hay() {
        dsk.tick.will_paint = true;
    }
    if dsk.tick.will_paint {
        for &(vx, vy, va, vl) in dsk.tick.dead_boxes[..dead].iter() {
            erase_window(&p, &dsk.run_box, vx, vy, va, vl, dsk.win.visible);
        }
        if dead > 0 {
            uncover(&p, &dsk.run_box, &dsk.launcher, dsk.win.visible, &mut dsk.out.grid, &mut dsk.tick.repaint_field);
            // Lo que quedara debajo de la que se fue tiene que volver a
            // pintarse: `erase_window` devuelve el FONDO, no las ventanas.
            for s in dsk.table.iter_mut() {
                s.repaint_all();
            }
        }
        devolver(dsk, p);
        dsk.table.compose(&p);
    }

    if dsk.tick.loops == 1 {
        bmo::consola("primer fotograma completo\n");
    }

    // -- ** LAS VITALES SE REPINTAN SOLAS, y eso es lo que las hace vistas
    //
    // Aqui, al final del fotograma y ANTES del cursor: lo que se pinte
    // despues del cursor le come el save-under.
    //
    // Cada 15 vueltas y no cada una, por dos razones que van juntas:
    //
    //   * Un panel de ~500x400 repintado a 60 fps son megabytes por segundo
    //     de volcado por unos numeros que cambian despacio. Es justo el
    //     derroche que el troceado por regiones acaba de quitar en otro
    //     sitio.
    //   * Y los numeros del CPU son MEDIDAS POR DIFERENCIA: con intervalos
    //     de 16 ms la ventana es tan corta que el resultado tiembla. Un cuarto
    //     de segundo es donde un vatio se lee quieto.
    //
    // O sea que refrescar mas no daria mas informacion: daria la misma
    // temblando.
    //
    // ** Y ESO ES EXACTAMENTE LO QUE PASABA. Aqui ponia `frames % 15 == 0`, con
    // un comentario que daba por hecho que quince vueltas eran ~250 ms porque
    // el escritorio iba a 60 por segundo. El bucle no tiene freno: `Tick::pulse`
    // ya lo mide, y el cuarto de segundo lo pone ahora el reloj de referencia.
    if (dsk.win.cpu_open || dsk.win.mem_open) && dsk.tick.quarter {
        if dsk.win.cpu_open {
            scene::vitals::paint(&p, &dsk.win.cpu, dsk.tick.loops_per_second, dsk.tick.consumo.ultimo);
        }
        if dsk.win.mem_open {
            scene::vitals::paint(&p, &dsk.win.mem, dsk.tick.loops_per_second, dsk.tick.consumo.ultimo);
        }
    }

    // -- ** EL TESTIGO DEL BUS USB: la luz que no hay que abrir --
    //
    // Aqui, con las vitales y antes del cursor, por el mismo motivo: lo que se
    // pinta despues del cursor le come el save-under.
    //
    // ** Y SOLO EN FOTOGRAMAS QUE VAYAN A PINTAR, que es lo que lo hace seguro
    // sin apartar el puntero a mano. La duda razonable es si eso lo puede dejar
    // sin repintar justo el dia malo --si el teclado esta muerto y el raton
    // quieto, no hay entrada que dispare nada--, y no: el parpadeo del cursor
    // de escritura pone `will_paint` cada `BLINK` vueltas **sin que nadie toque
    // un aparato**. La luz llega tarde como mucho medio parpadeo.
    //
    // Cada cuarto de segundo, como las vitales: es un estado que cambia despacio
    // y mirarlo mas rapido no da mas informacion. Va en CICLOS y no en el flanco
    // `quarter` porque esta llamada solo ocurre en vueltas que pintan, y un
    // flanco que se levanta en una vuelta que no pinta no lo veria nadie.
    if dsk.tick.will_paint {
        scene::testigo::refrescar(&p, dsk.tick.quarter_cycles());
        // ** LOS INSTRUMENTOS DE DIAGNOSTICO, en CABINA (2026-09-22): el
        // reparto del pulso, el volcado y la entrada iban al lado de esta luz, en
        // la barra de arriba, y la barra se fundio en el panel. Van donde se
        // mira cuando algo va mal, y SOLO con CABINA delante: pintar encima de
        // la ventana que la tapa seria peor que no pintar. El pulso con su
        // aguja --lo que dice si el escritorio vive-- se quedo en el panel.
        if dsk.win.cabina_open && dsk.win.top_before == Ventana::Cabina && !fs {
            scene::cabina::instrumentos(&p, &dsk.win.cabina, &dsk.tick.lectura_pulso(), &p.volcado());
        }
    }

    // -- La capa del RECORTE (Ctrl+Shift+S), justo ANTES del cursor: el cursor
    // guarda lo que tapa, y lo que tapa tiene que incluir la capa. --
    if dsk.tick.will_paint {
        crate::desktop::captura::capa_poner(&p);
    }

    // -- El cursor del raton, ENCIMA de todo y lo ultimo --
    //
    // Aqui ya no queda nada por pintar en este fotograma, asi que lo que se
    // guarda debajo es lo definitivo. Ponerlo antes obligaria a que cada
    // ventana supiera esquivarlo -- que es justo lo que no se puede pedir a
    // una ventana que todavia no existe.
    if dsk.tick.ax != u32::MAX {
        // * QUE ESTA DICIENDO EL PUNTERO.
        //
        // Se decide aqui, al final del fotograma, porque es aqui donde ya
        // se sabe todo lo que paso en el: que ventana quedo arriba, donde
        // acabo el raton y si la calculadora esta abierta.
        //
        // El orden de las preguntas es el Z-order otra vez: lo que esta
        // encima manda. Un boton de la calculadora tapado por la consola
        // del kernel no puede pedir la mano -- senalaria algo que no se
        // puede pulsar, que es peor que no marcar nada.
        let shape = if dsk.calc.visible
            && dsk.win.top_before == Ventana::Run
            && dsk.calc_pad.key_at(dsk.tick.ax, dsk.tick.ay).is_some()
        {
            scene::cursor::Shape::Hand
        } else if dsk.win.visible && dsk.win.top_before == Ventana::Run && dsk.run_box.on_field(dsk.tick.ax, dsk.tick.ay) {
            scene::cursor::Shape::Beam
        } else {
            scene::cursor::Shape::Arrow
        };
        dsk.save_under.place(&p, dsk.tick.ax, dsk.tick.ay, shape);
    }

    // * Y ahora EMPUJARLO a la pantalla.
    //
    // El framebuffer esta mapeado en write-combining: el CPU acumula las
    // escrituras y las suelta cuando el bufer se llena. Sin esta linea, lo
    // pintado en este fotograma se queda esperando a que alguien escriba
    // mas -- y el sintoma es exactamente el que aparecio en el Ryzen:
    // teclear no pintaba nada hasta que se movia el raton, porque mover el
    // raton era lo que llenaba el bufer.
    //
    // Una instruccion, una vez por fotograma, al final de todo. Ver
    // `Pantalla::vaciar`.
    p.vaciar();
}

/// **Las sugerencias bajo el campo** (2026-09-24), solo si cambio lo tecleado:
/// el parpadeo del cursor repinta el campo cuatro veces por segundo y la linea
/// no tiene por que ir detras. Ver `commands::sugerencias`.
///
/// La linea de estado es de quien la escribio: una orden deja ahi su mensaje
/// y eso no se pisa hasta que se teclea otra vez. Al vaciar el campo con
/// sugerencias puestas, vuelve la pista del consejero.
fn sugerencias(dsk: &mut Desktop, p: &bmo::Pantalla) {
    use crate::commands::sugerencias as sg;
    // Copias: la linea y la base se leen mientras se apunta la firma.
    let copia = dsk.field.path;
    let linea = &copia[..dsk.field.n];
    let base_copia = dsk.field.sug_base;
    // FNV-1a; `| 1` para que una linea escrita nunca firme 0 (la vacia).
    let firma = if linea.is_empty() {
        0
    } else {
        linea.iter().fold(0xCBF2_9CE4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01B3)) | 1
    };
    if firma == dsk.field.sug_firma {
        return;
    }
    // Dando vueltas con TAB, la lista es la de lo tecleado ANTES del primer
    // TAB, y la resaltada, la que hay escrita ahora.
    let base = if dsk.field.sug_base_n > 0 { &base_copia[..dsk.field.sug_base_n] } else { linea };
    let s = sg::para(base);
    dsk.field.sug_firma = firma;
    if s.total == 0 {
        if dsk.field.sug_pintadas {
            dsk.field.sug_pintadas = false;
            // Campo vacio: vuelve la pista. Con algo escrito que no es una
            // orden de la lista (una ruta tras `ls `), la linea se calla.
            if firma == 0 {
                pista_consejero(dsk, p);
            } else {
                paint_status(p, &dsk.run_box, "", scene::INK_DIM);
            }
        }
        return;
    }
    // La elegida (la que TAB escribe, o la escrita al dar vueltas) y una
    // ventana de `MAX` que la contiene.
    let elegida = s.donde(linea).unwrap_or(0);
    let desde = elegida.saturating_sub(sg::MAX - 1).min(s.total.saturating_sub(sg::MAX));
    let n = (s.total - desde).min(sg::MAX);
    let mut lineas: [&[u8]; sg::MAX] = [b""; sg::MAX];
    for k in 0..n {
        lineas[k] = sg::linea(s.i[desde + k]);
    }
    scene::sugerir::pintar(p, &dsk.run_box, &lineas[..n], elegida - desde, s.total - n, sg::que(s.i[elegida]));
    dsk.field.sug_pintadas = true;
}

/// **La pista del consejero** en la linea de estado: lo siguiente recomendado,
/// en UNA linea. Sale al invocar la caja (Ctrl+Alt) y al vaciar el campo; el
/// consejero entero solo va a la salida al arrancar y tras `save mode`.
pub(crate) fn pista_consejero(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let mut t = [0u8; 160];
    let (etiqueta, n) = crate::commands::verificar::pista(&mut t);
    scene::sugerir::pista(p, &dsk.run_box, etiqueta, &t[..n]);
    dsk.field.sug_pintadas = false;
    dsk.field.sug_firma = 0;
}
