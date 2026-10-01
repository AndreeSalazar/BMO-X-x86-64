//! **`ws2_32.dll` de la casa** (P4f4, 27-09): la red de Windows EXISTE para
//! que un `.exe` cargue, y contesta lo que Windows contesta cuando no hay red.
//!
//! ** CORREGIDO EL 01-10, por Cyberpunk. Antes WSAStartup decia
//! WSASYSNOTREADY (10091), y eso NO es "Windows sin cable": un Windows sin
//! cable arranca Winsock igual (0, version 2.2) y lo que falla es resolver
//! nombres. Galaxy (REDGalaxy64.dll) llama a WSAStartup y, con el 10091,
//! su Init falla y el juego sale con 0. Ahora:
//!
//! ```text
//!    WSAStartup          0 y el WSADATA de 2.2 ("WinSock 2.0", "Running");
//!                        cuenta las veces, como Windows
//!    WSACleanup          0 si hubo WSAStartup; si no, WSANOTINITIALISED
//!    getaddrinfo         una IPv4 en numeros (127.0.0.1) SE CONTESTA, con
//!                        su ADDRINFO en el monton; un nombre:
//!                        WSAHOST_NOT_FOUND (11001), lo de Windows sin DNS
//!    socket y demas      antes de WSAStartup, WSANOTINITIALISED (10093);
//!                        despues, WSAENETDOWN (10050) y una nota en el
//!                        diario con lo que se pidio: aqui falta la red de
//!                        verdad (ni el bucle local 127.0.0.1, todavia)
//!    WSAGetLastError     el LastError del hilo (en Windows es el mismo)
//!    GetHostNameW        "BMO-X" (lo unico que se sabe sin red)
//!    lo puro (tanda 12)  en [`crate::red_puro`]: no cambia cuando haya red
//!    los ordinales       ws2_32 se importa por NUMERO (#115 WSAStartup,
//!                        #3 closesocket...): [`por_ordinal`]
//! ```
//!
//! Lo que no es Windows, dicho: BMO-X tiene red (`PLAN_RED_TX.md`), pero
//! PROTON-X no la cose todavia; la `std` de Rust y un juego ven "sin red",
//! que es un caso que ya saben llevar.

use alloc::string::String;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::{dir, kernel32};

const WSANOTINITIALISED: u32 = 10093;
const WSAVERNOTSUPPORTED: u32 = 10092;
const WSAENETDOWN: u32 = 10050;
const WSAHOST_NOT_FOUND: u32 = 11001;
const WSATYPE_NOT_FOUND: u32 = 10109;
const WSAEAFNOSUPPORT: u32 = 10047;
const AF_INET: i32 = 2;

/// Cuantas veces se arranco Winsock sin su WSACleanup (Windows cuenta igual).
static ARRANCADO: AtomicU32 = AtomicU32::new(0);

fn arrancado() -> bool {
    ARRANCADO.load(Ordering::Relaxed) > 0
}
const SOCKET_ERROR: i32 = -1;
const INVALID_SOCKET: u64 = u64::MAX;

// -- LA FRONTERA ------------------------------------------------------------------------
//
// Todo lo que pide red pasa por aqui: hoy la casa contesta "sin red"
// (WSAStartup no arranca, y lo demas es lo de Windows sin WSAStartup). El dia
// que PROTON-X cosa la red de BMO-X (el launcher de TITAN++), se cambian
// ESTAS funciones y nada mas: lo puro esta aparte, en `red_puro`.

/// El error de todo lo que necesita red: WSANOTINITIALISED sin WSAStartup;
/// con el, WSAENETDOWN (la red de verdad aun no esta cosida).
fn sin_red() -> u32 {
    let e = if arrancado() { WSAENETDOWN } else { WSANOTINITIALISED };
    kernel32::poner_error(e);
    e
}

/// **El WSADATA de x64** (408 bytes): `wVersion`, `wHighVersion`,
/// `iMaxSockets` y `iMaxUdpDg` (0 desde Winsock 2), `lpVendorInfo` (NULL),
/// `szDescription[257]` en +16 y `szSystemStatus[129]` en +273.
pub(crate) fn wsadata(pedida: u16, d: &mut [u8; 408]) {
    *d = [0; 408];
    let (mayor, menor) = ((pedida & 0xFF) as u8, (pedida >> 8) as u8);
    // La que se usa: la pedida, si no pasa de 2.2.
    let usada = if mayor > 2 || (mayor == 2 && menor > 2) { 0x0202 } else { pedida };
    d[0..2].copy_from_slice(&usada.to_le_bytes());
    d[2..4].copy_from_slice(&0x0202u16.to_le_bytes());
    d[16..16 + 11].copy_from_slice(b"WinSock 2.0");
    d[273..273 + 7].copy_from_slice(b"Running");
}

extern "win64" fn wsa_startup(version: u16, datos: *mut u8) -> i32 {
    if datos.is_null() {
        return 10014; // WSAEFAULT
    }
    // Una version de mayor 0 (menos de 1.0) no se da.
    if version & 0xFF == 0 {
        // SAFETY: un WSADATA del `.exe`.
        unsafe { core::ptr::write_bytes(datos, 0, 408) };
        return WSAVERNOTSUPPORTED as i32;
    }
    let mut d = [0u8; 408];
    wsadata(version, &mut d);
    // SAFETY: un WSADATA del `.exe` (408 bytes en x64).
    unsafe { core::ptr::copy_nonoverlapping(d.as_ptr(), datos, 408) };
    ARRANCADO.fetch_add(1, Ordering::Relaxed);
    0
}

extern "win64" fn wsa_cleanup() -> i32 {
    if !arrancado() {
        kernel32::poner_error(WSANOTINITIALISED);
        return SOCKET_ERROR;
    }
    ARRANCADO.fetch_sub(1, Ordering::Relaxed);
    0
}

extern "win64" fn wsa_get_last_error() -> u32 {
    kernel32::ultimo_error()
}

/// Todas las de socket que devuelven un `int`: SOCKET_ERROR.
extern "win64" fn no_int(_a: u64, _b: u64, _c: u64, _d: u64) -> i32 {
    sin_red();
    SOCKET_ERROR
}

/// Las que devuelven un SOCKET: INVALID_SOCKET.
extern "win64" fn no_socket(_a: u64, _b: u64, _c: u64, _d: u64) -> u64 {
    sin_red();
    INVALID_SOCKET
}

/// `socket`/`WSASocketW`: INVALID_SOCKET, y en el diario QUE se pidio --
/// es lo que dira si hace falta el bucle local (127.0.0.1) o la red entera.
extern "win64" fn socket(af: i32, tipo: i32, proto: i32) -> u64 {
    let e = sin_red();
    if e == WSAENETDOWN {
        crate::diario::nota(&alloc::format!("red: socket({af}, {tipo}, {proto}) negado: la casa aun no tiene red (WSAENETDOWN)"));
        kernel32::poner_error(e);
    }
    INVALID_SOCKET
}

/// Lo que pide `getaddrinfo` en sus pistas: `(flags, familia, tipo, protocolo)`.
fn pistas(h: *const i32) -> (i32, i32, i32, i32) {
    if h.is_null() {
        return (0, 0, 0, 0);
    }
    // SAFETY: un ADDRINFO del `.exe`: cuatro `int` al principio.
    unsafe { (*h, *h.add(1), *h.add(2), *h.add(3)) }
}

/// **Un ADDRINFO de x64 con su sockaddr_in detras**, en un bloque del monton
/// del proceso: `ai_flags` +0, `ai_family` +4, `ai_socktype` +8,
/// `ai_protocol` +12, `ai_addrlen` +16, `ai_canonname` +24, `ai_addr` +32,
/// `ai_next` +40; el sockaddr_in (16 B) en +48. Igual para la A y la W.
pub(crate) fn addrinfo_v4(tipo: i32, proto: i32, ip: [u8; 4], puerto: u16, d: &mut [u8; 64], base: u64) {
    *d = [0; 64];
    d[4..8].copy_from_slice(&AF_INET.to_le_bytes());
    d[8..12].copy_from_slice(&tipo.to_le_bytes());
    d[12..16].copy_from_slice(&proto.to_le_bytes());
    d[16..24].copy_from_slice(&16u64.to_le_bytes());
    d[32..40].copy_from_slice(&(base + 48).to_le_bytes());
    d[48..50].copy_from_slice(&(AF_INET as u16).to_le_bytes());
    d[50..52].copy_from_slice(&puerto.to_be_bytes());
    d[52..56].copy_from_slice(&ip);
}

/// El corazon de `getaddrinfo` y `GetAddrInfoW`, con las cadenas ya leidas.
fn resolver(nodo: Option<String>, servicio: Option<String>, h: *const i32, r: *mut u64) -> i32 {
    if !r.is_null() {
        // SAFETY: un ADDRINFO** del `.exe`.
        unsafe { *r = 0 };
    }
    let fallo = |e: u32| {
        kernel32::poner_error(e);
        e as i32
    };
    if !arrancado() {
        return fallo(WSANOTINITIALISED);
    }
    let (_, familia, tipo, proto) = pistas(h);
    let puerto = match servicio.as_deref() {
        None | Some("") => 0,
        Some(s) => match s.parse::<u16>() {
            Ok(p) => p,
            Err(_) => return fallo(WSATYPE_NOT_FOUND),
        },
    };
    let Some(n) = nodo else { return fallo(WSAHOST_NOT_FOUND) };
    let Some(ip) = bmo_proton_x::direcciones::ipv4(&n) else {
        crate::diario::nota(&alloc::format!("red: getaddrinfo(\"{n}\"): sin DNS (WSAHOST_NOT_FOUND)"));
        return fallo(WSAHOST_NOT_FOUND);
    };
    if familia != 0 && familia != AF_INET {
        return fallo(WSAEAFNOSUPPORT);
    }
    if r.is_null() {
        return fallo(10014); // WSAEFAULT
    }
    let Some(base) = crate::memoria::pedir_del_proceso(64) else { return fallo(8) }; // WSA_NOT_ENOUGH_MEMORY
    let mut d = [0u8; 64];
    addrinfo_v4(tipo, proto, ip, puerto, &mut d, base);
    // SAFETY: un bloque de 64 bytes recien pedido, y el ADDRINFO** del `.exe`.
    unsafe {
        core::ptr::copy_nonoverlapping(d.as_ptr(), base as *mut u8, 64);
        *r = base;
    }
    0
}

/// Una cadena ANSI del `.exe`, o `None` si es NULL.
fn ansi(p: *const u8) -> Option<String> {
    if p.is_null() {
        return None;
    }
    Some(String::from_utf8_lossy(&crate::crt::cadena_c(p as u64)).into_owned())
}

/// Una cadena ancha del `.exe`, o `None` si es NULL.
fn ancha(p: *const u16) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: una cadena del `.exe` acabada en 0.
    Some(String::from_utf16_lossy(&unsafe { crate::user32::utf16(p) }))
}

/// `getaddrinfo` devuelve el error (y tambien lo deja en WSAGetLastError).
extern "win64" fn getaddrinfo(n: *const u8, s: *const u8, h: *const i32, r: *mut u64) -> i32 {
    resolver(ansi(n), ansi(s), h, r)
}

extern "win64" fn get_addr_info_w(n: *const u16, s: *const u16, h: *const i32, r: *mut u64) -> i32 {
    resolver(ancha(n), ancha(s), h, r)
}

/// `freeaddrinfo`: cada eslabon de la cadena, de vuelta al monton.
extern "win64" fn freeaddrinfo(mut p: u64) {
    while p != 0 {
        // SAFETY: un ADDRINFO que dio `resolver` (ai_next en +40).
        let siguiente = unsafe { *((p + 40) as *const u64) };
        crate::memoria::soltar_del_proceso(p);
        p = siguiente;
    }
}

extern "win64" fn get_host_name_w(buf: *mut u16, n: i32) -> i32 {
    let w = [b'B' as u16, b'M' as u16, b'O' as u16, b'-' as u16, b'X' as u16, 0];
    if buf.is_null() || (n as usize) < w.len() {
        kernel32::poner_error(10014); // WSAEFAULT
        return SOCKET_ERROR;
    }
    // SAFETY: el `.exe` da `n` >= 6 caracteres.
    unsafe { core::ptr::copy_nonoverlapping(w.as_ptr(), buf, w.len()) };
    0
}

/// gethostbyname, gethostbyaddr: NULL, sin WSAStartup.
extern "win64" fn no_puntero(_a: u64, _b: u64, _c: u64) -> u64 {
    sin_red();
    0
}

/// getnameinfo devuelve el error, como getaddrinfo.
extern "win64" fn getnameinfo() -> i32 {
    sin_red() as i32
}

/// **Los ordinales de ws2_32** (los de Windows, que no cambian desde
/// Winsock 1.1).
pub(crate) fn por_ordinal(o: u16) -> Option<u64> {
    let n = match o {
        1 => "accept",
        2 => "bind",
        3 => "closesocket",
        4 => "connect",
        5 => "getpeername",
        6 => "getsockname",
        7 => "getsockopt",
        8 => "htonl",
        9 => "htons",
        10 => "ioctlsocket",
        11 => "inet_addr",
        12 => "inet_ntoa",
        13 => "listen",
        14 => "ntohl",
        15 => "ntohs",
        16 => "recv",
        17 => "recvfrom",
        18 => "select",
        19 => "send",
        20 => "sendto",
        21 => "setsockopt",
        22 => "shutdown",
        23 => "socket",
        51 => "gethostbyaddr",
        52 => "gethostbyname",
        57 => "gethostname",
        111 => "WSAGetLastError",
        112 => "WSASetLastError",
        115 => "WSAStartup",
        116 => "WSACleanup",
        151 => "__WSAFDIsSet",
        _ => return None,
    };
    buscar(n)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "WSAStartup" => dir!(wsa_startup),
        "WSACleanup" => dir!(wsa_cleanup),
        "WSAGetLastError" => dir!(wsa_get_last_error),
        "WSASocketW" | "socket" => dir!(socket),
        "accept" => dir!(no_socket),
        "WSADuplicateSocketW" | "WSARecv" | "WSASend" | "bind" | "closesocket" | "connect" | "getpeername" | "getsockname" | "getsockopt" | "ioctlsocket" | "listen" | "recv" | "recvfrom" | "select" | "send" | "sendto"
        | "setsockopt" | "shutdown" | "WSASendTo" | "WSARecvFrom" | "WSAIoctl" | "WSASendDisconnect" | "WSAAddressToStringW" | "WSAStringToAddressW" | "gethostname" => dir!(no_int),
        "gethostbyname" | "gethostbyaddr" => dir!(no_puntero),
        "getnameinfo" | "GetNameInfoW" => dir!(getnameinfo),
        "getaddrinfo" => dir!(getaddrinfo),
        "GetAddrInfoW" => dir!(get_addr_info_w),
        "freeaddrinfo" | "FreeAddrInfoW" => dir!(freeaddrinfo),
        "GetHostNameW" => dir!(get_host_name_w),
        _ => return crate::red_puro::buscar(n),
    })
}
