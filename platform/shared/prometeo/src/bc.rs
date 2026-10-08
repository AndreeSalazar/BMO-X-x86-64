//! **LAS TEXTURAS COMPRIMIDAS: BC1 a BC7** -- un bloque de 4 x 4 texeles,
//! descomprimido a 8 bits por canal (RGBA, R en el byte bajo: lo que lee
//! [`crate::textura`]). Asi guarda un juego casi todas sus texturas
//! (Cyberpunk: color en BC1/BC7, normales en BC5, cielos HDR en BC6H): la
//! 3060 las lee COMPRIMIDAS, y la casa tambien -- se guardan como llegan y
//! se descomprime el bloque al muestrear --. Descomprimirlas al subirlas
//! costaria de 4 a 8 veces su memoria.
//!
//! capa: puro -- bytes que entran y texeles que salen; ni un aparato, ni `std`
//!
//! ```text
//!    BC1  8 B    dos colores 565 y 2 bits por texel (con el modo de alfa
//!                de un bit si c0 <= c1)
//!    BC2  16 B   alfa explicito de 4 bits + un BC1 de cuatro colores
//!    BC3  16 B   alfa interpolado (como BC4) + un BC1 de cuatro colores
//!    BC4  8 B    un canal (R) interpolado; BC5: dos (R, G)
//!    BC6H 16 B   HDR, 14 modos: medios floats, aqui a 8 bits (0..1)
//!    BC7  16 B   8 modos, particiones de 1, 2 o 3 subconjuntos
//! ```
//!
//! **De donde sale:** es un puerto de `bcdec.h` (Sergii Kudlai, 2022, de
//! dominio publico: The Unlicense / MIT), que sigue la referencia de
//! Microsoft (y no las tablas de Khronos, que traen errores). Las tablas de
//! BC6H y BC7 se GENERARON de su fuente (no se copiaron a mano). Como se
//! sabe que esta bien (02-10):
//!
//! ```text
//!    contra bcdec.h compilado en C   20000 bloques: BC6H (con y sin signo,
//!                                    los 14 modos y los reservados), BC1 y
//!                                    BC3, IGUALES bit a bit; y la prueba de
//!                                    abajo, 2000 por formato, en cada build
//!    contra OTRO decodificador       `texture2ddecoder` (PyPI), 3000 por
//!    (texture2ddecoder, de PyPI)     formato: BC4, BC5 y BC7 (los 8 modos)
//!                                    IGUALES; BC1 y BC3 a +-1 (su redondeo)
//!                                    y con el alfa de BC1 opaco donde D3D
//!                                    lo manda transparente; BC6H, distinto
//!                                    solo en su paso de HDR a 8 bits
//! ```
//!
//! [!] Lo que es aproximado, dicho: BC6H se lleva a 8 bits (lo de fuera de
//! 0..1 se sujeta: un cielo HDR se ve, sin su rango); y BC4/BC5 con signo
//! (SNORM) se guardan corridos a 0..1 (`(v + 127) / 254`), porque la
//! textura de la casa no tiene valores negativos todavia.

/// Un formato comprimido por bloques.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bc {
    Bc1,
    Bc2,
    Bc3,
    Bc4,
    Bc4Signo,
    Bc5,
    Bc5Signo,
    Bc6,
    Bc6Signo,
    Bc7,
}

impl Bc {
    /// **El de un `DXGI_FORMAT`** (sus variantes TYPELESS, UNORM y SRGB).
    pub const fn de_dxgi(f: u32) -> Option<Bc> {
        Some(match f {
            70..=72 => Bc::Bc1,
            73..=75 => Bc::Bc2,
            76..=78 => Bc::Bc3,
            79 | 80 => Bc::Bc4,
            81 => Bc::Bc4Signo,
            82 | 83 => Bc::Bc5,
            84 => Bc::Bc5Signo,
            94 | 95 => Bc::Bc6,
            96 => Bc::Bc6Signo,
            97..=99 => Bc::Bc7,
            _ => return None,
        })
    }

    /// Lo que mide un bloque de 4 x 4.
    pub const fn bytes(self) -> usize {
        match self {
            Bc::Bc1 | Bc::Bc4 | Bc::Bc4Signo => 8,
            _ => 16,
        }
    }
}

/// **Un bloque** (`b`, de [`Bc::bytes`] bytes) descomprimido: 16 texeles en
/// orden de fila, cada uno `0xAABBGGRR`.
pub fn bloque(tipo: Bc, b: &[u8]) -> [u32; 16] {
    let mut t = [0u32; 16];
    if b.len() < tipo.bytes() {
        return t;
    }
    match tipo {
        Bc::Bc1 => color(&b[0..8], &mut t, false),
        Bc::Bc2 => {
            color(&b[8..16], &mut t, true);
            for (k, p) in t.iter_mut().enumerate() {
                let a = (b[k / 2] >> (4 * (k % 2))) as u32 & 0xF;
                *p = *p & 0x00FF_FFFF | (a * 17) << 24;
            }
        }
        Bc::Bc3 => {
            color(&b[8..16], &mut t, true);
            let a = canal(&b[0..8], false);
            for (p, a) in t.iter_mut().zip(a) {
                *p = *p & 0x00FF_FFFF | (a as u32) << 24;
            }
        }
        Bc::Bc4 | Bc::Bc4Signo => {
            let r = canal(&b[0..8], tipo == Bc::Bc4Signo);
            for (p, r) in t.iter_mut().zip(r) {
                *p = 0xFF00_0000 | r as u32;
            }
        }
        Bc::Bc5 | Bc::Bc5Signo => {
            let s = tipo == Bc::Bc5Signo;
            let (r, g) = (canal(&b[0..8], s), canal(&b[8..16], s));
            for k in 0..16 {
                t[k] = 0xFF00_0000 | (g[k] as u32) << 8 | r[k] as u32;
            }
        }
        Bc::Bc6 | Bc::Bc6Signo => {
            let h = bc6h(b, tipo == Bc::Bc6Signo);
            for k in 0..16 {
                let c = |i: usize| medio_a_8(h[k][i], tipo == Bc::Bc6Signo) as u32;
                t[k] = 0xFF00_0000 | c(2) << 16 | c(1) << 8 | c(0);
            }
        }
        Bc::Bc7 => bc7(b, &mut t),
    }
    t
}

fn u16_de(b: &[u8], o: usize) -> u32 {
    u16::from_le_bytes([b[o], b[o + 1]]) as u32
}

/// El bloque de color de BC1 (y de BC2/BC3, con `opaco`: siempre cuatro
/// colores). Los redondeos son los de `bcdec` (iguales al calculo en float).
fn color(b: &[u8], t: &mut [u32; 16], opaco: bool) {
    let (c0, c1) = (u16_de(b, 0), u16_de(b, 2));
    let (r0, g0, b0) = (c0 >> 11 & 0x1F, c0 >> 5 & 0x3F, c0 & 0x1F);
    let (r1, g1, b1) = (c1 >> 11 & 0x1F, c1 >> 5 & 0x3F, c1 & 0x1F);
    let px = |r: u32, g: u32, b: u32| 0xFF00_0000 | b << 16 | g << 8 | r;
    let a888 = |r: u32, g: u32, b: u32| px((r * 527 + 23) >> 6, (g * 259 + 33) >> 6, (b * 527 + 23) >> 6);
    let mut c = [a888(r0, g0, b0), a888(r1, g1, b1), 0, 0];
    if c0 > c1 || opaco {
        c[2] = px(((2 * r0 + r1) * 351 + 61) >> 7, ((2 * g0 + g1) * 2763 + 1039) >> 11, ((2 * b0 + b1) * 351 + 61) >> 7);
        c[3] = px(((r0 + 2 * r1) * 351 + 61) >> 7, ((g0 + 2 * g1) * 2763 + 1039) >> 11, ((b0 + 2 * b1) * 351 + 61) >> 7);
    } else {
        c[2] = px(((r0 + r1) * 1053 + 125) >> 8, ((g0 + g1) * 4145 + 1019) >> 11, ((b0 + b1) * 1053 + 125) >> 8);
        c[3] = 0;
    }
    let mut i = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
    for p in t.iter_mut() {
        *p = c[(i & 3) as usize];
        i >>= 2;
    }
}

/// Un canal interpolado (el alfa de BC3, BC4, cada canal de BC5): 16 bytes.
/// Con `signo` (SNORM) se corre a 0..255 (ver la cabecera).
fn canal(b: &[u8], signo: bool) -> [u8; 16] {
    let v = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    let mut a = [0i32; 8];
    if signo {
        a[0] = (b[0] as i8 as i32).max(-127);
        a[1] = (b[1] as i8 as i32).max(-127);
    } else {
        a[0] = b[0] as i32;
        a[1] = b[1] as i32;
    }
    if a[0] > a[1] {
        for k in 1..7 {
            a[k + 1] = ((7 - k as i32) * a[0] + k as i32 * a[1]) / 7;
        }
    } else {
        for k in 1..5 {
            a[k + 1] = ((5 - k as i32) * a[0] + k as i32 * a[1]) / 5;
        }
        (a[6], a[7]) = if signo { (-127, 127) } else { (0, 255) };
    }
    let mut s = [0u8; 16];
    for (k, x) in s.iter_mut().enumerate() {
        let y = a[(v >> (16 + 3 * k) & 7) as usize];
        *x = if signo { ((y + 127) * 255 / 254) as u8 } else { y as u8 };
    }
    s
}

/// Un lector de bits, del menos significativo al mas.
struct Bits(u128);

impl Bits {
    fn de(b: &[u8]) -> Self {
        let mut a = [0u8; 16];
        a.copy_from_slice(&b[..16]);
        Bits(u128::from_le_bytes(a))
    }

    fn leer(&mut self, n: u32) -> u32 {
        let v = (self.0 & ((1u128 << n) - 1)) as u32;
        self.0 >>= n;
        v
    }
}

fn interpolar(a: i32, b: i32, w: i32) -> i32 {
    (a * (64 - w) + b * w + 32) >> 6
}

const PESOS_2: [i32; 4] = [0, 21, 43, 64];
const PESOS_3: [i32; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const PESOS_4: [i32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

fn pesos(bits: u32) -> &'static [i32] {
    match bits {
        2 => &PESOS_2,
        3 => &PESOS_3,
        _ => &PESOS_4,
    }
}

/// El subconjunto del texel `k` y si es un ancla, en una particion de `n`.
fn particion(n: usize, p: usize, k: usize) -> (usize, bool) {
    match n {
        1 => (0, k == 0),
        2 => (PARTICION_2[p].0 as usize >> (2 * k) & 3, PARTICION_2[p].1 >> k & 1 != 0),
        _ => (PARTICION_3[p].0 as usize >> (2 * k) & 3, PARTICION_3[p].1 >> k & 1 != 0),
    }
}

/// **BC7.**
fn bc7(b: &[u8], t: &mut [u32; 16]) {
    // (subconjuntos, bits de particion, bits RGB, bits de alfa) por modo.
    const RGB: [u32; 8] = [4, 6, 5, 7, 5, 7, 7, 5];
    const ALFA: [u32; 8] = [0, 0, 0, 0, 6, 8, 7, 5];
    const CON_P: u8 = 0b1100_1011;
    let mut s = Bits::de(b);
    let mut modo = 0;
    while modo < 8 && s.leer(1) == 0 {
        modo += 1;
    }
    if modo >= 8 {
        *t = [0; 16];
        return;
    }
    let (mut particion_n, mut p, mut rotacion, mut seleccion) = (1, 0, 0, 0);
    if matches!(modo, 0..=3 | 7) {
        particion_n = if modo == 0 || modo == 2 { 3 } else { 2 };
        p = s.leer(if modo == 0 { 4 } else { 6 }) as usize;
    }
    let extremos = particion_n * 2;
    if modo == 4 || modo == 5 {
        rotacion = s.leer(2);
        if modo == 4 {
            seleccion = s.leer(1);
        }
    }
    let mut e = [[0i32; 4]; 6];
    for c in 0..3 {
        for x in e.iter_mut().take(extremos) {
            x[c] = s.leer(RGB[modo]) as i32;
        }
    }
    if ALFA[modo] > 0 {
        for x in e.iter_mut().take(extremos) {
            x[3] = s.leer(ALFA[modo]) as i32;
        }
    }
    if matches!(modo, 0 | 1 | 3 | 6 | 7) {
        for x in e.iter_mut().take(extremos) {
            for c in x.iter_mut() {
                *c <<= 1;
            }
        }
        if modo == 1 {
            let (i, j) = (s.leer(1) as i32, s.leer(1) as i32);
            for c in 0..3 {
                e[0][c] |= i;
                e[1][c] |= i;
                e[2][c] |= j;
                e[3][c] |= j;
            }
        } else if CON_P >> modo & 1 != 0 {
            for x in e.iter_mut().take(extremos) {
                let j = s.leer(1) as i32;
                for c in x.iter_mut() {
                    *c |= j;
                }
            }
        }
    }
    let p_bit = (CON_P >> modo & 1) as u32;
    for x in e.iter_mut().take(extremos) {
        let j = RGB[modo] + p_bit;
        for c in x.iter_mut().take(3) {
            *c <<= 8 - j;
            *c |= *c >> j;
        }
        let j = ALFA[modo] + p_bit;
        x[3] <<= 8 - j;
        x[3] |= x[3] >> j;
        if ALFA[modo] == 0 {
            x[3] = 0xFF;
        }
    }
    let bits1 = match modo {
        0 | 1 => 3,
        6 => 4,
        _ => 2,
    };
    let bits2 = match modo {
        4 => 3,
        5 => 2,
        _ => 0,
    };
    let mut indices = [0u32; 16];
    for (k, i) in indices.iter_mut().enumerate() {
        let (_, ancla) = particion(particion_n, p, k);
        *i = s.leer(bits1 - ancla as u32);
    }
    for k in 0..16 {
        let (sub, _) = particion(particion_n, p, k);
        let (a, z) = (e[2 * sub], e[2 * sub + 1]);
        let i = indices[k] as usize;
        let (mut r, mut g, mut bl, mut al);
        if bits2 == 0 {
            let w = pesos(bits1)[i];
            r = interpolar(a[0], z[0], w);
            g = interpolar(a[1], z[1], w);
            bl = interpolar(a[2], z[2], w);
            al = interpolar(a[3], z[3], w);
        } else {
            let i2 = s.leer(if k == 0 { bits2 - 1 } else { bits2 }) as usize;
            let (wc, wa) = if seleccion == 0 { (pesos(bits1)[i], pesos(bits2)[i2]) } else { (pesos(bits2)[i2], pesos(bits1)[i]) };
            r = interpolar(a[0], z[0], wc);
            g = interpolar(a[1], z[1], wc);
            bl = interpolar(a[2], z[2], wc);
            al = interpolar(a[3], z[3], wa);
        }
        match rotacion {
            1 => core::mem::swap(&mut al, &mut r),
            2 => core::mem::swap(&mut al, &mut g),
            3 => core::mem::swap(&mut al, &mut bl),
            _ => {}
        }
        t[k] = (al as u32) << 24 | (bl as u32) << 16 | (g as u32) << 8 | r as u32;
    }
}

fn extender_signo(v: i32, bits: u32) -> i32 {
    (v << (32 - bits)) >> (32 - bits)
}

fn sin_cuantizar(v: i32, bits: u32, signo: bool) -> i32 {
    if !signo {
        if bits >= 15 {
            v
        } else if v == 0 {
            0
        } else if v == (1 << bits) - 1 {
            0xFFFF
        } else {
            ((v << 16) + 0x8000) >> bits
        }
    } else if bits >= 16 {
        v
    } else {
        let (s, m) = if v < 0 { (true, -v) } else { (false, v) };
        let u = if m == 0 {
            0
        } else if m >= (1 << (bits - 1)) - 1 {
            0x7FFF
        } else {
            ((m << 15) + 0x4000) >> (bits - 1)
        };
        if s {
            -u
        } else {
            u
        }
    }
}

fn acabar(v: i32, signo: bool) -> u16 {
    if !signo {
        ((v * 31) >> 6) as u16
    } else {
        let v = if v < 0 { -(((-v) * 31) >> 5) } else { (v * 31) >> 5 };
        if v < 0 {
            0x8000 | (-v) as u16
        } else {
            v as u16
        }
    }
}

/// **BC6H**: 16 texeles de tres medios floats (R, G, B).
fn bc6h(b: &[u8], signo: bool) -> [[u16; 3]; 16] {
    // Los bits de W (la base) y de las diferencias de R, G y B, por modo.
    const BITS: [[u32; 14]; 4] = [
        [10, 7, 11, 11, 11, 9, 8, 8, 8, 6, 10, 11, 12, 16],
        [5, 6, 5, 4, 4, 5, 6, 5, 5, 6, 10, 9, 8, 4],
        [5, 6, 4, 5, 4, 5, 5, 6, 5, 6, 10, 9, 8, 4],
        [5, 6, 4, 4, 5, 5, 5, 5, 6, 6, 10, 9, 8, 4],
    ];
    let mut s = Bits::de(b);
    let mut codigo = s.leer(2);
    if codigo > 1 {
        codigo |= s.leer(3) << 2;
    }
    let Some(modo) = BC6H_MODOS.iter().position(|m| m.0 as u32 == codigo) else {
        // Los reservados: todo a cero (lo que manda la referencia).
        return [[0; 3]; 16];
    };
    let mut c = [[0i32; 4]; 3];
    let mut p = 0usize;
    for &l in BC6H_MODOS[modo].1 {
        let (canal, i, n, d, alreves) = ((l & 3) as usize, (l >> 2 & 3) as usize, (l >> 4 & 31) as u32, (l >> 9 & 15) as u32, l >> 13 & 1 != 0);
        let mut v = s.leer(n);
        if alreves {
            v = v.reverse_bits() >> (32 - n);
        }
        if canal == 3 {
            p = v as usize;
        } else {
            c[canal][i] |= (v << d) as i32;
        }
    }
    let dos = modo < 10;
    let extremos = if dos { 4 } else { 2 };
    let w = BITS[0][modo];
    if signo {
        for x in c.iter_mut() {
            x[0] = extender_signo(x[0], w);
        }
    }
    if (modo != 9 && modo != 10) || signo {
        for (k, x) in c.iter_mut().enumerate() {
            for v in x.iter_mut().take(extremos).skip(1) {
                *v = extender_signo(*v, BITS[k + 1][modo]);
            }
        }
    }
    if modo != 9 && modo != 10 {
        for x in c.iter_mut() {
            for i in 1..extremos {
                let mut v = (x[i] + x[0]) & ((1 << w) - 1);
                if signo {
                    v = extender_signo(v, w);
                }
                x[i] = v;
            }
        }
    }
    for x in c.iter_mut() {
        for v in x.iter_mut().take(extremos) {
            *v = sin_cuantizar(*v, w, signo);
        }
    }
    let pesos = if dos { &PESOS_3[..] } else { &PESOS_4[..] };
    let mut t = [[0u16; 3]; 16];
    for (k, px) in t.iter_mut().enumerate() {
        let (sub, ancla) = if dos { particion(2, p, k) } else { (0, k == 0) };
        let bits = if dos { 3 } else { 4 } - ancla as u32;
        let w = pesos[s.leer(bits) as usize];
        for (canal, v) in px.iter_mut().enumerate() {
            *v = acabar(interpolar(c[canal][2 * sub], c[canal][2 * sub + 1], w), signo);
        }
    }
    t
}

/// Un medio float a 8 bits, sujeto a 0..1 (lo de fuera, el borde).
fn medio_a_8(h: u16, signo: bool) -> u8 {
    if signo && h & 0x8000 != 0 {
        return 0;
    }
    let (e, m) = ((h >> 10 & 0x1F) as i32, (h & 0x3FF) as u32);
    if e == 0x1F {
        // Infinito: 1; NaN: 0.
        return if m == 0 { 255 } else { 0 };
    }
    // El valor es m' * 2^(e - 25) con m' = 1024 + m (o m si es subnormal).
    let (mm, ex) = if e == 0 { (m, -24) } else { (1024 + m, e - 25) };
    // round(valor * 255), sujeto a 255: con enteros.
    if ex >= 0 {
        return if mm == 0 { 0 } else { 255 };
    }
    let sh = (-ex) as u32;
    let r = (mm as u64 * 255 + (1 << (sh - 1))) >> sh;
    r.min(255) as u8
}

/// Los 14 modos de BC6H: su codigo (2 o 5 bits) y sus lecturas de bits, en
/// orden: `canal | indice << 2 | bits << 4 | desplazamiento << 9 | al reves << 13`
/// (canal 0 r, 1 g, 2 b, 3 la particion). Generada de `bcdec.h`.
const BC6H_MODOS: [(u8, &[u16]); 14] = [
    (0b00000, &[2073, 2074, 2078, 160, 161, 162, 84, 2077, 73, 85, 30, 77, 86, 542, 74, 88, 1054, 92, 1566, 83]),
    (0b00001, &[2585, 2077, 2589, 112, 30, 542, 2074, 113, 2586, 1054, 2073, 114, 1566, 2590, 2078, 100, 73, 101, 77, 102, 74, 104, 108, 83]),
    (0b00010, &[160, 161, 162, 84, 5136, 73, 69, 5137, 30, 77, 70, 5138, 542, 74, 88, 1054, 92, 1566, 83]),
    (0b00110, &[160, 161, 162, 68, 5136, 2077, 73, 85, 5137, 77, 70, 5138, 542, 74, 72, 30, 1054, 76, 2073, 1566, 83]),
    (0b01010, &[160, 161, 162, 68, 5136, 2074, 73, 69, 5137, 30, 77, 86, 5138, 74, 72, 542, 1054, 76, 2078, 1566, 83]),
    (0b01110, &[144, 2074, 145, 2073, 146, 2078, 84, 2077, 73, 85, 30, 77, 86, 542, 74, 88, 1054, 92, 1566, 83]),
    (0b10010, &[128, 2077, 2074, 129, 1054, 2073, 130, 1566, 2078, 100, 73, 85, 30, 77, 86, 542, 74, 104, 108, 83]),
    (0b10110, &[128, 30, 2074, 129, 2585, 2073, 130, 2589, 2078, 84, 2077, 73, 101, 77, 86, 542, 74, 88, 1054, 92, 1566, 83]),
    (0b11010, &[128, 542, 2074, 129, 2586, 2073, 130, 2590, 2078, 84, 2077, 73, 85, 30, 77, 102, 74, 88, 1054, 92, 1566, 83]),
    (0b11110, &[96, 2077, 30, 542, 2074, 97, 2585, 2586, 1054, 2073, 98, 2589, 1566, 2590, 2078, 100, 73, 101, 77, 102, 74, 104, 108, 83]),
    (0b00011, &[160, 161, 162, 164, 165, 166]),
    (0b00111, &[160, 161, 162, 148, 5136, 149, 5137, 150, 5138]),
    (0b01011, &[160, 161, 162, 132, 13344, 133, 13345, 134, 13346]),
    (0b01111, &[160, 161, 162, 68, 13408, 69, 13409, 70, 13410]),
];
/// BPTC de 2 subconjuntos: por particion, el subconjunto de cada texel
/// (2 bits, en orden de fila) y las ANCLAS (los texeles con un bit de indice
/// menos). Generada de `bcdec.h` (la tabla de Microsoft, no la de Khronos).
const PARTICION_2: [(u32, u16); 64] = [
    (0x50505050, 0x8001),
    (0x40404040, 0x8001),
    (0x54545454, 0x8001),
    (0x54505040, 0x8001),
    (0x50404000, 0x8001),
    (0x55545450, 0x8001),
    (0x55545040, 0x8001),
    (0x54504000, 0x8001),
    (0x50400000, 0x8001),
    (0x55555450, 0x8001),
    (0x55544000, 0x8001),
    (0x54400000, 0x8001),
    (0x55555440, 0x8001),
    (0x55550000, 0x8001),
    (0x55555500, 0x8001),
    (0x55000000, 0x8001),
    (0x55150100, 0x8001),
    (0x00004054, 0x0005),
    (0x15010000, 0x0101),
    (0x00405054, 0x0005),
    (0x00004050, 0x0005),
    (0x15050100, 0x0101),
    (0x05010000, 0x0101),
    (0x40505054, 0x8001),
    (0x00404050, 0x0005),
    (0x05010100, 0x0101),
    (0x14141414, 0x0005),
    (0x05141450, 0x0005),
    (0x01155440, 0x0101),
    (0x00555500, 0x0101),
    (0x15014054, 0x0005),
    (0x05414150, 0x0005),
    (0x44444444, 0x8001),
    (0x55005500, 0x8001),
    (0x11441144, 0x0041),
    (0x05055050, 0x0101),
    (0x05500550, 0x0005),
    (0x11114444, 0x0101),
    (0x41144114, 0x8001),
    (0x44111144, 0x8001),
    (0x15055054, 0x0005),
    (0x01055040, 0x0101),
    (0x05041050, 0x0005),
    (0x05455150, 0x0005),
    (0x14414114, 0x0005),
    (0x50050550, 0x8001),
    (0x41411414, 0x8001),
    (0x00141400, 0x0041),
    (0x00041504, 0x0041),
    (0x00105410, 0x0005),
    (0x10541000, 0x0041),
    (0x04150400, 0x0101),
    (0x50410514, 0x8001),
    (0x41051450, 0x8001),
    (0x05415014, 0x0005),
    (0x14054150, 0x0005),
    (0x41050514, 0x8001),
    (0x41505014, 0x8001),
    (0x40011554, 0x8001),
    (0x54150140, 0x8001),
    (0x50505500, 0x8001),
    (0x00555050, 0x0005),
    (0x15151010, 0x0005),
    (0x54540404, 0x8001),
];
/// BPTC de 3 subconjuntos: por particion, el subconjunto de cada texel
/// (2 bits, en orden de fila) y las ANCLAS (los texeles con un bit de indice
/// menos). Generada de `bcdec.h` (la tabla de Microsoft, no la de Khronos).
const PARTICION_3: [(u32, u16); 64] = [
    (0xaa685050, 0x8009),
    (0x6a5a5040, 0x0109),
    (0x5a5a4200, 0x8101),
    (0x5450a0a8, 0x8009),
    (0xa5a50000, 0x8101),
    (0xa0a05050, 0x8009),
    (0x5555a0a0, 0x8009),
    (0x5a5a5050, 0x8101),
    (0xaa550000, 0x8101),
    (0xaa555500, 0x8101),
    (0xaaaa5500, 0x8041),
    (0x90909090, 0x8041),
    (0x94949494, 0x8041),
    (0xa4a4a4a4, 0x8021),
    (0xa9a59450, 0x8009),
    (0x2a0a4250, 0x0109),
    (0xa5945040, 0x8009),
    (0x0a425054, 0x0109),
    (0xa5a5a500, 0x8101),
    (0x55a0a0a0, 0x8009),
    (0xa8a85454, 0x8009),
    (0x6a6a4040, 0x0109),
    (0xa4a45000, 0x8041),
    (0x1a1a0500, 0x0501),
    (0x0050a4a4, 0x0029),
    (0xaaa59090, 0x8101),
    (0x14696914, 0x0141),
    (0x69691400, 0x0441),
    (0xa08585a0, 0x8101),
    (0xaa821414, 0x8021),
    (0x50a4a450, 0x8401),
    (0x6a5a0200, 0x8101),
    (0xa9a58000, 0x8101),
    (0x5090a0a8, 0x8009),
    (0xa8a09050, 0x8009),
    (0x24242424, 0x0421),
    (0x00aa5500, 0x0441),
    (0x24924924, 0x0501),
    (0x24499224, 0x0301),
    (0x50a50a50, 0x8401),
    (0x500aa550, 0x8041),
    (0xaaaa4444, 0x8009),
    (0x66660000, 0x8101),
    (0xa5a0a5a0, 0x8021),
    (0x50a050a0, 0x8009),
    (0x69286928, 0x8041),
    (0x44aaaa44, 0x8041),
    (0x66666600, 0x8101),
    (0xaa444444, 0x8009),
    (0x54a854a8, 0x8009),
    (0x95809580, 0x8021),
    (0x96969600, 0x8021),
    (0xa85454a8, 0x8021),
    (0x80959580, 0x8101),
    (0xaa141414, 0x8021),
    (0x96960000, 0x8401),
    (0xaaaa1414, 0x8021),
    (0xa05050a0, 0x8401),
    (0xa0a5a5a0, 0x8101),
    (0x96000000, 0xa001),
    (0x40804080, 0x8009),
    (0xa9a8a9a8, 0x9001),
    (0xaaaaaa44, 0x8009),
    (0x2a4a5254, 0x0109),
];

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El generador de los bloques (xorshift64), el MISMO que el programa en
    /// C que saco las huellas de `bcdec.h`.
    struct Azar(u64);
    impl Azar {
        fn n(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    /// FNV-1a de 64 bits.
    struct Huella(u64);
    impl Huella {
        fn mas(&mut self, b: &[u8]) {
            for &x in b {
                self.0 ^= x as u64;
                self.0 = self.0.wrapping_mul(0x100_0000_01b3);
            }
        }
    }

    /// **2000 bloques por formato, como `bcdec.h` compilado en C**: la huella
    /// de lo que el da (02-10, `gcc` sobre bcdec 0.985). BC7 con sus 8 modos
    /// por turno; BC6H con sus 14 modos y 2 reservados, con y sin signo.
    #[test]
    fn como_bcdec_en_c() {
        const M6: [u64; 16] = [0, 1, 2, 6, 10, 14, 18, 22, 26, 30, 3, 7, 11, 15, 19, 23];
        let esperadas: [(Bc, u64); 7] = [
            (Bc::Bc1, 0x605548c7cda6b596),
            (Bc::Bc2, 0xbc542ba10784d3bd),
            (Bc::Bc3, 0x03b0173d5c381776),
            (Bc::Bc4, 0x74ddca8aa6bf9129),
            (Bc::Bc5, 0xad8ce5ce23c61c29),
            (Bc::Bc6, 0x7133c2a6b9ceedf3),
            (Bc::Bc7, 0x536d03beb72664dc),
        ];
        let mut az = Azar(0x0BC7_BC6B_3060);
        for (tipo, esperada) in esperadas {
            let mut h = Huella(0xcbf2_9ce4_8422_2325);
            for k in 0..2000u64 {
                let (mut lo, hi) = (az.n(), az.n());
                if tipo == Bc::Bc7 {
                    let m = k % 8;
                    lo = (lo >> (m + 1) << (m + 1)) | 1 << m;
                }
                if tipo == Bc::Bc6 {
                    let m = M6[(k % 16) as usize];
                    let w = if m < 2 { 2 } else { 5 };
                    lo = (lo >> w << w) | m;
                }
                let mut b = [0u8; 16];
                b[..8].copy_from_slice(&lo.to_le_bytes());
                b[8..].copy_from_slice(&hi.to_le_bytes());
                match tipo {
                    Bc::Bc6 => {
                        for signo in [false, true] {
                            for px in bc6h(&b, signo) {
                                for c in px {
                                    h.mas(&c.to_le_bytes());
                                }
                            }
                        }
                    }
                    _ => {
                        for p in bloque(tipo, &b) {
                            match tipo {
                                Bc::Bc4 => h.mas(&[p as u8]),
                                Bc::Bc5 => h.mas(&[p as u8, (p >> 8) as u8]),
                                _ => h.mas(&p.to_le_bytes()),
                            }
                        }
                    }
                }
            }
            assert_eq!(h.0, esperada, "{tipo:?}");
        }
    }

    #[test]
    fn los_formatos_de_dxgi() {
        assert_eq!(Bc::de_dxgi(71), Some(Bc::Bc1));
        assert_eq!(Bc::de_dxgi(72), Some(Bc::Bc1));
        assert_eq!(Bc::de_dxgi(99), Some(Bc::Bc7));
        assert_eq!(Bc::de_dxgi(95), Some(Bc::Bc6));
        assert_eq!(Bc::de_dxgi(96), Some(Bc::Bc6Signo));
        assert_eq!(Bc::de_dxgi(28), None);
        assert_eq!((Bc::Bc1.bytes(), Bc::Bc4.bytes(), Bc::Bc7.bytes()), (8, 8, 16));
    }

    #[test]
    fn el_medio_float_a_8_bits() {
        // 0, 0.5, 1, 2 (sujeto), infinito, NaN, negativo con signo.
        assert_eq!(medio_a_8(0x0000, false), 0);
        assert_eq!(medio_a_8(0x3800, false), 128);
        assert_eq!(medio_a_8(0x3C00, false), 255);
        assert_eq!(medio_a_8(0x4000, false), 255);
        assert_eq!(medio_a_8(0x7C00, false), 255);
        assert_eq!(medio_a_8(0x7E00, false), 0);
        assert_eq!(medio_a_8(0xBC00, true), 0);
        // El mas chico que no es cero, aun 0.
        assert_eq!(medio_a_8(0x0001, false), 0);
    }
}
