//! **Las carpetas y lo que se pregunta de un fichero, de la casa** (P4f3,
//! 27-09).
//!
//! ```text
//!    FindFirstFileW/ExW, FindNextFileW, FindClose   Plataforma::listar y los
//!                                                   comodines de Windows
//!    GetFileAttributesExW, GetFullPathNameW, Get/SetCurrentDirectoryW,
//!    GetTempPathW, GetLogicalDrives
//!    GetFileInformationByHandle(Ex), SetFileInformationByHandle (la medida),
//!    SetEndOfFile, GetFinalPathNameByHandleW, LockFile(Ex)/UnlockFile(Ex)
//!    CopyFileW/ExW                                  leer entero y crear
//!    SetFileAttributesW, SetFileTime
//!    CreateDirectoryW, RemoveDirectoryW, DeleteFileW, MoveFileExW  --> NO
//! ```
//!
//! **Lo que el FAT32 de BMO-X da a Ring 3 hoy**: leer, crear o reemplazar un
//! fichero entero, y listar una carpeta. NO borra, NO crea carpetas y NO
//! renombra: eso es codigo del kernel que no existe. Asi que esas cuatro
//! contestan lo que Windows diria si no hay (ERROR_FILE_NOT_FOUND,
//! ERROR_PATH_NOT_FOUND, ERROR_ALREADY_EXISTS) y, si HAY, ERROR_ACCESS_DENIED
//! y lo dicen por la consola: es la lista de lo que el kernel tiene que
//! aprender.
//!
//! Lo que no es Windows, dicho: las fechas de los ficheros van a 0 (el
//! listado de BMO-X no las da), los atributos son NORMAL o DIRECTORY, un
//! cerrojo de fichero siempre se da (hay un solo proceso que pueda pedirlo),
//! SetFileTime dice que no, y la rutina de progreso de CopyFileExW no se llama.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::ficheros::{self, Entrada};
use bmo_proton_x::proceso;

use crate::ficheros::{abierto, directorio, ruta_de};
use crate::{aviso, dir, kernel32, plataforma};

pub(crate) const DIRECTORIO: u32 = 0x10;
const NORMAL: u32 = 0x80;
const ARCHIVO: u32 = 0x20;

const NO_VALE: u64 = u64::MAX;
/// Los handles de busqueda: `BUSQUEDA + indice`.
const BUSQUEDA: u64 = 0x5F80_0000;

// Privados, como en ficheros.rs: el guardian del contrato (R23, L6j) no deja
// que un numero publico signifique dos cosas en la casa y en el kernel.
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_PATH_NOT_FOUND: u32 = 3;
const ERROR_ACCESS_DENIED: u32 = 5;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_NO_MORE_FILES: u32 = 18;
const ERROR_NOT_SUPPORTED: u32 = 50;
const ERROR_FILE_EXISTS: u32 = 80;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_ALREADY_EXISTS: u32 = 183;
const ERROR_DIRECTORY: u32 = 267;

const COPY_FILE_FAIL_IF_EXISTS: u32 = 1;

struct Estado {
    /// Cada busqueda abierta: lo que encontro y por donde va.
    busquedas: Vec<Option<(Vec<Entrada>, usize)>>,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { busquedas: Vec::new() }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    estado().busquedas.clear();
}

// -- Lo que se sabe de una ruta ----------------------------------------------------------

/// Una cadena UTF-16 del `.exe`, sin su 0.
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

/// Si `p` nombra la raiz del volumen (`C:\\`, `\\`, `..` desde `window`).
pub(crate) fn es_raiz(p: *const u16) -> bool {
    ficheros::ruta_o_raiz(&ancho(p), &directorio()).is_ok_and(|r| r.is_empty())
}

/// Carpeta y nombre de una ruta del volumen.
fn partir(ruta: &str) -> (&str, &str) {
    ruta.rsplit_once('/').unwrap_or(("", ruta))
}

/// **Lo que hay en `ruta`**, por la lista de su carpeta (sin leer nada).
pub(crate) fn entrada(ruta: &str) -> Option<Entrada> {
    if ruta.is_empty() {
        return Some(Entrada { nombre: String::new(), carpeta: true, bytes: 0 });
    }
    let (padre, nombre) = partir(ruta);
    (plataforma().listar)(padre.as_bytes())?.into_iter().find(|e| e.nombre.eq_ignore_ascii_case(nombre))
}

/// Si la carpeta de `ruta` existe (para distinguir el 2 del 3).
pub(crate) fn padre_existe(ruta: &str) -> bool {
    let (padre, _) = partir(ruta);
    padre.is_empty() || entrada(padre).is_some_and(|e| e.carpeta)
}

pub(crate) fn atributos(e: &Entrada) -> u32 {
    if e.carpeta {
        DIRECTORIO
    } else {
        NORMAL
    }
}

/// La ruta de Windows de una ruta del volumen: `C:\window\x.txt`.
fn de_windows(ruta: &str) -> Vec<u16> {
    proceso::ruta_windows(ruta).encode_utf16().collect()
}

/// Dar `s` como GetCurrentDirectoryW / GetFullPathNameW / GetTempPathW: cabe,
/// su largo sin el 0; no cabe, lo que hace falta CON el 0 y nada escrito.
fn dar(s: &[u16], buf: *mut u16, n: u32) -> u32 {
    if (n as usize) <= s.len() || buf.is_null() {
        return s.len() as u32 + 1;
    }
    // SAFETY: el `.exe` da `n` > s.len() caracteres.
    unsafe {
        core::ptr::copy_nonoverlapping(s.as_ptr(), buf, s.len());
        *buf.add(s.len()) = 0;
    }
    s.len() as u32
}

/// La ruta del volumen de un nombre del `.exe`, o el error de Windows.
fn ruta(p: *const u16) -> Result<String, u32> {
    match ruta_de(p) {
        Err(ERROR_FILE_NOT_FOUND) if es_raiz(p) => Ok(String::new()),
        r => r,
    }
}

// -- Buscar --------------------------------------------------------------------------------

/// `WIN32_FIND_DATAW` (592 bytes): atributos, tres fechas, la medida en dos
/// mitades y el nombre (260 caracteres).
fn poner_hallazgo(e: &Entrada, d: *mut u8) {
    let mut b = [0u8; 592];
    b[0..4].copy_from_slice(&atributos(e).to_le_bytes());
    b[28..32].copy_from_slice(&((e.bytes >> 32) as u32).to_le_bytes());
    b[32..36].copy_from_slice(&(e.bytes as u32).to_le_bytes());
    for (i, c) in e.nombre.encode_utf16().take(259).enumerate() {
        b[44 + 2 * i..46 + 2 * i].copy_from_slice(&c.to_le_bytes());
    }
    // SAFETY: el `.exe` da un WIN32_FIND_DATAW.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), d, 592) };
}

extern "win64" fn find_first_file_w(nombre: *const u16, datos: *mut u8) -> u64 {
    let w = ancho(nombre);
    let (carpeta, patron) = match ficheros::partir_patron(&w, &directorio()) {
        Ok(x) => x,
        Err(_) => {
            kernel32::poner_error(ERROR_FILE_NOT_FOUND);
            return NO_VALE;
        }
    };
    if !entrada(&carpeta).is_some_and(|e| e.carpeta) {
        kernel32::poner_error(ERROR_PATH_NOT_FOUND);
        return NO_VALE;
    }
    let mut todas = (plataforma().listar)(carpeta.as_bytes()).unwrap_or_default();
    todas.retain(|e| e.nombre != "." && e.nombre != "..");
    // Fuera de la raiz, Windows da "." y ".." primero.
    if !carpeta.is_empty() {
        for n in ["..", "."] {
            todas.insert(0, Entrada { nombre: String::from(n), carpeta: true, bytes: 0 });
        }
    }
    let halladas: Vec<Entrada> = todas.into_iter().filter(|e| ficheros::comodin(&patron, &e.nombre)).collect();
    let Some(primera) = halladas.first() else {
        kernel32::poner_error(ERROR_FILE_NOT_FOUND);
        return NO_VALE;
    };
    poner_hallazgo(primera, datos);
    let v = &mut estado().busquedas;
    v.push(Some((halladas, 1)));
    BUSQUEDA + (v.len() - 1) as u64
}

extern "win64" fn find_first_file_ex_w(nombre: *const u16, _nivel: u32, datos: *mut u8, _op: u32, _filtro: u64, _banderas: u32) -> u64 {
    find_first_file_w(nombre, datos)
}

fn busqueda(h: u64) -> Option<&'static mut Option<(Vec<Entrada>, usize)>> {
    let i = h.checked_sub(BUSQUEDA)? as usize;
    estado().busquedas.get_mut(i).filter(|b| b.is_some())
}

extern "win64" fn find_next_file_w(h: u64, datos: *mut u8) -> i32 {
    let Some(Some((v, i))) = busqueda(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    match v.get(*i) {
        Some(e) => {
            poner_hallazgo(e, datos);
            *i += 1;
            1
        }
        None => {
            kernel32::poner_error(ERROR_NO_MORE_FILES);
            0
        }
    }
}

extern "win64" fn find_close(h: u64) -> i32 {
    match busqueda(h) {
        Some(b) => {
            *b = None;
            1
        }
        None => {
            kernel32::poner_error(ERROR_INVALID_HANDLE);
            0
        }
    }
}

// -- Rutas y directorio actual ---------------------------------------------------------------

extern "win64" fn get_file_attributes_ex_w(nombre: *const u16, nivel: u32, datos: *mut u8) -> i32 {
    if nivel != 0 || datos.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let r = match ruta(nombre) {
        Ok(r) => r,
        Err(e) => {
            kernel32::poner_error(e);
            return 0;
        }
    };
    let Some(e) = entrada(&r) else {
        kernel32::poner_error(if padre_existe(&r) { ERROR_FILE_NOT_FOUND } else { ERROR_PATH_NOT_FOUND });
        return 0;
    };
    // WIN32_FILE_ATTRIBUTE_DATA (36 bytes): atributos, tres fechas, la medida.
    let mut b = [0u8; 36];
    b[0..4].copy_from_slice(&atributos(&e).to_le_bytes());
    b[28..32].copy_from_slice(&((e.bytes >> 32) as u32).to_le_bytes());
    b[32..36].copy_from_slice(&(e.bytes as u32).to_le_bytes());
    // SAFETY: el `.exe` da un WIN32_FILE_ATTRIBUTE_DATA.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), datos, 36) };
    1
}

/// `GetFullPathNameW(nombre, n, bufer, *parte)`: la ruta ENTERA, sin mirar
/// si existe (como Windows), y `*parte` en el ultimo trozo.
extern "win64" fn get_full_path_name_w(nombre: *const u16, n: u32, buf: *mut u16, parte: *mut *mut u16) -> u32 {
    let r = match ruta(nombre) {
        Ok(r) => r,
        Err(e) => {
            kernel32::poner_error(e);
            return 0;
        }
    };
    let w = de_windows(&r);
    let k = dar(&w, buf, n);
    if k as usize == w.len() && !parte.is_null() {
        let corte = w.iter().rposition(|&c| c == b'\\' as u16).map_or(0, |i| i + 1);
        // SAFETY: `buf` tiene la ruta entera; `*parte` apunta dentro (o NULL
        // si acaba en `\`, como Windows).
        unsafe { *parte = if corte == w.len() { core::ptr::null_mut() } else { buf.add(corte) } };
    }
    k
}

/// `C:\window`, sin la barra del final (salvo la raiz, `C:\`).
fn actual() -> Vec<u16> {
    de_windows(&directorio())
}

extern "win64" fn get_current_directory_w(n: u32, buf: *mut u16) -> u32 {
    dar(&actual(), buf, n)
}

extern "win64" fn set_current_directory_w(nombre: *const u16) -> i32 {
    let r = match ruta(nombre) {
        Ok(r) => r,
        Err(e) => {
            kernel32::poner_error(e);
            return 0;
        }
    };
    match entrada(&r) {
        Some(e) if e.carpeta => {
            crate::ficheros::poner_directorio(&r);
            1
        }
        Some(_) => {
            kernel32::poner_error(ERROR_DIRECTORY);
            0
        }
        None => {
            kernel32::poner_error(ERROR_FILE_NOT_FOUND);
            0
        }
    }
}

/// `GetTempPathW`: TMP del entorno, con su barra al final (como Windows).
extern "win64" fn get_temp_path_w(n: u32, buf: *mut u16) -> u32 {
    let mut t = crate::proceso::variable("TMP").unwrap_or_else(|| "C:\\".encode_utf16().collect());
    if t.last() != Some(&(b'\\' as u16)) {
        t.push(b'\\' as u16);
    }
    dar(&t, buf, n)
}

extern "win64" fn get_logical_drives() -> u32 {
    // Solo C: (el volumen de datos de BMO-X).
    1 << 2
}

// -- Preguntas a un handle ---------------------------------------------------------------------

/// Un numero que no cambia para la misma ruta (el "indice" del fichero).
fn indice(r: &str) -> u64 {
    r.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b.to_ascii_lowercase() as u64).wrapping_mul(0x100_0000_01b3))
}

extern "win64" fn get_file_information_by_handle(h: u64, info: *mut u8) -> i32 {
    let Some(a) = abierto(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    // BY_HANDLE_FILE_INFORMATION (52 bytes).
    let mut b = [0u8; 52];
    let atr = if a.carpeta { DIRECTORIO } else { NORMAL };
    let n = a.bytes.len() as u64;
    let i = indice(&a.ruta);
    b[0..4].copy_from_slice(&atr.to_le_bytes());
    b[28..32].copy_from_slice(&0xB0B0_0001u32.to_le_bytes());
    b[32..36].copy_from_slice(&((n >> 32) as u32).to_le_bytes());
    b[36..40].copy_from_slice(&(n as u32).to_le_bytes());
    b[40..44].copy_from_slice(&1u32.to_le_bytes());
    b[44..48].copy_from_slice(&((i >> 32) as u32).to_le_bytes());
    b[48..52].copy_from_slice(&(i as u32).to_le_bytes());
    // SAFETY: el `.exe` da un BY_HANDLE_FILE_INFORMATION.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), info, 52) };
    1
}

/// Las clases de `GetFileInformationByHandleEx` que se saben: FileBasicInfo
/// (0), FileStandardInfo (1) y FileAttributeTagInfo (9).
extern "win64" fn get_file_information_by_handle_ex(h: u64, clase: u32, buf: *mut u8, n: u32) -> i32 {
    let Some(a) = abierto(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    let atr = if a.carpeta { DIRECTORIO } else { NORMAL };
    let fin = a.bytes.len() as u64;
    let mut b = [0u8; 40];
    let medida = match clase {
        0 => {
            b[32..36].copy_from_slice(&atr.to_le_bytes());
            40
        }
        1 => {
            b[0..8].copy_from_slice(&((fin + 4095) & !4095).to_le_bytes());
            b[8..16].copy_from_slice(&fin.to_le_bytes());
            b[16..20].copy_from_slice(&1u32.to_le_bytes());
            b[21] = a.carpeta as u8;
            24
        }
        9 => {
            b[0..4].copy_from_slice(&atr.to_le_bytes());
            8
        }
        _ => {
            aviso(&alloc::format!("GetFileInformationByHandleEx: la clase {clase} no la sabe la casa"));
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            return 0;
        }
    };
    if (n as usize) < medida {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    // SAFETY: el `.exe` da `n` >= medida bytes.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), buf, medida) };
    1
}

/// Cambiar la medida de un fichero abierto para escribir.
fn nueva_medida(h: u64, m: u64) -> i32 {
    match abierto(h) {
        Some(a) if a.escribe && !a.carpeta => {
            a.bytes.resize(m as usize, 0);
            a.sucio = true;
            1
        }
        Some(_) => {
            kernel32::poner_error(ERROR_ACCESS_DENIED);
            0
        }
        None => {
            kernel32::poner_error(ERROR_INVALID_HANDLE);
            0
        }
    }
}

/// `SetFileInformationByHandle`: la medida (FileEndOfFileInfo, 6) si; la
/// reserva (FileAllocationInfo, 5) es una pista y se acepta; borrar (4, 21)
/// y renombrar (3) no hay en el FAT32 de BMO-X.
extern "win64" fn set_file_information_by_handle(h: u64, clase: u32, buf: *const u8, n: u32) -> i32 {
    if abierto(h).is_none() {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    }
    match clase {
        6 if n >= 8 => {
            // SAFETY: un FILE_END_OF_FILE_INFO del `.exe`.
            let m = unsafe { (buf as *const i64).read_unaligned() };
            if m < 0 {
                kernel32::poner_error(ERROR_INVALID_PARAMETER);
                return 0;
            }
            nueva_medida(h, m as u64)
        }
        5 => 1,
        _ => {
            aviso(&alloc::format!("SetFileInformationByHandle clase {clase}: el FAT32 de BMO-X no borra ni renombra desde Ring 3"));
            kernel32::poner_error(ERROR_NOT_SUPPORTED);
            0
        }
    }
}

extern "win64" fn set_end_of_file(h: u64) -> i32 {
    let Some(p) = abierto(h).map(|a| a.pos) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    nueva_medida(h, p)
}

/// `GetFinalPathNameByHandleW`: `\\?\C:\window\x.txt`.
extern "win64" fn get_final_path_name_by_handle_w(h: u64, buf: *mut u16, n: u32, banderas: u32) -> u32 {
    let Some(a) = abierto(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    // VOLUME_NAME_DOS (0) es lo unico que tiene sentido aqui.
    if banderas & 0x7 != 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let mut w: Vec<u16> = "\\\\?\\".encode_utf16().collect();
    w.extend(de_windows(&a.ruta));
    dar(&w, buf, n)
}

/// Un cerrojo de fichero: hay UN proceso que pueda pedirlo, asi que siempre
/// se da.
extern "win64" fn lock_file_ex(h: u64, _banderas: u32, _r: u32, _bajo: u32, _alto: u32, ov: u64) -> i32 {
    if abierto(h).is_none() || ov == 0 {
        kernel32::poner_error(if ov == 0 { ERROR_INVALID_PARAMETER } else { ERROR_INVALID_HANDLE });
        return 0;
    }
    1
}

extern "win64" fn unlock_file_ex(h: u64, _r: u32, _bajo: u32, _alto: u32, ov: u64) -> i32 {
    lock_file_ex(h, 0, 0, 0, 0, ov)
}

extern "win64" fn lock_file(h: u64, _a: u32, _b: u32, _c: u32, _d: u32) -> i32 {
    lock_file_ex(h, 0, 0, 0, 0, 1)
}

// -- Lo que cambia el volumen -----------------------------------------------------------------

/// `CopyFileExW(origen, destino, progreso, dato, *cancelar, banderas)`.
extern "win64" fn copy_file_ex_w(origen: *const u16, destino: *const u16, _progreso: u64, _dato: u64, _cancelar: u64, banderas: u32) -> i32 {
    let (a, b) = match (ruta(origen), ruta(destino)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            kernel32::poner_error(e);
            return 0;
        }
    };
    let Some(bytes) = (plataforma().leer_fichero)(a.as_bytes()) else {
        kernel32::poner_error(if padre_existe(&a) { ERROR_FILE_NOT_FOUND } else { ERROR_PATH_NOT_FOUND });
        return 0;
    };
    match entrada(&b) {
        Some(e) if e.carpeta => {
            kernel32::poner_error(ERROR_ACCESS_DENIED);
            return 0;
        }
        Some(_) if banderas & COPY_FILE_FAIL_IF_EXISTS != 0 => {
            kernel32::poner_error(ERROR_FILE_EXISTS);
            return 0;
        }
        None if !padre_existe(&b) => {
            kernel32::poner_error(ERROR_PATH_NOT_FOUND);
            return 0;
        }
        _ => {}
    }
    if !(plataforma().escribir_fichero)(b.as_bytes(), &bytes) {
        aviso("CopyFileExW: el destino no salio entero al volumen");
        kernel32::poner_error(ERROR_ACCESS_DENIED);
        return 0;
    }
    1
}

extern "win64" fn copy_file_w(origen: *const u16, destino: *const u16, no_pisar: i32) -> i32 {
    copy_file_ex_w(origen, destino, 0, 0, 0, if no_pisar != 0 { COPY_FILE_FAIL_IF_EXISTS } else { 0 })
}

/// Lo que el FAT32 de BMO-X no sabe hacer desde Ring 3: si lo nombrado no
/// esta, el error de Windows de siempre; si esta, ACCESO DENEGADO y dicho.
fn no_se_puede(que: &str, r: &str, si_esta: u32) -> i32 {
    match entrada(r) {
        Some(_) => {
            if si_esta == ERROR_ALREADY_EXISTS {
                kernel32::poner_error(si_esta);
            } else {
                aviso(&alloc::format!("{que}: el FAT32 de BMO-X no lo sabe hacer desde Ring 3 todavia"));
                kernel32::poner_error(ERROR_ACCESS_DENIED);
            }
        }
        None if !padre_existe(r) => kernel32::poner_error(ERROR_PATH_NOT_FOUND),
        None if si_esta == ERROR_ALREADY_EXISTS => {
            aviso(&alloc::format!("{que}: el FAT32 de BMO-X no crea carpetas desde Ring 3 todavia"));
            kernel32::poner_error(ERROR_ACCESS_DENIED);
        }
        None => kernel32::poner_error(ERROR_FILE_NOT_FOUND),
    }
    0
}

fn con_ruta(p: *const u16, f: impl FnOnce(&str) -> i32) -> i32 {
    match ruta(p) {
        Ok(r) => f(&r),
        Err(e) => {
            kernel32::poner_error(e);
            0
        }
    }
}

extern "win64" fn create_directory_w(n: *const u16, _seg: u64) -> i32 {
    con_ruta(n, |r| no_se_puede("CreateDirectoryW", r, ERROR_ALREADY_EXISTS))
}

extern "win64" fn remove_directory_w(n: *const u16) -> i32 {
    con_ruta(n, |r| no_se_puede("RemoveDirectoryW", r, ERROR_ACCESS_DENIED))
}

extern "win64" fn delete_file_w(n: *const u16) -> i32 {
    con_ruta(n, |r| no_se_puede("DeleteFileW", r, ERROR_ACCESS_DENIED))
}

extern "win64" fn move_file_ex_w(origen: *const u16, _destino: *const u16, _banderas: u32) -> i32 {
    con_ruta(origen, |r| no_se_puede("MoveFileExW", r, ERROR_ACCESS_DENIED))
}

extern "win64" fn move_file_w(origen: *const u16, destino: *const u16) -> i32 {
    move_file_ex_w(origen, destino, 0)
}

/// `SetFileAttributesW`: dejarlo NORMAL (o ARCHIVE) es lo que ya es; otra
/// cosa (oculto, solo lectura) no se escribe en el FAT32 desde Ring 3.
extern "win64" fn set_file_attributes_w(n: *const u16, atr: u32) -> i32 {
    con_ruta(n, |r| match entrada(r) {
        Some(e) if !e.carpeta && matches!(atr, 0 | NORMAL | ARCHIVO) => 1,
        Some(_) => {
            aviso("SetFileAttributesW: los atributos del FAT32 no se escriben desde Ring 3 todavia");
            kernel32::poner_error(ERROR_ACCESS_DENIED);
            0
        }
        None => {
            kernel32::poner_error(if padre_existe(r) { ERROR_FILE_NOT_FOUND } else { ERROR_PATH_NOT_FOUND });
            0
        }
    })
}

extern "win64" fn set_file_time(h: u64, _c: u64, _a: u64, _e: u64) -> i32 {
    if abierto(h).is_none() {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    }
    aviso("SetFileTime: las fechas del FAT32 no se escriben desde Ring 3 todavia");
    kernel32::poner_error(ERROR_NOT_SUPPORTED);
    0
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "FindFirstFileW" => dir!(find_first_file_w),
        "FindFirstFileExW" => dir!(find_first_file_ex_w),
        "FindNextFileW" => dir!(find_next_file_w),
        "FindClose" => dir!(find_close),
        "GetFileAttributesExW" => dir!(get_file_attributes_ex_w),
        "GetFullPathNameW" => dir!(get_full_path_name_w),
        "GetCurrentDirectoryW" => dir!(get_current_directory_w),
        "SetCurrentDirectoryW" => dir!(set_current_directory_w),
        "GetTempPathW" | "GetTempPath2W" => dir!(get_temp_path_w),
        "GetLogicalDrives" => dir!(get_logical_drives),
        "GetFileInformationByHandle" => dir!(get_file_information_by_handle),
        "GetFileInformationByHandleEx" => dir!(get_file_information_by_handle_ex),
        "SetFileInformationByHandle" => dir!(set_file_information_by_handle),
        "SetEndOfFile" => dir!(set_end_of_file),
        "GetFinalPathNameByHandleW" => dir!(get_final_path_name_by_handle_w),
        "LockFileEx" => dir!(lock_file_ex),
        "UnlockFileEx" => dir!(unlock_file_ex),
        "LockFile" | "UnlockFile" => dir!(lock_file),
        "CopyFileExW" => dir!(copy_file_ex_w),
        "CopyFileW" => dir!(copy_file_w),
        "CreateDirectoryW" => dir!(create_directory_w),
        "RemoveDirectoryW" => dir!(remove_directory_w),
        "DeleteFileW" => dir!(delete_file_w),
        "MoveFileExW" => dir!(move_file_ex_w),
        "MoveFileW" => dir!(move_file_w),
        "SetFileAttributesW" => dir!(set_file_attributes_w),
        "SetFileTime" => dir!(set_file_time),
        _ => return None,
    })
}

