//! **`user32.dll`, grupo 2: lo que se sabe de una ventana** (tanda 6 de
//! Cyberpunk, 29-09).
//!
//! ```text
//!    Get/SetWindowLongPtrW/A    WNDPROC -4 (subclasificar), HINSTANCE -6,
//!    Get/SetWindowLongW/A       padre -8, ID -12, estilo -16, estilo ex -20,
//!                               USERDATA -21, y los cbWndExtra de la clase
//!    Get/SetClassLongPtrW/A     lo del WNDCLASSEX, y los cbClsExtra
//!    RegisterClassW/A/ExA       las demas maneras de registrar una clase
//!    UnregisterClassW/A         (no con ventanas vivas: ERROR_CLASS_HAS_WINDOWS)
//!    GetClassInfo(Ex)W/A        el WNDCLASSEX de vuelta
//!    CreateWindowExA            la misma ventana, con nombres de bytes
//!    DefWindowProcA             la de W, con los textos en bytes
//!    IsWindow, IsWindowVisible, IsWindowUnicode, IsIconic, IsZoomed, IsWindowEnabled,
//!    EnableWindow, IsChild      el estado
//!    Get/SetForegroundWindow, Get/SetActiveWindow, Get/SetFocus,
//!    BringWindowToTop           cual esta delante (la ultima mostrada)
//!    GetParent, GetAncestor, GetWindow   la familia
//!    FindWindow(Ex)W/A, EnumWindows, EnumChildWindows   buscar
//!    GetClassNameW/A, GetWindowText(Length)W/A, SetWindowTextA
//!    GetWindowThreadProcessId
//!    SetWindowPos, MoveWindow   la posicion (la MEDIDA no cambia todavia:
//!                               la superficie es la que se pidio al crearla)
//! ```
//!
//! El titulo se lee y se escribe directamente (GetWindowText de una ventana
//! propia manda WM_GETTEXT en Windows; aqui lo contesta DefWindowProc igual,
//! para quien lo mande con SendMessage).

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::user32::{self, def_window_proc, llamar, proc_de, utf16};
use crate::{aviso, con, dir, kernel32, Ventana};

const ERROR_INVALID_WINDOW_HANDLE: u32 = 1400;
const ERROR_CLASS_DOES_NOT_EXIST: u32 = 1411;
const ERROR_CLASS_HAS_WINDOWS: u32 = 1412;
const ERROR_INVALID_INDEX: u32 = 1413;

const WS_CHILD: u32 = 0x4000_0000;
const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_DISABLED: u32 = 0x0800_0000;
const WS_MINIMIZE: u32 = 0x2000_0000;
const WS_MAXIMIZE: u32 = 0x0100_0000;

/// Lo de una clase que no es su nombre ni su WndProc.
pub(crate) struct DatosClase {
    pub(crate) estilo: u32,
    pub(crate) extra_clase: Vec<u8>,
    pub(crate) extra_ventana: u32,
    pub(crate) inst: u64,
    pub(crate) icono: u64,
    pub(crate) cursor: u64,
    pub(crate) fondo: u64,
    pub(crate) menu: u64,
    pub(crate) icono_chico: u64,
    /// Registrada con RegisterClass*A.
    pub(crate) ansi: bool,
}

/// Lo de una ventana que no es su superficie ni su WndProc.
pub(crate) struct DatosVentana {
    pub(crate) estilo: u32,
    pub(crate) ex: u32,
    pub(crate) atomo: u16,
    pub(crate) titulo: Vec<u16>,
    pub(crate) usuario: u64,
    pub(crate) id: u64,
    pub(crate) inst: u64,
    pub(crate) padre: u64,
    pub(crate) extra: Vec<u8>,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) desactivada: bool,
    pub(crate) hilo: u32,
    pub(crate) ansi: bool,
}

// -- El estado de aqui: los atomos, la de delante y la del foco ------------

struct Estado {
    /// Los atomos de cadena (clases y RegisterWindowMessage): 0xC000 en
    /// adelante, como en Windows.
    atomos: Vec<Vec<u16>>,
    frente: u64,
    foco: u64,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
// [hilos] cerrojo -- estado del proceso que tocan los hilos del juego: necesita un cerrojo (H2.1)
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { atomos: Vec::new(), frente: 0, foco: 0 }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.atomos.clear();
    e.frente = 0;
    e.foco = 0;
}

/// Dos nombres iguales sin mirar mayusculas (las clases y los titulos de
/// FindWindow no las miran; aqui, las de ASCII).
pub(crate) fn igual_sin_caso(a: &[u16], b: &[u16]) -> bool {
    let baja = |c: u16| if (b'A' as u16..=b'Z' as u16).contains(&c) { c + 32 } else { c };
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| baja(*x) == baja(*y))
}

/// **El atomo de una cadena**: el mismo para la misma (sin mayusculas).
pub(crate) fn atomo_nuevo(nombre: &[u16]) -> u16 {
    let e = estado();
    if let Some(i) = e.atomos.iter().position(|a| igual_sin_caso(a, nombre)) {
        return 0xC000 + i as u16;
    }
    e.atomos.push(nombre.to_vec());
    0xC000 + (e.atomos.len() - 1) as u16
}

// -- Las cadenas de bytes (las A) ------------------------------------------

/// Una cadena de bytes hasta su cero, en UTF-16 (Latin-1: el byte ES el
/// caracter; lo de ASCII, que es lo que traen los nombres, sale igual).
///
/// # Safety
/// `p` apunta a bytes terminados en cero.
pub(crate) unsafe fn bytes(p: *const u8) -> Vec<u16> {
    let mut v = Vec::new();
    while v.len() < 256 {
        let c = p.add(v.len()).read();
        if c == 0 {
            break;
        }
        v.push(c as u16);
    }
    v
}

/// Copiar `t` a un bufer de `n` caracteres (W o A), con su cero, cortando si
/// no cabe. Devuelve los copiados sin el cero.
pub(crate) fn copiar_texto(t: &[u16], buf: u64, n: usize, ansi: bool) -> usize {
    if buf == 0 || n == 0 {
        return 0;
    }
    let k = t.len().min(n - 1);
    // SAFETY: el `.exe` da `n` caracteres escribibles.
    unsafe {
        for (i, &c) in t[..k].iter().enumerate() {
            if ansi {
                (buf as *mut u8).add(i).write(if c < 0x100 { c as u8 } else { b'?' });
            } else {
                (buf as *mut u16).add(i).write(c);
            }
        }
        if ansi {
            (buf as *mut u8).add(k).write(0);
        } else {
            (buf as *mut u16).add(k).write(0);
        }
    }
    k
}

/// Un nombre de clase o de ventana del `.exe`: un atomo (`Err`) o una cadena.
fn nombre(p: u64, ansi: bool) -> Result<Vec<u16>, u16> {
    if p < 0x1_0000 {
        return Err(p as u16);
    }
    // SAFETY: una cadena suya, terminada en cero.
    Ok(unsafe { if ansi { bytes(p as *const u8) } else { utf16(p as *const u16) } })
}

// -- Las ventanas ------------------------------------------------------------

/// Algo de una ventana VIVA; si no lo es, ERROR_INVALID_WINDOW_HANDLE.
fn con_ventana<R>(h: u64, f: impl FnOnce(&mut Ventana) -> R) -> Option<R> {
    let r = con(|e| e.ventanas.iter_mut().find(|v| v.hwnd == h && v.viva).map(f));
    if r.is_none() {
        kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
    }
    r
}

fn viva(h: u64) -> bool {
    con(|e| e.ventanas.iter().any(|v| v.hwnd == h && v.viva))
}

pub(crate) fn poner_titulo(h: u64, t: Vec<u16>) -> bool {
    con_ventana(h, |v| v.datos.titulo = t).is_some()
}

/// Donde esta una ventana en la pantalla (el escritorio, en el 0, 0).
pub(crate) fn posicion(h: u64) -> Option<(i32, i32)> {
    con(|e| e.ventanas.iter().find(|v| v.hwnd == h && v.viva).map(|v| (v.datos.x, v.datos.y)))
}

/// Una ventana se mostro: pasa a estar delante y con el foco.
pub(crate) fn al_frente(h: u64) {
    let e = estado();
    e.frente = h;
    e.foco = h;
}

/// Una ventana murio: ni delante ni con el foco.
pub(crate) fn murio(h: u64) {
    let e = estado();
    if e.frente == h {
        e.frente = 0;
    }
    if e.foco == h {
        e.foco = 0;
    }
}

/// WM_SETTEXT, WM_GETTEXT y WM_GETTEXTLENGTH, como DefWindowProc.
pub(crate) fn texto_por_mensaje(h: u64, m: u32, w: u64, l: u64, ansi: bool) -> i64 {
    match m {
        0x000C => {
            let t = if l == 0 { Vec::new() } else { nombre(l, ansi).unwrap_or_default() };
            poner_titulo(h, t) as i64
        }
        0x000D => con_ventana(h, |v| v.datos.titulo.clone()).map_or(0, |t| copiar_texto(&t, l, w as usize, ansi) as i64),
        _ => con_ventana(h, |v| v.datos.titulo.len() as i64).unwrap_or(0),
    }
}

// -- Los "longs" de una ventana ---------------------------------------------

/// Leer (`nuevo` None) o cambiar un long de `ancho` bytes (4 u 8); devuelve
/// el que habia. 0 y ERROR_INVALID_INDEX si el indice no vale.
fn largo(h: u64, i: i32, ancho: usize, nuevo: Option<u64>) -> u64 {
    let r = con_ventana(h, |v| {
        let d = &mut v.datos;
        let campo: &mut u64 = match i {
            -4 if ancho == 8 => &mut v.wndproc,
            -6 if ancho == 8 => &mut d.inst,
            -8 if ancho == 8 => &mut d.padre,
            -12 => &mut d.id,
            -21 => &mut d.usuario,
            -16 => {
                let viejo = (d.estilo & !WS_VISIBLE) | if v.mostrada { WS_VISIBLE } else { 0 };
                if let Some(n) = nuevo {
                    d.estilo = n as u32;
                }
                return Some(viejo as u64);
            }
            -20 => {
                let viejo = d.ex;
                if let Some(n) = nuevo {
                    d.ex = n as u32;
                }
                return Some(viejo as u64);
            }
            i if i >= 0 && i as usize + ancho <= d.extra.len() => {
                let b = &mut d.extra[i as usize..i as usize + ancho];
                let mut viejo = [0u8; 8];
                viejo[..ancho].copy_from_slice(b);
                if let Some(n) = nuevo {
                    b.copy_from_slice(&n.to_le_bytes()[..ancho]);
                }
                return Some(u64::from_le_bytes(viejo));
            }
            _ => return None,
        };
        let viejo = *campo;
        if let Some(n) = nuevo {
            *campo = n;
        }
        Some(viejo)
    });
    match r {
        Some(Some(v)) if ancho == 4 => v as u32 as i32 as i64 as u64,
        Some(Some(v)) => v,
        Some(None) => {
            kernel32::poner_error(ERROR_INVALID_INDEX);
            0
        }
        None => 0,
    }
}

extern "win64" fn get_window_long_ptr(h: u64, i: i32) -> u64 {
    largo(h, i, 8, None)
}

extern "win64" fn set_window_long_ptr(h: u64, i: i32, n: u64) -> u64 {
    largo(h, i, 8, Some(n))
}

extern "win64" fn get_window_long(h: u64, i: i32) -> i32 {
    largo(h, i, 4, None) as i32
}

extern "win64" fn set_window_long(h: u64, i: i32, n: i32) -> i32 {
    largo(h, i, 4, Some(n as u32 as u64)) as i32
}

// -- Las clases --------------------------------------------------------------

/// El long de una clase, por la ventana. `nuevo`: cambiarlo.
fn largo_clase(h: u64, i: i32, ancho: usize, nuevo: Option<u64>) -> u64 {
    let Some(atomo) = con_ventana(h, |v| v.datos.atomo) else { return 0 };
    let r = con(|e| {
        let c = e.clases.iter_mut().find(|c| c.atomo == atomo)?;
        let d = &mut c.datos;
        let campo: &mut u64 = match i {
            -8 => &mut d.menu,
            -10 => &mut d.fondo,
            -12 => &mut d.cursor,
            -14 => &mut d.icono,
            -16 => &mut d.inst,
            -24 => &mut c.wndproc,
            -34 => &mut d.icono_chico,
            -18 => return Some(d.extra_ventana as u64),
            -20 => return Some(d.extra_clase.len() as u64),
            -32 => return Some(atomo as u64),
            -26 => {
                let viejo = d.estilo;
                if let Some(n) = nuevo {
                    d.estilo = n as u32;
                }
                return Some(viejo as u64);
            }
            i if i >= 0 && i as usize + ancho <= d.extra_clase.len() => {
                let b = &mut d.extra_clase[i as usize..i as usize + ancho];
                let mut viejo = [0u8; 8];
                viejo[..ancho].copy_from_slice(b);
                if let Some(n) = nuevo {
                    b.copy_from_slice(&n.to_le_bytes()[..ancho]);
                }
                return Some(u64::from_le_bytes(viejo));
            }
            _ => return None,
        };
        let viejo = *campo;
        if let Some(n) = nuevo {
            *campo = n;
        }
        Some(viejo)
    });
    r.unwrap_or_else(|| {
        kernel32::poner_error(ERROR_INVALID_INDEX);
        0
    })
}

extern "win64" fn get_class_long_ptr(h: u64, i: i32) -> u64 {
    largo_clase(h, i, 8, None)
}

extern "win64" fn set_class_long_ptr(h: u64, i: i32, n: u64) -> u64 {
    largo_clase(h, i, 8, Some(n))
}

extern "win64" fn get_class_long(h: u64, i: i32) -> u32 {
    largo_clase(h, i, 4, None) as u32
}

extern "win64" fn set_class_long(h: u64, i: i32, n: u32) -> u32 {
    largo_clase(h, i, 4, Some(n as u64)) as u32
}

/// `RegisterClassW/A`: un `WNDCLASS` (72 bytes) como si fuera un
/// `WNDCLASSEX`. En x64 los dos tienen lo mismo en los mismos sitios desde
/// el +8 (el puntero de la WndProc se alinea); cambian los 4 primeros (style
/// en uno, cbSize en el otro) y el hIconSm del final.
fn registrar_sin_ex(wc: *const u8, ansi: bool) -> u16 {
    if wc.is_null() {
        return 0;
    }
    let mut ex = [0u8; 80];
    // SAFETY: un WNDCLASS suyo de 72 bytes.
    unsafe { core::ptr::copy_nonoverlapping(wc, ex.as_mut_ptr(), 72) };
    let estilo = [ex[0], ex[1], ex[2], ex[3]];
    ex[..4].copy_from_slice(&80u32.to_le_bytes());
    ex[4..8].copy_from_slice(&estilo);
    registrar(ex.as_ptr(), ansi)
}

fn registrar(wc: *const u8, ansi: bool) -> u16 {
    // SAFETY: las cadenas del `.exe`, terminadas en cero.
    user32::registrar(wc, |p| unsafe { if ansi { bytes(p as *const u8) } else { utf16(p as *const u16) } }, ansi)
}

extern "win64" fn register_class_w(wc: *const u8) -> u16 {
    registrar_sin_ex(wc, false)
}

extern "win64" fn register_class_a(wc: *const u8) -> u16 {
    registrar_sin_ex(wc, true)
}

extern "win64" fn register_class_ex_a(wc: *const u8) -> u16 {
    registrar(wc, true)
}

fn buscar_clase(n: &Result<Vec<u16>, u16>) -> Option<usize> {
    con(|e| {
        e.clases.iter().position(|c| match n {
            Ok(n) => igual_sin_caso(&c.nombre, n),
            Err(a) => c.atomo == *a,
        })
    })
}

fn unregister_class(n: u64, ansi: bool) -> i32 {
    let n = nombre(n, ansi);
    let Some(i) = buscar_clase(&n) else {
        kernel32::poner_error(ERROR_CLASS_DOES_NOT_EXIST);
        return 0;
    };
    let quitada = con(|e| {
        let atomo = e.clases[i].atomo;
        if e.ventanas.iter().any(|v| v.viva && v.datos.atomo == atomo) {
            return false;
        }
        e.clases.remove(i);
        true
    });
    if !quitada {
        kernel32::poner_error(ERROR_CLASS_HAS_WINDOWS);
    }
    quitada as i32
}

extern "win64" fn unregister_class_w(n: *const u16, _inst: u64) -> i32 {
    unregister_class(n as u64, false)
}

extern "win64" fn unregister_class_a(n: *const u8, _inst: u64) -> i32 {
    unregister_class(n as u64, true)
}

/// `GetClassInfoEx`: el `WNDCLASSEX` de la clase (el nombre, el puntero que
/// se dio). Devuelve su atomo; 0 y ERROR_CLASS_DOES_NOT_EXIST si no esta.
fn class_info(n: u64, wc: *mut u8, ex: bool, ansi: bool) -> u32 {
    let Some(i) = buscar_clase(&nombre(n, ansi)) else {
        kernel32::poner_error(ERROR_CLASS_DOES_NOT_EXIST);
        return 0;
    };
    let (atomo, c) = con(|e| {
        let c = &e.clases[i];
        let d = &c.datos;
        (c.atomo, [d.estilo as u64, c.wndproc, d.extra_clase.len() as u64 | (d.extra_ventana as u64) << 32, d.inst, d.icono, d.cursor, d.fondo, d.menu, d.icono_chico])
    });
    if wc.is_null() {
        return atomo as u32;
    }
    // El WNDCLASS (sin Ex): el style en el +0 y sin hIconSm; lo demas, igual
    // (ver `registrar_sin_ex`). El cbSize del EX lo pone el `.exe`.
    // SAFETY: un WNDCLASS(EX) suyo, escribible.
    unsafe {
        let w32 = |o: usize, v: u32| (wc.add(o) as *mut u32).write_unaligned(v);
        let w64 = |o: usize, v: u64| (wc.add(o) as *mut u64).write_unaligned(v);
        w32(if ex { 4 } else { 0 }, c[0] as u32);
        w64(8, c[1]);
        w64(16, c[2]);
        for (k, v) in c[3..8].iter().enumerate() {
            w64(24 + 8 * k, *v);
        }
        w64(64, n);
        if ex {
            w64(72, c[8]);
        }
    }
    atomo as u32
}

extern "win64" fn get_class_info_ex_w(_inst: u64, n: *const u16, wc: *mut u8) -> u32 {
    class_info(n as u64, wc, true, false)
}

extern "win64" fn get_class_info_ex_a(_inst: u64, n: *const u8, wc: *mut u8) -> u32 {
    class_info(n as u64, wc, true, true)
}

extern "win64" fn get_class_info_w(_inst: u64, n: *const u16, wc: *mut u8) -> u32 {
    class_info(n as u64, wc, false, false)
}

extern "win64" fn get_class_info_a(_inst: u64, n: *const u8, wc: *mut u8) -> u32 {
    class_info(n as u64, wc, false, true)
}

/// `CreateWindowExA`: la de W, con los nombres de bytes. La WndProc recibe
/// el `CREATESTRUCTA` con los punteros que se dieron.
#[allow(clippy::too_many_arguments)]
extern "win64" fn create_window_ex_a(
    ex: u32,
    clase: *const u8,
    titulo: *const u8,
    estilo: u32,
    x: i32,
    y: i32,
    ancho: i32,
    alto: i32,
    padre: u64,
    menu: u64,
    inst: u64,
    param: u64,
) -> u64 {
    let n = nombre(clase as u64, true);
    // SAFETY: una cadena suya.
    let t = if titulo.is_null() { Vec::new() } else { unsafe { bytes(titulo) } };
    let cs = user32::CreateStruct { params: param, inst, menu, padre, cy: alto, cx: ancho, y, x, estilo: estilo as i32, nombre: titulo as u64, clase: clase as u64, ex };
    user32::crear(n, t, cs)
}

extern "win64" fn def_window_proc_a(h: u64, m: u32, w: u64, l: u64) -> i64 {
    def_window_proc(h, m, w, l, true)
}

// -- El estado de una ventana ------------------------------------------------

extern "win64" fn is_window(h: u64) -> i32 {
    viva(h) as i32
}

/// `IsWindowUnicode`: si su clase se registro con W.
extern "win64" fn is_window_unicode(h: u64) -> i32 {
    con(|e| e.ventanas.iter().any(|v| v.hwnd == h && v.viva && !v.datos.ansi)) as i32
}

extern "win64" fn is_window_visible(h: u64) -> i32 {
    con(|e| e.ventanas.iter().any(|v| v.hwnd == h && v.viva && v.mostrada)) as i32
}

fn con_estilo(h: u64, bit: u32) -> i32 {
    con(|e| e.ventanas.iter().any(|v| v.hwnd == h && v.viva && v.datos.estilo & bit != 0)) as i32
}

extern "win64" fn is_iconic(h: u64) -> i32 {
    con_estilo(h, WS_MINIMIZE)
}

extern "win64" fn is_zoomed(h: u64) -> i32 {
    con_estilo(h, WS_MAXIMIZE)
}

extern "win64" fn is_window_enabled(h: u64) -> i32 {
    con(|e| e.ventanas.iter().any(|v| v.hwnd == h && v.viva && !v.datos.desactivada)) as i32
}

/// `EnableWindow`: devuelve si ESTABA desactivada. Si cambia, WM_ENABLE.
extern "win64" fn enable_window(h: u64, si: i32) -> i32 {
    let Some(antes) = con_ventana(h, |v| {
        let antes = v.datos.desactivada;
        v.datos.desactivada = si == 0;
        if si == 0 {
            v.datos.estilo |= WS_DISABLED;
        } else {
            v.datos.estilo &= !WS_DISABLED;
        }
        antes
    }) else {
        return 0;
    };
    if antes != (si == 0) {
        if let Some(wp) = proc_de(h) {
            llamar(wp, h, 0x000A, (si != 0) as u64, 0);
        }
    }
    antes as i32
}

extern "win64" fn get_foreground_window() -> u64 {
    estado().frente
}

extern "win64" fn set_foreground_window(h: u64) -> i32 {
    if !viva(h) {
        return 0;
    }
    estado().frente = h;
    1
}

/// `SetActiveWindow`: devuelve la que lo era.
extern "win64" fn set_active_window(h: u64) -> u64 {
    if !viva(h) {
        kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
        return 0;
    }
    core::mem::replace(&mut estado().frente, h)
}

extern "win64" fn get_focus() -> u64 {
    estado().foco
}

/// La del foco, o si no la de delante (a quien va la entrada inventada).
pub(crate) fn foco() -> u64 {
    let e = estado();
    if e.foco != 0 {
        e.foco
    } else {
        e.frente
    }
}

/// `SetFocus`: WM_KILLFOCUS a la que lo tenia y WM_SETFOCUS a la nueva.
/// Devuelve la que lo tenia.
extern "win64" fn set_focus(h: u64) -> u64 {
    if h != 0 && !viva(h) {
        kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
        return 0;
    }
    let antes = core::mem::replace(&mut estado().foco, h);
    if antes != h {
        if let Some(wp) = proc_de(antes) {
            llamar(wp, antes, 0x0008, h, 0);
        }
        if let Some(wp) = proc_de(h) {
            llamar(wp, h, 0x0007, antes, 0);
        }
    }
    antes
}

extern "win64" fn bring_window_to_top(h: u64) -> i32 {
    set_foreground_window(h)
}

// -- La familia --------------------------------------------------------------

/// (padre, estilo) de una ventana viva.
fn familia(h: u64) -> Option<(u64, u32)> {
    con(|e| e.ventanas.iter().find(|v| v.hwnd == h && v.viva).map(|v| (v.datos.padre, v.datos.estilo)))
}

/// `GetParent`: el padre de una hija; el propietario de una WS_POPUP; si no, 0.
extern "win64" fn get_parent(h: u64) -> u64 {
    match familia(h) {
        Some((p, e)) if e & (WS_CHILD | WS_POPUP) != 0 => p,
        Some(_) => 0,
        None => {
            kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
            0
        }
    }
}

/// `GetAncestor`: GA_PARENT 1 (el escritorio para una de arriba), GA_ROOT 2
/// (subiendo por los padres), GA_ROOTOWNER 3 (subiendo por GetParent).
extern "win64" fn get_ancestor(h: u64, como: u32) -> u64 {
    let Some((p, e)) = familia(h) else { return 0 };
    match como {
        1 => {
            if e & WS_CHILD != 0 {
                p
            } else {
                user32_escritorio()
            }
        }
        2 | 3 => {
            let mut x = h;
            for _ in 0..64 {
                let sube = if como == 2 { familia(x).filter(|f| f.1 & WS_CHILD != 0).map(|f| f.0) } else { Some(get_parent(x)) };
                match sube {
                    Some(p) if p != 0 && viva(p) => x = p,
                    _ => break,
                }
            }
            x
        }
        _ => 0,
    }
}

fn user32_escritorio() -> u64 {
    crate::user32_medidas::ESCRITORIO
}

/// Las hermanas de `h` (misma madre, hijas o de arriba), en su orden.
fn hermanas(padre: u64, hijas: bool) -> Vec<u64> {
    con(|e| {
        e.ventanas
            .iter()
            .filter(|v| v.viva && (v.datos.estilo & WS_CHILD != 0) == hijas && (!hijas || v.datos.padre == padre))
            .map(|v| v.hwnd)
            .collect()
    })
}

/// `GetWindow`: GW_HWNDFIRST 0, LAST 1, NEXT 2, PREV 3, GW_OWNER 4, GW_CHILD 5.
extern "win64" fn get_window(h: u64, que: u32) -> u64 {
    let Some((p, e)) = familia(h) else {
        kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
        return 0;
    };
    let hija = e & WS_CHILD != 0;
    let lista = || hermanas(p, hija);
    match que {
        0 => lista().first().copied().unwrap_or(0),
        1 => lista().last().copied().unwrap_or(0),
        2 | 3 => {
            let l = lista();
            let i = l.iter().position(|&x| x == h).unwrap_or(0);
            let j = if que == 2 { i.checked_add(1) } else { i.checked_sub(1) };
            j.and_then(|j| l.get(j)).copied().unwrap_or(0)
        }
        4 => {
            if hija {
                0
            } else {
                p
            }
        }
        5 => hermanas(h, true).first().copied().unwrap_or(0),
        _ => 0,
    }
}

extern "win64" fn is_child(padre: u64, h: u64) -> i32 {
    let mut x = h;
    for _ in 0..64 {
        match familia(x) {
            Some((p, e)) if e & WS_CHILD != 0 => {
                if p == padre {
                    return 1;
                }
                x = p;
            }
            _ => return 0,
        }
    }
    0
}

// -- Buscar ------------------------------------------------------------------

/// `FindWindowEx(padre, despues, clase, titulo)`: la primera (tras
/// `despues`) de esa madre (0: las de arriba) que casa. Sin clase ni titulo,
/// cualquiera.
fn buscar_ventana(padre: u64, despues: u64, clase: u64, titulo: u64, ansi: bool) -> u64 {
    let clase = if clase == 0 { None } else { Some(nombre(clase, ansi)) };
    let atomo = match &clase {
        None => None,
        Some(n) => match buscar_clase(n) {
            Some(i) => Some(con(|e| e.clases[i].atomo)),
            None => return 0,
        },
    };
    let titulo = if titulo == 0 { None } else { nombre(titulo, ansi).ok() };
    let hijas = padre != 0 && padre != user32_escritorio();
    let mut pasado = despues == 0;
    for h in hermanas(padre, hijas) {
        if !pasado {
            pasado = h == despues;
            continue;
        }
        let casa = con(|e| {
            e.ventanas.iter().find(|v| v.hwnd == h).is_some_and(|v| atomo.is_none_or(|a| v.datos.atomo == a) && titulo.as_ref().is_none_or(|t| igual_sin_caso(&v.datos.titulo, t)))
        });
        if casa {
            return h;
        }
    }
    kernel32::poner_error(2); // ERROR_FILE_NOT_FOUND, como Windows
    0
}

extern "win64" fn find_window_w(clase: *const u16, titulo: *const u16) -> u64 {
    buscar_ventana(0, 0, clase as u64, titulo as u64, false)
}

extern "win64" fn find_window_a(clase: *const u8, titulo: *const u8) -> u64 {
    buscar_ventana(0, 0, clase as u64, titulo as u64, true)
}

extern "win64" fn find_window_ex_w(padre: u64, despues: u64, clase: *const u16, titulo: *const u16) -> u64 {
    buscar_ventana(padre, despues, clase as u64, titulo as u64, false)
}

extern "win64" fn find_window_ex_a(padre: u64, despues: u64, clase: *const u8, titulo: *const u8) -> u64 {
    buscar_ventana(padre, despues, clase as u64, titulo as u64, true)
}

/// `WNDENUMPROC`: `BOOL CALLBACK(HWND, LPARAM)`; 0 para parar.
fn enumerar(lista: Vec<u64>, f: u64, l: u64) -> i32 {
    if f == 0 {
        return 0;
    }
    // SAFETY: la funcion del `.exe`, con la convencion de Windows.
    let f: extern "win64" fn(u64, u64) -> i32 = unsafe { core::mem::transmute(f as usize) };
    for h in lista {
        if viva(h) && f(h, l) == 0 {
            break;
        }
    }
    1
}

extern "win64" fn enum_windows(f: u64, l: u64) -> i32 {
    enumerar(hermanas(0, false), f, l)
}

extern "win64" fn enum_child_windows(padre: u64, f: u64, l: u64) -> i32 {
    // Todas las de debajo, nietas incluidas, como Windows.
    let todas = con(|e| e.ventanas.iter().filter(|v| v.viva).map(|v| v.hwnd).collect::<Vec<_>>());
    enumerar(todas.into_iter().filter(|&h| is_child(padre, h) != 0).collect(), f, l)
}

// -- Los nombres -------------------------------------------------------------

fn class_name(h: u64, buf: u64, n: i32, ansi: bool) -> i32 {
    let Some(atomo) = con_ventana(h, |v| v.datos.atomo) else { return 0 };
    let t = con(|e| e.clases.iter().find(|c| c.atomo == atomo).map(|c| c.nombre.clone())).unwrap_or_default();
    copiar_texto(&t, buf, n.max(0) as usize, ansi) as i32
}

extern "win64" fn get_class_name_w(h: u64, buf: *mut u16, n: i32) -> i32 {
    class_name(h, buf as u64, n, false)
}

extern "win64" fn get_class_name_a(h: u64, buf: *mut u8, n: i32) -> i32 {
    class_name(h, buf as u64, n, true)
}

fn window_text(h: u64, buf: u64, n: i32, ansi: bool) -> i32 {
    let t = con_ventana(h, |v| v.datos.titulo.clone()).unwrap_or_default();
    copiar_texto(&t, buf, n.max(0) as usize, ansi) as i32
}

extern "win64" fn get_window_text_w(h: u64, buf: *mut u16, n: i32) -> i32 {
    window_text(h, buf as u64, n, false)
}

extern "win64" fn get_window_text_a(h: u64, buf: *mut u8, n: i32) -> i32 {
    window_text(h, buf as u64, n, true)
}

extern "win64" fn get_window_text_length(h: u64) -> i32 {
    con_ventana(h, |v| v.datos.titulo.len() as i32).unwrap_or(0)
}

extern "win64" fn set_window_text_a(h: u64, t: *const u8) -> i32 {
    // SAFETY: una cadena suya.
    let t = if t.is_null() { Vec::new() } else { unsafe { bytes(t) } };
    poner_titulo(h, t) as i32
}

/// `GetWindowThreadProcessId`: el hilo que la creo; y el proceso, este.
extern "win64" fn get_window_thread_process_id(h: u64, pid: *mut u32) -> u32 {
    let Some(hilo) = con_ventana(h, |v| v.datos.hilo) else { return 0 };
    if !pid.is_null() {
        // SAFETY: un DWORD suyo.
        unsafe { pid.write(kernel32::id_del_proceso()) };
    }
    hilo
}

// -- La posicion -------------------------------------------------------------

const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_SHOWWINDOW: u32 = 0x0040;

/// Mover una ventana (y, si la medida cambia, decir que eso todavia no).
fn colocar(h: u64, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32 {
    let Some(sup) = user32::superficie_de(h) else {
        kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
        return 0;
    };
    if flags & SWP_NOMOVE == 0 {
        con_ventana(h, |v| {
            v.datos.x = x;
            v.datos.y = y;
        });
    }
    if flags & SWP_NOSIZE == 0 && (cx != sup.ancho as i32 || cy != sup.alto as i32) {
        aviso("SetWindowPos/MoveWindow con otra medida: la ventana se queda con la suya (todavia)");
    }
    if flags & SWP_SHOWWINDOW != 0 {
        user32::show_window(h, 5);
    }
    1
}

extern "win64" fn set_window_pos(h: u64, _despues: u64, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32 {
    colocar(h, x, y, cx, cy, flags)
}

extern "win64" fn move_window(h: u64, x: i32, y: i32, cx: i32, cy: i32, _repintar: i32) -> i32 {
    colocar(h, x, y, cx, cy, 0)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "GetWindowLongPtrW" | "GetWindowLongPtrA" => dir!(get_window_long_ptr),
        "SetWindowLongPtrW" | "SetWindowLongPtrA" => dir!(set_window_long_ptr),
        "GetWindowLongW" | "GetWindowLongA" => dir!(get_window_long),
        "SetWindowLongW" | "SetWindowLongA" => dir!(set_window_long),
        "GetClassLongPtrW" | "GetClassLongPtrA" => dir!(get_class_long_ptr),
        "SetClassLongPtrW" | "SetClassLongPtrA" => dir!(set_class_long_ptr),
        "GetClassLongW" | "GetClassLongA" => dir!(get_class_long),
        "SetClassLongW" | "SetClassLongA" => dir!(set_class_long),
        "RegisterClassW" => dir!(register_class_w),
        "RegisterClassA" => dir!(register_class_a),
        "RegisterClassExA" => dir!(register_class_ex_a),
        "UnregisterClassW" => dir!(unregister_class_w),
        "UnregisterClassA" => dir!(unregister_class_a),
        "GetClassInfoExW" => dir!(get_class_info_ex_w),
        "GetClassInfoExA" => dir!(get_class_info_ex_a),
        "GetClassInfoW" => dir!(get_class_info_w),
        "GetClassInfoA" => dir!(get_class_info_a),
        "CreateWindowExA" => dir!(create_window_ex_a),
        "DefWindowProcA" => dir!(def_window_proc_a),
        "IsWindow" => dir!(is_window),
        "IsWindowVisible" => dir!(is_window_visible),
        "IsWindowUnicode" => dir!(is_window_unicode),
        "IsIconic" => dir!(is_iconic),
        "IsZoomed" => dir!(is_zoomed),
        "IsWindowEnabled" => dir!(is_window_enabled),
        "EnableWindow" => dir!(enable_window),
        "GetForegroundWindow" | "GetActiveWindow" => dir!(get_foreground_window),
        "SetForegroundWindow" => dir!(set_foreground_window),
        "SetActiveWindow" => dir!(set_active_window),
        "GetFocus" => dir!(get_focus),
        "SetFocus" => dir!(set_focus),
        "BringWindowToTop" => dir!(bring_window_to_top),
        "GetParent" => dir!(get_parent),
        "GetAncestor" => dir!(get_ancestor),
        "GetWindow" => dir!(get_window),
        "IsChild" => dir!(is_child),
        "FindWindowW" => dir!(find_window_w),
        "FindWindowA" => dir!(find_window_a),
        "FindWindowExW" => dir!(find_window_ex_w),
        "FindWindowExA" => dir!(find_window_ex_a),
        "EnumWindows" => dir!(enum_windows),
        "EnumChildWindows" => dir!(enum_child_windows),
        "GetClassNameW" => dir!(get_class_name_w),
        "GetClassNameA" => dir!(get_class_name_a),
        "GetWindowTextW" => dir!(get_window_text_w),
        "GetWindowTextA" => dir!(get_window_text_a),
        "GetWindowTextLengthW" | "GetWindowTextLengthA" => dir!(get_window_text_length),
        "SetWindowTextA" => dir!(set_window_text_a),
        "GetWindowThreadProcessId" => dir!(get_window_thread_process_id),
        "SetWindowPos" => dir!(set_window_pos),
        "MoveWindow" => dir!(move_window),
        _ => return None,
    })
}
