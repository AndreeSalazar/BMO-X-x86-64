//! **`time_put<char>` de la casa** (tanda 19 de Cyberpunk, 30-09): la
//! faceta que escribe una fecha (`std::put_time`), con el `strftime` del
//! locale "C" de MSVC.
//!
//! ```text
//!    faceta      24 bytes: la de locale::facet y `_Tnames`; vtabla: las
//!                tres de facet y do_put
//!    put         recorre el patron: lo que no es % se copia; cada %x (y
//!                %Ex, %Ox, %#x) va a do_put POR LA VTABLA (la del juego, si
//!                la faceta es suya); uno que no existe se copia tal cual
//!    do_put      "C": %a %A %b %B %c (%m/%d/%y %H:%M:%S) %C %d %D %e %F %g
//!                %G %h %H %I %j %m %M %n %p %r %R %S %t %T %u %U %V %w %W
//!                %x %X %y %Y %z %Z %%; con '#', los numeros sin ceros
//! ```
//!
//! El iterador de salida (`ostreambuf_iterator`) son 16 bytes: si fallo y
//! su streambuf. Va y vuelve por puntero, como en MSVC.
//!
//! Lo que no, dicho: `%z` y `%Z` dicen `+0000` y `UTC` (la casa no tiene
//! zona horaria).

use alloc::vec::Vec;
use core::sync::atomic::AtomicU32;

use crate::msvcp_locale::{self, Faceta};
use crate::{crt, dir, msvcp_flujos};

const X_TIME: usize = 5;

#[repr(C)]
struct TimePut {
    f: Faceta,
    nombres: u64,
}

/// `ostreambuf_iterator<char>`.
#[repr(C)]
#[derive(Clone, Copy)]
struct Iter {
    fallo: bool,
    sb: u64,
}

impl Iter {
    fn poner(&mut self, c: u8) {
        if !self.fallo && msvcp_flujos::sputc(self.sb, c) == -1 {
            self.fallo = true;
        }
    }
}

struct Vt(core::cell::UnsafeCell<*const u64>);
// SAFETY: se llena una vez; los hilos de la casa son cooperativos.
unsafe impl Sync for Vt {}
static VT: Vt = Vt(core::cell::UnsafeCell::new(core::ptr::null()));

fn vtabla() -> *const u64 {
    // SAFETY: ver `Vt`.
    let vt = unsafe { &mut *VT.0.get() };
    if vt.is_null() {
        let v: Vec<u64> = alloc::vec![0, dir!(msvcp_locale::borrar_faceta), dir!(msvcp_locale::incref), dir!(msvcp_locale::decref), dir!(do_put)];
        // SAFETY: el hueco 0 es el del RTTI; la vtabla empieza en el 1.
        *vt = unsafe { Vec::leak(v).as_ptr().add(1) };
    }
    *vt
}

/// `time_put<char>::_Getcat`.
extern "win64" fn getcat(pf: *mut *mut Faceta, _loc: u64) -> usize {
    // SAFETY: un puntero del `.exe`, o nulo.
    if !pf.is_null() && unsafe { (*pf).is_null() } {
        let p = crt::malloc(core::mem::size_of::<TimePut>()) as *mut TimePut;
        if !p.is_null() {
            // SAFETY: un bloque recien pedido; el hueco del `.exe`.
            unsafe {
                p.write(TimePut { f: Faceta { vt: vtabla(), refs: AtomicU32::new(0) }, nombres: 0 });
                *pf = p as *mut Faceta;
            }
        }
    }
    X_TIME
}

const VALIDOS: &[u8] = b"aAbBcCdDeFgGhHIjmMnprRStTuUVwWxXyYzZ";

/// Los campos que usa cada especificador, en su rango (si no, MSVC pone ?).
fn datos_validos(e: u8, tm: &[i32; 9]) -> bool {
    let [seg, min, hora, dia, mes, _anio, dsem, dia_del_anio, _] = *tm;
    let en = |v: i32, a: i32, b: i32| (a..=b).contains(&v);
    match e {
        b'a' | b'A' | b'u' | b'w' => en(dsem, 0, 6),
        b'b' | b'B' | b'h' | b'm' => en(mes, 0, 11),
        b'd' | b'e' => en(dia, 1, 31),
        b'H' | b'I' | b'p' => en(hora, 0, 23),
        b'M' => en(min, 0, 59),
        b'S' => en(seg, 0, 60),
        b'j' => en(dia_del_anio, 0, 365),
        b'c' | b'D' | b'x' | b'F' => en(mes, 0, 11) && en(dia, 1, 31),
        b'r' | b'R' | b'T' | b'X' => en(hora, 0, 23) && en(min, 0, 59) && en(seg, 0, 60),
        _ => true,
    }
}

const DIAS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MESES: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

fn num(t: &mut Vec<u8>, v: i64, ancho: usize, sin_ceros: bool) {
    let s = alloc::format!("{v}");
    if !sin_ceros {
        for _ in s.len()..ancho {
            t.push(b'0');
        }
    }
    t.extend_from_slice(s.as_bytes());
}

/// El anio y la semana ISO 8601 (%G y %V).
fn iso(tm: &[i32; 9]) -> (i64, i64) {
    let anio = tm[5] as i64 + 1900;
    let dia_del_anio = tm[7] as i64;
    let lunes0 = (tm[6] as i64 + 6) % 7; // lunes = 0
    let semana = (dia_del_anio - lunes0 + 10) / 7;
    let bisiesto = |a: i64| a % 4 == 0 && (a % 100 != 0 || a % 400 == 0);
    let semanas = |a: i64| {
        // un anio tiene 53 si empieza en jueves, o en miercoles y es bisiesto
        let p = (lunes0 - dia_del_anio).rem_euclid(7); // dia (lunes 0) del 1 de enero
        let p = if a == anio { p } else { (p - if bisiesto(a) { 366 } else { 365 }).rem_euclid(7) };
        if p == 3 || (p == 2 && bisiesto(a)) {
            53
        } else {
            52
        }
    };
    if semana < 1 {
        (anio - 1, semanas(anio - 1))
    } else if semana > semanas(anio) {
        (anio + 1, 1)
    } else {
        (anio, semana)
    }
}

/// Un especificador, en "C".
fn formato(t: &mut Vec<u8>, e: u8, almohadilla: bool, tm: &[i32; 9]) {
    let [seg, min, hora, dia, mes, anio, dsem, dia_del_anio, _] = *tm;
    let z = almohadilla;
    let dia_s = DIAS[dsem.clamp(0, 6) as usize];
    let mes_s = MESES[mes.clamp(0, 11) as usize];
    let h12 = if hora % 12 == 0 { 12 } else { hora % 12 };
    let a = anio as i64 + 1900;
    match e {
        b'a' => t.extend_from_slice(&dia_s.as_bytes()[..3]),
        b'A' => t.extend_from_slice(dia_s.as_bytes()),
        b'b' | b'h' => t.extend_from_slice(&mes_s.as_bytes()[..3]),
        b'B' => t.extend_from_slice(mes_s.as_bytes()),
        b'c' => {
            formato(t, b'x', z, tm);
            t.push(b' ');
            formato(t, b'X', z, tm);
        }
        b'C' => num(t, a / 100, 2, z),
        b'd' => num(t, dia as i64, 2, z),
        b'D' | b'x' => {
            num(t, mes as i64 + 1, 2, z);
            t.push(b'/');
            num(t, dia as i64, 2, z);
            t.push(b'/');
            num(t, a % 100, 2, z);
        }
        b'e' => {
            if dia < 10 && !z {
                t.push(b' ');
            }
            num(t, dia as i64, 1, z);
        }
        b'F' => {
            num(t, a, 4, z);
            t.push(b'-');
            num(t, mes as i64 + 1, 2, z);
            t.push(b'-');
            num(t, dia as i64, 2, z);
        }
        b'g' => num(t, iso(tm).0 % 100, 2, z),
        b'G' => num(t, iso(tm).0, 4, z),
        b'V' => num(t, iso(tm).1, 2, z),
        b'H' => num(t, hora as i64, 2, z),
        b'I' => num(t, h12 as i64, 2, z),
        b'j' => num(t, dia_del_anio as i64 + 1, 3, z),
        b'm' => num(t, mes as i64 + 1, 2, z),
        b'M' => num(t, min as i64, 2, z),
        b'n' => t.push(b'\n'),
        b't' => t.push(b'\t'),
        b'p' => t.extend_from_slice(if hora < 12 { b"AM" } else { b"PM" }),
        b'r' => {
            num(t, h12 as i64, 2, z);
            t.push(b':');
            num(t, min as i64, 2, z);
            t.push(b':');
            num(t, seg as i64, 2, z);
            t.extend_from_slice(if hora < 12 { b" AM" } else { b" PM" });
        }
        b'R' => {
            num(t, hora as i64, 2, z);
            t.push(b':');
            num(t, min as i64, 2, z);
        }
        b'S' => num(t, seg as i64, 2, z),
        b'T' | b'X' => {
            num(t, hora as i64, 2, z);
            t.push(b':');
            num(t, min as i64, 2, z);
            t.push(b':');
            num(t, seg as i64, 2, z);
        }
        b'u' => num(t, if dsem == 0 { 7 } else { dsem as i64 }, 1, z),
        b'w' => num(t, dsem as i64, 1, z),
        b'U' => num(t, (dia_del_anio as i64 + 7 - dsem as i64) / 7, 2, z),
        b'W' => num(t, (dia_del_anio as i64 + 7 - (dsem as i64 + 6) % 7) / 7, 2, z),
        b'y' => num(t, a % 100, 2, z),
        b'Y' => num(t, a, 4, z),
        b'z' => t.extend_from_slice(b"+0000"),
        b'Z' => t.extend_from_slice(b"UTC"),
        _ => t.push(b'%'),
    }
}

/// `do_put(dest, ios, relleno, tm, especificador, modificador)` (hueco 3).
#[allow(clippy::too_many_arguments)]
extern "win64" fn do_put(_this: u64, r: *mut Iter, d: *const Iter, _ios: u64, _relleno: u64, tm: *const [i32; 9], e: u8, m: u8) -> *mut Iter {
    // SAFETY: el iterador y el tm del `.exe`.
    let (mut it, tm) = unsafe { (*d, &*tm) };
    let mut t = Vec::new();
    formato(&mut t, e, m == b'#', tm);
    for c in t {
        it.poner(c);
    }
    // SAFETY: la vuelta del `.exe`.
    unsafe { r.write(it) };
    r
}

/// `put(dest, ios, relleno, tm, patron, fin) const`: el patron, y cada
/// especificador por `do_put` de la vtabla de `this`.
#[allow(clippy::too_many_arguments)]
extern "win64" fn put(this: *const Faceta, r: *mut Iter, d: *const Iter, ios: u64, relleno: u64, tm: *const [i32; 9], mut p: *const u8, fin: *const u8) -> *mut Iter {
    type DoPut = extern "win64" fn(*const Faceta, *mut Iter, *const Iter, u64, u64, *const [i32; 9], u8, u8) -> *mut Iter;
    let ct = msvcp_locale::ctype_de(msvcp_flujos::locale_de_ios(ios));
    let estrecho = |c: u8| msvcp_locale::estrechar(ct, c, 0);
    // SAFETY: el iterador del `.exe`; su tm.
    let (mut it, tm_ref) = unsafe { (*d, &*tm) };
    // SAFETY: la vtabla de `this` (hueco 3).
    let do_put_v: DoPut = unsafe { core::mem::transmute(msvcp_locale::virtual_de(this, 3)) };
    // SAFETY: el tramo [p, fin) del `.exe`.
    let sig = |q: *const u8| unsafe { q.add(1) };
    while p < fin {
        // SAFETY: dentro del tramo.
        let c = unsafe { *p };
        if estrecho(c) != b'%' {
            it.poner(c);
            p = sig(p);
            continue;
        }
        p = sig(p);
        if p >= fin {
            it.poner(c);
            break;
        }
        // SAFETY: dentro del tramo.
        let mut e = estrecho(unsafe { *p });
        let mut m = 0u8;
        if matches!(e, b'E' | b'O' | b'Q' | b'#') {
            p = sig(p);
            if p >= fin {
                it.poner(c);
                it.poner(e);
                break;
            }
            m = e;
            // SAFETY: dentro del tramo.
            e = estrecho(unsafe { *p });
        }
        if e == b'%' && m == 0 {
            it.poner(c);
        } else if !VALIDOS.contains(&e) {
            it.poner(c);
            if m != 0 {
                it.poner(m);
            }
            it.poner(e);
        } else if datos_validos(e, tm_ref) {
            let mut vuelta = it;
            do_put_v(this, &mut vuelta, &it, ios, relleno, tm, e, m);
            it = vuelta;
        } else {
            it.poner(b'?');
        }
        p = sig(p);
    }
    // SAFETY: la vuelta del `.exe`.
    unsafe { r.write(it) };
    r
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "?_Getcat@?$time_put@DV?$ostreambuf_iterator@DU?$char_traits@D@std@@@std@@@std@@SA_KPEAPEBVfacet@locale@2@PEBV42@@Z" => dir!(getcat),
        "?put@?$time_put@DV?$ostreambuf_iterator@DU?$char_traits@D@std@@@std@@@std@@QEBA?AV?$ostreambuf_iterator@DU?$char_traits@D@std@@@2@V32@AEAVios_base@2@DPEBUtm@@PEBD3@Z" => dir!(put),
        _ => return None,
    })
}
