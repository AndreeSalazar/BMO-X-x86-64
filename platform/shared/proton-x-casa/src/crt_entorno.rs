//! **El entorno del CRT, de la casa** (tanda 1 de Cyberpunk, 29-09; el mapa,
//! en `crt_cadenas.rs`):
//!
//! ```text
//!    entorno      getenv _wgetenv _wgetenv_s _dupenv_s _wdupenv_s
//!    la hora      _time64 _gmtime64 _localtime64_s _ctime64 _wctime64
//!                 _tzset __timezone __tzname (en UTC: la placa no dice zona)
//!    rutas        _fullpath _wfullpath _splitpath_s _wsplitpath_s
//!                 _wmakepath_s
//!    el proceso   _getpid _beginthreadex abort _amsg_exit _purecall
//!                 _invalid_parameter_noinfo(_noreturn) _onexit __dllonexit
//!                 _crt_at_quick_exit _seh_filter_dll _XcptFilter
//!                 _calloc_base _recalloc _lock _unlock __uncaught_exception
//!    de C++       operator delete y delete[], terminate, __std_terminate,
//!                 __std_exception_copy/destroy, __std_type_info_*
//! ```
//!
//! Lo que no, dicho: `__std_type_info_name` da el nombre decorado (sin el
//! `.`), no el legible. Lo que falta de C++ (RTTI, `__CxxFrameHandler4`, la
//! clase `exception` de msvcrt) va con la tanda de MSVCP140.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::crt;
use crate::crt_cadenas::{largo, largo_w, poner_errno, strdup, trozo, trozo_w, EINVAL, ENOENT, ERANGE};
use crate::crt_numeros::entrada_de;
use crate::{aviso, dir, kernel32, memoria, plataforma};

struct Estado {
    /// Las copias que dan getenv/_wgetenv: viven hasta que la casa se reinicia.
    entorno_a: Vec<Vec<u8>>,
    entorno_w: Vec<Vec<u16>>,
    tm: [i32; 9],
    ctime_a: [u8; 32],
    ctime_w: [u16; 32],
    timezone: i32,
    daylight: i32,
    /// La semilla de `rand` de cada hilo, por su numero (E2.3b, 05-10): en
    /// el UCRT cada hilo tiene la suya y empieza en 1.
    semillas: Vec<u32>,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
// [hilos] cerrojo -- estado del proceso que tocan los hilos del juego: necesita un cerrojo (H2.1)
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado {
    entorno_a: Vec::new(),
    entorno_w: Vec::new(),
    tm: [0; 9],
    ctime_a: [0; 32],
    ctime_w: [0; 32],
    timezone: 0,
    daylight: 0,
    semillas: Vec::new(),
}));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia de un turno a otro.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.entorno_a.clear();
    e.entorno_w.clear();
    e.daylight = 0;
    e.semillas.clear();
}

// -- El entorno ------------------------------------------------------------------------------

/// La del entorno del CRT (su copia del arranque), como en Windows.
fn variable_w(nombre: &[u16]) -> Option<Vec<u16>> {
    crt::variable_del_crt(nombre)
}

extern "win64" fn getenv(n: *const u8) -> *const u8 {
    if n.is_null() {
        return core::ptr::null();
    }
    let Some(v) = variable_w(&String::from_utf8_lossy(trozo(n)).encode_utf16().collect::<Vec<u16>>()) else { return core::ptr::null() };
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
    let Some(v) = variable_w(&String::from_utf8_lossy(trozo(nombre)).encode_utf16().collect::<Vec<u16>>()) else { return 0 };
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

/// `__daylight`: la DIRECCION de `_daylight`, si la zona tiene horario de
/// verano. La de la casa es UTC (ver `__tzname`): 0.
extern "win64" fn daylight() -> *mut i32 {
    &mut estado().daylight
}

/// `rand_s(*r)` (E1.1 de la ESCALERA, 05-10): un numero al azar de 32 bits,
/// del MISMO generador que SystemFunction036 y BCryptGenRandom
/// (`sistema::process_prng`). NULL: EINVAL, sin tocar nada.
extern "win64" fn rand_s(r: *mut u32) -> i32 {
    if r.is_null() {
        crate::crt_cadenas::poner_errno(crate::crt_cadenas::EINVAL);
        return crate::crt_cadenas::EINVAL;
    }
    crate::sistema::process_prng(r as *mut u8, 4);
    0
}

/// La semilla de `rand` del hilo de ahora (1 si no la ha tocado).
fn semilla() -> &'static mut u32 {
    let (s, h) = (&mut estado().semillas, crate::hilos::actual());
    if s.len() <= h {
        s.resize(h + 1, 1);
    }
    &mut s[h]
}

/// `srand(semilla)` (E2.3b, 05-10: nBodyGravity coloca sus particulas con
/// `rand`): la semilla del hilo.
extern "win64" fn srand(s: u32) {
    *semilla() = s;
}

/// `rand()`: el de MSVC, x = x * 214013 + 2531011 y los bits 16..30 (0 a
/// RAND_MAX, 0x7FFF). Con la misma semilla, la misma serie que en Windows:
/// las particulas de la muestra caen donde caen alli.
extern "win64" fn rand() -> i32 {
    let x = semilla();
    *x = x.wrapping_mul(214_013).wrapping_add(2_531_011);
    ((*x >> 16) & 0x7FFF) as i32
}

static UTC: [u8; 4] = *b"UTC\0";
struct Tz([*const u8; 2]);
// SAFETY: solo se lee.
// [hilos] uno -- solo se lee (los nombres de la zona horaria)
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

/// `_wassert(expresion, fichero, linea)`: un `assert` que fallo (E2.2 de la
/// ESCALERA, 05-10: lo importa DynamicIndexing). Como el UCRT: dice la
/// expresion, el fichero y la linea, y acaba como `abort()` (codigo 3).
extern "win64" fn wassert(expresion: *const u16, fichero: *const u16, linea: u32) -> ! {
    let ancha = |p: *const u16| if p.is_null() { alloc::string::String::new() } else { alloc::string::String::from_utf16_lossy(&crt::cadena_w(p as u64)) };
    aviso(&alloc::format!("Assertion failed: {}, file {}, line {linea}", ancha(expresion), ancha(fichero)));
    (plataforma().salir)(3)
}

/// `_invoke_watson`: un parametro invalido sin manejador; Windows acaba el
/// proceso con STATUS_INVALID_CRUNTIME_PARAMETER.
extern "win64" fn invoke_watson(_e: u64, _f: u64, _fi: u64, _l: u32, _r: u64) -> ! {
    aviso("_invoke_watson: un parametro invalido para el CRT; el proceso acaba");
    (plataforma().salir)(0xC000_0417)
}

extern "win64" fn amsg_exit(n: i32) -> ! {
    aviso(&alloc::format!("_amsg_exit({n}): error de arranque del CRT"));
    (plataforma().salir)(255)
}

extern "win64" fn purecall() -> i32 {
    aviso("_purecall: se llamo a una funcion virtual pura (un objeto de C++ a medio construir o ya destruido)");
    (plataforma().salir)(3)
}

/// Como el UCRT: el manejador del programa, o (sin el) terminar.
extern "win64" fn invalid_parameter_noinfo() {
    crate::crt_cadenas::parametro_invalido();
}

extern "win64" fn invalid_parameter_noinfo_noreturn() -> ! {
    crate::crt_cadenas::parametro_invalido();
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
        "_lock_locales" | "_unlock_locales" | "_lock" | "_unlock" | "_tzset" => dir!(nada),
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
        "__daylight" => dir!(daylight),
        "rand_s" => dir!(rand_s),
        "rand" => dir!(rand),
        "srand" => dir!(srand),
        "__tzname" => dir!(tzname),
        "_fullpath" => dir!(fullpath),
        "_wfullpath" => dir!(wfullpath),
        "_splitpath_s" => dir!(splitpath_s),
        "_wsplitpath_s" => dir!(wsplitpath_s),
        "_wmakepath_s" => dir!(wmakepath_s),
        "_getpid" => dir!(getpid),
        "_beginthreadex" => dir!(beginthreadex),
        "abort" => dir!(abort),
        "_wassert" => dir!(wassert),
        "_invoke_watson" => dir!(invoke_watson),
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
