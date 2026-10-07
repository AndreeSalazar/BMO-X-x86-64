//! **`user32.dll`, grupo 4: dialogos, el HDC, los textos y lo demas** (tanda
//! 10 de Cyberpunk, 30-09): lo que quedaba de user32.
//!
//! ```text
//!    dialogos    CreateDialogIndirectParamW (DLGTEMPLATE y DLGTEMPLATEEX:
//!                la ventana "#32770", sus controles con su ID y
//!                WM_INITDIALOG), EndDialog, GetDlgItem, GetDialogBaseUnits
//!    el HDC      GetDC ReleaseDC FillRect DrawFocusRect ValidateRect
//!                DrawTextW DrawTextExW GetSysColor
//!    recursos    LoadStringW LoadStringA (el STRINGTABLE del modulo)
//!    y lo demas  GetTopWindow GetProcessWindowStation
//!                GetUserObjectInformationW MessageBoxIndirectA
//!                RegisterDeviceNotificationW UnregisterDeviceNotification
//!                WaitForInputIdle DisableProcessWindowsGhosting
//! ```
//!
//! Lo que no, dicho: la casa NO tiene fuentes todavia, asi que DrawText mide
//! con una celda fija de 8 x 16 (la del "System" de Windows a 96 DPI) y, si
//! no es DT_CALCRECT, no pinta las letras (lo dice con [`aviso`]). FillRect
//! pinta los pinceles de color de sistema (COLOR_x + 1), que son los unicos
//! que la casa tiene. Los colores de sistema son los de Windows 10 sin tema
//! de contraste.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::user32::{self, def_window_proc, llamar, utf16, BIT_HDC};
use crate::{aviso, con, dir, kernel32, plataforma};

const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_INVALID_WINDOW_HANDLE: u32 = 1400;
const ERROR_CONTROL_ID_NOT_FOUND: u32 = 1421;

const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const DS_SETFONT: u32 = 0x40;
const WM_INITDIALOG: u32 = 0x0110;

struct Dialogo {
    h: u64,
    proc_: u64,
    resultado: i64,
    acabado: bool,
}

struct Estado {
    dialogos: Vec<Dialogo>,
    avisos: u32,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
// [hilos] cerrojo -- estado del proceso que tocan los hilos del juego: necesita un cerrojo (H2.1)
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { dialogos: Vec::new(), avisos: 0 }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.dialogos.clear();
    e.avisos = 0;
}

fn no(e: u32) -> i32 {
    kernel32::poner_error(e);
    0
}

fn viva(h: u64) -> bool {
    con(|e| e.ventanas.iter().any(|v| v.hwnd == h && v.viva))
}

// -- Las clases de los dialogos --------------------------------------------------------------

/// Las clases que Windows ya trae: la del dialogo y las de sus controles
/// (por atomo en una plantilla: 0x80 Button ... 0x85 ComboBox).
const CLASES: [&str; 7] = ["#32770", "Button", "Edit", "Static", "ListBox", "ScrollBar", "ComboBox"];

fn ancho(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

/// La clase `n` de las de Windows, registrada la primera vez que hace falta.
fn clase_de_sistema(n: &str) {
    let nombre = ancho(n);
    let ya = con(|e| e.clases.iter().any(|c| crate::user32_ventanas::igual_sin_caso(&c.nombre, &nombre[..nombre.len() - 1])));
    if ya {
        return;
    }
    let wndproc = if n == "#32770" { dir!(dialogo_proc) } else { dir!(control_proc) };
    let mut wc = [0u8; 80];
    wc[0..4].copy_from_slice(&80u32.to_le_bytes());
    wc[8..16].copy_from_slice(&wndproc.to_le_bytes());
    // DLGWINDOWEXTRA de x64: DWLP_MSGRESULT, DWLP_DLGPROC, DWLP_USER.
    if n == "#32770" {
        wc[20..24].copy_from_slice(&48u32.to_le_bytes());
    }
    wc[64..72].copy_from_slice(&(nombre.as_ptr() as u64).to_le_bytes());
    // SAFETY: `nombre` vive hasta que `registrar` lo copia.
    user32::registrar(wc.as_ptr(), |p| unsafe { utf16(p as *const u16) }, false);
}

/// Los controles de la casa: lo de DefWindowProc (el texto, el ID).
extern "win64" fn control_proc(h: u64, m: u32, w: u64, l: u64) -> i64 {
    def_window_proc(h, m, w, l, false)
}

/// `DefDlgProc`: el DLGPROC primero; si dice FALSE, DefWindowProc.
extern "win64" fn dialogo_proc(h: u64, m: u32, w: u64, l: u64) -> i64 {
    let p = estado().dialogos.iter().find(|d| d.h == h).map(|d| d.proc_);
    if let Some(p) = p.filter(|&p| p != 0) {
        let r = llamar(p, h, m, w, l);
        if r != 0 {
            return r;
        }
    }
    def_window_proc(h, m, w, l, false)
}

// -- La plantilla ------------------------------------------------------------------------------

/// Un lector de la plantilla: WORDs, alineada a DWORD cuando toca.
struct Plantilla {
    base: *const u8,
    i: usize,
}

impl Plantilla {
    fn u16(&mut self) -> u16 {
        // SAFETY: la plantilla del `.exe`, que se lee entera.
        let v = unsafe { (self.base.add(self.i) as *const u16).read_unaligned() };
        self.i += 2;
        v
    }
    fn u32(&mut self) -> u32 {
        self.u16() as u32 | (self.u16() as u32) << 16
    }
    fn i16(&mut self) -> i32 {
        self.u16() as i16 as i32
    }
    fn alinear(&mut self) {
        let dir = self.base as usize + self.i;
        self.i += (4 - dir % 4) % 4;
    }
    /// Un `sz_Or_Ord`: nada, un ordinal (0xFFFF, n) o una cadena.
    fn nombre(&mut self) -> Result<Vec<u16>, u16> {
        match self.u16() {
            0 => Ok(Vec::new()),
            0xFFFF => Err(self.u16()),
            c => {
                let mut v = alloc::vec![c];
                loop {
                    match self.u16() {
                        0 => break,
                        c => v.push(c),
                    }
                }
                Ok(v)
            }
        }
    }
}

/// Un control de la plantilla: estilo, estilo ex, x y ancho alto, ID, la
/// clase y el texto.
struct Control {
    estilo: u32,
    ex: u32,
    rect: [i32; 4],
    id: u32,
    clase: Result<Vec<u16>, u16>,
    texto: Vec<u16>,
}

/// Los pixeles de unas unidades de dialogo (la celda de 8 x 16).
fn pixeles(r: [i32; 4]) -> [i32; 4] {
    let (bx, by) = base_units();
    [r[0] * bx / 4, r[1] * by / 8, r[2] * bx / 4, r[3] * by / 8]
}

fn base_units() -> (i32, i32) {
    (CELDA.0, CELDA.1)
}

/// `CreateDialogIndirectParamW(inst, plantilla, padre, dlgproc, param)`.
extern "win64" fn create_dialog_indirect_param_w(inst: u64, plantilla: *const u8, padre: u64, proc_: u64, param: u64) -> u64 {
    if plantilla.is_null() {
        return no(ERROR_INVALID_PARAMETER) as u64;
    }
    let mut p = Plantilla { base: plantilla, i: 0 };
    // DLGTEMPLATEEX empieza por dlgVer 1 y la firma 0xFFFF.
    let ext = p.u16() == 1 && p.u16() == 0xFFFF;
    p.i = 0;
    let (estilo, ex, n) = if ext {
        p.i = 4;
        let _ayuda = p.u32();
        let ex = p.u32();
        (p.u32(), ex, 0)
    } else {
        let estilo = p.u32();
        (estilo, p.u32(), 0)
    };
    let n = n + p.u16() as usize;
    let rect = [p.i16(), p.i16(), p.i16(), p.i16()];
    let _menu = p.nombre();
    let clase = p.nombre();
    let titulo = p.nombre().unwrap_or_default();
    if estilo & DS_SETFONT != 0 {
        p.u16();
        if ext {
            p.u16();
            p.u16();
        }
        let _letra = p.nombre();
    }
    let mut controles = Vec::new();
    for _ in 0..n {
        p.alinear();
        let (estilo, ex, rect, id);
        if ext {
            let _ayuda = p.u32();
            ex = p.u32();
            estilo = p.u32();
            rect = [p.i16(), p.i16(), p.i16(), p.i16()];
            id = p.u32();
        } else {
            estilo = p.u32();
            ex = p.u32();
            rect = [p.i16(), p.i16(), p.i16(), p.i16()];
            id = p.u16() as u32;
        }
        let clase = p.nombre();
        let texto = p.nombre().unwrap_or_default();
        let extra = p.u16() as usize;
        p.i += extra;
        controles.push(Control { estilo, ex, rect, id, clase, texto });
    }
    for c in CLASES {
        clase_de_sistema(c);
    }
    let clase_dialogo = match clase {
        Ok(v) if !v.is_empty() => Ok(v),
        Err(a) => Err(a),
        _ => Ok(ancho("#32770")[..6].to_vec()),
    };
    let r = pixeles(rect);
    let h = crear(clase_dialogo, titulo, estilo & !WS_VISIBLE, ex, r, padre, 0, inst, param);
    if h == 0 {
        return 0;
    }
    estado().dialogos.push(Dialogo { h, proc_, resultado: 0, acabado: false });
    let mut primero = 0;
    for c in controles {
        let clase = match c.clase {
            Err(a @ 0x80..=0x85) => Ok(ancho(CLASES[(a - 0x80 + 1) as usize]).split_last().map(|x| x.1.to_vec()).unwrap_or_default()),
            otra => otra,
        };
        let hc = crear(clase, c.texto, (c.estilo | WS_CHILD) & !WS_VISIBLE, c.ex, pixeles(c.rect), h, c.id as u64, inst, 0);
        if hc == 0 {
            crate::user32::destroy_window(h);
            return 0;
        }
        // El estilo tal como lo dijo la plantilla (sin mostrarla: la casa
        // mostraria su superficie aparte en el escritorio).
        con(|e| {
            if let Some(v) = e.ventanas.iter_mut().find(|v| v.hwnd == hc) {
                v.datos.estilo = c.estilo | WS_CHILD;
            }
        });
        if primero == 0 {
            primero = hc;
        }
    }
    if proc_ != 0 {
        llamar(proc_, h, WM_INITDIALOG, primero, param);
    }
    if estilo & WS_VISIBLE != 0 && viva(h) {
        crate::user32::show_window(h, 5);
    }
    h
}

#[allow(clippy::too_many_arguments)]
fn crear(clase: Result<Vec<u16>, u16>, titulo: Vec<u16>, estilo: u32, ex: u32, r: [i32; 4], padre: u64, menu: u64, inst: u64, param: u64) -> u64 {
    let cs = user32::CreateStruct { params: param, inst, menu, padre, cy: r[3], cx: r[2], y: r[1], x: r[0], estilo: estilo as i32, nombre: 0, clase: 0, ex };
    user32::crear(clase, titulo, cs)
}

/// `EndDialog`: el resultado queda, y el dialogo se da por acabado.
extern "win64" fn end_dialog(h: u64, resultado: i64) -> i32 {
    let Some(d) = estado().dialogos.iter_mut().find(|d| d.h == h) else {
        return if viva(h) { 1 } else { no(ERROR_INVALID_WINDOW_HANDLE) };
    };
    d.resultado = resultado;
    d.acabado = true;
    1
}

/// `GetDlgItem`: la hija con ese ID.
extern "win64" fn get_dlg_item(h: u64, id: i32) -> u64 {
    if !viva(h) {
        return no(ERROR_INVALID_WINDOW_HANDLE) as u64;
    }
    let hija = con(|e| e.ventanas.iter().find(|v| v.viva && v.datos.padre == h && v.datos.estilo & WS_CHILD != 0 && v.datos.id as u32 == id as u32).map(|v| v.hwnd));
    hija.unwrap_or_else(|| no(ERROR_CONTROL_ID_NOT_FOUND) as u64)
}

/// La celda de la casa: 8 x 16 (el "System" de Windows a 96 DPI).
const CELDA: (i32, i32) = (8, 16);

extern "win64" fn get_dialog_base_units() -> i32 {
    CELDA.1 << 16 | CELDA.0
}

// -- El HDC ------------------------------------------------------------------------------------

/// `GetDC(h)`: el HDC de la ventana (el de BeginPaint); con 0, el de la
/// pantalla (que no se pinta: la casa no tiene la pantalla entera).
extern "win64" fn get_dc(h: u64) -> u64 {
    if h != 0 && !viva(h) {
        return 0;
    }
    h | BIT_HDC
}

/// `ReleaseDC`: lo pintado se ve ya, si la ventana se ve.
extern "win64" fn release_dc(h: u64, hdc: u64) -> i32 {
    if hdc & BIT_HDC == 0 || hdc & !BIT_HDC != h {
        return 0;
    }
    presentar(h);
    1
}

fn presentar(h: u64) {
    if let Some(sup) = con(|e| e.ventanas.iter().find(|v| v.hwnd == h && v.viva && v.mostrada).map(|v| v.sup)) {
        (plataforma().presentar)(&sup);
    }
}

/// Los colores de sistema de Windows 10 (COLORREF: 0x00BBGGRR).
const COLORES: [u32; 31] = [
    0xC8C8C8, 0x000000, 0xD1B499, 0xDBCDBF, 0xF0F0F0, 0xFFFFFF, 0x646464, 0x000000, 0x000000, 0x000000, 0xB4B4B4, 0xFCF7F4, 0xABABAB, 0xD77800, 0xFFFFFF, 0xF0F0F0, 0xA0A0A0,
    0x6D6D6D, 0x000000, 0x000000, 0xFFFFFF, 0x696969, 0xE3E3E3, 0x000000, 0xE1FFFF, 0x000000, 0xCC6600, 0xEAD1B9, 0xF2E4D7, 0xFF9933, 0xF0F0F0,
];

extern "win64" fn get_sys_color(i: i32) -> u32 {
    COLORES.get(i as usize).copied().unwrap_or(0)
}

/// Un rectangulo del `.exe` (RECT: izquierda, arriba, derecha, abajo).
fn rect(r: *const i32) -> Option<[i32; 4]> {
    // SAFETY: un RECT suyo, si no es NULL.
    (!r.is_null()).then(|| unsafe { [r.read_unaligned(), r.add(1).read_unaligned(), r.add(2).read_unaligned(), r.add(3).read_unaligned()] })
}

/// Cada pixel de `r` (recortado a la superficie de `hdc`), cambiado por `f`.
fn pintar(hdc: u64, r: [i32; 4], mut f: impl FnMut(i32, i32, u32) -> u32) {
    let Some(sup) = user32::superficie_de(hdc & !BIT_HDC) else { return };
    let (x0, y0) = (r[0].max(0), r[1].max(0));
    let (x1, y1) = (r[2].min(sup.ancho as i32), r[3].min(sup.alto as i32));
    for y in y0..y1 {
        for x in x0..x1 {
            // SAFETY: (x, y) dentro de la superficie, de `stride` pixeles por fila.
            unsafe {
                let p = sup.pixeles.add(y as usize * sup.stride as usize + x as usize);
                *p = f(x, y, *p);
            }
        }
    }
}

/// COLORREF (0x00BBGGRR) al pixel de la superficie (0x00RRGGBB).
fn pixel(c: u32) -> u32 {
    (c & 0xFF) << 16 | (c & 0xFF00) | (c >> 16 & 0xFF)
}

/// `FillRect(hdc, rect, pincel)`: cualquier pincel de `pinceles` (03-10):
/// los de color de sistema, los de serie y los de CreateSolidBrush.
extern "win64" fn fill_rect(hdc: u64, r: *const i32, pincel: u64) -> i32 {
    let Some(r) = rect(r) else { return 0 };
    if hdc & BIT_HDC == 0 {
        return 0;
    }
    let c = match crate::pinceles::de(pincel) {
        Some(crate::pinceles::Relleno::Sistema(i)) => match COLORES.get(i) {
            Some(&c) => c,
            None => return 0,
        },
        Some(crate::pinceles::Relleno::Color(c)) => c,
        Some(crate::pinceles::Relleno::Hueco) => return 1,
        None => {
            avisar("FillRect: un pincel que la casa no conoce (ni de sistema, ni de serie, ni de CreateSolidBrush)");
            return 0;
        }
    };
    let c = pixel(c);
    pintar(hdc, r, |_, _, _| c);
    1
}

/// `DrawFocusRect`: el borde de puntos, en XOR (dos veces, se borra).
extern "win64" fn draw_focus_rect(hdc: u64, r: *const i32) -> i32 {
    let Some(r) = rect(r) else { return 0 };
    if hdc & BIT_HDC == 0 {
        return 0;
    }
    pintar(hdc, r, |x, y, p| {
        let borde = x == r[0] || x == r[2] - 1 || y == r[1] || y == r[3] - 1;
        if borde && (x + y) % 2 == 0 {
            p ^ 0x00FF_FFFF
        } else {
            p
        }
    });
    1
}

/// `ValidateRect`: la ventana ya no tiene que pintarse (entera, como la
/// invalida InvalidateRect).
extern "win64" fn validate_rect(h: u64, _r: *const i32) -> i32 {
    con(|e| {
        let hs: Vec<u64> = e.ventanas.iter().filter(|v| v.viva && (h == 0 || v.hwnd == h)).map(|v| v.hwnd).collect();
        for x in &hs {
            e.cola.validar(*x);
        }
    });
    1
}

fn avisar(t: &str) {
    let e = estado();
    if e.avisos < 8 {
        e.avisos += 1;
        aviso(t);
    }
}

const DT_SINGLELINE: u32 = 0x20;
const DT_CALCRECT: u32 = 0x400;

/// **DrawText**: las lineas del texto, medidas con la celda. Devuelve el
/// alto; con DT_CALCRECT, el rectangulo es lo que ocupa.
fn draw_text(hdc: u64, t: *const u16, n: i32, r: *mut i32, formato: u32) -> i32 {
    if hdc & BIT_HDC == 0 || t.is_null() || r.is_null() {
        return 0;
    }
    // SAFETY: `n` WCHAR suyos, o hasta el 0 con -1.
    let texto: Vec<u16> = if n < 0 { unsafe { utf16(t) } } else { unsafe { core::slice::from_raw_parts(t, n as usize).to_vec() } };
    let lineas: Vec<&[u16]> = if formato & DT_SINGLELINE != 0 { alloc::vec![&texto[..]] } else { texto.split(|&c| c == b'\n' as u16).collect() };
    let largo = |l: &[u16]| l.iter().filter(|&&c| c != b'\r' as u16).count() as i32;
    let ancho = lineas.iter().map(|l| largo(l)).max().unwrap_or(0) * CELDA.0;
    let alto = lineas.len() as i32 * CELDA.1;
    if formato & DT_CALCRECT != 0 {
        // SAFETY: el RECT suyo (izquierda, arriba, derecha, abajo).
        unsafe {
            *r.add(2) = *r + ancho;
            *r.add(3) = *r.add(1) + alto;
        }
    } else if ancho > 0 {
        avisar("DrawText: la casa no tiene fuentes todavia; mide, pero no pinta las letras");
    }
    alto
}

extern "win64" fn draw_text_w(hdc: u64, t: *const u16, n: i32, r: *mut i32, formato: u32) -> i32 {
    draw_text(hdc, t, n, r, formato)
}

extern "win64" fn draw_text_ex_w(hdc: u64, t: *const u16, n: i32, r: *mut i32, formato: u32, _p: u64) -> i32 {
    draw_text(hdc, t, n, r, formato)
}

// -- Recursos ----------------------------------------------------------------------------------

/// Un u32 de la imagen en `base + off`.
fn leer32(base: u64, off: u64) -> u32 {
    // SAFETY: la imagen cargada, dentro de sus cabeceras y su .rsrc.
    unsafe { ((base + off) as *const u32).read_unaligned() }
}

fn leer16(base: u64, off: u64) -> u16 {
    // SAFETY: lo mismo.
    unsafe { ((base + off) as *const u16).read_unaligned() }
}

/// La entrada `id` (o la primera, con `None`) de un directorio de
/// recursos: su desplazamiento desde el principio de .rsrc, y si es otro
/// directorio.
fn entrada(base: u64, rsrc: u64, dir: u64, id: Option<u16>) -> Option<(u64, bool)> {
    let d = rsrc + dir;
    let n = leer16(base, d + 12) as u64 + leer16(base, d + 14) as u64;
    (0..n).map(|i| d + 16 + 8 * i).find_map(|e| {
        let nombre = leer32(base, e);
        let ok = match id {
            None => true,
            Some(id) => nombre & 0x8000_0000 == 0 && nombre == id as u32,
        };
        let a = leer32(base, e + 4);
        ok.then_some(((a & 0x7FFF_FFFF) as u64, a & 0x8000_0000 != 0))
    })
}

/// **El idioma** de un recurso, como Windows en en-US: el neutro, en-US,
/// otro ingles, el del usuario o el del sistema; si no hay ninguno, el
/// primero. Los idiomas van en orden de su numero, y "el primero" seria el
/// arabe (0x401) antes que el ingles (0x409): Cyberpunk saco su aviso en
/// arabe por eso.
fn idioma(base: u64, rsrc: u64, dir: u64) -> Option<(u64, bool)> {
    let d = rsrc + dir;
    let n = leer16(base, d + 12) as u64 + leer16(base, d + 14) as u64;
    let lenguas: Vec<(u32, u32)> = (0..n).map(|i| d + 16 + 8 * i).map(|e| (leer32(base, e), leer32(base, e + 4))).collect();
    let rango = |l: u32| match l {
        0x0000 => 0,
        0x0409 => 1,
        _ if l & 0x3FF == 0x09 => 2,
        0x0400 => 3,
        0x0800 => 4,
        _ => 5,
    };
    let (_, a) = lenguas.iter().filter(|(l, _)| l & 0x8000_0000 == 0).min_by_key(|(l, _)| rango(*l)).or(lenguas.first())?;
    Some(((a & 0x7FFF_FFFF) as u64, a & 0x8000_0000 != 0))
}

/// **El recurso** `tipo`/`id` del modulo `base` (en su idioma): su
/// direccion y su medida.
fn recurso(base: u64, tipo: u16, id: u16) -> Option<(u64, u32)> {
    if base == 0 || leer16(base, 0) != 0x5A4D {
        return None;
    }
    let nt = leer32(base, 0x3C) as u64;
    // PE32+: el directorio de recursos es el 2 (opcional + 112 + 2 * 8).
    let rsrc = leer32(base, nt + 24 + 112 + 16) as u64;
    if rsrc == 0 {
        return None;
    }
    let (t, sub) = entrada(base, rsrc, 0, Some(tipo))?;
    let (n, sub2) = sub.then(|| entrada(base, rsrc, t, Some(id))).flatten()?;
    let (l, hoja) = sub2.then(|| idioma(base, rsrc, n)).flatten()?;
    if hoja {
        return None;
    }
    let dato = rsrc + l;
    Some((base + leer32(base, dato) as u64, leer32(base, dato + 4)))
}

/// La cadena `id` del STRINGTABLE (bloques de 16: el `id / 16 + 1`).
fn cadena(inst: u64, id: u32) -> Option<(*const u16, usize)> {
    let base = if inst == 0 { crate::kernel32_a::w::<extern "win64" fn(*const u16) -> u64>("GetModuleHandleW")(core::ptr::null()) } else { inst };
    let (p, medida) = recurso(base, 6, (id / 16 + 1) as u16)?;
    let fin = p + medida as u64;
    let mut q = p;
    for _ in 0..id % 16 {
        q += 2 + 2 * leer16(q, 0) as u64;
        if q >= fin {
            return None;
        }
    }
    let n = leer16(q, 0) as usize;
    (n > 0).then_some(((q + 2) as *const u16, n))
}

/// `LoadStringW`: con 0 de medida, un puntero a la cadena (sin su 0) y su
/// largo; si no, copiada y cortada, con su 0.
extern "win64" fn load_string_w(inst: u64, id: u32, buf: *mut u16, n: i32) -> i32 {
    if buf.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    let c = cadena(inst, id);
    if n == 0 {
        let Some((p, k)) = c else { return 0 };
        // SAFETY: el LPWSTR del `.exe`, donde va el puntero.
        unsafe { (buf as *mut u64).write_unaligned(p as u64) };
        return k as i32;
    }
    let Some((p, k)) = c else {
        // SAFETY: al menos un WCHAR suyo.
        unsafe { *buf = 0 };
        return 0;
    };
    let k = k.min(n as usize - 1);
    // SAFETY: `n` WCHAR suyos; la cadena, del recurso.
    unsafe {
        core::ptr::copy_nonoverlapping(p, buf, k);
        *buf.add(k) = 0;
    }
    k as i32
}

extern "win64" fn load_string_a(inst: u64, id: u32, buf: *mut u8, n: i32) -> i32 {
    if buf.is_null() || n <= 0 {
        return no(ERROR_INVALID_PARAMETER);
    }
    let Some((p, k)) = cadena(inst, id) else {
        // SAFETY: al menos un byte suyo.
        unsafe { *buf = 0 };
        return 0;
    };
    // SAFETY: la cadena del recurso.
    let w = unsafe { core::slice::from_raw_parts(p, k) };
    let a = bmo_proton_x::texto::a_estrecho(w, false).unwrap_or_default();
    let k = a.len().min(n as usize - 1);
    // SAFETY: `n` bytes suyos.
    unsafe {
        core::ptr::copy_nonoverlapping(a.as_ptr(), buf, k);
        *buf.add(k) = 0;
    }
    k as i32
}

// -- Y lo demas --------------------------------------------------------------------------------

/// `GetTopWindow`: la primera hija (la primera creada, como el orden Z de
/// Windows con las hijas); con 0, la de delante.
extern "win64" fn get_top_window(h: u64) -> u64 {
    if h == 0 {
        return con(|e| e.ventanas.iter().rev().find(|v| v.viva && v.datos.estilo & WS_CHILD == 0).map_or(0, |v| v.hwnd));
    }
    if !viva(h) {
        return no(ERROR_INVALID_WINDOW_HANDLE) as u64;
    }
    con(|e| e.ventanas.iter().find(|v| v.viva && v.datos.padre == h && v.datos.estilo & WS_CHILD != 0).map_or(0, |v| v.hwnd))
}

/// La estacion de ventanas del proceso: la interactiva, "WinSta0".
const ESTACION: u64 = 0x5D00_0010;

extern "win64" fn get_process_window_station() -> u64 {
    ESTACION
}

/// `GetUserObjectInformationW(h, que, buf, medida, *hace_falta)`:
/// UOI_FLAGS (visible), UOI_NAME y UOI_TYPE de la estacion.
extern "win64" fn get_user_object_information_w(h: u64, que: i32, buf: *mut u8, medida: u32, hace_falta: *mut u32) -> i32 {
    if h != ESTACION {
        return no(ERROR_INVALID_HANDLE);
    }
    let datos: Vec<u8> = match que {
        // USEROBJECTFLAGS: fInherit, fReserved, dwFlags = WSF_VISIBLE.
        1 => [0u32, 0, 1].iter().flat_map(|v| v.to_le_bytes()).collect(),
        2 | 3 => ancho(if que == 2 { "WinSta0" } else { "WindowStation" }).iter().flat_map(|c| c.to_le_bytes()).collect(),
        _ => return no(ERROR_INVALID_PARAMETER),
    };
    if !hace_falta.is_null() {
        // SAFETY: el DWORD del `.exe`.
        unsafe { *hace_falta = datos.len() as u32 };
    }
    if buf.is_null() || (medida as usize) < datos.len() {
        return no(ERROR_INSUFFICIENT_BUFFER);
    }
    // SAFETY: `medida` bytes suyos.
    unsafe { core::ptr::copy_nonoverlapping(datos.as_ptr(), buf, datos.len()) };
    1
}

/// `MessageBoxIndirectA`: el MSGBOXPARAMSA (texto +24, titulo +32, estilo
/// +40) a MessageBoxA.
extern "win64" fn message_box_indirect_a(p: *const u8) -> i32 {
    if p.is_null() {
        return no(ERROR_INVALID_PARAMETER);
    }
    // SAFETY: el MSGBOXPARAMSA del `.exe` (80 bytes).
    let (h, texto, titulo, estilo) = unsafe { ((p.add(8) as *const u64).read_unaligned(), (p.add(24) as *const u64).read_unaligned(), (p.add(32) as *const u64).read_unaligned(), (p.add(40) as *const u32).read_unaligned()) };
    let Some(f) = crate::tabla_casa("user32.dll", &bmo_proton_x::Funcion::Nombre("MessageBoxA".into())) else { return 0 };
    // SAFETY: la MessageBoxA de la casa.
    let f: extern "win64" fn(u64, u64, u64, u32) -> i32 = unsafe { core::mem::transmute::<u64, _>(f) };
    f(h, texto, titulo, estilo)
}

/// Los avisos de dispositivos (mandos que se enchufan): la casa no los
/// manda todavia, pero el registro vale.
const AVISO_DISPOSITIVO: u64 = 0x5D00_1000;

extern "win64" fn register_device_notification_w(_h: u64, filtro: *const u8, _banderas: u32) -> u64 {
    if filtro.is_null() {
        return no(ERROR_INVALID_PARAMETER) as u64;
    }
    AVISO_DISPOSITIVO
}

extern "win64" fn unregister_device_notification(h: u64) -> i32 {
    if h != AVISO_DISPOSITIVO {
        return no(ERROR_INVALID_HANDLE);
    }
    1
}

/// `WaitForInputIdle`: el proceso ya espera entrada (0).
extern "win64" fn wait_for_input_idle(_p: u64, _ms: u32) -> u32 {
    0
}

/// `DisableProcessWindowsGhosting`: el escritorio de BMO-X no hace
/// "fantasmas" de las ventanas que no contestan.
extern "win64" fn disable_process_windows_ghosting() {}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "CreateDialogIndirectParamW" => dir!(create_dialog_indirect_param_w),
        "EndDialog" => dir!(end_dialog),
        "GetDlgItem" => dir!(get_dlg_item),
        "GetDialogBaseUnits" => dir!(get_dialog_base_units),
        "GetDC" => dir!(get_dc),
        "ReleaseDC" => dir!(release_dc),
        "GetSysColor" => dir!(get_sys_color),
        "FillRect" => dir!(fill_rect),
        "DrawFocusRect" => dir!(draw_focus_rect),
        "ValidateRect" => dir!(validate_rect),
        "DrawTextW" => dir!(draw_text_w),
        "DrawTextExW" => dir!(draw_text_ex_w),
        "LoadStringW" => dir!(load_string_w),
        "LoadStringA" => dir!(load_string_a),
        "GetTopWindow" => dir!(get_top_window),
        "GetProcessWindowStation" => dir!(get_process_window_station),
        "GetUserObjectInformationW" => dir!(get_user_object_information_w),
        "MessageBoxIndirectA" => dir!(message_box_indirect_a),
        "RegisterDeviceNotificationW" => dir!(register_device_notification_w),
        "UnregisterDeviceNotification" => dir!(unregister_device_notification),
        "WaitForInputIdle" => dir!(wait_for_input_idle),
        "DisableProcessWindowsGhosting" => dir!(disable_process_windows_ghosting),
        _ => return None,
    })
}
