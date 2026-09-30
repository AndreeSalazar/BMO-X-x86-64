//! **El CRT universal de Windows (UCRT) y `vcruntime140.dll`, de la casa**
//! (P4f5, 27-09): lo que importa un `.exe` compilado con MSVC y el CRT
//! dinamico (el de la `std` de Rust para `x86_64-pc-windows-msvc`).
//!
//! ```text
//!    el arranque   _configure_narrow/wide_argv, __p___argc/argv/wargv,
//!                  _initialize_narrow/wide_environment,
//!                  _get_initial_narrow/wide_environment, _initterm(_e),
//!                  _set_app_type, _configthreadlocale, _set_new_mode,
//!                  _set_fmode, __p__commode, __setusermatherr
//!    la salida     exit, _exit, _cexit, _c_exit, _crt_atexit,
//!                  _initialize/_register/_execute_onexit_table
//!    el monton     malloc, calloc, realloc, free, _callnewh: el monton de
//!                  Windows del proceso (P4e)
//!    memoria y     memcpy, memmove, memset, memcmp, memchr, strlen, wcslen,
//!    cadenas       strcmp, strncmp
//! ```
//!
//! Se resuelven por su DLL de verdad (`ucrtbase.dll`, `vcruntime140.dll`) y
//! por los API set del CRT (`api-ms-win-crt-*-l1-1-0.dll`), que es como los
//! importa un `.exe` de MSVC.
//!
//!    stdio         __acrt_iob_func (stdin, stdout, stderr), los
//!                  __stdio_common_v(f)(w)printf y v(s)(w)printf con el
//!                  formato de `bmo_proton_x::formato`, puts, fputs, fputc,
//!                  putc, putchar, fwrite, fflush; en modo TEXTO: "\n" sale
//!                  "\r\n", como en Windows
//!
//! Lo que no es Windows, dicho: `fopen` y los FILE de ficheros no estan (los
//! tres estandar si); y lo que es del mecanismo de excepciones de C++ va con
//! P4c.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::{dir, memoria, plataforma, proceso};

struct Estado {
    /// argv: las cadenas (con su 0) y los punteros (acabados en NULL).
    argv_a: Vec<Vec<u8>>,
    argv_w: Vec<Vec<u16>>,
    punteros_a: Vec<u64>,
    punteros_w: Vec<u64>,
    argc: i32,
    /// `__argv` y `__wargv`: las variables cuya DIRECCION da __p___argv.
    argv: u64,
    wargv: u64,
    entorno_a: Vec<Vec<u8>>,
    entorno_w: Vec<Vec<u16>>,
    env_a: Vec<u64>,
    env_w: Vec<u64>,
    /// Lo que se registro con _crt_atexit: corre al salir, al reves.
    al_salir: Vec<u64>,
    commode: i32,
    tl_atexit: u64,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado {
    argv_a: Vec::new(),
    argv_w: Vec::new(),
    punteros_a: Vec::new(),
    punteros_w: Vec::new(),
    argc: 0,
    argv: 0,
    wargv: 0,
    entorno_a: Vec::new(),
    entorno_w: Vec::new(),
    env_a: Vec::new(),
    env_w: Vec::new(),
    al_salir: Vec::new(),
    commode: 0,
    tl_atexit: 0,
}));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia de un turno a otro.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.argv_a.clear();
    e.argv_w.clear();
    e.punteros_a.clear();
    e.punteros_w.clear();
    e.argc = 0;
    e.argv = 0;
    e.wargv = 0;
    e.entorno_a.clear();
    e.entorno_w.clear();
    e.env_a.clear();
    e.env_w.clear();
    e.al_salir.clear();
    e.commode = 0;
    e.tl_atexit = 0;
}

// -- El arranque -----------------------------------------------------------------

/// argv, de la linea de ordenes y con las reglas del CRT
/// (`bmo_proton_x::proceso::argumentos`). Una vez: los punteros no se mueven.
fn preparar_argv() {
    let e = estado();
    if !e.punteros_a.is_empty() {
        return;
    }
    for a in bmo_proton_x::proceso::argumentos(&proceso::linea()) {
        e.argv_a.push(a.bytes().chain([0]).collect());
        e.argv_w.push(a.encode_utf16().chain([0]).collect());
    }
    e.punteros_a = e.argv_a.iter().map(|v| v.as_ptr() as u64).chain([0]).collect();
    e.punteros_w = e.argv_w.iter().map(|v| v.as_ptr() as u64).chain([0]).collect();
    e.argc = e.argv_a.len() as i32;
    e.argv = e.punteros_a.as_ptr() as u64;
    e.wargv = e.punteros_w.as_ptr() as u64;
}

/// La copia del entorno del CRT: una vez, al empezar el proceso (como la del
/// UCRT al cargarse, antes que nada del `.exe`).
pub(crate) fn preparar_entorno() {
    let e = estado();
    if !e.env_a.is_empty() {
        return;
    }
    for p in proceso::pares_del_entorno() {
        e.entorno_a.push(p.iter().map(|&c| if c < 0x80 { c as u8 } else { b'?' }).chain([0]).collect());
        e.entorno_w.push(p.iter().copied().chain([0]).collect());
    }
    e.env_a = e.entorno_a.iter().map(|v| v.as_ptr() as u64).chain([0]).collect();
    e.env_w = e.entorno_w.iter().map(|v| v.as_ptr() as u64).chain([0]).collect();
}

/// **Una variable del entorno DEL CRT** (getenv y los suyos): la copia que
/// hizo `_initialize_*_environment` al arrancar, no el del sistema; `None`
/// si todavia no se hizo. Asi es en Windows (el metal, 29-09: tanda1.exe sin
/// el arranque del CRT ve getenv NULL, y un SetEnvironmentVariable de
/// despues no cambia lo que dice getenv).
pub(crate) fn variable_del_crt(nombre: &[u16]) -> Option<Vec<u16>> {
    let e = estado();
    if e.env_w.is_empty() {
        return None;
    }
    let igual = |a: &[u16], b: &[u16]| a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| (x < 0x80 && y < 0x80 && (x as u8).eq_ignore_ascii_case(&(y as u8))) || x == y);
    e.entorno_w.iter().find_map(|p| {
        let p = &p[..p.len() - 1];
        let k = p.iter().skip(1).position(|&c| c == b'=' as u16)? + 1;
        igual(&p[..k], nombre).then(|| p[k + 1..].to_vec())
    })
}

extern "win64" fn configure_argv(_modo: i32) -> i32 {
    preparar_argv();
    0
}

extern "win64" fn p_argc() -> *mut i32 {
    preparar_argv();
    &mut estado().argc
}

extern "win64" fn p_argv() -> *mut u64 {
    preparar_argv();
    &mut estado().argv
}

extern "win64" fn p_wargv() -> *mut u64 {
    preparar_argv();
    &mut estado().wargv
}

extern "win64" fn initialize_environment() -> i32 {
    preparar_entorno();
    0
}

extern "win64" fn get_initial_narrow_environment() -> u64 {
    preparar_entorno();
    estado().env_a.as_ptr() as u64
}

extern "win64" fn get_initial_wide_environment() -> u64 {
    preparar_entorno();
    estado().env_w.as_ptr() as u64
}

/// `_initterm(primero, ultimo)`: cada puntero no nulo de la tabla, en orden
/// (los constructores globales del `.exe`).
extern "win64" fn initterm(primero: *const u64, ultimo: *const u64) {
    let mut p = primero;
    while p < ultimo {
        // SAFETY: la tabla es del `.exe`, entre sus dos marcas.
        let f = unsafe { p.read() };
        if f != 0 {
            // SAFETY: `void f(void)`, del `.exe`.
            unsafe { crate::hilos::llamar_win64(f, 0, 0, 0) };
        }
        // SAFETY: dentro de la tabla.
        p = unsafe { p.add(1) };
    }
}

/// `_initterm_e`: como `_initterm`, pero cada una devuelve un `int`; el
/// primero que no sea 0 para la cuenta y se devuelve.
extern "win64" fn initterm_e(primero: *const u64, ultimo: *const u64) -> i32 {
    let mut p = primero;
    while p < ultimo {
        // SAFETY: como arriba.
        let f = unsafe { p.read() };
        if f != 0 {
            // SAFETY: `int f(void)`, del `.exe`.
            let r = unsafe { crate::hilos::llamar_win64(f, 0, 0, 0) } as i32;
            if r != 0 {
                return r;
            }
        }
        // SAFETY: dentro de la tabla.
        p = unsafe { p.add(1) };
    }
    0
}

extern "win64" fn nada(_a: u64) {}

extern "win64" fn cero(_a: u64) -> i32 {
    0
}

/// `_configthreadlocale`: la configuracion de antes (_DISABLE_PER_THREAD_LOCALE).
extern "win64" fn configthreadlocale(_n: i32) -> i32 {
    2
}

extern "win64" fn p_commode() -> *mut i32 {
    &mut estado().commode
}

// -- La salida -----------------------------------------------------------------------------

/// Lo registrado con _crt_atexit, al reves, una vez.
fn correr_al_salir() {
    let tl = core::mem::take(&mut estado().tl_atexit);
    if tl != 0 {
        // SAFETY: un PIMAGE_TLS_CALLBACK del `.exe`: (base, motivo, 0).
        unsafe { crate::hilos::llamar_win64(tl, crate::kernel32::base_imagen(), 0, 0) };
    }
    while let Some(f) = estado().al_salir.pop() {
        // SAFETY: `void f(void)` que el `.exe` registro.
        unsafe { crate::hilos::llamar_win64(f, 0, 0, 0) };
    }
}

pub(crate) extern "win64" fn crt_atexit(f: u64) -> i32 {
    if f == 0 {
        return -1;
    }
    estado().al_salir.push(f);
    0
}

extern "win64" fn exit(codigo: i32) -> ! {
    correr_al_salir();
    (plataforma().salir)(codigo as u32)
}

extern "win64" fn exit_rapido(codigo: i32) -> ! {
    (plataforma().salir)(codigo as u32)
}

extern "win64" fn cexit() {
    correr_al_salir();
}

/// `_onexit_table_t`: tres punteros del `.exe` (primero, ultimo, fin); la
/// tabla en si sale del monton del proceso, como en el UCRT.
#[repr(C)]
struct Tabla {
    primero: u64,
    ultimo: u64,
    fin: u64,
}

/// Como el UCRT: una tabla cuyo primero NO es su fin se da por iniciada y
/// no se toca (asi la iniciar dos veces no pierde lo registrado). Una tabla
/// con basura, por eso, no sirve: tiene que llegar a ceros, como las
/// estaticas del CRT. (Se vio el 27-09 en Windows: la casa perdonaba lo que
/// Windows no.)
extern "win64" fn initialize_onexit_table(t: *mut Tabla) -> i32 {
    if t.is_null() {
        return -1;
    }
    // SAFETY: una tabla del `.exe`.
    unsafe {
        if (*t).primero != (*t).fin {
            return 0;
        }
        *t = Tabla { primero: 0, ultimo: 0, fin: 0 };
    }
    0
}

extern "win64" fn register_onexit_function(t: *mut Tabla, f: u64) -> i32 {
    if t.is_null() {
        return -1;
    }
    // SAFETY: una tabla del `.exe` (la de arriba).
    let tb = unsafe { &mut *t };
    if tb.ultimo == tb.fin {
        let n = if tb.primero == 0 { 0 } else { (tb.fin - tb.primero) / 8 };
        let nuevo_n = (n * 2).max(8);
        let Some(p) = memoria::pedir_del_proceso(nuevo_n * 8) else { return -1 };
        if tb.primero != 0 {
            // SAFETY: la tabla vieja, `n` punteros, a la nueva.
            unsafe { core::ptr::copy_nonoverlapping(tb.primero as *const u64, p as *mut u64, n as usize) };
            memoria::soltar_del_proceso(tb.primero);
        }
        tb.ultimo = p + (tb.ultimo - tb.primero);
        tb.primero = p;
        tb.fin = p + nuevo_n * 8;
    }
    // SAFETY: hay sitio entre `ultimo` y `fin`.
    unsafe { (tb.ultimo as *mut u64).write(f) };
    tb.ultimo += 8;
    0
}

/// `_execute_onexit_table`: al reves, y la tabla queda vacia.
extern "win64" fn execute_onexit_table(t: *mut Tabla) -> i32 {
    if t.is_null() {
        return -1;
    }
    // SAFETY: una tabla del `.exe`.
    let (primero, mut ultimo) = unsafe { ((*t).primero, (*t).ultimo) };
    // SAFETY: como arriba: se vacia antes de llamar (una funcion podria registrar otra).
    unsafe { *t = Tabla { primero: 0, ultimo: 0, fin: 0 } };
    while ultimo > primero {
        ultimo -= 8;
        // SAFETY: un puntero de la tabla.
        let f = unsafe { (ultimo as *const u64).read() };
        if f != 0 {
            // SAFETY: `void f(void)` que el `.exe` registro.
            unsafe { crate::hilos::llamar_win64(f, 0, 0, 0) };
        }
    }
    if primero != 0 {
        memoria::soltar_del_proceso(primero);
    }
    0
}

// -- El monton ----------------------------------------------------------------------------

pub(crate) extern "win64" fn malloc(n: usize) -> u64 {
    memoria::pedir_del_proceso(n as u64).unwrap_or(0)
}

pub(crate) extern "win64" fn calloc(n: usize, m: usize) -> u64 {
    let Some(t) = n.checked_mul(m) else { return 0 };
    let p = malloc(t);
    if p != 0 {
        // SAFETY: un bloque recien pedido de `t` bytes.
        unsafe { core::ptr::write_bytes(p as *mut u8, 0, t) };
    }
    p
}

pub(crate) extern "win64" fn free(p: u64) {
    if p != 0 {
        memoria::soltar_del_proceso(p);
    }
}

pub(crate) extern "win64" fn realloc(p: u64, n: usize) -> u64 {
    if p == 0 {
        return malloc(n);
    }
    if n == 0 {
        free(p);
        return 0;
    }
    memoria::cambiar_del_proceso(p, n as u64).unwrap_or(0)
}

// -- Memoria y cadenas ------------------------------------------------------------------------

extern "win64" fn memcpy(d: *mut u8, s: *const u8, n: usize) -> *mut u8 {
    // SAFETY: `n` bytes del `.exe` en los dos lados (memcpy no admite solape).
    unsafe { core::ptr::copy_nonoverlapping(s, d, n) };
    d
}

extern "win64" fn memmove(d: *mut u8, s: *const u8, n: usize) -> *mut u8 {
    // SAFETY: `n` bytes del `.exe` en los dos lados, que pueden solaparse.
    unsafe { core::ptr::copy(s, d, n) };
    d
}

extern "win64" fn memset(d: *mut u8, c: i32, n: usize) -> *mut u8 {
    // SAFETY: `n` bytes del `.exe`.
    unsafe { core::ptr::write_bytes(d, c as u8, n) };
    d
}

extern "win64" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    for i in 0..n {
        // SAFETY: `n` bytes del `.exe` en los dos lados.
        let (x, y) = unsafe { (*a.add(i), *b.add(i)) };
        if x != y {
            return x as i32 - y as i32;
        }
    }
    0
}

extern "win64" fn memchr(p: *const u8, c: i32, n: usize) -> u64 {
    for i in 0..n {
        // SAFETY: `n` bytes del `.exe`.
        if unsafe { *p.add(i) } == c as u8 {
            return p as u64 + i as u64;
        }
    }
    0
}

extern "win64" fn strlen(s: *const u8) -> usize {
    let mut n = 0;
    // SAFETY: una cadena del `.exe` acabada en 0.
    while unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

extern "win64" fn wcslen(s: *const u16) -> usize {
    let mut n = 0;
    // SAFETY: una cadena UTF-16 del `.exe` acabada en 0.
    while unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

extern "win64" fn strncmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    for i in 0..n {
        // SAFETY: dos cadenas del `.exe`: se para en el primer 0.
        let (x, y) = unsafe { (*a.add(i), *b.add(i)) };
        if x != y || x == 0 {
            return x as i32 - y as i32;
        }
    }
    0
}

extern "win64" fn strcmp(a: *const u8, b: *const u8) -> i32 {
    strncmp(a, b, usize::MAX)
}

// -- stdio ------------------------------------------------------------------------------------

use bmo_proton_x::formato::{self, Argumentos};

/// Los tres FILE estandar: la casa solo necesita sus DIRECCIONES distintas.
struct Files(UnsafeCell<[u64; 3]>);
// SAFETY: nadie los lee ni escribe; solo se da su direccion.
unsafe impl Sync for Files {}
static FILES: Files = Files(UnsafeCell::new([0; 3]));

pub(crate) extern "win64" fn acrt_iob_func(i: u32) -> u64 {
    FILES.0.get() as u64 + 8 * (i.min(2) as u64)
}

/// El numero (0, 1, 2) de un FILE estandar.
pub(crate) fn cual(f: u64) -> Option<u64> {
    let base = FILES.0.get() as u64;
    (f >= base && f < base + 24 && (f - base) % 8 == 0).then(|| (f - base) / 8)
}

/// Escribir en un FILE estandar, en modo texto. Los bytes, o -1.
pub(crate) fn a_stream(f: u64, b: &[u8]) -> i32 {
    let h = match cual(f) {
        Some(1) => crate::kernel32::estandar(-11),
        Some(2) => crate::kernel32::estandar(-12),
        // Tanda 1 de Cyberpunk: un FILE de fopen.
        None if crate::crt_ficheros::es_flujo(f) => return crate::crt_ficheros::escribir_flujo(f, b),
        _ => return -1,
    };
    let mut t = Vec::with_capacity(b.len() + 8);
    for &c in b {
        if c == b'\n' {
            t.push(b'\r');
        }
        t.push(c);
    }
    let mut n = 0u32;
    if crate::kernel32::write_file(h, t.as_ptr(), t.len() as u32, &mut n, 0) == 0 {
        return -1;
    }
    b.len() as i32
}

/// Un `va_list` de Windows x64: ranuras de 8 bytes seguidas.
struct Va(*const u64);

pub(crate) fn cadena_c(p: u64) -> Vec<u8> {
    let mut v = Vec::new();
    // SAFETY: una cadena del `.exe` acabada en 0.
    unsafe {
        while *((p + v.len() as u64) as *const u8) != 0 {
            v.push(*((p + v.len() as u64) as *const u8));
        }
    }
    v
}

pub(crate) fn cadena_w(p: u64) -> Vec<u16> {
    let mut v = Vec::new();
    // SAFETY: una cadena UTF-16 del `.exe` acabada en 0.
    unsafe {
        while *((p as *const u16).add(v.len())) != 0 {
            v.push(*((p as *const u16).add(v.len())));
        }
    }
    v
}

impl Argumentos for Va {
    fn entero(&mut self) -> u64 {
        // SAFETY: el `va_list` que dio el `.exe`: una ranura por argumento.
        let v = unsafe { self.0.read_unaligned() };
        // SAFETY: la siguiente ranura.
        self.0 = unsafe { self.0.add(1) };
        v
    }
    fn cadena(&mut self, p: u64) -> Vec<u8> {
        cadena_c(p)
    }
    fn cadena_ancha(&mut self, p: u64) -> Vec<u8> {
        bmo_proton_x::texto::a_estrecho(&cadena_w(p), false).unwrap_or_default()
    }
}

/// `_CRT_INTERNAL_PRINTF_LEGACY_VSPRINTF_NULL_TERMINATION`,
/// `..._STANDARD_SNPRINTF_BEHAVIOR` y `..._LEGACY_WIDE_SPECIFIERS`.
pub(crate) const NULO_LEGADO: u64 = 1;
pub(crate) const SNPRINTF_ESTANDAR: u64 = 2;
pub(crate) const ANCHOS_LEGADOS: u64 = 4;

pub(crate) fn formatear_a(fmt: *const u8, va: u64) -> Vec<u8> {
    formato::formatear(&cadena_c(fmt as u64), &mut Va(va as *const u64), false)
}

pub(crate) fn formatear_w(opciones: u64, fmt: *const u16, va: u64) -> Vec<u8> {
    let f = bmo_proton_x::texto::a_estrecho(&cadena_w(fmt as u64), false).unwrap_or_default();
    formato::formatear(&f, &mut Va(va as *const u64), opciones & ANCHOS_LEGADOS != 0)
}

extern "win64" fn stdio_vfprintf(_op: u64, f: u64, fmt: *const u8, _loc: u64, va: u64) -> i32 {
    a_stream(f, &formatear_a(fmt, va))
}

extern "win64" fn stdio_vfwprintf(op: u64, f: u64, fmt: *const u16, _loc: u64, va: u64) -> i32 {
    a_stream(f, &formatear_w(op, fmt, va))
}

/// Dejar `r` (sin su 0) en un bufer de `n` elementos con las reglas del
/// UCRT: cabe con su 0, el largo; `buf` NULL y `n` 0, el largo que haria
/// falta; no cabe: estandar (snprintf) corta con su 0 y da el largo entero;
/// legado (lo de _vsnprintf) da -2 y solo pone el 0 si se pidio.
pub(crate) fn a_bufer<T: Copy + Default>(r: &[T], buf: *mut T, n: usize, opciones: u64) -> i32 {
    if buf.is_null() && n == 0 {
        return r.len() as i32;
    }
    if buf.is_null() {
        return -1;
    }
    let cabe = r.len() < n;
    let k = r.len().min(n);
    // SAFETY: el `.exe` da `n` elementos en `buf`.
    unsafe { core::ptr::copy_nonoverlapping(r.as_ptr(), buf, if cabe { r.len() } else { k.saturating_sub((opciones & (SNPRINTF_ESTANDAR | NULO_LEGADO) != 0) as usize) }) };
    if cabe {
        // SAFETY: cabe con su 0.
        unsafe { *buf.add(r.len()) = T::default() };
        return r.len() as i32;
    }
    if n > 0 && opciones & (SNPRINTF_ESTANDAR | NULO_LEGADO) != 0 {
        // SAFETY: el ultimo elemento del bufer.
        unsafe { *buf.add(n - 1) = T::default() };
    }
    // Legado: -2, "no cabia" (lo vio Windows el 27-09). Es la funcion en
    // linea `_vsnprintf` de las cabeceras la que lo vuelve -1.
    if opciones & SNPRINTF_ESTANDAR != 0 {
        r.len() as i32
    } else {
        -2
    }
}

extern "win64" fn stdio_vsprintf(op: u64, buf: *mut u8, n: usize, fmt: *const u8, _loc: u64, va: u64) -> i32 {
    a_bufer(&formatear_a(fmt, va), buf, n, op)
}

extern "win64" fn stdio_vswprintf(op: u64, buf: *mut u16, n: usize, fmt: *const u16, _loc: u64, va: u64) -> i32 {
    let w: Vec<u16> = alloc::string::String::from_utf8_lossy(&formatear_w(op, fmt, va)).encode_utf16().collect();
    a_bufer(&w, buf, n, op)
}

extern "win64" fn puts(s: *const u8) -> i32 {
    let mut b = cadena_c(s as u64);
    b.push(b'\n');
    if a_stream(acrt_iob_func(1), &b) < 0 {
        -1
    } else {
        0
    }
}

extern "win64" fn fputs(s: *const u8, f: u64) -> i32 {
    if a_stream(f, &cadena_c(s as u64)) < 0 {
        -1
    } else {
        0
    }
}

extern "win64" fn fputc(c: i32, f: u64) -> i32 {
    if a_stream(f, &[c as u8]) < 0 {
        -1
    } else {
        c & 0xFF
    }
}

extern "win64" fn putchar(c: i32) -> i32 {
    fputc(c, acrt_iob_func(1))
}

extern "win64" fn fwrite(p: *const u8, medida: usize, n: usize, f: u64) -> usize {
    let Some(t) = medida.checked_mul(n).filter(|&t| t > 0) else { return 0 };
    // SAFETY: `medida * n` bytes del `.exe`.
    let b = unsafe { core::slice::from_raw_parts(p, t) };
    if a_stream(f, b) < 0 {
        0
    } else {
        n
    }
}

extern "win64" fn fflush(f: u64) -> i32 {
    // Nada en un bufer: cada escritura salio ya. NULL (todos) tambien vale.
    if f == 0 || cual(f).is_some() || crate::crt_ficheros::es_flujo(f) {
        0
    } else {
        -1
    }
}

// -- Lo de P3c1: lo que un .exe de MSVC pide ademas ----------------------------

// `ceil(double)`: el `double` llega y vuelve en xmm0, y la casa es soft-float
// (no sabe de xmm0): tres instrucciones a mano. ROUNDSD (SSE4.1) con modo 2,
// hacia +infinito, es exactamente ceil: -0.5 da -0, y NaN/inf quedan igual.
core::arch::global_asm!(".globl proton_x_ceil", "proton_x_ceil:", "roundsd xmm0, xmm0, 2", "ret");
core::arch::global_asm!(".globl proton_x_floor", "proton_x_floor:", "roundsd xmm0, xmm0, 1", "ret");
extern "C" {
    fn proton_x_ceil();
    fn proton_x_floor();
}

/// `_register_thread_local_exe_atexit_callback`: lo que el CRT estatico de
/// un `.exe` registra para sus `thread_local` (un callback de TLS); se llama
/// al salir con DLL_PROCESS_DETACH.
extern "win64" fn register_tl_atexit(cb: u64) {
    estado().tl_atexit = cb;
}

/// `terminate()`: como `abort` del UCRT, sale con 3.
pub(crate) extern "win64" fn terminate() -> ! {
    crate::aviso("terminate(): el .exe se termina (como abort, codigo 3)");
    (plataforma().salir)(3)
}

const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

/// `_seh_filter_exe(codigo, punteros)`: el filtro del `__except` que rodea al
/// `main` de un `.exe` de MSVC. El del UCRT atiende lo que se puso con
/// `signal()` de C; en la casa no hay ninguna puesta, asi que ninguna
/// excepcion es suya: CONTINUE_SEARCH, como el UCRT sin ninguna.
pub(crate) extern "win64" fn seh_filter_exe(_codigo: u32, _punteros: u64) -> i32 {
    EXCEPTION_CONTINUE_SEARCH
}


/// Si `dll` es del CRT: la suya, `vcruntime140.dll` o un API set `api-ms-win-crt-*`.
pub(crate) fn es_del_crt(dll: &str) -> bool {
    // Tanda 1 de Cyberpunk: tambien msvcrt.dll (el CRT de Windows, que piden
    // las DLL de terceros) y vcruntime140_1.dll.
    dll.eq_ignore_ascii_case("ucrtbase.dll") || dll.eq_ignore_ascii_case("vcruntime140.dll") || dll.eq_ignore_ascii_case("vcruntime140_1.dll") || dll.eq_ignore_ascii_case("msvcrt.dll") || (dll.len() > 15 && dll.as_bytes()[..15].eq_ignore_ascii_case(b"api-ms-win-crt-"))
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    // `api-ms-win-crt-private` exporta las mismas con `_o_` delante.
    if let Some(r) = n.strip_prefix("_o_") {
        return buscar(r);
    }
    esta(n).or_else(|| crate::cxx::buscar(n)).or_else(|| crate::crt_cadenas::buscar(n)).or_else(|| crate::crt_numeros::buscar(n)).or_else(|| crate::crt_entorno::buscar(n)).or_else(|| crate::crt_mates::buscar(n)).or_else(|| crate::crt_ficheros::buscar(n)).or_else(|| crate::msvcp_errores::buscar_rtti(n)).or_else(|| crate::rtti::buscar(n))
}

fn esta(n: &str) -> Option<u64> {
    Some(match n {
        "_configure_narrow_argv" | "_configure_wide_argv" => dir!(configure_argv),
        // P4c: el manejador de `__try` de C tambien lo da vcruntime140.
        "__C_specific_handler" => dir!(crate::excepciones::c_specific_handler),
        "__p___argc" => dir!(p_argc),
        "__p___argv" => dir!(p_argv),
        "__p___wargv" => dir!(p_wargv),
        "_initialize_narrow_environment" | "_initialize_wide_environment" => dir!(initialize_environment),
        "_get_initial_narrow_environment" => dir!(get_initial_narrow_environment),
        "_get_initial_wide_environment" => dir!(get_initial_wide_environment),
        "_initterm" => dir!(initterm),
        "_initterm_e" => dir!(initterm_e),
        "_set_app_type" | "__setusermatherr" => dir!(nada),
        "_set_new_mode" | "_set_fmode" | "_callnewh" => dir!(cero),
        "_configthreadlocale" => dir!(configthreadlocale),
        "__p__commode" => dir!(p_commode),
        "_crt_atexit" => dir!(crt_atexit),
        "exit" => dir!(exit),
        "_exit" | "_Exit" => dir!(exit_rapido),
        "_cexit" => dir!(cexit),
        "_c_exit" => dir!(nada),
        "_initialize_onexit_table" => dir!(initialize_onexit_table),
        "_register_onexit_function" => dir!(register_onexit_function),
        "_execute_onexit_table" => dir!(execute_onexit_table),
        "malloc" => dir!(malloc),
        "calloc" => dir!(calloc),
        "realloc" => dir!(realloc),
        "free" => dir!(free),
        "memcpy" => dir!(memcpy),
        "memmove" => dir!(memmove),
        "memset" => dir!(memset),
        "memcmp" => dir!(memcmp),
        "memchr" => dir!(memchr),
        "strlen" => dir!(strlen),
        "wcslen" => dir!(wcslen),
        "strcmp" => dir!(strcmp),
        "strncmp" => dir!(strncmp),
        "ceil" => dir!(proton_x_ceil),
        "floor" => dir!(proton_x_floor),
        "_register_thread_local_exe_atexit_callback" => dir!(register_tl_atexit),
        "terminate" => dir!(terminate),
        "__acrt_iob_func" => dir!(acrt_iob_func),
        "__stdio_common_vfprintf" | "__stdio_common_vfprintf_s" => dir!(stdio_vfprintf),
        "__stdio_common_vfwprintf" => dir!(stdio_vfwprintf),
        "__stdio_common_vsprintf" => dir!(stdio_vsprintf),
        "__stdio_common_vswprintf" => dir!(stdio_vswprintf),
        "puts" => dir!(puts),
        "fputs" => dir!(fputs),
        "fputc" | "putc" => dir!(fputc),
        "putchar" => dir!(putchar),
        "fwrite" => dir!(fwrite),
        "fflush" => dir!(fflush),
        // P3c (BMOX-12): el filtro de `main` y lo de C++ (el panic de Rust).
        "_seh_filter_exe" => dir!(seh_filter_exe),
        _ => return None,
    })
}
