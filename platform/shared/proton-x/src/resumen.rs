//! **Los resumenes (hashes) de CryptoAPI** (tanda 11 de Cyberpunk, 30-09):
//! MD5, SHA-1 y SHA-256, puros. `CryptCreateHash`/`CryptHashData`/
//! `CryptGetHashParam` de la casa (advapi32) juntan los bytes y piden aqui el
//! resumen al final.
//!
//! Los tres son Merkle-Damgard de bloques de 64 bytes: el relleno es el
//! mismo (un 0x80, ceros y el largo en bits, de 8 bytes); MD5 lo pone en
//! little-endian y los SHA en big-endian.

use alloc::vec::Vec;

/// Los bloques de 64 bytes de `datos`, ya con su relleno.
fn bloques(datos: &[u8], largo_be: bool) -> Vec<[u8; 64]> {
    let mut v = datos.to_vec();
    let bits = (datos.len() as u64).wrapping_mul(8);
    v.push(0x80);
    while v.len() % 64 != 56 {
        v.push(0);
    }
    v.extend_from_slice(&if largo_be { bits.to_be_bytes() } else { bits.to_le_bytes() });
    v.chunks_exact(64).map(|c| c.try_into().unwrap_or([0; 64])).collect()
}

const MD5_S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10,
    15, 21, 6, 10, 15, 21,
];

/// Las constantes de MD5: floor(abs(sin(i + 1)) * 2^32).
const MD5_K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1, 0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

pub fn md5(datos: &[u8]) -> [u8; 16] {
    let mut h: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];
    for b in bloques(datos, false) {
        let m: Vec<u32> = b.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let [mut a, mut bb, mut c, mut d] = h;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((bb & c) | (!bb & d), i),
                1 => ((d & bb) | (!d & c), (5 * i + 1) % 16),
                2 => (bb ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (bb | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(MD5_K[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = bb;
            bb = bb.wrapping_add(f.rotate_left(MD5_S[i]));
        }
        for (x, y) in h.iter_mut().zip([a, bb, c, d]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut r = [0u8; 16];
    for (i, x) in h.iter().enumerate() {
        r[4 * i..4 * i + 4].copy_from_slice(&x.to_le_bytes());
    }
    r
}

fn palabras_be(b: &[u8; 64]) -> [u32; 16] {
    let mut w = [0u32; 16];
    for (i, c) in b.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
    }
    w
}

pub fn sha1(datos: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    for b in bloques(datos, true) {
        let mut w = [0u32; 80];
        w[..16].copy_from_slice(&palabras_be(&b));
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut bb, mut c, mut d, mut e] = h;
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i / 20 {
                0 => ((bb & c) | (!bb & d), 0x5A827999),
                1 => (bb ^ c ^ d, 0x6ED9EBA1),
                2 => ((bb & c) | (bb & d) | (c & d), 0x8F1BBCDC),
                _ => (bb ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(wi);
            e = d;
            d = c;
            c = bb.rotate_left(30);
            bb = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, bb, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut r = [0u8; 20];
    for (i, x) in h.iter().enumerate() {
        r[4 * i..4 * i + 4].copy_from_slice(&x.to_be_bytes());
    }
    r
}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub fn sha256(datos: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    for b in bloques(datos, true) {
        let mut w = [0u32; 64];
        w[..16].copy_from_slice(&palabras_be(&b));
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let [a, b, c, d, e, f, g, hh] = v;
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(SHA256_K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), a, b, c, d.wrapping_add(t1), e, f, g];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    let mut r = [0u8; 32];
    for (i, x) in h.iter().enumerate() {
        r[4 * i..4 * i + 4].copy_from_slice(&x.to_be_bytes());
    }
    r
}

/// **HMAC** (RFC 2104) con uno de estos resumenes, de bloque de 64 bytes.
pub fn hmac(resumen: fn(&[u8]) -> Vec<u8>, clave: &[u8], datos: &[u8]) -> Vec<u8> {
    let mut k = if clave.len() > 64 { resumen(clave) } else { clave.to_vec() };
    k.resize(64, 0);
    let mut dentro: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    dentro.extend_from_slice(datos);
    let mut fuera: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    fuera.extend_from_slice(&resumen(&dentro));
    resumen(&fuera)
}
