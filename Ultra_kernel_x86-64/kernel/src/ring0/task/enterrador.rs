//! **EL ENTERRADOR: desmonta a los muertos FUERA del cerrojo del planificador.**
//!
//! [carril]  ROJO      devuelve la pila y el espacio de direcciones de un muerto
//! [consumo] NADA      duerme sin plazo; lo despierta una muerte (L6h)
//!
//! [cuesta]  MAQUINA -- devolver un marco que alguien sigue usando es memoria
//!           viva entregada a otro, y el fallo sale tres arranques despues.
//!
//! [riesgo]  AJENO -- lo que se lee de la ranura de un muerto es lo que mas
//!           motivos tiene para estar pisado.
//!
//! # Por que existe (2026-09-23)
//!
//! El `save` de las 01:08 dijo `retenido 231 us: sched en roja.rs:771` -- el
//! cerrojo del planificador, tomado por `on_timer`, con las interrupciones
//! cerradas un cuarto de milisegundo. Acababa de morir DOOM.
//!
//! El motivo estaba escrito en la propia tabla: `reap` corria DENTRO de
//! `schedule_locked`, o sea en el tick del reloj y con `SCHED_LOCK` en la mano,
//! y ahi desmontaba el espacio de direcciones entero del muerto
//! (`destroy_address_space`: recorrer sus tablas y devolver cada hoja) y su pila.
//!
//! ** No era que `reap` fuera lento. Era que **desmontar un muerto no es
//! planificar**. El cerrojo del planificador existe para decidir QUIEN corre, y
//! estaba prestado a un trabajo de memoria. Y habia una asimetria que lo decia
//! sola:
//!
//! ```text
//!    NACER   spawn_kernel / spawn_user   piden la memoria FUERA del cerrojo y
//!                                        solo lo toman para apuntar la ranura
//!    MORIR   reap                        devolvia la memoria DENTRO del
//!                                        cerrojo, desde el tick
//! ```
//!
//! Y una segunda, mas callada: `reap` era el UNICO sitio que anidaba el cerrojo
//! `phys` dentro de `sched`. Con un solo nucleo no se nota; el dia de AXION (dos
//! nucleos planificando) una anidacion asi es un abrazo mortal esperando turno.
//!
//! # La pieza: morir y enterrar son dos momentos
//!
//! ```text
//!    Exited      el planificador ya no le da el CPU. La ranura sigue OCUPADA:
//!                el tid, el cr3 y la pila no se pueden reutilizar todavia
//!    preparar    con el cerrojo, MUY corto: las guardas y los testigos de la
//!                morgue, y la foto de los espacios vivos
//!    enterrar    SIN el cerrojo y con las interrupciones abiertas: la pila y
//!                el espacio de direcciones vuelven al asignador
//!    Empty       con el cerrojo otra vez: la ranura se libera
//! ```
//!
//! # La frontera: la TABLA es del planificador, el TRABAJO es de aqui
//!
//! ```text
//!    task/scheduler/roja.rs   `tomar_muerto` (la foto, con las guardas y la
//!                             morgue), `liberar_ranura`, y despertar al
//!                             enterrador desde el cambio de tarea
//!    task/enterrador.rs       el hilo, y `enterrar`: la pila y el espacio de
//!                             direcciones vuelven al asignador
//! ```
//!
//! El enterrador no toca la tabla por dentro, y el planificador ya no toca la
//! memoria de un muerto. Si el enterrador no existe todavia (el arranque, antes
//! de su hilo) el planificador entierra dentro, como antes: el camino viejo se
//! queda como red, no como costumbre.

use crate::ring0::mm::{self, phys};
use crate::ring0::task::scheduler::{self, Cadaver};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Por encima de las apps: la memoria de un muerto vuelve pronto. Por debajo
/// del bus USB, que no espera a nadie.
const PRIORIDAD: u8 = 1;

/// El tid del enterrador. `0` = no hay.
static TID: AtomicU32 = AtomicU32::new(0);
/// Entierros hechos, y el mas largo en microsegundos (fuera del cerrojo).
static ENTIERROS: AtomicU32 = AtomicU32::new(0);
static PEOR_US: AtomicU64 = AtomicU64::new(0);

/// **Sin el cerrojo**: la pila y el espacio vuelven al asignador.
///
/// Nadie mas puede tocar nada de esto: la ranura sigue `Exited` (no se reutiliza
/// ni el tid ni el cr3), el muerto no corre, y su espacio no es el de nadie. Lo
/// llama tambien el camino viejo del planificador, antes de que exista el hilo.
pub(crate) fn enterrar(c: &Cadaver) {
    for p in 0..c.stack_pages {
        phys::free_frame(c.stack_phys + p * mm::PAGE);
    }
    if let Some(cr3) = c.cr3 {
        let (hojas, tablas) = mm::vmm::destroy_address_space(cr3, &c.vivos[..c.nv]);
        crate::ring0::cabina::info("mm", "hojas devueltas al reciclar", hojas);
        crate::ring0::cabina::info("mm", "tablas devueltas al reciclar", tablas);
    }
}

/// **Arranca el enterrador.** Desde aqui, morir ya no le cuesta nada al tick.
pub fn arrancar() -> Option<u32> {
    let tid = scheduler::spawn_kernel(hilo as *const () as usize as u64, 0, PRIORIDAD)?;
    TID.store(tid, Ordering::Release);
    scheduler::nombrar_enterrador(tid);
    crate::ring0::cabina::id("sched", "el ENTERRADOR en pie (los muertos, fuera del cerrojo), tid", tid as u64);
    Some(tid)
}

// -- El contrato, espejo de `bmo_abi::...::informe::ENTERRADOR_*` -----------

pub const ENTERRADOR_ENTIERROS_MASK: u64 = 0xFFFF_FFFF;
pub const ENTERRADOR_PEOR_US_SHIFT: u64 = 32;
pub const ENTERRADOR_VIVO: u64 = 1 << 63;

/// `INFO_ENTERRADOR`, empaquetado. Ver el ABI.
pub fn cuentas() -> u64 {
    (ENTIERROS.load(Ordering::Relaxed) as u64 & ENTERRADOR_ENTIERROS_MASK)
        | ((PEOR_US.load(Ordering::Relaxed) & 0x7FFF_FFFF) << ENTERRADOR_PEOR_US_SHIFT)
        | if TID.load(Ordering::Relaxed) != 0 { ENTERRADOR_VIVO } else { 0 }
}

extern "C" fn hilo(_arg: u64) -> ! {
    let yo = TID.load(Ordering::Acquire);
    loop {
        // 1. Con el cerrojo del planificador, CORTO: la foto de un muerto.
        let Some(c) = scheduler::tomar_muerto(yo) else {
            scheduler::aparcar_hasta_un_muerto(yo);
            continue;
        };
        // 2. SIN el cerrojo y con las interrupciones abiertas: el trabajo.
        let t0 = scheduler::rdtsc();
        enterrar(&c);
        let us = scheduler::rdtsc().wrapping_sub(t0) / (scheduler::tsc_freq() / 1_000_000).max(1);
        // 3. Con el cerrojo otra vez: la ranura queda libre.
        if !scheduler::liberar_ranura(&c) {
            crate::ring0::cabina::fault("sched", "el enterrador encontro OTRA ranura", c.tid as u64);
        }
        ENTIERROS.fetch_add(1, Ordering::Relaxed);
        if us > PEOR_US.load(Ordering::Relaxed) {
            PEOR_US.store(us, Ordering::Relaxed);
            crate::ring0::cabina::count("sched", "entierro mas largo, FUERA del cerrojo (us)", us);
        }
    }
}
