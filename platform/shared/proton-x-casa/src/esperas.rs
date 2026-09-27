//! **Mas esperas de Windows, de la casa** (P4f2, 27-09): lo que la `std` de
//! Rust y un CRT piden a los hilos ademas de lo de P4.
//!
//! ```text
//!    CreateMutexA/W/ExW, ReleaseMutex       Objeto::Mutex del planificador
//!    CreateWaitableTimerW/ExW,              Objeto::Temporizador: vence a su
//!    SetWaitableTimer(Ex), Cancel...        hora, y con periodo vuelve
//!    WaitOnAddress, WakeByAddressSingle/All las mismas esperas por DIRECCION
//!                                           que las condiciones
//!    FlsAlloc/Free/GetValue/SetValue        por hilo; sus callbacks corren
//!                                           al acabar el hilo (y en FlsFree)
//!    DuplicateHandle, SetHandleInformation, GetCurrentProcess,
//!    IsThreadAFiber, SetThreadStackGuarantee
//!    GetSystemTimeAsFileTime (y Precise)    la fecha de la placa + el reloj
//! ```
//!
//! Lo que no es Windows, dicho: sin APC (un SetWaitableTimer con rutina dice
//! que no), un handle duplicado es el MISMO numero con una copia mas (se
//! cierra una vez por copia), un fichero no se duplica todavia, y la hora
//! de la placa se toma como UTC (`bmo_proton_x::hora`).

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::hilos::Objeto;
use bmo_proton_x::hora;

use crate::{aviso, dir, hilos, kernel32, plataforma};

const CREATE_MUTEX_INITIAL_OWNER: u32 = 1;
const CREATE_WAITABLE_TIMER_MANUAL_RESET: u32 = 1;
const FLS_OUT_OF_INDEXES: u32 = 0xFFFF_FFFF;
const DUPLICATE_CLOSE_SOURCE: u32 = 1;
const PSEUDO_PROCESO: u64 = u64::MAX;
const PSEUDO_HILO: u64 = u64::MAX - 1;

const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_NOT_SUPPORTED: u32 = 50;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_NOT_OWNER: u32 = 288;
const ERROR_TIMEOUT: u32 = 1460;

struct Estado {
    /// Las ranuras de FLS: `Some(callback)` (0 sin callback) si esta dada.
    fls: Vec<Option<u64>>,
    /// Por hilo, sus valores de FLS.
    valores: Vec<Vec<u64>>,
    /// Copias de mas de un handle (DuplicateHandle).
    copias: Vec<(u64, u32)>,
    /// (FILETIME, ns del reloj) cuando se leyo la placa por primera vez.
    origen: Option<(u64, u64)>,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos y ninguna funcion
// de aqui cede el turno con el estado prestado.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { fls: Vec::new(), valores: Vec::new(), copias: Vec::new(), origen: None }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.fls.clear();
    e.valores.clear();
    e.copias.clear();
    e.origen = None;
}

// -- Mutex ---------------------------------------------------------------------------

fn crear_mutex(inicial: bool) -> u64 {
    let propietario = inicial.then(hilos::actual);
    hilos::nuevo_handle(Objeto::Mutex { propietario, cuenta: inicial as u32, abandonado: false })
}

extern "win64" fn create_mutex_w(_attr: u64, inicial: i32, _nombre: u64) -> u64 {
    crear_mutex(inicial != 0)
}

extern "win64" fn create_mutex_ex_w(_attr: u64, _nombre: u64, banderas: u32, _acceso: u32) -> u64 {
    crear_mutex(banderas & CREATE_MUTEX_INITIAL_OWNER != 0)
}

extern "win64" fn release_mutex(h: u64) -> i32 {
    let quien = hilos::actual();
    if hilos::objeto(h).is_some_and(|o| hilos::con_plan(|p| p.soltar_mutex(o, quien))) {
        return 1;
    }
    kernel32::poner_error(ERROR_NOT_OWNER);
    0
}

// -- Temporizadores --------------------------------------------------------------------

extern "win64" fn create_waitable_timer_w(_attr: u64, manual: i32, _nombre: u64) -> u64 {
    hilos::nuevo_handle(Objeto::Temporizador { manual: manual != 0, encendido: false, vence: None, periodo: 0 })
}

extern "win64" fn create_waitable_timer_ex_w(_attr: u64, _nombre: u64, banderas: u32, _acceso: u32) -> u64 {
    create_waitable_timer_w(0, (banderas & CREATE_WAITABLE_TIMER_MANUAL_RESET != 0) as i32, 0)
}

/// `SetWaitableTimer(h, *vence, periodo_ms, rutina, arg, reanudar)`: `vence`
/// negativo es RELATIVO (centenas de ns); positivo, un FILETIME absoluto.
extern "win64" fn set_waitable_timer(h: u64, vence: *const i64, periodo_ms: i32, rutina: u64, _arg: u64, _reanudar: i32) -> i32 {
    if rutina != 0 {
        aviso("SetWaitableTimer con rutina: la casa no tiene APC");
        kernel32::poner_error(ERROR_NOT_SUPPORTED);
        return 0;
    }
    let (Some(o), false, true) = (hilos::objeto(h), vence.is_null(), periodo_ms >= 0) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    // SAFETY: un LARGE_INTEGER del `.exe`.
    let v = unsafe { *vence };
    let ahora = hilos::ahora_ns();
    let cuando = if v < 0 {
        ahora.saturating_add(v.unsigned_abs().saturating_mul(100))
    } else {
        ahora.saturating_add((v as u64).saturating_sub(filetime_ahora()).saturating_mul(100))
    };
    if hilos::con_plan(|p| p.poner_temporizador(o, cuando, periodo_ms as u64 * 1_000_000)) {
        1
    } else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        0
    }
}

extern "win64" fn set_waitable_timer_ex(h: u64, vence: *const i64, periodo_ms: i32, rutina: u64, arg: u64, _contexto: u64, _tolerancia: u32) -> i32 {
    set_waitable_timer(h, vence, periodo_ms, rutina, arg, 0)
}

extern "win64" fn cancel_waitable_timer(h: u64) -> i32 {
    hilos::objeto(h).is_some_and(|o| hilos::con_plan(|p| p.cancelar_temporizador(o))) as i32
}

// -- WaitOnAddress -------------------------------------------------------------------------

/// Si lo que hay en `dir` sigue siendo lo de `cmp`, se duerme ahi hasta un
/// WakeByAddress* o el plazo. Despertar sin cambio es legal (Windows tambien).
extern "win64" fn wait_on_address(dir: u64, cmp: u64, n: usize, ms: u32) -> i32 {
    if !matches!(n, 1 | 2 | 4 | 8) || dir == 0 || cmp == 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    // SAFETY: `n` bytes del `.exe` en las dos direcciones.
    let igual = unsafe { core::slice::from_raw_parts(dir as *const u8, n) == core::slice::from_raw_parts(cmp as *const u8, n) };
    if !igual {
        return 1;
    }
    if hilos::esperar_en(dir, ms) {
        return 1;
    }
    kernel32::poner_error(ERROR_TIMEOUT);
    0
}

extern "win64" fn wake_by_address_single(dir: u64) {
    hilos::con_plan(|p| p.despertar(dir, false));
}

extern "win64" fn wake_by_address_all(dir: u64) {
    hilos::con_plan(|p| p.despertar(dir, true));
}

// -- FLS -------------------------------------------------------------------------------------

fn valores_de(h: usize) -> &'static mut Vec<u64> {
    let e = estado();
    if e.valores.len() <= h {
        e.valores.resize_with(h + 1, Vec::new);
    }
    &mut e.valores[h]
}

extern "win64" fn fls_alloc(callback: u64) -> u32 {
    let e = estado();
    let i = match e.fls.iter().position(|x| x.is_none()) {
        Some(i) => i,
        None if e.fls.len() < 4096 => {
            e.fls.push(None);
            e.fls.len() - 1
        }
        None => return FLS_OUT_OF_INDEXES,
    };
    e.fls[i] = Some(callback);
    // Una ranura nueva empieza a 0 en TODOS los hilos.
    for v in &mut e.valores {
        if v.len() > i {
            v[i] = 0;
        }
    }
    i as u32
}

fn dada(i: u32) -> Option<u64> {
    estado().fls.get(i as usize).copied().flatten()
}

extern "win64" fn fls_get_value(i: u32) -> u64 {
    if dada(i).is_none() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    kernel32::poner_error(0);
    valores_de(hilos::actual()).get(i as usize).copied().unwrap_or(0)
}

extern "win64" fn fls_set_value(i: u32, v: u64) -> i32 {
    if dada(i).is_none() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let vs = valores_de(hilos::actual());
    if vs.len() <= i as usize {
        vs.resize(i as usize + 1, 0);
    }
    vs[i as usize] = v;
    1
}

/// Llamar el callback `cb` con `v` (fuera del estado prestado).
fn llamar(cb: u64, v: u64) {
    if cb != 0 && v != 0 {
        // SAFETY: el callback que el `.exe` dio a FlsAlloc: VOID WINAPI f(PVOID).
        unsafe { hilos::llamar_win64(cb, v, 0, 0) };
    }
}

/// `FlsFree`: el callback con el valor de CADA hilo que lo tenga, y la
/// ranura libre.
extern "win64" fn fls_free(i: u32) -> i32 {
    let Some(cb) = dada(i) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    estado().fls[i as usize] = None;
    let pendientes: Vec<u64> = estado().valores.iter_mut().filter_map(|v| v.get_mut(i as usize).map(core::mem::take)).collect();
    for v in pendientes {
        llamar(cb, v);
    }
    1
}

/// Al acabar un hilo: el callback de cada ranura con valor, como Windows.
pub(crate) fn fls_al_salir() {
    let h = hilos::actual();
    let mut i = 0;
    while i < estado().fls.len() {
        let cb = estado().fls[i];
        let v = valores_de(h).get_mut(i).map(core::mem::take).unwrap_or(0);
        if let Some(cb) = cb {
            llamar(cb, v);
        }
        i += 1;
    }
}

// -- Handles -----------------------------------------------------------------------------------

/// Si `h` tiene copias de mas, se gasta una y `true` (CloseHandle no cierra).
pub(crate) fn soltar_copia(h: u64) -> bool {
    let e = estado();
    match e.copias.iter().position(|&(x, _)| x == h) {
        Some(i) => {
            e.copias[i].1 -= 1;
            if e.copias[i].1 == 0 {
                e.copias.swap_remove(i);
            }
            true
        }
        None => false,
    }
}

extern "win64" fn get_current_process() -> u64 {
    PSEUDO_PROCESO
}

/// `DuplicateHandle` dentro del proceso: el hilo actual (su pseudo-handle)
/// da un handle de verdad; un objeto o la consola, el mismo numero con una
/// copia mas.
extern "win64" fn duplicate_handle(p1: u64, h: u64, p2: u64, destino: *mut u64, _acceso: u32, _hereda: i32, opciones: u32) -> i32 {
    if p1 != PSEUDO_PROCESO || p2 != PSEUDO_PROCESO {
        aviso("DuplicateHandle entre procesos: solo hay uno");
        kernel32::poner_error(ERROR_NOT_SUPPORTED);
        return 0;
    }
    let nuevo = if h == PSEUDO_HILO {
        hilos::nuevo_handle(Objeto::Hilo(hilos::actual()))
    } else if h == PSEUDO_PROCESO || kernel32::es_consola(h) {
        h
    } else if hilos::objeto(h).is_some() {
        if opciones & DUPLICATE_CLOSE_SOURCE == 0 {
            let e = estado();
            match e.copias.iter_mut().find(|(x, _)| *x == h) {
                Some((_, n)) => *n += 1,
                None => e.copias.push((h, 1)),
            }
        }
        h
    } else if crate::ficheros::es_fichero(h) {
        aviso("DuplicateHandle de un fichero: cada handle de la casa tiene su copia, y no se comparte todavia");
        kernel32::poner_error(ERROR_NOT_SUPPORTED);
        return 0;
    } else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    if !destino.is_null() {
        // SAFETY: un HANDLE del `.exe`.
        unsafe { *destino = nuevo };
    }
    1
}

extern "win64" fn set_handle_information(_h: u64, _mascara: u32, _banderas: u32) -> i32 {
    // Heredar un handle no significa nada: no hay procesos hijos.
    1
}

extern "win64" fn is_thread_a_fiber() -> i32 {
    0
}

extern "win64" fn set_thread_stack_guarantee(antes: *mut u32) -> i32 {
    if !antes.is_null() {
        // SAFETY: un ULONG del `.exe`: pide una medida y recibe la de antes.
        unsafe { *antes = 0 };
    }
    1
}

// -- La hora del dia -----------------------------------------------------------------------------

/// El FILETIME de ahora: la fecha de la placa una vez, y el reloj despues.
fn filetime_ahora() -> u64 {
    let ns = hilos::ahora_ns();
    let e = estado();
    let (base, desde) = *e.origen.get_or_insert_with(|| {
        let unix = (plataforma().fecha)().unwrap_or_else(|| {
            aviso("GetSystemTimeAsFileTime: la placa no dice que dia es; se cuenta desde 1970");
            0
        });
        (hora::filetime(unix, 0), ns)
    });
    base + (ns - desde) / 100
}

extern "win64" fn get_system_time_as_file_time(ft: *mut u64) {
    // SAFETY: un FILETIME del `.exe` (dos DWORD seguidos: un u64 sin alinear).
    unsafe { ft.write_unaligned(filetime_ahora()) };
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "CreateMutexW" | "CreateMutexA" => dir!(create_mutex_w),
        "CreateMutexExW" => dir!(create_mutex_ex_w),
        "ReleaseMutex" => dir!(release_mutex),
        "CreateWaitableTimerW" => dir!(create_waitable_timer_w),
        "CreateWaitableTimerExW" => dir!(create_waitable_timer_ex_w),
        "SetWaitableTimer" => dir!(set_waitable_timer),
        "SetWaitableTimerEx" => dir!(set_waitable_timer_ex),
        "CancelWaitableTimer" => dir!(cancel_waitable_timer),
        "WaitOnAddress" => dir!(wait_on_address),
        "WakeByAddressSingle" => dir!(wake_by_address_single),
        "WakeByAddressAll" => dir!(wake_by_address_all),
        "FlsAlloc" => dir!(fls_alloc),
        "FlsFree" => dir!(fls_free),
        "FlsGetValue" => dir!(fls_get_value),
        "FlsSetValue" => dir!(fls_set_value),
        "GetCurrentProcess" => dir!(get_current_process),
        "DuplicateHandle" => dir!(duplicate_handle),
        "SetHandleInformation" => dir!(set_handle_information),
        "IsThreadAFiber" => dir!(is_thread_a_fiber),
        "SetThreadStackGuarantee" => dir!(set_thread_stack_guarantee),
        "GetSystemTimeAsFileTime" | "GetSystemTimePreciseAsFileTime" => dir!(get_system_time_as_file_time),
        _ => return None,
    })
}

