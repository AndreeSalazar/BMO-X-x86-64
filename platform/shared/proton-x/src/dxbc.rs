//! **El contenedor DXBC y su huella** (P3b2, 27-09).
//!
//! Todo lo compilado para D3D -- un sombreador DXIL, una root signature
//! serializada -- va dentro del mismo sobre:
//!
//! ```text
//!    "DXBC" | huella (16) | version (1, 0) | medida total | n partes
//!    | desplazamiento de cada parte | partes: FourCC | medida | bytes
//! ```
//!
//! La HUELLA es un MD5 con un final propio (el de Microsoft): el MD5 normal
//! sobre todo lo que va detras de la huella (desde el byte 20), pero el ultimo
//! bloque no lleva la longitud al final como en el MD5 de siempre: lleva la
//! longitud en bits DELANTE (primeros 4 bytes) y `(bits >> 2) | 1` en los
//! ultimos 4. Asi la describen quienes la han reproducido en abierto (Wine,
//! vkd3d, DXVK: `dxbc_checksum`). Con ella, un blob de la casa es IGUAL, byte
//! a byte, al que escribe el compilador de Microsoft: el banco lo compara con
//! los de `dxc`.

use alloc::vec::Vec;

/// La tabla de senos del MD5 (RFC 1321): `floor(abs(sin(i + 1)) * 2^32)`.
const K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1,
    0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453,
    0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942,
    0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
    0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d,
    0x85845dd1, 0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

/// Las rotaciones de cada paso (RFC 1321).
const R: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11,
    16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// Un bloque de 64 bytes por el MD5.
fn bloque(s: &mut [u32; 4], b: &[u8; 64]) {
    let m: [u32; 16] = core::array::from_fn(|i| u32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]));
    let [mut a, mut bb, mut c, mut d] = *s;
    for i in 0..64 {
        let (f, g) = match i / 16 {
            0 => ((bb & c) | (!bb & d), i),
            1 => ((d & bb) | (!d & c), (5 * i + 1) % 16),
            2 => (bb ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (bb | !d), (7 * i) % 16),
        };
        let t = d;
        d = c;
        c = bb;
        bb = bb.wrapping_add(a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]).rotate_left(R[i]));
        a = t;
    }
    s[0] = s[0].wrapping_add(a);
    s[1] = s[1].wrapping_add(bb);
    s[2] = s[2].wrapping_add(c);
    s[3] = s[3].wrapping_add(d);
}

/// **La huella de un contenedor DXBC**: sobre `d[20..]` (todo menos la
/// magia y la propia huella).
pub fn huella(d: &[u8]) -> [u8; 16] {
    let datos = d.get(20..).unwrap_or(&[]);
    let mut s = [0x6745_2301u32, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    let enteros = datos.len() / 64 * 64;
    for trozo in datos[..enteros].chunks_exact(64) {
        bloque(&mut s, trozo.try_into().unwrap_or(&[0; 64]));
    }
    let resto = &datos[enteros..];
    let bits = (datos.len() as u32).wrapping_mul(8);
    let mut b = [0u8; 64];
    if 64 - (resto.len() + 1) < 8 {
        // No caben los 8 bytes del final: este bloque se cierra con ceros y
        // la longitud va sola en el siguiente.
        b[..resto.len()].copy_from_slice(resto);
        b[resto.len()] = 0x80;
        bloque(&mut s, &b);
        b = [0; 64];
    } else {
        // La longitud DELANTE: los datos corren 4 bytes.
        b[4..4 + resto.len()].copy_from_slice(resto);
        b[4 + resto.len()] = 0x80;
    }
    b[..4].copy_from_slice(&bits.to_le_bytes());
    b[60..].copy_from_slice(&((bits >> 2) | 1).to_le_bytes());
    bloque(&mut s, &b);
    let mut h = [0u8; 16];
    for (i, v) in s.iter().enumerate() {
        h[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
    }
    h
}

/// **Un contenedor DXBC** con estas partes, y su huella puesta.
pub fn contenedor(partes: &[([u8; 4], &[u8])]) -> Vec<u8> {
    let cabecera = 32 + 4 * partes.len();
    let total = cabecera + partes.iter().map(|(_, p)| 8 + p.len()).sum::<usize>();
    let mut d = Vec::with_capacity(total);
    d.extend_from_slice(b"DXBC");
    d.extend_from_slice(&[0; 16]);
    d.extend_from_slice(&1u32.to_le_bytes());
    d.extend_from_slice(&(total as u32).to_le_bytes());
    d.extend_from_slice(&(partes.len() as u32).to_le_bytes());
    let mut o = cabecera;
    for (_, p) in partes {
        d.extend_from_slice(&(o as u32).to_le_bytes());
        o += 8 + p.len();
    }
    for (cc, p) in partes {
        d.extend_from_slice(cc);
        d.extend_from_slice(&(p.len() as u32).to_le_bytes());
        d.extend_from_slice(p);
    }
    let h = huella(&d);
    d[4..20].copy_from_slice(&h);
    d
}

/// Las partes de un contenedor: (FourCC, bytes). `None` si no cuadra.
pub fn partes(d: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    let u = |o: usize| d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    if d.get(..4) != Some(b"DXBC") || u(24)? > d.len() {
        return None;
    }
    let n = u(28)?;
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let o = u(32 + 4 * i)?;
        let cc: [u8; 4] = d.get(o..o + 4)?.try_into().ok()?;
        let tam = u(o + 4)?;
        v.push((cc, d.get(o + 8..o + 8 + tam)?));
    }
    Some(v)
}
