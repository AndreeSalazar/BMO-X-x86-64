//! **Las funciones "A" (ASCII) de `kernel32.dll`, de la casa** (tanda 3 de
//! Cyberpunk, paso 3, 29-09): cada una pasa su texto a UTF-16, llama a la
//! "W" de la casa y devuelve el resultado con las reglas de Windows (cabe: el
//! largo sin el 0; no cabe: lo que hace falta, CON el 0).
//!
//! ```text
//!    entorno    GetEnvironmentVariableA SetEnvironmentVariableA
//!               GetEnvironmentStrings(A) FreeEnvironmentStringsA
//!               ExpandEnvironmentStringsA/W
//!    rutas      GetFileAttributesA GetFullPathNameA GetTempPathA
//!               GetSystemDirectoryA GetWindowsDirectoryA
//!               GetFinalPathNameByHandleA CreateDirectoryA DeleteFileA
//!               CreateDirectoryExW FindFirstFileExA FindNextFileA
//!    modulos    GetModuleHandleExA K32GetModuleBaseNameA
//!               QueryFullProcessImageNameA
//!    objetos    CreateEventExA/W CreateWaitableTimerA
//!    consola    WriteConsoleA
//! ```
//!
//! La pagina de codigos "A" de la casa es UTF-8 (GetACP dice 65001): una
//! ruta de D: con acentos va y vuelve entera.

use alloc::string::String;
use alloc::vec::Vec;

use bmo_proton_x::texto;
use bmo_proton_x::Funcion;

use crate::{dir, kernel32, memoria};

const ERROR_INVALID_PARAMETER: u32 = 87;
/// Lo mas largo que da una "W" de la casa (el tope de rutas de Windows).
const TOPE: usize = 32_768;

/// La "W" de la casa que se llama `n` (de kernel32 o sus API set).
pub(crate) fn w<F: Copy>(n: &str) -> F {
    let d = crate::tabla_casa("kernel32.dll", &Funcion::Nombre(n.into())).unwrap_or_else(|| panic!("PROTON-X: la casa no tiene {n}"));
    // SAFETY: `n` es una funcion de la casa con la firma `F` (8 bytes).
    unsafe { core::mem::transmute_copy(&d) }
}

/// Una cadena "A" del `.exe` en UTF-16, con su 0 (NULL: `None`).
fn ancha(p: *const u8) -> Option<Vec<u16>> {
    if p.is_null() {
        return None;
    }
    let b = crate::crt::cadena_c(p as u64);
    let mut v = texto::a_ancho(&b, false).unwrap_or_default();
    v.push(0);
    Some(v)
}

fn ptr(v: &Option<Vec<u16>>) -> *const u16 {
    v.as_ref().map_or(core::ptr::null(), |v| v.as_ptr())
}

fn estrecha(w: &[u16]) -> Vec<u8> {
    texto::a_estrecho(w, false).unwrap_or_default()
}

/// Dar `b` en un bufer "A" de `n` bytes: cabe, el largo; no cabe, lo que
/// hace falta con el 0 (y el bufer sin tocar).
fn dar(b: &[u8], buf: *mut u8, n: u32) -> u32 {
    if (n as usize) <= b.len() || buf.is_null() {
        return b.len() as u32 + 1;
    }
    // SAFETY: `n` > b.len() bytes del `.exe`.
    unsafe {
        core::ptr::copy_nonoverlapping(b.as_ptr(), buf, b.len());
        *buf.add(b.len()) = 0;
    }
    b.len() as u32
}

/// Llamar a una "W" que llena un bufer y dice cuantos caracteres puso (o 0
/// si fallo): el texto, sin su 0.
fn pedir_w(f: impl FnOnce(*mut u16, u32) -> u32) -> Option<Vec<u16>> {
    let mut buf = alloc::vec![0u16; TOPE];
    let k = f(buf.as_mut_ptr(), TOPE as u32) as usize;
    if k == 0 || k >= TOPE {
        return None;
    }
    buf.truncate(k);
    Some(buf)
}

// -- El entorno ------------------------------------------------------------------------------

type GetEnvW = extern "win64" fn(*const u16, *mut u16, u32) -> u32;
type SetEnvW = extern "win64" fn(*const u16, *const u16) -> i32;

extern "win64" fn get_environment_variable_a(nombre: *const u8, buf: *mut u8, n: u32) -> u32 {
    let nombre = ancha(nombre);
    let f: GetEnvW = w("GetEnvironmentVariableW");
    match pedir_w(|b, m| f(ptr(&nombre), b, m)) {
        Some(v) => {
            kernel32::poner_error(0);
            dar(&estrecha(&v), buf, n)
        }
        // Vacia (0 sin error) o no esta: el LastError de la W manda.
        None => 0,
    }
}

extern "win64" fn set_environment_variable_a(nombre: *const u8, valor: *const u8) -> i32 {
    let (n, v) = (ancha(nombre), ancha(valor));
    let f: SetEnvW = w("SetEnvironmentVariableW");
    f(ptr(&n), ptr(&v))
}

type GetStringsW = extern "win64" fn() -> *const u16;

/// `GetEnvironmentStrings` (la "A"): el bloque "N=V\0...\0\0" en UTF-8, en
/// el monton del proceso (lo suelta FreeEnvironmentStringsA).
extern "win64" fn get_environment_strings_a() -> u64 {
    let f: GetStringsW = w("GetEnvironmentStringsW");
    let p = f();
    if p.is_null() {
        return 0;
    }
    // El bloque W acaba en dos ceros seguidos.
    let mut n = 0;
    // SAFETY: el bloque que acaba de dar la casa.
    while unsafe { *p.add(n) != 0 || *p.add(n + 1) != 0 } {
        n += 1;
    }
    // SAFETY: como arriba: n + 2 caracteres.
    let bloque = unsafe { core::slice::from_raw_parts(p, n + 2) };
    let b = estrecha(bloque);
    let free: extern "win64" fn(*const u16) -> i32 = w("FreeEnvironmentStringsW");
    free(p);
    let Some(q) = memoria::pedir_del_proceso(b.len() as u64) else { return 0 };
    // SAFETY: `b.len()` bytes recien pedidos.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), q as *mut u8, b.len()) };
    q
}

extern "win64" fn free_environment_strings_a(p: u64) -> i32 {
    memoria::soltar_del_proceso(p) as i32
}

/// Sustituir `%NOMBRE%` por su valor; lo que no esta se queda tal cual.
fn expandir(s: &[u16]) -> Vec<u16> {
    let pct = b'%' as u16;
    let mut r = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if s[i] == pct {
            if let Some(k) = s[i + 1..].iter().position(|&c| c == pct) {
                let nombre = String::from_utf16_lossy(&s[i + 1..i + 1 + k]);
                if let Some(v) = crate::proceso::variable(&nombre).filter(|_| k > 0) {
                    r.extend_from_slice(&v);
                    i += k + 2;
                    continue;
                }
            }
        }
        r.push(s[i]);
        i += 1;
    }
    r
}

/// `ExpandEnvironmentStringsW`: los caracteres CON el 0; si no caben, los
/// que hacen falta y no se escribe.
extern "win64" fn expand_environment_strings_w(src: *const u16, dst: *mut u16, n: u32) -> u32 {
    if src.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let mut r = expandir(&crate::crt::cadena_w(src as u64));
    r.push(0);
    if !dst.is_null() && (n as usize) >= r.len() {
        // SAFETY: `n` caracteres del `.exe`; caben.
        unsafe { core::ptr::copy_nonoverlapping(r.as_ptr(), dst, r.len()) };
    }
    r.len() as u32
}

extern "win64" fn expand_environment_strings_a(src: *const u8, dst: *mut u8, n: u32) -> u32 {
    let Some(s) = ancha(src) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    let mut b = estrecha(&expandir(&s[..s.len() - 1]));
    b.push(0);
    if !dst.is_null() && (n as usize) >= b.len() {
        // SAFETY: `n` bytes del `.exe`; caben.
        unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), dst, b.len()) };
    }
    b.len() as u32
}

// -- Rutas -----------------------------------------------------------------------------------

type UnaRutaW = extern "win64" fn(*const u16) -> u32;
type UnaRutaSegW = extern "win64" fn(*const u16, u64) -> i32;
type FullPathW = extern "win64" fn(*const u16, u32, *mut u16, *mut *mut u16) -> u32;
type BufLenW = extern "win64" fn(*mut u16, u32) -> u32;
type LenBufW = extern "win64" fn(u32, *mut u16) -> u32;

extern "win64" fn get_file_attributes_a(nombre: *const u8) -> u32 {
    let n = ancha(nombre);
    let f: UnaRutaW = w("GetFileAttributesW");
    f(ptr(&n))
}

extern "win64" fn create_directory_a(nombre: *const u8, seg: u64) -> i32 {
    let n = ancha(nombre);
    let f: UnaRutaSegW = w("CreateDirectoryW");
    f(ptr(&n), seg)
}

extern "win64" fn create_directory_ex_w(_plantilla: *const u16, nombre: *const u16, seg: u64) -> i32 {
    let f: UnaRutaSegW = w("CreateDirectoryW");
    f(nombre, seg)
}

extern "win64" fn delete_file_a(nombre: *const u8) -> i32 {
    let n = ancha(nombre);
    let f: UnaRutaW = w("DeleteFileW");
    f(ptr(&n)) as i32
}

extern "win64" fn get_full_path_name_a(nombre: *const u8, n: u32, buf: *mut u8, parte: *mut *mut u8) -> u32 {
    let ruta = ancha(nombre);
    let f: FullPathW = w("GetFullPathNameW");
    let Some(v) = pedir_w(|b, m| f(ptr(&ruta), m, b, core::ptr::null_mut())) else { return 0 };
    let b = estrecha(&v);
    let r = dar(&b, buf, n);
    if r as usize == b.len() && !parte.is_null() {
        let corte = b.iter().rposition(|&c| c == b'\\').map_or(0, |i| i + 1);
        // SAFETY: `buf` tiene la ruta entera; `*parte` apunta dentro.
        unsafe { *parte = if corte == b.len() { core::ptr::null_mut() } else { buf.add(corte) } };
    }
    r
}

extern "win64" fn get_temp_path_a(n: u32, buf: *mut u8) -> u32 {
    let f: LenBufW = w("GetTempPathW");
    match pedir_w(|b, m| f(m, b)) {
        Some(v) => dar(&estrecha(&v), buf, n),
        None => 0,
    }
}

fn directorio_a(nombre_w: &str, buf: *mut u8, n: u32) -> u32 {
    let f: BufLenW = w(nombre_w);
    match pedir_w(|b, m| f(b, m)) {
        Some(v) => dar(&estrecha(&v), buf, n),
        None => 0,
    }
}

extern "win64" fn get_system_directory_a(buf: *mut u8, n: u32) -> u32 {
    directorio_a("GetSystemDirectoryW", buf, n)
}

extern "win64" fn get_windows_directory_a(buf: *mut u8, n: u32) -> u32 {
    directorio_a("GetWindowsDirectoryW", buf, n)
}

type FinalPathW = extern "win64" fn(u64, *mut u16, u32, u32) -> u32;

extern "win64" fn get_final_path_name_by_handle_a(h: u64, buf: *mut u8, n: u32, banderas: u32) -> u32 {
    let f: FinalPathW = w("GetFinalPathNameByHandleW");
    match pedir_w(|b, m| f(h, b, m, banderas)) {
        Some(v) => dar(&estrecha(&v), buf, n),
        None => 0,
    }
}

// -- Buscar ficheros ---------------------------------------------------------------------------

type FindFirstExW = extern "win64" fn(*const u16, u32, *mut u8, u32, u64, u32) -> u64;
type FindNextW = extern "win64" fn(u64, *mut u8) -> i32;

/// WIN32_FIND_DATAW (592 bytes) a WIN32_FIND_DATAA (320): los 44 primeros
/// iguales; los nombres (260 y 14), a UTF-8, cortados si no caben.
fn find_data_a(de: &[u8; 592], a: *mut u8) {
    let nombre = |desde: usize, n: usize| -> Vec<u8> {
        let v: Vec<u16> = (0..n).map(|i| u16::from_le_bytes([de[desde + 2 * i], de[desde + 2 * i + 1]])).take_while(|&c| c != 0).collect();
        estrecha(&v)
    };
    let (largo, corto) = (nombre(44, 260), nombre(44 + 520, 14));
    // SAFETY: los 320 bytes del WIN32_FIND_DATAA del `.exe`.
    unsafe {
        core::ptr::write_bytes(a, 0, 320);
        core::ptr::copy_nonoverlapping(de.as_ptr(), a, 44);
        core::ptr::copy_nonoverlapping(largo.as_ptr(), a.add(44), largo.len().min(259));
        core::ptr::copy_nonoverlapping(corto.as_ptr(), a.add(44 + 260), corto.len().min(13));
    }
}

extern "win64" fn find_first_file_ex_a(nombre: *const u8, nivel: u32, datos: *mut u8, op: u32, filtro: u64, banderas: u32) -> u64 {
    let n = ancha(nombre);
    let f: FindFirstExW = w("FindFirstFileExW");
    let mut d = [0u8; 592];
    let h = f(ptr(&n), nivel, d.as_mut_ptr(), op, filtro, banderas);
    if h != u64::MAX && !datos.is_null() {
        find_data_a(&d, datos);
    }
    h
}

extern "win64" fn find_next_file_a(h: u64, datos: *mut u8) -> i32 {
    let f: FindNextW = w("FindNextFileW");
    let mut d = [0u8; 592];
    let r = f(h, d.as_mut_ptr());
    if r != 0 && !datos.is_null() {
        find_data_a(&d, datos);
    }
    r
}

// -- Modulos y proceso ---------------------------------------------------------------------------

type HandleExW = extern "win64" fn(u32, *const u16, *mut u64) -> i32;
type FileNameA = extern "win64" fn(u64, *mut u8, u32) -> u32;

const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 4;

extern "win64" fn get_module_handle_ex_a(banderas: u32, nombre: *const u8, h: *mut u64) -> i32 {
    let f: HandleExW = w("GetModuleHandleExW");
    if banderas & GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS != 0 {
        // `nombre` es una direccion, no un texto.
        return f(banderas, nombre as *const u16, h);
    }
    let n = ancha(nombre);
    f(banderas, ptr(&n), h)
}

/// La ruta del modulo `m` (0: el `.exe`), en UTF-8.
fn ruta_de_modulo(m: u64) -> Option<Vec<u8>> {
    let f: FileNameA = w("GetModuleFileNameA");
    let mut b = alloc::vec![0u8; 1024];
    let k = f(m, b.as_mut_ptr(), b.len() as u32) as usize;
    (k > 0 && k < b.len()).then(|| {
        b.truncate(k);
        b
    })
}

/// `K32GetModuleBaseNameA(proceso, modulo, bufer, n)`: el nombre sin la
/// ruta; se corta si no cabe. Los caracteres copiados.
extern "win64" fn k32_get_module_base_name_a(_proceso: u64, m: u64, buf: *mut u8, n: u32) -> u32 {
    let Some(r) = ruta_de_modulo(m) else { return 0 };
    let nombre = &r[r.iter().rposition(|&c| c == b'\\').map_or(0, |i| i + 1)..];
    if buf.is_null() || n == 0 {
        return 0;
    }
    let k = nombre.len().min(n as usize - 1);
    // SAFETY: `n` bytes del `.exe`.
    unsafe {
        core::ptr::copy_nonoverlapping(nombre.as_ptr(), buf, k);
        *buf.add(k) = 0;
    }
    k as u32
}

/// `QueryFullProcessImageNameA(proceso, banderas, bufer, *n)`: la ruta del
/// `.exe`; `*n` entra con la medida y sale con el largo.
extern "win64" fn query_full_process_image_name_a(_proceso: u64, _banderas: u32, buf: *mut u8, n: *mut u32) -> i32 {
    if n.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let Some(r) = ruta_de_modulo(0) else { return 0 };
    // SAFETY: el DWORD del `.exe`.
    let medida = unsafe { *n };
    let k = dar(&r, buf, medida);
    if k as usize != r.len() {
        kernel32::poner_error(122);
        return 0;
    }
    // SAFETY: como arriba.
    unsafe { *n = k };
    1
}

// -- Objetos -------------------------------------------------------------------------------------

type CreateEventW = extern "win64" fn(u64, i32, i32, *const u16) -> u64;
type CreateTimerW = extern "win64" fn(u64, i32, *const u16) -> u64;

const CREATE_EVENT_MANUAL_RESET: u32 = 1;
const CREATE_EVENT_INITIAL_SET: u32 = 2;

extern "win64" fn create_event_ex_w(attr: u64, nombre: *const u16, banderas: u32, _acceso: u32) -> u64 {
    let f: CreateEventW = w("CreateEventW");
    f(attr, (banderas & CREATE_EVENT_MANUAL_RESET != 0) as i32, (banderas & CREATE_EVENT_INITIAL_SET != 0) as i32, nombre)
}

extern "win64" fn create_event_ex_a(attr: u64, nombre: *const u8, banderas: u32, acceso: u32) -> u64 {
    let n = ancha(nombre);
    create_event_ex_w(attr, ptr(&n), banderas, acceso)
}

extern "win64" fn create_waitable_timer_a(attr: u64, manual: i32, nombre: *const u8) -> u64 {
    let n = ancha(nombre);
    let f: CreateTimerW = w("CreateWaitableTimerW");
    f(attr, manual, ptr(&n))
}

// -- Consola -------------------------------------------------------------------------------------

type WriteConsoleW = extern "win64" fn(u64, *const u16, u32, *mut u32, u64) -> i32;

/// `WriteConsoleA(h, bytes, n, *escritos, _)`: los bytes a UTF-16 y a la W;
/// `*escritos` en BYTES, como pidio el `.exe`.
extern "win64" fn write_console_a(h: u64, b: *const u8, n: u32, escritos: *mut u32, r: u64) -> i32 {
    // SAFETY: `n` bytes del `.exe`.
    let bytes = if n == 0 || b.is_null() { &[][..] } else { unsafe { core::slice::from_raw_parts(b, n as usize) } };
    let v = texto::a_ancho(bytes, false).unwrap_or_default();
    let f: WriteConsoleW = w("WriteConsoleW");
    let mut k = 0u32;
    let ok = f(h, v.as_ptr(), v.len() as u32, &mut k, r);
    if ok != 0 && !escritos.is_null() {
        // SAFETY: el DWORD del `.exe`.
        unsafe { *escritos = n };
    }
    ok
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "GetEnvironmentVariableA" => dir!(get_environment_variable_a),
        "SetEnvironmentVariableA" => dir!(set_environment_variable_a),
        "GetEnvironmentStrings" | "GetEnvironmentStringsA" => dir!(get_environment_strings_a),
        "FreeEnvironmentStringsA" => dir!(free_environment_strings_a),
        "ExpandEnvironmentStringsW" => dir!(expand_environment_strings_w),
        "ExpandEnvironmentStringsA" => dir!(expand_environment_strings_a),
        "GetFileAttributesA" => dir!(get_file_attributes_a),
        "CreateDirectoryA" => dir!(create_directory_a),
        "CreateDirectoryExW" => dir!(create_directory_ex_w),
        "DeleteFileA" => dir!(delete_file_a),
        "GetFullPathNameA" => dir!(get_full_path_name_a),
        "GetTempPathA" => dir!(get_temp_path_a),
        "GetSystemDirectoryA" => dir!(get_system_directory_a),
        "GetWindowsDirectoryA" => dir!(get_windows_directory_a),
        "GetFinalPathNameByHandleA" => dir!(get_final_path_name_by_handle_a),
        "FindFirstFileExA" => dir!(find_first_file_ex_a),
        "FindNextFileA" => dir!(find_next_file_a),
        "GetModuleHandleExA" => dir!(get_module_handle_ex_a),
        "K32GetModuleBaseNameA" => dir!(k32_get_module_base_name_a),
        "QueryFullProcessImageNameA" => dir!(query_full_process_image_name_a),
        "CreateEventExW" => dir!(create_event_ex_w),
        "CreateEventExA" => dir!(create_event_ex_a),
        "CreateWaitableTimerA" => dir!(create_waitable_timer_a),
        "WriteConsoleA" => dir!(write_console_a),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn dar_con_las_reglas_de_windows() {
        let mut b = [7u8; 4];
        assert_eq!(dar(b"abc", b.as_mut_ptr(), 4), 3);
        assert_eq!(&b, b"abc\0");
        let mut c = [7u8; 3];
        assert_eq!(dar(b"abc", c.as_mut_ptr(), 3), 4, "no cabe: lo que hace falta con el 0");
        assert_eq!(c, [7; 3], "y el bufer sin tocar");
    }

    #[test]
    fn expandir_deja_lo_que_no_conoce() {
        let s: Vec<u16> = "100%, %%, %NO_HAY_TAL%".encode_utf16().collect();
        // Sin entorno en la prueba: nada se sustituye y nada se pierde.
        assert_eq!(String::from_utf16_lossy(&expandir(&s)), "100%, %%, %NO_HAY_TAL%");
    }
}
