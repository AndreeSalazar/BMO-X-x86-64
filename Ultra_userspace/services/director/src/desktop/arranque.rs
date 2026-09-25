//! **EL ARRANQUE ORQUESTADO: cuando y que** (2026-09-25). Pedido: *"el splash
//! eso es muy especial porque ahi es mi GPU TIENE QUE TOMAR TODO el control
//! ... mientras la CPU esta preparando"*.
//!
//! [consumo] NADA      solo en el arranque con `save mode` ARMADO: un panel
//!                     por paso y, al final, ~1,4 s de la 3060; despues, nada
//!
//! # Lo que se puede y lo que no, dicho claro
//!
//! La 3060 no puede pintar NADA hasta que su GSP arranca, y el GSP lo
//! arrancan justo estos pasos: la primera mitad del arranque la lleva la CPU
//! por fuerza. Y un paso es una orden que BLOQUEA (el director es un solo
//! hilo), asi que el panel se repinta ENTRE pasos, no durante.
//!
//! ```text
//!    CPU   el fondo y el panel, antes de cada paso: por donde va, que hace,
//!          la bitacora y el reloj de la CPU al acabar cada uno
//!    3060  en cuanto `pantalla` esta probado, TOMA EL CONTROL: ~1,4 s de
//!          fotogramas a pantalla completa, cada pixel suyo; la CPU solo pone
//!          el panel del resultado encima, y devuelve el escritorio
//! ```
//!
//! La cara la pinta `scene::arranque`; esto lleva el estado. Lo llama
//! `commands::verificar` desde `al_arrancar` (y SOLO desde ahi: un `save mode`
//! tecleado sigue en la caja de salida).

use bmo_userland as bmo;

use crate::desktop::Desktop;
use crate::scene::arranque::{self as sa, Hecho, Tira, Vista, AHORA, CIAN, MAGENTA, PENDIENTE, VERDE};

const MAX: usize = 64;
const BITACORA: usize = 16;
/// Lo que la 3060 lleva la pantalla ella sola, al final.
const TOMA_MS: u64 = 1400;
/// Tope de fotogramas, por si alguno sale mucho mas rapido de lo que se mide.
const TOMA_MAX: u32 = 1200;
/// Lo que se sostiene el aviso de antes y el resultado de despues.
const ANTES_MS: u64 = 450;
const DESPUES_MS: u64 = 1100;

const VACIO: Hecho = Hecho { nombre: b"", estado: PENDIENTE, us: 0, tsc: 0 };

struct Estado {
    activo: bool,
    por_ms: u64,
    desde: u64,
    estados: [u8; MAX],
    total: usize,
    bitacora: [Hecho; BITACORA],
    n: usize,
    /// Pasos que corrieron de verdad (no quitados ni ya hechos).
    corridos: u32,
    /// Un paso cambio lo que lee el monitor: el fondo entero otra vez.
    fondo_sucio: bool,
    /// Lo que brilla el fondo mientras la 3060 duerme: baja en cada pintada.
    luz: u32,
    /// La 3060 ya desperto y ya se vio (la animacion va una vez).
    despierta: bool,
    /// El gato del final, a todo brillo.
    final_: bool,
}

static mut ESTADO: Estado = Estado {
    activo: false,
    por_ms: 1,
    desde: 0,
    estados: [PENDIENTE; MAX],
    total: 0,
    bitacora: [VACIO; BITACORA],
    n: 0,
    corridos: 0,
    fondo_sucio: false,
    luz: LUZ,
    despierta: false,
    final_: false,
};

/// La luz del fondo al empezar, y lo que baja en cada pintada.
const LUZ: u32 = 110;
const BAJA: u32 = 6;
/// La animacion de LA 3060 DESPERTO.
const DESPIERTA_MS: u64 = 1100;

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el arranque.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

fn ms(e: &Estado) -> u64 {
    bmo::ciclos().wrapping_sub(e.desde) / e.por_ms
}

/// Espera durmiendo (no dando vueltas), como la intro.
fn esperar_ms(ms: u64) {
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    if hz == 0 {
        return;
    }
    let hasta = bmo::ciclos() + hz / 1000 * ms;
    while bmo::ciclos() < hasta {
        bmo::wait(0, 0, 4_000_000);
    }
}

/// **Empieza**: el fondo a pantalla completa y el primer panel. `total`: los
/// pasos de `save mode`.
pub(crate) fn empezar(p: &bmo::Pantalla, total: usize) {
    let e = estado();
    e.activo = true;
    e.por_ms = (bmo::info(bmo::INFO_TSC_HZ) / 1000).max(1);
    e.desde = bmo::ciclos();
    e.total = total.min(MAX);
    e.estados = [PENDIENTE; MAX];
    e.n = 0;
    e.corridos = 0;
    e.fondo_sucio = true;
    e.luz = LUZ;
    e.despierta = false;
    e.final_ = false;
    pintar(p, b"save mode", b"el modo esta ARMADO: se repite cada paso de la 3060, con un save antes de cada uno", b"CPU: leyendo datos/modo.txt", CIAN);
}

/// **Una etapa del escritorio**, que se prepara DETRAS del panel: la marca
/// del arranque (`boot::marca`) la dice aqui en vez de abajo a la izquierda,
/// y el fondo entero se repinta encima de lo que el escritorio dejo en el
/// lienzo (el escritorio sale entero al final, `repintar_escritorio`).
pub(crate) fn etapa(p: &bmo::Pantalla, que: &str) {
    if !estado().activo {
        return;
    }
    estado().fondo_sucio = true;
    pintar(p, b"escritorio", que.as_bytes(), b"CPU: el escritorio se prepara detras de este panel", CIAN);
}

/// **Sigue** (lo llama `al_arrancar`): si ya empezo tras el gato, no se
/// empieza otra vez; si no, ahora.
pub(crate) fn seguir(p: &bmo::Pantalla, total: usize) {
    if !estado().activo {
        empezar(p, total);
        return;
    }
    estado().fondo_sucio = true;
}

/// Si el arranque orquestado lleva la pantalla (entonces nadie mas pinta).
pub(crate) fn activo() -> bool {
    estado().activo
}

/// **Va a correr el paso `i`**: se dice cual y que hace.
pub(crate) fn paso(p: &bmo::Pantalla, i: usize, nombre: &[u8], que: &[u8]) {
    let e = estado();
    if !e.activo {
        return;
    }
    if i < e.total {
        e.estados[i] = AHORA;
    }
    e.corridos += 1;
    pintar(p, nombre, que, b"CPU: preparando la RAM y la 3060 -- la 3060 toma el control en cuanto pueda", CIAN);
}

/// **El paso `i` acabo** (o se salto): a la barra y a la bitacora.
pub(crate) fn salio(i: usize, nombre: &'static [u8], como: u8, us: u64) {
    let e = estado();
    if !e.activo {
        return;
    }
    if i < e.total {
        e.estados[i] = como;
    }
    // Lo saltado no llena la bitacora: esta para lo que paso.
    if como == sa::SALTO || como == sa::YA {
        return;
    }
    if e.n == BITACORA {
        e.bitacora.copy_within(1.., 0);
        e.n -= 1;
    }
    e.bitacora[e.n] = Hecho { nombre, estado: como, us, tsc: bmo::ciclos() };
    e.n += 1;
}

/// **Un paso cambio lo que lee el monitor** (`init`, `pantalla`, `volcado`):
/// en vez de repintar el escritorio, el fondo del arranque entero otra vez.
pub(crate) fn repintar() {
    estado().fondo_sucio = true;
}

/// **LA 3060 DESPERTO**: el gato grande que se enciende, ~[`DESPIERTA_MS`]
/// a ~30 fotogramas por segundo. Una vez por arranque.
fn despertar(p: &bmo::Pantalla) {
    let e = estado();
    sa::fondo(p, 0);
    let desde = bmo::ciclos();
    loop {
        let ms = bmo::ciclos().wrapping_sub(desde) / e.por_ms;
        let t = (ms.min(DESPIERTA_MS) * 1000 / DESPIERTA_MS) as u32;
        sa::despertar(p, t);
        p.vaciar();
        if ms >= DESPIERTA_MS {
            break;
        }
        esperar_ms(33);
    }
    e.fondo_sucio = true;
}

fn pintar(p: &bmo::Pantalla, nombre: &[u8], que: &[u8], etapa: &[u8], acento: u32) {
    let e = estado();
    // La 3060 acaba de despertar (el paso `despertar`): se celebra, y desde
    // aqui el fondo brilla y el gato del panel tambien.
    if !e.despierta && crate::commands::gsp::despierto() {
        e.despierta = true;
        despertar(p);
    }
    // Mientras duerme, la pantalla se va apagando: el fondo entero cada vez.
    if !e.despierta {
        e.luz = e.luz.saturating_sub(BAJA).max(8);
        e.fondo_sucio = true;
    }
    if e.fondo_sucio {
        sa::fondo(p, if e.despierta { 256 } else { e.luz });
        e.fondo_sucio = false;
    }
    let gato = if e.final_ {
        256
    } else if e.despierta {
        150 + crate::scene::globo::onda(ms(e), 1400) * 60 / 256
    } else {
        0
    };
    let v = Vista {
        estados: &e.estados[..e.total],
        nombre,
        que,
        hechos: &e.bitacora[..e.n],
        ms: ms(e),
        etapa,
        acento,
        gato,
    };
    sa::panel(p, &v);
    p.vaciar();
}

/// **LA 3060 TOMA EL CONTROL**, y el escritorio vuelve. Al final de `save
/// mode`, antes de que el volcado por la 3060 se arme.
pub(crate) fn acabar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let e = estado();
    if !e.activo {
        return;
    }
    if e.corridos > 0 {
        if crate::commands::gspcomputo::pantalla_hecha() {
            tomar(p);
        } else {
            pintar(
                p,
                b"listo",
                b"la 3060 no llego a pintar la pantalla en este arranque: `gpu` dice que fila falta. El escritorio lo pinta la CPU",
                b"CPU: arranque hecho sin la 3060",
                VERDE,
            );
            esperar_ms(DESPUES_MS);
        }
    }
    estado().activo = false;
    crate::repintar_escritorio(p, dsk, "arranque orquestado");
}

/// La 3060 lleva la pantalla ENTERA ~[`TOMA_MS`]; despues, el resultado.
fn tomar(p: &bmo::Pantalla) {
    pintar(
        p,
        b"LA 3060 TOMA EL CONTROL",
        b"la CPU ya preparo la RAM y la 3060 esta probada: desde aqui cada pixel del monitor lo escribe la 3060",
        b"3060: tomando la pantalla entera",
        MAGENTA,
    );
    esperar_ms(ANTES_MS);
    estado().final_ = true;
    let desde = bmo::ciclos();
    let hasta = desde + TOMA_MS * estado().por_ms;
    let (mut buenos, mut motivo) = (0u32, None);
    while buenos < TOMA_MAX && bmo::ciclos() < hasta {
        match crate::commands::gspcomputo::fotograma(buenos, buenos == 0) {
            Ok(true) => buenos += 1,
            Ok(false) => break,
            Err(m) => {
                motivo = Some(m);
                break;
            }
        }
    }
    let us = (bmo::ciclos() - desde) * 1000 / estado().por_ms;
    // Lo que dijo, en el panel, ENCIMA del ultimo fotograma de la 3060: solo
    // la caja del panel se vuelca, el resto de la pantalla sigue siendo suyo.
    let mut t = Tira::nueva();
    t.d(buenos as u64).t(b" fotogramas de ").d(p.ancho as u64).t(b"x").d(p.alto as u64);
    t.t(b" pintados por la 3060, a ").d(buenos as u64 * 1_000_000 / us.max(1));
    t.t(b" por segundo. La CPU solo pone este panel encima");
    let bien = buenos > 0 && motivo.is_none();
    let que: &[u8] = if buenos > 0 {
        t.s()
    } else {
        motivo.map_or(&b"la 3060 no pinto ningun fotograma"[..], crate::commands::iommu::motivo)
    };
    pintar(
        p,
        if bien { b"LA 3060 LLEVA LA PANTALLA" as &[u8] } else { b"la 3060 se paro" },
        que,
        if bien { b"3060: el escritorio sale ahora" as &[u8] } else { b"CPU: el escritorio sale por la CPU" },
        if bien { VERDE } else { MAGENTA },
    );
    esperar_ms(DESPUES_MS);
}
