//! **Lo que le faltaba a DXGI, contado contra vkd3d-proton** (tanda 47,
//! 02-10).
//!
//! El inventario de la casa contra lo que vkd3d-proton (dxgi de Proton, en
//! su `swapchain.c`) contesta: de IDXGISwapChain3 faltaban 33 huecos, de la
//! fabrica 21, del adaptador 5. Lo que un juego llama al montar y al cambiar
//! de medida, aqui; lo de las pantallas estereo, las ventanas de UWP y la
//! composicion, que un juego de escritorio no usa, sigue siendo un hueco que
//! se dice y sale.
//!
//! ```text
//!    cadena   GetDesc, GetDesc1, GetFullscreenDesc, GetHwnd, ResizeBuffers(1),
//!             ResizeTarget, Set/GetFullscreenState (en ventana: la casa no
//!             cambia el modo del monitor), Present1, GetLastPresentCount,
//!             GetFrameStatistics (DISJOINT, como en ventana), Set/Get
//!             MaximumFrameLatency y su evento (siempre encendido: la cola es
//!             sincrona), Set/GetSourceSize, rotacion, fondo, salida,
//!             CheckColorSpaceSupport y SetColorSpace1 (sRGB, el de siempre)
//!    fabrica  GetWindowAssociation, CreateSwapChain (la de DXGI 1.0, por la
//!             1.2), IsCurrent, IsWindowedStereoEnabled, GetCreationFlags,
//!             EnumAdapterByLuid, EnumWarpAdapter y CreateSoftwareAdapter (no
//!             hay), el estado de oclusion y estereo (se apunta, no avisa)
//!    adaptador CheckInterfaceSupport(IDXGIDevice): la version del driver
//! ```

use crate::com::{de, Guid, E_INVALIDARG, S_OK};
use crate::dxgi::Cadena;
use crate::{aviso, dir};

const DXGI_ERROR_INVALID_CALL: i32 = 0x887A_0001_u32 as i32;
const DXGI_ERROR_NOT_FOUND: i32 = 0x887A_0002_u32 as i32;
const DXGI_ERROR_UNSUPPORTED: i32 = 0x887A_0004_u32 as i32;
const DXGI_ERROR_FRAME_STATISTICS_DISJOINT: i32 = 0x887A_000B_u32 as i32;
/// DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709: sRGB, lo unico que la casa pinta.
const SRGB: u32 = 0;

/// Los huecos de IDXGISwapChain3 que pone esto.
pub(crate) fn cadena() -> [(usize, u64); 30] {
    [
        (10, dir!(set_fullscreen_state)),
        (11, dir!(get_fullscreen_state)),
        (12, dir!(get_desc)),
        (13, dir!(resize_buffers)),
        (14, dir!(resize_target)),
        (16, dir!(get_frame_statistics)),
        (17, dir!(get_last_present_count)),
        (18, dir!(get_desc1)),
        (19, dir!(get_fullscreen_desc)),
        (20, dir!(get_hwnd)),
        (21, dir!(get_core_window)),
        (22, dir!(present1)),
        (23, dir!(is_temporary_mono_supported)),
        (24, dir!(get_restrict_to_output)),
        (25, dir!(set_background_color)),
        (26, dir!(get_background_color)),
        (27, dir!(set_rotation)),
        (28, dir!(get_rotation)),
        (29, dir!(set_source_size)),
        (30, dir!(get_source_size)),
        (31, dir!(set_maximum_frame_latency)),
        (32, dir!(get_maximum_frame_latency)),
        (33, dir!(get_frame_latency_waitable_object)),
        (34, dir!(set_matrix_transform)),
        (35, dir!(get_matrix_transform)),
        (37, dir!(check_color_space_support)),
        (38, dir!(set_color_space1)),
        (39, dir!(resize_buffers1)),
        // IDXGIDeviceSubObject::GetDevice: el dispositivo de D3D12.
        (7, dir!(crate::com_objeto::get_device)),
        // IDXGISwapChain4 (tanda 48).
        (40, dir!(set_hdr_meta_data)),
    ]
}

/// Los huecos de IDXGIFactory6 que pone esto.
pub(crate) fn fabrica() -> [(usize, u64); 18] {
    [
        (9, dir!(get_window_association)),
        (10, dir!(create_swap_chain)),
        (11, dir!(create_software_adapter)),
        (13, dir!(is_current)),
        (14, dir!(is_windowed_stereo_enabled)),
        (16, dir!(create_swap_chain_for_core_window)),
        (18, dir!(registrar_estado_ventana)),
        (19, dir!(registrar_estado_evento)),
        (20, dir!(quitar_estado)),
        (21, dir!(registrar_estado_ventana)),
        (22, dir!(registrar_estado_evento)),
        (23, dir!(quitar_estado)),
        (24, dir!(create_swap_chain_for_composition)),
        (25, dir!(get_creation_flags)),
        (26, dir!(enum_adapter_by_luid)),
        (27, dir!(enum_warp_adapter)),
        // IDXGIFactory7 (tanda 48).
        (30, dir!(registrar_estado_evento)),
        (31, dir!(quitar_estado_hr)),
    ]
}

fn c<'a>(this: u64) -> &'a mut Cadena {
    // SAFETY: `this` es una Cadena de la casa (lo dice su vtabla).
    unsafe { de::<Cadena>(this) }
}

fn u32_en(p: *mut u8, o: usize, v: u32) {
    // SAFETY: dentro de la estructura del `.exe` que se esta llenando.
    unsafe { (p.add(o) as *mut u32).write_unaligned(v) };
}

extern "win64" fn set_fullscreen_state(this: u64, completa: i32, _salida: u64) -> i32 {
    // La casa no cambia el modo del monitor: la ventana ya ocupa la pantalla
    // de BMO-X. Se apunta para GetFullscreenState.
    c(this).completa = completa != 0;
    S_OK
}

extern "win64" fn get_fullscreen_state(this: u64, completa: *mut i32, salida: *mut u64) -> i32 {
    // SAFETY: los punteros del `.exe` que no son nulos.
    unsafe {
        if !completa.is_null() {
            completa.write_unaligned(c(this).completa as i32);
        }
        if !salida.is_null() {
            salida.write_unaligned(0);
        }
    }
    S_OK
}

/// `GetDesc(this, DXGI_SWAP_CHAIN_DESC*)` (72 B): BufferDesc (Width, Height,
/// RefreshRate, Format, ScanlineOrdering, Scaling) +0, SampleDesc +28,
/// BufferUsage +36, BufferCount +40, OutputWindow +48, Windowed +56,
/// SwapEffect +60, Flags +64.
extern "win64" fn get_desc(this: u64, d: *mut u8) -> i32 {
    if d.is_null() {
        return E_INVALIDARG;
    }
    let k = c(this);
    let u = |o: usize| u32::from_le_bytes([k.desc1[o], k.desc1[o + 1], k.desc1[o + 2], k.desc1[o + 3]]);
    // SAFETY: 72 bytes del `.exe`.
    unsafe { core::ptr::write_bytes(d, 0, 72) };
    for (o, v) in [(0, u(0)), (4, u(4)), (8, crate::user32_medidas::HERCIOS), (12, 1), (16, u(8)), (28, u(16)), (32, u(20)), (36, u(24)), (40, u(28)), (56, !k.completa as u32), (60, u(36)), (64, u(44))] {
        u32_en(d, o, v);
    }
    // SAFETY: como arriba.
    unsafe { (d.add(48) as *mut u64).write_unaligned(k.hwnd) };
    S_OK
}

extern "win64" fn get_desc1(this: u64, d: *mut u8) -> i32 {
    if d.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: 48 bytes del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(c(this).desc1.as_ptr(), d, 48) };
    S_OK
}

/// `GetFullscreenDesc(this, d)`: RefreshRate +0, ScanlineOrdering +8,
/// Scaling +12, Windowed +16.
extern "win64" fn get_fullscreen_desc(this: u64, d: *mut u8) -> i32 {
    if d.is_null() {
        return E_INVALIDARG;
    }
    for (o, v) in [(0, crate::user32_medidas::HERCIOS), (4, 1), (8, 0), (12, 0), (16, !c(this).completa as u32)] {
        u32_en(d, o, v);
    }
    S_OK
}

extern "win64" fn get_hwnd(this: u64, h: *mut u64) -> i32 {
    if h.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el HWND del `.exe`.
    unsafe { h.write_unaligned(c(this).hwnd) };
    S_OK
}

extern "win64" fn get_core_window(_this: u64, _riid: *const Guid, pp: *mut u64) -> i32 {
    if !pp.is_null() {
        // SAFETY: el `void **` del `.exe`.
        unsafe { *pp = 0 };
    }
    DXGI_ERROR_INVALID_CALL
}

/// **`ResizeBuffers(this, n, ancho, alto, formato, banderas)`**: back
/// buffers nuevos. 0 = lo que habia (el ancho y el alto 0, los de la
/// ventana); los viejos no se sueltan (los objetos de la casa no se liberan).
extern "win64" fn resize_buffers(this: u64, n: u32, ancho: u32, alto: u32, formato: u32, banderas: u32) -> i32 {
    let k = c(this);
    let u = |o: usize| u32::from_le_bytes([k.desc1[o], k.desc1[o + 1], k.desc1[o + 2], k.desc1[o + 3]]);
    let sup = crate::user32::superficie_de(k.hwnd);
    let ancho = if ancho != 0 { ancho } else { sup.as_ref().map_or(u(0), |s| s.ancho) };
    let alto = if alto != 0 { alto } else { sup.as_ref().map_or(u(4), |s| s.alto) };
    let formato = if formato != 0 { formato } else { u(8) };
    let n = if n != 0 { n } else { u(28) };
    if !matches!(formato, crate::d3d12::DXGI_FORMAT_R8G8B8A8_UNORM | crate::d3d12::DXGI_FORMAT_B8G8R8A8_UNORM) {
        aviso("ResizeBuffers: solo R8G8B8A8_UNORM y B8G8R8A8_UNORM, todavia");
        return E_INVALIDARG;
    }
    let Some(buffers) = (0..n.clamp(1, 4)).map(|_| crate::d3d12::recurso(ancho, alto, formato, true)).collect::<Option<alloc::vec::Vec<u64>>>() else {
        aviso("ResizeBuffers: no hay memoria para los back buffers: E_OUTOFMEMORY");
        return crate::com::E_OUTOFMEMORY;
    };
    k.buffers = buffers;
    k.actual = 0;
    for (o, v) in [(0, ancho), (4, alto), (8, formato), (28, n), (44, banderas)] {
        k.desc1[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    S_OK
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn resize_buffers1(this: u64, n: u32, ancho: u32, alto: u32, formato: u32, banderas: u32, _nodos: *const u32, _colas: *const u64) -> i32 {
    resize_buffers(this, n, ancho, alto, formato, banderas)
}

extern "win64" fn resize_target(_this: u64, _modo: *const u8) -> i32 {
    S_OK
}

extern "win64" fn get_frame_statistics(_this: u64, _e: *mut u8) -> i32 {
    DXGI_ERROR_FRAME_STATISTICS_DISJOINT
}

extern "win64" fn get_last_present_count(this: u64, n: *mut u32) -> i32 {
    if n.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el UINT del `.exe`.
    unsafe { n.write_unaligned(c(this).presentes) };
    S_OK
}

extern "win64" fn present1(this: u64, intervalo: u32, banderas: u32, _p: *const u8) -> i32 {
    crate::dxgi::present(this, intervalo, banderas)
}

extern "win64" fn is_temporary_mono_supported(_this: u64) -> i32 {
    0
}

extern "win64" fn get_restrict_to_output(_this: u64, salida: *mut u64) -> i32 {
    if !salida.is_null() {
        // SAFETY: el `IDXGIOutput **` del `.exe`.
        unsafe { salida.write_unaligned(0) };
    }
    S_OK
}

extern "win64" fn set_background_color(_this: u64, _color: *const f32) -> i32 {
    S_OK
}

extern "win64" fn get_background_color(_this: u64, color: *mut f32) -> i32 {
    if !color.is_null() {
        // SAFETY: un DXGI_RGBA (4 floats) del `.exe`.
        unsafe { core::ptr::write_bytes(color, 0, 4) };
    }
    S_OK
}

extern "win64" fn set_rotation(_this: u64, rotacion: u32) -> i32 {
    // DXGI_MODE_ROTATION_IDENTITY (1), o UNSPECIFIED (0).
    if rotacion <= 1 {
        S_OK
    } else {
        DXGI_ERROR_UNSUPPORTED
    }
}

extern "win64" fn get_rotation(_this: u64, rotacion: *mut u32) -> i32 {
    if rotacion.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el DXGI_MODE_ROTATION del `.exe`.
    unsafe { rotacion.write_unaligned(1) };
    S_OK
}

extern "win64" fn set_source_size(this: u64, ancho: u32, alto: u32) -> i32 {
    let k = c(this);
    let u = |o: usize| u32::from_le_bytes([k.desc1[o], k.desc1[o + 1], k.desc1[o + 2], k.desc1[o + 3]]);
    if ancho == 0 || alto == 0 || ancho > u(0) || alto > u(4) {
        return E_INVALIDARG;
    }
    k.fuente = (ancho, alto);
    S_OK
}

extern "win64" fn get_source_size(this: u64, ancho: *mut u32, alto: *mut u32) -> i32 {
    if ancho.is_null() || alto.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: dos UINT del `.exe`.
    unsafe {
        ancho.write_unaligned(c(this).fuente.0);
        alto.write_unaligned(c(this).fuente.1);
    }
    S_OK
}

extern "win64" fn set_maximum_frame_latency(this: u64, n: u32) -> i32 {
    if n == 0 || n > 16 {
        return E_INVALIDARG;
    }
    c(this).latencia = n;
    S_OK
}

extern "win64" fn get_maximum_frame_latency(this: u64, n: *mut u32) -> i32 {
    if n.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el UINT del `.exe`.
    unsafe { n.write_unaligned(c(this).latencia) };
    S_OK
}

/// El evento de la latencia: uno manual y ENCENDIDO, el mismo cada vez. La
/// cola de la casa es sincrona: cuando el juego espera un fotograma, ya esta.
extern "win64" fn get_frame_latency_waitable_object(this: u64) -> u64 {
    let k = c(this);
    if k.espera == 0 {
        k.espera = crate::hilos::evento_encendido();
    }
    k.espera
}

extern "win64" fn set_matrix_transform(_this: u64, _m: *const f32) -> i32 {
    DXGI_ERROR_INVALID_CALL
}

extern "win64" fn get_matrix_transform(_this: u64, _m: *mut f32) -> i32 {
    DXGI_ERROR_INVALID_CALL
}

/// `CheckColorSpaceSupport(this, espacio, soporte)`: sRGB se presenta; lo
/// demas (HDR10, scRGB), no.
extern "win64" fn check_color_space_support(_this: u64, espacio: u32, soporte: *mut u32) -> i32 {
    if soporte.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el UINT del `.exe`. DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT = 1.
    unsafe { soporte.write_unaligned((espacio == SRGB) as u32) };
    S_OK
}

extern "win64" fn set_color_space1(_this: u64, espacio: u32) -> i32 {
    if espacio == SRGB {
        S_OK
    } else {
        E_INVALIDARG
    }
}

// -- La fabrica -------------------------------------------------------------

extern "win64" fn get_window_association(_this: u64, h: *mut u64) -> i32 {
    if h.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el HWND del `.exe`.
    unsafe { h.write_unaligned(0) };
    S_OK
}

/// `CreateSwapChain(this, cola, DXGI_SWAP_CHAIN_DESC*, pp)`: la de DXGI 1.0,
/// pasada a un DXGI_SWAP_CHAIN_DESC1 y a CreateSwapChainForHwnd.
extern "win64" fn create_swap_chain(this: u64, cola: u64, d: *const u8, pp: *mut u64) -> i32 {
    if d.is_null() || pp.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un DXGI_SWAP_CHAIN_DESC del `.exe` (72 B; ver `get_desc`).
    let (u, hwnd) = unsafe { (|o: usize| (d.add(o) as *const u32).read_unaligned(), (d.add(48) as *const u64).read_unaligned()) };
    let mut d1 = [0u8; 48];
    for (o, v) in [(0, u(0)), (4, u(4)), (8, u(16)), (16, u(28)), (20, u(32)), (24, u(36)), (28, u(40)), (36, u(60)), (44, u(64))] {
        d1[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    crate::dxgi::create_swap_chain_for_hwnd(this, cola, hwnd, d1.as_ptr(), core::ptr::null(), 0, pp)
}

extern "win64" fn create_software_adapter(_this: u64, _modulo: u64, pp: *mut u64) -> i32 {
    if !pp.is_null() {
        // SAFETY: el `void **` del `.exe`.
        unsafe { *pp = 0 };
    }
    DXGI_ERROR_UNSUPPORTED
}

extern "win64" fn is_current(_this: u64) -> i32 {
    1
}

extern "win64" fn is_windowed_stereo_enabled(_this: u64) -> i32 {
    0
}

extern "win64" fn create_swap_chain_for_core_window(_this: u64, _cola: u64, _w: u64, _d: *const u8, _s: u64, pp: *mut u64) -> i32 {
    aviso("CreateSwapChainForCoreWindow: una ventana de UWP, no hay");
    if !pp.is_null() {
        // SAFETY: el `void **` del `.exe`.
        unsafe { *pp = 0 };
    }
    DXGI_ERROR_INVALID_CALL
}

extern "win64" fn create_swap_chain_for_composition(_this: u64, _cola: u64, _d: *const u8, _s: u64, pp: *mut u64) -> i32 {
    aviso("CreateSwapChainForComposition: la composicion de DirectComposition, no hay");
    if !pp.is_null() {
        // SAFETY: el `void **` del `.exe`.
        unsafe { *pp = 0 };
    }
    DXGI_ERROR_INVALID_CALL
}

/// RegisterStereoStatusWindow / RegisterOcclusionStatusWindow: un numero; la
/// casa no avisa nunca (ni estereo, ni ventana tapada).
extern "win64" fn registrar_estado_ventana(_this: u64, _hwnd: u64, _msg: u32, cookie: *mut u32) -> i32 {
    if cookie.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el DWORD del `.exe`.
    unsafe { cookie.write_unaligned(1) };
    S_OK
}

extern "win64" fn registrar_estado_evento(_this: u64, _evento: u64, cookie: *mut u32) -> i32 {
    if cookie.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: como arriba.
    unsafe { cookie.write_unaligned(1) };
    S_OK
}

extern "win64" fn quitar_estado(_this: u64, _cookie: u32) {}

/// UnregisterAdaptersChangedEvent: como `quitar_estado`, con su HRESULT.
extern "win64" fn quitar_estado_hr(_this: u64, _cookie: u32) -> i32 {
    S_OK
}

/// `SetHDRMetaData(this, tipo, medida, datos)`: se aceptan y no cambian nada
/// (la casa presenta sRGB: CheckColorSpaceSupport no ofrece HDR).
extern "win64" fn set_hdr_meta_data(_this: u64, tipo: u32, n: u32, datos: *const u8) -> i32 {
    if tipo != 0 && (n == 0 || datos.is_null()) {
        return E_INVALIDARG;
    }
    S_OK
}

extern "win64" fn get_creation_flags(_this: u64) -> u32 {
    0
}

/// `EnumAdapterByLuid(this, LUID, riid, pp)`: el LUID va POR VALOR (8 B, en
/// un registro). El de la 3060, o no esta.
extern "win64" fn enum_adapter_by_luid(_this: u64, luid: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    if luid == crate::dxgi::LUID {
        crate::dxgi::adaptador(0, riid, pp)
    } else {
        if !pp.is_null() {
            // SAFETY: el `void **` del `.exe`.
            unsafe { *pp = 0 };
        }
        DXGI_ERROR_NOT_FOUND
    }
}

/// `EnumWarpAdapter`: el rasterizador por software de Microsoft no esta.
extern "win64" fn enum_warp_adapter(_this: u64, _riid: *const Guid, pp: *mut u64) -> i32 {
    if !pp.is_null() {
        // SAFETY: el `void **` del `.exe`.
        unsafe { *pp = 0 };
    }
    DXGI_ERROR_NOT_FOUND
}

// -- El adaptador -----------------------------------------------------------

/// IID_IDXGIDevice {54ec77fa-1377-44e6-8c32-88fd5f44c84c}.
const IID_DXGI_DEVICE: Guid = crate::com::guid(0x54ec77fa, 0x1377, 0x44e6, [0x8c, 0x32, 0x88, 0xfd, 0x5f, 0x44, 0xc8, 0x4c]);
/// La version del UMD del driver que el propietario midio en su Windows
/// (`32.0.16.1074`, el 610.74; COMO_LE_HABLA_NVIDIA.md): lo que los motores
/// leen aqui para sus listas de drivers buenos y malos.
const VERSION_UMD: u64 = (32 << 48) | (16 << 16) | 1074;

/// `CheckInterfaceSupport(this, guid, version)`: con IDXGIDevice es la forma
/// de saber la version del driver; lo demas (D3D10), no.
pub(crate) extern "win64" fn check_interface_support(_this: u64, g: *const Guid, version: *mut u64) -> i32 {
    // SAFETY: un GUID del `.exe`, si no es nulo.
    if g.is_null() || unsafe { g.read_unaligned() } != IID_DXGI_DEVICE {
        return DXGI_ERROR_UNSUPPORTED;
    }
    if !version.is_null() {
        // SAFETY: el LARGE_INTEGER del `.exe`.
        unsafe { version.write_unaligned(VERSION_UMD) };
    }
    S_OK
}
