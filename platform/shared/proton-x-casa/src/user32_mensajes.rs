//! **`user32.dll`, grupo 2: los mensajes** (tanda 6 de Cyberpunk, 29-09).
//!
//! ```text
//!    SendMessageW/A, SendMessageTimeoutW/A, SendNotifyMessageW/A
//!                               a la WndProc, YA (un hilo: no hay que cruzar)
//!    PostMessageW/A             a la cola (hwnd 0: del hilo; 0xFFFF: a todas)
//!    PostThreadMessageW/A       a la cola, sin ventana
//!    CallWindowProcW/A          la WndProc de antes (subclasificar)
//!    RegisterWindowMessageW/A   un numero de 0xC000 en adelante, el mismo
//!                               para el mismo nombre
//!    SetTimer, KillTimer        WM_TIMER a su tiempo; con TIMERPROC en
//!                               lParam, DispatchMessage la llama
//!    MsgWaitForMultipleObjects(Ex)   los objetos O un mensaje (devuelve n)
//!    WaitMessage, GetQueueStatus, GetInputState
//!    Get/Peek/DispatchMessageA  los de W (los mensajes de la casa no llevan
//!                               texto que traducir)
//!    MessageBoxW/A              a la consola, y contesta el primer boton
//! ```
//!
//! Los WM_TIMER se juntan como en Windows: uno por temporizador en la cola,
//! aunque pasen dos periodos sin que nadie lo saque.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::ventanas::Msg;

use crate::user32::{self, bombear, llamar, proc_de, utf16};
use crate::user32_ventanas::{atomo_nuevo, bytes};
use crate::{con, dir, hilos, kernel32, plataforma};

pub(crate) const WM_TIMER: u32 = 0x0113;
const HWND_BROADCAST: u64 = 0xFFFF;
const ERROR_INVALID_WINDOW_HANDLE: u32 = 1400;
const WAIT_TIMEOUT: u32 = 258;
const INFINITE: u32 = u32::MAX;

// -- Los temporizadores ------------------------------------------------------

struct Temporizador {
    hwnd: u64,
    id: u64,
    periodo: u64,
    siguiente: u64,
    /// TIMERPROC, o 0.
    funcion: u64,
}

struct Estado {
    lista: Vec<Temporizador>,
    /// El siguiente id de un temporizador sin ventana.
    nuevo_id: u64,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
// [hilos] cerrojo -- estado del proceso que tocan los hilos del juego: necesita un cerrojo (H2.1)
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { lista: Vec::new(), nuevo_id: 1 }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.lista.clear();
    e.nuevo_id = 1;
}

fn viva(h: u64) -> bool {
    con(|e| e.ventanas.iter().any(|v| v.hwnd == h && v.viva))
}

/// **Los WM_TIMER que tocan, a la cola** (lo llama `bombear`).
pub(crate) fn temporizadores() {
    let ahora = hilos::ahora_ns();
    for t in estado().lista.iter_mut() {
        if ahora < t.siguiente {
            continue;
        }
        t.siguiente = ahora + t.periodo;
        let m = Msg { hwnd: t.hwnd, mensaje: WM_TIMER, wparam: t.id, lparam: t.funcion };
        con(|e| {
            if !e.cola.espera(m.hwnd, WM_TIMER, m.wparam) {
                e.cola.publicar(m);
            }
        });
    }
}

/// Si (hwnd, id, TIMERPROC) es un temporizador vivo: DispatchMessage solo
/// llama a una TIMERPROC que alguien dio a SetTimer.
pub(crate) fn es_temporizador(h: u64, id: u64, funcion: u64) -> bool {
    estado().lista.iter().any(|t| t.hwnd == h && t.id == id && t.funcion == funcion)
}

/// Una ventana murio: sus temporizadores con ella.
pub(crate) fn murio(h: u64) {
    estado().lista.retain(|t| t.hwnd != h);
}

/// `SetTimer(hwnd, id, ms, TIMERPROC)`: con ventana devuelve `id` (o 1);
/// sin ella, un id nuevo (o el mismo, si `id` ya era uno suyo). Menos de 10
/// ms son 10, como en Windows.
extern "win64" fn set_timer(h: u64, id: u64, ms: u32, funcion: u64) -> u64 {
    if h != 0 && !viva(h) {
        kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
        return 0;
    }
    let e = estado();
    let id = if h != 0 || e.lista.iter().any(|t| t.hwnd == 0 && t.id == id && id != 0) {
        id
    } else {
        // Los de Windows sin ventana no son chicos; aqui tampoco.
        e.nuevo_id += 1;
        0x7FF0 + e.nuevo_id
    };
    let periodo = ms.clamp(10, 0x7FFF_FFFF) as u64 * 1_000_000;
    e.lista.retain(|t| !(t.hwnd == h && t.id == id));
    e.lista.push(Temporizador { hwnd: h, id, periodo, siguiente: hilos::ahora_ns() + periodo, funcion });
    if h != 0 && id == 0 {
        1
    } else {
        id
    }
}

/// `KillTimer`: fuera, con el WM_TIMER que tuviera en la cola.
extern "win64" fn kill_timer(h: u64, id: u64) -> i32 {
    let e = estado();
    let antes = e.lista.len();
    e.lista.retain(|t| !(t.hwnd == h && t.id == id));
    if e.lista.len() == antes {
        kernel32::poner_error(1402); // ERROR_INVALID_TIMER_HANDLE (casi: lo que se dice es que no esta)
        return 0;
    }
    con(|e| e.cola.quitar(h, WM_TIMER, id));
    1
}

// -- Enviar y publicar -------------------------------------------------------

fn de_arriba() -> Vec<u64> {
    con(|e| e.ventanas.iter().filter(|v| v.viva && v.datos.estilo & 0x4000_0000 == 0).map(|v| v.hwnd).collect())
}

/// **SendMessage**: la WndProc de la ventana, ya.
fn enviar(h: u64, m: u32, w: u64, l: u64) -> Option<i64> {
    if h == HWND_BROADCAST {
        for x in de_arriba() {
            if let Some(wp) = proc_de(x) {
                llamar(wp, x, m, w, l);
            }
        }
        return Some(1);
    }
    match proc_de(h) {
        Some(wp) => Some(llamar(wp, h, m, w, l)),
        None => {
            kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
            None
        }
    }
}

extern "win64" fn send_message(h: u64, m: u32, w: u64, l: u64) -> i64 {
    enviar(h, m, w, l).unwrap_or(0)
}

extern "win64" fn send_notify_message(h: u64, m: u32, w: u64, l: u64) -> i32 {
    enviar(h, m, w, l).is_some() as i32
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn send_message_timeout(h: u64, m: u32, w: u64, l: u64, _banderas: u32, _ms: u32, r: *mut i64) -> i64 {
    match enviar(h, m, w, l) {
        Some(v) => {
            if !r.is_null() {
                // SAFETY: un DWORD_PTR suyo.
                unsafe { r.write(v) };
            }
            1
        }
        None => 0,
    }
}

/// **PostMessage**: a la cola. hwnd 0, del hilo; HWND_BROADCAST, una por
/// ventana de arriba.
extern "win64" fn post_message(h: u64, m: u32, w: u64, l: u64) -> i32 {
    let para: Vec<u64> = match h {
        0 => alloc::vec![0],
        HWND_BROADCAST => de_arriba(),
        h if viva(h) => alloc::vec![h],
        _ => {
            kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
            return 0;
        }
    };
    con(|e| {
        for hwnd in para {
            e.cola.publicar(Msg { hwnd, mensaje: m, wparam: w, lparam: l });
        }
    });
    1
}

/// `PostThreadMessage`: a la cola, sin ventana (una cola: la del proceso).
extern "win64" fn post_thread_message(_hilo: u32, m: u32, w: u64, l: u64) -> i32 {
    post_message(0, m, w, l)
}

extern "win64" fn call_window_proc(antes: u64, h: u64, m: u32, w: u64, l: u64) -> i64 {
    if antes == 0 {
        return 0;
    }
    llamar(antes, h, m, w, l)
}

fn register_window_message(n: Vec<u16>) -> u32 {
    if n.is_empty() {
        kernel32::poner_error(87); // ERROR_INVALID_PARAMETER
        return 0;
    }
    atomo_nuevo(&n) as u32
}

extern "win64" fn register_window_message_w(n: *const u16) -> u32 {
    // SAFETY: una cadena suya.
    register_window_message(if n.is_null() { Vec::new() } else { unsafe { utf16(n) } })
}

extern "win64" fn register_window_message_a(n: *const u8) -> u32 {
    // SAFETY: una cadena suya.
    register_window_message(if n.is_null() { Vec::new() } else { unsafe { bytes(n) } })
}

// -- Esperar -----------------------------------------------------------------

/// `MsgWaitForMultipleObjectsEx(n, handles, ms, QS_*, banderas)`: el indice
/// del objeto que se encendio, `n` si en la cola hay algo de `mascara`, o
/// WAIT_TIMEOUT. MWMO_WAITALL (1): todos los objetos. (Windows solo cuenta
/// lo que llego DESPUES de la ultima vez que se miro la cola; aqui, lo que
/// haya: un bucle que vacia la cola antes de esperar ve lo mismo.)
extern "win64" fn msg_wait_ex(n: u32, hs: *const u64, ms: u32, mascara: u32, banderas: u32) -> u32 {
    let plazo = (ms != INFINITE).then(|| hilos::ahora_ns() + ms as u64 * 1_000_000);
    loop {
        // Primero los objetos: la cola es como un objeto mas, el `n`.
        if n > 0 {
            let r = hilos::wait_for_multiple_objects(n, hs, (banderas & 1) as i32, 0);
            if r != WAIT_TIMEOUT {
                return r;
            }
        }
        bombear();
        if con(|e| e.cola.tipos()) & mascara != 0 {
            return n;
        }
        if plazo.is_some_and(|p| hilos::ahora_ns() >= p) {
            return WAIT_TIMEOUT;
        }
        hilos::sleep(1);
    }
}

extern "win64" fn msg_wait(n: u32, hs: *const u64, todos: i32, ms: u32, mascara: u32) -> u32 {
    msg_wait_ex(n, hs, ms, mascara, (todos != 0) as u32)
}

/// `WaitMessage`: hasta que haya algo en la cola.
extern "win64" fn wait_message() -> i32 {
    loop {
        bombear();
        if con(|e| e.cola.hay_algo()) {
            return 1;
        }
        if !hilos::ceder() {
            (plataforma().dormir)();
        }
    }
}

/// `GetQueueStatus`: los QS_* que hay (arriba) y los nuevos (abajo; aqui,
/// los mismos).
extern "win64" fn get_queue_status(banderas: u32) -> u32 {
    bombear();
    let t = con(|e| e.cola.tipos()) & banderas & 0xFFFF;
    t << 16 | t
}

/// `GetInputState`: si hay teclas o raton en la cola.
extern "win64" fn get_input_state() -> i32 {
    bombear();
    (con(|e| e.cola.tipos()) & 0x7 != 0) as i32
}

extern "win64" fn get_message_time() -> i32 {
    (hilos::ahora_ns() / 1_000_000) as i32
}

extern "win64" fn cero() -> u64 {
    0
}

// -- MessageBox --------------------------------------------------------------

/// El primer boton de cada MB_*: OK, OKCANCEL -> IDOK; ABORTRETRYIGNORE ->
/// IDABORT; YESNOCANCEL, YESNO -> IDYES; RETRYCANCEL -> IDRETRY;
/// CANCELTRYCONTINUE -> IDCANCEL.
fn message_box(texto: Vec<u16>, titulo: Vec<u16>, tipo: u32) -> i32 {
    let p = plataforma();
    let mut linea = Vec::new();
    linea.extend_from_slice(b"MessageBox [");
    let a_bytes = |t: &[u16], v: &mut Vec<u8>| v.extend(t.iter().map(|&c| if c < 0x80 { c as u8 } else { b'?' }));
    a_bytes(&titulo, &mut linea);
    linea.extend_from_slice(b"] ");
    a_bytes(&texto, &mut linea);
    linea.push(b'\n');
    (p.escribir)(&linea);
    match tipo & 0xF {
        2 => 3,
        3 | 4 => 6,
        5 => 4,
        6 => 2,
        _ => 1,
    }
}

extern "win64" fn message_box_w(_h: u64, texto: *const u16, titulo: *const u16, tipo: u32) -> i32 {
    // SAFETY: las cadenas suyas.
    let leer = |p: *const u16| if p.is_null() { Vec::new() } else { unsafe { utf16(p) } };
    message_box(leer(texto), leer(titulo), tipo)
}

extern "win64" fn message_box_a(_h: u64, texto: *const u8, titulo: *const u8, tipo: u32) -> i32 {
    // SAFETY: las cadenas suyas.
    let leer = |p: *const u8| if p.is_null() { Vec::new() } else { unsafe { bytes(p) } };
    message_box(leer(texto), leer(titulo), tipo)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "SendMessageW" | "SendMessageA" => dir!(send_message),
        "SendNotifyMessageW" | "SendNotifyMessageA" => dir!(send_notify_message),
        "SendMessageTimeoutW" | "SendMessageTimeoutA" => dir!(send_message_timeout),
        "PostMessageW" | "PostMessageA" => dir!(post_message),
        "PostThreadMessageW" | "PostThreadMessageA" => dir!(post_thread_message),
        "CallWindowProcW" | "CallWindowProcA" => dir!(call_window_proc),
        "RegisterWindowMessageW" => dir!(register_window_message_w),
        "RegisterWindowMessageA" => dir!(register_window_message_a),
        "SetTimer" => dir!(set_timer),
        "KillTimer" => dir!(kill_timer),
        "MsgWaitForMultipleObjects" => dir!(msg_wait),
        "MsgWaitForMultipleObjectsEx" => dir!(msg_wait_ex),
        "WaitMessage" => dir!(wait_message),
        "GetQueueStatus" => dir!(get_queue_status),
        "GetInputState" => dir!(get_input_state),
        "GetMessageTime" => dir!(get_message_time),
        "GetMessagePos" | "GetMessageExtraInfo" | "InSendMessage" | "ReplyMessage" => dir!(cero),
        "GetMessageA" => dir!(user32::get_message_w),
        "PeekMessageA" => dir!(user32::peek_message_w),
        "DispatchMessageA" => dir!(user32::dispatch_message_w),
        "MessageBoxW" => dir!(message_box_w),
        "MessageBoxA" => dir!(message_box_a),
        _ => return None,
    })
}
