//! **LA MUSICA DE FONDO DEL ESCRITORIO** (S4e de `PLAN_EL_SONIDO.md`,
//! 2026-10-03).
//!
//! [consumo] APARATO   mientras suena, el tubo esta armado y el orquestador
//!                     mezcla una voz mas por trama; al encenderla o cambiar
//!                     de pieza, se COMPONE una vuelta entera (~20 s de
//!                     musica) de una vez. Apagada, nada (L6h)
//!
//! El propietario: *"me encantan las musicas que pusiste [...] como si fuera
//! fondo, integrado, que relaja al usuario hasta que aparece una notificacion
//! para avisar"*.
//!
//! # Lo que hay en el banco
//!
//! ```text
//!    [0      .. 2,5 MiB)   la vuelta que suena            canal 0 o 1
//!    [2,5 MiB.. 5 MiB)     la vuelta de la siguiente      el otro canal
//!    [5 MiB  .. 6 MiB)     los seis avisos                canales 8..16
//! ```
//!
//! Dos sitios para la musica porque cambiar de pieza no puede dejar un hueco:
//! la siguiente se compone en el sitio libre MIENTRAS suena la de ahora, y el
//! cambio es bajar una y subir otra (las dos por la rampa de 5 ms del
//! orquestador: sin clic). Y los avisos van en canales 8..16 porque esos son
//! los que el orquestador NO agacha y los que agachan la musica: un globo de
//! aviso (`globo::avisar`) suena encima, la musica se aparta 15 dB en 30 ms y
//! vuelve sola en algo mas de un segundo.
//!
//! [!] Los avisos solo suenan con el fondo encendido: armar el tubo en cada
//! arranque por si llega un aviso seria trafico en el bus sin nada que oir
//! (la regla de `audio::armar_silencio`).

use core::ptr::addr_of_mut;

use bmo_fondo::avisos::{self, AVISOS};
use bmo_fondo::{Aviso, Compositor, Mezcla, PIEZAS, TRANQUILAS};
use bmo_userland as bmo;

/// Un sitio de musica: la vuelta mas larga (76 pulsos, 2,43 MB) cabe.
const SITIO: usize = 2_621_440;
/// Donde empiezan los avisos.
const AVISOS_DESDE: usize = 2 * SITIO;
const BANCO: usize = AVISOS_DESDE + 1_048_576;
/// El primer canal de aviso (`AUDIO_FONDO_AVISO`).
const CANAL_AVISO: u64 = bmo::AUDIO_FONDO_AVISO;

struct Estado {
    memoria: Option<bmo::Memoria>,
    fondo: Option<bmo::Fondo>,
    /// La pieza que suena, y en que sitio (0 o 1, que es tambien su canal).
    pieza: usize,
    sitio: usize,
    sonando: bool,
    /// 0..=100, como lo teclea el propietario.
    volumen: u32,
    /// Donde esta cada aviso en el banco (bytes) y cuantas muestras mide.
    avisos: [(u32, u32); AVISOS.len()],
    /// El siguiente canal de aviso, en rueda: dos avisos seguidos no se pisan.
    rueda: u64,
    /// Lo que tardo la ultima composicion, en ms.
    compuso_ms: u64,
    /// En pausa: la voz sigue en su bucle, CALLADA por la rampa. Es una radio
    /// que se baja, no una cinta que se para: el orquestador no dice por
    /// donde va una voz, y volver a empezar la vuelta cada vez seria peor.
    pausada: bool,
    /// Sube con cada cambio que se ve (pieza, volumen, pausa): la PASTILLA lo
    /// mira para asomarse y decirlo.
    cambios: u32,
}

static mut ESTADO: Estado = Estado {
    memoria: None,
    fondo: None,
    pieza: TRANQUILAS[0],
    sitio: 0,
    sonando: false,
    volumen: 70,
    avisos: [(0, 0); AVISOS.len()],
    rueda: 0,
    compuso_ms: 0,
    pausada: false,
    cambios: 0,
};

fn estado() -> &'static mut Estado {
    // SAFETY: el hilo del escritorio es el unico que llega aqui.
    unsafe { &mut *addr_of_mut!(ESTADO) }
}

/// Del 0..100 tecleado al 0..256 de la voz, al cuadrado: el oido no es
/// lineal, y un mando lineal deja todo lo util en el ultimo cuarto.
fn voz(pct: u32) -> u16 {
    let p = pct.min(100);
    (p * p * 256 / 10_000) as u16
}

fn ahora_ms() -> u64 {
    crate::commands::red_nodo::ahora_ms()
}

/// Por que no se pudo.
pub(crate) enum Fallo {
    /// No hay 6 MiB contiguos (o este proceso ya gasto sus peticiones).
    Memoria,
    /// El kernel no acepto el banco: no hay tubo abierto, o no somos quien
    /// tiene la pantalla. El motivo exacto, en CABINA (`fondo`).
    Banco,
}

impl Fallo {
    pub(crate) fn texto(&self) -> &'static [u8] {
        match self {
            Fallo::Memoria => b"no hay 6 MiB de memoria para el banco de la musica",
            Fallo::Banco => b"el kernel no acepto el banco: sin tubo de audio abierto? (CABINA, `fondo`)",
        }
    }
}

/// El banco, como muestras, desde el byte `desde`.
fn muestras(m: &bmo::Memoria, desde: usize, n: usize) -> &'static mut [i16] {
    // SAFETY: `desde + 2n` cae dentro de los BANCO bytes pedidos (lo
    // aseguran SITIO y AVISOS_DESDE), la base es de pagina y `desde` es par.
    unsafe { core::slice::from_raw_parts_mut(m.base().add(desde) as *mut i16, n) }
}

/// Pide la memoria, compone los avisos y presta el banco, si no estaba.
fn preparar() -> Result<(), Fallo> {
    let e = estado();
    if e.fondo.is_some() {
        return Ok(());
    }
    if e.memoria.is_none() {
        e.memoria = Some(bmo::Memoria::request(BANCO as u64).ok_or(Fallo::Memoria)?);
    }
    let m = e.memoria.as_ref().ok_or(Fallo::Memoria)?;
    let mut desde = AVISOS_DESDE;
    for (i, a) in AVISOS.iter().enumerate() {
        let n = avisos::muestras(*a);
        let hecho = avisos::componer(*a, muestras(m, desde, n));
        e.avisos[i] = (desde as u32, hecho as u32);
        desde += (2 * n + 1) & !1;
    }
    e.fondo = Some(bmo::Fondo::prestar(m.base()).ok_or(Fallo::Banco)?);
    Ok(())
}

/// **Toca la pieza `i`** (de [`PIEZAS`]): se compone en el sitio libre y
/// entra por la rampa mientras la de antes se va por la suya.
pub(crate) fn tocar(i: usize) -> Result<(), Fallo> {
    preparar()?;
    let e = estado();
    let i = i % PIEZAS.len();
    let nuevo = if e.sonando { 1 - e.sitio } else { e.sitio };
    let t0 = ahora_ms();
    let mut c = Compositor::nuevo(&PIEZAS[i], Mezcla::FONDO);
    let n = c.muestras_del_bucle();
    let (Some(m), Some(f)) = (e.memoria.as_ref(), e.fondo.as_ref()) else { return Err(Fallo::Banco) };
    c.llenar(muestras(m, nuevo * SITIO, n));
    e.compuso_ms = ahora_ms().saturating_sub(t0);
    // Entra en silencio y sube por la rampa; la de antes baja por la suya y
    // se queda sonando muda hasta que su canal se reuse (callarla ya seria
    // cortarla en seco).
    if !f.tocar(nuevo as u64, (nuevo * SITIO) as u32, n as u32, 0, true) {
        return Err(Fallo::Banco);
    }
    f.volumen(nuevo as u64, voz(e.volumen));
    e.pausada = false;
    e.cambios = e.cambios.wrapping_add(1);
    if e.sonando && nuevo != e.sitio {
        f.volumen(e.sitio as u64, 0);
    }
    e.pieza = i;
    e.sitio = nuevo;
    e.sonando = true;
    Ok(())
}

/// **La siguiente de las tranquilas.**
pub(crate) fn siguiente() -> Result<(), Fallo> {
    let e = estado();
    let ahora = TRANQUILAS.iter().position(|&x| x == e.pieza);
    let i = match ahora {
        Some(k) if e.sonando => TRANQUILAS[(k + 1) % TRANQUILAS.len()],
        _ => TRANQUILAS[0],
    };
    tocar(i)
}

/// **El volumen**, 0..=100.
pub(crate) fn volumen(pct: u32) {
    let e = estado();
    e.volumen = pct.min(100);
    e.cambios = e.cambios.wrapping_add(1);
    if let (true, false, Some(f)) = (e.sonando, e.pausada, e.fondo.as_ref()) {
        f.volumen(e.sitio as u64, voz(e.volumen));
    }
}

/// **Pausa o sigue**: baja la voz a cero por la rampa, o la devuelve.
pub(crate) fn pausa() {
    let e = estado();
    let (true, Some(f)) = (e.sonando, e.fondo.as_ref()) else { return };
    e.pausada = !e.pausada;
    f.volumen(e.sitio as u64, if e.pausada { 0 } else { voz(e.volumen) });
    e.cambios = e.cambios.wrapping_add(1);
}

/// **La recomendacion**: la siguiente de las tranquilas que NO suena. Sale
/// del catalogo de la casa, en su orden; no hay amigos ni algoritmo que
/// inventar.
pub(crate) fn recomendada() -> usize {
    let e = estado();
    let k = TRANQUILAS.iter().position(|&x| x == e.pieza).map(|k| k + 1).unwrap_or(0);
    TRANQUILAS[k % TRANQUILAS.len()]
}

/// **Lo que la PASTILLA necesita saber**, de una vez.
#[derive(Clone, Copy)]
pub(crate) struct Vista {
    pub pieza: usize,
    pub volumen: u32,
    pub pausada: bool,
    pub cambios: u32,
}

/// Lo que suena, para la PASTILLA; `None` con el fondo apagado.
pub(crate) fn vista() -> Option<Vista> {
    let e = estado();
    e.sonando.then_some(Vista { pieza: e.pieza, volumen: e.volumen, pausada: e.pausada, cambios: e.cambios })
}

/// **Apagar**: baja por la rampa, espera a que acabe, y suelta banco y tubo.
pub(crate) fn apagar() {
    let e = estado();
    if let Some(f) = e.fondo.take() {
        f.volumen(0, 0);
        f.volumen(1, 0);
        // La rampa del orquestador son 5 ms; se le dan 15. Soltar antes
        // callaria las voces en seco, y eso es un clic.
        let t0 = ahora_ms();
        while ahora_ms().saturating_sub(t0) < 15 {
            core::hint::spin_loop();
        }
        f.soltar();
    }
    // El banco vuelve al asignador al caer el `Memoria` (es prestado).
    e.memoria = None;
    e.sonando = false;
    e.pausada = false;
}

/// **Un aviso**: suena encima de la musica, que se agacha sola. Sin fondo
/// encendido no suena (ver la cabecera).
pub(crate) fn avisar(a: Aviso) {
    let e = estado();
    let (Some(f), Some(k)) = (e.fondo.as_ref(), AVISOS.iter().position(|&x| x == a)) else { return };
    let (desde, n) = e.avisos[k];
    if n == 0 {
        return;
    }
    let canal = CANAL_AVISO + e.rueda % 8;
    e.rueda = e.rueda.wrapping_add(1);
    // Y en su SITIO (la voz de BMO-X): la orden va detras en la misma cola,
    // asi que el aviso entra ya situado. Lo urgente delante, HERMES a la
    // izquierda, lo que llega de detras (`avisos::angulo`).
    if f.tocar(canal, desde, n, 256, false) {
        f.situar(canal, 256, avisos::angulo(a));
    }
}

/// Lo que suena: la pieza, el volumen y lo que tardo en componerse.
pub(crate) fn que_suena() -> Option<(usize, u32, u64)> {
    let e = estado();
    e.sonando.then_some((e.pieza, e.volumen, e.compuso_ms))
}
