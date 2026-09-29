//! **`user32.dll`, grupo 1: rectangulos, medidas, DPI y monitores** (tanda 5
//! de Cyberpunk, 29-09).
//!
//! ```text
//!    rectangulos  SetRect SetRectEmpty CopyRect InflateRect OffsetRect
//!                 IntersectRect UnionRect SubtractRect EqualRect IsRectEmpty
//!                 PtInRect
//!    medidas      GetSystemMetrics(ForDpi) SystemParametersInfoW/A
//!                 AdjustWindowRectExForDpi
//!    DPI          GetDpiForWindow GetDpiForSystem SetProcessDPIAware
//!                 IsProcessDPIAware SetProcessDpiAwarenessContext
//!                 Get/SetThreadDpiAwarenessContext AreDpiAwarenessContextsEqual
//!    monitores    MonitorFromWindow/Point/Rect GetMonitorInfoW/A
//!                 EnumDisplayMonitors EnumDisplaySettingsW/A(Ex)
//!                 EnumDisplayDevicesW/A
//!    ventanas     GetClientRect GetWindowRect ClientToScreen ScreenToClient
//!                 MapWindowPoints GetDesktopWindow
//! ```
//!
//! **UN monitor, dicho:** la casa no sabe todavia la medida de la pantalla
//! de BMO-X (la plataforma no la da), asi que tiene UNO fijo:
//! [`PANTALLA`], 1920x1080 a 96 DPI y 60 Hz, con 40 pixeles de barra de
//! tareas abajo (el area de trabajo). Todo lo de aqui sale de ahi: cuando la
//! plataforma la diga, se cambia en un sitio. Las medidas del marco (barras,
//! bordes, titulo) son las de Windows 10 a 96 DPI.
//!
//! **Las ventanas, dicho:** el marco lo pinta el escritorio de BMO-X, asi que
//! la ventana ES su area de cliente, en el (0, 0) de la pantalla.

use crate::{dir, kernel32, user32};

const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INVALID_WINDOW_HANDLE: u32 = 1400;

/// La pantalla de la casa: ancho, alto, DPI, hercios y la barra de tareas.
pub(crate) const PANTALLA: (i32, i32) = (1920, 1080);
const DPI: u32 = 96;
const HERCIOS: u32 = 60;
const BARRA: i32 = 40;
/// El HMONITOR del unico monitor y el HWND del escritorio.
const MONITOR: u64 = 0x5B00_0001;
const ESCRITORIO: u64 = 0x0001_0000;

// -- Los rectangulos (RECT: left, top, right, bottom; i32) ----------------------------

type Rect = [i32; 4];

fn leer(r: *const i32) -> Option<Rect> {
    // SAFETY: un RECT del `.exe` (si no es nulo).
    (!r.is_null()).then(|| unsafe { [r.read(), r.add(1).read(), r.add(2).read(), r.add(3).read()] })
}

fn poner(r: *mut i32, v: Rect) -> bool {
    if r.is_null() {
        return false;
    }
    // SAFETY: un RECT del `.exe`.
    unsafe {
        for (k, x) in v.iter().enumerate() {
            r.add(k).write(*x);
        }
    }
    true
}

const fn vacio(r: &Rect) -> bool {
    r[2] <= r[0] || r[3] <= r[1]
}

extern "win64" fn set_rect(r: *mut i32, a: i32, b: i32, c: i32, d: i32) -> i32 {
    poner(r, [a, b, c, d]) as i32
}

extern "win64" fn set_rect_empty(r: *mut i32) -> i32 {
    poner(r, [0; 4]) as i32
}

extern "win64" fn copy_rect(d: *mut i32, s: *const i32) -> i32 {
    leer(s).is_some_and(|v| poner(d, v)) as i32
}

extern "win64" fn inflate_rect(r: *mut i32, dx: i32, dy: i32) -> i32 {
    leer(r).is_some_and(|v| poner(r, [v[0] - dx, v[1] - dy, v[2] + dx, v[3] + dy])) as i32
}

extern "win64" fn offset_rect(r: *mut i32, dx: i32, dy: i32) -> i32 {
    leer(r).is_some_and(|v| poner(r, [v[0] + dx, v[1] + dy, v[2] + dx, v[3] + dy])) as i32
}

/// `IntersectRect`: el comun; si no lo hay, vacio (a ceros) y FALSE.
extern "win64" fn intersect_rect(d: *mut i32, a: *const i32, b: *const i32) -> i32 {
    let (Some(a), Some(b)) = (leer(a), leer(b)) else { return 0 };
    let c = [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])];
    if vacio(&a) || vacio(&b) || vacio(&c) {
        poner(d, [0; 4]);
        return 0;
    }
    poner(d, c) as i32
}

/// `UnionRect`: el menor que los contiene (un vacio no cuenta).
extern "win64" fn union_rect(d: *mut i32, a: *const i32, b: *const i32) -> i32 {
    let (Some(a), Some(b)) = (leer(a), leer(b)) else { return 0 };
    let c = match (vacio(&a), vacio(&b)) {
        (true, true) => [0; 4],
        (true, false) => b,
        (false, true) => a,
        (false, false) => [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])],
    };
    poner(d, c);
    (!vacio(&c)) as i32
}

/// `SubtractRect`: `a` menos `b`, SOLO si lo que queda es un rectangulo (`b`
/// cubre `a` de lado a lado por un borde); si no, `a` entero.
extern "win64" fn subtract_rect(d: *mut i32, a: *const i32, b: *const i32) -> i32 {
    let (Some(a), Some(b)) = (leer(a), leer(b)) else { return 0 };
    let mut c = a;
    if vacio(&a) {
        poner(d, [0; 4]);
        return 0;
    }
    if !vacio(&b) {
        let ancho = b[0] <= a[0] && b[2] >= a[2];
        let alto = b[1] <= a[1] && b[3] >= a[3];
        if ancho && alto {
            c = [0; 4];
        } else if ancho && b[1] <= a[1] && b[3] > a[1] {
            c[1] = b[3];
        } else if ancho && b[3] >= a[3] && b[1] < a[3] {
            c[3] = b[1];
        } else if alto && b[0] <= a[0] && b[2] > a[0] {
            c[0] = b[2];
        } else if alto && b[2] >= a[2] && b[0] < a[2] {
            c[2] = b[0];
        }
    }
    if vacio(&c) {
        c = [0; 4];
    }
    poner(d, c);
    (!vacio(&c)) as i32
}

extern "win64" fn equal_rect(a: *const i32, b: *const i32) -> i32 {
    matches!((leer(a), leer(b)), (Some(a), Some(b)) if a == b) as i32
}

extern "win64" fn is_rect_empty(r: *const i32) -> i32 {
    leer(r).is_none_or(|v| vacio(&v)) as i32
}

/// `PtInRect(rect, POINT por valor)`: el borde derecho y el de abajo, fuera.
extern "win64" fn pt_in_rect(r: *const i32, p: u64) -> i32 {
    let (x, y) = (p as u32 as i32, (p >> 32) as u32 as i32);
    leer(r).is_some_and(|v| x >= v[0] && x < v[2] && y >= v[1] && y < v[3]) as i32
}

// -- Las medidas ------------------------------------------------------------------------

/// Las de Windows 10 que dependen de los DPI (a 96).
fn medida_96(i: i32) -> Option<i32> {
    Some(match i {
        2 | 3 | 9 | 10 | 20 | 21 => 17,
        4 => 23,
        5 | 6 | 83 | 84 => 1,
        7 | 8 => 3,
        11 | 12 | 13 | 14 => 32,
        15 => 20,
        28 | 34 => 136,
        29 | 35 => 39,
        30 => 36,
        31 => 22,
        32 | 33 | 92 => 4,
        36 | 37 | 68 | 69 => 4,
        45 | 46 => 2,
        49 | 50 => 16,
        52 => 16,
        53 => 22,
        54 => 36,
        55 => 22,
        57 => 160,
        58 => 28,
        71 | 72 => 13,
        _ => return None,
    })
}

/// `GetSystemMetricsForDpi`: las de medida, a esos DPI; las de la pantalla,
/// tal cual.
extern "win64" fn get_system_metrics_for_dpi(i: i32, dpi: u32) -> i32 {
    let (w, h) = PANTALLA;
    if let Some(v) = medida_96(i) {
        return (v as u32 * dpi.max(1)).div_ceil(DPI) as i32;
    }
    match i {
        0 | 16 | 78 => w,
        1 | 79 => h,
        17 => h - BARRA - 23,
        59 => w + 22,
        60 => h + 22,
        61 => w + 16,
        62 => h - BARRA + 16,
        // SM_MOUSEPRESENT, SM_MOUSEWHEELPRESENT, SM_CMONITORS,
        // SM_SAMEDISPLAYFORMAT; tres botones.
        19 | 75 | 80 | 81 => 1,
        43 => 3,
        _ => 0,
    }
}

extern "win64" fn get_system_metrics(i: i32) -> i32 {
    get_system_metrics_for_dpi(i, DPI)
}

fn area_de_trabajo() -> Rect {
    [0, 0, PANTALLA.0, PANTALLA.1 - BARRA]
}

/// `SystemParametersInfoW/A`: lo que un juego pregunta (el area de trabajo,
/// las teclas de accesibilidad para apagar sus atajos, el raton); lo que
/// PONE se acepta y no cambia nada.
extern "win64" fn system_parameters_info(accion: u32, _p: u32, v: *mut u8, _ini: u32) -> i32 {
    // SAFETY: lo que cada accion dice que es `v`, del `.exe`.
    unsafe {
        match accion {
            // SPI_GETWORKAREA
            0x30 => poner(v as *mut i32, area_de_trabajo()) as i32,
            // SPI_GETSCREENSAVEACTIVE, SPI_GETWHEELSCROLLLINES
            0x10 => poner32(v, 0),
            0x68 => poner32(v, 3),
            // SPI_GETMOUSE: umbrales 6 y 10, aceleracion 1.
            0x03 if !v.is_null() => {
                (v as *mut [i32; 3]).write_unaligned([6, 10, 1]);
                1
            }
            // SPI_GETMOUSESPEED (1..20)
            0x70 => poner32(v, 10),
            // SPI_GETSTICKYKEYS, TOGGLEKEYS, FILTERKEYS: cbSize y las banderas
            // (disponibles, sin encender).
            0x3A | 0x34 if !v.is_null() => {
                (v.add(4) as *mut u32).write_unaligned(0x02 | 0x04);
                1
            }
            0x32 if !v.is_null() => {
                core::ptr::write_bytes(v.add(4), 0, 20);
                (v.add(4) as *mut u32).write_unaligned(0x02);
                1
            }
            // Los SET de esas mismas: si, y nada cambia.
            0x3B | 0x35 | 0x33 | 0x11 | 0x69 | 0x71 => 1,
            _ => {
                crate::aviso(&alloc::format!("SystemParametersInfo({accion:#x}): la casa no la sabe"));
                kernel32::poner_error(ERROR_INVALID_PARAMETER);
                0
            }
        }
    }
}

fn poner32(v: *mut u8, x: u32) -> i32 {
    if v.is_null() {
        return 0;
    }
    // SAFETY: un UINT/BOOL del `.exe`.
    unsafe { (v as *mut u32).write_unaligned(x) };
    1
}

/// `AdjustWindowRectExForDpi`: como AdjustWindowRectEx (el marco lo pone el
/// escritorio: la ventana es su area de cliente).
extern "win64" fn adjust_window_rect_ex_for_dpi(r: *mut i32, _estilo: u32, _menu: i32, _ex: u32, _dpi: u32) -> i32 {
    (!r.is_null()) as i32
}

// -- DPI --------------------------------------------------------------------------------

/// DPI_AWARENESS_CONTEXT: -1 no se entera, -2 sistema, -3 por monitor, -4
/// por monitor v2, -5 GDI escalado.
const SIN_ENTERARSE: i64 = -1;

struct Dpi(core::cell::UnsafeCell<(bool, i64)>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
unsafe impl Sync for Dpi {}
static DPI_PROCESO: Dpi = Dpi(core::cell::UnsafeCell::new((false, SIN_ENTERARSE)));

fn dpi_estado() -> &'static mut (bool, i64) {
    // SAFETY: ver `Dpi`.
    unsafe { &mut *DPI_PROCESO.0.get() }
}

pub(crate) fn reiniciar() {
    *dpi_estado() = (false, SIN_ENTERARSE);
}

extern "win64" fn get_dpi() -> u32 {
    DPI
}

extern "win64" fn get_dpi_for_window(h: u64) -> u32 {
    if user32::superficie_de(h).is_some() || h == ESCRITORIO {
        DPI
    } else {
        kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
        0
    }
}

extern "win64" fn set_process_dpi_aware() -> i32 {
    *dpi_estado() = (true, -2);
    1
}

extern "win64" fn is_process_dpi_aware() -> i32 {
    dpi_estado().0 as i32
}

extern "win64" fn set_process_dpi_awareness_context(c: i64) -> i32 {
    if !(-5..=-1).contains(&c) {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    *dpi_estado() = (c != SIN_ENTERARSE, c);
    1
}

extern "win64" fn get_thread_dpi_awareness_context() -> i64 {
    dpi_estado().1
}

/// Devuelve el de antes (y el proceso entero pasa a este: un hilo).
extern "win64" fn set_thread_dpi_awareness_context(c: i64) -> i64 {
    if !(-5..=-1).contains(&c) {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let antes = dpi_estado().1;
    *dpi_estado() = (c != SIN_ENTERARSE, c);
    antes
}

extern "win64" fn are_dpi_awareness_contexts_equal(a: i64, b: i64) -> i32 {
    (a == b) as i32
}

// -- Los monitores ----------------------------------------------------------------------

extern "win64" fn monitor_from(_x: u64, _banderas: u32) -> u64 {
    MONITOR
}

/// `GetMonitorInfoW/A`: MONITORINFO (40 B) o MONITORINFOEX (104 B la W, 72
/// la A), con su nombre.
fn monitor_info(m: u64, i: *mut u8, ancho: bool) -> i32 {
    if m != MONITOR || i.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    // SAFETY: el MONITORINFO del `.exe`, de la medida que dice su cbSize.
    unsafe {
        let cb = (i as *const u32).read_unaligned();
        let ex = if ancho { 104 } else { 72 };
        if cb != 40 && cb != ex {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            return 0;
        }
        poner(i.add(4) as *mut i32, [0, 0, PANTALLA.0, PANTALLA.1]);
        poner(i.add(20) as *mut i32, area_de_trabajo());
        // MONITORINFOF_PRIMARY
        (i.add(36) as *mut u32).write_unaligned(1);
        if cb == ex {
            nombre(i.add(40), NOMBRE_PANTALLA, 32, ancho);
        }
    }
    1
}

const NOMBRE_PANTALLA: &str = "\\\\.\\DISPLAY1";

/// `texto` en `d` (UTF-16 si `ancho`), con su 0, en `max` caracteres.
///
/// # Safety
/// `d` tiene sitio para `max` caracteres.
unsafe fn nombre(d: *mut u8, texto: &str, max: usize, ancho: bool) {
    let n = texto.len().min(max - 1);
    for (k, b) in texto.bytes().take(n).enumerate() {
        if ancho {
            (d.add(2 * k) as *mut u16).write_unaligned(b as u16);
        } else {
            d.add(k).write(b);
        }
    }
    if ancho {
        (d.add(2 * n) as *mut u16).write_unaligned(0);
    } else {
        d.add(n).write(0);
    }
}

extern "win64" fn get_monitor_info_w(m: u64, i: *mut u8) -> i32 {
    monitor_info(m, i, true)
}

extern "win64" fn get_monitor_info_a(m: u64, i: *mut u8) -> i32 {
    monitor_info(m, i, false)
}

/// `EnumDisplayMonitors(hdc, recorte, f, dato)`: `f(hmonitor, hdc, rect,
/// dato)` con el unico monitor.
extern "win64" fn enum_display_monitors(hdc: u64, _recorte: *const i32, f: u64, dato: u64) -> i32 {
    if f == 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let r: Rect = [0, 0, PANTALLA.0, PANTALLA.1];
    type Cada = extern "win64" fn(u64, u64, *const i32, u64) -> i32;
    // SAFETY: el MONITORENUMPROC del `.exe`.
    let f: Cada = unsafe { core::mem::transmute(f as usize) };
    f(MONITOR, hdc, r.as_ptr(), dato);
    1
}

/// Los modos que se ofrecen (el ultimo, el de la pantalla), a 32 bits.
const MODOS: [(u32, u32); 6] = [(640, 480), (800, 600), (1024, 768), (1280, 720), (1600, 900), (1920, 1080)];

/// `EnumDisplaySettings(Ex)W/A`: el modo actual (ENUM_CURRENT_SETTINGS o
/// ENUM_REGISTRY_SETTINGS) o el `n` de la lista, en un DEVMODE (220 B la W,
/// 156 la A).
fn display_settings(modo: u32, d: *mut u8, ancho: bool) -> i32 {
    let (w, h) = match modo {
        0xFFFF_FFFF | 0xFFFF_FFFE => (PANTALLA.0 as u32, PANTALLA.1 as u32),
        n if (n as usize) < MODOS.len() => MODOS[n as usize],
        _ => return 0,
    };
    if d.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    // Las medidas de DEVMODEW y DEVMODEA: los campos de la pantalla.
    let (base, tam) = if ancho { (64usize, 220u16) } else { (32usize, 156u16) };
    let pixeles = if ancho { 168 } else { 104 };
    // SAFETY: el DEVMODE del `.exe` (dmSize lo pone quien llama; se escribe
    // en su medida estandar).
    unsafe {
        core::ptr::write_bytes(d, 0, tam as usize);
        nombre(d, NOMBRE_PANTALLA, 32, ancho);
        (d.add(base) as *mut u16).write_unaligned(0x0401);
        (d.add(base + 4) as *mut u16).write_unaligned(tam);
        // DM_POSITION | DM_BITSPERPEL | DM_PELSWIDTH | DM_PELSHEIGHT |
        // DM_DISPLAYFLAGS | DM_DISPLAYFREQUENCY
        (d.add(base + 8) as *mut u32).write_unaligned(0x20 | 0x4_0000 | 0x8_0000 | 0x10_0000 | 0x20_0000 | 0x40_0000);
        (d.add(pixeles) as *mut u32).write_unaligned(32);
        (d.add(pixeles + 4) as *mut u32).write_unaligned(w);
        (d.add(pixeles + 8) as *mut u32).write_unaligned(h);
        (d.add(pixeles + 16) as *mut u32).write_unaligned(HERCIOS);
    }
    1
}

extern "win64" fn enum_display_settings_w(_disp: *const u16, modo: u32, d: *mut u8) -> i32 {
    display_settings(modo, d, true)
}

extern "win64" fn enum_display_settings_a(_disp: *const u8, modo: u32, d: *mut u8) -> i32 {
    display_settings(modo, d, false)
}

extern "win64" fn enum_display_settings_ex_w(_disp: *const u16, modo: u32, d: *mut u8, _banderas: u32) -> i32 {
    display_settings(modo, d, true)
}

/// `EnumDisplayDevicesW/A(dispositivo, n, DISPLAY_DEVICE, banderas)`: sin
/// dispositivo, el adaptador 0 (la pantalla de BMO-X, primaria); con el,
/// su monitor 0. DISPLAY_DEVICEW mide 840 B; la A, 424.
fn display_devices(disp: bool, n: u32, d: *mut u8, ancho: bool) -> i32 {
    if n != 0 || d.is_null() {
        return 0;
    }
    let (nom, texto, estado) = if disp { ("\\\\.\\DISPLAY1\\Monitor0", "Monitor de BMO-X", 0x1u32 | 0x2) } else { (NOMBRE_PANTALLA, "PROTON-X (la pantalla de BMO-X)", 0x1u32 | 0x4) };
    let (o_texto, o_estado, o_id, o_clave) = if ancho { (68, 324, 328, 584) } else { (36, 164, 168, 296) };
    // SAFETY: el DISPLAY_DEVICE del `.exe`, de su medida (cb).
    unsafe {
        let cb = (d as *const u32).read_unaligned();
        core::ptr::write_bytes(d.add(4), 0, (cb as usize).saturating_sub(4).min(if ancho { 836 } else { 420 }));
        nombre(d.add(4), nom, 32, ancho);
        nombre(d.add(o_texto), texto, 128, ancho);
        (d.add(o_estado) as *mut u32).write_unaligned(estado);
        nombre(d.add(o_id), "BMOX\\PANTALLA", 128, ancho);
        nombre(d.add(o_clave), "", 128, ancho);
    }
    1
}

extern "win64" fn enum_display_devices_w(disp: *const u16, n: u32, d: *mut u8, _banderas: u32) -> i32 {
    display_devices(!disp.is_null(), n, d, true)
}

extern "win64" fn enum_display_devices_a(disp: *const u8, n: u32, d: *mut u8, _banderas: u32) -> i32 {
    display_devices(!disp.is_null(), n, d, false)
}

// -- La geometria de una ventana --------------------------------------------------------

fn cliente(h: u64) -> Option<Rect> {
    if h == ESCRITORIO {
        return Some([0, 0, PANTALLA.0, PANTALLA.1]);
    }
    user32::superficie_de(h).map(|s| [0, 0, s.ancho as i32, s.alto as i32])
}

extern "win64" fn get_client_rect(h: u64, r: *mut i32) -> i32 {
    match cliente(h) {
        Some(c) => poner(r, c) as i32,
        None => {
            kernel32::poner_error(ERROR_INVALID_WINDOW_HANDLE);
            0
        }
    }
}

/// `GetWindowRect`: la ventana ES su cliente, en el (0, 0).
extern "win64" fn get_window_rect(h: u64, r: *mut i32) -> i32 {
    get_client_rect(h, r)
}

/// `ClientToScreen` y `ScreenToClient`: el cliente esta en el (0, 0).
extern "win64" fn client_to_screen(h: u64, p: *mut i32) -> i32 {
    (cliente(h).is_some() && !p.is_null()) as i32
}

/// `MapWindowPoints(de, a, puntos, n)`: todo en el mismo origen: nada se
/// mueve. Devuelve el desplazamiento (0).
extern "win64" fn map_window_points(_de: u64, _a: u64, _p: *mut i32, _n: u32) -> i32 {
    0
}

extern "win64" fn get_desktop_window() -> u64 {
    ESCRITORIO
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "SetRect" => dir!(set_rect),
        "SetRectEmpty" => dir!(set_rect_empty),
        "CopyRect" => dir!(copy_rect),
        "InflateRect" => dir!(inflate_rect),
        "OffsetRect" => dir!(offset_rect),
        "IntersectRect" => dir!(intersect_rect),
        "UnionRect" => dir!(union_rect),
        "SubtractRect" => dir!(subtract_rect),
        "EqualRect" => dir!(equal_rect),
        "IsRectEmpty" => dir!(is_rect_empty),
        "PtInRect" => dir!(pt_in_rect),
        "GetSystemMetrics" => dir!(get_system_metrics),
        "GetSystemMetricsForDpi" => dir!(get_system_metrics_for_dpi),
        "SystemParametersInfoW" | "SystemParametersInfoA" => dir!(system_parameters_info),
        "AdjustWindowRectExForDpi" => dir!(adjust_window_rect_ex_for_dpi),
        "GetDpiForSystem" => dir!(get_dpi),
        "GetDpiForWindow" => dir!(get_dpi_for_window),
        "SetProcessDPIAware" => dir!(set_process_dpi_aware),
        "IsProcessDPIAware" => dir!(is_process_dpi_aware),
        "SetProcessDpiAwarenessContext" => dir!(set_process_dpi_awareness_context),
        "GetThreadDpiAwarenessContext" => dir!(get_thread_dpi_awareness_context),
        "SetThreadDpiAwarenessContext" => dir!(set_thread_dpi_awareness_context),
        "AreDpiAwarenessContextsEqual" => dir!(are_dpi_awareness_contexts_equal),
        "MonitorFromWindow" | "MonitorFromPoint" | "MonitorFromRect" => dir!(monitor_from),
        "GetMonitorInfoW" => dir!(get_monitor_info_w),
        "GetMonitorInfoA" => dir!(get_monitor_info_a),
        "EnumDisplayMonitors" => dir!(enum_display_monitors),
        "EnumDisplaySettingsW" => dir!(enum_display_settings_w),
        "EnumDisplaySettingsA" => dir!(enum_display_settings_a),
        "EnumDisplaySettingsExW" => dir!(enum_display_settings_ex_w),
        "EnumDisplayDevicesW" => dir!(enum_display_devices_w),
        "EnumDisplayDevicesA" => dir!(enum_display_devices_a),
        "GetClientRect" => dir!(get_client_rect),
        "GetWindowRect" => dir!(get_window_rect),
        "ClientToScreen" | "ScreenToClient" => dir!(client_to_screen),
        "MapWindowPoints" => dir!(map_window_points),
        "GetDesktopWindow" => dir!(get_desktop_window),
        _ => return None,
    })
}
