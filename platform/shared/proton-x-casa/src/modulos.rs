//! **Los modulos, de la casa** (P4f, 27-09): `LoadLibrary`, `GetModuleHandle`
//! y `GetProcAddress` sobre LA TABLA DE LA CASA.
//!
//! ```text
//!    LoadLibraryA/W("d3d12.dll")         un HANDLE por DLL de la casa (sus
//!    GetModuleHandleA/W/ExW("kernel32")  funciones estan siempre dentro:
//!                                        no hay nada que cargar)
//!    GetProcAddress(h, "D3D12CreateDevice")  crate::tabla, la MISMA que
//!                                        resolvio las importaciones
//!    FreeLibrary                         exito: no hay nada que soltar
//! ```
//!
//! Un juego carga asi lo opcional (D3D12 "si esta"), y la `std` de Rust sus
//! funciones de Windows 8 y 10. Un nombre sin `.dll` se busca con `.dll`, y la
//! ruta delante no cuenta, como en Windows. Lo que no es de la casa: NULL y
//! ERROR_MOD_NOT_FOUND, dicho por la consola con su nombre -- es la lista
//! de lo que falta para el siguiente juego.
//!
//! Lo que no es Windows, dicho: un ordinal en GetProcAddress (la casa solo
//! tiene nombres), una DLL de verdad del disco (no se cargan: P5 lo pedira),
//! y GetModuleHandleExW desde una DIRECCION.

use alloc::string::String;
use alloc::vec::Vec;

use bmo_proton_x::Funcion;

use crate::{aviso, dir, kernel32};

/// Las DLL de la casa, en el orden de sus HANDLE.
/// (P4f4: tambien las de la `std` de Rust -- ntdll, kernelbase, ws2_32,
/// userenv, bcryptprimitives -- que pide por GetModuleHandle + GetProcAddress.)
const DLL: [&str; 11] = ["kernel32.dll", "user32.dll", "gdi32.dll", "d3d12.dll", "dxgi.dll", "ntdll.dll", "kernelbase.dll", "ws2_32.dll", "userenv.dll", "bcryptprimitives.dll", "oleaut32.dll"];

const ERROR_MOD_NOT_FOUND: u32 = 126;
const ERROR_PROC_NOT_FOUND: u32 = 127;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_INVALID_PARAMETER: u32 = 87;
const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 4;

/// El HANDLE de la DLL `i`: un numero que no es un puntero.
const fn asa(i: usize) -> u64 {
    0x5A1D_D000_0000 + ((i as u64 + 1) << 16)
}

fn dll_de(h: u64) -> Option<&'static str> {
    DLL.iter().enumerate().find(|&(i, _)| asa(i) == h).map(|(_, d)| *d)
}

/// **Un modulo por su nombre**: una DLL de la casa, o el propio `.exe`.
fn por_nombre(n: &str) -> Option<u64> {
    let base = n.rsplit(['\\', '/']).next().unwrap_or(n);
    let con = if base.contains('.') { String::from(base) } else { alloc::format!("{base}.dll") };
    if let Some(i) = DLL.iter().position(|d| d.eq_ignore_ascii_case(&con)) {
        return Some(asa(i));
    }
    let exe = crate::proceso::nombre_exe();
    (!exe.is_empty() && exe.eq_ignore_ascii_case(&con)).then(kernel32::base_imagen)
}

/// # Safety
/// `p` es una cadena del `.exe` terminada en 0 (o NULL).
unsafe fn estrecha(p: *const u8) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let mut v = Vec::new();
    while v.len() < 32767 && *p.add(v.len()) != 0 {
        v.push(*p.add(v.len()));
    }
    String::from_utf8(v).ok()
}

/// # Safety
/// Como [`estrecha`], en UTF-16.
unsafe fn ancha(p: *const u16) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let mut v = Vec::new();
    while v.len() < 32767 && *p.add(v.len()) != 0 {
        v.push(*p.add(v.len()));
    }
    String::from_utf16(&v).ok()
}

pub(crate) fn por_nombre_w(p: *const u16) -> Option<u64> {
    // SAFETY: lo que promete el `.exe`.
    por_nombre(&unsafe { ancha(p) }?)
}

fn cargar(n: Option<String>) -> u64 {
    let Some(n) = n else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    por_nombre(&n).unwrap_or_else(|| {
        aviso(&alloc::format!("LoadLibrary(\"{n}\"): no es una DLL de la casa"));
        kernel32::poner_error(ERROR_MOD_NOT_FOUND);
        0
    })
}

extern "win64" fn load_library_a(n: *const u8) -> u64 {
    // SAFETY: lo que promete el `.exe`.
    cargar(unsafe { estrecha(n) })
}

extern "win64" fn load_library_w(n: *const u16) -> u64 {
    // SAFETY: lo que promete el `.exe`.
    cargar(unsafe { ancha(n) })
}

extern "win64" fn load_library_ex_a(n: *const u8, _fichero: u64, _banderas: u32) -> u64 {
    load_library_a(n)
}

extern "win64" fn load_library_ex_w(n: *const u16, _fichero: u64, _banderas: u32) -> u64 {
    load_library_w(n)
}

extern "win64" fn free_library(_h: u64) -> i32 {
    1
}

extern "win64" fn get_module_handle_a(n: *const u8) -> u64 {
    if n.is_null() {
        return kernel32::base_imagen();
    }
    // SAFETY: lo que promete el `.exe`.
    match unsafe { estrecha(n) }.as_deref().and_then(por_nombre) {
        Some(h) => h,
        None => {
            kernel32::poner_error(ERROR_MOD_NOT_FOUND);
            0
        }
    }
}

extern "win64" fn get_module_handle_ex_w(banderas: u32, n: *const u16, h: *mut u64) -> i32 {
    if h.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let r = if banderas & GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS != 0 {
        aviso("GetModuleHandleExW desde una direccion: la casa no sabe que modulo la tiene");
        None
    } else if n.is_null() {
        Some(kernel32::base_imagen())
    } else {
        por_nombre_w(n)
    };
    // SAFETY: un HMODULE del `.exe`.
    unsafe { *h = r.unwrap_or(0) };
    if r.is_none() {
        kernel32::poner_error(ERROR_MOD_NOT_FOUND);
    }
    r.is_some() as i32
}

extern "win64" fn get_proc_address(h: u64, n: *const u8) -> u64 {
    let Some(dll) = dll_de(h) else {
        if h == kernel32::base_imagen() {
            aviso("GetProcAddress sobre el propio .exe: sus exportaciones no se leen todavia");
            kernel32::poner_error(ERROR_PROC_NOT_FOUND);
        } else {
            kernel32::poner_error(ERROR_INVALID_HANDLE);
        }
        return 0;
    };
    if (n as u64) < 0x1_0000 {
        aviso(&alloc::format!("GetProcAddress({dll}, ordinal {}): la casa solo tiene nombres", n as u64));
        kernel32::poner_error(ERROR_PROC_NOT_FOUND);
        return 0;
    }
    // SAFETY: lo que promete el `.exe`.
    let Some(nombre) = (unsafe { estrecha(n) }) else {
        kernel32::poner_error(ERROR_PROC_NOT_FOUND);
        return 0;
    };
    crate::tabla(dll, &Funcion::Nombre(nombre)).unwrap_or_else(|| {
        // Sin aviso: preguntar "esta?" es lo normal (la `std` de Rust lo hace
        // con lo de Windows 8 y 10), y decir que no es la respuesta.
        kernel32::poner_error(ERROR_PROC_NOT_FOUND);
        0
    })
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "LoadLibraryA" => dir!(load_library_a),
        "LoadLibraryW" => dir!(load_library_w),
        "LoadLibraryExW" => dir!(load_library_ex_w),
        "LoadLibraryExA" => dir!(load_library_ex_a),
        "FreeLibrary" => dir!(free_library),
        "GetModuleHandleA" => dir!(get_module_handle_a),
        "GetModuleHandleExW" => dir!(get_module_handle_ex_w),
        "GetProcAddress" => dir!(get_proc_address),
        _ => return None,
    })
}
