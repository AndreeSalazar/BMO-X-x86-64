//! **LA BIENVENIDA: "El emisor salta" al llegar** (S4i de
//! `PLAN_EL_SONIDO.md`, 2026-10-03).
//!
//! [consumo] LATE      ~8 s a ~30 fotogramas por segundo, UNA vez por
//!                     arranque; despues se queda la musica (la de
//!                     `desktop::musica`, con su pastilla) y esto no pide
//!                     nada. Con `bienvenida = no` en `sys/director.cfg`,
//!                     NADA: ni panel, ni musica, ni tubo armado (L6h)
//!
//! El propietario: *"'el emisor salta' esa musica ME ENCANTA y sirve como
//! fondo en pantalla cuando lleguen, pero con animacion, como que encanta al
//! usuario, y ya para que se minimice"*.
//!
//! ```text
//!    llega     al acabar el arranque (`desktop::boot`) suena la pieza de
//!              fondo y el panel de 8 bits CRECE en el centro, con rebote
//!    encanta   estrellas que pasan, BMO-X en pixeles que salta letra a
//!              letra, el gato en bloques que bota a 128 pulsos, y las
//!              barras del ecualizador, que siguen al medidor del MAESTRO
//!              (lo que de verdad sale por el cable, no un dibujo)
//!    se va     a los ENCANTA_MS, o con una tecla o un clic: el panel se
//!              encoge hacia arriba, hasta la pastilla, y la pastilla asoma
//!              diciendo lo que suena. La musica sigue
//! ```
//!
//! [!] Es la UNICA vez que el escritorio arma el tubo sin que se lo pidan
//! (la regla de `audio::armar_silencio` es "solo a proposito"): por eso es
//! una clave del fichero, y `bienvenida = no` llega en silencio, como antes.
//!
//! La cara la pinta `scene::bienvenida`; esto lleva el reloj y decide.

use bmo_fondo::PIEZAS;
use bmo_userland as bmo;

use crate::desktop::musica;
use crate::desktop::Desktop;
use crate::scene::bienvenida::{self as sb, Cara};

/// Lo que crece al llegar, lo que encanta y lo que tarda en irse.
const CRECE_MS: u64 = 450;
const ENCANTA_MS: u64 = 8_000;
const SE_VA_MS: u64 = 650;
const FOTOGRAMA_MS: u64 = 33;

/// La pieza de la bienvenida, por su nombre: si un dia cambia de sitio en
/// `PIEZAS`, se sigue encontrando.
const PIEZA: &[u8] = b"el emisor salta";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fase {
    Nada,
    Encanta,
    SeVa,
}

struct Estado {
    fase: Fase,
    por_ms: u64,
    /// Cuando empezo (el compas 0 de la pieza) y cuando empezo a irse.
    desde: u64,
    se_va_desde: u64,
    pintado: u64,
    /// La caja de lo que se pinto, para saber si un clic cae dentro.
    caja: (u32, u32, u32, u32),
    /// Habia algo puesto: un fotograma mas para quitarlo al acabar.
    estaba: bool,
    /// La pieza suena (si no, el panel lo dice: sin tubo, solo se ve).
    suena: bool,
    pieza: usize,
}

static mut ESTADO: Estado =
    Estado { fase: Fase::Nada, por_ms: 0, desde: 0, se_va_desde: 0, pintado: 0, caja: (0, 0, 0, 0), estaba: false, suena: false, pieza: 0 };

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

fn ms(e: &Estado, desde: u64) -> u64 {
    bmo::ciclos().wrapping_sub(desde) / e.por_ms.max(1)
}

/// **Empieza la bienvenida**, si el fichero no dice `bienvenida = no`. Lo
/// llama `desktop::boot` al final, con el escritorio ya pintado.
pub(crate) fn empezar() {
    if !crate::scene::estilo::estilo().bienvenida {
        return;
    }
    let e = estado();
    e.por_ms = (bmo::info(bmo::INFO_TSC_HZ) / 1000).max(1);
    e.pieza = bmo_fondo::buscar(PIEZA).unwrap_or(0);
    // Se compone AQUI (una vuelta entera, de una vez); el reloj del panel
    // arranca DESPUES, que es cuando suena el primer pulso.
    e.suena = musica::tocar(e.pieza).is_ok();
    if !e.suena {
        bmo::consola("bienvenida: sin tubo de audio, solo se ve\n");
    }
    e.desde = bmo::ciclos();
    e.fase = Fase::Encanta;
}

/// **Esta en pantalla?** La pastilla espera a que acabe: es a ella a donde
/// se minimiza, y asoma justo despues.
pub(crate) fn activa() -> bool {
    estado().fase != Fase::Nada
}

/// Que se vaya ya (una tecla, un clic, o el tiempo).
fn irse(e: &mut Estado) {
    if e.fase == Fase::Encanta {
        e.fase = Fase::SeVa;
        e.se_va_desde = bmo::ciclos();
    }
}

/// Lee el medidor del maestro: pico y fuerza de los dos lados, en 1/256 dB.
fn medidor() -> ([i32; 2], [i32; 2]) {
    let m = bmo::info(bmo::INFO_AUDIO_MEDIDOR);
    let d = |s: u32| ((m >> s) & 0xFFFF) as u16 as i16 as i32;
    ([d(0), d(16)], [d(32), d(48)])
}

/// **La bienvenida de este fotograma.** Al FINAL del fotograma, debajo de
/// la pastilla y del globo. `tapado`: una ventana a pantalla completa (un
/// juego que arranco solo): ahi no se encanta a nadie, se va sin mas.
pub(crate) fn poner(_dsk: &Desktop, p: &bmo::Pantalla, tapado: bool) {
    let e = estado();
    if e.fase == Fase::Nada {
        e.estaba = false;
        return;
    }
    if tapado {
        e.fase = Fase::Nada;
        return;
    }
    let t = ms(e, e.desde);
    if e.fase == Fase::Encanta && t >= ENCANTA_MS {
        irse(e);
    }
    let se_va = if e.fase == Fase::SeVa { (ms(e, e.se_va_desde) * 1000 / SE_VA_MS).min(1000) } else { 0 };
    if e.fase == Fase::SeVa && se_va >= 1000 {
        // Ya es la pastilla: ella asoma con lo que suena.
        e.fase = Fase::Nada;
        return;
    }
    let (pico, rms) = medidor();
    let pieza = &PIEZAS[e.pieza % PIEZAS.len()];
    let cara = Cara {
        ms: t,
        crece: (t * 1000 / CRECE_MS).min(1000),
        se_va,
        queda: 1000 - (t.min(ENCANTA_MS) * 1000 / ENCANTA_MS),
        pulsos: pieza.bpm,
        titulo: pieza.nombre.as_bytes(),
        suena: e.suena,
        pico,
        rms,
    };
    e.caja = sb::poner(p, &cara);
    e.pintado = bmo::ciclos();
    e.estaba = true;
}

/// **Pide fotograma** mientras esta, y uno mas al acabar para quitarse.
pub(crate) fn anima() -> bool {
    let e = estado();
    if e.fase == Fase::Nada {
        return core::mem::replace(&mut e.estaba, false);
    }
    bmo::ciclos().wrapping_sub(e.pintado) >= FOTOGRAMA_MS * e.por_ms
}

/// **Una tecla**: se minimiza. Devuelve si la tecla se queda aqui: solo ESC
/// (las demas siguen su camino, que quien llega tecleando no pierda la
/// primera letra).
pub(crate) fn tecla(c: u8) -> bool {
    let e = estado();
    if e.fase != Fase::Encanta {
        return false;
    }
    irse(e);
    c == 0x1B
}

/// **Un clic** (el flanco de pulsar): se minimiza. Se queda el clic si cae
/// dentro del panel; fuera, sigue como si no estuviera.
pub(crate) fn raton(x: u32, y: u32, button: bool, antes: bool) -> bool {
    let e = estado();
    if e.fase != Fase::Encanta || !(button && !antes) {
        return false;
    }
    irse(e);
    let (cx, cy, w, h) = e.caja;
    x >= cx && y >= cy && x < cx + w && y < cy + h
}
