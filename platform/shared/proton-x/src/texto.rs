//! **El texto de Windows** (P4f, 27-09): UTF-8 y UTF-16 como los pasan
//! `MultiByteToWideChar` / `WideCharToMultiByte`, y `CompareStringOrdinal`.
//!
//! ```text
//!    a_ancho    UTF-8 -> UTF-16; lo mal formado es U+FFFD, o un NO si se
//!               pidio MB_ERR_INVALID_CHARS
//!    a_estrecho UTF-16 -> UTF-8; un sustituto suelto es U+FFFD (EF BF BD),
//!               o un NO con WC_ERR_INVALID_CHARS
//!    comparar   el orden de CompareStringOrdinal: por unidad de UTF-16, y
//!               sin mayusculas que cuenten si se pide
//! ```
//!
//! Lo que no es Windows, dicho: sin mayusculas que cuenten solo en ASCII
//! (Windows usa su tabla de Unicode entera), y las paginas de codigos son una,
//! UTF-8 (`GetACP` dice 65001, lo que Windows 10 da con el manifiesto de
//! UTF-8).

use alloc::vec::Vec;

/// Una secuencia mal formada, pedida como error (ERROR_NO_UNICODE_TRANSLATION).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalFormado;

const REEMPLAZO: u16 = 0xFFFD;

/// **UTF-8 a UTF-16**. `estricto`: MB_ERR_INVALID_CHARS. Cada trozo mal
/// formado maximo (la regla de Unicode) es UN U+FFFD.
pub fn a_ancho(b: &[u8], estricto: bool) -> Result<Vec<u16>, MalFormado> {
    let mut v = Vec::with_capacity(b.len());
    for t in b.utf8_chunks() {
        v.extend(t.valid().encode_utf16());
        if !t.invalid().is_empty() {
            if estricto {
                return Err(MalFormado);
            }
            v.push(REEMPLAZO);
        }
    }
    Ok(v)
}

/// **UTF-16 a UTF-8**. `estricto`: WC_ERR_INVALID_CHARS.
pub fn a_estrecho(w: &[u16], estricto: bool) -> Result<Vec<u8>, MalFormado> {
    let mut v = Vec::with_capacity(w.len());
    for r in char::decode_utf16(w.iter().copied()) {
        let c = match r {
            Ok(c) => c,
            Err(_) if estricto => return Err(MalFormado),
            Err(_) => '\u{FFFD}',
        };
        let mut b = [0u8; 4];
        v.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
    }
    Ok(v)
}

fn mayus(c: u16) -> u16 {
    if (b'a' as u16..=b'z' as u16).contains(&c) {
        c - 32
    } else {
        c
    }
}

/// **`CompareStringOrdinal`**: -1, 0 o 1 (Windows suma 2: CSTR_LESS_THAN,
/// CSTR_EQUAL, CSTR_GREATER_THAN).
pub fn comparar(a: &[u16], b: &[u16], sin_mayusculas: bool) -> i32 {
    let f = |c: u16| if sin_mayusculas { mayus(c) } else { c };
    match a.iter().map(|&c| f(c)).cmp(b.iter().map(|&c| f(c))) {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    }
}
