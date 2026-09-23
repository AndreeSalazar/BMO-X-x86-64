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
//! [!] UN SOLO CLIENTE: el hilo del disco. Si un dia hay dos, la cosecha tiene
//! que llevar de quien era.

use super::*;

/// La orden que esta en el aparato sin que nadie tenga el disco.
#[derive(Clone, Copy)]
struct Vuelo {
    lba: u64,
    sectores: u16,
    fisica: u64,
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
            unsafe { VUELO = Some(Vuelo { lba, sectores, fisica }) };
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

/// Cierra el vuelo: quita las marcas y dice en que quedo.
fn aterrizar(v: Vuelo, e: bmo_ahci::Estado) -> Option<u16> {
    let bytes = v.sectores as u64 * SECTOR as u64;
    transfer::marcar_el_tramo(v.fisica, bytes, false, crate::ring0::task::scheduler::rdtsc());
    unsafe { VUELO = None };
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
        match bmo_ahci::sondear(PORT, true, false) {
            bmo_ahci::Estado::EnCurso => Estado::EnCurso,
            e => Estado::Termino(aterrizar(v, e)),
        }
    }
}

/// **Espera a que acabe, girando.** Para quien no puede seguir sin ella: cerrar
/// el fichero cuyo bufer es el destino, o traer ese mismo trozo por el camino
/// sincrono. Toma el disco, y tomarlo ya la cosecha.
pub fn esperar() -> Estado {
    if unsafe { VUELO.is_none() && COSECHA.is_none() } {
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
        let e = unsafe { bmo_ahci::sondear(PORT, true, false) };
        if e != bmo_ahci::Estado::EnCurso {
            let r = aterrizar(v, e);
            unsafe { COSECHA = Some(r) };
            AJENAS.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if rdtsc() >= plazo {
            // [!] El DMA puede seguir llegando: el bufer se queda MARCADO en
            // vuelo a proposito (no se llama a `aterrizar`), y el juez del DMA
            // rechazara a quien quiera reasignar esos marcos.
            crate::ring0::cabina::fault("disk", "orden en vuelo sin contestar", v.lba);
            unsafe {
                VUELO = None;
                COSECHA = Some(None);
            }
            return;
        }
        core::hint::spin_loop();
    }
}

/// Lo mas que se espera a una orden ajena. Una de 4 MiB son ~8 ms en un SATA;
/// dos segundos solo los agota un aparato que dejo de contestar.
const COSECHA_MAX_MS: u64 = 2000;
