//! **LOS SUB-DIRECTORES de PROTON-X** (H4.3 de `PLAN_LOS_DOCE_DIRECTORES`,
//! 07-10; *"te doy permiso con CPU pero que sea brutal para que exprima"*).
//!
//! [carril]  ROJO      manda codigo de la casa a los otros nucleos
//! [cuesta]  MAQUINA   una parte que pisa memoria de otra no falla: pinta mal
//! [riesgo]  UNICO     cada parte tiene SU hueco (pila y arena), por su numero
//! [consumo] NADA      solo cuando un dibujo de la CPU se reparte
//!
//! La casa pinta un dibujo de la CPU en franjas (`bmo_proton_x::bandas`) y
//! pide aqui que se repartan. Cada franja `k >= 1` la corre un obrero del
//! kernel EN RING 3, con este espacio (`bmo::subdirectores`); la 0, este
//! nucleo. Lo que una parte no pudo hacer, la casa lo rehace.
//!
//! ```text
//!    el tramo de los obreros: 16 huecos de 64 MiB, justo debajo del monton
//!
//!    hueco k   [ arena (4 MiB, crece) | ...sin hacer: guarda... | pila 2 MiB ]
//!              ^ bloque(k)                                  pila(k): FIN ^
//! ```
//!
//! * **Su memoria**: lo que una parte pide sale de la arena de SU hueco
//!   (`monton::ARENAS`, por su `rsp`): ni el cerrojo del monton ni un syscall
//!   para crecer. Si se le acaba, la parte falla (la rehace la casa) y su
//!   arena crece para el siguiente reparto, hasta [`ARENA_TOPE`].
//! * **Su pila**: arriba del hueco. Lo de en medio no esta hecho: una pila
//!   que se desborda pisa una pagina que no existe y la parte falla, en vez
//!   de pisar su arena.
//! * **Su MXCSR**: el de quien reparte (un juego que pone FTZ/DAZ pinta igual
//!   en las franjas de los obreros que en la suya).
//! * **Sin syscalls**: un obrero no tiene puerta (#UD); por eso el
//!   `panic_handler` de una parte es un `ud2` y nada mas (`monton::en_parte`).
//! * **Segun la CPU**: las partes son los obreros SANOS que el kernel cuenta
//!   ahora (`TASK_OP_SUB_INFO`) mas este, hasta 16. Sin `smp all`, ninguno:
//!   la casa dibuja como siempre.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

use bmo_orquesta::ring3::{empaquetar, Pedido, FIN};
use bmo_userland as bmo;

use crate::monton::{ARENAS, ARENAS_MAX};

/// Lo que mide el hueco de cada parte.
const HUECO: u64 = 64 << 20;
/// Su pila, arriba.
const PILA: u64 = 2 << 20;
/// Su arena, abajo: con lo que empieza y hasta donde crece (lo de en medio
/// queda sin hacer: la guarda de la pila).
const ARENA_INICIAL: u64 = 4 << 20;
pub const ARENA_TOPE: u64 = 32 << 20;
/// El tramo entero, y donde empieza: justo debajo del del monton.
pub const TRAMO: u64 = HUECO * ARENAS_MAX as u64;
pub const BASE: u64 = crate::plataforma::TRAMO_MONTON_BASE - TRAMO;

/// Lo hecho de cada hueco (solo lo toca este nucleo, entre repartos).
static PILA_HECHA: [AtomicBool; ARENAS_MAX] = [const { AtomicBool::new(false) }; ARENAS_MAX];
static ARENA_HECHA: [AtomicU64; ARENAS_MAX] = [const { AtomicU64::new(0) }; ARENAS_MAX];
static PUESTAS: AtomicBool = AtomicBool::new(false);

/// Los obreros sanos, preguntados de vez en cuando (no en cada dibujo).
static SANOS: AtomicU32 = AtomicU32::new(0);
static PREGUNTAS: AtomicU32 = AtomicU32::new(0);
/// El MXCSR de quien reparte, para las partes.
static MXCSR: AtomicU32 = AtomicU32::new(0x1F80);
/// Para decirlo una vez.
static DICHO: AtomicBool = AtomicBool::new(false);
static REPARTOS: AtomicU64 = AtomicU64::new(0);
static REHECHAS: AtomicU64 = AtomicU64::new(0);

/// `--sin-obreros`: apagados.
static APAGADOS: AtomicBool = AtomicBool::new(false);

/// **Apagar los sub-directores** (`--sin-obreros`): todo el dibujo en este
/// nucleo, aunque haya obreros en pie.
pub fn apagar() {
    APAGADOS.store(true, Relaxed);
    bmo::consola("PROTON-X: los sub-directores APAGADOS (--sin-obreros): el dibujo de la CPU, en un nucleo\n");
}

/// **En cuantas partes se puede repartir ahora** (`Plataforma::obreros`).
pub fn cuantos() -> u32 {
    if APAGADOS.load(Relaxed) {
        return 0;
    }
    if PREGUNTAS.fetch_add(1, Relaxed) % 512 == 0 {
        SANOS.store(bmo::subdirectores::info(), Relaxed);
    }
    match SANOS.load(Relaxed) {
        0 => 0,
        s => (s + 1).min(ARENAS_MAX as u32),
    }
}

fn pedido(n: u32) -> Pedido {
    Pedido { funcion: entrada as *const () as usize as u64, arg: 0, partes: n, bloques: BASE, bloque_bytes: HUECO }
}

/// Que esten hechas la pila y la arena de las partes `1..n`.
fn asegurar(n: u32) -> bool {
    if !PUESTAS.swap(true, Relaxed) {
        ARENAS.poner(BASE, HUECO);
    }
    let p = pedido(n);
    for k in 1..n as usize {
        let b = p.bloque(k as u32);
        if !PILA_HECHA[k].load(Relaxed) {
            if bmo::reserva::hacer(b + HUECO - PILA, PILA).is_err() {
                return false;
            }
            PILA_HECHA[k].store(true, Relaxed);
        }
        if ARENA_HECHA[k].load(Relaxed) == 0 {
            if bmo::reserva::hacer(b, ARENA_INICIAL).is_err() {
                return false;
            }
            ARENA_HECHA[k].store(ARENA_INICIAL, Relaxed);
        }
    }
    true
}

/// La arena `k` se quedo corta: el doble para el siguiente reparto.
fn crecer(k: usize) {
    let hecha = ARENA_HECHA[k].load(Relaxed);
    let nueva = (hecha * 2).min(ARENA_TOPE);
    if nueva > hecha && bmo::reserva::hacer(BASE + k as u64 * HUECO + hecha, nueva - hecha).is_ok() {
        ARENA_HECHA[k].store(nueva, Relaxed);
    }
}

/// **Repartir** `f(k)` para `k` en `0..n` (`Plataforma::obreros`): la 0
/// aqui, las demas en los obreros. Devuelve las que NO salieron.
pub fn repartir(n: u32, f: &(dyn Fn(u32) + Sync)) -> u64 {
    let n = n.min(ARENAS_MAX as u32);
    let fuera = ((1u64 << n) - 1) & !1;
    if n < 2 {
        f(0);
        return 0;
    }
    let Some(paquete) = empaquetar(HUECO, n) else {
        f(0);
        return fuera;
    };
    if !asegurar(n) {
        f(0);
        return fuera;
    }
    let p = pedido(n);
    for k in 1..n {
        ARENAS.preparar(k as usize, ARENA_HECHA[k as usize].load(Relaxed));
        // SAFETY: lo alto de la pila del hueco `k`, hecha (`asegurar`).
        unsafe { (p.pila(k) as *mut u64).write(FIN) };
    }
    let mut m = 0u32;
    // SAFETY: guardar el MXCSR de este nucleo.
    unsafe { core::arch::asm!("stmxcsr [{}]", in(reg) &mut m, options(nostack)) };
    MXCSR.store(m, Relaxed);
    // `f` es un puntero gordo: se deja en la pila y se pasa SU direccion.
    let gordo: &(dyn Fn(u32) + Sync) = f;
    let arg = &gordo as *const &(dyn Fn(u32) + Sync) as u64;
    if !bmo::subdirectores::preparar(BASE, paquete) {
        f(0);
        return fuera;
    }
    if bmo::subdirectores::repartir(p.funcion, arg).is_err() {
        // Los obreros que habia ya no estan (o hay otra faena): se vuelve a
        // preguntar en el siguiente dibujo.
        PREGUNTAS.store(0, Relaxed);
        f(0);
        return fuera;
    }
    f(0);
    let mut mal = loop {
        if let Some(m) = bmo::subdirectores::esperar() {
            break m & fuera;
        }
    };
    for k in 1..n as usize {
        if ARENAS.agotada(k) {
            mal |= 1 << k;
            crecer(k);
        }
    }
    REPARTOS.fetch_add(1, Relaxed);
    REHECHAS.fetch_add(mal.count_ones() as u64, Relaxed);
    if !DICHO.swap(true, Relaxed) {
        bmo::consola(&alloc::format!("PROTON-X: los sub-directores pintan en franjas: {n} partes (este nucleo y {} obreros en Ring 3) (H4.3)\n", n - 1));
    }
    mal
}

/// (repartos hechos, partes que rehizo la casa) desde que empezo.
#[allow(dead_code)]
pub fn cuentas() -> (u64, u64) {
    (REPARTOS.load(Relaxed), REHECHAS.load(Relaxed))
}

/// **Donde entra cada obrero** (`TASK_OP_SUB_REPARTIR`): en Ring 3, con la
/// pila de su hueco y `FIN` encima. Vuelve a `FIN`: el kernel lo ve y la
/// parte esta hecha.
extern "sysv64" fn entrada(k: u64, _n: u64, arg: u64) {
    let m = MXCSR.load(Relaxed);
    // SAFETY: el MXCSR de quien reparte (uno valido: lo leyo `stmxcsr`).
    unsafe { core::arch::asm!("ldmxcsr [{}]", in(reg) &m, options(nostack)) };
    // SAFETY: `arg` es la direccion de `gordo` en `repartir`, que espera a
    // que acaben todas antes de volver.
    let f = unsafe { *(arg as *const &(dyn Fn(u32) + Sync)) };
    f(k as u32);
}
