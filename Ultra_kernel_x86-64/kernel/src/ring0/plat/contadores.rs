//! **CONTAR UN PROGRAMA** con los contadores del nucleo (C4 de
//! `docs/plan/EL_FOCO.md`, LB5 de `PLAN_LA_LENTE_BINARIA.md`, 15-10).
//!
//! [carril]  AMARILLO  escribe los PERF_CTL de este nucleo al cambiar de tarea
//! [consumo] NADA      sin nadie a quien contar, una lectura atomica por cambio
//!
//! Vive en la PLATAFORMA (como el LAPIC: es hardware de cada nucleo) y no en
//! el perfil del Ryzen: el planificador (`task`) lo llama en cada cambio de
//! tarea, y `task` no conecta con `cpu_vendor` (L8b). Si se puede tocar lo
//! dice el perfil a traves de la puerta (`syscall::contar`), como parametro.
//! La calibracion -- que cuenta cada contador en ESTE silicio -- si es del
//! perfil: `cpu_vendor/ryzen_5_5600x/contadores.rs`.

use bmo_contadores::{self as pc, Anillo, Control};
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack));
    (hi as u64) << 32 | lo as u64
}

unsafe fn wrmsr(msr: u32, v: u64) {
    asm!("wrmsr", in("ecx") msr, in("eax") v as u32, in("edx") (v >> 32) as u32, options(nostack));
}

// Los contadores 0..3 cuentan SOLO a una tarea y solo en Ring 3: el cambio de
// tarea (`scheduler::roja`) los enciende al entrar ella y los apaga al salir.
// Lo que cuentan no se pierde apagados: siguen donde estaban. El ultimo que
// empieza se queda con ellos (contar es un instrumento, no un recurso).

/// Lo que cuenta cada uno de los cuatro: el indice en `bmo_contadores::EVENTOS`.
pub const DE_UN_PROGRAMA: [usize; 4] = [0, 1, 3, 5];

/// A quien se cuenta: `tid + 1` (0 = a nadie).
static CONTANDO: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// Si estan encendidos ahora.
static ENCENDIDOS: AtomicBool = AtomicBool::new(false);

fn encender(si: bool) {
    for (k, &e) in DE_UN_PROGRAMA.iter().enumerate() {
        let c = Control::de(&pc::EVENTOS[e], Anillo::Usuario);
        let c = if si { c } else { c.apagado() };
        // SAFETY: solo se llega con `CONTANDO != 0`, que solo pone `empezar`
        // tras `se_pueden`: estos MSR existen aqui.
        unsafe { wrmsr(pc::msr(k as u32, true).0, c.palabra()) };
    }
    ENCENDIDOS.store(si, Ordering::Release);
}

/// **Empezar a contar a `tid`** (quien llama): los cuatro a cero.
/// `se_pueden` lo dice quien llama (el perfil del Ryzen: la CPU del perfil,
/// con contadores extendidos); sin eso, un MSR que no existe es un `#GP`.
pub fn empezar(tid: u32, se_pueden: bool) -> bool {
    if !se_pueden {
        return false;
    }
    CONTANDO.store(0, Ordering::Release);
    encender(false);
    for k in 0..DE_UN_PROGRAMA.len() as u32 {
        // SAFETY: como en `encender`.
        unsafe { wrmsr(pc::msr(k, true).1, 0) };
    }
    CONTANDO.store(tid.wrapping_add(1), Ordering::Release);
    // Quien llama esta corriendo: se cuenta desde ya.
    encender(true);
    true
}

/// **Al cambiar de tarea** (`scheduler::roja`, con su cerrojo): si entra la
/// que se cuenta, encendidos; si no, apagados. Sin nadie a quien contar, una
/// lectura atomica y fuera.
#[inline]
pub fn al_cambiar(tid_entrante: u32) {
    let c = CONTANDO.load(Ordering::Relaxed);
    if c == 0 {
        return;
    }
    let quiere = c == tid_entrante.wrapping_add(1);
    if quiere != ENCENDIDOS.load(Ordering::Relaxed) {
        encender(quiere);
    }
}

/// El contador `k` (0..3) de lo contado, o `None` si no se cuenta a nadie.
pub fn leer(k: u32) -> Option<u64> {
    if CONTANDO.load(Ordering::Acquire) == 0 || k as usize >= DE_UN_PROGRAMA.len() {
        return None;
    }
    // SAFETY: como en `encender`.
    Some(unsafe { rdmsr(pc::msr(k, true).1) } & pc::MASCARA_CUENTA)
}

/// Dejar de contar.
pub fn parar() {
    if CONTANDO.swap(0, Ordering::AcqRel) != 0 {
        encender(false);
    }
}
