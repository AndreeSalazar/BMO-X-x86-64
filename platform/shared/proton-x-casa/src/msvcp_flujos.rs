//! **Los flujos de la biblioteca de C++ de MSVC, de la casa** (tanda 19 de
//! Cyberpunk, 30-09): `basic_streambuf`, `basic_ios`, `basic_ostream`,
//! `basic_istream` y `basic_iostream` de `char`, `cerr`, `setw` y
//! `_Fiopen`, con las clases de MSVC x64 byte a byte.
//!
//! ```text
//!    streambuf   104 bytes: vptr; los punteros de lectura y escritura y los
//!                seis que apuntan a ellos (_IGfirst...); dos cuentas y sus
//!                punteros; el locale. Vtabla de 15: dtor _Lock _Unlock
//!                overflow pbackfail showmanyc underflow uflow xsgetn xsputn
//!                seekoff seekpos setbuf sync imbue
//!    ios_base    72: vptr, _Stdstr, estado, excepciones, banderas,
//!                precision, ancho, dos listas y el locale; basic_ios le
//!                pone el streambuf, el tie y el relleno: 96
//!    ostream     BASE VIRTUAL: vbptr (su vbtable dice donde esta el ios) y
//!                el basic_ios detras: 104. istream lleva ademas _Chcount
//!                (112) e iostream los dos (120)
//! ```
//!
//! Un constructor de una clase con base virtual lleva un argumento de mas,
//! el ULTIMO: si es la clase entera (pone los vbptr y construye el ios) o
//! una parte de otra. Lo que vive en el ios se busca SIEMPRE por el vbptr.
//! Los numeros salen como los de `num_put` con el locale "C": base, signo,
//! prefijo, ancho y relleno (a la izquierda, a la derecha o por dentro).
//!
//! Lo que no, dicho: un estado que pide excepcion (`exceptions()`) se avisa
//! y NO se lanza; `iword`/`pword` no se liberan al destruir.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::msvcp_locale::{self, Locale};
use crate::{aviso, crt, crt_ficheros, dir, kernel32};

const EOF: i32 = -1;
const BADBIT: i32 = 4;
const UNITBUF: i32 = 0x2;
const SKIPWS_DEC: i32 = 0x201;


/// `basic_streambuf<char>`.
#[repr(C)]
pub(crate) struct Sb {
    pub(crate) vt: *const u64,
    pub(crate) gfirst: *mut u8,
    pub(crate) pfirst: *mut u8,
    pub(crate) igfirst: *mut *mut u8,
    pub(crate) ipfirst: *mut *mut u8,
    pub(crate) gnext: *mut u8,
    pub(crate) pnext: *mut u8,
    pub(crate) ignext: *mut *mut u8,
    pub(crate) ipnext: *mut *mut u8,
    pub(crate) gcount: i32,
    pub(crate) pcount: i32,
    pub(crate) igcount: *mut i32,
    pub(crate) ipcount: *mut i32,
    pub(crate) ploc: *mut Locale,
}

/// `basic_ios<char>` (con su `ios_base` delante).
#[repr(C)]
pub(crate) struct Ios {
    pub(crate) vt: *const u64,
    pub(crate) stdstr: usize,
    pub(crate) estado: i32,
    pub(crate) excepciones: i32,
    pub(crate) banderas: i32,
    pub(crate) prec: i64,
    pub(crate) ancho: i64,
    pub(crate) arr: u64,
    pub(crate) calls: u64,
    pub(crate) ploc: *mut Locale,
    pub(crate) sb: *mut Sb,
    pub(crate) tie: *mut u8,
    pub(crate) relleno: u8,
}

// -- Por la vtabla ----------------------------------------------------------------------------

/// El hueco `h` de la vtabla de un streambuf (de la casa o del `.exe`).
pub(crate) fn hueco(sb: *const Sb, h: usize) -> u64 {
    // SAFETY: un streambuf con su vtabla de 15.
    unsafe { *(*sb).vt.add(h) }
}

fn v_overflow(sb: *mut Sb, c: i32) -> i32 {
    // SAFETY: el hueco 3, `overflow(int)`.
    unsafe { core::mem::transmute::<u64, extern "win64" fn(*mut Sb, i32) -> i32>(hueco(sb, 3))(sb, c) }
}

fn v_underflow(sb: *mut Sb) -> i32 {
    // SAFETY: el hueco 6.
    unsafe { core::mem::transmute::<u64, extern "win64" fn(*mut Sb) -> i32>(hueco(sb, 6))(sb) }
}

fn v_uflow(sb: *mut Sb) -> i32 {
    // SAFETY: el hueco 7.
    unsafe { core::mem::transmute::<u64, extern "win64" fn(*mut Sb) -> i32>(hueco(sb, 7))(sb) }
}

fn v_xsputn(sb: *mut Sb, p: *const u8, n: i64) -> i64 {
    // SAFETY: el hueco 9.
    unsafe { core::mem::transmute::<u64, extern "win64" fn(*mut Sb, *const u8, i64) -> i64>(hueco(sb, 9))(sb, p, n) }
}

fn v_sync(sb: *mut Sb) -> i32 {
    // SAFETY: el hueco 13.
    unsafe { core::mem::transmute::<u64, extern "win64" fn(*mut Sb) -> i32>(hueco(sb, 13))(sb) }
}

pub(crate) fn v_lock(sb: *mut Sb, abrir: bool) {
    // SAFETY: los huecos 1 y 2.
    unsafe { core::mem::transmute::<u64, extern "win64" fn(*mut Sb)>(hueco(sb, if abrir { 2 } else { 1 }))(sb) }
}

// -- basic_streambuf -------------------------------------------------------------------------

/// Los seis punteros a los suyos, y los buferes vacios (`_Init()`).
extern "win64" fn sb_init(this: *mut Sb) {
    // SAFETY: un streambuf de 104 bytes.
    let s = unsafe { &mut *this };
    s.igfirst = &mut s.gfirst;
    s.ipfirst = &mut s.pfirst;
    s.ignext = &mut s.gnext;
    s.ipnext = &mut s.pnext;
    s.igcount = &mut s.gcount;
    s.ipcount = &mut s.pcount;
    s.gfirst = core::ptr::null_mut();
    s.pfirst = core::ptr::null_mut();
    s.gnext = core::ptr::null_mut();
    s.pnext = core::ptr::null_mut();
    s.gcount = 0;
    s.pcount = 0;
}

extern "win64" fn sb_nuevo(this: *mut Sb) -> *mut Sb {
    // SAFETY: el streambuf del `.exe`.
    unsafe {
        (*this).vt = vtabla(VT_SB);
        (*this).ploc = msvcp_locale::locale_nuevo();
    }
    sb_init(this);
    this
}

extern "win64" fn sb_fin(this: *mut Sb) {
    // SAFETY: un streambuf.
    unsafe {
        msvcp_locale::locale_soltar((*this).ploc);
        (*this).ploc = core::ptr::null_mut();
    }
}

extern "win64" fn sb_borrar(this: *mut Sb, flags: u32) -> *mut Sb {
    sb_fin(this);
    if flags & 1 != 0 {
        crt::free(this as u64);
    }
    this
}

extern "win64" fn sb_nada(_this: u64) {}

extern "win64" fn sb_eof(_this: u64, _c: i32) -> i32 {
    EOF
}

extern "win64" fn sb_cero(_this: u64) -> i64 {
    0
}

extern "win64" fn sb_sync(_this: u64) -> i32 {
    0
}

extern "win64" fn sb_setbuf(this: u64, _p: u64, _n: i64) -> u64 {
    this
}

extern "win64" fn sb_imbue(_this: u64, _loc: u64) {}

/// `seekoff`/`seekpos` de la base: la posicion -1 (un `fpos` de 24 bytes,
/// por puntero).
extern "win64" fn sb_seek(_this: u64, r: *mut [i64; 3], _a: u64, _b: u64, _c: u64) -> *mut [i64; 3] {
    // SAFETY: la vuelta del `.exe`.
    unsafe { r.write([-1, 0, 0]) };
    r
}

pub(crate) fn gnavail(s: &Sb) -> i64 {
    // SAFETY: los punteros de `_Init`, o los del `.exe`.
    unsafe {
        if (*s.ignext).is_null() {
            0
        } else {
            *s.igcount as i64
        }
    }
}

pub(crate) fn pnavail(s: &Sb) -> i64 {
    // SAFETY: como `gnavail`.
    unsafe {
        if (*s.ipnext).is_null() {
            0
        } else {
            *s.ipcount as i64
        }
    }
}

/// `_Gninc()`: el de ahora, y un paso.
pub(crate) fn gninc(s: &mut Sb) -> *mut u8 {
    // SAFETY: como `gnavail`, con uno disponible.
    unsafe {
        *s.igcount -= 1;
        let p = *s.ignext;
        *s.ignext = p.add(1);
        p
    }
}

extern "win64" fn sb_pninc(this: *mut Sb) -> *mut u8 {
    // SAFETY: un streambuf, con sitio para uno.
    unsafe {
        let s = &mut *this;
        *s.ipcount -= 1;
        let p = *s.ipnext;
        *s.ipnext = p.add(1);
        p
    }
}

extern "win64" fn sb_uflow(this: *mut Sb) -> i32 {
    if v_underflow(this) == EOF {
        return EOF;
    }
    // SAFETY: underflow dejo uno.
    unsafe { *gninc(&mut *this) as i32 }
}

extern "win64" fn sb_sbumpc(this: *mut Sb) -> i32 {
    // SAFETY: un streambuf.
    let s = unsafe { &mut *this };
    if gnavail(s) > 0 {
        // SAFETY: hay uno.
        unsafe { *gninc(s) as i32 }
    } else {
        v_uflow(this)
    }
}

extern "win64" fn sb_sputc(this: *mut Sb, c: u8) -> i32 {
    // SAFETY: un streambuf.
    if pnavail(unsafe { &*this }) > 0 {
        // SAFETY: hay sitio.
        unsafe { *sb_pninc(this) = c };
        c as i32
    } else {
        v_overflow(this, c as i32)
    }
}

extern "win64" fn sb_sputn(this: *mut Sb, p: *const u8, n: i64) -> i64 {
    v_xsputn(this, p, n)
}

extern "win64" fn sb_xsgetn(this: *mut Sb, mut p: *mut u8, n: i64) -> i64 {
    let mut falta = n;
    while falta > 0 {
        // SAFETY: un streambuf.
        let s = unsafe { &mut *this };
        let hay = gnavail(s).min(falta);
        if hay > 0 {
            // SAFETY: `hay` del bufer de lectura al del `.exe`.
            unsafe {
                core::ptr::copy(*s.ignext, p, hay as usize);
                p = p.add(hay as usize);
                *s.igcount -= hay as i32;
                *s.ignext = (*s.ignext).add(hay as usize);
            }
            falta -= hay;
        } else {
            let c = v_uflow(this);
            if c == EOF {
                break;
            }
            // SAFETY: el bufer del `.exe`.
            unsafe {
                *p = c as u8;
                p = p.add(1);
            }
            falta -= 1;
        }
    }
    n - falta
}

extern "win64" fn sb_xsputn(this: *mut Sb, mut p: *const u8, n: i64) -> i64 {
    let mut falta = n;
    while falta > 0 {
        // SAFETY: un streambuf.
        let s = unsafe { &mut *this };
        let hay = pnavail(s).min(falta);
        if hay > 0 {
            // SAFETY: `hay` del `.exe` al bufer de escritura.
            unsafe {
                core::ptr::copy(p, *s.ipnext, hay as usize);
                p = p.add(hay as usize);
                *s.ipcount -= hay as i32;
                *s.ipnext = (*s.ipnext).add(hay as usize);
            }
            falta -= hay;
        } else {
            // SAFETY: uno del `.exe`.
            if v_overflow(this, unsafe { *p } as i32) == EOF {
                break;
            }
            // SAFETY: lo mismo.
            p = unsafe { p.add(1) };
            falta -= 1;
        }
    }
    n - falta
}

/// `getloc() const`: una copia de su locale (vuelve por puntero).
extern "win64" fn sb_getloc(this: *const Sb, r: *mut Locale) -> *mut Locale {
    // SAFETY: un streambuf.
    msvcp_locale::locale_copiar(unsafe { (*this).ploc }, r);
    r
}

// -- El streambuf de cerr ---------------------------------------------------------------------

fn a_stderr(b: &[u8]) {
    let mut n = 0u32;
    kernel32::write_file(kernel32::estandar(-12), b.as_ptr(), b.len() as u32, &mut n, 0);
}

extern "win64" fn err_overflow(_this: u64, c: i32) -> i32 {
    if c != EOF {
        a_stderr(&[c as u8]);
    }
    c.max(0)
}

extern "win64" fn err_xsputn(_this: u64, p: *const u8, n: i64) -> i64 {
    if n > 0 {
        // SAFETY: `n` bytes del `.exe`.
        a_stderr(unsafe { core::slice::from_raw_parts(p, n as usize) });
    }
    n.max(0)
}

extern "win64" fn err_borrar(this: u64, _flags: u32) -> u64 {
    this
}

// -- Las vtablas ------------------------------------------------------------------------------

const VT_SB: usize = 0;
const VT_SB_ERR: usize = 1;
const VT_IOS: usize = 2;
const VT_OS: usize = 3;
const VT_IS: usize = 4;
const VT_IOSTREAM: usize = 5;

struct Tablas(UnsafeCell<[*const u64; 6]>);
// SAFETY: se llenan una vez; los hilos de la casa son cooperativos.
unsafe impl Sync for Tablas {}
static TABLAS: Tablas = Tablas(UnsafeCell::new([core::ptr::null(); 6]));

fn vtabla(k: usize) -> *const u64 {
    // SAFETY: ver `Tablas`.
    let cache = unsafe { &mut *TABLAS.0.get() };
    if !cache[k].is_null() {
        return cache[k];
    }
    let v: Vec<u64> = match k {
        VT_SB | VT_SB_ERR => {
            let mut v = alloc::vec![
                0,
                dir!(sb_borrar),
                dir!(sb_nada),
                dir!(sb_nada),
                dir!(sb_eof),
                dir!(sb_eof),
                dir!(sb_cero),
                dir!(sb_eof),
                dir!(sb_uflow),
                dir!(sb_xsgetn),
                dir!(sb_xsputn),
                dir!(sb_seek),
                dir!(sb_seek),
                dir!(sb_setbuf),
                dir!(sb_sync),
                dir!(sb_imbue),
            ];
            if k == VT_SB_ERR {
                v[1] = dir!(err_borrar);
                v[4] = dir!(err_overflow);
                v[10] = dir!(err_xsputn);
            }
            v
        }
        VT_IOS => alloc::vec![0, dir!(ios_borrar)],
        VT_OS => alloc::vec![0, dir!(os_borrar)],
        VT_IS => alloc::vec![0, dir!(is_borrar)],
        _ => alloc::vec![0, dir!(iostream_borrar)],
    };
    // SAFETY: el hueco 0 es el del RTTI; la vtabla empieza en el 1.
    cache[k] = unsafe { Vec::leak(v).as_ptr().add(1) };
    cache[k]
}

/// Las vbtables: [lo que hay del vbptr al principio de su parte, lo que
/// hay del vbptr al ios].
static VBT_OS: [i32; 2] = [0, 8];
static VBT_IS: [i32; 2] = [0, 16];
static VBT_IOSTREAM_IS: [i32; 2] = [0, 24];
static VBT_IOSTREAM_OS: [i32; 2] = [0, 8];

/// El ios de un flujo con base virtual (`this` apunta a su vbptr).
pub(crate) fn ios_de(this: *mut u8) -> *mut Ios {
    // SAFETY: un flujo con su vbptr puesto.
    unsafe {
        let vbt = *(this as *const *const i32);
        this.offset(*vbt.add(1) as isize) as *mut Ios
    }
}

// -- basic_ios --------------------------------------------------------------------------------

/// `basic_ios()` (protegido): todo a cero, con su vtabla.
extern "win64" fn ios_nuevo(this: *mut Ios) -> *mut Ios {
    // SAFETY: los 96 bytes del `.exe`.
    unsafe {
        core::ptr::write_bytes(this as *mut u8, 0, core::mem::size_of::<Ios>());
        (*this).vt = vtabla(VT_IOS);
    }
    this
}

/// `init(sb, isstd)`: `_Init()` del ios_base y el streambuf, sin tie, el
/// relleno es `widen(' ')`.
fn ios_init(ios: *mut Ios, sb: *mut Sb, isstd: bool) {
    // SAFETY: un ios.
    let i = unsafe { &mut *ios };
    i.stdstr = isstd as usize;
    i.excepciones = 0;
    i.banderas = SKIPWS_DEC;
    i.prec = 6;
    i.ancho = 0;
    i.arr = 0;
    i.calls = 0;
    i.estado = 0;
    i.ploc = msvcp_locale::locale_nuevo();
    i.sb = sb;
    i.tie = core::ptr::null_mut();
    i.relleno = ios_widen(ios, b' ');
    if sb.is_null() {
        i.estado = BADBIT;
    }
}

/// `~basic_ios()` (y el `~ios_base` que lleva dentro): suelta su locale,
/// salvo en los estandar.
pub(crate) extern "win64" fn ios_fin(this: *mut Ios) {
    // SAFETY: un ios.
    let i = unsafe { &mut *this };
    if i.stdstr == 0 {
        msvcp_locale::locale_soltar(i.ploc);
        i.ploc = core::ptr::null_mut();
    }
}

extern "win64" fn ios_borrar(this: *mut Ios, flags: u32) -> *mut Ios {
    ios_fin(this);
    if flags & 1 != 0 {
        crt::free(this as u64);
    }
    this
}

/// El destructor que borra de un flujo con base virtual: `this` es su ios,
/// `atras` bytes detras del principio.
fn borrar_desde(ios: *mut Ios, atras: usize, flags: u32) -> *mut u8 {
    // SAFETY: el ios de un flujo entero.
    let todo = unsafe { (ios as *mut u8).sub(atras) };
    ios_fin(ios);
    if flags & 1 != 0 {
        crt::free(todo as u64);
    }
    todo
}

extern "win64" fn os_borrar(this: *mut Ios, flags: u32) -> *mut u8 {
    borrar_desde(this, 8, flags)
}

extern "win64" fn is_borrar(this: *mut Ios, flags: u32) -> *mut u8 {
    borrar_desde(this, 16, flags)
}

extern "win64" fn iostream_borrar(this: *mut Ios, flags: u32) -> *mut u8 {
    borrar_desde(this, 24, flags)
}

/// `clear(estado, relanzar)`: sin streambuf, badbit.
extern "win64" fn ios_clear(this: *mut Ios, estado: i32, _relanzar: bool) {
    // SAFETY: un ios.
    let i = unsafe { &mut *this };
    let e = (estado | if i.sb.is_null() { BADBIT } else { 0 }) & 0x17;
    i.estado = e;
    if e & i.excepciones != 0 {
        aviso("ios_base::failure: el flujo pide una excepcion y la casa no la lanza");
    }
}

pub(crate) extern "win64" fn ios_setstate(this: *mut Ios, estado: i32, relanzar: bool) {
    // SAFETY: un ios.
    let e = unsafe { (*this).estado };
    ios_clear(this, e | estado, relanzar);
}

extern "win64" fn ios_widen(this: *const Ios, c: u8) -> u8 {
    // SAFETY: un ios.
    let f = msvcp_locale::ctype_de(unsafe { (*this).ploc });
    msvcp_locale::ensanchar(f, c)
}

/// `ios_base::getloc() const`.
extern "win64" fn ios_getloc(this: *const Ios, r: *mut Locale) -> *mut Locale {
    // SAFETY: un ios.
    msvcp_locale::locale_copiar(unsafe { (*this).ploc }, r);
    r
}

// -- basic_ostream, basic_istream, basic_iostream ------------------------------------------------

/// `basic_ostream(sb, isstd)` y, el ultimo, si es la clase entera.
extern "win64" fn os_nuevo(this: *mut u8, sb: *mut Sb, isstd: bool, entera: i32) -> *mut u8 {
    if entera != 0 {
        // SAFETY: el flujo del `.exe` (104 bytes).
        unsafe {
            *(this as *mut *const i32) = VBT_OS.as_ptr();
            ios_nuevo(this.add(8) as *mut Ios);
        }
    }
    let ios = ios_de(this);
    ios_init(ios, sb, isstd);
    // SAFETY: su ios.
    unsafe { (*ios).vt = vtabla(VT_OS) };
    this
}

extern "win64" fn is_nuevo(this: *mut u8, sb: *mut Sb, isstd: bool, entera: i32) -> *mut u8 {
    if entera != 0 {
        // SAFETY: el flujo del `.exe` (112 bytes).
        unsafe {
            *(this as *mut *const i32) = VBT_IS.as_ptr();
            ios_nuevo(this.add(16) as *mut Ios);
        }
    }
    // SAFETY: `_Chcount`, detras del vbptr.
    unsafe { *(this.add(8) as *mut i64) = 0 };
    let ios = ios_de(this);
    ios_init(ios, sb, isstd);
    // SAFETY: su ios.
    unsafe { (*ios).vt = vtabla(VT_IS) };
    this
}

/// `basic_iostream(sb)`: la parte de istream (que hace el `init`) y la de
/// ostream (sin init: `_Noinit`).
extern "win64" fn iostream_nuevo(this: *mut u8, sb: *mut Sb, entera: i32) -> *mut u8 {
    if entera != 0 {
        // SAFETY: el flujo del `.exe` (120 bytes).
        unsafe {
            *(this as *mut *const i32) = VBT_IOSTREAM_IS.as_ptr();
            *(this.add(16) as *mut *const i32) = VBT_IOSTREAM_OS.as_ptr();
            ios_nuevo(this.add(24) as *mut Ios);
        }
    }
    is_nuevo(this, sb, false, 0);
    // SAFETY: su ios.
    unsafe { (*ios_de(this)).vt = vtabla(VT_IOSTREAM) };
    this
}

/// Los destructores de las partes (`??1`): no hay nada suyo que soltar; el
/// ios lo suelta `~basic_ios`, que llama el `.exe`.
extern "win64" fn flujo_fin(_this: u64) {}

/// El centinela de ostream: bloquea el streambuf, mira el estado y vacia
/// el tie. Devuelve si se puede escribir.
fn centinela(os: *mut u8) -> bool {
    let ios = ios_de(os);
    // SAFETY: un ios.
    let i = unsafe { &mut *ios };
    if !i.sb.is_null() {
        v_lock(i.sb, false);
    }
    if i.estado != 0 {
        return false;
    }
    if !i.tie.is_null() && i.tie != os {
        os_flush(i.tie);
    }
    // SAFETY: un ios.
    unsafe { (*ios).estado == 0 }
}

/// El final del centinela: `_Osfx` y desbloquear.
fn fin_centinela(os: *mut u8) {
    os_osfx(os);
    // SAFETY: un ios.
    let sb = unsafe { (*ios_de(os)).sb };
    if !sb.is_null() {
        v_lock(sb, true);
    }
}

extern "win64" fn os_osfx(os: *mut u8) {
    let ios = ios_de(os);
    // SAFETY: un ios.
    let i = unsafe { &*ios };
    if i.estado == 0 && i.banderas & UNITBUF != 0 && v_sync(i.sb) == -1 {
        ios_setstate(ios, BADBIT, false);
    }
}

pub(crate) extern "win64" fn os_flush(os: *mut u8) -> *mut u8 {
    let ios = ios_de(os);
    // SAFETY: un ios.
    let sb = unsafe { (*ios).sb };
    if !sb.is_null() {
        if centinela(os) && v_sync(sb) == -1 {
            ios_setstate(ios, BADBIT, false);
        }
        fin_centinela(os);
    }
    os
}

/// Escribir `b` con el centinela; si no entra entero, badbit.
pub(crate) fn escribir(os: *mut u8, b: &[u8]) -> *mut u8 {
    let ios = ios_de(os);
    let mut mal = !centinela(os);
    if !mal && !b.is_empty() {
        // SAFETY: un ios con su streambuf (el centinela lo dijo).
        mal = v_xsputn(unsafe { (*ios).sb }, b.as_ptr(), b.len() as i64) != b.len() as i64;
    }
    if mal {
        ios_setstate(ios, BADBIT, false);
    }
    fin_centinela(os);
    os
}

extern "win64" fn os_put(os: *mut u8, c: u8) -> *mut u8 {
    let ios = ios_de(os);
    // SAFETY: un ios.
    if !centinela(os) || sb_sputc(unsafe { (*ios).sb }, c) == EOF {
        ios_setstate(ios, BADBIT, false);
    }
    fin_centinela(os);
    os
}

extern "win64" fn os_write(os: *mut u8, p: *const u8, n: i64) -> *mut u8 {
    // SAFETY: `n` bytes del `.exe`.
    escribir(os, if n > 0 { unsafe { core::slice::from_raw_parts(p, n as usize) } } else { &[] })
}

/// Un entero como `num_put` en "C": `neg` y la magnitud; `con_signo` dice
/// si `showpos` cuenta. El ancho se gasta.
pub(crate) fn numero(os: *mut u8, neg: bool, v: u64, con_signo: bool) -> *mut u8 {
    // SAFETY: un ios.
    let i = unsafe { &mut *ios_de(os) };
    let f = i.banderas;
    let base = match f & 0x0E00 {
        0x0800 => 16,
        0x0400 => 8,
        _ => 10,
    };
    let mut delante: Vec<u8> = Vec::new();
    if base == 10 && neg {
        delante.push(b'-');
    } else if base == 10 && con_signo && f & 0x20 != 0 {
        delante.push(b'+');
    }
    let mayus = f & 0x4 != 0;
    if f & 0x8 != 0 && v != 0 {
        match base {
            16 => delante.extend_from_slice(if mayus { b"0X" } else { b"0x" }),
            8 => delante.push(b'0'),
            _ => {}
        }
    }
    let mut cifras: Vec<u8> = Vec::new();
    let mut x = v;
    loop {
        let d = (x % base) as u8;
        cifras.push(match d {
            0..=9 => b'0' + d,
            _ if mayus => b'A' + d - 10,
            _ => b'a' + d - 10,
        });
        x /= base;
        if x == 0 {
            break;
        }
    }
    cifras.reverse();
    let largo = delante.len() + cifras.len();
    let falta = (i.ancho.max(0) as usize).saturating_sub(largo);
    let relleno = alloc::vec![i.relleno; falta];
    let mut t: Vec<u8> = Vec::with_capacity(largo + falta);
    match f & 0x1C0 {
        0x40 => t.extend_from_slice(&[&delante[..], &cifras, &relleno].concat()),
        0x100 => t.extend_from_slice(&[&delante[..], &relleno, &cifras].concat()),
        _ => t.extend_from_slice(&[&relleno[..], &delante, &cifras].concat()),
    }
    i.ancho = 0;
    escribir(os, &t)
}

/// En octal y hexadecimal, un `int` negativo sale como su `unsigned`.
pub(crate) fn con_signo(os: *mut u8, v: i64, bits: u32) -> *mut u8 {
    // SAFETY: un ios.
    let base10 = unsafe { (*ios_de(os)).banderas } & 0x0C00 == 0;
    if base10 || v >= 0 {
        numero(os, v < 0, v.unsigned_abs(), true)
    } else {
        numero(os, false, (v as u64) & (u64::MAX >> (64 - bits)), true)
    }
}

extern "win64" fn os_int(os: *mut u8, v: i32) -> *mut u8 {
    con_signo(os, v as i64, 32)
}

extern "win64" fn os_u32(os: *mut u8, v: u32) -> *mut u8 {
    numero(os, false, v as u64, false)
}

extern "win64" fn os_u64(os: *mut u8, v: u64) -> *mut u8 {
    numero(os, false, v, false)
}

/// `<<` de un manipulador de ostream (`endl`...): lo que el devuelva.
extern "win64" fn os_manip(os: *mut u8, f: extern "win64" fn(*mut u8) -> *mut u8) -> *mut u8 {
    f(os)
}

/// `<<` de un manipulador de ios_base (`hex`...): sobre el ios.
extern "win64" fn os_manip_ios(os: *mut u8, f: extern "win64" fn(*mut Ios) -> *mut Ios) -> *mut u8 {
    f(ios_de(os));
    os
}

// -- setw, _Fiopen y cerr ----------------------------------------------------------------------

extern "win64" fn poner_ancho(ios: *mut Ios, n: i64) {
    // SAFETY: un ios.
    unsafe { (*ios).ancho = n };
}

/// `setw(n)`: un `_Smanip<streamsize>` (la funcion y el numero; 16 bytes,
/// vuelve por puntero).
extern "win64" fn setw(r: *mut [u64; 2], n: i64) -> *mut [u64; 2] {
    // SAFETY: la vuelta del `.exe`.
    unsafe { r.write([dir!(poner_ancho), n as u64]) };
    r
}

/// `_Fiopen(ruta, modo, proteccion)`: el FILE de un basic_filebuf. El modo
/// de ios_base a uno de fopen, como la tabla de MSVC; `ate`, al final.
pub(crate) extern "win64" fn fiopen(ruta: *const u8, modo: i32, _prot: i32) -> u64 {
    const IN: i32 = 1;
    const OUT: i32 = 2;
    const ATE: i32 = 4;
    const APP: i32 = 8;
    const TRUNC: i32 = 0x10;
    const BINARY: i32 = 0x20;
    let m = modo & !(ATE | BINARY | 0xC0);
    let base: &[u8] = if m == OUT || m == OUT | TRUNC {
        b"w"
    } else if m == OUT | APP || m == APP {
        b"a"
    } else if m == IN {
        b"r"
    } else if m == IN | OUT {
        b"r+"
    } else if m == IN | OUT | TRUNC {
        b"w+"
    } else if m == IN | OUT | APP || m == IN | APP {
        b"a+"
    } else {
        return 0;
    };
    let mut f: Vec<u8> = base.to_vec();
    if modo & BINARY != 0 {
        f.push(b'b');
    }
    f.push(0);
    let fp = crt_ficheros::fopen(ruta, f.as_ptr());
    if fp != 0 && modo & ATE != 0 && crt_ficheros::fseeki64(fp, 0, 2) != 0 {
        crt_ficheros::fclose(fp);
        return 0;
    }
    fp
}

#[repr(C, align(8))]
struct Crudo<const N: usize>(UnsafeCell<[u8; N]>);
// SAFETY: ver `Tablas`.
unsafe impl<const N: usize> Sync for Crudo<N> {}
static CERR: Crudo<104> = Crudo(UnsafeCell::new([0; 104]));
static CERR_SB: Crudo<104> = Crudo(UnsafeCell::new([0; 104]));

/// `std::cerr`: un ostream sobre un streambuf que escribe en stderr, sin
/// bufer (`unitbuf`). La tabla da su DIRECCION sin tocarlo (el censo y el
/// cargador la piden antes de que la casa arranque: construirlo ahi pedia
/// memoria sin plataforma, metal 30-09); se construye en `reiniciar`, al
/// arrancar la casa, antes de saltar al `.exe`.
fn cerr() -> u64 {
    CERR.0.get() as u64
}

pub(crate) fn reiniciar() {
    let os = CERR.0.get() as *mut u8;
    let sb = CERR_SB.0.get() as *mut Sb;
    sb_nuevo(sb);
    // SAFETY: recien hecho; los 104 bytes de aqui.
    unsafe { (*sb).vt = vtabla(VT_SB_ERR) };
    os_nuevo(os, sb, true, 1);
    // SAFETY: su ios.
    unsafe { (*ios_de(os)).banderas |= UNITBUF };
}

/// `cerr` es un DATO: la tabla da su direccion y el diario no la envuelve.
pub(crate) fn es_dato(n: &str) -> bool {
    n == CERR_N
}

const CERR_N: &str = "?cerr@std@@3V?$basic_ostream@DU?$char_traits@D@std@@@1@A";

pub(crate) fn buscar(n: &str) -> Option<u64> {
    if n == CERR_N {
        return Some(cerr());
    }
    Some(match n {
        "??0?$basic_streambuf@DU?$char_traits@D@std@@@std@@IEAA@XZ" => dir!(sb_nuevo),
        "??1?$basic_streambuf@DU?$char_traits@D@std@@@std@@UEAA@XZ" => dir!(sb_fin),
        "?_Init@?$basic_streambuf@DU?$char_traits@D@std@@@std@@IEAAXXZ" => dir!(sb_init),
        "?_Lock@?$basic_streambuf@DU?$char_traits@D@std@@@std@@UEAAXXZ" | "?_Unlock@?$basic_streambuf@DU?$char_traits@D@std@@@std@@UEAAXXZ" => dir!(sb_nada),
        "?_Pninc@?$basic_streambuf@DU?$char_traits@D@std@@@std@@IEAAPEADXZ" => dir!(sb_pninc),
        "?getloc@?$basic_streambuf@DU?$char_traits@D@std@@@std@@QEBA?AVlocale@2@XZ" => dir!(sb_getloc),
        "?imbue@?$basic_streambuf@DU?$char_traits@D@std@@@std@@MEAAXAEBVlocale@2@@Z" => dir!(sb_imbue),
        "?sbumpc@?$basic_streambuf@DU?$char_traits@D@std@@@std@@QEAAHXZ" => dir!(sb_sbumpc),
        "?setbuf@?$basic_streambuf@DU?$char_traits@D@std@@@std@@MEAAPEAV12@PEAD_J@Z" => dir!(sb_setbuf),
        "?showmanyc@?$basic_streambuf@DU?$char_traits@D@std@@@std@@MEAA_JXZ" => dir!(sb_cero),
        "?sputc@?$basic_streambuf@DU?$char_traits@D@std@@@std@@QEAAHD@Z" => dir!(sb_sputc),
        "?sputn@?$basic_streambuf@DU?$char_traits@D@std@@@std@@QEAA_JPEBD_J@Z" => dir!(sb_sputn),
        "?sync@?$basic_streambuf@DU?$char_traits@D@std@@@std@@MEAAHXZ" => dir!(sb_sync),
        "?uflow@?$basic_streambuf@DU?$char_traits@D@std@@@std@@MEAAHXZ" => dir!(sb_uflow),
        "?xsgetn@?$basic_streambuf@DU?$char_traits@D@std@@@std@@MEAA_JPEAD_J@Z" => dir!(sb_xsgetn),
        "?xsputn@?$basic_streambuf@DU?$char_traits@D@std@@@std@@MEAA_JPEBD_J@Z" => dir!(sb_xsputn),
        "??0?$basic_ios@DU?$char_traits@D@std@@@std@@IEAA@XZ" => dir!(ios_nuevo),
        "??1?$basic_ios@DU?$char_traits@D@std@@@std@@UEAA@XZ" => dir!(ios_fin),
        "?clear@?$basic_ios@DU?$char_traits@D@std@@@std@@QEAAXH_N@Z" => dir!(ios_clear),
        "?setstate@?$basic_ios@DU?$char_traits@D@std@@@std@@QEAAXH_N@Z" => dir!(ios_setstate),
        "?widen@?$basic_ios@DU?$char_traits@D@std@@@std@@QEBADD@Z" => dir!(ios_widen),
        "?getloc@ios_base@std@@QEBA?AVlocale@2@XZ" => dir!(ios_getloc),
        "??0?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAA@PEAV?$basic_streambuf@DU?$char_traits@D@std@@@1@_N@Z" => dir!(os_nuevo),
        "??0?$basic_istream@DU?$char_traits@D@std@@@std@@QEAA@PEAV?$basic_streambuf@DU?$char_traits@D@std@@@1@_N@Z" => dir!(is_nuevo),
        "??0?$basic_iostream@DU?$char_traits@D@std@@@std@@QEAA@PEAV?$basic_streambuf@DU?$char_traits@D@std@@@1@@Z" => dir!(iostream_nuevo),
        "??1?$basic_ostream@DU?$char_traits@D@std@@@std@@UEAA@XZ" | "??1?$basic_istream@DU?$char_traits@D@std@@@std@@UEAA@XZ" | "??1?$basic_iostream@DU?$char_traits@D@std@@@std@@UEAA@XZ" => dir!(flujo_fin),
        "?_Osfx@?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAXXZ" => dir!(os_osfx),
        "?flush@?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV12@XZ" => dir!(os_flush),
        "?put@?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV12@D@Z" => dir!(os_put),
        "?write@?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV12@PEBD_J@Z" => dir!(os_write),
        "??6?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV01@H@Z" => dir!(os_int),
        "??6?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV01@I@Z" | "??6?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV01@K@Z" => dir!(os_u32),
        "??6?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV01@_K@Z" => dir!(os_u64),
        "??6?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV01@P6AAEAV01@AEAV01@@Z@Z" => dir!(os_manip),
        "??6?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV01@P6AAEAVios_base@1@AEAV21@@Z@Z" => dir!(os_manip_ios),
        "?setw@std@@YA?AU?$_Smanip@_J@1@_J@Z" => dir!(setw),
        "?_Fiopen@std@@YAPEAU_iobuf@@PEBDHH@Z" => dir!(fiopen),
        _ => return None,
    })
}

// -- Para time_put (msvcp_tiempo) ---------------------------------------------------------------

/// `sputc` de un streambuf cualquiera (el de un ostreambuf_iterator).
pub(crate) fn sputc(sb: u64, c: u8) -> i32 {
    sb_sputc(sb as *mut Sb, c)
}

/// El locale de un `ios_base&`.
pub(crate) fn locale_de_ios(ios: u64) -> *const Locale {
    // SAFETY: un ios_base del `.exe`.
    unsafe { (*(ios as *const Ios)).ploc }
}

#[cfg(test)]
mod pruebas {
    /// El censo y el cargador buscan `cerr` ANTES de que la casa arranque
    /// (sin plataforma): buscarlo no puede pedir memoria (metal 30-09).
    #[test]
    fn cerr_se_busca_sin_plataforma() {
        assert_eq!(super::buscar(super::CERR_N), Some(super::CERR.0.get() as u64));
    }
}
