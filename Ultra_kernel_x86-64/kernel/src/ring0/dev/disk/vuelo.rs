//! **UNA ORDEN EN VUELO: la que el hilo del disco deja al aparato y se va.**
//!
//! [carril]  ROJO      el HBA escribe en memoria mientras NADIE tiene el disco
//! [consumo] NADA      corre cuando el hilo del disco emite, o cuando alguien
//!                     toma el disco con una orden suya pendiente
//!
//! [eje]     CORRECCION -- paso D1 del plan del disco (2026-09-23)
//! [exige]   R-DMA-3 (no se reasigna un marco con un DMA dentro), R-DMA-4 (un
//!           bufer, un aparato)
//!
//! # Por que existe
//!
//! Hasta hoy una orden al disco empezaba y terminaba con el disco TOMADO
//! (`tomar_disco`), y como todo el que lee corre dentro de un syscall --con las
//! interrupciones cerradas--, nadie podia entrar en medio. El hilo del disco
//! rompe eso a proposito: manda la orden, SUELTA el disco y duerme hasta la IRQ.
//!
//! # ** La regla que lo hace seguro: NADIE ESPERA AL HILO
//!
//! Quien toma el disco con una orden del hilo en vuelo la TERMINA el mismo
//! ([`cosechar`], dentro de `tomar_disco`), girando como giraria con la suya, y
//! deja el resultado apuntado. El hilo lo recoge al despertar.
//!
//! La alternativa --que el que llega espere a que el hilo acabe-- es un abrazo
//! mortal: el que llega es casi siempre un syscall, con `IF=0`, y con las
//! interrupciones cerradas el hilo no vuelve a correr nunca.
//!
//! [!] UN SOLO CLIENTE para las LECTURAS: el hilo del disco. La cosecha no
//! lleva de quien era, porque solo puede ser suya.
//!
//! # ** Y EL VACIADO (10-10, op 4 de `docs/plan/EL_FOCO.md`)
//!
//! Cerrar un fichero escrito acababa en FLUSH CACHE, esperado girando dentro
//! del syscall: en un disco con cache, la orden mas larga de todas, y con el
//! reloj callado (la puerta larga de 1638 ms del 08-10, casi seguro). Ahora el
//! FLUSH tambien se deja en el aparato ([`emitir_vaciado`]): el syscall vuelve
//! y el hilo lo aterriza al despertar con su aviso. Es OTRA clase de vuelo
//! (`Que::Vaciar`): no tiene bufer, no es del hilo, y su resultado NO va a la
//! cosecha -- el hilo nunca lo confunde con su lectura.

use super::*;

/// Que clase de orden esta en vuelo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Que {
    /// Una lectura del hilo, con bufer de destino.
    Lectura,
    /// Un FLUSH CACHE del cierre de un fichero: sin bufer, de nadie.
    Vaciar,
}

/// La orden que esta en el aparato sin que nadie tenga el disco.
#[derive(Clone, Copy)]
struct Vuelo {
    que: Que,
    lba: u64,
    sectores: u16,
    fisica: u64,
    /// El TSC al emitirla: lo que tardo, al aterrizar.
    desde: u64,
}

// ** Los dos se tocan SOLO con el disco tomado o desde el hilo con las
// interrupciones cerradas: los dos caminos son atomicos en un solo nucleo.
static mut VUELO: Option<Vuelo> = None;
/// El resultado de una orden que termino OTRO. `Some(None)` = fallo.
static mut COSECHA: Option<Option<u16>> = None;

/// Ordenes mandadas en vuelo desde el arranque.
pub(super) static VUELOS: AtomicU32 = AtomicU32::new(0);
/// De esas, las que termino otro que tomo el disco antes de que el hilo
/// despertara. Si sube mucho, el hilo llega tarde a sus propias ordenes.
pub(super) static AJENAS: AtomicU32 = AtomicU32::new(0);
/// Vaciados (FLUSH) dejados en vuelo desde el arranque.
pub(super) static VACIADOS: AtomicU32 = AtomicU32::new(0);
/// El vaciado mas largo, en microsegundos: de emitirlo a verlo acabado. Lo
/// que antes era reloj callado DENTRO de cerrar un fichero.
pub(super) static VACIADO_PEOR_US: AtomicU32 = AtomicU32::new(0);

/// En que va la orden del hilo.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// No hay ninguna.
    Libre,
    /// El aparato sigue con ella.
    EnCurso,
    /// Termino: los sectores que movio de verdad, o `None` si fallo.
    Termino(Option<u16>),
}

/// **Manda una lectura y NO espera.** El disco tiene que estar tomado (el
/// testigo lo demuestra), y se suelta al volver: la orden sigue en el aparato.
///
/// `fisica` es el bufer de destino, contiguo. `prestando` dice de quien es: el
/// bufer de un fichero es de quien lo abrio y se juzga como prestado, igual
/// que el camino directo de `read`; la ventana de la FAT es del kernel.
pub fn emitir_lectura(lba: u64, sectores: u16, fisica: u64, prestando: bool) -> bool {
    if !is_ready() || sectores == 0 {
        return false;
    }
    let _testigo = tomar_disco();
    let bytes = sectores as u64 * SECTOR as u64;
    if !transfer::juzgar_el_dma(fisica, bytes, prestando) {
        return false;
    }
    let ahora = crate::ring0::task::scheduler::rdtsc();
    transfer::marcar_el_tramo(fisica, bytes, true, ahora);
    let r = unsafe {
        bmo_ahci::emitir(PORT, bmo_ahci::ATA_CMD_READ_DMA_EX, lba, sectores, Some((fisica, bytes as u32)), false)
    };
    match r {
        Ok(()) => {
            unsafe { VUELO = Some(Vuelo { que: Que::Lectura, lba, sectores, fisica, desde: ahora }) };
            VUELOS.fetch_add(1, Ordering::Relaxed);
            true
        }
        Err(e) => {
            transfer::marcar_el_tramo(fisica, bytes, false, ahora);
            crate::ring0::cabina::fault("disk", e.name(), lba);
            false
        }
    }
}

/// **Deja un FLUSH CACHE en el aparato y NO espera.** Lo pide el cierre de un
/// fichero escrito (`fsys::fs::guardar_en`): los sectores ya estan en el
/// disco, y lo que falta es que el disco los baje de su cache. `false` si no
/// se pudo emitir (entonces quien llama vacia como antes, esperando).
///
/// Toma el disco, y tomarlo cosecha la lectura del hilo si la habia: la
/// ranura 0 queda libre para el vaciado.
pub fn emitir_vaciado() -> bool {
    if !is_ready() || !write_armed() {
        return false;
    }
    let _testigo = tomar_disco();
    // CERROJO 2 (N1a): nunca al puerto del disco ajeno.
    if !ajeno::escribible(unsafe { PORT }) {
        return false;
    }
    let ahora = crate::ring0::task::scheduler::rdtsc();
    match unsafe { bmo_ahci::emitir_vaciado(PORT) } {
        Ok(()) => {
            unsafe { VUELO = Some(Vuelo { que: Que::Vaciar, lba: 0, sectores: 0, fisica: 0, desde: ahora }) };
            VACIADOS.fetch_add(1, Ordering::Relaxed);
            true
        }
        Err(e) => {
            crate::ring0::cabina::fault("disk", e.name(), 0);
            false
        }
    }
}

/// Pregunta al aparato por la orden en vuelo, segun su clase: un FLUSH no
/// mueve bytes y no tiene `PRDBC` que leer.
fn sondear(v: Vuelo) -> bmo_ahci::Estado {
    unsafe { bmo_ahci::sondear(PORT, v.que == Que::Lectura, false) }
}

/// **El vaciado, si lo hay: lo aterriza si acabo.** `true` si sigue en el
/// aparato. Lo mira el hilo ANTES de su paso: con un vaciado en vuelo, el hilo
/// no emite nada suyo (tomar el disco le haria girar hasta que acabe, con las
/// interrupciones cerradas) y duerme hasta el aviso.
pub fn vaciado_en_vuelo() -> bool {
    let Some(v) = (unsafe { VUELO }) else { return false };
    if v.que != Que::Vaciar {
        return false;
    }
    match sondear(v) {
        bmo_ahci::Estado::EnCurso => true,
        e => {
            aterrizar(v, e);
            false
        }
    }
}

/// Cierra el vuelo: quita las marcas y dice en que quedo.
fn aterrizar(v: Vuelo, e: bmo_ahci::Estado) -> Option<u16> {
    let ahora = crate::ring0::task::scheduler::rdtsc();
    unsafe { VUELO = None };
    if v.que == Que::Vaciar {
        let hz = crate::ring0::task::scheduler::tsc_freq().max(1);
        let us = (ahora.wrapping_sub(v.desde) as u128 * 1_000_000 / hz as u128).min(u32::MAX as u128) as u32;
        VACIADO_PEOR_US.fetch_max(us, Ordering::Relaxed);
        return match e {
            bmo_ahci::Estado::Fallo(err) => {
                crate::ring0::cabina::fault("disk", "el vaciado (FLUSH) en vuelo fallo", 0);
                crate::ring0::cabina::fault("disk", err.name(), 0);
                None
            }
            _ => Some(0),
        };
    }
    let bytes = v.sectores as u64 * SECTOR as u64;
    transfer::marcar_el_tramo(v.fisica, bytes, false, ahora);
    match e {
        bmo_ahci::Estado::Hecho(n) => Some(if n == u16::MAX { v.sectores } else { n }),
        bmo_ahci::Estado::Fallo(err) => {
            crate::ring0::cabina::fault("disk", err.name(), v.lba);
            None
        }
        bmo_ahci::Estado::EnCurso => None,
    }
}

/// **Mira sin esperar.** Lo llama el hilo al despertar, con las interrupciones
/// cerradas. Lee tres registros por MMIO y nada mas.
pub fn mirar() -> Estado {
    unsafe {
        if let Some(r) = COSECHA.take() {
            return Estado::Termino(r);
        }
        let Some(v) = VUELO else { return Estado::Libre };
        // Un vaciado no es del hilo: se aterriza si acabo, y para el hilo no
        // hay orden suya.
        if v.que == Que::Vaciar {
            vaciado_en_vuelo();
            return Estado::Libre;
        }
        match sondear(v) {
            bmo_ahci::Estado::EnCurso => Estado::EnCurso,
            e => Estado::Termino(aterrizar(v, e)),
        }
    }
}

/// **Espera a que acabe, girando.** Para quien no puede seguir sin ella: cerrar
/// el fichero cuyo bufer es el destino, o traer ese mismo trozo por el camino
/// sincrono. Toma el disco, y tomarlo ya la cosecha.
pub fn esperar() -> Estado {
    // Un vaciado no es el bufer de nadie: no hay por que esperarlo aqui.
    let lectura = unsafe { matches!(VUELO, Some(v) if v.que == Que::Lectura) };
    if !lectura && unsafe { COSECHA.is_none() } {
        return Estado::Libre;
    }
    drop(tomar_disco());
    mirar()
}

/// **Termina la orden del hilo, si hay una.** La llama `tomar_disco` nada mas
/// conseguirlo, antes de que el nuevo propietario toque la ranura 0.
///
/// Gira con plazo de RELOJ ([`COSECHA_MAX_MS`]) y no de vueltas: aqui cada
/// vuelta es una lectura de MMIO, que no cuesta lo mismo que las vueltas sobre
/// memoria de `run_command`. Si el aparato no contesta, se apunta como fallo.
pub(super) fn cosechar() {
    let Some(v) = (unsafe { VUELO }) else { return };
    use crate::ring0::task::scheduler::rdtsc;
    let plazo = rdtsc().saturating_add(
        crate::ring0::task::scheduler::tsc_freq() / 1000 * COSECHA_MAX_MS,
    );
    loop {
        let e = sondear(v);
        if e != bmo_ahci::Estado::EnCurso {
            let r = aterrizar(v, e);
            // El vaciado no es del hilo: su resultado no es una cosecha.
            if v.que == Que::Lectura {
                unsafe { COSECHA = Some(r) };
                AJENAS.fetch_add(1, Ordering::Relaxed);
            }
            return;
        }
        if rdtsc() >= plazo {
            // [!] El DMA puede seguir llegando: el bufer se queda MARCADO en
            // vuelo a proposito (no se llama a `aterrizar`), y el juez del DMA
            // rechazara a quien quiera reasignar esos marcos.
            crate::ring0::cabina::fault("disk", "orden en vuelo sin contestar", v.lba);
            unsafe {
                VUELO = None;
                if v.que == Que::Lectura {
                    COSECHA = Some(None);
                }
            }
            return;
        }
        core::hint::spin_loop();
    }
}

/// Lo mas que se espera a una orden ajena. Una de 4 MiB son ~8 ms en un SATA;
/// dos segundos solo los agota un aparato que dejo de contestar.
const COSECHA_MAX_MS: u64 = 2000;
