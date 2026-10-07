//! **Los hilos de Windows, dentro de PROTON-X** (P4, 27-09).
//!
//! BMO-X no tiene hilos de Ring 3 (el FUERO los deja para cuando haya SMP):
//! un `.exe` corre en UNA tarea. Pero un juego crea hilos antes de pintar nada,
//! asi que la casa se los da DENTRO de esa tarea, cooperativos:
//!
//! ```text
//!    cada hilo    su pila (del monton, con un canario al fondo), su TEB (gs:),
//!                 su copia del TLS estatico y sus ranuras de TlsAlloc
//!    el relevo    `proton_x_cambiar` (abajo, en ensamblador): guarda TODO lo
//!                 que Windows x64 promete que sobrevive a una llamada (rbx
//!                 rbp rdi rsi r12..r15, xmm6..xmm15, MXCSR y la palabra de
//!                 control de la x87), cambia de pila y pone el GS del otro
//!    el turno     lo decide `bmo_proton_x::hilos::Planificador` (puro, con
//!                 banco): se cede cuando un hilo ESPERA -- WaitFor* (con
//!                 plazo, aunque ya este cumplido: 05-10), Sleep, una
//!                 seccion critica ocupada, una condicion, GetMessage
//! ```
//!
//! **Por que el relevo guarda xmm6..xmm15 a mano:** la casa se compila sin SSE
//! (soft-float en Ring 3) y el compilador no sabe que esos registros existen;
//! el codigo de Microsoft del `.exe` si, y los da por conservados. Si un hilo
//! los pisa y el otro vuelve, sus floats cambiarian por debajo.
//!
//! **Lo que no hay, dicho:** un hilo que da vueltas sin esperar a nada no
//! suelta el turno (no hay reloj que se lo quite). Y si TODOS esperan algo
//! que solo otro que tambien espera podria dar, es un bloqueo mutuo: se dice
//! con quien, y el proceso sale con 0xDEAD10CC en vez de colgarse callado.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::hilos::{Objeto, Planificador, Turno, WAIT_OBJECT_0, WAIT_TIMEOUT};
use bmo_proton_x::teb;
use bmo_proton_x::tls::{self, Tls, DLL_PROCESS_ATTACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH};

use crate::{aviso, dir, kernel32, plataforma};

/// Los handles de los objetos: `OBJETO + indice`.
const OBJETO: u64 = 0x5E00_0000;
const WAIT_FAILED: u32 = 0xFFFF_FFFF;
const INFINITE: u32 = 0xFFFF_FFFF;
const STILL_ACTIVE: u32 = 259;
const CREATE_SUSPENDED: u32 = 4;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_ACCESS_DENIED: u32 = 5;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_TOO_MANY_POSTS: u32 = 298;
const ERROR_TIMEOUT: u32 = 1460;
/// Lo que sale un proceso cuyos hilos se esperan unos a otros para siempre.
pub const BLOQUEO_MUTUO: u32 = 0xDEAD_10CC;

/// La pila de un hilo si el `.exe` no dice (la reserva de Windows).
const PILA_POR_DEFECTO: usize = 1 << 20;
const PILA_MAXIMA: usize = 16 << 20;
/// El canario del fondo de cada pila: si cambia, el hilo se salio.
const CANARIO: usize = 512;
const CANARIO_BYTE: u8 = 0xC5;

/// `TlsSlots` del TEB de x64: 64 ranuras de 8 bytes.
const TEB_TLS_SLOTS: u64 = 0x1480;
const RANURAS_TLS: u32 = 64;

struct Hilo {
    teb: u64,
    /// Donde quedo su pila al ceder el turno (lo escribe el relevo).
    rsp: u64,
    /// El fondo de su pila (el canario), o 0 para el principal.
    fondo: u64,
    funcion: u64,
    arg: u64,
}

struct Casa {
    plan: Planificador,
    hilos: Vec<Hilo>,
    /// Los modulos con TLS, en el orden de su indice (P0.4b.9: el `.exe`, si
    /// tiene, el 0; luego las DLL del juego que lo tienen), y su base.
    tls: Vec<(Tls, u64)>,
    ranuras: u64,
}

struct Global(UnsafeCell<Option<Casa>>);
// SAFETY: una tarea de BMO-X; los hilos de aqui son cooperativos y nunca hay
// dos a la vez dentro de la casa. `casa()` no se guarda de un relevo a otro.
unsafe impl Sync for Global {}
static CASA: Global = Global(UnsafeCell::new(None));

fn casa() -> &'static mut Casa {
    // SAFETY: ver `Global`. Cada llamada toma y suelta; ninguna referencia
    // cruza `proton_x_cambiar`.
    let c = unsafe { &mut *CASA.0.get() };
    c.get_or_insert_with(|| Casa {
        plan: Planificador::nuevo(),
        hilos: vec![Hilo { teb: 0, rsp: 0, fondo: 0, funcion: 0, arg: 0 }],
        tls: Vec::new(),
        ranuras: 0,
    })
}

pub(crate) fn reiniciar() {
    // SAFETY: antes de saltar al `.exe` (ver `empezar`).
    unsafe { *CASA.0.get() = None };
}

fn ahora() -> u64 {
    (plataforma().ahora_ns)()
}

// -- El relevo, en ensamblador --------------------------------------------------

core::arch::global_asm!(
    ".globl proton_x_cambiar",
    "proton_x_cambiar:",
    // Lo que Windows x64 conserva, en la pila del que se va...
    "push rbp",
    "push rbx",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "push rdi",
    "push rsi",
    "sub rsp, 176",
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
    // ...su rsp donde dice rdi, y la pila del que viene (rsi)...
    "mov [rdi], rsp",
    "mov rsp, rsi",
    // ...y lo suyo, en el orden inverso.
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
    "add rsp, 176",
    "pop rsi",
    "pop rdi",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbx",
    "pop rbp",
    "ret",
    // Donde nace un hilo: el relevo "vuelve" aqui con su numero en r12 y la
    // pila alineada a 16; `call` la deja como la espera Rust.
    ".globl proton_x_arranque",
    "proton_x_arranque:",
    "mov rdi, r12",
    "call {empieza}",
    "ud2",
    empieza = sym hilo_empieza,
);

extern "C" {
    fn proton_x_cambiar(guardar: *mut u64, cargar: u64);
    fn proton_x_arranque();
}

/// Lo que el relevo saca de la pila de un hilo NUEVO: los xmm a cero con el
/// MXCSR y la x87 de Windows, los registros a cero (r12, su numero) y la
/// vuelta a `proton_x_arranque`.
const MARCO: usize = 176 + 8 * 8 + 8;
const MXCSR_WINDOWS: u32 = 0x1F80;
const X87_WINDOWS: u16 = 0x027F;

/// **Llamar al `.exe`** como `extern "win64"`, con la pila alineada a 16 y su
/// sombra de 32 bytes AUNQUE quien llama no lo este (la app de Ring 3 corre
/// desalineada 8: ver `apps/proton-x/src/main.rs`, paso 7).
///
/// # Safety
/// `f` es codigo del `.exe` con esa firma.
pub unsafe fn llamar_win64(f: u64, a: u64, b: u64, c: u64) -> u64 {
    let r: u64;
    core::arch::asm!(
        "mov r12, rsp",
        "and rsp, -16",
        "sub rsp, 32",
        "call {f}",
        "mov rsp, r12",
        f = in(reg) f,
        in("rcx") a,
        in("rdx") b,
        in("r8") c,
        out("r12") _,
        lateout("rax") r,
        clobber_abi("win64"),
    );
    r
}

fn canario_intacto(fondo: u64) -> bool {
    // SAFETY: `fondo` es el principio de una pila de la casa, de al menos
    // CANARIO bytes, viva para siempre.
    fondo == 0 || unsafe { core::slice::from_raw_parts(fondo as *const u8, CANARIO) }.iter().all(|&b| b == CANARIO_BYTE)
}

/// **Pasarle el turno a `destino`.** Vuelve cuando alguien se lo devuelva.
fn cambiar_a(destino: usize) {
    let c = casa();
    let origen = c.plan.actual;
    if destino == origen {
        return;
    }
    for h in [origen, destino] {
        if !canario_intacto(c.hilos[h].fondo) {
            aviso("un hilo se salio de su pila (el canario del fondo cambio): no se sigue");
            (plataforma().salir)(0xC00000FD);
        }
    }
    c.plan.actual = destino;
    SONDEOS.store(0, core::sync::atomic::Ordering::Relaxed);
    let (guardar, cargar, gs) = (&mut c.hilos[origen].rsp as *mut u64, c.hilos[destino].rsp, c.hilos[destino].teb);
    (plataforma().poner_gs)(gs);
    // SAFETY: `cargar` es la pila guardada de otro hilo de la casa (o su marco
    // de nacimiento); `guardar`, el sitio de este. Ninguna referencia a la
    // casa cruza el relevo (`c` no se usa despues).
    unsafe { proton_x_cambiar(guardar, cargar) };
}

/// **Esperar el turno**: el hilo actual ya dejo dicho en el planificador a
/// que espera (o esta listo y cede). Vuelve cuando le toca otra vez.
fn bloquear() {
    loop {
        // El latido del sonido (N4.5): lo convertido, al anillo; y el evento
        // de un flujo de WASAPI, si ya cabe un periodo. Antes de elegir: su
        // hilo puede ser el que toca.
        crate::wasapi_flujo::latir();
        let t = {
            let c = casa();
            c.plan.siguiente(ahora())
        };
        match t {
            Turno::Hilo(h) => {
                cambiar_a(h);
                return;
            }
            Turno::Esperar(_) => {
                // El pulso (02-10): todos esperan; si el `.exe` se queda
                // aqui, la foto dice a que.
                crate::pulso::latido();
                (plataforma().dormir)()
            }
            // Todos esperan sin plazo, pero un flujo de WASAPI corre: su
            // evento se encendera cuando suene lo que tiene. No es un bloqueo.
            Turno::Bloqueo if crate::wasapi_flujo::alguno_corre() => {
                crate::pulso::latido();
                (plataforma().dormir)()
            }
            Turno::Bloqueo => {
                aviso("todos los hilos esperan algo que solo otro que tambien espera podria dar: bloqueo mutuo");
                (plataforma().salir)(BLOQUEO_MUTUO);
            }
        }
    }
}

/// **Ceder el turno** si hay otro hilo que pueda seguir. `true` si se cedio.
/// Es lo que hace GetMessage sin mensajes antes de dormir.
pub(crate) fn ceder() -> bool {
    crate::wasapi_flujo::latir();
    let (t, yo) = {
        let c = casa();
        if c.hilos.len() == 1 {
            return false;
        }
        (c.plan.siguiente(ahora()), c.plan.actual)
    };
    match t {
        Turno::Hilo(h) if h != yo => {
            cambiar_a(h);
            true
        }
        _ => false,
    }
}

/// Cada cuanto lo largo mira si el hilo del sonido ya puede seguir (T1).
const RESPIRO_NS: u64 = 1_000_000;
static ULTIMO_RESPIRO: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
/// Cuantas veces lo largo le presto el turno al sonido (lo dice el pulso).
static PRESTAMOS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// **Respirar a mitad de lo largo** (T1 y T2 de `PLAN_LOS_DOCE_DIRECTORES`,
/// 07-10). Lo llama lo que tarda sin esperar nada -- cada orden de una lista
/// de D3D12, cada PSO creado -- en un punto donde no tiene nada de la casa
/// a medias. Como mucho cada [`RESPIRO_NS`]: el latido del sonido (lo
/// convertido, al anillo; y el evento del flujo, si cabe un periodo); y si
/// el hilo que espera ese evento ya puede seguir, le PRESTA el turno. El
/// turno vuelve aqui en cuanto ese hilo espere otra vez
/// (`Planificador::prestado`): ningun otro hilo del juego corre entre medias.
///
/// Cyberpunk en el metal (06-10): 96 cortes de sonido, uno de 33 s, porque
/// el hilo del sonido solo corria cuando el que dibujaba esperaba algo.
pub fn respirar() {
    use core::sync::atomic::Ordering;
    let t = ahora();
    if t.saturating_sub(ULTIMO_RESPIRO.load(Ordering::Relaxed)) < RESPIRO_NS {
        return;
    }
    ULTIMO_RESPIRO.store(t, Ordering::Relaxed);
    if casa().hilos.len() == 1 {
        return;
    }
    crate::wasapi_flujo::latir();
    let u = {
        let c = casa();
        let u = c.plan.urgente(t);
        if u.is_some() {
            c.plan.prestado = Some(c.plan.actual);
        }
        u
    };
    if let Some(u) = u {
        PRESTAMOS.fetch_add(1, Ordering::Relaxed);
        cambiar_a(u);
    }
}

/// **El evento `h` es del sonido** (`IAudioClient::SetEventHandle`): quien
/// lo espere es el hilo del sonido, y [`respirar`] le presta el turno.
pub(crate) fn del_sonido(h: u64) {
    if let Some(o) = objeto_de(h) {
        casa().plan.del_sonido(o);
    }
}

/// Cuantas veces lo largo le presto el turno al sonido ([`respirar`]).
pub fn prestamos() -> u64 {
    PRESTAMOS.load(core::sync::atomic::Ordering::Relaxed)
}

/// `SetThreadPriority` (T2, 07-10): se guarda (ver `Planificador::urgente`:
/// no presta el turno). `false` si no es un hilo.
pub(crate) fn poner_prioridad(h: u64, p: i32) -> bool {
    match hilo_de(h) {
        Some(n) => casa().plan.poner_prioridad(n, p),
        None => false,
    }
}

/// `GetThreadPriority`: la que se puso, o `None` si no es un hilo.
pub(crate) fn prioridad(h: u64) -> Option<i32> {
    casa().plan.prioridad(hilo_de(h)?)
}

// -- El TLS -------------------------------------------------------------------------

/// Un bloque de TLS estatico para un hilo, y su tabla de un modulo; lo que va
/// en `TEB+0x58`.
/// La tabla de TLS de un hilo: un bloque por modulo, en el orden del indice.
fn bloque_tls(ts: &[(Tls, u64)]) -> u64 {
    let tabla: Vec<u64> = ts.iter().map(|(t, _)| Box::leak(t.bloque().into_boxed_slice()).as_mut_ptr() as u64).collect();
    Box::leak(tabla.into_boxed_slice()).as_ptr() as u64
}

/// Los callbacks de TLS de los modulos (todos, o solo el de `base`).
fn llamar_callbacks_de(motivo: u32, solo: Option<u64>) {
    let cbs: Vec<(u64, u64)> = casa().tls.iter().filter(|(_, b)| solo.is_none_or(|s| s == *b)).flat_map(|(t, b)| t.callbacks.iter().map(move |f| (*f, *b))).collect();
    for (f, base) in cbs {
        // SAFETY: los callbacks del directorio de TLS de un modulo cargado,
        // que caen en su imagen (`tls::leer` lo comprueba), con la firma de
        // Windows.
        unsafe { llamar_win64(f, base, motivo as u64, 0) };
    }
}

fn llamar_callbacks(motivo: u32) {
    llamar_callbacks_de(motivo, None);
}

/// **El TLS de una DLL del juego** (P0.4b.9, 30-09): `libxess_fg.dll` de
/// Cyberpunk lo tiene, y su DllMain leyo `0+0x8` porque la tabla del hilo
/// solo tenia el bloque del `.exe`. Como el cargador de Windows con las DLL
/// de arranque: cada una su indice (detras del `.exe`), su bloque en cada
/// hilo, y sus callbacks justo antes de su DllMain ([`tls_de_dll_attach`]).
/// Se registran ANTES de [`preparar_tls`].
pub fn registrar_tls_dll(t: Tls, base: u64) {
    casa().tls.push((t, base));
}

/// Los callbacks de TLS de la DLL de `base`, con PROCESS_ATTACH.
pub(crate) fn tls_de_dll_attach(base: u64) {
    llamar_callbacks_de(DLL_PROCESS_ATTACH, Some(base));
}

/// **El TLS del proceso, antes de saltar a la entrada**: el bloque del hilo
/// principal en `TEB+0x58` (un bloque por modulo con TLS: el `.exe` el 0, y
/// las DLL registradas con [`registrar_tls_dll`] detras), el indice de cada
/// uno en su sitio, y los callbacks del `.exe` con PROCESS_ATTACH.
///
/// # Safety
/// Con el GS ya en el TEB del hilo principal, `empezar` hecho y la imagen
/// colocada en `base`, antes de saltar.
pub unsafe fn preparar_tls(t: Option<Tls>, base: u64) {
    let c = casa();
    if let Some(t) = t {
        c.tls.insert(0, (t, base));
    }
    if c.tls.is_empty() {
        return;
    }
    for (i, (t, b)) in c.tls.iter().enumerate() {
        ((b + t.indice_rva as u64) as *mut u32).write(i as u32);
    }
    let teb = kernel32::teb();
    ((teb + tls::TEB_TLS_POINTER as u64) as *mut u64).write(bloque_tls(&c.tls));
    // Los del `.exe` ahora; los de cada DLL, antes de su DllMain.
    if c.tls[0].1 == base {
        llamar_callbacks_de(DLL_PROCESS_ATTACH, Some(base));
    }
}

// -- Los objetos ----------------------------------------------------------------------

// -- Lo que usan las esperas de P4f2 (`esperas.rs`) ------------------------------

/// El hilo que tiene el turno.
pub(crate) fn actual() -> usize {
    casa().plan.actual
}

/// El planificador, un momento (sin ceder dentro).
pub(crate) fn con_plan<R>(f: impl FnOnce(&mut Planificador) -> R) -> R {
    f(&mut casa().plan)
}

/// Un objeto nuevo y su handle.
pub(crate) fn nuevo_handle(o: Objeto) -> u64 {
    handle(casa().plan.nuevo_objeto(o))
}

/// El objeto de un handle vivo.
pub(crate) fn objeto(h: u64) -> Option<usize> {
    objeto_de(h)
}

/// Dormir en la direccion `dir` (WaitOnAddress): `true` si la despertaron.
pub(crate) fn esperar_en(dir: u64, ms: u32) -> bool {
    dormir_en(dir, ms)
}

pub(crate) fn ahora_ns() -> u64 {
    ahora()
}

fn objeto_de(h: u64) -> Option<usize> {
    let o = h.checked_sub(OBJETO)? as usize;
    matches!(casa().plan.objeto(o), Some(x) if x != Objeto::Cerrado).then_some(o)
}

fn handle(o: usize) -> u64 {
    OBJETO + o as u64
}

/// Un evento MANUAL ya encendido (tanda 47: la espera de latencia de una
/// cadena de intercambio, que nunca hace esperar: la cola es sincrona).
pub(crate) fn evento_encendido() -> u64 {
    handle(casa().plan.nuevo_objeto(Objeto::Evento { manual: true, encendido: true }))
}

/// Un evento AUTOMATICO apagado, de la casa (E2.1: lo que espera
/// `SetEventOnCompletion` sin evento).
pub(crate) fn evento_nuevo() -> u64 {
    handle(casa().plan.nuevo_objeto(Objeto::Evento { manual: false, encendido: false }))
}

/// Esperar a un evento de la casa sin plazo (cediendo el turno) y cerrarlo.
pub(crate) fn esperar_y_cerrar(h: u64) {
    wait_for_single_object(h, INFINITE);
    close_handle(h);
}

/// Encender un evento (lo usa la valla de D3D12).
pub(crate) fn encender_evento(h: u64) {
    if let Some(o) = objeto_de(h) {
        casa().plan.encender(o, true);
    }
}

extern "win64" fn create_event_w(_attr: u64, manual: i32, inicial: i32, _nombre: *const u16) -> u64 {
    handle(casa().plan.nuevo_objeto(Objeto::Evento { manual: manual != 0, encendido: inicial != 0 }))
}

extern "win64" fn set_event(h: u64) -> i32 {
    objeto_de(h).is_some_and(|o| casa().plan.encender(o, true)) as i32
}

extern "win64" fn reset_event(h: u64) -> i32 {
    objeto_de(h).is_some_and(|o| casa().plan.encender(o, false)) as i32
}

extern "win64" fn create_semaphore_w(_attr: u64, inicial: i32, max: i32, _nombre: *const u16) -> u64 {
    if max <= 0 || inicial < 0 || inicial > max {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    handle(casa().plan.nuevo_objeto(Objeto::Semaforo { cuenta: inicial as u32, max: max as u32 }))
}

extern "win64" fn release_semaphore(h: u64, n: i32, antes: *mut i32) -> i32 {
    let r = objeto_de(h).and_then(|o| if n > 0 { casa().plan.soltar_semaforo(o, n as u32) } else { None });
    match r {
        Some(a) => {
            if !antes.is_null() {
                // SAFETY: un LONG del `.exe`.
                unsafe { *antes = a as i32 };
            }
            1
        }
        None => {
            kernel32::poner_error(ERROR_TOO_MANY_POSTS);
            0
        }
    }
}

/// `CloseHandle`: un objeto de la casa se cierra; la consola, que no es de
/// nadie, dice que si.
pub(crate) extern "win64" fn close_handle(h: u64) -> i32 {
    // P4f2: un handle duplicado se cierra una vez por copia.
    if crate::esperas::soltar_copia(h) {
        return 1;
    }
    if crate::ficheros::es_fichero(h) {
        return crate::ficheros::cerrar(h);
    }
    // Tanda 3, paso 4b: los mapeos y los puertos de finalizacion.
    if crate::kernel32_procesos::es_suyo(h) {
        return crate::kernel32_procesos::cerrar(h);
    }
    if crate::kernel32_mapeo::es_suyo(h) {
        return crate::kernel32_mapeo::cerrar(h);
    }
    match objeto_de(h) {
        Some(o) => casa().plan.cerrar(o) as i32,
        None => 1,
    }
}

/// **`WaitFor*`**: si se cumple ya, en el acto; si no, se cede el turno hasta
/// que se cumpla o venza el plazo.
fn esperar(hs: &[u64], todos: bool, ms: u32) -> u32 {
    let mut objetos = Vec::with_capacity(hs.len());
    for &h in hs {
        match objeto_de(h) {
            Some(o) => objetos.push(o),
            None => {
                aviso("WaitFor* sobre algo que no es un objeto de la casa");
                return WAIT_FAILED;
            }
        }
    }
    let plazo = (ms != INFINITE).then(|| ahora() + ms as u64 * 1_000_000);
    let r = {
        let c = casa();
        c.plan.esperar(&objetos, todos, plazo, ahora())
    };
    if let Some(r) = r {
        // E2.3b (05-10): una espera DE VERDAD (con plazo) que ya esta
        // cumplida cede el turno, si otro hilo puede seguir. En la casa las
        // vallas de D3D12 se cumplen siempre al momento (la cola es
        // sincrona): el hilo de computo de nBodyGravity espera su valla, la
        // encuentra cumplida y vuelve a calcular, y el de dibujo no correria
        // nunca. En Windows ese hilo se dormiria lo que tarda la tarjeta. El
        // resultado ya esta decidido (y gastado, si era automatico): ceder
        // aqui es lo que Windows hace cuando le quita el turno al volver.
        // Esperar 0 (preguntar) no cede.
        if ms != 0 {
            ceder();
        }
        return r;
    }
    bloquear();
    let c = casa();
    c.plan.resultado(c.plan.actual)
}

pub(crate) extern "win64" fn wait_for_single_object(h: u64, ms: u32) -> u32 {
    esperar(&[h], false, ms)
}

/// Alertable (30-09): corre las APC de este hilo (QueueUserAPC) y lo dice
/// con WAIT_IO_COMPLETION.
extern "win64" fn wait_for_single_object_ex(h: u64, ms: u32, alertable: i32) -> u32 {
    if alertable == 0 {
        return esperar(&[h], false, ms);
    }
    crate::kernel32_procesos::espera_alertable(ms, |t| esperar(&[h], false, t))
}

pub(crate) extern "win64" fn wait_for_multiple_objects(n: u32, hs: *const u64, todos: i32, ms: u32) -> u32 {
    if n == 0 || n > 64 || hs.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return WAIT_FAILED;
    }
    // SAFETY: `n` handles del `.exe`.
    let v = unsafe { core::slice::from_raw_parts(hs, n as usize) }.to_vec();
    esperar(&v, todos != 0, ms)
}

pub(crate) extern "win64" fn sleep(ms: u32) {
    if ms == 0 {
        ceder();
        return;
    }
    let hasta = ahora() + ms as u64 * 1_000_000;
    casa().plan.dormir(hasta);
    bloquear();
}

extern "win64" fn sleep_ex(ms: u32, alertable: i32) -> u32 {
    if alertable == 0 {
        sleep(ms);
        return 0;
    }
    match crate::kernel32_procesos::espera_alertable(ms, |t| {
        sleep(t);
        WAIT_TIMEOUT
    }) {
        WAIT_TIMEOUT => 0,
        r => r,
    }
}

pub(crate) extern "win64" fn switch_to_thread() -> i32 {
    ceder() as i32
}

// -- Los hilos --------------------------------------------------------------------------

/// `CreateThread(attr, pila, funcion, arg, banderas, *id)`.
pub(crate) extern "win64" fn create_thread(_attr: u64, pila: usize, funcion: u64, arg: u64, banderas: u32, id: *mut u32) -> u64 {
    if funcion == 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let principal = kernel32::teb();
    let c = casa();
    if c.hilos[0].teb == 0 {
        c.hilos[0].teb = principal;
    }
    // La pila: del monton, alineada, con el canario al fondo.
    let bytes = if pila == 0 { PILA_POR_DEFECTO } else { pila.clamp(64 << 10, PILA_MAXIMA) };
    let mem = Box::leak(vec![0u8; bytes + 16].into_boxed_slice());
    let fondo = (mem.as_mut_ptr() as u64 + 15) & !15;
    let tope = (fondo + bytes as u64) & !15;
    // SAFETY: `fondo..tope` es la pila recien pedida, nuestra.
    unsafe { core::ptr::write_bytes(fondo as *mut u8, CANARIO_BYTE, CANARIO) };
    let (n, obj) = c.plan.crear(banderas & CREATE_SUSPENDED != 0);
    // El TEB: el PEB, el proceso y la base son los del principal; el id, suyo.
    let Ok(forma) = core::alloc::Layout::from_size_align(teb::TEB_BYTES, 4096) else { return 0 };
    // SAFETY: `forma` no mide cero.
    let t = unsafe { alloc::alloc::alloc_zeroed(forma) };
    if t.is_null() {
        return 0;
    }
    // SAFETY: el TEB del principal es de este proceso y R+W.
    let (peb, proceso) = unsafe { (((principal + teb::TEB_PEB as u64) as *const u64).read(), ((principal + teb::TEB_PROCESS_ID as u64) as *const u64).read()) };
    // SAFETY: el PEB del proceso.
    let base_imagen = unsafe { ((peb + teb::PEB_IMAGE_BASE as u64) as *const u64).read() };
    let tid = 0x1000 + 4 * n as u64;
    let h = teb::Hilo { teb: t as u64, peb, pila_tope: tope, pila_fondo: fondo + CANARIO as u64, proceso, hilo: tid, base_imagen };
    // SAFETY: TEB_BYTES recien pedidos.
    teb::escribir_teb(unsafe { core::slice::from_raw_parts_mut(t, teb::TEB_BYTES) }, &h);
    if !c.tls.is_empty() {
        // SAFETY: el TEB nuevo, R+W.
        unsafe { ((t as u64 + tls::TEB_TLS_POINTER as u64) as *mut u64).write(bloque_tls(&c.tls)) };
    }
    // Su marco de nacimiento (ver MARCO): lo que `proton_x_cambiar` desapila.
    let rsp = tope - MARCO as u64;
    // SAFETY: `rsp..tope` cae dentro de la pila nueva.
    unsafe {
        core::ptr::write_bytes(rsp as *mut u8, 0, MARCO);
        ((rsp + 160) as *mut u32).write(MXCSR_WINDOWS);
        ((rsp + 164) as *mut u16).write(X87_WINDOWS);
        // pop rsi, rdi, r15, r14, r13, r12: r12 es el sexto.
        ((rsp + 176 + 5 * 8) as *mut u64).write(n as u64);
        ((rsp + 176 + 8 * 8) as *mut u64).write(proton_x_arranque as *const () as usize as u64);
    }
    c.hilos.push(Hilo { teb: t as u64, rsp, fondo, funcion, arg });
    if !id.is_null() {
        // SAFETY: un DWORD del `.exe`.
        unsafe { *id = tid as u32 };
    }
    handle(obj)
}

/// **Donde empieza un hilo**, ya en su pila y con su GS: los callbacks del
/// TLS con THREAD_ATTACH, su funcion, y ExitThread con lo que devuelva.
extern "C" fn hilo_empieza(n: u64) -> ! {
    llamar_callbacks(DLL_THREAD_ATTACH);
    let (f, a) = {
        let h = &casa().hilos[n as usize];
        (h.funcion, h.arg)
    };
    // SAFETY: la funcion que el `.exe` dio a CreateThread: DWORD WINAPI f(void*).
    let r = unsafe { llamar_win64(f, a, 0, 0) } as u32;
    exit_thread(r)
}

extern "win64" fn exit_thread(codigo: u32) -> ! {
    // P4f2: los callbacks de FLS de este hilo, y luego los del TLS.
    crate::esperas::fls_al_salir();
    llamar_callbacks(DLL_THREAD_DETACH);
    let vivos = {
        let c = casa();
        let a = c.plan.actual;
        c.plan.terminar(a, codigo);
        c.plan.vivos()
    };
    if vivos == 0 {
        (plataforma().salir)(codigo);
    }
    bloquear();
    // Un hilo terminado no vuelve a tener el turno.
    (plataforma().salir)(codigo)
}

/// El numero de hilo de un handle (o del pseudo-handle de GetCurrentThread).
fn hilo_de(h: u64) -> Option<usize> {
    if h == u64::MAX - 1 {
        return Some(casa().plan.actual);
    }
    match casa().plan.objeto(objeto_de(h)?) {
        Some(Objeto::Hilo(n)) => Some(n),
        _ => None,
    }
}

/// `GetThreadId(h)`: el id (de Windows) del hilo de un handle, tambien el
/// pseudo-handle de GetCurrentThread; 0 y ERROR_INVALID_HANDLE si no es de
/// un hilo (E1.1 de la ESCALERA, 05-10: lo importa el `.exe` de
/// `D3D12HelloWindow` compilado con mingw-w64).
extern "win64" fn get_thread_id(h: u64) -> u32 {
    id_de(h).unwrap_or_else(|| {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        0
    })
}

pub(crate) extern "win64" fn get_exit_code_thread(h: u64, codigo: *mut u32) -> i32 {
    let Some(n) = hilo_de(h) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    if !codigo.is_null() {
        // SAFETY: un DWORD del `.exe`.
        unsafe { *codigo = casa().plan.salida(n).unwrap_or(STILL_ACTIVE) };
    }
    1
}

extern "win64" fn resume_thread(h: u64) -> u32 {
    hilo_de(h).and_then(|n| casa().plan.reanudar(n)).unwrap_or(u32::MAX)
}

extern "win64" fn get_current_thread() -> u64 {
    u64::MAX - 1
}

/// `SuspendThread` (30-09): la cuenta de antes, o u32::MAX. Suspenderse a si
/// mismo cede el turno ya (y se sigue cuando otro lo reanude).
pub(crate) fn suspender(h: u64) -> u32 {
    let Some(n) = hilo_de(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return u32::MAX;
    };
    let (r, yo) = {
        let c = casa();
        (c.plan.suspender(n), c.plan.actual == n)
    };
    let Some(r) = r else {
        kernel32::poner_error(ERROR_ACCESS_DENIED);
        return u32::MAX;
    };
    if yo {
        bloquear();
    }
    r
}

/// `TerminateThread` (30-09): el hilo acaba YA, sin sus callbacks (como en
/// Windows); a si mismo, no vuelve.
pub(crate) fn terminar(h: u64, codigo: u32) -> bool {
    let Some(n) = hilo_de(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return false;
    };
    let (yo, vivos) = {
        let c = casa();
        c.plan.terminar(n, codigo);
        (c.plan.actual == n, c.plan.vivos())
    };
    if vivos == 0 {
        (plataforma().salir)(codigo);
    }
    if yo {
        bloquear();
    }
    true
}

/// El id (de Windows) del hilo de un handle.
pub(crate) fn id_de(h: u64) -> Option<u32> {
    let n = hilo_de(h)?;
    let yo = kernel32::get_current_thread_id();
    let c = casa();
    match c.hilos.get(n) {
        // SAFETY: el TEB de un hilo de la casa, vivo.
        Some(x) if x.teb != 0 => Some(unsafe { ((x.teb + teb::TEB_THREAD_ID as u64) as *const u64).read() as u32 }),
        _ if n == c.plan.actual => Some(yo),
        _ => None,
    }
}

/// Los ids (de Windows) de los hilos vivos, del principal al ultimo.
pub(crate) fn ids_vivos() -> Vec<u32> {
    let yo = kernel32::get_current_thread_id();
    let c = casa();
    let mut v = Vec::new();
    for (n, h) in c.hilos.iter().enumerate() {
        if matches!(c.plan.estado(n), Some(bmo_proton_x::hilos::Estado::Terminado(_)) | None) {
            continue;
        }
        let id = if h.teb != 0 {
            // SAFETY: el TEB de un hilo de la casa, vivo.
            unsafe { ((h.teb + teb::TEB_THREAD_ID as u64) as *const u64).read() as u32 }
        } else if n == c.plan.actual {
            yo
        } else {
            continue;
        };
        v.push(id);
    }
    v
}

/// El id del hilo principal (su TEB no es de la casa: lo pone el cargador),
/// visto la ultima vez que el pulso corrio en el.
static PRINCIPAL: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// **Cada hilo, en una linea** (el pulso, 02-10): su id, su funcion de
/// arranque, en que esta (a que espera, con el objeto y como esta) y su
/// ultima llamada que queda en el anillo del diario.
pub(crate) fn describir() -> Vec<alloc::string::String> {
    use bmo_proton_x::hilos::Estado as E;
    use core::sync::atomic::Ordering;
    let yo = kernel32::get_current_thread_id();
    let c = casa();
    if c.plan.actual == 0 {
        PRINCIPAL.store(yo, Ordering::Relaxed);
    }
    let t = ahora();
    let ms = |hasta: u64| hasta.saturating_sub(t) / 1_000_000;
    let mut v = Vec::new();
    for (n, h) in c.hilos.iter().enumerate() {
        let id = if n == c.plan.actual {
            yo
        } else if h.teb != 0 {
            // SAFETY: el TEB de un hilo de la casa, vivo.
            unsafe { ((h.teb + teb::TEB_THREAD_ID as u64) as *const u64).read() as u32 }
        } else {
            PRINCIPAL.load(Ordering::Relaxed)
        };
        let quien = if n == 0 { alloc::string::String::from("principal") } else { crate::pulso::donde(h.funcion) };
        let objeto = |o: usize| alloc::format!("objeto {o} = {:?}", c.plan.objeto(o));
        let que = match c.plan.estado(n) {
            Some(E::Listo) if n == c.plan.actual => alloc::string::String::from("CORRIENDO"),
            Some(E::Listo) => alloc::string::String::from("listo"),
            Some(E::Espera { objetos, todos, plazo }) => {
                let lista: Vec<_> = objetos.iter().map(|&o| objeto(o)).collect();
                let plazo = plazo.map_or(alloc::string::String::from("sin plazo"), |p| alloc::format!("le quedan {} ms", ms(p)));
                alloc::format!("ESPERA ({}, {plazo}) a {}", if *todos { "todos" } else { "cualquiera" }, lista.join(", "))
            }
            Some(E::Dormido { hasta }) => alloc::format!("Sleep, le quedan {} ms", ms(*hasta)),
            Some(E::Suspendido(k)) => alloc::format!("suspendido ({k})"),
            Some(E::Cerrojo { dir, exclusivo }) => alloc::format!("ESPERA un cerrojo en {dir:#x} ({}; lo tiene: {:?})", if *exclusivo { "exclusivo" } else { "compartido" }, c.plan.ver_cerrojo(*dir)),
            Some(E::Condicion { dir, plazo }) => alloc::format!("ESPERA una condicion en {dir:#x} ({})", plazo.map_or(alloc::string::String::from("sin plazo"), |p| alloc::format!("le quedan {} ms", ms(p)))),
            Some(E::Terminado(k)) => alloc::format!("terminado ({k:#x})"),
            None => alloc::string::String::from("?"),
        };
        let ultima = crate::diario::ultima_de(id).unwrap_or_else(|| alloc::string::String::from("(ninguna en el anillo)"));
        v.push(alloc::format!("{id:>5} [{quien}] {que}; ultima: {ultima}"));
    }
    v
}

// -- TLS dinamico ------------------------------------------------------------------------

extern "win64" fn tls_alloc() -> u32 {
    let c = casa();
    let libre = (!c.ranuras).trailing_zeros();
    if libre >= RANURAS_TLS {
        return u32::MAX; // TLS_OUT_OF_INDEXES
    }
    c.ranuras |= 1 << libre;
    libre
}

extern "win64" fn tls_free(i: u32) -> i32 {
    let c = casa();
    if i >= RANURAS_TLS || c.ranuras & (1 << i) == 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    c.ranuras &= !(1 << i);
    // Como Windows: la ranura vuelve a 0 en TODOS los hilos.
    let tebs: Vec<u64> = c.hilos.iter().map(|h| h.teb).filter(|&t| t != 0).chain(core::iter::once(kernel32::teb())).collect();
    for t in tebs {
        // SAFETY: TEBs de este proceso; la ranura cae dentro.
        unsafe { ((t + TEB_TLS_SLOTS + 8 * i as u64) as *mut u64).write(0) };
    }
    1
}

extern "win64" fn tls_get_value(i: u32) -> u64 {
    if i >= RANURAS_TLS {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    // Windows pone LastError a 0 en el exito: un 0 guardado no es un error.
    kernel32::poner_error(0);
    // SAFETY: el TEB de este hilo; la ranura cae dentro.
    unsafe { ((kernel32::teb() + TEB_TLS_SLOTS + 8 * i as u64) as *const u64).read() }
}

extern "win64" fn tls_set_value(i: u32, v: u64) -> i32 {
    if i >= RANURAS_TLS {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    // SAFETY: como arriba.
    unsafe { ((kernel32::teb() + TEB_TLS_SLOTS + 8 * i as u64) as *mut u64).write(v) };
    1
}

// -- Secciones criticas, SRW y condiciones -------------------------------------------------

fn entrar(dir: u64, exclusivo: bool, recursivo: bool) {
    if !casa().plan.entrar(dir, exclusivo, recursivo) {
        bloquear();
    }
}

fn salir(dir: u64, exclusivo: bool) {
    if !casa().plan.salir(dir, exclusivo) {
        aviso("Leave/Release de un cerrojo que este hilo no tiene");
    }
}

extern "win64" fn initialize_critical_section(_cs: u64) {}

extern "win64" fn initialize_critical_section_and_spin_count(_cs: u64, _vueltas: u32) -> i32 {
    1
}

extern "win64" fn initialize_critical_section_ex(_cs: u64, _vueltas: u32, _banderas: u32) -> i32 {
    1
}

pub(crate) extern "win64" fn enter_critical_section(cs: u64) {
    entrar(cs, true, true);
}

pub(crate) extern "win64" fn try_enter_critical_section(cs: u64) -> i32 {
    casa().plan.probar(cs, true, true) as i32
}

pub(crate) extern "win64" fn leave_critical_section(cs: u64) {
    salir(cs, true);
}

extern "win64" fn delete_critical_section(_cs: u64) {}

extern "win64" fn initialize_srw_lock(_l: u64) {}

extern "win64" fn acquire_srw_lock_exclusive(l: u64) {
    entrar(l, true, false);
}

extern "win64" fn acquire_srw_lock_shared(l: u64) {
    entrar(l, false, false);
}

extern "win64" fn try_acquire_srw_lock_exclusive(l: u64) -> u8 {
    casa().plan.probar(l, true, false) as u8
}

extern "win64" fn try_acquire_srw_lock_shared(l: u64) -> u8 {
    casa().plan.probar(l, false, false) as u8
}

extern "win64" fn release_srw_lock_exclusive(l: u64) {
    salir(l, true);
}

extern "win64" fn release_srw_lock_shared(l: u64) {
    salir(l, false);
}

extern "win64" fn initialize_condition_variable(_c: u64) {}

/// Dormir en la condicion `cv` hasta que la despierten o venza `ms`: `true`
/// si la despertaron. El cerrojo lo suelta y lo vuelve a coger quien llama.
fn dormir_en(cv: u64, ms: u32) -> bool {
    let plazo = (ms != INFINITE).then(|| ahora() + ms as u64 * 1_000_000);
    casa().plan.dormir_en(cv, plazo);
    bloquear();
    let c = casa();
    c.plan.resultado(c.plan.actual) == WAIT_OBJECT_0
}

pub(crate) extern "win64" fn sleep_condition_variable_cs(cv: u64, cs: u64, ms: u32) -> i32 {
    let r = casa().plan.soltar_todo(cs);
    let despierto = dormir_en(cv, ms);
    entrar(cs, true, true);
    casa().plan.poner_recursion(cs, r);
    if !despierto {
        kernel32::poner_error(ERROR_TIMEOUT);
    }
    despierto as i32
}

/// `CONDITION_VARIABLE_LOCKMODE_SHARED`.
const COMPARTIDO: u32 = 1;

extern "win64" fn sleep_condition_variable_srw(cv: u64, l: u64, ms: u32, banderas: u32) -> i32 {
    let exclusivo = banderas & COMPARTIDO == 0;
    salir(l, exclusivo);
    let despierto = dormir_en(cv, ms);
    entrar(l, exclusivo, false);
    if !despierto {
        kernel32::poner_error(ERROR_TIMEOUT);
    }
    despierto as i32
}

pub(crate) extern "win64" fn wake_condition_variable(cv: u64) {
    casa().plan.despertar(cv, false);
}

pub(crate) extern "win64" fn wake_all_condition_variable(cv: u64) {
    casa().plan.despertar(cv, true);
}

// -- La hora -------------------------------------------------------------------------------

/// Cuantas veces seguidas el hilo que corre ha preguntado la hora sin ceder.
static SONDEOS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Cada cuantas preguntas de la hora seguidas se cede el turno.
const SONDEOS_POR_TURNO: u32 = 64;

/// **Preguntar la hora tambien suelta el turno, de vez en cuando** (02-10).
///
/// Un motor espera muchas veces a otro hilo DANDO VUELTAS y mirando el
/// reloj (`while (QueryPerformanceCounter() < fin) {}`, o hasta que el otro
/// ponga una bandera). En Windows el reloj del sistema le quita el turno y el
/// otro hilo corre; aqui los hilos son cooperativos y no hay quien se lo
/// quite: el que da vueltas no acaba nunca y el otro no empieza (Cyberpunk
/// en el metal, 02-10: la CPU al 100 % y nada nuevo tras un
/// ReleaseSemaphore). Cada [`SONDEOS_POR_TURNO`] preguntas seguidas de la
/// misma hora se cede, si hay otro listo: el reloj sigue corriendo igual.
fn sondeo() {
    use core::sync::atomic::Ordering;
    if SONDEOS.fetch_add(1, Ordering::Relaxed) + 1 >= SONDEOS_POR_TURNO {
        SONDEOS.store(0, Ordering::Relaxed);
        ceder();
    }
}

/// La hora para el `.exe`: [`sondeo`] y la de la casa.
pub(crate) fn ahora_del_exe() -> u64 {
    sondeo();
    ahora()
}

extern "win64" fn get_tick_count() -> u32 {
    (ahora_del_exe() / 1_000_000) as u32
}

extern "win64" fn get_tick_count64() -> u64 {
    ahora_del_exe() / 1_000_000
}

/// El contador de rendimiento de la casa cuenta NANOSEGUNDOS.
pub(crate) extern "win64" fn query_performance_counter(v: *mut i64) -> i32 {
    if v.is_null() {
        return 0;
    }
    let t = ahora_del_exe();
    // SAFETY: un LARGE_INTEGER del `.exe`.
    unsafe { *v = t as i64 };
    1
}

extern "win64" fn query_performance_frequency(v: *mut i64) -> i32 {
    if v.is_null() {
        return 0;
    }
    // SAFETY: como arriba.
    unsafe { *v = 1_000_000_000 };
    1
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "CreateEventW" | "CreateEventA" => dir!(create_event_w),
        "SetEvent" => dir!(set_event),
        "ResetEvent" => dir!(reset_event),
        "CreateSemaphoreW" | "CreateSemaphoreA" => dir!(create_semaphore_w),
        "ReleaseSemaphore" => dir!(release_semaphore),
        "CloseHandle" => dir!(close_handle),
        "WaitForSingleObject" => dir!(wait_for_single_object),
        "WaitForSingleObjectEx" => dir!(wait_for_single_object_ex),
        "WaitForMultipleObjects" => dir!(wait_for_multiple_objects),
        "Sleep" => dir!(sleep),
        "SleepEx" => dir!(sleep_ex),
        "SwitchToThread" => dir!(switch_to_thread),
        "CreateThread" => dir!(create_thread),
        "ExitThread" => dir!(exit_thread),
        "GetExitCodeThread" => dir!(get_exit_code_thread),
        "GetThreadId" => dir!(get_thread_id),
        "ResumeThread" => dir!(resume_thread),
        "GetCurrentThread" => dir!(get_current_thread),
        "TlsAlloc" => dir!(tls_alloc),
        "TlsFree" => dir!(tls_free),
        "TlsGetValue" => dir!(tls_get_value),
        "TlsSetValue" => dir!(tls_set_value),
        "InitializeCriticalSection" => dir!(initialize_critical_section),
        "InitializeCriticalSectionAndSpinCount" => dir!(initialize_critical_section_and_spin_count),
        "InitializeCriticalSectionEx" => dir!(initialize_critical_section_ex),
        "EnterCriticalSection" => dir!(enter_critical_section),
        "TryEnterCriticalSection" => dir!(try_enter_critical_section),
        "LeaveCriticalSection" => dir!(leave_critical_section),
        "DeleteCriticalSection" => dir!(delete_critical_section),
        "InitializeSRWLock" => dir!(initialize_srw_lock),
        "AcquireSRWLockExclusive" => dir!(acquire_srw_lock_exclusive),
        "AcquireSRWLockShared" => dir!(acquire_srw_lock_shared),
        "TryAcquireSRWLockExclusive" => dir!(try_acquire_srw_lock_exclusive),
        "TryAcquireSRWLockShared" => dir!(try_acquire_srw_lock_shared),
        "ReleaseSRWLockExclusive" => dir!(release_srw_lock_exclusive),
        "ReleaseSRWLockShared" => dir!(release_srw_lock_shared),
        "InitializeConditionVariable" => dir!(initialize_condition_variable),
        "SleepConditionVariableCS" => dir!(sleep_condition_variable_cs),
        "SleepConditionVariableSRW" => dir!(sleep_condition_variable_srw),
        "WakeConditionVariable" => dir!(wake_condition_variable),
        "WakeAllConditionVariable" => dir!(wake_all_condition_variable),
        "GetTickCount" => dir!(get_tick_count),
        "GetTickCount64" => dir!(get_tick_count64),
        "QueryPerformanceCounter" => dir!(query_performance_counter),
        "QueryPerformanceFrequency" => dir!(query_performance_frequency),
        _ => return None,
    })
}
