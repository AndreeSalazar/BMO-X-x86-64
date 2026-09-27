//! **Las ventanas de Win32, sin la memoria** (P2, 27-09): la cola de mensajes,
//! la traduccion de lo que manda el escritorio de BMO-X y la copia de un DIB.
//!
//! Lo que hace `user32` por dentro y se puede decir con bytes: que mensaje sale
//! antes que cual, que `WM_*` es una tecla de BMO-X, que `VK_*` es un scancode,
//! y como cae un DIB de 32 bits en los pixeles de una superficie. Quien llama a
//! la `WndProc` del `.exe` y quien pide la superficie es `bmo-proton-x-casa`;
//! aqui no hay ni un puntero.
//!
//! # La cola, en el orden de Windows
//!
//! ```text
//!    1  lo PUBLICADO y la ENTRADA, en el orden en que llego
//!    2  WM_QUIT, si alguien llamo a PostQuitMessage y ya no queda nada de 1
//!    3  WM_PAINT, de una ventana invalidada: se SINTETIZA al pedirlo, nunca
//!       se encola. Diez InvalidateRect seguidos son UN WM_PAINT
//! ```
//!
//! Es lo que documenta Microsoft para `GetMessage` (los mensajes enviados, que
//! aqui no hay, van antes de todo; WM_TIMER despues de WM_PAINT). Y es lo que
//! hace que un `.exe` que invalida en cada tecla no pinte cien veces.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

pub const WM_CREATE: u32 = 0x0001;
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_QUIT: u32 = 0x0012;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
pub const WM_CHAR: u32 = 0x0102;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_RBUTTONDOWN: u32 = 0x0204;
pub const WM_RBUTTONUP: u32 = 0x0205;

/// `wParam` de los botones: cual esta pulsado (MK_LBUTTON, MK_RBUTTON).
pub const MK_LBUTTON: u64 = 0x0001;
pub const MK_RBUTTON: u64 = 0x0002;

/// Un mensaje, como el `MSG` de Windows sin la hora ni el punto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Msg {
    pub hwnd: u64,
    pub mensaje: u32,
    pub wparam: u64,
    pub lparam: u64,
}

// -- Los eventos del buzon de BMO-X (bmo_abi `SUP_EV_*`) -------------------

const EV_RATON: u64 = 1 << 63;
const EV_LETRA: u64 = 1 << 62;
const EV_CONFIGURAR: u64 = 1 << 61;
const EV_HAY: u64 = 1 << 8;
const EV_PULSADA: u64 = 1 << 9;

/// **La tecla virtual de Windows** (`VK_*`) de un scancode del set 1, el que
/// da el teclado de BMO-X. Las que no estan no son un `VK` inventado: son
/// `None`, y esa tecla solo llega como `WM_CHAR` si escribe algo.
pub fn vk_de_scancode(sc: u8) -> Option<u8> {
    const LETRAS: [(u8, &[u8]); 3] = [(0x10, b"QWERTYUIOP"), (0x1E, b"ASDFGHJKL"), (0x2C, b"ZXCVBNM")];
    for (desde, fila) in LETRAS {
        if sc >= desde && ((sc - desde) as usize) < fila.len() {
            return Some(fila[(sc - desde) as usize]);
        }
    }
    Some(match sc {
        0x01 => 0x1B,               // VK_ESCAPE
        0x02..=0x0A => b'1' + sc - 0x02,
        0x0B => b'0',
        0x0E => 0x08,               // VK_BACK
        0x0F => 0x09,               // VK_TAB
        0x1C => 0x0D,               // VK_RETURN
        0x1D => 0x11,               // VK_CONTROL
        0x2A | 0x36 => 0x10,        // VK_SHIFT
        0x38 => 0x12,               // VK_MENU (Alt)
        0x39 => 0x20,               // VK_SPACE
        0x3B..=0x44 => 0x70 + sc - 0x3B, // VK_F1..VK_F10
        0x48 => 0x26,               // VK_UP
        0x4B => 0x25,               // VK_LEFT
        0x4D => 0x27,               // VK_RIGHT
        0x50 => 0x28,               // VK_DOWN
        _ => return None,
    })
}

/// **Un evento del buzon de BMO-X, como mensaje de Windows** para `hwnd`.
///
/// ```text
///    letra (bit 62)        WM_CHAR con la letra (Latin-1 = los primeros 256
///                          de UTF-16: la letra ES el caracter)
///    raton (bit 63)        WM_LBUTTONDOWN/UP o WM_RBUTTONDOWN/UP, con
///                          lParam = x | y << 16 dentro de la ventana
///    scancode (sin bit alto)  WM_KEYDOWN / WM_KEYUP con su VK_*
///    configurar (bit 61)   nada todavia: la ventana no cambia de medida (P2)
/// ```
///
/// Un raton sin boton (solo se movio) tampoco es mensaje: WM_MOUSEMOVE llega
/// cuando un `.exe` lo pida.
pub fn de_evento(hwnd: u64, e: u64) -> Option<Msg> {
    if e & EV_HAY == 0 && e & (EV_RATON | EV_LETRA) == 0 {
        return None;
    }
    let pulsada = e & EV_PULSADA != 0;
    let m = |mensaje, wparam, lparam| Some(Msg { hwnd, mensaje, wparam, lparam });
    if e & EV_RATON != 0 {
        let (x, y) = ((e >> 16) & 0xFFFF, (e >> 32) & 0xFFFF);
        let lparam = x | y << 16;
        let botones = e & 0xFF;
        return match (botones & 1 != 0, botones & 2 != 0, pulsada) {
            (true, _, true) => m(WM_LBUTTONDOWN, MK_LBUTTON, lparam),
            (true, _, false) => m(WM_LBUTTONUP, 0, lparam),
            (false, true, true) => m(WM_RBUTTONDOWN, MK_RBUTTON, lparam),
            (false, true, false) => m(WM_RBUTTONUP, 0, lparam),
            _ => None,
        };
    }
    if e & EV_LETRA != 0 {
        return m(WM_CHAR, e & 0xFF, 1);
    }
    if e & EV_CONFIGURAR != 0 {
        return None;
    }
    let sc = (e & 0x7F) as u8;
    let vk = vk_de_scancode(sc)? as u64;
    // lParam de WM_KEYDOWN: repeticion 1, el scancode en los bits 16..23, y
    // los bits 30 y 31 a 1 al soltar (como Windows).
    let lparam = 1 | (sc as u64) << 16 | if pulsada { 0 } else { 3 << 30 };
    m(if pulsada { WM_KEYDOWN } else { WM_KEYUP }, vk, lparam)
}

/// **La cola de mensajes del hilo**, con las prioridades de Windows (ver la
/// cabecera del modulo).
#[derive(Debug, Default)]
pub struct Cola {
    llegados: VecDeque<Msg>,
    salir: Option<i32>,
    /// Las ventanas con algo que pintar, en el orden en que se invalidaron.
    invalidas: Vec<u64>,
}

impl Cola {
    pub const fn nueva() -> Self {
        Cola { llegados: VecDeque::new(), salir: None, invalidas: Vec::new() }
    }

    /// `PostMessage`, o la entrada que llego del escritorio.
    pub fn publicar(&mut self, m: Msg) {
        self.llegados.push_back(m);
    }

    /// `PostQuitMessage(codigo)`: WM_QUIT sale cuando no quede nada delante.
    pub fn salir(&mut self, codigo: i32) {
        self.salir = Some(codigo);
    }

    /// `InvalidateRect`: esta ventana tiene que pintarse. Una vez basta.
    pub fn invalidar(&mut self, hwnd: u64) {
        if !self.invalidas.contains(&hwnd) {
            self.invalidas.push(hwnd);
        }
    }

    /// `BeginPaint` / `ValidateRect`: ya se pinto.
    pub fn validar(&mut self, hwnd: u64) {
        self.invalidas.retain(|&h| h != hwnd);
    }

    pub fn por_pintar(&self, hwnd: u64) -> bool {
        self.invalidas.contains(&hwnd)
    }

    /// Una ventana que muere se lleva lo suyo: ni su WM_PAINT ni lo que
    /// quedara para ella.
    pub fn olvidar(&mut self, hwnd: u64) {
        self.validar(hwnd);
        self.llegados.retain(|m| m.hwnd != hwnd);
    }

    /// **El siguiente SIN sacarlo** (`PeekMessage` con PM_NOREMOVE, P3c1):
    /// el mismo que daria [`Self::sacar`].
    pub fn mirar(&self) -> Option<Msg> {
        if let Some(m) = self.llegados.front() {
            return Some(*m);
        }
        if let Some(c) = self.salir {
            return Some(Msg { hwnd: 0, mensaje: WM_QUIT, wparam: c as u32 as u64, lparam: 0 });
        }
        self.invalidas.first().map(|&hwnd| Msg { hwnd, mensaje: WM_PAINT, wparam: 0, lparam: 0 })
    }

    /// **El siguiente**, en el orden de Windows. `None` si no hay nada (y
    /// `GetMessage` duerme).
    pub fn sacar(&mut self) -> Option<Msg> {
        if let Some(m) = self.llegados.pop_front() {
            return Some(m);
        }
        if let Some(c) = self.salir.take() {
            return Some(Msg { hwnd: 0, mensaje: WM_QUIT, wparam: c as u32 as u64, lparam: 0 });
        }
        // El WM_PAINT NO valida: la ventana sigue invalida hasta BeginPaint,
        // como en Windows. Un WndProc que no pinte lo recibe otra vez.
        self.invalidas.first().map(|&hwnd| Msg { hwnd, mensaje: WM_PAINT, wparam: 0, lparam: 0 })
    }
}

// -- Los DIB: lo que StretchDIBits pone en una superficie -------------------

/// Lo que se sabe de un DIB por su `BITMAPINFOHEADER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dib {
    pub ancho: u32,
    pub alto: u32,
    /// `biHeight` negativo: la fila 0 es la de ARRIBA. Positivo (lo mas comun
    /// en los ejemplos viejos): la fila 0 del bufer es la de ABAJO.
    pub de_arriba: bool,
}

/// Por que un DIB no se pinta. Cada NO dice cual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoPinta {
    /// La cabecera no es un `BITMAPINFOHEADER` (40 bytes o mas).
    Cabecera,
    /// Solo 32 bits por pixel, sin comprimir (`BI_RGB`), por ahora.
    Formato { bits: u16, compresion: u32 },
    /// StretchDIBits sin ESTIRAR: el origen y el destino miden lo mismo.
    Escala,
    /// Un trozo del origen que no es la imagen entera.
    Trozo,
    /// Los pixeles que se dan no llegan a lo que la cabecera promete.
    Corto,
}

fn u16_de(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn u32_de(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// **Leer un `BITMAPINFOHEADER`** (los primeros 40 bytes de un `BITMAPINFO`).
pub fn leer_dib(b: &[u8]) -> Result<Dib, NoPinta> {
    if b.len() < 40 || u32_de(b, 0) < 40 {
        return Err(NoPinta::Cabecera);
    }
    let ancho = u32_de(b, 4) as i32;
    let alto = u32_de(b, 8) as i32;
    let (bits, compresion) = (u16_de(b, 14), u32_de(b, 16));
    if bits != 32 || compresion != 0 {
        return Err(NoPinta::Formato { bits, compresion });
    }
    if ancho <= 0 || alto == 0 {
        return Err(NoPinta::Cabecera);
    }
    Ok(Dib { ancho: ancho as u32, alto: alto.unsigned_abs(), de_arriba: alto < 0 })
}

/// Un rectangulo de `StretchDIBits`: x, y, ancho, alto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub ancho: i32,
    pub alto: i32,
}

/// **Copiar un DIB de 32 bits** a los pixeles de una superficie (`destino`,
/// `d_ancho` x `d_alto`, `d_stride` pixeles por fila, BGRA como el DIB).
///
/// Hoy: la imagen ENTERA y sin estirar (`origen` = todo el DIB, `dst` del
/// mismo medida); lo que caiga fuera de la superficie se recorta. El alfa se
/// pone a 255: en un DIB `BI_RGB` el cuarto byte no significa nada, y la
/// superficie lo leeria. Devuelve las filas copiadas, lo que `StretchDIBits`
/// contesta.
#[allow(clippy::too_many_arguments)]
pub fn copiar_dib(
    destino: &mut [u32],
    d_ancho: u32,
    d_alto: u32,
    d_stride: u32,
    dst: Rect,
    origen: Rect,
    bits: &[u8],
    dib: &Dib,
) -> Result<u32, NoPinta> {
    if origen.x != 0 || origen.y != 0 || origen.ancho != dib.ancho as i32 || origen.alto != dib.alto as i32 {
        return Err(NoPinta::Trozo);
    }
    if dst.ancho != origen.ancho || dst.alto != origen.alto {
        return Err(NoPinta::Escala);
    }
    let fila_bytes = dib.ancho as usize * 4;
    if bits.len() < fila_bytes * dib.alto as usize {
        return Err(NoPinta::Corto);
    }
    let mut filas = 0;
    for y in 0..dib.alto as i32 {
        let dy = dst.y + y;
        if dy < 0 || dy >= d_alto as i32 {
            continue;
        }
        let fy = if dib.de_arriba { y } else { dib.alto as i32 - 1 - y } as usize;
        let fuente = &bits[fy * fila_bytes..][..fila_bytes];
        for x in 0..dib.ancho as i32 {
            let dx = dst.x + x;
            if dx < 0 || dx >= d_ancho as i32 {
                continue;
            }
            let o = x as usize * 4;
            let p = u32::from_le_bytes([fuente[o], fuente[o + 1], fuente[o + 2], fuente[o + 3]]);
            destino[dy as usize * d_stride as usize + dx as usize] = p | 0xFF00_0000;
        }
        filas += 1;
    }
    Ok(filas)
}
