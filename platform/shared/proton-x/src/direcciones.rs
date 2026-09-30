//! **Las direcciones IP y Base64, puras** (tanda 12 de Cyberpunk, 30-09):
//! lo de ws2_32 y crypt32 que no necesita red.
//!
//! ```text
//!    ipv4 / ipv6         leer como inet_pton (solo la forma decimal con
//!                        puntos; en IPv6, "::" una vez y un IPv4 al final)
//!    ipv4_texto / ipv6_texto   escribir como inet_ntop: la RFC 5952 (la
//!                        racha de ceros mas larga, de dos o mas, se vuelve
//!                        "::"; la primera si empatan; en minusculas) y las
//!                        ::ffff:a.b.c.d con su IPv4
//!    base64 / de_base64  la de la RFC 4648 (con '='); leer salta espacios
//!                        y saltos de linea, como CryptStringToBinary
//! ```

use alloc::string::String;
use alloc::vec::Vec;

/// "a.b.c.d": cuatro numeros de 0 a 255, sin ceros delante (como
/// inet_pton).
pub fn ipv4(s: &str) -> Option<[u8; 4]> {
    let mut r = [0u8; 4];
    let mut n = 0;
    for p in s.split('.') {
        if n == 4 || p.is_empty() || p.len() > 3 || !p.bytes().all(|c| c.is_ascii_digit()) || (p.len() > 1 && p.starts_with('0')) {
            return None;
        }
        r[n] = p.parse::<u16>().ok().filter(|&v| v < 256)? as u8;
        n += 1;
    }
    (n == 4).then_some(r)
}

pub fn ipv6(s: &str) -> Option<[u8; 16]> {
    let (izq, der, doble) = match s.find("::") {
        Some(i) => (&s[..i], &s[i + 2..], true),
        None => (s, "", false),
    };
    if doble && der.contains("::") {
        return None;
    }
    let grupos = |t: &str| -> Option<Vec<u16>> {
        if t.is_empty() {
            return Some(Vec::new());
        }
        let partes: Vec<&str> = t.split(':').collect();
        let mut v = Vec::new();
        for (i, p) in partes.iter().enumerate() {
            if i == partes.len() - 1 && p.contains('.') {
                let a = ipv4(p)?;
                v.push(u16::from_be_bytes([a[0], a[1]]));
                v.push(u16::from_be_bytes([a[2], a[3]]));
            } else {
                if p.is_empty() || p.len() > 4 {
                    return None;
                }
                v.push(u16::from_str_radix(p, 16).ok()?);
            }
        }
        Some(v)
    };
    let a = grupos(izq)?;
    let b = grupos(der)?;
    // Un IPv4 solo puede ir al final.
    if doble && izq.contains('.') {
        return None;
    }
    let total = a.len() + b.len();
    if (doble && total > 7) || (!doble && total != 8) {
        return None;
    }
    let mut g = a;
    g.resize(8 - b.len(), 0);
    g.extend(b);
    let mut r = [0u8; 16];
    for (i, x) in g.iter().enumerate() {
        r[2 * i..2 * i + 2].copy_from_slice(&x.to_be_bytes());
    }
    Some(r)
}

pub fn ipv4_texto(a: [u8; 4]) -> String {
    alloc::format!("{}.{}.{}.{}", a[0], a[1], a[2], a[3])
}

pub fn ipv6_texto(a: [u8; 16]) -> String {
    let g: Vec<u16> = (0..8).map(|i| u16::from_be_bytes([a[2 * i], a[2 * i + 1]])).collect();
    // ::ffff:a.b.c.d (IPv4 mapeada).
    if g[..5].iter().all(|&x| x == 0) && g[5] == 0xFFFF {
        return alloc::format!("::ffff:{}", ipv4_texto([a[12], a[13], a[14], a[15]]));
    }
    // La racha de ceros mas larga (>= 2), la primera si empatan.
    let (mut mejor, mut largo) = (8, 0);
    let mut i = 0;
    while i < 8 {
        if g[i] == 0 {
            let j = (i..8).find(|&j| g[j] != 0).unwrap_or(8);
            if j - i > largo && j - i >= 2 {
                mejor = i;
                largo = j - i;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    let hex = |v: &[u16]| v.iter().map(|x| alloc::format!("{x:x}")).collect::<Vec<_>>().join(":");
    if largo == 0 {
        return hex(&g);
    }
    alloc::format!("{}::{}", hex(&g[..mejor]), hex(&g[mejor + largo..]))
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(d: &[u8]) -> String {
    let mut s = String::new();
    for c in d.chunks(3) {
        let v = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { B64[(v >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// Base64 a bytes: salta espacios, tabuladores y saltos de linea; `None`
/// si hay otra cosa o el largo no cuadra.
pub fn de_base64(s: &[u8]) -> Option<Vec<u8>> {
    let limpio: Vec<u8> = s.iter().copied().filter(|c| !matches!(c, b' ' | b'\t' | b'\r' | b'\n')).collect();
    if limpio.len() % 4 != 0 {
        return None;
    }
    let mut r = Vec::new();
    for (k, c) in limpio.chunks(4).enumerate() {
        let ultimo = k == limpio.len() / 4 - 1;
        let mut v = 0u32;
        let mut n = 0;
        for (i, &x) in c.iter().enumerate() {
            let d = if x == b'=' {
                if !ultimo || i < 2 {
                    return None;
                }
                0
            } else {
                if n < i {
                    return None; // algo despues de un '='
                }
                n += 1;
                B64.iter().position(|&b| b == x)? as u32
            };
            v = v << 6 | d;
        }
        let bytes = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
        r.extend_from_slice(&bytes[..n - 1]);
    }
    Some(r)
}
