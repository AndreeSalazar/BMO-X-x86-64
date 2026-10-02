//! **El registro de Windows, de la casa** (tanda 11 de Cyberpunk, 30-09):
//! un arbol chico, de SOLO LECTURA, con lo que un juego y su runtime leen.
//!
//! ```text
//!    abrir y cerrar   RegOpenKeyExW/A RegCloseKey
//!    leer             RegQueryValueExW/A RegGetValueW/A
//!    recorrer         RegEnumKeyExW/A RegEnumValueW/A RegQueryInfoKeyW/A
//! ```
//!
//! Son de kernelbase (y de api-ms-win-core-localregistry): estan en la
//! cadena de kernel32, y ADVAPI32 llega a ellas por la misma.
//!
//! Lo que hay, dicho: la version de Windows (la MISMA que GetVersionEx: 10,
//! 22H2, 19045), el procesador (su nombre y su fabricante son los del CPUID
//! de verdad; UNO, como GetSystemInfo), la version de DirectX, las carpetas
//! de programas, el MachineGuid de la casa y el locale del usuario (en-US,
//! como GetLocaleInfo). Lo que no esta contesta ERROR_FILE_NOT_FOUND, como
//! Windows con una clave que no existe. Escribir no se puede todavia (nadie
//! lo importa).

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::texto;

use crate::dir;

const ERROR_SUCCESS: u32 = 0;
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_MORE_DATA: u32 = 234;
const ERROR_NO_MORE_ITEMS: u32 = 259;
const ERROR_UNSUPPORTED_TYPE: u32 = 1630;

const REG_SZ: u32 = 1;
const REG_DWORD: u32 = 4;

struct Valor {
    nombre: Vec<u16>,
    tipo: u32,
    datos: Vec<u8>,
}

struct Clave {
    /// La ruta entera, desde su raiz (`HKEY_LOCAL_MACHINE\SOFTWARE`).
    ruta: Vec<u16>,
    valores: Vec<Valor>,
}

struct Estado {
    claves: Vec<Clave>,
    /// Los HKEY abiertos: (el handle, la clave).
    abiertas: Vec<(u64, usize)>,
    siguiente: u64,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { claves: Vec::new(), abiertas: Vec::new(), siguiente: 0 }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    let e = unsafe { &mut *ESTADO.0.get() };
    if e.claves.is_empty() {
        e.claves = arbol();
    }
    e
}

pub(crate) fn reiniciar() {
    // SAFETY: ver `Global`.
    let e = unsafe { &mut *ESTADO.0.get() };
    e.claves.clear();
    e.abiertas.clear();
    e.siguiente = 0;
}

// -- El arbol ---------------------------------------------------------------------------------

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn sz(n: &str, v: &str) -> Valor {
    let datos = v.encode_utf16().chain(core::iter::once(0)).flat_map(|c| c.to_le_bytes()).collect();
    Valor { nombre: w(n), tipo: REG_SZ, datos }
}

fn dw(n: &str, v: u32) -> Valor {
    Valor { nombre: w(n), tipo: REG_DWORD, datos: v.to_le_bytes().to_vec() }
}

/// Lo que dice CPUID: el fabricante (hoja 0), la familia, el modelo y el
/// paso (hoja 1) y el nombre (0x8000_0002..4).
fn cpuid() -> (Vec<u8>, (u32, u32, u32), Vec<u8>) {
    use core::arch::x86_64::__cpuid;
    // CPUID existe en todo x86-64 (y esta version de Rust lo da seguro).
    let (h0, h1, ext) = (__cpuid(0), __cpuid(1), __cpuid(0x8000_0000).eax);
    let mut fab = Vec::new();
    for r in [h0.ebx, h0.edx, h0.ecx] {
        fab.extend_from_slice(&r.to_le_bytes());
    }
    let base = (h1.eax >> 8) & 0xF;
    let familia = if base == 0xF { base + ((h1.eax >> 20) & 0xFF) } else { base };
    let modelo = ((h1.eax >> 4) & 0xF) | if base >= 6 { ((h1.eax >> 16) & 0xF) << 4 } else { 0 };
    let mut nombre = Vec::new();
    if ext >= 0x8000_0004 {
        for hoja in 0x8000_0002..=0x8000_0004u32 {
            // La hoja existe: `ext` lo dijo.
            let r = __cpuid(hoja);
            for x in [r.eax, r.ebx, r.ecx, r.edx] {
                nombre.extend_from_slice(&x.to_le_bytes());
            }
        }
    }
    let nombre: Vec<u8> = nombre.into_iter().take_while(|&c| c != 0).collect();
    (fab, (familia, modelo, h1.eax & 0xF), nombre.trim_ascii().to_vec())
}

fn texto_de(b: &[u8]) -> alloc::string::String {
    b.iter().map(|&c| if c.is_ascii() { c as char } else { '?' }).collect()
}

const HKLM: &str = "HKEY_LOCAL_MACHINE";
const HKCU: &str = "HKEY_CURRENT_USER";

fn arbol() -> Vec<Clave> {
    let (fab, (familia, modelo, paso), nombre) = cpuid();
    let version = alloc::format!("{HKLM}\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
    let mut hojas: Vec<(alloc::string::String, Vec<Valor>)> = alloc::vec![
        (
            version,
            alloc::vec![
                sz("ProductName", "Windows 10 Pro"),
                sz("EditionID", "Professional"),
                sz("InstallationType", "Client"),
                sz("CurrentVersion", "6.3"),
                sz("CurrentBuild", "19045"),
                sz("CurrentBuildNumber", "19045"),
                dw("CurrentMajorVersionNumber", 10),
                dw("CurrentMinorVersionNumber", 0),
                sz("ReleaseId", "2009"),
                sz("DisplayVersion", "22H2"),
                dw("UBR", 0),
                sz("CurrentType", "Multiprocessor Free"),
                sz("SystemRoot", "C:\\Windows"),
            ],
        ),
        (alloc::format!("{HKLM}\\HARDWARE\\DESCRIPTION\\System\\BIOS"), Vec::new()),
        (alloc::format!("{HKLM}\\SOFTWARE\\Microsoft\\DirectX"), alloc::vec![sz("Version", "4.09.00.0904")]),
        (alloc::format!("{HKLM}\\SOFTWARE\\Microsoft\\Cryptography"), alloc::vec![sz("MachineGuid", "b0e0b0e0-2077-4bb0-9a1b-00000000b0e0")]),
        (
            alloc::format!("{HKLM}\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion"),
            alloc::vec![sz("ProgramFilesDir", "C:\\Program Files"), sz("CommonFilesDir", "C:\\Program Files\\Common Files"), sz("ProgramFilesDir (x86)", "C:\\Program Files (x86)")],
        ),
        (alloc::format!("{HKLM}\\SYSTEM\\CurrentControlSet\\Control"), Vec::new()),
        (
            alloc::format!("{HKCU}\\Control Panel\\International"),
            alloc::vec![sz("LocaleName", "en-US"), sz("Locale", "00000409"), sz("sDecimal", "."), sz("sThousand", ","), sz("sShortDate", "M/d/yyyy")],
        ),
        (alloc::format!("{HKCU}\\Software\\Microsoft"), Vec::new()),
        (alloc::format!("{HKCU}\\Environment"), Vec::new()),
        (alloc::string::String::from("HKEY_CLASSES_ROOT"), Vec::new()),
        (alloc::string::String::from("HKEY_USERS"), Vec::new()),
        (alloc::string::String::from("HKEY_CURRENT_CONFIG"), Vec::new()),
    ];
    // Una subclave por procesador logico, como Windows (02-10: era solo la
    // 0; ver `bmo_proton_x::procesadores`).
    for k in 0..bmo_proton_x::procesadores::LOGICOS {
        hojas.push((
            alloc::format!("{HKLM}\\HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\{k}"),
            alloc::vec![
                sz("ProcessorNameString", &texto_de(&nombre)),
                sz("VendorIdentifier", &texto_de(&fab)),
                sz("Identifier", &alloc::format!("{} Family {familia} Model {modelo} Stepping {paso}", if fab == b"GenuineIntel" { "Intel64" } else { "AMD64" })),
                dw("~MHz", 3700),
            ],
        ));
    }
    let mut claves: Vec<Clave> = Vec::new();
    for (ruta, valores) in hojas {
        let ruta = w(&ruta);
        // Sus antepasados primero, vacios, si no estan.
        for (i, _) in ruta.iter().enumerate().filter(|(_, &c)| c == b'\\' as u16) {
            if !claves.iter().any(|c| igual(&c.ruta, &ruta[..i])) {
                claves.push(Clave { ruta: ruta[..i].to_vec(), valores: Vec::new() });
            }
        }
        claves.push(Clave { ruta, valores });
    }
    claves
}

fn igual(a: &[u16], b: &[u16]) -> bool {
    let m = |c: u16| if (b'a' as u16..=b'z' as u16).contains(&c) { c - 32 } else { c };
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| m(x) == m(y))
}

fn mayus(v: &[u16]) -> Vec<u16> {
    v.iter().map(|&c| if (b'a' as u16..=b'z' as u16).contains(&c) { c - 32 } else { c }).collect()
}

/// Las subclaves de `i`: su nombre corto, en orden (sin mayusculas), como
/// las da Windows.
fn hijas(e: &Estado, i: usize) -> Vec<Vec<u16>> {
    let p = &e.claves[i].ruta;
    let mut v: Vec<Vec<u16>> = e
        .claves
        .iter()
        .filter(|c| c.ruta.len() > p.len() + 1 && igual(&c.ruta[..p.len()], p) && c.ruta[p.len()] == b'\\' as u16 && !c.ruta[p.len() + 1..].contains(&(b'\\' as u16)))
        .map(|c| c.ruta[p.len() + 1..].to_vec())
        .collect();
    v.sort_by_key(|n| mayus(n));
    v
}

// -- Los HKEY -------------------------------------------------------------------------------------

const RAICES: [(u64, &str); 5] = [
    (0x8000_0000, "HKEY_CLASSES_ROOT"),
    (0x8000_0001, "HKEY_CURRENT_USER"),
    (0x8000_0002, "HKEY_LOCAL_MACHINE"),
    (0x8000_0003, "HKEY_USERS"),
    (0x8000_0005, "HKEY_CURRENT_CONFIG"),
];

/// Los HKEY que da RegOpenKeyEx: un rango propio.
const HKEY: u64 = 0x5B00_0000;

fn clave_de(e: &Estado, h: u64) -> Option<usize> {
    let h = h & 0xFFFF_FFFF;
    if let Some((_, r)) = RAICES.iter().find(|(x, _)| *x == h) {
        return e.claves.iter().position(|c| igual(&c.ruta, &w(r)));
    }
    e.abiertas.iter().find(|a| a.0 == h).map(|a| a.1)
}

/// La clave `sub` debajo de `base` (sin barras de mas).
fn buscar_clave(e: &Estado, base: usize, sub: &[u16]) -> Option<usize> {
    let mut ruta = e.claves[base].ruta.clone();
    for trozo in sub.split(|&c| c == b'\\' as u16).filter(|t| !t.is_empty()) {
        ruta.push(b'\\' as u16);
        ruta.extend_from_slice(trozo);
    }
    e.claves.iter().position(|c| igual(&c.ruta, &ruta))
}

/// Una cadena W del `.exe` (NULL: vacia).
fn cadena_w(p: *const u16) -> Vec<u16> {
    if p.is_null() {
        return Vec::new();
    }
    // SAFETY: una cadena suya, terminada en 0.
    unsafe { crate::user32::utf16(p) }
}

fn cadena_a(p: *const u8) -> Vec<u16> {
    if p.is_null() {
        return Vec::new();
    }
    texto::a_ancho(&crate::crt::cadena_c(p as u64), false).unwrap_or_default()
}

fn abrir(h: u64, sub: &[u16], res: *mut u64) -> u32 {
    if res.is_null() {
        return ERROR_INVALID_PARAMETER;
    }
    let e = estado();
    let Some(base) = clave_de(e, h) else { return ERROR_INVALID_HANDLE };
    let Some(i) = buscar_clave(e, base, sub) else { return ERROR_FILE_NOT_FOUND };
    e.siguiente += 1;
    let nuevo = HKEY + 4 * e.siguiente;
    e.abiertas.push((nuevo, i));
    // SAFETY: el PHKEY del `.exe`.
    unsafe { *res = nuevo };
    ERROR_SUCCESS
}

extern "win64" fn reg_open_key_ex_w(h: u64, sub: *const u16, _o: u32, _acceso: u32, res: *mut u64) -> u32 {
    abrir(h, &cadena_w(sub), res)
}

extern "win64" fn reg_open_key_ex_a(h: u64, sub: *const u8, _o: u32, _acceso: u32, res: *mut u64) -> u32 {
    abrir(h, &cadena_a(sub), res)
}

extern "win64" fn reg_close_key(h: u64) -> u32 {
    let h = h & 0xFFFF_FFFF;
    if RAICES.iter().any(|r| r.0 == h) {
        return ERROR_SUCCESS;
    }
    let e = estado();
    match e.abiertas.iter().position(|a| a.0 == h) {
        Some(i) => {
            e.abiertas.remove(i);
            ERROR_SUCCESS
        }
        None => ERROR_INVALID_HANDLE,
    }
}

// -- Leer ---------------------------------------------------------------------------------------

/// Los datos de un valor, como los ve una funcion A: las cadenas en bytes.
fn datos_a(v: &Valor) -> Vec<u8> {
    if v.tipo != REG_SZ && v.tipo != 2 && v.tipo != 7 {
        return v.datos.clone();
    }
    let u: Vec<u16> = v.datos.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    texto::a_estrecho(&u, false).unwrap_or_default()
}

/// **Dar unos datos** como el registro: sin bufer, la medida; con bufer
/// corto, ERROR_MORE_DATA y la medida; si cabe, los datos.
fn dar(datos: &[u8], p: *mut u8, cb: *mut u32) -> u32 {
    if cb.is_null() {
        return if p.is_null() { ERROR_SUCCESS } else { ERROR_INVALID_PARAMETER };
    }
    // SAFETY: el DWORD del `.exe`.
    let hay = unsafe { *cb } as usize;
    // SAFETY: lo mismo.
    unsafe { *cb = datos.len() as u32 };
    if p.is_null() {
        return ERROR_SUCCESS;
    }
    if hay < datos.len() {
        return ERROR_MORE_DATA;
    }
    // SAFETY: `hay` bytes suyos.
    unsafe { core::ptr::copy_nonoverlapping(datos.as_ptr(), p, datos.len()) };
    ERROR_SUCCESS
}

fn poner_tipo(t: *mut u32, v: u32) {
    if !t.is_null() {
        // SAFETY: el DWORD del `.exe`.
        unsafe { *t = v };
    }
}

fn consultar(h: u64, nombre: &[u16], tipo: *mut u32, p: *mut u8, cb: *mut u32, ansi: bool) -> u32 {
    let e = estado();
    let Some(i) = clave_de(e, h) else { return ERROR_INVALID_HANDLE };
    let Some(v) = e.claves[i].valores.iter().find(|v| igual(&v.nombre, nombre)) else { return ERROR_FILE_NOT_FOUND };
    poner_tipo(tipo, v.tipo);
    let d = if ansi { datos_a(v) } else { v.datos.clone() };
    dar(&d, p, cb)
}

extern "win64" fn reg_query_value_ex_w(h: u64, n: *const u16, _r: u64, tipo: *mut u32, p: *mut u8, cb: *mut u32) -> u32 {
    consultar(h, &cadena_w(n), tipo, p, cb, false)
}

extern "win64" fn reg_query_value_ex_a(h: u64, n: *const u8, _r: u64, tipo: *mut u32, p: *mut u8, cb: *mut u32) -> u32 {
    consultar(h, &cadena_a(n), tipo, p, cb, true)
}

/// El bit RRF_RT_* de un tipo.
fn bit_de(tipo: u32) -> u32 {
    match tipo {
        0 => 0x1,
        1 => 0x2,
        2 => 0x4,
        3 => 0x8,
        4 => 0x10,
        7 => 0x20,
        11 => 0x40,
        _ => 0,
    }
}

fn get_value(h: u64, sub: &[u16], nombre: &[u16], banderas: u32, tipo: *mut u32, p: *mut u8, cb: *mut u32, ansi: bool) -> u32 {
    if banderas & 0xFFFF == 0 || (!p.is_null() && cb.is_null()) {
        return ERROR_INVALID_PARAMETER;
    }
    let e = estado();
    let Some(base) = clave_de(e, h) else { return ERROR_INVALID_HANDLE };
    let Some(i) = buscar_clave(e, base, sub) else { return ERROR_FILE_NOT_FOUND };
    let Some(v) = e.claves[i].valores.iter().find(|v| igual(&v.nombre, nombre)) else { return ERROR_FILE_NOT_FOUND };
    if bit_de(v.tipo) & banderas == 0 {
        return ERROR_UNSUPPORTED_TYPE;
    }
    poner_tipo(tipo, v.tipo);
    let d = if ansi { datos_a(v) } else { v.datos.clone() };
    dar(&d, p, cb)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn reg_get_value_w(h: u64, sub: *const u16, n: *const u16, banderas: u32, tipo: *mut u32, p: *mut u8, cb: *mut u32) -> u32 {
    get_value(h, &cadena_w(sub), &cadena_w(n), banderas, tipo, p, cb, false)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn reg_get_value_a(h: u64, sub: *const u8, n: *const u8, banderas: u32, tipo: *mut u32, p: *mut u8, cb: *mut u32) -> u32 {
    get_value(h, &cadena_a(sub), &cadena_a(n), banderas, tipo, p, cb, true)
}

// -- Recorrer -------------------------------------------------------------------------------------

/// Un nombre en un bufer de `*cch` caracteres (W o A): si cabe con su 0,
/// `*cch` = su largo sin el 0; si no, ERROR_MORE_DATA.
fn dar_nombre(n: &[u16], p: *mut u8, cch: *mut u32, ansi: bool) -> u32 {
    if p.is_null() || cch.is_null() {
        return ERROR_INVALID_PARAMETER;
    }
    // SAFETY: el DWORD del `.exe`.
    let hay = unsafe { *cch } as usize;
    let a = if ansi { texto::a_estrecho(n, false).unwrap_or_default() } else { Vec::new() };
    let largo = if ansi { a.len() } else { n.len() };
    if hay <= largo {
        return ERROR_MORE_DATA;
    }
    // SAFETY: `hay` caracteres suyos.
    unsafe {
        if ansi {
            core::ptr::copy_nonoverlapping(a.as_ptr(), p, largo);
            *p.add(largo) = 0;
        } else {
            let q = p as *mut u16;
            core::ptr::copy_nonoverlapping(n.as_ptr(), q, largo);
            *q.add(largo) = 0;
        }
        *cch = largo as u32;
    }
    ERROR_SUCCESS
}

#[allow(clippy::too_many_arguments)]
fn enum_clave(h: u64, i: u32, n: *mut u8, cch: *mut u32, clase: *mut u8, cch_clase: *mut u32, ft: *mut u64, ansi: bool) -> u32 {
    let e = estado();
    let Some(k) = clave_de(e, h) else { return ERROR_INVALID_HANDLE };
    let Some(nombre) = hijas(e, k).into_iter().nth(i as usize) else { return ERROR_NO_MORE_ITEMS };
    let r = dar_nombre(&nombre, n, cch, ansi);
    if r == ERROR_SUCCESS {
        if !clase.is_null() && !cch_clase.is_null() {
            // SAFETY: la clase del `.exe` (vacia), y su medida.
            unsafe {
                *clase = 0;
                if !ansi {
                    *clase.add(1) = 0;
                }
                *cch_clase = 0;
            }
        }
        if !ft.is_null() {
            // SAFETY: el FILETIME del `.exe`.
            unsafe { ft.write_unaligned(0) };
        }
    }
    r
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn reg_enum_key_ex_w(h: u64, i: u32, n: *mut u16, cch: *mut u32, _r: u64, clase: *mut u16, cch_clase: *mut u32, ft: *mut u64) -> u32 {
    enum_clave(h, i, n as *mut u8, cch, clase as *mut u8, cch_clase, ft, false)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn reg_enum_key_ex_a(h: u64, i: u32, n: *mut u8, cch: *mut u32, _r: u64, clase: *mut u8, cch_clase: *mut u32, ft: *mut u64) -> u32 {
    enum_clave(h, i, n, cch, clase, cch_clase, ft, true)
}

#[allow(clippy::too_many_arguments)]
fn enum_valor(h: u64, i: u32, n: *mut u8, cch: *mut u32, tipo: *mut u32, p: *mut u8, cb: *mut u32, ansi: bool) -> u32 {
    let e = estado();
    let Some(k) = clave_de(e, h) else { return ERROR_INVALID_HANDLE };
    let Some(v) = e.claves[k].valores.get(i as usize) else { return ERROR_NO_MORE_ITEMS };
    let r = dar_nombre(&v.nombre, n, cch, ansi);
    if r != ERROR_SUCCESS {
        return r;
    }
    poner_tipo(tipo, v.tipo);
    let d = if ansi { datos_a(v) } else { v.datos.clone() };
    if cb.is_null() && p.is_null() {
        return ERROR_SUCCESS;
    }
    dar(&d, p, cb)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn reg_enum_value_w(h: u64, i: u32, n: *mut u16, cch: *mut u32, _r: u64, tipo: *mut u32, p: *mut u8, cb: *mut u32) -> u32 {
    enum_valor(h, i, n as *mut u8, cch, tipo, p, cb, false)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn reg_enum_value_a(h: u64, i: u32, n: *mut u8, cch: *mut u32, _r: u64, tipo: *mut u32, p: *mut u8, cb: *mut u32) -> u32 {
    enum_valor(h, i, n, cch, tipo, p, cb, true)
}

fn poner32(p: *mut u32, v: u32) {
    if !p.is_null() {
        // SAFETY: un DWORD del `.exe`.
        unsafe { *p = v };
    }
}

#[allow(clippy::too_many_arguments)]
fn info_clave(h: u64, cch_clase: *mut u32, hijas_n: *mut u32, max_hija: *mut u32, max_clase: *mut u32, valores: *mut u32, max_nombre: *mut u32, max_dato: *mut u32, sd: *mut u32, ft: *mut u64, ansi: bool) -> u32 {
    let e = estado();
    let Some(k) = clave_de(e, h) else { return ERROR_INVALID_HANDLE };
    let hs = hijas(e, k);
    let c = &e.claves[k];
    poner32(cch_clase, 0);
    poner32(hijas_n, hs.len() as u32);
    poner32(max_hija, hs.iter().map(|n| n.len()).max().unwrap_or(0) as u32);
    poner32(max_clase, 0);
    poner32(valores, c.valores.len() as u32);
    poner32(max_nombre, c.valores.iter().map(|v| v.nombre.len()).max().unwrap_or(0) as u32);
    poner32(max_dato, c.valores.iter().map(|v| if ansi { datos_a(v).len() } else { v.datos.len() }).max().unwrap_or(0) as u32);
    poner32(sd, 0);
    if !ft.is_null() {
        // SAFETY: el FILETIME del `.exe`.
        unsafe { ft.write_unaligned(0) };
    }
    ERROR_SUCCESS
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn reg_query_info_key_w(
    h: u64,
    _clase: *mut u16,
    cch_clase: *mut u32,
    _r: u64,
    hijas_n: *mut u32,
    max_hija: *mut u32,
    max_clase: *mut u32,
    valores: *mut u32,
    max_nombre: *mut u32,
    max_dato: *mut u32,
    sd: *mut u32,
    ft: *mut u64,
) -> u32 {
    info_clave(h, cch_clase, hijas_n, max_hija, max_clase, valores, max_nombre, max_dato, sd, ft, false)
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn reg_query_info_key_a(
    h: u64,
    _clase: *mut u8,
    cch_clase: *mut u32,
    _r: u64,
    hijas_n: *mut u32,
    max_hija: *mut u32,
    max_clase: *mut u32,
    valores: *mut u32,
    max_nombre: *mut u32,
    max_dato: *mut u32,
    sd: *mut u32,
    ft: *mut u64,
) -> u32 {
    info_clave(h, cch_clase, hijas_n, max_hija, max_clase, valores, max_nombre, max_dato, sd, ft, true)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "RegOpenKeyExW" => dir!(reg_open_key_ex_w),
        "RegOpenKeyExA" => dir!(reg_open_key_ex_a),
        "RegCloseKey" => dir!(reg_close_key),
        "RegQueryValueExW" => dir!(reg_query_value_ex_w),
        "RegQueryValueExA" => dir!(reg_query_value_ex_a),
        "RegGetValueW" => dir!(reg_get_value_w),
        "RegGetValueA" => dir!(reg_get_value_a),
        "RegEnumKeyExW" => dir!(reg_enum_key_ex_w),
        "RegEnumKeyExA" => dir!(reg_enum_key_ex_a),
        "RegEnumValueW" => dir!(reg_enum_value_w),
        "RegEnumValueA" => dir!(reg_enum_value_a),
        "RegQueryInfoKeyW" => dir!(reg_query_info_key_w),
        "RegQueryInfoKeyA" => dir!(reg_query_info_key_a),
        _ => return None,
    })
}
