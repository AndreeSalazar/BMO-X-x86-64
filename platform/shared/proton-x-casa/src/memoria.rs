//! **La memoria de Windows, de la casa** (P4e, 27-09).
//!
//! ```text
//!    GetProcessHeap, HeapCreate, HeapDestroy     un monton = un PROPIETARIO
//!    HeapAlloc, HeapFree, HeapReAlloc, HeapSize,  bmo_proton_x::monton: las
//!    HeapValidate, HeapSetInformation            cabeceras DENTRO del bloque
//!    VirtualAlloc, VirtualFree, VirtualQuery,    bmo_proton_x::regiones, sobre
//!    VirtualProtect                              el mismo monton a 64 KiB
//!    GetSystemInfo, GetNativeSystemInfo          4 KiB, 64 KiB, un procesador
//! ```
//!
//! La memoria sale de ARENAS que da la plataforma (`Plataforma::memoria`: en
//! BMO-X un bloque del kernel, que da ocho por proceso y de hasta 64 MiB
//! cada uno), y se pide la primera cuando el `.exe` pide algo. Soltar
//! DEVUELVE: un CRT que hace `malloc`/`free` un millon de veces no se come
//! nada (el monton del cargador, el que solo avanza, no se toca).
//!
//! Lo que no es Windows, dicho: una region reservada ya gasta su memoria (ver
//! `regiones`); reservar en una direccion fija, ejecutar lo pedido
//! (`PAGE_EXECUTE_*`, `HEAP_CREATE_ENABLE_EXECUTE`: el W^X de la casa solo
//! sella codigo al cargar) y `PAGE_READONLY`/`PAGE_NOACCESS` que de verdad
//! protejan (se apuntan y `VirtualQuery` las dice, pero la pagina sigue RW):
//! todo contesta y lo dice por la consola.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::monton::{self, Monton, Palabras, PROPIETARIO_PROCESO, PROPIETARIO_VIRTUAL};
use bmo_proton_x::regiones::{self, Regiones, GRANO, MEM_COMMIT, MEM_DECOMMIT, MEM_PRIVATE, MEM_RELEASE, MEM_RESERVE, PAGINA};

use crate::{aviso, dir, kernel32, plataforma};

/// Lo que se pide a la plataforma de una vez: lo que el kernel da en un
/// bloque. Los bloques son lo escaso (ocho), no los bytes.
const ARENA: u64 = 64 << 20;

const HEAP_ZERO_MEMORY: u32 = 0x8;
const HEAP_REALLOC_IN_PLACE_ONLY: u32 = 0x10;
const HEAP_CREATE_ENABLE_EXECUTE: u32 = 0x4_0000;
const MEM_RESET: u32 = 0x8_0000;
const MEM_RESET_UNDO: u32 = 0x100_0000;
const MEM_TOP_DOWN: u32 = 0x10_0000;

const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_NOT_ENOUGH_MEMORY: u32 = 8;
const ERROR_BAD_LENGTH: u32 = 24;
const ERROR_INVALID_PARAMETER: u32 = 87;

struct Estado {
    monton: Monton,
    regiones: Regiones,
    /// Los montones de `HeapCreate` vivos (el del proceso no esta: siempre vive).
    creados: Vec<u16>,
    siguiente: u16,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos y ninguna funcion
// de aqui cede el turno.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { monton: Monton::nuevo(), regiones: Regiones::nuevas(), creados: Vec::new(), siguiente: 2 }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.monton = Monton::nuevo();
    e.regiones = Regiones::nuevas();
    e.creados.clear();
    e.siguiente = 2;
}

/// La memoria de verdad, para el monton.
struct Real;
impl Palabras for Real {
    fn leer(&self, d: u64) -> u64 {
        // SAFETY: el monton solo lee dentro de sus arenas, que son de este
        // proceso, R+W, y alineadas a 16.
        unsafe { (d as *const u64).read() }
    }
    fn poner(&mut self, d: u64, v: u64) {
        // SAFETY: como arriba.
        unsafe { (d as *mut u64).write(v) }
    }
}

/// **Pedir** al monton; si no cabe, una arena mas y otra vez.
fn pedir(tam: u64, alin: u64, propietario: u16) -> Option<u64> {
    let e = estado();
    if let Some(p) = e.monton.pedir(&mut Real, tam, alin, propietario) {
        return Some(p);
    }
    let hace_falta = tam.checked_add(alin + 256)?;
    if hace_falta > ARENA {
        aviso("una pedida de mas de 64 MiB de una vez: el kernel de BMO-X da bloques de hasta 64 MiB");
        return None;
    }
    let base = (plataforma().memoria)(ARENA as usize)?;
    if !e.monton.agregar(&mut Real, base, ARENA) {
        aviso("el monton de Windows no tiene sitio para otra arena");
        return None;
    }
    e.monton.pedir(&mut Real, tam, alin, propietario)
}

fn a_cero(p: u64, n: u64) {
    // SAFETY: `[p, p+n)` es de un bloque o una region recien pedidos.
    unsafe { core::ptr::write_bytes(p as *mut u8, 0, n as usize) };
}

/// Pedir del monton del proceso (para lo que la casa da y el `.exe` suelta
/// con `HeapFree`/`Free*`, como el bloque del entorno).
pub(crate) fn pedir_del_proceso(tam: u64) -> Option<u64> {
    pedir(tam, 16, PROPIETARIO_PROCESO)
}

/// `realloc` sobre el monton del proceso (P4f5): en su sitio si se puede; si
/// no, uno nuevo, la copia y el viejo suelto. `None` (y el viejo intacto) si
/// no es de ese monton o no cabe.
pub(crate) fn cambiar_del_proceso(p: u64, n: u64) -> Option<u64> {
    let antes = match estado().monton.bloque(&Real, p) {
        Some(b) if b.propietario == PROPIETARIO_PROCESO => b.pedido,
        _ => return None,
    };
    if estado().monton.cambiar_en_sitio(&mut Real, p, n) {
        return Some(p);
    }
    let q = pedir(n, 16, PROPIETARIO_PROCESO)?;
    // SAFETY: dos bloques distintos del monton.
    unsafe { core::ptr::copy_nonoverlapping(p as *const u8, q as *mut u8, antes.min(n) as usize) };
    estado().monton.soltar(&mut Real, p);
    Some(q)
}

/// Lo que se pidio para el bloque `p` del monton del proceso (`_recalloc`
/// sabe asi que parte es nueva).
pub(crate) fn medida_del_proceso(p: u64) -> Option<u64> {
    match estado().monton.bloque(&Real, p) {
        Some(b) if b.propietario == PROPIETARIO_PROCESO => Some(b.pedido),
        _ => None,
    }
}

pub(crate) fn soltar_del_proceso(p: u64) -> bool {
    let e = estado();
    matches!(e.monton.bloque(&Real, p), Some(b) if b.propietario == PROPIETARIO_PROCESO) && e.monton.soltar(&mut Real, p).is_some()
}

/// El propietario de un HANDLE de monton VIVO.
fn propietario(h: u64) -> Option<u16> {
    let d = monton::propietario_de(h)?;
    (d == PROPIETARIO_PROCESO || estado().creados.contains(&d)).then_some(d)
}

/// El bloque `p` si es del monton `d`; si no, lo dice.
fn suyo(que: &str, d: u16, p: u64) -> Option<u64> {
    match estado().monton.bloque(&Real, p) {
        Some(b) if b.propietario == d => Some(b.pedido),
        Some(_) => {
            aviso(&alloc::format!("{que}: un bloque de OTRO monton (en Windows, corromperia los dos)"));
            None
        }
        None => {
            aviso(&alloc::format!("{que}: {p:#x} no es un bloque del monton (ya suelto, o no es el principio)"));
            None
        }
    }
}

extern "win64" fn get_process_heap() -> u64 {
    monton::asa(PROPIETARIO_PROCESO)
}

extern "win64" fn heap_alloc(h: u64, banderas: u32, n: usize) -> u64 {
    let Some(d) = propietario(h) else {
        aviso("HeapAlloc con un HANDLE que no es un monton vivo");
        return 0;
    };
    let Some(p) = pedir(n as u64, 16, d) else {
        aviso(&alloc::format!("HeapAlloc: sin memoria para {n} bytes"));
        return 0;
    };
    if banderas & HEAP_ZERO_MEMORY != 0 {
        a_cero(p, n as u64);
    }
    p
}

extern "win64" fn heap_free(h: u64, _banderas: u32, p: u64) -> i32 {
    if p == 0 {
        return 1;
    }
    let Some(d) = propietario(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    if suyo("HeapFree", d, p).is_none() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    estado().monton.soltar(&mut Real, p);
    1
}

extern "win64" fn heap_realloc(h: u64, banderas: u32, p: u64, n: usize) -> u64 {
    let (Some(d), true) = (propietario(h), p != 0) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    let Some(antes) = suyo("HeapReAlloc", d, p) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    let n = n as u64;
    let q = if estado().monton.cambiar_en_sitio(&mut Real, p, n) {
        p
    } else if banderas & HEAP_REALLOC_IN_PLACE_ONLY != 0 {
        return 0;
    } else {
        let Some(q) = pedir(n, 16, d) else { return 0 };
        // SAFETY: dos bloques distintos del monton, de `antes` y `n` bytes.
        unsafe { core::ptr::copy_nonoverlapping(p as *const u8, q as *mut u8, antes.min(n) as usize) };
        estado().monton.soltar(&mut Real, p);
        q
    };
    if banderas & HEAP_ZERO_MEMORY != 0 && n > antes {
        a_cero(q + antes, n - antes);
    }
    q
}

extern "win64" fn heap_size(h: u64, _banderas: u32, p: u64) -> usize {
    match propietario(h).and_then(|d| suyo("HeapSize", d, p)) {
        Some(n) => n as usize,
        None => usize::MAX,
    }
}

extern "win64" fn heap_validate(h: u64, _banderas: u32, p: u64) -> i32 {
    let Some(d) = propietario(h) else { return 0 };
    let bien = if p == 0 { estado().monton.comprobar(&Real).is_ok() } else { matches!(estado().monton.bloque(&Real, p), Some(b) if b.propietario == d) };
    bien as i32
}

extern "win64" fn heap_create(banderas: u32, _inicial: usize, _maximo: usize) -> u64 {
    if banderas & HEAP_CREATE_ENABLE_EXECUTE != 0 {
        aviso("HeapCreate(HEAP_CREATE_ENABLE_EXECUTE): la casa no ejecuta lo que se pide en marcha (W^X)");
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let e = estado();
    if e.siguiente == PROPIETARIO_VIRTUAL {
        kernel32::poner_error(ERROR_NOT_ENOUGH_MEMORY);
        return 0;
    }
    let d = e.siguiente;
    e.siguiente += 1;
    e.creados.push(d);
    monton::asa(d)
}

extern "win64" fn heap_destroy(h: u64) -> i32 {
    let e = estado();
    let Some(i) = monton::propietario_de(h).and_then(|d| e.creados.iter().position(|&x| x == d)) else {
        // El del proceso tampoco: vive lo que el proceso.
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    let d = e.creados.swap_remove(i);
    e.monton.soltar_de(&mut Real, d);
    1
}

extern "win64" fn heap_set_information(_h: u64, _clase: u32, _info: u64, _n: usize) -> i32 {
    // La "low fragmentation heap" y compania: el monton es el que es.
    1
}

/// Las protecciones que se dan: las de leer y escribir. Las de ejecutar, no.
fn proteccion(p: u32) -> Result<u32, u32> {
    // PAGE_GUARD, NOCACHE y WRITECOMBINE modifican; aqui no cambian nada.
    match p & 0xFF {
        0x01 | 0x02 | 0x04 => Ok(p),
        0x10 | 0x20 | 0x40 | 0x80 => {
            aviso("PAGE_EXECUTE_*: la casa no ejecuta lo que se pide en marcha (W^X; el codigo se sella al cargar)");
            Err(ERROR_INVALID_PARAMETER)
        }
        _ => Err(ERROR_INVALID_PARAMETER),
    }
}

extern "win64" fn virtual_alloc(dir: u64, n: usize, tipo: u32, prot: u32) -> u64 {
    let r = (|| -> Result<u64, u32> {
        let prot = proteccion(prot)?;
        if tipo & (MEM_RESET | MEM_RESET_UNDO) != 0 {
            // "Ya no me importa lo de dentro": no hay nada que hacer.
            return estado().regiones.consultar(dir).map(|_| dir & !(PAGINA - 1)).ok_or(487);
        }
        if tipo & !(MEM_COMMIT | MEM_RESERVE | MEM_TOP_DOWN) != 0 {
            aviso("VirtualAlloc con MEM_LARGE_PAGES/PHYSICAL/WRITE_WATCH: no hay");
            return Err(ERROR_INVALID_PARAMETER);
        }
        if n == 0 {
            return Err(ERROR_INVALID_PARAMETER);
        }
        if dir == 0 {
            if tipo & (MEM_COMMIT | MEM_RESERVE) == 0 {
                return Err(ERROR_INVALID_PARAMETER);
            }
            // Sin direccion, MEM_COMMIT solo tambien reserva (como Windows).
            let (_, tam) = regiones::paginas(0, n as u64);
            let p = pedir(tam, GRANO, PROPIETARIO_VIRTUAL).ok_or(ERROR_NOT_ENOUGH_MEMORY)?;
            let hecha = tipo & MEM_COMMIT != 0;
            if hecha {
                a_cero(p, tam);
            }
            estado().regiones.nueva(p, tam, prot, hecha);
            return Ok(p);
        }
        if tipo & MEM_RESERVE != 0 {
            aviso("VirtualAlloc(MEM_RESERVE) en una direccion fija: la direccion la da quien tiene la memoria");
            return Err(487);
        }
        if tipo & MEM_COMMIT == 0 {
            return Err(ERROR_INVALID_PARAMETER);
        }
        estado().regiones.hacer(dir, n as u64, prot, a_cero).map_err(|x| x.error())
    })();
    r.unwrap_or_else(|err| {
        kernel32::poner_error(err);
        0
    })
}

extern "win64" fn virtual_free(dir: u64, n: usize, tipo: u32) -> i32 {
    let e = estado();
    let r = match tipo {
        MEM_RELEASE => e.regiones.soltar(dir, n as u64).map(|_| {
            e.monton.soltar(&mut Real, dir);
        }),
        MEM_DECOMMIT => e.regiones.deshacer(dir, n as u64),
        _ => Err(regiones::NoVirtual::Parametro),
    };
    match r {
        Ok(()) => 1,
        Err(x) => {
            kernel32::poner_error(x.error());
            0
        }
    }
}

/// **Se puede leer** `n` bytes desde `dir`: todo dentro de regiones de
/// VirtualAlloc comprometidas y no PAGE_NOACCESS (ReadProcessMemory).
pub(crate) fn legible(dir: u64, n: u64) -> bool {
    let mut d = dir;
    let fin = dir.saturating_add(n);
    while d < fin {
        let Some(c) = estado().regiones.consultar(d) else { return false };
        if c.estado != MEM_COMMIT || c.prot == 0x01 || c.tam == 0 {
            return false;
        }
        d = c.base.saturating_add(c.tam);
    }
    true
}

extern "win64" fn virtual_query(dir: u64, mbi: *mut u8, n: usize) -> usize {
    if n < 48 {
        kernel32::poner_error(ERROR_BAD_LENGTH);
        return 0;
    }
    let Some(c) = estado().regiones.consultar(dir) else {
        aviso("VirtualQuery de una direccion que no es de VirtualAlloc: la casa solo sabe de las suyas");
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    // MEMORY_BASIC_INFORMATION de x64: 48 bytes.
    let mut b = [0u8; 48];
    b[0..8].copy_from_slice(&c.base.to_le_bytes());
    b[8..16].copy_from_slice(&c.base_region.to_le_bytes());
    b[16..20].copy_from_slice(&c.prot_inicial.to_le_bytes());
    b[24..32].copy_from_slice(&c.tam.to_le_bytes());
    b[32..36].copy_from_slice(&c.estado.to_le_bytes());
    b[36..40].copy_from_slice(&c.prot.to_le_bytes());
    b[40..44].copy_from_slice(&MEM_PRIVATE.to_le_bytes());
    // SAFETY: el `.exe` da 48 bytes (comprobado arriba).
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), mbi, 48) };
    48
}

extern "win64" fn virtual_protect(dir: u64, n: usize, prot: u32, antes: *mut u32) -> i32 {
    let r = proteccion(prot).and_then(|p| {
        estado().regiones.proteger(dir, n as u64, p).map_err(|x| {
            if estado().regiones.consultar(dir).is_none() {
                aviso("VirtualProtect fuera de VirtualAlloc: la imagen del .exe se sella al cargar y no se cambia");
            }
            x.error()
        })
    });
    match r {
        Ok(a) => {
            if !antes.is_null() {
                // SAFETY: un DWORD del `.exe`.
                unsafe { *antes = a };
            }
            1
        }
        Err(err) => {
            kernel32::poner_error(err);
            0
        }
    }
}

/// `SYSTEM_INFO` de x64 (48 bytes). Un procesador: los hilos de la casa son
/// M:1, y es lo que un programa debe creer para no esperar paralelismo.
extern "win64" fn get_system_info(si: *mut u8) {
    let mut b = [0u8; 48];
    b[0..2].copy_from_slice(&9u16.to_le_bytes()); // PROCESSOR_ARCHITECTURE_AMD64
    b[4..8].copy_from_slice(&(PAGINA as u32).to_le_bytes());
    b[8..16].copy_from_slice(&0x1_0000u64.to_le_bytes());
    b[16..24].copy_from_slice(&0x7FFF_FFFE_FFFFu64.to_le_bytes());
    b[24..32].copy_from_slice(&1u64.to_le_bytes());
    b[32..36].copy_from_slice(&1u32.to_le_bytes());
    b[36..40].copy_from_slice(&8664u32.to_le_bytes()); // PROCESSOR_AMD_X8664
    b[40..44].copy_from_slice(&(GRANO as u32).to_le_bytes());
    b[44..46].copy_from_slice(&0x19u16.to_le_bytes()); // familia 19h: Zen 3
    // SAFETY: el `.exe` da un SYSTEM_INFO.
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), si, 48) };
}

// -- GlobalAlloc y LocalAlloc (tanda 7: el portapapeles los usa) ------------

/// `GlobalAlloc`/`LocalAlloc`: del monton del proceso, a cero. El HGLOBAL es
/// el puntero mismo (lo que Windows da con GMEM_FIXED); con GMEM_MOVEABLE
/// tambien: `GlobalLock` lo devuelve tal cual, y nadie puede saber que no se
/// mueve.
extern "win64" fn global_alloc(_banderas: u32, n: u64) -> u64 {
    match pedir_del_proceso(n.max(1)) {
        Some(p) => {
            a_cero(p, n.max(1));
            p
        }
        None => {
            kernel32::poner_error(8); // ERROR_NOT_ENOUGH_MEMORY
            0
        }
    }
}

/// `GlobalLock`/`LocalLock`: el puntero (0 si no es un bloque suyo).
extern "win64" fn global_lock(h: u64) -> u64 {
    if medida_del_proceso(h).is_none() {
        kernel32::poner_error(6); // ERROR_INVALID_HANDLE
        return 0;
    }
    h
}

/// `GlobalUnlock`: 0 con NO_ERROR: ya no esta bloqueado.
extern "win64" fn global_unlock(h: u64) -> i32 {
    kernel32::poner_error(if medida_del_proceso(h).is_some() { 0 } else { 6 });
    0
}

extern "win64" fn global_size(h: u64) -> u64 {
    medida_del_proceso(h).unwrap_or(0)
}

/// `GlobalFree`: 0 si salio; si no, el handle.
extern "win64" fn global_free(h: u64) -> u64 {
    if h == 0 || soltar_del_proceso(h) {
        return 0;
    }
    kernel32::poner_error(6);
    h
}

/// `GlobalReAlloc(h, n, banderas)`: lo nuevo, a cero.
extern "win64" fn global_realloc(h: u64, n: u64, _banderas: u32) -> u64 {
    let Some(antes) = medida_del_proceso(h) else {
        kernel32::poner_error(6);
        return 0;
    };
    match cambiar_del_proceso(h, n.max(1)) {
        Some(q) => {
            if n > antes {
                a_cero(q + antes, n - antes);
            }
            q
        }
        None => {
            kernel32::poner_error(8);
            0
        }
    }
}

/// `GlobalHandle(p)`: el handle de un puntero bloqueado: el mismo.
extern "win64" fn global_handle(p: u64) -> u64 {
    medida_del_proceso(p).map_or(0, |_| p)
}

extern "win64" fn global_flags(h: u64) -> u32 {
    if medida_del_proceso(h).is_some() {
        0
    } else {
        0x8000 // GMEM_INVALID_HANDLE
    }
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "GetProcessHeap" => dir!(get_process_heap),
        "HeapAlloc" => dir!(heap_alloc),
        "HeapFree" => dir!(heap_free),
        "HeapReAlloc" => dir!(heap_realloc),
        "HeapSize" => dir!(heap_size),
        "HeapValidate" => dir!(heap_validate),
        "HeapCreate" => dir!(heap_create),
        "HeapDestroy" => dir!(heap_destroy),
        "HeapSetInformation" => dir!(heap_set_information),
        "VirtualAlloc" => dir!(virtual_alloc),
        "VirtualFree" => dir!(virtual_free),
        "VirtualQuery" => dir!(virtual_query),
        "VirtualProtect" => dir!(virtual_protect),
        "GetSystemInfo" => dir!(get_system_info),
        "GetNativeSystemInfo" => dir!(get_system_info),
        "GlobalAlloc" | "LocalAlloc" => dir!(global_alloc),
        "GlobalLock" | "LocalLock" => dir!(global_lock),
        "GlobalUnlock" | "LocalUnlock" => dir!(global_unlock),
        "GlobalSize" | "LocalSize" => dir!(global_size),
        "GlobalFree" => dir!(global_free),
        "GlobalReAlloc" | "LocalReAlloc" => dir!(global_realloc),
        "GlobalHandle" | "LocalHandle" => dir!(global_handle),
        "GlobalFlags" | "LocalFlags" => dir!(global_flags),
        _ => return None,
    })
}
