//! **Los errores de la biblioteca de C++ de MSVC, de la casa** (tanda 13 de
//! Cyberpunk, 30-09): lo que `msvcp140.dll` LANZA, y `exception_ptr`.
//!
//! ```text
//!    lanzar      _Xlength_error _Xout_of_range _Xinvalid_argument _Xbad_alloc
//!                _Xbad_function_call _Xregex_error _Throw_Cpp_error
//!                _Throw_C_error _XGetLastError _Throw_future_error
//!    guardar     __ExceptionPtrCreate/Destroy/Copy/Assign/ToBool
//!                __ExceptionPtrCurrentException __ExceptionPtrRethrow
//!                _Rethrow_future_exception
//!    y lo demas  _Lockit uncaught_exception(s)
//! ```
//!
//! **Como se lanza desde la casa:** cada `_X...` es un trozo de asm que
//! SALTA (no llama) a `proton_x_cxx_throw` con el objeto y su ThrowInfo, asi
//! que la foto del throw es la del `.exe` que llamo, como si el throw fuera
//! suyo. Las tablas (TypeDescriptor, CatchableType, ThrowInfo) las hace la
//! casa una vez en [`TABLAS`], con los NOMBRES de MSVC (`.?AVlength_error@std@@`):
//! un `catch (std::logic_error&)` del juego casa por nombre, como en Windows.
//! El objeto es el de MSVC x64: vptr, el texto de what() y su bandera (24
//! bytes); `system_error` y `future_error` llevan detras su `error_code`
//! (valor y categoria: 40 bytes); `regex_error`, su codigo (32).
//!
//! **Las categorias:** `generic_category()` y `system_category()` son del
//! `.exe` (inline en sus cabeceras) y se comparan por su `_Addr` (3 y 7): las
//! de la casa llevan el mismo.
//!
//! Lo que no, dicho: los objetos lanzados no se liberan (son pocos bytes, y
//! solo cuando algo ya fue mal); `_Lockit` no bloquea (los hilos de la casa
//! son cooperativos: nadie entra mientras otro lo tiene).

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::{aviso, cxx, dir, kernel32, plataforma};

// -- Las clases ----------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Clase {
    LengthError,
    OutOfRange,
    InvalidArgument,
    BadAlloc,
    BadFunctionCall,
    RegexError,
    SystemError,
    FutureError,
}

const CLASES: [Clase; 8] = [Clase::LengthError, Clase::OutOfRange, Clase::InvalidArgument, Clase::BadAlloc, Clase::BadFunctionCall, Clase::RegexError, Clase::SystemError, Clase::FutureError];

impl Clase {
    /// Sus nombres decorados, de la mas derivada a std::exception.
    fn nombres(self) -> &'static [&'static str] {
        match self {
            Clase::LengthError => &[".?AVlength_error@std@@", ".?AVlogic_error@std@@", ".?AVexception@std@@"],
            Clase::OutOfRange => &[".?AVout_of_range@std@@", ".?AVlogic_error@std@@", ".?AVexception@std@@"],
            Clase::InvalidArgument => &[".?AVinvalid_argument@std@@", ".?AVlogic_error@std@@", ".?AVexception@std@@"],
            Clase::BadAlloc => &[".?AVbad_alloc@std@@", ".?AVexception@std@@"],
            Clase::BadFunctionCall => &[".?AVbad_function_call@std@@", ".?AVexception@std@@"],
            Clase::RegexError => &[".?AVregex_error@std@@", ".?AVruntime_error@std@@", ".?AVexception@std@@"],
            Clase::SystemError => &[".?AVsystem_error@std@@", ".?AV_System_error@std@@", ".?AVruntime_error@std@@", ".?AVexception@std@@"],
            Clase::FutureError => &[".?AVfuture_error@std@@", ".?AVlogic_error@std@@", ".?AVexception@std@@"],
        }
    }

    fn medida(self) -> usize {
        match self {
            Clase::RegexError => 32,
            Clase::SystemError | Clase::FutureError => 40,
            _ => 24,
        }
    }
}

// -- Las tablas ----------------------------------------------------------------------------------

const MEDIDA_TABLAS: usize = 4096;

#[repr(C, align(16))]
struct Tablas {
    bytes: UnsafeCell<[u8; MEDIDA_TABLAS]>,
    /// El ThrowInfo de cada clase (en el orden de CLASES) y el vptr de sus
    /// objetos; 0 hasta que se hacen.
    hechas: UnsafeCell<([u64; 8], [u64; 8])>,
    /// Las dos categorias: vptr y _Addr.
    generica: UnsafeCell<[u64; 2]>,
    sistema: UnsafeCell<[u64; 2]>,
}

// SAFETY: una tarea, hilos cooperativos; se escriben una vez, al principio.
unsafe impl Sync for Tablas {}

static TABLAS: Tablas = Tablas { bytes: UnsafeCell::new([0; MEDIDA_TABLAS]), hechas: UnsafeCell::new(([0; 8], [0; 8])), generica: UnsafeCell::new([0; 2]), sistema: UnsafeCell::new([0; 2]) };

/// Quien escribe las tablas: el sitio y lo que va.
struct Escritor {
    base: u64,
    i: usize,
    /// Donde empiezan las tablas (desde `base`).
    desde: usize,
}

impl Escritor {
    fn alinear(&mut self, a: usize) {
        self.i = self.i.div_ceil(a) * a;
    }
    fn aqui(&self) -> u64 {
        self.base + self.i as u64
    }
    fn bytes(&mut self, b: &[u8]) {
        assert!(self.i + b.len() <= self.desde + MEDIDA_TABLAS, "PROTON-X: las tablas de msvcp_errores no caben");
        // SAFETY: dentro de TABLAS (lo acaba de mirar el assert).
        unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), (self.base as *mut u8).add(self.i), b.len()) };
        self.i += b.len();
    }
    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    /// La RVA de `d` sobre la base (codigo incluido: la casa es una imagen,
    /// y su codigo y sus datos caben en 4 GiB).
    fn rva(&self, d: u64) -> u32 {
        assert!(d >= self.base && d - self.base <= u32::MAX as u64, "PROTON-X: una RVA de la casa no cabe en 32 bits");
        (d - self.base) as u32
    }
}

/// Las tablas, hechas la primera vez: (ThrowInfo, vptr) de cada clase.
fn tablas() -> (&'static [u64; 8], &'static [u64; 8]) {
    // SAFETY: ver `Tablas`.
    let hechas = unsafe { &mut *TABLAS.hechas.get() };
    if hechas.0[0] == 0 {
        hacer(hechas);
    }
    (&hechas.0, &hechas.1)
}

fn hacer(hechas: &mut ([u64; 8], [u64; 8])) {
    let tablas = TABLAS.bytes.get() as u64;
    // La base de las RVAs: por debajo de las tablas y del codigo que
    // nombran (las RVAs de un ThrowInfo no tienen signo).
    let base = [tablas, dir!(obj_deshacer), dir!(copiar24), dir!(copiar32), dir!(copiar40)].into_iter().min().unwrap_or(tablas) & !0xFFFF;
    let mut w = Escritor { base, i: (tablas - base) as usize, desde: (tablas - base) as usize };
    // Las categorias: vptr (tras el hueco del COL) y _Addr.
    for (cat, addr) in [(TABLAS.generica.get(), 3u64), (TABLAS.sistema.get(), 7)] {
        w.alinear(8);
        w.u64(0);
        let vt = w.aqui();
        for f in [dir!(cat_destruir), dir!(cat_nombre), dir!(cat_mensaje), dir!(cat_condicion), dir!(cat_equivalente), dir!(cat_equivalente_codigo)] {
            w.u64(f);
        }
        // SAFETY: ver `Tablas`.
        unsafe { *cat = [vt, addr] };
    }
    // Un TypeDescriptor por nombre (vftable 0, spare 0, el nombre).
    let mut descriptores: Vec<(&str, u64)> = Vec::new();
    for c in CLASES {
        for n in c.nombres() {
            if !descriptores.iter().any(|d| d.0 == *n) {
                w.alinear(8);
                let d = w.aqui();
                w.u64(0);
                w.u64(0);
                w.bytes(n.as_bytes());
                w.bytes(&[0]);
                descriptores.push((n, d));
            }
        }
    }
    for (k, c) in CLASES.iter().enumerate() {
        // El vtable: [COL (no hay), destructor, what].
        w.alinear(8);
        w.u64(0);
        hechas.1[k] = w.aqui();
        w.u64(dir!(obj_destruir));
        w.u64(dir!(obj_what));
        let copiar = match c.medida() {
            24 => dir!(copiar24),
            32 => dir!(copiar32),
            _ => dir!(copiar40),
        };
        // Los CatchableType: properties 0, el tipo, PMD {0, -1, 0}, la
        // medida y el constructor de copia.
        let mut cts = Vec::new();
        for n in c.nombres() {
            w.alinear(4);
            cts.push(w.aqui());
            let d = descriptores.iter().find(|d| d.0 == *n).map_or(0, |d| d.1);
            for v in [0, w.rva(d), 0, u32::MAX, 0, c.medida() as u32, w.rva(copiar)] {
                w.u32(v);
            }
        }
        w.alinear(4);
        let arreglo = w.aqui();
        w.u32(cts.len() as u32);
        for ct in cts {
            let r = w.rva(ct);
            w.u32(r);
        }
        w.alinear(8);
        hechas.0[k] = w.aqui();
        let (u, a) = (w.rva(dir!(obj_deshacer)), w.rva(arreglo));
        for v in [0, u, 0, a] {
            w.u32(v);
        }
    }
    cxx::tablas_de_la_casa(base, tablas, tablas + MEDIDA_TABLAS as u64);
}

// -- Los objetos ---------------------------------------------------------------------------------

extern "win64" fn obj_destruir(this: u64, _banderas: u32) -> u64 {
    this
}

extern "win64" fn obj_deshacer(_this: u64) {}

extern "win64" fn obj_what(this: *const u64) -> *const u8 {
    // SAFETY: un objeto de estos: el texto en +8.
    let t = unsafe { *this.add(1) };
    if t == 0 {
        b"Unknown exception\0".as_ptr()
    } else {
        t as *const u8
    }
}

fn copiar(d: *mut u8, s: *const u8, n: usize) -> *mut u8 {
    // SAFETY: dos objetos de `n` bytes (el del catch y el lanzado).
    unsafe { core::ptr::copy_nonoverlapping(s, d, n) };
    d
}

extern "win64" fn copiar24(d: *mut u8, s: *const u8) -> *mut u8 {
    copiar(d, s, 24)
}

extern "win64" fn copiar32(d: *mut u8, s: *const u8) -> *mut u8 {
    copiar(d, s, 32)
}

extern "win64" fn copiar40(d: *mut u8, s: *const u8) -> *mut u8 {
    copiar(d, s, 40)
}

/// Unos bytes que viven para siempre (con su 0 si es texto).
fn para_siempre(b: &[u8]) -> u64 {
    let mut v: Vec<u64> = alloc::vec![0; b.len() / 8 + 1];
    // SAFETY: `v` tiene sitio para `b` y un 0.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), v.as_mut_ptr() as *mut u8, b.len()) };
    v.leak().as_ptr() as u64
}

/// Un objeto de `c` con su texto (y su codigo, si lo lleva).
fn objeto(c: Clase, texto: &[u8], codigo: Option<(i32, u64)>) -> u64 {
    let k = CLASES.iter().position(|&x| x == c).unwrap_or(0);
    let (_, vptrs) = tablas();
    let mut o = alloc::vec![vptrs[k], para_siempre(texto), 1, 0, 0];
    if let Some((v, cat)) = codigo {
        o[3] = v as u32 as u64;
        o[4] = cat;
    }
    o.leak().as_ptr() as u64
}

fn cadena(p: u64) -> Vec<u8> {
    if p == 0 {
        return Vec::new();
    }
    crate::crt::cadena_c(p)
}

fn texto_de_sistema(e: i32) -> Vec<u8> {
    cadena(crate::msvcp_hilos::syserror_map(e) as u64)
}

fn categoria(sistema: bool) -> u64 {
    tablas();
    (if sistema { TABLAS.sistema.get() } else { TABLAS.generica.get() }) as u64
}

// -- Lanzar --------------------------------------------------------------------------------------

const K_LENGTH: u32 = 0;
const K_OUT_OF_RANGE: u32 = 1;
const K_INVALID: u32 = 2;
const K_BAD_ALLOC: u32 = 3;
const K_BAD_FUNCTION: u32 = 4;
const K_REGEX: u32 = 5;
const K_CPP: u32 = 6;
const K_C: u32 = 7;
const K_LAST_ERROR: u32 = 8;
const K_FUTURE: u32 = 9;
const K_RELANZAR: u32 = 10;

/// **Lo que se lanza**: el objeto (y en `*info`, su ThrowInfo), segun lo
/// que se pidio (`k`) y su argumento.
extern "win64" fn construir(k: u32, arg: u64, info: *mut u64) -> u64 {
    let (infos, _) = tablas();
    let (c, o) = match k {
        K_LENGTH | K_OUT_OF_RANGE | K_INVALID => {
            let c = [Clase::LengthError, Clase::OutOfRange, Clase::InvalidArgument][k as usize];
            (c, objeto(c, &cadena(arg), None))
        }
        K_BAD_ALLOC => (Clase::BadAlloc, objeto(Clase::BadAlloc, b"bad allocation", None)),
        K_BAD_FUNCTION => (Clase::BadFunctionCall, objeto(Clase::BadFunctionCall, b"bad function call", None)),
        K_REGEX => {
            let mut o = alloc::vec![0u64; 4];
            o[0] = tablas().1[5];
            o[1] = para_siempre(b"regex_error");
            o[2] = 1;
            o[3] = arg as u32 as u64;
            (Clase::RegexError, o.leak().as_ptr() as u64)
        }
        K_CPP | K_C => {
            // _Throw_Cpp_error: el _Cpp_error; _Throw_C_error: el _Thrd_result.
            let errc = if k == K_CPP {
                [16, 22, 3, 12, 1, 36, 11].get(arg as usize).copied().unwrap_or(22)
            } else {
                match arg {
                    1 | 2 => 11,
                    3 => 16,
                    _ => 22,
                }
            };
            // msvcp140 lanza system_error(errc, generic_category(), _Msgs[..])
            // y su what() es "<_Msgs>: <message()>": los dos textos son el
            // mismo, asi que sale dos veces. Lo dijo tanda13.exe en Windows:
            // "operation not permitted: operation not permitted".
            let t = texto_de_sistema(errc);
            let mut w = t.clone();
            w.extend_from_slice(b": ");
            w.extend_from_slice(&t);
            (Clase::SystemError, objeto(Clase::SystemError, &w, Some((errc, categoria(false)))))
        }
        K_LAST_ERROR => {
            let e = kernel32::ultimo_error() as i32;
            (Clase::SystemError, objeto(Clase::SystemError, alloc::format!("Windows error {e}").as_bytes(), Some((e, categoria(true)))))
        }
        K_FUTURE => {
            // SAFETY: el error_code del `.exe`: valor (+0) y categoria (+8).
            let (v, cat) = unsafe { (*(arg as *const i32), *((arg + 8) as *const u64)) };
            let t: &[u8] = match v {
                1 => b"broken promise",
                2 => b"future already retrieved",
                3 => b"promise already satisfied",
                _ => b"no state",
            };
            (Clase::FutureError, objeto(Clase::FutureError, t, Some((v, cat))))
        }
        K_RELANZAR => {
            // Una copia nueva de lo guardado en el exception_ptr.
            // SAFETY: el exception_ptr del `.exe` (su primer puntero).
            let caja = unsafe { *(arg as *const u64) } as *const Caja;
            if caja.is_null() {
                aviso("rethrow_exception de un exception_ptr vacio: std::terminate");
                (plataforma().salir)(3)
            }
            // SAFETY: una Caja de las de aqui.
            let c = unsafe { &*caja };
            // SAFETY: el `*info` del trozo de asm.
            unsafe { *info = c.info };
            return copiar_objeto(c.objeto, c.info, c.base);
        }
        _ => {
            aviso("msvcp_errores: un lanzamiento que no existe");
            (plataforma().salir)(3)
        }
    };
    let k = CLASES.iter().position(|&x| x == c).unwrap_or(0);
    // SAFETY: el `*info` del trozo de asm.
    unsafe { *info = infos[k] };
    o
}

core::arch::global_asm!(
    // Cada una pone su numero (ecx) y su argumento (rdx) y salta.
    ".globl proton_x_xlength_error", "proton_x_xlength_error:", "mov rdx, rcx", "mov ecx, 0", "jmp proton_x_lanzar_std",
    ".globl proton_x_xout_of_range", "proton_x_xout_of_range:", "mov rdx, rcx", "mov ecx, 1", "jmp proton_x_lanzar_std",
    ".globl proton_x_xinvalid_argument", "proton_x_xinvalid_argument:", "mov rdx, rcx", "mov ecx, 2", "jmp proton_x_lanzar_std",
    ".globl proton_x_xbad_alloc", "proton_x_xbad_alloc:", "mov ecx, 3", "jmp proton_x_lanzar_std",
    ".globl proton_x_xbad_function_call", "proton_x_xbad_function_call:", "mov ecx, 4", "jmp proton_x_lanzar_std",
    ".globl proton_x_xregex_error", "proton_x_xregex_error:", "mov rdx, rcx", "mov ecx, 5", "jmp proton_x_lanzar_std",
    ".globl proton_x_throw_cpp_error", "proton_x_throw_cpp_error:", "mov rdx, rcx", "mov ecx, 6", "jmp proton_x_lanzar_std",
    ".globl proton_x_throw_c_error", "proton_x_throw_c_error:", "mov rdx, rcx", "mov ecx, 7", "jmp proton_x_lanzar_std",
    ".globl proton_x_xget_last_error", "proton_x_xget_last_error:", "mov ecx, 8", "jmp proton_x_lanzar_std",
    ".globl proton_x_throw_future_error", "proton_x_throw_future_error:", "mov rdx, rcx", "mov ecx, 9", "jmp proton_x_lanzar_std",
    ".globl proton_x_exception_ptr_rethrow", "proton_x_exception_ptr_rethrow:", "mov rdx, rcx", "mov ecx, 10", "jmp proton_x_lanzar_std",
    // construir(k, arg, &info) y SALTAR a _CxxThrowException(objeto, info):
    // la pila queda como la dejo el `.exe` al llamar.
    "proton_x_lanzar_std:",
    "sub rsp, 0x38",
    "lea r8, [rsp + 0x28]",
    "call {construir}",
    "mov rcx, rax",
    "mov rdx, [rsp + 0x28]",
    "add rsp, 0x38",
    "jmp {tirar}",
    construir = sym construir,
    tirar = sym cxx::proton_x_cxx_throw,
);

extern "C" {
    fn proton_x_xlength_error();
    fn proton_x_xout_of_range();
    fn proton_x_xinvalid_argument();
    fn proton_x_xbad_alloc();
    fn proton_x_xbad_function_call();
    fn proton_x_xregex_error();
    fn proton_x_throw_cpp_error();
    fn proton_x_throw_c_error();
    fn proton_x_xget_last_error();
    fn proton_x_throw_future_error();
    fn proton_x_exception_ptr_rethrow();
}

// -- Las categorias ------------------------------------------------------------------------------

fn es_sistema(this: u64) -> bool {
    this == TABLAS.sistema.get() as u64
}

extern "win64" fn cat_destruir(this: u64, _b: u32) -> u64 {
    this
}

extern "win64" fn cat_nombre(this: u64) -> *const u8 {
    if es_sistema(this) {
        b"system\0".as_ptr()
    } else {
        b"generic\0".as_ptr()
    }
}

/// `message(int)`: un std::string de MSVC x64 (32 bytes: el texto dentro si
/// cabe en 16 con su 0, o su puntero; el largo +16; la capacidad +24).
extern "win64" fn cat_mensaje(this: u64, ret: *mut u64, v: i32) -> *mut u64 {
    let t = if es_sistema(this) { alloc::format!("Windows error {v}").into_bytes() } else { texto_de_sistema(v) };
    // SAFETY: el std::string (sin hacer) del `.exe`.
    unsafe {
        core::ptr::write_bytes(ret as *mut u8, 0, 32);
        if t.len() < 16 {
            core::ptr::copy_nonoverlapping(t.as_ptr(), ret as *mut u8, t.len());
            *ret.add(3) = 15;
        } else {
            *ret = para_siempre(&t);
            *ret.add(3) = t.len() as u64;
        }
        *ret.add(2) = t.len() as u64;
    }
    ret
}

/// `default_error_condition(int)`: la misma, en esta categoria.
extern "win64" fn cat_condicion(this: u64, ret: *mut u64, v: i32) -> *mut u64 {
    // SAFETY: el error_condition del `.exe` (valor, categoria).
    unsafe {
        *ret = v as u32 as u64;
        *ret.add(1) = this;
    }
    ret
}

extern "win64" fn cat_equivalente(this: u64, v: i32, cond: *const u64) -> bool {
    // SAFETY: un error_condition del `.exe`.
    unsafe { *(cond as *const i32) == v && *cond.add(1) == this }
}

extern "win64" fn cat_equivalente_codigo(this: u64, ec: *const u64, v: i32) -> bool {
    // SAFETY: un error_code del `.exe`.
    unsafe { *(ec as *const i32) == v && *ec.add(1) == this }
}

// -- exception_ptr -------------------------------------------------------------------------------

/// Lo que guarda un exception_ptr: una copia del objeto, su ThrowInfo y la
/// base de sus RVAs. El exception_ptr de MSVC son dos punteros: los dos
/// apuntan aqui.
struct Caja {
    cuenta: u32,
    objeto: u64,
    info: u64,
    base: u64,
}

fn leer32(d: u64) -> u32 {
    // SAFETY: las tablas de C++ de una imagen cargada (o de la casa).
    unsafe { (d as *const u32).read_unaligned() }
}

type Copia = extern "win64" fn(u64, u64) -> u64;
type CopiaVirtual = extern "win64" fn(u64, u64, i32) -> u64;
type Deshacer = extern "win64" fn(u64);

/// Una copia de `objeto` con el constructor de copia de su tipo mas
/// derivado (el primero de su CatchableTypeArray).
fn copiar_objeto(objeto: u64, info: u64, base: u64) -> u64 {
    let arreglo = base + leer32(info + 12) as u64;
    let ct = base + leer32(arreglo + 4) as u64;
    let (propiedades, medida, copia) = (leer32(ct), leer32(ct + 20) as usize, leer32(ct + 24));
    let nuevo = alloc::vec![0u64; medida.div_ceil(8).max(1)].leak().as_mut_ptr() as u64;
    if copia == 0 {
        copiar(nuevo as *mut u8, objeto as *const u8, medida);
    } else if propiedades & 4 != 0 {
        // SAFETY: el constructor de copia (con base virtual) del `.exe`.
        let f: CopiaVirtual = unsafe { core::mem::transmute::<u64, CopiaVirtual>(base + copia as u64) };
        f(nuevo, objeto, 1);
    } else {
        // SAFETY: el constructor de copia del `.exe`.
        let f: Copia = unsafe { core::mem::transmute::<u64, Copia>(base + copia as u64) };
        f(nuevo, objeto);
    }
    nuevo
}

fn poner(p: *mut u64, caja: u64) {
    // SAFETY: el exception_ptr del `.exe` (16 bytes).
    unsafe {
        *p = caja;
        *p.add(1) = caja;
    }
}

fn caja_de(p: *const u64) -> *mut Caja {
    // SAFETY: el exception_ptr del `.exe`.
    if p.is_null() { core::ptr::null_mut() } else { unsafe { *p as *mut Caja } }
}

extern "win64" fn ep_create(p: *mut u64) {
    poner(p, 0);
}

extern "win64" fn ep_destroy(p: *mut u64) {
    let c = caja_de(p);
    if c.is_null() {
        return;
    }
    // SAFETY: una Caja de las de aqui.
    let caja = unsafe { &mut *c };
    caja.cuenta -= 1;
    if caja.cuenta == 0 {
        let u = leer32(caja.info + 4);
        if u != 0 {
            // SAFETY: el destructor del tipo (pmfnUnwind).
            let f: Deshacer = unsafe { core::mem::transmute::<u64, Deshacer>(caja.base + u as u64) };
            f(caja.objeto);
        }
        // SAFETY: la Caja se hizo con Box::leak.
        drop(unsafe { alloc::boxed::Box::from_raw(c) });
    }
    poner(p, 0);
}

extern "win64" fn ep_copy(d: *mut u64, s: *const u64) {
    let c = caja_de(s);
    if !c.is_null() {
        // SAFETY: una Caja de las de aqui.
        unsafe { (*c).cuenta += 1 };
    }
    poner(d, c as u64);
}

extern "win64" fn ep_assign(d: *mut u64, s: *const u64) {
    if caja_de(d) == caja_de(s) {
        return;
    }
    ep_destroy(d);
    ep_copy(d, s);
}

extern "win64" fn ep_to_bool(p: *const u64) -> bool {
    !caja_de(p).is_null()
}

/// `current_exception()`: una copia de la de C++ que se esta cogiendo.
extern "win64" fn ep_current_exception(p: *mut u64) {
    // SAFETY: el EXCEPTION_RECORD* que guarda cxx (o 0).
    let rec = unsafe { *(cxx::current_exception() as *const u64) };
    // SAFETY: un EXCEPTION_RECORD: codigo +0, parametros +32.
    let es_cxx = rec != 0 && unsafe { *(rec as *const u32) } == cxx::EXCEPCION_CXX;
    if !es_cxx {
        poner(p, 0);
        return;
    }
    // SAFETY: lo mismo: [magia, objeto, ThrowInfo, base].
    let (objeto, info, base) = unsafe { (*((rec + 40) as *const u64), *((rec + 48) as *const u64), *((rec + 56) as *const u64)) };
    let copia = copiar_objeto(objeto, info, base);
    let caja = alloc::boxed::Box::leak(alloc::boxed::Box::new(Caja { cuenta: 1, objeto: copia, info, base }));
    poner(p, caja as *mut Caja as u64);
}

// -- Lo demas ------------------------------------------------------------------------------------

/// `_Lockit(int)`: guarda su tipo (`_Locktype`, lo unico que tiene).
extern "win64" fn lockit(this: *mut i32, tipo: i32) -> *mut i32 {
    // SAFETY: el _Lockit del `.exe`.
    unsafe { *this = tipo };
    this
}

extern "win64" fn lockit_fin(_this: u64) {}

extern "win64" fn uncaught_exception() -> bool {
    cxx::uncaught_exceptions() > 0
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "?_Xlength_error@std@@YAXPEBD@Z" => dir!(proton_x_xlength_error),
        "?_Xout_of_range@std@@YAXPEBD@Z" => dir!(proton_x_xout_of_range),
        "?_Xinvalid_argument@std@@YAXPEBD@Z" => dir!(proton_x_xinvalid_argument),
        "?_Xbad_alloc@std@@YAXXZ" => dir!(proton_x_xbad_alloc),
        "?_Xbad_function_call@std@@YAXXZ" => dir!(proton_x_xbad_function_call),
        "?_Xregex_error@std@@YAXW4error_type@regex_constants@1@@Z" => dir!(proton_x_xregex_error),
        "?_Throw_Cpp_error@std@@YAXH@Z" => dir!(proton_x_throw_cpp_error),
        "?_Throw_C_error@std@@YAXH@Z" => dir!(proton_x_throw_c_error),
        "?_XGetLastError@std@@YAXXZ" => dir!(proton_x_xget_last_error),
        "?_Throw_future_error@std@@YAXAEBVerror_code@1@@Z" => dir!(proton_x_throw_future_error),
        "?__ExceptionPtrRethrow@@YAXPEBX@Z" | "?_Rethrow_future_exception@std@@YAXVexception_ptr@1@@Z" => dir!(proton_x_exception_ptr_rethrow),
        "?__ExceptionPtrCreate@@YAXPEAX@Z" => dir!(ep_create),
        "?__ExceptionPtrDestroy@@YAXPEAX@Z" => dir!(ep_destroy),
        "?__ExceptionPtrCopy@@YAXPEAXPEBX@Z" => dir!(ep_copy),
        "?__ExceptionPtrAssign@@YAXPEAXPEBX@Z" => dir!(ep_assign),
        "?__ExceptionPtrToBool@@YA_NPEBX@Z" => dir!(ep_to_bool),
        "?__ExceptionPtrCurrentException@@YAXPEAX@Z" => dir!(ep_current_exception),
        "??0_Lockit@std@@QEAA@H@Z" => dir!(lockit),
        "??1_Lockit@std@@QEAA@XZ" => dir!(lockit_fin),
        "?uncaught_exception@std@@YA_NXZ" => dir!(uncaught_exception),
        "?uncaught_exceptions@std@@YAHXZ" => dir!(cxx::uncaught_exceptions),
        _ => return None,
    })
}
