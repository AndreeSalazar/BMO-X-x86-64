//! **Lo puro de `ws2_32.dll`** (tanda 12 de Cyberpunk, 30-09): lo que en
//! Windows tampoco pide red ni WSAStartup, de verdad.
//!
//! ```text
//!    htonl htons ntohl ntohs       el orden de la red
//!    inet_addr inet_ntoa           IPv4 a numero y de vuelta
//!    inet_pton inet_ntop           IPv4 e IPv6 (bmo_proton_x::direcciones)
//!    __WSAFDIsSet WSASetLastError  el fd_set y el error del hilo
//! ```
//!
//! AISLADO de [`crate::red`] a proposito: la red de verdad (el launcher de
//! TITAN++) se cose alli, en su frontera; esto se queda igual.

use core::cell::UnsafeCell;

use bmo_proton_x::direcciones;

use crate::{dir, kernel32};

const SOCKET_ERROR: i32 = -1;

extern "win64" fn wsa_set_last_error(e: u32) {
    kernel32::poner_error(e);
}

extern "win64" fn htonl(v: u32) -> u32 {
    v.swap_bytes()
}

extern "win64" fn htons(v: u16) -> u16 {
    v.swap_bytes()
}

const AF_INET: i32 = 2;
const AF_INET6: i32 = 23;
const WSAEAFNOSUPPORT: u32 = 10047;
const ERROR_INVALID_PARAMETER: u32 = 87;

fn cadena(p: *const u8) -> Option<alloc::string::String> {
    if p.is_null() {
        return None;
    }
    let b = crate::crt::cadena_c(p as u64);
    core::str::from_utf8(&b).ok().map(alloc::string::String::from)
}

/// `inet_addr`: la IPv4 en el orden de la red; INADDR_NONE si no lo es.
extern "win64" fn inet_addr(p: *const u8) -> u32 {
    cadena(p).and_then(|s| direcciones::ipv4(&s)).map_or(u32::MAX, u32::from_ne_bytes)
}

/// El bufer de inet_ntoa (uno, como el de cada hilo en Windows: los hilos
/// de la casa se turnan).
struct Bufer(UnsafeCell<[u8; 16]>);
// SAFETY: una tarea, hilos cooperativos.
unsafe impl Sync for Bufer {}
static NTOA: Bufer = Bufer(UnsafeCell::new([0; 16]));

extern "win64" fn inet_ntoa(a: u32) -> *const u8 {
    let t = direcciones::ipv4_texto(a.to_ne_bytes());
    // SAFETY: ver `Bufer`; "255.255.255.255" y su 0 caben en 16.
    let b = unsafe { &mut *NTOA.0.get() };
    b.fill(0);
    b[..t.len()].copy_from_slice(t.as_bytes());
    b.as_ptr()
}

extern "win64" fn inet_pton(familia: i32, s: *const u8, d: *mut u8) -> i32 {
    let Some(t) = cadena(s) else {
        kernel32::poner_error(10014); // WSAEFAULT
        return SOCKET_ERROR;
    };
    let v: Option<alloc::vec::Vec<u8>> = match familia {
        AF_INET => direcciones::ipv4(&t).map(|a| a.to_vec()),
        AF_INET6 => direcciones::ipv6(&t).map(|a| a.to_vec()),
        _ => {
            kernel32::poner_error(WSAEAFNOSUPPORT);
            return SOCKET_ERROR;
        }
    };
    let Some(v) = v else { return 0 };
    // SAFETY: un in_addr o in6_addr del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(v.as_ptr(), d, v.len()) };
    1
}

extern "win64" fn inet_ntop(familia: i32, a: *const u8, buf: *mut u8, n: usize) -> *const u8 {
    // SAFETY: un in_addr (4) o in6_addr (16) del `.exe`.
    let t = match familia {
        AF_INET if !a.is_null() => direcciones::ipv4_texto(unsafe { *(a as *const [u8; 4]) }),
        AF_INET6 if !a.is_null() => direcciones::ipv6_texto(unsafe { *(a as *const [u8; 16]) }),
        AF_INET | AF_INET6 => {
            kernel32::poner_error(ERROR_INVALID_PARAMETER);
            return core::ptr::null();
        }
        _ => {
            kernel32::poner_error(WSAEAFNOSUPPORT);
            return core::ptr::null();
        }
    };
    if buf.is_null() || n <= t.len() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return core::ptr::null();
    }
    // SAFETY: `n` bytes del `.exe`.
    unsafe {
        core::ptr::copy_nonoverlapping(t.as_ptr(), buf, t.len());
        *buf.add(t.len()) = 0;
    }
    buf
}

/// `__WSAFDIsSet(s, fd_set*)`: fd_count +0, fd_array (SOCKET) +8.
extern "win64" fn wsa_fd_is_set(s: u64, set: *const u8) -> i32 {
    if set.is_null() {
        return 0;
    }
    // SAFETY: el fd_set del `.exe` (hasta 64 SOCKET).
    unsafe {
        let n = (*(set as *const u32)).min(64) as usize;
        (0..n).any(|i| (set.add(8 + 8 * i) as *const u64).read_unaligned() == s) as i32
    }
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "WSASetLastError" => dir!(wsa_set_last_error),
        "htonl" | "ntohl" => dir!(htonl),
        "htons" | "ntohs" => dir!(htons),
        "inet_addr" => dir!(inet_addr),
        "inet_ntoa" => dir!(inet_ntoa),
        "inet_pton" => dir!(inet_pton),
        "inet_ntop" => dir!(inet_ntop),
        "__WSAFDIsSet" => dir!(wsa_fd_is_set),
        _ => return None,
    })
}
