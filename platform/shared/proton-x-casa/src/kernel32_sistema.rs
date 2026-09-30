//! **Lo que `kernel32.dll` dice del sistema, de la casa** (tanda 3 de
//! Cyberpunk, paso 2, 29-09).
//!
//! ```text
//!    memoria      GlobalMemoryStatus(Ex) GetPhysicallyInstalledSystemMemory
//!    procesador   GetCurrentProcessorNumber, las afinidades, el ideal, los
//!                 CPU sets, Get/SetThreadPriority, SetThreadDescription,
//!                 SetThreadInformation, FlushInstructionCache,
//!                 FlushProcessWriteBuffers, DisableThreadLibraryCalls
//!    version      GetVersionExA/W VerifyVersionInfoA/W: Windows 10, 22H2
//!    arranque     GetStartupInfoA/W
//!    locale       en-US: GetUserDefaultLCID/LocaleName/UILanguage,
//!                 GetThreadUILanguage, GetUserGeoID, IsValidLocale,
//!                 IsValidCodePage, GetCPInfo, LocaleNameToLCID,
//!                 LCIDToLocaleName, ResolveLocaleName, EnumSystemLocalesW,
//!                 GetStringTypeW/A/ExA
//!    consola      GetConsoleWindow AllocConsole SetConsoleCtrlHandler
//!                 SetConsoleMode SetConsoleTextAttribute SetConsoleTitleA
//!                 AreFileApisANSI; OutputDebugStringA/W (sin depurador: nada)
//! ```
//!
//! La memoria que se dice: con la reserva (P0.4c), la del kernel (total y
//! libre de ahora); sin ella, una cifra FIJA (16 GiB, la del Ryzen del
//! propietario; 12 libres). Un procesador, como `GetSystemInfo`: los hilos de
//! la casa se turnan en uno.

use crate::{dir, kernel32};

const GIB: u64 = 1 << 30;
const MEMORIA_TOTAL: u64 = 16 * GIB;
const MEMORIA_LIBRE: u64 = 12 * GIB;
/// Lo que un proceso de 64 bits puede direccionar en Windows (128 TiB).
const VIRTUAL: u64 = 0x7FFF_FFFE_FFFF - 0x1_0000 + 1;

const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_OLD_WIN_VERSION: u32 = 1150;

// -- Memoria -----------------------------------------------------------------------------------

/// (total, libre): lo del kernel si la plataforma lo sabe (P0.4c); si no,
/// la cifra fija de siempre.
fn memoria() -> (u64, u64) {
    crate::memoria::ram().unwrap_or((MEMORIA_TOTAL, MEMORIA_LIBRE))
}

/// `GlobalMemoryStatusEx(MEMORYSTATUSEX*)`: 64 bytes, con dwLength puesto.
extern "win64" fn global_memory_status_ex(p: *mut u8) -> i32 {
    if p.is_null() {
        return 0;
    }
    // SAFETY: el MEMORYSTATUSEX del `.exe`; dwLength dice que son 64.
    unsafe {
        if (p as *const u32).read_unaligned() != 64 {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            return 0;
        }
        let (total, libre) = memoria();
        let carga = (100 - libre * 100 / total.max(1)) as u32;
        (p.add(4) as *mut u32).write_unaligned(carga);
        for (i, v) in [total, libre, 2 * total, total + libre, VIRTUAL, VIRTUAL, 0].into_iter().enumerate() {
            (p.add(8 + 8 * i) as *mut u64).write_unaligned(v);
        }
    }
    1
}

/// `GlobalMemoryStatus(MEMORYSTATUS*)`: 56 bytes, los SIZE_T de x64.
extern "win64" fn global_memory_status(p: *mut u8) {
    if p.is_null() {
        return;
    }
    // SAFETY: el MEMORYSTATUS del `.exe` (56 bytes).
    unsafe {
        (p as *mut u32).write_unaligned(56);
        let (total, libre) = memoria();
        (p.add(4) as *mut u32).write_unaligned((100 - libre * 100 / total.max(1)) as u32);
        for (i, v) in [total, libre, 2 * total, total + libre, VIRTUAL, VIRTUAL].into_iter().enumerate() {
            (p.add(8 + 8 * i) as *mut u64).write_unaligned(v);
        }
    }
}

extern "win64" fn get_physically_installed_system_memory(kb: *mut u64) -> i32 {
    if kb.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    // SAFETY: el ULONGLONG del `.exe`.
    unsafe { kb.write_unaligned(memoria().0 / 1024) };
    1
}

// -- El procesador y los hilos -----------------------------------------------------------------

extern "win64" fn cero() -> u32 {
    0
}

extern "win64" fn uno(_a: u64) -> i32 {
    1
}

extern "win64" fn uno2(_a: u64, _b: u64) -> i32 {
    1
}

extern "win64" fn uno3(_a: u64, _b: u64, _c: u64) -> i32 {
    1
}

extern "win64" fn uno4(_a: u64, _b: u64, _c: u64, _d: u64) -> i32 {
    1
}

extern "win64" fn nada(_a: u64) {}

/// `GetProcessAffinityMask(h, *proceso, *sistema)`: un procesador.
extern "win64" fn get_process_affinity_mask(_h: u64, proceso: *mut u64, sistema: *mut u64) -> i32 {
    for p in [proceso, sistema] {
        if !p.is_null() {
            // SAFETY: los DWORD_PTR del `.exe`.
            unsafe { p.write_unaligned(1) };
        }
    }
    1
}

/// `SetThreadAffinityMask(h, mascara)`: la de antes (1); 0 si la nueva no
/// deja el unico procesador.
extern "win64" fn set_thread_affinity_mask(_h: u64, mascara: u64) -> u64 {
    if mascara & 1 == 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        0
    } else {
        1
    }
}

/// `SetThreadDescription`: S_OK (el nombre no se ve en ningun sitio).
extern "win64" fn set_thread_description(_h: u64, _nombre: u64) -> i32 {
    0
}

/// `GetThreadPriority`: THREAD_PRIORITY_NORMAL. La casa no tiene
/// prioridades: todos los hilos se turnan igual.
extern "win64" fn get_thread_priority(_h: u64) -> i32 {
    0
}

// -- La version --------------------------------------------------------------------------------

/// Windows 10 22H2: lo que un juego de 2020-2025 espera ver.
const MAYOR: u32 = 10;
const MENOR: u32 = 0;
const COMPILACION: u32 = 19045;
const VER_PLATFORM_WIN32_NT: u32 = 2;
const VER_NT_WORKSTATION: u8 = 1;

/// OSVERSIONINFO(EX)W o A: su medida dice cual es.
fn poner_version(p: *mut u8, ancha: bool) -> i32 {
    if p.is_null() {
        return 0;
    }
    let (base, ex) = if ancha { (276u32, 284u32) } else { (148, 156) };
    // SAFETY: el OSVERSIONINFO del `.exe`: su primer DWORD dice su medida.
    let tam = unsafe { (p as *const u32).read_unaligned() };
    if tam != base && tam != ex {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    // SAFETY: `tam` bytes del `.exe`.
    unsafe {
        core::ptr::write_bytes(p.add(4), 0, tam as usize - 4);
        for (i, v) in [MAYOR, MENOR, COMPILACION, VER_PLATFORM_WIN32_NT].into_iter().enumerate() {
            (p.add(4 + 4 * i) as *mut u32).write_unaligned(v);
        }
        if tam == ex {
            *p.add(base as usize + 6) = VER_NT_WORKSTATION;
        }
    }
    1
}

extern "win64" fn get_version_ex_w(p: *mut u8) -> i32 {
    poner_version(p, true)
}

extern "win64" fn get_version_ex_a(p: *mut u8) -> i32 {
    poner_version(p, false)
}

const VER_MINORVERSION: u32 = 0x1;
const VER_MAJORVERSION: u32 = 0x2;
const VER_BUILDNUMBER: u32 = 0x4;
const VER_PLATFORMID: u32 = 0x8;
const VER_SERVICEPACKMINOR: u32 = 0x10;
const VER_SERVICEPACKMAJOR: u32 = 0x20;
const VER_PRODUCT_TYPE: u32 = 0x80;

/// La condicion (3 bits por tipo) de `VerSetConditionMask`.
fn condicion(mascara: u64, tipo: u32) -> u8 {
    ((mascara >> (3 * tipo.trailing_zeros())) & 7) as u8
}

/// `a` frente a `b` con una condicion de VER_*: 1 igual, 2 mayor, 3 mayor o
/// igual, 4 menor, 5 menor o igual.
fn cumple(a: core::cmp::Ordering, c: u8) -> bool {
    use core::cmp::Ordering::*;
    match c {
        1 => a == Equal,
        2 => a == Greater,
        3 => a != Less,
        4 => a == Less,
        5 => a != Greater,
        _ => false,
    }
}

/// `VerifyVersionInfo`: la version de la casa frente a la pedida. Mayor,
/// menor y service pack van juntos (como Windows: la condicion del mayor
/// manda); la compilacion, la plataforma y el tipo, cada uno por su cuenta.
fn verificar(p: *const u8, tipos: u32, mascara: u64, ancha: bool) -> i32 {
    match comparar_version(p, tipos, mascara, ancha) {
        Some(true) => 1,
        Some(false) => {
            kernel32::poner_error(ERROR_OLD_WIN_VERSION);
            0
        }
        None => {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            0
        }
    }
}

/// Lo de `verificar`, sin tocar LastError: `None` si la pregunta no vale.
fn comparar_version(p: *const u8, tipos: u32, mascara: u64, ancha: bool) -> Option<bool> {
    if p.is_null() || tipos == 0 {
        return None;
    }
    let base = if ancha { 276 } else { 148 };
    // SAFETY: un OSVERSIONINFOEX del `.exe`.
    let (mayor, menor, comp, plat, sp_mayor, sp_menor, tipo_prod) = unsafe {
        let d = |o: usize| (p.add(o) as *const u32).read_unaligned();
        let w = |o: usize| (p.add(o) as *const u16).read_unaligned();
        (d(4), d(8), d(12), d(16), w(base), w(base + 2), *p.add(base + 6))
    };
    let mut bien = true;
    let juntos = tipos & (VER_MAJORVERSION | VER_MINORVERSION | VER_SERVICEPACKMAJOR | VER_SERVICEPACKMINOR);
    if juntos != 0 {
        // La condicion del primero que haya, del mas alto al mas bajo.
        let c = [VER_MAJORVERSION, VER_MINORVERSION, VER_SERVICEPACKMAJOR, VER_SERVICEPACKMINOR].into_iter().find(|&t| juntos & t != 0).map_or(3, |t| condicion(mascara, t));
        let campo = |t: u32, casa: u32, pedido: u32| if juntos & t != 0 { (casa, pedido) } else { (0, 0) };
        let (a1, b1) = campo(VER_MAJORVERSION, MAYOR, mayor);
        let (a2, b2) = campo(VER_MINORVERSION, MENOR, menor);
        let (a3, b3) = campo(VER_SERVICEPACKMAJOR, 0, sp_mayor as u32);
        let (a4, b4) = campo(VER_SERVICEPACKMINOR, 0, sp_menor as u32);
        bien &= cumple((a1, a2, a3, a4).cmp(&(b1, b2, b3, b4)), c);
    }
    if tipos & VER_BUILDNUMBER != 0 {
        bien &= cumple(COMPILACION.cmp(&comp), condicion(mascara, VER_BUILDNUMBER));
    }
    if tipos & VER_PLATFORMID != 0 {
        bien &= cumple(VER_PLATFORM_WIN32_NT.cmp(&plat), condicion(mascara, VER_PLATFORMID));
    }
    if tipos & VER_PRODUCT_TYPE != 0 {
        bien &= cumple(VER_NT_WORKSTATION.cmp(&tipo_prod), condicion(mascara, VER_PRODUCT_TYPE));
    }
    Some(bien)
}

extern "win64" fn verify_version_info_w(p: *const u8, tipos: u32, mascara: u64) -> i32 {
    verificar(p, tipos, mascara, true)
}

extern "win64" fn verify_version_info_a(p: *const u8, tipos: u32, mascara: u64) -> i32 {
    verificar(p, tipos, mascara, false)
}

// -- El arranque -------------------------------------------------------------------------------

/// `GetStartupInfoW/A`: un STARTUPINFO (104 bytes) de un proceso que arranco
/// sin nada especial.
extern "win64" fn get_startup_info(p: *mut u8) {
    if p.is_null() {
        return;
    }
    // SAFETY: los 104 bytes del `.exe`.
    unsafe {
        core::ptr::write_bytes(p, 0, 104);
        (p as *mut u32).write_unaligned(104);
    }
}

// -- El locale: en-US --------------------------------------------------------------------------

const LCID_EN_US: u32 = 0x409;
const LOCALE_INVARIANT: u32 = 0x7F;
const EN_US: &str = "en-US";

fn dar_w(t: &str, buf: *mut u16, n: i32) -> i32 {
    let w: alloc::vec::Vec<u16> = t.encode_utf16().chain([0]).collect();
    if n == 0 {
        return w.len() as i32;
    }
    if buf.is_null() || (n as usize) < w.len() {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    // SAFETY: `n` caracteres del `.exe`; cabe con su 0.
    unsafe { core::ptr::copy_nonoverlapping(w.as_ptr(), buf, w.len()) };
    w.len() as i32
}

extern "win64" fn lcid() -> u32 {
    LCID_EN_US
}

extern "win64" fn get_user_default_locale_name(buf: *mut u16, n: i32) -> i32 {
    dar_w(EN_US, buf, n)
}

/// `GetUserGeoID(GEOCLASS_NATION)`: 244, los Estados Unidos.
extern "win64" fn get_user_geo_id(_clase: u32) -> i32 {
    244
}

extern "win64" fn is_valid_locale(l: u32, _banderas: u32) -> i32 {
    (l == LCID_EN_US || l == LOCALE_INVARIANT || l == 0x400 || l == 0x800) as i32
}

const CODIGOS: [u32; 6] = [437, 850, 1252, 20127, 28591, 65001];

extern "win64" fn is_valid_code_page(cp: u32) -> i32 {
    CODIGOS.contains(&cp) as i32
}

/// `GetCPInfo(cp, CPINFO*)`: MaxCharSize, DefaultChar[2], LeadByte[12].
extern "win64" fn get_cp_info(cp: u32, p: *mut u8) -> i32 {
    // CP_ACP (0), CP_OEMCP (1), CP_THREAD_ACP (3): la de la casa, UTF-8.
    let cp = if matches!(cp, 0 | 1 | 3) { 65001 } else { cp };
    if p.is_null() || !CODIGOS.contains(&cp) {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    // SAFETY: los 18 bytes del CPINFO del `.exe`.
    unsafe {
        core::ptr::write_bytes(p, 0, 18);
        (p as *mut u32).write_unaligned(if cp == 65001 { 4 } else { 1 });
        *p.add(4) = b'?';
    }
    1
}

fn nombre_w(p: *const u16) -> alloc::string::String {
    if p.is_null() {
        return alloc::string::String::new();
    }
    alloc::string::String::from_utf16_lossy(&crate::crt::cadena_w(p as u64))
}

extern "win64" fn locale_name_to_lcid(nombre: *const u16, _banderas: u32) -> u32 {
    let n = nombre_w(nombre);
    if nombre.is_null() || n.eq_ignore_ascii_case(EN_US) || n.eq_ignore_ascii_case("en") {
        LCID_EN_US
    } else if n.is_empty() {
        LOCALE_INVARIANT
    } else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        0
    }
}

extern "win64" fn lcid_to_locale_name(l: u32, buf: *mut u16, n: i32, _banderas: u32) -> i32 {
    match l {
        LOCALE_INVARIANT => dar_w("", buf, n),
        _ if is_valid_locale(l, 0) != 0 => dar_w(EN_US, buf, n),
        _ => {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            0
        }
    }
}

extern "win64" fn resolve_locale_name(_nombre: *const u16, buf: *mut u16, n: i32) -> i32 {
    dar_w(EN_US, buf, n)
}

type EnumLocales = extern "win64" fn(*const u16) -> i32;

/// `EnumSystemLocalesW(f, banderas)`: uno, "00000409".
extern "win64" fn enum_system_locales_w(f: u64, _banderas: u32) -> i32 {
    if f == 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let w: alloc::vec::Vec<u16> = "00000409".encode_utf16().chain([0]).collect();
    // SAFETY: la funcion del `.exe`: BOOL CALLBACK f(LPWSTR).
    let f = unsafe { core::mem::transmute::<u64, EnumLocales>(f) };
    f(w.as_ptr());
    1
}

const CT_CTYPE1: u32 = 1;

/// Las banderas C1_* de un caracter.
fn tipo_c1(c: u32) -> u16 {
    let Some(ch) = char::from_u32(c) else { return 0 };
    let mut f = 0x200; // C1_DEFINED
    if ch.is_uppercase() {
        f |= 0x1;
    }
    if ch.is_lowercase() {
        f |= 0x2;
    }
    if ch.is_ascii_digit() {
        f |= 0x4;
    }
    if ch.is_whitespace() {
        f |= 0x8;
    }
    if ch.is_ascii_punctuation() {
        f |= 0x10;
    }
    if ch.is_control() {
        f |= 0x20;
    }
    if ch == ' ' || ch == '\t' {
        f |= 0x40;
    }
    if ch.is_ascii_hexdigit() {
        f |= 0x80;
    }
    if ch.is_alphabetic() {
        f |= 0x100;
    }
    f
}

fn tipos(tipo: u32, cs: &[u32], out: *mut u16) -> i32 {
    if out.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    for (i, &c) in cs.iter().enumerate() {
        let v = if tipo == CT_CTYPE1 { tipo_c1(c) } else { 0 };
        // SAFETY: el `.exe` da un WORD por caracter.
        unsafe { out.add(i).write_unaligned(v) };
    }
    1
}

/// Los caracteres de una cadena del `.exe`: `n` o, con -1, hasta su 0.
fn chars<T: Copy + Into<u32>>(p: *const T, n: i32) -> alloc::vec::Vec<u32> {
    let mut v = alloc::vec::Vec::new();
    if p.is_null() {
        return v;
    }
    loop {
        if n >= 0 && v.len() >= n as usize {
            break;
        }
        // SAFETY: hasta `n` elementos, o hasta el 0.
        let c: u32 = unsafe { *p.add(v.len()) }.into();
        if n < 0 && c == 0 {
            v.push(0);
            break;
        }
        v.push(c);
    }
    v
}

extern "win64" fn get_string_type_w(tipo: u32, s: *const u16, n: i32, out: *mut u16) -> i32 {
    tipos(tipo, &chars(s, n), out)
}

extern "win64" fn get_string_type_a(_l: u32, tipo: u32, s: *const u8, n: i32, out: *mut u16) -> i32 {
    tipos(tipo, &chars(s, n), out)
}

// -- La consola --------------------------------------------------------------------------------

extern "win64" fn get_console_window() -> u64 {
    0
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "GlobalMemoryStatusEx" => dir!(global_memory_status_ex),
        "GlobalMemoryStatus" => dir!(global_memory_status),
        "GetPhysicallyInstalledSystemMemory" => dir!(get_physically_installed_system_memory),
        "GetCurrentProcessorNumber" => dir!(cero),
        "GetProcessAffinityMask" => dir!(get_process_affinity_mask),
        "SetProcessAffinityMask" | "SetThreadPriority" | "SetConsoleMode" | "SetConsoleTextAttribute" => dir!(uno2),
        "SetThreadAffinityMask" => dir!(set_thread_affinity_mask),
        "SetThreadIdealProcessor" => dir!(cero),
        "SetThreadSelectedCpuSets" | "FlushInstructionCache" => dir!(uno3),
        "SetThreadInformation" => dir!(uno4),
        "SetThreadDescription" => dir!(set_thread_description),
        "GetThreadPriority" => dir!(get_thread_priority),
        "FlushProcessWriteBuffers" => dir!(cero),
        "DisableThreadLibraryCalls" | "AllocConsole" | "SetConsoleTitleA" | "AreFileApisANSI" => dir!(uno),
        "SetConsoleCtrlHandler" => dir!(uno2),
        "OutputDebugStringA" | "OutputDebugStringW" => dir!(nada),
        "GetVersionExW" => dir!(get_version_ex_w),
        "GetVersionExA" => dir!(get_version_ex_a),
        "VerifyVersionInfoW" => dir!(verify_version_info_w),
        "VerifyVersionInfoA" => dir!(verify_version_info_a),
        "GetStartupInfoW" | "GetStartupInfoA" => dir!(get_startup_info),
        "GetUserDefaultLCID" | "GetUserDefaultUILanguage" | "GetThreadUILanguage" | "GetSystemDefaultLCID" | "GetSystemDefaultUILanguage" | "GetUserDefaultLangID" => dir!(lcid),
        "GetUserDefaultLocaleName" => dir!(get_user_default_locale_name),
        "GetUserGeoID" => dir!(get_user_geo_id),
        "IsValidLocale" => dir!(is_valid_locale),
        "IsValidCodePage" => dir!(is_valid_code_page),
        "GetCPInfo" => dir!(get_cp_info),
        "LocaleNameToLCID" => dir!(locale_name_to_lcid),
        "LCIDToLocaleName" => dir!(lcid_to_locale_name),
        "ResolveLocaleName" => dir!(resolve_locale_name),
        "EnumSystemLocalesW" => dir!(enum_system_locales_w),
        "GetStringTypeW" => dir!(get_string_type_w),
        "GetStringTypeA" | "GetStringTypeExA" => dir!(get_string_type_a),
        "GetConsoleWindow" => dir!(get_console_window),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_version_se_verifica_como_en_windows() {
        let mut v = [0u8; 284];
        v[0..4].copy_from_slice(&284u32.to_le_bytes());
        v[4..8].copy_from_slice(&10u32.to_le_bytes());
        let mayor_igual = 3u64 << 3; // VER_MAJORVERSION, VER_GREATER_EQUAL
        assert_eq!(comparar_version(v.as_ptr(), VER_MAJORVERSION, mayor_igual, true), Some(true));
        v[4..8].copy_from_slice(&11u32.to_le_bytes());
        assert_eq!(comparar_version(v.as_ptr(), VER_MAJORVERSION, mayor_igual, true), Some(false));
        v[4..8].copy_from_slice(&6u32.to_le_bytes());
        v[8..12].copy_from_slice(&1u32.to_le_bytes());
        let mm = mayor_igual | 3 << 0;
        assert_eq!(comparar_version(v.as_ptr(), VER_MAJORVERSION | VER_MINORVERSION, mm, true), Some(true), "10.0 >= 6.1");
        let mut out = [0u16; 4];
        assert_eq!(get_string_type_w(CT_CTYPE1, [b'A' as u16, b'7' as u16, b' ' as u16, 0xE9].as_ptr(), 4, out.as_mut_ptr()), 1);
        assert_eq!(out[0] & 0x101, 0x101);
        assert_eq!(out[1] & 0x84, 0x84);
        assert_eq!(out[2] & 0x48, 0x48);
        assert_eq!(out[3] & 0x102, 0x102, "e con tilde: minuscula y letra");
    }
}
