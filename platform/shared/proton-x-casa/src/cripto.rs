//! **`crypt32.dll` y `bcrypt.dll` de la casa** (tanda 12 de Cyberpunk,
//! 30-09): la cripto de Windows, sin red.
//!
//! ```text
//!    crypt32   CryptBinaryToStringW CryptStringToBinaryA/W: Base64 (con
//!              cabecera o sin ella) y hex, de verdad
//!              CertOpenStore y los almacenes: VACIOS (la casa no tiene
//!              certificados: sin red no hay TLS que verificar), y lo que
//!              pide un certificado contesta que no hay
//!              CryptDecodeObjectEx CryptQueryObject PFXImportCertStore:
//!              la casa no lee ASN.1 todavia (lo dice con un aviso)
//!    bcrypt    BCryptOpenAlgorithmProvider BCryptGetProperty
//!              BCryptCreateHash BCryptHashData BCryptFinishHash
//!              BCryptDestroyHash: MD5, SHA-1 y SHA-256, y su HMAC, de
//!              verdad; BCryptGenRandom (RDRAND)
//!              las claves (AES, RSA, ECDH, ECDSA): no hay todavia; crearlas
//!              o importarlas lo dice con un aviso (STATUS_NOT_SUPPORTED)
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::direcciones::{base64, de_base64};
use bmo_proton_x::resumen;

use crate::{aviso, dir, kernel32};

const ERROR_INVALID_DATA: u32 = 13;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_MORE_DATA: u32 = 234;
const E_INVALIDARG: u32 = 0x8007_0057;
const CRYPT_E_NOT_FOUND: u32 = 0x8009_2004;
const CRYPT_E_NO_MATCH: u32 = 0x8009_2009;
const CRYPT_E_ASN1_BADTAG: u32 = 0x8009_310B;

const STATUS_INVALID_HANDLE: u32 = 0xC000_0008;
const STATUS_INVALID_PARAMETER: u32 = 0xC000_000D;
const STATUS_BUFFER_TOO_SMALL: u32 = 0xC000_0023;
const STATUS_NOT_SUPPORTED: u32 = 0xC000_00BB;
const STATUS_NOT_FOUND: u32 = 0xC000_0225;

#[derive(Clone, Copy, PartialEq)]
enum Alg {
    Md5,
    Sha1,
    Sha256,
    /// Se abre (Windows lo tiene), pero la casa no lo calcula todavia.
    OtroResumen,
    Rng,
    /// AES, RSA, ECDH, ECDSA: de claves.
    Claves,
}

struct Algoritmo {
    h: u64,
    alg: Alg,
    hmac: bool,
}

struct Resumen {
    h: u64,
    alg: Alg,
    clave: Option<Vec<u8>>,
    datos: Vec<u8>,
}

struct Estado {
    algoritmos: Vec<Algoritmo>,
    resumenes: Vec<Resumen>,
    almacenes: Vec<u64>,
    siguiente: u64,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { algoritmos: Vec::new(), resumenes: Vec::new(), almacenes: Vec::new(), siguiente: 0 }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.algoritmos.clear();
    e.resumenes.clear();
    e.almacenes.clear();
    e.siguiente = 0;
}

fn nuevo(base: u64) -> u64 {
    let e = estado();
    e.siguiente += 1;
    base + 4 * e.siguiente
}

fn no(e: u32) -> i32 {
    kernel32::poner_error(e);
    0
}

fn poner32(p: *mut u32, v: u32) {
    if !p.is_null() {
        // SAFETY: un DWORD/ULONG del `.exe`.
        unsafe { *p = v };
    }
}

// -- crypt32: Base64 y hex -----------------------------------------------------------------------

const CRYPT_STRING_BASE64HEADER: u32 = 0;
const CRYPT_STRING_BASE64: u32 = 1;
const CRYPT_STRING_BINARY: u32 = 2;
const CRYPT_STRING_BASE64REQUESTHEADER: u32 = 3;
const CRYPT_STRING_HEX: u32 = 4;
const CRYPT_STRING_BASE64_ANY: u32 = 6;
const CRYPT_STRING_ANY: u32 = 7;
const CRYPT_STRING_HEX_ANY: u32 = 8;
const CRYPT_STRING_BASE64X509CRLHEADER: u32 = 9;
const CRYPT_STRING_HEXRAW: u32 = 0xC;
const CRYPT_STRING_NOCRLF: u32 = 0x4000_0000;
const CRYPT_STRING_NOCR: u32 = 0x8000_0000;

fn hex_de(b: &[u8]) -> String {
    b.iter().map(|x| alloc::format!("{x:02x}")).collect()
}

/// El texto de `d` en el formato de `banderas` (sin su 0).
fn a_texto(d: &[u8], banderas: u32) -> Option<String> {
    let fin = if banderas & CRYPT_STRING_NOCRLF != 0 {
        ""
    } else if banderas & CRYPT_STRING_NOCR != 0 {
        "\n"
    } else {
        "\r\n"
    };
    // Base64 en lineas de 64, cada una con su fin.
    let en_lineas = |t: String| -> String {
        if fin.is_empty() {
            return t;
        }
        t.as_bytes().chunks(64).map(|l| alloc::format!("{}{fin}", core::str::from_utf8(l).unwrap_or(""))).collect()
    };
    Some(match banderas & 0xFFFF {
        CRYPT_STRING_BASE64 => en_lineas(base64(d)),
        CRYPT_STRING_BASE64HEADER => alloc::format!("-----BEGIN CERTIFICATE-----{fin}{}-----END CERTIFICATE-----{fin}", en_lineas(base64(d))),
        CRYPT_STRING_BASE64REQUESTHEADER => alloc::format!("-----BEGIN NEW CERTIFICATE REQUEST-----{fin}{}-----END NEW CERTIFICATE REQUEST-----{fin}", en_lineas(base64(d))),
        CRYPT_STRING_BASE64X509CRLHEADER => alloc::format!("-----BEGIN X509 CRL-----{fin}{}-----END X509 CRL-----{fin}", en_lineas(base64(d))),
        CRYPT_STRING_HEXRAW => alloc::format!("{}{fin}", hex_de(d)),
        // "de ad be ef ..." de 16 en 16, con un espacio de mas tras el 8.
        CRYPT_STRING_HEX => d
            .chunks(16)
            .map(|l| {
                let v: Vec<String> = l.iter().enumerate().map(|(i, x)| alloc::format!("{}{x:02x}", if i == 8 { " " } else { "" })).collect();
                alloc::format!("\t{}{fin}", v.join(" "))
            })
            .collect(),
        _ => return None,
    })
}

extern "win64" fn crypt_binary_to_string_w(d: *const u8, n: u32, banderas: u32, s: *mut u16, cch: *mut u32) -> i32 {
    if (d.is_null() && n > 0) || cch.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    // SAFETY: `n` bytes del `.exe`.
    let datos = if n == 0 { &[][..] } else { unsafe { core::slice::from_raw_parts(d, n as usize) } };
    let Some(t) = a_texto(datos, banderas) else { return no(ERROR_INVALID_PARAMETER) };
    let w: Vec<u16> = t.encode_utf16().collect();
    // SAFETY: el DWORD del `.exe`.
    let hay = unsafe { *cch } as usize;
    if s.is_null() {
        poner32(cch, w.len() as u32 + 1);
        return 1;
    }
    if hay <= w.len() {
        poner32(cch, w.len() as u32 + 1);
        return no(ERROR_MORE_DATA);
    }
    // SAFETY: `hay` WCHAR suyos.
    unsafe {
        core::ptr::copy_nonoverlapping(w.as_ptr(), s, w.len());
        *s.add(w.len()) = 0;
    }
    poner32(cch, w.len() as u32);
    1
}

/// Base64 sin las lineas "-----BEGIN ...-----" / "-----END ...-----".
fn sin_cabecera(t: &[u8]) -> Option<Vec<u8>> {
    let s = core::str::from_utf8(t).ok()?;
    let ini = s.find("-----BEGIN ")?;
    let tras = ini + s[ini..].find("-----\n").or_else(|| s[ini + 11..].find("-----").map(|k| k + 11))? + 5;
    let fin = s[tras..].find("-----END ")? + tras;
    de_base64(s[tras..fin].as_bytes())
}

fn de_hex(t: &[u8]) -> Option<Vec<u8>> {
    let d: Vec<u8> = t.iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
    if d.len() % 2 != 0 {
        return None;
    }
    d.chunks(2).map(|c| u8::from_str_radix(core::str::from_utf8(c).ok()?, 16).ok()).collect()
}

/// Los bytes de un texto en el formato de `banderas`; y el formato que era.
fn de_texto(t: &[u8], banderas: u32) -> Option<(Vec<u8>, u32)> {
    let probar = |f: u32| -> Option<(Vec<u8>, u32)> {
        match f {
            CRYPT_STRING_BASE64 => de_base64(t),
            CRYPT_STRING_BASE64HEADER | CRYPT_STRING_BASE64REQUESTHEADER | CRYPT_STRING_BASE64X509CRLHEADER => sin_cabecera(t),
            CRYPT_STRING_BINARY => Some(t.to_vec()),
            CRYPT_STRING_HEX | CRYPT_STRING_HEXRAW => de_hex(t),
            _ => None,
        }
        .map(|v| (v, f))
    };
    match banderas & 0xFFFF {
        CRYPT_STRING_BASE64_ANY => probar(CRYPT_STRING_BASE64HEADER).or_else(|| probar(CRYPT_STRING_BASE64)),
        CRYPT_STRING_ANY => probar(CRYPT_STRING_BASE64HEADER).or_else(|| probar(CRYPT_STRING_BASE64)).or_else(|| probar(CRYPT_STRING_BINARY)),
        CRYPT_STRING_HEX_ANY => probar(CRYPT_STRING_HEX),
        f => probar(f),
    }
}

fn string_to_binary(t: Vec<u8>, banderas: u32, d: *mut u8, cb: *mut u32, salto: *mut u32, usado: *mut u32) -> i32 {
    if cb.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    let Some((v, f)) = de_texto(&t, banderas) else { return no(ERROR_INVALID_DATA) };
    poner32(salto, 0);
    poner32(usado, f);
    // SAFETY: el DWORD del `.exe`.
    let hay = unsafe { *cb } as usize;
    poner32(cb, v.len() as u32);
    if d.is_null() {
        return 1;
    }
    if hay < v.len() {
        return no(ERROR_MORE_DATA);
    }
    // SAFETY: `hay` bytes suyos.
    unsafe { core::ptr::copy_nonoverlapping(v.as_ptr(), d, v.len()) };
    1
}

extern "win64" fn crypt_string_to_binary_a(s: *const u8, n: u32, banderas: u32, d: *mut u8, cb: *mut u32, salto: *mut u32, usado: *mut u32) -> i32 {
    if s.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    // SAFETY: `n` bytes del `.exe`, o hasta su 0.
    let t = if n == 0 { crate::crt::cadena_c(s as u64) } else { unsafe { core::slice::from_raw_parts(s, n as usize) }.to_vec() };
    string_to_binary(t, banderas, d, cb, salto, usado)
}

extern "win64" fn crypt_string_to_binary_w(s: *const u16, n: u32, banderas: u32, d: *mut u8, cb: *mut u32, salto: *mut u32, usado: *mut u32) -> i32 {
    if s.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    // SAFETY: `n` WCHAR del `.exe`, o hasta su 0.
    let w = if n == 0 { unsafe { crate::user32::utf16(s) } } else { unsafe { core::slice::from_raw_parts(s, n as usize) }.to_vec() };
    // Base64 y hex son ASCII: lo demas no vale.
    let t: Vec<u8> = w.iter().map(|&c| if c < 0x80 { c as u8 } else { b'!' }).collect();
    string_to_binary(t, banderas, d, cb, salto, usado)
}

// -- crypt32: los almacenes (vacios) -------------------------------------------------------------

extern "win64" fn cert_open_store(_prov: u64, _cod: u32, _p: u64, _b: u32, _para: u64) -> u64 {
    let h = nuevo(0x5BA0_0000);
    estado().almacenes.push(h);
    h
}

extern "win64" fn cert_close_store(h: u64, _b: u32) -> i32 {
    let e = estado();
    match e.almacenes.iter().position(|&x| x == h) {
        Some(i) => {
            e.almacenes.remove(i);
            1
        }
        None => no(E_INVALIDARG),
    }
}

/// Buscar en un almacen vacio: NULL y CRYPT_E_NOT_FOUND.
extern "win64" fn no_hay_certificado() -> u64 {
    no(CRYPT_E_NOT_FOUND) as u64
}

/// Lo que recibe un certificado: nunca hay ninguno de la casa.
extern "win64" fn sin_certificado() -> i32 {
    no(E_INVALIDARG)
}

extern "win64" fn cert_free_certificate_context(_c: u64) -> i32 {
    1
}

extern "win64" fn cert_duplicate_certificate_context(c: u64) -> u64 {
    c
}

/// `CertGetNameString`: sin nombre, la cadena vacia (1: solo su 0).
extern "win64" fn cert_get_name_string(_c: u64, _t: u32, _b: u32, _p: u64, s: *mut u16, n: u32) -> u32 {
    if !s.is_null() && n > 0 {
        // SAFETY: al menos un caracter suyo (W o A: un 0 de 2 bytes cabe
        // en el primero de W; en A, el primer byte ya es el 0).
        unsafe { *(s as *mut u8) = 0 };
    }
    1
}

extern "win64" fn cert_find_extension(_o: u64, _n: u32, _e: u64) -> u64 {
    0
}

const MOTOR: u64 = 0x5BA0_1000;

extern "win64" fn cert_create_certificate_chain_engine(_cfg: u64, motor: *mut u64) -> i32 {
    if motor.is_null() {
        return no(E_INVALIDARG);
    }
    // SAFETY: el HCERTCHAINENGINE del `.exe`.
    unsafe { *motor = MOTOR };
    1
}

extern "win64" fn nada(_a: u64) {}

extern "win64" fn crypt_decode_object_ex() -> i32 {
    aviso("CryptDecodeObjectEx: la casa no lee ASN.1 todavia");
    no(CRYPT_E_ASN1_BADTAG)
}

extern "win64" fn crypt_query_object() -> i32 {
    aviso("CryptQueryObject: la casa no lee certificados todavia");
    no(CRYPT_E_NO_MATCH)
}

/// `CryptMsgClose(hMsg)`: la casa no abre mensajes (CryptQueryObject y
/// CryptMsgOpenToDecode no dan ninguno), asi que no hay nada que cerrar; y
/// cerrar un NULL es valido en Windows. TRUE (03-10: Cyberpunk lo pedia por
/// GetProcAddress, la casa daba NULL y el juego saltaba a la direccion 0).
extern "win64" fn crypt_msg_close(_msg: u64) -> i32 {
    1
}

extern "win64" fn pfx_import_cert_store() -> u64 {
    aviso("PFXImportCertStore: la casa no lee PKCS#12 todavia");
    no(CRYPT_E_NO_MATCH) as u64
}

// -- bcrypt --------------------------------------------------------------------------------------

fn cadena_w(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: una cadena suya.
    let w = unsafe { crate::user32::utf16(p) };
    String::from_utf16_lossy(&w)
}

const BCRYPT_ALG_HANDLE_HMAC_FLAG: u32 = 8;

extern "win64" fn b_open_algorithm_provider(h: *mut u64, nombre: *const u16, _impl: *const u16, banderas: u32) -> u32 {
    if h.is_null() {
        return STATUS_INVALID_PARAMETER;
    }
    let alg = match cadena_w(nombre).as_str() {
        "MD5" => Alg::Md5,
        "SHA1" => Alg::Sha1,
        "SHA256" => Alg::Sha256,
        "SHA384" | "SHA512" | "MD4" | "MD2" => Alg::OtroResumen,
        "RNG" => Alg::Rng,
        "AES" | "RSA" | "ECDH_P256" | "ECDH_P384" | "ECDSA_P256" | "ECDSA_P384" | "DH" | "DSA" | "3DES" => Alg::Claves,
        _ => return STATUS_NOT_FOUND,
    };
    let hmac = banderas & BCRYPT_ALG_HANDLE_HMAC_FLAG != 0;
    if hmac && !matches!(alg, Alg::Md5 | Alg::Sha1 | Alg::Sha256 | Alg::OtroResumen) {
        return STATUS_NOT_FOUND;
    }
    let nuevo_h = nuevo(0x5BB0_0000);
    estado().algoritmos.push(Algoritmo { h: nuevo_h, alg, hmac });
    // SAFETY: el BCRYPT_ALG_HANDLE del `.exe`.
    unsafe { *h = nuevo_h };
    0
}

fn algoritmo(h: u64) -> Option<(Alg, bool)> {
    estado().algoritmos.iter().find(|a| a.h == h).map(|a| (a.alg, a.hmac))
}

extern "win64" fn b_close_algorithm_provider(h: u64, _b: u32) -> u32 {
    let e = estado();
    match e.algoritmos.iter().position(|a| a.h == h) {
        Some(i) => {
            e.algoritmos.remove(i);
            0
        }
        None => STATUS_INVALID_HANDLE,
    }
}

fn digesto(alg: Alg) -> u32 {
    match alg {
        Alg::Md5 => 16,
        Alg::Sha1 => 20,
        _ => 32,
    }
}

/// `BCryptGetProperty` de un algoritmo o de un resumen: las medidas.
extern "win64" fn b_get_property(h: u64, nombre: *const u16, p: *mut u8, cb: u32, res: *mut u32, _b: u32) -> u32 {
    let alg = algoritmo(h).map(|a| a.0).or_else(|| estado().resumenes.iter().find(|r| r.h == h).map(|r| r.alg));
    let Some(alg) = alg else { return STATUS_INVALID_HANDLE };
    let es_resumen = matches!(alg, Alg::Md5 | Alg::Sha1 | Alg::Sha256);
    let v: u32 = match (cadena_w(nombre).as_str(), es_resumen) {
        ("HashDigestLength", true) => digesto(alg),
        ("HashBlockLength", true) => 64,
        ("ObjectLength", true) => 0x200,
        ("BlockLength", false) if alg == Alg::Claves => 16,
        _ => return STATUS_NOT_SUPPORTED,
    };
    poner32(res, 4);
    if p.is_null() || cb < 4 {
        return if p.is_null() { 0 } else { STATUS_BUFFER_TOO_SMALL };
    }
    // SAFETY: 4 bytes del `.exe`.
    unsafe { (p as *mut u32).write_unaligned(v) };
    0
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn b_create_hash(h: u64, hh: *mut u64, _obj: *mut u8, _nobj: u32, secreto: *const u8, nsec: u32, _b: u32) -> u32 {
    let Some((alg, hmac)) = algoritmo(h) else { return STATUS_INVALID_HANDLE };
    if hh.is_null() {
        return STATUS_INVALID_PARAMETER;
    }
    match alg {
        Alg::Md5 | Alg::Sha1 | Alg::Sha256 => {}
        Alg::OtroResumen => {
            aviso("BCryptCreateHash: la casa calcula MD5, SHA-1 y SHA-256; este todavia no");
            return STATUS_NOT_SUPPORTED;
        }
        _ => return STATUS_INVALID_HANDLE,
    }
    // SAFETY: `nsec` bytes del `.exe`.
    let clave = hmac.then(|| if secreto.is_null() { Vec::new() } else { unsafe { core::slice::from_raw_parts(secreto, nsec as usize) }.to_vec() });
    let n = nuevo(0x5BC0_0000);
    estado().resumenes.push(Resumen { h: n, alg, clave, datos: Vec::new() });
    // SAFETY: el BCRYPT_HASH_HANDLE del `.exe`.
    unsafe { *hh = n };
    0
}

extern "win64" fn b_hash_data(hh: u64, p: *const u8, n: u32, _b: u32) -> u32 {
    let Some(r) = estado().resumenes.iter_mut().find(|r| r.h == hh) else { return STATUS_INVALID_HANDLE };
    if n > 0 {
        if p.is_null() {
            return STATUS_INVALID_PARAMETER;
        }
        // SAFETY: `n` bytes del `.exe`.
        r.datos.extend_from_slice(unsafe { core::slice::from_raw_parts(p, n as usize) });
    }
    0
}

fn calcular(alg: Alg, d: &[u8]) -> Vec<u8> {
    match alg {
        Alg::Md5 => resumen::md5(d).to_vec(),
        Alg::Sha1 => resumen::sha1(d).to_vec(),
        _ => resumen::sha256(d).to_vec(),
    }
}

extern "win64" fn b_finish_hash(hh: u64, p: *mut u8, n: u32, _b: u32) -> u32 {
    let Some(r) = estado().resumenes.iter_mut().find(|r| r.h == hh) else { return STATUS_INVALID_HANDLE };
    if p.is_null() || n != digesto(r.alg) {
        return STATUS_INVALID_PARAMETER;
    }
    let alg = r.alg;
    let f: fn(&[u8]) -> Vec<u8> = match alg {
        Alg::Md5 => |d| calcular(Alg::Md5, d),
        Alg::Sha1 => |d| calcular(Alg::Sha1, d),
        _ => |d| calcular(Alg::Sha256, d),
    };
    let v = match &r.clave {
        Some(k) => resumen::hmac(f, k, &r.datos),
        None => f(&r.datos),
    };
    r.datos.clear();
    // SAFETY: `n` bytes del `.exe` (la medida del resumen).
    unsafe { core::ptr::copy_nonoverlapping(v.as_ptr(), p, v.len()) };
    0
}

extern "win64" fn b_destroy_hash(hh: u64) -> u32 {
    let e = estado();
    match e.resumenes.iter().position(|r| r.h == hh) {
        Some(i) => {
            e.resumenes.remove(i);
            0
        }
        None => STATUS_INVALID_HANDLE,
    }
}

const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 2;

extern "win64" fn b_gen_random(h: u64, p: *mut u8, n: u32, banderas: u32) -> u32 {
    let ok = if h == 0 { banderas & BCRYPT_USE_SYSTEM_PREFERRED_RNG != 0 } else { algoritmo(h).is_some() };
    if !ok {
        return STATUS_INVALID_HANDLE;
    }
    if n > 0 {
        if p.is_null() {
            return STATUS_INVALID_PARAMETER;
        }
        crate::sistema::process_prng(p, n as usize);
    }
    0
}

/// Crear o importar una clave: no hay todavia.
extern "win64" fn b_sin_claves(h: u64) -> u32 {
    if algoritmo(h).is_none() {
        return STATUS_INVALID_HANDLE;
    }
    aviso("BCrypt: la casa no tiene claves todavia (AES, RSA, ECDH, ECDSA)");
    STATUS_NOT_SUPPORTED
}

/// Lo que recibe un BCRYPT_KEY_HANDLE (o un secreto): nunca hay ninguno.
extern "win64" fn b_con_clave() -> u32 {
    STATUS_INVALID_HANDLE
}

pub(crate) fn buscar_crypt32(n: &str) -> Option<u64> {
    Some(match n {
        "CryptBinaryToStringW" => dir!(crypt_binary_to_string_w),
        "CryptStringToBinaryA" => dir!(crypt_string_to_binary_a),
        "CryptStringToBinaryW" => dir!(crypt_string_to_binary_w),
        "CertOpenStore" => dir!(cert_open_store),
        "CertCloseStore" => dir!(cert_close_store),
        "CertEnumCertificatesInStore" | "CertFindCertificateInStore" => dir!(no_hay_certificado),
        "CertAddCertificateContextToStore" | "CertGetCertificateContextProperty" | "CertGetCertificateChain" => dir!(sin_certificado),
        "CertFreeCertificateContext" => dir!(cert_free_certificate_context),
        "CertDuplicateCertificateContext" => dir!(cert_duplicate_certificate_context),
        "CertGetNameStringA" | "CertGetNameStringW" => dir!(cert_get_name_string),
        "CertFindExtension" => dir!(cert_find_extension),
        "CertCreateCertificateChainEngine" => dir!(cert_create_certificate_chain_engine),
        "CertFreeCertificateChainEngine" | "CertFreeCertificateChain" => dir!(nada),
        "CryptDecodeObjectEx" => dir!(crypt_decode_object_ex),
        "CryptQueryObject" => dir!(crypt_query_object),
        "CryptMsgClose" => dir!(crypt_msg_close),
        "PFXImportCertStore" => dir!(pfx_import_cert_store),
        _ => return None,
    })
}

pub(crate) fn buscar_bcrypt(n: &str) -> Option<u64> {
    Some(match n {
        "BCryptOpenAlgorithmProvider" => dir!(b_open_algorithm_provider),
        "BCryptCloseAlgorithmProvider" => dir!(b_close_algorithm_provider),
        "BCryptGetProperty" => dir!(b_get_property),
        "BCryptCreateHash" => dir!(b_create_hash),
        "BCryptHashData" => dir!(b_hash_data),
        "BCryptFinishHash" => dir!(b_finish_hash),
        "BCryptDestroyHash" => dir!(b_destroy_hash),
        "BCryptGenRandom" => dir!(b_gen_random),
        "BCryptGenerateSymmetricKey" | "BCryptImportKeyPair" | "BCryptGenerateKeyPair" => dir!(b_sin_claves),
        "BCryptFinalizeKeyPair" | "BCryptExportKey" | "BCryptEncrypt" | "BCryptDecrypt" | "BCryptSignHash" | "BCryptVerifySignature" | "BCryptSecretAgreement" | "BCryptDeriveKey" | "BCryptDestroyKey"
        | "BCryptDestroySecret" => dir!(b_con_clave),
        _ => return None,
    })
}
