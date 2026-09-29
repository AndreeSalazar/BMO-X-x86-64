//! **Las cadenas, los numeros y lo de alrededor del CRT, de la casa** (tanda 1
//! de Cyberpunk, 29-09): lo que el censo de `Cyberpunk2077.exe` y sus DLL
//! pide de `msvcrt.dll`, `vcruntime140.dll` y los `api-ms-win-crt-*`.
//!
//! ```text
//!    cadenas      strchr strrchr strstr strspn strcspn strpbrk strnlen
//!                 strncpy strncat _strdup _stricmp _strnicmp _strlwr _mbscmp
//!                 _memicmp strerror; las _s (strcpy_s, strcat_s, strncpy_s,
//!                 strncat_s, memcpy_s) con sus errno
//!    anchas       wcschr wcsrchr wcsstr wcscmp wcsncmp wcsnlen _wcsdup
//!                 _wcsicmp _wcsnicmp _wcslwr(_s) _wcsupr; wcscpy_s wcscat_s
//!                 wcsncpy_s wcsncat_s wmemcpy_s
//!    caracteres   isalpha isdigit isspace isupper tolower towlower iswprint
//!                 iswspace iswxdigit __pctype_func: la tabla del locale "C"
//!    locale       setlocale ("C" y ""), localeconv, ___lc_codepage_func,
//!                 ___mb_cur_max_func, ___lc_locale_name_func
//!    numeros      atoi atol strtol strtoll strtoul strtoull _strtoui64 _wtoi
//!                 _wtoi64 wcstol wcstoul; atof strtod _wtof (el double en
//!                 xmm0); _itoa_s _ltoa
//!    printf _s    __stdio_common_vs(n)(w)printf_s, _vsnprintf_s,
//!                 _vsnwprintf(_s), vswprintf_s, swprintf_s
//!    scanf        __stdio_common_vsscanf / vswscanf, swscanf_s
//!    entorno      getenv _wgetenv _wgetenv_s _dupenv_s _wdupenv_s
//!    la hora      _time64 _gmtime64 _localtime64_s _ctime64 _wctime64
//!                 _tzset __timezone __tzname (en UTC: la placa no dice zona)
//!    rutas        _fullpath _wfullpath _splitpath_s _wsplitpath_s
//!                 _wmakepath_s
//!    el proceso   _errno __doserrno __sys_nerr _getpid _beginthreadex abort
//!                 _amsg_exit _purecall _invalid_parameter_noinfo(_noreturn)
//!                 _onexit __dllonexit _crt_at_quick_exit _seh_filter_dll
//!                 _XcptFilter _calloc_base _recalloc _lock _unlock
//!                 __uncaught_exception
//!    de C++       operator delete y delete[], terminate, __std_terminate,
//!                 __std_exception_copy/destroy, __std_type_info_*
//! ```
//!
//! Lo que no, dicho: el locale es SIEMPRE "C" (setlocale de otro devuelve
//! NULL, como Windows con un nombre que no conoce); `__std_type_info_name`
//! da el nombre decorado (sin el `.`), no el legible. Lo que falta de C++
//! (RTTI, `__CxxFrameHandler4`, la clase `exception` de msvcrt) va con la
//! tanda de MSVCP140.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::crt;
use crate::{aviso, dir, kernel32, memoria, plataforma, proceso};

pub(crate) const EINVAL: i32 = 22;
pub(crate) const ERANGE: i32 = 34;
pub(crate) const ENOENT: i32 = 2;
pub(crate) const EACCES: i32 = 13;
pub(crate) const EBADF: i32 = 9;
pub(crate) const EEXIST: i32 = 17;
const STRUNCATE: i32 = 80;
const TRUNCATE: usize = usize::MAX;

struct Estado {
    errno: i32,
    doserrno: u32,
    /// Las copias que dan getenv/_wgetenv: viven hasta que la casa se reinicia.
    entorno_a: Vec<Vec<u8>>,
    entorno_w: Vec<Vec<u16>>,
    tm: [i32; 9],
    ctime_a: [u8; 32],
    ctime_w: [u16; 32],
    timezone: i32,
    nerr: i32,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado {
    errno: 0,
    doserrno: 0,
    entorno_a: Vec::new(),
    entorno_w: Vec::new(),
    tm: [0; 9],
    ctime_a: [0; 32],
    ctime_w: [0; 32],
    timezone: 0,
    nerr: 43,
}));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia de un turno a otro.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.errno = 0;
    e.doserrno = 0;
    e.entorno_a.clear();
    e.entorno_w.clear();
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

fn largo(s: *const u8) -> usize {
    let mut n = 0;
    // SAFETY: una cadena del `.exe` acabada en 0.
    while unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

fn largo_w(s: *const u16) -> usize {
    let mut n = 0;
    // SAFETY: una cadena UTF-16 del `.exe` acabada en 0.
    while unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

/// La cadena del `.exe` como slice, sin su 0.
fn trozo<'a>(s: *const u8) -> &'a [u8] {
    // SAFETY: `largo` bytes de una cadena del `.exe`.
    unsafe { core::slice::from_raw_parts(s, largo(s)) }
}

fn trozo_w<'a>(s: *const u16) -> &'a [u16] {
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

extern "win64" fn strdup(s: *const u8) -> *mut u8 {
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
core::arch::global_asm!(".globl proton_x_strtod", "proton_x_strtod:", "sub rsp, 40", "call proton_x_strtod_bits", "movq xmm0, rax", "add rsp, 40", "ret");
core::arch::global_asm!(".globl proton_x_atof", "proton_x_atof:", "sub rsp, 40", "xor edx, edx", "call proton_x_strtod_bits", "movq xmm0, rax", "add rsp, 40", "ret");
core::arch::global_asm!(".globl proton_x_wtof", "proton_x_wtof:", "sub rsp, 40", "call proton_x_wtof_bits", "movq xmm0, rax", "add rsp, 40", "ret");
extern "C" {
    fn proton_x_strtod();
    fn proton_x_atof();
    fn proton_x_wtof();
}

#[no_mangle]
extern "win64" fn proton_x_strtod_bits(s: *const u8, fin: *mut *const u8) -> u64 {
    let (b, n) = real(&chars_a(s));
    poner_fin(fin, s, n);
    b
}

#[no_mangle]
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
        return EINVAL;
    }
    let t = a_texto(v, base as u32);
    if t.len() + 1 > n {
        // SAFETY: `n` > 0 bytes en `buf`.
        unsafe { *buf = 0 };
        return ERANGE;
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
        poner_errno(EINVAL);
        return -1;
    }
    if r.len() >= n {
        // SAFETY: `n` > 0 elementos en `buf`.
        unsafe { *buf = T::default() };
        poner_errno(ERANGE);
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
        poner_errno(EINVAL);
        return -1;
    }
    let tope = if cuenta == TRUNCATE { n - 1 } else { cuenta };
    if r.len() <= tope && r.len() < n {
        return a_bufer_s(r, buf, n);
    }
    if tope >= n {
        // SAFETY: `n` > 0 elementos en `buf`.
        unsafe { *buf = T::default() };
        poner_errno(ERANGE);
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

#[no_mangle]
extern "win64" fn proton_x_vswprintf_s(buf: *mut u16, n: usize, fmt: *const u16, va: u64) -> i32 {
    a_bufer_s(&ancho(&crt::formatear_w(crt::ANCHOS_LEGADOS, fmt, va)), buf, n)
}

// `swprintf_s(buf, n, fmt, ...)`: los variadicos de Windows x64 siguen en la
// pila justo despues del hueco de r9. Se guarda r9 en SU hueco (el que el
// que llama reservo) y la direccion es el va_list.
core::arch::global_asm!(".globl proton_x_swprintf_s", "proton_x_swprintf_s:", "mov [rsp + 32], r9", "lea r9, [rsp + 32]", "jmp proton_x_vswprintf_s");
// `swscanf_s(buf, fmt, ...)`: los variadicos empiezan en r8.
core::arch::global_asm!(".globl proton_x_swscanf_s", "proton_x_swscanf_s:", "mov [rsp + 24], r8", "mov [rsp + 32], r9", "lea r8, [rsp + 24]", "jmp proton_x_vswscanf_s");
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

fn entrada_de<T: Copy + Into<u32>>(p: *const T, n: usize) -> Vec<u32> {
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

#[no_mangle]
extern "win64" fn proton_x_vswscanf_s(buf: *const u16, fmt: *const u16, va: u64) -> i32 {
    escanear(&chars_w(buf), &chars_w(fmt), &mut Ranuras(va as *const u64), true, true)
}

// -- El entorno ------------------------------------------------------------------------------

fn variable_w(nombre: &[u16]) -> Option<Vec<u16>> {
    proceso::variable(&String::from_utf16_lossy(nombre))
}

extern "win64" fn getenv(n: *const u8) -> *const u8 {
    if n.is_null() {
        return core::ptr::null();
    }
    let Some(v) = proceso::variable(&String::from_utf8_lossy(trozo(n))) else { return core::ptr::null() };
    let mut a: Vec<u8> = String::from_utf16_lossy(&v).into_bytes();
    a.push(0);
    let e = estado();
    e.entorno_a.push(a);
    e.entorno_a.last().map_or(core::ptr::null(), |x| x.as_ptr())
}

extern "win64" fn wgetenv(n: *const u16) -> *const u16 {
    if n.is_null() {
        return core::ptr::null();
    }
    let Some(mut v) = variable_w(trozo_w(n)) else { return core::ptr::null() };
    v.push(0);
    let e = estado();
    e.entorno_w.push(v);
    e.entorno_w.last().map_or(core::ptr::null(), |x| x.as_ptr())
}

extern "win64" fn wgetenv_s(req: *mut usize, buf: *mut u16, n: usize, nombre: *const u16) -> i32 {
    if req.is_null() || nombre.is_null() || (buf.is_null() && n > 0) {
        return EINVAL;
    }
    let v = variable_w(trozo_w(nombre));
    let hace_falta = v.as_ref().map_or(0, |v| v.len() + 1);
    // SAFETY: el size_t del `.exe`.
    unsafe { *req = hace_falta };
    if n > 0 {
        // SAFETY: al menos un elemento.
        unsafe { *buf = 0 };
    }
    let Some(v) = v else { return 0 };
    if n < hace_falta {
        return if n == 0 { 0 } else { ERANGE };
    }
    // SAFETY: cabe con su 0.
    unsafe {
        core::ptr::copy_nonoverlapping(v.as_ptr(), buf, v.len());
        *buf.add(v.len()) = 0;
    }
    0
}

extern "win64" fn dupenv_s(buf: *mut *mut u8, largo_: *mut usize, nombre: *const u8) -> i32 {
    if buf.is_null() || nombre.is_null() {
        return EINVAL;
    }
    // SAFETY: los punteros del `.exe` donde dejar el resultado.
    unsafe {
        *buf = core::ptr::null_mut();
        if !largo_.is_null() {
            *largo_ = 0;
        }
    }
    let Some(v) = proceso::variable(&String::from_utf8_lossy(trozo(nombre))) else { return 0 };
    let a = String::from_utf16_lossy(&v).into_bytes();
    let p = crt::malloc(a.len() + 1) as *mut u8;
    if p.is_null() {
        return 12;
    }
    // SAFETY: `a.len() + 1` bytes recien pedidos.
    unsafe {
        core::ptr::copy_nonoverlapping(a.as_ptr(), p, a.len());
        *p.add(a.len()) = 0;
        *buf = p;
        if !largo_.is_null() {
            *largo_ = a.len() + 1;
        }
    }
    0
}

extern "win64" fn wdupenv_s(buf: *mut *mut u16, largo_: *mut usize, nombre: *const u16) -> i32 {
    if buf.is_null() || nombre.is_null() {
        return EINVAL;
    }
    // SAFETY: como `dupenv_s`.
    unsafe {
        *buf = core::ptr::null_mut();
        if !largo_.is_null() {
            *largo_ = 0;
        }
    }
    let Some(v) = variable_w(trozo_w(nombre)) else { return 0 };
    let p = crt::malloc(2 * (v.len() + 1)) as *mut u16;
    if p.is_null() {
        return 12;
    }
    // SAFETY: `v.len() + 1` caracteres recien pedidos.
    unsafe {
        core::ptr::copy_nonoverlapping(v.as_ptr(), p, v.len());
        *p.add(v.len()) = 0;
        *buf = p;
        if !largo_.is_null() {
            *largo_ = v.len() + 1;
        }
    }
    0
}

// -- La hora ---------------------------------------------------------------------------------

/// Segundos desde 1970, del reloj de la casa (UTC).
pub(crate) fn unix_ahora() -> i64 {
    ((crate::esperas::filetime_ahora() - 116_444_736_000_000_000) / 10_000_000) as i64
}

extern "win64" fn time64(t: *mut i64) -> i64 {
    let v = unix_ahora();
    if !t.is_null() {
        // SAFETY: el __time64_t del `.exe`.
        unsafe { t.write_unaligned(v) };
    }
    v
}

/// La fecha civil de un instante: el `struct tm` (9 ints). Algoritmo de los
/// dias civiles de Howard Hinnant.
fn a_tm(t: i64) -> [i32; 9] {
    let dias = t.div_euclid(86_400);
    let seg = t.rem_euclid(86_400) as i32;
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    let bisiesto = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    const ANTES: [i32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let yday = ANTES[(m - 1) as usize] + d as i32 - 1 + (bisiesto && m > 2) as i32;
    let wday = (dias + 4).rem_euclid(7) as i32;
    [seg % 60, seg / 60 % 60, seg / 3600, d as i32, m as i32 - 1, (y - 1900) as i32, wday, yday, 0]
}

extern "win64" fn gmtime64(t: *const i64) -> *mut [i32; 9] {
    if t.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: el __time64_t del `.exe`.
    let v = unsafe { t.read_unaligned() };
    if v < 0 {
        poner_errno(EINVAL);
        return core::ptr::null_mut();
    }
    let e = estado();
    e.tm = a_tm(v);
    &mut e.tm
}

extern "win64" fn localtime64_s(tm: *mut [i32; 9], t: *const i64) -> i32 {
    if tm.is_null() || t.is_null() {
        return EINVAL;
    }
    // SAFETY: el __time64_t y el struct tm del `.exe`.
    unsafe {
        let v = t.read_unaligned();
        if v < 0 {
            tm.write_unaligned([-1; 9]);
            return EINVAL;
        }
        tm.write_unaligned(a_tm(v));
    }
    0
}

/// "Wed Jan 02 02:03:55 1980\n" de `ctime`.
fn ctime_texto(t: i64) -> Vec<u8> {
    let tm = a_tm(t);
    const DIAS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MESES: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    alloc::format!("{} {} {:02} {:02}:{:02}:{:02} {}\n", DIAS[tm[6] as usize], MESES[tm[4] as usize], tm[3], tm[2], tm[1], tm[0], tm[5] + 1900).into_bytes()
}

extern "win64" fn ctime64(t: *const i64) -> *const u8 {
    if t.is_null() {
        return core::ptr::null();
    }
    // SAFETY: el __time64_t del `.exe`.
    let v = unsafe { t.read_unaligned() };
    if v < 0 {
        return core::ptr::null();
    }
    let b = ctime_texto(v);
    let e = estado();
    e.ctime_a = [0; 32];
    e.ctime_a[..b.len().min(31)].copy_from_slice(&b[..b.len().min(31)]);
    e.ctime_a.as_ptr()
}

extern "win64" fn wctime64(t: *const i64) -> *const u16 {
    if t.is_null() {
        return core::ptr::null();
    }
    // SAFETY: el __time64_t del `.exe`.
    let v = unsafe { t.read_unaligned() };
    if v < 0 {
        return core::ptr::null();
    }
    let b = ctime_texto(v);
    let e = estado();
    e.ctime_w = [0; 32];
    for (i, &c) in b.iter().take(31).enumerate() {
        e.ctime_w[i] = c as u16;
    }
    e.ctime_w.as_ptr()
}

extern "win64" fn timezone() -> *mut i32 {
    &mut estado().timezone
}

static UTC: [u8; 4] = *b"UTC\0";
struct Tz([*const u8; 2]);
// SAFETY: solo se lee.
unsafe impl Sync for Tz {}
static TZNAME: Tz = Tz([UTC.as_ptr(), UTC.as_ptr()]);

extern "win64" fn tzname() -> *const *const u8 {
    TZNAME.0.as_ptr()
}

// -- Rutas -----------------------------------------------------------------------------------

fn completa_w(rel: &[u16]) -> Option<Vec<u16>> {
    let mut r: Vec<u16> = rel.to_vec();
    r.push(0);
    let mut buf = alloc::vec![0u16; 1024];
    let n = crate::carpetas::get_full_path_name_w(r.as_ptr(), buf.len() as u32, buf.as_mut_ptr(), core::ptr::null_mut()) as usize;
    (n > 0 && n < buf.len()).then(|| {
        buf.truncate(n);
        buf
    })
}

extern "win64" fn fullpath(abs: *mut u8, rel: *const u8, max: usize) -> *mut u8 {
    let w: Vec<u16> = if rel.is_null() || largo(rel) == 0 { alloc::vec![b'.' as u16] } else { String::from_utf8_lossy(trozo(rel)).encode_utf16().collect() };
    let Some(r) = completa_w(&w) else {
        poner_errno(ENOENT);
        return core::ptr::null_mut();
    };
    let a = String::from_utf16_lossy(&r).into_bytes();
    let (p, n) = if abs.is_null() { (crt::malloc(a.len() + 1) as *mut u8, a.len() + 1) } else { (abs, max) };
    if p.is_null() || a.len() + 1 > n {
        poner_errno(ERANGE);
        return core::ptr::null_mut();
    }
    // SAFETY: cabe con su 0.
    unsafe {
        core::ptr::copy_nonoverlapping(a.as_ptr(), p, a.len());
        *p.add(a.len()) = 0;
    }
    p
}

extern "win64" fn wfullpath(abs: *mut u16, rel: *const u16, max: usize) -> *mut u16 {
    let w: Vec<u16> = if rel.is_null() || largo_w(rel) == 0 { alloc::vec![b'.' as u16] } else { trozo_w(rel).to_vec() };
    let Some(r) = completa_w(&w) else {
        poner_errno(ENOENT);
        return core::ptr::null_mut();
    };
    let (p, n) = if abs.is_null() { (crt::malloc(2 * (r.len() + 1)) as *mut u16, r.len() + 1) } else { (abs, max) };
    if p.is_null() || r.len() + 1 > n {
        poner_errno(ERANGE);
        return core::ptr::null_mut();
    }
    // SAFETY: cabe con su 0.
    unsafe {
        core::ptr::copy_nonoverlapping(r.as_ptr(), p, r.len());
        *p.add(r.len()) = 0;
    }
    p
}

/// `(unidad, carpeta, nombre, extension)` de una ruta, en indices.
fn partir(p: &[u32]) -> [core::ops::Range<usize>; 4] {
    let barra = |c: u32| c == b'\\' as u32 || c == b'/' as u32;
    let u = if p.len() >= 2 && p[1] == b':' as u32 { 2 } else { 0 };
    let c = p.iter().rposition(|&x| barra(x)).map_or(u, |i| (i + 1).max(u));
    let e = p[c..].iter().rposition(|&x| x == b'.' as u32).map_or(p.len(), |i| c + i);
    [0..u, u..c, c..e, e..p.len()]
}

/// Dejar cada trozo en su bufer (`(puntero, medida)`, NULL/0 = no se quiere).
fn splitpath_de<T: Copy + Default + Into<u32>>(ruta: *const T, destinos: [(*mut T, usize); 4]) -> i32 {
    if ruta.is_null() {
        return EINVAL;
    }
    let p = entrada_de(ruta, usize::MAX);
    let trozos = partir(&p);
    // Primero mirar que todo cabe; si no, todos vacios y ERANGE.
    let mut error = 0;
    for (r, &(d, n)) in trozos.iter().zip(destinos.iter()) {
        if (d.is_null()) != (n == 0) {
            error = EINVAL;
        } else if !d.is_null() && r.len() + 1 > n {
            error = ERANGE;
        }
    }
    for (r, &(d, n)) in trozos.iter().zip(destinos.iter()) {
        if d.is_null() || n == 0 {
            continue;
        }
        // SAFETY: `n` elementos en `d`; si hay error, solo el primero.
        unsafe {
            if error != 0 {
                *d = T::default();
                continue;
            }
            for (j, i) in r.clone().enumerate() {
                *d.add(j) = *ruta.add(i);
            }
            *d.add(r.len()) = T::default();
        }
    }
    if error != 0 {
        poner_errno(error);
    }
    error
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn splitpath_s(ruta: *const u8, u: *mut u8, un: usize, c: *mut u8, cn: usize, n: *mut u8, nn: usize, e: *mut u8, en: usize) -> i32 {
    splitpath_de(ruta, [(u, un), (c, cn), (n, nn), (e, en)])
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn wsplitpath_s(ruta: *const u16, u: *mut u16, un: usize, c: *mut u16, cn: usize, n: *mut u16, nn: usize, e: *mut u16, en: usize) -> i32 {
    splitpath_de(ruta, [(u, un), (c, cn), (n, nn), (e, en)])
}

extern "win64" fn wmakepath_s(buf: *mut u16, n: usize, u: *const u16, c: *const u16, nombre: *const u16, ext: *const u16) -> i32 {
    if buf.is_null() || n == 0 {
        return EINVAL;
    }
    let de = |p: *const u16| if p.is_null() { Vec::new() } else { trozo_w(p).to_vec() };
    let mut r: Vec<u16> = Vec::new();
    let u = de(u);
    if let Some(&l) = u.first() {
        r.push(l);
        r.push(b':' as u16);
    }
    let c = de(c);
    if !c.is_empty() {
        r.extend_from_slice(&c);
        if !matches!(c.last(), Some(&x) if x == b'\\' as u16 || x == b'/' as u16) {
            r.push(b'\\' as u16);
        }
    }
    r.extend_from_slice(&de(nombre));
    let e = de(ext);
    if !e.is_empty() {
        if e[0] != b'.' as u16 {
            r.push(b'.' as u16);
        }
        r.extend_from_slice(&e);
    }
    if r.len() + 1 > n {
        // SAFETY: `n` > 0 elementos en `buf`.
        unsafe { *buf = 0 };
        return ERANGE;
    }
    // SAFETY: cabe con su 0.
    unsafe {
        core::ptr::copy_nonoverlapping(r.as_ptr(), buf, r.len());
        *buf.add(r.len()) = 0;
    }
    0
}

// -- El proceso ------------------------------------------------------------------------------

extern "win64" fn getpid() -> i32 {
    kernel32::id_del_proceso() as i32
}

/// `_beginthreadex`: CreateThread con la rutina `unsigned __stdcall f(void*)`
/// (en x64, la misma convencion que la de un hilo de Windows).
extern "win64" fn beginthreadex(seg: u64, pila: u32, f: u64, arg: u64, banderas: u32, id: *mut u32) -> u64 {
    let h = crate::hilos::create_thread(seg, pila as usize, f, arg, banderas, id);
    if h == 0 {
        poner_errno(EINVAL);
    }
    h
}

extern "win64" fn abort() -> ! {
    aviso("abort(): el programa se aborto a si mismo");
    (plataforma().salir)(3)
}

extern "win64" fn amsg_exit(n: i32) -> ! {
    aviso(&alloc::format!("_amsg_exit({n}): error de arranque del CRT"));
    (plataforma().salir)(255)
}

extern "win64" fn purecall() -> i32 {
    aviso("_purecall: se llamo a una funcion virtual pura (un objeto de C++ a medio construir o ya destruido)");
    (plataforma().salir)(3)
}

extern "win64" fn invalid_parameter_noinfo() {
    // El UCRT llamaria al manejador; el de por defecto termina. Aqui se
    // dice y se sigue: la funcion ya devolvio su EINVAL.
    aviso("_invalid_parameter_noinfo: una funcion del CRT recibio un parametro invalido");
}

extern "win64" fn invalid_parameter_noinfo_noreturn() -> ! {
    aviso("_invalid_parameter_noinfo_noreturn: parametro invalido; el proceso termina");
    (plataforma().salir)(0xC000_0417)
}

extern "win64" fn onexit(f: u64) -> u64 {
    if crt::crt_atexit(f) == 0 {
        f
    } else {
        0
    }
}

extern "win64" fn dllonexit(f: u64, _inicio: u64, _fin: u64) -> u64 {
    onexit(f)
}

extern "win64" fn cero(_a: u64) -> i32 {
    0
}

extern "win64" fn nada(_a: u64) {}

/// `_XcptFilter`: EXCEPTION_CONTINUE_SEARCH, que lo vea el de arriba.
extern "win64" fn xcpt_filter(_codigo: u32, _punteros: u64) -> i32 {
    0
}

extern "win64" fn recalloc(p: u64, n: usize, t: usize) -> u64 {
    let Some(total) = n.checked_mul(t) else {
        poner_errno(12);
        return 0;
    };
    let antes = if p == 0 { 0 } else { memoria::medida_del_proceso(p).unwrap_or(0) as usize };
    let q = crt::realloc(p, total);
    if q != 0 && total > antes {
        // SAFETY: lo nuevo del bloque, de `antes` a `total`.
        unsafe { core::ptr::write_bytes((q + antes as u64) as *mut u8, 0, total - antes) };
    }
    q
}

// -- Lo de C++ que es de C -------------------------------------------------------------------

extern "win64" fn operator_delete(p: u64) {
    if p != 0 {
        crt::free(p);
    }
}

/// `__std_exception_data`: el texto y si hay que soltarlo.
#[repr(C)]
struct ExcepcionDatos {
    que: *const u8,
    soltar: u8,
}

extern "win64" fn std_exception_copy(de: *const ExcepcionDatos, a: *mut ExcepcionDatos) {
    // SAFETY: dos __std_exception_data del `.exe`.
    unsafe {
        let d = &*de;
        let a = &mut *a;
        if d.soltar != 0 && !d.que.is_null() {
            a.que = strdup(d.que);
            a.soltar = (!a.que.is_null()) as u8;
        } else {
            a.que = d.que;
            a.soltar = 0;
        }
    }
}

extern "win64" fn std_exception_destroy(d: *mut ExcepcionDatos) {
    // SAFETY: un __std_exception_data del `.exe`.
    unsafe {
        let d = &mut *d;
        if d.soltar != 0 {
            crt::free(d.que as u64);
        }
        d.que = core::ptr::null();
        d.soltar = 0;
    }
}

/// `__std_type_info_data`: `{ const char* legible; char decorado[]; }`. El
/// nombre decorado empieza con `.`, que no cuenta.
fn decorado(d: u64) -> &'static [u8] {
    trozo((d + 9) as *const u8)
}

extern "win64" fn std_type_info_compare(a: u64, b: u64) -> i32 {
    if a == b {
        return 0;
    }
    let (x, y) = (decorado(a), decorado(b));
    match x.cmp(y) {
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Less => -1,
        _ => 1,
    }
}

extern "win64" fn std_type_info_hash(d: u64) -> u64 {
    // FNV-1a de 64 bits, como el de vcruntime.
    decorado(d).iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

extern "win64" fn std_type_info_name(d: u64, _lista: u64) -> *const u8 {
    // SAFETY: el __std_type_info_data del `.exe`: su primer campo es la
    // cache del nombre legible.
    unsafe {
        let cache = d as *mut *const u8;
        if (*cache).is_null() {
            *cache = (d + 9) as *const u8;
        }
        *cache
    }
}

extern "win64" fn uncaught_exception() -> i32 {
    0
}

/// Los nombres de estas (y de las `_o_` de `api-ms-win-crt-private`).
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
        "_lock_locales" | "_unlock_locales" | "_lock" | "_unlock" | "_tzset" => dir!(nada),
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
        "getenv" => dir!(getenv),
        "_wgetenv" => dir!(wgetenv),
        "_wgetenv_s" => dir!(wgetenv_s),
        "_dupenv_s" => dir!(dupenv_s),
        "_wdupenv_s" => dir!(wdupenv_s),
        "_time64" => dir!(time64),
        "_gmtime64" => dir!(gmtime64),
        "_localtime64_s" => dir!(localtime64_s),
        "_ctime64" => dir!(ctime64),
        "_wctime64" => dir!(wctime64),
        "__timezone" => dir!(timezone),
        "__tzname" => dir!(tzname),
        "_fullpath" => dir!(fullpath),
        "_wfullpath" => dir!(wfullpath),
        "_splitpath_s" => dir!(splitpath_s),
        "_wsplitpath_s" => dir!(wsplitpath_s),
        "_wmakepath_s" => dir!(wmakepath_s),
        "_getpid" => dir!(getpid),
        "_beginthreadex" => dir!(beginthreadex),
        "abort" => dir!(abort),
        "_amsg_exit" => dir!(amsg_exit),
        "_purecall" => dir!(purecall),
        "_invalid_parameter_noinfo" => dir!(invalid_parameter_noinfo),
        "_invalid_parameter_noinfo_noreturn" => dir!(invalid_parameter_noinfo_noreturn),
        "_onexit" => dir!(onexit),
        "__dllonexit" => dir!(dllonexit),
        "_crt_at_quick_exit" => dir!(cero),
        "_seh_filter_dll" => dir!(crt::seh_filter_exe),
        "_XcptFilter" => dir!(xcpt_filter),
        "_calloc_base" => dir!(crt::calloc),
        "_recalloc" => dir!(recalloc),
        "__uncaught_exception" => dir!(uncaught_exception),
        "??3@YAXPEAX@Z" | "??_V@YAXPEAX@Z" => dir!(operator_delete),
        "?terminate@@YAXXZ" | "__std_terminate" => dir!(crt::terminate),
        "__std_exception_copy" => dir!(std_exception_copy),
        "__std_exception_destroy" => dir!(std_exception_destroy),
        "__std_type_info_compare" => dir!(std_type_info_compare),
        "__std_type_info_hash" => dir!(std_type_info_hash),
        "__std_type_info_name" => dir!(std_type_info_name),
        "__std_type_info_destroy_list" => dir!(nada),
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

    #[test]
    fn la_hora_civil_y_las_rutas() {
        // 2026-09-29 15:04:05 UTC, martes.
        let t = 1_790_694_245;
        let tm = a_tm(t);
        assert_eq!(&tm[..8], &[5, 4, 15, 29, 8, 126, 2, 271]);
        assert_eq!(ctime_texto(0), b"Thu Jan 01 00:00:00 1970\n");
        let p = cs("C:\\juegos\\cp\\bin.x64\\Cyberpunk2077.exe");
        let [u, c, n, e] = partir(&p);
        assert_eq!((u, c.clone(), n.clone(), e.clone()), (0..2, 2..21, 21..34, 34..38));
        assert_eq!(partir(&cs("sin_ext"))[2], 0..7);
    }
}
