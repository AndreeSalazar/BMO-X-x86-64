//! **El texto y la consola de Windows, de la casa** (P4f, 27-09).
//!
//! ```text
//!    MultiByteToWideChar, WideCharToMultiByte   bmo_proton_x::texto (UTF-8)
//!    CompareStringOrdinal, lstrlenW, GetACP, GetOEMCP
//!    GetConsoleMode, GetConsoleOutputCP/CP, SetConsoleOutputCP,
//!    WriteConsoleW                              la consola de quien lanzo
//! ```
//!
//! La consola es la de BMO-X: de lineas y en UTF-8. `WriteConsoleW` pasa el
//! UTF-16 a UTF-8 y lo escribe como `WriteFile`. La unica pagina de codigos
//! es UTF-8 (65001): la que Windows 10 da con el manifiesto de UTF-8.

use bmo_proton_x::texto;

use crate::{aviso, dir, kernel32, plataforma};

const CP_ACP: u32 = 0;
const CP_OEMCP: u32 = 1;
const CP_THREAD_ACP: u32 = 3;
const CP_UTF8: u32 = 65001;
const MB_ERR_INVALID_CHARS: u32 = 0x8;
const WC_ERR_INVALID_CHARS: u32 = 0x80;

const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_INVALID_FLAGS: u32 = 1004;
const ERROR_NO_UNICODE_TRANSLATION: u32 = 1113;

fn es_utf8(cp: u32) -> bool {
    if matches!(cp, CP_ACP | CP_OEMCP | CP_THREAD_ACP | CP_UTF8) {
        return true;
    }
    aviso(&alloc::format!("la pagina de codigos {cp}: la casa solo sabe UTF-8 (65001)"));
    false
}

/// Una cadena del `.exe`: `n` elementos, o hasta su 0 (incluido) con -1.
///
/// # Safety
/// `p` apunta a `n` elementos, o a una cadena terminada en 0 si `n` es -1.
unsafe fn trozo<'a, T: Copy + PartialEq + Default>(p: *const T, n: i32) -> Option<&'a [T]> {
    if p.is_null() || n == 0 || n < -1 {
        return None;
    }
    let largo = if n == -1 {
        let mut k = 0;
        while *p.add(k) != T::default() {
            k += 1;
        }
        k + 1
    } else {
        n as usize
    };
    Some(core::slice::from_raw_parts(p, largo))
}

/// Dar `r` como las dos: `m` 0, cuanto hace falta; si no cabe, 0 y 122.
fn dar<T: Copy>(r: &[T], dst: *mut T, m: i32) -> i32 {
    if m == 0 {
        return r.len() as i32;
    }
    if m < 0 || (m as usize) < r.len() || dst.is_null() {
        kernel32::poner_error(if m < 0 { ERROR_INVALID_PARAMETER } else { ERROR_INSUFFICIENT_BUFFER });
        return 0;
    }
    // SAFETY: el `.exe` da `m` >= r.len() elementos en `dst`.
    unsafe { core::ptr::copy_nonoverlapping(r.as_ptr(), dst, r.len()) };
    r.len() as i32
}

extern "win64" fn multi_byte_to_wide_char(cp: u32, banderas: u32, src: *const u8, n: i32, dst: *mut u16, m: i32) -> i32 {
    if !es_utf8(cp) {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    if banderas & !MB_ERR_INVALID_CHARS != 0 {
        kernel32::poner_error(ERROR_INVALID_FLAGS);
        return 0;
    }
    // SAFETY: lo que promete el `.exe`, como en Windows.
    let Some(b) = (unsafe { trozo(src, n) }) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    match texto::a_ancho(b, banderas & MB_ERR_INVALID_CHARS != 0) {
        Ok(w) => dar(&w, dst, m),
        Err(_) => {
            kernel32::poner_error(ERROR_NO_UNICODE_TRANSLATION);
            0
        }
    }
}

extern "win64" fn wide_char_to_multi_byte(cp: u32, banderas: u32, src: *const u16, n: i32, dst: *mut u8, m: i32, por_defecto: u64, usado: u64) -> i32 {
    if !es_utf8(cp) || por_defecto != 0 || usado != 0 {
        // Con UTF-8, Windows pide que esos dos vayan a NULL.
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    if banderas & !WC_ERR_INVALID_CHARS != 0 {
        kernel32::poner_error(ERROR_INVALID_FLAGS);
        return 0;
    }
    // SAFETY: lo que promete el `.exe`.
    let Some(w) = (unsafe { trozo(src, n) }) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    match texto::a_estrecho(w, banderas & WC_ERR_INVALID_CHARS != 0) {
        Ok(b) => dar(&b, dst, m),
        Err(_) => {
            kernel32::poner_error(ERROR_NO_UNICODE_TRANSLATION);
            0
        }
    }
}

/// 1, 2 o 3 (menor, igual, mayor); 0 si no se puede.
extern "win64" fn compare_string_ordinal(a: *const u16, na: i32, b: *const u16, nb: i32, sin_mayusculas: i32) -> i32 {
    // SAFETY: lo que promete el `.exe`. Con -1, sin su 0: se compara el texto.
    let quitar_cero = |s: &'static [u16], n: i32| if n == -1 { &s[..s.len() - 1] } else { s };
    let vacio: &[u16] = &[];
    let (x, y) = unsafe {
        (
            if na == 0 { Some(vacio) } else { trozo(a, na).map(|s| quitar_cero(s, na)) },
            if nb == 0 { Some(vacio) } else { trozo(b, nb).map(|s| quitar_cero(s, nb)) },
        )
    };
    match (x, y) {
        (Some(x), Some(y)) => 2 + texto::comparar(x, y, sin_mayusculas != 0),
        _ => {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            0
        }
    }
}

extern "win64" fn lstrlen_w(s: *const u16) -> i32 {
    // SAFETY: una cadena del `.exe` terminada en 0.
    unsafe { trozo(s, -1) }.map_or(0, |s| s.len() as i32 - 1)
}

extern "win64" fn get_acp() -> u32 {
    CP_UTF8
}

extern "win64" fn get_console_mode(h: u64, modo: *mut u32) -> i32 {
    if !kernel32::es_consola(h) || modo.is_null() {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    }
    // ENABLE_PROCESSED_OUTPUT | ENABLE_WRAP_AT_EOL_OUTPUT.
    // SAFETY: un DWORD del `.exe`.
    unsafe { *modo = 3 };
    1
}

extern "win64" fn set_console_output_cp(cp: u32) -> i32 {
    if cp == CP_UTF8 {
        return 1;
    }
    aviso("SetConsoleOutputCP: la consola de BMO-X es UTF-8 (65001) y no cambia");
    kernel32::poner_error(ERROR_INVALID_PARAMETER);
    0
}

extern "win64" fn write_console_w(h: u64, b: *const u16, n: u32, escritos: *mut u32, _reservado: u64) -> i32 {
    if !kernel32::es_consola(h) {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    }
    // SAFETY: el `.exe` da `n` caracteres en `b`.
    let w = if n == 0 { &[][..] } else { unsafe { core::slice::from_raw_parts(b, n as usize) } };
    let bytes = texto::a_estrecho(w, false).unwrap_or_default();
    (plataforma().escribir)(&bytes);
    if !escritos.is_null() {
        // SAFETY: un DWORD del `.exe`.
        unsafe { *escritos = n };
    }
    1
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "MultiByteToWideChar" => dir!(multi_byte_to_wide_char),
        "WideCharToMultiByte" => dir!(wide_char_to_multi_byte),
        "CompareStringOrdinal" => dir!(compare_string_ordinal),
        "lstrlenW" => dir!(lstrlen_w),
        "GetACP" => dir!(get_acp),
        "GetOEMCP" => dir!(get_acp),
        "GetConsoleMode" => dir!(get_console_mode),
        "GetConsoleOutputCP" => dir!(get_acp),
        "GetConsoleCP" => dir!(get_acp),
        "SetConsoleOutputCP" => dir!(set_console_output_cp),
        "WriteConsoleW" => dir!(write_console_w),
        _ => return None,
    })
}
