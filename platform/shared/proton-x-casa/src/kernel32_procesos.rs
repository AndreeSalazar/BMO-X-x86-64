//! **Los procesos, los hilos y las fibras de `kernel32.dll`, de la casa**
//! (tanda 9 de Cyberpunk, 30-09): lo que quedaba de kernel32.
//!
//! ```text
//!    hilos       SuspendThread TerminateThread QueueUserAPC (corre en las
//!                esperas alertables) FreeLibraryAndExitThread
//!    fibras      ConvertThreadToFiber(Ex) CreateFiber(Ex) SwitchToFiber
//!                DeleteFiber ConvertFiberToThread IsThreadAFiber: cada fibra
//!                con su pila, y el TEB (FiberData, StackBase/Limit) al dia
//!    Toolhelp    CreateToolhelp32Snapshot, Thread32First/Next,
//!                Process32FirstW/NextW, Module32FirstW/NextW
//!    psapi       K32EnumProcesses K32EnumProcessModules
//!                K32GetModuleFileNameExW K32GetModuleInformation
//!                K32GetProcessMemoryInfo
//!    procesador  GetLogicalProcessorInformation(Ex) GetSystemCpuSetInformation
//!                (el Ryzen 5 5600X visto como la casa lo corre: UN nucleo,
//!                con sus caches: 32 KiB, 512 KiB y 32 MiB)
//!    la pila     RtlCaptureStackBackTrace RtlPcToFileHeader RtlUnwind
//!                DebugBreak RaiseFailFastException
//!    discos      GetDiskFreeSpaceExW GetDriveTypeW GetFileTime
//!    y lo demas  HeapQueryInformation CancelSynchronousIo PeekNamedPipe
//!                ReadConsoleA ReadConsoleInputA CreateProcessA
//!                OpenProcessToken OpenThreadToken
//! ```
//!
//! Lo que no, dicho: el espacio de disco es una cifra FIJA (64 GiB, 32
//! libres; la plataforma todavia no lo pregunta); las fechas de un fichero
//! son 0 (como las de la lista de su carpeta); una APC puesta a un hilo que
//! YA duerme en una espera alertable se ve en <= 10 ms, no al instante.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::kernel32_a::w;
use crate::{aviso, dir, hilos, kernel32, modulos, plataforma};

const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_NO_MORE_FILES: u32 = 18;
const ERROR_BAD_LENGTH: u32 = 24;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_NO_TOKEN: u32 = 1008;
const ERROR_NOT_FOUND: u32 = 1168;
const ERROR_ALREADY_FIBER: u32 = 1280;
const ERROR_ALREADY_THREAD: u32 = 1281;
const WAIT_TIMEOUT: u32 = 0x102;
const WAIT_IO_COMPLETION: u32 = 0xC0;
const INFINITE: u32 = u32::MAX;

fn no(e: u32) -> i32 {
    kernel32::poner_error(e);
    0
}

struct Estado {
    /// QueueUserAPC: (el id del hilo, la funcion, su dato).
    apcs: Vec<(u32, u64, u64)>,
    /// Las fotos de Toolhelp: (handle, hilos, modulos, donde va cada lista).
    fotos: Vec<Foto>,
}

struct Foto {
    h: u64,
    hilos: Vec<u32>,
    modulos: Vec<u64>,
    proceso: bool,
    i_hilo: usize,
    i_modulo: usize,
    i_proceso: usize,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { apcs: Vec::new(), fotos: Vec::new() }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia de un turno a otro.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.apcs.clear();
    e.fotos.clear();
}

// -- Hilos -------------------------------------------------------------------------------------

extern "win64" fn suspend_thread(h: u64) -> u32 {
    hilos::suspender(h)
}

extern "win64" fn terminate_thread(h: u64, codigo: u32) -> i32 {
    hilos::terminar(h, codigo) as i32
}

/// `QueueUserAPC(f, hilo, dato)`: a la cola de ese hilo; corre cuando entre
/// en una espera alertable (SleepEx, WaitFor*Ex con TRUE).
extern "win64" fn queue_user_apc(f: u64, h: u64, dato: u64) -> u32 {
    let Some(id) = hilos::id_de(h) else { return no(ERROR_INVALID_HANDLE) as u32 };
    if f == 0 {
        return no(ERROR_INVALID_PARAMETER) as u32;
    }
    estado().apcs.push((id, f, dato));
    1
}

type Apc = extern "win64" fn(u64);

/// Las APC de este hilo, en orden: `true` si corrio alguna.
fn correr_apcs() -> bool {
    let yo = kernel32::get_current_thread_id();
    let mut alguna = false;
    loop {
        let e = estado();
        let Some(i) = e.apcs.iter().position(|a| a.0 == yo) else { break };
        let (_, f, dato) = e.apcs.remove(i);
        // SAFETY: la PAPCFUNC del `.exe`: VOID f(ULONG_PTR).
        let f: Apc = unsafe { core::mem::transmute::<u64, Apc>(f) };
        f(dato);
        alguna = true;
    }
    alguna
}

/// **Una espera alertable**: las APC primero (y WAIT_IO_COMPLETION); si no,
/// `esperar` a trozos de 10 ms, mirando las APC entre uno y otro.
pub(crate) fn espera_alertable(ms: u32, mut esperar: impl FnMut(u32) -> u32) -> u32 {
    const TROZO: u32 = 10;
    let mut queda = ms;
    loop {
        if correr_apcs() {
            return WAIT_IO_COMPLETION;
        }
        let t = if queda == INFINITE { TROZO } else { queda.min(TROZO) };
        let r = esperar(t);
        if r != WAIT_TIMEOUT {
            return r;
        }
        if queda != INFINITE {
            queda -= t;
            if queda == 0 {
                return WAIT_TIMEOUT;
            }
        }
    }
}

extern "win64" fn free_library_and_exit_thread(m: u64, codigo: u32) -> ! {
    w::<extern "win64" fn(u64) -> i32>("FreeLibrary")(m);
    w::<extern "win64" fn(u32) -> !>("ExitThread")(codigo)
}

// -- Fibras ------------------------------------------------------------------------------------
//
// Una fibra: su dato PRIMERO (GetFiberData, en linea, lee *GetCurrentFiber()),
// y luego donde quedo su pila y los limites de esa pila.

#[repr(C)]
struct Fibra {
    dato: u64,
    rsp: u64,
    tope: u64,
    fondo: u64,
    funcion: u64,
    /// La memoria de su pila (0: la del hilo que se convirtio).
    pila: u64,
    bytes: usize,
}

const TEB_FIBER_DATA: u64 = 0x20;
const TEB_STACK_BASE: u64 = 0x08;
const TEB_STACK_LIMIT: u64 = 0x10;

fn teb_u64(off: u64) -> u64 {
    // SAFETY: el TEB de este hilo, R+W.
    unsafe { ((kernel32::teb() + off) as *const u64).read() }
}

fn teb_poner(off: u64, v: u64) {
    // SAFETY: como `teb_u64`.
    unsafe { ((kernel32::teb() + off) as *mut u64).write(v) }
}

/// La fibra de este hilo, o 0. (En Windows, un hilo que no es fibra tiene
/// ahi 0x1E00; la casa lo deja a 0.)
fn actual() -> u64 {
    let f = teb_u64(TEB_FIBER_DATA);
    if f == 0x1E00 {
        0
    } else {
        f
    }
}

fn nueva(f: Fibra) -> u64 {
    Box::into_raw(Box::new(f)) as u64
}

fn fibra<'a>(p: u64) -> &'a mut Fibra {
    // SAFETY: una fibra que dio `nueva` y no se borro.
    unsafe { &mut *(p as *mut Fibra) }
}

extern "win64" fn convert_thread_to_fiber(dato: u64) -> u64 {
    if actual() != 0 {
        return no(ERROR_ALREADY_FIBER) as u64;
    }
    let f = nueva(Fibra { dato, rsp: 0, tope: teb_u64(TEB_STACK_BASE), fondo: teb_u64(TEB_STACK_LIMIT), funcion: 0, pila: 0, bytes: 0 });
    teb_poner(TEB_FIBER_DATA, f);
    f
}

extern "win64" fn convert_thread_to_fiber_ex(dato: u64, _banderas: u32) -> u64 {
    convert_thread_to_fiber(dato)
}

extern "win64" fn convert_fiber_to_thread() -> i32 {
    let f = actual();
    if f == 0 {
        return no(ERROR_ALREADY_THREAD);
    }
    teb_poner(TEB_FIBER_DATA, 0);
    if fibra(f).pila == 0 {
        // SAFETY: la creo `nueva`; ya no es de nadie.
        drop(unsafe { Box::from_raw(f as *mut Fibra) });
    }
    1
}

extern "win64" fn is_thread_a_fiber() -> i32 {
    (actual() != 0) as i32
}

const PILA_FIBRA: usize = 1 << 20;

/// `CreateFiber(pila, funcion, dato)`: una pila del monton y el marco que
/// `proton_x_fibra_cambiar` desapila la primera vez.
extern "win64" fn create_fiber(pila: usize, funcion: u64, dato: u64) -> u64 {
    if funcion == 0 {
        return no(ERROR_INVALID_PARAMETER) as u64;
    }
    let bytes = if pila == 0 { PILA_FIBRA } else { pila.clamp(64 << 10, 64 << 20) };
    let Ok(forma) = core::alloc::Layout::from_size_align(bytes, 16) else { return 0 };
    // SAFETY: `forma` no mide cero.
    let m = unsafe { alloc::alloc::alloc_zeroed(forma) };
    if m.is_null() {
        return no(8) as u64;
    }
    let fondo = m as u64;
    let tope = (fondo + bytes as u64) & !15;
    let f = nueva(Fibra { dato, rsp: 0, tope, fondo, funcion, pila: m as u64, bytes });
    // El marco: xmm6..15 (160), mxcsr y el control del x87, los 8 que se
    // desapilan (r12 lleva la fibra) y la vuelta a su arranque.
    let marco = tope - 248;
    // SAFETY: `marco..tope` es la pila recien pedida.
    unsafe {
        core::ptr::write_bytes(marco as *mut u8, 0, 248);
        ((marco + 160) as *mut u32).write(0x1F80);
        ((marco + 164) as *mut u16).write(0x027F);
        ((marco + 192) as *mut u64).write(f);
        ((marco + 232) as *mut u64).write(proton_x_fibra_arranque as *const () as u64);
    }
    fibra(f).rsp = marco;
    f
}

extern "win64" fn create_fiber_ex(_compromiso: usize, reserva: usize, _banderas: u32, funcion: u64, dato: u64) -> u64 {
    create_fiber(reserva, funcion, dato)
}

/// Donde empieza una fibra: su funcion, y si vuelve, el hilo acaba (como en
/// Windows).
extern "win64" fn fibra_empieza(f: u64) -> ! {
    let (func, dato) = (fibra(f).funcion, fibra(f).dato);
    // SAFETY: la funcion del `.exe`: VOID WINAPI f(LPVOID).
    unsafe { hilos::llamar_win64(func, dato, 0, 0) };
    w::<extern "win64" fn(u32) -> !>("ExitThread")(0)
}

core::arch::global_asm!(
    // proton_x_fibra_cambiar(&mut rsp_de_la_que_sale, rsp_de_la_que_entra)
    ".globl proton_x_fibra_cambiar",
    "proton_x_fibra_cambiar:",
    "push rbp",
    "push rbx",
    "push rdi",
    "push rsi",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "sub rsp, 168",
    "movups [rsp], xmm6",
    "movups [rsp + 16], xmm7",
    "movups [rsp + 32], xmm8",
    "movups [rsp + 48], xmm9",
    "movups [rsp + 64], xmm10",
    "movups [rsp + 80], xmm11",
    "movups [rsp + 96], xmm12",
    "movups [rsp + 112], xmm13",
    "movups [rsp + 128], xmm14",
    "movups [rsp + 144], xmm15",
    "stmxcsr [rsp + 160]",
    "fnstcw [rsp + 164]",
    "mov [rcx], rsp",
    "mov rsp, rdx",
    "ldmxcsr [rsp + 160]",
    "fldcw [rsp + 164]",
    "movups xmm6, [rsp]",
    "movups xmm7, [rsp + 16]",
    "movups xmm8, [rsp + 32]",
    "movups xmm9, [rsp + 48]",
    "movups xmm10, [rsp + 64]",
    "movups xmm11, [rsp + 80]",
    "movups xmm12, [rsp + 96]",
    "movups xmm13, [rsp + 112]",
    "movups xmm14, [rsp + 128]",
    "movups xmm15, [rsp + 144]",
    "add rsp, 168",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rsi",
    "pop rdi",
    "pop rbx",
    "pop rbp",
    "ret",
    // La primera vez: rsp queda en tope - 8, como tras un `call`.
    ".globl proton_x_fibra_arranque",
    "proton_x_fibra_arranque:",
    "mov rcx, r12",
    "sub rsp, 40",
    "call {empieza}",
    "ud2",
    empieza = sym fibra_empieza,
);

extern "C" {
    fn proton_x_fibra_arranque();
}

extern "win64" {
    fn proton_x_fibra_cambiar(sale: *mut u64, entra: u64);
}

extern "win64" fn switch_to_fiber(destino: u64) {
    let sale = actual();
    if destino == 0 || sale == 0 {
        aviso("SwitchToFiber sin ser una fibra (o a ninguna): el hilo tiene que ConvertThreadToFiber antes");
        return;
    }
    if sale == destino {
        return;
    }
    let d = fibra(destino);
    teb_poner(TEB_FIBER_DATA, destino);
    teb_poner(TEB_STACK_BASE, d.tope);
    teb_poner(TEB_STACK_LIMIT, d.fondo);
    // SAFETY: `destino` quedo en `proton_x_fibra_cambiar` (o es su marco de
    // nacimiento); `sale` es la fibra que corre, con su rsp por guardar.
    unsafe { proton_x_fibra_cambiar(&mut fibra(sale).rsp, d.rsp) };
    // De vuelta aqui: otra fibra cambio a esta, y ya puso el TEB.
}

extern "win64" fn delete_fiber(f: u64) {
    if f == 0 {
        return;
    }
    if f == actual() {
        // Borrar la fibra que corre acaba el hilo (Windows).
        w::<extern "win64" fn(u32) -> !>("ExitThread")(0);
    }
    let x = fibra(f);
    if x.pila != 0 {
        if let Ok(forma) = core::alloc::Layout::from_size_align(x.bytes, 16) {
            // SAFETY: la pila que pidio `create_fiber` con esta forma.
            unsafe { alloc::alloc::dealloc(x.pila as *mut u8, forma) };
        }
    }
    // SAFETY: la creo `nueva`; no es la actual.
    drop(unsafe { Box::from_raw(f as *mut Fibra) });
}

// -- Toolhelp ----------------------------------------------------------------------------------

const FOTO: u64 = 0x5C00_0000;
const TH32CS_SNAPPROCESS: u32 = 0x2;
const TH32CS_SNAPTHREAD: u32 = 0x4;
const TH32CS_SNAPMODULE: u32 = 0x8;
const TH32CS_SNAPMODULE32: u32 = 0x10;

pub(crate) fn es_suyo(h: u64) -> bool {
    estado().fotos.iter().any(|f| f.h == h)
}

pub(crate) fn cerrar(h: u64) -> i32 {
    estado().fotos.retain(|f| f.h != h);
    1
}

/// Las imagenes del proceso: el `.exe` y las DLL propias.
fn imagenes() -> Vec<u64> {
    let mut v = alloc::vec![kernel32::base_imagen()];
    v.extend(modulos::bases());
    v
}

pub(crate) fn medida_imagen(b: u64) -> u32 {
    // SAFETY: la cabecera PE de una imagen cargada.
    unsafe {
        let nt = b + ((b + 0x3C) as *const u32).read_unaligned() as u64;
        ((nt + 0x50) as *const u32).read_unaligned()
    }
}

fn entrada_imagen(b: u64) -> u64 {
    // SAFETY: como `medida_imagen`.
    unsafe {
        let nt = b + ((b + 0x3C) as *const u32).read_unaligned() as u64;
        let e = ((nt + 0x28) as *const u32).read_unaligned();
        if e == 0 {
            0
        } else {
            b + e as u64
        }
    }
}

extern "win64" fn create_toolhelp32_snapshot(banderas: u32, pid: u32) -> u64 {
    if pid != 0 && pid != kernel32::id_del_proceso() {
        return no(ERROR_INVALID_PARAMETER) as u64 | u64::MAX;
    }
    let e = estado();
    let h = FOTO + e.fotos.len() as u64 * 4 + 4;
    e.fotos.push(Foto {
        h,
        hilos: if banderas & TH32CS_SNAPTHREAD != 0 { hilos::ids_vivos() } else { Vec::new() },
        modulos: if banderas & (TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32) != 0 { imagenes() } else { Vec::new() },
        proceso: banderas & TH32CS_SNAPPROCESS != 0,
        i_hilo: 0,
        i_modulo: 0,
        i_proceso: 0,
    });
    h
}

fn foto(h: u64) -> Option<&'static mut Foto> {
    estado().fotos.iter_mut().find(|f| f.h == h)
}

fn medida_ok(p: *const u8, minima: u32) -> bool {
    // SAFETY: el dwSize de la estructura del `.exe`.
    !p.is_null() && unsafe { (p as *const u32).read_unaligned() } >= minima
}

/// THREADENTRY32 (28 bytes): cntUsage, el id, el proceso, la prioridad.
fn hilo_entrada(h: u64, p: *mut u8, desde_cero: bool) -> i32 {
    let Some(f) = foto(h) else { return no(ERROR_INVALID_HANDLE) };
    if !medida_ok(p, 28) {
        return no(ERROR_BAD_LENGTH);
    }
    if desde_cero {
        f.i_hilo = 0;
    }
    let Some(&id) = f.hilos.get(f.i_hilo) else { return no(ERROR_NO_MORE_FILES) };
    f.i_hilo += 1;
    // SAFETY: 28 bytes del `.exe` (su dwSize lo dijo).
    unsafe {
        for (i, v) in [0u32, id, kernel32::id_del_proceso(), 8, 0, 0].into_iter().enumerate() {
            (p.add(4 + 4 * i) as *mut u32).write_unaligned(v);
        }
    }
    1
}

extern "win64" fn thread32_first(h: u64, p: *mut u8) -> i32 {
    hilo_entrada(h, p, true)
}

extern "win64" fn thread32_next(h: u64, p: *mut u8) -> i32 {
    hilo_entrada(h, p, false)
}

fn poner_w(p: *mut u8, t: &[u16], max: usize) {
    let k = t.len().min(max - 1);
    // SAFETY: `max` WCHAR del `.exe` en `p`.
    unsafe {
        core::ptr::copy_nonoverlapping(t.as_ptr(), p as *mut u16, k);
        (p as *mut u16).add(k).write_unaligned(0);
    }
}

fn nombre_de_modulo(b: u64) -> Vec<u16> {
    let f: extern "win64" fn(u64, *mut u16, u32) -> u32 = w("GetModuleFileNameW");
    let mut buf = alloc::vec![0u16; 520];
    let k = f(b, buf.as_mut_ptr(), 520) as usize;
    buf.truncate(k.min(519));
    buf
}

/// PROCESSENTRY32W (568 bytes): el propio proceso, uno.
fn proceso_entrada(h: u64, p: *mut u8, desde_cero: bool) -> i32 {
    let Some(f) = foto(h) else { return no(ERROR_INVALID_HANDLE) };
    if !medida_ok(p, 568) {
        return no(ERROR_BAD_LENGTH);
    }
    if desde_cero {
        f.i_proceso = 0;
    }
    if !f.proceso || f.i_proceso > 0 {
        return no(ERROR_NO_MORE_FILES);
    }
    f.i_proceso = 1;
    let ruta = nombre_de_modulo(0);
    let nombre = &ruta[ruta.iter().rposition(|&c| c == b'\\' as u16).map_or(0, |i| i + 1)..];
    // SAFETY: 568 bytes del `.exe`.
    unsafe {
        core::ptr::write_bytes(p.add(4), 0, 564);
        (p.add(8) as *mut u32).write_unaligned(kernel32::id_del_proceso());
        (p.add(28) as *mut u32).write_unaligned(hilos::ids_vivos().len() as u32);
        (p.add(36) as *mut i32).write_unaligned(8);
    }
    poner_w(unsafe { p.add(44) }, nombre, 260);
    1
}

extern "win64" fn process32_first_w(h: u64, p: *mut u8) -> i32 {
    proceso_entrada(h, p, true)
}

extern "win64" fn process32_next_w(h: u64, p: *mut u8) -> i32 {
    proceso_entrada(h, p, false)
}

/// MODULEENTRY32W (1080 bytes): base, medida, el handle, nombre y ruta.
fn modulo_entrada(h: u64, p: *mut u8, desde_cero: bool) -> i32 {
    let Some(f) = foto(h) else { return no(ERROR_INVALID_HANDLE) };
    if !medida_ok(p, 1080) {
        return no(ERROR_BAD_LENGTH);
    }
    if desde_cero {
        f.i_modulo = 0;
    }
    let Some(&b) = f.modulos.get(f.i_modulo) else { return no(ERROR_NO_MORE_FILES) };
    f.i_modulo += 1;
    let ruta = nombre_de_modulo(if b == kernel32::base_imagen() { 0 } else { b });
    let nombre = &ruta[ruta.iter().rposition(|&c| c == b'\\' as u16).map_or(0, |i| i + 1)..];
    // SAFETY: 1080 bytes del `.exe`.
    unsafe {
        core::ptr::write_bytes(p.add(4), 0, 1076);
        (p.add(4) as *mut u32).write_unaligned(1);
        (p.add(8) as *mut u32).write_unaligned(kernel32::id_del_proceso());
        (p.add(12) as *mut u32).write_unaligned(0xFFFF);
        (p.add(16) as *mut u32).write_unaligned(0xFFFF);
        (p.add(24) as *mut u64).write_unaligned(b);
        (p.add(32) as *mut u32).write_unaligned(medida_imagen(b));
        (p.add(40) as *mut u64).write_unaligned(b);
        poner_w(p.add(48), nombre, 256);
        poner_w(p.add(560), &ruta, 260);
    }
    1
}

extern "win64" fn module32_first_w(h: u64, p: *mut u8) -> i32 {
    modulo_entrada(h, p, true)
}

extern "win64" fn module32_next_w(h: u64, p: *mut u8) -> i32 {
    modulo_entrada(h, p, false)
}

// -- psapi -------------------------------------------------------------------------------------

/// Dar una lista de u32/u64 en un bufer de `cb` bytes y decir cuanto hacia
/// falta (psapi: nunca falla por corto).
fn dar_lista<T: Copy>(v: &[T], p: *mut T, cb: u32, hace_falta: *mut u32) -> i32 {
    let t = core::mem::size_of::<T>();
    if !hace_falta.is_null() {
        // SAFETY: el DWORD del `.exe`.
        unsafe { *hace_falta = (v.len() * t) as u32 };
    }
    let k = v.len().min(cb as usize / t);
    if k > 0 && !p.is_null() {
        // SAFETY: `cb` bytes del `.exe`.
        unsafe { core::ptr::copy_nonoverlapping(v.as_ptr(), p, k) };
    }
    1
}

extern "win64" fn k32_enum_processes(p: *mut u32, cb: u32, hace_falta: *mut u32) -> i32 {
    dar_lista(&[kernel32::id_del_proceso()], p, cb, hace_falta)
}

/// Solo el propio proceso (su pseudo-handle -1, o un handle suyo).
fn es_propio(proceso: u64) -> bool {
    proceso == u64::MAX || proceso != 0
}

extern "win64" fn k32_enum_process_modules(proceso: u64, p: *mut u64, cb: u32, hace_falta: *mut u32) -> i32 {
    if !es_propio(proceso) {
        return no(ERROR_INVALID_HANDLE);
    }
    dar_lista(&imagenes(), p, cb, hace_falta)
}

extern "win64" fn k32_get_module_file_name_ex_w(proceso: u64, m: u64, buf: *mut u16, n: u32) -> u32 {
    if !es_propio(proceso) {
        return no(ERROR_INVALID_HANDLE) as u32;
    }
    w::<extern "win64" fn(u64, *mut u16, u32) -> u32>("GetModuleFileNameW")(m, buf, n)
}

/// MODULEINFO (24 bytes): base, medida y entrada.
extern "win64" fn k32_get_module_information(proceso: u64, m: u64, p: *mut u8, cb: u32) -> i32 {
    if !es_propio(proceso) {
        return no(ERROR_INVALID_HANDLE);
    }
    if p.is_null() || cb < 24 {
        return no(ERROR_INSUFFICIENT_BUFFER);
    }
    let b = if m == 0 { kernel32::base_imagen() } else { m };
    if !imagenes().contains(&b) {
        return no(ERROR_INVALID_HANDLE);
    }
    // SAFETY: 24 bytes del `.exe`.
    unsafe {
        (p as *mut u64).write_unaligned(b);
        (p.add(8) as *mut u64).write_unaligned(medida_imagen(b) as u64);
        (p.add(16) as *mut u64).write_unaligned(entrada_imagen(b));
    }
    1
}

/// PROCESS_MEMORY_COUNTERS (72) o _EX (80): lo que se sabe es la memoria
/// de las imagenes; el monton no se cuenta todavia.
extern "win64" fn k32_get_process_memory_info(proceso: u64, p: *mut u8, cb: u32) -> i32 {
    if !es_propio(proceso) {
        return no(ERROR_INVALID_HANDLE);
    }
    if p.is_null() || cb < 72 {
        return no(ERROR_INSUFFICIENT_BUFFER);
    }
    let usado: u64 = imagenes().iter().map(|&b| medida_imagen(b) as u64).sum();
    // SAFETY: `cb` (72 u 80) bytes del `.exe`.
    unsafe {
        core::ptr::write_bytes(p, 0, cb.min(80) as usize);
        (p as *mut u32).write_unaligned(cb.min(80));
        for off in [8usize, 16, 64] {
            (p.add(off) as *mut u64).write_unaligned(usado);
        }
        (p.add(56) as *mut u64).write_unaligned(usado);
        if cb >= 80 {
            (p.add(72) as *mut u64).write_unaligned(usado);
        }
    }
    1
}

// -- El procesador -----------------------------------------------------------------------------

/// (nivel, asociatividad, linea, medida, tipo): 0 unificada, 1 instrucciones, 2 datos.
const CACHES: [(u8, u8, u16, u32, u32); 4] = [(1, 8, 64, 32 << 10, 2), (1, 8, 64, 32 << 10, 1), (2, 8, 64, 512 << 10, 0), (3, 16, 64, 32 << 20, 0)];

/// SYSTEM_LOGICAL_PROCESSOR_INFORMATION (32 bytes cada una): el nucleo, el
/// nodo NUMA, las cuatro caches y el paquete.
extern "win64" fn get_logical_processor_information(p: *mut u8, largo: *mut u32) -> i32 {
    if largo.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    let n = 7 * 32;
    // SAFETY: el DWORD del `.exe`.
    let tiene = unsafe { *largo } as usize;
    // SAFETY: como arriba.
    unsafe { *largo = n as u32 };
    if p.is_null() || tiene < n {
        return no(ERROR_INSUFFICIENT_BUFFER);
    }
    let mut b = alloc::vec![0u8; n];
    let mut poner = |i: usize, rel: u32, cuerpo: &[u8]| {
        b[32 * i..32 * i + 8].copy_from_slice(&1u64.to_le_bytes());
        b[32 * i + 8..32 * i + 12].copy_from_slice(&rel.to_le_bytes());
        b[32 * i + 16..32 * i + 16 + cuerpo.len()].copy_from_slice(cuerpo);
    };
    poner(0, 0, &[0]);
    poner(1, 1, &[0]);
    for (k, &(nivel, asoc, linea, tam, tipo)) in CACHES.iter().enumerate() {
        let mut c = alloc::vec![nivel, asoc];
        c.extend_from_slice(&linea.to_le_bytes());
        c.extend_from_slice(&tam.to_le_bytes());
        c.extend_from_slice(&tipo.to_le_bytes());
        poner(2 + k, 2, &c);
    }
    poner(6, 3, &[0]);
    // SAFETY: `tiene` >= n bytes del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), p, n) };
    1
}

/// GROUP_AFFINITY del unico procesador: mascara 1, grupo 0.
fn afinidad() -> [u8; 16] {
    let mut a = [0u8; 16];
    a[..8].copy_from_slice(&1u64.to_le_bytes());
    a
}

/// SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX: cada una con su medida.
extern "win64" fn get_logical_processor_information_ex(rel: u32, p: *mut u8, largo: *mut u32) -> i32 {
    if largo.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    let todas = rel == 0xFFFF;
    let mut b: Vec<u8> = Vec::new();
    let mut una = |tipo: u32, cuerpo: Vec<u8>| {
        if todas || rel == tipo {
            b.extend_from_slice(&tipo.to_le_bytes());
            b.extend_from_slice(&(cuerpo.len() as u32 + 8).to_le_bytes());
            b.extend_from_slice(&cuerpo);
        }
    };
    // Nucleo (0) y paquete (3): Flags, EfficiencyClass, 20 reservados,
    // GroupCount 1 y su afinidad.
    let procesador = || {
        let mut c = alloc::vec![0u8; 22];
        c.extend_from_slice(&1u16.to_le_bytes());
        c.extend_from_slice(&afinidad());
        c
    };
    una(0, procesador());
    // Nodo NUMA (1): el 0, 18 reservados, GroupCount 1 y su afinidad.
    let mut numa = alloc::vec![0u8; 22];
    numa.extend_from_slice(&1u16.to_le_bytes());
    numa.extend_from_slice(&afinidad());
    una(1, numa);
    // Caches (2): nivel, asociatividad, linea, medida, tipo, 18 reservados,
    // GroupCount 1 y su afinidad.
    for &(nivel, asoc, linea, tam, tipo) in &CACHES {
        let mut c = alloc::vec![nivel, asoc];
        c.extend_from_slice(&linea.to_le_bytes());
        c.extend_from_slice(&tam.to_le_bytes());
        c.extend_from_slice(&tipo.to_le_bytes());
        c.extend_from_slice(&[0u8; 18]);
        c.extend_from_slice(&1u16.to_le_bytes());
        c.extend_from_slice(&afinidad());
        una(2, c);
    }
    una(3, procesador());
    // Grupo (4): 1 y 1, 20 reservados, y el grupo 0 con 1 procesador.
    let mut g = alloc::vec![1u8, 0, 1, 0];
    g.extend_from_slice(&[0u8; 20]);
    g.extend_from_slice(&[1, 1]);
    g.extend_from_slice(&[0u8; 38]);
    g.extend_from_slice(&1u64.to_le_bytes());
    una(4, g);
    // SAFETY: el DWORD del `.exe`.
    let tiene = unsafe { *largo } as usize;
    // SAFETY: como arriba.
    unsafe { *largo = b.len() as u32 };
    if b.is_empty() {
        return no(ERROR_INVALID_PARAMETER);
    }
    if p.is_null() || tiene < b.len() {
        return no(ERROR_INSUFFICIENT_BUFFER);
    }
    // SAFETY: `tiene` bytes del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), p, b.len()) };
    1
}

/// SYSTEM_CPU_SET_INFORMATION (32 bytes): un CPU set, el 0x100.
extern "win64" fn get_system_cpu_set_information(p: *mut u8, largo: u32, devuelto: *mut u32, _proceso: u64, _b: u32) -> i32 {
    if !devuelto.is_null() {
        // SAFETY: el ULONG del `.exe`.
        unsafe { *devuelto = 32 };
    }
    if p.is_null() || largo < 32 {
        return no(ERROR_INSUFFICIENT_BUFFER);
    }
    // SAFETY: 32 bytes del `.exe`.
    unsafe {
        core::ptr::write_bytes(p, 0, 32);
        (p as *mut u32).write_unaligned(32);
        (p.add(8) as *mut u32).write_unaligned(0x100);
    }
    1
}

// -- La pila -----------------------------------------------------------------------------------

/// Lo de detras de la foto: `skip` marcos fuera y hasta `n` en `marcos`.
extern "win64" fn pila(skip: u32, n: u32, marcos: *mut u64, hash: *mut u32, ctx: *mut u8) -> u16 {
    let pcs = crate::excepciones::pcs_desde(ctx, (skip + n) as usize);
    let v: Vec<u64> = pcs.into_iter().skip(skip as usize).take(n as usize).collect();
    if !marcos.is_null() {
        // SAFETY: `n` punteros del `.exe`.
        unsafe { core::ptr::copy_nonoverlapping(v.as_ptr(), marcos, v.len()) };
    }
    if !hash.is_null() {
        // SAFETY: el ULONG del `.exe`.
        unsafe { *hash = v.iter().fold(0u32, |h, &x| h.wrapping_add(x as u32)) };
    }
    v.len() as u16
}

// `RtlCaptureStackBackTrace(skip, n, marcos, hash)`: la foto de QUIEN LLAMA
// (como RaiseException) y a recorrer su pila.
core::arch::global_asm!(
    ".globl proton_x_capture_stack_back_trace",
    "proton_x_capture_stack_back_trace:",
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
    "call {pila}",
    "add rsp, 0x508",
    "ret",
    // DebugBreak: EXCEPTION_BREAKPOINT, lanzada desde quien llama.
    ".globl proton_x_debug_break",
    "proton_x_debug_break:",
    "mov ecx, 0x80000003",
    "xor edx, edx",
    "xor r8d, r8d",
    "xor r9d, r9d",
    "jmp proton_x_raise_exception",
    pila = sym pila,
);

extern "C" {
    fn proton_x_capture_stack_back_trace();
    fn proton_x_debug_break();
    fn proton_x_rtl_unwind_ex();
}

/// `RtlPcToFileHeader(pc, *base)`: la imagen que tiene `pc`, o NULL.
/// **El modulo que tiene la direccion `pc`**: su base, si cae dentro de la
/// imagen del `.exe` o de una DLL del juego (RtlPcToFileHeader,
/// GetModuleHandleExW con FROM_ADDRESS).
pub(crate) fn imagen_con(pc: u64) -> Option<u64> {
    imagenes().into_iter().find(|&b| pc >= b && pc < b + medida_imagen(b) as u64)
}

extern "win64" fn rtl_pc_to_file_header(pc: u64, base: *mut u64) -> u64 {
    let b = imagen_con(pc).unwrap_or(0);
    if !base.is_null() {
        // SAFETY: el PVOID del `.exe`.
        unsafe { *base = b };
    }
    b
}

extern "win64" fn raise_fail_fast_exception(rec: *const u32, _ctx: u64, _banderas: u32) -> ! {
    // SAFETY: el EXCEPTION_RECORD del `.exe` (su codigo primero), o NULL.
    let codigo = if rec.is_null() { 0xC000_0602 } else { unsafe { rec.read_unaligned() } };
    aviso(&alloc::format!("RaiseFailFastException({codigo:#x}): el proceso acaba ya, sin manejadores"));
    (plataforma().salir)(codigo)
}

// -- Discos y ficheros -------------------------------------------------------------------------

const GIB: u64 = 1 << 30;

extern "win64" fn get_disk_free_space_ex_w(_dir: *const u16, libre_para_mi: *mut u64, total: *mut u64, libre: *mut u64) -> i32 {
    for (p, v) in [(libre_para_mi, 32 * GIB), (total, 64 * GIB), (libre, 32 * GIB)] {
        if !p.is_null() {
            // SAFETY: los ULARGE_INTEGER del `.exe`.
            unsafe { p.write_unaligned(v) };
        }
    }
    1
}

const DRIVE_NO_ROOT_DIR: u32 = 1;
const DRIVE_FIXED: u32 = 3;

/// `GetDriveTypeW`: C: (el volumen de la casa) y D: (el disco Personal) son
/// discos fijos; lo demas, que no esta.
extern "win64" fn get_drive_type_w(raiz: *const u16) -> u32 {
    if raiz.is_null() {
        return DRIVE_FIXED;
    }
    // SAFETY: una cadena del `.exe` ("C:\", "D:", ...).
    let (l, dos) = unsafe { (*raiz | 0x20, *raiz.add(1)) };
    if (l == b'c' as u16 || l == b'd' as u16) && dos == b':' as u16 {
        DRIVE_FIXED
    } else {
        DRIVE_NO_ROOT_DIR
    }
}

/// `GetFileTime`: las fechas del listado de su carpeta (01-10: las de NTFS
/// en el disco Personal; 0 donde el volumen no las da).
extern "win64" fn get_file_time(h: u64, creado: *mut u64, leido: *mut u64, escrito: *mut u64) -> i32 {
    if !crate::ficheros::es_fichero(h) {
        return no(ERROR_INVALID_HANDLE);
    }
    let f = crate::ficheros::abierto(h).and_then(|a| crate::carpetas::entrada(&a.ruta)).map_or([0; 3], |e| e.fechas);
    for (p, v) in [(creado, f[0]), (leido, f[2]), (escrito, f[1])] {
        if !p.is_null() {
            // SAFETY: los FILETIME del `.exe`.
            unsafe { p.write_unaligned(v) };
        }
    }
    1
}

// -- Y lo demas --------------------------------------------------------------------------------

/// `HeapQueryInformation(HeapCompatibilityInformation)`: 2, el LFH (lo que
/// dice Windows 10 de cualquier heap).
extern "win64" fn heap_query_information(_heap: u64, clase: u32, p: *mut u32, largo: usize, devuelto: *mut usize) -> i32 {
    if clase != 0 {
        return no(ERROR_INVALID_PARAMETER);
    }
    if !devuelto.is_null() {
        // SAFETY: el SIZE_T del `.exe`.
        unsafe { *devuelto = 4 };
    }
    if p.is_null() || largo < 4 {
        return no(ERROR_INSUFFICIENT_BUFFER);
    }
    // SAFETY: el ULONG del `.exe`.
    unsafe { p.write_unaligned(2) };
    1
}

/// `CancelSynchronousIo`: en la casa ninguna E/S se queda a medias.
extern "win64" fn cancel_synchronous_io(_h: u64) -> i32 {
    no(ERROR_NOT_FOUND)
}

/// `PeekNamedPipe`: la casa no tiene tuberias.
extern "win64" fn peek_named_pipe(_h: u64, _b: u64, _n: u32, _leidos: u64, _total: u64, _resto: u64) -> i32 {
    no(ERROR_INVALID_HANDLE)
}

/// `ReadConsoleA` / `ReadConsoleInputA`: como `ReadConsoleW`, sin entrada.
extern "win64" fn read_console_a(_h: u64, _b: u64, _n: u32, _leidos: u64, _ctl: u64) -> i32 {
    no(ERROR_INVALID_HANDLE)
}

extern "win64" fn read_console_input_a(_h: u64, _b: u64, _n: u32, _leidos: u64) -> i32 {
    no(ERROR_INVALID_HANDLE)
}

type CreateProcessW = extern "win64" fn(*const u16, *const u16, u64, u64, i32, u32, u64, u64, u64, u64) -> i32;

fn ancha(p: *const u8) -> Option<Vec<u16>> {
    if p.is_null() {
        return None;
    }
    let mut v = bmo_proton_x::texto::a_ancho(&crate::crt::cadena_c(p as u64), false).unwrap_or_default();
    v.push(0);
    Some(v)
}

/// `CreateProcessA`: a UTF-16 y a `CreateProcessW` (el STARTUPINFOA no se
/// mira: la casa no lanza procesos).
extern "win64" fn create_process_a(app: *const u8, linea: *const u8, a: u64, b: u64, c: i32, d: u32, e: u64, f: u64, g: u64, h: u64) -> i32 {
    let (app, linea) = (ancha(app), ancha(linea));
    let p = |v: &Option<Vec<u16>>| v.as_ref().map_or(core::ptr::null(), |v| v.as_ptr());
    w::<CreateProcessW>("CreateProcessW")(p(&app), p(&linea), a, b, c, d, e, f, g, h)
}

/// El token del proceso: uno fijo (lo que pregunten, ADVAPI32 lo contesta).
const TOKEN: u64 = 0x5C80_0000;

extern "win64" fn open_process_token(_proceso: u64, _acceso: u32, token: *mut u64) -> i32 {
    if token.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    // SAFETY: el HANDLE del `.exe`.
    unsafe { *token = TOKEN };
    1
}

/// `OpenThreadToken`: el token del proceso si el hilo se hace pasar por si
/// mismo (ImpersonateSelf); si no, no hay (ERROR_NO_TOKEN).
extern "win64" fn open_thread_token(_hilo: u64, acceso: u32, _propio: i32, token: *mut u64) -> i32 {
    if crate::version_y_seguridad::suplantando() {
        return open_process_token(0, acceso, token);
    }
    no(ERROR_NO_TOKEN)
}

/// Las funciones de este modulo, por su nombre.
pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "SuspendThread" => dir!(suspend_thread),
        "TerminateThread" => dir!(terminate_thread),
        "QueueUserAPC" => dir!(queue_user_apc),
        "FreeLibraryAndExitThread" => dir!(free_library_and_exit_thread),
        "ConvertThreadToFiber" => dir!(convert_thread_to_fiber),
        "ConvertThreadToFiberEx" => dir!(convert_thread_to_fiber_ex),
        "ConvertFiberToThread" => dir!(convert_fiber_to_thread),
        "IsThreadAFiber" => dir!(is_thread_a_fiber),
        "CreateFiber" => dir!(create_fiber),
        "CreateFiberEx" => dir!(create_fiber_ex),
        "SwitchToFiber" => dir!(switch_to_fiber),
        "DeleteFiber" => dir!(delete_fiber),
        "CreateToolhelp32Snapshot" => dir!(create_toolhelp32_snapshot),
        "Thread32First" => dir!(thread32_first),
        "Thread32Next" => dir!(thread32_next),
        "Process32FirstW" => dir!(process32_first_w),
        "Process32NextW" => dir!(process32_next_w),
        "Module32FirstW" => dir!(module32_first_w),
        "Module32NextW" => dir!(module32_next_w),
        "K32EnumProcesses" => dir!(k32_enum_processes),
        "K32EnumProcessModules" => dir!(k32_enum_process_modules),
        "K32GetModuleFileNameExW" => dir!(k32_get_module_file_name_ex_w),
        "K32GetModuleInformation" => dir!(k32_get_module_information),
        "K32GetProcessMemoryInfo" => dir!(k32_get_process_memory_info),
        "GetLogicalProcessorInformation" => dir!(get_logical_processor_information),
        "GetLogicalProcessorInformationEx" => dir!(get_logical_processor_information_ex),
        "GetSystemCpuSetInformation" => dir!(get_system_cpu_set_information),
        "RtlCaptureStackBackTrace" => dir!(proton_x_capture_stack_back_trace),
        "RtlPcToFileHeader" => dir!(rtl_pc_to_file_header),
        "RtlUnwind" => dir!(proton_x_rtl_unwind_ex),
        "DebugBreak" => dir!(proton_x_debug_break),
        "RaiseFailFastException" => dir!(raise_fail_fast_exception),
        "GetDiskFreeSpaceExW" => dir!(get_disk_free_space_ex_w),
        "GetDriveTypeW" => dir!(get_drive_type_w),
        "GetFileTime" => dir!(get_file_time),
        "HeapQueryInformation" => dir!(heap_query_information),
        "CancelSynchronousIo" => dir!(cancel_synchronous_io),
        "PeekNamedPipe" => dir!(peek_named_pipe),
        "ReadConsoleA" => dir!(read_console_a),
        "ReadConsoleInputA" => dir!(read_console_input_a),
        "CreateProcessA" => dir!(create_process_a),
        "OpenProcessToken" => dir!(open_process_token),
        "OpenThreadToken" => dir!(open_thread_token),
        _ => return None,
    })
}
