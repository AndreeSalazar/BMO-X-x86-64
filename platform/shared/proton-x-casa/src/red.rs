//! **`ws2_32.dll` de la casa** (P4f4, 27-09): la red de Windows EXISTE para
//! que un `.exe` cargue, y contesta lo que Windows contesta cuando no hay red.
//!
//! ```text
//!    WSAStartup          WSASYSNOTREADY (10091): "el subsistema de red no
//!                        esta listo" -- lo que Windows dice sin red
//!    todo lo demas       SOCKET_ERROR / INVALID_SOCKET y WSAGetLastError =
//!                        WSANOTINITIALISED (10093); getaddrinfo lo devuelve
//!    WSAGetLastError     el LastError del hilo (en Windows es el mismo)
//!    GetHostNameW        "BMO-X" (lo unico que se sabe sin red)
//! ```
//!
//! Lo que no es Windows, dicho: BMO-X tiene red (`PLAN_RED_TX.md`), pero
//! PROTON-X no la cose todavia; la `std` de Rust y un juego ven "sin red",
//! que es un caso que ya saben llevar.

use crate::{dir, kernel32};

const WSASYSNOTREADY: u32 = 10091;
const WSANOTINITIALISED: u32 = 10093;
const SOCKET_ERROR: i32 = -1;
const INVALID_SOCKET: u64 = u64::MAX;

extern "win64" fn wsa_startup(_version: u16, datos: *mut u8) -> i32 {
    if !datos.is_null() {
        // WSADATA de x64 (408 bytes): a ceros; nadie lo mira si falla.
        // SAFETY: un WSADATA del `.exe`.
        unsafe { core::ptr::write_bytes(datos, 0, 408) };
    }
    WSASYSNOTREADY as i32
}

extern "win64" fn wsa_cleanup() -> i32 {
    kernel32::poner_error(WSANOTINITIALISED);
    SOCKET_ERROR
}

extern "win64" fn wsa_get_last_error() -> u32 {
    kernel32::ultimo_error()
}

/// Todas las de socket que devuelven un `int`: SOCKET_ERROR.
extern "win64" fn no_int(_a: u64, _b: u64, _c: u64, _d: u64) -> i32 {
    kernel32::poner_error(WSANOTINITIALISED);
    SOCKET_ERROR
}

/// Las que devuelven un SOCKET: INVALID_SOCKET.
extern "win64" fn no_socket(_a: u64, _b: u64, _c: u64, _d: u64) -> u64 {
    kernel32::poner_error(WSANOTINITIALISED);
    INVALID_SOCKET
}

/// `getaddrinfo` devuelve el error (no lo deja en LastError).
extern "win64" fn getaddrinfo(_n: u64, _s: u64, _h: u64, r: *mut u64) -> i32 {
    if !r.is_null() {
        // SAFETY: un ADDRINFOA** del `.exe`.
        unsafe { *r = 0 };
    }
    WSANOTINITIALISED as i32
}

extern "win64" fn freeaddrinfo(_p: u64) {}

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

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "WSAStartup" => dir!(wsa_startup),
        "WSACleanup" => dir!(wsa_cleanup),
        "WSAGetLastError" => dir!(wsa_get_last_error),
        "WSASocketW" | "socket" | "accept" => dir!(no_socket),
        "WSADuplicateSocketW" | "WSARecv" | "WSASend" | "bind" | "closesocket" | "connect" | "getpeername" | "getsockname" | "getsockopt" | "ioctlsocket" | "listen" | "recv" | "recvfrom" | "select" | "send" | "sendto"
        | "setsockopt" | "shutdown" => dir!(no_int),
        "getaddrinfo" | "GetAddrInfoW" => dir!(getaddrinfo),
        "freeaddrinfo" | "FreeAddrInfoW" => dir!(freeaddrinfo),
        "GetHostNameW" => dir!(get_host_name_w),
        _ => return None,
    })
}
