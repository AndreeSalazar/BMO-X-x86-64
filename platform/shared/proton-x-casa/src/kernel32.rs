//! **`kernel32.dll` de la casa**: la consola, la salida y lo que vive en el TEB.
//!
//! ```text
//!    GetStdHandle, WriteFile     la consola de quien nos lanzo      (P1b)
//!    ExitProcess                 la salida, con su codigo           (P1b)
//!    SetLastError, GetLastError  en el TEB, por gs:[0x68]           (P1d)
//!    GetCurrentProcessId/ThreadId  el ClientId del TEB              (P1d)
//!    GetModuleHandleW(NULL)      la base del .exe, del PEB          (P2)
//!    los objetos, los hilos, el TLS y la hora: `hilos.rs`       (P4)
//!    los ficheros: `ficheros.rs`                                (P4d)
//!    la memoria: `memoria.rs`; el nombre, la linea y el entorno:
//!    `proceso.rs`                                               (P4e)
//! ```
//!
//! Las que leen el TEB lo leen por `gs:`, como Windows: el TEB es la verdad, no
//! una variable de aqui. Un `.exe` que lea `gs:[0x68]` sin llamar a nadie ve
//! lo mismo que `GetLastError`.

use bmo_proton_x::teb;

use crate::{dir, plataforma};

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
        // P4d: un fichero de la casa.
        return crate::ficheros::escribir(h, b, n, escritos);
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
/// lee Windows. Con un nombre (P4f), el del `.exe` o el de una DLL de la casa
/// (`modulos.rs`); si no, 0 y `ERROR_MOD_NOT_FOUND`.
extern "win64" fn get_module_handle_w(nombre: *const u16) -> u64 {
    if !nombre.is_null() {
        // P4f: las DLL de la casa y el propio `.exe`, por su nombre.
        return crate::modulos::por_nombre_w(nombre).unwrap_or_else(|| {
            set_last_error(ERROR_MOD_NOT_FOUND);
            0
        });
    }
    base_imagen()
}

/// La base del `.exe`, del PEB.
pub(crate) fn base_imagen() -> u64 {
    // SAFETY: el TEB y el PEB son de este proceso; `+0x60` y `+0x10` caen dentro.
    unsafe {
        let peb = ((teb() + teb::TEB_PEB as u64) as *const u64).read();
        ((peb + teb::PEB_IMAGE_BASE as u64) as *const u64).read()
    }
}

/// Si `h` es la consola (la salida o los errores).
pub(crate) fn es_consola(h: u64) -> bool {
    h == SALIDA || h == ERRORES
}

/// LastError del hilo actual (para las demas DLL de la casa).
pub(crate) fn poner_error(e: u32) {
    set_last_error(e);
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
        _ => return None,
    })
}
