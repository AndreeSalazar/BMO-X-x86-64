//! **LA PUERTA DEL FUTEX** (07-10): esperar en una palabra de Ring 3 y
//! despertar a N, como `futex(2)` de Linux. El juez (la llave, el plazo,
//! los NO) es `bmo-futex`, con banco; aqui solo se lee la palabra y se duerme.
//!
//! [carril]  ROJO      lee memoria de la app bajo el cerrojo del planificador
//! [consumo] NADA      corre cuando un hilo tiene que ESPERAR (lo libre no
//!                     llega aqui: se toma en Ring 3 con una atomica)
//!
//! ```text
//!    esperar(dir, visto|ms)   bajo el cerrojo: la palabra, por su fisica
//!                             (`vmm::translate` + physmap, sin `stac`); si
//!                             no vale `visto`, NO (FUTEX_CAMBIO) en el acto;
//!                             si vale, duerme en la llave de (pid, dir)
//!    despertar(dir, n)        despierta a lo mas n de esa llave
//! ```
//!
//! ** La comparacion va DENTRO de `wait_current_checked`: un `despertar`
//! entre "vale" y "duermo" tomaria el mismo cerrojo, asi que no se pierde.

use super::*;
use bmo_futex::{desempaquetar, llave, plazo, NoFutex};

fn no(n: NoFutex) -> BmoStatus {
    BmoStatus::negado(n as u32, 0)
}

/// La palabra de 32 bits en `dir` del espacio de quien llama, por su
/// fisica; `None` si no esta mapeada.
fn palabra(dir: u64) -> Option<u32> {
    use crate::ring0::mm::{phys_to_virt, vmm, PAGE, PHYSMAP_SIZE};
    let f = vmm::translate(vmm::read_cr3(), dir)?;
    let f = if f & (PAGE - 1) == 0 { f | (dir & (PAGE - 1)) } else { f };
    if f + 4 > PHYSMAP_SIZE {
        return None;
    }
    // SAFETY: `f` es RAM que la tabla de quien llama mapea (alineada a 4: no
    // cruza pagina), y el physmap espeja `0..PHYSMAP_SIZE`.
    Some(unsafe { core::ptr::read_volatile(phys_to_virt(f) as *const u32) })
}

/// `FUTEX_ESPERAR(dir, visto | ms << 32)`.
pub(super) fn esperar(dir: u64, paquete: u64) -> BmoStatus {
    let pid = scheduler::current_pid();
    let key = match llave(pid, dir) {
        Ok(k) => k,
        Err(n) => return no(n),
    };
    let (visto, ms) = desempaquetar(paquete);
    match palabra(dir) {
        None => return no(NoFutex::NoEsRing3),
        Some(v) if v != visto => return no(NoFutex::Cambio),
        Some(_) => {}
    }
    let hasta = plazo(scheduler::rdtsc(), ms, scheduler::tsc_freq().max(1));
    // Bajo el cerrojo del planificador, otra vez: entre la mirada de arriba
    // y esta, otro hilo pudo cambiarla (y despertar a nadie todavia).
    let r = scheduler::wait_current_checked(key, hasta, visto as u64, || palabra(dir).map_or(u64::MAX, |v| v as u64));
    if r != visto as u64 {
        return no(NoFutex::Cambio);
    }
    BmoStatus::ok_value(0)
}

/// `FUTEX_DESPERTAR(dir, n)`: cuantos desperto.
pub(super) fn despertar(dir: u64, n: u64) -> BmoStatus {
    let pid = scheduler::current_pid();
    match llave(pid, dir) {
        Ok(k) => BmoStatus::ok_value(scheduler::wake_n_by_key(k, n.min(u32::MAX as u64) as u32) as u64),
        Err(e) => no(e),
    }
}
