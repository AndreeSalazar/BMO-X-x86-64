//! **El locale de `kernel32.dll`, de la casa** (tanda 8 de Cyberpunk, 30-09):
//! comparar y cambiar texto, y lo que un locale sabe de fechas, horas,
//! numeros y moneda.
//!
//! ```text
//!    comparar    CompareStringW/Ex: por letras (sin mayusculas), y a igualdad,
//!                la minuscula antes (como Windows)
//!    cambiar     LCMapStringW/Ex/A: minusculas, mayusculas, las claves de
//!                ordenar (LCMAP_SORTKEY) y el orden de bytes
//!    el locale   GetLocaleInfoW/Ex/A (con LOCALE_RETURN_NUMBER), GetGeoInfoW
//!    formatos    GetDateFormatW/Ex, GetTimeFormatW/Ex, GetNumberFormatEx,
//!                GetCurrencyFormatEx, con sus "pictures" (dddd, MMMM, tt...)
//! ```
//!
//! Dos locales: el del usuario de la casa, en-US (0x409, que tambien es el
//! del sistema y el "neutro"), y el invariante (0x7F, ""). Otro nombre es
//! ERROR_INVALID_PARAMETER.

use alloc::string::String;
use alloc::vec::Vec;

use crate::{dir, kernel32};

const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INVALID_FLAGS: u32 = 1004;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Locale {
    EnUs,
    Invariante,
}

fn de_lcid(l: u32) -> Option<Locale> {
    match l {
        0 | 0x400 | 0x800 | 0x409 | 0xC00 => Some(Locale::EnUs),
        0x7F => Some(Locale::Invariante),
        _ => None,
    }
}

fn texto_w(p: *const u16, n: i32) -> Option<Vec<u16>> {
    if p.is_null() {
        return None;
    }
    let mut v = Vec::new();
    loop {
        if n >= 0 && v.len() >= n as usize {
            break;
        }
        // SAFETY: hasta `n` caracteres del `.exe`, o hasta el 0 (con -1).
        let c = unsafe { *p.add(v.len()) };
        if n < 0 && c == 0 {
            break;
        }
        v.push(c);
    }
    Some(v)
}

fn de_nombre(p: *const u16) -> Option<Locale> {
    if p.is_null() {
        return Some(Locale::EnUs);
    }
    let n = String::from_utf16_lossy(&texto_w(p, -1)?);
    match n.as_str() {
        "" => Some(Locale::Invariante),
        _ if n.eq_ignore_ascii_case("en-US") || n.eq_ignore_ascii_case("en") || n == "!x-sys-default-locale" => Some(Locale::EnUs),
        _ => None,
    }
}

fn invalido(e: u32) -> i32 {
    kernel32::poner_error(e);
    0
}

/// Dar `t` en un bufer W de `n` caracteres: con 0, lo que hace falta (con
/// el 0); no cabe, 0 y ERROR_INSUFFICIENT_BUFFER; cabe, los escritos con el 0.
fn dar(t: &[u16], buf: *mut u16, n: i32) -> i32 {
    let k = t.len() + 1;
    if n == 0 {
        return k as i32;
    }
    if buf.is_null() || (n as usize) < k {
        return invalido(ERROR_INSUFFICIENT_BUFFER);
    }
    // SAFETY: `n` >= k caracteres del `.exe`.
    unsafe {
        core::ptr::copy_nonoverlapping(t.as_ptr(), buf, t.len());
        *buf.add(t.len()) = 0;
    }
    k as i32
}

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

// -- Comparar ----------------------------------------------------------------------------------

const NORM_IGNORECASE: u32 = 0x1;
const NORM_IGNORENONSPACE: u32 = 0x2;
const NORM_IGNORESYMBOLS: u32 = 0x4;
const LINGUISTIC_IGNORECASE: u32 = 0x10;
const NORM_IGNOREWIDTH: u32 = 0x2_0000;
const CSTR_LESS_THAN: i32 = 1;
const CSTR_EQUAL: i32 = 2;
const CSTR_GREATER_THAN: i32 = 3;

fn bajar(c: u16) -> u16 {
    match char::from_u32(c as u32) {
        Some(ch) if ch.is_uppercase() => ch.to_lowercase().next().map_or(c, |l| if (l as u32) < 0x1_0000 { l as u16 } else { c }),
        _ => c,
    }
}

fn es_simbolo(c: u16) -> bool {
    char::from_u32(c as u32).is_some_and(|ch| !ch.is_alphanumeric() && !ch.is_whitespace())
}

/// La comparacion de Windows, en dos niveles: las letras sin mayusculas
/// (el primario) y, a igualdad, la mayuscula DESPUES de la minuscula.
fn comparar(a: &[u16], b: &[u16], banderas: u32) -> i32 {
    let limpia = |s: &[u16]| -> Vec<u16> { s.iter().copied().filter(|&c| !(banderas & NORM_IGNORESYMBOLS != 0 && es_simbolo(c))).collect() };
    let (a, b) = (limpia(a), limpia(b));
    let primario = |s: &[u16]| -> Vec<u16> { s.iter().map(|&c| bajar(c)).collect() };
    let orden = primario(&a).cmp(&primario(&b));
    let orden = if orden.is_eq() && banderas & (NORM_IGNORECASE | LINGUISTIC_IGNORECASE) == 0 {
        // A igualdad de letras: la minuscula antes. `a` mayuscula y `b`
        // minuscula en el primer sitio distinto: `a` va despues.
        match a.iter().zip(&b).find(|(x, y)| x != y) {
            Some((&x, _)) if bajar(x) != x => core::cmp::Ordering::Greater,
            Some(_) => core::cmp::Ordering::Less,
            None => core::cmp::Ordering::Equal,
        }
    } else {
        orden
    };
    match orden {
        core::cmp::Ordering::Less => CSTR_LESS_THAN,
        core::cmp::Ordering::Equal => CSTR_EQUAL,
        core::cmp::Ordering::Greater => CSTR_GREATER_THAN,
    }
}

fn compare_de(loc: Option<Locale>, banderas: u32, s1: *const u16, n1: i32, s2: *const u16, n2: i32) -> i32 {
    if loc.is_none() {
        return invalido(ERROR_INVALID_PARAMETER);
    }
    let conocidas = NORM_IGNORECASE | NORM_IGNORENONSPACE | NORM_IGNORESYMBOLS | LINGUISTIC_IGNORECASE | NORM_IGNOREWIDTH | 0x8 | 0x1000 | 0x2000 | 0x1_0000 | 0x800_0000 | 0x1000_0000;
    if banderas & !conocidas != 0 {
        return invalido(ERROR_INVALID_FLAGS);
    }
    match (texto_w(s1, n1), texto_w(s2, n2)) {
        (Some(a), Some(b)) => comparar(&a, &b, banderas),
        _ => invalido(ERROR_INVALID_PARAMETER),
    }
}

extern "win64" fn compare_string_w(lcid: u32, banderas: u32, s1: *const u16, n1: i32, s2: *const u16, n2: i32) -> i32 {
    compare_de(de_lcid(lcid), banderas, s1, n1, s2, n2)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn compare_string_ex(nombre: *const u16, banderas: u32, s1: *const u16, n1: i32, s2: *const u16, n2: i32, _v: u64, _r: u64, _p: u64) -> i32 {
    compare_de(de_nombre(nombre), banderas, s1, n1, s2, n2)
}

// -- Cambiar -----------------------------------------------------------------------------------

const LCMAP_LOWERCASE: u32 = 0x100;
const LCMAP_UPPERCASE: u32 = 0x200;
const LCMAP_TITLECASE: u32 = 0x300;
const LCMAP_SORTKEY: u32 = 0x400;
const LCMAP_BYTEREV: u32 = 0x800;

fn subir(c: u16) -> u16 {
    match char::from_u32(c as u32) {
        Some(ch) if ch.is_lowercase() => ch.to_uppercase().next().map_or(c, |u| if (u as u32) < 0x1_0000 { u as u16 } else { c }),
        _ => c,
    }
}

/// La clave de ordenar: comparada con memcmp, da el orden de `comparar`.
/// Las letras sin mayusculas (2 bytes cada una), 01 01 01, las mayusculas
/// (1 byte cada una), 01 00.
fn clave(s: &[u16], banderas: u32) -> Vec<u8> {
    let mut k = Vec::new();
    for &c in s {
        if banderas & NORM_IGNORESYMBOLS != 0 && es_simbolo(c) {
            continue;
        }
        let p = bajar(c).wrapping_add(2);
        k.extend_from_slice(&p.to_be_bytes());
    }
    k.extend_from_slice(&[1, 1, 1]);
    if banderas & (NORM_IGNORECASE | LINGUISTIC_IGNORECASE) == 0 {
        for &c in s {
            if banderas & NORM_IGNORESYMBOLS != 0 && es_simbolo(c) {
                continue;
            }
            k.push(if bajar(c) != c { 0x12 } else { 0x02 });
        }
    }
    k.extend_from_slice(&[1, 0]);
    k
}

fn lcmap(loc: Option<Locale>, banderas: u32, src: *const u16, n: i32, dst: *mut u16, m: i32) -> i32 {
    if loc.is_none() {
        return invalido(ERROR_INVALID_PARAMETER);
    }
    let Some(s) = texto_w(src, n) else { return invalido(ERROR_INVALID_PARAMETER) };
    let con_cero = n < 0;
    if banderas & LCMAP_SORTKEY != 0 {
        // En bytes, con su 0 al final siempre.
        let k = clave(&s, banderas);
        if m == 0 {
            return k.len() as i32;
        }
        if dst.is_null() || (m as usize) < k.len() {
            return invalido(ERROR_INSUFFICIENT_BUFFER);
        }
        // SAFETY: `m` bytes del `.exe` (con SORTKEY, `dst` son bytes).
        unsafe { core::ptr::copy_nonoverlapping(k.as_ptr(), dst as *mut u8, k.len()) };
        return k.len() as i32;
    }
    let caso = banderas & 0x300;
    let mut r: Vec<u16> = match caso {
        LCMAP_LOWERCASE => s.iter().map(|&c| bajar(c)).collect(),
        LCMAP_UPPERCASE => s.iter().map(|&c| subir(c)).collect(),
        LCMAP_TITLECASE => {
            let mut nueva = true;
            s.iter()
                .map(|&c| {
                    let r = if nueva { subir(c) } else { bajar(c) };
                    nueva = !char::from_u32(c as u32).is_some_and(|x| x.is_alphanumeric());
                    r
                })
                .collect()
        }
        _ => s.clone(),
    };
    if banderas & LCMAP_BYTEREV != 0 {
        for c in &mut r {
            *c = c.swap_bytes();
        }
    }
    if con_cero {
        return dar(&r, dst, m);
    }
    // Sin el 0: los que caben justos.
    if m == 0 {
        return r.len() as i32;
    }
    if dst.is_null() || (m as usize) < r.len() {
        return invalido(ERROR_INSUFFICIENT_BUFFER);
    }
    // SAFETY: `m` >= r.len() caracteres del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(r.as_ptr(), dst, r.len()) };
    r.len() as i32
}

extern "win64" fn lcmap_string_w(lcid: u32, banderas: u32, src: *const u16, n: i32, dst: *mut u16, m: i32) -> i32 {
    lcmap(de_lcid(lcid), banderas, src, n, dst, m)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn lcmap_string_ex(nombre: *const u16, banderas: u32, src: *const u16, n: i32, dst: *mut u16, m: i32, _v: u64, _r: u64, _p: u64) -> i32 {
    lcmap(de_nombre(nombre), banderas, src, n, dst, m)
}

/// `LCMapStringA`: a UTF-16, la W, y de vuelta (en bytes UTF-8; la clave de
/// ordenar sale tal cual).
extern "win64" fn lcmap_string_a(lcid: u32, banderas: u32, src: *const u8, n: i32, dst: *mut u8, m: i32) -> i32 {
    if src.is_null() {
        return invalido(ERROR_INVALID_PARAMETER);
    }
    let bytes: Vec<u8> = if n < 0 { crate::crt::cadena_c(src as u64) } else {
        // SAFETY: `n` bytes del `.exe`.
        unsafe { core::slice::from_raw_parts(src, n as usize) }.to_vec()
    };
    let s = bmo_proton_x::texto::a_ancho(&bytes, false).unwrap_or_default();
    if banderas & LCMAP_SORTKEY != 0 {
        return lcmap(de_lcid(lcid), banderas, s.as_ptr(), s.len() as i32, dst as *mut u16, m);
    }
    let mut r = alloc::vec![0u16; s.len() + 1];
    let k = lcmap(de_lcid(lcid), banderas, s.as_ptr(), s.len() as i32, r.as_mut_ptr(), r.len() as i32);
    if k == 0 {
        return 0;
    }
    let mut b = bmo_proton_x::texto::a_estrecho(&r[..k as usize], false).unwrap_or_default();
    if n < 0 {
        b.push(0);
    }
    if m == 0 {
        return b.len() as i32;
    }
    if dst.is_null() || (m as usize) < b.len() {
        return invalido(ERROR_INSUFFICIENT_BUFFER);
    }
    // SAFETY: `m` bytes del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), dst, b.len()) };
    b.len() as i32
}

// -- Lo que el locale sabe ---------------------------------------------------------------------

const DIAS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MESES: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

const LOCALE_RETURN_NUMBER: u32 = 0x2000_0000;

/// El valor de un LCTYPE, o `None` si la casa no lo sabe.
fn dato(loc: Locale, tipo: u32) -> Option<String> {
    let inv = loc == Locale::Invariante;
    let s = |a: &str, b: &str| Some(String::from(if inv { b } else { a }));
    match tipo {
        0x1 => s("0409", "007f"),                         // LOCALE_ILANGUAGE
        0x2 => s("English (United States)", "Invariant Language (Invariant Country)"),
        0x3 => s("ENU", "IVL"),                           // LOCALE_SABBREVLANGNAME
        0x4 | 0x1001 => s("English", "Invariant Language"),
        0x5 => s("1", "1"),                               // LOCALE_ICOUNTRY
        0x6 | 0x1002 | 0x8 => s("United States", "Invariant Country"),
        0x7 => s("USA", "IVC"),
        0x9 => s("0409", "007f"),                         // LOCALE_IDEFAULTLANGUAGE
        0xA => s("1", "1"),                               // LOCALE_IDEFAULTCOUNTRY
        0xB => s("437", "437"),                           // LOCALE_IDEFAULTCODEPAGE
        0xC => s(",", ","),                               // LOCALE_SLIST
        0xD => s("1", "0"),                               // LOCALE_IMEASURE
        0xE | 0x16 => s(".", "."),                        // SDECIMAL, SMONDECIMALSEP
        0xF | 0x17 => s(",", ","),                        // STHOUSAND, SMONTHOUSANDSEP
        0x10 | 0x18 => s("3;0", "3;0"),                   // SGROUPING, SMONGROUPING
        0x11 | 0x19 => s("2", "2"),                       // IDIGITS, ICURRDIGITS
        0x12 => s("1", "1"),                              // ILZERO
        0x13 => s("0123456789", "0123456789"),
        0x14 => Some(String::from(if inv { "\u{a4}" } else { "$" })),
        0x15 => s("USD", "XDR"),
        0x1B => s("0", "0"),                              // ICURRENCY: $1.1
        0x1C => s("1", "0"),                              // INEGCURR: -$1.1
        0x1D => s("/", "/"),                              // SDATE
        0x1E => s(":", ":"),                              // STIME
        0x1F => s("M/d/yyyy", "MM/dd/yyyy"),              // SSHORTDATE
        0x20 => s("dddd, MMMM d, yyyy", "dddd, dd MMMM yyyy"),
        0x23 => s("0", "1"),                              // ITIME: 12 horas en en-US
        0x28 => s("AM", "AM"),
        0x29 => s("PM", "PM"),
        0x2A..=0x30 => Some(String::from(DIAS[(tipo - 0x2A) as usize])),
        0x31..=0x37 => Some(String::from(&DIAS[(tipo - 0x31) as usize][..3])),
        0x38..=0x43 => Some(String::from(MESES[(tipo - 0x38) as usize])),
        0x44..=0x4F => Some(String::from(&MESES[(tipo - 0x44) as usize][..3])),
        0x50 => s("", ""),                                // SPOSITIVESIGN
        0x51 => s("-", "-"),                              // SNEGATIVESIGN
        0x59 => s("en", "iv"),                            // SISO639LANGNAME
        0x5A => s("US", "IV"),                            // SISO3166CTRYNAME
        0x5C => s("en-US", ""),                           // LOCALE_SNAME
        0x67 => s("eng", "ivl"),
        0x68 => s("USA", "IVC"),
        0x6D => s("en", ""),                              // SPARENT
        0x72 => s("English (United States)", "Invariant Language (Invariant Country)"),
        0x79 => s("h:mm tt", "HH:mm"),                    // SSHORTTIME
        0x1003 => s("h:mm:ss tt", "HH:mm:ss"),            // STIMEFORMAT
        0x1004 => s("1252", "1252"),                      // IDEFAULTANSICODEPAGE
        0x1009 => s("1", "1"),                            // ICALENDARTYPE: gregoriano
        0x100C => s("6", "0"),                            // IFIRSTDAYOFWEEK: domingo
        0x1010 => s("1", "1"),                            // INEGNUMBER: -1.1
        0x1011 => s("10000", "10000"),
        0x1012 => s("037", "037"),
        0x1014 => s("2", "2"),                            // IDIGITSUBSTITUTION
        _ => None,
    }
}

fn info(loc: Option<Locale>, tipo: u32, buf: *mut u16, n: i32) -> i32 {
    let Some(loc) = loc else { return invalido(ERROR_INVALID_PARAMETER) };
    let Some(v) = dato(loc, tipo & 0x0FFF_FFFF) else { return invalido(ERROR_INVALID_FLAGS) };
    if tipo & LOCALE_RETURN_NUMBER != 0 {
        // Un DWORD en el bufer: 2 caracteres.
        let Ok(x) = u32::from_str_radix(&v, if tipo & 0x0FFF_FFFF == 0x1 || tipo & 0x0FFF_FFFF == 0x9 { 16 } else { 10 }) else { return invalido(ERROR_INVALID_FLAGS) };
        if n == 0 {
            return 2;
        }
        if buf.is_null() || n < 2 {
            return invalido(ERROR_INSUFFICIENT_BUFFER);
        }
        // SAFETY: 2 WCHAR del `.exe`.
        unsafe { (buf as *mut u32).write_unaligned(x) };
        return 2;
    }
    dar(&w(&v), buf, n)
}

extern "win64" fn get_locale_info_w(lcid: u32, tipo: u32, buf: *mut u16, n: i32) -> i32 {
    info(de_lcid(lcid), tipo, buf, n)
}

extern "win64" fn get_locale_info_ex(nombre: *const u16, tipo: u32, buf: *mut u16, n: i32) -> i32 {
    info(de_nombre(nombre), tipo, buf, n)
}

extern "win64" fn get_locale_info_a(lcid: u32, tipo: u32, buf: *mut u8, n: i32) -> i32 {
    let Some(loc) = de_lcid(lcid) else { return invalido(ERROR_INVALID_PARAMETER) };
    if tipo & LOCALE_RETURN_NUMBER != 0 {
        let mut x = [0u16; 2];
        if info(Some(loc), tipo, x.as_mut_ptr(), 2) == 0 {
            return 0;
        }
        if n == 0 {
            return 4;
        }
        if buf.is_null() || n < 4 {
            return invalido(ERROR_INSUFFICIENT_BUFFER);
        }
        // SAFETY: 4 bytes del `.exe`.
        unsafe { core::ptr::copy_nonoverlapping(x.as_ptr() as *const u8, buf, 4) };
        return 4;
    }
    let Some(v) = dato(loc, tipo & 0x0FFF_FFFF) else { return invalido(ERROR_INVALID_FLAGS) };
    let b = v.as_bytes();
    if n == 0 {
        return b.len() as i32 + 1;
    }
    if buf.is_null() || (n as usize) <= b.len() {
        return invalido(ERROR_INSUFFICIENT_BUFFER);
    }
    // SAFETY: `n` > b.len() bytes del `.exe`.
    unsafe {
        core::ptr::copy_nonoverlapping(b.as_ptr(), buf, b.len());
        *buf.add(b.len()) = 0;
    }
    b.len() as i32 + 1
}

/// `GetGeoInfoW(244, tipo, bufer, n, idioma)`: los Estados Unidos.
extern "win64" fn get_geo_info_w(geo: i32, tipo: u32, buf: *mut u16, n: i32, _idioma: u16) -> i32 {
    if geo != 244 {
        return invalido(ERROR_INVALID_PARAMETER);
    }
    let v = match tipo {
        1 => "244",
        2 => "39.45",
        3 => "-98.908",
        4 => "US",
        5 => "USA",
        6 => "en-US",
        7 => "00000409",
        8 => "United States",
        9 => "United States of America",
        10 => "840",
        14 => "USD",
        _ => return invalido(ERROR_INVALID_FLAGS),
    };
    dar(&w(v), buf, n)
}

// -- Fechas y horas ----------------------------------------------------------------------------

const DATE_SHORTDATE: u32 = 0x1;
const DATE_LONGDATE: u32 = 0x2;
const DATE_YEARMONTH: u32 = 0x8;
const TIME_NOMINUTESORSECONDS: u32 = 0x1;
const TIME_NOSECONDS: u32 = 0x2;
const TIME_NOTIMEMARKER: u32 = 0x4;
const TIME_FORCE24HOURFORMAT: u32 = 0x8;

/// La hora que piden, o la de ahora (la local de la casa es UTC).
fn hora(st: *const u16) -> Option<[u16; 8]> {
    if st.is_null() {
        return crate::kernel32_hora::a_system_time(crate::esperas::filetime_ahora());
    }
    let mut v = [0u16; 8];
    for (i, x) in v.iter_mut().enumerate() {
        // SAFETY: un SYSTEMTIME del `.exe`.
        *x = unsafe { st.add(i).read_unaligned() };
    }
    // El dia de la semana, de la fecha (Windows no se fia del que le dan).
    let ft = crate::kernel32_hora::a_file_time(&v)?;
    crate::kernel32_hora::a_system_time(ft)
}

/// Los trozos de una "picture": letras repetidas, literales ('...') y lo
/// demas tal cual.
fn trozos(p: &[u16]) -> Vec<(u16, usize, Vec<u16>)> {
    let mut r = Vec::new();
    let mut i = 0;
    while i < p.len() {
        let c = p[i];
        if c == b'\'' as u16 {
            let mut lit = Vec::new();
            i += 1;
            while i < p.len() {
                if p[i] == b'\'' as u16 {
                    if p.get(i + 1) == Some(&(b'\'' as u16)) {
                        lit.push(b'\'' as u16);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                lit.push(p[i]);
                i += 1;
            }
            r.push((0, 0, lit));
            continue;
        }
        let mut k = 1;
        while i + k < p.len() && p[i + k] == c {
            k += 1;
        }
        let letra = (c as u8 as char).is_ascii_alphabetic() && c < 0x80;
        if letra {
            r.push((c, k, Vec::new()));
        } else {
            r.push((0, 0, p[i..i + k].to_vec()));
        }
        i += k;
    }
    r
}

fn num(v: u16, ancho: usize) -> String {
    if ancho >= 2 {
        alloc::format!("{v:02}")
    } else {
        alloc::format!("{v}")
    }
}

fn formatear_fecha(st: &[u16; 8], p: &[u16]) -> Vec<u16> {
    let mut r = String::new();
    for (c, k, lit) in trozos(p) {
        match (c as u8, k) {
            (0, _) => r.push_str(&String::from_utf16_lossy(&lit)),
            (b'd', 1 | 2) => r.push_str(&num(st[3], k)),
            (b'd', 3) => r.push_str(&DIAS[(st[2] as usize + 6) % 7][..3]),
            (b'd', _) => r.push_str(DIAS[(st[2] as usize + 6) % 7]),
            (b'M', 1 | 2) => r.push_str(&num(st[1], k)),
            (b'M', 3) => r.push_str(&MESES[st[1] as usize - 1][..3]),
            (b'M', _) => r.push_str(MESES[st[1] as usize - 1]),
            (b'y', 1) => r.push_str(&alloc::format!("{}", st[0] % 100)),
            (b'y', 2) => r.push_str(&alloc::format!("{:02}", st[0] % 100)),
            (b'y', _) => r.push_str(&alloc::format!("{}", st[0])),
            (b'g', _) => r.push_str("A.D."),
            (x, k) => r.push_str(&core::iter::repeat_n(x as char, k).collect::<String>()),
        }
    }
    w(&r)
}

/// Quitar de una picture de hora los segundos (y los minutos) con el
/// separador de delante, y la marca AM/PM con el espacio.
fn recortar_hora(p: &[u16], banderas: u32) -> Vec<u16> {
    let mut t = trozos(p);
    let quitar = |t: &mut Vec<(u16, usize, Vec<u16>)>, letra: u8| {
        while let Some(i) = t.iter().position(|x| x.0 == letra as u16) {
            t.remove(i);
            if i > 0 && t[i - 1].0 == 0 {
                t.remove(i - 1);
            }
        }
    };
    if banderas & (TIME_NOSECONDS | TIME_NOMINUTESORSECONDS) != 0 {
        quitar(&mut t, b's');
    }
    if banderas & TIME_NOMINUTESORSECONDS != 0 {
        quitar(&mut t, b'm');
    }
    if banderas & TIME_NOTIMEMARKER != 0 {
        quitar(&mut t, b't');
    }
    let mut r = Vec::new();
    for (c, k, lit) in t {
        if c == 0 {
            if lit.is_empty() {
                continue;
            }
            // Un literal suelto se vuelve a escribir entre comillas.
            r.push(b'\'' as u16);
            for &x in &lit {
                if x == b'\'' as u16 {
                    r.push(x);
                }
                r.push(x);
            }
            r.push(b'\'' as u16);
        } else {
            r.extend(core::iter::repeat_n(c, k));
        }
    }
    r
}

fn formatear_hora(st: &[u16; 8], p: &[u16], banderas: u32) -> Vec<u16> {
    let p = recortar_hora(p, banderas);
    let mut r = String::new();
    let h12 = if st[4] % 12 == 0 { 12 } else { st[4] % 12 };
    for (c, k, lit) in trozos(&p) {
        match (c as u8, k) {
            (0, _) => r.push_str(&String::from_utf16_lossy(&lit)),
            (b'h', _) if banderas & TIME_FORCE24HOURFORMAT != 0 => r.push_str(&num(st[4], k)),
            (b'h', _) => r.push_str(&num(h12, k)),
            (b'H', _) => r.push_str(&num(st[4], k)),
            (b'm', _) => r.push_str(&num(st[5], k)),
            (b's', _) => r.push_str(&num(st[6], k)),
            (b't', 1) => r.push(if st[4] < 12 { 'A' } else { 'P' }),
            (b't', _) => r.push_str(if st[4] < 12 { "AM" } else { "PM" }),
            (x, k) => r.push_str(&core::iter::repeat_n(x as char, k).collect::<String>()),
        }
    }
    w(r.trim_end())
}

fn fecha(loc: Option<Locale>, banderas: u32, st: *const u16, pic: *const u16, buf: *mut u16, n: i32) -> i32 {
    let Some(loc) = loc else { return invalido(ERROR_INVALID_PARAMETER) };
    let Some(st) = hora(st) else { return invalido(ERROR_INVALID_PARAMETER) };
    let p = match texto_w(pic, -1) {
        Some(p) => {
            if banderas & (DATE_SHORTDATE | DATE_LONGDATE | DATE_YEARMONTH) != 0 {
                return invalido(ERROR_INVALID_FLAGS);
            }
            p
        }
        None => {
            let t = if banderas & DATE_LONGDATE != 0 {
                0x20
            } else if banderas & DATE_YEARMONTH != 0 {
                return dar(&formatear_fecha(&st, &w(if loc == Locale::EnUs { "MMMM yyyy" } else { "yyyy MMMM" })), buf, n);
            } else {
                0x1F
            };
            w(&dato(loc, t).unwrap_or_default())
        }
    };
    dar(&formatear_fecha(&st, &p), buf, n)
}

fn tiempo(loc: Option<Locale>, banderas: u32, st: *const u16, pic: *const u16, buf: *mut u16, n: i32) -> i32 {
    let Some(loc) = loc else { return invalido(ERROR_INVALID_PARAMETER) };
    let Some(st) = hora(st) else { return invalido(ERROR_INVALID_PARAMETER) };
    let p = texto_w(pic, -1).unwrap_or_else(|| w(&dato(loc, 0x1003).unwrap_or_default()));
    dar(&formatear_hora(&st, &p, banderas), buf, n)
}

extern "win64" fn get_date_format_w(lcid: u32, banderas: u32, st: *const u16, pic: *const u16, buf: *mut u16, n: i32) -> i32 {
    fecha(de_lcid(lcid), banderas, st, pic, buf, n)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn get_date_format_ex(nombre: *const u16, banderas: u32, st: *const u16, pic: *const u16, buf: *mut u16, n: i32, _cal: u64) -> i32 {
    fecha(de_nombre(nombre), banderas, st, pic, buf, n)
}

extern "win64" fn get_time_format_w(lcid: u32, banderas: u32, st: *const u16, pic: *const u16, buf: *mut u16, n: i32) -> i32 {
    tiempo(de_lcid(lcid), banderas, st, pic, buf, n)
}

extern "win64" fn get_time_format_ex(nombre: *const u16, banderas: u32, st: *const u16, pic: *const u16, buf: *mut u16, n: i32) -> i32 {
    tiempo(de_nombre(nombre), banderas, st, pic, buf, n)
}

// -- Numeros y moneda --------------------------------------------------------------------------

/// Como NUMBERFMTW (y la parte comun de CURRENCYFMTW).
struct Formato {
    cifras: usize,
    cero_delante: bool,
    grupos: u32,
    decimal: Vec<u16>,
    miles: Vec<u16>,
    negativo: u32,
}

fn formato_de(loc: Locale, _moneda: bool) -> Formato {
    let _ = loc;
    Formato { cifras: 2, cero_delante: true, grupos: 3, decimal: w("."), miles: w(","), negativo: 1 }
}

/// El texto de un numero ("-1234.5678") redondeado a `cifras` decimales
/// (la mitad lejos del cero) y agrupado: (negativo, entero, decimales).
fn partir_numero(v: &[u16], f: &Formato) -> Option<(bool, String, String)> {
    let s = String::from_utf16(v).ok()?;
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.as_str()),
    };
    let (ent, dec) = s.split_once('.').unwrap_or((s, ""));
    if ent.is_empty() && dec.is_empty() || !ent.bytes().all(|c| c.is_ascii_digit()) || !dec.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // Todas las cifras juntas, y redondear en la posicion `cifras`.
    let mut digitos: Vec<u8> = ent.bytes().chain(dec.bytes().chain(core::iter::repeat(b'0')).take(f.cifras.max(dec.len()))).map(|c| c - b'0').collect();
    let corte = ent.len() + f.cifras;
    let sube = digitos.get(corte).is_some_and(|&d| d >= 5);
    digitos.truncate(corte);
    if sube {
        let mut i = digitos.len();
        loop {
            if i == 0 {
                digitos.insert(0, 1);
                break;
            }
            i -= 1;
            if digitos[i] == 9 {
                digitos[i] = 0;
            } else {
                digitos[i] += 1;
                break;
            }
        }
    }
    let n_ent = digitos.len() - f.cifras;
    let mut e: String = digitos[..n_ent].iter().map(|d| (b'0' + d) as char).collect::<String>().trim_start_matches('0').into();
    if e.is_empty() && f.cero_delante {
        e.push('0');
    }
    let d: String = digitos[n_ent..].iter().map(|d| (b'0' + d) as char).collect();
    let cero = e.trim_matches('0').is_empty() && d.trim_matches('0').is_empty();
    Some((neg && !cero, e, d))
}

fn agrupar(e: &str, grupos: u32, miles: &str) -> String {
    // 3: de 3 en 3; 32: los ultimos 3 y luego de 2 en 2; 0: sin grupos.
    let (primero, resto) = match grupos {
        0 => return String::from(e),
        g if g >= 10 => ((g / 10) as usize, (g % 10) as usize),
        g => (g as usize, g as usize),
    };
    let b = e.as_bytes();
    let mut partes: Vec<&str> = Vec::new();
    let mut fin = b.len();
    let mut tam = primero;
    while fin > tam && tam > 0 {
        partes.push(&e[fin - tam..fin]);
        fin -= tam;
        tam = if resto == 0 { fin } else { resto };
    }
    partes.push(&e[..fin]);
    partes.reverse();
    partes.join(miles)
}

fn leer_formato(p: *const u8, f: &mut Formato) {
    // SAFETY: un NUMBERFMTW/CURRENCYFMTW del `.exe`: 0 cifras, 4 cero
    // delante, 8 grupos, 16 y 24 separadores, 32 orden del negativo.
    unsafe {
        f.cifras = (p as *const u32).read_unaligned() as usize;
        f.cero_delante = (p.add(4) as *const u32).read_unaligned() != 0;
        f.grupos = (p.add(8) as *const u32).read_unaligned();
        f.decimal = texto_w((p.add(16) as *const u64).read_unaligned() as *const u16, -1).unwrap_or_default();
        f.miles = texto_w((p.add(24) as *const u64).read_unaligned() as *const u16, -1).unwrap_or_default();
        f.negativo = (p.add(32) as *const u32).read_unaligned();
    }
}

fn numero(loc: Option<Locale>, v: *const u16, fmt: *const u8, buf: *mut u16, n: i32) -> i32 {
    let Some(loc) = loc else { return invalido(ERROR_INVALID_PARAMETER) };
    let mut f = formato_de(loc, false);
    if !fmt.is_null() {
        leer_formato(fmt, &mut f);
    }
    let Some(val) = texto_w(v, -1) else { return invalido(ERROR_INVALID_PARAMETER) };
    let Some((neg, e, d)) = partir_numero(&val, &f) else { return invalido(ERROR_INVALID_PARAMETER) };
    let mut cuerpo = agrupar(&e, f.grupos, &String::from_utf16_lossy(&f.miles));
    if f.cifras > 0 {
        cuerpo.push_str(&String::from_utf16_lossy(&f.decimal));
        cuerpo.push_str(&d);
    }
    let t = if !neg {
        cuerpo
    } else {
        match f.negativo {
            0 => alloc::format!("({cuerpo})"),
            2 => alloc::format!("- {cuerpo}"),
            3 => alloc::format!("{cuerpo}-"),
            4 => alloc::format!("{cuerpo} -"),
            _ => alloc::format!("-{cuerpo}"),
        }
    };
    dar(&w(&t), buf, n)
}

extern "win64" fn get_number_format_ex(nombre: *const u16, _banderas: u32, v: *const u16, fmt: *const u8, buf: *mut u16, n: i32) -> i32 {
    numero(de_nombre(nombre), v, fmt, buf, n)
}

/// `GetCurrencyFormatEx`: en-US, "$1,234.50" y "-$1,234.50".
extern "win64" fn get_currency_format_ex(nombre: *const u16, _banderas: u32, v: *const u16, fmt: *const u8, buf: *mut u16, n: i32) -> i32 {
    let Some(loc) = de_nombre(nombre) else { return invalido(ERROR_INVALID_PARAMETER) };
    let mut f = formato_de(loc, true);
    let mut positivo = 0u32;
    let mut simbolo = w(&dato(loc, 0x14).unwrap_or_default());
    if !fmt.is_null() {
        leer_formato(fmt, &mut f);
        // SAFETY: CURRENCYFMTW: 36 el orden del positivo, 40 el simbolo.
        unsafe {
            positivo = (fmt.add(36) as *const u32).read_unaligned();
            simbolo = texto_w((fmt.add(40) as *const u64).read_unaligned() as *const u16, -1).unwrap_or_default();
        }
    }
    let Some(val) = texto_w(v, -1) else { return invalido(ERROR_INVALID_PARAMETER) };
    let Some((neg, e, d)) = partir_numero(&val, &f) else { return invalido(ERROR_INVALID_PARAMETER) };
    let mut cuerpo = agrupar(&e, f.grupos, &String::from_utf16_lossy(&f.miles));
    if f.cifras > 0 {
        cuerpo.push_str(&String::from_utf16_lossy(&f.decimal));
        cuerpo.push_str(&d);
    }
    let s = String::from_utf16_lossy(&simbolo);
    let t = if !neg {
        match positivo {
            1 => alloc::format!("{cuerpo}{s}"),
            2 => alloc::format!("{s} {cuerpo}"),
            3 => alloc::format!("{cuerpo} {s}"),
            _ => alloc::format!("{s}{cuerpo}"),
        }
    } else {
        match f.negativo {
            0 => alloc::format!("({s}{cuerpo})"),
            2 => alloc::format!("{s}-{cuerpo}"),
            3 => alloc::format!("{s}{cuerpo}-"),
            4 => alloc::format!("({cuerpo}{s})"),
            5 => alloc::format!("-{cuerpo}{s}"),
            8 => alloc::format!("-{cuerpo} {s}"),
            9 => alloc::format!("-{s} {cuerpo}"),
            _ => alloc::format!("-{s}{cuerpo}"),
        }
    };
    dar(&w(&t), buf, n)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "CompareStringW" => dir!(compare_string_w),
        "CompareStringEx" => dir!(compare_string_ex),
        "LCMapStringW" => dir!(lcmap_string_w),
        "LCMapStringEx" => dir!(lcmap_string_ex),
        "LCMapStringA" => dir!(lcmap_string_a),
        "GetLocaleInfoW" => dir!(get_locale_info_w),
        "GetLocaleInfoEx" => dir!(get_locale_info_ex),
        "GetLocaleInfoA" => dir!(get_locale_info_a),
        "GetGeoInfoW" => dir!(get_geo_info_w),
        "GetDateFormatW" => dir!(get_date_format_w),
        "GetDateFormatEx" => dir!(get_date_format_ex),
        "GetTimeFormatW" => dir!(get_time_format_w),
        "GetTimeFormatEx" => dir!(get_time_format_ex),
        "GetNumberFormatEx" => dir!(get_number_format_ex),
        "GetCurrencyFormatEx" => dir!(get_currency_format_ex),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn s(x: &str) -> Vec<u16> {
        w(x)
    }

    #[test]
    fn comparar_como_windows() {
        assert_eq!(comparar(&s("abc"), &s("ABC"), NORM_IGNORECASE), CSTR_EQUAL);
        assert_eq!(comparar(&s("a"), &s("A"), 0), CSTR_LESS_THAN, "la minuscula antes");
        assert_eq!(comparar(&s("apple"), &s("Banana"), 0), CSTR_LESS_THAN);
        assert_eq!(comparar(&s("co-op"), &s("coop"), NORM_IGNORESYMBOLS), CSTR_EQUAL);
        assert!(clave(&s("a"), 0) < clave(&s("A"), 0));
        assert!(clave(&s("A"), 0) < clave(&s("b"), 0));
    }

    #[test]
    fn fechas_horas_y_numeros() {
        let st = [2026u16, 9, 2, 29, 15, 4, 5, 0];
        assert_eq!(String::from_utf16_lossy(&formatear_fecha(&st, &s("dddd, MMMM d, yyyy"))), "Tuesday, September 29, 2026");
        assert_eq!(String::from_utf16_lossy(&formatear_fecha(&st, &s("M/d/yyyy"))), "9/29/2026");
        assert_eq!(String::from_utf16_lossy(&formatear_fecha(&st, &s("dd 'de' MMM yy"))), "29 de Sep 26");
        assert_eq!(String::from_utf16_lossy(&formatear_hora(&st, &s("h:mm:ss tt"), 0)), "3:04:05 PM");
        assert_eq!(String::from_utf16_lossy(&formatear_hora(&st, &s("h:mm:ss tt"), TIME_NOSECONDS)), "3:04 PM");
        assert_eq!(String::from_utf16_lossy(&formatear_hora(&st, &s("h:mm:ss tt"), TIME_FORCE24HOURFORMAT | TIME_NOTIMEMARKER)), "15:04:05");
        let f = formato_de(Locale::EnUs, false);
        let (neg, e, d) = partir_numero(&s("-1234567.895"), &f).unwrap();
        assert_eq!((neg, agrupar(&e, 3, ","), d.as_str()), (true, String::from("1,234,567"), "90"));
        assert_eq!(agrupar("1234567", 32, ","), "12,34,567");
        let (_, e, d) = partir_numero(&s(".5"), &f).unwrap();
        assert_eq!((e.as_str(), d.as_str()), ("0", "50"));
        let (_, e, d) = partir_numero(&s("9.999"), &f).unwrap();
        assert_eq!((e.as_str(), d.as_str()), ("10", "00"));
    }
}
