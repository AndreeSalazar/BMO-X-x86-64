//! **LA ALARMA: el reloj le quita el turno a un hilo de Ring 3** (07-10,
//! EXPROPIAR; *"se reinicio, entonces vamos a aplicar EXPROPIAR"*).
//!
//! [carril]  ROJO      cambia el RIP del marco de una tarea de Ring 3
//! [cuesta]  TAREA     una puerta mala solo rompe a la tarea que la armo
//! [consumo] NADA      sin alarmas armadas, una comparacion por tick
//!
//! La casa de PROTON-X corre los hilos de un juego DENTRO de una tarea, uno
//! detras de otro. El metal (07-10): Cyberpunk a los 24 s, un hilo dando
//! vueltas en su codigo sin llamar a nada, y los otros sin turno para
//! siempre. Linux lo resuelve con un signal de reloj (`setitimer`; Go la usa
//! desde 2020 para quitarle el turno a una gorrutina que no lo suelta). Aqui
//! es lo mismo, con lo minimo:
//!
//! ```text
//!    TASK_OP_ALARMA   la tarea da su PUERTA (codigo de Ring 3), su BUZON
//!                     (8 bytes que ella puede escribir) y cada cuantos ms
//!    el tick          si pilla a esa tarea en Ring 3, fuera de la puerta y
//!                     ya toca: el RIP que llevaba, al buzon (por su fisica);
//!                     y el RIP del marco, a la puerta
//! ```
//!
//! Nada mas: el kernel no escribe en la pila de nadie ni sabe que hay al
//! otro lado. La puerta guarda TODO, decide si cede el turno y salta al RIP
//! del buzon con todo como estaba. El juez (que vale, cuando salta) es
//! `bmo_alarma`, con banco; aqui solo se aplica.
//!
//! ** Nunca salta DENTRO de la puerta: ahi aun no ha copiado el buzon (o ya
//! lo restauro todo y va a saltar), y una segunda alarma lo pisaria.
//!
//! ** El reloj es el TSC, no la cuenta de ticks: un tick que llega tarde
//! (el metal perdio ticks con las interrupciones cerradas) no estira el
//! periodo.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering::{Acquire, Relaxed, Release}};

use bmo_alarma::{proxima, salta, validar, Alarma, NoAlarma};

use crate::ring0::mm::{phys_to_virt, vmm, PHYSMAP_SIZE};
use crate::ring0::plat::trap::TrapFrame;
use crate::ring0::task::scheduler;

/// Cuantas tareas pueden tener una alarma a la vez (una por proceso con
/// hilos de la casa: sobra).
const PUESTOS: usize = 8;

struct Puesto {
    /// 0: libre.
    tid: AtomicU32,
    inicio: AtomicU64,
    fin: AtomicU64,
    buzon: AtomicU64,
    periodo: AtomicU64,
    proxima: AtomicU64,
}

impl Puesto {
    const LIBRE: Self = Self {
        tid: AtomicU32::new(0),
        inicio: AtomicU64::new(0),
        fin: AtomicU64::new(0),
        buzon: AtomicU64::new(0),
        periodo: AtomicU64::new(0),
        proxima: AtomicU64::new(u64::MAX),
    };

    fn alarma(&self) -> Alarma {
        Alarma {
            inicio: self.inicio.load(Relaxed),
            fin: self.fin.load(Relaxed),
            buzon: self.buzon.load(Relaxed),
            periodo: self.periodo.load(Relaxed),
        }
    }
}

static TABLA: [Puesto; PUESTOS] = [Puesto::LIBRE; PUESTOS];
/// Cuantas armadas: con 0, el tick no mira la tabla.
static ARMADAS: AtomicU32 = AtomicU32::new(0);
/// Las que saltaron, y las que no pudieron (el buzon ya no era escribible).
static SALTOS: AtomicU64 = AtomicU64::new(0);
static PERDIDAS: AtomicU64 = AtomicU64::new(0);

fn ciclos_por_ms() -> u64 {
    (scheduler::tsc_freq() / 1000).max(1)
}

/// La fisica del buzon de 8 bytes, si la tarea actual lo puede escribir.
fn buzon_fisico(buzon: u64) -> Option<u64> {
    let f = vmm::fisica_escribible_ring3(vmm::read_cr3(), buzon)?;
    (f + 8 <= PHYSMAP_SIZE).then_some(f)
}

/// **`TASK_OP_ALARMA`**, para la tarea que llama (`tid`): armar, cambiar o
/// (con 0 ms) apagar la suya. Se llama desde una puerta: las interrupciones
/// estan cerradas y el tick no puede pillar la tabla a medias en este nucleo.
pub fn armar(tid: u32, inicio: u64, buzon: u64, paquete: u64) -> Result<(), NoAlarma> {
    let a = validar(inicio, buzon, paquete, ciclos_por_ms())?;
    let mio = TABLA.iter().find(|p| p.tid.load(Acquire) == tid);
    let Some(a) = a else {
        if let Some(p) = mio {
            p.tid.store(0, Release);
            ARMADAS.fetch_sub(1, Relaxed);
        }
        return Ok(());
    };
    if buzon_fisico(a.buzon).is_none() {
        return Err(NoAlarma::BuzonNoEscribible);
    }
    // El suyo, o uno libre, o el de una tarea que ya no vive (los tid no se
    // repiten).
    let p = match mio {
        Some(p) => p,
        None => {
            let p = TABLA
                .iter()
                .find(|p| p.tid.load(Acquire) == 0)
                .or_else(|| TABLA.iter().find(|p| !scheduler::vive(p.tid.load(Acquire))))
                .ok_or(NoAlarma::PuertaMala)?;
            if p.tid.swap(0, Release) == 0 {
                ARMADAS.fetch_add(1, Relaxed);
            }
            p
        }
    };
    p.tid.store(0, Release);
    p.inicio.store(a.inicio, Relaxed);
    p.fin.store(a.fin, Relaxed);
    p.buzon.store(a.buzon, Relaxed);
    p.periodo.store(a.periodo, Relaxed);
    p.proxima.store(proxima(&a, scheduler::rdtsc()), Relaxed);
    p.tid.store(tid, Release);
    crate::ring0::cabina::info("sched", "EXPROPIAR: alarma armada (puerta, cada ms) para el tid", tid as u64);
    Ok(())
}

/// **El tick**: si la tarea que pillo tiene alarma y toca, el RIP al buzon
/// y la puerta al marco. Lo llama `timer_dispatch` ANTES de planificar: el
/// marco es el de la tarea interrumpida.
pub fn al_tick(frame: &mut TrapFrame) {
    if ARMADAS.load(Relaxed) == 0 || frame.cs & 3 != 3 {
        return;
    }
    // SAFETY: en el tick, con las interrupciones cerradas (la promesa de
    // `current_tid_en_trap`).
    let tid = unsafe { scheduler::current_tid_en_trap() };
    let Some(p) = TABLA.iter().find(|p| p.tid.load(Acquire) == tid) else {
        return;
    };
    let a = p.alarma();
    let ahora = scheduler::rdtsc();
    if !salta(&a, frame.cs, frame.rip, ahora, p.proxima.load(Relaxed)) {
        return;
    }
    p.proxima.store(proxima(&a, ahora), Relaxed);
    match buzon_fisico(a.buzon) {
        Some(f) => {
            // SAFETY: RAM que la tarea interrumpida puede escribir (presente,
            // de usuario y escribible en todos los pisos), alineada a 8 (no
            // cruza pagina), dentro del physmap.
            unsafe { core::ptr::write_volatile(phys_to_virt(f) as *mut u64, frame.rip) };
            {
                use crate::ring0::cabina::tablero as t;
                t::marcar(t::ALARMA, t::SALTA, frame.rip, tid as u64);
            }
            frame.rip = a.inicio;
            SALTOS.fetch_add(1, Relaxed);
        }
        None => {
            PERDIDAS.fetch_add(1, Relaxed);
        }
    }
}

/// `(saltos, perdidas)` desde el arranque.
pub fn cuentas() -> (u64, u64) {
    (SALTOS.load(Relaxed), PERDIDAS.load(Relaxed))
}
