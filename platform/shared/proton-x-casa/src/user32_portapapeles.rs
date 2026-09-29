//! **`user32.dll`, grupo 3: el portapapeles** (tanda 7 de Cyberpunk, 29-09).
//!
//! ```text
//!    OpenClipboard, CloseClipboard, EmptyClipboard
//!    SetClipboardData, GetClipboardData     un HGLOBAL por formato
//!    IsClipboardFormatAvailable, CountClipboardFormats,
//!    EnumClipboardFormats, GetClipboardSequenceNumber
//!    RegisterClipboardFormatW/A, GetClipboardFormatNameW/A
//!    GetClipboardOwner, GetOpenClipboardWindow,
//!    Add/RemoveClipboardFormatListener
//! ```
//!
//! Es el portapapeles DEL PROCESO: todavia no hay uno del escritorio de
//! BMO-X que compartir. Como Windows, CF_TEXT (1) y CF_UNICODETEXT (13) se
//! dan el uno por el otro: quien pone uno puede leer el otro, pero solo
//! despues de CloseClipboard (con el portapapeles aun abierto por quien lo
//! puso, Windows no lo da: lo dijo tanda7.exe en Windows). Los datos son del
//! portapapeles al darlos (EmptyClipboard los suelta).

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::user32_ventanas::{atomo_nuevo, bytes, copiar_texto};
use crate::{dir, kernel32, memoria};

const CF_TEXT: u32 = 1;
const CF_UNICODETEXT: u32 = 13;
const ERROR_CLIPBOARD_NOT_OPEN: u32 = 1418;

struct Estado {
    abierto: Option<u64>,
    /// (formato, HGLOBAL, sintetizado por la casa).
    datos: Vec<(u32, u64, bool)>,
    secuencia: u32,
    nombres: Vec<(u32, Vec<u16>)>,
    /// Se cerro despues de poner algo: el otro texto ya se puede sacar.
    sintesis: bool,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { abierto: None, datos: Vec::new(), secuencia: 1, nombres: Vec::new(), sintesis: false }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.abierto = None;
    e.datos.clear();
    e.secuencia = 1;
    e.nombres.clear();
    e.sintesis = false;
}

fn abierto() -> bool {
    if estado().abierto.is_none() {
        kernel32::poner_error(ERROR_CLIPBOARD_NOT_OPEN);
        return false;
    }
    true
}

extern "win64" fn open_clipboard(h: u64) -> i32 {
    estado().abierto = Some(h);
    1
}

extern "win64" fn close_clipboard() -> i32 {
    if !abierto() {
        return 0;
    }
    let e = estado();
    e.abierto = None;
    e.sintesis = true;
    1
}

fn soltar_todo() {
    for (_, h, _) in estado().datos.drain(..) {
        memoria::soltar_del_proceso(h);
    }
}

extern "win64" fn empty_clipboard() -> i32 {
    if !abierto() {
        return 0;
    }
    soltar_todo();
    estado().secuencia += 1;
    estado().sintesis = false;
    1
}

/// `SetClipboardData`: el HGLOBAL pasa a ser del portapapeles. Lo que se
/// sintetizo del otro texto ya no vale.
extern "win64" fn set_clipboard_data(f: u32, h: u64) -> u64 {
    if !abierto() {
        return 0;
    }
    let e = estado();
    let (fuera, quedan): (Vec<_>, Vec<_>) = e.datos.drain(..).partition(|d| d.0 == f || (d.2 && (f == CF_TEXT || f == CF_UNICODETEXT)));
    e.datos = quedan;
    for (_, viejo, _) in fuera {
        if viejo != h {
            memoria::soltar_del_proceso(viejo);
        }
    }
    e.datos.push((f, h, false));
    e.secuencia += 1;
    e.sintesis = false;
    h
}

/// El texto de un HGLOBAL de texto (hasta su cero o su medida).
fn texto_de(h: u64, ancho: bool) -> Vec<u16> {
    let n = memoria::medida_del_proceso(h).unwrap_or(0) as usize;
    let mut v = Vec::new();
    for k in 0..if ancho { n / 2 } else { n } {
        // SAFETY: `n` bytes del bloque del monton del proceso.
        let c = unsafe { if ancho { (h as *const u16).add(k).read() } else { (h as *const u8).add(k).read() as u16 } };
        if c == 0 {
            break;
        }
        v.push(c);
    }
    v
}

/// Un HGLOBAL nuevo con un texto (y su cero).
fn nuevo_texto(t: &[u16], ancho: bool) -> Option<u64> {
    let n = (t.len() + 1) * if ancho { 2 } else { 1 };
    let h = memoria::pedir_del_proceso(n as u64)?;
    copiar_texto(t, h, t.len() + 1, !ancho);
    Some(h)
}

extern "win64" fn get_clipboard_data(f: u32) -> u64 {
    if !abierto() {
        return 0;
    }
    let e = estado();
    if let Some(d) = e.datos.iter().find(|d| d.0 == f) {
        return d.1;
    }
    // El otro texto, si lo hay: se hace una vez y se queda.
    if !e.sintesis {
        return 0;
    }
    let otro = match f {
        CF_TEXT => CF_UNICODETEXT,
        CF_UNICODETEXT => CF_TEXT,
        _ => return 0,
    };
    let Some(&(_, h, _)) = e.datos.iter().find(|d| d.0 == otro) else { return 0 };
    let t = texto_de(h, otro == CF_UNICODETEXT);
    match nuevo_texto(&t, f == CF_UNICODETEXT) {
        Some(n) => {
            estado().datos.push((f, n, true));
            n
        }
        None => 0,
    }
}

fn hay(f: u32) -> bool {
    let d = &estado().datos;
    d.iter().any(|x| x.0 == f) || (estado().sintesis && (f == CF_TEXT || f == CF_UNICODETEXT) && d.iter().any(|x| x.0 == CF_TEXT || x.0 == CF_UNICODETEXT))
}

extern "win64" fn is_clipboard_format_available(f: u32) -> i32 {
    hay(f) as i32
}

/// Los formatos que hay, con los dos textos si hay uno.
fn formatos() -> Vec<u32> {
    let mut v: Vec<u32> = estado().datos.iter().map(|d| d.0).collect();
    for f in [CF_UNICODETEXT, CF_TEXT] {
        if hay(f) && !v.contains(&f) {
            v.push(f);
        }
    }
    v
}

extern "win64" fn count_clipboard_formats() -> i32 {
    formatos().len() as i32
}

/// `EnumClipboardFormats(anterior)`: el siguiente (0 empieza); 0 al final.
extern "win64" fn enum_clipboard_formats(anterior: u32) -> u32 {
    if !abierto() {
        return 0;
    }
    kernel32::poner_error(0);
    let v = formatos();
    let i = if anterior == 0 { 0 } else { v.iter().position(|&f| f == anterior).map_or(v.len(), |i| i + 1) };
    v.get(i).copied().unwrap_or(0)
}

extern "win64" fn get_clipboard_sequence_number() -> u32 {
    estado().secuencia
}

fn register_format(n: Vec<u16>) -> u32 {
    if n.is_empty() {
        kernel32::poner_error(87);
        return 0;
    }
    let a = atomo_nuevo(&n) as u32;
    let e = estado();
    if !e.nombres.iter().any(|x| x.0 == a) {
        e.nombres.push((a, n));
    }
    a
}

extern "win64" fn register_clipboard_format_w(n: *const u16) -> u32 {
    // SAFETY: una cadena suya.
    register_format(if n.is_null() { Vec::new() } else { unsafe { crate::user32::utf16(n) } })
}

extern "win64" fn register_clipboard_format_a(n: *const u8) -> u32 {
    // SAFETY: una cadena suya.
    register_format(if n.is_null() { Vec::new() } else { unsafe { bytes(n) } })
}

fn format_name(f: u32, buf: u64, n: i32, ansi: bool) -> i32 {
    match estado().nombres.iter().find(|x| x.0 == f) {
        Some((_, t)) => copiar_texto(&t.clone(), buf, n.max(0) as usize, ansi) as i32,
        None => 0,
    }
}

extern "win64" fn get_clipboard_format_name_w(f: u32, buf: *mut u16, n: i32) -> i32 {
    format_name(f, buf as u64, n, false)
}

extern "win64" fn get_clipboard_format_name_a(f: u32, buf: *mut u8, n: i32) -> i32 {
    format_name(f, buf as u64, n, true)
}

extern "win64" fn get_open_clipboard_window() -> u64 {
    estado().abierto.unwrap_or(0)
}

extern "win64" fn get_clipboard_owner() -> u64 {
    0
}

extern "win64" fn si(_h: u64) -> i32 {
    1
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "OpenClipboard" => dir!(open_clipboard),
        "CloseClipboard" => dir!(close_clipboard),
        "EmptyClipboard" => dir!(empty_clipboard),
        "SetClipboardData" => dir!(set_clipboard_data),
        "GetClipboardData" => dir!(get_clipboard_data),
        "IsClipboardFormatAvailable" => dir!(is_clipboard_format_available),
        "CountClipboardFormats" => dir!(count_clipboard_formats),
        "EnumClipboardFormats" => dir!(enum_clipboard_formats),
        "GetClipboardSequenceNumber" => dir!(get_clipboard_sequence_number),
        "RegisterClipboardFormatW" => dir!(register_clipboard_format_w),
        "RegisterClipboardFormatA" => dir!(register_clipboard_format_a),
        "GetClipboardFormatNameW" => dir!(get_clipboard_format_name_w),
        "GetClipboardFormatNameA" => dir!(get_clipboard_format_name_a),
        "GetOpenClipboardWindow" => dir!(get_open_clipboard_window),
        "GetClipboardOwner" => dir!(get_clipboard_owner),
        "AddClipboardFormatListener" | "RemoveClipboardFormatListener" => dir!(si),
        _ => return None,
    })
}
