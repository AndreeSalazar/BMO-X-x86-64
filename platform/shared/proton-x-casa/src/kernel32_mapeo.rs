//! **El mapeo de ficheros, los puertos de finalizacion y los Open* de
//! `kernel32.dll`, de la casa** (tanda 3 de Cyberpunk, paso 4b, 29-09).
//!
//! ```text
//!    mapeo      CreateFileMappingW/A OpenFileMappingW MapViewOfFile(Ex)
//!               UnmapViewOfFile FlushViewOfFile
//!    puertos    CreateIoCompletionPort Get/PostQueuedCompletionStatus
//!    Open*      OpenEventA/W OpenMutexW OpenSemaphoreW (y CreateSemaphoreExW)
//!    proceso    OpenProcess OpenThread ReadProcessMemory
//!    y          WaitForMultipleObjectsEx GetOverlappedResultEx CancelIoEx
//!               FormatMessageA
//! ```
//!
//! **Un mapeo escribible** tiene UNA memoria de respaldo (de `VirtualAlloc`
//! de la casa): dos vistas del mismo mapeo ven lo mismo. Una vista de solo
//! lectura de un fichero reserva y lee solo el rango pedido. Si `n == 0`,
//! Windows pide el resto entero del mapeo y aun se materializa ese rango; no
//! hay paginacion bajo demanda. El puntero del fichero no se mueve.
//!
//! **Un puerto** es una cola de paquetes y un semaforo de la casa: esperar
//! un paquete es esperar el semaforo (cediendo el turno como cualquier
//! WaitFor*).
//!
//! **Lo que no es Windows, dicho:** los nombres de objetos no se guardan
//! (un Open* por nombre dice ERROR_FILE_NOT_FOUND, y un mapeo con nombre es
//! uno nuevo); la memoria de una vista de solo lectura se deja escribir; la
//! E/S de la casa acaba al volver, asi que un fichero asociado a un puerto
//! no le manda paquetes (solo PostQueuedCompletionStatus); y
//! OpenProcess/OpenThread/ReadProcessMemory son solo del propio proceso.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cell::UnsafeCell;


use crate::kernel32_a::w;
use crate::{dir, hilos, kernel32, memoria};

const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_NOT_ENOUGH_MEMORY: u32 = 8;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const WAIT_TIMEOUT: u32 = 258;
const ERROR_PARTIAL_COPY: u32 = 299;
const ERROR_FILE_INVALID: u32 = 1006;
const ERROR_MAPPED_ALIGNMENT: u32 = 1132;
const ERROR_NOT_FOUND: u32 = 1168;
const INVALID_HANDLE_VALUE: u64 = u64::MAX;
/// El pseudo-handle del proceso y del hilo actual.
const ESTE_PROCESO: u64 = u64::MAX;
const ESTE_HILO: u64 = u64::MAX - 1;
/// Las protecciones y los accesos que escriben.
const PAGE_READWRITE: u32 = 0x04;
const PAGE_WRITECOPY: u32 = 0x08;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;
const FILE_MAP_WRITE: u32 = 0x02;
/// La granularidad de Windows: una vista empieza en un multiplo de 64 KiB.
const GRANO: u64 = 0x1_0000;
const PAGINA: u64 = 0x1000;
const MEM_COMMIT_RESERVE: u32 = 0x3000;
const MEM_RELEASE: u32 = 0x8000;
const FILE_BEGIN: u32 = 0;
const FILE_CURRENT: u32 = 1;
const FORMAT_MESSAGE_ALLOCATE_BUFFER: u32 = 0x100;

/// Los handles de mapeos y puertos: `SUYO + indice`.
const SUYO: u64 = 0x5D00_0000;

enum Cosa {
    Mapeo(Mapeo),
    Puerto(Puerto),
    Cerrada,
}

struct Mapeo {
    /// El fichero, o INVALID_HANDLE_VALUE (del fichero de paginacion).
    fichero: u64,
    tam: u64,
    escribe: bool,
    /// La memoria de respaldo (0 hasta la primera vista de un fichero).
    memoria: u64,
    reservado: u64,
    vistas: u32,
    cerrado: bool,
}

struct Puerto {
    paquetes: VecDeque<(u32, u64, u64)>,
    semaforo: u64,
}

struct Vista {
    dir: u64,
    largo: u64,
    mapeo: usize,
    escribe: bool,
    /// Una vista de solo lectura de un fichero tiene su propio rango en RAM;
    /// no obliga a materializar el resto del `.archive`.
    memoria: u64,
    propia: bool,
}

struct Estado {
    cosas: Vec<Cosa>,
    vistas: Vec<Vista>,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos y nadie cede el
// turno con una referencia viva (ver `con`).
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { cosas: Vec::new(), vistas: Vec::new() }));

/// Lo del paso 4b, un momento: `f` no cede ni llama al `.exe`.
fn con<R>(f: impl FnOnce(&mut Estado) -> R) -> R {
    // SAFETY: ver `Global`.
    f(unsafe { &mut *ESTADO.0.get() })
}

pub(crate) fn reiniciar() {
    con(|e| {
        e.cosas.clear();
        e.vistas.clear();
    });
}

fn nueva(c: Cosa) -> u64 {
    con(|e| {
        e.cosas.push(c);
        SUYO + (e.cosas.len() - 1) as u64
    })
}

fn indice(h: u64) -> Option<usize> {
    let i = h.checked_sub(SUYO)? as usize;
    con(|e| (i < e.cosas.len() && !matches!(e.cosas[i], Cosa::Cerrada)).then_some(i))
}

/// Si `h` es un mapeo o un puerto de la casa (para CloseHandle).
pub(crate) fn es_suyo(h: u64) -> bool {
    indice(h).is_some()
}

// -- Llamar a la casa -------------------------------------------------------------------

type Alloc = extern "win64" fn(u64, u64, u32, u32) -> u64;
type Free = extern "win64" fn(u64, u64, u32) -> i32;
type Puntero = extern "win64" fn(u64, i64, *mut i64, u32) -> i32;
type Medida = extern "win64" fn(u64, *mut i64) -> i32;
type Leer = extern "win64" fn(u64, u64, u32, *mut u32, u64) -> i32;
type Soltar = extern "win64" fn(u64, i32, *mut i32) -> i32;
type Semaforo = extern "win64" fn(u64, i32, i32, u64) -> u64;
type Esperar = extern "win64" fn(u64, u32) -> u32;

/// Leer (o escribir) `n` bytes del fichero en `desde`, sin mover su puntero.
fn en_el_fichero(f: u64, desde: u64, buf: u64, n: u64, escribir: bool) -> bool {
    let mover = w::<Puntero>("SetFilePointerEx");
    let mut antes = 0i64;
    if mover(f, 0, &mut antes, FILE_CURRENT) == 0 || mover(f, desde as i64, core::ptr::null_mut(), FILE_BEGIN) == 0 {
        return false;
    }
    let es = w::<Leer>(if escribir { "WriteFile" } else { "ReadFile" });
    let mut hecho = 0u64;
    let mut bien = true;
    while hecho < n {
        let trozo = (n - hecho).min(1 << 30) as u32;
        let mut k = 0u32;
        if es(f, buf + hecho, trozo, &mut k, 0) == 0 {
            bien = false;
            break;
        }
        if k == 0 {
            bien = false;
            break;
        }
        hecho += k as u64;
    }
    mover(f, antes, core::ptr::null_mut(), FILE_BEGIN);
    bien && hecho == n
}

/// La longitud de una vista valida de `tam` bytes. `n == 0` significa desde
/// `desde` hasta el final, como en MapViewOfFile; no convierte la vista en
/// una copia de otro rango.
fn largo_vista(tam: u64, desde: u64, n: usize) -> Option<u64> {
    if desde >= tam {
        return None;
    }
    let largo = if n == 0 { tam - desde } else { n as u64 };
    (largo != 0 && desde.checked_add(largo).is_some_and(|fin| fin <= tam)).then_some(largo)
}

// -- El mapeo ---------------------------------------------------------------------------

extern "win64" fn create_file_mapping_w(f: u64, _attr: u64, prot: u32, alto: u32, bajo: u32, _nombre: *const u16) -> u64 {
    let escribe = matches!(prot & 0xFF, PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE);
    let mut tam = (alto as u64) << 32 | bajo as u64;
    if f == INVALID_HANDLE_VALUE {
        if tam == 0 {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            return 0;
        }
        let reservado = tam.div_ceil(PAGINA) * PAGINA;
        let m = w::<Alloc>("VirtualAlloc")(0, reservado, MEM_COMMIT_RESERVE, PAGE_READWRITE);
        if m == 0 {
            kernel32::poner_error(ERROR_NOT_ENOUGH_MEMORY);
            return 0;
        }
        return nueva(Cosa::Mapeo(Mapeo { fichero: f, tam, escribe: true, memoria: m, reservado, vistas: 0, cerrado: false }));
    }
    if !crate::ficheros::es_fichero(f) {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    }
    let mut medida = 0i64;
    if w::<Medida>("GetFileSizeEx")(f, &mut medida) == 0 {
        return 0;
    }
    if tam == 0 {
        tam = medida as u64;
    }
    if tam == 0 {
        kernel32::poner_error(ERROR_FILE_INVALID);
        return 0;
    }
    nueva(Cosa::Mapeo(Mapeo { fichero: f, tam, escribe, memoria: 0, reservado: 0, vistas: 0, cerrado: false }))
}

extern "win64" fn create_file_mapping_a(f: u64, attr: u64, prot: u32, alto: u32, bajo: u32, _nombre: *const u8) -> u64 {
    create_file_mapping_w(f, attr, prot, alto, bajo, core::ptr::null())
}

/// Los Open* por nombre: la casa no guarda nombres.
extern "win64" fn open_por_nombre(_acceso: u32, _heredar: i32, _nombre: u64) -> u64 {
    kernel32::poner_error(ERROR_FILE_NOT_FOUND);
    0
}

extern "win64" fn map_view_of_file_ex(h: u64, acceso: u32, alto: u32, bajo: u32, n: usize, _en: u64) -> u64 {
    let desde = (alto as u64) << 32 | bajo as u64;
    let Some(i) = indice(h) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    if desde % GRANO != 0 {
        kernel32::poner_error(ERROR_MAPPED_ALIGNMENT);
        return 0;
    }
    let Some((fichero, tam, memoria, escribe)) = con(|e| match &e.cosas[i] {
        Cosa::Mapeo(m) => Some((m.fichero, m.tam, m.memoria, m.escribe)),
        _ => None,
    }) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    let Some(largo) = largo_vista(tam, desde, n) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    let (memoria, propia) = if memoria != 0 {
        // Los mapeos anonimos y escribibles conservan una sola memoria de
        // respaldo para que sus vistas sigan compartiendo los mismos bytes.
        (memoria + desde, false)
    } else if fichero == INVALID_HANDLE_VALUE {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    } else if !escribe {
        // `.archive` y otros datos grandes: solo se trae el rango pedido. Una
        // vista de longitud 0 sigue siendo el resto del fichero por contrato.
        let reservado_vista = largo.div_ceil(PAGINA) * PAGINA;
        let base = w::<Alloc>("VirtualAlloc")(0, reservado_vista, MEM_COMMIT_RESERVE, PAGE_READWRITE);
        if base == 0 {
            kernel32::poner_error(ERROR_NOT_ENOUGH_MEMORY);
            return 0;
        }
        if !en_el_fichero(fichero, desde, base, largo, false) {
            w::<Free>("VirtualFree")(base, 0, MEM_RELEASE);
            kernel32::poner_error(ERROR_FILE_INVALID);
            return 0;
        }
        (base, true)
    } else {
        // Un mapeo escribible comparte una sola memoria para conservar la
        // coherencia de todas sus vistas; la escritura vuelve al cerrar.
        let reservado_mapeo = tam.div_ceil(PAGINA) * PAGINA;
        let base = w::<Alloc>("VirtualAlloc")(0, reservado_mapeo, MEM_COMMIT_RESERVE, PAGE_READWRITE);
        if base == 0 {
            kernel32::poner_error(ERROR_NOT_ENOUGH_MEMORY);
            return 0;
        }
        if !en_el_fichero(fichero, 0, base, tam, false) {
            w::<Free>("VirtualFree")(base, 0, MEM_RELEASE);
            return 0;
        }
        con(|e| {
            if let Cosa::Mapeo(x) = &mut e.cosas[i] {
                x.memoria = base;
                x.reservado = reservado_mapeo;
            }
        });
        (base + desde, false)
    };
    let dir = memoria;
    con(|e| {
        if let Cosa::Mapeo(x) = &mut e.cosas[i] {
            x.vistas += 1;
            e.vistas.push(Vista {
                dir,
                largo,
                mapeo: i,
                escribe: acceso & FILE_MAP_WRITE != 0 && x.escribe,
                memoria: if propia { memoria } else { 0 },
                propia,
            });
        }
    });
    dir
}

extern "win64" fn map_view_of_file(h: u64, acceso: u32, alto: u32, bajo: u32, n: usize) -> u64 {
    map_view_of_file_ex(h, acceso, alto, bajo, n, 0)
}

/// Escribir de vuelta el mapeo `i` si es de un fichero.
fn volcar(i: usize) -> bool {
    let Some((f, tam, m)) = con(|e| match &e.cosas[i] {
        Cosa::Mapeo(x) if x.fichero != INVALID_HANDLE_VALUE && x.memoria != 0 => Some((x.fichero, x.tam, x.memoria)),
        _ => None,
    }) else {
        return true;
    };
    en_el_fichero(f, 0, m, tam, true)
}

/// Soltar la memoria del mapeo `i` si ya no tiene handle ni vistas.
fn quiza_soltar(i: usize) {
    let m = con(|e| match &mut e.cosas[i] {
        Cosa::Mapeo(x) if x.cerrado && x.vistas == 0 => {
            let m = x.memoria;
            e.cosas[i] = Cosa::Cerrada;
            Some(m)
        }
        _ => None,
    });
    if let Some(m) = m.filter(|&m| m != 0) {
        w::<Free>("VirtualFree")(m, 0, MEM_RELEASE);
    }
}

extern "win64" fn unmap_view_of_file(dir: u64) -> i32 {
    let Some(v) = con(|e| {
        let k = e.vistas.iter().position(|v| v.dir == dir)?;
        Some(e.vistas.remove(k))
    }) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    if v.escribe {
        volcar(v.mapeo);
    }
    if v.propia {
        w::<Free>("VirtualFree")(v.memoria, 0, MEM_RELEASE);
    }
    con(|e| {
        if let Cosa::Mapeo(x) = &mut e.cosas[v.mapeo] {
            x.vistas -= 1;
        }
    });
    quiza_soltar(v.mapeo);
    1
}

extern "win64" fn flush_view_of_file(dir: u64, _n: usize) -> i32 {
    // Una vista de solo lectura no tiene nada que volcar. Las escribibles
    // mantienen la memoria compartida completa del mapeo.
    let Some((i, escribe)) = con(|e| {
        e.vistas
            .iter()
            .find(|v| dir >= v.dir && dir < v.dir.saturating_add(v.largo))
            .map(|v| (v.mapeo, v.escribe))
    }) else {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    };
    if escribe { volcar(i) as i32 } else { 1 }
}

/// **CloseHandle** de un mapeo o un puerto de la casa.
pub(crate) fn cerrar(h: u64) -> i32 {
    let Some(i) = indice(h) else { return 0 };
    let sem = con(|e| match &mut e.cosas[i] {
        Cosa::Mapeo(x) => {
            x.cerrado = true;
            None
        }
        Cosa::Puerto(p) => {
            let s = p.semaforo;
            e.cosas[i] = Cosa::Cerrada;
            Some(s)
        }
        Cosa::Cerrada => None,
    });
    match sem {
        Some(s) => hilos::close_handle(s),
        None => {
            quiza_soltar(i);
            1
        }
    }
}

// -- Los puertos de finalizacion ----------------------------------------------------------

/// `CreateIoCompletionPort`: con INVALID_HANDLE_VALUE y sin puerto, uno
/// nuevo; con un fichero, se asocia (la casa no le manda paquetes: su E/S
/// acaba al volver) y se devuelve el puerto.
extern "win64" fn create_io_completion_port(f: u64, puerto: u64, _clave: u64, _hilos: u32) -> u64 {
    if f != INVALID_HANDLE_VALUE {
        if puerto == 0 || indice(puerto).is_none() {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            return 0;
        }
        return puerto;
    }
    if puerto != 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let semaforo = w::<Semaforo>("CreateSemaphoreW")(0, 0, i32::MAX, 0);
    nueva(Cosa::Puerto(Puerto { paquetes: VecDeque::new(), semaforo }))
}

fn semaforo_de(p: u64) -> Option<(usize, u64)> {
    let i = indice(p)?;
    con(|e| match &e.cosas[i] {
        Cosa::Puerto(x) => Some((i, x.semaforo)),
        _ => None,
    })
}

extern "win64" fn post_queued_completion_status(p: u64, n: u32, clave: u64, ov: u64) -> i32 {
    let Some((i, s)) = semaforo_de(p) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    con(|e| {
        if let Cosa::Puerto(x) = &mut e.cosas[i] {
            x.paquetes.push_back((n, clave, ov));
        }
    });
    w::<Soltar>("ReleaseSemaphore")(s, 1, core::ptr::null_mut())
}

extern "win64" fn get_queued_completion_status(p: u64, n: *mut u32, clave: *mut u64, ov: *mut u64, ms: u32) -> i32 {
    let Some((i, s)) = semaforo_de(p) else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    // SAFETY: los punteros del `.exe` (el del OVERLAPPED es obligatorio).
    let poner = |a: u32, b: u64, c: u64| unsafe {
        if !n.is_null() {
            *n = a;
        }
        if !clave.is_null() {
            *clave = b;
        }
        if !ov.is_null() {
            *ov = c;
        }
    };
    if w::<Esperar>("WaitForSingleObject")(s, ms) != 0 {
        poner(0, 0, 0);
        kernel32::poner_error(WAIT_TIMEOUT);
        return 0;
    }
    let Some((a, b, c)) = con(|e| match &mut e.cosas[i] {
        Cosa::Puerto(x) => x.paquetes.pop_front(),
        _ => None,
    }) else {
        poner(0, 0, 0);
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    };
    poner(a, b, c);
    1
}

// -- Lo demas -----------------------------------------------------------------------------

extern "win64" fn create_semaphore_ex_w(attr: u64, inicial: i32, max: i32, nombre: u64, _banderas: u32, _acceso: u32) -> u64 {
    w::<Semaforo>("CreateSemaphoreW")(attr, inicial, max, nombre)
}

extern "win64" fn open_process(_acceso: u32, _heredar: i32, pid: u32) -> u64 {
    if pid == w::<extern "win64" fn() -> u32>("GetCurrentProcessId")() {
        return ESTE_PROCESO;
    }
    kernel32::poner_error(ERROR_INVALID_PARAMETER);
    0
}

extern "win64" fn open_thread(_acceso: u32, _heredar: i32, tid: u32) -> u64 {
    if tid == w::<extern "win64" fn() -> u32>("GetCurrentThreadId")() {
        return ESTE_HILO;
    }
    kernel32::poner_error(ERROR_INVALID_PARAMETER);
    0
}

/// `ReadProcessMemory` del propio proceso: de una region de VirtualAlloc, de
/// la pila de este hilo o de una imagen.
extern "win64" fn read_process_memory(p: u64, desde: u64, buf: u64, n: usize, leidos: *mut usize) -> i32 {
    let propio = p == ESTE_PROCESO || p == w::<extern "win64" fn() -> u64>("GetCurrentProcess")();
    if !propio {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    }
    let (m, _) = crate::excepciones::Viva::de_ahora();
    let se_puede = m.deja(desde, n as u64) || memoria::legible(desde, n as u64);
    if !se_puede {
        if !leidos.is_null() {
            // SAFETY: un SIZE_T del `.exe`.
            unsafe { *leidos = 0 };
        }
        kernel32::poner_error(ERROR_PARTIAL_COPY);
        return 0;
    }
    // SAFETY: `desde..desde+n` es memoria legible de este proceso (arriba) y
    // `buf` la del `.exe`.
    unsafe {
        core::ptr::copy(desde as *const u8, buf as *mut u8, n);
        if !leidos.is_null() {
            *leidos = n;
        }
    }
    1
}

extern "win64" fn wait_for_multiple_objects_ex(n: u32, hs: *const u64, todos: i32, ms: u32, alertable: i32) -> u32 {
    if alertable == 0 {
        return hilos::wait_for_multiple_objects(n, hs, todos, ms);
    }
    crate::kernel32_procesos::espera_alertable(ms, |t| hilos::wait_for_multiple_objects(n, hs, todos, t))
}

extern "win64" fn get_overlapped_result_ex(h: u64, ov: u64, n: *mut u32, _ms: u32, _alertable: i32) -> i32 {
    w::<extern "win64" fn(u64, u64, *mut u32, i32) -> i32>("GetOverlappedResult")(h, ov, n, 1)
}

/// `CancelIoEx`: en la casa no hay E/S pendiente que cancelar.
extern "win64" fn cancel_io_ex(h: u64, _ov: u64) -> i32 {
    if crate::ficheros::es_fichero(h) || kernel32::es_consola(h) {
        kernel32::poner_error(ERROR_NOT_FOUND);
    } else {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
    }
    0
}

/// `FormatMessageA`: la "W" de la casa, en UTF-8 (la pagina "A" de la casa).
extern "win64" fn format_message_a(banderas: u32, fuente: u64, id: u32, idioma: u32, buf: *mut u8, n: u32, args: u64) -> i32 {
    let mut w16 = alloc::vec![0u16; 4096];
    let k = w::<extern "win64" fn(u32, u64, u32, u32, *mut u16, u32, u64) -> u32>("FormatMessageW")(banderas & !FORMAT_MESSAGE_ALLOCATE_BUFFER, fuente, id, idioma, w16.as_mut_ptr(), 4096, args);
    if k == 0 {
        return 0;
    }
    let texto = alloc::string::String::from_utf16_lossy(&w16[..k as usize]);
    let b = texto.as_bytes();
    if banderas & FORMAT_MESSAGE_ALLOCATE_BUFFER != 0 {
        let Some(p) = memoria::pedir_del_proceso(b.len().max(n as usize) as u64 + 1) else {
            kernel32::poner_error(ERROR_NOT_ENOUGH_MEMORY);
            return 0;
        };
        // SAFETY: el bloque recien pedido, y el LPSTR* del `.exe`.
        unsafe {
            core::ptr::copy_nonoverlapping(b.as_ptr(), p as *mut u8, b.len());
            *(p as *mut u8).add(b.len()) = 0;
            *(buf as *mut u64) = p;
        }
        return b.len() as i32;
    }
    if buf.is_null() || b.len() + 1 > n as usize {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    // SAFETY: `n` bytes del `.exe` (cabe, con el 0).
    unsafe {
        core::ptr::copy_nonoverlapping(b.as_ptr(), buf, b.len());
        *buf.add(b.len()) = 0;
    }
    b.len() as i32
}

#[cfg(test)]
pub(crate) fn crear_mapeo_solo_lectura_para_prueba(fichero: u64) -> u64 {
    create_file_mapping_w(fichero, 0, 0x02, 0, 0, core::ptr::null())
}

#[cfg(test)]
pub(crate) fn mapear_rango_para_prueba(mapeo: u64, desde: u64, n: usize) -> u64 {
    map_view_of_file(mapeo, 0x0004, (desde >> 32) as u32, desde as u32, n)
}

#[cfg(test)]
pub(crate) fn desmapear_para_prueba(dir: u64) -> i32 {
    unmap_view_of_file(dir)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "CreateFileMappingW" => dir!(create_file_mapping_w),
        "CreateFileMappingA" => dir!(create_file_mapping_a),
        "OpenFileMappingW" | "OpenFileMappingA" | "OpenEventA" | "OpenEventW" | "OpenMutexW" | "OpenMutexA" | "OpenSemaphoreW" | "OpenSemaphoreA" => dir!(open_por_nombre),
        "MapViewOfFile" => dir!(map_view_of_file),
        "MapViewOfFileEx" => dir!(map_view_of_file_ex),
        "UnmapViewOfFile" => dir!(unmap_view_of_file),
        "FlushViewOfFile" => dir!(flush_view_of_file),
        "CreateIoCompletionPort" => dir!(create_io_completion_port),
        "PostQueuedCompletionStatus" => dir!(post_queued_completion_status),
        "GetQueuedCompletionStatus" => dir!(get_queued_completion_status),
        "CreateSemaphoreExW" => dir!(create_semaphore_ex_w),
        "OpenProcess" => dir!(open_process),
        "OpenThread" => dir!(open_thread),
        "ReadProcessMemory" => dir!(read_process_memory),
        "WaitForMultipleObjectsEx" => dir!(wait_for_multiple_objects_ex),
        "GetOverlappedResultEx" => dir!(get_overlapped_result_ex),
        "CancelIoEx" => dir!(cancel_io_ex),
        "FormatMessageA" => dir!(format_message_a),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas_mapeo {
    use super::largo_vista;

    #[test]
    fn una_vista_de_64_kib_en_un_archive_de_5_gib_no_reserva_el_archivo() {
        let tam = (5u64 << 30) + 37;
        let desde = 1u64 << 32;
        let n = 64 * 1024;
        let largo = largo_vista(tam, desde, n).unwrap();
        assert_eq!(largo, n as u64);
        assert_eq!(largo.div_ceil(4096) * 4096, n as u64);
    }

    #[test]
    fn una_vista_de_cero_bytes_va_hasta_el_final_y_no_pasa_el_archive() {
        let tam = (5u64 << 30) + 37;
        assert_eq!(largo_vista(tam, 0, 0), Some(tam));
        assert_eq!(largo_vista(tam, tam - 64 * 1024, 0), Some(64 * 1024));
        assert_eq!(largo_vista(tam, tam - 10, 11), None);
        assert_eq!(largo_vista(tam, tam, 1), None);
    }
}
