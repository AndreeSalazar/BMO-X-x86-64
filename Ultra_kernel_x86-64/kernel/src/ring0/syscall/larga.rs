//! **LA PUERTA LARGA** (03-10) -- cual syscall tuvo el CPU con el reloj
//! callado, y cuanto.
//!
//! [carril]  AMARILLO  mide, apunta y nunca cambia lo que hace la puerta
//! [consumo] NADA      corre solo cuando una tarea cruza la puerta
//!
//! El SALIDA.TXT del 03-10 decia `latido tarde 467 ms`, `el reloj dio 2
//! ticks`, `el CPU lo tuvo: d.bex`. Eso es: el ESCRITORIO estuvo 467 ms
//! dentro de UNA puerta, y una puerta corre ENTERA con las interrupciones
//! cerradas (`entry.rs`, `SFMASK`): ni el reloj ni el hilo del bus pueden
//! entrar hasta que vuelve. El audio se corta (los `tirones` de 580 ms) por
//! lo mismo. Lo que faltaba era el NOMBRE de esa puerta: la nota del 25-09 ya
//! lo pedia ("apuntar que syscall corria").
//!
//! ```text
//!    toda puerta     rdtsc al entrar y al salir (lo que ya hace `meter`)
//!    >= 2 ms         se apunta como la ULTIMA LARGA: su operacion, su handle,
//!                    el tid y los us
//!    la mas larga    se guarda aparte: INFO_PUERTA_LARGA
//!    latido tarde    el hilo del bus, al ver el retraso, se lleva la ULTIMA
//!                    LARGA en su foto: INFO_PUERTA_DEL_LATIDO
//! ```
//!
//! El paquete (los dos INFO): `[0..16)` la operacion (`rsi`), `[16..24)` el
//! tid, `[24..32)` la clase (0 consola, 1 tarea, 2 handle, 3 wait), `[32..64)`
//! los microsegundos.
//!
//! Coste: dos `rdtsc` y una comparacion por puerta (~40 ciclos); lo demas
//! solo en las largas.

use core::sync::atomic::{AtomicU64, Ordering};

/// A partir de aqui una puerta es LARGA: medio latido del bus (4 ms).
const LARGA_US: u64 = 2_000;

static PEOR: AtomicU64 = AtomicU64::new(0);
static ULTIMA: AtomicU64 = AtomicU64::new(0);
/// El TSC en que acabo la ULTIMA larga.
static ULTIMA_FIN: AtomicU64 = AtomicU64::new(0);

/// El TSC al entrar.
#[inline(always)]
pub fn empieza() -> u64 {
    crate::ring0::task::scheduler::rdtsc()
}

/// Al salir: si fue larga, se apunta.
#[inline(always)]
pub fn acaba(t0: u64, op: u64, clase: u32, tid: u32) {
    let fin = crate::ring0::task::scheduler::rdtsc();
    let hz = crate::ring0::task::scheduler::tsc_freq();
    if hz == 0 {
        return;
    }
    let ciclos = fin.wrapping_sub(t0);
    if ciclos < hz / (1_000_000 / LARGA_US) {
        return;
    }
    apuntar(ciclos, hz, fin, op, clase, tid);
}

#[cold]
fn apuntar(ciclos: u64, hz: u64, fin: u64, op: u64, clase: u32, tid: u32) {
    let us = (ciclos as u128 * 1_000_000 / hz as u128).min(u32::MAX as u128) as u64;
    let v = (op & 0xFFFF) | ((tid as u64 & 0xFF) << 16) | ((clase as u64 & 0xFF) << 24) | (us << 32);
    ULTIMA.store(v, Ordering::Relaxed);
    ULTIMA_FIN.store(fin, Ordering::Relaxed);
    if us > PEOR.load(Ordering::Relaxed) >> 32 {
        PEOR.store(v, Ordering::Relaxed);
    }
}

/// `INFO_PUERTA_LARGA`: la puerta mas larga desde el arranque.
pub fn peor() -> u64 {
    PEOR.load(Ordering::Relaxed)
}

/// La ULTIMA puerta larga, si acabo despues de `desde` (TSC): la que estaba
/// corriendo mientras el bus esperaba. 0 si no hubo ninguna.
pub fn ultima_desde(desde: u64) -> u64 {
    if ULTIMA_FIN.load(Ordering::Relaxed) >= desde {
        ULTIMA.load(Ordering::Relaxed)
    } else {
        0
    }
}
