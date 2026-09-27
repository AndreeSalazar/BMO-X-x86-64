//! **`kernel32.dll` de la casa**: la consola, la salida y lo que vive en el TEB.
//!
//! ```text
//!    GetStdHandle, WriteFile     la consola de quien nos lanzo      (P1b)
//!    ExitProcess                 la salida, con su codigo           (P1b)
//!    SetLastError, GetLastError  en el TEB, por gs:[0x68]           (P1d)
//!    GetCurrentProcessId/ThreadId  el ClientId del TEB              (P1d)
//!    GetModuleHandleW(NULL)      la base del .exe, del PEB          (P2)
//!    CreateEventW, SetEvent,     los eventos que enciende la valla  (P3a)
//!    WaitForSingleObject, CloseHandle
//! ```
//!
//! **Los eventos, en un mundo sincrono:** la cola de D3D12 de la casa termina
//! antes de volver, asi que un evento que se espera ya esta encendido. Uno que
//! NO lo esta no se va a encender nunca (no hay otro hilo): esperar para siempre
//! seria colgar el `.exe` en silencio, y se dice en vez de eso.
//!
//! Las que leen el TEB lo leen por `gs:`, como Windows: el TEB es la verdad, no
//! una variable de aqui. Un `.exe` que lea `gs:[0x68]` sin llamar a nadie ve
//! lo mismo que `GetLastError`.

use bmo_proton_x::teb;

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::{aviso, dir, plataforma};

/// Los dos que van a la consola. Valores que no son punteros ni se confunden
/// con `INVALID_HANDLE_VALUE` (-1).
const SALIDA: u64 = 0x5A1D_0001;
const ERRORES: u64 = 0x5A1D_0002;
const NO_VALE: u64 = u64::MAX;

/// `ERROR_MOD_NOT_FOUND`: lo que `GetModuleHandleW` deja en LastError.
const ERROR_MOD_NOT_FOUND: u32 = 126;

extern "win64" fn get_std_handle(n: u32) -> u64 {
    match n as i32 {
        -11 => SALIDA,
        -12 => ERRORES,
        _ => NO_VALE,
    }
}

/// `WriteFile` sobre la consola: los bytes tal cual. Quien pone la plataforma
/// decide que hacer con el `\r` de Windows (la consola de BMO-X es de lineas).
extern "win64" fn write_file(h: u64, b: *const u8, n: u32, escritos: *mut u32, _solapado: u64) -> i32 {
    if h != SALIDA && h != ERRORES {
        return 0;
    }
    // SAFETY: el `.exe` promete `n` bytes legibles en `b`, como en Windows.
    let bytes = unsafe { core::slice::from_raw_parts(b, n as usize) };
    (plataforma().escribir)(bytes);
    if !escritos.is_null() {
        // SAFETY: un DWORD suyo, como arriba.
        unsafe { *escritos = n };
    }
    1
}

extern "win64" fn exit_process(codigo: u32) -> ! {
    (plataforma().salir)(codigo)
}

/// El TEB de este hilo: `gs:[0x30]`. Lo pone quien carga antes de saltar.
pub(crate) fn teb() -> u64 {
    let v: u64;
    // SAFETY: el GS apunta al TEB desde antes de la entrada del `.exe` (P1d).
    unsafe { core::arch::asm!("mov {}, gs:[0x30]", out(reg) v, options(nostack, readonly, preserves_flags)) };
    v
}

extern "win64" fn set_last_error(e: u32) {
    // SAFETY: el TEB es de este proceso, R+W, y `+0x68` cae dentro.
    unsafe { ((teb() + teb::TEB_LAST_ERROR as u64) as *mut u32).write(e) };
}

extern "win64" fn get_last_error() -> u32 {
    // SAFETY: como arriba.
    unsafe { ((teb() + teb::TEB_LAST_ERROR as u64) as *const u32).read() }
}

extern "win64" fn get_current_process_id() -> u32 {
    // SAFETY: como arriba.
    unsafe { ((teb() + teb::TEB_PROCESS_ID as u64) as *const u64).read() as u32 }
}

extern "win64" fn get_current_thread_id() -> u32 {
    // SAFETY: como arriba.
    unsafe { ((teb() + teb::TEB_THREAD_ID as u64) as *const u64).read() as u32 }
}

/// `GetModuleHandleW(NULL)`: el propio `.exe`, su base, leida del PEB como la
/// lee Windows. Con un nombre, 0 y `ERROR_MOD_NOT_FOUND`: el unico modulo que
/// hay es el `.exe` (las DLL de la casa no son PE que se puedan nombrar).
extern "win64" fn get_module_handle_w(nombre: *const u16) -> u64 {
    if !nombre.is_null() {
        set_last_error(ERROR_MOD_NOT_FOUND);
        return 0;
    }
    // SAFETY: el TEB y el PEB son de este proceso; `+0x60` y `+0x10` caen dentro.
    unsafe {
        let peb = ((teb() + teb::TEB_PEB as u64) as *const u64).read();
        ((peb + teb::PEB_IMAGE_BASE as u64) as *const u64).read()
    }
}

/// Los eventos: (reinicio manual, encendido). El handle es `EVENTO + i`.
struct Eventos(UnsafeCell<Vec<(bool, bool)>>);
// SAFETY: un hilo (ver `Global` en lib.rs).
unsafe impl Sync for Eventos {}
static EVENTOS: Eventos = Eventos(UnsafeCell::new(Vec::new()));
const EVENTO: u64 = 0x5E00_0000;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 0x102;
const WAIT_FAILED: u32 = 0xFFFF_FFFF;
const INFINITE: u32 = 0xFFFF_FFFF;

fn eventos() -> &'static mut Vec<(bool, bool)> {
    // SAFETY: un hilo, y nadie guarda la referencia.
    unsafe { &mut *EVENTOS.0.get() }
}

pub(crate) fn reiniciar() {
    eventos().clear();
}

pub(crate) fn encender_evento(h: u64) {
    if let Some(e) = h.checked_sub(EVENTO).and_then(|i| eventos().get_mut(i as usize)) {
        e.1 = true;
    }
}

extern "win64" fn create_event_w(_attr: u64, manual: i32, inicial: i32, _nombre: *const u16) -> u64 {
    let v = eventos();
    v.push((manual != 0, inicial != 0));
    EVENTO + (v.len() - 1) as u64
}

extern "win64" fn set_event(h: u64) -> i32 {
    encender_evento(h);
    1
}

/// `WaitForSingleObject`: un evento encendido contesta YA (y se apaga si es de
/// reinicio automatico). Uno apagado no se va a encender (ver la cabecera).
extern "win64" fn wait_for_single_object(h: u64, ms: u32) -> u32 {
    let Some(e) = h.checked_sub(EVENTO).and_then(|i| eventos().get_mut(i as usize)) else {
        aviso("WaitForSingleObject sobre algo que no es un evento de la casa");
        return WAIT_FAILED;
    };
    if e.1 {
        if !e.0 {
            e.1 = false;
        }
        return WAIT_OBJECT_0;
    }
    if ms == INFINITE {
        aviso("WaitForSingleObject(INFINITE) sobre un evento que nadie va a encender: no se cuelga");
        return WAIT_FAILED;
    }
    WAIT_TIMEOUT
}

extern "win64" fn close_handle(_h: u64) -> i32 {
    1
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "GetStdHandle" => dir!(get_std_handle),
        "WriteFile" => dir!(write_file),
        "ExitProcess" => dir!(exit_process),
        "SetLastError" => dir!(set_last_error),
        "GetLastError" => dir!(get_last_error),
        "GetCurrentProcessId" => dir!(get_current_process_id),
        "GetCurrentThreadId" => dir!(get_current_thread_id),
        "GetModuleHandleW" => dir!(get_module_handle_w),
        "CreateEventW" => dir!(create_event_w),
        "SetEvent" => dir!(set_event),
        "WaitForSingleObject" => dir!(wait_for_single_object),
        "CloseHandle" => dir!(close_handle),
        _ => return None,
    })
}
