//! **Las cadenas y la memoria de C**: `memcpy`, `strlen`, `strcmp`... y lo
//! que un juego de los 90 da por hecho (`strcasecmp`, `toupper`, `atoi`,
//! `strtol`). Sin monton (salvo `strdup`) y sin syscalls: solo cuentas.
//!
//! [!] El crate es `#![no_builtins]`: sin eso, el compilador reconoce el bucle
//! de `memcpy` como "una copia" y lo cambia por... una llamada a `memcpy`. En
//! el `.bex`, `memcpy` es ESTA: se llamaria a si misma para siempre.
//!
//! Las clases de caracteres son ASCII: no hay `locale` en BMO-X.

use core::ffi::{c_char, c_void};

// Los cinco que tambien usa el compilador llevan la firma EXACTA de C
// (`void *`, `char *`): con `u8`, el `.bex` los enlazaba igual, pero rustc
// avisaba en cada compilacion de que no son los que espera. Por dentro, las
// de bytes de abajo.

/// Copiar `n` bytes; si se pisan, en el sentido que no se come lo que falta
/// (asi `memcpy` y `memmove` son la misma, y ninguna sorprende).
pub unsafe fn copiar(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if n == 0 || dest.is_null() || src.is_null() {
        return dest;
    }
    if dest as usize > src as usize {
        for i in (0..n).rev() {
            *dest.add(i) = *src.add(i);
        }
    } else {
        for i in 0..n {
            *dest.add(i) = *src.add(i);
        }
    }
    dest
}

/// Lo que mide una cadena C de bytes.
pub unsafe fn largo(s: *const u8) -> usize {
    if s.is_null() {
        return 0;
    }
    let mut n = 0;
    while *s.add(n) != 0 {
        n += 1;
    }
    n
}

/// Comparar `n` bytes como `unsigned char`.
pub unsafe fn comparar(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let (a, b) = (*s1.add(i), *s2.add(i));
        if a != b {
            return a as i32 - b as i32;
        }
    }
    0
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    copiar(dest.cast(), src.cast(), n).cast()
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    copiar(dest.cast(), src.cast(), n).cast()
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let p: *mut u8 = s.cast();
    if !p.is_null() {
        for i in 0..n {
            *p.add(i) = c as u8;
        }
    }
    s
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    comparar(s1.cast(), s2.cast(), n)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    largo(s.cast())
}

/// Compare two null-terminated strings.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strcmp(s1: *const u8, s2: *const u8) -> i32 {
    let mut i = 0;
    loop {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b { return a as i32 - b as i32; }
        if a == 0 { return 0; }
        i += 1;
    }
}

/// Compare up to `n` characters of `s1` and `s2`.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strncmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b { return a as i32 - b as i32; }
        if a == 0 { return 0; }
    }
    0
}

/// Copy `src` to `dest`, including the null terminator.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strcpy(dest: *mut u8, src: *const u8) -> *mut u8 {
    let mut i = 0;
    loop {
        let c = *src.add(i);
        *dest.add(i) = c;
        if c == 0 { break; }
        i += 1;
    }
    dest
}

/// Copy at most `n` characters from `src` to `dest`, padding with \0.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strncpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        let c = *src.add(i);
        *dest.add(i) = c;
        if c == 0 {
            i += 1;
            while i < n { *dest.add(i) = 0; i += 1; }
            break;
        }
        i += 1;
    }
    dest
}

/// Find the first occurrence of `c` in `s`. Returns pointer or null.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strchr(s: *const u8, c: i32) -> *const u8 {
    let mut i = 0;
    loop {
        let ch = *s.add(i);
        if ch == c as u8 { return s.add(i); }
        if ch == 0 { break; }
        i += 1;
    }
    core::ptr::null()
}

/// Find the last occurrence of `c` in `s`.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strrchr(s: *const u8, c: i32) -> *const u8 {
    let len = largo(s);
    for i in (0..=len).rev() {
        if *s.add(i) == c as u8 { return s.add(i); }
    }
    core::ptr::null()
}

/// Find `needle` in `haystack`. Returns pointer or null.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strstr(haystack: *const u8, needle: *const u8) -> *const u8 {
    if *needle == 0 { return haystack; }
    let nlen = largo(needle);
    let hlen = largo(haystack);
    if nlen > hlen { return core::ptr::null(); }
    for i in 0..=(hlen - nlen) {
        if comparar(haystack.add(i), needle, nlen) == 0 {
            return haystack.add(i);
        }
    }
    core::ptr::null()
}

/// Duplicate a string (malloc + copy). Returns pointer to copy.
/// Caller must free() the result.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strdup(s: *const u8) -> *mut u8 {
    let len = largo(s);
    let p = crate::heap::malloc(len + 1) as *mut u8;
    if p.is_null() { return p; }
    strcpy(p, s);
    p
}

/// Duplicate at most `n` characters.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strndup(s: *const u8, n: usize) -> *mut u8 {
    let len = largo(s).min(n);
    let p = crate::heap::malloc(len + 1) as *mut u8;
    if p.is_null() { return p; }
    for i in 0..len { *p.add(i) = *s.add(i); }
    *p.add(len) = 0;
    p
}

/// Append `src` to `dest`. dest must have enough space.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strcat(dest: *mut u8, src: *const u8) -> *mut u8 {
    let dlen = largo(dest);
    strcpy(dest.add(dlen), src);
    dest
}

/// Append at most `n` characters, and ALWAYS the terminating zero (que es lo
/// que C promete y no lo que hacia esto: con `strncpy` debajo, un `src` largo
/// dejaba `dest` sin cerrar).
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strncat(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let d = dest.add(largo(dest));
    let mut i = 0;
    while i < n && *src.add(i) != 0 {
        *d.add(i) = *src.add(i);
        i += 1;
    }
    *d.add(i) = 0;
    dest
}

// -- Mayusculas, minusculas y clases (ASCII) ---------------------------------

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn toupper(c: i32) -> i32 {
    if (b'a' as i32..=b'z' as i32).contains(&c) { c - 32 } else { c }
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn tolower(c: i32) -> i32 {
    if (b'A' as i32..=b'Z' as i32).contains(&c) { c + 32 } else { c }
}

fn clase(c: i32, f: fn(&u8) -> bool) -> bool {
    (0..=127).contains(&c) && f(&(c as u8))
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn isdigit(c: i32) -> i32 {
    clase(c, u8::is_ascii_digit) as i32
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn isalpha(c: i32) -> i32 {
    clase(c, u8::is_ascii_alphabetic) as i32
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn isalnum(c: i32) -> i32 {
    clase(c, u8::is_ascii_alphanumeric) as i32
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn isupper(c: i32) -> i32 {
    clase(c, u8::is_ascii_uppercase) as i32
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn islower(c: i32) -> i32 {
    clase(c, u8::is_ascii_lowercase) as i32
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn isxdigit(c: i32) -> i32 {
    clase(c, u8::is_ascii_hexdigit) as i32
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn isprint(c: i32) -> i32 {
    (0x20..0x7F).contains(&c) as i32
}

/// Espacio, tabulador, salto, retorno, tabulador vertical, salto de pagina.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn isspace(c: i32) -> i32 {
    matches!(c, 0x20 | 0x09..=0x0D) as i32
}

/// `strcmp` sin mirar mayusculas: DOOM compara asi los nombres de lump.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strcasecmp(s1: *const u8, s2: *const u8) -> i32 {
    strncasecmp(s1, s2, usize::MAX)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strncasecmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let a = tolower(*s1.add(i) as i32);
        let b = tolower(*s2.add(i) as i32);
        if a != b {
            return a - b;
        }
        if a == 0 {
            return 0;
        }
        i += 1;
    }
    0
}

// -- Numeros desde texto -------------------------------------------------------

/// `strtol`: espacios, signo, `0x`/`0` con base 0 o 16, y las cifras que haya.
/// Satura en vez de desbordar. `fin`, si no es nulo, queda detras de lo leido.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strtol(s: *const u8, fin: *mut *const u8, base: i32) -> i64 {
    let (v, neg, p) = numero(s, base);
    if !fin.is_null() {
        *fin = p;
    }
    if neg {
        if v > i64::MAX as u64 + 1 { i64::MIN } else { (v as i64).wrapping_neg() }
    } else {
        v.min(i64::MAX as u64) as i64
    }
}

/// `strtoul`: como `strtol`, sin signo (un `-` da la vuelta, como C).
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn strtoul(s: *const u8, fin: *mut *const u8, base: i32) -> u64 {
    let (v, neg, p) = numero(s, base);
    if !fin.is_null() {
        *fin = p;
    }
    if neg { v.wrapping_neg() } else { v }
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn atoi(s: *const u8) -> i32 {
    strtol(s, core::ptr::null_mut(), 10) as i32
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn atol(s: *const u8) -> i64 {
    strtol(s, core::ptr::null_mut(), 10)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn abs(v: i32) -> i32 {
    v.wrapping_abs()
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub extern "C" fn labs(v: i64) -> i64 {
    v.wrapping_abs()
}

/// `(valor, negativo, donde acabo)`. Sin cifras, acaba en `s` (lo que C pide).
unsafe fn numero(s: *const u8, base: i32) -> (u64, bool, *const u8) {
    let mut p = s;
    while isspace(*p as i32) != 0 {
        p = p.add(1);
    }
    let neg = *p == b'-';
    if *p == b'-' || *p == b'+' {
        p = p.add(1);
    }
    let mut base = base as u64;
    let hex = *p == b'0' && (*p.add(1) | 0x20) == b'x' && (*p.add(2) as char).is_ascii_hexdigit();
    if (base == 0 || base == 16) && hex {
        p = p.add(2);
        base = 16;
    } else if base == 0 {
        base = if *p == b'0' { 8 } else { 10 };
    }
    if !(2..=36).contains(&base) {
        return (0, false, s);
    }
    let desde = p;
    let mut v = 0u64;
    loop {
        let d = match *p {
            c @ b'0'..=b'9' => (c - b'0') as u64,
            c @ (b'a'..=b'z' | b'A'..=b'Z') => ((c | 0x20) - b'a') as u64 + 10,
            _ => break,
        };
        if d >= base {
            break;
        }
        v = v.saturating_mul(base).saturating_add(d);
        p = p.add(1);
    }
    if p == desde {
        return (0, false, s);
    }
    (v, neg, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> std::vec::Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        v
    }

    #[test]
    fn memoria() {
        let mut a = *b"0123456789";
        unsafe {
            memmove(a.as_mut_ptr().add(2).cast(), a.as_ptr().cast(), 5);
            assert_eq!(&a, b"0101234789");
            memset(a.as_mut_ptr().cast(), b'x' as i32, 3);
            assert_eq!(&a[..4], b"xxx1");
            assert!(memcmp(b"abc".as_ptr().cast(), b"abd".as_ptr().cast(), 3) < 0);
            assert_eq!(memcmp(b"abc".as_ptr().cast(), b"abd".as_ptr().cast(), 2), 0);
        }
    }

    #[test]
    fn cadenas() {
        unsafe {
            assert_eq!(strlen(c("PLAYPAL").as_ptr().cast()), 7);
            assert_eq!(strcmp(c("a").as_ptr(), c("a").as_ptr()), 0);
            assert!(strcmp(c("a").as_ptr(), c("b").as_ptr()) < 0);
            assert!(strcmp(c("\u{e9}").as_ptr(), c("a").as_ptr()) > 0, "como unsigned char");
            let h = c("e1m1.wad");
            assert_eq!(strchr(h.as_ptr(), b'.' as i32), h.as_ptr().add(4));
            assert_eq!(strrchr(h.as_ptr(), b'1' as i32), h.as_ptr().add(3));
            assert_eq!(strstr(h.as_ptr(), c("m1").as_ptr()), h.as_ptr().add(2));
            assert!(strstr(h.as_ptr(), c("zz").as_ptr()).is_null());
        }
    }

    #[test]
    fn strncat_siempre_cierra() {
        let mut d = [0u8; 16];
        d[..3].copy_from_slice(b"ab\0");
        unsafe { strncat(d.as_mut_ptr(), c("cdefgh").as_ptr(), 3) };
        assert_eq!(&d[..6], b"abcde\0");
    }

    #[test]
    fn sin_mayusculas() {
        unsafe {
            assert_eq!(strcasecmp(c("PlayPal").as_ptr(), c("PLAYPAL").as_ptr()), 0);
            assert!(strcasecmp(c("a").as_ptr(), c("B").as_ptr()) < 0);
            assert_eq!(strncasecmp(c("E1M1x").as_ptr(), c("e1m1Y").as_ptr(), 4), 0);
        }
        assert_eq!(toupper(b'q' as i32), b'Q' as i32);
        assert_eq!(tolower(b'Q' as i32), b'q' as i32);
        assert_eq!(toupper(-1), -1);
        assert_eq!(isspace(b'\t' as i32), 1);
        assert_eq!(isdigit(b'7' as i32), 1);
        assert_eq!(isalpha(200), 0);
    }

    #[test]
    fn numeros() {
        unsafe {
            assert_eq!(atoi(c("  -42xyz").as_ptr()), -42);
            assert_eq!(atoi(c("+7").as_ptr()), 7);
            assert_eq!(atoi(c("nada").as_ptr()), 0);
            let s = c("0x1F resto");
            let mut fin = core::ptr::null();
            assert_eq!(strtol(s.as_ptr(), &mut fin, 0), 31);
            assert_eq!(fin, s.as_ptr().add(4));
            assert_eq!(strtol(c("017").as_ptr(), core::ptr::null_mut(), 0), 15);
            assert_eq!(strtol(c("zz").as_ptr(), core::ptr::null_mut(), 36), 35 * 36 + 35);
            assert_eq!(strtol(c("99999999999999999999").as_ptr(), core::ptr::null_mut(), 10), i64::MAX);
            let v = c("x");
            let mut fin = core::ptr::null();
            assert_eq!(strtol(v.as_ptr(), &mut fin, 10), 0);
            assert_eq!(fin, v.as_ptr(), "sin cifras, acaba donde empezo");
            assert_eq!(strtoul(c("ff").as_ptr(), core::ptr::null_mut(), 16), 255);
        }
        assert_eq!(abs(-5), 5);
        assert_eq!(labs(i64::MIN + 1), i64::MAX);
    }
}
