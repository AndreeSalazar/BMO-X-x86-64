//! **El sonido de la casa, en BMO-X** (03-10, N4.5): `Plataforma::sonido`
//! sobre el TUBO del audifono USB -- por donde suena lo que el juego da a
//! WASAPI.
//!
//! [carril]  AMARILLO  reclama el audifono para este proceso y le presta un
//!                     bloque (el xHC lo lee por DMA)
//! [cuesta]  APARATO   un anillo mal contado se OYE: chasquidos o eco
//! [riesgo]  RELOJ     lo que suena lo dice el aparato (`pendientes`), no la
//!                     app: si el juego no llega, hay hueco, no se inventa
//! [consumo] NADA      escribir es copiar en el bloque y decir hasta donde;
//!                     el tubo lo empuja el kernel (250 latidos por segundo
//!                     mientras esta armado: eso lo cuenta `obj/audio.rs`)
//!
//! ```text
//!    abrir      reclamar el sonido; el tubo tiene que estar y ser ESTEREO
//!               de 16 bits (bytes por trama = Hz / 1000 x 4); un bloque de
//!               64 KiB, prestado y armado
//!    escribir   lo que quepa, dejando UNA trama libre (escrito == leido es
//!               "vacio": lleno del todo no se distinguiria), dando la vuelta
//!               donde la da el kernel (`anillo`)
//!    cerrar     soltar el bloque y el sonido
//! ```
//!
//! El audifono es EXCLUSIVO: si otro proceso lo tiene, `abrir` dice que no y
//! la casa corre el reloj de WASAPI sin sonar (el juego sigue, mudo). La
//! musica del escritorio (el atril del fondo) no lo reclama, asi que no
//! estorba: el kernel la mezcla y la agacha bajo la app.

use alloc::format;
use bmo_proton_x_casa::aviso;
use bmo_userland as bmo;
use core::cell::UnsafeCell;

/// Lo que mide el bloque prestado: ~340 ms a 48 kHz, de sobra para los
/// 20-100 ms que pide un motor de sonido.
const BLOQUE: u64 = 64 * 1024;

struct Estado {
    aparato: Option<bmo::Sonido>,
    /// El bloque prestado (vive mientras suene).
    bloque: Option<bmo::Memoria>,
    hz: u32,
    /// La medida del anillo (tramas enteras) y lo que mide una trama.
    anillo: u64,
    trama: u64,
    /// Por donde va la app, dentro del anillo.
    pos: u64,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, y los hilos de la casa son cooperativos.
// [hilos] cerrojo -- el audifono: lo bombea quien lata
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { aparato: None, bloque: None, hz: 0, anillo: 0, trama: 0, pos: 0 }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub const SONIDO: bmo_proton_x_casa::Sonido = bmo_proton_x_casa::Sonido { abrir, escribir, pendientes, cerrar };

fn abrir() -> Option<u32> {
    let e = estado();
    if e.aparato.is_some() {
        return Some(e.hz);
    }
    let s = bmo::Sonido::claim()?;
    let t = s.tubo();
    let (hz, trama) = (t.frecuencia(), t.bytes_por_trama());
    // Solo s16 ESTEREO: es lo que convierte la casa. Otro tubo (mono, 5.1)
    // se dice y no se usa.
    if !t.abierto() || hz == 0 || trama != hz / 1000 * 4 {
        aviso(&format!("sonido: el tubo no esta, o no es estereo de 16 bits ({hz} Hz, {trama} B por trama): el juego, mudo"));
        s.release();
        return None;
    }
    let Some(m) = bmo::Memoria::request(BLOQUE) else {
        s.release();
        return None;
    };
    if !t.ofrecer(m.base() as u64) || !t.armar() {
        aviso("sonido: el tubo no acepto el bloque (el motivo, en CABINA): el juego, mudo");
        t.soltar();
        s.release();
        return None;
    }
    let anillo = t.anillo();
    aviso(&format!("sonido: el audifono para el juego, {hz} Hz estereo, anillo de {anillo} B"));
    *e = Estado { aparato: Some(s), bloque: Some(m), hz: hz as u32, anillo, trama, pos: 0 };
    Some(hz as u32)
}

fn escribir(b: &[u8]) -> usize {
    let e = estado();
    let (Some(s), Some(m)) = (&e.aparato, &e.bloque) else { return 0 };
    let t = s.tubo();
    if e.anillo == 0 {
        return 0;
    }
    let libre = e.anillo.saturating_sub(t.pendientes()).saturating_sub(e.trama);
    let n = (b.len() as u64).min(libre) / 4 * 4;
    let mut hecho = 0u64;
    while hecho < n {
        let trozo = (n - hecho).min(e.anillo - e.pos);
        // SAFETY: `[pos, pos + trozo)` cae dentro del anillo, que cabe en el
        // bloque de este proceso.
        unsafe { core::ptr::copy_nonoverlapping(b.as_ptr().add(hecho as usize), m.base().add(e.pos as usize), trozo as usize) };
        hecho += trozo;
        e.pos = (e.pos + trozo) % e.anillo;
    }
    if n > 0 {
        t.escrito(e.pos);
    }
    n as usize
}

fn pendientes() -> usize {
    let e = estado();
    e.aparato.as_ref().map_or(0, |s| s.tubo().pendientes() as usize)
}

fn cerrar() {
    let e = estado();
    if let Some(s) = e.aparato.take() {
        // Sin `callar`: el tubo lo comparte la musica del escritorio (el
        // atril del fondo), y callarlo la callaria a ella. Solo se suelta
        // lo nuestro: el bloque y el reclamo.
        s.tubo().soltar();
        s.release();
    }
    e.bloque = None;
    e.anillo = 0;
    e.pos = 0;
}
