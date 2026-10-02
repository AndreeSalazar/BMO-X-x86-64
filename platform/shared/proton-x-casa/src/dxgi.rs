//! **`dxgi.dll` de la casa** (P3a, 27-09): la fabrica y la cadena de
//! intercambio, sobre la ventana de P2.
//!
//! ```text
//!    CreateDXGIFactory1/2       la fabrica
//!    CreateSwapChainForHwnd     N back buffers (recursos de d3d12.rs) del
//!                               medida de la ventana, y la ventana
//!    GetBuffer(i)               el back buffer i
//!    Present                    el back buffer ACTUAL a la superficie de la
//!                               ventana (el de P2), y se pasa al siguiente
//!    MakeWindowAssociation      nada: aqui no hay Alt+Enter que desactivar
//!    (P3c4, lo que pide el cubo de BMOX-12)
//!    la fabrica es IDXGIFactory6 EnumAdapterByGpuPreference, EnumAdapters1,
//!                               EnumAdapters: UN adaptador, el 0
//!                               CheckFeatureSupport(PRESENT_ALLOW_TEARING):
//!                               FALSE (Present no rompe la imagen)
//!    IDXGIAdapter1              GetDesc1: "PROTON-X (la CPU de BMO-X)", con
//!                               la bandera SOFTWARE: dibuja la CPU, y se dice;
//!                               GetDesc (01-10, Cyberpunk): lo mismo sin Flags
//!    IDXGISwapChain3            GetCurrentBackBufferIndex
//! ```
//!
//! Los buffers giran como en el modelo FLIP de Windows: tras `Present` el
//! actual es el siguiente (el `.exe` lleva la cuenta o pregunta a
//! `GetCurrentBackBufferIndex`).

use alloc::vec::Vec;

use crate::com::{self, dar, de, nuevo, pide, vtabla, Guid, E_INVALIDARG, E_NOINTERFACE, E_OUTOFMEMORY, S_OK};
use crate::d3d12::{recurso, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM};
use crate::{aviso, dir, plataforma, user32};

use bmo_proton_x::registro::Registro;
use core::cell::UnsafeCell;

// -- EL REGISTRO (28-09): una linea por segundo con los fps y los tiempos ----

struct Cuenta(UnsafeCell<Option<Registro>>);
// SAFETY: un hilo dibuja (ver `Global` en lib.rs).
unsafe impl Sync for Cuenta {}
static REGISTRO: Cuenta = Cuenta(UnsafeCell::new(None));

fn registro() -> &'static mut Registro {
    // SAFETY: un hilo; nadie guarda la referencia.
    unsafe { (*REGISTRO.0.get()).get_or_insert_with(Registro::default) }
}

/// Un `.exe` nuevo, un registro nuevo.
pub(crate) fn reiniciar() {
    // SAFETY: antes de saltar al `.exe` (ver `empezar`).
    unsafe { *REGISTRO.0.get() = None };
}

/// Lo que tardo un ExecuteCommandLists (lo dibujado: sombreadores y trama).
pub(crate) fn dibujado(ns: u64) {
    registro().dibujo(ns);
}

pub struct Fabrica;

pub struct Cadena {
    pub(crate) hwnd: u64,
    pub(crate) buffers: Vec<u64>,
    pub(crate) actual: usize,
    /// Tanda 47: su DXGI_SWAP_CHAIN_DESC1 (48 B), con el ancho y el alto ya
    /// resueltos; y lo que el `.exe` va pidiendo (ver `dxgi_resto`).
    pub(crate) desc1: [u8; 48],
    pub(crate) presentes: u32,
    pub(crate) latencia: u32,
    pub(crate) completa: bool,
    pub(crate) espera: u64,
    pub(crate) fuente: (u32, u32),
}

fn fabrica(riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::FACTORY) {
        aviso("CreateDXGIFactory: pide una fabrica que la casa no tiene (IDXGIFactory7+?)");
        return E_NOINTERFACE;
    }
    let mut m = alloc::vec![
        (7, dir!(enum_adapters)),
        (8, dir!(make_window_association)),
        (12, dir!(enum_adapters)),
        (15, dir!(create_swap_chain_for_hwnd)),
        (28, dir!(check_feature_support)),
        (29, dir!(enum_adapter_by_gpu_preference)),
    ];
    m.extend_from_slice(&crate::dxgi_resto::fabrica());
    let vt = vtabla::<{ com::FACTORY }>(&m);
    dar(pp, nuevo(com::FACTORY, vt, Fabrica) as u64)
}

extern "win64" fn create_dxgi_factory1(riid: *const Guid, pp: *mut u64) -> i32 {
    fabrica(riid, pp)
}

extern "win64" fn create_dxgi_factory2(_banderas: u32, riid: *const Guid, pp: *mut u64) -> i32 {
    fabrica(riid, pp)
}

extern "win64" fn make_window_association(_this: u64, _hwnd: u64, _banderas: u32) -> i32 {
    S_OK
}

/// `CreateSwapChainForHwnd(this, cola, hwnd, desc1, fs, salida, pp)`.
/// `DXGI_SWAP_CHAIN_DESC1`: Width +0, Height +4, Format +8, BufferCount +28.
/// Un ancho o alto 0 es "el de la ventana", como en Windows.
pub(crate) extern "win64" fn create_swap_chain_for_hwnd(_this: u64, _cola: u64, hwnd: u64, desc: *const u8, _fs: *const u8, _salida: u64, pp: *mut u64) -> i32 {
    if desc.is_null() {
        return E_INVALIDARG;
    }
    let Some(sup) = user32::superficie_de(hwnd) else {
        aviso("CreateSwapChainForHwnd: esa ventana no es de la casa");
        return E_INVALIDARG;
    };
    // SAFETY: un DXGI_SWAP_CHAIN_DESC1 del `.exe` (48 bytes).
    let (w, h, formato, n) = unsafe {
        let u = |o: usize| (desc.add(o) as *const u32).read_unaligned();
        (u(0), u(4), u(8), u(28))
    };
    if formato != DXGI_FORMAT_R8G8B8A8_UNORM && formato != DXGI_FORMAT_B8G8R8A8_UNORM {
        aviso("CreateSwapChainForHwnd: solo R8G8B8A8_UNORM y B8G8R8A8_UNORM, todavia");
        return E_INVALIDARG;
    }
    let (w, h) = (if w == 0 { sup.ancho } else { w }, if h == 0 { sup.alto } else { h });
    let Some(buffers) = (0..n.clamp(1, 4)).map(|_| recurso(w, h, formato, true)).collect::<Option<Vec<u64>>>() else {
        aviso("CreateSwapChainForHwnd: no hay memoria para los back buffers: E_OUTOFMEMORY");
        return E_OUTOFMEMORY;
    };
    let mut m = alloc::vec![(8, dir!(present)), (9, dir!(get_buffer)), (15, dir!(get_containing_output)), (36, dir!(get_current_back_buffer_index))];
    m.extend_from_slice(&crate::dxgi_resto::cadena());
    let vt = vtabla::<{ com::SWAPCHAIN }>(&m);
    let mut d1 = [0u8; 48];
    // SAFETY: el DXGI_SWAP_CHAIN_DESC1 del `.exe` (48 bytes).
    unsafe { core::ptr::copy_nonoverlapping(desc, d1.as_mut_ptr(), 48) };
    d1[0..4].copy_from_slice(&w.to_le_bytes());
    d1[4..8].copy_from_slice(&h.to_le_bytes());
    let c = Cadena { hwnd, buffers, actual: 0, desc1: d1, presentes: 0, latencia: 3, completa: false, espera: 0, fuente: (w, h) };
    dar(pp, nuevo(com::SWAPCHAIN, vt, c) as u64)
}

extern "win64" fn get_buffer(this: u64, i: u32, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::RESOURCE) {
        return E_NOINTERFACE;
    }
    // SAFETY: `this` es una Cadena de la casa.
    let c = unsafe { de::<Cadena>(this) };
    match c.buffers.get(i as usize) {
        Some(&b) => dar(pp, b),
        None => E_INVALIDARG,
    }
}

/// `Present(this, intervalo, banderas)`: el back buffer actual a la ventana.
/// Lo que no cabe se recorta; lo que sobra de la ventana no se toca.
pub(crate) extern "win64" fn present(this: u64, _intervalo: u32, _banderas: u32) -> i32 {
    let empezo = (plataforma().ahora_ns)();
    // SAFETY: `this` es una Cadena de la casa.
    let c = unsafe { de::<Cadena>(this) };
    let Some(sup) = user32::superficie_de(c.hwnd) else { return E_INVALIDARG };
    // Lo que se limpio y nadie dibujo encima, antes de mirarlo (P3b4c).
    crate::tuberia::aplicar_limpieza(c.buffers[c.actual]);
    // SAFETY: un Recurso de la casa.
    let r = unsafe { de::<crate::d3d12::Recurso>(c.buffers[c.actual]) };
    // ** P3b4c.9 Z1: si la 3060 lo puso YA en la pantalla, no hay nada que
    // copiar: su RAM no tiene el fotograma, y la superficie no la compone
    // nadie. Se olvida aqui: el siguiente fotograma lo vuelve a decir.
    let en_pantalla = core::mem::take(&mut r.en_pantalla);
    // SAFETY: la superficie mide `stride * alto` pixeles y es de este proceso.
    let destino = unsafe { core::slice::from_raw_parts_mut(sup.pixeles, sup.stride as usize * sup.alto as usize) };
    let (w, h) = if en_pantalla { (0, 0) } else { (r.ancho.min(sup.ancho) as usize, r.alto.min(sup.alto) as usize) };
    for y in 0..h {
        let fila = &r.pixeles[y * r.ancho as usize..][..w];
        let dst = &mut destino[y * sup.stride as usize..][..w];
        for (d, &p) in dst.iter_mut().zip(fila) {
            // La superficie es B,G,R,A: R8G8B8A8 cambia R y B de sitio.
            *d = if r.formato == DXGI_FORMAT_R8G8B8A8_UNORM { p & 0xFF00_FF00 | (p & 0xFF) << 16 | (p >> 16) & 0xFF } else { p };
        }
    }
    (plataforma().presentar)(&sup);
    c.actual = (c.actual + 1) % c.buffers.len();
    c.presentes = c.presentes.wrapping_add(1);
    let ahora = (plataforma().ahora_ns)();
    if let Some(linea) = registro().presente(ahora, ahora.saturating_sub(empezo)) {
        (plataforma().escribir)(linea.as_bytes());
    }
    S_OK
}

// -- P3c4: el adaptador, Factory5/6 y SwapChain3 ----------------------------

const DXGI_ERROR_NOT_FOUND: i32 = 0x887A_0002_u32 as i32;
const DXGI_FEATURE_PRESENT_ALLOW_TEARING: u32 = 0;

pub struct Adaptador;

/// El adaptador `i`: solo hay uno, el 0 (la CPU de la casa).
pub(crate) fn adaptador(i: u32, riid: *const Guid, pp: *mut u64) -> i32 {
    if pp.is_null() {
        return E_INVALIDARG;
    }
    if i != 0 {
        // SAFETY: un puntero del `.exe`.
        unsafe { *pp = 0 };
        return DXGI_ERROR_NOT_FOUND;
    }
    if !riid.is_null() && !pide(riid, com::ADAPTER) {
        return E_NOINTERFACE;
    }
    let vt = vtabla::<{ com::ADAPTER }>(&[
        (7, dir!(enum_outputs)),
        (8, dir!(get_desc)),
        (10, dir!(get_desc1)),
        (11, dir!(get_desc2)),
        (12, dir!(registrar_aviso)),
        (13, dir!(quitar_aviso)),
        (14, dir!(query_video_memory_info)),
        (15, dir!(set_video_memory_reservation)),
        (16, dir!(registrar_aviso)),
        (17, dir!(quitar_aviso)),
        (18, dir!(get_desc2)),
        (9, dir!(crate::dxgi_resto::check_interface_support)),
    ]);
    dar(pp, nuevo(com::ADAPTER, vt, Adaptador) as u64)
}

/// `EnumAdapters(this, i, pp)` y `EnumAdapters1`: la misma forma.
extern "win64" fn enum_adapters(_this: u64, i: u32, pp: *mut u64) -> i32 {
    adaptador(i, core::ptr::null(), pp)
}

/// `EnumAdapterByGpuPreference(this, i, preferencia, riid, pp)`.
extern "win64" fn enum_adapter_by_gpu_preference(_this: u64, i: u32, _preferencia: u32, riid: *const Guid, pp: *mut u64) -> i32 {
    adaptador(i, riid, pp)
}

/// `CheckFeatureSupport(this, que, datos, medida)`: el tearing no (Present
/// copia la imagen entera: nunca la rompe). Lo demas, E_INVALIDARG.
extern "win64" fn check_feature_support(_this: u64, que: u32, datos: *mut u32, medida: u32) -> i32 {
    if que != DXGI_FEATURE_PRESENT_ALLOW_TEARING || datos.is_null() || medida != 4 {
        return E_INVALIDARG;
    }
    // SAFETY: un BOOL del `.exe`.
    unsafe { datos.write_unaligned(0) };
    S_OK
}

/// `GetDesc1(this, desc)`: DXGI_ADAPTER_DESC1 (312 B): Description (128
/// WCHAR) +0, VendorId +256, DeviceId +260, SubSysId +264, Revision +268,
/// las tres memorias (SIZE_T) +272 +280 +288, AdapterLuid +296, Flags +304
/// (0: una tarjeta de verdad, no DXGI_ADAPTER_FLAG_SOFTWARE).
extern "win64" fn get_desc1(_this: u64, desc: *mut u8) -> i32 {
    if desc.is_null() {
        return E_INVALIDARG;
    }
    escribir_desc(desc, 312);
    S_OK
}

/// `GetDesc2` y `GetDesc3` (DXGI_ADAPTER_DESC2/3, 320 B): lo del 1, y las
/// granularidades de expropiacion de graficos +308 y de computo +312 (0:
/// por bufer DMA, la mas gruesa).
extern "win64" fn get_desc2(_this: u64, desc: *mut u8) -> i32 {
    if desc.is_null() {
        return E_INVALIDARG;
    }
    escribir_desc(desc, 320);
    S_OK
}

/// `GetDesc(this, desc)` (hueco 8, 01-10: Cyberpunk lo pide antes que el 1):
/// DXGI_ADAPTER_DESC, lo mismo que el 1 hasta el LUID (304 B, sin Flags).
extern "win64" fn get_desc(_this: u64, desc: *mut u8) -> i32 {
    if desc.is_null() {
        return E_INVALIDARG;
    }
    escribir_desc(desc, 304);
    S_OK
}

const GIB: u64 = 1 << 30;
/// La tarjeta del propietario, la que BMO-X lee en el bus (SALIDA, `sysinfo`):
/// una RTX 3060 de MSI, 10DE:2504 sub 1462:397D rev A1, 12 GiB (02-10: era
/// "PROTON-X", fabricante 0, SIN memoria y por SOFTWARE, y un juego descarta
/// un adaptador asi). Como los procesadores (`bmo_proton_x::procesadores`):
/// lo que se CUENTA es la maquina; lo que dibuja es la casa.
const NOMBRE: &str = "NVIDIA GeForce RTX 3060";
const FABRICANTE: u32 = 0x10DE;
const APARATO: u32 = 0x2504;
const SUBSISTEMA: u32 = 0x397D_1462;
const REVISION: u32 = 0xA1;
const VRAM: u64 = 12 * GIB;
/// La compartida: la mitad de la RAM (16 GiB), como Windows.
const COMPARTIDA: u64 = 8 * GIB;
/// El LUID del adaptador (no cero: Windows nunca da 0). El mismo que da
/// `ID3D12Device::GetAdapterLuid`.
pub(crate) const LUID: u64 = 0x0000_0000_0000_B0E0;

/// Lo comun de GetDesc, 1, 2 y 3: `n` bytes (304, 312 o 320), lo que no se
/// dice a cero.
fn escribir_desc(desc: *mut u8, n: usize) {
    // SAFETY: `n` bytes del `.exe` (quien llama lo comprobo no nulo).
    unsafe {
        core::ptr::write_bytes(desc, 0, n);
        for (k, c) in NOMBRE.encode_utf16().enumerate() {
            (desc.add(2 * k) as *mut u16).write_unaligned(c);
        }
        for (off, v) in [(256, FABRICANTE), (260, APARATO), (264, SUBSISTEMA), (268, REVISION)] {
            (desc.add(off) as *mut u32).write_unaligned(v);
        }
        (desc.add(272) as *mut u64).write_unaligned(VRAM);
        (desc.add(288) as *mut u64).write_unaligned(COMPARTIDA);
        (desc.add(296) as *mut u64).write_unaligned(LUID);
    }
}

/// `QueryVideoMemoryInfo(this, nodo, grupo, info)`: DXGI_QUERY_VIDEO_MEMORY_INFO
/// (Budget, CurrentUsage, AvailableForReservation, CurrentReservation): la
/// local (0) es la VRAM y la otra (1) la compartida; el presupuesto, el 90 %
/// como en Windows; nada usado.
extern "win64" fn query_video_memory_info(_this: u64, nodo: u32, grupo: u32, info: *mut u64) -> i32 {
    if info.is_null() || nodo != 0 || grupo > 1 {
        return E_INVALIDARG;
    }
    let total = if grupo == 0 { VRAM } else { COMPARTIDA };
    // SAFETY: los cuatro UINT64 del `.exe`.
    unsafe {
        info.write_unaligned(total / 10 * 9);
        info.add(1).write_unaligned(0);
        info.add(2).write_unaligned(total / 2);
        info.add(3).write_unaligned(0);
    }
    S_OK
}

extern "win64" fn set_video_memory_reservation(_this: u64, _nodo: u32, _grupo: u32, _bytes: u64) -> i32 {
    S_OK
}

/// `Register...Event(this, evento, *galleta)`: se apunta, nunca avisa (el
/// presupuesto no cambia, nadie quita la proteccion).
extern "win64" fn registrar_aviso(_this: u64, _evento: u64, galleta: *mut u32) -> i32 {
    if galleta.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el DWORD del `.exe`.
    unsafe { galleta.write_unaligned(1) };
    S_OK
}

extern "win64" fn quitar_aviso(_this: u64, _galleta: u32) {}

// -- 01-10: la salida (el monitor) -------------------------------------------------
//
// Cyberpunk, con el dispositivo ya hecho, pregunta al adaptador "que monitores
// tienes" (EnumOutputs) y sale si no hay respuesta. La casa tiene uno: la
// pantalla de user32 (1920x1080 a 60 Hz, el mismo HMONITOR y el mismo nombre
// que GetMonitorInfoW), asi que DXGI y user32 dicen lo mismo.

use crate::user32_medidas::{HERCIOS, MONITOR, NOMBRE_PANTALLA, PANTALLA};

const DXGI_ERROR_MORE_DATA: i32 = 0x887A_0003_u32 as i32;
const DXGI_ERROR_UNSUPPORTED: i32 = 0x887A_0004_u32 as i32;
const DXGI_MODE_ROTATION_IDENTITY: u32 = 1;
const DXGI_MODE_SCALING_UNSPECIFIED: u32 = 0;
const DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE: u32 = 1;
/// DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709: SDR de toda la vida.
const COLOR_SRGB: u32 = 0;
/// Las medidas que se ofrecen, de la mayor (la pantalla) a la menor.
const MODOS: [(u32, u32); 4] = [(PANTALLA.0 as u32, PANTALLA.1 as u32), (1600, 900), (1280, 720), (1024, 768)];
/// Los formatos que la casa presenta (Present copia de estos dos).
const FORMATOS: [u32; 2] = [DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM];

pub struct Salida;

/// La salida `i` del adaptador: solo la 0.
fn salida(i: u32, pp: *mut u64) -> i32 {
    if pp.is_null() {
        return E_INVALIDARG;
    }
    if i != 0 {
        // SAFETY: un puntero del `.exe`.
        unsafe { *pp = 0 };
        return DXGI_ERROR_NOT_FOUND;
    }
    let vt = vtabla::<{ com::OUTPUT }>(&[
        (6, dir!(salida_get_parent)),
        (7, dir!(salida_get_desc)),
        (8, dir!(get_display_mode_list)),
        (9, dir!(find_closest_matching_mode)),
        (10, dir!(wait_for_vblank)),
        (11, dir!(take_ownership)),
        (12, dir!(release_ownership)),
        (18, dir!(get_frame_statistics)),
        (19, dir!(get_display_mode_list1)),
        (20, dir!(find_closest_matching_mode1)),
        (22, dir!(duplicate_output)),
        (23, dir!(supports_overlays)),
        (24, dir!(check_overlay_support)),
        (25, dir!(check_overlay_color_space_support)),
        (26, dir!(duplicate_output1)),
        (27, dir!(salida_get_desc1)),
        (28, dir!(check_hardware_composition_support)),
    ]);
    dar(pp, nuevo(com::OUTPUT, vt, Salida) as u64)
}

/// `EnumOutputs(this, i, pp)` (hueco 7 del adaptador).
extern "win64" fn enum_outputs(_this: u64, i: u32, pp: *mut u64) -> i32 {
    salida(i, pp)
}

/// `GetContainingOutput(this, pp)` de la cadena: el unico monitor.
extern "win64" fn get_containing_output(_this: u64, pp: *mut u64) -> i32 {
    salida(0, pp)
}

/// `GetParent(this, riid, pp)`: el adaptador.
extern "win64" fn salida_get_parent(_this: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    adaptador(0, riid, pp)
}

/// DXGI_OUTPUT_DESC (96 B): DeviceName (32 WCHAR) +0, DesktopCoordinates
/// (RECT) +64, AttachedToDesktop +80, Rotation +84, Monitor +88.
fn escribir_desc_salida(d: *mut u8, n: usize) {
    // SAFETY: `n` >= 96 bytes del `.exe` (quien llama lo comprobo no nulo).
    unsafe {
        core::ptr::write_bytes(d, 0, n);
        for (k, c) in NOMBRE_PANTALLA.encode_utf16().enumerate() {
            (d.add(2 * k) as *mut u16).write_unaligned(c);
        }
        (d.add(64) as *mut [i32; 4]).write_unaligned([0, 0, PANTALLA.0, PANTALLA.1]);
        (d.add(80) as *mut u32).write_unaligned(1);
        (d.add(84) as *mut u32).write_unaligned(DXGI_MODE_ROTATION_IDENTITY);
        (d.add(88) as *mut u64).write_unaligned(MONITOR);
    }
}

extern "win64" fn salida_get_desc(_this: u64, d: *mut u8) -> i32 {
    if d.is_null() {
        return E_INVALIDARG;
    }
    escribir_desc_salida(d, 96);
    S_OK
}

/// `GetDesc1` (Output6): lo de GetDesc y luego BitsPerColor +96, ColorSpace
/// +100, los primarios y el blanco (8 f32) +104, y las luminancias +136
/// +140 +144 (152 B). Un monitor SDR corriente de 8 bits y 270 nits.
extern "win64" fn salida_get_desc1(_this: u64, d: *mut u8) -> i32 {
    if d.is_null() {
        return E_INVALIDARG;
    }
    escribir_desc_salida(d, 152);
    // Rec. 709: rojo, verde, azul y el blanco D65 (x, y).
    let primarios: [f32; 8] = [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290];
    // SAFETY: 152 bytes del `.exe`.
    unsafe {
        (d.add(96) as *mut u32).write_unaligned(8);
        (d.add(100) as *mut u32).write_unaligned(COLOR_SRGB);
        (d.add(104) as *mut [f32; 8]).write_unaligned(primarios);
        (d.add(136) as *mut [f32; 3]).write_unaligned([0.5, 270.0, 270.0]);
    }
    S_OK
}

/// Un DXGI_MODE_DESC (7 u32: Width, Height, RefreshRate {n, d}, Format,
/// ScanlineOrdering, Scaling); el 1 agrega Stereo (32 B).
fn modo(ancho: u32, alto: u32, formato: u32) -> [u32; 7] {
    [ancho, alto, HERCIOS, 1, formato, DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE, DXGI_MODE_SCALING_UNSPECIFIED]
}

/// `GetDisplayModeList(this, formato, banderas, n, lista)` y el 1 (`paso`
/// 28 o 32): sin lista, cuantos; con lista, hasta `*n` y MORE_DATA si no
/// caben. Un formato que la casa no presenta tiene cero modos.
fn lista_de_modos(formato: u32, n: *mut u32, lista: *mut u8, paso: usize) -> i32 {
    if n.is_null() {
        return E_INVALIDARG;
    }
    let total = if FORMATOS.contains(&formato) { MODOS.len() as u32 } else { 0 };
    // SAFETY: `n` es del `.exe`; `lista`, si no es nula, tiene `*n` modos.
    unsafe {
        if lista.is_null() {
            n.write_unaligned(total);
            return S_OK;
        }
        let cabe = n.read_unaligned().min(total);
        for (k, &(w, h)) in MODOS.iter().take(cabe as usize).enumerate() {
            let m = lista.add(k * paso);
            core::ptr::write_bytes(m, 0, paso);
            (m as *mut [u32; 7]).write_unaligned(modo(w, h, formato));
        }
        n.write_unaligned(cabe);
        if cabe < total { DXGI_ERROR_MORE_DATA } else { S_OK }
    }
}

extern "win64" fn get_display_mode_list(_this: u64, formato: u32, _banderas: u32, n: *mut u32, lista: *mut u8) -> i32 {
    lista_de_modos(formato, n, lista, 28)
}

extern "win64" fn get_display_mode_list1(_this: u64, formato: u32, _banderas: u32, n: *mut u32, lista: *mut u8) -> i32 {
    lista_de_modos(formato, n, lista, 32)
}

/// `FindClosestMatchingMode(this, pedido, mas_cercano, dispositivo)` y el 1:
/// el modo de la lista que mas se parece en medida (sin medida, la pantalla);
/// el formato, el pedido si la casa lo presenta.
fn mas_cercano(pedido: *const u32, sale: *mut u8, paso: usize) -> i32 {
    if pedido.is_null() || sale.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un DXGI_MODE_DESC(1) del `.exe`, y donde dejar otro.
    unsafe {
        let p = (pedido as *const [u32; 7]).read_unaligned();
        let formato = if FORMATOS.contains(&p[4]) { p[4] } else { DXGI_FORMAT_R8G8B8A8_UNORM };
        let (w, h) = if p[0] == 0 || p[1] == 0 {
            MODOS[0]
        } else {
            *MODOS.iter().min_by_key(|&&(w, h)| w.abs_diff(p[0]) as u64 + h.abs_diff(p[1]) as u64).unwrap_or(&MODOS[0])
        };
        core::ptr::write_bytes(sale, 0, paso);
        (sale as *mut [u32; 7]).write_unaligned(modo(w, h, formato));
    }
    S_OK
}

extern "win64" fn find_closest_matching_mode(_this: u64, pedido: *const u32, sale: *mut u8, _dispositivo: u64) -> i32 {
    mas_cercano(pedido, sale, 28)
}

extern "win64" fn find_closest_matching_mode1(_this: u64, pedido: *const u32, sale: *mut u8, _dispositivo: u64) -> i32 {
    mas_cercano(pedido, sale, 32)
}

/// Sin reloj de barrido: se vuelve enseguida (Present ya marca el paso).
extern "win64" fn wait_for_vblank(_this: u64) -> i32 {
    S_OK
}

/// La pantalla completa exclusiva: la casa dice que si y no cambia nada.
extern "win64" fn take_ownership(_this: u64, _dispositivo: u64, _exclusiva: i32) -> i32 {
    S_OK
}

extern "win64" fn release_ownership(_this: u64) {}

/// Sin estadisticas de cuadros, como una salida en ventana.
extern "win64" fn get_frame_statistics(_this: u64, _e: *mut u8) -> i32 {
    DXGI_ERROR_UNSUPPORTED
}

/// Sin duplicar el escritorio (eso es para grabadores de pantalla).
extern "win64" fn duplicate_output(_this: u64, _dispositivo: u64, pp: *mut u64) -> i32 {
    if !pp.is_null() {
        // SAFETY: el puntero a interfaz del `.exe`.
        unsafe { pp.write_unaligned(0) };
    }
    DXGI_ERROR_UNSUPPORTED
}

extern "win64" fn duplicate_output1(_this: u64, _dispositivo: u64, _banderas: u32, _n: u32, _formatos: *const u32, pp: *mut u64) -> i32 {
    duplicate_output(0, 0, pp)
}

/// Sin planos superpuestos: FALSE.
extern "win64" fn supports_overlays(_this: u64) -> i32 {
    0
}

/// Ninguna bandera en el UINT del `.exe` (si lo hay).
fn sin_banderas(banderas: *mut u32) -> i32 {
    if banderas.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un UINT del `.exe`.
    unsafe { banderas.write_unaligned(0) };
    S_OK
}

/// `CheckOverlaySupport(this, formato, dispositivo, banderas)`: ninguna.
extern "win64" fn check_overlay_support(_this: u64, _formato: u32, _dispositivo: u64, banderas: *mut u32) -> i32 {
    sin_banderas(banderas)
}

/// `CheckOverlayColorSpaceSupport(this, formato, color, dispositivo,
/// banderas)`: ninguna.
extern "win64" fn check_overlay_color_space_support(_this: u64, _formato: u32, _color: u32, _dispositivo: u64, banderas: *mut u32) -> i32 {
    sin_banderas(banderas)
}

/// `CheckHardwareCompositionSupport(this, banderas)`: ninguna.
extern "win64" fn check_hardware_composition_support(_this: u64, banderas: *mut u32) -> i32 {
    sin_banderas(banderas)
}

extern "win64" fn get_current_back_buffer_index(this: u64) -> u32 {
    // SAFETY: `this` es una Cadena de la casa.
    unsafe { de::<Cadena>(this).actual as u32 }
}

/// `CreateDXGIFactory`: la misma fabrica (01-10).
extern "win64" fn create_dxgi_factory(riid: *const Guid, pp: *mut u64) -> i32 {
    fabrica(riid, pp)
}

/// Sin capa de depuracion de DXGI, como un Windows sin el SDK (01-10).
extern "win64" fn dxgi_get_debug_interface1(_banderas: u32, _riid: *const Guid, pp: *mut u64) -> i32 {
    if !pp.is_null() {
        // SAFETY: el puntero a interfaz del `.exe`.
        unsafe { pp.write_unaligned(0) };
    }
    E_NOINTERFACE
}

/// El `.exe` dice que sabe vivir con un adaptador que se quita: apuntado.
extern "win64" fn dxgi_declare_adapter_removal_support() -> i32 {
    S_OK
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "CreateDXGIFactory" => dir!(create_dxgi_factory),
        "DXGIGetDebugInterface1" => dir!(dxgi_get_debug_interface1),
        "DXGIDeclareAdapterRemovalSupport" => dir!(dxgi_declare_adapter_removal_support),
        "CreateDXGIFactory1" => dir!(create_dxgi_factory1),
        "CreateDXGIFactory2" => dir!(create_dxgi_factory2),
        _ => return None,
    })
}
