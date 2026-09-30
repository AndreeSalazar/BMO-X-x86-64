//! **`advapi32.dll` de la casa** (tanda 11 de Cyberpunk, 30-09): lo que
//! Cyberpunk importa de ADVAPI32 que no es el registro (ese esta en
//! [`crate::advapi32_registro`], en la cadena de kernel32, y ADVAPI32 llega
//! a el por la misma, igual que a OpenProcessToken).
//!
//! ```text
//!    el usuario  GetUserNameW/A (el USERNAME del entorno)
//!    CryptoAPI   CryptAcquireContextW/A CryptReleaseContext CryptGetProvParam
//!                CryptEnumProvidersW CryptGenRandom (RDRAND)
//!                CryptCreateHash CryptHashData CryptGetHashParam
//!                CryptSetHashParam CryptDestroyHash: MD5, SHA-1 y SHA-256
//!                de verdad (bmo_proton_x::resumen)
//!    las claves  CryptGetUserKey CryptImportKey CryptExportKey CryptEncrypt
//!                CryptDecrypt CryptDestroyKey CryptSignHashW: la casa NO
//!                tiene claves todavia (ni RSA ni AES): NTE_NO_KEY/NTE_BAD_KEY,
//!                y CryptImportKey lo dice con un aviso
//!    eventos     RegisterEventSourceW ReportEventW DeregisterEventSource
//!                (el visor de eventos de la casa no guarda nada), y ETW:
//!                EventRegister EventUnregister EventSetInformation
//!                EventWriteTransfer (nadie escucha) EventActivityIdControl
//!    servicios   OpenSCManagerA OpenServiceA (la casa no tiene servicios:
//!                ERROR_SERVICE_DOES_NOT_EXIST) QueryServiceStatus
//!                ControlService StartServiceA CloseServiceHandle
//! ```

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::resumen;
use bmo_proton_x::texto;

use crate::{aviso, dir, kernel32};

const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_MORE_DATA: u32 = 234;
const ERROR_NO_MORE_ITEMS: u32 = 259;
const ERROR_SERVICE_DOES_NOT_EXIST: u32 = 1060;

const NTE_BAD_UID: u32 = 0x8009_0001;
const NTE_BAD_HASH: u32 = 0x8009_0002;
const NTE_BAD_KEY: u32 = 0x8009_0003;
const NTE_BAD_ALGID: u32 = 0x8009_0008;
const NTE_BAD_TYPE: u32 = 0x8009_000A;
const NTE_BAD_HASH_STATE: u32 = 0x8009_000C;
const NTE_NO_KEY: u32 = 0x8009_000D;
const NTE_BAD_KEYSET: u32 = 0x8009_0016;
const NTE_PROV_TYPE_NOT_DEF: u32 = 0x8009_0017;
const NTE_KEYSET_NOT_DEF: u32 = 0x8009_0019;
const NTE_NOT_SUPPORTED: u32 = 0x8009_0029;

struct Proveedor {
    h: u64,
    nombre: &'static str,
    tipo: u32,
}

struct Resumen {
    h: u64,
    alg: u32,
    datos: Vec<u8>,
    fin: Option<Vec<u8>>,
}

struct Estado {
    proveedores: Vec<Proveedor>,
    resumenes: Vec<Resumen>,
    siguiente: u64,
    /// EventActivityIdControl: (el hilo, su id de actividad).
    actividades: Vec<(u32, [u8; 16])>,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { proveedores: Vec::new(), resumenes: Vec::new(), siguiente: 0, actividades: Vec::new() }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.proveedores.clear();
    e.resumenes.clear();
    e.siguiente = 0;
    e.actividades.clear();
}

fn no(e: u32) -> i32 {
    kernel32::poner_error(e);
    0
}

fn poner32(p: *mut u32, v: u32) {
    if !p.is_null() {
        // SAFETY: un DWORD del `.exe`.
        unsafe { *p = v };
    }
}

/// Unos bytes a un bufer con su medida (`*n`), a la manera de CryptoAPI:
/// sin bufer, la medida y TRUE; corto, ERROR_MORE_DATA.
fn dar(datos: &[u8], p: *mut u8, n: *mut u32) -> i32 {
    if n.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    // SAFETY: el DWORD del `.exe`.
    let hay = unsafe { *n } as usize;
    poner32(n, datos.len() as u32);
    if p.is_null() {
        return 1;
    }
    if hay < datos.len() {
        return no(ERROR_MORE_DATA);
    }
    // SAFETY: `hay` bytes suyos.
    unsafe { core::ptr::copy_nonoverlapping(datos.as_ptr(), p, datos.len()) };
    1
}

fn cadena_w(p: *const u16) -> Option<Vec<u16>> {
    // SAFETY: una cadena suya, terminada en 0.
    (!p.is_null()).then(|| unsafe { crate::user32::utf16(p) })
}

fn cadena_a(p: *const u8) -> Option<Vec<u16>> {
    (!p.is_null()).then(|| texto::a_ancho(&crate::crt::cadena_c(p as u64), false).unwrap_or_default())
}

// -- El usuario ---------------------------------------------------------------------------------

fn usuario() -> Vec<u16> {
    crate::proceso::variable("USERNAME").filter(|v| !v.is_empty()).unwrap_or_else(|| "BMO".encode_utf16().collect())
}

/// `GetUserNameW(buf, *n)`: `*n` en caracteres, con el 0.
extern "win64" fn get_user_name_w(buf: *mut u16, n: *mut u32) -> i32 {
    let u = usuario();
    if n.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    // SAFETY: el DWORD del `.exe`.
    let hay = unsafe { *n } as usize;
    poner32(n, u.len() as u32 + 1);
    if buf.is_null() || hay <= u.len() {
        return no(ERROR_INSUFFICIENT_BUFFER);
    }
    // SAFETY: `hay` WCHAR suyos.
    unsafe {
        core::ptr::copy_nonoverlapping(u.as_ptr(), buf, u.len());
        *buf.add(u.len()) = 0;
    }
    1
}

extern "win64" fn get_user_name_a(buf: *mut u8, n: *mut u32) -> i32 {
    let u = texto::a_estrecho(&usuario(), false).unwrap_or_default();
    if n.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    // SAFETY: el DWORD del `.exe`.
    let hay = unsafe { *n } as usize;
    poner32(n, u.len() as u32 + 1);
    if buf.is_null() || hay <= u.len() {
        return no(ERROR_INSUFFICIENT_BUFFER);
    }
    // SAFETY: `hay` bytes suyos.
    unsafe {
        core::ptr::copy_nonoverlapping(u.as_ptr(), buf, u.len());
        *buf.add(u.len()) = 0;
    }
    1
}

// -- CryptoAPI: los proveedores -----------------------------------------------------------------

/// Los proveedores de Windows 10, en el orden en que los da
/// CryptEnumProviders (el de su clave del registro).
const PROVEEDORES: [(&str, u32); 10] = [
    ("Microsoft Base Cryptographic Provider v1.0", 1),
    ("Microsoft Base DSS and Diffie-Hellman Cryptographic Provider", 13),
    ("Microsoft Base DSS Cryptographic Provider", 3),
    ("Microsoft Base Smart Card Crypto Provider", 1),
    ("Microsoft DH SChannel Cryptographic Provider", 18),
    ("Microsoft Enhanced Cryptographic Provider v1.0", 1),
    ("Microsoft Enhanced DSS and Diffie-Hellman Cryptographic Provider", 13),
    ("Microsoft Enhanced RSA and AES Cryptographic Provider", 24),
    ("Microsoft RSA SChannel Cryptographic Provider", 12),
    ("Microsoft Strong Cryptographic Provider", 1),
];

/// El de cada tipo si no se nombra ninguno.
fn por_defecto(tipo: u32) -> Option<&'static str> {
    Some(match tipo {
        1 => "Microsoft Strong Cryptographic Provider",
        3 => "Microsoft Base DSS Cryptographic Provider",
        12 => "Microsoft RSA SChannel Cryptographic Provider",
        13 => "Microsoft Enhanced DSS and Diffie-Hellman Cryptographic Provider",
        18 => "Microsoft DH SChannel Cryptographic Provider",
        24 => "Microsoft Enhanced RSA and AES Cryptographic Provider",
        _ => return None,
    })
}

const CRYPT_NEWKEYSET: u32 = 0x8;
const CRYPT_DELETEKEYSET: u32 = 0x10;
const CRYPT_VERIFYCONTEXT: u32 = 0xF000_0000;

fn adquirir(p: *mut u64, proveedor: Option<Vec<u16>>, tipo: u32, banderas: u32) -> i32 {
    if p.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    let Some(defecto) = por_defecto(tipo) else { return no(NTE_PROV_TYPE_NOT_DEF) };
    let nombre = match proveedor.filter(|v| !v.is_empty()) {
        None => defecto,
        Some(v) => match PROVEEDORES.iter().find(|(n, t)| *t == tipo && n.encode_utf16().eq(v.iter().copied())) {
            Some((n, _)) => n,
            None => return no(NTE_KEYSET_NOT_DEF),
        },
    };
    if banderas & CRYPT_DELETEKEYSET != 0 {
        return 1;
    }
    // Sin VERIFYCONTEXT se pide un contenedor de claves: la casa no los
    // guarda, asi que solo existe si se crea (y nace vacio).
    if banderas & (CRYPT_VERIFYCONTEXT | CRYPT_NEWKEYSET) == 0 {
        return no(NTE_BAD_KEYSET);
    }
    let e = estado();
    e.siguiente += 1;
    let h = 0x5B40_0000 + 4 * e.siguiente;
    e.proveedores.push(Proveedor { h, nombre, tipo });
    // SAFETY: el HCRYPTPROV del `.exe`.
    unsafe { *p = h };
    1
}

extern "win64" fn crypt_acquire_context_w(p: *mut u64, _cont: *const u16, prov: *const u16, tipo: u32, banderas: u32) -> i32 {
    adquirir(p, cadena_w(prov), tipo, banderas)
}

extern "win64" fn crypt_acquire_context_a(p: *mut u64, _cont: *const u8, prov: *const u8, tipo: u32, banderas: u32) -> i32 {
    adquirir(p, cadena_a(prov), tipo, banderas)
}

fn proveedor(h: u64) -> Option<(&'static str, u32)> {
    estado().proveedores.iter().find(|p| p.h == h).map(|p| (p.nombre, p.tipo))
}

extern "win64" fn crypt_release_context(h: u64, _b: u32) -> i32 {
    let e = estado();
    match e.proveedores.iter().position(|p| p.h == h) {
        Some(i) => {
            e.proveedores.remove(i);
            1
        }
        None => no(NTE_BAD_UID),
    }
}

const PP_NAME: u32 = 4;
const PP_PROVTYPE: u32 = 16;

extern "win64" fn crypt_get_prov_param(h: u64, que: u32, p: *mut u8, n: *mut u32, _b: u32) -> i32 {
    let Some((nombre, tipo)) = proveedor(h) else { return no(NTE_BAD_UID) };
    match que {
        PP_NAME => {
            let mut v = nombre.as_bytes().to_vec();
            v.push(0);
            dar(&v, p, n)
        }
        PP_PROVTYPE => dar(&tipo.to_le_bytes(), p, n),
        _ => no(NTE_BAD_TYPE),
    }
}

/// `CryptEnumProvidersW(i, res, banderas, *tipo, nombre, *cb)`: `*cb` en
/// bytes, con el 0.
extern "win64" fn crypt_enum_providers_w(i: u32, _r: u64, _b: u32, tipo: *mut u32, p: *mut u16, n: *mut u32) -> i32 {
    let Some((nombre, t)) = PROVEEDORES.get(i as usize) else { return no(ERROR_NO_MORE_ITEMS) };
    poner32(tipo, *t);
    let v: Vec<u8> = nombre.encode_utf16().chain(core::iter::once(0)).flat_map(|c| c.to_le_bytes()).collect();
    dar(&v, p as *mut u8, n)
}

extern "win64" fn crypt_gen_random(h: u64, n: u32, p: *mut u8) -> i32 {
    if proveedor(h).is_none() {
        return no(NTE_BAD_UID);
    }
    if n > 0 && !p.is_null() {
        crate::sistema::process_prng(p, n as usize);
    }
    1
}

// -- CryptoAPI: los resumenes -------------------------------------------------------------------

const CALG_MD5: u32 = 0x8003;
const CALG_SHA1: u32 = 0x8004;
const CALG_SHA_256: u32 = 0x800C;

fn medida(alg: u32) -> usize {
    match alg {
        CALG_MD5 => 16,
        CALG_SHA1 => 20,
        _ => 32,
    }
}

extern "win64" fn crypt_create_hash(prov: u64, alg: u32, clave: u64, _b: u32, h: *mut u64) -> i32 {
    let Some((_, tipo)) = proveedor(prov) else { return no(NTE_BAD_UID) };
    if h.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    if clave != 0 {
        return no(NTE_BAD_KEY);
    }
    // SHA-256 es de los proveedores de AES (PROV_RSA_AES).
    let ok = matches!(alg, CALG_MD5 | CALG_SHA1) || (alg == CALG_SHA_256 && tipo == 24);
    if !ok {
        return no(NTE_BAD_ALGID);
    }
    let e = estado();
    e.siguiente += 1;
    let nuevo = 0x5B50_0000 + 4 * e.siguiente;
    e.resumenes.push(Resumen { h: nuevo, alg, datos: Vec::new(), fin: None });
    // SAFETY: el HCRYPTHASH del `.exe`.
    unsafe { *h = nuevo };
    1
}

fn resumen_de(h: u64) -> Option<&'static mut Resumen> {
    estado().resumenes.iter_mut().find(|r| r.h == h)
}

extern "win64" fn crypt_hash_data(h: u64, p: *const u8, n: u32, _b: u32) -> i32 {
    let Some(r) = resumen_de(h) else { return no(NTE_BAD_HASH) };
    if r.fin.is_some() {
        return no(NTE_BAD_HASH_STATE);
    }
    if n > 0 {
        if p.is_null() {
            return no(ERROR_INVALID_PARAMETER);
        }
        // SAFETY: `n` bytes del `.exe`.
        r.datos.extend_from_slice(unsafe { core::slice::from_raw_parts(p, n as usize) });
    }
    1
}

const HP_ALGID: u32 = 1;
const HP_HASHVAL: u32 = 2;
const HP_HASHSIZE: u32 = 4;

extern "win64" fn crypt_get_hash_param(h: u64, que: u32, p: *mut u8, n: *mut u32, _b: u32) -> i32 {
    let Some(r) = resumen_de(h) else { return no(NTE_BAD_HASH) };
    match que {
        HP_ALGID => dar(&r.alg.to_le_bytes(), p, n),
        HP_HASHSIZE => dar(&(medida(r.alg) as u32).to_le_bytes(), p, n),
        HP_HASHVAL => {
            // Pedir el valor lo cierra: despues, CryptHashData no vale.
            let v = r.fin.get_or_insert_with(|| match r.alg {
                CALG_MD5 => resumen::md5(&r.datos).to_vec(),
                CALG_SHA1 => resumen::sha1(&r.datos).to_vec(),
                _ => resumen::sha256(&r.datos).to_vec(),
            });
            dar(v, p, n)
        }
        _ => no(NTE_BAD_TYPE),
    }
}

extern "win64" fn crypt_set_hash_param(h: u64, que: u32, p: *const u8, _b: u32) -> i32 {
    let Some(r) = resumen_de(h) else { return no(NTE_BAD_HASH) };
    if que != HP_HASHVAL || p.is_null() {
        return no(NTE_BAD_TYPE);
    }
    // SAFETY: un resumen entero del `.exe`.
    r.fin = Some(unsafe { core::slice::from_raw_parts(p, medida(r.alg)) }.to_vec());
    1
}

extern "win64" fn crypt_destroy_hash(h: u64) -> i32 {
    let e = estado();
    match e.resumenes.iter().position(|r| r.h == h) {
        Some(i) => {
            e.resumenes.remove(i);
            1
        }
        None => no(NTE_BAD_HASH),
    }
}

// -- CryptoAPI: las claves (no hay) ---------------------------------------------------------------

extern "win64" fn crypt_get_user_key(prov: u64, _que: u32, _k: *mut u64) -> i32 {
    if proveedor(prov).is_none() {
        return no(NTE_BAD_UID);
    }
    no(NTE_NO_KEY)
}

extern "win64" fn crypt_import_key(prov: u64, _p: *const u8, _n: u32, _pub: u64, _b: u32, _k: *mut u64) -> i32 {
    if proveedor(prov).is_none() {
        return no(NTE_BAD_UID);
    }
    aviso("CryptImportKey: la casa no tiene claves todavia (ni RSA ni AES)");
    no(NTE_NOT_SUPPORTED)
}

/// Las que reciben un HCRYPTKEY: como nunca se da ninguno, NTE_BAD_KEY.
extern "win64" fn con_clave() -> i32 {
    no(NTE_BAD_KEY)
}

/// `CryptSignHashW(h, AT_*, ...)`: firma con la clave del contenedor, que
/// no hay.
extern "win64" fn crypt_sign_hash_w(h: u64, _que: u32, _d: u64, _b: u32, _f: *mut u8, _n: *mut u32) -> i32 {
    if resumen_de(h).is_none() {
        return no(NTE_BAD_HASH);
    }
    no(NTE_NO_KEY)
}

// -- Eventos -------------------------------------------------------------------------------------

const FUENTE: u64 = 0x5B60_0010;

extern "win64" fn register_event_source_w(_servidor: *const u16, fuente: *const u16) -> u64 {
    if fuente.is_null() {
        return no(ERROR_INVALID_PARAMETER) as u64;
    }
    FUENTE
}

extern "win64" fn deregister_event_source(h: u64) -> i32 {
    if h != FUENTE {
        return no(ERROR_INVALID_HANDLE);
    }
    1
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn report_event_w(h: u64, _t: u16, _c: u16, _id: u32, _sid: u64, _n: u16, _d: u32, _s: u64, _datos: u64) -> i32 {
    if h != FUENTE {
        return no(ERROR_INVALID_HANDLE);
    }
    1
}

/// ETW: un REGHANDLE por proveedor; nadie escucha, asi que escribir es 0.
extern "win64" fn event_register(guid: *const u8, _cb: u64, _ctx: u64, h: *mut u64) -> u32 {
    if guid.is_null() || h.is_null() {
        return ERROR_INVALID_PARAMETER;
    }
    let e = estado();
    e.siguiente += 1;
    // SAFETY: el REGHANDLE del `.exe`.
    unsafe { *h = 0x5B80_0000 + 4 * e.siguiente };
    0
}

extern "win64" fn event_cero() -> u32 {
    0
}

/// `EventActivityIdControl(codigo, *guid)`: el id de actividad del hilo.
extern "win64" fn event_activity_id_control(codigo: u32, g: *mut [u8; 16]) -> u32 {
    if g.is_null() || !(1..=5).contains(&codigo) {
        return ERROR_INVALID_PARAMETER;
    }
    let yo = kernel32::get_current_thread_id();
    let e = estado();
    let i = match e.actividades.iter().position(|a| a.0 == yo) {
        Some(i) => i,
        None => {
            e.actividades.push((yo, [0; 16]));
            e.actividades.len() - 1
        }
    };
    let nuevo = || {
        let mut v = [0u8; 16];
        crate::sistema::process_prng(v.as_mut_ptr(), 16);
        v
    };
    // SAFETY: el GUID del `.exe`.
    let dado = unsafe { &mut *g };
    let actual = &mut e.actividades[i].1;
    match codigo {
        1 => *dado = *actual,
        2 => *actual = *dado,
        3 => *dado = nuevo(),
        4 => core::mem::swap(actual, dado),
        _ => {
            *dado = *actual;
            *actual = nuevo();
        }
    }
    0
}

// -- Servicios -----------------------------------------------------------------------------------

const SCM: u64 = 0x5B70_0010;

extern "win64" fn open_sc_manager_a(maquina: *const u8, _bd: *const u8, _acceso: u32) -> u64 {
    if cadena_a(maquina).is_some_and(|v| !v.is_empty()) {
        return no(1722) as u64; // RPC_S_SERVER_UNAVAILABLE: sin red
    }
    SCM
}

extern "win64" fn open_service_a(scm: u64, nombre: *const u8, _acceso: u32) -> u64 {
    if scm != SCM {
        return no(ERROR_INVALID_HANDLE) as u64;
    }
    if nombre.is_null() {
        return no(ERROR_INVALID_PARAMETER) as u64;
    }
    no(ERROR_SERVICE_DOES_NOT_EXIST) as u64
}

extern "win64" fn close_service_handle(h: u64) -> i32 {
    if h != SCM {
        return no(ERROR_INVALID_HANDLE);
    }
    1
}

/// QueryServiceStatus, ControlService, StartServiceA: nunca hay un
/// SC_HANDLE de servicio.
extern "win64" fn sin_servicio() -> i32 {
    no(ERROR_INVALID_HANDLE)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "GetUserNameW" => dir!(get_user_name_w),
        "GetUserNameA" => dir!(get_user_name_a),
        "CryptAcquireContextW" => dir!(crypt_acquire_context_w),
        "CryptAcquireContextA" => dir!(crypt_acquire_context_a),
        "CryptReleaseContext" => dir!(crypt_release_context),
        "CryptGetProvParam" => dir!(crypt_get_prov_param),
        "CryptEnumProvidersW" => dir!(crypt_enum_providers_w),
        "CryptGenRandom" => dir!(crypt_gen_random),
        "CryptCreateHash" => dir!(crypt_create_hash),
        "CryptHashData" => dir!(crypt_hash_data),
        "CryptGetHashParam" => dir!(crypt_get_hash_param),
        "CryptSetHashParam" => dir!(crypt_set_hash_param),
        "CryptDestroyHash" => dir!(crypt_destroy_hash),
        "CryptGetUserKey" => dir!(crypt_get_user_key),
        "CryptImportKey" => dir!(crypt_import_key),
        "CryptExportKey" | "CryptEncrypt" | "CryptDecrypt" | "CryptDestroyKey" => dir!(con_clave),
        "CryptSignHashW" => dir!(crypt_sign_hash_w),
        "RegisterEventSourceW" => dir!(register_event_source_w),
        "DeregisterEventSource" => dir!(deregister_event_source),
        "ReportEventW" => dir!(report_event_w),
        "EventRegister" => dir!(event_register),
        "EventUnregister" | "EventSetInformation" | "EventWriteTransfer" => dir!(event_cero),
        "EventActivityIdControl" => dir!(event_activity_id_control),
        "OpenSCManagerA" => dir!(open_sc_manager_a),
        "OpenServiceA" => dir!(open_service_a),
        "CloseServiceHandle" => dir!(close_service_handle),
        "QueryServiceStatus" | "ControlService" | "StartServiceA" => dir!(sin_servicio),
        _ => return None,
    })
}
