//! **LA RED LOCAL DE LA CASA** -- sockets de verdad en 127.0.0.1, dentro del
//! proceso (01-10).
//!
//! Por Cyberpunk. La IA local desarmo `REDGalaxy64.dll` (solo lectura) y dijo
//! lo que Galaxy necesita de la red sin cable: **un par de sockets TCP en
//! 127.0.0.1** para despertar su hilo (dos sitios: 0x879645, que emula un
//! `socketpair`, y 0x7c69b0, el `select_interrupter` de Boost.Asio):
//!
//! ```text
//!    socket  setsockopt(SO_REUSEADDR)  bind(127.0.0.1:0)  getsockname
//!    listen  socket  connect  accept  send(1 byte)  recv(1 byte)
//! ```
//!
//! Y el logger UDP (`gog::UdpOutputSocket`, 0x8116b0): `socket(2,2,17)`,
//! `ioctlsocket(FIONBIO)` y `sendto` a su servidor, sin comprobar nada.
//!
//! Esto es **un Windows desenchufado**, no una red inventada: los sockets se
//! crean, el bucle local funciona de verdad (lo que uno manda, el otro lo
//! recibe) y lo que va FUERA de la maquina contesta WSAENETUNREACH. Ni un
//! byte sale del proceso. La red de BMO-X (`PLAN_RED_TX.md`) se cose el dia
//! que haga falta salir; este fichero es lo que NO necesita salir.
//!
//! ```text
//!    los SOCKET    0x5000 + 4*i, como los handles chicos de Windows
//!    bloqueantes   recv/accept sin nada CEDEN el turno a otro hilo de la
//!                  casa hasta que llega (hilos cooperativos); si no hay
//!                  otro hilo, WSAEWOULDBLOCK y una nota en el diario
//!    connect       instantaneo; en un socket no bloqueante devuelve
//!                  WSAEWOULDBLOCK (como Windows) con la conexion ya hecha
//! ```

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::kernel32;

const SOCKET_ERROR: i32 = -1;
const INVALID_SOCKET: u64 = u64::MAX;
const BASE: u64 = 0x5000;

const WSANOTINITIALISED: u32 = 10093;
const WSAEWOULDBLOCK: u32 = 10035;
const WSAENOTSOCK: u32 = 10038;
const WSAEAFNOSUPPORT: u32 = 10047;
const WSAEADDRINUSE: u32 = 10048;
const WSAEADDRNOTAVAIL: u32 = 10049;
const WSAENETUNREACH: u32 = 10051;
const WSAECONNRESET: u32 = 10054;
const WSAEISCONN: u32 = 10056;
const WSAENOTCONN: u32 = 10057;
const WSAECONNREFUSED: u32 = 10061;
const WSAEINVAL: u32 = 10022;
const WSAEFAULT: u32 = 10014;
const WSAEPROTOTYPE: u32 = 10041;
const WSAESOCKTNOSUPPORT: u32 = 10044;

const AF_INET: i32 = 2;
const SOCK_STREAM: i32 = 1;
const SOCK_DGRAM: i32 = 2;
const FIONBIO: u32 = 0x8004_667E;
const FIONREAD: u32 = 0x4004_667F;
const SOL_SOCKET: i32 = 0xFFFF;
const SO_TYPE: i32 = 0x1008;
const SO_ERROR: i32 = 0x1007;
const SO_REUSEADDR: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Estado {
    Nuevo,
    Escuchando,
    Conectado,
    Cerrado,
}

struct Socket {
    tipo: i32,
    bloqueante: bool,
    reusar: bool,
    /// `(ip, puerto)` local, en orden de la red, si esta atado.
    local: Option<([u8; 4], u16)>,
    estado: Estado,
    /// El otro extremo de una conexion TCP.
    par: Option<usize>,
    /// Conexiones esperando a `accept` (sockets ya creados).
    cola: VecDeque<usize>,
    /// TCP: los bytes recibidos. UDP: los datagramas, con quien los mando.
    flujo: VecDeque<u8>,
    datagramas: VecDeque<(([u8; 4], u16), Vec<u8>)>,
    /// El otro cerro o hizo shutdown de envio: recv devuelve 0 al vaciar.
    fin: bool,
}

struct Tabla(UnsafeCell<(Vec<Option<Socket>>, u16)>);
// SAFETY: una tarea, hilos cooperativos (ver `hilos`).
// [hilos] cerrojo -- estado del proceso que tocan los hilos del juego: necesita un cerrojo (H2.1)
unsafe impl Sync for Tabla {}
static TABLA: Tabla = Tabla(UnsafeCell::new((Vec::new(), 49152)));

fn tabla() -> &'static mut Vec<Option<Socket>> {
    // SAFETY: ver `Tabla`.
    unsafe { &mut (*TABLA.0.get()).0 }
}

fn puerto_libre() -> u16 {
    // SAFETY: ver `Tabla`.
    let p = unsafe { &mut (*TABLA.0.get()).1 };
    *p = if *p >= 65000 { 49152 } else { *p + 1 };
    *p
}

fn indice(s: u64) -> Option<usize> {
    if s < BASE || (s - BASE) % 4 != 0 {
        return None;
    }
    let i = ((s - BASE) / 4) as usize;
    matches!(tabla().get(i), Some(Some(k)) if k.estado != Estado::Cerrado).then_some(i)
}

fn sock(i: usize) -> &'static mut Socket {
    tabla()[i].as_mut().expect("indice() ya lo comprobo")
}

fn error_i(e: u32) -> i32 {
    kernel32::poner_error(e);
    SOCKET_ERROR
}

fn es_local(ip: [u8; 4]) -> bool {
    ip[0] == 127 || ip == [0, 0, 0, 0]
}

/// Un sockaddr_in del `.exe`: `(ip, puerto)` en orden de la red.
fn leer_dir(a: *const u8, n: i32) -> Option<([u8; 4], u16)> {
    if a.is_null() || n < 16 {
        return None;
    }
    // SAFETY: 16 bytes de un sockaddr_in del `.exe`.
    let b = unsafe { core::slice::from_raw_parts(a, 16) };
    if u16::from_le_bytes([b[0], b[1]]) != AF_INET as u16 {
        return None;
    }
    Some(([b[4], b[5], b[6], b[7]], u16::from_be_bytes([b[2], b[3]])))
}

/// Escribe un sockaddr_in en `a` (con `*n` >= 16) y deja `*n = 16`.
fn escribir_dir(a: *mut u8, n: *mut i32, d: ([u8; 4], u16)) -> bool {
    if a.is_null() || n.is_null() {
        return true;
    }
    // SAFETY: el `.exe` da `*n` bytes en `a`.
    unsafe {
        if *n < 16 {
            return false;
        }
        let b = core::slice::from_raw_parts_mut(a, 16);
        b.fill(0);
        b[0..2].copy_from_slice(&(AF_INET as u16).to_le_bytes());
        b[2..4].copy_from_slice(&d.1.to_be_bytes());
        b[4..8].copy_from_slice(&d.0);
        *n = 16;
    }
    true
}

fn nuevo(tipo: i32) -> u64 {
    let s = Socket {
        tipo,
        bloqueante: true,
        reusar: false,
        local: None,
        estado: Estado::Nuevo,
        par: None,
        cola: VecDeque::new(),
        flujo: VecDeque::new(),
        datagramas: VecDeque::new(),
        fin: false,
    };
    let t = tabla();
    let i = match t.iter().position(|x| x.is_none()) {
        Some(i) => {
            t[i] = Some(s);
            i
        }
        None => {
            t.push(Some(s));
            t.len() - 1
        }
    };
    BASE + 4 * i as u64
}

/// El socket de `tipo` atado al puerto `p`, si lo hay (y no es `menos`).
fn atado_a(tipo: i32, p: u16, menos: Option<usize>) -> Option<usize> {
    tabla().iter().enumerate().find_map(|(i, x)| match x {
        Some(k) if Some(i) != menos && k.tipo == tipo && k.estado != Estado::Cerrado && k.local.map(|l| l.1) == Some(p) => Some(i),
        _ => None,
    })
}

/// Ata a 127.0.0.1 con un puerto libre si no lo estaba (lo que hace Windows
/// en el primer `connect` o `sendto`).
fn atar_si_falta(i: usize) {
    if sock(i).local.is_none() {
        sock(i).local = Some(([127, 0, 0, 1], puerto_libre()));
    }
}

/// **Esperar** a que `listo()` se cumpla, cediendo el turno a otro hilo de la
/// casa. `false` si no hay otro hilo que pueda hacerlo cumplir.
fn esperar(listo: impl Fn() -> bool) -> bool {
    let mut vueltas = 0u32;
    while !listo() {
        if !crate::hilos::ceder() || vueltas > 100_000 {
            return false;
        }
        vueltas += 1;
    }
    true
}

// ===================================================================
//  Las funciones de ws2_32
// ===================================================================

pub(crate) extern "win64" fn socket(af: i32, tipo: i32, proto: i32) -> u64 {
    if !crate::red::arrancado() {
        kernel32::poner_error(WSANOTINITIALISED);
        return INVALID_SOCKET;
    }
    if af != AF_INET {
        // IPv6 sin red: Windows lo deja crear, pero la casa no lleva IPv6
        // local; se dice con el error de familia.
        crate::diario::nota(&alloc::format!("red: socket({af}, {tipo}, {proto}): solo IPv4 en la red local"));
        kernel32::poner_error(WSAEAFNOSUPPORT);
        return INVALID_SOCKET;
    }
    match (tipo, proto) {
        (SOCK_STREAM, 0 | 6) | (SOCK_DGRAM, 0 | 17) => nuevo(tipo),
        (SOCK_STREAM | SOCK_DGRAM, _) => {
            kernel32::poner_error(WSAEPROTOTYPE);
            INVALID_SOCKET
        }
        _ => {
            kernel32::poner_error(WSAESOCKTNOSUPPORT);
            INVALID_SOCKET
        }
    }
}

pub(crate) extern "win64" fn wsa_socket_w(af: i32, tipo: i32, proto: i32, _info: u64, _g: u32, _banderas: u32) -> u64 {
    socket(af, tipo, proto)
}

pub(crate) extern "win64" fn closesocket(s: u64) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    if let Some(p) = sock(i).par {
        if indice(BASE + 4 * p as u64).is_some() {
            sock(p).fin = true;
            sock(p).par = None;
        }
    }
    // Las conexiones que esperaban su `accept` se cierran con el.
    let cola: Vec<usize> = sock(i).cola.drain(..).collect();
    for c in cola {
        if let Some(p) = sock(c).par {
            sock(p).fin = true;
        }
        tabla()[c] = None;
    }
    tabla()[i] = None;
    0
}

pub(crate) extern "win64" fn bind(s: u64, a: *const u8, n: i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    let Some((ip, mut p)) = leer_dir(a, n) else { return error_i(WSAEFAULT) };
    if sock(i).local.is_some() {
        return error_i(WSAEINVAL);
    }
    if !es_local(ip) {
        return error_i(WSAEADDRNOTAVAIL);
    }
    if p == 0 {
        p = puerto_libre();
    } else if let Some(j) = atado_a(sock(i).tipo, p, Some(i)) {
        if !(sock(i).reusar && sock(j).reusar) {
            return error_i(WSAEADDRINUSE);
        }
    }
    sock(i).local = Some((ip, p));
    0
}

pub(crate) extern "win64" fn getsockname(s: u64, a: *mut u8, n: *mut i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    let Some(d) = sock(i).local else { return error_i(WSAEINVAL) };
    if !escribir_dir(a, n, d) {
        return error_i(WSAEFAULT);
    }
    0
}

pub(crate) extern "win64" fn getpeername(s: u64, a: *mut u8, n: *mut i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    let Some(p) = sock(i).par else { return error_i(WSAENOTCONN) };
    let d = sock(p).local.unwrap_or(([127, 0, 0, 1], 0));
    if !escribir_dir(a, n, d) {
        return error_i(WSAEFAULT);
    }
    0
}

pub(crate) extern "win64" fn listen(s: u64, _cola: i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    if sock(i).tipo != SOCK_STREAM || sock(i).local.is_none() || sock(i).estado == Estado::Conectado {
        return error_i(WSAEINVAL);
    }
    sock(i).estado = Estado::Escuchando;
    0
}

pub(crate) extern "win64" fn connect(s: u64, a: *const u8, n: i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    let Some((ip, p)) = leer_dir(a, n) else { return error_i(WSAEFAULT) };
    if sock(i).tipo == SOCK_DGRAM {
        // UDP: connect solo fija el destino; aqui basta con atarlo.
        atar_si_falta(i);
        return 0;
    }
    if sock(i).estado == Estado::Conectado {
        return error_i(WSAEISCONN);
    }
    if !es_local(ip) {
        return error_i(WSAENETUNREACH);
    }
    let Some(l) = tabla().iter().enumerate().find_map(|(j, x)| match x {
        Some(k) if k.estado == Estado::Escuchando && k.local.map(|d| d.1) == Some(p) => Some(j),
        _ => None,
    }) else {
        return error_i(WSAECONNREFUSED);
    };
    atar_si_falta(i);
    // El extremo del servidor nace ya conectado, en la cola de `accept`.
    let srv = ((nuevo(SOCK_STREAM) - BASE) / 4) as usize;
    let puerto = sock(l).local.map(|d| d.1).unwrap_or(p);
    sock(srv).local = Some(([127, 0, 0, 1], puerto));
    sock(srv).estado = Estado::Conectado;
    sock(srv).par = Some(i);
    sock(i).estado = Estado::Conectado;
    sock(i).par = Some(srv);
    sock(l).cola.push_back(srv);
    if !sock(i).bloqueante {
        // Windows: un connect no bloqueante dice "en curso" aunque al mirar
        // con select ya este hecho.
        return error_i(WSAEWOULDBLOCK);
    }
    0
}

pub(crate) extern "win64" fn accept(s: u64, a: *mut u8, n: *mut i32) -> u64 {
    if !crate::red::arrancado() {
        kernel32::poner_error(WSANOTINITIALISED);
        return INVALID_SOCKET;
    }
    let Some(i) = indice(s) else {
        kernel32::poner_error(WSAENOTSOCK);
        return INVALID_SOCKET;
    };
    if sock(i).estado != Estado::Escuchando {
        kernel32::poner_error(WSAEINVAL);
        return INVALID_SOCKET;
    }
    if sock(i).cola.is_empty() && (!sock(i).bloqueante || !esperar(|| !sock(i).cola.is_empty())) {
        if sock(i).bloqueante {
            crate::diario::nota("red: accept bloqueante sin nadie que conecte: WSAEWOULDBLOCK");
        }
        kernel32::poner_error(WSAEWOULDBLOCK);
        return INVALID_SOCKET;
    }
    let c = sock(i).cola.pop_front().expect("no vacia");
    if let Some(p) = sock(c).par {
        let d = sock(p).local.unwrap_or(([127, 0, 0, 1], 0));
        escribir_dir(a, n, d);
    }
    BASE + 4 * c as u64
}

pub(crate) extern "win64" fn send(s: u64, buf: *const u8, n: i32, _banderas: i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    if sock(i).estado != Estado::Conectado {
        return error_i(WSAENOTCONN);
    }
    let Some(p) = sock(i).par else { return error_i(WSAECONNRESET) };
    if n < 0 || (buf.is_null() && n > 0) {
        return error_i(WSAEFAULT);
    }
    // SAFETY: `n` bytes del `.exe`.
    let datos = unsafe { core::slice::from_raw_parts(buf, n as usize) };
    sock(p).flujo.extend(datos.iter().copied());
    n
}

pub(crate) extern "win64" fn recv(s: u64, buf: *mut u8, n: i32, _banderas: i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    if sock(i).tipo == SOCK_DGRAM {
        return recvfrom(s, buf, n, 0, core::ptr::null_mut(), core::ptr::null_mut());
    }
    if sock(i).estado != Estado::Conectado {
        return error_i(WSAENOTCONN);
    }
    if sock(i).flujo.is_empty() && !sock(i).fin {
        if !sock(i).bloqueante {
            return error_i(WSAEWOULDBLOCK);
        }
        if !esperar(|| !sock(i).flujo.is_empty() || sock(i).fin) {
            crate::diario::nota("red: recv bloqueante sin nadie que mande: WSAEWOULDBLOCK");
            return error_i(WSAEWOULDBLOCK);
        }
    }
    let k = (n.max(0) as usize).min(sock(i).flujo.len());
    for j in 0..k {
        // SAFETY: `n` bytes del `.exe`.
        unsafe { *buf.add(j) = sock(i).flujo.pop_front().expect("k <= len") };
    }
    k as i32
}

pub(crate) extern "win64" fn sendto(s: u64, buf: *const u8, n: i32, _banderas: i32, a: *const u8, an: i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    if sock(i).tipo == SOCK_STREAM {
        return send(s, buf, n, 0);
    }
    let Some((ip, p)) = leer_dir(a, an) else { return error_i(WSAEFAULT) };
    if !es_local(ip) {
        return error_i(WSAENETUNREACH);
    }
    if n < 0 || (buf.is_null() && n > 0) {
        return error_i(WSAEFAULT);
    }
    atar_si_falta(i);
    let de = sock(i).local.expect("atado");
    // SAFETY: `n` bytes del `.exe`.
    let datos = unsafe { core::slice::from_raw_parts(buf, n as usize) }.to_vec();
    // UDP local sin nadie escuchando: se pierde, como en Windows.
    if let Some(d) = atado_a(SOCK_DGRAM, p, None) {
        sock(d).datagramas.push_back((de, datos));
    }
    n
}

pub(crate) extern "win64" fn recvfrom(s: u64, buf: *mut u8, n: i32, _banderas: i32, a: *mut u8, an: *mut i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    if sock(i).tipo == SOCK_STREAM {
        return recv(s, buf, n, 0);
    }
    if sock(i).local.is_none() {
        return error_i(WSAEINVAL);
    }
    if sock(i).datagramas.is_empty() {
        if !sock(i).bloqueante || !esperar(|| !sock(i).datagramas.is_empty()) {
            return error_i(WSAEWOULDBLOCK);
        }
    }
    let (de, d) = sock(i).datagramas.pop_front().expect("no vacia");
    let k = (n.max(0) as usize).min(d.len());
    // SAFETY: `n` bytes del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(d.as_ptr(), buf, k) };
    escribir_dir(a, an, de);
    if k < d.len() {
        return error_i(10040); // WSAEMSGSIZE: el datagrama no cabia
    }
    k as i32
}

pub(crate) extern "win64" fn ioctlsocket(s: u64, cmd: u32, arg: *mut u32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    if arg.is_null() {
        return error_i(WSAEFAULT);
    }
    match cmd {
        // SAFETY: un u_long del `.exe`.
        FIONBIO => sock(i).bloqueante = unsafe { *arg } == 0,
        FIONREAD => {
            let k = &sock(i);
            let n = if k.tipo == SOCK_DGRAM { k.datagramas.front().map_or(0, |d| d.1.len()) } else { k.flujo.len() };
            // SAFETY: un u_long del `.exe`.
            unsafe { *arg = n as u32 };
        }
        _ => return error_i(WSAEINVAL),
    }
    0
}

pub(crate) extern "win64" fn setsockopt(s: u64, nivel: i32, opcion: i32, valor: *const u8, n: i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    if nivel == SOL_SOCKET && opcion == SO_REUSEADDR && !valor.is_null() && n >= 1 {
        // SAFETY: al menos un byte del `.exe` (un BOOL o un int).
        sock(i).reusar = unsafe { *valor } != 0;
    }
    // El resto (TCP_NODELAY, SO_RCVBUF, SO_LINGER...) no cambia nada en un
    // bucle local dentro del proceso: se acepta, como lo aceptaria Windows.
    0
}

pub(crate) extern "win64" fn getsockopt(s: u64, nivel: i32, opcion: i32, valor: *mut u8, n: *mut i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    if valor.is_null() || n.is_null() {
        return error_i(WSAEFAULT);
    }
    let v: i32 = match (nivel, opcion) {
        (SOL_SOCKET, SO_TYPE) => sock(i).tipo,
        (SOL_SOCKET, SO_ERROR) => 0,
        (SOL_SOCKET, SO_REUSEADDR) => sock(i).reusar as i32,
        _ => 0,
    };
    // SAFETY: `*n` bytes del `.exe`.
    unsafe {
        if *n < 4 {
            return error_i(WSAEFAULT);
        }
        core::ptr::copy_nonoverlapping(v.to_le_bytes().as_ptr(), valor, 4);
        *n = 4;
    }
    0
}

pub(crate) extern "win64" fn shutdown(s: u64, como: i32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let Some(i) = indice(s) else { return error_i(WSAENOTSOCK) };
    if sock(i).estado != Estado::Conectado {
        return error_i(WSAENOTCONN);
    }
    // SD_SEND (1) o SD_BOTH (2): el otro ve el final de lo que le mandan.
    if como != 0 {
        if let Some(p) = sock(i).par {
            sock(p).fin = true;
        }
    }
    0
}

/// Que esta listo de un socket: `(leer, escribir)`.
fn listo(i: usize) -> (bool, bool) {
    let k = sock(i);
    let leer = match k.estado {
        Estado::Escuchando => !k.cola.is_empty(),
        _ if k.tipo == SOCK_DGRAM => !k.datagramas.is_empty(),
        Estado::Conectado => !k.flujo.is_empty() || k.fin,
        _ => false,
    };
    let escribir = k.estado == Estado::Conectado || k.tipo == SOCK_DGRAM;
    (leer, escribir)
}

/// Deja en el fd_set solo los que cumplen; devuelve cuantos quedan.
fn filtrar(set: *mut u8, que: impl Fn(usize) -> bool) -> i32 {
    if set.is_null() {
        return 0;
    }
    // SAFETY: el fd_set del `.exe`: fd_count +0, fd_array (SOCKET) +8, 64.
    unsafe {
        let n = (*(set as *const u32)).min(64) as usize;
        let mut quedan = 0usize;
        for k in 0..n {
            let s = (set.add(8 + 8 * k) as *const u64).read_unaligned();
            if indice(s).is_some_and(&que) {
                (set.add(8 + 8 * quedan) as *mut u64).write_unaligned(s);
                quedan += 1;
            }
        }
        *(set as *mut u32) = quedan as u32;
        quedan as i32
    }
}

/// `select`: quien esta listo YA. Con un plazo y nada listo, cede el turno
/// mientras haya otro hilo; un bucle local no tiene a nadie mas a quien esperar.
pub(crate) extern "win64" fn select(_n: i32, leer: *mut u8, escribir: *mut u8, excepto: *mut u8, plazo: *const u32) -> i32 {
    if !crate::red::arrancado() {
        return error_i(WSANOTINITIALISED);
    }
    let alguno = |set: *mut u8, cual: fn((bool, bool)) -> bool| -> bool {
        if set.is_null() {
            return false;
        }
        // SAFETY: el fd_set del `.exe`.
        unsafe {
            let n = (*(set as *const u32)).min(64) as usize;
            (0..n).any(|k| indice((set.add(8 + 8 * k) as *const u64).read_unaligned()).is_some_and(|i| cual(listo(i))))
        }
    };
    let hay = || alguno(leer, |l| l.0) || alguno(escribir, |l| l.1);
    // Sin plazo (NULL) = esperar para siempre; con plazo 0 = mirar y volver.
    let espera = plazo.is_null() || unsafe { *plazo != 0 || *plazo.add(1) != 0 };
    if !hay() && espera {
        esperar(hay);
    }
    let r = filtrar(leer, |i| listo(i).0);
    let w = filtrar(escribir, |i| listo(i).1);
    filtrar(excepto, |_| false);
    r + w
}

/// **Lo que la red local contesta**, por nombre. `None`: no es suyo.
pub(crate) fn buscar(n: &str) -> Option<u64> {
    use crate::dir;
    Some(match n {
        "socket" => dir!(socket),
        "WSASocketW" => dir!(wsa_socket_w),
        "closesocket" => dir!(closesocket),
        "bind" => dir!(bind),
        "getsockname" => dir!(getsockname),
        "getpeername" => dir!(getpeername),
        "listen" => dir!(listen),
        "connect" => dir!(connect),
        "accept" => dir!(accept),
        "send" => dir!(send),
        "recv" => dir!(recv),
        "sendto" => dir!(sendto),
        "recvfrom" => dir!(recvfrom),
        "ioctlsocket" => dir!(ioctlsocket),
        "setsockopt" => dir!(setsockopt),
        "getsockopt" => dir!(getsockopt),
        "shutdown" => dir!(shutdown),
        "select" => dir!(select),
        _ => return None,
    })
}
