//! **Quien es el proceso, de la casa** (P4e, 27-09).
//!
//! ```text
//!    GetModuleFileNameW/A     "C:\window\juego.exe" (bmo_proton_x::proceso)
//!    GetCommandLineW/A        el `.exe` entre comillas y lo escrito detras
//!    GetEnvironmentVariableW, SetEnvironmentVariableW,
//!    GetEnvironmentStringsW, FreeEnvironmentStringsW
//! ```
//!
//! Quien carga dice la ruta del `.exe` en el volumen y lo que se escribio
//! detras ([`poner_exe`]), antes de saltar. El bloque del entorno sale del
//! monton del proceso, como en Windows, y vuelve con `FreeEnvironmentStringsW`.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::proceso::{self, Entorno};

use crate::{aviso, dir, kernel32, memoria};

const ERROR_MOD_NOT_FOUND: u32 = 126;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_ENVVAR_NOT_FOUND: u32 = 203;
const ERROR_INVALID_PARAMETER: u32 = 87;

struct Estado {
    /// Con su 0 al final, para dar el puntero tal cual.
    exe_w: Vec<u16>,
    exe_a: Vec<u8>,
    linea_w: Vec<u16>,
    linea_a: Vec<u8>,
    entorno: Entorno,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { exe_w: Vec::new(), exe_a: Vec::new(), linea_w: Vec::new(), linea_a: Vec::new(), entorno: Entorno::vacio() }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    // Vacias pero con su 0: GetCommandLineW siempre da un puntero bueno.
    e.exe_w = alloc::vec![0];
    e.exe_a = alloc::vec![0];
    e.linea_w = alloc::vec![0];
    e.linea_a = alloc::vec![0];
    e.entorno = Entorno::de_bmo("C:\\");
}

/// **Quien es el `.exe`**: su ruta en el volumen (`window/juego.exe`) y lo que
/// se escribio detras. Lo dice quien carga, antes de saltar.
pub fn poner_exe(ruta: &str, resto: &str) {
    let e = estado();
    let exe = proceso::ruta_windows(ruta);
    let linea = proceso::linea(&exe, resto);
    let dir = exe.rfind('\\').map_or("C:\\", |i| &exe[..i.max(3)]);
    e.entorno = Entorno::de_bmo(dir);
    e.exe_w = exe.encode_utf16().chain([0]).collect();
    e.exe_a = exe.bytes().chain([0]).collect();
    e.linea_w = linea.encode_utf16().chain([0]).collect();
    e.linea_a = linea.bytes().chain([0]).collect();
}

/// La linea de ordenes, sin su 0 (P4f5: los argv del CRT).
pub(crate) fn linea() -> alloc::string::String {
    let l = &estado().linea_w;
    alloc::string::String::from_utf16_lossy(&l[..l.len().saturating_sub(1)])
}

/// El entorno como "N=V" (P4f5: el `environ` del CRT).
pub(crate) fn pares_del_entorno() -> Vec<Vec<u16>> {
    let b = estado().entorno.bloque();
    b.split(|&c| c == 0).filter(|x| !x.is_empty()).map(|x| x.to_vec()).collect()
}

/// Una variable del entorno (P4f3: GetTempPathW lee TMP).
pub(crate) fn variable(n: &str) -> Option<Vec<u16>> {
    let w: Vec<u16> = n.encode_utf16().collect();
    estado().entorno.leer(&w).map(|v| v.to_vec())
}

/// El nombre del `.exe` sin su ruta (`crt.exe`), para GetModuleHandle.
pub(crate) fn nombre_exe() -> alloc::string::String {
    let e = &estado().exe_a;
    let s = core::str::from_utf8(&e[..e.len() - 1]).unwrap_or("");
    alloc::string::String::from(s.rsplit('\\').next().unwrap_or(""))
}

/// Una cadena UTF-16 del `.exe`, sin su 0 (hasta 32767, lo que da Windows).
fn ancho(p: *const u16) -> Vec<u16> {
    let mut v = Vec::new();
    if p.is_null() {
        return v;
    }
    // SAFETY: el `.exe` promete una cadena terminada en 0.
    unsafe {
        while v.len() < 32767 && *p.add(v.len()) != 0 {
            v.push(*p.add(v.len()));
        }
    }
    v
}

/// Copiar `s` (con su 0 al final) a un bufer de `n` como GetModuleFileName:
/// cabe, la medida sin el 0; no cabe, `n - 1` y el 0, y devuelve `n` con
/// ERROR_INSUFFICIENT_BUFFER.
fn copiar_nombre<T: Copy + Default>(s: &[T], buf: *mut T, n: u32) -> u32 {
    let largo = s.len() - 1;
    if n == 0 {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    let cabe = (n as usize - 1).min(largo);
    // SAFETY: el `.exe` da `n` elementos en `buf`.
    unsafe {
        core::ptr::copy_nonoverlapping(s.as_ptr(), buf, cabe);
        *buf.add(cabe) = T::default();
    }
    if cabe < largo {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return n;
    }
    kernel32::poner_error(0);
    largo as u32
}

/// El unico modulo que tiene nombre es el `.exe`: NULL o su base.
fn es_el_exe(h: u64) -> bool {
    h == 0 || h == kernel32::base_imagen()
}

extern "win64" fn get_module_file_name_w(h: u64, buf: *mut u16, n: u32) -> u32 {
    if !es_el_exe(h) {
        kernel32::poner_error(ERROR_MOD_NOT_FOUND);
        return 0;
    }
    let e = estado();
    if e.exe_w.len() <= 1 {
        aviso("GetModuleFileNameW: quien cargo no dijo la ruta del .exe");
        return 0;
    }
    copiar_nombre(&e.exe_w, buf, n)
}

extern "win64" fn get_module_file_name_a(h: u64, buf: *mut u8, n: u32) -> u32 {
    if !es_el_exe(h) {
        kernel32::poner_error(ERROR_MOD_NOT_FOUND);
        return 0;
    }
    copiar_nombre(&estado().exe_a, buf, n)
}

extern "win64" fn get_command_line_w() -> *const u16 {
    estado().linea_w.as_ptr()
}

extern "win64" fn get_command_line_a() -> *const u8 {
    estado().linea_a.as_ptr()
}

/// Cabe: los caracteres sin el 0. No cabe: lo que hace falta CON el 0, y el
/// bufer sin tocar. No esta: 0 y ERROR_ENVVAR_NOT_FOUND.
extern "win64" fn get_environment_variable_w(nombre: *const u16, buf: *mut u16, n: u32) -> u32 {
    let e = estado();
    let Some(v) = e.entorno.leer(&ancho(nombre)) else {
        kernel32::poner_error(ERROR_ENVVAR_NOT_FOUND);
        return 0;
    };
    if (n as usize) <= v.len() {
        return v.len() as u32 + 1;
    }
    // SAFETY: el `.exe` da `n` > v.len() caracteres en `buf`.
    unsafe {
        core::ptr::copy_nonoverlapping(v.as_ptr(), buf, v.len());
        *buf.add(v.len()) = 0;
    }
    kernel32::poner_error(0);
    v.len() as u32
}

extern "win64" fn set_environment_variable_w(nombre: *const u16, valor: *const u16) -> i32 {
    let val = (!valor.is_null()).then(|| ancho(valor));
    match estado().entorno.poner(&ancho(nombre), val.as_deref()) {
        Ok(()) => 1,
        Err(_) => {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            0
        }
    }
}

extern "win64" fn get_environment_strings_w() -> u64 {
    let b = estado().entorno.bloque();
    let Some(p) = memoria::pedir_del_proceso(2 * b.len() as u64) else { return 0 };
    // SAFETY: un bloque recien pedido de 2 * len bytes.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), p as *mut u16, b.len()) };
    p
}

extern "win64" fn free_environment_strings_w(p: u64) -> i32 {
    if memoria::soltar_del_proceso(p) {
        1
    } else {
        aviso("FreeEnvironmentStringsW de algo que no dio GetEnvironmentStringsW");
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        0
    }
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "GetModuleFileNameW" => dir!(get_module_file_name_w),
        "GetModuleFileNameA" => dir!(get_module_file_name_a),
        "GetCommandLineW" => dir!(get_command_line_w),
        "GetCommandLineA" => dir!(get_command_line_a),
        "GetEnvironmentVariableW" => dir!(get_environment_variable_w),
        "SetEnvironmentVariableW" => dir!(set_environment_variable_w),
        "GetEnvironmentStringsW" => dir!(get_environment_strings_w),
        "FreeEnvironmentStringsW" => dir!(free_environment_strings_w),
        _ => return None,
    })
}
