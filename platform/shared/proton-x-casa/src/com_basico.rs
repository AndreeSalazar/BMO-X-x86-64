//! **COM lo justo** (tanda 15 de Cyberpunk, 30-09): lo de ole32 y
//! OLEAUT32 que el juego importa (DURAS del censo). No hay clases de COM que
//! crear (ni WMI, ni el portapapeles de OLE): lo que se pide, se contesta como
//! un Windows donde esa clase no esta registrada.
//!
//! ```text
//!    CoInitializeEx, CoUninitialize   por hilo: S_OK la primera vez, S_FALSE
//!                        las demas, RPC_E_CHANGED_MODE si cambia el modelo
//!    OleInitialize, OleUninitialize   lo mismo, en apartamento
//!    CoInitializeSecurity  una vez; la segunda, RPC_E_TOO_LATE
//!    CoCreateInstance(Ex)  REGDB_E_CLASSNOTREG (y el CLSID, por la consola)
//!    CoSetProxyBlanket     E_NOINTERFACE: la casa no tiene proxies
//!    PropVariantClear, VariantInit, VariantClear   soltar lo que lleven
//!    SysAllocString(Len/ByteLen), SysReAllocString, SysStringByteLen
//!                        los BSTR: la medida en bytes, 4 antes del texto
//!    RegisterDragDrop, RevokeDragDrop   sin OleInitialize, E_OUTOFMEMORY
//!                        (lo que dice Windows); con el, se acepta y nadie
//!                        arrastra nada
//!    ReleaseStgMedium    soltar lo que lleve el STGMEDIUM
//! ```
//!
//! OLEAUT32 se importa por ORDINAL: `por_ordinal` dice su nombre.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::dll_chicas::co_task_mem_free;
use crate::{aviso, dir, hilos, kernel32, memoria};

const S_OK: u32 = 0;
const S_FALSE: u32 = 1;
const E_INVALIDARG: u32 = 0x8007_0057;
const E_NOINTERFACE: u32 = 0x8000_4002;
const E_OUTOFMEMORY: u32 = 0x8007_000E;
const RPC_E_CHANGED_MODE: u32 = 0x8001_0106;
const RPC_E_TOO_LATE: u32 = 0x8001_0119;
const REGDB_E_CLASSNOTREG: u32 = 0x8004_0154;
const DRAGDROP_E_INVALIDHWND: u32 = 0x8004_0102;
const COINIT_APARTMENTTHREADED: u32 = 2;

struct Estado {
    /// (hilo, cuantas veces, en apartamento, con OLE).
    iniciados: Vec<(u32, u32, bool, bool)>,
    seguridad: bool,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { iniciados: Vec::new(), seguridad: false }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.iniciados.clear();
    e.seguridad = false;
}

// -- Iniciar COM --------------------------------------------------------------------------

fn iniciar(apartamento: bool, ole: bool) -> u32 {
    let yo = kernel32::get_current_thread_id();
    let e = estado();
    match e.iniciados.iter_mut().find(|x| x.0 == yo) {
        Some(x) if x.2 != apartamento => RPC_E_CHANGED_MODE,
        Some(x) => {
            x.1 += 1;
            x.3 |= ole;
            S_FALSE
        }
        None => {
            e.iniciados.push((yo, 1, apartamento, ole));
            S_OK
        }
    }
}

extern "win64" fn co_initialize_ex(_r: u64, banderas: u32) -> u32 {
    iniciar(banderas & COINIT_APARTMENTTHREADED != 0, false)
}

extern "win64" fn co_initialize(_r: u64) -> u32 {
    iniciar(true, false)
}

extern "win64" fn ole_initialize(_r: u64) -> u32 {
    iniciar(true, true)
}

extern "win64" fn co_uninitialize() {
    let yo = kernel32::get_current_thread_id();
    let e = estado();
    if let Some(i) = e.iniciados.iter().position(|x| x.0 == yo) {
        e.iniciados[i].1 -= 1;
        if e.iniciados[i].1 == 0 {
            e.iniciados.remove(i);
        }
    }
}

fn con_ole() -> bool {
    let yo = kernel32::get_current_thread_id();
    estado().iniciados.iter().any(|x| x.0 == yo && x.3)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn co_initialize_security(_sd: u64, _n: i32, _s: u64, _r: u64, _a: u32, _i: u32, _l: u64, _c: u32, _r3: u64) -> u32 {
    let e = estado();
    if e.seguridad {
        return RPC_E_TOO_LATE;
    }
    e.seguridad = true;
    S_OK
}

// -- Crear objetos: no hay clases -------------------------------------------------------------

fn clsid_texto(g: *const u8) -> alloc::string::String {
    if g.is_null() {
        return alloc::string::String::from("(nulo)");
    }
    // SAFETY: un GUID del `.exe`.
    let b: [u8; 16] = unsafe { core::ptr::read_unaligned(g as *const [u8; 16]) };
    alloc::format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        u16::from_le_bytes([b[4], b[5]]),
        u16::from_le_bytes([b[6], b[7]]),
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

extern "win64" fn co_create_instance(clsid: *const u8, _fuera: u64, _ctx: u32, _iid: *const u8, sale: *mut u64) -> u32 {
    if sale.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un puntero del `.exe`.
    unsafe { sale.write(0) };
    aviso(&alloc::format!("CoCreateInstance {}: la casa no tiene esa clase", clsid_texto(clsid)));
    REGDB_E_CLASSNOTREG
}

/// `CoCreateInstanceEx`: cada MULTI_QI (24 bytes: iid, puntero, hr) con su
/// error.
extern "win64" fn co_create_instance_ex(clsid: *const u8, _fuera: u64, _ctx: u32, _srv: u64, n: u32, qis: *mut u8) -> u32 {
    for k in 0..n as usize {
        // SAFETY: `n` MULTI_QI del `.exe`.
        unsafe {
            (qis.add(24 * k + 8) as *mut u64).write_unaligned(0);
            (qis.add(24 * k + 16) as *mut u32).write_unaligned(REGDB_E_CLASSNOTREG);
        }
    }
    aviso(&alloc::format!("CoCreateInstanceEx {}: la casa no tiene esa clase", clsid_texto(clsid)));
    REGDB_E_CLASSNOTREG
}

extern "win64" fn co_set_proxy_blanket(_p: u64, _a: u32, _z: u32, _n: u64, _l: u32, _i: u32, _c: u64, _b: u32) -> u32 {
    E_NOINTERFACE
}

// -- BSTR y VARIANT ---------------------------------------------------------------------------

/// Un BSTR nuevo de `bytes` bytes (con la medida delante y dos ceros
/// detras), copiando `desde` si lo hay.
fn bstr(desde: *const u8, bytes: usize) -> u64 {
    let Some(p) = memoria::pedir_del_proceso((4 + bytes + 2) as u64) else { return 0 };
    // SAFETY: un bloque recien pedido de 4 + bytes + 2; `desde`, `bytes`
    // del `.exe`.
    unsafe {
        (p as *mut u32).write_unaligned(bytes as u32);
        if desde.is_null() {
            core::ptr::write_bytes((p + 4) as *mut u8, 0, bytes);
        } else {
            core::ptr::copy_nonoverlapping(desde, (p + 4) as *mut u8, bytes);
        }
        ((p + 4 + bytes as u64) as *mut u16).write_unaligned(0);
    }
    p + 4
}

fn largo_w(p: *const u16) -> usize {
    let mut n = 0;
    // SAFETY: una cadena del `.exe`, terminada en cero.
    while n < 1 << 24 && unsafe { p.add(n).read() } != 0 {
        n += 1;
    }
    n
}

extern "win64" fn sys_alloc_string(p: *const u16) -> u64 {
    if p.is_null() {
        return 0;
    }
    bstr(p as *const u8, largo_w(p) * 2)
}

extern "win64" fn sys_alloc_string_len(p: *const u16, n: u32) -> u64 {
    bstr(p as *const u8, n as usize * 2)
}

extern "win64" fn sys_alloc_string_byte_len(p: *const u8, n: u32) -> u64 {
    bstr(p, n as usize)
}

extern "win64" fn sys_string_byte_len(b: u64) -> u32 {
    if b == 0 {
        return 0;
    }
    // SAFETY: un BSTR: la medida, 4 bytes antes.
    unsafe { ((b - 4) as *const u32).read_unaligned() }
}

fn soltar_bstr(b: u64) {
    if b != 0 {
        memoria::soltar_del_proceso(b - 4);
    }
}

/// `SysReAllocString(*bstr, nuevo)`: uno nuevo con el texto, y el viejo
/// fuera.
extern "win64" fn sys_re_alloc_string(b: *mut u64, p: *const u16) -> i32 {
    if b.is_null() {
        return 0;
    }
    let n = sys_alloc_string(p);
    // SAFETY: el BSTR* del `.exe`.
    unsafe {
        soltar_bstr(b.read());
        b.write(n);
    }
    (n != 0) as i32
}

/// Soltar una interfaz: IUnknown::Release (el tercer hueco de su vtabla).
fn soltar_interfaz(p: u64) {
    if p != 0 {
        // SAFETY: un objeto COM del `.exe`: su vtabla, y Release.
        unsafe {
            let vt = (p as *const u64).read();
            hilos::llamar_win64((vt as *const u64).add(2).read(), p, 0, 0);
        }
    }
}

extern "win64" fn variant_init(v: *mut u8) {
    if !v.is_null() {
        // SAFETY: un VARIANT del `.exe` (24 bytes).
        unsafe { core::ptr::write_bytes(v, 0, 24) };
    }
}

/// `VariantClear`: lo que lleve (BSTR, interfaz), fuera; y VT_EMPTY.
extern "win64" fn variant_clear(v: *mut u8) -> u32 {
    if v.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un VARIANT del `.exe`.
    unsafe {
        let vt = (v as *const u16).read_unaligned();
        let x = (v.add(8) as *const u64).read_unaligned();
        match vt {
            8 => soltar_bstr(x),
            9 | 13 => soltar_interfaz(x),
            _ => {}
        }
        core::ptr::write_bytes(v, 0, 24);
    }
    S_OK
}

/// `PropVariantClear`: lo que lleve (textos de CoTaskMem, BSTR, blobs,
/// interfaces), fuera; y a cero.
extern "win64" fn prop_variant_clear(v: *mut u8) -> u32 {
    if v.is_null() {
        return S_OK;
    }
    // SAFETY: un PROPVARIANT del `.exe` (24 bytes).
    unsafe {
        let vt = (v as *const u16).read_unaligned();
        let x = (v.add(8) as *const u64).read_unaligned();
        match vt {
            30 | 31 | 72 => co_task_mem_free(x),
            8 => soltar_bstr(x),
            9 | 13 => soltar_interfaz(x),
            65 => co_task_mem_free((v.add(16) as *const u64).read_unaligned()),
            _ => {}
        }
        core::ptr::write_bytes(v, 0, 24);
    }
    S_OK
}

// -- Arrastrar y soltar, STGMEDIUM --------------------------------------------------------------

extern "win64" fn register_drag_drop(h: u64, _destino: u64) -> u32 {
    if !con_ole() {
        return E_OUTOFMEMORY;
    }
    if crate::kernel32_a::w::<extern "win64" fn(u64) -> i32>("IsWindow")(h) == 0 {
        return DRAGDROP_E_INVALIDHWND;
    }
    S_OK
}

extern "win64" fn revoke_drag_drop(_h: u64) -> u32 {
    S_OK
}

/// `ReleaseStgMedium`: si trae pUnkForRelease, su Release; si no, segun su
/// TYMED: HGLOBAL (1) GlobalFree; FILE (2) el nombre, CoTaskMemFree; ISTREAM
/// (4) e ISTORAGE (8), Release.
extern "win64" fn release_stg_medium(s: *mut u8) {
    if s.is_null() {
        return;
    }
    // SAFETY: un STGMEDIUM del `.exe` (24 bytes).
    unsafe {
        let tymed = (s as *const u32).read_unaligned();
        let x = (s.add(8) as *const u64).read_unaligned();
        let quien_suelta = (s.add(16) as *const u64).read_unaligned();
        if quien_suelta != 0 {
            soltar_interfaz(quien_suelta);
        } else {
            match tymed {
                1 => {
                    memoria::soltar_del_proceso(x);
                }
                2 => co_task_mem_free(x),
                4 | 8 => soltar_interfaz(x),
                _ => {}
            }
        }
        core::ptr::write_bytes(s, 0, 24);
    }
}

/// Los ordinales de OLEAUT32 que se importan por numero, y su nombre.
pub(crate) fn por_ordinal(o: u16) -> Option<&'static str> {
    Some(match o {
        2 => "SysAllocString",
        3 => "SysReAllocString",
        4 => "SysAllocStringLen",
        6 => "SysFreeString",
        7 => "SysStringLen",
        8 => "VariantInit",
        9 => "VariantClear",
        149 => "SysStringByteLen",
        150 => "SysAllocStringByteLen",
        200 => "GetErrorInfo",
        _ => return None,
    })
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "CoInitializeEx" => dir!(co_initialize_ex),
        "CoInitialize" => dir!(co_initialize),
        "OleInitialize" => dir!(ole_initialize),
        "CoUninitialize" | "OleUninitialize" => dir!(co_uninitialize),
        "CoInitializeSecurity" => dir!(co_initialize_security),
        "CoCreateInstance" => dir!(co_create_instance),
        "CoCreateInstanceEx" => dir!(co_create_instance_ex),
        "CoSetProxyBlanket" => dir!(co_set_proxy_blanket),
        "PropVariantClear" => dir!(prop_variant_clear),
        "VariantInit" => dir!(variant_init),
        "VariantClear" => dir!(variant_clear),
        "SysAllocString" => dir!(sys_alloc_string),
        "SysAllocStringLen" => dir!(sys_alloc_string_len),
        "SysAllocStringByteLen" => dir!(sys_alloc_string_byte_len),
        "SysReAllocString" => dir!(sys_re_alloc_string),
        "SysStringByteLen" => dir!(sys_string_byte_len),
        "RegisterDragDrop" => dir!(register_drag_drop),
        "RevokeDragDrop" => dir!(revoke_drag_drop),
        "ReleaseStgMedium" => dir!(release_stg_medium),
        _ => return None,
    })
}
