//! **El pool de hilos, InitOnce y las SList de `kernel32.dll`, de la casa**
//! (tanda 3 de Cyberpunk, paso 4a, 29-09).
//!
//! ```text
//!    InitOnce   InitOnceBeginInitialize InitOnceComplete InitOnceExecuteOnce
//!    SList      InterlockedPushEntrySList InterlockedPopEntrySList
//!               InterlockedFlushSList QueryDepthSList
//!    trabajos   Create/Submit/WaitForThreadpoolWorkCallbacks/CloseThreadpoolWork
//!    relojes    Create/Set/WaitForThreadpoolTimerCallbacks/CloseThreadpoolTimer
//!               IsThreadpoolTimerSet
//!    esperas    Create/Set/WaitForThreadpoolWaitCallbacks/CloseThreadpoolWait
//!               RegisterWaitForSingleObject UnregisterWait(Ex)
//!    y          FreeLibraryWhenCallbackReturns
//! ```
//!
//! **Como corre un callback:** cada objeto del pool (un trabajo, un reloj,
//! una espera) tiene SU hilo de la casa (`hilos::create_thread`) mientras
//! vive, y un evento para despertarlo. No uno por envio: la pila de un hilo
//! de la casa no se devuelve nunca (1 MiB), y un juego envia miles de
//! trabajos. Cancelar es subir la GENERACION del objeto y despertar su hilo:
//! lo que el hilo iba a correr con la generacion vieja ya no corre.
//!
//! **Lo que no es Windows, dicho:** los envios de UN mismo trabajo corren uno
//! detras de otro (Windows puede correrlos a la vez), y los hilos son los
//! cooperativos de la casa (`hilos.rs`): un callback que da vueltas sin
//! esperar a nada no suelta el turno. Una SList no necesita atomicos: nadie
//! le quita el turno a un hilo a mitad de un Push.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::hilos::Objeto;

use crate::{dir, hilos, kernel32};

const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_GEN_FAILURE: u32 = 31;
const INFINITE: u32 = 0xFFFF_FFFF;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 0x102;
const INVALID_HANDLE_VALUE: u64 = u64::MAX;
/// `INIT_ONCE_CHECK_ONLY`, `INIT_ONCE_ASYNC`, `INIT_ONCE_INIT_FAILED`.
const INIT_ONCE_CHECK_ONLY: u32 = 1;
const INIT_ONCE_ASYNC: u32 = 2;
const INIT_ONCE_INIT_FAILED: u32 = 4;
/// `WT_EXECUTEONLYONCE`.
const WT_EXECUTEONLYONCE: u32 = 8;

/// Los "handles" del pool (PTP_WORK, PTP_TIMER, PTP_WAIT y los de
/// RegisterWaitForSingleObject): `OBRA + indice`. El `.exe` no los mira por
/// dentro: son punteros opacos que devuelve a la casa.
const OBRA: u64 = 0x5C00_0000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tipo {
    /// `PTP_WORK_CALLBACK(instancia, contexto, trabajo)`.
    Trabajo,
    /// `PTP_TIMER_CALLBACK(instancia, contexto, reloj)`.
    Reloj,
    /// `PTP_WAIT_CALLBACK(instancia, contexto, espera, resultado)`.
    Espera,
    /// `WAITORTIMERCALLBACK(contexto, vencio)`; `una_vez` con
    /// WT_EXECUTEONLYONCE.
    Registrada { una_vez: bool },
}

struct Obra {
    tipo: Tipo,
    funcion: u64,
    contexto: u64,
    /// Sube al cancelar o al volver a poner: lo de antes ya no corre.
    generacion: u32,
    /// Trabajo: los envios que faltan por correr.
    pendientes: u32,
    corriendo: bool,
    cerrada: bool,
    /// El evento (auto) que despierta a su hilo.
    evento: u64,
    /// Reloj y espera: puestos, hasta cuando (ns de la casa) y cada cuanto.
    puesta: bool,
    vence: Option<u64>,
    periodo_ms: u32,
    /// Espera: el objeto que se espera.
    objeto: u64,
}

struct Estado {
    obras: Vec<Obra>,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: la casa corre en UNA tarea con hilos cooperativos: nadie toca esto
// a la vez (nadie cede el turno con una referencia viva: ver `con`).
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { obras: Vec::new() }));

/// Lo del pool, un momento: `f` no puede ceder el turno.
fn con<R>(f: impl FnOnce(&mut Estado) -> R) -> R {
    // SAFETY: ver `Global`; `f` no cede, asi que nadie mas entra mientras.
    f(unsafe { &mut *ESTADO.0.get() })
}

pub(crate) fn reiniciar() {
    con(|e| e.obras.clear());
}

fn indice(h: u64) -> Option<usize> {
    let i = h.checked_sub(OBRA)? as usize;
    con(|e| (i < e.obras.len() && !e.obras[i].cerrada).then_some(i))
}

/// **Llamar al `.exe` con cuatro argumentos**, como `hilos::llamar_win64`.
///
/// # Safety
/// `f` es codigo del `.exe` con esa firma.
unsafe fn llamar4(f: u64, a: u64, b: u64, c: u64, d: u64) -> u64 {
    let r: u64;
    core::arch::asm!(
        "mov r12, rsp",
        "and rsp, -16",
        "sub rsp, 32",
        "call {f}",
        "mov rsp, r12",
        f = in(reg) f,
        in("rcx") a,
        in("rdx") b,
        in("r8") c,
        in("r9") d,
        out("r12") _,
        lateout("rax") r,
        clobber_abi("win64"),
    );
    r
}

/// Una obra nueva con su hilo. `0` (y el error) si falta la funcion.
fn nueva(tipo: Tipo, funcion: u64, contexto: u64) -> u64 {
    if funcion == 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let evento = hilos::nuevo_handle(Objeto::Evento { manual: false, encendido: false });
    let i = con(|e| {
        e.obras.push(Obra { tipo, funcion, contexto, generacion: 0, pendientes: 0, corriendo: false, cerrada: false, evento, puesta: false, vence: None, periodo_ms: 0, objeto: 0 });
        e.obras.len() - 1
    });
    if hilos::create_thread(0, 0, dir!(obrero), i as u64, 0, core::ptr::null_mut()) == 0 {
        con(|e| e.obras[i].cerrada = true);
        return 0;
    }
    OBRA + i as u64
}

/// Despertar al hilo de la obra `i` (algo cambio).
fn despertar(i: usize) {
    let ev = con(|e| e.obras[i].evento);
    hilos::encender_evento(ev);
}

/// Los ms que faltan hasta `vence` (redondeando hacia arriba), o INFINITE.
fn ms_hasta(vence: Option<u64>) -> u32 {
    match vence {
        None => INFINITE,
        Some(v) => {
            let ahora = hilos::ahora_ns();
            if v <= ahora {
                0
            } else {
                ((v - ahora).div_ceil(1_000_000)).min(INFINITE as u64 - 1) as u32
            }
        }
    }
}

/// Lo que el hilo de una obra hace en una vuelta.
enum Vuelta {
    Salir,
    /// Esperar a su evento (y al objeto, si lo hay) hasta el plazo.
    Dormir { objeto: u64, ms: u32 },
    /// Correr el callback con esta generacion y este resultado.
    Correr { generacion: u32, resultado: u32 },
}

/// **El hilo de una obra**: corre sus callbacks mientras viva.
extern "win64" fn obrero(i: u64) -> u32 {
    let i = i as usize;
    loop {
        let v = con(|e| {
            let o = &mut e.obras[i];
            match o.tipo {
                Tipo::Trabajo if o.pendientes > 0 => {
                    o.pendientes -= 1;
                    Vuelta::Correr { generacion: o.generacion, resultado: 0 }
                }
                Tipo::Trabajo if o.cerrada => Vuelta::Salir,
                Tipo::Trabajo => Vuelta::Dormir { objeto: 0, ms: INFINITE },
                _ if o.cerrada => Vuelta::Salir,
                _ if !o.puesta => Vuelta::Dormir { objeto: 0, ms: INFINITE },
                Tipo::Reloj => match o.vence {
                    Some(v) if v <= hilos::ahora_ns() => Vuelta::Correr { generacion: o.generacion, resultado: 0 },
                    v => Vuelta::Dormir { objeto: 0, ms: ms_hasta(v) },
                },
                Tipo::Espera | Tipo::Registrada { .. } => Vuelta::Dormir { objeto: o.objeto, ms: ms_hasta(o.vence) },
            }
        });
        match v {
            Vuelta::Salir => return 0,
            Vuelta::Dormir { objeto: 0, ms } => {
                let ev = con(|e| e.obras[i].evento);
                hilos::wait_for_single_object(ev, ms);
            }
            Vuelta::Dormir { objeto, ms } => {
                let (ev, g) = con(|e| (e.obras[i].evento, e.obras[i].generacion));
                let hs = [ev, objeto];
                let r = hilos::wait_for_multiple_objects(2, hs.as_ptr(), 0, ms);
                // El objeto se cumplio, o vencio el plazo: si nadie cambio la
                // espera mientras, se corre.
                let resultado = match r {
                    1 => WAIT_OBJECT_0,
                    WAIT_TIMEOUT => WAIT_TIMEOUT,
                    _ => continue,
                };
                if con(|e| e.obras[i].generacion == g && e.obras[i].puesta && !e.obras[i].cerrada) {
                    correr(i, g, resultado);
                }
            }
            Vuelta::Correr { generacion, resultado } => correr(i, generacion, resultado),
        }
    }
}

/// **Correr el callback** de la obra `i`, si su generacion sigue siendo `g`.
fn correr(i: usize, g: u32, resultado: u32) {
    let Some((tipo, f, ctx)) = con(|e| {
        let o = &mut e.obras[i];
        if o.generacion != g {
            return None;
        }
        o.corriendo = true;
        // Lo que queda puesto tras este disparo.
        match o.tipo {
            Tipo::Reloj if o.periodo_ms > 0 => o.vence = Some(hilos::ahora_ns() + o.periodo_ms as u64 * 1_000_000),
            Tipo::Reloj | Tipo::Espera => o.puesta = false,
            Tipo::Registrada { una_vez: true } => o.puesta = false,
            Tipo::Registrada { una_vez: false } => o.vence = (o.periodo_ms != INFINITE).then(|| hilos::ahora_ns() + o.periodo_ms as u64 * 1_000_000),
            Tipo::Trabajo => {}
        }
        Some((o.tipo, o.funcion, o.contexto))
    }) else {
        return;
    };
    let h = OBRA + i as u64;
    // La "instancia" (PTP_CALLBACK_INSTANCE) es la obra: opaca para el `.exe`.
    // SAFETY: la funcion que el `.exe` dio al crear la obra, con su firma.
    unsafe {
        match tipo {
            Tipo::Trabajo | Tipo::Reloj => {
                llamar4(f, h, ctx, h, 0);
            }
            Tipo::Espera => {
                llamar4(f, h, ctx, h, resultado as u64);
            }
            Tipo::Registrada { .. } => {
                llamar4(f, ctx, (resultado == WAIT_TIMEOUT) as u64, 0, 0);
            }
        }
    }
    con(|e| e.obras[i].corriendo = false);
}

/// Esperar (cediendo el turno) a que la obra `i` no tenga nada corriendo, y
/// si es un trabajo, a que no le queden envios.
fn esperar_callbacks(i: usize) {
    while con(|e| e.obras[i].corriendo || (e.obras[i].tipo == Tipo::Trabajo && e.obras[i].pendientes > 0)) {
        hilos::sleep(1);
    }
}

/// Cancelar lo puesto de la obra `i` (sin cerrarla).
fn cancelar(i: usize) {
    con(|e| {
        let o = &mut e.obras[i];
        o.generacion = o.generacion.wrapping_add(1);
        o.puesta = false;
        if o.tipo == Tipo::Trabajo {
            o.pendientes = 0;
        }
    });
    despertar(i);
}

/// Cerrar la obra `i`: su hilo sale al despertar (un trabajo, cuando acabe
/// sus envios, como en Windows).
fn cerrar(i: usize) {
    con(|e| {
        let o = &mut e.obras[i];
        o.cerrada = true;
        if o.tipo != Tipo::Trabajo {
            o.generacion = o.generacion.wrapping_add(1);
            o.puesta = false;
        }
    });
    despertar(i);
}

/// Un plazo de FILETIME de Windows: negativo = relativo (100 ns), positivo =
/// absoluto (UTC). En ns de la casa.
fn plazo_de(ft: *const i64) -> Option<u64> {
    if ft.is_null() {
        return None;
    }
    // SAFETY: un FILETIME del `.exe`.
    let v = unsafe { ft.read_unaligned() };
    let ahora = hilos::ahora_ns();
    Some(if v < 0 {
        ahora + v.unsigned_abs() * 100
    } else {
        ahora + (v as u64).saturating_sub(crate::esperas::filetime_ahora()) * 100
    })
}

// -- Los trabajos ---------------------------------------------------------------------

extern "win64" fn create_threadpool_work(f: u64, ctx: u64, _entorno: u64) -> u64 {
    nueva(Tipo::Trabajo, f, ctx)
}

extern "win64" fn submit_threadpool_work(w: u64) {
    if let Some(i) = indice(w) {
        con(|e| e.obras[i].pendientes += 1);
        despertar(i);
    }
}

extern "win64" fn wait_for_threadpool_work_callbacks(w: u64, cancelar_pendientes: i32) {
    if let Some(i) = indice(w) {
        if cancelar_pendientes != 0 {
            con(|e| e.obras[i].pendientes = 0);
        }
        esperar_callbacks(i);
    }
}

extern "win64" fn close_threadpool_work(w: u64) {
    if let Some(i) = indice(w) {
        cerrar(i);
    }
}

// -- Los relojes ----------------------------------------------------------------------

extern "win64" fn create_threadpool_timer(f: u64, ctx: u64, _entorno: u64) -> u64 {
    nueva(Tipo::Reloj, f, ctx)
}

/// `SetThreadpoolTimer`: `vence` NULL lo para; si no, lo pone (y lo de antes
/// que aun no corrio ya no corre).
extern "win64" fn set_threadpool_timer(t: u64, vence: *const i64, periodo_ms: u32, _ventana_ms: u32) {
    let Some(i) = indice(t) else { return };
    let v = plazo_de(vence);
    con(|e| {
        let o = &mut e.obras[i];
        o.generacion = o.generacion.wrapping_add(1);
        o.puesta = v.is_some();
        o.vence = v;
        o.periodo_ms = periodo_ms;
    });
    despertar(i);
}

extern "win64" fn is_threadpool_timer_set(t: u64) -> i32 {
    indice(t).is_some_and(|i| con(|e| e.obras[i].puesta)) as i32
}

extern "win64" fn wait_for_threadpool_timer_callbacks(t: u64, cancelar_pendientes: i32) {
    if let Some(i) = indice(t) {
        if cancelar_pendientes != 0 {
            cancelar(i);
        }
        esperar_callbacks(i);
    }
}

extern "win64" fn close_threadpool_timer(t: u64) {
    if let Some(i) = indice(t) {
        cerrar(i);
    }
}

// -- Las esperas ----------------------------------------------------------------------

extern "win64" fn create_threadpool_wait(f: u64, ctx: u64, _entorno: u64) -> u64 {
    nueva(Tipo::Espera, f, ctx)
}

/// `SetThreadpoolWait`: `h` 0 la para; `plazo` NULL = sin plazo.
extern "win64" fn set_threadpool_wait(w: u64, h: u64, plazo: *const i64) {
    let Some(i) = indice(w) else { return };
    let v = plazo_de(plazo);
    con(|e| {
        let o = &mut e.obras[i];
        o.generacion = o.generacion.wrapping_add(1);
        o.puesta = h != 0;
        o.objeto = h;
        o.vence = v;
    });
    despertar(i);
}

extern "win64" fn wait_for_threadpool_wait_callbacks(w: u64, cancelar_pendientes: i32) {
    if let Some(i) = indice(w) {
        if cancelar_pendientes != 0 {
            cancelar(i);
        }
        esperar_callbacks(i);
    }
}

extern "win64" fn close_threadpool_wait(w: u64) {
    if let Some(i) = indice(w) {
        cerrar(i);
    }
}

/// `RegisterWaitForSingleObject`: `f(contexto, vencio)` cada vez que `h` se
/// cumple (o vence `ms`), hasta UnregisterWait; una vez con
/// WT_EXECUTEONLYONCE.
extern "win64" fn register_wait_for_single_object(nuevo: *mut u64, h: u64, f: u64, ctx: u64, ms: u32, banderas: u32) -> i32 {
    if nuevo.is_null() || h == 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let w = nueva(Tipo::Registrada { una_vez: banderas & WT_EXECUTEONLYONCE != 0 }, f, ctx);
    if w == 0 {
        return 0;
    }
    let i = (w - OBRA) as usize;
    con(|e| {
        let o = &mut e.obras[i];
        o.objeto = h;
        o.periodo_ms = ms;
        o.vence = (ms != INFINITE).then(|| hilos::ahora_ns() + ms as u64 * 1_000_000);
        o.puesta = true;
    });
    despertar(i);
    // SAFETY: un HANDLE del `.exe`.
    unsafe { *nuevo = w };
    1
}

extern "win64" fn unregister_wait(w: u64) -> i32 {
    match indice(w) {
        Some(i) => {
            cerrar(i);
            1
        }
        None => {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            0
        }
    }
}

/// `UnregisterWaitEx`: con INVALID_HANDLE_VALUE espera a que acabe el
/// callback que corra; con un evento, lo enciende cuando acaba.
extern "win64" fn unregister_wait_ex(w: u64, evento: u64) -> i32 {
    let Some(i) = indice(w) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    cerrar(i);
    if evento != 0 {
        esperar_callbacks(i);
        if evento != INVALID_HANDLE_VALUE {
            hilos::encender_evento(evento);
        }
    }
    1
}

/// `FreeLibraryWhenCallbackReturns`: la casa no descarga modulos mientras el
/// proceso vive; no hay nada que soltar.
extern "win64" fn free_library_when_callback_returns(_instancia: u64, _modulo: u64) {}

// -- InitOnce (INIT_ONCE: un puntero; 0 sin empezar, 1 a medias, ctx|2 hecho) --------

const A_MEDIAS: u64 = 1;
const HECHO: u64 = 2;

/// Esperar (cediendo) a que otro hilo acabe lo que tiene a medias.
fn mientras_a_medias(p: *mut u64) -> u64 {
    loop {
        // SAFETY: un INIT_ONCE del `.exe` (comprobado no nulo).
        let v = unsafe { p.read_volatile() };
        if v != A_MEDIAS {
            return v;
        }
        hilos::sleep(1);
    }
}

extern "win64" fn init_once_begin_initialize(p: *mut u64, banderas: u32, pendiente: *mut i32, ctx: *mut u64) -> i32 {
    if p.is_null() || pendiente.is_null() || banderas & !(INIT_ONCE_CHECK_ONLY | INIT_ONCE_ASYNC) != 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let v = if banderas & INIT_ONCE_CHECK_ONLY != 0 {
        // SAFETY: como arriba.
        unsafe { p.read_volatile() }
    } else {
        mientras_a_medias(p)
    };
    if v & HECHO != 0 {
        // SAFETY: lo del `.exe`, no nulo (el contexto es opcional).
        unsafe {
            *pendiente = 0;
            if !ctx.is_null() {
                *ctx = v & !3;
            }
        }
        return 1;
    }
    if banderas & INIT_ONCE_CHECK_ONLY != 0 {
        kernel32::poner_error(ERROR_GEN_FAILURE);
        return 0;
    }
    // SAFETY: como arriba.
    unsafe {
        p.write_volatile(A_MEDIAS);
        *pendiente = 1;
    }
    1
}

extern "win64" fn init_once_complete(p: *mut u64, banderas: u32, ctx: u64) -> i32 {
    // SAFETY: un INIT_ONCE del `.exe` (si no es nulo).
    if p.is_null() || unsafe { p.read_volatile() } != A_MEDIAS || ctx & 3 != 0 || banderas & !(INIT_ONCE_ASYNC | INIT_ONCE_INIT_FAILED) != 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let v = if banderas & INIT_ONCE_INIT_FAILED != 0 { 0 } else { ctx | HECHO };
    // SAFETY: como arriba.
    unsafe { p.write_volatile(v) };
    1
}

/// `InitOnceExecuteOnce`: `f(init_once, parametro, &contexto)` UNA vez; si
/// devuelve FALSE, queda sin hacer (y se devuelve FALSE).
extern "win64" fn init_once_execute_once(p: *mut u64, f: u64, parametro: u64, ctx: *mut u64) -> i32 {
    let mut pendiente = 0i32;
    if init_once_begin_initialize(p, 0, &mut pendiente, ctx) == 0 {
        return 0;
    }
    if pendiente == 0 {
        return 1;
    }
    let mut c = 0u64;
    // SAFETY: PINIT_ONCE_FN del `.exe`: BOOL f(PINIT_ONCE, PVOID, PVOID*).
    let bien = unsafe { hilos::llamar_win64(f, p as u64, parametro, &mut c as *mut u64 as u64) } as u32 != 0;
    if !bien {
        init_once_complete(p, INIT_ONCE_INIT_FAILED, 0);
        return 0;
    }
    init_once_complete(p, 0, c & !3);
    if !ctx.is_null() {
        // SAFETY: un PVOID del `.exe`.
        unsafe { *ctx = c & !3 };
    }
    1
}

// -- Las SList (SLIST_HEADER: la profundidad abajo, el primero en el 2o u64) --------

fn cabeza(h: *mut u64) -> (u64, u64) {
    // SAFETY: un SLIST_HEADER del `.exe` (16 bytes, alineado a 16).
    unsafe { (h.read_volatile(), h.add(1).read_volatile()) }
}

fn poner_cabeza(h: *mut u64, profundidad: u64, secuencia: u64, primero: u64) {
    // SAFETY: como `cabeza`.
    unsafe {
        h.write_volatile((profundidad & 0xFFFF) | (secuencia & 0xFFFF_FFFF_FFFF) << 16);
        h.add(1).write_volatile(primero);
    }
}

extern "win64" fn interlocked_push_entry_slist(h: *mut u64, entrada: *mut u64) -> u64 {
    if h.is_null() || entrada.is_null() {
        return 0;
    }
    let (a, primero) = cabeza(h);
    // SAFETY: un SLIST_ENTRY del `.exe` (su Next es el primer u64).
    unsafe { entrada.write_volatile(primero) };
    poner_cabeza(h, (a & 0xFFFF) + 1, (a >> 16) + 1, entrada as u64);
    primero
}

extern "win64" fn interlocked_pop_entry_slist(h: *mut u64) -> u64 {
    if h.is_null() {
        return 0;
    }
    let (a, primero) = cabeza(h);
    if primero == 0 {
        return 0;
    }
    // SAFETY: la entrada que el `.exe` metio en la lista.
    let siguiente = unsafe { (primero as *const u64).read_volatile() };
    poner_cabeza(h, (a & 0xFFFF).saturating_sub(1), (a >> 16) + 1, siguiente);
    primero
}

extern "win64" fn interlocked_flush_slist(h: *mut u64) -> u64 {
    if h.is_null() {
        return 0;
    }
    let (a, primero) = cabeza(h);
    poner_cabeza(h, 0, (a >> 16) + 1, 0);
    primero
}

extern "win64" fn query_depth_slist(h: *mut u64) -> u16 {
    if h.is_null() {
        return 0;
    }
    (cabeza(h).0 & 0xFFFF) as u16
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "InitOnceBeginInitialize" => dir!(init_once_begin_initialize),
        "InitOnceComplete" => dir!(init_once_complete),
        "InitOnceExecuteOnce" => dir!(init_once_execute_once),
        "InterlockedPushEntrySList" => dir!(interlocked_push_entry_slist),
        "InterlockedPopEntrySList" => dir!(interlocked_pop_entry_slist),
        "InterlockedFlushSList" => dir!(interlocked_flush_slist),
        "QueryDepthSList" => dir!(query_depth_slist),
        "CreateThreadpoolWork" => dir!(create_threadpool_work),
        "SubmitThreadpoolWork" => dir!(submit_threadpool_work),
        "WaitForThreadpoolWorkCallbacks" => dir!(wait_for_threadpool_work_callbacks),
        "CloseThreadpoolWork" => dir!(close_threadpool_work),
        "CreateThreadpoolTimer" => dir!(create_threadpool_timer),
        "SetThreadpoolTimer" => dir!(set_threadpool_timer),
        "IsThreadpoolTimerSet" => dir!(is_threadpool_timer_set),
        "WaitForThreadpoolTimerCallbacks" => dir!(wait_for_threadpool_timer_callbacks),
        "CloseThreadpoolTimer" => dir!(close_threadpool_timer),
        "CreateThreadpoolWait" => dir!(create_threadpool_wait),
        "SetThreadpoolWait" => dir!(set_threadpool_wait),
        "WaitForThreadpoolWaitCallbacks" => dir!(wait_for_threadpool_wait_callbacks),
        "CloseThreadpoolWait" => dir!(close_threadpool_wait),
        "RegisterWaitForSingleObject" => dir!(register_wait_for_single_object),
        "UnregisterWait" => dir!(unregister_wait),
        "UnregisterWaitEx" => dir!(unregister_wait_ex),
        "FreeLibraryWhenCallbackReturns" => dir!(free_library_when_callback_returns),
        _ => return None,
    })
}
