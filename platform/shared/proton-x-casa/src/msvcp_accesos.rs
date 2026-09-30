//! **Lo que MSVC pone inline, por si no lo pone** (30-09): los accesores
//! de ios_base, basic_ios, basic_streambuf y basic_istream. El juego (con
//! /O2) los lleva dentro; un `.exe` hecho con /O1 los IMPORTA de msvcp140
//! (tanda19m.exe, cl 19.44: una treintena). Cada uno es lo que dice su
//! cabecera, sobre los campos de `msvcp_flujos`.
//!
//! ```text
//!    ios_base    good flags setf(2) unsetf precision width(2) rdstate
//!    basic_ios   tie rdbuf fill(2)
//!    streambuf   eback gptr egptr pbase pptr epptr gbump pbump setg setp(2)
//!                _Gninc _Gndec _Gnavail _Pnavail _Init(6) sgetn
//!    ostream     ??_D (el destructor de la clase entera: el del ios)
//!    istream     gcount
//! ```

use crate::dir;
use crate::msvcp_flujos::{self, Ios, Sb};

// SAFETY de todo el fichero: `this` es un objeto del `.exe` de la clase que
// dice el nombre, con los campos de MSVC (ver `msvcp_flujos`).

extern "win64" fn good(i: *const Ios) -> bool {
    // SAFETY: ver arriba.
    unsafe { (*i).estado == 0 }
}

extern "win64" fn rdstate(i: *const Ios) -> i32 {
    // SAFETY: ver arriba.
    unsafe { (*i).estado }
}

extern "win64" fn flags(i: *const Ios) -> i32 {
    // SAFETY: ver arriba.
    unsafe { (*i).banderas }
}

extern "win64" fn setf(i: *mut Ios, f: i32) -> i32 {
    // SAFETY: ver arriba.
    unsafe {
        let v = (*i).banderas;
        (*i).banderas = v | (f & 0xFFFF);
        v
    }
}

extern "win64" fn setf_mascara(i: *mut Ios, f: i32, m: i32) -> i32 {
    // SAFETY: ver arriba.
    unsafe {
        let v = (*i).banderas;
        (*i).banderas = (v & !m) | (f & m & 0xFFFF);
        v
    }
}

extern "win64" fn unsetf(i: *mut Ios, m: i32) {
    // SAFETY: ver arriba.
    unsafe { (*i).banderas &= !m };
}

extern "win64" fn precision(i: *const Ios) -> i64 {
    // SAFETY: ver arriba.
    unsafe { (*i).prec }
}

extern "win64" fn width(i: *const Ios) -> i64 {
    // SAFETY: ver arriba.
    unsafe { (*i).ancho }
}

extern "win64" fn width_poner(i: *mut Ios, n: i64) -> i64 {
    // SAFETY: ver arriba.
    unsafe { core::mem::replace(&mut (*i).ancho, n) }
}

extern "win64" fn tie(i: *const Ios) -> u64 {
    // SAFETY: ver arriba.
    unsafe { (*i).tie as u64 }
}

extern "win64" fn rdbuf(i: *const Ios) -> u64 {
    // SAFETY: ver arriba.
    unsafe { (*i).sb as u64 }
}

extern "win64" fn fill(i: *const Ios) -> u8 {
    // SAFETY: ver arriba.
    unsafe { (*i).relleno }
}

extern "win64" fn fill_poner(i: *mut Ios, c: u8) -> u8 {
    // SAFETY: ver arriba.
    unsafe { core::mem::replace(&mut (*i).relleno, c) }
}

/// `??_D` de basic_ostream: la clase entera (`this` a su vbptr); lo suyo es
/// nada, y luego su ios.
extern "win64" fn os_destruir(os: *mut u8) {
    msvcp_flujos::ios_fin(msvcp_flujos::ios_de(os));
}

extern "win64" fn gcount(is: *const u8) -> i64 {
    // SAFETY: `_Chcount`, detras del vbptr.
    unsafe { *(is.add(8) as *const i64) }
}

// -- basic_streambuf ---------------------------------------------------------------------------

extern "win64" fn eback(s: *const Sb) -> *mut u8 {
    // SAFETY: ver arriba.
    unsafe { *(*s).igfirst }
}

extern "win64" fn gptr(s: *const Sb) -> *mut u8 {
    // SAFETY: ver arriba.
    unsafe { *(*s).ignext }
}

extern "win64" fn egptr(s: *const Sb) -> *mut u8 {
    // SAFETY: ver arriba.
    unsafe { (*(*s).ignext).offset(*(*s).igcount as isize) }
}

extern "win64" fn pbase(s: *const Sb) -> *mut u8 {
    // SAFETY: ver arriba.
    unsafe { *(*s).ipfirst }
}

extern "win64" fn pptr(s: *const Sb) -> *mut u8 {
    // SAFETY: ver arriba.
    unsafe { *(*s).ipnext }
}

extern "win64" fn epptr(s: *const Sb) -> *mut u8 {
    // SAFETY: ver arriba.
    unsafe { (*(*s).ipnext).offset(*(*s).ipcount as isize) }
}

extern "win64" fn gbump(s: *mut Sb, n: i32) {
    // SAFETY: ver arriba.
    unsafe {
        *(*s).igcount -= n;
        *(*s).ignext = (*(*s).ignext).offset(n as isize);
    }
}

extern "win64" fn pbump(s: *mut Sb, n: i32) {
    // SAFETY: ver arriba.
    unsafe {
        *(*s).ipcount -= n;
        *(*s).ipnext = (*(*s).ipnext).offset(n as isize);
    }
}

extern "win64" fn setg(s: *mut Sb, f: *mut u8, n: *mut u8, l: *mut u8) {
    // SAFETY: ver arriba.
    unsafe {
        *(*s).igfirst = f;
        *(*s).ignext = n;
        *(*s).igcount = l.offset_from(n) as i32;
    }
}

extern "win64" fn setp(s: *mut Sb, f: *mut u8, l: *mut u8) {
    setp3(s, f, f, l);
}

extern "win64" fn setp3(s: *mut Sb, f: *mut u8, n: *mut u8, l: *mut u8) {
    // SAFETY: ver arriba.
    unsafe {
        *(*s).ipfirst = f;
        *(*s).ipnext = n;
        *(*s).ipcount = l.offset_from(n) as i32;
    }
}

extern "win64" fn gninc(s: *mut Sb) -> *mut u8 {
    // SAFETY: ver arriba.
    msvcp_flujos::gninc(unsafe { &mut *s })
}

extern "win64" fn gndec(s: *mut Sb) -> *mut u8 {
    // SAFETY: ver arriba.
    unsafe {
        *(*s).igcount += 1;
        *(*s).ignext = (*(*s).ignext).sub(1);
        *(*s).ignext
    }
}

extern "win64" fn gnavail(s: *const Sb) -> i64 {
    // SAFETY: ver arriba.
    msvcp_flujos::gnavail(unsafe { &*s })
}

extern "win64" fn pnavail(s: *const Sb) -> i64 {
    // SAFETY: ver arriba.
    msvcp_flujos::pnavail(unsafe { &*s })
}

/// `_Init(gf, gn, gc, pf, pn, pc)`: los seis punteros, a donde diga.
extern "win64" fn init6(s: *mut Sb, gf: *mut *mut u8, gn: *mut *mut u8, gc: *mut i32, pf: *mut *mut u8, pn: *mut *mut u8, pc: *mut i32) {
    // SAFETY: ver arriba.
    unsafe {
        (*s).igfirst = gf;
        (*s).ipfirst = pf;
        (*s).ignext = gn;
        (*s).ipnext = pn;
        (*s).igcount = gc;
        (*s).ipcount = pc;
    }
}

/// `sgetn` es `xsgetn` (hueco 8), la del streambuf de verdad.
extern "win64" fn sgetn(s: *mut Sb, p: *mut u8, n: i64) -> i64 {
    // SAFETY: el hueco 8 de su vtabla.
    unsafe { core::mem::transmute::<u64, extern "win64" fn(*mut Sb, *mut u8, i64) -> i64>(msvcp_flujos::hueco(s, 8))(s, p, n) }
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    const SB: &str = "?$basic_streambuf@DU?$char_traits@D@std@@@std@@";
    const IOS: &str = "?$basic_ios@DU?$char_traits@D@std@@@std@@";
    if let Some(r) = n.strip_prefix('?').and_then(|r| r.split_once('@')).filter(|(_, c)| c.starts_with(SB)).map(|(m, c)| (m, &c[SB.len()..])) {
        return Some(match r {
            ("eback", "IEBAPEADXZ") => dir!(eback),
            ("gptr", "IEBAPEADXZ") => dir!(gptr),
            ("egptr", "IEBAPEADXZ") => dir!(egptr),
            ("pbase", "IEBAPEADXZ") => dir!(pbase),
            ("pptr", "IEBAPEADXZ") => dir!(pptr),
            ("epptr", "IEBAPEADXZ") => dir!(epptr),
            ("gbump", "IEAAXH@Z") => dir!(gbump),
            ("pbump", "IEAAXH@Z") => dir!(pbump),
            ("setg", "IEAAXPEAD00@Z") => dir!(setg),
            ("setp", "IEAAXPEAD0@Z") => dir!(setp),
            ("setp", "IEAAXPEAD00@Z") => dir!(setp3),
            ("_Gninc", "IEAAPEADXZ") => dir!(gninc),
            ("_Gndec", "IEAAPEADXZ") => dir!(gndec),
            ("_Gnavail", "IEBA_JXZ") => dir!(gnavail),
            ("_Pnavail", "IEBA_JXZ") => dir!(pnavail),
            ("_Init", "IEAAXPEAPEAD0PEAH001@Z") => dir!(init6),
            ("sgetn", "QEAA_JPEAD_J@Z") => dir!(sgetn),
            _ => return None,
        });
    }
    if let Some(r) = n.strip_prefix('?').and_then(|r| r.split_once('@')).filter(|(_, c)| c.starts_with(IOS)).map(|(m, c)| (m, &c[IOS.len()..])) {
        return Some(match r {
            ("tie", "QEBAPEAV?$basic_ostream@DU?$char_traits@D@std@@@2@XZ") => dir!(tie),
            ("rdbuf", "QEBAPEAV?$basic_streambuf@DU?$char_traits@D@std@@@2@XZ") => dir!(rdbuf),
            ("fill", "QEBADXZ") => dir!(fill),
            ("fill", "QEAADD@Z") => dir!(fill_poner),
            _ => return None,
        });
    }
    Some(match n {
        "?good@ios_base@std@@QEBA_NXZ" => dir!(good),
        "?rdstate@ios_base@std@@QEBAHXZ" => dir!(rdstate),
        "?flags@ios_base@std@@QEBAHXZ" => dir!(flags),
        "?setf@ios_base@std@@QEAAHH@Z" => dir!(setf),
        "?setf@ios_base@std@@QEAAHHH@Z" => dir!(setf_mascara),
        "?unsetf@ios_base@std@@QEAAXH@Z" => dir!(unsetf),
        "?precision@ios_base@std@@QEBA_JXZ" => dir!(precision),
        "?width@ios_base@std@@QEBA_JXZ" => dir!(width),
        "?width@ios_base@std@@QEAA_J_J@Z" => dir!(width_poner),
        "??_D?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAXXZ" => dir!(os_destruir),
        "?gcount@?$basic_istream@DU?$char_traits@D@std@@@std@@QEBA_JXZ" => dir!(gcount),
        _ => return None,
    })
}
