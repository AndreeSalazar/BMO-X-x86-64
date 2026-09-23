//! **EL HILO DEL DISCO: manda una orden, suelta el disco y DUERME hasta la IRQ.**
//!
//! [carril]  AMARILLO  solo decide CUANDO correr; el trabajo se lo dan
//! [consumo] NADA      sin trabajo duerme sin plazo: no late, no pregunta
//!
//! [eje]     CORRECCION -- paso D1 del plan del disco (2026-09-23)
//!
//! # Por que es un HILO y no "que el syscall duerma"
//!
//! Porque un syscall corre ENTERO con las interrupciones cerradas
//! (`MSR_SFMASK`). No tiene donde dormir a mitad, y dentro de el la IRQ del
//! disco ni siquiera llega. Un hilo de kernel corre con `IF=1`: puede aparcar
//! con `hlt` y el aviso del aparato lo despierta (`irq.rs`), cambiando de tarea
//! en el acto si tiene mas rango que quien corria.
//!
//! # Lo que hace, y lo que NO sabe
//!
//! Da vueltas a un PASO que le entregan al arrancar (`arrancar(trabajo)`): hoy
//! es la carga por trozos de los ficheros (`obj::cargando::paso`). El hilo no
//! sabe que es un fichero -- `dev` no nombra a `obj`, y el guardian de capas lo
//! vigila --; solo sabe lo que el paso le contesta:
//!
//! ```text
//!    Otra        hay mas trabajo ya: otra vuelta
//!    Esperando   hay una orden en el aparato: dormir hasta la IRQ
//!    Nada        no hay trabajo: dormir hasta que alguien avise
//! ```
//!
//! # ** Por que el paso va con las interrupciones CERRADAS
//!
//! Porque todo lo que toca --la FAT, el cursor del fichero, el disco-- lo tocan
//! tambien los syscalls, y un syscall da por hecho que nadie entra en medio. El
//! paso corre igual que uno: entero, bajo el cerrojo con nombre `disco` (que
//! ademas mide cuanto las tuvo cerradas). El hilo solo duerme **sin nada en la
//! mano**: ni el disco, ni un cursor a medias.

use super::*;
use core::sync::atomic::{AtomicBool, AtomicU64};

/// Lo que el trabajo le contesta al hilo.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    /// Queda trabajo ahora mismo.
    Otra,
    /// Hay una orden en el aparato: dormir hasta su aviso.
    Esperando,
    /// No hay nada que hacer.
    Nada,
}

/// Por encima de las apps (0) y por debajo del bus USB (2): al despertar entra
/// delante de quien estuviera pintando, sin quitarle el turno al sonido ni a
/// la entrada.
const PRIORIDAD: u8 = 1;

/// Con una orden en vuelo, cuanto se duerme como mucho si el aviso no llega.
/// Es la red de seguridad de una placa que no enruta MSI; con MSI la IRQ
/// despierta mucho antes.
const RED_MS: u64 = 2;

static CERROJO: crate::ring0::plat::spin::SpinLock = crate::ring0::plat::spin::SpinLock::new("disco");
static VIVO: AtomicBool = AtomicBool::new(false);
static TRABAJO: AtomicU64 = AtomicU64::new(0);
/// Cada aviso de "hay trabajo nuevo". Es el testigo de la guarda al dormir.
static TOQUES: AtomicU64 = AtomicU64::new(0);
/// Cuantas veces lo desperto la IRQ del disco.
static POR_IRQ: AtomicU32 = AtomicU32::new(0);

/// **Arranca el hilo** con el trabajo que va a mover. `None` si no hubo sitio:
/// entonces nadie concede esperar sobre un fichero y todo sigue como antes, con
/// el que pregunta trayendo sus trozos.
pub fn arrancar(trabajo: fn() -> Paso) -> Option<u32> {
    if !is_ready() {
        return None;
    }
    TRABAJO.store(trabajo as usize as u64, Ordering::Relaxed);
    let tid = crate::ring0::task::scheduler::spawn_kernel(hilo as *const () as usize as u64, 0, PRIORIDAD);
    match tid {
        Some(t) => {
            VIVO.store(true, Ordering::Release);
            crate::ring0::cabina::id("disk", "el disco tiene hilo propio, tid", t as u64);
        }
        None => crate::ring0::cabina::warn("disk", "NO hubo ranura para el hilo del disco", 0),
    }
    tid
}

/// Existe el hilo? Quien concede `RIGHT_WAIT` sobre un fichero lo pregunta: sin
/// hilo no hay quien mueva la secuencia, y el derecho seria una promesa rota.
pub fn vivo() -> bool {
    VIVO.load(Ordering::Acquire)
}

/// **Hay trabajo nuevo.** Desde un syscall: sube el testigo y despierta al
/// hilo. No espera a que corra -- no podria, con las interrupciones cerradas.
pub fn avisar() {
    TOQUES.fetch_add(1, Ordering::Release);
    crate::ring0::task::scheduler::wake_by_key(irq::CLAVE_ESPERA);
}

/// **Hace `f` con el paso del hilo PARADO**: bajo el mismo cerrojo `disco`.
///
/// Para quien toca lo mismo que el paso desde fuera de un syscall -- la purga
/// de Ring 3 corre en el hilo del bus, con las interrupciones abiertas, y
/// suelta ficheros. Sin esto el reloj podria cortarla a mitad y meter el paso
/// del hilo encima del mismo estado.
pub fn sin_el_hilo<R>(f: impl FnOnce() -> R) -> R {
    let _g = CERROJO.lock();
    f()
}

/// Lo llama `irq::atender` cuando el aviso era del disco.
pub(super) fn despertado_por_irq() {
    POR_IRQ.fetch_add(1, Ordering::Relaxed);
}

// -- El contrato, espejo de `bmo_abi::...::informe::DISCO_HILO_*` -----------

pub const DISCO_HILO_VUELOS_MASK: u64 = 0xFF_FFFF;
pub const DISCO_HILO_AJENAS_SHIFT: u64 = 24;
pub const DISCO_HILO_AJENAS_MASK: u64 = 0xFFFF;
pub const DISCO_HILO_IRQ_SHIFT: u64 = 40;
pub const DISCO_HILO_IRQ_MASK: u64 = 0x3F_FFFF;
pub const DISCO_HILO_VIVO: u64 = 1 << 63;

/// `INFO_DISCO_HILO`, empaquetado. Ver el ABI.
pub fn cuentas() -> u64 {
    (vuelo::VUELOS.load(Ordering::Relaxed) as u64 & DISCO_HILO_VUELOS_MASK)
        | ((vuelo::AJENAS.load(Ordering::Relaxed) as u64 & DISCO_HILO_AJENAS_MASK) << DISCO_HILO_AJENAS_SHIFT)
        | ((POR_IRQ.load(Ordering::Relaxed) as u64 & DISCO_HILO_IRQ_MASK) << DISCO_HILO_IRQ_SHIFT)
        | if vivo() { DISCO_HILO_VIVO } else { 0 }
}

extern "C" fn hilo(_arg: u64) -> ! {
    use crate::ring0::task::scheduler;
    let trabajo: fn() -> Paso = unsafe { core::mem::transmute(TRABAJO.load(Ordering::Relaxed) as usize) };
    loop {
        // Los testigos se leen ANTES del paso: un aviso que llegue durante el
        // paso los mueve, y la guarda de abajo no deja dormir sobre el.
        let toques = TOQUES.load(Ordering::Acquire);
        let avisos = bmo_ahci::AVISOS.load(Ordering::Acquire);
        let paso = {
            let _g = CERROJO.lock();
            trabajo()
        };
        let plazo = match paso {
            Paso::Otra => continue,
            Paso::Esperando => {
                scheduler::rdtsc().saturating_add(scheduler::tsc_freq() / 1000 * RED_MS)
            }
            // Sin plazo: sin trabajo, este hilo no gasta ni un tick.
            Paso::Nada => 0,
        };
        scheduler::aparcar_en(irq::CLAVE_ESPERA, plazo, || {
            TOQUES.load(Ordering::Acquire) != toques
                || bmo_ahci::AVISOS.load(Ordering::Acquire) != avisos
        });
    }
}
