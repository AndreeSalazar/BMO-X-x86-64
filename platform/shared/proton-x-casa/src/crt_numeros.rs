//! **Los numeros del CRT, de la casa** (tanda 1 de Cyberpunk, 29-09; el
//! mapa, en `crt_cadenas.rs`):
//!
//! ```text
//!    enteros      atoi atol strtol strtoll strtoul strtoull _strtoui64 _wtoi
//!                 _wtoi64 wcstol wcstoul _itoa_s _ltoa
//!    reales       atof strtod _wtof: el double vuelve en xmm0 (la casa es
//!                 soft-float: un trampolin lo pone ahi); decimal, inf, nan
//!                 y hexadecimal
//!    printf _s    __stdio_common_vs(n)(w)printf_s, _vsnprintf_s,
//!                 _vsnwprintf(_s), vswprintf_s, swprintf_s
//!    scanf        __stdio_common_vsscanf / vswscanf, swscanf_s
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use crate::crt;
use crate::crt_cadenas::{invalido, poner_errno, trozo, trozo_w, EINVAL, ERANGE, TRUNCATE};
use crate::dir;

// -- Numeros ---------------------------------------------------------------------------------

/// Los caracteres de una cadena del `.exe` (estrecha o ancha), sin su 0.
fn chars_a(p: *const u8) -> Vec<u32> {
    trozo(p).iter().map(|&c| c as u32).collect()
}

fn chars_w(p: *const u16) -> Vec<u32> {
    trozo_w(p).iter().map(|&c| c as u32).collect()
}

fn blanco(c: u32) -> bool {
    c == 32 || (9..=13).contains(&c)
}

/// Un entero como `strtol`: `(magnitud, negativo, se paso de u64, usados)`.
/// `usados` 0 = no habia numero.
fn entero(s: &[u32], base: u32) -> (u64, bool, bool, usize) {
    let mut i = 0;
    while i < s.len() && blanco(s[i]) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' as u32 || s[i] == b'-' as u32) {
        neg = s[i] == b'-' as u32;
        i += 1;
    }
    let mut base = base;
    let hex = |j: usize| j + 1 < s.len() && s[j] == b'0' as u32 && (s[j + 1] | 0x20) == b'x' as u32;
    // "0x" solo cuenta si detras hay un digito hex: "0xg" es el 0 y ya.
    let hex_de_verdad = |j: usize| hex(j) && s.get(j + 2).is_some_and(|&c| char::from_u32(c).is_some_and(|c| c.is_ascii_hexdigit()));
    if (base == 0 || base == 16) && hex_de_verdad(i) {
        base = 16;
        i += 2;
    } else if base == 0 {
        base = if s.get(i) == Some(&(b'0' as u32)) { 8 } else { 10 };
    }
    if !(2..=36).contains(&base) {
        return (0, false, false, 0);
    }
    let mut v: u64 = 0;
    let mut pasado = false;
    let desde = i;
    while i < s.len() {
        let Some(d) = char::from_u32(s[i]).and_then(|c| c.to_digit(base)) else { break };
        match v.checked_mul(base as u64).and_then(|x| x.checked_add(d as u64)) {
            Some(x) => v = x,
            None => pasado = true,
        }
        i += 1;
    }
    if i == desde {
        return (0, false, false, 0);
    }
    (v, neg, pasado, i)
}

/// Con signo, de `bits` bits: satura y pone ERANGE.
fn con_signo(s: &[u32], base: u32, bits: u32) -> (i64, usize) {
    let (v, neg, pasado, n) = entero(s, base);
    let max = (1u64 << (bits - 1)) - 1;
    if pasado || v > max + neg as u64 {
        poner_errno(ERANGE);
        return (if neg { -(max as i64) - 1 } else { max as i64 }, n);
    }
    (if neg { (v as i64).wrapping_neg() } else { v as i64 }, n)
}

/// Sin signo, de `bits` bits: el `-` niega (como C), el que se pasa satura.
fn sin_signo(s: &[u32], base: u32, bits: u32) -> (u64, usize) {
    let (v, neg, pasado, n) = entero(s, base);
    let max = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
    if pasado || v > max {
        poner_errno(ERANGE);
        return (max, n);
    }
    (if neg { v.wrapping_neg() & max } else { v }, n)
}

/// Poner `*fin` (si lo piden) en `s + usados`, o en `s` si no hubo numero.
fn poner_fin<T>(fin: *mut *const T, s: *const T, usados: usize) {
    if !fin.is_null() {
        // SAFETY: un puntero del `.exe` donde dejar el final.
        unsafe { *fin = s.add(usados) };
    }
}

extern "win64" fn strtol(s: *const u8, fin: *mut *const u8, base: i32) -> i32 {
    let (v, n) = con_signo(&chars_a(s), base as u32, 32);
    poner_fin(fin, s, n);
    v as i32
}

extern "win64" fn strtoll(s: *const u8, fin: *mut *const u8, base: i32) -> i64 {
    let (v, n) = con_signo(&chars_a(s), base as u32, 64);
    poner_fin(fin, s, n);
    v
}

extern "win64" fn strtoul(s: *const u8, fin: *mut *const u8, base: i32) -> u32 {
    let (v, n) = sin_signo(&chars_a(s), base as u32, 32);
    poner_fin(fin, s, n);
    v as u32
}

extern "win64" fn strtoull(s: *const u8, fin: *mut *const u8, base: i32) -> u64 {
    let (v, n) = sin_signo(&chars_a(s), base as u32, 64);
    poner_fin(fin, s, n);
    v
}

extern "win64" fn wcstol(s: *const u16, fin: *mut *const u16, base: i32) -> i32 {
    let (v, n) = con_signo(&chars_w(s), base as u32, 32);
    poner_fin(fin, s, n);
    v as i32
}

extern "win64" fn wcstoul(s: *const u16, fin: *mut *const u16, base: i32) -> u32 {
    let (v, n) = sin_signo(&chars_w(s), base as u32, 32);
    poner_fin(fin, s, n);
    v as u32
}

extern "win64" fn atoi(s: *const u8) -> i32 {
    con_signo(&chars_a(s), 10, 32).0 as i32
}

extern "win64" fn wtoi(s: *const u16) -> i32 {
    con_signo(&chars_w(s), 10, 32).0 as i32
}

extern "win64" fn wtoi64(s: *const u16) -> i64 {
    con_signo(&chars_w(s), 10, 64).0
}

/// **Un double como `strtod`**: `(bits, usados)`. Decimal, `inf`,
/// `infinity` y `nan`; el hexadecimal (`0x1.8p3`) tambien.
pub(crate) fn real(s: &[u32]) -> (u64, usize) {
    let mut i = 0;
    while i < s.len() && blanco(s[i]) {
        i += 1;
    }
    let inicio = i;
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' as u32 || s[i] == b'-' as u32) {
        neg = s[i] == b'-' as u32;
        i += 1;
    }
    let signo = if neg { 1u64 << 63 } else { 0 };
    let es = |j: usize, p: &[u8]| p.iter().enumerate().all(|(k, &c)| s.get(j + k).is_some_and(|&x| x | 0x20 == c as u32));
    if es(i, b"infinity") {
        return (0x7FF0_0000_0000_0000 | signo, i + 8);
    }
    if es(i, b"inf") {
        return (0x7FF0_0000_0000_0000 | signo, i + 3);
    }
    if es(i, b"nan") {
        return (0x7FF8_0000_0000_0000 | signo, i + 3);
    }
    let digito = |j: usize, base: u32| s.get(j).and_then(|&c| char::from_u32(c)).and_then(|c| c.to_digit(base));
    // Hexadecimal: mantisa en u64 y exponente binario.
    if es(i, b"0x") && (digito(i + 2, 16).is_some() || (s.get(i + 2) == Some(&(b'.' as u32)) && digito(i + 3, 16).is_some())) {
        let mut j = i + 2;
        let mut m: u64 = 0;
        let mut e: i32 = 0;
        let mut punto = false;
        loop {
            if let Some(d) = digito(j, 16) {
                if m >> 60 == 0 {
                    m = m << 4 | d as u64;
                    if punto {
                        e -= 4;
                    }
                } else if !punto {
                    e += 4;
                }
                j += 1;
            } else if !punto && s.get(j) == Some(&(b'.' as u32)) {
                punto = true;
                j += 1;
            } else {
                break;
            }
        }
        if s.get(j).is_some_and(|&c| c | 0x20 == b'p' as u32) {
            let (v, n, _, k) = entero(&s[j + 1..], 10);
            if k > 0 && !(s.get(j + 1).is_some_and(|&c| blanco(c))) {
                let v = v.min(100_000) as i32;
                e += if n { -v } else { v };
                j += 1 + k;
            }
        }
        let x = (m as f64) * potencia2(e);
        return (x.to_bits() | signo, j);
    }
    // Decimal: lo mas largo que sea un numero, y core lo convierte.
    let mut j = i;
    let mut hay = false;
    while digito(j, 10).is_some() {
        j += 1;
        hay = true;
    }
    if s.get(j) == Some(&(b'.' as u32)) {
        j += 1;
        while digito(j, 10).is_some() {
            j += 1;
            hay = true;
        }
    }
    if !hay {
        return (0, 0);
    }
    if s.get(j).is_some_and(|&c| c | 0x20 == b'e' as u32) {
        let mut k = j + 1;
        if s.get(k).is_some_and(|&c| c == b'+' as u32 || c == b'-' as u32) {
            k += 1;
        }
        if digito(k, 10).is_some() {
            while digito(k, 10).is_some() {
                k += 1;
            }
            j = k;
        }
    }
    let texto: String = s[inicio..j].iter().filter_map(|&c| char::from_u32(c)).collect();
    let v: f64 = texto.parse().unwrap_or(0.0);
    let b = v.to_bits();
    if b & !(1 << 63) == 0x7FF0_0000_0000_0000 {
        poner_errno(ERANGE);
    }
    (b, j)
}

fn potencia2(e: i32) -> f64 {
    let e = e.clamp(-1100, 1100);
    let mut x = 1.0f64;
    let (paso, n) = if e < 0 { (0.5f64, -e) } else { (2.0f64, e) };
    for _ in 0..n {
        x *= paso;
    }
    x
}

// El double vuelve en xmm0: el trampolin lo mueve.
core::arch::global_asm!(".globl proton_x_strtod", "proton_x_strtod:", "sub rsp, 40", "call {f}", "movq xmm0, rax", "add rsp, 40", "ret", f = sym proton_x_strtod_bits);
core::arch::global_asm!(".globl proton_x_atof", "proton_x_atof:", "sub rsp, 40", "xor edx, edx", "call {f}", "movq xmm0, rax", "add rsp, 40", "ret", f = sym proton_x_strtod_bits);
core::arch::global_asm!(".globl proton_x_wtof", "proton_x_wtof:", "sub rsp, 40", "call {f}", "movq xmm0, rax", "add rsp, 40", "ret", f = sym proton_x_wtof_bits);
extern "C" {
    fn proton_x_strtod();
    fn proton_x_atof();
    fn proton_x_wtof();
}

extern "win64" fn proton_x_strtod_bits(s: *const u8, fin: *mut *const u8) -> u64 {
    let (b, n) = real(&chars_a(s));
    poner_fin(fin, s, n);
    b
}

extern "win64" fn proton_x_wtof_bits(s: *const u16) -> u64 {
    real(&chars_w(s)).0
}

/// `_itoa_s` / `_ltoa`: en `base`; con signo solo en base 10, como el CRT.
fn a_texto(v: i32, base: u32) -> Vec<u8> {
    let (mut m, neg) = if base == 10 && v < 0 { ((v as i64).unsigned_abs(), true) } else { (v as u32 as u64, false) };
    let mut t = Vec::new();
    loop {
        let d = (m % base as u64) as u8;
        t.push(if d < 10 { b'0' + d } else { b'a' + d - 10 });
        m /= base as u64;
        if m == 0 {
            break;
        }
    }
    if neg {
        t.push(b'-');
    }
    t.reverse();
    t
}

extern "win64" fn itoa_s(v: i32, buf: *mut u8, n: usize, base: i32) -> i32 {
    if buf.is_null() || n == 0 || !(2..=36).contains(&base) {
        return invalido(EINVAL);
    }
    let t = a_texto(v, base as u32);
    if t.len() + 1 > n {
        // SAFETY: `n` > 0 bytes en `buf`.
        unsafe { *buf = 0 };
        return invalido(ERANGE);
    }
    // SAFETY: cabe con su 0.
    unsafe {
        core::ptr::copy_nonoverlapping(t.as_ptr(), buf, t.len());
        *buf.add(t.len()) = 0;
    }
    0
}

extern "win64" fn ltoa(v: i32, buf: *mut u8, base: i32) -> *mut u8 {
    if (2..=36).contains(&base) {
        let t = a_texto(v, base as u32);
        // SAFETY: el `.exe` da sitio (hasta 33 bytes con base 2).
        unsafe {
            core::ptr::copy_nonoverlapping(t.as_ptr(), buf, t.len());
            *buf.add(t.len()) = 0;
        }
    }
    buf
}

// -- printf con medida -----------------------------------------------------------------------

/// Las `_s`: cabe con su 0, el largo; no cabe, `buf[0] = 0`, ERANGE y -1.
fn a_bufer_s<T: Copy + Default>(r: &[T], buf: *mut T, n: usize) -> i32 {
    if buf.is_null() || n == 0 {
        invalido(EINVAL);
        return -1;
    }
    if r.len() >= n {
        // SAFETY: `n` > 0 elementos en `buf`.
        unsafe { *buf = T::default() };
        invalido(ERANGE);
        return -1;
    }
    // SAFETY: cabe con su 0.
    unsafe {
        core::ptr::copy_nonoverlapping(r.as_ptr(), buf, r.len());
        *buf.add(r.len()) = T::default();
    }
    r.len() as i32
}

/// `_snprintf_s`: a lo mas `cuenta` (o lo que quepa con `TRUNCATE`); si se
/// corta, -1.
fn a_bufer_ns<T: Copy + Default>(r: &[T], buf: *mut T, n: usize, cuenta: usize) -> i32 {
    if buf.is_null() || n == 0 {
        invalido(EINVAL);
        return -1;
    }
    let tope = if cuenta == TRUNCATE { n - 1 } else { cuenta };
    if r.len() <= tope && r.len() < n {
        return a_bufer_s(r, buf, n);
    }
    if tope >= n {
        // SAFETY: `n` > 0 elementos en `buf`.
        unsafe { *buf = T::default() };
        invalido(ERANGE);
        return -1;
    }
    // SAFETY: `tope < n`: cabe `tope` y su 0.
    unsafe {
        core::ptr::copy_nonoverlapping(r.as_ptr(), buf, tope);
        *buf.add(tope) = T::default();
    }
    -1
}

fn ancho(r: &[u8]) -> Vec<u16> {
    String::from_utf8_lossy(r).encode_utf16().collect()
}

extern "win64" fn stdio_vsprintf_s(_op: u64, buf: *mut u8, n: usize, fmt: *const u8, _loc: u64, va: u64) -> i32 {
    a_bufer_s(&crt::formatear_a(fmt, va), buf, n)
}

extern "win64" fn stdio_vsnprintf_s(_op: u64, buf: *mut u8, n: usize, cuenta: usize, fmt: *const u8, _loc: u64, va: u64) -> i32 {
    a_bufer_ns(&crt::formatear_a(fmt, va), buf, n, cuenta)
}

extern "win64" fn stdio_vswprintf_s(op: u64, buf: *mut u16, n: usize, fmt: *const u16, _loc: u64, va: u64) -> i32 {
    a_bufer_s(&ancho(&crt::formatear_w(op, fmt, va)), buf, n)
}

extern "win64" fn stdio_vsnwprintf_s(op: u64, buf: *mut u16, n: usize, cuenta: usize, fmt: *const u16, _loc: u64, va: u64) -> i32 {
    a_bufer_ns(&ancho(&crt::formatear_w(op, fmt, va)), buf, n, cuenta)
}

// Las de msvcrt.dll: sin opciones delante, y con los especificadores anchos
// de siempre (`%s` en una funcion ancha es una cadena ancha).
extern "win64" fn vsnprintf_s(buf: *mut u8, n: usize, cuenta: usize, fmt: *const u8, va: u64) -> i32 {
    a_bufer_ns(&crt::formatear_a(fmt, va), buf, n, cuenta)
}

extern "win64" fn vsnwprintf_s(buf: *mut u16, n: usize, cuenta: usize, fmt: *const u16, va: u64) -> i32 {
    a_bufer_ns(&ancho(&crt::formatear_w(crt::ANCHOS_LEGADOS, fmt, va)), buf, n, cuenta)
}

extern "win64" fn vsnwprintf(buf: *mut u16, cuenta: usize, fmt: *const u16, va: u64) -> i32 {
    // El de siempre: sin 0 si no cabe, y -1.
    match crt::a_bufer(&ancho(&crt::formatear_w(crt::ANCHOS_LEGADOS, fmt, va)), buf, cuenta, 0) {
        -2 => -1,
        r => r,
    }
}

extern "win64" fn proton_x_vswprintf_s(buf: *mut u16, n: usize, fmt: *const u16, va: u64) -> i32 {
    a_bufer_s(&ancho(&crt::formatear_w(crt::ANCHOS_LEGADOS, fmt, va)), buf, n)
}

// `swprintf_s(buf, n, fmt, ...)`: los variadicos de Windows x64 siguen en la
// pila justo despues del hueco de r9. Se guarda r9 en SU hueco (el que el
// que llama reservo) y la direccion es el va_list.
core::arch::global_asm!(".globl proton_x_swprintf_s", "proton_x_swprintf_s:", "mov [rsp + 32], r9", "lea r9, [rsp + 32]", "jmp {f}", f = sym proton_x_vswprintf_s);
// `swscanf_s(buf, fmt, ...)`: los variadicos empiezan en r8.
core::arch::global_asm!(".globl proton_x_swscanf_s", "proton_x_swscanf_s:", "mov [rsp + 24], r8", "mov [rsp + 32], r9", "lea r8, [rsp + 24]", "jmp {f}", f = sym proton_x_vswscanf_s);
extern "C" {
    fn proton_x_swprintf_s();
    fn proton_x_swscanf_s();
}

// -- scanf -----------------------------------------------------------------------------------

/// Un `va_list` de punteros (y, en las `_s`, medidas).
struct Ranuras(*const u64);

impl Ranuras {
    fn siguiente(&mut self) -> u64 {
        // SAFETY: el `va_list` del `.exe`: una ranura por argumento.
        let v = unsafe { self.0.read_unaligned() };
        // SAFETY: la ranura siguiente.
        self.0 = unsafe { self.0.add(1) };
        v
    }
}

/// **`sscanf` y los suyos**, sobre caracteres. `ancha` = la funcion es de
/// las anchas (su `%s` y `%c` escriben `wchar_t`); `segura` = las `_s`
/// (cada `%s`, `%c` y `%[` lleva su medida detras). Los asignados, o -1 si
/// la entrada se acabo antes de la primera conversion.
fn escanear(ent: &[u32], fmt: &[u32], va: &mut Ranuras, ancha: bool, segura: bool) -> i32 {
    let (mut i, mut f) = (0usize, 0usize);
    let mut asignados = 0;
    let mut alguna = false;
    let c = |x: u8| x as u32;
    while f < fmt.len() {
        let x = fmt[f];
        if blanco(x) {
            while i < ent.len() && blanco(ent[i]) {
                i += 1;
            }
            f += 1;
            continue;
        }
        if x != c(b'%') || fmt.get(f + 1) == Some(&c(b'%')) {
            if x == c(b'%') {
                f += 1;
                while i < ent.len() && blanco(ent[i]) {
                    i += 1;
                }
            }
            if ent.get(i) != Some(&fmt[f]) {
                break;
            }
            i += 1;
            f += 1;
            continue;
        }
        f += 1;
        let saltar = fmt.get(f) == Some(&c(b'*'));
        if saltar {
            f += 1;
        }
        let mut anchura = 0usize;
        while let Some(d) = fmt.get(f).and_then(|&x| char::from_u32(x)).and_then(|x| x.to_digit(10)) {
            anchura = anchura * 10 + d as usize;
            f += 1;
        }
        // La medida: h hh l ll L I64 I32 z j t.
        let mut tam = 4usize;
        let mut largo_l = None::<bool>;
        loop {
            match fmt.get(f).copied().and_then(char::from_u32) {
                Some('h') => {
                    tam = if tam == 2 { 1 } else { 2 };
                    largo_l = Some(false);
                }
                Some('l') => {
                    tam = if largo_l == Some(true) { 8 } else { 4 };
                    largo_l = Some(true);
                }
                Some('L') | Some('j') | Some('z') | Some('t') => tam = 8,
                Some('I') => {
                    if fmt.get(f + 1) == Some(&c(b'6')) && fmt.get(f + 2) == Some(&c(b'4')) {
                        tam = 8;
                        f += 2;
                    } else if fmt.get(f + 1) == Some(&c(b'3')) && fmt.get(f + 2) == Some(&c(b'2')) {
                        tam = 4;
                        f += 2;
                    } else {
                        tam = 8;
                    }
                }
                _ => break,
            }
            f += 1;
        }
        let Some(conv) = fmt.get(f).copied().and_then(char::from_u32) else { break };
        f += 1;
        if conv != 'c' && conv != '[' && conv != 'n' {
            while i < ent.len() && blanco(ent[i]) {
                i += 1;
            }
        }
        if i >= ent.len() && conv != 'n' {
            if !alguna {
                return -1;
            }
            break;
        }
        let tope = if anchura == 0 { usize::MAX } else { anchura };
        let resto = &ent[i..ent.len().min(i.saturating_add(tope))];
        match conv {
            'd' | 'i' | 'u' | 'x' | 'X' | 'o' => {
                let base = match conv {
                    'd' | 'u' => 10,
                    'i' => 0,
                    'o' => 8,
                    _ => 16,
                };
                let (v, neg, _, n) = entero(resto, base);
                if n == 0 {
                    break;
                }
                i += n;
                alguna = true;
                if !saltar {
                    let v = if neg { v.wrapping_neg() } else { v };
                    let p = va.siguiente();
                    // SAFETY: el entero del `.exe`, de la medida que pidio.
                    unsafe {
                        match tam {
                            1 => *(p as *mut u8) = v as u8,
                            2 => (p as *mut u16).write_unaligned(v as u16),
                            8 => (p as *mut u64).write_unaligned(v),
                            _ => (p as *mut u32).write_unaligned(v as u32),
                        }
                    }
                    asignados += 1;
                }
            }
            'f' | 'e' | 'g' | 'E' | 'G' | 'a' | 'A' => {
                let (b, n) = real(resto);
                if n == 0 {
                    break;
                }
                i += n;
                alguna = true;
                if !saltar {
                    let p = va.siguiente();
                    // SAFETY: el float o double del `.exe`.
                    unsafe {
                        if tam == 8 || largo_l == Some(true) {
                            (p as *mut u64).write_unaligned(b);
                        } else {
                            (p as *mut f32).write_unaligned(f64::from_bits(b) as f32);
                        }
                    }
                    asignados += 1;
                }
            }
            's' | 'c' | 'S' | 'C' | '[' => {
                // Que caracteres entran.
                let mut set: Vec<u32> = Vec::new();
                let mut negado = false;
                if conv == '[' {
                    if fmt.get(f) == Some(&c(b'^')) {
                        negado = true;
                        f += 1;
                    }
                    if fmt.get(f) == Some(&c(b']')) {
                        set.push(c(b']'));
                        f += 1;
                    }
                    while f < fmt.len() && fmt[f] != c(b']') {
                        if fmt.get(f + 1) == Some(&c(b'-')) && fmt.get(f + 2).is_some_and(|&x| x != c(b']')) {
                            for y in fmt[f]..=fmt[f + 2] {
                                set.push(y);
                            }
                            f += 3;
                        } else {
                            set.push(fmt[f]);
                            f += 1;
                        }
                    }
                    f += 1;
                }
                let tope_c = if conv == 'c' || conv == 'C' { if anchura == 0 { 1 } else { anchura } } else { tope };
                let entra = |x: u32| match conv {
                    'c' | 'C' => true,
                    '[' => set.contains(&x) != negado,
                    _ => !blanco(x),
                };
                let mut k = 0;
                while i + k < ent.len() && k < tope_c && entra(ent[i + k]) {
                    k += 1;
                }
                if k == 0 || ((conv == 'c' || conv == 'C') && k < tope_c) {
                    break;
                }
                let texto = &ent[i..i + k];
                i += k;
                alguna = true;
                if !saltar {
                    let p = va.siguiente();
                    let medida = if segura { va.siguiente() as u32 as usize } else { usize::MAX };
                    let con_cero = !(conv == 'c' || conv == 'C');
                    let hace_falta = k + con_cero as usize;
                    if hace_falta > medida {
                        if medida > 0 && con_cero {
                            // SAFETY: al menos un elemento en el destino.
                            unsafe {
                                if ancha_de(conv, largo_l, ancha) {
                                    *(p as *mut u16) = 0
                                } else {
                                    *(p as *mut u8) = 0
                                }
                            };
                        }
                        break;
                    }
                    // SAFETY: el destino del `.exe` tiene `hace_falta` elementos.
                    unsafe {
                        if ancha_de(conv, largo_l, ancha) {
                            let d = p as *mut u16;
                            for (j, &x) in texto.iter().enumerate() {
                                *d.add(j) = x as u16;
                            }
                            if con_cero {
                                *d.add(k) = 0;
                            }
                        } else {
                            let d = p as *mut u8;
                            for (j, &x) in texto.iter().enumerate() {
                                *d.add(j) = x as u8;
                            }
                            if con_cero {
                                *d.add(k) = 0;
                            }
                        }
                    }
                    asignados += 1;
                }
            }
            'n' => {
                if !saltar {
                    let p = va.siguiente();
                    // SAFETY: el int del `.exe`.
                    unsafe { (p as *mut i32).write_unaligned(i as i32) };
                }
            }
            _ => break,
        }
    }
    asignados
}

/// Si una conversion de cadena escribe `wchar_t`: `%ls`/`%lc`, `%S`/`%C` en
/// una estrecha, y lo sin letra en una ancha (`%hs` siempre estrecho).
fn ancha_de(conv: char, largo_l: Option<bool>, ancha: bool) -> bool {
    match largo_l {
        Some(l) => l,
        None => (conv == 'S' || conv == 'C') != ancha,
    }
}

/// Las opciones de `__stdio_common_vsscanf`: `_CRT_INTERNAL_SCANF_SECURECRT`.
const SCANF_SEGURO: u64 = 1;

pub(crate) fn entrada_de<T: Copy + Into<u32>>(p: *const T, n: usize) -> Vec<u32> {
    let mut v = Vec::new();
    // SAFETY: hasta `n` (o `usize::MAX`: hasta el 0) elementos del `.exe`.
    while v.len() < n {
        let x: u32 = unsafe { *p.add(v.len()) }.into();
        if x == 0 {
            break;
        }
        v.push(x);
    }
    v
}

extern "win64" fn stdio_vsscanf(op: u64, buf: *const u8, n: usize, fmt: *const u8, _loc: u64, va: u64) -> i32 {
    escanear(&entrada_de(buf, n), &chars_a(fmt), &mut Ranuras(va as *const u64), false, op & SCANF_SEGURO != 0)
}

extern "win64" fn stdio_vswscanf(op: u64, buf: *const u16, n: usize, fmt: *const u16, _loc: u64, va: u64) -> i32 {
    escanear(&entrada_de(buf, n), &chars_w(fmt), &mut Ranuras(va as *const u64), true, op & SCANF_SEGURO != 0)
}

extern "win64" fn proton_x_vswscanf_s(buf: *const u16, fmt: *const u16, va: u64) -> i32 {
    escanear(&chars_w(buf), &chars_w(fmt), &mut Ranuras(va as *const u64), true, true)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "atoi" | "atol" => dir!(atoi),
        "_wtoi" => dir!(wtoi),
        "_wtoi64" => dir!(wtoi64),
        "strtol" => dir!(strtol),
        "strtoll" => dir!(strtoll),
        "strtoul" => dir!(strtoul),
        "strtoull" | "_strtoui64" => dir!(strtoull),
        "wcstol" => dir!(wcstol),
        "wcstoul" => dir!(wcstoul),
        "strtod" => dir!(proton_x_strtod),
        "atof" => dir!(proton_x_atof),
        "_wtof" => dir!(proton_x_wtof),
        "_itoa_s" => dir!(itoa_s),
        "_ltoa" => dir!(ltoa),
        "__stdio_common_vsprintf_s" => dir!(stdio_vsprintf_s),
        "__stdio_common_vsnprintf_s" => dir!(stdio_vsnprintf_s),
        "__stdio_common_vswprintf_s" => dir!(stdio_vswprintf_s),
        "__stdio_common_vsnwprintf_s" => dir!(stdio_vsnwprintf_s),
        "_vsnprintf_s" => dir!(vsnprintf_s),
        "_vsnwprintf_s" => dir!(vsnwprintf_s),
        "_vsnwprintf" => dir!(vsnwprintf),
        "vswprintf_s" => dir!(proton_x_vswprintf_s),
        "swprintf_s" => dir!(proton_x_swprintf_s),
        "__stdio_common_vsscanf" => dir!(stdio_vsscanf),
        "__stdio_common_vswscanf" => dir!(stdio_vswscanf),
        "swscanf_s" => dir!(proton_x_swscanf_s),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cs(s: &str) -> Vec<u32> {
        s.chars().map(|c| c as u32).collect()
    }

    #[test]
    fn enteros_como_strtol() {
        assert_eq!(con_signo(&cs("  -123abc"), 10, 32), (-123, 6));
        assert_eq!(con_signo(&cs("0x1F"), 0, 32), (31, 4));
        assert_eq!(con_signo(&cs("017"), 0, 32), (15, 3));
        assert_eq!(con_signo(&cs("0xg"), 16, 32), (0, 1));
        assert_eq!(con_signo(&cs("99999999999"), 10, 32).0, i32::MAX as i64);
        assert_eq!(sin_signo(&cs("-1"), 10, 32).0, u32::MAX as u64);
        assert_eq!(sin_signo(&cs("18446744073709551615"), 10, 64).0, u64::MAX);
        assert_eq!(con_signo(&cs("zz"), 10, 32).1, 0);
    }

    #[test]
    fn reales_como_strtod() {
        assert_eq!(real(&cs(" 3.25e2x")), (325.0f64.to_bits(), 7));
        assert_eq!(real(&cs("-inf")).0, f64::NEG_INFINITY.to_bits());
        assert_eq!(real(&cs("0x1.8p3")).0, 12.0f64.to_bits());
        assert_eq!(real(&cs(".5")).0, 0.5f64.to_bits());
        assert_eq!(real(&cs("1e")), (1.0f64.to_bits(), 1));
        assert_eq!(real(&cs("abc")).1, 0);
    }

    #[test]
    fn scanf_de_numeros_cadenas_y_conjuntos() {
        let (mut a, mut b, mut x) = (0i32, 0u64, 0f64);
        let mut s = [0u8; 16];
        let mut f = [0f32; 1];
        let ranuras = [&mut a as *mut i32 as u64, &mut b as *mut u64 as u64, &mut x as *mut f64 as u64, s.as_mut_ptr() as u64, 16, f.as_mut_ptr() as u64];
        let n = escanear(&cs("  42 ff 2.5 hola:mundo 7"), &cs("%d %llx %lf %[^:]:%*s %f"), &mut Ranuras(ranuras.as_ptr()), false, true);
        assert_eq!(n, 5);
        assert_eq!((a, b, x), (42, 255, 2.5));
        assert_eq!(&s[..5], b"hola\0");
        assert_eq!(f[0], 7.0);
        assert_eq!(escanear(&cs(""), &cs("%d"), &mut Ranuras(ranuras.as_ptr()), false, false), -1);
    }
}
