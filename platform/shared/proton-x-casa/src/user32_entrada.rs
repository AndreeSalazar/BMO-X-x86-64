//! **`user32.dll`, grupo 3: el teclado, el raton y el cursor** (tanda 7 de
//! Cyberpunk, 29-09).
//!
//! ```text
//!    GetKeyState, GetKeyboardState,   el estado del HILO: cambia al SACAR
//!    SetKeyboardState                 de la cola la tecla (como Windows)
//!    GetAsyncKeyState                 el de AHORA: cambia al llegar del
//!                                     escritorio; el bit 0, "pulsada desde
//!                                     la ultima vez que se pregunto"
//!    MapVirtualKey(Ex)W/A, VkKeyScan(Ex)W/A, ToUnicode(Ex), ToAscii,
//!    GetKeyNameTextW/A                el teclado de EE. UU. (el de la casa)
//!    GetKeyboardLayout(List/Name), ActivateKeyboardLayout, GetKeyboardType
//!    Get/SetCursorPos, Get/SetPhysicalCursorPos, GetCursorInfo, ShowCursor,
//!    Set/GetCursor, LoadCursorA, LoadIconW/A, DestroyCursor/Icon,
//!    ClipCursor, GetClipCursor         el cursor (lo pinta el escritorio)
//!    SetCapture, GetCapture, ReleaseCapture, TrackMouseEvent,
//!    Get/SetDoubleClickTime, WindowFromPoint
//!    SendInput, keybd_event, mouse_event   entrada inventada, a la cola
//!    RegisterRawInputDevices, GetRegisteredRawInputDevices,
//!    GetRawInputDeviceList, GetRawInputDeviceInfoW/A, GetRawInputData,
//!    GetRawInputBuffer, DefRawInputProc   el raw input: WM_INPUT
//! ```
//!
//! **Lo que llega del escritorio** (ver [`evento`]): la tecla o el boton a
//! la cola (con WM_MOUSEMOVE cuando el raton solo se mueve), el estado de
//! AHORA, donde esta el cursor y, si el `.exe` pidio raw input del raton o
//! del teclado, un WM_INPUT con el RAWINPUT (el raton, RELATIVO: lo que se
//! movio desde el anterior). Con RIDEV_NOLEGACY, solo el WM_INPUT.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::ventanas::{de_evento, vk_de_scancode, Msg, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP};

use crate::user32_medidas::PANTALLA;
use crate::user32_ventanas::copiar_texto;
use crate::{con, dir, kernel32};

const WM_MOUSEMOVE: u32 = 0x0200;
const WM_CHAR: u32 = 0x0102;
const WM_INPUT: u32 = 0x00FF;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_INVALID_HANDLE: u32 = 6;

// -- El estado de las teclas (puro: se prueba sin `.exe`) --------------------

/// Una tabla de 256 teclas como la de GetKeyboardState: 0x80 abajo, 0x01
/// el interruptor (cambia en cada pulsacion: Bloq Mayus).
#[derive(Clone)]
pub(crate) struct Teclas(pub(crate) [u8; 256]);

impl Teclas {
    pub(crate) const fn nuevas() -> Self {
        Teclas([0; 256])
    }

    fn una(&mut self, vk: u8, abajo: bool) {
        let t = &mut self.0[vk as usize];
        if abajo {
            if *t & 0x80 == 0 {
                *t ^= 1;
            }
            *t |= 0x80;
        } else {
            *t &= !0x80;
        }
    }

    /// Una tecla (con su scancode, para saber si es la izquierda o la
    /// derecha de Shift, Ctrl y Alt: VK_LSHIFT...) o un boton.
    pub(crate) fn tecla(&mut self, vk: u8, sc: u8, abajo: bool) {
        self.una(vk, abajo);
        let lado = match (vk, sc) {
            (0x10, 0x36) => 0xA1,
            (0x10, _) => 0xA0,
            (0x11, _) => 0xA2,
            (0x12, _) => 0xA4,
            _ => return,
        };
        self.una(lado, abajo);
    }

    pub(crate) fn abajo(&self, vk: u8) -> bool {
        self.0[vk as usize] & 0x80 != 0
    }
}

/// La tecla de un mensaje de teclado o de raton (vk, scancode, abajo).
pub(crate) fn tecla_de(m: &Msg) -> Option<(u8, u8, bool)> {
    let sc = ((m.lparam >> 16) & 0xFF) as u8;
    Some(match m.mensaje {
        WM_KEYDOWN => (m.wparam as u8, sc, true),
        WM_KEYUP => (m.wparam as u8, sc, false),
        WM_LBUTTONDOWN => (1, 0, true),
        WM_LBUTTONUP => (1, 0, false),
        WM_RBUTTONDOWN => (2, 0, true),
        WM_RBUTTONUP => (2, 0, false),
        _ => return None,
    })
}

// -- El teclado de EE. UU. ----------------------------------------------------

/// El scancode de una tecla virtual (el primero que la da).
pub(crate) fn scancode_de(vk: u8) -> u8 {
    let vk = match vk {
        0xA0 => 0x10,
        0xA1 => return 0x36,
        0xA2 | 0xA3 => 0x11,
        0xA4 | 0xA5 => 0x12,
        v => v,
    };
    (1..0x80).find(|&sc| vk_de_scancode(sc) == Some(vk)).unwrap_or(0)
}

/// El caracter de una tecla sin nada pulsado (MAPVK_VK_TO_CHAR): las letras
/// en mayuscula, como Windows.
fn caracter_de(vk: u8) -> u16 {
    match vk {
        b'A'..=b'Z' | b'0'..=b'9' | 0x20 | 0x08 | 0x09 | 0x1B => vk as u16,
        0x0D => 0x0D,
        _ => 0,
    }
}

/// `ToUnicode`: lo que escribe una tecla con este estado del teclado (Shift
/// y Bloq Mayus para las letras; Ctrl+letra, su caracter de control).
pub(crate) fn escribe(vk: u8, teclas: &Teclas) -> Option<u16> {
    let shift = teclas.abajo(0x10) || teclas.abajo(0xA0) || teclas.abajo(0xA1);
    let ctrl = teclas.abajo(0x11) || teclas.abajo(0xA2) || teclas.abajo(0xA3);
    let mayus = teclas.0[0x14] & 1 != 0;
    Some(match vk {
        b'A'..=b'Z' if ctrl => (vk - b'A' + 1) as u16,
        b'A'..=b'Z' if shift != mayus => vk as u16,
        b'A'..=b'Z' => (vk + 32) as u16,
        b'0'..=b'9' if !shift => vk as u16,
        0x20 | 0x08 | 0x09 | 0x1B | 0x0D => vk as u16,
        _ => return None,
    })
}

// -- El estado de aqui ----------------------------------------------------------

struct Dispositivo {
    uso: u32,
    banderas: u32,
    hwnd: u64,
}

struct Estado {
    /// El del hilo (al sacar de la cola) y el de ahora (al llegar).
    sincronas: Teclas,
    asincronas: Teclas,
    /// El bit "pulsada desde la ultima vez" de GetAsyncKeyState.
    pulsadas: [bool; 256],
    cursor: (i32, i32),
    /// Donde estaba el raton del ultimo evento (para el raw input relativo).
    raton: Option<(i32, i32)>,
    visibles: i32,
    forma: u64,
    captura: u64,
    recorte: Option<[i32; 4]>,
    doble_clic: u32,
    crudos: Vec<Dispositivo>,
    /// Los ultimos RAWINPUT, por su HRAWINPUT.
    entregados: Vec<(u64, Vec<u8>)>,
    siguiente: u64,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado {
    sincronas: Teclas::nuevas(),
    asincronas: Teclas::nuevas(),
    pulsadas: [false; 256],
    cursor: (0, 0),
    raton: None,
    visibles: 0,
    forma: 0,
    captura: 0,
    recorte: None,
    doble_clic: 500,
    crudos: Vec::new(),
    entregados: Vec::new(),
    siguiente: 0,
}));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.sincronas = Teclas::nuevas();
    e.asincronas = Teclas::nuevas();
    e.pulsadas = [false; 256];
    e.cursor = (PANTALLA.0 / 2, PANTALLA.1 / 2);
    e.raton = None;
    e.visibles = 0;
    e.forma = cursor_de(32512);
    e.captura = 0;
    e.recorte = None;
    e.doble_clic = 500;
    e.crudos.clear();
    e.entregados.clear();
    e.siguiente = 0;
}

fn apretar_ahora(vk: u8, sc: u8, abajo: bool) {
    let e = estado();
    if abajo {
        e.pulsadas[vk as usize] = true;
    }
    e.asincronas.tecla(vk, sc, abajo);
}

// -- Lo que llega del escritorio ------------------------------------------------

const RIDEV_REMOVE: u32 = 0x01;
const RIDEV_NOLEGACY: u32 = 0x30;
const RAWINPUT_RATON: u64 = 0x5A1E_0001;
const RAWINPUT_TECLADO: u64 = 0x5A1E_0002;

fn crudo(uso: u32) -> Option<(u32, u64)> {
    estado().crudos.iter().find(|d| d.uso == uso).map(|d| (d.banderas, d.hwnd))
}

/// **Un evento del buzon de la ventana `hwnd`**, en todo lo que cambia (ver
/// la cabecera del modulo).
pub(crate) fn evento(hwnd: u64, ev: u64) {
    let m = de_evento(hwnd, ev);
    let raton = ev & (1 << 63) != 0;
    let mut publicar = Vec::new();
    if raton {
        let (x, y) = (((ev >> 16) & 0xFFFF) as i32, ((ev >> 32) & 0xFFFF) as i32);
        let (ox, oy) = crate::user32_ventanas::posicion(hwnd).unwrap_or((0, 0));
        let e = estado();
        let antes = e.raton.replace((x, y)).unwrap_or((x, y));
        e.cursor = (ox + x, oy + y);
        let botones = m.as_ref().and_then(tecla_de);
        if let Some((vk, sc, abajo)) = botones {
            apretar_ahora(vk, sc, abajo);
        }
        let legado = match crudo(0x0001_0002) {
            Some((banderas, destino)) => {
                let bits = match botones {
                    Some((1, _, true)) => 0x1,
                    Some((1, _, false)) => 0x2,
                    Some((2, _, true)) => 0x4,
                    Some((2, _, false)) => 0x8,
                    _ => 0,
                };
                let r = rawinput_raton(x - antes.0, y - antes.1, bits);
                publicar.push(entregar(if destino != 0 { destino } else { hwnd }, r));
                banderas & RIDEV_NOLEGACY == 0
            }
            None => true,
        };
        if legado {
            publicar.push(m.unwrap_or_else(|| {
                let a = &estado().asincronas;
                let mk = (a.abajo(1) as u64) | (a.abajo(2) as u64) << 1;
                Msg { hwnd, mensaje: WM_MOUSEMOVE, wparam: mk, lparam: (x as u64 & 0xFFFF) | (y as u64 & 0xFFFF) << 16 }
            }));
        }
    } else if let Some(m) = m {
        let tecla = tecla_de(&m);
        if let Some((vk, sc, abajo)) = tecla {
            apretar_ahora(vk, sc, abajo);
        }
        let legado = match (tecla, crudo(0x0001_0006)) {
            (Some((vk, sc, abajo)), Some((banderas, destino))) => {
                publicar.push(entregar(if destino != 0 { destino } else { hwnd }, rawinput_teclado(vk, sc, abajo)));
                banderas & RIDEV_NOLEGACY == 0
            }
            _ => true,
        };
        if legado {
            publicar.push(m);
        }
    }
    con(|e| {
        for m in publicar {
            e.cola.publicar(m);
        }
    });
}

/// Un mensaje SALE de la cola (GetMessage, PeekMessage con PM_REMOVE): el
/// estado del hilo cambia ahora, como en Windows.
pub(crate) fn leido(m: &Msg) {
    if let Some((vk, sc, abajo)) = tecla_de(m) {
        estado().sincronas.tecla(vk, sc, abajo);
    }
}

/// El RAWINPUT de un raton: la cabecera (24) y el RAWMOUSE (24).
pub(crate) fn rawinput_raton(dx: i32, dy: i32, botones: u16) -> Vec<u8> {
    let mut r = cabecera(0, 48, RAWINPUT_RATON);
    r.extend_from_slice(&0u16.to_le_bytes()); // usFlags: MOUSE_MOVE_RELATIVE
    r.extend_from_slice(&0u16.to_le_bytes());
    r.extend_from_slice(&botones.to_le_bytes()); // usButtonFlags
    r.extend_from_slice(&0u16.to_le_bytes()); // usButtonData
    r.extend_from_slice(&0u32.to_le_bytes()); // ulRawButtons
    r.extend_from_slice(&dx.to_le_bytes());
    r.extend_from_slice(&dy.to_le_bytes());
    r.extend_from_slice(&0u32.to_le_bytes()); // ulExtraInformation
    r
}

/// El RAWINPUT de un teclado: la cabecera (24) y el RAWKEYBOARD (16).
pub(crate) fn rawinput_teclado(vk: u8, sc: u8, abajo: bool) -> Vec<u8> {
    let mut r = cabecera(1, 40, RAWINPUT_TECLADO);
    r.extend_from_slice(&(sc as u16).to_le_bytes()); // MakeCode
    r.extend_from_slice(&(!abajo as u16).to_le_bytes()); // Flags: RI_KEY_BREAK
    r.extend_from_slice(&0u16.to_le_bytes());
    r.extend_from_slice(&(vk as u16).to_le_bytes()); // VKey
    r.extend_from_slice(&(if abajo { WM_KEYDOWN } else { WM_KEYUP }).to_le_bytes());
    r.extend_from_slice(&0u32.to_le_bytes());
    r
}

fn cabecera(tipo: u32, medida: u32, dispositivo: u64) -> Vec<u8> {
    let mut r = Vec::with_capacity(medida as usize);
    r.extend_from_slice(&tipo.to_le_bytes());
    r.extend_from_slice(&medida.to_le_bytes());
    r.extend_from_slice(&dispositivo.to_le_bytes());
    r.extend_from_slice(&0u64.to_le_bytes()); // wParam: RIM_INPUT
    r
}

/// Guardar un RAWINPUT y el WM_INPUT que lo lleva (sus ultimos 64 viven).
fn entregar(hwnd: u64, r: Vec<u8>) -> Msg {
    let e = estado();
    e.siguiente += 1;
    let h = 0x5A1F_0000 + e.siguiente;
    if e.entregados.len() >= 64 {
        e.entregados.remove(0);
    }
    e.entregados.push((h, r));
    Msg { hwnd, mensaje: WM_INPUT, wparam: 0, lparam: h }
}

// -- El estado de las teclas ------------------------------------------------------

extern "win64" fn get_key_state(vk: i32) -> i16 {
    let t = estado().sincronas.0[(vk & 0xFF) as usize];
    ((if t & 0x80 != 0 { 0xFF80u16 } else { 0 }) | (t & 1) as u16) as i16
}

extern "win64" fn get_async_key_state(vk: i32) -> i16 {
    let e = estado();
    let i = (vk & 0xFF) as usize;
    let antes = core::mem::replace(&mut e.pulsadas[i], false);
    ((if e.asincronas.0[i] & 0x80 != 0 { 0x8000u16 } else { 0 }) | antes as u16) as i16
}

extern "win64" fn get_keyboard_state(t: *mut u8) -> i32 {
    if t.is_null() {
        return 0;
    }
    // SAFETY: 256 bytes suyos.
    unsafe { core::ptr::copy_nonoverlapping(estado().sincronas.0.as_ptr(), t, 256) };
    1
}

extern "win64" fn set_keyboard_state(t: *const u8) -> i32 {
    if t.is_null() {
        return 0;
    }
    // SAFETY: 256 bytes suyos.
    unsafe { core::ptr::copy_nonoverlapping(t, estado().sincronas.0.as_mut_ptr(), 256) };
    1
}

/// `MapVirtualKey(Ex)`: 0 VK->scancode, 1 scancode->VK, 2 VK->caracter, 3
/// scancode->VK con lado (VK_LSHIFT...), 4 VK->scancode (como 0).
extern "win64" fn map_virtual_key(codigo: u32, tipo: u32) -> u32 {
    let c = (codigo & 0xFF) as u8;
    match tipo {
        0 | 4 => scancode_de(c) as u32,
        1 | 3 => match vk_de_scancode(c & 0x7F) {
            Some(0x10) if tipo == 3 => if c == 0x36 { 0xA1 } else { 0xA0 },
            Some(0x11) if tipo == 3 => 0xA2,
            Some(0x12) if tipo == 3 => 0xA4,
            Some(v) => v as u32,
            None => 0,
        },
        2 => caracter_de(c) as u32,
        _ => 0,
    }
}

extern "win64" fn map_virtual_key_ex(codigo: u32, tipo: u32, _distribucion: u64) -> u32 {
    map_virtual_key(codigo, tipo)
}

/// `VkKeyScan`: la tecla (abajo) y el Shift (arriba, 1) de un caracter;
/// -1 si no hay.
extern "win64" fn vk_key_scan(c: u16) -> i16 {
    match c {
        0x61..=0x7A => (c - 32) as i16,
        0x41..=0x5A => (c | 0x100) as i16,
        0x30..=0x39 | 0x20 | 0x08 | 0x09 | 0x1B | 0x0D => c as i16,
        _ => -1,
    }
}

extern "win64" fn vk_key_scan_ex(c: u16, _distribucion: u64) -> i16 {
    vk_key_scan(c)
}

fn to_unicode(vk: u32, estado_teclas: *const u8, buf: u64, n: i32, ansi: bool) -> i32 {
    let mut t = Teclas::nuevas();
    if !estado_teclas.is_null() {
        // SAFETY: los 256 bytes del `.exe`.
        unsafe { core::ptr::copy_nonoverlapping(estado_teclas, t.0.as_mut_ptr(), 256) };
    }
    match escribe(vk as u8, &t) {
        Some(c) => {
            if n <= 0 || buf == 0 {
                return 0;
            }
            // SAFETY: `n` caracteres suyos (ToAscii: un WORD, sin cero).
            unsafe {
                (buf as *mut u16).write(c);
                if !ansi && n > 1 {
                    (buf as *mut u16).add(1).write(0);
                }
            }
            1
        }
        None => 0,
    }
}

extern "win64" fn to_unicode_w(vk: u32, _sc: u32, t: *const u8, buf: *mut u16, n: i32, _banderas: u32) -> i32 {
    to_unicode(vk, t, buf as u64, n, false)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn to_unicode_ex(vk: u32, _sc: u32, t: *const u8, buf: *mut u16, n: i32, _banderas: u32, _d: u64) -> i32 {
    to_unicode(vk, t, buf as u64, n, false)
}

extern "win64" fn to_ascii(vk: u32, _sc: u32, t: *const u8, buf: *mut u16, _banderas: u32) -> i32 {
    to_unicode(vk, t, buf as u64, 1, true)
}

/// El nombre de una tecla por su scancode (el de GetKeyNameText: el
/// scancode en los bits 16..23 del lParam de WM_KEYDOWN).
fn nombre_tecla(sc: u8) -> Vec<u16> {
    let fijo = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    match sc {
        0x01 => fijo("Esc"),
        0x0E => fijo("Backspace"),
        0x0F => fijo("Tab"),
        0x1C => fijo("Enter"),
        0x1D => fijo("Ctrl"),
        0x2A => fijo("Shift"),
        0x36 => fijo("Right Shift"),
        0x38 => fijo("Alt"),
        0x39 => fijo("Space"),
        _ => match vk_de_scancode(sc) {
            Some(v @ (b'A'..=b'Z' | b'0'..=b'9')) => alloc::vec![v as u16],
            Some(v @ 0x70..=0x79) => {
                let mut s = fijo("F");
                let k = v - 0x6F;
                if k >= 10 {
                    s.push(b'1' as u16);
                }
                s.push((b'0' + k % 10) as u16);
                s
            }
            _ => Vec::new(),
        },
    }
}

extern "win64" fn get_key_name_text_w(l: i32, buf: *mut u16, n: i32) -> i32 {
    copiar_texto(&nombre_tecla((l >> 16) as u8), buf as u64, n.max(0) as usize, false) as i32
}

extern "win64" fn get_key_name_text_a(l: i32, buf: *mut u8, n: i32) -> i32 {
    copiar_texto(&nombre_tecla((l >> 16) as u8), buf as u64, n.max(0) as usize, true) as i32
}

/// La distribucion de la casa: ingles de EE. UU. (0409).
const DISTRIBUCION: u64 = 0x0409_0409;

extern "win64" fn get_keyboard_layout(_hilo: u32) -> u64 {
    DISTRIBUCION
}

extern "win64" fn get_keyboard_layout_list(n: i32, lista: *mut u64) -> i32 {
    if n > 0 && !lista.is_null() {
        // SAFETY: `n` HKL suyos.
        unsafe { lista.write(DISTRIBUCION) };
    }
    1
}

extern "win64" fn get_keyboard_layout_name(buf: u64, ansi: bool) -> i32 {
    let t: Vec<u16> = "00000409".encode_utf16().collect();
    copiar_texto(&t, buf, 9, ansi);
    1
}

extern "win64" fn get_keyboard_layout_name_w(buf: *mut u16) -> i32 {
    get_keyboard_layout_name(buf as u64, false)
}

extern "win64" fn get_keyboard_layout_name_a(buf: *mut u8) -> i32 {
    get_keyboard_layout_name(buf as u64, true)
}

extern "win64" fn activate_keyboard_layout(_hkl: u64, _banderas: u32) -> u64 {
    DISTRIBUCION
}

/// `GetKeyboardType`: 4 (el de 101/102 teclas), sin subtipo, 12 de funcion.
extern "win64" fn get_keyboard_type(que: i32) -> i32 {
    match que {
        0 => 4,
        2 => 12,
        _ => 0,
    }
}

// -- El cursor --------------------------------------------------------------------

fn poner_punto(p: *mut i32, (x, y): (i32, i32)) -> i32 {
    if p.is_null() {
        return 0;
    }
    // SAFETY: un POINT suyo.
    unsafe {
        p.write(x);
        p.add(1).write(y);
    }
    1
}

extern "win64" fn get_cursor_pos(p: *mut i32) -> i32 {
    poner_punto(p, estado().cursor)
}

/// `SetCursorPos`: dentro del recorte (o de la pantalla).
extern "win64" fn set_cursor_pos(x: i32, y: i32) -> i32 {
    let r = recorte();
    estado().cursor = (x.clamp(r[0], r[2] - 1), y.clamp(r[1], r[3] - 1));
    1
}

fn recorte() -> [i32; 4] {
    estado().recorte.unwrap_or([0, 0, PANTALLA.0, PANTALLA.1])
}

/// `GetCursorInfo`: CURSORINFO (cbSize 24, flags, hCursor +8, pt +16).
extern "win64" fn get_cursor_info(ci: *mut u8) -> i32 {
    if ci.is_null() {
        return 0;
    }
    let e = estado();
    // SAFETY: un CURSORINFO suyo de 24 bytes.
    unsafe {
        (ci.add(4) as *mut u32).write((e.visibles >= 0) as u32);
        (ci.add(8) as *mut u64).write_unaligned(e.forma);
        (ci.add(16) as *mut i32).write(e.cursor.0);
        (ci.add(20) as *mut i32).write(e.cursor.1);
    }
    1
}

/// `ShowCursor`: el contador (se ve con 0 o mas), y lo devuelve.
extern "win64" fn show_cursor(si: i32) -> i32 {
    let e = estado();
    e.visibles += if si != 0 { 1 } else { -1 };
    e.visibles
}

extern "win64" fn set_cursor(c: u64) -> u64 {
    core::mem::replace(&mut estado().forma, c)
}

extern "win64" fn get_cursor() -> u64 {
    estado().forma
}

/// Los cursores e iconos del sistema: un numero por su IDC_/IDI_, que no es
/// un puntero (el cursor lo pinta el escritorio de BMO-X).
fn cursor_de(id: u64) -> u64 {
    0x5A1D_C000_0000 | (id & 0xFFFF)
}

extern "win64" fn load_cursor_a(_inst: u64, id: u64) -> u64 {
    cursor_de(id)
}

extern "win64" fn load_icon(_inst: u64, id: u64) -> u64 {
    0x5A1D_1000_0000 | (id & 0xFFFF)
}

extern "win64" fn destruir(_h: u64) -> i32 {
    1
}

/// `ClipCursor`: NULL suelta; si no, el cursor vive dentro (y se mete).
extern "win64" fn clip_cursor(r: *const i32) -> i32 {
    let e = estado();
    e.recorte = if r.is_null() {
        None
    } else {
        // SAFETY: un RECT suyo.
        let r = unsafe { [r.read(), r.add(1).read(), r.add(2).read(), r.add(3).read()] };
        let (x0, y0) = (r[0].max(0), r[1].max(0));
        Some([x0, y0, r[2].min(PANTALLA.0).max(x0 + 1), r[3].min(PANTALLA.1).max(y0 + 1)])
    };
    let (x, y) = e.cursor;
    set_cursor_pos(x, y)
}

extern "win64" fn get_clip_cursor(r: *mut i32) -> i32 {
    if r.is_null() {
        return 0;
    }
    let c = recorte();
    // SAFETY: un RECT suyo.
    unsafe { core::ptr::copy_nonoverlapping(c.as_ptr(), r, 4) };
    1
}

extern "win64" fn set_capture(h: u64) -> u64 {
    core::mem::replace(&mut estado().captura, h)
}

extern "win64" fn get_capture() -> u64 {
    estado().captura
}

extern "win64" fn release_capture() -> i32 {
    estado().captura = 0;
    1
}

extern "win64" fn track_mouse_event(t: *const u8) -> i32 {
    (!t.is_null()) as i32
}

extern "win64" fn get_double_click_time() -> u32 {
    estado().doble_clic
}

extern "win64" fn set_double_click_time(ms: u32) -> i32 {
    estado().doble_clic = if ms == 0 { 500 } else { ms };
    1
}

/// `WindowFromPoint`: la ultima mostrada que tenga el punto; si no, el
/// escritorio.
extern "win64" fn window_from_point(p: u64) -> u64 {
    let (x, y) = (p as u32 as i32, (p >> 32) as u32 as i32);
    let vistas = con(|e| e.ventanas.iter().filter(|v| v.viva && v.mostrada).map(|v| (v.hwnd, v.datos.x, v.datos.y, v.sup.ancho as i32, v.sup.alto as i32)).collect::<Vec<_>>());
    vistas.iter().rev().find(|v| x >= v.1 && y >= v.2 && x < v.1 + v.3 && y < v.2 + v.4).map_or(crate::user32_medidas::ESCRITORIO, |v| v.0)
}

// -- La entrada inventada ------------------------------------------------------------

/// A quien va una entrada inventada: la de la captura, la del foco o la de
/// delante.
fn destino() -> u64 {
    let c = estado().captura;
    if c != 0 {
        return c;
    }
    crate::user32_ventanas::foco()
}

/// Una tecla inventada (KEYBDINPUT): KEYEVENTF_KEYUP 2, KEYEVENTF_UNICODE 4
/// (un WM_CHAR con ese caracter), KEYEVENTF_SCANCODE 8.
fn teclear(vk: u16, sc: u16, banderas: u32) {
    let h = destino();
    let arriba = banderas & 2 != 0;
    if banderas & 4 != 0 {
        if !arriba && h != 0 {
            con(|e| e.cola.publicar(Msg { hwnd: h, mensaje: WM_CHAR, wparam: sc as u64, lparam: 1 }));
        }
        return;
    }
    let vk = if banderas & 8 != 0 { vk_de_scancode(sc as u8).unwrap_or(0) } else { vk as u8 };
    let sc = if banderas & 8 != 0 { sc as u8 } else { scancode_de(vk) };
    apretar_ahora(vk, sc, !arriba);
    if h != 0 {
        let lparam = 1 | (sc as u64) << 16 | if arriba { 3 << 30 } else { 0 };
        con(|e| e.cola.publicar(Msg { hwnd: h, mensaje: if arriba { WM_KEYUP } else { WM_KEYDOWN }, wparam: vk as u64, lparam }));
    }
}

/// Un raton inventado (MOUSEINPUT): MOVE 1 (relativo; con ABSOLUTE 0x8000,
/// de 0 a 65535 en la pantalla), LEFTDOWN 2, LEFTUP 4, RIGHTDOWN 8, RIGHTUP
/// 0x10.
fn mover(dx: i32, dy: i32, banderas: u32) {
    if banderas & 1 != 0 {
        let (x, y) = if banderas & 0x8000 != 0 {
            ((dx as i64 * PANTALLA.0 as i64 / 65536) as i32, (dy as i64 * PANTALLA.1 as i64 / 65536) as i32)
        } else {
            let c = estado().cursor;
            (c.0 + dx, c.1 + dy)
        };
        set_cursor_pos(x, y);
    }
    let h = destino();
    let (cx, cy) = estado().cursor;
    let (ox, oy) = crate::user32_ventanas::posicion(h).unwrap_or((0, 0));
    let lparam = ((cx - ox) as u64 & 0xFFFF) | ((cy - oy) as u64 & 0xFFFF) << 16;
    for (bit, m, vk, abajo) in [(2, WM_LBUTTONDOWN, 1, true), (4, WM_LBUTTONUP, 1, false), (8, WM_RBUTTONDOWN, 2, true), (0x10, WM_RBUTTONUP, 2, false)] {
        if banderas & bit != 0 {
            apretar_ahora(vk, 0, abajo);
            if h != 0 {
                con(|e| e.cola.publicar(Msg { hwnd: h, mensaje: m, wparam: 0, lparam }));
            }
        }
    }
    if banderas & 1 != 0 && h != 0 {
        con(|e| e.cola.publicar(Msg { hwnd: h, mensaje: WM_MOUSEMOVE, wparam: 0, lparam }));
    }
}

/// `SendInput(n, INPUT[], 40)`: cuantos entraron.
extern "win64" fn send_input(n: u32, entradas: *const u8, medida: i32) -> u32 {
    if entradas.is_null() || medida != 40 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    for k in 0..n as usize {
        // SAFETY: `n` INPUT suyos de 40 bytes.
        let i = unsafe { entradas.add(40 * k) };
        let d = |o: usize| unsafe { (i.add(o) as *const u32).read_unaligned() };
        match d(0) {
            0 => mover(d(8) as i32, d(12) as i32, d(20)),
            1 => teclear(d(8) as u16, (d(8) >> 16) as u16, d(12)),
            _ => {}
        }
    }
    n
}

extern "win64" fn keybd_event(vk: u8, sc: u8, banderas: u32, _extra: u64) {
    teclear(vk as u16, sc as u16, banderas & !4);
}

extern "win64" fn mouse_event(banderas: u32, dx: i32, dy: i32, _datos: u32, _extra: u64) {
    mover(dx, dy, banderas);
}

// -- El raw input --------------------------------------------------------------------

/// `RegisterRawInputDevices(RAWINPUTDEVICE[], n, 16)`: pagina y uso (1,2 el
/// raton; 1,6 el teclado), banderas y la ventana destino.
extern "win64" fn register_raw_input_devices(d: *const u8, n: u32, medida: u32) -> i32 {
    if d.is_null() || medida != 16 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    for k in 0..n as usize {
        // SAFETY: `n` RAWINPUTDEVICE suyos de 16 bytes.
        let (uso, banderas, hwnd) = unsafe {
            let p = d.add(16 * k);
            ((p as *const u32).read_unaligned(), (p.add(4) as *const u32).read_unaligned(), (p.add(8) as *const u64).read_unaligned())
        };
        let uso = uso.rotate_left(16); // (pagina << 16) | uso
        if banderas & RIDEV_REMOVE != 0 && hwnd != 0 {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            return 0;
        }
        let e = estado();
        e.crudos.retain(|x| x.uso != uso);
        if banderas & RIDEV_REMOVE == 0 {
            e.crudos.push(Dispositivo { uso, banderas, hwnd });
        }
    }
    1
}

/// Poner `n` elementos de `medida` bytes en un bufer del `.exe` con su
/// cuenta, a la manera de GetRawInput*: sin bufer, la cuenta y 0; corto,
/// ERROR_INSUFFICIENT_BUFFER y -1; si cabe, cuantos.
fn dar_lista(buf: *mut u8, cuantos: *mut u32, medida: usize, elementos: &[Vec<u8>]) -> u32 {
    if cuantos.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return u32::MAX;
    }
    // SAFETY: un UINT suyo.
    let caben = unsafe { cuantos.read() } as usize;
    // SAFETY: lo mismo.
    unsafe { cuantos.write(elementos.len() as u32) };
    if buf.is_null() {
        return 0;
    }
    if caben < elementos.len() {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return u32::MAX;
    }
    for (k, x) in elementos.iter().enumerate() {
        // SAFETY: `caben` elementos de `medida` bytes.
        unsafe { core::ptr::copy_nonoverlapping(x.as_ptr(), buf.add(k * medida), medida) };
    }
    elementos.len() as u32
}

extern "win64" fn get_registered_raw_input_devices(buf: *mut u8, n: *mut u32, medida: u32) -> u32 {
    if medida != 16 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return u32::MAX;
    }
    let l: Vec<Vec<u8>> = estado()
        .crudos
        .iter()
        .map(|d| {
            let mut v = d.uso.rotate_left(16).to_le_bytes().to_vec();
            v.extend_from_slice(&d.banderas.to_le_bytes());
            v.extend_from_slice(&d.hwnd.to_le_bytes());
            v
        })
        .collect();
    dar_lista(buf, n, 16, &l)
}

/// `GetRawInputDeviceList`: un raton (tipo 0) y un teclado (tipo 1).
extern "win64" fn get_raw_input_device_list(buf: *mut u8, n: *mut u32, medida: u32) -> u32 {
    if medida != 16 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return u32::MAX;
    }
    let uno = |h: u64, t: u64| [h.to_le_bytes(), t.to_le_bytes()].concat();
    dar_lista(buf, n, 16, &[uno(RAWINPUT_RATON, 0), uno(RAWINPUT_TECLADO, 1)])
}

/// `GetRawInputDeviceInfo(dispositivo, que, datos, *medida)`: RIDI_DEVICEINFO
/// (RID_DEVICE_INFO, 32 bytes) y RIDI_DEVICENAME (en caracteres).
fn device_info(d: u64, que: u32, datos: *mut u8, medida: *mut u32, ansi: bool) -> u32 {
    if (d != RAWINPUT_RATON && d != RAWINPUT_TECLADO) || medida.is_null() {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return u32::MAX;
    }
    let raton = d == RAWINPUT_RATON;
    let (bytes_, en_caracteres): (Vec<u8>, bool) = match que {
        0x2000_000B => {
            let campos: [u32; 8] = if raton { [32, 0, 256, 5, 1000, 1, 0, 0] } else { [32, 1, 4, 0, 1, 12, 3, 101] };
            (campos.iter().flat_map(|c| c.to_le_bytes()).collect(), false)
        }
        0x2000_0007 => {
            let n = if raton { "\\\\?\\BMO#RATON#0" } else { "\\\\?\\BMO#TECLADO#0" };
            let t: Vec<u16> = n.encode_utf16().chain(core::iter::once(0)).collect();
            let b = if ansi { t.iter().map(|&c| c as u8).collect() } else { t.iter().flat_map(|c| c.to_le_bytes()).collect() };
            (b, true)
        }
        _ => (Vec::new(), false),
    };
    let unidad = if en_caracteres && !ansi { 2 } else { 1 };
    let pide = (bytes_.len() / unidad) as u32;
    // SAFETY: un UINT suyo.
    let hay = unsafe { medida.read() };
    if datos.is_null() {
        // SAFETY: lo mismo.
        unsafe { medida.write(pide) };
        return 0;
    }
    if hay < pide {
        // SAFETY: lo mismo.
        unsafe { medida.write(pide) };
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return u32::MAX;
    }
    // SAFETY: `hay` unidades suyas.
    unsafe { core::ptr::copy_nonoverlapping(bytes_.as_ptr(), datos, bytes_.len()) };
    pide
}

extern "win64" fn get_raw_input_device_info_w(d: u64, que: u32, datos: *mut u8, medida: *mut u32) -> u32 {
    device_info(d, que, datos, medida, false)
}

extern "win64" fn get_raw_input_device_info_a(d: u64, que: u32, datos: *mut u8, medida: *mut u32) -> u32 {
    device_info(d, que, datos, medida, true)
}

/// `GetRawInputData(HRAWINPUT, RID_INPUT/RID_HEADER, datos, *medida, 24)`.
extern "win64" fn get_raw_input_data(h: u64, que: u32, datos: *mut u8, medida: *mut u32, cabecera: u32) -> u32 {
    let Some(r) = estado().entregados.iter().find(|x| x.0 == h).map(|x| x.1.clone()) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return u32::MAX;
    };
    if cabecera != 24 || medida.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return u32::MAX;
    }
    let r = if que == 0x1000_0005 { &r[..24] } else { &r[..] };
    // SAFETY: un UINT suyo.
    let hay = unsafe { medida.read() } as usize;
    if datos.is_null() {
        // SAFETY: lo mismo.
        unsafe { medida.write(r.len() as u32) };
        return 0;
    }
    if hay < r.len() {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return u32::MAX;
    }
    // SAFETY: `hay` bytes suyos.
    unsafe { core::ptr::copy_nonoverlapping(r.as_ptr(), datos, r.len()) };
    r.len() as u32
}

/// `GetRawInputBuffer`: aqui todo llega por WM_INPUT; en el bufer, nada.
extern "win64" fn get_raw_input_buffer(_datos: *mut u8, medida: *mut u32, _cabecera: u32) -> u32 {
    if !medida.is_null() {
        // SAFETY: un UINT suyo.
        unsafe { medida.write(0) };
    }
    0
}

extern "win64" fn def_raw_input_proc(_p: u64, _n: i32, _cabecera: u32) -> i64 {
    0
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "GetKeyState" => dir!(get_key_state),
        "GetAsyncKeyState" => dir!(get_async_key_state),
        "GetKeyboardState" => dir!(get_keyboard_state),
        "SetKeyboardState" => dir!(set_keyboard_state),
        "MapVirtualKeyW" | "MapVirtualKeyA" => dir!(map_virtual_key),
        "MapVirtualKeyExW" | "MapVirtualKeyExA" => dir!(map_virtual_key_ex),
        "VkKeyScanW" | "VkKeyScanA" => dir!(vk_key_scan),
        "VkKeyScanExW" | "VkKeyScanExA" => dir!(vk_key_scan_ex),
        "ToUnicode" => dir!(to_unicode_w),
        "ToUnicodeEx" => dir!(to_unicode_ex),
        "ToAscii" => dir!(to_ascii),
        "GetKeyNameTextW" => dir!(get_key_name_text_w),
        "GetKeyNameTextA" => dir!(get_key_name_text_a),
        "GetKeyboardLayout" => dir!(get_keyboard_layout),
        "GetKeyboardLayoutList" => dir!(get_keyboard_layout_list),
        "GetKeyboardLayoutNameW" => dir!(get_keyboard_layout_name_w),
        "GetKeyboardLayoutNameA" => dir!(get_keyboard_layout_name_a),
        "ActivateKeyboardLayout" => dir!(activate_keyboard_layout),
        "GetKeyboardType" => dir!(get_keyboard_type),
        "GetCursorPos" | "GetPhysicalCursorPos" => dir!(get_cursor_pos),
        "SetCursorPos" | "SetPhysicalCursorPos" => dir!(set_cursor_pos),
        "GetCursorInfo" => dir!(get_cursor_info),
        "ShowCursor" => dir!(show_cursor),
        "SetCursor" => dir!(set_cursor),
        "GetCursor" => dir!(get_cursor),
        "LoadCursorA" => dir!(load_cursor_a),
        "LoadIconW" | "LoadIconA" => dir!(load_icon),
        "DestroyCursor" | "DestroyIcon" => dir!(destruir),
        "ClipCursor" => dir!(clip_cursor),
        "GetClipCursor" => dir!(get_clip_cursor),
        "SetCapture" => dir!(set_capture),
        "GetCapture" => dir!(get_capture),
        "ReleaseCapture" => dir!(release_capture),
        "TrackMouseEvent" => dir!(track_mouse_event),
        "GetDoubleClickTime" => dir!(get_double_click_time),
        "SetDoubleClickTime" => dir!(set_double_click_time),
        "WindowFromPoint" | "WindowFromPhysicalPoint" => dir!(window_from_point),
        "SendInput" => dir!(send_input),
        "keybd_event" => dir!(keybd_event),
        "mouse_event" => dir!(mouse_event),
        "RegisterRawInputDevices" => dir!(register_raw_input_devices),
        "GetRegisteredRawInputDevices" => dir!(get_registered_raw_input_devices),
        "GetRawInputDeviceList" => dir!(get_raw_input_device_list),
        "GetRawInputDeviceInfoW" => dir!(get_raw_input_device_info_w),
        "GetRawInputDeviceInfoA" => dir!(get_raw_input_device_info_a),
        "GetRawInputData" => dir!(get_raw_input_data),
        "GetRawInputBuffer" => dir!(get_raw_input_buffer),
        "DefRawInputProc" => dir!(def_raw_input_proc),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_teclas_y_su_interruptor() {
        let mut t = Teclas::nuevas();
        t.tecla(0x14, 0x3A, true);
        assert_eq!(t.0[0x14], 0x81, "abajo, y el interruptor encendido");
        t.tecla(0x14, 0x3A, true);
        assert_eq!(t.0[0x14], 0x81, "la repeticion no lo cambia");
        t.tecla(0x14, 0x3A, false);
        assert_eq!(t.0[0x14], 0x01);
        t.tecla(0x10, 0x36, true);
        assert!(t.abajo(0x10) && t.abajo(0xA1) && !t.abajo(0xA0), "el Shift de la derecha");
    }

    #[test]
    fn lo_que_escribe_una_tecla() {
        let mut t = Teclas::nuevas();
        assert_eq!(escribe(b'A', &t), Some(b'a' as u16));
        t.0[0x10] = 0x80;
        assert_eq!(escribe(b'A', &t), Some(b'A' as u16));
        t.0[0x14] = 1;
        assert_eq!(escribe(b'A', &t), Some(b'a' as u16), "Shift con Bloq Mayus: minuscula");
        assert_eq!(escribe(b'1', &t), None, "Shift+1 depende de la distribucion");
        let mut c = Teclas::nuevas();
        c.0[0x11] = 0x80;
        assert_eq!(escribe(b'C', &c), Some(3), "Ctrl+C");
    }

    #[test]
    fn scancodes_y_rawinput() {
        assert_eq!(scancode_de(b'A'), 0x1E);
        assert_eq!(scancode_de(0x1B), 0x01);
        assert_eq!(scancode_de(0xA1), 0x36);
        let r = rawinput_raton(-3, 7, 1);
        assert_eq!(r.len(), 48);
        assert_eq!(&r[36..40], &(-3i32).to_le_bytes());
        assert_eq!(&r[40..44], &7i32.to_le_bytes());
        let k = rawinput_teclado(b'A', 0x1E, false);
        assert_eq!(k.len(), 40);
        assert_eq!(u16::from_le_bytes([k[26], k[27]]), 1, "RI_KEY_BREAK al soltar");
        let m = Msg { hwnd: 1, mensaje: WM_KEYDOWN, wparam: b'A' as u64, lparam: 1 | 0x1E << 16 };
        assert_eq!(tecla_de(&m), Some((b'A', 0x1E, true)));
    }
}
