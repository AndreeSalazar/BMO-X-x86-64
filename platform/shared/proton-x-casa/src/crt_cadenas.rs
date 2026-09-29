//! **Las cadenas del CRT, de la casa** (tanda 1 de Cyberpunk, 29-09): lo que
//! el censo de `Cyberpunk2077.exe` y sus DLL pide de `msvcrt.dll`,
//! `vcruntime140.dll` y los `api-ms-win-crt-*`, en tres ficheros:
//!
//! ```text
//!    crt_cadenas  (este) errno; cadenas: strchr strrchr strstr strspn
//!                 strcspn strpbrk strnlen strncpy strncat _strdup _stricmp
//!                 _strnicmp _strlwr _mbscmp _memicmp strerror; las _s
//!                 (strcpy_s, strcat_s, strncpy_s, strncat_s, memcpy_s) con
//!                 sus errno; anchas: wcschr wcsrchr wcsstr wcscmp wcsncmp
//!                 wcsnlen _wcsdup _wcsicmp _wcsnicmp _wcslwr(_s) _wcsupr
//!                 wcscpy_s wcscat_s wcsncpy_s wcsncat_s wmemcpy_s;
//!                 caracteres (isalpha... __pctype_func) y el locale "C"
//!                 (setlocale, localeconv, ___lc_*_func)
//!    crt_numeros  strtol y los suyos, strtod (el double en xmm0), _itoa_s,
//!                 printf _s, scanf
//!    crt_entorno  getenv y los suyos, la hora, las rutas, el proceso y lo de
//!                 C++ que es de C
//! ```
//!
//! Lo que no, dicho: el locale es SIEMPRE "C" (setlocale de otro devuelve
//! NULL, como Windows con un nombre que no conoce).

use core::cell::UnsafeCell;

use crate::crt;
use crate::dir;

pub(crate) const EINVAL: i32 = 22;
pub(crate) const ERANGE: i32 = 34;
pub(crate) const ENOENT: i32 = 2;
pub(crate) const EACCES: i32 = 13;
pub(crate) const EBADF: i32 = 9;
pub(crate) const EEXIST: i32 = 17;
const STRUNCATE: i32 = 80;
/// `_TRUNCATE`: "lo que quepa".
pub(crate) const TRUNCATE: usize = usize::MAX;

struct Estado {
    errno: i32,
    doserrno: u32,
    nerr: i32,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { errno: 0, doserrno: 0, nerr: 43 }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia de un turno a otro.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.errno = 0;
    e.doserrno = 0;
}

/// Poner `errno` (para las demas partes del CRT de la casa).
pub(crate) fn poner_errno(v: i32) {
    estado().errno = v;
}

/// El `errno` de ahora.
pub(crate) fn errno_actual() -> i32 {
    estado().errno
}

extern "win64" fn errno() -> *mut i32 {
    &mut estado().errno
}

extern "win64" fn doserrno() -> *mut u32 {
    &mut estado().doserrno
}

extern "win64" fn sys_nerr() -> *mut i32 {
    &mut estado().nerr
}

// -- Cadenas estrechas -----------------------------------------------------------------------

pub(crate) fn largo(s: *const u8) -> usize {
    let mut n = 0;
    // SAFETY: una cadena del `.exe` acabada en 0.
    while unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

pub(crate) fn largo_w(s: *const u16) -> usize {
    let mut n = 0;
    // SAFETY: una cadena UTF-16 del `.exe` acabada en 0.
    while unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

/// La cadena del `.exe` como slice, sin su 0.
pub(crate) fn trozo<'a>(s: *const u8) -> &'a [u8] {
    // SAFETY: `largo` bytes de una cadena del `.exe`.
    unsafe { core::slice::from_raw_parts(s, largo(s)) }
}

pub(crate) fn trozo_w<'a>(s: *const u16) -> &'a [u16] {
    // SAFETY: como `trozo`.
    unsafe { core::slice::from_raw_parts(s, largo_w(s)) }
}

extern "win64" fn strchr(s: *const u8, c: i32) -> *const u8 {
    let c = c as u8;
    let mut p = s;
    loop {
        // SAFETY: dentro de la cadena, hasta su 0 (que tambien se encuentra).
        let v = unsafe { *p };
        if v == c {
            return p;
        }
        if v == 0 {
            return core::ptr::null();
        }
        // SAFETY: como arriba.
        p = unsafe { p.add(1) };
    }
}

extern "win64" fn strrchr(s: *const u8, c: i32) -> *const u8 {
    let t = trozo(s);
    let c = c as u8;
    if c == 0 {
        // SAFETY: el 0 del final.
        return unsafe { s.add(t.len()) };
    }
    match t.iter().rposition(|&x| x == c) {
        // SAFETY: dentro de la cadena.
        Some(i) => unsafe { s.add(i) },
        None => core::ptr::null(),
    }
}

fn buscar_en<T: PartialEq>(h: &[T], n: &[T]) -> Option<usize> {
    if n.is_empty() {
        return Some(0);
    }
    h.windows(n.len()).position(|w| w == n)
}

extern "win64" fn strstr(h: *const u8, n: *const u8) -> *const u8 {
    match buscar_en(trozo(h), trozo(n)) {
        // SAFETY: dentro de la cadena.
        Some(i) => unsafe { h.add(i) },
        None => core::ptr::null(),
    }
}

extern "win64" fn strspn(s: *const u8, set: *const u8) -> usize {
    let set = trozo(set);
    trozo(s).iter().take_while(|c| set.contains(c)).count()
}

extern "win64" fn strcspn(s: *const u8, set: *const u8) -> usize {
    let set = trozo(set);
    trozo(s).iter().take_while(|c| !set.contains(c)).count()
}

extern "win64" fn strpbrk(s: *const u8, set: *const u8) -> *const u8 {
    let set = trozo(set);
    match trozo(s).iter().position(|c| set.contains(c)) {
        // SAFETY: dentro de la cadena.
        Some(i) => unsafe { s.add(i) },
        None => core::ptr::null(),
    }
}

extern "win64" fn strnlen(s: *const u8, max: usize) -> usize {
    let mut n = 0;
    // SAFETY: hasta `max` bytes o el 0, lo que llegue antes.
    while n < max && unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

extern "win64" fn wcsnlen(s: *const u16, max: usize) -> usize {
    let mut n = 0;
    // SAFETY: como `strnlen`.
    while n < max && unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

extern "win64" fn strncpy(d: *mut u8, s: *const u8, n: usize) -> *mut u8 {
    let k = strnlen(s, n);
    // SAFETY: `n` bytes en `d`; `k` <= n de `s`. El resto, a cero (C).
    unsafe {
        core::ptr::copy(s, d, k);
        core::ptr::write_bytes(d.add(k), 0, n - k);
    }
    d
}

extern "win64" fn strncat(d: *mut u8, s: *const u8, n: usize) -> *mut u8 {
    let a = largo(d);
    let k = strnlen(s, n);
    // SAFETY: el `.exe` garantiza sitio para `a + k + 1`.
    unsafe {
        core::ptr::copy(s, d.add(a), k);
        *d.add(a + k) = 0;
    }
    d
}

pub(crate) extern "win64" fn strdup(s: *const u8) -> *mut u8 {
    if s.is_null() {
        return core::ptr::null_mut();
    }
    let n = largo(s) + 1;
    let p = crt::malloc(n) as *mut u8;
    if !p.is_null() {
        // SAFETY: `n` bytes recien pedidos.
        unsafe { core::ptr::copy_nonoverlapping(s, p, n) };
    }
    p
}

extern "win64" fn wcsdup(s: *const u16) -> *mut u16 {
    if s.is_null() {
        return core::ptr::null_mut();
    }
    let n = largo_w(s) + 1;
    let p = crt::malloc(2 * n) as *mut u16;
    if !p.is_null() {
        // SAFETY: `n` caracteres recien pedidos.
        unsafe { core::ptr::copy_nonoverlapping(s, p, n) };
    }
    p
}

/// Comparar como `_stricmp`: en minusculas ASCII (las del locale "C").
fn cmp_sin_mayusculas<T: Copy + Into<u32>>(a: *const T, b: *const T, max: usize) -> i32 {
    let bajar = |c: u32| if (b'A' as u32..=b'Z' as u32).contains(&c) { c + 32 } else { c };
    for i in 0..max {
        // SAFETY: dos cadenas del `.exe`: se para en el primer 0.
        let (x, y) = unsafe { ((*a.add(i)).into(), (*b.add(i)).into()) };
        let (x, y) = (bajar(x), bajar(y));
        if x != y || x == 0 {
            return x as i32 - y as i32;
        }
    }
    0
}

extern "win64" fn stricmp(a: *const u8, b: *const u8) -> i32 {
    cmp_sin_mayusculas(a, b, usize::MAX)
}

extern "win64" fn strnicmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    cmp_sin_mayusculas(a, b, n)
}

extern "win64" fn wcsicmp(a: *const u16, b: *const u16) -> i32 {
    cmp_sin_mayusculas(a, b, usize::MAX)
}

extern "win64" fn wcsnicmp(a: *const u16, b: *const u16, n: usize) -> i32 {
    cmp_sin_mayusculas(a, b, n)
}

extern "win64" fn memicmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    for i in 0..n {
        // SAFETY: `n` bytes del `.exe` en los dos lados.
        let (x, y) = unsafe { ((*a.add(i)).to_ascii_lowercase(), (*b.add(i)).to_ascii_lowercase()) };
        if x != y {
            return x as i32 - y as i32;
        }
    }
    0
}

extern "win64" fn strcmp(a: *const u8, b: *const u8) -> i32 {
    let (a, b) = (trozo(a), trozo(b));
    for i in 0..=a.len().min(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x as i32 - y as i32;
        }
    }
    0
}

extern "win64" fn strlwr(s: *mut u8) -> *mut u8 {
    let n = largo(s);
    // SAFETY: la cadena del `.exe`, en su sitio.
    unsafe { core::slice::from_raw_parts_mut(s, n) }.make_ascii_lowercase();
    s
}

fn cambiar_w(s: *mut u16, n: usize, subir: bool) {
    for i in 0..n {
        // SAFETY: `n` caracteres de la cadena del `.exe`.
        let c = unsafe { &mut *s.add(i) };
        if subir && (b'a' as u16..=b'z' as u16).contains(c) {
            *c -= 32;
        } else if !subir && (b'A' as u16..=b'Z' as u16).contains(c) {
            *c += 32;
        }
    }
}

extern "win64" fn wcslwr(s: *mut u16) -> *mut u16 {
    cambiar_w(s, largo_w(s), false);
    s
}

extern "win64" fn wcslwr_s(s: *mut u16, n: usize) -> i32 {
    if s.is_null() {
        return EINVAL;
    }
    let k = wcsnlen(s, n);
    if k == n {
        return EINVAL;
    }
    cambiar_w(s, k, false);
    0
}

extern "win64" fn wcsupr(s: *mut u16) -> *mut u16 {
    cambiar_w(s, largo_w(s), true);
    s
}

extern "win64" fn strerror(e: i32) -> *const u8 {
    let t: &'static [u8] = match e {
        0 => b"No error\0",
        2 => b"No such file or directory\0",
        9 => b"Bad file descriptor\0",
        12 => b"Not enough space\0",
        13 => b"Permission denied\0",
        17 => b"File exists\0",
        22 => b"Invalid argument\0",
        24 => b"Too many open files\0",
        28 => b"No space left on device\0",
        33 => b"Domain error\0",
        34 => b"Result too large\0",
        _ => b"Unknown error\0",
    };
    t.as_ptr()
}

// -- Anchas ----------------------------------------------------------------------------------

extern "win64" fn wcschr(s: *const u16, c: u16) -> *const u16 {
    let t = trozo_w(s);
    if c == 0 {
        // SAFETY: el 0 del final.
        return unsafe { s.add(t.len()) };
    }
    match t.iter().position(|&x| x == c) {
        // SAFETY: dentro de la cadena.
        Some(i) => unsafe { s.add(i) },
        None => core::ptr::null(),
    }
}

extern "win64" fn wcsrchr(s: *const u16, c: u16) -> *const u16 {
    let t = trozo_w(s);
    if c == 0 {
        // SAFETY: el 0 del final.
        return unsafe { s.add(t.len()) };
    }
    match t.iter().rposition(|&x| x == c) {
        // SAFETY: dentro de la cadena.
        Some(i) => unsafe { s.add(i) },
        None => core::ptr::null(),
    }
}

extern "win64" fn wcsstr(h: *const u16, n: *const u16) -> *const u16 {
    match buscar_en(trozo_w(h), trozo_w(n)) {
        // SAFETY: dentro de la cadena.
        Some(i) => unsafe { h.add(i) },
        None => core::ptr::null(),
    }
}

extern "win64" fn wcscmp(a: *const u16, b: *const u16) -> i32 {
    wcsncmp(a, b, usize::MAX)
}

extern "win64" fn wcsncmp(a: *const u16, b: *const u16, n: usize) -> i32 {
    for i in 0..n {
        // SAFETY: dos cadenas del `.exe`: se para en el primer 0.
        let (x, y) = unsafe { (*a.add(i), *b.add(i)) };
        if x != y || x == 0 {
            return x as i32 - y as i32;
        }
    }
    0
}

// -- Las _s: copiar y juntar con medida ------------------------------------------------------

/// `strcpy_s` y las suyas, para `u8` y `u16`: `dst` tiene `n` elementos;
/// se pone `src` (hasta `cuenta`, o `TRUNCATE`) detras de lo que haya si
/// `juntar`. Lo que no cabe: `dst[0] = 0` y ERANGE, o STRUNCATE con
/// `TRUNCATE`.
fn copiar_s<T: Copy + Default + PartialEq>(dst: *mut T, n: usize, src: *const T, cuenta: usize, juntar: bool) -> i32 {
    if dst.is_null() || n == 0 {
        return EINVAL;
    }
    let cero = T::default();
    let vacio = |e: i32| {
        // SAFETY: `n` > 0 elementos en `dst`.
        unsafe { *dst = cero };
        poner_errno(e);
        e
    };
    // Donde empieza: al final de lo que ya hay (juntar) o al principio.
    let mut desde = 0;
    if juntar {
        // SAFETY: hasta `n` elementos de `dst`.
        while desde < n && unsafe { *dst.add(desde) } != cero {
            desde += 1;
        }
        if desde == n {
            return vacio(EINVAL);
        }
    }
    if src.is_null() {
        return vacio(EINVAL);
    }
    let mut k = 0;
    // SAFETY: `src` es una cadena del `.exe`, hasta su 0 o `cuenta`.
    while k < cuenta && unsafe { *src.add(k) } != cero {
        k += 1;
    }
    let sitio = n - desde;
    let (k, corto) = if k < sitio {
        (k, false)
    } else if cuenta == TRUNCATE {
        (sitio - 1, true)
    } else {
        return vacio(ERANGE);
    };
    // SAFETY: `desde + k + 1 <= n` elementos en `dst`.
    unsafe {
        core::ptr::copy(src, dst.add(desde), k);
        *dst.add(desde + k) = cero;
    }
    if corto {
        STRUNCATE
    } else {
        0
    }
}

extern "win64" fn strcpy_s(d: *mut u8, n: usize, s: *const u8) -> i32 {
    copiar_s(d, n, s, usize::MAX - 1, false)
}

extern "win64" fn strcat_s(d: *mut u8, n: usize, s: *const u8) -> i32 {
    copiar_s(d, n, s, usize::MAX - 1, true)
}

extern "win64" fn strncpy_s(d: *mut u8, n: usize, s: *const u8, c: usize) -> i32 {
    copiar_s(d, n, s, c, false)
}

extern "win64" fn strncat_s(d: *mut u8, n: usize, s: *const u8, c: usize) -> i32 {
    copiar_s(d, n, s, c, true)
}

extern "win64" fn wcscpy_s(d: *mut u16, n: usize, s: *const u16) -> i32 {
    copiar_s(d, n, s, usize::MAX - 1, false)
}

extern "win64" fn wcscat_s(d: *mut u16, n: usize, s: *const u16) -> i32 {
    copiar_s(d, n, s, usize::MAX - 1, true)
}

extern "win64" fn wcsncpy_s(d: *mut u16, n: usize, s: *const u16, c: usize) -> i32 {
    copiar_s(d, n, s, c, false)
}

extern "win64" fn wcsncat_s(d: *mut u16, n: usize, s: *const u16, c: usize) -> i32 {
    copiar_s(d, n, s, c, true)
}

fn memcpy_s_de<T>(d: *mut T, n: usize, s: *const T, c: usize) -> i32 {
    if c == 0 {
        return 0;
    }
    if d.is_null() {
        return EINVAL;
    }
    if s.is_null() || c > n {
        // SAFETY: `n` elementos en `d`: se borra, como el UCRT.
        unsafe { core::ptr::write_bytes(d, 0, n) };
        let e = if s.is_null() { EINVAL } else { ERANGE };
        poner_errno(e);
        return e;
    }
    // SAFETY: `c <= n` elementos en los dos lados.
    unsafe { core::ptr::copy(s, d, c) };
    0
}

extern "win64" fn memcpy_s(d: *mut u8, n: usize, s: *const u8, c: usize) -> i32 {
    memcpy_s_de(d, n, s, c)
}

extern "win64" fn wmemcpy_s(d: *mut u16, n: usize, s: *const u16, c: usize) -> i32 {
    memcpy_s_de(d, n, s, c)
}

// -- Caracteres y locale "C" -----------------------------------------------------------------

const UPPER: u16 = 0x1;
const LOWER: u16 = 0x2;
const DIGIT: u16 = 0x4;
const SPACE: u16 = 0x8;
const PUNCT: u16 = 0x10;
const CONTROL: u16 = 0x20;
const BLANK: u16 = 0x40;
const HEX: u16 = 0x80;
const ALPHA: u16 = 0x100;

const fn clase(c: u8) -> u16 {
    let mut f = 0;
    if c.is_ascii_uppercase() {
        f |= UPPER | ALPHA;
    }
    if c.is_ascii_lowercase() {
        f |= LOWER | ALPHA;
    }
    if c.is_ascii_digit() {
        f |= DIGIT;
    }
    if c.is_ascii_hexdigit() {
        f |= HEX;
    }
    if c == b' ' || (c >= 9 && c <= 13) {
        f |= SPACE;
    }
    if c == b' ' || c == b'\t' {
        f |= BLANK;
    }
    if c.is_ascii_punctuation() {
        f |= PUNCT;
    }
    if c < 32 || c == 127 {
        f |= CONTROL;
    }
    f
}

/// La tabla de `_pctype`: la entrada 0 es la de EOF (-1), como en el UCRT.
static PCTYPE: [u16; 257] = {
    let mut t = [0u16; 257];
    let mut i = 0;
    while i < 128 {
        t[i + 1] = clase(i as u8);
        i += 1;
    }
    t
};

extern "win64" fn pctype_func() -> *const u16 {
    // SAFETY: la entrada de la c = 0; la de -1 queda justo antes.
    unsafe { PCTYPE.as_ptr().add(1) }
}

fn es(c: i32, f: u16) -> i32 {
    if (0..128).contains(&c) {
        (PCTYPE[c as usize + 1] & f) as i32
    } else {
        0
    }
}

extern "win64" fn isalpha(c: i32) -> i32 {
    es(c, ALPHA)
}

extern "win64" fn isdigit(c: i32) -> i32 {
    es(c, DIGIT)
}

extern "win64" fn isspace(c: i32) -> i32 {
    es(c, SPACE)
}

extern "win64" fn isupper(c: i32) -> i32 {
    es(c, UPPER)
}

extern "win64" fn tolower(c: i32) -> i32 {
    if (b'A' as i32..=b'Z' as i32).contains(&c) {
        c + 32
    } else {
        c
    }
}

extern "win64" fn towlower(c: u16) -> u16 {
    tolower(c as i32) as u16
}

extern "win64" fn iswspace(c: u16) -> i32 {
    es(c as i32, SPACE)
}

extern "win64" fn iswxdigit(c: u16) -> i32 {
    es(c as i32, HEX)
}

extern "win64" fn iswprint(c: u16) -> i32 {
    if c >= 0xA0 {
        return 1;
    }
    (c >= 0x20 && c < 0x7F) as i32
}

static C: [u8; 2] = *b"C\0";

extern "win64" fn setlocale(_categoria: i32, nombre: *const u8) -> *const u8 {
    // Consultar, "C" y "" (el del sistema: aqui tambien "C") dan "C"; otro
    // nombre, NULL: Windows hace lo mismo con uno que no conoce.
    if nombre.is_null() || matches!(trozo(nombre), b"C" | b"") {
        C.as_ptr()
    } else {
        core::ptr::null()
    }
}

#[repr(C)]
struct Lconv {
    textos: [*const u8; 10],
    numeros: [u8; 8],
    anchos: [*const u16; 6],
}
struct Lc(Lconv);
// SAFETY: solo se lee.
unsafe impl Sync for Lc {}
static PUNTO: [u8; 2] = *b".\0";
static NADA: [u8; 1] = [0];
static PUNTO_W: [u16; 2] = [b'.' as u16, 0];
static NADA_W: [u16; 1] = [0];
static LCONV: Lc = Lc(Lconv {
    // decimal_point, thousands_sep, grouping, int_curr_symbol,
    // currency_symbol, mon_decimal_point, mon_thousands_sep, mon_grouping,
    // positive_sign, negative_sign.
    textos: [PUNTO.as_ptr(), NADA.as_ptr(), NADA.as_ptr(), NADA.as_ptr(), NADA.as_ptr(), NADA.as_ptr(), NADA.as_ptr(), NADA.as_ptr(), NADA.as_ptr(), NADA.as_ptr()],
    numeros: [127; 8],
    anchos: [PUNTO_W.as_ptr(), NADA_W.as_ptr(), NADA_W.as_ptr(), NADA_W.as_ptr(), NADA_W.as_ptr(), NADA_W.as_ptr()],
});

extern "win64" fn localeconv() -> *const Lconv {
    &LCONV.0
}

extern "win64" fn lc_codepage() -> u32 {
    0
}

extern "win64" fn mb_cur_max() -> i32 {
    1
}

struct Nombres([u64; 6]);
// SAFETY: solo se lee.
unsafe impl Sync for Nombres {}
static SIN_NOMBRES: Nombres = Nombres([0; 6]);

extern "win64" fn lc_locale_name() -> *const u64 {
    SIN_NOMBRES.0.as_ptr()
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "_errno" => dir!(errno),
        "__doserrno" => dir!(doserrno),
        "__sys_nerr" => dir!(sys_nerr),
        "strchr" => dir!(strchr),
        "strrchr" => dir!(strrchr),
        "strstr" => dir!(strstr),
        "strspn" => dir!(strspn),
        "strcspn" => dir!(strcspn),
        "strpbrk" => dir!(strpbrk),
        "strnlen" => dir!(strnlen),
        "wcsnlen" => dir!(wcsnlen),
        "strncpy" => dir!(strncpy),
        "strncat" => dir!(strncat),
        "_strdup" => dir!(strdup),
        "_wcsdup" => dir!(wcsdup),
        "_stricmp" => dir!(stricmp),
        "_strnicmp" => dir!(strnicmp),
        "_wcsicmp" => dir!(wcsicmp),
        "_wcsnicmp" => dir!(wcsnicmp),
        "_memicmp" => dir!(memicmp),
        "_mbscmp" => dir!(strcmp),
        "_strlwr" => dir!(strlwr),
        "_wcslwr" => dir!(wcslwr),
        "_wcslwr_s" => dir!(wcslwr_s),
        "_wcsupr" => dir!(wcsupr),
        "strerror" => dir!(strerror),
        "wcschr" => dir!(wcschr),
        "wcsrchr" => dir!(wcsrchr),
        "wcsstr" => dir!(wcsstr),
        "wcscmp" => dir!(wcscmp),
        "wcsncmp" => dir!(wcsncmp),
        "strcpy_s" => dir!(strcpy_s),
        "strcat_s" => dir!(strcat_s),
        "strncpy_s" => dir!(strncpy_s),
        "strncat_s" => dir!(strncat_s),
        "wcscpy_s" => dir!(wcscpy_s),
        "wcscat_s" => dir!(wcscat_s),
        "wcsncpy_s" => dir!(wcsncpy_s),
        "wcsncat_s" => dir!(wcsncat_s),
        "memcpy_s" => dir!(memcpy_s),
        "wmemcpy_s" => dir!(wmemcpy_s),
        "__pctype_func" => dir!(pctype_func),
        "isalpha" => dir!(isalpha),
        "isdigit" => dir!(isdigit),
        "isspace" => dir!(isspace),
        "isupper" => dir!(isupper),
        "tolower" => dir!(tolower),
        "towlower" => dir!(towlower),
        "iswspace" => dir!(iswspace),
        "iswxdigit" => dir!(iswxdigit),
        "iswprint" => dir!(iswprint),
        "setlocale" => dir!(setlocale),
        "localeconv" => dir!(localeconv),
        "___lc_codepage_func" => dir!(lc_codepage),
        "___mb_cur_max_func" => dir!(mb_cur_max),
        "___lc_locale_name_func" => dir!(lc_locale_name),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn copiar_s_corta_o_se_niega() {
        let mut d = [1u8; 4];
        assert_eq!(copiar_s(d.as_mut_ptr(), 4, b"abc\0".as_ptr(), usize::MAX - 1, false), 0);
        assert_eq!(&d, b"abc\0");
        assert_eq!(copiar_s(d.as_mut_ptr(), 4, b"abcd\0".as_ptr(), usize::MAX - 1, false), ERANGE);
        assert_eq!(d[0], 0);
        assert_eq!(copiar_s(d.as_mut_ptr(), 4, b"abcdef\0".as_ptr(), TRUNCATE, false), STRUNCATE);
        assert_eq!(&d, b"abc\0");
        let mut j = *b"ab\0\0\0\0";
        assert_eq!(copiar_s(j.as_mut_ptr(), 6, b"cd\0".as_ptr(), usize::MAX - 1, true), 0);
        assert_eq!(&j[..5], b"abcd\0");
    }
}
