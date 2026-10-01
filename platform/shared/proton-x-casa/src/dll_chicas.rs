//! **Las DLL chicas del censo** (tanda 14a de Cyberpunk, 30-09): lo que el
//! juego importa de DLL de las que pide una, dos o cuatro funciones. Son
//! DURAS: sin ellas el cargador no arranca.
//!
//! ```text
//!    winmm      timeGetTime, timeBeginPeriod/EndPeriod, timeGetDevCaps
//!    shlwapi    PathFileExistsW, PathRemoveFileSpecW
//!    shell32    SHGetFolderPathW, SHGetSpecialFolderPathW,
//!               SHGetKnownFolderPath (las carpetas, del entorno), ShellExecuteA
//!    ntdll      NtQueryInformationProcess, RtlRunOnceExecuteOnce,
//!               RtlUTF8ToUnicodeN (y RtlUnwind, que ya era de kernel32)
//!    gdi32      GetStockObject
//!    powrprof   CallNtPowerInformation (los MHz y la bateria)
//!    normaliz   IdnToAscii (Punycode, RFC 3492)
//!    ole32      CoTaskMemAlloc/Realloc/Free, CoCreateGuid, StringFromGUID2
//!    rpcrt4     UuidCreate(Sequential)
//!    (el ETW de advapi32 ya era de la tanda 11: aqui se enruta su API set)
//!    sin red    InternetGetConnectedState (no), IcmpCreateFile/SendEcho,
//!               AcceptEx, GetAcceptExSockaddrs
//!    xinput     XInputGetState y compania: no hay mando (BMO-X no tiene XInput)
//! ```

use alloc::vec::Vec;

use crate::kernel32_a::w;
use crate::{aviso, dir, hilos, kernel32, memoria, proceso};

const S_OK: u32 = 0;
const E_INVALIDARG: u32 = 0x8007_0057;
const E_FILE_NOT_FOUND: u32 = 0x8007_0002;
const STATUS_INFO_LENGTH_MISMATCH: u32 = 0xC000_0004;
const STATUS_INVALID_INFO_CLASS: u32 = 0xC000_0003;
const STATUS_BUFFER_TOO_SMALL: u32 = 0xC000_0023;
const STATUS_INVALID_PARAMETER: u32 = 0xC000_000D;
const STATUS_UNSUCCESSFUL: u32 = 0xC000_0001;
const STATUS_PORT_NOT_SET: u32 = 0xC000_0353;
const STATUS_SOME_NOT_MAPPED: u32 = 0x0000_0107;
const TIMERR_NOCANDO: u32 = 97;
const ERROR_DEVICE_NOT_CONNECTED: u32 = 1167;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_INVALID_PARAMETER: u32 = 87;

// -- winmm ------------------------------------------------------------------------------

/// `timeGetTime`: milisegundos desde el arranque (el mismo reloj que
/// GetTickCount).
extern "win64" fn time_get_time() -> u32 {
    (hilos::ahora_ns() / 1_000_000) as u32
}

/// `timeBeginPeriod`/`timeEndPeriod`: de 1 a 1.000.000 ms, si; aqui el reloj
/// ya va al milisegundo.
extern "win64" fn time_period(p: u32) -> u32 {
    if (1..=1_000_000).contains(&p) {
        0
    } else {
        TIMERR_NOCANDO
    }
}

/// `timeGetDevCaps`: TIMECAPS { wPeriodMin 1, wPeriodMax 1.000.000 }.
extern "win64" fn time_get_dev_caps(t: *mut u32, cb: u32) -> u32 {
    if t.is_null() || cb < 8 {
        return TIMERR_NOCANDO;
    }
    // SAFETY: un TIMECAPS suyo de 8 bytes.
    unsafe {
        t.write(1);
        t.add(1).write(1_000_000);
    }
    0
}

// -- shlwapi ----------------------------------------------------------------------------

/// Una cadena UTF-16 del `.exe` hasta su cero.
///
/// # Safety
/// `p` apunta a UTF-16 terminado en cero.
unsafe fn cadena(p: *const u16) -> Vec<u16> {
    let mut v = Vec::new();
    while v.len() < 32_768 {
        let c = p.add(v.len()).read();
        if c == 0 {
            break;
        }
        v.push(c);
    }
    v
}

extern "win64" fn path_file_exists_w(p: *const u16) -> i32 {
    if p.is_null() {
        return 0;
    }
    (w::<extern "win64" fn(*const u16) -> u32>("GetFileAttributesW")(p) != u32::MAX) as i32
}

/// `PathRemoveFileSpecW`: quita el ultimo nombre y su barra; la barra de la
/// raiz (`C:\`, `\`) se queda. TRUE si quito algo.
extern "win64" fn path_remove_file_spec_w(p: *mut u16) -> i32 {
    if p.is_null() {
        return 0;
    }
    // SAFETY: una cadena suya.
    let s = unsafe { cadena(p) };
    let barra = b'\\' as u16;
    let unidad = s.len() >= 2 && s[1] == b':' as u16;
    let corte = match s.iter().rposition(|&c| c == barra) {
        // La barra de la raiz se queda.
        Some(i) if i == 0 || (i == 2 && unidad) => i + 1,
        Some(i) => i,
        None if unidad => 2,
        None => 0,
    };
    if corte >= s.len() {
        return 0;
    }
    // SAFETY: `corte` < s.len(): dentro de su cadena.
    unsafe { p.add(corte).write(0) };
    1
}

// -- shell32: las carpetas ----------------------------------------------------------------

/// El perfil, del entorno (USERPROFILE), sin barra al final.
fn perfil() -> Vec<u16> {
    let mut p = proceso::variable("USERPROFILE").unwrap_or_else(|| "C:\\".encode_utf16().collect());
    while p.len() > 3 && p.last() == Some(&(b'\\' as u16)) {
        p.pop();
    }
    p
}

fn debajo(base: Vec<u16>, resto: &str) -> Vec<u16> {
    let mut v = base;
    v.extend(resto.encode_utf16());
    v
}

/// Una carpeta de Windows por su CSIDL (lo de abajo, sin las banderas).
fn carpeta_csidl(c: u32) -> Option<Vec<u16>> {
    Some(match c & 0xFF {
        0x1A => proceso::variable("APPDATA").unwrap_or_else(|| debajo(perfil(), "\\AppData\\Roaming")),
        0x1C => proceso::variable("LOCALAPPDATA").unwrap_or_else(|| debajo(perfil(), "\\AppData\\Local")),
        0x28 => perfil(),
        0x05 => debajo(perfil(), "\\Documents"),
        0x10 | 0x00 => debajo(perfil(), "\\Desktop"),
        0x23 => "C:\\ProgramData".encode_utf16().collect(),
        0x24 => "C:\\Windows".encode_utf16().collect(),
        0x25 => "C:\\Windows\\System32".encode_utf16().collect(),
        _ => return None,
    })
}

/// Las carpetas conocidas (KNOWNFOLDERID) que se piden, con su CSIDL o su
/// ruta bajo el perfil.
fn carpeta_guid(g: &[u8; 16]) -> Option<Vec<u16>> {
    const CONOCIDAS: [(u32, u16, u16, [u8; 8], u32); 7] = [
        (0x3EB6_85DB, 0x65F9, 0x4CF6, [0xA0, 0x3A, 0xE3, 0xEF, 0x65, 0x72, 0x9F, 0x3D], 0x1A), // RoamingAppData
        (0xF1B3_2785, 0x6FBA, 0x4FCF, [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91], 0x1C), // LocalAppData
        (0x5E6C_858F, 0x0E22, 0x4760, [0x9A, 0xFE, 0xEA, 0x33, 0x17, 0xB6, 0x71, 0x73], 0x28), // Profile
        (0xFDD3_9AD0, 0x238F, 0x46AF, [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7], 0x05), // Documents
        (0x62AB_5D82, 0xFDC1, 0x4DC3, [0xA9, 0xDD, 0x07, 0x0D, 0x1D, 0x49, 0x5D, 0x97], 0x23), // ProgramData
        (0xB4BF_CC3A, 0xDB2C, 0x424C, [0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41], 0x10), // Desktop
        (0x4C5C_32FF, 0xBB9D, 0x43B0, [0xB5, 0xB4, 0x2D, 0x72, 0xE5, 0x4E, 0xAA, 0xA4], 0x100), // SavedGames
    ];
    let d1 = u32::from_le_bytes([g[0], g[1], g[2], g[3]]);
    let d2 = u16::from_le_bytes([g[4], g[5]]);
    let d3 = u16::from_le_bytes([g[6], g[7]]);
    let (_, _, _, _, c) = CONOCIDAS.iter().find(|k| k.0 == d1 && k.1 == d2 && k.2 == d3 && k.3 == g[8..16])?;
    if *c == 0x100 {
        return Some(debajo(perfil(), "\\Saved Games"));
    }
    carpeta_csidl(*c)
}

/// Copiar una ruta a un bufer de MAX_PATH (260) caracteres, con su cero.
fn dar_ruta(r: &[u16], buf: *mut u16) -> bool {
    if buf.is_null() || r.len() >= 260 {
        return false;
    }
    // SAFETY: MAX_PATH caracteres del `.exe`.
    unsafe {
        core::ptr::copy_nonoverlapping(r.as_ptr(), buf, r.len());
        buf.add(r.len()).write(0);
    }
    true
}

extern "win64" fn sh_get_folder_path_w(_h: u64, csidl: i32, _token: u64, _banderas: u32, buf: *mut u16) -> u32 {
    match carpeta_csidl(csidl as u32) {
        Some(r) if dar_ruta(&r, buf) => S_OK,
        Some(_) => E_INVALIDARG,
        None => {
            aviso("SHGetFolderPathW de una carpeta que la casa no sabe");
            E_INVALIDARG
        }
    }
}

extern "win64" fn sh_get_special_folder_path_w(_h: u64, buf: *mut u16, csidl: i32, _crear: i32) -> i32 {
    (sh_get_folder_path_w(0, csidl, 0, 0, buf) == S_OK) as i32
}

/// `SHGetKnownFolderPath`: la ruta en un bloque de CoTaskMemAlloc, que el
/// `.exe` suelta con CoTaskMemFree.
extern "win64" fn sh_get_known_folder_path(id: *const u8, _banderas: u32, _token: u64, sale: *mut u64) -> u32 {
    if id.is_null() || sale.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un GUID suyo (16 bytes) y un puntero suyo.
    let g: [u8; 16] = unsafe { core::ptr::read_unaligned(id as *const [u8; 16]) };
    unsafe { sale.write(0) };
    let Some(r) = carpeta_guid(&g) else {
        aviso("SHGetKnownFolderPath de una carpeta que la casa no sabe");
        crate::diario::nota(&alloc::format!("SHGetKnownFolderPath({g:02x?}): no la sabe"));
        return E_FILE_NOT_FOUND;
    };
    crate::diario::nota(&alloc::format!("SHGetKnownFolderPath: \"{}\"", alloc::string::String::from_utf16_lossy(&r)));
    let p = co_task_mem_alloc(((r.len() + 1) * 2) as u64);
    if p == 0 {
        return 0x8007_000E; // E_OUTOFMEMORY
    }
    // SAFETY: un bloque recien pedido de r.len() + 1 caracteres.
    unsafe {
        core::ptr::copy_nonoverlapping(r.as_ptr(), p as *mut u16, r.len());
        (p as *mut u16).add(r.len()).write(0);
        sale.write(p);
    }
    S_OK
}

/// `ShellExecuteA`: BMO-X no abre otros programas (un enlace, una carpeta).
/// SE_ERR_NOASSOC (31): nada sabe abrirlo.
extern "win64" fn shell_execute_a(_h: u64, _op: *const u8, _f: *const u8, _p: *const u8, _d: *const u8, _mostrar: i32) -> u64 {
    aviso("ShellExecuteA: BMO-X no abre otros programas");
    31
}

// -- ntdll ------------------------------------------------------------------------------

/// El PEB del proceso: `gs:[0x60]`, como en Windows.
fn peb() -> u64 {
    let p: u64;
    // SAFETY: el GS apunta al TEB del hilo (lo pone quien carga).
    unsafe { core::arch::asm!("mov {}, gs:[0x60]", out(reg) p, options(nostack, readonly)) };
    p
}

/// `NtQueryInformationProcess`: la basica (0), el puerto de depuracion (7:
/// no hay), WOW64 (26: no), el objeto de depuracion (30: no hay) y las
/// banderas de depuracion (31: 1, no se depura).
extern "win64" fn nt_query_information_process(_h: u64, clase: u32, buf: *mut u8, n: u32, sale: *mut u32) -> u32 {
    let poner = |bytes: &[u8]| -> u32 {
        if !sale.is_null() {
            // SAFETY: un ULONG suyo.
            unsafe { sale.write(bytes.len() as u32) };
        }
        if n as usize != bytes.len() || buf.is_null() {
            return STATUS_INFO_LENGTH_MISMATCH;
        }
        // SAFETY: `n` bytes suyos.
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len()) };
        0
    };
    match clase {
        0 => {
            let mut b = [0u8; 48];
            b[0..4].copy_from_slice(&0x103u32.to_le_bytes()); // STATUS_PENDING: vivo
            b[8..16].copy_from_slice(&peb().to_le_bytes());
            b[16..24].copy_from_slice(&1u64.to_le_bytes());
            b[24..28].copy_from_slice(&8u32.to_le_bytes());
            b[32..40].copy_from_slice(&(kernel32::id_del_proceso() as u64).to_le_bytes());
            poner(&b)
        }
        7 | 26 => poner(&0u64.to_le_bytes()),
        30 => {
            let r = poner(&0u64.to_le_bytes());
            if r == 0 {
                STATUS_PORT_NOT_SET
            } else {
                r
            }
        }
        31 => poner(&1u32.to_le_bytes()),
        _ => {
            aviso("NtQueryInformationProcess de una clase que la casa no sabe");
            STATUS_INVALID_INFO_CLASS
        }
    }
}

/// `RtlRunOnceExecuteOnce`: InitOnceExecuteOnce con NTSTATUS (la funcion
/// de inicio tiene la misma firma y el mismo "distinto de cero es exito").
extern "win64" fn rtl_run_once_execute_once(una: u64, f: u64, param: u64, ctx: u64) -> u32 {
    let hecho = w::<extern "win64" fn(u64, u64, u64, u64) -> i32>("InitOnceExecuteOnce")(una, f, param, ctx);
    if hecho != 0 {
        0
    } else {
        STATUS_UNSUCCESSFUL
    }
}

/// UTF-8 a UTF-16, con U+FFFD en lo que no es UTF-8 valido. Devuelve las
/// unidades y si hubo algo sin traducir.
pub(crate) fn utf8_a_utf16(s: &[u8]) -> (Vec<u16>, bool) {
    let mut v = Vec::with_capacity(s.len());
    let mut malo = false;
    let mut i = 0;
    while i < s.len() {
        let b = s[i];
        let (n, min, base) = match b {
            0x00..=0x7F => (1, 0, b as u32),
            0xC2..=0xDF => (2, 0x80, (b & 0x1F) as u32),
            0xE0..=0xEF => (3, 0x800, (b & 0x0F) as u32),
            0xF0..=0xF4 => (4, 0x1_0000, (b & 0x07) as u32),
            _ => (0, 0, 0),
        };
        let seguidas = s.get(i + 1..i + n.max(1)).is_some_and(|t| t.iter().all(|&c| c & 0xC0 == 0x80));
        if n == 0 || !seguidas {
            v.push(0xFFFD);
            malo = true;
            i += 1;
            continue;
        }
        let c = s[i + 1..i + n].iter().fold(base, |a, &c| a << 6 | (c & 0x3F) as u32);
        if c < min || c > 0x10_FFFF || (0xD800..=0xDFFF).contains(&c) {
            v.push(0xFFFD);
            malo = true;
        } else if c >= 0x1_0000 {
            v.push((0xD800 + ((c - 0x1_0000) >> 10)) as u16);
            v.push((0xDC00 + ((c - 0x1_0000) & 0x3FF)) as u16);
        } else {
            v.push(c as u16);
        }
        i += n;
    }
    (v, malo)
}

/// `RtlUTF8ToUnicodeN(destino, bytes de destino, *bytes escritos, origen,
/// bytes de origen)`. Sin destino: cuantos bytes harian falta.
extern "win64" fn rtl_utf8_to_unicode_n(d: *mut u16, max: u32, sale: *mut u32, s: *const u8, n: u32) -> u32 {
    if sale.is_null() || (s.is_null() && n > 0) {
        return STATUS_INVALID_PARAMETER;
    }
    // SAFETY: `n` bytes del `.exe`.
    let o = if n == 0 { &[][..] } else { unsafe { core::slice::from_raw_parts(s, n as usize) } };
    let (v, malo) = utf8_a_utf16(o);
    let bien = if malo { STATUS_SOME_NOT_MAPPED } else { 0 };
    if d.is_null() {
        // SAFETY: un ULONG suyo.
        unsafe { sale.write(v.len() as u32 * 2) };
        return bien;
    }
    let caben = (max / 2) as usize;
    let k = v.len().min(caben);
    // SAFETY: `max` bytes del `.exe`; un ULONG suyo.
    unsafe {
        core::ptr::copy_nonoverlapping(v.as_ptr(), d, k);
        sale.write(k as u32 * 2);
    }
    if k < v.len() {
        STATUS_BUFFER_TOO_SMALL
    } else {
        bien
    }
}

// -- gdi32, powrprof, wininet -------------------------------------------------------------

/// `GetStockObject`: un handle fijo por objeto de 0 a 19 (el 9 no existe).
extern "win64" fn get_stock_object(i: i32) -> u64 {
    if (0..=19).contains(&i) && i != 9 {
        0x5A1E_5000 + i as u64
    } else {
        0
    }
}

/// `CallNtPowerInformation`: ProcessorInformation (11), un
/// PROCESSOR_POWER_INFORMATION de 24 bytes por procesador (los de
/// GetSystemInfo), a los MHz del 5600X; SystemBatteryState (5): enchufado y
/// sin bateria.
extern "win64" fn call_nt_power_information(nivel: i32, _e: *const u8, _en: u32, s: *mut u8, sn: u32) -> u32 {
    let datos: Vec<u8> = match nivel {
        11 => {
            let mut si = [0u8; 48];
            w::<extern "win64" fn(*mut u8)>("GetSystemInfo")(si.as_mut_ptr());
            let cpus = u32::from_le_bytes([si[32], si[33], si[34], si[35]]).max(1);
            (0..cpus).flat_map(|k| [k, 3700, 3700, 3700, 0, 0].into_iter().flat_map(u32::to_le_bytes)).collect()
        }
        5 => {
            let mut b = alloc::vec![0u8; 32];
            b[0] = 1; // AcOnLine
            b
        }
        _ => {
            aviso("CallNtPowerInformation de un nivel que la casa no sabe");
            return STATUS_INVALID_PARAMETER;
        }
    };
    if s.is_null() || (sn as usize) < datos.len() {
        return STATUS_BUFFER_TOO_SMALL;
    }
    // SAFETY: `sn` bytes suyos.
    unsafe { core::ptr::copy_nonoverlapping(datos.as_ptr(), s, datos.len()) };
    0
}

/// `InternetGetConnectedState`: sin red (BMO-X no da red a los `.exe`
/// todavia): FALSE e INTERNET_CONNECTION_OFFLINE.
extern "win64" fn internet_get_connected_state(banderas: *mut u32, _r: u32) -> i32 {
    if !banderas.is_null() {
        // SAFETY: un DWORD suyo.
        unsafe { banderas.write(0x20) };
    }
    0
}

// -- normaliz: Punycode -------------------------------------------------------------------

/// **Punycode** de una etiqueta (RFC 3492), sin el `xn--`.
pub(crate) fn punycode(entrada: &[u32]) -> Option<Vec<u8>> {
    const BASE: u32 = 36;
    const TMIN: u32 = 1;
    const TMAX: u32 = 26;
    let digito = |d: u32| -> u8 { if d < 26 { b'a' + d as u8 } else { b'0' + (d - 26) as u8 } };
    let adaptar = |mut delta: u32, n: u32, primera: bool| -> u32 {
        delta /= if primera { 700 } else { 2 };
        delta += delta / n;
        let mut k = 0;
        while delta > ((BASE - TMIN) * TMAX) / 2 {
            delta /= BASE - TMIN;
            k += BASE;
        }
        k + (BASE - TMIN + 1) * delta / (delta + 38)
    };
    let mut sale: Vec<u8> = entrada.iter().filter(|&&c| c < 0x80).map(|&c| c as u8).collect();
    let basicos = sale.len() as u32;
    let mut h = basicos;
    if basicos > 0 {
        sale.push(b'-');
    }
    let (mut n, mut delta, mut sesgo) = (0x80u32, 0u32, 72u32);
    while (h as usize) < entrada.len() {
        let m = *entrada.iter().filter(|&&c| c >= n).min()?;
        delta = delta.checked_add((m - n).checked_mul(h + 1)?)?;
        n = m;
        for &c in entrada {
            if c < n {
                delta = delta.checked_add(1)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = if k <= sesgo { TMIN } else if k >= sesgo + TMAX { TMAX } else { k - sesgo };
                    if q < t {
                        break;
                    }
                    sale.push(digito(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                sale.push(digito(q));
                sesgo = adaptar(delta, h + 1, h == basicos);
                delta = 0;
                h += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    Some(sale)
}

/// Un nombre de dominio a ASCII: cada etiqueta con algo que no es ASCII, a
/// `xn--` y Punycode (las mayusculas de ASCII, a minusculas: nameprep).
pub(crate) fn idn_a_ascii(nombre: &[u16]) -> Option<Vec<u16>> {
    let mut sale: Vec<u16> = Vec::new();
    for (k, etiqueta) in nombre.split(|&c| c == b'.' as u16).enumerate() {
        if k > 0 {
            sale.push(b'.' as u16);
        }
        let cs: Vec<u32> = char::decode_utf16(etiqueta.iter().copied()).map(|c| c.map(|c| c.to_lowercase().next().unwrap_or(c) as u32)).collect::<Result<_, _>>().ok()?;
        if cs.iter().all(|&c| c < 0x80) {
            sale.extend(cs.iter().map(|&c| c as u16));
        } else {
            sale.extend("xn--".encode_utf16());
            sale.extend(punycode(&cs)?.iter().map(|&b| b as u16));
        }
    }
    Some(sale)
}

/// `IdnToAscii(banderas, origen, largo (-1: hasta el 0), destino, largo)`:
/// los caracteres escritos (con el 0 si el origen lo llevaba); sin destino,
/// los que harian falta.
extern "win64" fn idn_to_ascii(_banderas: u32, s: *const u16, n: i32, d: *mut u16, dn: i32) -> i32 {
    if s.is_null() || n == 0 || n < -1 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    // SAFETY: la cadena suya, hasta su cero o de `n` caracteres.
    let o = if n == -1 { unsafe { cadena(s) } } else { unsafe { core::slice::from_raw_parts(s, n as usize) }.to_vec() };
    let Some(mut r) = idn_a_ascii(&o) else {
        kernel32::poner_error(1113); // ERROR_NO_UNICODE_TRANSLATION
        return 0;
    };
    if n == -1 {
        r.push(0);
    }
    if dn == 0 {
        return r.len() as i32;
    }
    if d.is_null() || (dn as usize) < r.len() {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    // SAFETY: `dn` caracteres suyos.
    unsafe { core::ptr::copy_nonoverlapping(r.as_ptr(), d, r.len()) };
    r.len() as i32
}

// -- ole32 y rpcrt4: memoria de COM y GUID ------------------------------------------------

pub(crate) extern "win64" fn co_task_mem_alloc(n: u64) -> u64 {
    memoria::pedir_del_proceso(n.max(1)).unwrap_or(0)
}

extern "win64" fn co_task_mem_realloc(p: u64, n: u64) -> u64 {
    if p == 0 {
        return co_task_mem_alloc(n);
    }
    memoria::cambiar_del_proceso(p, n.max(1)).unwrap_or(0)
}

pub(crate) extern "win64" fn co_task_mem_free(p: u64) {
    if p != 0 {
        memoria::soltar_del_proceso(p);
    }
}

/// Un GUID nuevo, al azar (version 4, variante RFC 4122), como los de Windows.
pub(crate) fn guid_nuevo(g: *mut u8) {
    crate::sistema::process_prng(g, 16);
    // SAFETY: 16 bytes suyos.
    unsafe {
        *g.add(7) = (*g.add(7) & 0x0F) | 0x40;
        *g.add(8) = (*g.add(8) & 0x3F) | 0x80;
    }
}

extern "win64" fn co_create_guid(g: *mut u8) -> u32 {
    if g.is_null() {
        return E_INVALIDARG;
    }
    guid_nuevo(g);
    S_OK
}

extern "win64" fn uuid_create(g: *mut u8) -> u32 {
    if g.is_null() {
        return 1783; // RPC_S_INVALID_ARG... el mas parecido: RPC_X_NULL_REF_POINTER
    }
    guid_nuevo(g);
    0
}

/// `StringFromGUID2`: `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` y su cero (39);
/// 0 si no cabe.
extern "win64" fn string_from_guid2(g: *const u8, buf: *mut u16, n: i32) -> i32 {
    if g.is_null() || buf.is_null() || n < 39 {
        return 0;
    }
    // SAFETY: un GUID suyo.
    let b: [u8; 16] = unsafe { core::ptr::read_unaligned(g as *const [u8; 16]) };
    let d1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let d2 = u16::from_le_bytes([b[4], b[5]]);
    let d3 = u16::from_le_bytes([b[6], b[7]]);
    let t = alloc::format!(
        "{{{d1:08X}-{d2:04X}-{d3:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    );
    for (k, c) in t.encode_utf16().chain(core::iter::once(0)).enumerate() {
        // SAFETY: `n` >= 39 caracteres suyos.
        unsafe { buf.add(k).write(c) };
    }
    39
}

// -- sin red, sin mando -------------------------------------------------------------------

extern "win64" fn icmp_create_file() -> u64 {
    0x5A1E_1C00
}

/// `IcmpSendEcho`: sin red, nadie contesta: 0 respuestas, IP_REQ_TIMED_OUT.
extern "win64" fn icmp_send_echo(_h: u64, _ip: u32, _d: *const u8, _n: u16, _o: *const u8, _r: *mut u8, _rn: u32, _ms: u32) -> u32 {
    kernel32::poner_error(11010);
    0
}

extern "win64" fn icmp_close_handle(_h: u64) -> i32 {
    1
}

/// `AcceptEx`: sin red, no hay conexiones que aceptar (WSAEOPNOTSUPP).
extern "win64" fn accept_ex(_s: u64, _a: u64, _b: *mut u8, _n: u32, _ln: u32, _rn: u32, _r: *mut u32, _o: *mut u8) -> i32 {
    kernel32::poner_error(10045);
    0
}

/// `GetAcceptExSockaddrs`: nada que leer: punteros a nada y largos a 0.
extern "win64" fn get_accept_ex_sockaddrs(_b: *const u8, _n: u32, _ln: u32, _rn: u32, l: *mut u64, lln: *mut i32, r: *mut u64, rln: *mut i32) {
    for (p, n) in [(l, lln), (r, rln)] {
        // SAFETY: los punteros suyos, si los da.
        unsafe {
            if !p.is_null() {
                p.write(0);
            }
            if !n.is_null() {
                n.write(0);
            }
        }
    }
}

/// XInput: BMO-X no tiene mandos de XInput: ERROR_DEVICE_NOT_CONNECTED.
extern "win64" fn xinput_sin_mando(_usuario: u32, _p: *mut u8) -> u32 {
    ERROR_DEVICE_NOT_CONNECTED
}

/// Lo de ntdll de aqui (y RtlUnwind, que la casa ya tenia en kernel32).
pub(crate) fn buscar_ntdll(n: &str) -> Option<u64> {
    Some(match n {
        "NtQueryInformationProcess" | "ZwQueryInformationProcess" => dir!(nt_query_information_process),
        "RtlRunOnceExecuteOnce" => dir!(rtl_run_once_execute_once),
        "RtlUTF8ToUnicodeN" => dir!(rtl_utf8_to_unicode_n),
        "RtlUnwind" => return crate::kernel32_procesos::buscar(n),
        _ => return None,
    })
}

/// Lo demas, por su nombre (ninguno se repite entre estas DLL).
pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "timeGetTime" => dir!(time_get_time),
        "timeBeginPeriod" | "timeEndPeriod" => dir!(time_period),
        "timeGetDevCaps" => dir!(time_get_dev_caps),
        "PathFileExistsW" => dir!(path_file_exists_w),
        "PathRemoveFileSpecW" => dir!(path_remove_file_spec_w),
        "SHGetFolderPathW" => dir!(sh_get_folder_path_w),
        "SHGetSpecialFolderPathW" => dir!(sh_get_special_folder_path_w),
        "SHGetKnownFolderPath" => dir!(sh_get_known_folder_path),
        "ShellExecuteA" => dir!(shell_execute_a),
        "GetStockObject" => dir!(get_stock_object),
        "CallNtPowerInformation" => dir!(call_nt_power_information),
        "InternetGetConnectedState" => dir!(internet_get_connected_state),
        "IdnToAscii" => dir!(idn_to_ascii),
        "CoTaskMemAlloc" => dir!(co_task_mem_alloc),
        "CoTaskMemRealloc" => dir!(co_task_mem_realloc),
        "CoTaskMemFree" => dir!(co_task_mem_free),
        "CoCreateGuid" => dir!(co_create_guid),
        "StringFromGUID2" => dir!(string_from_guid2),
        "UuidCreate" | "UuidCreateSequential" => dir!(uuid_create),
        "IcmpCreateFile" => dir!(icmp_create_file),
        "IcmpSendEcho" => dir!(icmp_send_echo),
        "IcmpCloseHandle" => dir!(icmp_close_handle),
        "AcceptEx" => dir!(accept_ex),
        "GetAcceptExSockaddrs" => dir!(get_accept_ex_sockaddrs),
        "XInputGetState" | "XInputSetState" | "XInputGetCapabilities" => dir!(xinput_sin_mando),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use alloc::string::String;

    #[test]
    fn punycode_de_la_rfc() {
        let p = |s: &str| String::from_utf8(punycode(&s.chars().map(|c| c as u32).collect::<Vec<_>>()).unwrap()).unwrap();
        assert_eq!(p("b\u{fc}cher"), "bcher-kva");
        assert_eq!(p("ma\u{f1}ana"), "maana-pta");
        assert_eq!(p("\u{fc}"), "tda");
        let idn = |s: &str| String::from_utf16(&idn_a_ascii(&s.encode_utf16().collect::<Vec<_>>()).unwrap()).unwrap();
        assert_eq!(idn("B\u{fc}cher.Example"), "xn--bcher-kva.example");
        assert_eq!(idn("example.com"), "example.com");
    }

    #[test]
    fn utf8_con_lo_malo() {
        let (v, malo) = utf8_a_utf16("h\u{e9}llo \u{1F600}".as_bytes());
        assert_eq!(String::from_utf16(&v).unwrap(), "h\u{e9}llo \u{1F600}");
        assert!(!malo);
        let (v, malo) = utf8_a_utf16(&[b'a', 0xFF, b'b', 0xC3]);
        assert_eq!(v, [b'a' as u16, 0xFFFD, b'b' as u16, 0xFFFD]);
        assert!(malo);
    }
}

