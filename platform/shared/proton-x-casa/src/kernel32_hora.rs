//! **La hora y lo sencillo de `kernel32.dll`, de la casa** (tanda 3 de
//! Cyberpunk, 29-09).
//!
//! ```text
//!    la hora      GetSystemTime GetLocalTime SystemTimeToFileTime
//!                 FileTimeToSystemTime FileTimeToLocalFileTime
//!                 LocalFileTimeToFileTime SystemTimeToTzSpecificLocalTime
//!                 GetTimeZoneInformation GetDynamicTimeZoneInformation
//!    sencillas    MulDiv EncodePointer DecodePointer lstrcmpA
//!                 VerSetConditionMask SetHandleCount
//!                 Get/SetErrorMode Get/SetThreadErrorMode
//! ```
//!
//! Lo que no, dicho: la zona horaria es UTC (la placa no dice otra), asi que
//! la hora local es la de UTC.

use core::cell::UnsafeCell;

use crate::dir;

/// FILETIME de 1601-01-01 a 1970-01-01, en unidades de 100 ns.
const DE_1601_A_1970: u64 = 116_444_736_000_000_000;

/// SYSTEMTIME: anio, mes, dia de la semana, dia, hora, minuto, segundo, ms.
type SystemTime = [u16; 8];

/// Los dias desde 1970 de una fecha civil (algoritmo de Howard Hinnant).
fn dias_desde_1970(a: i64, m: i64, d: i64) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let era = a.div_euclid(400);
    let yoe = a - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// La fecha civil de un dia desde 1970: (anio, mes, dia).
fn civil(dias: i64) -> (i64, i64, i64) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (m <= 2) as i64, m, d)
}

/// Un FILETIME (100 ns desde 1601) a SYSTEMTIME.
pub(crate) fn a_system_time(ft: u64) -> Option<SystemTime> {
    let desde_1970 = ft as i128 - DE_1601_A_1970 as i128;
    let seg = desde_1970.div_euclid(10_000_000) as i64;
    let ms = (desde_1970.rem_euclid(10_000_000) / 10_000) as u16;
    let dias = seg.div_euclid(86_400);
    let s = seg.rem_euclid(86_400);
    let (a, m, d) = civil(dias);
    if !(1601..=30827).contains(&a) {
        return None;
    }
    let semana = (dias + 4).rem_euclid(7) as u16;
    Some([a as u16, m as u16, semana, d as u16, (s / 3600) as u16, (s / 60 % 60) as u16, (s % 60) as u16, ms])
}

/// Un SYSTEMTIME a FILETIME; `None` si algun campo se sale.
pub(crate) fn a_file_time(st: &SystemTime) -> Option<u64> {
    let [a, m, _, d, h, mi, s, ms] = *st;
    if !(1601..=30827).contains(&a) || !(1..=12).contains(&m) || d < 1 || h > 23 || mi > 59 || s > 59 || ms > 999 {
        return None;
    }
    let bisiesto = (a % 4 == 0 && a % 100 != 0) || a % 400 == 0;
    const DIAS: [u16; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let max = DIAS[m as usize - 1] + (m == 2 && bisiesto) as u16;
    if d > max {
        return None;
    }
    let dias = dias_desde_1970(a as i64, m as i64, d as i64);
    let seg = dias * 86_400 + h as i64 * 3600 + mi as i64 * 60 + s as i64;
    Some((seg as i128 * 10_000_000 + ms as i128 * 10_000 + DE_1601_A_1970 as i128) as u64)
}

fn leer_st(p: *const u16) -> SystemTime {
    let mut st = [0u16; 8];
    for (i, x) in st.iter_mut().enumerate() {
        // SAFETY: un SYSTEMTIME del `.exe` (8 WORD).
        *x = unsafe { p.add(i).read_unaligned() };
    }
    st
}

fn poner_st(p: *mut u16, st: &SystemTime) {
    for (i, &x) in st.iter().enumerate() {
        // SAFETY: como `leer_st`.
        unsafe { p.add(i).write_unaligned(x) };
    }
}

extern "win64" fn get_system_time(p: *mut u16) {
    if let Some(st) = a_system_time(crate::esperas::filetime_ahora()) {
        poner_st(p, &st);
    }
}

extern "win64" fn system_time_to_file_time(st: *const u16, ft: *mut u64) -> i32 {
    if st.is_null() || ft.is_null() {
        return 0;
    }
    match a_file_time(&leer_st(st)) {
        Some(v) => {
            // SAFETY: el FILETIME del `.exe`.
            unsafe { ft.write_unaligned(v) };
            1
        }
        None => {
            crate::kernel32::poner_error(87);
            0
        }
    }
}

extern "win64" fn file_time_to_system_time(ft: *const u64, st: *mut u16) -> i32 {
    if ft.is_null() || st.is_null() {
        return 0;
    }
    // SAFETY: el FILETIME del `.exe`.
    match a_system_time(unsafe { ft.read_unaligned() }) {
        Some(v) => {
            poner_st(st, &v);
            1
        }
        None => {
            crate::kernel32::poner_error(87);
            0
        }
    }
}

/// UTC: la local es la misma.
extern "win64" fn file_time_igual(de: *const u64, a: *mut u64) -> i32 {
    if de.is_null() || a.is_null() {
        return 0;
    }
    // SAFETY: dos FILETIME del `.exe`.
    unsafe { a.write_unaligned(de.read_unaligned()) };
    1
}

extern "win64" fn system_time_to_tz_local(_tz: u64, de: *const u16, a: *mut u16) -> i32 {
    if de.is_null() || a.is_null() {
        return 0;
    }
    poner_st(a, &leer_st(de));
    1
}

/// TIME_ZONE_INFORMATION (172 bytes) a cero, con "UTC" de nombre:
/// TIME_ZONE_ID_UNKNOWN (0).
extern "win64" fn get_time_zone_information(tz: *mut u8) -> u32 {
    if !tz.is_null() {
        // SAFETY: los 172 bytes del `.exe`.
        unsafe {
            core::ptr::write_bytes(tz, 0, 172);
            for (i, c) in "UTC".encode_utf16().enumerate() {
                (tz.add(4 + 2 * i) as *mut u16).write_unaligned(c);
                (tz.add(88 + 2 * i) as *mut u16).write_unaligned(c);
            }
        }
    }
    0
}

/// DYNAMIC_TIME_ZONE_INFORMATION (432 bytes): la misma, y la clave "UTC".
extern "win64" fn get_dynamic_time_zone_information(tz: *mut u8) -> u32 {
    if !tz.is_null() {
        // SAFETY: los 432 bytes del `.exe`.
        unsafe { core::ptr::write_bytes(tz, 0, 432) };
        get_time_zone_information(tz);
        for (i, c) in "UTC".encode_utf16().enumerate() {
            // SAFETY: TimeZoneKeyName en +172.
            unsafe { (tz.add(172 + 2 * i) as *mut u16).write_unaligned(c) };
        }
    }
    0
}

// -- Sencillas ---------------------------------------------------------------------------------

/// `MulDiv(a, b, c)`: a*b/c redondeado, en 64 bits; -1 si c es 0 o se sale.
extern "win64" fn mul_div(a: i32, b: i32, c: i32) -> i32 {
    if c == 0 {
        return -1;
    }
    let p = a as i64 * b as i64;
    // Redondear a lo mas cercano, la mitad lejos del cero.
    let r = if (p < 0) != (c < 0) { (p - (c as i64).abs() / 2 * (c as i64).signum()) / c as i64 } else { (p + c as i64 / 2) / c as i64 };
    if r > i32::MAX as i64 || r < i32::MIN as i64 {
        -1
    } else {
        r as i32
    }
}

/// La clave con que se esconden los punteros: una por proceso.
const CLAVE: u64 = 0x5A1D_B0B0_C0DE_2077;

extern "win64" fn encode_pointer(p: u64) -> u64 {
    (p ^ CLAVE).rotate_right(17)
}

extern "win64" fn decode_pointer(p: u64) -> u64 {
    p.rotate_left(17) ^ CLAVE
}

extern "win64" fn lstrcmp_a(a: *const u8, b: *const u8) -> i32 {
    let (a, b) = (crate::crt::cadena_c(a as u64), crate::crt::cadena_c(b as u64));
    match a.cmp(&b) {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    }
}

/// `VerSetConditionMask(mascara, tipo, condicion)`: 3 bits por tipo.
extern "win64" fn ver_set_condition_mask(mascara: u64, tipo: u32, condicion: u8) -> u64 {
    let mut m = mascara;
    for i in 0..8 {
        if tipo & (1 << i) != 0 {
            m |= ((condicion & 7) as u64) << (3 * i);
        }
    }
    m
}

extern "win64" fn set_handle_count(n: u32) -> u32 {
    n
}

struct Modos(UnsafeCell<(u32, u32)>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Modos {}
static MODOS: Modos = Modos(UnsafeCell::new((0, 0)));

fn modos() -> &'static mut (u32, u32) {
    // SAFETY: ver `Modos`.
    unsafe { &mut *MODOS.0.get() }
}

extern "win64" fn set_error_mode(m: u32) -> u32 {
    core::mem::replace(&mut modos().0, m)
}

extern "win64" fn get_error_mode() -> u32 {
    modos().0
}

extern "win64" fn get_thread_error_mode() -> u32 {
    modos().1
}

extern "win64" fn set_thread_error_mode(m: u32, antes: *mut u32) -> i32 {
    let viejo = core::mem::replace(&mut modos().1, m);
    if !antes.is_null() {
        // SAFETY: el DWORD del `.exe`.
        unsafe { *antes = viejo };
    }
    1
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "GetSystemTime" | "GetLocalTime" => dir!(get_system_time),
        "SystemTimeToFileTime" => dir!(system_time_to_file_time),
        "FileTimeToSystemTime" => dir!(file_time_to_system_time),
        "FileTimeToLocalFileTime" | "LocalFileTimeToFileTime" => dir!(file_time_igual),
        "SystemTimeToTzSpecificLocalTime" => dir!(system_time_to_tz_local),
        "GetTimeZoneInformation" => dir!(get_time_zone_information),
        "GetDynamicTimeZoneInformation" => dir!(get_dynamic_time_zone_information),
        "MulDiv" => dir!(mul_div),
        "EncodePointer" => dir!(encode_pointer),
        "DecodePointer" => dir!(decode_pointer),
        "lstrcmpA" => dir!(lstrcmp_a),
        "VerSetConditionMask" => dir!(ver_set_condition_mask),
        "SetHandleCount" => dir!(set_handle_count),
        "SetErrorMode" => dir!(set_error_mode),
        "GetErrorMode" => dir!(get_error_mode),
        "GetThreadErrorMode" => dir!(get_thread_error_mode),
        "SetThreadErrorMode" => dir!(set_thread_error_mode),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_fecha_va_y_vuelve() {
        // 2026-09-29 15:04:05.250 UTC, martes.
        let ft = (1_790_694_245u64 * 10_000_000) + 2_500_000 + DE_1601_A_1970;
        let st = a_system_time(ft).unwrap();
        assert_eq!(st, [2026, 9, 2, 29, 15, 4, 5, 250]);
        assert_eq!(a_file_time(&st), Some(ft));
        assert_eq!(a_system_time(0).unwrap(), [1601, 1, 1, 1, 0, 0, 0, 0]);
        assert_eq!(a_file_time(&[2025, 2, 0, 29, 0, 0, 0, 0]), None);
        assert!(a_file_time(&[2024, 2, 0, 29, 0, 0, 0, 0]).is_some());
    }

    #[test]
    fn mul_div_y_punteros() {
        assert_eq!(mul_div(10, 3, 4), 8);
        assert_eq!(mul_div(-10, 3, 4), -8);
        assert_eq!(mul_div(1, 1, 0), -1);
        assert_eq!(mul_div(i32::MAX, 2, 1), -1);
        assert_eq!(decode_pointer(encode_pointer(0x1234_5678)), 0x1234_5678);
        assert_eq!(ver_set_condition_mask(0, 2, 3), 3 << 3);
    }
}
