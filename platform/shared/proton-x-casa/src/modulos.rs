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
/// userenv, bcryptprimitives -- que pide por GetModuleHandle + GetProcAddress;
/// y advapi32, desde la tanda 11; crypt32 y bcrypt, desde la 12; y las
/// chicas del censo, desde la 14a, al final: los HANDLE de antes no cambian.)
const DLL: [&str; 37] = [
    "kernel32.dll",
    "user32.dll",
    "gdi32.dll",
    "d3d12.dll",
    "dxgi.dll",
    "ntdll.dll",
    "kernelbase.dll",
    "ws2_32.dll",
    "userenv.dll",
    "bcryptprimitives.dll",
    "oleaut32.dll",
    "d3dcompiler_47.dll",
    "msvcp140.dll",
    "advapi32.dll",
    "crypt32.dll",
    "bcrypt.dll",
    "winmm.dll",
    "shlwapi.dll",
    "shell32.dll",
    "powrprof.dll",
    "wininet.dll",
    "normaliz.dll",
    "iphlpapi.dll",
    "mswsock.dll",
    "xinput9_1_0.dll",
    "xinput1_3.dll",
    "xinput1_4.dll",
    "rpcrt4.dll",
    "ole32.dll",
    "version.dll",
    "hid.dll",
    "setupapi.dll",
    "cfgmgr32.dll",
    "wldap32.dll",
    // 01-10: la SSPI (libcurl de Galaxy la carga por su ruta del sistema).
    "secur32.dll",
    "sspicli.dll",
    // P0.4b.8: el anfitrion de los API set del CRT (`api-ms-win-crt-*`).
    "ucrtbase.dll",
];

const ERROR_MOD_NOT_FOUND: u32 = 126;
const ERROR_PROC_NOT_FOUND: u32 = 127;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_INVALID_PARAMETER: u32 = 87;
const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 4;

// -- P5a: las DLL PROPIAS del `.exe` (las que trae un juego) ------------------

/// Una DLL propia ya cargada: su nombre de fichero (en minusculas), su BASE
/// (que es su HMODULE, como en Windows), su DllMain y lo que exporta (ya en
/// direcciones: los reenvios los resolvio quien cargo).
struct Propia {
    nombre: String,
    base: u64,
    entrada: u64,
    exps: Vec<(Option<String>, u32, u64)>,
    iniciada: bool,
}

struct Propias(core::cell::UnsafeCell<Vec<Propia>>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Propias {}
static PROPIAS: Propias = Propias(core::cell::UnsafeCell::new(Vec::new()));

fn propias() -> &'static mut Vec<Propia> {
    // SAFETY: ver `Propias`; nadie guarda la referencia de un turno a otro.
    unsafe { &mut *PROPIAS.0.get() }
}

pub(crate) fn reiniciar() {
    propias().clear();
}

/// **Una DLL propia cargada** (lo dice quien carga, despues de colocarla,
/// relocalizarla y resolver lo que ELLA pide). `exps`: (nombre, ordinal,
/// direccion). Su DllMain corre en [`iniciar_dlls`].
pub fn registrar_dll(nombre: &str, base: u64, entrada: u64, exps: Vec<(Option<String>, u32, u64)>) {
    propias().push(Propia { nombre: bmo_proton_x::dll::fichero(nombre), base, entrada, exps, iniciada: false });
}

/// Las bases de las DLL propias cargadas (P4c: donde buscar su `.pdata`).
pub(crate) fn bases() -> Vec<u64> {
    propias().iter().map(|p| p.base).collect()
}

/// Si una DLL propia ya esta cargada (por su nombre de fichero).
pub fn cargada(dll: &str) -> bool {
    let n = bmo_proton_x::dll::fichero(dll);
    propias().iter().any(|p| p.nombre == n)
}

/// Si `dll` es una de las de la casa (si no, hay que buscarla junto al `.exe`).
pub fn es_de_la_casa(dll: &str) -> bool {
    let f = bmo_proton_x::dll::fichero(dll);
    DLL.iter().any(|d| d.eq_ignore_ascii_case(&f)) || crate::es_api_set_o_crt(&f)
}

/// Lo que exporta una DLL propia (para resolver importaciones contra ella).
pub(crate) fn exportada(dll: &str, f: &Funcion) -> Option<u64> {
    let n = bmo_proton_x::dll::fichero(dll);
    let p = propias().iter().find(|p| p.nombre == n)?;
    p.exps.iter().find(|(nombre, ord, _)| match f {
        Funcion::Nombre(x) => nombre.as_deref() == Some(x.as_str()),
        Funcion::Ordinal(o) => *ord == *o as u32,
    }).map(|e| e.2)
}

/// **Los DllMain** de las DLL propias, con DLL_PROCESS_ATTACH, en el orden en
/// que se cargaron (cada una despues de las que ella usa), una vez. Antes de
/// la entrada del `.exe`, con el GS ya puesto (como el cargador de Windows).
///
/// # Safety
/// Las entradas son codigo sellado de DLL cargadas por quien llama.
pub unsafe fn iniciar_dlls() -> Result<(), String> {
    iniciar_dlls_con(|_| {})
}

/// [`iniciar_dlls`], diciendo antes de cada DllMain de quien es (P0.4b.9:
/// si uno se cae, la ultima linea dice cual).
///
/// # Safety
/// Como [`iniciar_dlls`].
pub unsafe fn iniciar_dlls_con(mut antes: impl FnMut(&str)) -> Result<(), String> {
    let mut i = 0;
    while i < propias().len() {
        let (base, entrada, nombre, hecha) = {
            let p = &propias()[i];
            (p.base, p.entrada, p.nombre.clone(), p.iniciada)
        };
        propias()[i].iniciada = true;
        if !hecha && entrada != 0 {
            antes(&nombre);
        }
        if !hecha {
            // Como Windows: sus callbacks de TLS, justo antes de su DllMain.
            crate::hilos::tls_de_dll_attach(base);
        }
        if !hecha && entrada != 0 && crate::hilos::llamar_win64(entrada, base, 1, 0) as u32 == 0 {
            return Err(alloc::format!("{nombre}: su DllMain dijo FALSE al PROCESS_ATTACH"));
        }
        i += 1;
    }
    Ok(())
}

/// **De que modulo es `dir`**: su nombre y la RVA (el `.exe` o una DLL
/// del juego: el de base mas alta que no pase de `dir`).
pub(crate) fn nombre_de(dir: u64) -> Option<(String, u64)> {
    let exe = (crate::proceso::nombre_exe(), kernel32::base_imagen());
    propias()
        .iter()
        .map(|p| (p.nombre.clone(), p.base))
        .chain(core::iter::once(exe))
        .filter(|(n, b)| *b != 0 && *b <= dir && !n.is_empty())
        .max_by_key(|(_, b)| *b)
        .map(|(n, b)| (n, dir - b))
}

fn propia_por_base(h: u64) -> Option<usize> {
    propias().iter().position(|p| p.base == h)
}

/// El HANDLE de la DLL `i`: un numero que no es un puntero.
const fn asa(i: usize) -> u64 {
    0x5A1D_D000_0000 + ((i as u64 + 1) << 16)
}

fn dll_de(h: u64) -> Option<&'static str> {
    DLL.iter().enumerate().find(|&(i, _)| asa(i) == h).map(|(_, d)| *d)
}

/// **La DLL de verdad detras de un API set** (P0.4b.8, 30-09): en Windows,
/// `LoadLibrary("api-ms-win-core-synch-l1-2-0")` no abre un fichero: devuelve
/// el modulo que lo implementa (kernelbase). El CRT de MSVC lo hace al
/// arrancar para buscar funciones con `GetProcAddress`, y el juego tambien.
fn anfitrion(dll: &str) -> Option<String> {
    let d = dll.to_ascii_lowercase();
    let r = d.strip_prefix("api-ms-win-")?;
    let host = if r.starts_with("core-") {
        "kernelbase.dll".into()
    } else if r.starts_with("crt-") {
        "ucrtbase.dll".into()
    } else if r.starts_with("security-") || r.starts_with("eventing-") {
        "advapi32.dll".into()
    } else if r.starts_with("devices-config-") {
        "cfgmgr32.dll".into()
    } else if let Some(x) = r.strip_prefix("downlevel-") {
        alloc::format!("{}.dll", x.split('-').next()?)
    } else {
        return None;
    };
    DLL.iter().any(|h| h.eq_ignore_ascii_case(&host)).then_some(host)
}

/// **Un modulo por su nombre**: una DLL de la casa, o el propio `.exe`.
fn por_nombre(n: &str) -> Option<u64> {
    let base = n.rsplit(['\\', '/']).next().unwrap_or(n);
    let con = if base.contains('.') { String::from(base) } else { alloc::format!("{base}.dll") };
    let con = anfitrion(&con).unwrap_or(con);
    if let Some(i) = DLL.iter().position(|d| d.eq_ignore_ascii_case(&con)) {
        return Some(asa(i));
    }
    if let Some(p) = propias().iter().find(|p| p.nombre.eq_ignore_ascii_case(&con)) {
        return Some(p.base);
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
        // Tanda 26: el modulo cuya imagen tiene esa direccion (el juego la
        // pide con una de sus funciones, para saber quien es). Una direccion
        // de la casa no es de ninguna imagen: ERROR_MOD_NOT_FOUND.
        crate::kernel32_procesos::imagen_con(n as u64)
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
    // P5a: una DLL propia, por nombre o por ORDINAL (las suyas si los tienen).
    if let Some(i) = propia_por_base(h) {
        let f = if (n as u64) < 0x1_0000 {
            Funcion::Ordinal(n as u64 as u16)
        } else {
            // SAFETY: lo que promete el `.exe`.
            match unsafe { estrecha(n) } {
                Some(x) => Funcion::Nombre(x),
                None => {
                    kernel32::poner_error(ERROR_PROC_NOT_FOUND);
                    return 0;
                }
            }
        };
        let nombre = propias()[i].nombre.clone();
        return exportada(&nombre, &f).unwrap_or_else(|| {
            kernel32::poner_error(ERROR_PROC_NOT_FOUND);
            0
        });
    }
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
