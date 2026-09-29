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
//!                               la bandera SOFTWARE: dibuja la CPU, y se dice
//!    IDXGISwapChain3            GetCurrentBackBufferIndex
//! ```
//!
//! Los buffers giran como en el modelo FLIP de Windows: tras `Present` el
//! actual es el siguiente (el `.exe` lleva la cuenta o pregunta a
//! `GetCurrentBackBufferIndex`).

use alloc::vec::Vec;

use crate::com::{self, dar, de, nuevo, pide, vtabla, Guid, E_INVALIDARG, E_NOINTERFACE, S_OK};
use crate::d3d12::{recurso, recurso_de, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM};
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
    hwnd: u64,
    buffers: Vec<u64>,
    actual: usize,
}

fn fabrica(riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::FACTORY) {
        aviso("CreateDXGIFactory: pide una fabrica que la casa no tiene (IDXGIFactory7+?)");
        return E_NOINTERFACE;
    }
    let vt = vtabla::<{ com::FACTORY }>(&[
        (7, dir!(enum_adapters)),
        (8, dir!(make_window_association)),
        (12, dir!(enum_adapters)),
        (15, dir!(create_swap_chain_for_hwnd)),
        (28, dir!(check_feature_support)),
        (29, dir!(enum_adapter_by_gpu_preference)),
    ]);
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
extern "win64" fn create_swap_chain_for_hwnd(_this: u64, _cola: u64, hwnd: u64, desc: *const u8, _fs: *const u8, _salida: u64, pp: *mut u64) -> i32 {
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
    let buffers = (0..n.clamp(1, 4)).map(|_| recurso(w, h, formato)).collect();
    let vt = vtabla::<{ com::SWAPCHAIN }>(&[(8, dir!(present)), (9, dir!(get_buffer)), (36, dir!(get_current_back_buffer_index))]);
    dar(pp, nuevo(com::SWAPCHAIN, vt, Cadena { hwnd, buffers, actual: 0 }) as u64)
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
extern "win64" fn present(this: u64, _intervalo: u32, _banderas: u32) -> i32 {
    let empezo = (plataforma().ahora_ns)();
    // SAFETY: `this` es una Cadena de la casa.
    let c = unsafe { de::<Cadena>(this) };
    let Some(sup) = user32::superficie_de(c.hwnd) else { return E_INVALIDARG };
    // Lo que se limpio y nadie dibujo encima, antes de mirarlo (P3b4c).
    crate::tuberia::aplicar_limpieza(c.buffers[c.actual]);
    // SAFETY: un Recurso de la casa.
    let r = unsafe { recurso_de(c.buffers[c.actual]) };
    // SAFETY: la superficie mide `stride * alto` pixeles y es de este proceso.
    let destino = unsafe { core::slice::from_raw_parts_mut(sup.pixeles, sup.stride as usize * sup.alto as usize) };
    let (w, h) = (r.ancho.min(sup.ancho) as usize, r.alto.min(sup.alto) as usize);
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
    let ahora = (plataforma().ahora_ns)();
    if let Some(linea) = registro().presente(ahora, ahora.saturating_sub(empezo)) {
        (plataforma().escribir)(linea.as_bytes());
    }
    S_OK
}

// -- P3c4: el adaptador, Factory5/6 y SwapChain3 ----------------------------

const DXGI_ERROR_NOT_FOUND: i32 = 0x887A_0002_u32 as i32;
const DXGI_FEATURE_PRESENT_ALLOW_TEARING: u32 = 0;
const DXGI_ADAPTER_FLAG_SOFTWARE: u32 = 2;

pub struct Adaptador;

/// El adaptador `i`: solo hay uno, el 0 (la CPU de la casa).
fn adaptador(i: u32, riid: *const Guid, pp: *mut u64) -> i32 {
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
    let vt = vtabla::<{ com::ADAPTER }>(&[(10, dir!(get_desc1))]);
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
/// las tres memorias (SIZE_T) +272 +280 +288, AdapterLuid +296, Flags +304.
extern "win64" fn get_desc1(_this: u64, desc: *mut u8) -> i32 {
    if desc.is_null() {
        return E_INVALIDARG;
    }
    let nombre = "PROTON-X (la CPU de BMO-X)";
    // SAFETY: 312 bytes del `.exe`.
    unsafe {
        core::ptr::write_bytes(desc, 0, 312);
        for (k, c) in nombre.encode_utf16().enumerate() {
            (desc.add(2 * k) as *mut u16).write_unaligned(c);
        }
        (desc.add(304) as *mut u32).write_unaligned(DXGI_ADAPTER_FLAG_SOFTWARE);
    }
    S_OK
}

extern "win64" fn get_current_back_buffer_index(this: u64) -> u32 {
    // SAFETY: `this` es una Cadena de la casa.
    unsafe { de::<Cadena>(this).actual as u32 }
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "CreateDXGIFactory1" => dir!(create_dxgi_factory1),
        "CreateDXGIFactory2" => dir!(create_dxgi_factory2),
        _ => return None,
    })
}
