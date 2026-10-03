//! **LA PASTILLA: cuando se esconde, cuando asoma, y que hace un clic.**
//!
//! [consumo] LATE      con el fondo sonando pide fotograma (`anima`, que solo
//!                     mira el reloj): ~10 por segundo escondida, ~30
//!                     asomada o abierta. Sin fondo, nada (L6h)
//!
//! La cara la pinta `scene::pastilla`; esto la mueve. Reglas:
//!
//! ```text
//!    escondida   siempre que suene el fondo: una rayita arriba que late
//!    asoma       el raton cerca de ella, o algo cambio (otra pieza, el
//!                volumen, la pausa): la dice durante ASOMA_MS y se va
//!    abierta     un clic en ella; se cierra con su x o con un clic fuera
//! ```
//!
//! *** Nada de esto quita el foco ni para lo que se esta haciendo: la
//! pastilla no es una ventana. Un clic que la toca se queda en ella; uno que
//! cae fuera sigue su camino como si no estuviera.
//!
//! [!] Con un juego a pantalla completa el escritorio no pinta, y la pastilla
//! tampoco: la musica sigue (es del kernel) y se agacha sola.

use bmo_fondo::PIEZAS;
use bmo_userland as bmo;

use crate::desktop::musica;
use crate::desktop::Desktop;
use crate::scene::pastilla::{self, en, Cara, Modo, Sitios};

/// Lo que se queda asomada tras un cambio o tras irse el raton.
const ASOMA_MS: u64 = 2_500;
/// Cada cuanto pinta escondida (solo late) y asomada o abierta.
const LENTO_MS: u64 = 100;
const VIVO_MS: u64 = 33;
/// Lo cerca de arriba que tiene que estar el raton para que asome.
const CERCA: u32 = 44;

struct Estado {
    modo: Modo,
    /// Cuando entro en el modo, y hasta cuando se queda asomada.
    desde: u64,
    hasta: u64,
    /// El ultimo `cambios` de la musica que se vio.
    visto: u32,
    pintado: u64,
    por_ms: u64,
    t0: u64,
    sitios: Sitios,
    /// Habia algo que quitar en el fotograma anterior: para pedir uno mas al
    /// apagar el fondo, y que la rayita no se quede pintada.
    estaba: bool,
}

static mut ESTADO: Estado = Estado {
    modo: Modo::Escondida,
    desde: 0,
    hasta: 0,
    visto: 0,
    pintado: 0,
    por_ms: 0,
    t0: 0,
    sitios: Sitios { todo: (0, 0, 0, 0), pausa: (0, 0, 0, 0), siguiente: (0, 0, 0, 0), menos: (0, 0, 0, 0), mas: (0, 0, 0, 0), recomendada: (0, 0, 0, 0), cerrar: (0, 0, 0, 0) },
    estaba: false,
};

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

fn ms(e: &Estado, desde: u64) -> u64 {
    bmo::ciclos().wrapping_sub(desde) / e.por_ms.max(1)
}

fn cambiar(e: &mut Estado, m: Modo) {
    if e.modo != m {
        e.modo = m;
        e.desde = bmo::ciclos();
    }
}

/// Lee el medidor del maestro: pico y fuerza de los dos lados, en 1/256 dB.
fn medidor() -> ([i32; 2], [i32; 2]) {
    let m = bmo::info(bmo::INFO_AUDIO_MEDIDOR);
    let d = |s: u32| ((m >> s) & 0xFFFF) as u16 as i16 as i32;
    ([d(0), d(16)], [d(32), d(48)])
}

/// **La pastilla de este fotograma.** Al FINAL del fotograma, debajo del
/// globo. `tapado`: una ventana a pantalla completa.
pub(crate) fn poner(dsk: &Desktop, p: &bmo::Pantalla, tapado: bool) {
    let e = estado();
    if e.por_ms == 0 {
        e.por_ms = (bmo::info(bmo::INFO_TSC_HZ) / 1000).max(1);
        e.t0 = bmo::ciclos();
    }
    let Some(v) = musica::vista() else {
        e.estaba = false;
        e.modo = Modo::Escondida;
        return;
    };
    if tapado {
        return;
    }
    let ahora = bmo::ciclos();
    // Algo cambio: asoma y lo dice.
    if v.cambios != e.visto {
        e.visto = v.cambios;
        if e.modo == Modo::Escondida {
            cambiar(e, Modo::Asoma);
        }
        e.hasta = ahora + ASOMA_MS * e.por_ms;
    }
    // El raton cerca: asoma mientras este, y un rato despues.
    let (ax, ay) = (dsk.tick.ax, dsk.tick.ay);
    let centro = p.ancho / 2;
    let cerca = ax != u32::MAX && ay < CERCA && ax.abs_diff(centro) < 200;
    if cerca {
        if e.modo == Modo::Escondida {
            cambiar(e, Modo::Asoma);
        }
        e.hasta = ahora + ASOMA_MS * e.por_ms / 2;
    }
    if e.modo == Modo::Asoma && ahora > e.hasta {
        cambiar(e, Modo::Escondida);
    }
    let (pico, rms) = medidor();
    let pieza = &PIEZAS[v.pieza % PIEZAS.len()];
    let reco = &PIEZAS[musica::recomendada() % PIEZAS.len()];
    let cara = Cara {
        modo: e.modo,
        en_modo_ms: ms(e, e.desde),
        ms: ms(e, e.t0),
        titulo: pieza.nombre.as_bytes(),
        pulsos: pieza.bpm,
        volumen: v.volumen,
        pausada: v.pausada,
        recomendada: reco.nombre.as_bytes(),
        pico,
        rms,
    };
    e.sitios = pastilla::poner(p, &cara);
    e.pintado = ahora;
    e.estaba = true;
}

/// **Pide fotograma** mientras suena el fondo, al ritmo de su modo. Lo
/// pregunta el bucle en cada vuelta: solo lee el reloj (y una variable).
pub(crate) fn anima() -> bool {
    let e = estado();
    if e.por_ms == 0 {
        return false;
    }
    let Some(v) = musica::vista() else {
        // Un fotograma mas para quitar la rayita, y ninguno despues.
        return core::mem::replace(&mut e.estaba, false);
    };
    let cada = if e.modo == Modo::Escondida && v.cambios == e.visto { LENTO_MS } else { VIVO_MS };
    bmo::ciclos().wrapping_sub(e.pintado) >= cada * e.por_ms
}

/// **El raton sobre la pastilla.** Devuelve si se quedo el clic. Solo mira
/// FLANCOS (pulsar), como los botones del panel del sonido.
pub(crate) fn raton(x: u32, y: u32, button: bool, antes: bool) -> bool {
    let e = estado();
    if !(button && !antes) || musica::vista().is_none() {
        return false;
    }
    let s = e.sitios;
    if !en(s.todo, x, y) {
        // Fuera: si estaba abierta se cierra, y el clic sigue su camino.
        if e.modo == Modo::Abierta {
            cambiar(e, Modo::Escondida);
        }
        return false;
    }
    if en(s.pausa, x, y) {
        musica::pausa();
    } else if en(s.siguiente, x, y) {
        let _ = musica::siguiente();
    } else if en(s.menos, x, y) {
        if let Some(v) = musica::vista() {
            musica::volumen(v.volumen.saturating_sub(10));
        }
    } else if en(s.mas, x, y) {
        if let Some(v) = musica::vista() {
            musica::volumen((v.volumen + 10).min(100));
        }
    } else if en(s.recomendada, x, y) {
        let _ = musica::tocar(musica::recomendada());
    } else if en(s.cerrar, x, y) {
        cambiar(e, Modo::Escondida);
    } else if e.modo != Modo::Abierta {
        cambiar(e, Modo::Abierta);
    }
    // Lo que cambio la musica no tiene que volver a "asomar": ya esta abierta.
    if e.modo == Modo::Abierta {
        if let Some(v) = musica::vista() {
            e.visto = v.cambios;
        }
    }
    true
}
