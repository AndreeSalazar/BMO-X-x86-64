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
use crate::scene::vitals::{filas, MandoRed, MandoSmp, Solapa, FICHAS, FILA_VACIA};
use crate::{erase_window, uncover};

/// Pintarla con lo que el escritorio ya midio.
pub(crate) fn pintar(dsk: &Desktop, p: &bmo::Pantalla) {
    scene::vitals::paint(p, &dsk.win.mem, dsk.tick.loops_per_second, dsk.tick.consumo.ultimo);
}

fn mw(dsk: &Desktop) -> Option<u64> {
    dsk.tick.consumo.ultimo.map(|m| m.mw_paquete).filter(|&v| v > 0)
}

/// **F7 (`Solapa::Cpu`), F8 (`Solapa::Memoria`) o F6 (`Solapa::Red`)**: abre en esa solapa; si
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

/// **Un mando del SMP** desde la solapa CPU (03-10): lo mismo que la orden
/// `smp`, y lo que paso, en el pie. Despertar y medir BLOQUEAN (hasta un
/// segundo): el aviso se pinta ANTES, para que se vea que esta pasando.
fn mando_smp(dsk: &mut Desktop, p: &bmo::Pantalla, m: MandoSmp) {
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
    let mut d = [0u8; 10];
    match m {
        MandoSmp::Todos => {
            dsk.win.mem.decir(b"despertando nucleos (esto tarda)...");
            pintar(dsk, p);
            p.volcar();
            let (vivos, esperados, parados) = bmo::smp_censo(u32::MAX);
            poner(b"en pie: ", &mut k);
            let n = crate::text::decimal(vivos as u64 + 1, &mut d);
            poner(&d[..n], &mut k);
            poner(b" de ", &mut k);
            let n = crate::text::decimal(esperados as u64 + 1, &mut d);
            poner(&d[..n], &mut k);
            if parados {
                poner(b"  [!] pero PARADOS: en pie no es trabajando", &mut k);
            }
        }
        MandoSmp::Parar => {
            bmo::smp_parar();
            poner(b"obreros parados: siguen en pie (encendidos), no trabajan. A los despierta", &mut k);
        }
        MandoSmp::Medir => {
            dsk.win.mem.decir(b"midiendo el reparto (esto tarda)...");
            pintar(dsk, p);
            p.volcar();
            match bmo::smp_prueba_juzgada() {
                Some(x100) if x100 > 0 => {
                    poner(b"aceleracion: ", &mut k);
                    let n = crate::text::decimal(x100 / 100, &mut d);
                    poner(&d[..n], &mut k);
                    poner(if x100 % 100 < 10 { b".0" } else { b"." }, &mut k);
                    let n = crate::text::decimal(x100 % 100, &mut d);
                    poner(&d[..n], &mut k);
                    poner(b"x  (el techo: una cuenta pura, sin memoria compartida)", &mut k);
                }
                Some(_) => poner(b"0 = falto una parte: el numero no vale", &mut k),
                None => poner(b"la prueba NO se pudo juzgar: el barrido no completo", &mut k),
            }
        }
    }
    dsk.win.mem.decir(&t[..k]);
}

/// **Un mando de la RED** desde su solapa (03-10, F6): `red rx` y `red
/// velocidad`, con lo que paso en el pie.
fn mando_red(dsk: &mut Desktop, m: MandoRed) {
    match m {
        MandoRed::Armar => {
            let antes = bmo::info(bmo::INFO_NET_RX_TRAMAS);
            let frase: &[u8] = match bmo::red::armar() {
                bmo::red::Armado::Ok => {
                    bmo::red::sondear();
                    if bmo::info(bmo::INFO_NET_RX_TRAMAS) > antes {
                        b"receptor ARMADO, y ya llegan tramas"
                    } else {
                        b"receptor ARMADO: ninguna todavia (es lo normal al armar)"
                    }
                }
                bmo::red::Armado::SinEnlace => b"el enlace esta ABAJO: enchufa el cable antes de armar",
                bmo::red::Armado::NoArma => b"el receptor no se pudo armar: F11 dice por que",
                bmo::red::Armado::SinTarjeta => b"no hay tarjeta que este kernel sepa leer",
                bmo::red::Armado::Raro(_) => b"el kernel contesto algo que no conozco",
            };
            dsk.win.mem.decir(frase);
        }
        MandoRed::Velocidad => {
            dsk.win.mem.decir(match bmo::red::renegociar() {
                Some(_) => b"renegociando 10/100/1000: el enlace se cae unos segundos y vuelve",
                None => b"no se pudo: sin tarjeta, el PHY no contesta, o falta la autoridad RED",
            });
        }
    }
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
        b'4' => cambiar(dsk, p, Solapa::Red),
        b'a' | b'A' if s == Solapa::Red => {
            mando_red(dsk, MandoRed::Armar);
            pintar(dsk, p);
        }
        b'v' | b'V' if s == Solapa::Red => {
            mando_red(dsk, MandoRed::Velocidad);
            pintar(dsk, p);
        }
        b'\t' | 0x83 => cambiar(dsk, p, s.siguiente()),
        0x82 => cambiar(dsk, p, s.siguiente().siguiente().siguiente()),
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
        // ** EL SMP vive en la CPU (03-10).
        b'a' | b'A' if s == Solapa::Cpu => {
            mando_smp(dsk, p, MandoSmp::Todos);
            pintar(dsk, p);
        }
        b's' | b'S' if s == Solapa::Cpu => {
            mando_smp(dsk, p, MandoSmp::Parar);
            pintar(dsk, p);
        }
        b'm' | b'M' if s == Solapa::Cpu => {
            mando_smp(dsk, p, MandoSmp::Medir);
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
    if let Some(m) = dsk.win.mem.mando_red_en(x, y) {
        mando_red(dsk, m);
        pintar(dsk, p);
        return true;
    }
    if let Some(m) = dsk.win.mem.mando_smp_en(x, y) {
        mando_smp(dsk, p, m);
        pintar(dsk, p);
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
