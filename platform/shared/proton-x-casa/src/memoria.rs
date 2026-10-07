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
//! **Con la RESERVA** (P0.4c, 30-09: `Plataforma::reserva`, en BMO-X la
//! ventana de 384 GiB de `TASK_OP_RESERVA_*`) es como Windows: `VirtualAlloc`
//! RESERVA direcciones de la ventana sin gastar nada, y cada MEM_COMMIT pide
//! al kernel solo esas paginas (a cero), juzgadas contra la RAM libre; cada
//! DECOMMIT o RELEASE las devuelve. Las arenas del monton tambien salen de
//! ahi, y de la medida que haga falta (un HeapAlloc de 100 MiB cabe).
//!
//! Lo que no es Windows, dicho: sin reserva, una region reservada ya gasta su
//! memoria (ver `regiones`); reservar en una direccion fija, ejecutar lo pedido
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
    /// Los huecos de la ventana de reserva (`(base, bytes)`, a 64 KiB), y si
    /// ya se tomo la ventana de la plataforma.
    huecos: Vec<(u64, u64)>,
    ventana: bool,
    /// Cuantas veces se ha dicho NO a una pedida de memoria (se cuentan
    /// todas; se dicen las `MAX_NOES` primeras).
    noes: u32,
    /// Las pedidas grandes que SI se dieron (se dicen las `MAX_NOES` primeras).
    grandes: u32,
    /// Las VirtualQuery (se dicen las `MAX_CONSULTAS` primeras).
    consultas: u32,
    /// Los montones de `HeapCreate` vivos (el del proceso no esta: siempre vive).
    creados: Vec<u16>,
    siguiente: u16,
    /// La memoria PRESTABLE a la 3060 (tanda 45): su monton, y cuantos
    /// bloques de la plataforma lleva.
    prestable: Monton,
    bloques_prestables: u32,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos y ninguna funcion
// de aqui cede el turno.
// [hilos] cerrojo -- el monton de Windows (HeapAlloc) y la reserva: de todo el proceso
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { monton: Monton::nuevo(), regiones: Regiones::nuevas(), huecos: Vec::new(), ventana: false, noes: 0, grandes: 0, consultas: 0, creados: Vec::new(), siguiente: 2, prestable: Monton::nuevo(), bloques_prestables: 0 }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    // Lo tomado de la ventana vuelve a la plataforma. En BMO-X no hay nada
    // (se reinicia antes del `.exe`); en el banco un `.exe` corre detras de
    // otro, y sin esto el siguiente encontraria hechas, y SUCIAS, las paginas
    // del anterior (un VirtualAlloc que no da ceros).
    if let Some(r) = reserva().filter(|_| e.ventana) {
        let mut d = r.base;
        for &(b, n) in &e.huecos {
            if b > d {
                (r.deshacer)(d, b - d);
            }
            d = b + n;
        }
        if d < r.base + r.bytes {
            (r.deshacer)(d, r.base + r.bytes - d);
        }
    }
    e.monton = Monton::nuevo();
    e.regiones = Regiones::nuevas();
    e.creados.clear();
    e.huecos.clear();
    e.ventana = false;
    e.noes = 0;
    e.grandes = 0;
    tramos().clear();
    e.siguiente = 2;
    // Los bloques prestables no se devuelven (la plataforma no tiene como):
    // en el banco, el `.exe` siguiente empieza con un monton vacio.
    e.prestable = Monton::nuevo();
    e.bloques_prestables = 0;
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

// -- Decir los NO (tanda 26 de Cyberpunk) --------------------------------

/// Cuantos NO (y cuantas pedidas grandes) se dicen por la consola.
const MAX_NOES: u32 = 12;
/// Desde cuanto una pedida de VirtualAlloc se dice aunque salga bien.
const GRANDE: u64 = 1 << 30;

/// Lo tomado de la ventana de reserva y lo que mide, en MiB.
fn ventana_mib() -> (u64, u64) {
    let Some(r) = reserva() else { return (0, 0) };
    let e = estado();
    let libre: u64 = if e.ventana { e.huecos.iter().map(|h| h.1).sum() } else { r.bytes };
    ((r.bytes - libre.min(r.bytes)) >> 20, r.bytes >> 20)
}

/// **Un NO a una pedida de memoria, dicho con sus numeros.** En el metal,
/// Cyberpunk se rindio con "Out of Memory! Failed to allocate %llu bytes"
/// (su redMemory, un `int3` a proposito) y la consola no decia a que
/// funcion de la casa se le habia dicho que no: ahora lo dice (con el
/// diario encendido: un NO es parte de Windows, y las tandas lo piden).
fn decir_no(que: &str) {
    let e = estado();
    e.noes += 1;
    if e.noes <= MAX_NOES && crate::diario::encendido() {
        let (usado, total) = ventana_mib();
        crate::decir(&alloc::format!("memoria NEGADA: {que}; ventana de reserva: {usado} de {total} MiB tomados; RAM libre {} MiB", ram().map_or(0, |r| r.1 >> 20)));
    }
}

/// Una pedida grande que salio bien: para saber, si luego falta, quien se
/// llevo la ventana.
fn decir_grande(dir: u64, n: u64, tipo: u32) {
    let e = estado();
    e.grandes += 1;
    if e.grandes <= MAX_NOES && crate::diario::encendido() {
        let (usado, total) = ventana_mib();
        crate::decir(&alloc::format!("VirtualAlloc grande: {} MiB en {dir:#x} (tipo {tipo:#x}); ventana: {usado} de {total} MiB tomados", n >> 20));
    }
}

// -- La RESERVA (P0.4c) --------------------------------------------------

fn reserva() -> Option<crate::Reserva> {
    plataforma().reserva
}

/// **Tomar direcciones** de la ventana: `bytes` (a 64 KiB), el primer hueco
/// que quepa. No gasta RAM.
fn tomar_va(bytes: u64) -> Option<u64> {
    let r = reserva()?;
    let e = estado();
    if !e.ventana {
        e.huecos.push((r.base, r.bytes));
        e.ventana = true;
    }
    let bytes = bytes.checked_add(GRANO - 1)? & !(GRANO - 1);
    let i = e.huecos.iter().position(|h| h.1 >= bytes)?;
    let (b, n) = e.huecos[i];
    if n == bytes {
        e.huecos.remove(i);
    } else {
        e.huecos[i] = (b + bytes, n - bytes);
    }
    Some(b)
}

/// Devolver direcciones a la ventana, juntando con los huecos de al lado.
fn soltar_va(base: u64, bytes: u64) {
    let bytes = (bytes + GRANO - 1) & !(GRANO - 1);
    let h = &mut estado().huecos;
    let i = h.iter().position(|x| x.0 > base).unwrap_or(h.len());
    h.insert(i, (base, bytes));
    if i + 1 < h.len() && h[i].0 + h[i].1 == h[i + 1].0 {
        h[i].1 += h[i + 1].1;
        h.remove(i + 1);
    }
    if i > 0 && h[i - 1].0 + h[i - 1].1 == h[i].0 {
        h[i - 1].1 += h[i].1;
        h.remove(i);
    }
}

fn en_ventana(d: u64) -> bool {
    reserva().is_some_and(|r| d >= r.base && d - r.base < r.bytes)
}

/// Las paginas de una tirada que se HACE: en la ventana, del kernel (ya a
/// cero); fuera, del monton, a cero aqui.
fn dar(d: u64, n: u64) -> bool {
    match reserva() {
        Some(r) if en_ventana(d) => (r.hacer)(d, n),
        _ => {
            a_cero(d, n);
            true
        }
    }
}

/// Las de una tirada que se DESHACE: las de la ventana vuelven al kernel.
fn devolver(d: u64, n: u64) {
    if let Some(r) = reserva().filter(|_| en_ventana(d)) {
        (r.deshacer)(d, n);
    }
}

/// La RAM de la maquina (total, libre), si la plataforma la sabe.
pub(crate) fn ram() -> Option<(u64, u64)> {
    reserva().map(|r| (r.ram)())
}

/// **Pedir** al monton; si no cabe, una arena mas y otra vez.
fn pedir(tam: u64, alin: u64, propietario: u16) -> Option<u64> {
    let e = estado();
    if let Some(p) = e.monton.pedir(&mut Real, tam, alin, propietario) {
        return Some(p);
    }
    let hace_falta = tam.checked_add(alin + 256)?;
    // Con la reserva, la arena sale de la ventana y mide lo que haga falta.
    if let Some(r) = reserva() {
        let n = hace_falta.max(ARENA).checked_add(GRANO - 1)? & !(GRANO - 1);
        let Some(base) = tomar_va(n) else {
            decir_no(&alloc::format!("una arena de {} MiB para el monton (pedida de {tam} B, alineada a {alin}): no queda hueco en la ventana", n >> 20));
            return None;
        };
        if !(r.hacer)(base, n) {
            soltar_va(base, n);
            aviso(&alloc::format!("el monton de Windows pide una arena de {} MiB y el kernel dice que no hay RAM", n >> 20));
            return None;
        }
        if !e.monton.agregar(&mut Real, base, n) {
            (r.deshacer)(base, n);
            soltar_va(base, n);
            aviso("el monton de Windows no tiene sitio para otra arena");
            return None;
        }
        return e.monton.pedir(&mut Real, tam, alin, propietario);
    }
    if hace_falta > ARENA {
        aviso(&alloc::format!("una pedida de {} MiB de una vez: el kernel de BMO-X da bloques de hasta 64 MiB (P0.4c)", tam.div_ceil(1 << 20)));
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
/// **La pila del hilo principal** (tanda 29 de Cyberpunk, 01-10): `bytes`
/// hechos, R+W, a cero, como la reserva de pila que Windows le da al hilo
/// principal (`SizeOfStackReserve` del `.exe`). La de BMO-X para cualquier
/// programa son 64 KiB, y Cyberpunk (su catch de C++, varios marcos de la
/// casa debajo) la desbordo. Sale de la ventana de reserva si la hay (y la
/// cuenta de VirtualAlloc la ve, como en Windows); si no, del monton.
/// Devuelve el FONDO (la direccion mas baja).
pub fn pila_principal(bytes: u64) -> Option<u64> {
    let bytes = bytes.checked_add(GRANO - 1)? & !(GRANO - 1);
    if let Some(r) = reserva() {
        let base = tomar_va(bytes)?;
        if !(r.hacer)(base, bytes) {
            soltar_va(base, bytes);
            return None;
        }
        estado().regiones.nueva(base, bytes, 0x04, true); // PAGE_READWRITE
        return Some(base);
    }
    let p = pedir(bytes, 16, PROPIETARIO_VIRTUAL)?;
    a_cero(p, bytes);
    Some(p)
}

/// **Paginas propias de la ventana de reserva**: `bytes` (a 64 KiB), hechas,
/// R+W y a cero. `None` (y dicho, con `que`) si no hay hueco o RAM. Sin
/// reserva, del monton de Windows, a 4 KiB.
fn pedir_paginas(bytes: u64, que: &str) -> Option<u64> {
    let Some(r) = reserva() else {
        let p = pedir(bytes.max(1), PAGINA, PROPIETARIO_VIRTUAL)?;
        a_cero(p, bytes);
        return Some(p);
    };
    let n = bytes.max(1).checked_add(GRANO - 1)? & !(GRANO - 1);
    let Some(base) = tomar_va(n) else {
        decir_no(&alloc::format!("{que} de {} MiB: no queda hueco en la ventana", bytes >> 20));
        return None;
    };
    if !(r.hacer)(base, n) {
        soltar_va(base, n);
        decir_no(&alloc::format!("{que} de {} MiB: el kernel dice que no hay RAM", bytes >> 20));
        return None;
    }
    Some(base)
}

/// **La memoria de un bufer de D3D12** (tanda 44 de Cyberpunk, 02-10): `bytes`
/// R+W, a cero, alineados a 256 (lo que D3D12 pide a un bufer de
/// constantes). Antes era un `Vec` del monton del CARGADOR, que mide 48 MiB y
/// solo avanza: Cyberpunk pidio un bufer de 192 MiB y el cargador entro en
/// panico. Ahora, como en Windows, es memoria del proceso: lo de 64 KiB o mas,
/// paginas propias de la ventana de reserva (sin gastar una arena del monton,
/// que son 64); lo chico, del monton de Windows. Como todo objeto de la casa,
/// no se devuelve (ver `com::release`).
pub(crate) fn pedir_bufer(bytes: u64) -> Option<u64> {
    if reserva().is_some() && bytes >= GRANO {
        return pedir_paginas(bytes, "un bufer de D3D12");
    }
    let p = pedir(bytes.max(1), 256, PROPIETARIO_VIRTUAL)?;
    a_cero(p, bytes);
    Some(p)
}

/// Cuantos bloques de la plataforma se gastan, como mucho, en memoria
/// prestable: un proceso de BMO-X tiene OCHO, y el cargador, la superficie de
/// la ventana y el codigo de los sombreadores tambien los quieren.
const MAX_PRESTABLES: u32 = 2;

/// **Memoria PRESTABLE a la 3060** (tanda 45, 02-10): `bytes` a cero, a
/// PAGINA, dentro de un BLOQUE de la plataforma -- en BMO-X un bloque del
/// kernel, contiguo, que es lo unico que `IOMMU_OP_GPU_DIBUJAR` sabe prestar
/// (el kernel lo busca con `fisica_de` entre los bloques del proceso; una
/// pagina de la ventana de reserva no esta ahi). Lo reparte un monton propio
/// en bloques de 64 MiB, como mucho [`MAX_PRESTABLES`]. `None` si no cabe:
/// quien llama tira de [`pedir_paginas`], que la CPU dibuja igual.
fn pedir_prestable(bytes: u64) -> Option<u64> {
    let e = estado();
    let bytes = bytes.max(1);
    if let Some(p) = e.prestable.pedir(&mut Real, bytes, PAGINA, PROPIETARIO_VIRTUAL) {
        a_cero(p, bytes);
        return Some(p);
    }
    if e.bloques_prestables >= MAX_PRESTABLES || bytes.checked_add(PAGINA + 256)? > ARENA {
        return None;
    }
    let base = (plataforma().memoria)(ARENA as usize)?;
    e.bloques_prestables += 1;
    if !e.prestable.agregar(&mut Real, base, ARENA) {
        return None;
    }
    let p = e.prestable.pedir(&mut Real, bytes, PAGINA, PROPIETARIO_VIRTUAL)?;
    a_cero(p, bytes);
    Some(p)
}

/// Lo mas que mide una textura (que no es de la cadena) para ir a memoria
/// prestable: las que la 3060 muestrea hoy son chicas, y un render target de
/// 1080p (8 MiB) llenaria los bloques en un momento.
pub(crate) const TEXTURA_PRESTABLE: u64 = 4 << 20;

/// **Los pixeles de una textura** (tanda 45): `bytes` a cero, a PAGINA, de
/// memoria del proceso -- antes eran un `Vec` del monton del cargador (48
/// MiB, solo avanza), donde un juego a 1080p no cabe. `prestable`: la 3060
/// podria usarla (un back buffer de la cadena, una textura chica), y se
/// intenta primero en un bloque prestable; lo demas, de la ventana.
pub(crate) fn pedir_pixeles(bytes: u64, prestable: bool) -> Option<u64> {
    if prestable {
        if let Some(p) = pedir_prestable(bytes) {
            return Some(p);
        }
    }
    pedir_paginas(bytes, "una textura de D3D12")
}

/// **Las direcciones de un `ID3D12Heap`** (tanda 44): `bytes` de la ventana de
/// reserva, SIN hacer ninguna pagina (un monton de 256 MiB no gasta nada hasta
/// que se coloca algo en el). `None` sin reserva, o sin hueco.
pub(crate) fn reservar_direcciones(bytes: u64) -> Option<u64> {
    reserva()?;
    tomar_va(bytes)
}

/// **Una region de `VirtualAlloc` ENTERA, desde su base** (tanda 46, para
/// `OpenExistingHeapFromAddress`): su medida si `dir` es la base de la
/// region, y TODAS sus paginas estan hechas con la misma proteccion (lo que
/// `VirtualQuery` daria como una sola region). `None` si no.
pub(crate) fn region_entera(dir: u64) -> Option<u64> {
    let e = estado();
    let c = e.regiones.consultar(dir)?;
    if c.base_region != dir || c.estado != MEM_COMMIT {
        return None;
    }
    let mut tam = c.tam;
    while let Some(s) = e.regiones.consultar(dir + tam) {
        if s.base_region != dir {
            break;
        }
        if s.estado != MEM_COMMIT || s.prot != c.prot {
            return None;
        }
        tam += s.tam;
    }
    Some(tam)
}

/// Si hay ventana de reserva (en BMO-X, si; en el banco viejo, no).
pub(crate) fn hay_reserva() -> bool {
    reserva().is_some()
}

/// Hacer las paginas que falten de `[va, va + bytes)`, dentro de unas
/// direcciones de [`reservar_direcciones`]. `false` si el kernel dice que no.
pub(crate) fn hacer_paginas(va: u64, bytes: u64) -> bool {
    let (d, fin) = regiones::paginas(va, bytes);
    match reserva() {
        Some(r) if en_ventana(d) && en_ventana(fin - 1) => (r.hacer)(d, fin - d),
        _ => false,
    }
}

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
            // P0.4c: con la reserva, reservar son solo direcciones.
            if reserva().is_some() {
                let (_, tam) = regiones::paginas(0, n as u64);
                let p = tomar_va(tam).ok_or(ERROR_NOT_ENOUGH_MEMORY)?;
                estado().regiones.nueva(p, tam, prot, false);
                if tipo & MEM_COMMIT != 0 && estado().regiones.hacer_con(p, tam, prot, dar).is_err() {
                    let _ = estado().regiones.deshacer_con(p, 0, devolver);
                    let _ = estado().regiones.soltar(p, 0);
                    soltar_va(p, tam);
                    return Err(ERROR_NOT_ENOUGH_MEMORY);
                }
                return Ok(p);
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
        estado().regiones.hacer_con(dir, n as u64, prot, dar).map_err(|x| x.error())
    })();
    match r {
        Ok(p) => {
            if n as u64 >= GRANDE {
                decir_grande(p, n as u64, tipo);
            }
            p
        }
        Err(err) => {
            decir_no(&alloc::format!("VirtualAlloc({dir:#x}, {} MiB = {n} B, tipo {tipo:#x}, prot {prot:#x}) = NULL, error {err}", (n as u64) >> 20));
            kernel32::poner_error(err);
            0
        }
    }
}

extern "win64" fn virtual_free(dir: u64, n: usize, tipo: u32) -> i32 {
    let e = estado();
    let r = match tipo {
        // P0.4c: una region de la ventana devuelve sus paginas y sus direcciones.
        MEM_RELEASE if en_ventana(dir) => {
            if n != 0 {
                Err(regiones::NoVirtual::Parametro)
            } else {
                e.regiones.deshacer_con(dir, 0, devolver).and_then(|_| e.regiones.soltar(dir, 0)).map(|tam| soltar_va(dir, tam))
            }
        }
        MEM_RELEASE => e.regiones.soltar(dir, n as u64).map(|_| {
            e.monton.soltar(&mut Real, dir);
        }),
        MEM_DECOMMIT => e.regiones.deshacer_con(dir, n as u64, devolver),
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

const MEM_IMAGE: u32 = 0x100_0000;
const PAGE_READWRITE: u32 = 0x04;
const PAGE_EXECUTE_READ: u32 = 0x20;
const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;
/// Las VirtualQuery que se dicen (con el diario), de donde sean.
const MAX_CONSULTAS: u32 = 12;

/// **`VirtualQuery(dir, mbi, n)`: TODA direccion tiene respuesta**, como en
/// Windows (02-10).
///
/// ```text
///    de VirtualAlloc   la tirada de paginas iguales (regiones.rs)
///    de una imagen     MEM_IMAGE, AllocationBase = su HMODULE, el tramo
///                      (codigo EXECUTE_READ, datos READWRITE)
///    lo demas          el monton, los TEB, lo de la casa: MEM_PRIVATE,
///                      hecha, READWRITE, una pagina
/// ```
///
/// Hasta hoy, fuera de VirtualAlloc devolvia 0 con un aviso (y los avisos
/// solo se dicen los 8 primeros). Se sospecho que ese 0 era el `tabla[-1]`
/// de Cyberpunk (`Cyberpunk2077.exe+0x24d8b3`, `imul rdi, rax, 0x1900`); en
/// el metal (01-10 22:49) la caida SIGUIO igual: no era eso. Lo de Windows se
/// queda (tanda39), y las primeras consultas se dicen todas.
extern "win64" fn virtual_query(dir: u64, mbi: *mut u8, n: usize) -> usize {
    if n < 48 {
        kernel32::poner_error(ERROR_BAD_LENGTH);
        return 0;
    }
    if mbi.is_null() || dir >= 0x8000_0000_0000 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let (c, de_donde): (Respuesta, &str) = match estado().regiones.consultar(dir) {
        Some(c) => (c.into(), "VirtualAlloc"),
        None => {
            let c = fuera_de_regiones(dir);
            let tipo = if c.tipo == MEM_IMAGE { "imagen" } else { "privada" };
            (c, tipo)
        }
    };
    // Las primeras se dicen TODAS, de donde sean (02-10: el juego sigue
    // cayendo en `tabla[-1]` justo despues de su primera VirtualQuery, y
    // no fue de las de fuera: hace falta ver QUE pregunta y QUE se contesta).
    let e = estado();
    e.consultas += 1;
    if e.consultas <= MAX_CONSULTAS && crate::diario::encendido() {
        crate::decir(&alloc::format!(
            "VirtualQuery({dir:#x}) [{de_donde}]: base {:#x}, region {:#x} (prot {:#x}), {:#x} B, estado {:#x}, prot {:#x}",
            c.base, c.base_region, c.prot_inicial, c.tam, c.estado, c.prot
        ));
    }
    // MEMORY_BASIC_INFORMATION de x64: 48 bytes.
    let mut b = [0u8; 48];
    b[0..8].copy_from_slice(&c.base.to_le_bytes());
    b[8..16].copy_from_slice(&c.base_region.to_le_bytes());
    b[16..20].copy_from_slice(&c.prot_inicial.to_le_bytes());
    b[24..32].copy_from_slice(&c.tam.to_le_bytes());
    b[32..36].copy_from_slice(&c.estado.to_le_bytes());
    b[36..40].copy_from_slice(&c.prot.to_le_bytes());
    b[40..44].copy_from_slice(&c.tipo.to_le_bytes());
    // SAFETY: el `.exe` da 48 bytes (comprobado arriba).
    unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), mbi, 48) };
    48
}

/// Lo que dice VirtualQuery de una direccion que no es de VirtualAlloc.
struct Respuesta {
    base: u64,
    base_region: u64,
    prot_inicial: u32,
    tam: u64,
    estado: u32,
    prot: u32,
    tipo: u32,
}

impl From<regiones::Consulta> for Respuesta {
    fn from(c: regiones::Consulta) -> Self {
        Respuesta { base: c.base, base_region: c.base_region, prot_inicial: c.prot_inicial, tam: c.tam, estado: c.estado, prot: c.prot, tipo: MEM_PRIVATE }
    }
}

fn fuera_de_regiones(dir: u64) -> Respuesta {
    let pagina = dir & !(PAGINA - 1);
    if let Some(base) = crate::kernel32_procesos::imagen_con(dir) {
        // El tramo que la contiene (los pone el cargador al sellar); sin
        // tramos, la imagen entera como datos.
        let (hasta, codigo) = tramos().iter().find(|t| t.0 <= dir && dir < t.1).map_or((base + crate::kernel32_procesos::medida_imagen(base) as u64, false), |t| (t.1, t.2));
        let hasta = (hasta + PAGINA - 1) & !(PAGINA - 1);
        return Respuesta {
            base: pagina,
            base_region: base,
            prot_inicial: PAGE_EXECUTE_WRITECOPY,
            tam: hasta.saturating_sub(pagina).max(PAGINA),
            estado: MEM_COMMIT,
            prot: if codigo { PAGE_EXECUTE_READ } else { PAGE_READWRITE },
            tipo: MEM_IMAGE,
        };
    }
    Respuesta { base: pagina, base_region: dir & !(GRANO - 1), prot_inicial: PAGE_READWRITE, tam: PAGINA, estado: MEM_COMMIT, prot: PAGE_READWRITE, tipo: MEM_PRIVATE }
}

// -- VirtualProtect sobre la IMAGEN (P0.4b.8, 30-09) ----------------------

/// Los tramos de la imagen cargada: `(desde, hasta, codigo)`. Los pone el
/// cargador ([`registrar_tramos`]) al sellar.
struct Tramos(core::cell::UnsafeCell<Vec<(u64, u64, bool)>>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
// [hilos] cerrojo -- los tramos de VirtualAlloc: de todo el proceso
unsafe impl Sync for Tramos {}
static TRAMOS: Tramos = Tramos(core::cell::UnsafeCell::new(Vec::new()));

fn tramos() -> &'static mut Vec<(u64, u64, bool)> {
    // SAFETY: ver `Tramos`; nadie guarda la referencia de un turno a otro.
    unsafe { &mut *TRAMOS.0.get() }
}

/// **Los tramos de un modulo cargado** (su base, y cada tramo: bytes y si es
/// codigo), para que `VirtualProtect` sobre la imagen conteste como Windows.
pub fn registrar_tramos(base: u64, t: &[(u64, bool)]) {
    let mut d = base;
    for &(bytes, codigo) in t {
        tramos().push((d, d + bytes, codigo));
        d += bytes;
    }
}

/// `VirtualProtect` sobre la imagen. En Windows se puede todo; aqui la
/// imagen ya esta sellada (codigo R+X, datos R+W) y NO cambia: lo que no
/// pide lo imposible se contesta que si, con la proteccion de ahora en
/// `antes` (el juego suele hacer "RW y luego lo de antes"). Lo que pediria
/// W y X a la vez sobre una pagina, NO: el W^X de la casa.
///
/// ```text
///    codigo (R+X)   pide R, X o R+X    si; antes = PAGE_EXECUTE_READ
///                   pide W             NO (sellado)
///    datos  (R+W)   pide NOACCESS, R, RW o WRITECOPY
///                                      si, y sigue RW (Windows lo cambiaria)
///                   pide X             NO
/// ```
fn proteger_imagen(dir: u64, n: u64, prot: u32) -> Option<Result<u32, u32>> {
    let desde = dir & !(PAGINA - 1);
    let hasta = dir.checked_add(n.max(1))?.checked_add(PAGINA - 1)? & !(PAGINA - 1);
    let t = tramos().iter().find(|t| t.0 <= desde && desde < t.1)?;
    let codigo = t.2;
    // Todo el rango, del mismo permiso.
    let mut d = desde;
    while d < hasta {
        match tramos().iter().find(|t| t.0 <= d && d < t.1) {
            Some(t) if t.2 == codigo => d = t.1,
            _ => {
                aviso("VirtualProtect sobre la imagen: el rango mezcla codigo y datos, o se sale");
                return Some(Err(ERROR_INVALID_PARAMETER));
            }
        }
    }
    let ok = match (codigo, prot & 0xFF) {
        (true, 0x02 | 0x10 | 0x20) => true,
        (false, 0x01 | 0x02 | 0x04 | 0x08) => true,
        _ => false,
    };
    if !ok {
        aviso(if codigo { "VirtualProtect pide ESCRIBIR codigo de la imagen: esta sellado (W^X)" } else { "VirtualProtect pide EJECUTAR datos de la imagen: W^X" });
        return Some(Err(ERROR_INVALID_PARAMETER));
    }
    Some(Ok(if codigo { 0x20 } else { 0x04 }))
}

extern "win64" fn virtual_protect(dir: u64, n: usize, prot: u32, antes: *mut u32) -> i32 {
    let r = match proteger_imagen(dir, n as u64, prot) {
        Some(r) => r,
        None => proteccion(prot).and_then(|p| {
            estado().regiones.proteger(dir, n as u64, p).map_err(|x| {
                if estado().regiones.consultar(dir).is_none() {
                    aviso("VirtualProtect fuera de VirtualAlloc y de la imagen");
                }
                x.error()
            })
        }),
    };
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

/// `SYSTEM_INFO` de x64 (48 bytes). Los procesadores de
/// `bmo_proton_x::procesadores` (02-10: era uno, y Cyberpunk no lo aguanta).
extern "win64" fn get_system_info(si: *mut u8) {
    let mut b = [0u8; 48];
    b[0..2].copy_from_slice(&9u16.to_le_bytes()); // PROCESSOR_ARCHITECTURE_AMD64
    b[4..8].copy_from_slice(&(PAGINA as u32).to_le_bytes());
    b[8..16].copy_from_slice(&0x1_0000u64.to_le_bytes());
    b[16..24].copy_from_slice(&0x7FFF_FFFE_FFFFu64.to_le_bytes());
    b[24..32].copy_from_slice(&bmo_proton_x::procesadores::MASCARA.to_le_bytes());
    b[32..36].copy_from_slice(&bmo_proton_x::procesadores::LOGICOS.to_le_bytes());
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
