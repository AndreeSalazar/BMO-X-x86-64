//! **Los ficheros de Windows, de la casa** (P4d, 27-09).
//!
//! ```text
//!    CreateFileW/A        la ruta a la del volumen (bmo_proton_x::ficheros),
//!                         y el fichero ENTERO a memoria (Plataforma::leer_fichero)
//!    ReadFile             una copia desde la memoria
//!    WriteFile            en memoria; al cerrar, sale entero
//!                         (Plataforma::escribir_fichero)
//!    SetFilePointer(Ex), GetFileSize(Ex), GetFileType, FlushFileBuffers,
//!    GetFileAttributesW, CloseHandle
//! ```
//!
//! El directorio actual es el del `.exe` (lo dice quien carga). Lo que no
//! hay, dicho: compartir un fichero entre dos handles que escriben (cada uno
//! tiene su copia). En BMO-X un fichero escrito sale de UNA llamada
//! (`Archivo::escribir_de`, P4f3) y de la medida que sea; si no sale entero,
//! CloseHandle lo dice. Las carpetas: `carpetas.rs` (P4f3).

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::ficheros::{self, Abierto, NoRuta};

use crate::{aviso, dir, kernel32, plataforma};

/// Los handles de fichero: `FICHERO + indice`.
const FICHERO: u64 = 0x5F00_0000;
const NO_VALE: u64 = u64::MAX;

const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const CREATE_NEW: u32 = 1;
const CREATE_ALWAYS: u32 = 2;
const OPEN_EXISTING: u32 = 3;
const OPEN_ALWAYS: u32 = 4;
const TRUNCATE_EXISTING: u32 = 5;

const ERROR_INVALID_FUNCTION: u32 = 1;
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_ACCESS_DENIED: u32 = 5;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const ERROR_PATH_NOT_FOUND: u32 = 3;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_FILE_EXISTS: u32 = 80;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_NEGATIVE_SEEK: u32 = 131;
const ERROR_ALREADY_EXISTS: u32 = 183;
const ERROR_WRITE_FAULT: u32 = 29;
const INVALID_FILE_ATTRIBUTES: u32 = u32::MAX;
const FILE_TYPE_DISK: u32 = 1;

struct Estado {
    abiertos: Vec<Option<Abierto>>,
    dir: String,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { abiertos: Vec::new(), dir: String::new() }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.abiertos.clear();
    e.dir.clear();
}

/// **El directorio actual del `.exe`** (el suyo, `window` para `window/x.exe`).
/// Lo dice quien carga, antes de saltar.
pub fn poner_directorio(dir: &str) {
    estado().dir = String::from(dir.trim_matches('/'));
}

/// El directorio actual (ruta del volumen, `window`).
pub(crate) fn directorio() -> String {
    estado().dir.clone()
}

/// Un handle nuevo para `a`.
pub(crate) fn abrir(a: Abierto) -> u64 {
    let v = &mut estado().abiertos;
    v.push(Some(a));
    FICHERO + (v.len() - 1) as u64
}

/// Si `h` es un fichero de la casa.
pub(crate) fn es_fichero(h: u64) -> bool {
    abierto(h).is_some()
}

pub(crate) fn abierto(h: u64) -> Option<&'static mut Abierto> {
    let i = h.checked_sub(FICHERO)? as usize;
    estado().abiertos.get_mut(i)?.as_mut()
}

pub(crate) fn ruta_de(nombre: *const u16) -> Result<String, u32> {
    if nombre.is_null() {
        return Err(ERROR_INVALID_PARAMETER);
    }
    let mut n = 0;
    // SAFETY: una cadena UTF-16 del `.exe`, acabada en 0 (como pide Windows).
    while n < 1024 && unsafe { nombre.add(n).read() } != 0 {
        n += 1;
    }
    // SAFETY: los `n` caracteres de arriba.
    let w = unsafe { core::slice::from_raw_parts(nombre, n) };
    ficheros::ruta(w, &estado().dir).map_err(|e| match e {
        NoRuta::FueraDelVolumen | NoRuta::NoAscii => ERROR_PATH_NOT_FOUND,
        NoRuta::NoEsFichero => ERROR_FILE_NOT_FOUND,
    })
}

/// `CreateFileW(nombre, acceso, compartir, seguridad, disposicion, banderas, plantilla)`.
extern "win64" fn create_file_w(nombre: *const u16, acceso: u32, _compartir: u32, _seg: u64, disposicion: u32, banderas: u32, _plantilla: u64) -> u64 {
    let ruta = match ruta_de(nombre) {
        Ok(r) => r,
        // P4f3: la raiz del volumen es una carpeta ("C:\\", "\\").
        Err(ERROR_FILE_NOT_FOUND) if crate::carpetas::es_raiz(nombre) => String::new(),
        Err(e) => {
            kernel32::poner_error(e);
            return NO_VALE;
        }
    };
    // P4f3: una CARPETA se abre solo con FILE_FLAG_BACKUP_SEMANTICS, como en
    // Windows (la `std` de Rust lo hace para `metadata`); si no, acceso denegado.
    if ruta.is_empty() || crate::carpetas::entrada(&ruta).is_some_and(|e| e.carpeta) {
        if banderas & FILE_FLAG_BACKUP_SEMANTICS == 0 || !matches!(disposicion, OPEN_EXISTING | OPEN_ALWAYS) {
            kernel32::poner_error(ERROR_ACCESS_DENIED);
            return NO_VALE;
        }
        kernel32::poner_error(0);
        return abrir(Abierto { ruta, lee: true, carpeta: true, ..Abierto::default() });
    }
    // Una carpeta de por medio que no esta: ERROR_PATH_NOT_FOUND, no el 2.
    if !crate::carpetas::padre_existe(&ruta) {
        kernel32::poner_error(ERROR_PATH_NOT_FOUND);
        return NO_VALE;
    }
    let habia = (plataforma().leer_fichero)(ruta.as_bytes());
    let existe = habia.is_some();
    let bytes = match (disposicion, habia) {
        (CREATE_NEW, Some(_)) => {
            kernel32::poner_error(ERROR_FILE_EXISTS);
            return NO_VALE;
        }
        (OPEN_EXISTING | TRUNCATE_EXISTING, None) => {
            kernel32::poner_error(ERROR_FILE_NOT_FOUND);
            return NO_VALE;
        }
        (CREATE_ALWAYS | TRUNCATE_EXISTING | CREATE_NEW, _) => Vec::new(),
        (OPEN_EXISTING | OPEN_ALWAYS, b) => b.unwrap_or_default(),
        _ => {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            return NO_VALE;
        }
    };
    let escribe = acceso & GENERIC_WRITE != 0;
    // Crear (o vaciar) es escribir, aunque no se escriba nada despues.
    let sucio = escribe && matches!(disposicion, CREATE_ALWAYS | CREATE_NEW | TRUNCATE_EXISTING) || (disposicion == OPEN_ALWAYS && !existe);
    let a = Abierto { ruta, bytes, pos: 0, lee: acceso & GENERIC_READ != 0, escribe, sucio, carpeta: false };
    // Como Windows: CREATE_ALWAYS y OPEN_ALWAYS sobre uno que ya estaba lo dicen.
    kernel32::poner_error(if existe && matches!(disposicion, CREATE_ALWAYS | OPEN_ALWAYS) { ERROR_ALREADY_EXISTS } else { 0 });
    abrir(a)
}

/// `CreateFileA`: la misma, con la ruta en ASCII.
extern "win64" fn create_file_a(nombre: *const u8, acceso: u32, compartir: u32, seg: u64, disposicion: u32, banderas: u32, plantilla: u64) -> u64 {
    if nombre.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return NO_VALE;
    }
    let mut w: Vec<u16> = Vec::new();
    // SAFETY: una cadena del `.exe`, acabada en 0.
    while w.len() < 1024 && unsafe { nombre.add(w.len()).read() } != 0 {
        // SAFETY: como arriba.
        w.push(unsafe { nombre.add(w.len()).read() } as u16);
    }
    w.push(0);
    create_file_w(w.as_ptr(), acceso, compartir, seg, disposicion, banderas, plantilla)
}

/// P4f4: el desplazamiento de un OVERLAPPED (Offset en +16, OffsetHigh en
/// +20). En un handle SINCRONO, Windows lee o escribe ahi y deja el cursor
/// detras.
fn desde_solapado(ov: u64) -> Option<u64> {
    // SAFETY: un OVERLAPPED del `.exe` (32 bytes).
    (ov != 0).then(|| unsafe { ((ov + 16) as *const u32).read_unaligned() as u64 | (((ov + 20) as *const u32).read_unaligned() as u64) << 32 })
}

/// Lo que queda en el OVERLAPPED al acabar: Internal (el NTSTATUS, +0) e
/// InternalHigh (los bytes, +8). GetOverlappedResult lo lee de ahi.
fn cumplir_solapado(ov: u64, status: u32, n: usize) {
    if ov != 0 {
        // SAFETY: como arriba.
        unsafe {
            (ov as *mut u64).write_unaligned(status as u64);
            ((ov + 8) as *mut u64).write_unaligned(n as u64);
        }
    }
}

/// **Leer de un fichero de la casa** (ReadFile y NtReadFile): desde `desde`
/// o desde el cursor. Los bytes, o el error de Win32.
pub(crate) fn leer_de(h: u64, dst: &mut [u8], desde: Option<u64>) -> Result<usize, u32> {
    let a = abierto(h).filter(|a| a.lee).ok_or(ERROR_INVALID_HANDLE)?;
    if a.carpeta {
        return Err(ERROR_INVALID_FUNCTION);
    }
    if let Some(p) = desde {
        a.pos = p;
    }
    Ok(a.leer(dst))
}

/// **Escribir en un fichero de la casa** (WriteFile y NtWriteFile).
pub(crate) fn escribir_en(h: u64, src: &[u8], desde: Option<u64>) -> Result<usize, u32> {
    let a = abierto(h).filter(|a| a.escribe).ok_or(ERROR_INVALID_HANDLE)?;
    if let Some(p) = desde {
        a.pos = p;
    }
    Ok(a.escribir(src))
}

/// `ReadFile(h, bufer, n, *leidos, solapado)`.
extern "win64" fn read_file(h: u64, b: *mut u8, n: u32, leidos: *mut u32, solapado: u64) -> i32 {
    // SAFETY: `n` bytes del `.exe` donde escribir.
    let dst = if n == 0 { &mut [][..] } else { unsafe { core::slice::from_raw_parts_mut(b, n as usize) } };
    match leer_de(h, dst, desde_solapado(solapado)) {
        Ok(k) => {
            if !leidos.is_null() {
                // SAFETY: un DWORD del `.exe`.
                unsafe { *leidos = k as u32 };
            }
            cumplir_solapado(solapado, 0, k);
            1
        }
        Err(e) => {
            kernel32::poner_error(e);
            0
        }
    }
}

/// `WriteFile` sobre un fichero (la consola la lleva `kernel32`).
pub(crate) fn escribir(h: u64, b: *const u8, n: u32, escritos: *mut u32, solapado: u64) -> i32 {
    // SAFETY: `n` bytes del `.exe`.
    let src = if n == 0 { &[][..] } else { unsafe { core::slice::from_raw_parts(b, n as usize) } };
    match escribir_en(h, src, desde_solapado(solapado)) {
        Ok(k) => {
            if !escritos.is_null() {
                // SAFETY: un DWORD del `.exe`.
                unsafe { *escritos = k as u32 };
            }
            cumplir_solapado(solapado, 0, k);
            1
        }
        Err(e) => {
            kernel32::poner_error(e);
            0
        }
    }
}

/// Sacar un fichero escrito entero. `false` si la plataforma no pudo.
fn volcar(a: &mut Abierto) -> bool {
    if !a.sucio {
        return true;
    }
    a.sucio = false;
    (plataforma().escribir_fichero)(a.ruta.as_bytes(), &a.bytes)
}

/// `CloseHandle` sobre un fichero: si se escribio, sale entero.
pub(crate) fn cerrar(h: u64) -> i32 {
    let Some(i) = h.checked_sub(FICHERO).map(|i| i as usize) else { return 0 };
    let Some(mut a) = estado().abiertos.get_mut(i).and_then(Option::take) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    if !volcar(&mut a) {
        aviso("CloseHandle: el fichero escrito no salio entero al volumen");
        kernel32::poner_error(ERROR_WRITE_FAULT);
        return 0;
    }
    1
}

extern "win64" fn flush_file_buffers(h: u64) -> i32 {
    match abierto(h) {
        Some(a) => volcar(a) as i32,
        None => 0,
    }
}

extern "win64" fn set_file_pointer_ex(h: u64, dist: i64, nueva: *mut i64, metodo: u32) -> i32 {
    let Some(a) = abierto(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    match a.mover(dist, metodo) {
        Some(p) => {
            if !nueva.is_null() {
                // SAFETY: un LARGE_INTEGER del `.exe`.
                unsafe { *nueva = p as i64 };
            }
            1
        }
        None => {
            kernel32::poner_error(if metodo > 2 { ERROR_INVALID_PARAMETER } else { ERROR_NEGATIVE_SEEK });
            0
        }
    }
}

/// `SetFilePointer(h, bajo, *alto, metodo)`: la de 32 bits, con la mitad
/// alta por puntero (si lo dan).
extern "win64" fn set_file_pointer(h: u64, bajo: i32, alto: *mut i32, metodo: u32) -> u32 {
    // SAFETY: un LONG del `.exe`, o nulo.
    let dist = if alto.is_null() { bajo as i64 } else { (unsafe { *alto } as i64) << 32 | bajo as u32 as i64 };
    let mut p = 0i64;
    if set_file_pointer_ex(h, dist, &mut p, metodo) == 0 {
        return u32::MAX;
    }
    if !alto.is_null() {
        // SAFETY: como arriba.
        unsafe { *alto = (p >> 32) as i32 };
    }
    kernel32::poner_error(0);
    p as u32
}

extern "win64" fn get_file_size_ex(h: u64, medida: *mut i64) -> i32 {
    let Some(a) = abierto(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    if !medida.is_null() {
        // SAFETY: un LARGE_INTEGER del `.exe`.
        unsafe { *medida = a.bytes.len() as i64 };
    }
    1
}

extern "win64" fn get_file_size(h: u64, alto: *mut u32) -> u32 {
    let Some(a) = abierto(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return u32::MAX;
    };
    let n = a.bytes.len() as u64;
    if !alto.is_null() {
        // SAFETY: un DWORD del `.exe`.
        unsafe { *alto = (n >> 32) as u32 };
    }
    n as u32
}

extern "win64" fn get_file_type(h: u64) -> u32 {
    if es_fichero(h) {
        FILE_TYPE_DISK
    } else {
        2 // FILE_TYPE_CHAR: la consola
    }
}

extern "win64" fn get_file_attributes_w(nombre: *const u16) -> u32 {
    // P4f3: por la lista de su carpeta (sin leer el fichero entero), y las
    // carpetas con FILE_ATTRIBUTE_DIRECTORY.
    match ruta_de(nombre) {
        Ok(r) => match crate::carpetas::entrada(&r) {
            Some(e) => crate::carpetas::atributos(&e),
            None => {
                kernel32::poner_error(if crate::carpetas::padre_existe(&r) { ERROR_FILE_NOT_FOUND } else { ERROR_PATH_NOT_FOUND });
                INVALID_FILE_ATTRIBUTES
            }
        },
        Err(ERROR_FILE_NOT_FOUND) if crate::carpetas::es_raiz(nombre) => crate::carpetas::DIRECTORIO,
        Err(e) => {
            kernel32::poner_error(e);
            INVALID_FILE_ATTRIBUTES
        }
    }
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "CreateFileW" => dir!(create_file_w),
        "CreateFileA" => dir!(create_file_a),
        "ReadFile" => dir!(read_file),
        "SetFilePointerEx" => dir!(set_file_pointer_ex),
        "SetFilePointer" => dir!(set_file_pointer),
        "GetFileSizeEx" => dir!(get_file_size_ex),
        "GetFileSize" => dir!(get_file_size),
        "GetFileType" => dir!(get_file_type),
        "FlushFileBuffers" => dir!(flush_file_buffers),
        "GetFileAttributesW" => dir!(get_file_attributes_w),
        _ => return None,
    })
}
