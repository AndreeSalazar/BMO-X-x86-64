//! **Los ficheros de Windows, de la casa** (P4d, 27-09).
//!
//! ```text
//!    CreateFileW/A        la ruta a la del volumen (bmo_proton_x::ficheros)
//!    ReadFile             chicos: el fichero entero en memoria; grandes:
//!                         medida al abrir y rangos via Plataforma::trozos
//!    WriteFile            en memoria; al cerrar, sale entero
//!                         (Plataforma::escribir_fichero)
//!    SetFilePointer(Ex), GetFileSize(Ex), GetFileType, FlushFileBuffers,
//!    GetFileAttributesW, CloseHandle
//! ```
//!
//! El directorio actual es el del `.exe` (lo dice quien carga). D: nunca se
//! escribe: un juego con perfil publica sus cambios en una capa de ESTRATOS y
//! lee primero desde ahi. Lo que no hay, dicho: compartir un fichero entre dos
//! handles que escriben (cada uno tiene su copia). En BMO-X un fichero escrito
//! sale de UNA llamada (`Archivo::escribir_de`, P4f3) y de la medida que sea;
//! si no sale entero, CloseHandle lo dice. Las carpetas: `carpetas.rs` (P4f3).

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
const ERROR_READ_FAULT: u32 = 30;
const INVALID_FILE_ATTRIBUTES: u32 = u32::MAX;
const FILE_TYPE_DISK: u32 = 1;

struct Estado {
    abiertos: Vec<Option<Abierto>>,
    dir: String,
    capa: String,
    borrados: String,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado {
    abiertos: Vec::new(),
    dir: String::new(),
    capa: String::new(),
    borrados: String::new(),
}));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.abiertos.clear();
    e.dir.clear();
    e.capa.clear();
    e.borrados.clear();
}

/// **El directorio actual del `.exe`** (el suyo, `window` para `window/x.exe`).
/// Lo dice quien carga, antes de saltar.
pub fn poner_directorio(dir: &str) {
    estado().dir = String::from(dir.trim_matches('/'));
}

/// La capa de escritura del juego que corre desde D: (`proton-x/<juego>/capa`).
/// Sin ella, las rutas del disco Personal siguen siendo estrictamente de solo lectura.
pub fn poner_capa(capa: Option<&str>) {
    let e = estado();
    e.capa = capa.unwrap_or("").trim_matches('/').into();
    e.borrados = e
        .capa
        .strip_suffix("/capa")
        .map_or_else(String::new, |raiz| alloc::format!("{raiz}/borrados"));
}

/// La ruta paralela de ESTRATOS para un fichero del disco Personal.
pub(crate) fn ruta_capa(ruta: &str) -> Option<String> {
    let raiz = &estado().capa;
    if raiz.is_empty() {
        return None;
    }
    let relativa = ficheros::en_personal(ruta)?;
    let relativa = relativa.trim_start_matches('/');
    Some(if relativa.is_empty() {
        raiz.clone()
    } else {
        alloc::format!("{raiz}/{relativa}")
    })
}

pub(crate) fn huella_de_nombre(nombre: &str) -> String {
    let normal = nombre.to_lowercase();
    let hash = bmo_proton_x::resumen::sha256(normal.as_bytes());
    let mut hex = String::with_capacity(64);
    for b in hash {
        hex.push(char::from(b"0123456789abcdef"[(b >> 4) as usize]));
        hex.push(char::from(b"0123456789abcdef"[(b & 0x0f) as usize]));
    }
    hex
}

/// Carpeta de marcas para las entradas de `ruta` (una por directorio de D:).
pub(crate) fn carpeta_marcas(ruta: &str) -> Option<String> {
    let raiz = &estado().borrados;
    if raiz.is_empty() {
        return None;
    }
    let relativa = ficheros::en_personal(ruta)?.trim_start_matches('/');
    let padre = relativa.rsplit_once('/').map_or("", |(p, _)| p);
    Some(alloc::format!("{raiz}/{}", huella_de_nombre(padre)))
}

/// Ruta de la marca que oculta `ruta` sin escribir en D:.
pub(crate) fn ruta_marca(ruta: &str) -> Option<String> {
    let carpeta = carpeta_marcas(ruta)?;
    let relativa = ficheros::en_personal(ruta)?.trim_start_matches('/');
    let nombre = relativa.rsplit('/').next()?;
    Some(alloc::format!("{carpeta}/{}", huella_de_nombre(nombre)))
}

/// Ruta de lectura con prioridad a la copia publicada en ESTRATOS.
pub(crate) fn ruta_para_leer(ruta: &str) -> String {
    ruta_capa(ruta)
        .filter(|capa| crate::carpetas::entrada_directa(capa).is_some())
        .unwrap_or_else(|| String::from(ruta))
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
pub(crate) extern "win64" fn create_file_w(
    nombre: *const u16,
    acceso: u32,
    compartir: u32,
    seg: u64,
    disposicion: u32,
    banderas: u32,
    plantilla: u64,
) -> u64 {
    let h = create_file_dentro(
        nombre,
        acceso,
        compartir,
        seg,
        disposicion,
        banderas,
        plantilla,
    );
    if h == NO_VALE {
        crate::diario::no_esta("CreateFileW", nombre);
    }
    h
}

fn create_file_dentro(
    nombre: *const u16,
    acceso: u32,
    _compartir: u32,
    _seg: u64,
    disposicion: u32,
    banderas: u32,
    _plantilla: u64,
) -> u64 {
    let ruta_original = match ruta_de(nombre) {
        Ok(r) => r,
        // P4f3: la raiz del volumen es una carpeta ("C:\\", "\\"); N2, y la de D:.
        Err(ERROR_FILE_NOT_FOUND) if crate::carpetas::es_raiz(nombre) => {
            crate::carpetas::raiz_de(nombre).unwrap_or_default()
        }
        Err(e) => {
            kernel32::poner_error(e);
            return NO_VALE;
        }
    };
    let personal = ficheros::en_personal(&ruta_original).is_some();
    let existente = crate::carpetas::entrada(&ruta_original);
    if ruta_original.is_empty()
        || ruta_original == ficheros::PERSONAL
        || existente.as_ref().is_some_and(|e| e.carpeta)
    {
        if banderas & FILE_FLAG_BACKUP_SEMANTICS == 0
            || !matches!(disposicion, OPEN_EXISTING | OPEN_ALWAYS)
        {
            kernel32::poner_error(ERROR_ACCESS_DENIED);
            return NO_VALE;
        }
        if acceso & GENERIC_WRITE != 0 {
            kernel32::poner_error(ERROR_ACCESS_DENIED);
            return NO_VALE;
        }
        kernel32::poner_error(0);
        return abrir(Abierto {
            ruta: ruta_original,
            lee: true,
            carpeta: true,
            ..Abierto::default()
        });
    }
    if !matches!(
        disposicion,
        CREATE_NEW | CREATE_ALWAYS | OPEN_EXISTING | OPEN_ALWAYS | TRUNCATE_EXISTING
    ) {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return NO_VALE;
    }
    let existe = existente.is_some();
    if !crate::carpetas::padre_existe(&ruta_original) {
        kernel32::poner_error(ERROR_PATH_NOT_FOUND);
        return NO_VALE;
    }
    if disposicion == CREATE_NEW && existe {
        kernel32::poner_error(ERROR_FILE_EXISTS);
        return NO_VALE;
    }
    if matches!(disposicion, OPEN_EXISTING | TRUNCATE_EXISTING) && !existe {
        kernel32::poner_error(ERROR_FILE_NOT_FOUND);
        return NO_VALE;
    }

    // CREATE_NEW/TRUNCATE/CREATE_ALWAYS cambian el destino; OPEN_ALWAYS solo
    // lo crea cuando no estaba. Un handle de escritura sobre un fichero de D:
    // conserva los bytes originales y publica su copia en la capa de ESTRATOS.
    let crea_o_trunca = matches!(disposicion, CREATE_NEW | CREATE_ALWAYS | TRUNCATE_EXISTING)
        || (disposicion == OPEN_ALWAYS && !existe);
    let escribe = acceso & GENERIC_WRITE != 0;
    if matches!(disposicion, CREATE_NEW | CREATE_ALWAYS | TRUNCATE_EXISTING) && !escribe {
        kernel32::poner_error(ERROR_ACCESS_DENIED);
        return NO_VALE;
    }
    let mutacion = escribe || crea_o_trunca;
    let ruta_capa = if personal && mutacion {
        let Some(capa) = ruta_capa(&ruta_original) else {
            kernel32::poner_error(ERROR_ACCESS_DENIED);
            return NO_VALE;
        };
        if !crate::carpetas::preparar_capa(&ruta_original) {
            kernel32::poner_error(ERROR_ACCESS_DENIED);
            return NO_VALE;
        }
        Some(capa)
    } else {
        None
    };
    let ruta = ruta_capa.clone().unwrap_or_else(|| {
        if personal {
            ruta_para_leer(&ruta_original)
        } else {
            ruta_original.clone()
        }
    });

    // ** A LA CARTA: solo leer, abrir lo que hay, y grande: se mide y no se trae.
    if acceso & GENERIC_WRITE == 0 && matches!(disposicion, OPEN_EXISTING | OPEN_ALWAYS) {
        if let Some(t) = plataforma().trozos {
            if let Some(m) = (t.medida)(ruta.as_bytes()).filter(|&m| m >= t.umbral) {
                kernel32::poner_error(if disposicion == OPEN_ALWAYS {
                    ERROR_ALREADY_EXISTS
                } else {
                    0
                });
                return abrir(Abierto {
                    ruta,
                    lee: acceso & GENERIC_READ != 0,
                    a_la_carta: Some(m),
                    ..Abierto::default()
                });
            }
        }
    }
    let bytes = if matches!(disposicion, CREATE_ALWAYS | TRUNCATE_EXISTING | CREATE_NEW) {
        Vec::new()
    } else if existe {
        let origen = if ruta_capa
            .as_ref()
            .is_some_and(|c| crate::carpetas::entrada_directa(c).is_none())
        {
            &ruta_original
        } else {
            &ruta
        };
        let Some(bytes) = (plataforma().leer_fichero)(origen.as_bytes()) else {
            kernel32::poner_error(ERROR_READ_FAULT);
            return NO_VALE;
        };
        bytes
    } else {
        Vec::new()
    };
    // Crear o truncar con GENERIC_WRITE publica incluso si cierra sin escribir.
    let sucio = escribe && matches!(disposicion, CREATE_ALWAYS | CREATE_NEW | TRUNCATE_EXISTING)
        || (disposicion == OPEN_ALWAYS && !existe);
    let a = Abierto {
        ruta,
        bytes,
        pos: 0,
        lee: acceso & GENERIC_READ != 0,
        escribe,
        sucio,
        carpeta: false,
        a_la_carta: None,
    };
    // Como Windows: CREATE_ALWAYS y OPEN_ALWAYS sobre uno que ya estaba lo dicen.
    kernel32::poner_error(
        if existe && matches!(disposicion, CREATE_ALWAYS | OPEN_ALWAYS) {
            ERROR_ALREADY_EXISTS
        } else {
            0
        },
    );
    abrir(a)
}

/// `CreateFileA`: la misma, con la ruta en ASCII.
extern "win64" fn create_file_a(
    nombre: *const u8,
    acceso: u32,
    compartir: u32,
    seg: u64,
    disposicion: u32,
    banderas: u32,
    plantilla: u64,
) -> u64 {
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
    create_file_w(
        w.as_ptr(),
        acceso,
        compartir,
        seg,
        disposicion,
        banderas,
        plantilla,
    )
}

/// P4f4: el desplazamiento de un OVERLAPPED (Offset en +16, OffsetHigh en
/// +20). En un handle SINCRONO, Windows lee o escribe ahi y deja el cursor
/// detras.
fn desde_solapado(ov: u64) -> Option<u64> {
    // SAFETY: un OVERLAPPED del `.exe` (32 bytes).
    (ov != 0).then(|| unsafe {
        ((ov + 16) as *const u32).read_unaligned() as u64
            | (((ov + 20) as *const u32).read_unaligned() as u64) << 32
    })
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
    if a.a_la_carta.is_some() {
        let (desde, n) = a.trozo(dst.len());
        if n == 0 {
            return Ok(0);
        }
        let t = plataforma().trozos.ok_or(ERROR_INVALID_HANDLE)?;
        let k = (t.leer)(a.ruta.as_bytes(), desde, &mut dst[..n]).ok_or(ERROR_READ_FAULT)?;
        a.pos = desde + k as u64;
        return Ok(k);
    }
    Ok(a.leer(dst))
}

/// **Escribir en un fichero de la casa** (WriteFile y NtWriteFile).
pub(crate) fn escribir_en(h: u64, src: &[u8], desde: Option<u64>) -> Result<usize, u32> {
    let a = abierto(h)
        .filter(|a| a.escribe)
        .ok_or(ERROR_INVALID_HANDLE)?;
    if let Some(p) = desde {
        a.pos = p;
    }
    Ok(a.escribir(src))
}

/// `ReadFile(h, bufer, n, *leidos, solapado)`.
extern "win64" fn read_file(h: u64, b: *mut u8, n: u32, leidos: *mut u32, solapado: u64) -> i32 {
    // SAFETY: `n` bytes del `.exe` donde escribir.
    let dst = if n == 0 {
        &mut [][..]
    } else {
        unsafe { core::slice::from_raw_parts_mut(b, n as usize) }
    };
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
    let src = if n == 0 {
        &[][..]
    } else {
        unsafe { core::slice::from_raw_parts(b, n as usize) }
    };
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
    let Some(i) = h.checked_sub(FICHERO).map(|i| i as usize) else {
        return 0;
    };
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
            kernel32::poner_error(if metodo > 2 {
                ERROR_INVALID_PARAMETER
            } else {
                ERROR_NEGATIVE_SEEK
            });
            0
        }
    }
}

/// `SetFilePointer(h, bajo, *alto, metodo)`: la de 32 bits, con la mitad
/// alta por puntero (si lo dan).
extern "win64" fn set_file_pointer(h: u64, bajo: i32, alto: *mut i32, metodo: u32) -> u32 {
    // SAFETY: un LONG del `.exe`, o nulo.
    let dist = if alto.is_null() {
        bajo as i64
    } else {
        (unsafe { *alto } as i64) << 32 | bajo as u32 as i64
    };
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
        unsafe { *medida = a.medida() as i64 };
    }
    1
}

extern "win64" fn get_file_size(h: u64, alto: *mut u32) -> u32 {
    let Some(a) = abierto(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return u32::MAX;
    };
    let n = a.medida();
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

pub(crate) extern "win64" fn get_file_attributes_w(nombre: *const u16) -> u32 {
    let a = get_file_attributes_dentro(nombre);
    if a == INVALID_FILE_ATTRIBUTES {
        crate::diario::no_esta("GetFileAttributesW", nombre);
    }
    a
}

fn get_file_attributes_dentro(nombre: *const u16) -> u32 {
    // P4f3: por la lista de su carpeta (sin leer el fichero entero), y las
    // carpetas con FILE_ATTRIBUTE_DIRECTORY.
    match ruta_de(nombre) {
        Ok(r) => match crate::carpetas::entrada(&r) {
            Some(e) => crate::carpetas::atributos(&e),
            None => {
                kernel32::poner_error(if crate::carpetas::padre_existe(&r) {
                    ERROR_FILE_NOT_FOUND
                } else {
                    ERROR_PATH_NOT_FOUND
                });
                INVALID_FILE_ATTRIBUTES
            }
        },
        Err(ERROR_FILE_NOT_FOUND) if crate::carpetas::es_raiz(nombre) => {
            crate::carpetas::DIRECTORIO
        }
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

#[cfg(test)]
mod pruebas_capa {
    use super::*;
    extern crate std;
    use bmo_proton_x::ficheros::Entrada;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Mutex;

    #[derive(Default)]
    struct Volumen {
        carpetas: BTreeSet<String>,
        ficheros: BTreeMap<String, Vec<u8>>,
    }

    static VOLUMEN: Mutex<Volumen> = Mutex::new(Volumen {
        carpetas: BTreeSet::new(),
        ficheros: BTreeMap::new(),
    });
    static UNA_A_LA_VEZ: Mutex<()> = Mutex::new(());

    fn leer(r: &[u8]) -> Option<Vec<u8>> {
        VOLUMEN
            .lock()
            .unwrap()
            .ficheros
            .get(core::str::from_utf8(r).ok()?)
            .cloned()
    }

    fn escribir(r: &[u8], bytes: &[u8]) -> bool {
        let Ok(r) = core::str::from_utf8(r) else {
            return false;
        };
        VOLUMEN
            .lock()
            .unwrap()
            .ficheros
            .insert(String::from(r), bytes.to_vec());
        true
    }

    fn listar(r: &[u8]) -> Option<Vec<Entrada>> {
        let r = core::str::from_utf8(r).ok()?;
        let v = VOLUMEN.lock().unwrap();
        if !v.carpetas.contains(r) {
            return None;
        }
        let prefijo = if r.is_empty() {
            String::new()
        } else {
            alloc::format!("{r}/")
        };
        let mut entradas = Vec::new();
        for d in &v.carpetas {
            let Some(resto) = d.strip_prefix(&prefijo) else {
                continue;
            };
            if !resto.is_empty() && !resto.contains('/') {
                entradas.push(Entrada {
                    nombre: String::from(resto),
                    carpeta: true,
                    atributos: 0x10,
                    ..Default::default()
                });
            }
        }
        for (p, b) in &v.ficheros {
            let Some(resto) = p.strip_prefix(&prefijo) else {
                continue;
            };
            if !resto.is_empty() && !resto.contains('/') {
                entradas.push(Entrada {
                    nombre: String::from(resto),
                    bytes: b.len() as u64,
                    atributos: 0x20,
                    ..Default::default()
                });
            }
        }
        Some(entradas)
    }

    fn crear(r: &[u8]) -> bool {
        let Ok(r) = core::str::from_utf8(r) else {
            return false;
        };
        let padre = r.rsplit_once('/').map_or("", |(p, _)| p);
        let mut v = VOLUMEN.lock().unwrap();
        if v.carpetas.contains(r) || v.ficheros.contains_key(r) || !v.carpetas.contains(padre) {
            return false;
        }
        v.carpetas.insert(String::from(r))
    }

    fn no_quitar(r: &[u8]) -> bool {
        let Ok(r) = core::str::from_utf8(r) else {
            return false;
        };
        let mut v = VOLUMEN.lock().unwrap();
        if v.ficheros.remove(r).is_some() {
            return true;
        }
        let prefijo = alloc::format!("{r}/");
        let vacia = v.carpetas.contains(r)
            && !v.ficheros.keys().any(|p| p.starts_with(&prefijo))
            && !v.carpetas.iter().any(|p| p != r && p.starts_with(&prefijo));
        vacia && v.carpetas.remove(r)
    }
    fn no_renombrar(_: &[u8], _: &[u8]) -> bool {
        false
    }
    fn escribir_consola(_: &[u8]) {}
    fn salir(_: u32) -> ! {
        panic!("la plataforma de prueba no sale")
    }
    fn superficie(_: u32, _: u32) -> Option<crate::Superficie> {
        None
    }
    fn mostrar(_: &crate::Superficie) -> bool {
        false
    }
    fn presentar(_: &crate::Superficie) {}
    fn evento(_: &crate::Superficie) -> u64 {
        0
    }
    fn dormir() {}
    fn poner_gs(_: u64) {}
    fn ahora() -> u64 {
        0
    }
    fn sellar(_: &[u8]) -> Option<u64> {
        None
    }
    fn soltar(_: u64, _: usize) {}
    fn memoria(_: usize) -> Option<u64> {
        None
    }

    fn fecha() -> Option<u64> {
        None
    }

    const ARCHIVO_GIGANTE: &str = "d:Cyberpunk 2077/archive/pc/content/basegame_4_gamedata.archive";
    const MEDIDA_GIGANTE: u64 = (5 << 30) + 37;

    fn medida_gigante(ruta: &[u8]) -> Option<u64> {
        (ruta == ARCHIVO_GIGANTE.as_bytes()).then_some(MEDIDA_GIGANTE)
    }

    fn leer_gigante(ruta: &[u8], desde: u64, dst: &mut [u8]) -> Option<usize> {
        if ruta != ARCHIVO_GIGANTE.as_bytes() || desde >= MEDIDA_GIGANTE {
            return None;
        }
        let n = dst
            .len()
            .min((MEDIDA_GIGANTE - desde).min(usize::MAX as u64) as usize);
        for (k, byte) in dst[..n].iter_mut().enumerate() {
            *byte = desde.wrapping_add(k as u64) as u8;
        }
        Some(n)
    }

    // El TEB del hilo de la prueba (Linux): ver el fichero.
    mod teb_de_prueba;
    use teb_de_prueba::teb_de_prueba;

    fn plataforma_prueba_trozos() -> crate::Plataforma {
        let mut p = plataforma_prueba();
        p.trozos = Some(crate::Trozos {
            medida: medida_gigante,
            leer: leer_gigante,
            umbral: 1,
        });
        p
    }

    fn plataforma_prueba() -> crate::Plataforma {
        crate::Plataforma {
            escribir: escribir_consola,
            salir,
            superficie,
            mostrar,
            presentar,
            evento,
            dormir,
            poner_gs,
            ahora_ns: ahora,
            dibujar: bmo_proton_x::lote::en_cpu,
            sellar_codigo: sellar,
            soltar_codigo: soltar,
            leer_fichero: leer,
            escribir_fichero: escribir,
            memoria,
            fecha,
            listar,
            carpetas: Some(crate::Carpetas {
                crear,
                quitar: no_quitar,
                renombrar: no_renombrar,
            }),
            reserva: None,
            trozos: None,
            sonido: None,
        }
    }

    #[test]
    fn escribir_en_d_publica_una_copia_y_listar_la_prefiere() {
        let _una = UNA_A_LA_VEZ.lock().unwrap();
        {
            let mut v = VOLUMEN.lock().unwrap();
            *v = Volumen::default();
            for d in [
                "",
                "d:",
                "d:Cyberpunk 2077",
                "d:Cyberpunk 2077/bin",
                "d:Cyberpunk 2077/bin/x64",
            ] {
                v.carpetas.insert(String::from(d));
            }
            v.ficheros.insert(
                String::from("d:Cyberpunk 2077/bin/x64/settings.ini"),
                b"original".to_vec(),
            );
            v.ficheros.insert(
                String::from("d:Cyberpunk 2077/bin/x64/engine.dll"),
                b"base".to_vec(),
            );
        }
        // SAFETY: prueba serializada; las funciones usan solo el volumen de arriba.
        unsafe { crate::empezar(plataforma_prueba()) };
        teb_de_prueba();
        poner_directorio("d:Cyberpunk 2077/bin/x64");
        poner_capa(Some("proton-x/cyberpunk2077/capa"));
        let nombre: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\settings.ini"
            .encode_utf16()
            .chain([0])
            .collect();
        let h = create_file_dentro(
            nombre.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            0,
            OPEN_EXISTING,
            0,
            0,
        );
        assert_ne!(
            h, NO_VALE,
            "un OPEN_EXISTING para escribir crea la copia en la capa"
        );
        let mut antes = [0u8; 8];
        assert_eq!(leer_de(h, &mut antes, None), Ok(8));
        assert_eq!(&antes, b"original");
        assert_eq!(abierto(h).unwrap().mover(0, 0), Some(0));
        assert_eq!(escribir_en(h, b"patched!", None), Ok(8));
        assert_eq!(cerrar(h), 1);

        let original = "d:Cyberpunk 2077/bin/x64/settings.ini";
        let copia = "proton-x/cyberpunk2077/capa/Cyberpunk 2077/bin/x64/settings.ini";
        assert_eq!(
            leer(original.as_bytes()).unwrap(),
            b"original",
            "D: nunca cambia"
        );
        assert_eq!(leer(copia.as_bytes()).unwrap(), b"patched!");
        assert_eq!(
            ruta_para_leer(original),
            copia,
            "la siguiente lectura usa la capa"
        );

        let nuevas: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\trace.log"
            .encode_utf16()
            .chain([0])
            .collect();
        let h = create_file_dentro(nuevas.as_ptr(), GENERIC_WRITE, 0, 0, CREATE_NEW, 0, 0);
        assert_ne!(h, NO_VALE, "CREATE_NEW de D: se dirige a ESTRATOS");
        assert_eq!(escribir_en(h, b"log", None), Ok(3));
        assert_eq!(cerrar(h), 1);
        assert_eq!(
            leer(b"d:Cyberpunk 2077/bin/x64/trace.log"),
            None,
            "un fichero nuevo tampoco aparece en D:"
        );
        assert_eq!(
            leer(b"proton-x/cyberpunk2077/capa/Cyberpunk 2077/bin/x64/trace.log").unwrap(),
            b"log"
        );

        let entradas = crate::carpetas::listar("d:Cyberpunk 2077/bin/x64").unwrap();
        assert_eq!(
            entradas
                .iter()
                .find(|e| e.nombre == "settings.ini")
                .unwrap()
                .bytes,
            8
        );
        assert!(
            entradas.iter().any(|e| e.nombre == "engine.dll"),
            "D: sigue visible debajo de la capa"
        );
        assert!(
            entradas.iter().any(|e| e.nombre == "trace.log"),
            "los ficheros de la capa tambien se listan"
        );

        let origen: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\engine.dll"
            .encode_utf16()
            .chain([0])
            .collect();
        let destino: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\engine-copy.dll"
            .encode_utf16()
            .chain([0])
            .collect();
        assert_eq!(
            crate::carpetas::move_file_ex_w(origen.as_ptr(), destino.as_ptr(), 0),
            1
        );
        assert_eq!(
            leer(b"d:Cyberpunk 2077/bin/x64/engine.dll").unwrap(),
            b"base",
            "mover tambien conserva el original"
        );
        assert!(crate::carpetas::entrada("d:Cyberpunk 2077/bin/x64/engine.dll").is_none());
        assert_eq!(
            leer(b"proton-x/cyberpunk2077/capa/Cyberpunk 2077/bin/x64/engine-copy.dll").unwrap(),
            b"base"
        );

        let copia: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\engine-copy.dll"
            .encode_utf16()
            .chain([0])
            .collect();
        let respaldo: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\engine-backup.dll"
            .encode_utf16()
            .chain([0])
            .collect();
        assert_eq!(
            crate::carpetas::copy_file_ex_w(copia.as_ptr(), respaldo.as_ptr(), 0, 0, 0, 0),
            1
        );
        assert_eq!(
            leer(b"proton-x/cyberpunk2077/capa/Cyberpunk 2077/bin/x64/engine-backup.dll").unwrap(),
            b"base"
        );

        assert_eq!(
            crate::carpetas::delete_file_w(nombre.as_ptr()),
            1,
            "borrar marca el nombre sin tocar D:"
        );
        assert_eq!(leer(original.as_bytes()).unwrap(), b"original");
        assert!(
            crate::carpetas::entrada(original).is_none(),
            "el original queda oculto"
        );
        let entradas = crate::carpetas::listar("d:Cyberpunk 2077/bin/x64").unwrap();
        assert!(
            !entradas.iter().any(|e| e.nombre == "settings.ini"),
            "el listado respeta la marca"
        );

        poner_capa(None);
        let denied = create_file_dentro(nombre.as_ptr(), GENERIC_WRITE, 0, 0, OPEN_EXISTING, 0, 0);
        assert_eq!(denied, NO_VALE, "sin perfil, D: conserva el solo lectura");
    }

    #[test]
    fn archivo_mayor_de_4_gib_se_mide_y_lee_sin_cargarlo_entero() {
        let _una = UNA_A_LA_VEZ.lock().unwrap();
        {
            let mut v = VOLUMEN.lock().unwrap();
            *v = Volumen::default();
            for d in [
                "",
                "d:",
                "d:Cyberpunk 2077",
                "d:Cyberpunk 2077/archive",
                "d:Cyberpunk 2077/archive/pc",
                "d:Cyberpunk 2077/archive/pc/content",
            ] {
                v.carpetas.insert(String::from(d));
            }
            // Solo se registra el nombre. Los 5 GiB existen virtualmente en
            // los callbacks: la prueba no reserva memoria proporcional.
            v.ficheros.insert(String::from(ARCHIVO_GIGANTE), Vec::new());
        }
        // SAFETY: prueba serializada; todas las E/S van al volumen simulado.
        unsafe { crate::empezar(plataforma_prueba_trozos()) };
        teb_de_prueba();
        crate::ficheros::poner_directorio("d:Cyberpunk 2077/bin/x64");
        crate::ficheros::poner_capa(None);

        let nombre: Vec<u16> =
            "D:\\Cyberpunk 2077\\archive\\pc\\content\\basegame_4_gamedata.archive"
                .encode_utf16()
                .chain([0])
                .collect();
        let h = create_file_dentro(nombre.as_ptr(), GENERIC_READ, 0, 0, OPEN_EXISTING, 0, 0);
        assert_ne!(h, NO_VALE);
        assert_eq!(
            abierto(h).unwrap().bytes.len(),
            0,
            "no se carga el contenido"
        );
        assert_eq!(abierto(h).unwrap().medida(), MEDIDA_GIGANTE);

        let mut medida = 0i64;
        assert_eq!(get_file_size_ex(h, &mut medida), 1);
        assert_eq!(medida as u64, MEDIDA_GIGANTE);
        let mut alto = 0u32;
        assert_eq!(get_file_size(h, &mut alto), MEDIDA_GIGANTE as u32);
        assert_eq!(alto, (MEDIDA_GIGANTE >> 32) as u32);

        let desde = (1u64 << 32) + 19;
        let mut nueva = 0i64;
        assert_eq!(set_file_pointer_ex(h, desde as i64, &mut nueva, 0), 1);
        assert_eq!(nueva as u64, desde);
        let mut bytes = [0u8; 32];
        let mut leidos = 0u32;
        assert_eq!(
            read_file(h, bytes.as_mut_ptr(), bytes.len() as u32, &mut leidos, 0),
            1
        );
        assert_eq!(leidos as usize, bytes.len());
        for (k, byte) in bytes.iter().enumerate() {
            assert_eq!(*byte, desde.wrapping_add(k as u64) as u8);
        }

        assert_eq!(set_file_pointer_ex(h, -3, &mut nueva, 2), 1);
        assert_eq!(nueva as u64, MEDIDA_GIGANTE - 3);
        let mut final_bytes = [0xFF; 8];
        assert_eq!(
            read_file(
                h,
                final_bytes.as_mut_ptr(),
                final_bytes.len() as u32,
                &mut leidos,
                0
            ),
            1
        );
        assert_eq!(leidos, 3);
        assert_eq!(
            &final_bytes[..3],
            &[
                (MEDIDA_GIGANTE - 3) as u8,
                (MEDIDA_GIGANTE - 2) as u8,
                (MEDIDA_GIGANTE - 1) as u8
            ]
        );
        assert_eq!(
            read_file(
                h,
                final_bytes.as_mut_ptr(),
                final_bytes.len() as u32,
                &mut leidos,
                0
            ),
            1
        );
        assert_eq!(leidos, 0, "EOF es exito con 0 bytes");
        assert_eq!(cerrar(h), 1);
    }

    // En su carpeta (`ficheros/pruebas_capa/`) y sin `#[path]`: el `../` de
    // antes pasaba por una carpeta que no existe, y eso Windows lo resuelve
    // en el texto pero Linux no (02-10).
    mod pruebas_mapeo_archive;
}
