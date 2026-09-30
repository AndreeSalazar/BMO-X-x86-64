//! **VERSION y la seguridad de los ficheros** (tanda 14b de Cyberpunk,
//! 30-09): DURAS del censo.
//!
//! ```text
//!    GetFileVersionInfoSizeA/W, GetFileVersionInfoA/W
//!                         el recurso de version (RT_VERSION) del fichero,
//!                         leido de su .rsrc
//!    VerQueryValueA/W     "\"  (VS_FIXEDFILEINFO), "\VarFileInfo\Translation"
//!                         y "\StringFileInfo\<idioma>\<nombre>"; con A, el
//!                         texto se da en bytes, en el sitio de mas del bloque
//!    GetFileSecurityW     un descriptor de seguridad con la DACL NULA: todo
//!                         el mundo puede todo (el FAT32 de BMO-X no tiene
//!                         permisos que contar)
//!    ImpersonateSelf, RevertToSelf   el hilo "se hace pasar" por si mismo:
//!                         desde ahi, OpenThreadToken da el token
//!    AccessCheck          con la DACL nula: se concede lo pedido
//! ```
//!
//! El bloque de GetFileVersionInfo es el VS_VERSIONINFO del fichero tal
//! cual (en UTF-16), y detras un sitio del mismo largo para los textos que
//! VerQueryValueA pase a bytes.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::{kernel32, plataforma};

const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_RESOURCE_TYPE_NOT_FOUND: u32 = 1813;
const ERROR_RESOURCE_DATA_NOT_FOUND: u32 = 1812;

// -- Leer el recurso de version ------------------------------------------------------

fn u16_de(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

fn u32_de(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// **El VS_VERSIONINFO de un PE**: el primer RT_VERSION (16) de su .rsrc.
pub(crate) fn recurso_de_version(f: &[u8]) -> Result<Vec<u8>, u32> {
    let pe = u32_de(f, 0x3C).ok_or(ERROR_RESOURCE_DATA_NOT_FOUND)? as usize;
    if f.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err(ERROR_RESOURCE_DATA_NOT_FOUND);
    }
    let nsec = u16_de(f, pe + 6).ok_or(ERROR_RESOURCE_DATA_NOT_FOUND)? as usize;
    let tam_opt = u16_de(f, pe + 20).ok_or(ERROR_RESOURCE_DATA_NOT_FOUND)? as usize;
    let opt = pe + 24;
    let magia = u16_de(f, opt).ok_or(ERROR_RESOURCE_DATA_NOT_FOUND)?;
    // El directorio 2 (recursos): en PE32+ empieza en +112, en PE32 en +96.
    let dirs = opt + if magia == 0x20B { 112 } else { 96 };
    let rsrc = u32_de(f, dirs + 16).ok_or(ERROR_RESOURCE_TYPE_NOT_FOUND)?;
    if rsrc == 0 {
        return Err(ERROR_RESOURCE_TYPE_NOT_FOUND);
    }
    let secciones = opt + tam_opt;
    // De una RVA al fichero.
    let desde = |rva: u32| -> Option<usize> {
        (0..nsec).find_map(|k| {
            let s = secciones + 40 * k;
            let (va, vt) = (u32_de(f, s + 12)?, u32_de(f, s + 8)?.max(u32_de(f, s + 16)?));
            let bruto = u32_de(f, s + 20)?;
            (rva >= va && rva < va + vt).then(|| (bruto + (rva - va)) as usize)
        })
    };
    let raiz = desde(rsrc).ok_or(ERROR_RESOURCE_TYPE_NOT_FOUND)?;
    // Una entrada de un directorio de recursos: (id buscado o el primero).
    let entrada = |dir: usize, id: Option<u32>| -> Option<u32> {
        let n = u16_de(f, dir + 12)? as usize + u16_de(f, dir + 14)? as usize;
        (0..n).find_map(|k| {
            let e = dir + 16 + 8 * k;
            let nombre = u32_de(f, e)?;
            let a = u32_de(f, e + 4)?;
            (id.is_none() || (nombre & 0x8000_0000 == 0 && Some(nombre) == id)).then_some(a)
        })
    };
    let bajar = |a: u32| -> Option<usize> { (a & 0x8000_0000 != 0).then(|| raiz + (a & 0x7FFF_FFFF) as usize) };
    let tipo = entrada(raiz, Some(16)).ok_or(ERROR_RESOURCE_TYPE_NOT_FOUND)?;
    let nombre = bajar(tipo).and_then(|d| entrada(d, None)).ok_or(ERROR_RESOURCE_DATA_NOT_FOUND)?;
    let idioma = bajar(nombre).and_then(|d| entrada(d, None)).ok_or(ERROR_RESOURCE_DATA_NOT_FOUND)?;
    let dato = raiz + idioma as usize;
    let (rva, tam) = (u32_de(f, dato).ok_or(ERROR_RESOURCE_DATA_NOT_FOUND)?, u32_de(f, dato + 4).ok_or(ERROR_RESOURCE_DATA_NOT_FOUND)?);
    let o = desde(rva).ok_or(ERROR_RESOURCE_DATA_NOT_FOUND)?;
    f.get(o..o + tam as usize).map(|b| b.to_vec()).ok_or(ERROR_RESOURCE_DATA_NOT_FOUND)
}

/// Un nodo del arbol VS_VERSIONINFO: (clave, valor, hijos) como posiciones
/// en el bloque. wLength +0, wValueLength +2, wType +4, la clave desde +6,
/// y el valor y los hijos alineados a 4.
struct Nodo {
    clave: Vec<u16>,
    valor: usize,
    valor_bytes: usize,
    texto: bool,
    hijos: usize,
    fin: usize,
}

fn nodo(b: &[u8], o: usize) -> Option<Nodo> {
    let largo = u16_de(b, o)? as usize;
    let largo_valor = u16_de(b, o + 2)? as usize;
    let texto = u16_de(b, o + 4)? == 1;
    let mut k = o + 6;
    let mut clave = Vec::new();
    loop {
        let c = u16_de(b, k)?;
        k += 2;
        if c == 0 {
            break;
        }
        clave.push(c);
    }
    let valor = (k + 3) & !3;
    // wValueLength: en caracteres si es texto, en bytes si no.
    let valor_bytes = if texto { largo_valor * 2 } else { largo_valor };
    let hijos = (valor + valor_bytes + 3) & !3;
    let fin = o + largo;
    if largo < 6 || fin > b.len() {
        return None;
    }
    Some(Nodo { clave, valor, valor_bytes, texto, hijos, fin })
}

/// Buscar `camino` ("\\", "\\VarFileInfo\\Translation"...) en el bloque.
fn buscar_en(b: &[u8], camino: &[u16]) -> Option<Nodo> {
    let mut n = nodo(b, 0)?;
    for parte in camino.split(|&c| c == b'\\' as u16).filter(|p| !p.is_empty()) {
        let mut o = n.hijos;
        let mut hallado = None;
        while o + 6 <= n.fin {
            let h = nodo(b, o)?;
            let igual = h.clave.len() == parte.len() && h.clave.iter().zip(parte).all(|(a, c)| (*a as u8).eq_ignore_ascii_case(&(*c as u8)) && (*a < 0x80) == (*c < 0x80));
            let siguiente = (h.fin + 3) & !3;
            if igual {
                hallado = Some(h);
                break;
            }
            o = siguiente;
        }
        n = hallado?;
    }
    Some(n)
}

/// El fichero `nombre` (una ruta de Windows del `.exe`), entero.
fn leer(nombre: &[u16]) -> Result<Vec<u8>, u32> {
    let mut z = nombre.to_vec();
    z.push(0);
    let ruta = crate::ficheros::ruta_de(z.as_ptr())?;
    (plataforma().leer_fichero)(ruta.as_bytes()).ok_or(ERROR_FILE_NOT_FOUND)
}

fn ancha(p: *const u8, ansi: bool) -> Vec<u16> {
    let mut v = Vec::new();
    if p.is_null() {
        return v;
    }
    loop {
        // SAFETY: una cadena del `.exe`, terminada en cero.
        let c = unsafe { if ansi { p.add(v.len()).read() as u16 } else { (p as *const u16).add(v.len()).read() } };
        if c == 0 || v.len() > 32_768 {
            return v;
        }
        v.push(c);
    }
}

fn version_info_size(nombre: *const u8, ansi: bool, _h: *mut u32) -> u32 {
    match leer(&ancha(nombre, ansi)).and_then(|f| recurso_de_version(&f)) {
        // El bloque y, detras, sitio para los textos en bytes de la A.
        Ok(r) => (r.len() * 2) as u32,
        Err(e) => {
            kernel32::poner_error(e);
            0
        }
    }
}

extern "win64" fn get_file_version_info_size_a(n: *const u8, h: *mut u32) -> u32 {
    version_info_size(n, true, h)
}

extern "win64" fn get_file_version_info_size_w(n: *const u16, h: *mut u32) -> u32 {
    version_info_size(n as *const u8, false, h)
}

fn version_info(nombre: *const u8, ansi: bool, largo: u32, datos: *mut u8) -> i32 {
    let r = match leer(&ancha(nombre, ansi)).and_then(|f| recurso_de_version(&f)) {
        Ok(r) => r,
        Err(e) => {
            kernel32::poner_error(e);
            return 0;
        }
    };
    if datos.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let n = r.len().min(largo as usize);
    // SAFETY: `largo` bytes del `.exe`.
    unsafe {
        core::ptr::copy_nonoverlapping(r.as_ptr(), datos, n);
        if (largo as usize) > n {
            core::ptr::write_bytes(datos.add(n), 0, largo as usize - n);
        }
    }
    1
}

extern "win64" fn get_file_version_info_a(n: *const u8, _h: u32, largo: u32, datos: *mut u8) -> i32 {
    version_info(n, true, largo, datos)
}

extern "win64" fn get_file_version_info_w(n: *const u16, _h: u32, largo: u32, datos: *mut u8) -> i32 {
    version_info(n as *const u8, false, largo, datos)
}

/// `VerQueryValue(bloque, camino, *puntero, *largo)`: el puntero va DENTRO
/// del bloque; los textos de la A, pasados a bytes en la mitad de detras.
fn ver_query_value(bloque: *const u8, camino: *const u8, sale: *mut u64, largo: *mut u32, ansi: bool) -> i32 {
    if bloque.is_null() || sale.is_null() {
        return 0;
    }
    // SAFETY: el bloque de GetFileVersionInfo: su wLength dice lo que mide.
    let total = unsafe { (bloque as *const u16).read_unaligned() } as usize;
    // SAFETY: lo mismo.
    let b = unsafe { core::slice::from_raw_parts(bloque, total) };
    let Some(n) = buscar_en(b, &ancha(camino, ansi)) else { return 0 };
    let mut p = bloque as u64 + n.valor as u64;
    let mut l = if n.texto { (n.valor_bytes / 2) as u32 } else { n.valor_bytes as u32 };
    if n.texto && ansi && n.valor_bytes > 0 {
        // A: el texto, en bytes, en el sitio de detras (su misma posicion).
        let destino = bloque as u64 + total as u64 + n.valor as u64 / 2;
        for k in 0..n.valor_bytes / 2 {
            let c = u16_de(b, n.valor + 2 * k).unwrap_or(0);
            // SAFETY: el bloque mide el doble (GetFileVersionInfoSizeA).
            unsafe { ((destino as *mut u8).add(k)).write(if c < 0x100 { c as u8 } else { b'?' }) };
        }
        p = destino;
        l = (n.valor_bytes / 2) as u32;
    }
    // SAFETY: los punteros del `.exe`.
    unsafe {
        sale.write(p);
        if !largo.is_null() {
            largo.write(l);
        }
    }
    (l > 0 || !n.texto) as i32
}

extern "win64" fn ver_query_value_a(b: *const u8, c: *const u8, s: *mut u64, l: *mut u32) -> i32 {
    ver_query_value(b, c, s, l, true)
}

extern "win64" fn ver_query_value_w(b: *const u8, c: *const u16, s: *mut u64, l: *mut u32) -> i32 {
    ver_query_value(b, c as *const u8, s, l, false)
}

// -- La seguridad ------------------------------------------------------------------------

struct Global(UnsafeCell<bool>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
unsafe impl Sync for Global {}
static SUPLANTA: Global = Global(UnsafeCell::new(false));

/// Si el hilo se hace pasar por si mismo (ImpersonateSelf): OpenThreadToken
/// da entonces el token.
pub(crate) fn suplantando() -> bool {
    // SAFETY: ver `Global`.
    unsafe { *SUPLANTA.0.get() }
}

pub(crate) fn reiniciar() {
    // SAFETY: ver `Global`.
    unsafe { *SUPLANTA.0.get() = false };
}

extern "win64" fn impersonate_self(_nivel: u32) -> i32 {
    // SAFETY: ver `Global`.
    unsafe { *SUPLANTA.0.get() = true };
    1
}

extern "win64" fn revert_to_self() -> i32 {
    reiniciar();
    1
}

/// El descriptor de seguridad de la casa: auto-relativo, con la DACL
/// presente y NULA (todo el mundo puede todo), sin propietario ni grupo que
/// decir.
const DESCRIPTOR: [u8; 20] = [1, 0, 0x04, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

extern "win64" fn get_file_security_w(nombre: *const u16, _que: u32, sd: *mut u8, largo: u32, hace_falta: *mut u32) -> i32 {
    let existe = crate::kernel32_a::w::<extern "win64" fn(*const u16) -> u32>("GetFileAttributesW")(nombre) != u32::MAX;
    if !existe {
        kernel32::poner_error(ERROR_FILE_NOT_FOUND);
        return 0;
    }
    if !hace_falta.is_null() {
        // SAFETY: un DWORD del `.exe`.
        unsafe { hace_falta.write(DESCRIPTOR.len() as u32) };
    }
    if sd.is_null() || (largo as usize) < DESCRIPTOR.len() {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    // SAFETY: `largo` bytes del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(DESCRIPTOR.as_ptr(), sd, DESCRIPTOR.len()) };
    1
}

/// `AccessCheck`: con la DACL nula, se concede lo pedido (los genericos,
/// traducidos por el GENERIC_MAPPING; MAXIMUM_ALLOWED, el GenericAll).
#[allow(clippy::too_many_arguments)]
extern "win64" fn access_check(sd: *const u8, token: u64, deseado: u32, mapa: *const u32, _privs: *mut u8, _largo: *mut u32, concedido: *mut u32, estado: *mut i32) -> i32 {
    if sd.is_null() || token == 0 || mapa.is_null() || concedido.is_null() || estado.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    // SAFETY: el GENERIC_MAPPING del `.exe`: Read, Write, Execute, All.
    let m = unsafe { [mapa.read(), mapa.add(1).read(), mapa.add(2).read(), mapa.add(3).read()] };
    let mut d = deseado & 0x0FFF_FFFF & !0x0200_0000;
    for (bit, k) in [(0x8000_0000u32, 0), (0x4000_0000, 1), (0x2000_0000, 2), (0x1000_0000, 3)] {
        if deseado & bit != 0 {
            d |= m[k];
        }
    }
    if deseado & 0x0200_0000 != 0 {
        d |= m[3];
    }
    // SAFETY: los punteros del `.exe`.
    unsafe {
        concedido.write(d);
        estado.write(1);
    }
    1
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    use crate::dir;
    Some(match n {
        "GetFileVersionInfoSizeA" | "GetFileVersionInfoSizeExA" => dir!(get_file_version_info_size_a),
        "GetFileVersionInfoSizeW" => dir!(get_file_version_info_size_w),
        "GetFileVersionInfoA" => dir!(get_file_version_info_a),
        "GetFileVersionInfoW" => dir!(get_file_version_info_w),
        "VerQueryValueA" => dir!(ver_query_value_a),
        "VerQueryValueW" => dir!(ver_query_value_w),
        "GetFileSecurityW" => dir!(get_file_security_w),
        "ImpersonateSelf" => dir!(impersonate_self),
        "RevertToSelf" => dir!(revert_to_self),
        "AccessCheck" => dir!(access_check),
        _ => return None,
    })
}
