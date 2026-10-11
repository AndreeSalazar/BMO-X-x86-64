//! **LOS OBREROS DE ESTUDIO** (VE2): las filas de cada fotograma, repartidas
//! entre los sub-directores (H4.3 de `PLAN_LOS_DOCE_DIRECTORES`) -- los
//! otros nucleos, en Ring 3, en este espacio --. Con `smp all`; sin el, este
//! nucleo solo.
//!
//! [carril]  ROJO      manda codigo de la casa a los otros nucleos
//! [riesgo]  UNICO     cada parte, SUS filas y SUS hojas: nada se pisa
//!
//! Mas simple que los de PROTON-X: estas partes no piden memoria (ESTUDIO
//! VIVO calcula en buferes que ya existen: `Vivo::calcula_en`), asi que cada
//! una solo necesita su PILA. Sin syscalls dentro (un obrero no tiene
//! puerta): por eso el panico de una parte es un `ud2`.

use core::sync::atomic::{AtomicU32, Ordering::Relaxed};

use bmo_orquesta::ring3::{empaquetar, Pedido, FIN};
use bmo_userland as bmo;

/// Lo que mide el hueco de cada parte: su pila entera.
const HUECO: u64 = 1 << 20;
/// Hasta cuantas partes (esta y 15 obreros).
pub const MAX: u32 = 16;
/// El tramo, en la ventana de reserva.
const BASE: u64 = bmo::reserva::VENTANA_BASE + (1 << 32);

/// Las pilas ya hechas (de la 1 a esta).
static HECHAS: AtomicU32 = AtomicU32::new(1);
static MXCSR: AtomicU32 = AtomicU32::new(0x1F80);

/// **En cuantas partes se reparte ahora**: los obreros sanos y este nucleo.
pub fn cuantas() -> u32 {
    (bmo::subdirectores::info() + 1).min(MAX)
}

/// Si el que corre es un obrero (su pila esta en el tramo).
pub fn en_obrero() -> bool {
    let rsp: u64;
    // SAFETY: leer rsp.
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack)) };
    (BASE..BASE + MAX as u64 * HUECO).contains(&rsp)
}

fn pedido(n: u32) -> Pedido {
    Pedido { funcion: entrada as *const () as usize as u64, arg: 0, partes: n, bloques: BASE, bloque_bytes: HUECO }
}

/// **Repartir** `f(k)` para `k` en `0..n`: la 0 aqui, las demas en los
/// obreros. Devuelve las que NO salieron (las rehace quien llama).
pub fn repartir(n: u32, f: &(dyn Fn(u32) + Sync)) -> u64 {
    let n = n.min(MAX);
    let fuera = ((1u64 << n) - 1) & !1;
    if n < 2 {
        f(0);
        return 0;
    }
    let p = pedido(n);
    // las pilas que falten
    while HECHAS.load(Relaxed) < n {
        let k = HECHAS.load(Relaxed);
        if bmo::reserva::hacer(p.bloque(k), HUECO).is_err() {
            f(0);
            return fuera;
        }
        HECHAS.store(k + 1, Relaxed);
    }
    let Some(paquete) = empaquetar(HUECO, n) else {
        f(0);
        return fuera;
    };
    for k in 1..n {
        // SAFETY: lo alto de la pila del hueco `k`, hecha arriba.
        unsafe { (p.pila(k) as *mut u64).write(FIN) };
    }
    let mut m = 0u32;
    // SAFETY: guardar el MXCSR de este nucleo, para las partes.
    unsafe { core::arch::asm!("stmxcsr [{}]", in(reg) &mut m, options(nostack)) };
    MXCSR.store(m, Relaxed);
    let gordo: &(dyn Fn(u32) + Sync) = f;
    let arg = &gordo as *const &(dyn Fn(u32) + Sync) as u64;
    if !bmo::subdirectores::preparar(BASE, paquete) || bmo::subdirectores::repartir(p.funcion, arg).is_err() {
        f(0);
        return fuera;
    }
    f(0);
    loop {
        if let Some(mal) = bmo::subdirectores::esperar() {
            return mal & fuera;
        }
    }
}

/// **Donde entra cada obrero**: con la pila de su hueco y `FIN` encima.
extern "sysv64" fn entrada(k: u64, _n: u64, arg: u64) {
    let m = MXCSR.load(Relaxed);
    // SAFETY: el MXCSR de quien reparte (lo leyo `stmxcsr`).
    unsafe { core::arch::asm!("ldmxcsr [{}]", in(reg) &m, options(nostack)) };
    // SAFETY: `arg` es la direccion de `gordo` en `repartir`, que espera a
    // que acaben todas antes de volver.
    let f = unsafe { *(arg as *const &(dyn Fn(u32) + Sync)) };
    f(k as u32);
}
