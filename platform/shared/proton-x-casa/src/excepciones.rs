//! **Las excepciones estructuradas (SEH) de la casa** (P4c, 28-09).
//!
//! ```text
//!    RaiseException              la foto de quien llama y el DESPACHO: los
//!                                vectorizados, y luego marco a marco (primera
//!                                pasada) el manejador de cada uno
//!    __C_specific_handler        el de `__try` de C (ntdll y vcruntime140):
//!                                filtros, y al desenrollar los `__finally`
//!    RtlUnwindEx                 la segunda pasada hasta el marco destino, y
//!                                se sigue ALLI con sus registros
//!    RtlCaptureContext, RtlLookupFunctionEntry, RtlVirtualUnwind
//!    SetUnhandledExceptionFilter, Add/RemoveVectoredExceptionHandler
//! ```
//!
//! Lo que se decide (subir un marco, la tabla de ambitos, las dos pasadas) es
//! `bmo_proton_x::seh`, puro y probado en su banco; aqui esta lo que no se
//! puede decir sin la maquina: la foto de los registros y el salto a otro
//! contexto, en ensamblador, y las llamadas al `.exe`.
//!
//! **Por que la foto es de ensamblador:** la casa es soft-float en Ring 3 y
//! Rust no deja nombrar rbx ni rbp; el CONTEXT de Windows los pide TODOS
//! (enteros, xmm0..xmm15, el MXCSR), tal como los tenia quien llamo.
//!
//! **Lo que NO hace, dicho:**
//! - solo excepciones de SOFTWARE (RaiseException, RtlUnwindEx): un fallo de
//!   pagina, una division por cero o un `int3` del `.exe` no llegan a Ring 3
//!   (seria cosa del kernel);
//! - la pila que se recorre es la del `.exe` y sus DLL: un marco de la casa
//!   la corta. Una excepcion dentro de una WndProc (llamada desde
//!   DispatchMessageW) o de un filtro no cruza hacia arriba: queda sin manejar;
//! - ni excepciones anidadas ni desenrollados que chocan (Nested/Collided): se
//!   dicen y se sigue buscando;
//! - RtlUnwindEx sin marco destino (el desenrollado de salida) se dice y acaba;
//! - STATUS_UNWIND_CONSOLIDATE (tanda 28): al llegar al marco destino se
//!   llama a `ExceptionInformation[0](registro)` y se sigue donde devuelva
//!   (el catch del CRT de MSVC enlazado dentro de un modulo);
//! - las de C++ van aparte (`cxx.rs`), sobre esto: su `catch` se corre al
//!   llegar al marco destino (`desenrollar_y`, el "consolidate" de Windows);
//! - ExceptionAddress es la vuelta de RaiseException (Windows da una
//!   direccion dentro de ella); de 15 parametros en adelante no caben y se
//!   dejan fuera.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::desenrollar::{self, Contexto, Funcion, Memoria, CONTEXT_BYTES, RAX, RSP, UNW_FLAG_EHANDLER, UNW_FLAG_UHANDLER};
use bmo_proton_x::seh::{self, AlDesenrollar, Despacho, Imagen, Registro, Subida, Vectores, DESPACHO_BYTES, DESPACHO_INDICE, REGISTRO_BANDERAS, REGISTRO_BYTES};
use bmo_proton_x::teb;

use crate::{aviso, dir, kernel32, modulos, plataforma};

/// Lo que devuelve un filtro para seguir donde se paro.
const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;
/// EXCEPTION_DISPOSITION de un manejador de marco.
const DISPOSICION_SEGUIR: u32 = 0;
const DISPOSICION_BUSCAR: u32 = 1;

/// Los codigos con que acaba el proceso cuando la excepcion no se puede
/// seguir ni desenrollar.
const STATUS_NONCONTINUABLE_EXCEPTION: u32 = 0xC000_0025;
const STATUS_UNWIND: u32 = 0xC000_0027;
const STATUS_BAD_STACK: u32 = 0xC000_0028;
const STATUS_INVALID_UNWIND_TARGET: u32 = 0xC000_0029;
/// El desenrollado que CONSOLIDA (el `catch` de C++ del CRT de MSVC): al
/// llegar al marco destino se llama a `ExceptionInformation[0](registro)` y
/// se sigue donde devuelva.
const STATUS_UNWIND_CONSOLIDATE: u32 = 0x8000_0029;

// -- La foto y el salto, en ensamblador ---------------------------------------------

core::arch::global_asm!(
    // RtlCaptureContext(CONTEXT* rcx): quien llama, tal cual; no toca nada.
    ".globl proton_x_rtl_capture_context",
    "proton_x_rtl_capture_context:",
    "pushfq",
    "mov [rcx + 0x78], rax",
    "mov [rcx + 0x80], rcx",
    "mov [rcx + 0x88], rdx",
    "mov [rcx + 0x90], rbx",
    "lea rax, [rsp + 16]",
    "mov [rcx + 0x98], rax",
    "mov [rcx + 0xA0], rbp",
    "mov [rcx + 0xA8], rsi",
    "mov [rcx + 0xB0], rdi",
    "mov [rcx + 0xB8], r8",
    "mov [rcx + 0xC0], r9",
    "mov [rcx + 0xC8], r10",
    "mov [rcx + 0xD0], r11",
    "mov [rcx + 0xD8], r12",
    "mov [rcx + 0xE0], r13",
    "mov [rcx + 0xE8], r14",
    "mov [rcx + 0xF0], r15",
    "mov rax, [rsp + 8]",
    "mov [rcx + 0xF8], rax",
    "pop rax",
    "mov [rcx + 0x44], eax",
    "mov word ptr [rcx + 0x38], cs",
    "mov word ptr [rcx + 0x3A], ds",
    "mov word ptr [rcx + 0x3C], es",
    "mov word ptr [rcx + 0x3E], fs",
    "mov word ptr [rcx + 0x40], gs",
    "mov word ptr [rcx + 0x42], ss",
    "fnstcw [rcx + 0x100]",
    "stmxcsr [rcx + 0x34]",
    "stmxcsr [rcx + 0x118]",
    "movups [rcx + 0x1A0], xmm0",
    "movups [rcx + 0x1B0], xmm1",
    "movups [rcx + 0x1C0], xmm2",
    "movups [rcx + 0x1D0], xmm3",
    "movups [rcx + 0x1E0], xmm4",
    "movups [rcx + 0x1F0], xmm5",
    "movups [rcx + 0x200], xmm6",
    "movups [rcx + 0x210], xmm7",
    "movups [rcx + 0x220], xmm8",
    "movups [rcx + 0x230], xmm9",
    "movups [rcx + 0x240], xmm10",
    "movups [rcx + 0x250], xmm11",
    "movups [rcx + 0x260], xmm12",
    "movups [rcx + 0x270], xmm13",
    "movups [rcx + 0x280], xmm14",
    "movups [rcx + 0x290], xmm15",
    // CONTEXT_AMD64 | CONTROL | INTEGER | SEGMENTS | FLOATING_POINT.
    "mov dword ptr [rcx + 0x30], 0x10001F",
    "mov rax, [rcx + 0x78]",
    "ret",
    //
    // Seguir en un CONTEXT (rcx): todo lo suyo, y su rsp y su rip. La vuelta
    // se deja justo debajo de SU rsp (lo de debajo ya no es de nadie: en
    // Windows x64 no hay zona roja) y `ret` la toma.
    ".globl proton_x_restaurar",
    "proton_x_restaurar:",
    "ldmxcsr [rcx + 0x34]",
    "movups xmm0, [rcx + 0x1A0]",
    "movups xmm1, [rcx + 0x1B0]",
    "movups xmm2, [rcx + 0x1C0]",
    "movups xmm3, [rcx + 0x1D0]",
    "movups xmm4, [rcx + 0x1E0]",
    "movups xmm5, [rcx + 0x1F0]",
    "movups xmm6, [rcx + 0x200]",
    "movups xmm7, [rcx + 0x210]",
    "movups xmm8, [rcx + 0x220]",
    "movups xmm9, [rcx + 0x230]",
    "movups xmm10, [rcx + 0x240]",
    "movups xmm11, [rcx + 0x250]",
    "movups xmm12, [rcx + 0x260]",
    "movups xmm13, [rcx + 0x270]",
    "movups xmm14, [rcx + 0x280]",
    "movups xmm15, [rcx + 0x290]",
    "mov rax, [rcx + 0x78]",
    "mov rdx, [rcx + 0x88]",
    "mov rbx, [rcx + 0x90]",
    "mov rbp, [rcx + 0xA0]",
    "mov rsi, [rcx + 0xA8]",
    "mov rdi, [rcx + 0xB0]",
    "mov r8, [rcx + 0xB8]",
    "mov r9, [rcx + 0xC0]",
    "mov r10, [rcx + 0xC8]",
    "mov r11, [rcx + 0xD0]",
    "mov r12, [rcx + 0xD8]",
    "mov r13, [rcx + 0xE0]",
    "mov r14, [rcx + 0xE8]",
    "mov r15, [rcx + 0xF0]",
    "mov rsp, [rcx + 0x98]",
    "push qword ptr [rcx + 0xF8]",
    "mov rcx, [rcx + 0x80]",
    "ret",
    //
    // RaiseException(codigo, banderas, n, args): la foto de QUIEN LLAMA (su
    // rsp tras volver, su rip de vuelta, sus cuatro argumentos) en un CONTEXT
    // de esta pila, y al despachador, que no vuelve. Los argumentos se guardan
    // antes en su sitio de sombra, que es de quien es llamado.
    ".globl proton_x_raise_exception",
    "proton_x_raise_exception:",
    "mov [rsp + 8], rcx",
    "mov [rsp + 16], rdx",
    "mov [rsp + 24], r8",
    "mov [rsp + 32], r9",
    // 0x20 de sombra, el quinto argumento en 0x20 y el CONTEXT en 0x30
    // (alineado a 16: la entrada deja rsp en 8 modulo 16).
    "sub rsp, 0x508",
    "lea rcx, [rsp + 0x30]",
    "call proton_x_rtl_capture_context",
    "lea rax, [rsp + 0x510]",
    "mov [rsp + 0x30 + 0x98], rax",
    "mov rax, [rsp + 0x508]",
    "mov [rsp + 0x30 + 0xF8], rax",
    "mov rcx, [rsp + 0x510]",
    "mov [rsp + 0x30 + 0x80], rcx",
    "mov rdx, [rsp + 0x518]",
    "mov [rsp + 0x30 + 0x88], rdx",
    "mov r8, [rsp + 0x520]",
    "mov [rsp + 0x30 + 0xB8], r8",
    "mov r9, [rsp + 0x528]",
    "mov [rsp + 0x30 + 0xC0], r9",
    "lea rax, [rsp + 0x30]",
    "mov [rsp + 0x20], rax",
    "call {despachar}",
    "ud2",
    //
    // RtlUnwindEx(marco, destino, registro, valor, contexto, historia): la
    // misma foto; el contexto y la historia de quien llama no se usan.
    ".globl proton_x_rtl_unwind_ex",
    "proton_x_rtl_unwind_ex:",
    "mov [rsp + 8], rcx",
    "mov [rsp + 16], rdx",
    "mov [rsp + 24], r8",
    "mov [rsp + 32], r9",
    "sub rsp, 0x508",
    "lea rcx, [rsp + 0x30]",
    "call proton_x_rtl_capture_context",
    "lea rax, [rsp + 0x510]",
    "mov [rsp + 0x30 + 0x98], rax",
    "mov rax, [rsp + 0x508]",
    "mov [rsp + 0x30 + 0xF8], rax",
    "mov rcx, [rsp + 0x510]",
    "mov rdx, [rsp + 0x518]",
    "mov r8, [rsp + 0x520]",
    "mov r9, [rsp + 0x528]",
    "lea rax, [rsp + 0x30]",
    "mov [rsp + 0x20], rax",
    "call {desenrollar_ex}",
    "ud2",
    despachar = sym despachar,
    desenrollar_ex = sym rtl_unwind_ex,
);

extern "C" {
    fn proton_x_rtl_capture_context();
    fn proton_x_raise_exception();
    fn proton_x_rtl_unwind_ex();
}

extern "win64" {
    fn proton_x_restaurar(ctx: *const u8) -> !;
}

// -- El estado: el filtro, los vectorizados y los despachos en curso ----------------

struct Estado {
    /// SetUnhandledExceptionFilter, o 0.
    filtro: u64,
    vectores: Vectores,
    /// Los despachos en marcha: (TEB del hilo, su CONTEXT). RtlUnwindEx sigue
    /// por ahi cuando la pila se le acaba en un marco de la casa.
    activos: Vec<(u64, u64)>,
    /// Los saltos de catch: (TEB, marca de su pila, el CONTEXT del marco).
    saltos: Vec<(u64, u64, u64)>,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos y ninguna
// referencia al estado cruza una llamada al `.exe`.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { filtro: 0, vectores: Vectores::nuevos(), activos: Vec::new(), saltos: Vec::new() }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.filtro = 0;
    e.vectores = Vectores::nuevos();
    e.activos.clear();
    e.saltos.clear();
}

fn rsp_ahora() -> u64 {
    let r: u64;
    // SAFETY: leer un registro.
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) r, options(nomem, nostack, preserves_flags)) };
    r
}

/// Los despachos de ESTE hilo cuyo CONTEXT ya quedo por debajo de la pila (un
/// RtlUnwindEx salto por encima de ellos) estan muertos: fuera.
fn podar() {
    let (t, rsp) = (kernel32::teb(), rsp_ahora());
    estado().activos.retain(|&(h, c)| h != t || c > rsp);
}

fn activo_de_este_hilo() -> Option<u64> {
    podar();
    let t = kernel32::teb();
    estado().activos.iter().rev().find(|&&(h, _)| h == t).map(|&(_, c)| c)
}

// -- Leer la memoria del proceso, sin salirse ---------------------------------------

/// La memoria que se deja leer: la pila de este hilo (del TEB) y las imagenes
/// (el `.exe` y sus DLL propias). Lo de fuera es un contexto roto: `None`.
pub(crate) struct Viva {
    rangos: Vec<(u64, u64)>,
}

impl Viva {
    pub(crate) fn deja(&self, d: u64, n: u64) -> bool {
        self.rangos.iter().any(|&(a, b)| d >= a && d.checked_add(n).is_some_and(|e| e <= b))
    }

    fn leer<const N: usize>(&self, d: u64) -> Option<[u8; N]> {
        // SAFETY: `d..d+N` cae dentro de un rango de la pila o de una imagen,
        // memoria de este proceso que se puede leer.
        self.deja(d, N as u64).then(|| unsafe { (d as *const [u8; N]).read_unaligned() })
    }

    /// La pila de este hilo y las imagenes, con su `.pdata`.
    pub(crate) fn de_ahora() -> (Viva, Vec<Imagen>) {
        let t = kernel32::teb();
        // SAFETY: el TEB de este hilo, R+W; StackBase y StackLimit caen dentro.
        let (tope, fondo) = unsafe { (((t + teb::TEB_STACK_BASE as u64) as *const u64).read(), ((t + teb::TEB_STACK_LIMIT as u64) as *const u64).read()) };
        let mut bases = vec![kernel32::base_imagen()];
        bases.extend(modulos::bases());
        // Las cabeceras caben en la primera pagina de cada imagen.
        let cabeceras = Viva { rangos: bases.iter().map(|&b| (b, b + 0x1000)).collect() };
        let imagenes: Vec<Imagen> = bases.iter().filter_map(|&b| seh::imagen_en(&cabeceras, b)).collect();
        let mut rangos = vec![(fondo, tope)];
        rangos.extend(imagenes.iter().map(|i| (i.base, i.base + i.tam as u64)));
        // Las tablas de C++ que hace la casa (msvcp_errores), si las hay.
        rangos.extend(crate::cxx::rango_casa());
        (Viva { rangos }, imagenes)
    }
}

impl Memoria for Viva {
    fn u64_en(&self, d: u64) -> Option<u64> {
        self.leer(d).map(u64::from_le_bytes)
    }
    fn u32_en(&self, d: u64) -> Option<u32> {
        self.leer(d).map(u32::from_le_bytes)
    }
    fn u16_en(&self, d: u64) -> Option<u16> {
        self.leer(d).map(u16::from_le_bytes)
    }
    fn u8_en(&self, d: u64) -> Option<u8> {
        self.leer::<1>(d).map(|b| b[0])
    }
}

// -- Llamar al `.exe` ---------------------------------------------------------------

/// Un CONTEXT, un EXCEPTION_RECORD o un DISPATCHER_CONTEXT en la pila de la
/// casa, alineado como los de Windows.
#[repr(C, align(16))]
struct Bytes<const N: usize>([u8; N]);

type Filtro = extern "win64" fn(u64, u64) -> i32;
type Manejador = extern "win64" fn(u64, u64, u64, u64) -> u32;
type Vectorizado = extern "win64" fn(u64) -> i32;
type Terminacion = extern "win64" fn(u64, u64);

/// La funcion del `.exe` en `d`, con esa firma.
///
/// # Safety
/// `d` es codigo del `.exe` (o de la casa) con la firma `F`, de 8 bytes.
unsafe fn funcion<F: Copy>(d: u64) -> F {
    core::mem::transmute_copy(&d)
}

fn bytes<'a>(p: *mut u8, n: usize) -> &'a mut [u8] {
    // SAFETY: quien llama da `n` bytes suyos en `p` (un registro, un contexto
    // o un despacho de Windows, que el `.exe` tambien lee).
    unsafe { core::slice::from_raw_parts_mut(p, n) }
}

// -- La primera pasada: el despacho ----------------------------------------------------

/// **RaiseException**, detras de la foto: el registro, y a despachar.
extern "win64" fn despachar(codigo: u32, banderas: u32, n: u32, args: *const u64, ctx: *mut u8) -> ! {
    let c = Contexto::de_context(bytes(ctx, CONTEXT_BYTES));
    let n = (n as usize).min(seh::MAX_PARAMETROS);
    // SAFETY: el `.exe` promete `n` argumentos en `args`, como en Windows.
    let parametros = if args.is_null() { Vec::new() } else { unsafe { core::slice::from_raw_parts(args, n) }.to_vec() };
    let r = Registro { codigo, banderas: banderas & seh::EXCEPTION_NONCONTINUABLE, anidado: 0, direccion: c.rip, parametros };
    let mut rec = Bytes([0u8; REGISTRO_BYTES]);
    r.a_bytes(&mut rec.0);
    despachar_registro(rec.0.as_mut_ptr(), ctx)
}

fn despachar_registro(rec: *mut u8, ctx: *mut u8) -> ! {
    podar();
    estado().activos.push((kernel32::teb(), ctx as u64));
    let punteros = [rec as u64, ctx as u64];
    // Los vectorizados, antes que ningun marco.
    for h in estado().vectores.en_orden() {
        // SAFETY: lo dio AddVectoredExceptionHandler: LONG f(EXCEPTION_POINTERS*).
        if unsafe { funcion::<Vectorizado>(h) }(punteros.as_ptr() as u64) == EXCEPTION_CONTINUE_EXECUTION {
            seguir(rec, ctx);
        }
    }
    let (m, imagenes) = Viva::de_ahora();
    let mut c = Contexto::de_context(bytes(ctx, CONTEXT_BYTES));
    loop {
        match seh::subir(&m, &imagenes, &mut c, UNW_FLAG_EHANDLER) {
            Ok(Subida::Fuera) => match salto_para(c.gp[RSP]) {
                Some(s) => c = s,
                None => break,
            },
            Ok(Subida::Hoja) => {}
            Ok(Subida::Marco { pc, imagen, funcion: f, marco }) => {
                let Some(h) = marco.manejador else { continue };
                let mut dc = Bytes([0u8; DESPACHO_BYTES]);
                Despacho { pc, base_imagen: imagen.base, funcion: f.dir, establecido: marco.establecido, destino: 0, contexto: ctx as u64, manejador: h, datos: marco.datos, historia: 0, indice: 0 }.a_bytes(&mut dc.0);
                // SAFETY: el manejador de lenguaje que el `.exe` puso en su
                // UNWIND_INFO: EXCEPTION_DISPOSITION f(rec, marco, ctx, despacho).
                match unsafe { funcion::<Manejador>(h) }(rec as u64, marco.establecido, ctx as u64, dc.0.as_mut_ptr() as u64) {
                    DISPOSICION_SEGUIR => seguir(rec, ctx),
                    DISPOSICION_BUSCAR => {}
                    d => aviso(&format!("un manejador de marco dijo {d} (anidada o choque de desenrollados): todavia no, se sigue buscando")),
                }
            }
            Err(e) => {
                aviso(&format!("la excepcion no sube por la pila ({e:?}): queda sin manejar"));
                break;
            }
        }
    }
    sin_manejar(rec, ctx)
}

/// Seguir donde se paro (EXCEPTION_CONTINUE_EXECUTION), con lo que el filtro
/// dejara en el CONTEXT.
fn seguir(rec: *mut u8, ctx: *mut u8) -> ! {
    let r = Registro::de_bytes(bytes(rec, REGISTRO_BYTES));
    if r.banderas & seh::EXCEPTION_NONCONTINUABLE != 0 {
        aviso(&format!("la excepcion {:#x} no se puede seguir y un filtro dijo que siguiera", r.codigo));
        (plataforma().salir)(STATUS_NONCONTINUABLE_EXCEPTION);
    }
    estado().activos.retain(|&(_, c)| c != ctx as u64);
    // SAFETY: el CONTEXT de la excepcion, completo (la foto de RaiseException).
    unsafe { proton_x_restaurar(ctx) }
}

/// Nadie la cogio: el filtro de SetUnhandledExceptionFilter, y si no hay o no
/// dice que se siga, el proceso acaba con el codigo de la excepcion.
fn sin_manejar(rec: *mut u8, ctx: *mut u8) -> ! {
    let r = Registro::de_bytes(bytes(rec, REGISTRO_BYTES));
    let f = estado().filtro;
    if f != 0 {
        let punteros = [rec as u64, ctx as u64];
        // SAFETY: lo dio SetUnhandledExceptionFilter: LONG f(EXCEPTION_POINTERS*).
        if unsafe { funcion::<Vectorizado>(f) }(punteros.as_ptr() as u64) == EXCEPTION_CONTINUE_EXECUTION {
            seguir(rec, ctx);
        }
    } else {
        aviso(&format!("la excepcion {:#x} en {:#x} no la coge nadie: el proceso acaba con su codigo", r.codigo, r.direccion));
    }
    (plataforma().salir)(r.codigo)
}

// -- __C_specific_handler ------------------------------------------------------------

/// **El manejador de `__try` de C**: su tabla de ambitos va en HandlerData.
pub(crate) extern "win64" fn c_specific_handler(rec: *mut u8, establecido: u64, ctx: *mut u8, dc: *mut u8) -> u32 {
    let r = Registro::de_bytes(bytes(rec, REGISTRO_BYTES));
    let d = Despacho::de_bytes(bytes(dc, DESPACHO_BYTES));
    let (m, _) = Viva::de_ahora();
    let Some(t) = seh::ambitos(&m, d.datos) else {
        aviso("__C_specific_handler: la tabla de ambitos no se deja leer");
        return DISPOSICION_BUSCAR;
    };
    let pc = d.pc.wrapping_sub(d.base_imagen) as u32;
    let mut i = d.indice as usize;
    if r.banderas & seh::EXCEPTION_UNWINDING == 0 {
        let punteros = [rec as u64, ctx as u64];
        while let Some(k) = seh::al_buscar(&t, i, pc) {
            let a = t[k];
            let v = if a.manejador == seh::FILTRO_EJECUTAR {
                1
            } else {
                // SAFETY: el filtro del `__except`: LONG f(EXCEPTION_POINTERS*, marco).
                let filtro = unsafe { funcion::<Filtro>(d.base_imagen + a.manejador as u64) };
                filtro(punteros.as_ptr() as u64, establecido)
            };
            if v < 0 {
                return DISPOSICION_SEGUIR;
            }
            if v > 0 {
                // A su bloque, con el codigo en rax (GetExceptionCode()).
                let inicio = Contexto::de_context(bytes(ctx, CONTEXT_BYTES));
                desenrollar_hacia(establecido, d.base_imagen + a.destino as u64, rec, r.codigo as u64, inicio, false);
            }
            i = k + 1;
        }
    } else {
        let destino = (r.banderas & seh::EXCEPTION_TARGET_UNWIND != 0).then(|| d.destino.wrapping_sub(d.base_imagen) as u32);
        while let AlDesenrollar::Finally(k) = seh::al_desenrollar(&t, i, pc, destino) {
            i = k + 1;
            bytes(dc, DESPACHO_BYTES)[DESPACHO_INDICE..DESPACHO_INDICE + 4].copy_from_slice(&(i as u32).to_le_bytes());
            // SAFETY: el bloque del `__finally`: void f(anormal, marco).
            let bloque = unsafe { funcion::<Terminacion>(d.base_imagen + t[k].manejador as u64) };
            bloque(1, establecido);
        }
    }
    DISPOSICION_BUSCAR
}

// -- La segunda pasada: desenrollar ---------------------------------------------------

/// **RtlUnwindEx**, detras de la foto de quien llama.
extern "win64" fn rtl_unwind_ex(marco: u64, destino: u64, rec: *mut u8, valor: u64, ctx: *mut u8) -> ! {
    if marco == 0 {
        aviso("RtlUnwindEx sin marco destino (el desenrollado de salida): todavia no");
        (plataforma().salir)(STATUS_INVALID_UNWIND_TARGET);
    }
    let c = Contexto::de_context(bytes(ctx, CONTEXT_BYTES));
    // Tanda 28: el CRT de MSVC enlazado DENTRO de un modulo (REDGalaxy64.dll,
    // el .exe) coge sus excepciones de C++ con su propio
    // __CxxFrameHandler4, que llama aqui con STATUS_UNWIND_CONSOLIDATE: la
    // casa volvia a `destino` (la vuelta del call que lanzo) con rax = 0 en
    // vez de correr el catch.
    let r = if rec.is_null() { None } else { Some(Registro::de_bytes(bytes(rec, REGISTRO_BYTES))) };
    if let Some(llamada) = r.filter(|r| r.codigo == STATUS_UNWIND_CONSOLIDATE).and_then(|r| r.parametros.first().copied()).filter(|&f| f != 0) {
        desenrollar_con(marco, destino, rec, c, true, &mut |fin: &mut Contexto| {
            fin.gp[RAX] = valor;
            // En la pila de debajo del marco destino (ya no es de nadie),
            // como Windows: la llamada corre el funclet del catch y devuelve
            // donde sigue la funcion.
            // SAFETY: la funcion que el CRT del `.exe` puso en el registro:
            // PVOID f(EXCEPTION_RECORD*).
            fin.rip = unsafe { crate::hilos::llamar_win64(llamada, rec as u64, 0, 0) };
        })
    }
    desenrollar_hacia(marco, destino, rec, valor, c, true)
}

/// Desde `c` hacia arriba hasta el marco cuyo establisher es `objetivo`,
/// llamando al manejador de cada uno con EXCEPTION_UNWINDING, y seguir ALLI en
/// `destino` con `valor` en rax. `puede_saltar`: si la pila se acaba en un
/// marco de la casa (quien llamo es un manejador del `.exe` en pleno
/// despacho), se sigue por la excepcion de este hilo.
fn desenrollar_hacia(objetivo: u64, destino: u64, rec: *mut u8, valor: u64, c: Contexto, puede_saltar: bool) -> ! {
    desenrollar_con(objetivo, destino, rec, c, puede_saltar, &mut |fin: &mut Contexto| {
        fin.rip = destino;
        fin.gp[RAX] = valor;
    })
}

/// **Para las de C++** (`cxx.rs`): desenrollar hasta el marco `objetivo` y,
/// al llegar, `llegar` con SU contexto (ahi se corre el `catch`, en esta
/// pila de debajo, que ya no es de nadie) y seguir donde `llegar` lo deje.
/// Es el STATUS_UNWIND_CONSOLIDATE de Windows. Salta por encima de los
/// marcos de la casa como RtlUnwindEx.
pub(crate) fn desenrollar_y(objetivo: u64, rec: *mut u8, c: Contexto, llegar: &mut dyn FnMut(&mut Contexto)) -> ! {
    desenrollar_con(objetivo, 0, rec, c, true, llegar)
}

/// **Un salto de catch** (`cxx.rs`): mientras corre el funclet de un catch,
/// la pila del `.exe` sube hasta la casa (quien llamo al funclet, por debajo
/// de `marca`) y ahi se acaba. Un despacho o un desenrollado que llegue ahi
/// sigue por `ctx`: el marco de la funcion que tiene el catch. Asi un
/// `throw;` sube por donde subiria en Windows.
pub(crate) fn poner_salto(marca: u64, ctx: u64) {
    estado().saltos.push((kernel32::teb(), marca, ctx));
}

pub(crate) fn soltar_salto(ctx: u64) {
    estado().saltos.retain(|&(_, _, c)| c != ctx);
}

/// Los saltos de ESTE hilo cuya marca queda por debajo de `rsp` (un catch
/// que se abandono: su pila ya es de otro): fuera.
pub(crate) fn podar_saltos(rsp: u64) {
    let t = kernel32::teb();
    estado().saltos.retain(|&(h, m, _)| h != t || m > rsp);
}

/// El salto de catch para una pila que se acabo en la casa con `rsp`: el
/// de la marca mas cercana por encima.
fn salto_para(rsp: u64) -> Option<Contexto> {
    let t = kernel32::teb();
    let s = estado().saltos.iter().filter(|&&(h, m, _)| h == t && m > rsp).min_by_key(|&&(_, m, _)| m).map(|&(_, _, c)| c)?;
    Some(Contexto::de_context(bytes(s as *mut u8, CONTEXT_BYTES)))
}

fn desenrollar_con(objetivo: u64, destino: u64, rec: *mut u8, mut c: Contexto, mut puede_saltar: bool, llegar: &mut dyn FnMut(&mut Contexto)) -> ! {
    let mut propio = Bytes([0u8; REGISTRO_BYTES]);
    let rec = if rec.is_null() {
        Registro { codigo: STATUS_UNWIND, ..Registro::default() }.a_bytes(&mut propio.0);
        propio.0.as_mut_ptr()
    } else {
        rec
    };
    let banderas = Registro::de_bytes(bytes(rec, REGISTRO_BYTES)).banderas;
    let (m, imagenes) = Viva::de_ahora();
    let mut foto = Bytes([0u8; CONTEXT_BYTES]);
    loop {
        let antes = c;
        match seh::subir(&m, &imagenes, &mut c, UNW_FLAG_UHANDLER) {
            Ok(Subida::Fuera) if salto_para(c.gp[RSP]).is_some() => c = salto_para(c.gp[RSP]).expect("recien mirado"),
            Ok(Subida::Fuera) => match activo_de_este_hilo().filter(|_| puede_saltar) {
                Some(a) => {
                    c = Contexto::de_context(bytes(a as *mut u8, CONTEXT_BYTES));
                    puede_saltar = false;
                }
                None => {
                    aviso(&format!("RtlUnwindEx: el marco {objetivo:#x} no esta en la pila"));
                    (plataforma().salir)(STATUS_INVALID_UNWIND_TARGET);
                }
            },
            Ok(Subida::Hoja) => {}
            Ok(Subida::Marco { pc, imagen, funcion: f, marco }) => {
                if marco.establecido > objetivo {
                    aviso(&format!("RtlUnwindEx: se paso del marco {objetivo:#x} sin encontrarlo"));
                    (plataforma().salir)(STATUS_INVALID_UNWIND_TARGET);
                }
                let es_destino = marco.establecido == objetivo;
                if let Some(h) = marco.manejador {
                    let b = banderas | seh::EXCEPTION_UNWINDING | if es_destino { seh::EXCEPTION_TARGET_UNWIND } else { 0 };
                    bytes(rec, REGISTRO_BYTES)[REGISTRO_BANDERAS..REGISTRO_BANDERAS + 4].copy_from_slice(&b.to_le_bytes());
                    antes.a_context(&mut foto.0);
                    let mut dc = Bytes([0u8; DESPACHO_BYTES]);
                    Despacho { pc, base_imagen: imagen.base, funcion: f.dir, establecido: marco.establecido, destino, contexto: foto.0.as_ptr() as u64, manejador: h, datos: marco.datos, historia: 0, indice: 0 }.a_bytes(&mut dc.0);
                    // SAFETY: como en la primera pasada.
                    let d = unsafe { funcion::<Manejador>(h) }(rec as u64, marco.establecido, foto.0.as_mut_ptr() as u64, dc.0.as_mut_ptr() as u64);
                    if d != DISPOSICION_BUSCAR {
                        aviso(&format!("al desenrollar, un manejador de marco dijo {d}: se sigue"));
                    }
                }
                if es_destino {
                    let mut fin = antes;
                    llegar(&mut fin);
                    fin.a_context(&mut foto.0);
                    // SAFETY: el contexto del marco destino, completo; la pila
                    // de debajo (esta) ya no es de nadie.
                    unsafe { proton_x_restaurar(foto.0.as_ptr()) }
                }
            }
            Err(e) => {
                aviso(&format!("RtlUnwindEx: la pila no se desenrolla ({e:?})"));
                (plataforma().salir)(STATUS_BAD_STACK);
            }
        }
    }
}

// -- La pila, para RtlCaptureStackBackTrace (30-09) ----------------------------------

/// Los pc de la pila del `.exe` desde el CONTEXT `ctx`: el suyo y los de
/// quienes llamaron, hasta `max` o hasta salir de sus imagenes.
pub(crate) fn pcs_desde(ctx: *mut u8, max: usize) -> Vec<u64> {
    let (m, imagenes) = Viva::de_ahora();
    let mut c = Contexto::de_context(bytes(ctx, CONTEXT_BYTES));
    let mut v = Vec::new();
    while v.len() < max {
        if seh::imagen_de(&imagenes, c.rip).is_none() {
            break;
        }
        v.push(c.rip);
        match seh::subir(&m, &imagenes, &mut c, 0) {
            Ok(Subida::Fuera) | Err(_) => break,
            _ => {}
        }
    }
    v
}

/// **La pila de un CONTEXT**, marco a marco con el `.pdata` de cada imagen
/// (las hojas, sin entrada, suben por `[rsp]`): hasta `max` direcciones, la
/// primera la del propio contexto. Para decir DESDE DONDE se rindio el
/// `.exe` (UnhandledExceptionFilter), no solo donde.
pub(crate) fn pila_de(ctx: u64, max: usize) -> Vec<u64> {
    let mut c = Contexto::de_context(bytes(ctx as *mut u8, CONTEXT_BYTES));
    let (m, imagenes) = Viva::de_ahora();
    let mut v = vec![c.rip];
    while v.len() < max {
        let Some(im) = seh::imagen_de(&imagenes, c.rip) else { break };
        match seh::funcion_de(&m, &im, c.rip) {
            Some(f) => {
                if desenrollar::un_marco(&m, im.base, &f, &mut c, 0).is_err() {
                    break;
                }
            }
            None => {
                let rsp = c.gp[RSP];
                let Some(r) = m.u64_en(rsp) else { break };
                c.rip = r;
                c.gp[RSP] = rsp + 8;
            }
        }
        if c.rip == 0 {
            break;
        }
        v.push(c.rip);
    }
    v
}

// -- Lo demas que se exporta ----------------------------------------------------------

/// `RtlLookupFunctionEntry(pc, *base, historia)`: la RUNTIME_FUNCTION de `pc`,
/// o NULL (una hoja, o fuera de toda imagen).
extern "win64" fn rtl_lookup_function_entry(pc: u64, base: *mut u64, _historia: u64) -> u64 {
    let (m, imagenes) = Viva::de_ahora();
    let Some(im) = seh::imagen_de(&imagenes, pc) else { return 0 };
    let Some(f) = seh::funcion_de(&m, &im, pc) else { return 0 };
    if !base.is_null() {
        // SAFETY: un ULONG64 del `.exe`.
        unsafe { *base = im.base };
    }
    f.dir
}

/// `RtlVirtualUnwind(tipo, base, pc, funcion, ctx, *datos, *marco, punteros)`:
/// un marco hacia arriba sobre SU contexto; devuelve el manejador, si lo pidio
/// `tipo` y lo hay. Los punteros a donde se guardo cada registro no se dan.
#[allow(clippy::too_many_arguments)]
extern "win64" fn rtl_virtual_unwind(tipo: u32, base: u64, pc: u64, f: *const u32, ctx: *mut u8, datos: *mut u64, marco: *mut u64, _punteros: u64) -> u64 {
    if f.is_null() || ctx.is_null() {
        return 0;
    }
    // SAFETY: una RUNTIME_FUNCTION (tres DWORD) que dio RtlLookupFunctionEntry.
    let fun = unsafe { Funcion { inicio: *f, fin: *f.add(1), desenrollar: *f.add(2), dir: f as u64 } };
    let b = bytes(ctx, CONTEXT_BYTES);
    let mut c = Contexto::de_context(b);
    c.rip = pc;
    let (m, _) = Viva::de_ahora();
    match desenrollar::un_marco(&m, base, &fun, &mut c, tipo as u8) {
        Ok(mk) => {
            c.sobre_context(b);
            // SAFETY: dos ULONG64 del `.exe` (si los dio).
            unsafe {
                if !marco.is_null() {
                    *marco = mk.establecido;
                }
                if let (Some(_), false) = (mk.manejador, datos.is_null()) {
                    *datos = mk.datos;
                }
            }
            mk.manejador.unwrap_or(0)
        }
        Err(e) => {
            aviso(&format!("RtlVirtualUnwind: no se desenrolla ({e:?})"));
            0
        }
    }
}

extern "win64" fn set_unhandled_exception_filter(f: u64) -> u64 {
    core::mem::replace(&mut estado().filtro, f)
}

extern "win64" fn add_vectored_exception_handler(primero: u32, h: u64) -> u64 {
    if h == 0 {
        return 0;
    }
    estado().vectores.poner(primero != 0, h)
}

extern "win64" fn remove_vectored_exception_handler(asa: u64) -> u32 {
    estado().vectores.quitar(asa) as u32
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "RaiseException" => dir!(proton_x_raise_exception),
        "RtlCaptureContext" => dir!(proton_x_rtl_capture_context),
        "RtlLookupFunctionEntry" => dir!(rtl_lookup_function_entry),
        "RtlVirtualUnwind" => dir!(rtl_virtual_unwind),
        "RtlUnwindEx" => dir!(proton_x_rtl_unwind_ex),
        "SetUnhandledExceptionFilter" => dir!(set_unhandled_exception_filter),
        "AddVectoredExceptionHandler" => dir!(add_vectored_exception_handler),
        "RemoveVectoredExceptionHandler" => dir!(remove_vectored_exception_handler),
        _ => return None,
    })
}

/// Lo de `ntdll.dll`: el manejador de `__try` y los Rtl* con su nombre de alli.
pub(crate) fn buscar_ntdll(n: &str) -> Option<u64> {
    Some(match n {
        "__C_specific_handler" => dir!(c_specific_handler),
        "RtlAddVectoredExceptionHandler" => dir!(add_vectored_exception_handler),
        "RtlRemoveVectoredExceptionHandler" => dir!(remove_vectored_exception_handler),
        "RtlCaptureContext" | "RtlLookupFunctionEntry" | "RtlVirtualUnwind" | "RtlUnwindEx" => return buscar(n),
        _ => return None,
    })
}
