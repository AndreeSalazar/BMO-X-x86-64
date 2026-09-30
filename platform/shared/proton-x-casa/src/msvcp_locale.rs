//! **El locale de la biblioteca de C++ de MSVC, de la casa** (tanda 18 de
//! Cyberpunk, 30-09): lo que `msvcp140.dll` exporta de `std::locale` y sus
//! facetas, con las CLASES de MSVC x64 byte a byte, porque el juego las lee
//! y las llama por dentro (su codigo inline de las cabeceras).
//!
//! ```text
//!    locale      id (operator size_t), facet (ctor, dtor, _Incref,
//!                _Decref), _Locimp (_New_Locimp, _Addfac, _Locimp_Addfac),
//!                _Init y _Getgloballocale: el locale "C", el unico
//!    _Locinfo    ctor, dtor, _Getcoll, _Getcvt, _W_Getdays, _W_Getmonths
//!    _Yarn       operator= y c_str
//!    ctype       <char>: _Getcat y tolower (uno y un tramo)
//!    codecvt     <char,char>: _Getcat, in, out, unshift, always_noconv;
//!                <wchar_t,char>: ctor, dtor, in y out ("C": un byte es
//!                un caracter; de ancho a byte, lo que pasa de 255 es error)
//!    DATOS       los `id` de ctype, codecvt (las dos), collate y time_put
//! ```
//!
//! Las medidas, de las cabeceras publicas de la STL de Microsoft (la ABI,
//! no su codigo): facet 16 bytes (vptr y cuenta), _Locimp 56, _Locinfo 104,
//! _Yarn 16, ctype<char> 48, codecvt<char,char> 16, codecvt<wchar_t,char>
//! 64. Las vtablas, en el orden de MSVC: las sobrecargas de un mismo nombre
//! van juntas y AL REVES (do_tolower de un tramo antes que la de uno).
//!
//! **Los DATOS:** un `id` importado es una variable; la tabla da su
//! direccion y el diario no la envuelve ([`es_dato`]).
//!
//! Lo que no, dicho: solo existe el locale "C". Un nombre que no es "C" ni
//! "" se acepta y se avisa, y funciona como "C". El locale global empieza
//! SIN facetas: el juego las pide con `_Getcat` la primera vez, como hace
//! con las que el locale no tiene (en Windows vienen ya puestas).

use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};

use crate::{aviso, crt, dir};

// -- Las piezas, como las pone MSVC -----------------------------------------------------------

/// `locale::facet`: la vtabla y la cuenta de referencias.
#[repr(C)]
pub(crate) struct Faceta {
    pub(crate) vt: *const u64,
    pub(crate) refs: AtomicU32,
}

/// `_Yarn<char>` (y `<wchar_t>`: la misma forma): la cadena, suya.
#[repr(C)]
struct Hilo {
    p: *mut u8,
    nul: u16,
}

/// `locale::_Locimp`: las facetas de un locale, por su `id`.
#[repr(C)]
pub(crate) struct Locimp {
    f: Faceta,
    vec: *mut *mut Faceta,
    cuenta: usize,
    catmask: i32,
    xparent: bool,
    nombre: Hilo,
}

/// `_Locinfo`: su `_Lockit` y seis cadenas.
#[repr(C)]
struct Locinfo {
    lock: i32,
    dias: Hilo,
    meses: Hilo,
    wdias: Hilo,
    wmeses: Hilo,
    viejo: Hilo,
    nuevo: Hilo,
}

/// `_Cvtvec`: lo que un codecvt sabe del locale.
#[repr(C)]
#[derive(Clone, Copy)]
struct Cvtvec {
    pagina: u32,
    mbcurmax: u32,
    es_c: i32,
    principio: [u8; 32],
}

/// `_Ctypevec`.
#[repr(C)]
struct Ctypevec {
    pagina: u32,
    tabla: *const u16,
    borrar: i32,
    nombre: *mut u16,
}

#[repr(C)]
struct Ctype {
    f: Faceta,
    c: Ctypevec,
}

#[repr(C)]
struct CodecvtAncho {
    f: Faceta,
    cvt: Cvtvec,
}

/// El `_Cvtvec` del locale "C": un byte por caracter.
const CVT_C: Cvtvec = Cvtvec { pagina: 0, mbcurmax: 1, es_c: 1, principio: [0; 32] };

const X_CTYPE: usize = 2;
const OK: i32 = 0;
const PARCIAL: i32 = 1;
const ERROR: i32 = 2;
const NOCONV: i32 = 3;

// -- Pedir y soltar (el monton del CRT: lo que el juego libere con `free`) ---------------------

fn pedir<T>() -> *mut T {
    let p = crt::malloc(core::mem::size_of::<T>()) as *mut T;
    if p.is_null() {
        aviso("msvcp_locale: sin memoria");
    }
    p
}

fn dup(s: *const u8) -> *mut u8 {
    if s.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: una cadena C del `.exe` o de la casa.
    let n = unsafe { (0..).take_while(|&i| *s.add(i) != 0).count() };
    let p = crt::malloc(n + 1) as *mut u8;
    if !p.is_null() {
        // SAFETY: `n + 1` bytes, los dos lados.
        unsafe { core::ptr::copy_nonoverlapping(s, p, n + 1) };
    }
    p
}

// -- Las vtablas ----------------------------------------------------------------------------

const VT_FACETA: usize = 0;
const VT_LOCIMP: usize = 1;
const VT_CTYPE: usize = 2;
const VT_CODECVT: usize = 3;
const VT_CODECVT_ANCHO: usize = 4;

struct Vtablas(UnsafeCell<[*const u64; 5]>);
// SAFETY: se llenan una vez y no cambian (los hilos de la casa son
// cooperativos).
unsafe impl Sync for Vtablas {}
static VTABLAS: Vtablas = Vtablas(UnsafeCell::new([core::ptr::null(); 5]));

/// La vtabla `k`, hecha la primera vez. Delante, un hueco a cero donde MSVC
/// pone el RTTI (el Complete Object Locator).
fn vtabla(k: usize) -> *const u64 {
    // SAFETY: ver `Vtablas`.
    let cache = unsafe { &mut *VTABLAS.0.get() };
    if !cache[k].is_null() {
        return cache[k];
    }
    let mut v: Vec<u64> = alloc::vec![0, dir!(borrar_faceta), dir!(incref), dir!(decref)];
    match k {
        VT_LOCIMP => v[1] = dir!(borrar_locimp),
        VT_CTYPE => {
            v[1] = dir!(borrar_ctype);
            v.extend_from_slice(&[
                dir!(ctype_do_tolower_tramo),
                dir!(ctype_do_tolower),
                dir!(ctype_do_toupper_tramo),
                dir!(ctype_do_toupper),
                dir!(ctype_do_widen_tramo),
                dir!(ctype_do_widen),
                dir!(ctype_do_narrow_tramo),
                dir!(ctype_do_narrow),
            ]);
        }
        VT_CODECVT => v.extend_from_slice(&[dir!(cvt_si), dir!(cvt_uno), dir!(cvt_uno), dir!(cvt_do_in), dir!(cvt_do_out), dir!(cvt_do_unshift), dir!(cvt_do_length)]),
        VT_CODECVT_ANCHO => v.extend_from_slice(&[dir!(cvt_no), dir!(ancho_max_length), dir!(ancho_encoding), dir!(ancho_do_in), dir!(ancho_do_out), dir!(ancho_do_unshift), dir!(ancho_do_length)]),
        _ => {}
    }
    // SAFETY: el hueco 0 es el del RTTI; la vtabla empieza en el 1.
    cache[k] = unsafe { Vec::leak(v).as_ptr().add(1) };
    cache[k]
}

/// La funcion del hueco `h` de la vtabla de `this` (un objeto del `.exe`
/// o de la casa: se llama siempre por su vtabla, como el juego).
///
/// # Safety
/// `this` es un objeto con vptr y la vtabla tiene ese hueco.
pub(crate) unsafe fn virtual_de(this: *const Faceta, h: usize) -> u64 {
    // SAFETY: lo de arriba.
    unsafe { *(*this).vt.add(h) }
}

// -- facet ----------------------------------------------------------------------------------

/// `facet(size_t refs)`.
extern "win64" fn faceta_nueva(this: *mut Faceta, refs: usize) -> *mut Faceta {
    // SAFETY: la faceta del `.exe`, de 16 bytes como poco.
    unsafe { this.write(Faceta { vt: vtabla(VT_FACETA), refs: AtomicU32::new(refs as u32) }) };
    this
}

extern "win64" fn faceta_fin(_this: *mut Faceta) {}

pub(crate) extern "win64" fn incref(this: *const Faceta) {
    // SAFETY: una faceta.
    unsafe { (*this).refs.fetch_add(1, Ordering::AcqRel) };
}

pub(crate) extern "win64" fn decref(this: *mut Faceta) -> *mut Faceta {
    // SAFETY: una faceta.
    if unsafe { (*this).refs.fetch_sub(1, Ordering::AcqRel) } == 1 {
        this
    } else {
        core::ptr::null_mut()
    }
}

/// El destructor "que borra" (hueco 0): `flags & 1`, liberar.
pub(crate) extern "win64" fn borrar_faceta(this: *mut Faceta, flags: u32) -> *mut Faceta {
    if flags & 1 != 0 {
        crt::free(this as u64);
    }
    this
}

// -- locale::id ------------------------------------------------------------------------------

/// `id::operator size_t`: el numero, dado la primera vez.
/// `locale::id::_Id_cnt`: la cuenta de los `id`. Es un DATO: el
/// `_Get_index` inline de las cabeceras nuevas la sube el mismo (tanda19m,
/// de cl 19.44), asi que la casa usa la misma.
static ID_CNT: AtomicI32 = AtomicI32::new(0);

extern "win64" fn id_numero(this: *const AtomicU64) -> u64 {
    // SAFETY: un `id` (8 bytes) del `.exe` o de la casa.
    let id = unsafe { &*this };
    if id.load(Ordering::Acquire) == 0 {
        let n = (ID_CNT.fetch_add(1, Ordering::AcqRel) + 1) as u64;
        let _ = id.compare_exchange(0, n, Ordering::AcqRel, Ordering::Acquire);
    }
    id.load(Ordering::Acquire)
}

/// Los `id` que el juego importa como DATOS: ctype<char>,
/// codecvt<char,char>, codecvt<wchar_t,char>, collate<char> y
/// time_put<char>.
static IDS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

const NOMBRES_ID: [&str; 5] = [
    "?id@?$ctype@D@std@@2V0locale@2@A",
    "?id@?$codecvt@DDU_Mbstatet@@@std@@2V0locale@2@A",
    "?id@?$codecvt@_WDU_Mbstatet@@@std@@2V0locale@2@A",
    "?id@?$collate@D@std@@2V0locale@2@A",
    "?id@?$time_put@DV?$ostreambuf_iterator@DU?$char_traits@D@std@@@std@@@std@@2V0locale@2@A",
];

/// Si `n` es un DATO de msvcp140 (la tabla da su direccion: no se envuelve).
pub(crate) fn es_dato(n: &str) -> bool {
    NOMBRES_ID.contains(&n) || n == ID_CNT_N
}

const ID_CNT_N: &str = "?_Id_cnt@id@locale@std@@0HA";

// -- _Locimp ---------------------------------------------------------------------------------

struct Global(UnsafeCell<*mut Locimp>);
// SAFETY: ver `Vtablas`.
unsafe impl Sync for Global {}
static GLOBAL: Global = Global(UnsafeCell::new(core::ptr::null_mut()));

fn locimp_nuevo(nombre: *const u8) -> *mut Locimp {
    let p = pedir::<Locimp>();
    if !p.is_null() {
        let l = Locimp { f: Faceta { vt: vtabla(VT_LOCIMP), refs: AtomicU32::new(1) }, vec: core::ptr::null_mut(), cuenta: 0, catmask: 0, xparent: false, nombre: Hilo { p: dup(nombre), nul: 0 } };
        // SAFETY: un bloque recien pedido de su medida.
        unsafe { p.write(l) };
    }
    p
}

/// `locale::_Init(bool incref)`: el locale global, "C", hecho la primera vez.
extern "win64" fn locale_init(incref_: bool) -> *mut Locimp {
    // SAFETY: ver `Vtablas`.
    let g = unsafe { &mut *GLOBAL.0.get() };
    if g.is_null() {
        *g = locimp_nuevo(c"C".as_ptr() as *const u8);
        // SAFETY: recien hecho, o nulo.
        if let Some(l) = unsafe { g.as_mut() } {
            l.catmask = 0x3F; // locale::all
        }
    }
    if incref_ && !g.is_null() {
        incref(*g as *const Faceta);
    }
    *g
}

extern "win64" fn get_global_locale() -> *mut Locimp {
    locale_init(false)
}

/// `_Locimp::_New_Locimp(const _Locimp&)`: una copia, con sus facetas.
extern "win64" fn new_locimp(de: *const Locimp) -> *mut Locimp {
    // SAFETY: un _Locimp del `.exe` o de la casa.
    let de = unsafe { &*de };
    let p = locimp_nuevo(if de.nombre.p.is_null() { &de.nombre.nul as *const u16 as *const u8 } else { de.nombre.p });
    // SAFETY: recien hecho.
    let Some(l) = (unsafe { p.as_mut() }) else { return p };
    l.catmask = de.catmask;
    l.xparent = de.xparent;
    for i in 0..de.cuenta {
        // SAFETY: `cuenta` huecos.
        let f = unsafe { *de.vec.add(i) };
        if !f.is_null() {
            locimp_addfac(l, f, i);
        }
    }
    p
}

/// `_Locimp_Addfac(imp, faceta, id)`: pone la faceta en su hueco (el
/// vector crece; la que habia se suelta).
extern "win64" fn locimp_addfac(this: *mut Locimp, f: *mut Faceta, id: usize) {
    // SAFETY: un _Locimp.
    let l = unsafe { &mut *this };
    if l.cuenta <= id {
        let n = (id + 1).max(40);
        let v = crt::calloc(n, 8) as *mut *mut Faceta;
        if v.is_null() {
            aviso("msvcp_locale: sin memoria para las facetas");
            return;
        }
        if !l.vec.is_null() {
            // SAFETY: `cuenta` huecos viejos en un vector de `n`.
            unsafe { core::ptr::copy_nonoverlapping(l.vec, v, l.cuenta) };
            crt::free(l.vec as u64);
        }
        l.vec = v;
        l.cuenta = n;
    }
    incref(f);
    // SAFETY: `id < cuenta`.
    let hueco = unsafe { &mut *l.vec.add(id) };
    if !hueco.is_null() {
        soltar(*hueco);
    }
    *hueco = f;
}

/// `delete f->_Decref()`: por su vtabla (hueco 2, y el 0 con 1).
fn soltar(f: *mut Faceta) {
    // SAFETY: una faceta con su vtabla.
    unsafe {
        let d: extern "win64" fn(*mut Faceta) -> *mut Faceta = core::mem::transmute(virtual_de(f, 2));
        let r = d(f);
        if !r.is_null() {
            let b: extern "win64" fn(*mut Faceta, u32) -> *mut Faceta = core::mem::transmute(virtual_de(r, 0));
            b(r, 1);
        }
    }
}

extern "win64" fn borrar_locimp(this: *mut Locimp, flags: u32) -> *mut Locimp {
    // SAFETY: un _Locimp de la casa (lo dice su vtabla).
    let l = unsafe { &mut *this };
    for i in 0..l.cuenta {
        // SAFETY: `cuenta` huecos.
        let f = unsafe { *l.vec.add(i) };
        if !f.is_null() {
            soltar(f);
        }
    }
    crt::free(l.vec as u64);
    crt::free(l.nombre.p as u64);
    if flags & 1 != 0 {
        crt::free(this as u64);
    }
    this
}

/// **`std::locale` de MSVC: 16 bytes, y su `_Locimp` en el +8.** Hereda de
/// dos clases vacias (`_Locbase<int>` y `_Crt_new_delete`) y MSVC solo
/// aprovecha el hueco de la primera: la cabecera lo dice ("TRANSITION, ABI,
/// affects sizeof(locale)") y el `use_facet` de cl 19.44 lee `[loc+8]`
/// (tanda19m, 30-09). Con 8 bytes, getloc escribia donde no era.
#[repr(C)]
pub(crate) struct Locale {
    vacio: u64,
    pub(crate) ptr: *mut Locimp,
}

/// Un `locale` pedido al monton: el de un streambuf (`_Plocale`) o el de
/// un ios_base (`_Ploc`). Es el global.
pub(crate) fn locale_nuevo() -> *mut Locale {
    let p = pedir::<Locale>();
    if !p.is_null() {
        // SAFETY: un bloque recien pedido de su medida.
        unsafe { p.write(Locale { vacio: 0, ptr: locale_init(true) }) };
    }
    p
}

/// `delete` de un locale de [`locale_nuevo`]: suelta su `_Locimp`.
pub(crate) fn locale_soltar(p: *mut Locale) {
    if p.is_null() {
        return;
    }
    // SAFETY: un locale de la casa.
    let l = unsafe { (*p).ptr };
    if !l.is_null() {
        soltar(l as *mut Faceta);
    }
    crt::free(p as u64);
}

/// Una copia de `*de` en `a` (el `locale` que se devuelve por valor):
/// el mismo `_Locimp`, con una referencia mas.
pub(crate) fn locale_copiar(de: *const Locale, a: *mut Locale) {
    // SAFETY: dos locales, del `.exe` o de la casa.
    unsafe {
        let l = if de.is_null() { locale_init(false) } else { (*de).ptr };
        incref(l as *const Faceta);
        a.write(Locale { vacio: 0, ptr: l });
    }
}

/// La faceta `ctype<char>` de un locale (`use_facet`): la suya, o la de la
/// casa (una, hecha la primera vez).
pub(crate) fn ctype_de(loc: *const Locale) -> *const Faceta {
    let id = id_numero(&IDS[0]) as usize;
    // SAFETY: un locale; su _Locimp y su vector.
    unsafe {
        let l = if loc.is_null() { locale_init(false) } else { (*loc).ptr };
        if let Some(l) = l.as_ref() {
            if id < l.cuenta && !(*l.vec.add(id)).is_null() {
                return *l.vec.add(id);
            }
        }
    }
    static CASA: Global = Global(UnsafeCell::new(core::ptr::null_mut()));
    // SAFETY: ver `Vtablas`.
    let c = unsafe { &mut *CASA.0.get() };
    if c.is_null() {
        let mut f: *mut Faceta = core::ptr::null_mut();
        ctype_getcat(&mut f, 0);
        incref(f);
        *c = f as *mut Locimp;
    }
    *c as *const Faceta
}

/// `ctype<char>::widen` y `narrow` de una faceta, por su vtabla (huecos 8
/// y 10).
pub(crate) fn ensanchar(f: *const Faceta, c: u8) -> u8 {
    // SAFETY: una ctype<char>.
    unsafe {
        let g: extern "win64" fn(*const Faceta, u8) -> u8 = core::mem::transmute(virtual_de(f, 8));
        g(f, c)
    }
}

pub(crate) fn estrechar(f: *const Faceta, c: u8, defecto: u8) -> u8 {
    // SAFETY: una ctype<char>.
    unsafe {
        let g: extern "win64" fn(*const Faceta, u8, u8) -> u8 = core::mem::transmute(virtual_de(f, 10));
        g(f, c, defecto)
    }
}

// -- _Yarn ----------------------------------------------------------------------------------

/// `_Yarn<char>::operator=(const char*)`.
extern "win64" fn yarn_asignar(this: *mut Hilo, s: *const u8) -> *mut Hilo {
    // SAFETY: un _Yarn del `.exe`.
    let h = unsafe { &mut *this };
    if h.p as *const u8 != s {
        crt::free(h.p as u64);
        h.p = dup(s);
    }
    this
}

extern "win64" fn yarn_c_str(this: *const Hilo) -> *const u8 {
    // SAFETY: un _Yarn.
    let h = unsafe { &*this };
    if h.p.is_null() {
        &h.nul as *const u16 as *const u8
    } else {
        h.p
    }
}

// -- _Locinfo -------------------------------------------------------------------------------

const VACIO: Hilo = Hilo { p: core::ptr::null_mut(), nul: 0 };

extern "win64" fn locinfo_nuevo(this: *mut Locinfo, nombre: *const u8) -> *mut Locinfo {
    // SAFETY: una cadena C del `.exe`.
    let es_c = nombre.is_null() || unsafe { *nombre == 0 || (*nombre == b'C' && *nombre.add(1) == 0) };
    if !es_c {
        aviso("_Locinfo: la casa solo tiene el locale \"C\"; se usa ese");
    }
    let c = c"C".as_ptr() as *const u8;
    // SAFETY: el _Locinfo del `.exe` (104 bytes).
    unsafe { this.write(Locinfo { lock: 0, dias: VACIO, meses: VACIO, wdias: VACIO, wmeses: VACIO, viejo: Hilo { p: dup(c), nul: 0 }, nuevo: Hilo { p: dup(c), nul: 0 } }) };
    this
}

extern "win64" fn locinfo_fin(this: *mut Locinfo) {
    // SAFETY: un _Locinfo.
    let l = unsafe { &mut *this };
    for h in [&mut l.dias, &mut l.meses, &mut l.wdias, &mut l.wmeses, &mut l.viejo, &mut l.nuevo] {
        crt::free(h.p as u64);
        h.p = core::ptr::null_mut();
    }
}

/// `_Getcoll() const`: el `_Collvec` de "C" (pagina 0, sin nombre); la
/// vuelta es por puntero (16 bytes).
extern "win64" fn locinfo_getcoll(_this: u64, r: *mut [u64; 2]) -> *mut [u64; 2] {
    // SAFETY: la vuelta del `.exe`.
    unsafe { r.write([0, 0]) };
    r
}

extern "win64" fn locinfo_getcvt(_this: u64, r: *mut Cvtvec) -> *mut Cvtvec {
    // SAFETY: la vuelta del `.exe`.
    unsafe { r.write(CVT_C) };
    r
}

/// Una cadena ASCII, a UTF-16, en compilacion.
const fn ancho<const N: usize>(s: &str) -> [u16; N] {
    let b = s.as_bytes();
    let mut r = [0u16; N];
    let mut i = 0;
    while i < b.len() {
        r[i] = b[i] as u16;
        i += 1;
    }
    r
}

const DIAS: &str = ":Sun:Sunday:Mon:Monday:Tue:Tuesday:Wed:Wednesday:Thu:Thursday:Fri:Friday:Sat:Saturday";
const MESES: &str = ":Jan:January:Feb:February:Mar:March:Apr:April:May:May:Jun:June:Jul:July:Aug:August:Sep:September:Oct:October:Nov:November:Dec:December";
static WDIAS: [u16; DIAS.len() + 1] = ancho(DIAS);
static WMESES: [u16; MESES.len() + 1] = ancho(MESES);

extern "win64" fn locinfo_w_getdays(_this: u64) -> *const u16 {
    WDIAS.as_ptr()
}

extern "win64" fn locinfo_w_getmonths(_this: u64) -> *const u16 {
    WMESES.as_ptr()
}

// -- ctype<char> -----------------------------------------------------------------------------

/// `ctype<char>::_Getcat(&faceta, locale)`: la de "C", si no la tiene.
extern "win64" fn ctype_getcat(pf: *mut *mut Faceta, _loc: u64) -> usize {
    // SAFETY: un puntero del `.exe`, o nulo.
    if !pf.is_null() && unsafe { (*pf).is_null() } {
        let p = pedir::<Ctype>();
        if !p.is_null() {
            let c = Ctype { f: Faceta { vt: vtabla(VT_CTYPE), refs: AtomicU32::new(0) }, c: Ctypevec { pagina: 0, tabla: crate::crt_cadenas::tabla_ctype(), borrar: 0, nombre: core::ptr::null_mut() } };
            // SAFETY: un bloque recien pedido de su medida; y el hueco del `.exe`.
            unsafe {
                p.write(c);
                *pf = p as *mut Faceta;
            }
        }
    }
    X_CTYPE
}

extern "win64" fn borrar_ctype(this: *mut Faceta, flags: u32) -> *mut Faceta {
    borrar_faceta(this, flags)
}

extern "win64" fn ctype_do_tolower(_this: u64, c: u8) -> u8 {
    c.to_ascii_lowercase()
}

extern "win64" fn ctype_do_toupper(_this: u64, c: u8) -> u8 {
    c.to_ascii_uppercase()
}

fn tramo(a: *mut u8, b: *const u8, f: fn(u8) -> u8) -> *const u8 {
    let mut p = a;
    while (p as *const u8) < b {
        // SAFETY: el tramo [a, b) del `.exe`.
        unsafe {
            *p = f(*p);
            p = p.add(1);
        }
    }
    b
}

extern "win64" fn ctype_do_tolower_tramo(_this: u64, a: *mut u8, b: *const u8) -> *const u8 {
    tramo(a, b, |c| c.to_ascii_lowercase())
}

extern "win64" fn ctype_do_toupper_tramo(_this: u64, a: *mut u8, b: *const u8) -> *const u8 {
    tramo(a, b, |c| c.to_ascii_uppercase())
}

extern "win64" fn ctype_do_widen(_this: u64, c: u8) -> u8 {
    c
}

extern "win64" fn ctype_do_narrow(_this: u64, c: u8, _defecto: u8) -> u8 {
    c
}

fn copiar_tramo(a: *const u8, b: *const u8, d: *mut u8) -> *const u8 {
    if b > a {
        // SAFETY: el tramo [a, b) y su destino, del `.exe`.
        unsafe { core::ptr::copy(a, d, b.offset_from(a) as usize) };
    }
    b
}

extern "win64" fn ctype_do_widen_tramo(_this: u64, a: *const u8, b: *const u8, d: *mut u8) -> *const u8 {
    copiar_tramo(a, b, d)
}

extern "win64" fn ctype_do_narrow_tramo(_this: u64, a: *const u8, b: *const u8, _defecto: u8, d: *mut u8) -> *const u8 {
    copiar_tramo(a, b, d)
}

/// `ctype<char>::tolower(char) const`: por su vtabla (hueco 4).
extern "win64" fn ctype_tolower(this: *const Faceta, c: u8) -> u8 {
    // SAFETY: una ctype<char>, de la casa o derivada en el `.exe`.
    unsafe {
        let f: extern "win64" fn(*const Faceta, u8) -> u8 = core::mem::transmute(virtual_de(this, 4));
        f(this, c)
    }
}

/// `ctype<char>::tolower(char*, const char*) const` (hueco 3).
extern "win64" fn ctype_tolower_tramo(this: *const Faceta, a: *mut u8, b: *const u8) -> *const u8 {
    // SAFETY: como `ctype_tolower`.
    unsafe {
        let f: extern "win64" fn(*const Faceta, *mut u8, *const u8) -> *const u8 = core::mem::transmute(virtual_de(this, 3));
        f(this, a, b)
    }
}

// -- codecvt ---------------------------------------------------------------------------------

/// `codecvt<char,char,mbstate_t>::_Getcat`.
extern "win64" fn codecvt_getcat(pf: *mut *mut Faceta, _loc: u64) -> usize {
    // SAFETY: un puntero del `.exe`, o nulo.
    if !pf.is_null() && unsafe { (*pf).is_null() } {
        let p = pedir::<Faceta>();
        if !p.is_null() {
            // SAFETY: un bloque recien pedido; el hueco del `.exe`.
            unsafe {
                p.write(Faceta { vt: vtabla(VT_CODECVT), refs: AtomicU32::new(0) });
                *pf = p;
            }
        }
    }
    X_CTYPE
}

extern "win64" fn cvt_si(_this: u64) -> bool {
    true
}

extern "win64" fn cvt_no(_this: u64) -> bool {
    false
}

extern "win64" fn cvt_uno(_this: u64) -> i32 {
    1
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn cvt_do_in(_this: u64, _st: u64, a: u64, _b: u64, ma: *mut u64, d: u64, _e: u64, md: *mut u64) -> i32 {
    // SAFETY: los punteros de vuelta del `.exe`.
    unsafe {
        *ma = a;
        *md = d;
    }
    NOCONV
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn cvt_do_out(this: u64, st: u64, a: u64, b: u64, ma: *mut u64, d: u64, e: u64, md: *mut u64) -> i32 {
    cvt_do_in(this, st, a, b, ma, d, e, md)
}

extern "win64" fn cvt_do_unshift(_this: u64, _st: u64, d: u64, _e: u64, md: *mut u64) -> i32 {
    // SAFETY: el puntero de vuelta del `.exe`.
    unsafe { *md = d };
    NOCONV
}

extern "win64" fn cvt_do_length(_this: u64, _st: u64, a: u64, b: u64, n: usize) -> i32 {
    (b.saturating_sub(a) as usize).min(n).min(i32::MAX as usize) as i32
}

/// `codecvt_base::always_noconv() const` (hueco 3).
extern "win64" fn always_noconv(this: *const Faceta) -> bool {
    // SAFETY: un codecvt, por su vtabla.
    unsafe {
        let f: extern "win64" fn(*const Faceta) -> bool = core::mem::transmute(virtual_de(this, 3));
        f(this)
    }
}

/// `in`/`out` (huecos 6 y 7): siete argumentos detras de `this`.
type Convertir = extern "win64" fn(*const Faceta, u64, u64, u64, u64, u64, u64, u64) -> i32;

#[allow(clippy::too_many_arguments)]
extern "win64" fn codecvt_in(this: *const Faceta, st: u64, a: u64, b: u64, ma: u64, d: u64, e: u64, md: u64) -> i32 {
    // SAFETY: un codecvt, por su vtabla.
    unsafe { core::mem::transmute::<u64, Convertir>(virtual_de(this, 6))(this, st, a, b, ma, d, e, md) }
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn codecvt_out(this: *const Faceta, st: u64, a: u64, b: u64, ma: u64, d: u64, e: u64, md: u64) -> i32 {
    // SAFETY: un codecvt, por su vtabla.
    unsafe { core::mem::transmute::<u64, Convertir>(virtual_de(this, 7))(this, st, a, b, ma, d, e, md) }
}

/// `unshift` (hueco 8).
extern "win64" fn codecvt_unshift(this: *const Faceta, st: u64, d: u64, e: u64, md: u64) -> i32 {
    // SAFETY: un codecvt, por su vtabla.
    unsafe {
        let f: extern "win64" fn(*const Faceta, u64, u64, u64, u64) -> i32 = core::mem::transmute(virtual_de(this, 8));
        f(this, st, d, e, md)
    }
}

// -- codecvt<wchar_t, char> ------------------------------------------------------------------

/// `codecvt<wchar_t,char,mbstate_t>(size_t refs)`.
extern "win64" fn ancho_nuevo(this: *mut CodecvtAncho, refs: usize) -> *mut CodecvtAncho {
    // SAFETY: el objeto del `.exe` (64 bytes).
    unsafe { this.write(CodecvtAncho { f: Faceta { vt: vtabla(VT_CODECVT_ANCHO), refs: AtomicU32::new(refs as u32) }, cvt: CVT_C }) };
    this
}

extern "win64" fn ancho_fin(_this: u64) {}

fn mbcurmax(this: *const CodecvtAncho) -> u32 {
    // SAFETY: un codecvt<wchar_t,char>.
    unsafe { (*this).cvt.mbcurmax }
}

extern "win64" fn ancho_max_length(this: *const CodecvtAncho) -> i32 {
    mbcurmax(this) as i32
}

extern "win64" fn ancho_encoding(this: *const CodecvtAncho) -> i32 {
    (mbcurmax(this) == 1) as i32
}

/// Bytes a anchos: en "C", cada byte es su caracter.
#[allow(clippy::too_many_arguments)]
extern "win64" fn ancho_do_in(_this: u64, _st: u64, a: *const u8, b: *const u8, ma: *mut *const u8, d: *mut u16, e: *mut u16, md: *mut *mut u16) -> i32 {
    let (mut p, mut q) = (a, d);
    let r = loop {
        if p >= b {
            break OK;
        }
        if q >= e {
            break PARCIAL;
        }
        // SAFETY: los tramos del `.exe`.
        unsafe {
            *q = *p as u16;
            p = p.add(1);
            q = q.add(1);
        }
    };
    // SAFETY: los punteros de vuelta del `.exe`.
    unsafe {
        *ma = p;
        *md = q;
    }
    r
}

/// Anchos a bytes: lo que pasa de 255 no tiene byte en "C".
#[allow(clippy::too_many_arguments)]
extern "win64" fn ancho_do_out(_this: u64, _st: u64, a: *const u16, b: *const u16, ma: *mut *const u16, d: *mut u8, e: *mut u8, md: *mut *mut u8) -> i32 {
    let (mut p, mut q) = (a, d);
    let mut r = OK;
    while p < b && q < e {
        // SAFETY: los tramos del `.exe`.
        let c = unsafe { *p };
        if c > 255 {
            r = ERROR;
            break;
        }
        // SAFETY: lo mismo.
        unsafe {
            *q = c as u8;
            p = p.add(1);
            q = q.add(1);
        }
    }
    if r == OK && p != b {
        r = PARCIAL;
    }
    // SAFETY: los punteros de vuelta del `.exe`.
    unsafe {
        *ma = p;
        *md = q;
    }
    r
}

extern "win64" fn ancho_do_unshift(_this: u64, _st: u64, d: u64, _e: u64, md: *mut u64) -> i32 {
    // SAFETY: el puntero de vuelta del `.exe`.
    unsafe { *md = d };
    OK
}

extern "win64" fn ancho_do_length(this: u64, st: u64, a: u64, b: u64, n: usize) -> i32 {
    cvt_do_length(this, st, a, b, n)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    if let Some(i) = NOMBRES_ID.iter().position(|x| *x == n) {
        return Some(&IDS[i] as *const AtomicU64 as u64);
    }
    if n == ID_CNT_N {
        return Some(&ID_CNT as *const AtomicI32 as u64);
    }
    Some(match n {
        "??Bid@locale@std@@QEAA_KXZ" => dir!(id_numero),
        "??0facet@locale@std@@IEAA@_K@Z" => dir!(faceta_nueva),
        "??1facet@locale@std@@MEAA@XZ" => dir!(faceta_fin),
        "?_Incref@facet@locale@std@@UEAAXXZ" => dir!(incref),
        "?_Decref@facet@locale@std@@UEAAPEAV_Facet_base@3@XZ" => dir!(decref),
        "?_Init@locale@std@@CAPEAV_Locimp@12@_N@Z" => dir!(locale_init),
        "?_Getgloballocale@locale@std@@CAPEAV_Locimp@12@XZ" => dir!(get_global_locale),
        "?_New_Locimp@_Locimp@locale@std@@CAPEAV123@AEBV123@@Z" => dir!(new_locimp),
        "?_Addfac@_Locimp@locale@std@@AEAAXPEAVfacet@23@_K@Z" | "?_Locimp_Addfac@_Locimp@locale@std@@CAXPEAV123@PEAVfacet@23@_K@Z" => dir!(locimp_addfac),
        "??4?$_Yarn@D@std@@QEAAAEAV01@PEBD@Z" => dir!(yarn_asignar),
        "?c_str@?$_Yarn@D@std@@QEBAPEBDXZ" => dir!(yarn_c_str),
        "??0_Locinfo@std@@QEAA@PEBD@Z" => dir!(locinfo_nuevo),
        "??1_Locinfo@std@@QEAA@XZ" => dir!(locinfo_fin),
        "?_Getcoll@_Locinfo@std@@QEBA?AU_Collvec@@XZ" => dir!(locinfo_getcoll),
        "?_Getcvt@_Locinfo@std@@QEBA?AU_Cvtvec@@XZ" => dir!(locinfo_getcvt),
        "?_W_Getdays@_Locinfo@std@@QEBAPEBGXZ" => dir!(locinfo_w_getdays),
        "?_W_Getmonths@_Locinfo@std@@QEBAPEBGXZ" => dir!(locinfo_w_getmonths),
        "?_Getcat@?$ctype@D@std@@SA_KPEAPEBVfacet@locale@2@PEBV42@@Z" => dir!(ctype_getcat),
        "?tolower@?$ctype@D@std@@QEBADD@Z" => dir!(ctype_tolower),
        "?tolower@?$ctype@D@std@@QEBAPEBDPEADPEBD@Z" => dir!(ctype_tolower_tramo),
        "?_Getcat@?$codecvt@DDU_Mbstatet@@@std@@SA_KPEAPEBVfacet@locale@2@PEBV42@@Z" => dir!(codecvt_getcat),
        "?always_noconv@codecvt_base@std@@QEBA_NXZ" => dir!(always_noconv),
        "?in@?$codecvt@DDU_Mbstatet@@@std@@QEBAHAEAU_Mbstatet@@PEBD1AEAPEBDPEAD3AEAPEAD@Z" | "?in@?$codecvt@_WDU_Mbstatet@@@std@@QEBAHAEAU_Mbstatet@@PEBD1AEAPEBDPEA_W3AEAPEA_W@Z" => dir!(codecvt_in),
        "?out@?$codecvt@DDU_Mbstatet@@@std@@QEBAHAEAU_Mbstatet@@PEBD1AEAPEBDPEAD3AEAPEAD@Z" | "?out@?$codecvt@_WDU_Mbstatet@@@std@@QEBAHAEAU_Mbstatet@@PEB_W1AEAPEB_WPEAD3AEAPEAD@Z" => dir!(codecvt_out),
        "?unshift@?$codecvt@DDU_Mbstatet@@@std@@QEBAHAEAU_Mbstatet@@PEAD1AEAPEAD@Z" => dir!(codecvt_unshift),
        "??0?$codecvt@_WDU_Mbstatet@@@std@@QEAA@_K@Z" => dir!(ancho_nuevo),
        "??1?$codecvt@_WDU_Mbstatet@@@std@@MEAA@XZ" => dir!(ancho_fin),
        _ => return None,
    })
}
