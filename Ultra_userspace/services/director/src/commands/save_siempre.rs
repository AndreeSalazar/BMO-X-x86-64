//! **`save siempre`: CABINA guarda sola mientras se testea** (03-10).
//!
//! Eddi: *"automatizar la escritura en save mode"*. Hasta hoy un `save` se
//! tecleaba, y en el metal eso falla justo cuando importa: Cyberpunk se quedo
//! girando sin ventana, y si la maquina se congela entera no hay `save` que
//! teclear despues.
//!
//! ```text
//!    armarlo       `save siempre` (`save siempre off` lo desarma). Queda en
//!                  datos/siempre.txt y sigue ARMADO tras reiniciar, como
//!                  `save mode`
//!    al arrancar   el `save` entero; y si datos/vivo.txt no acaba en
//!                  "# ACABO", la vez anterior la maquina se quedo colgada
//!                  con un programa corriendo: se copia a datos/colgado.txt
//!                  y se dice cual era
//!    corriendo     al lanzar y cada 30 s, datos/vivo.txt: lo que escribio el
//!                  programa (cada linea una vez, con su xN) y las filas de
//!                  CABINA desde que se lanzo. Directo al fichero, sin
//!                  pintarlo (ver `bmo_registro::vivo`)
//!    al acabar     (termino, murio o ^C) el ultimo vivo con "# ACABO" y el
//!                  `save` entero, con su copia con fecha
//! ```
//!
//! [!] Un programa que se lleva la pantalla (`presta`, DOOM) para el bucle
//! del escritorio mientras corre: con el, solo el save del final.
//!
//! El DIARIO de PROTON-X (`personal diario`) no es esto: lo escribe el
//! propio PROTON-X, funcion a funcion, en informe/diario.txt. Los dos se
//! leen juntos: el diario dice que llamo el juego; el vivo, que dijo y que
//! se tecleo.
//!
//! [consumo] NADA      desarmado; armado, un fichero cada 30 s solo mientras
//!                     corre un programa

use bmo_userland as bmo;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::Relaxed};

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM, INK_OK};
use crate::DEFAULT_DUMP;
use bmo_registro::vivo;

/// Donde queda armado.
const ARMADO_EN: &[u8] = b"datos/siempre.txt";
/// El save vivo.
const VIVO: &[u8] = b"datos/vivo.txt";
/// El vivo de una vez que se quedo colgada, rescatado al arrancar.
const COLGADO: &[u8] = b"datos/colgado.txt";
/// Cada cuanto, mientras corre un programa.
const CADA_MS: u64 = 30_000;

static ARMADO: AtomicBool = AtomicBool::new(false);
/// Si la vuelta anterior habia un programa corriendo.
static CORRIA: AtomicBool = AtomicBool::new(false);
static EMPEZO_MS: AtomicU64 = AtomicU64::new(0);
static ULTIMO_MS: AtomicU64 = AtomicU64::new(0);
static VUELTA: AtomicU32 = AtomicU32::new(0);
/// La marca de CABINA al lanzar: la sesion del vivo empieza ahi.
static MARCA: AtomicUsize = AtomicUsize::new(0);

fn ahora_ms() -> u64 {
    super::red_nodo::ahora_ms()
}

/// Si esta armado.
pub(crate) fn armado() -> bool {
    ARMADO.load(Relaxed)
}

/// **`save siempre` / `save siempre off`**. `None` si `arg` no es suyo.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> Option<After> {
    let armar = match arg {
        b"siempre" => true,
        b"siempre off" | b"siempre no" => false,
        _ => return None,
    };
    let escrito = bmo::Archivo::create(ARMADO_EN).is_ok_and(|a| {
        a.write(if armar { b"save siempre\n" as &[u8] } else { b"save siempre off\n" });
        a.close()
    });
    ARMADO.store(armar, Relaxed);
    let g = &mut dsk.out.grid;
    g.with_ink(if armar { INK_GOOD } else { INK_ECHO });
    g.text(if armar {
        b"  save SIEMPRE: armado. Guardo solo al arrancar, al lanzar y cada 30 s mientras corre un programa (datos/vivo.txt), y al acabar (el save entero)\n" as &[u8]
    } else {
        b"  save siempre: desarmado. Vuelve a guardar solo cuando lo teclees\n"
    });
    if !escrito {
        g.with_ink(INK_ERR);
        g.text(b"  (no pude escribir datos/siempre.txt: vale hasta que apagues, no tras reiniciar)\n");
    }
    g.with_ink(INK_PLAIN);
    paint_status(p, &dsk.run_box, if armar { "save siempre" } else { "save manual" }, INK_DIM);
    // Armado con un programa ya en marcha: empieza a contar desde ahora.
    if armar && dsk.out.run.is_some() && !CORRIA.load(Relaxed) {
        empezar(dsk);
    }
    dsk.field.n = 0;
    Some(After::Settle)
}

/// **Al arrancar**, una vez: leer si esta armado, rescatar el vivo de una
/// vez colgada, y el save del arranque.
pub(crate) fn al_arrancar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let mut b = [0u8; 32];
    let n = bmo::Archivo::leer_de(ARMADO_EN).map_or(0, |a| {
        let n = a.read(&mut b);
        a.close();
        n
    });
    ARMADO.store(b[..n].starts_with(b"save siempre") && !b[..n].starts_with(b"save siempre off"), Relaxed);
    rescatar_colgado(dsk);
    if armado() {
        save_entero(dsk, p, b"el del arranque");
    }
}

/// Si el vivo que quedo no acabo, a `COLGADO`, y se dice.
fn rescatar_colgado(dsk: &mut Desktop) {
    let Ok(a) = bmo::Archivo::leer_de(VIVO) else { return };
    // La cabecera (para el nombre) y la cola (para la marca), sin traerlo
    // entero: el DIRECTOR no tiene monton.
    let mut cabeza = [0u8; 256];
    let mut k_cabeza = 0usize;
    let mut cola = [0u8; 64];
    let mut k_cola = 0usize;
    let mut b = [0u8; 2048];
    loop {
        let n = a.read(&mut b);
        if n == 0 {
            break;
        }
        let c = (cabeza.len() - k_cabeza).min(n);
        cabeza[k_cabeza..k_cabeza + c].copy_from_slice(&b[..c]);
        k_cabeza += c;
        for &x in &b[..n] {
            if k_cola == cola.len() {
                cola.copy_within(1.., 0);
                k_cola -= 1;
            }
            cola[k_cola] = x;
            k_cola += 1;
        }
    }
    a.close();
    if k_cabeza == 0 || vivo::acabo(&cola[..k_cola]) {
        return;
    }
    let copiado = match (bmo::Archivo::leer_de(VIVO), bmo::Archivo::create(COLGADO)) {
        (Ok(o), Ok(c)) => {
            loop {
                let n = o.read(&mut b);
                if n == 0 {
                    break;
                }
                c.write(&b[..n]);
            }
            o.close();
            c.close()
        }
        _ => false,
    };
    // Rescatado: el vivo se cierra, para no avisar otra vez al siguiente.
    if copiado {
        if let Ok(v) = bmo::Archivo::create(VIVO) {
            v.write(b"# save vivo rescatado a datos/colgado.txt al arrancar\r\n");
            v.write(vivo::MARCA_FIN);
            v.write(b"\r\n");
            v.close();
        }
    }
    let g = &mut dsk.out.grid;
    g.with_ink(INK_ERR);
    g.text(b"  save siempre: la vez anterior la maquina se QUEDO COLGADA corriendo ");
    g.text(vivo::programa(&cabeza[..k_cabeza]).unwrap_or(b"un programa"));
    g.byte(b'\n');
    g.text(if copiado {
        b"  su ultimo save vivo esta en datos/colgado.txt (lo que escribio y lo que se tecleo)\n" as &[u8]
    } else {
        b"  su ultimo save vivo sigue en datos/vivo.txt (no pude copiarlo a colgado.txt): copialo antes de lanzar otro\n"
    });
    g.with_ink(INK_PLAIN);
}

/// **Cada vuelta del escritorio**, despues del vigilante: `acabo` si el
/// programa lanzado termino en esta vuelta.
pub(crate) fn vuelta(dsk: &mut Desktop, p: &bmo::Pantalla, acabo: bool) {
    if !armado() {
        // Desarmado con un programa en marcha: su vivo se cierra, o el
        // siguiente arranque lo tomaria por una maquina colgada.
        if CORRIA.swap(false, Relaxed) {
            escribir_vivo(dsk, true);
        }
        return;
    }
    if acabo {
        if CORRIA.swap(false, Relaxed) {
            escribir_vivo(dsk, true);
        }
        save_entero(dsk, p, b"el programa acabo");
        return;
    }
    if dsk.out.run.is_none() {
        CORRIA.store(false, Relaxed);
        return;
    }
    if !CORRIA.load(Relaxed) {
        empezar(dsk);
        return;
    }
    if ahora_ms().saturating_sub(ULTIMO_MS.load(Relaxed)) >= CADA_MS {
        escribir_vivo(dsk, false);
    }
}

/// Un programa nuevo: el reloj, la marca y el primer vivo (si la maquina
/// se congela en los primeros 30 s, ya hay uno).
fn empezar(dsk: &mut Desktop) {
    CORRIA.store(true, Relaxed);
    EMPEZO_MS.store(ahora_ms(), Relaxed);
    VUELTA.store(0, Relaxed);
    MARCA.store(dsk.out.run.as_ref().map_or(dsk.out.grid.mark(), |r| r.mark), Relaxed);
    escribir_vivo(dsk, false);
}

/// **El vivo**, a `VIVO`, sin pintar nada (una linea en la barra).
fn escribir_vivo(dsk: &mut Desktop, fin: bool) {
    let ahora = ahora_ms();
    ULTIMO_MS.store(ahora, Relaxed);
    let vuelta = VUELTA.fetch_add(1, Relaxed) + 1;
    let mut fecha = [0u8; 24];
    let k = super::save_maestro::fecha_en(&mut fecha);
    let c = vivo::Cabecera {
        fecha: &fecha[..k],
        programa: crate::registro::nombre(),
        segundos: ahora.saturating_sub(EMPEZO_MS.load(Relaxed)) / 1000,
        vuelta,
    };
    let Ok(a) = bmo::Archivo::create(VIVO) else { return };
    let g = &dsk.out.grid;
    let (desde, hasta) = g.rows_since(MARCA.load(Relaxed));
    let mut filas = (desde..=hasta).map(|f| g.line(f));
    vivo::componer(&c, crate::registro::registro(), &mut filas, fin, &mut |b| {
        a.write(b);
    });
    a.close();
}

/// **El `save` entero**, como tecleado, pero sin vaciar lo que se este
/// escribiendo en Ejecutar.
fn save_entero(dsk: &mut Desktop, p: &bmo::Pantalla, por_que: &[u8]) {
    let g = &mut dsk.out.grid;
    g.with_ink(INK_ECHO);
    g.text(b"  save siempre: ");
    g.text(por_que);
    g.text(b"\n");
    g.with_ink(INK_PLAIN);
    let r = super::save_maestro::maestro(dsk, DEFAULT_DUMP, p.rayo());
    let g = &mut dsk.out.grid;
    match r {
        Ok(_) => {
            g.with_ink(INK_GOOD);
            g.text(b"  save siempre: guardado en ");
            g.text(DEFAULT_DUMP);
            g.text(b" (y su copia con fecha, y las hojas de informe/)\n");
            paint_status(p, &dsk.run_box, "save siempre: guardado", INK_OK);
        }
        Err(_) => {
            g.with_ink(INK_ERR);
            g.text(b"  save siempre: NO se pudo guardar (el motivo, en F11)\n");
        }
    }
    dsk.out.grid.with_ink(INK_PLAIN);
}
