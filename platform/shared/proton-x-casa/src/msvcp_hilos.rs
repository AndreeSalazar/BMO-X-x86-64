//! **Los hilos de la biblioteca de C++ de MSVC (`msvcp140.dll`), de la casa**
//! (tanda 2 de Cyberpunk, 29-09): lo que hay DEBAJO de `std::mutex`,
//! `std::condition_variable`, `std::thread`, `std::call_once`, los relojes
//! de `<chrono>` y las tareas de PPL (`Concurrency::task`).
//!
//! ```text
//!    _Mtx_*         init(_in_situ) destroy(_in_situ) lock trylock unlock
//!                   current_owns: una seccion critica de la casa, con su
//!                   propietario y su cuenta (como msvcp140: el mismo hilo
//!                   vuelve a entrar aunque no sea recursivo)
//!    _Cnd_*         init(_in_situ) destroy(_in_situ) wait timedwait signal
//!                   broadcast y los "at_thread_exit"
//!    _Thrd_*        start join detach id sleep yield
//!    relojes        _Query_perf_counter / _frequency, _Xtime_get_ticks
//!    una vez        _Execute_once (std::call_once)
//!    Concurrency    _Schedule_chore / _Release_chore (un hilo por tarea),
//!                   _ContextCallback, el registro de tareas (nada) y
//!                   platform::GetCurrentThreadId
//!    texto          _Strcoll _Strxfrm _Mbrtowc (el locale "C"),
//!                   _Syserror_map _Winerror_map _Winerror_message
//! ```
//!
//! Lo que no, dicho: `_Cnd_register_at_thread_exit` (el de
//! `std::notify_all_at_thread_exit` y `set_value_at_thread_exit`) suelta y
//! avisa EN EL MOMENTO, no al salir el hilo: quien espera no se queda
//! colgado, pero se despierta antes que en Windows.

use crate::{aviso, dir, esperas, hilos, kernel32};

const THRD_SUCCESS: i32 = 0;
const THRD_TIMEDOUT: i32 = 2;
const THRD_BUSY: i32 = 3;
const THRD_ERROR: i32 = 4;

const INFINITE: u32 = u32::MAX;

/// Lo que MSVC reserva para un `_Mtx_internal_imp_t` y un
/// `_Cnd_internal_imp_t` (las cabeceras de VS 2022): la casa usa lo suyo
/// dentro, y la DIRECCION es la llave del cerrojo.
const MTX_BYTES: usize = 80;
const CND_BYTES: usize = 72;

/// El `_Mtx_t` de la casa: el tipo, quien lo tiene y cuantas veces.
#[repr(C)]
struct Mutex {
    tipo: i32,
    propietario: u32,
    cuenta: u32,
}

fn mutex<'a>(m: u64) -> &'a mut Mutex {
    // SAFETY: un `_Mtx_t` que inicio `_Mtx_init(_in_situ)`: al menos 80 bytes.
    unsafe { &mut *(m as *mut Mutex) }
}

fn yo() -> u32 {
    kernel32::get_current_thread_id()
}

extern "win64" fn mtx_init_in_situ(m: u64, tipo: i32) {
    // SAFETY: los 80 bytes del `_Mtx_t`.
    unsafe { core::ptr::write_bytes(m as *mut u8, 0, MTX_BYTES) };
    mutex(m).tipo = tipo;
}

extern "win64" fn mtx_init(p: *mut u64, tipo: i32) -> i32 {
    let m = crate::crt::malloc(MTX_BYTES);
    if m == 0 || p.is_null() {
        return THRD_ERROR;
    }
    mtx_init_in_situ(m, tipo);
    // SAFETY: el `_Mtx_t*` del `.exe`.
    unsafe { *p = m };
    THRD_SUCCESS
}

extern "win64" fn mtx_destroy(m: u64) {
    if m != 0 {
        crate::crt::free(m);
    }
}

extern "win64" fn mtx_destroy_in_situ(_m: u64) {}

/// `_Mtx_lock`: como el `msvcp140` de verdad, si el hilo YA lo tiene (sea
/// recursivo o no) solo sube la cuenta y dice que si. (El metal lo dijo el
/// 29-09: tanda2.exe en Windows; la casa decia "ocupado" y el mutex se
/// quedaba cogido.)
extern "win64" fn mtx_lock(m: u64) -> i32 {
    let x = mutex(m);
    if x.cuenta > 0 && x.propietario == yo() {
        x.cuenta += 1;
        return THRD_SUCCESS;
    }
    hilos::enter_critical_section(m);
    let x = mutex(m);
    x.propietario = yo();
    x.cuenta = 1;
    THRD_SUCCESS
}

extern "win64" fn mtx_trylock(m: u64) -> i32 {
    let x = mutex(m);
    if x.cuenta > 0 && x.propietario == yo() {
        x.cuenta += 1;
        return THRD_SUCCESS;
    }
    if hilos::try_enter_critical_section(m) == 0 {
        return THRD_BUSY;
    }
    let x = mutex(m);
    x.propietario = yo();
    x.cuenta = 1;
    THRD_SUCCESS
}

extern "win64" fn mtx_unlock(m: u64) -> i32 {
    let x = mutex(m);
    if x.cuenta == 0 || x.propietario != yo() {
        aviso("_Mtx_unlock: un std::mutex que este hilo no tiene");
        return THRD_ERROR;
    }
    x.cuenta -= 1;
    if x.cuenta == 0 {
        x.propietario = 0;
        hilos::leave_critical_section(m);
    }
    THRD_SUCCESS
}

/// `int _Mtx_current_owns(_Mtx_t)`: un `int` entero (quien lo lee como
/// `bool` mira solo `al`, que tambien vale).
extern "win64" fn mtx_current_owns(m: u64) -> i32 {
    let x = mutex(m);
    (x.cuenta > 0 && x.propietario == yo()) as i32
}

// -- Condiciones -------------------------------------------------------------------------------

extern "win64" fn cnd_init_in_situ(c: u64) {
    // SAFETY: los 72 bytes del `_Cnd_t`.
    unsafe { core::ptr::write_bytes(c as *mut u8, 0, CND_BYTES) };
}

extern "win64" fn cnd_init(p: *mut u64) -> i32 {
    let c = crate::crt::malloc(CND_BYTES);
    if c == 0 || p.is_null() {
        return THRD_ERROR;
    }
    cnd_init_in_situ(c);
    // SAFETY: el `_Cnd_t*` del `.exe`.
    unsafe { *p = c };
    THRD_SUCCESS
}

extern "win64" fn cnd_destroy(c: u64) {
    if c != 0 {
        crate::crt::free(c);
    }
}

extern "win64" fn cnd_destroy_in_situ(_c: u64) {}

/// Esperar en `c` soltando `m` entero (tambien si es recursivo) y
/// recuperarlo como estaba. `true` si la despertaron.
fn esperar(c: u64, m: u64, ms: u32) -> bool {
    let x = mutex(m);
    let (propietario, cuenta) = (x.propietario, x.cuenta);
    x.propietario = 0;
    x.cuenta = 0;
    let r = hilos::sleep_condition_variable_cs(c, m, ms) != 0;
    let x = mutex(m);
    x.propietario = propietario;
    x.cuenta = cuenta;
    r
}

extern "win64" fn cnd_wait(c: u64, m: u64) -> i32 {
    esperar(c, m, INFINITE);
    THRD_SUCCESS
}

/// Los milisegundos que faltan hasta el instante absoluto `t` (un
/// `_timespec64`: segundos y nanosegundos desde 1970), redondeando arriba.
fn ms_hasta(t: *const i64) -> u32 {
    if t.is_null() {
        return INFINITE;
    }
    // SAFETY: el `_timespec64` del `.exe` (i64 y long).
    let (s, ns) = unsafe { (t.read_unaligned(), (t.add(1) as *const i32).read_unaligned()) };
    let objetivo = s as i128 * 1_000_000_000 + ns as i128;
    let ahora = xtime_get_ticks() as i128 * 100;
    let falta = objetivo - ahora;
    if falta <= 0 {
        0
    } else {
        ((falta + 999_999) / 1_000_000).min(INFINITE as i128 - 1) as u32
    }
}

extern "win64" fn cnd_timedwait(c: u64, m: u64, t: *const i64) -> i32 {
    if esperar(c, m, ms_hasta(t)) {
        THRD_SUCCESS
    } else {
        THRD_TIMEDOUT
    }
}

extern "win64" fn cnd_signal(c: u64) -> i32 {
    hilos::wake_condition_variable(c);
    THRD_SUCCESS
}

extern "win64" fn cnd_broadcast(c: u64) -> i32 {
    hilos::wake_all_condition_variable(c);
    THRD_SUCCESS
}

/// `_Cnd_register_at_thread_exit(c, m, *hecho)`: al salir el hilo, soltar
/// `m`, poner `*hecho = 1` y avisar a todos. Se hace YA (ver la cabecera).
extern "win64" fn cnd_register_at_thread_exit(c: u64, m: u64, hecho: *mut i32) {
    if !hecho.is_null() {
        // SAFETY: el int del `.exe`.
        unsafe { *hecho = 1 };
    }
    mtx_unlock(m);
    hilos::wake_all_condition_variable(c);
}

extern "win64" fn nada(_a: u64) {}

// -- Hilos -------------------------------------------------------------------------------------

/// El `_Thrd_t` de MSVC: `{ HANDLE, unsigned id }` (16 bytes: por valor, en
/// x64 llega por puntero).
#[repr(C)]
struct Hilo {
    h: u64,
    id: u32,
}

extern "win64" fn thrd_start(t: *mut Hilo, f: u64, arg: u64) -> i32 {
    let mut id = 0u32;
    // La funcion es `int f(void*)`: la misma convencion que la de un hilo de Windows.
    let h = hilos::create_thread(0, 0, f, arg, 0, &mut id);
    if h == 0 || t.is_null() {
        return THRD_ERROR;
    }
    // SAFETY: el `_Thrd_t` del `.exe`.
    unsafe { t.write_unaligned(Hilo { h, id }) };
    THRD_SUCCESS
}

extern "win64" fn thrd_join(t: *const Hilo, res: *mut i32) -> i32 {
    // SAFETY: el `_Thrd_t` que dio `_Thrd_start`.
    let h = unsafe { t.read_unaligned() }.h;
    if hilos::wait_for_single_object(h, INFINITE) != 0 {
        return THRD_ERROR;
    }
    let mut codigo = 0u32;
    hilos::get_exit_code_thread(h, &mut codigo);
    if !res.is_null() {
        // SAFETY: el int del `.exe`.
        unsafe { *res = codigo as i32 };
    }
    hilos::close_handle(h);
    THRD_SUCCESS
}

extern "win64" fn thrd_detach(t: *const Hilo) -> i32 {
    // SAFETY: como `thrd_join`.
    let h = unsafe { t.read_unaligned() }.h;
    if hilos::close_handle(h) != 0 {
        THRD_SUCCESS
    } else {
        THRD_ERROR
    }
}

extern "win64" fn thrd_id() -> u32 {
    yo()
}

extern "win64" fn thrd_sleep(t: *const i64) {
    hilos::sleep(ms_hasta(t));
}

extern "win64" fn thrd_yield() {
    hilos::switch_to_thread();
}

// -- Relojes -----------------------------------------------------------------------------------

/// `_Xtime_get_ticks`: unidades de 100 ns desde 1970 (el `system_clock`).
extern "win64" fn xtime_get_ticks() -> i64 {
    (esperas::filetime_ahora() - 116_444_736_000_000_000) as i64
}

/// `_Query_perf_counter`: el de QueryPerformanceCounter (la casa cuenta ns).
extern "win64" fn query_perf_counter() -> i64 {
    let mut v = 0i64;
    hilos::query_performance_counter(&mut v);
    v
}

extern "win64" fn query_perf_frequency() -> i64 {
    1_000_000_000
}

// -- Una vez -----------------------------------------------------------------------------------

type UnaVez = extern "win64" fn(u64, u64, *mut u64) -> i32;

/// `_Execute_once(once_flag&, callback, pv)`: el `once_flag` es un puntero
/// (INIT_ONCE): 0 sin hacer, 1 haciendose, 2 hecho. Si el callback falla
/// (devuelve 0, o lanza), vuelve a 0 y otro lo intentara.
extern "win64" fn execute_once(bandera: *mut u64, f: UnaVez, pv: u64) -> i32 {
    loop {
        // SAFETY: el `once_flag` del `.exe`; los hilos de la casa son
        // cooperativos: nadie lo toca entre la lectura y la escritura.
        match unsafe { bandera.read_volatile() } {
            2 => return 1,
            1 => {
                hilos::switch_to_thread();
            }
            _ => break,
        }
    }
    // SAFETY: como arriba.
    unsafe { bandera.write_volatile(1) };
    let mut ctx = 0u64;
    // Como InitOnceExecuteOnce: (la bandera, el parametro, el contexto). El
    // metal lo dijo el 29-09 (tanda2.exe): el parametro va SEGUNDO.
    let ok = f(bandera as u64, pv, &mut ctx) != 0;
    // SAFETY: como arriba.
    unsafe { bandera.write_volatile(if ok { 2 } else { 0 }) };
    ok as i32
}

// -- Concurrency (PPL) -------------------------------------------------------------------------

/// `_Threadpool_chore`: `{ PTP_WORK _M_work; void (*_M_callback)(void*); void* _M_data; }`.
#[repr(C)]
struct Tarea {
    trabajo: u64,
    llamada: u64,
    datos: u64,
}

type Llamada = extern "win64" fn(u64);

/// El hilo de una tarea: llama a su callback con sus datos.
extern "win64" fn correr_tarea(t: u64) -> u32 {
    // SAFETY: el `_Threadpool_chore` que dio `_Schedule_chore`, vivo hasta
    // que la tarea acaba (PPL lo guarda en la tarea misma).
    let (llamada, datos) = unsafe {
        let t = &*(t as *const Tarea);
        (t.llamada, t.datos)
    };
    // SAFETY: el callback de PPL: void f(void*).
    let f: Llamada = unsafe { core::mem::transmute::<u64, Llamada>(llamada) };
    f(datos);
    0
}

/// `_Schedule_chore`: en Windows, el pool de hilos; aqui, un hilo propio.
extern "win64" fn schedule_chore(t: *mut Tarea) -> i32 {
    if t.is_null() {
        return -1;
    }
    let h = hilos::create_thread(0, 0, dir!(correr_tarea), t as u64, 0, core::ptr::null_mut());
    if h == 0 {
        return -1;
    }
    hilos::close_handle(h);
    // SAFETY: el `_Threadpool_chore` del `.exe`: `_M_work` distinto de NULL
    // es "ya programada".
    unsafe { (*t).trabajo = 1 };
    0
}

extern "win64" fn release_chore(t: *mut Tarea) {
    if !t.is_null() {
        // SAFETY: como `schedule_chore`.
        unsafe { (*t).trabajo = 0 };
    }
}

/// `_ContextCallback::_Capture` / `_Reset`: sin apartamentos COM, el
/// contexto capturado es "ninguno" (su unico campo, a NULL).
extern "win64" fn contexto_a_cero(this: *mut u64) {
    if !this.is_null() {
        // SAFETY: el `_ContextCallback` del `.exe` (un puntero).
        unsafe { *this = 0 };
    }
}

/// `_ContextCallback::_CallInContext(std::function<void()>, bool)`: sin
/// apartamentos, se llama aqui mismo. El `std::function` llega por puntero
/// (por valor en C++, y MSVC deja que lo destruya QUIEN RECIBE): su `_Impl`
/// es el ultimo puntero de sus 64 bytes; `_Do_call` es su tercera virtual y
/// `_Delete_this(bool)`, la quinta.
extern "win64" fn call_in_context(_this: u64, funcion: *mut u64, _ignorar: bool) {
    if funcion.is_null() {
        return;
    }
    // SAFETY: un `std::function<void()>` de MSVC x64 (64 bytes).
    let imp = unsafe { *funcion.add(7) };
    if imp == 0 {
        aviso("_CallInContext: un std::function vacio (en Windows, bad_function_call)");
        return;
    }
    // SAFETY: `_Impl` es un `_Func_base` con su vtabla.
    let vt = unsafe { *(imp as *const *const u64) };
    type DoCall = extern "win64" fn(u64);
    type Borrar = extern "win64" fn(u64, bool);
    // SAFETY: las entradas 2 y 4 de la vtabla de `_Func_base<void>`.
    unsafe {
        core::mem::transmute::<u64, DoCall>(*vt.add(2))(imp);
        let en_el_monton = imp != funcion as u64;
        core::mem::transmute::<u64, Borrar>(*vt.add(4))(imp, en_el_monton);
    }
}

extern "win64" fn report_unhandled_error(_this: u64) {
    aviso("Concurrency: una tarea acabo con una excepcion que nadie miro (en Windows, terminate)");
}

extern "win64" fn report_unobserved() {
    aviso("Concurrency: una excepcion de tarea sin observar");
}

// -- Texto del locale "C" ----------------------------------------------------------------------

fn rango<'a>(a: *const u8, b: *const u8) -> &'a [u8] {
    if a.is_null() || b <= a {
        return &[];
    }
    // SAFETY: un rango [a, b) del `.exe`.
    unsafe { core::slice::from_raw_parts(a, b as usize - a as usize) }
}

extern "win64" fn strcoll(a1: *const u8, b1: *const u8, a2: *const u8, b2: *const u8, _col: u64) -> i32 {
    match rango(a1, b1).cmp(rango(a2, b2)) {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    }
}

/// `_Strxfrm`: en el locale "C", la transformada es la cadena misma. Da su
/// largo; si no cabe, no escribe.
extern "win64" fn strxfrm(d1: *mut u8, e1: *mut u8, a2: *const u8, b2: *const u8, _col: u64) -> usize {
    let s = rango(a2, b2);
    let sitio = if d1.is_null() || e1 <= d1 { 0 } else { e1 as usize - d1 as usize };
    if s.len() <= sitio {
        // SAFETY: cabe en [d1, e1).
        unsafe { core::ptr::copy_nonoverlapping(s.as_ptr(), d1, s.len()) };
    }
    s.len()
}

/// `_Mbrtowc`: en el locale "C", un byte es un caracter.
extern "win64" fn mbrtowc(pwc: *mut u16, s: *const u8, n: usize, _st: u64, _cvt: u64) -> isize {
    if s.is_null() {
        return 0;
    }
    // Como msvcp140 (no el mbrtowc de C, que da -2): sin estados, 0.
    if n == 0 {
        return 0;
    }
    // SAFETY: al menos un byte del `.exe`.
    let c = unsafe { *s };
    if !pwc.is_null() {
        // SAFETY: el wchar_t del `.exe`.
        unsafe { *pwc = c as u16 };
    }
    (c != 0) as isize
}

/// `_Syserror_map`: el texto de `std::generic_category().message()`, de la
/// tabla de msvcp140 (en minusculas: no es el de `strerror`).
extern "win64" fn syserror_map(e: i32) -> *const u8 {
    let t: &'static [u8] = match e {
        1 => b"operation not permitted\0",
        2 => b"no such file or directory\0",
        3 => b"no such process\0",
        4 => b"interrupted\0",
        5 => b"io error\0",
        9 => b"bad file descriptor\0",
        11 => b"resource unavailable try again\0",
        12 => b"not enough memory\0",
        13 => b"permission denied\0",
        16 => b"device or resource busy\0",
        17 => b"file exists\0",
        20 => b"not a directory\0",
        21 => b"is a directory\0",
        22 => b"invalid argument\0",
        24 => b"too many files open\0",
        28 => b"no space on device\0",
        36 => b"resource deadlock would occur\0",
        38 => b"filename too long\0",
        41 => b"directory not empty\0",
        138 => b"timed out\0",
        _ => b"unknown error\0",
    };
    t.as_ptr()
}

/// `_Winerror_map`: el `errc` de un error de Win32 (0 si no tiene).
extern "win64" fn winerror_map(e: i32) -> i32 {
    match e {
        2 | 3 | 15 | 161 => 2,
        4 => 24,
        5 | 19 | 32 | 33 | 1224 => 13,
        6 => 9,
        8 | 14 => 12,
        80 | 183 => 17,
        87 => 22,
        112 => 28,
        145 => 41,
        _ => 0,
    }
}

/// `_Winerror_message(codigo, bufer, medida)`: el texto; su largo sin el 0.
extern "win64" fn winerror_message(e: u32, buf: *mut u8, n: u32) -> u32 {
    let t = alloc::format!("error de Windows {e}");
    if buf.is_null() || n == 0 {
        return 0;
    }
    let k = t.len().min(n as usize - 1);
    // SAFETY: `n` bytes del `.exe`.
    unsafe {
        core::ptr::copy_nonoverlapping(t.as_ptr(), buf, k);
        *buf.add(k) = 0;
    }
    k as u32
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "_Mtx_init" => dir!(mtx_init),
        "_Mtx_init_in_situ" => dir!(mtx_init_in_situ),
        "_Mtx_destroy" => dir!(mtx_destroy),
        "_Mtx_destroy_in_situ" => dir!(mtx_destroy_in_situ),
        "_Mtx_lock" => dir!(mtx_lock),
        "_Mtx_trylock" => dir!(mtx_trylock),
        "_Mtx_unlock" => dir!(mtx_unlock),
        "_Mtx_current_owns" => dir!(mtx_current_owns),
        "_Cnd_init" => dir!(cnd_init),
        "_Cnd_init_in_situ" => dir!(cnd_init_in_situ),
        "_Cnd_destroy" => dir!(cnd_destroy),
        "_Cnd_destroy_in_situ" => dir!(cnd_destroy_in_situ),
        "_Cnd_wait" => dir!(cnd_wait),
        "_Cnd_timedwait" => dir!(cnd_timedwait),
        "_Cnd_signal" => dir!(cnd_signal),
        "_Cnd_broadcast" => dir!(cnd_broadcast),
        "_Cnd_register_at_thread_exit" => dir!(cnd_register_at_thread_exit),
        "_Cnd_unregister_at_thread_exit" | "_Cnd_do_broadcast_at_thread_exit" => dir!(nada),
        "_Thrd_start" => dir!(thrd_start),
        "_Thrd_join" => dir!(thrd_join),
        "_Thrd_detach" => dir!(thrd_detach),
        "_Thrd_id" | "?GetCurrentThreadId@platform@details@Concurrency@@YAJXZ" => dir!(thrd_id),
        "_Thrd_sleep" => dir!(thrd_sleep),
        "_Thrd_yield" => dir!(thrd_yield),
        "_Xtime_get_ticks" => dir!(xtime_get_ticks),
        "_Query_perf_counter" => dir!(query_perf_counter),
        "_Query_perf_frequency" => dir!(query_perf_frequency),
        "?_Execute_once@std@@YAHAEAUonce_flag@1@P6AHPEAX1PEAPEAX@Z1@Z" => dir!(execute_once),
        "?_Schedule_chore@details@Concurrency@@YAHPEAU_Threadpool_chore@12@@Z" => dir!(schedule_chore),
        "?_Release_chore@details@Concurrency@@YAXPEAU_Threadpool_chore@12@@Z" => dir!(release_chore),
        "?_Capture@_ContextCallback@details@Concurrency@@AEAAXXZ" | "?_Reset@_ContextCallback@details@Concurrency@@AEAAXXZ" => dir!(contexto_a_cero),
        "?_CallInContext@_ContextCallback@details@Concurrency@@QEBAXV?$function@$$A6AXXZ@std@@_N@Z" => dir!(call_in_context),
        "?ReportUnhandledError@_ExceptionHolder@details@Concurrency@@AEAAXXZ" => dir!(report_unhandled_error),
        "?_ReportUnobservedException@details@Concurrency@@YAXXZ" => dir!(report_unobserved),
        "?_LogCancelTask@_TaskEventLogger@details@Concurrency@@QEAAXXZ"
        | "?_LogScheduleTask@_TaskEventLogger@details@Concurrency@@QEAAX_N@Z"
        | "?_LogTaskCompleted@_TaskEventLogger@details@Concurrency@@QEAAXXZ"
        | "?_LogTaskExecutionCompleted@_TaskEventLogger@details@Concurrency@@QEAAXXZ"
        | "?_LogWorkItemCompleted@_TaskEventLogger@details@Concurrency@@QEAAXXZ"
        | "?_LogWorkItemStarted@_TaskEventLogger@details@Concurrency@@QEAAXXZ" => dir!(nada),
        "_Strcoll" => dir!(strcoll),
        "_Strxfrm" => dir!(strxfrm),
        "_Mbrtowc" => dir!(mbrtowc),
        "?_Syserror_map@std@@YAPEBDH@Z" => dir!(syserror_map),
        "?_Winerror_map@std@@YAHH@Z" => dir!(winerror_map),
        "?_Winerror_message@std@@YAKKPEADK@Z" => dir!(winerror_message),
        _ => return None,
    })
}
