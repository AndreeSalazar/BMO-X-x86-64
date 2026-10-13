//! **ESCRIBIR JPEG** (baseline, Huffman, 8 bits, 4:2:0) -- cada fotograma de
//! un video de FRAPS-X (`bmo-video`: MJPEG en AVI) y, si se quiere, una
//! captura que pesa diez veces menos que su PNG.
//!
//! generacion: nieto -- recibe pixeles y un bufer, y devuelve bytes o un motivo
//!
//! ```text
//!    los pixeles   0x00RRGGBB (o 0x00BBGGRR con `rgb`), con su paso de fila
//!    el color      YCbCr de JFIF, en punto fijo de 16 bits
//!    el muestreo   Y 2x2, Cb y Cr 1x1 (4:2:0): un MCU de 16x16, seis bloques;
//!                  el borde, repitiendo el ultimo pixel
//!    la DCT        entera: la MISMA tabla de cosenos de 13 bits del lector
//!                  (`jpeg::COS`), acumulada en i64, sin flotantes
//!    las tablas    las del Anexo K de la norma (cuantizacion escalada por la
//!                  calidad como libjpeg; las cuatro de Huffman tipicas)
//!    el fichero    SOI, APP0 (JFIF), DQT, SOF0, DHT, SOS, los datos, EOI
//! ```
//!
//! El juez es el lector de la casa (`jpeg::decodificar`): lo que escribe esto
//! se lee de vuelta y se compara pixel a pixel (pruebas de abajo). Y fuera de
//! `cargo test`, Pillow y ffmpeg lo abren (dicho en el commit que lo trajo).

use crate::Error;

/// El orden zig-zag: la posicion natural del coeficiente `k`-esimo.
const ZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14,
    21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53,
    60, 61, 54, 47, 55, 62, 63,
];

/// `round(cos((2x+1) u pi / 16) * c(u) * 2^13)`: la del lector.
const COS: [[i64; 8]; 8] = [
    [5793, 8035, 7568, 6811, 5793, 4551, 3135, 1598],
    [5793, 6811, 3135, -1598, -5793, -8035, -7568, -4551],
    [5793, 4551, -3135, -8035, -5793, 1598, 7568, 6811],
    [5793, 1598, -7568, -4551, 5793, 6811, -3135, -8035],
    [5793, -1598, -7568, 4551, 5793, -6811, -3135, 8035],
    [5793, -4551, -3135, 8035, -5793, -1598, 7568, -6811],
    [5793, -6811, 3135, 1598, -5793, 8035, -7568, 4551],
    [5793, -8035, 7568, -6811, 5793, -4551, 3135, -1598],
];

/// Anexo K.1: la de luminancia, en orden natural.
const Q_LUMA: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];
/// Anexo K.1: la de crominancia.
const Q_CROMA: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99, 47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

/// Anexo K.3: las cuatro tablas de Huffman tipicas (cuantos codigos de cada
/// longitud 1..16, y sus simbolos).
const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_CROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_VALORES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_LUMA: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1,
    0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47,
    0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86,
    0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba,
    0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3,
    0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
];
const AC_CROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CROMA: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52,
    0xf0, 0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46,
    0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84,
    0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8,
    0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3,
    0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
];

/// El lado mas grande que se escribe (el del lector).
pub const LADO_MAX: u32 = crate::LADO_MAX;

/// **Lo mas que puede medir** un JPEG de `ancho` x `alto` (cualquier calidad,
/// cualquier imagen): las cabeceras y, por pixel, 10 bytes (lo peor de 26
/// bits por coeficiente, con cada `FF` doblado). Lo normal es 20 veces menos.
pub const fn cota(ancho: u32, alto: u32) -> usize {
    let (w, h) = (ancho.div_ceil(16) as usize * 16, alto.div_ceil(16) as usize * 16);
    1024 + 10 * w * h
}

/// Una tabla de Huffman lista para escribir: `(codigo, longitud)` de cada simbolo.
struct Codigos([(u16, u8); 256]);

impl Codigos {
    const fn de(bits: &[u8; 16], valores: &[u8]) -> Codigos {
        let mut t = [(0u16, 0u8); 256];
        let (mut codigo, mut k, mut l) = (0u16, 0usize, 0usize);
        while l < 16 {
            let mut n = 0;
            while n < bits[l] {
                t[valores[k] as usize] = (codigo, l as u8 + 1);
                codigo += 1;
                k += 1;
                n += 1;
            }
            codigo <<= 1;
            l += 1;
        }
        Codigos(t)
    }
}

const DC_L: Codigos = Codigos::de(&DC_LUMA_BITS, &DC_VALORES);
const DC_C: Codigos = Codigos::de(&DC_CROMA_BITS, &DC_VALORES);
const AC_L: Codigos = Codigos::de(&AC_LUMA_BITS, &AC_LUMA);
const AC_C: Codigos = Codigos::de(&AC_CROMA_BITS, &AC_CROMA);

/// Las tablas de cuantizacion de una calidad (1..100), como libjpeg.
fn tablas(calidad: u8) -> ([u16; 64], [u16; 64]) {
    let q = calidad.clamp(1, 100) as u32;
    let escala = if q < 50 { 5000 / q } else { 200 - 2 * q };
    let una = |base: &[u16; 64]| core::array::from_fn(|k| ((base[k] as u32 * escala + 50) / 100).clamp(1, 255) as u16);
    (una(&Q_LUMA), una(&Q_CROMA))
}

/// El escritor de bytes, con el bit de entropia y el `FF 00`.
struct Salida<'a> {
    b: &'a mut [u8],
    n: usize,
    acum: u32,
    cuantos: u32,
    lleno: bool,
}

impl Salida<'_> {
    fn byte(&mut self, v: u8) {
        match self.b.get_mut(self.n) {
            Some(x) => {
                *x = v;
                self.n += 1;
            }
            None => self.lleno = true,
        }
    }
    fn bytes(&mut self, v: &[u8]) {
        for &x in v {
            self.byte(x);
        }
    }
    fn be16(&mut self, v: u16) {
        self.bytes(&v.to_be_bytes());
    }
    /// `n` bits de `v` (los de abajo), del mas alto al mas bajo.
    fn bits(&mut self, v: u32, n: u32) {
        if n == 0 {
            return;
        }
        self.acum = self.acum << n | (v & ((1 << n) - 1));
        self.cuantos += n;
        while self.cuantos >= 8 {
            let b = (self.acum >> (self.cuantos - 8)) as u8;
            self.byte(b);
            if b == 0xFF {
                self.byte(0);
            }
            self.cuantos -= 8;
        }
        self.acum &= (1 << self.cuantos) - 1;
    }
    /// Lo que quede, rellenado con unos (la norma).
    fn vaciar(&mut self) {
        if self.cuantos > 0 {
            let n = 8 - self.cuantos;
            self.bits((1 << n) - 1, n);
        }
    }
}

/// La categoria de un valor (cuantos bits) y sus bits (de un negativo, el
/// complemento a uno, como pide la norma).
fn categoria(v: i32) -> (u32, u32) {
    let a = v.unsigned_abs();
    let n = 32 - a.leading_zeros();
    let bits = if v < 0 { (v - 1) as u32 } else { v as u32 };
    (n, bits & ((1u32 << n) - 1).max(0))
}

/// La DCT de un bloque (`-128..127`) y su cuantizacion, en zig-zag.
fn dct(bloque: &[i32; 64], q: &[u16; 64]) -> [i32; 64] {
    // Filas: t[y][u] = sum_x f[y][x] * COS[x][u].
    let mut t = [0i64; 64];
    for y in 0..8 {
        for u in 0..8 {
            let mut s = 0i64;
            for x in 0..8 {
                s += bloque[8 * y + x] as i64 * COS[x][u];
            }
            t[8 * y + u] = s;
        }
    }
    // Columnas, el 1/4 de la norma y los 2^26 de los cosenos, y la
    // cuantizacion: round(sum / (2^28 * q)), simetrico.
    let mut z = [0i32; 64];
    for k in 0..64 {
        let natural = ZIGZAG[k] as usize;
        let (v, u) = (natural / 8, natural % 8);
        let mut s = 0i64;
        for y in 0..8 {
            s += t[8 * y + u] * COS[y][v];
        }
        let d = (q[natural] as i64) << 28;
        z[k] = (if s >= 0 { (s + d / 2) / d } else { -((-s + d / 2) / d) }) as i32;
    }
    z
}

/// Un bloque: su DC contra el anterior y sus AC, por la entropia.
fn bloque(s: &mut Salida, z: &[i32; 64], antes: &mut i32, dc: &Codigos, ac: &Codigos) {
    let (n, b) = categoria(z[0] - *antes);
    *antes = z[0];
    let (c, l) = dc.0[n as usize];
    s.bits(c as u32, l as u32);
    s.bits(b, n);
    let mut ceros = 0;
    for &v in &z[1..] {
        if v == 0 {
            ceros += 1;
            continue;
        }
        while ceros >= 16 {
            let (c, l) = ac.0[0xF0];
            s.bits(c as u32, l as u32);
            ceros -= 16;
        }
        let (n, b) = categoria(v);
        let (c, l) = ac.0[(ceros << 4 | n) as usize];
        s.bits(c as u32, l as u32);
        s.bits(b, n);
        ceros = 0;
    }
    if ceros > 0 {
        let (c, l) = ac.0[0x00];
        s.bits(c as u32, l as u32);
    }
}

/// `Y`, `Cb` y `Cr` (0..255) de un pixel, en punto fijo de 16 bits (JFIF).
fn ycc(p: u32, rgb: bool) -> (i32, i32, i32) {
    let (r, g, b) = if rgb { (p & 0xFF, p >> 8 & 0xFF, p >> 16 & 0xFF) } else { (p >> 16 & 0xFF, p >> 8 & 0xFF, p & 0xFF) };
    let (r, g, b) = (r as i32, g as i32, b as i32);
    let y = (19595 * r + 38470 * g + 7471 * b + 32768) >> 16;
    let cb = ((-11059 * r - 21709 * g + 32768 * b + 32768) >> 16) + 128;
    let cr = ((32768 * r - 27439 * g - 5329 * b + 32768) >> 16) + 128;
    (y, cb.clamp(0, 255), cr.clamp(0, 255))
}

/// **Escribir** `ancho` x `alto` pixeles de `px` (fila `y` desde `y * paso`)
/// como JPEG de `calidad` (1..100) en `out`. `rgb`: el pixel es `0x00BBGGRR`
/// (una pantalla RGB) y no `0x00RRGGBB`. Devuelve los bytes escritos.
pub fn codificar(px: &[u32], ancho: u32, alto: u32, paso: u32, rgb: bool, calidad: u8, out: &mut [u8]) -> Result<usize, Error> {
    if ancho == 0 || alto == 0 || ancho > LADO_MAX || alto > LADO_MAX || paso < ancho {
        return Err(Error::Medidas);
    }
    if px.len() < (alto as usize - 1) * paso as usize + ancho as usize {
        return Err(Error::Corto);
    }
    let (ql, qc) = tablas(calidad);
    let mut s = Salida { b: out, n: 0, acum: 0, cuantos: 0, lleno: false };
    // SOI y APP0 (JFIF 1.01, sin miniatura).
    s.bytes(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0]);
    // DQT: las dos, en zig-zag.
    for (id, q) in [(0u8, &ql), (1, &qc)] {
        s.bytes(&[0xFF, 0xDB]);
        s.be16(67);
        s.byte(id);
        for &k in &ZIGZAG {
            s.byte(q[k as usize] as u8);
        }
    }
    // SOF0: tres componentes, Y 2x2 con la tabla 0, Cb y Cr 1x1 con la 1.
    s.bytes(&[0xFF, 0xC0]);
    s.be16(17);
    s.byte(8);
    s.be16(alto as u16);
    s.be16(ancho as u16);
    s.bytes(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    // DHT: las cuatro.
    for (clase, bits, valores) in [(0x00u8, &DC_LUMA_BITS, &DC_VALORES[..]), (0x10, &AC_LUMA_BITS, &AC_LUMA[..]), (0x01, &DC_CROMA_BITS, &DC_VALORES[..]), (0x11, &AC_CROMA_BITS, &AC_CROMA[..])] {
        s.bytes(&[0xFF, 0xC4]);
        s.be16(3 + 16 + valores.len() as u16);
        s.byte(clase);
        s.bytes(bits);
        s.bytes(valores);
    }
    // SOS: los tres, Y con DC0/AC0 y los otros con DC1/AC1.
    s.bytes(&[0xFF, 0xDA, 0, 12, 3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);
    let (mut dy, mut dcb, mut dcr) = (0i32, 0i32, 0i32);
    let pixel = |x: u32, y: u32| px[(y.min(alto - 1) * paso + x.min(ancho - 1)) as usize];
    for my in 0..alto.div_ceil(16) {
        for mx in 0..ancho.div_ceil(16) {
            let (mut y4, mut cb, mut cr) = ([[0i32; 64]; 4], [0i32; 64], [0i32; 64]);
            for j in 0..16u32 {
                for i in 0..16u32 {
                    let (y, b, r) = ycc(pixel(16 * mx + i, 16 * my + j), rgb);
                    let k = (j / 8 * 2 + i / 8) as usize;
                    y4[k][(8 * (j % 8) + i % 8) as usize] = y - 128;
                    // La croma de 2x2, sumada (y luego la media).
                    let c = (8 * (j / 2) + i / 2) as usize;
                    cb[c] += b;
                    cr[c] += r;
                }
            }
            for k in 0..64 {
                cb[k] = ((cb[k] + 2) >> 2) - 128;
                cr[k] = ((cr[k] + 2) >> 2) - 128;
            }
            for b in &y4 {
                bloque(&mut s, &dct(b, &ql), &mut dy, &DC_L, &AC_L);
            }
            bloque(&mut s, &dct(&cb, &qc), &mut dcb, &DC_C, &AC_C);
            bloque(&mut s, &dct(&cr, &qc), &mut dcr, &DC_C, &AC_C);
            if s.lleno {
                return Err(Error::NoCabe);
            }
        }
    }
    s.vaciar();
    s.bytes(&[0xFF, 0xD9]);
    if s.lleno {
        return Err(Error::NoCabe);
    }
    Ok(s.n)
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec;
    use std::vec::Vec;

    /// Una imagen con de todo: degradados, bordes duros, ruido, y un lado que
    /// no es multiplo de 16.
    fn imagen(w: u32, h: u32) -> Vec<u32> {
        (0..w * h)
            .map(|k| {
                let (x, y) = (k % w, k / w);
                let r = (x * 255 / w.max(1)) & 0xFF;
                let g = if (x / 7 + y / 5) % 2 == 0 { 220 } else { 30 };
                let b = (y * 255 / h.max(1) ^ (k.wrapping_mul(2654435761) >> 28)) & 0xFF;
                r << 16 | g << 8 | b
            })
            .collect()
    }

    /// El error medio por canal entre dos imagenes.
    fn error(a: &[u32], b: &[u32]) -> f64 {
        let mut s = 0u64;
        for (&p, &q) in a.iter().zip(b) {
            for d in [0, 8, 16] {
                s += ((p >> d & 0xFF) as i64 - (q >> d & 0xFF) as i64).unsigned_abs();
            }
        }
        s as f64 / (3 * a.len()) as f64
    }

    #[test]
    fn las_tablas_de_la_norma() {
        assert_eq!(AC_LUMA_BITS.iter().map(|&x| x as usize).sum::<usize>(), AC_LUMA.len());
        assert_eq!(AC_CROMA_BITS.iter().map(|&x| x as usize).sum::<usize>(), AC_CROMA.len());
        assert_eq!(DC_LUMA_BITS.iter().map(|&x| x as usize).sum::<usize>(), 12);
        assert_eq!(DC_CROMA_BITS.iter().map(|&x| x as usize).sum::<usize>(), 12);
        assert_eq!(tablas(50).0, Q_LUMA, "la calidad 50 es la tabla tal cual");
        assert!(tablas(100).0.iter().all(|&q| q == 1));
        assert_eq!(categoria(-3), (2, 0b00));
        assert_eq!(categoria(5), (3, 0b101));
        assert_eq!(categoria(0), (0, 0));
    }

    /// Un degradado suave (lo que es casi toda una pantalla).
    fn suave(w: u32, h: u32) -> Vec<u32> {
        (0..w * h).map(|k| (k % w * 255 / w) << 16 | (k / w * 255 / h) << 8 | 128).collect()
    }

    /// *** El lector de la casa lo lee de vuelta, y se parece. Con un
    /// degradado, casi exacto; con la imagen DURA (ruido y bordes de color
    /// cada pocos pixeles, lo que el 4:2:0 suaviza), lo que da libjpeg. Contra
    /// Pillow 12.3 (13-10), la DURA de 320x200 a calidad 90 y 4:2:0: Pillow
    /// codificando, error 22,19 y 53 455 bytes; esto, 22,20 y 53 989, y leido
    /// por Pillow. El degradado de 16x16: Pillow 1,93; esto leido por Pillow,
    /// 2,02.
    #[test]
    fn el_lector_de_la_casa_lo_lee() {
        for (w, h) in [(16, 16), (37, 21), (320, 200)] {
            // Topes con margen sobre lo medido (13-10). En lo diminuto pesa el
            // lector: reparte la croma sin suavizarla (Pillow, con el mismo
            // fichero de 16x16, da 2,02 donde el lector da 4,77).
            let chico = w < 320;
            for (dura, calidad, tope) in [(false, 90u8, if chico { 5.5 } else { 0.8 }), (false, 100, if chico { 5.2 } else { 0.7 }), (false, 40, if chico { 7.5 } else { 2.1 }), (true, 90, if chico { 24.0 } else { 21.0 }), (true, 100, if chico { 22.0 } else { 18.0 })] {
                let px = if dura { imagen(w, h) } else { suave(w, h) };
                let mut out = vec![0u8; cota(w, h)];
                let n = codificar(&px, w, h, w, false, calidad, &mut out).unwrap();
                let m = crate::jpeg::medir(&out[..n]).unwrap_or_else(|e| panic!("{w}x{h} q{calidad}: {e:?}"));
                assert_eq!((m.ancho, m.alto), (w, h));
                let mut leido = vec![0u32; (w * h) as usize];
                crate::jpeg::decodificar(&out[..n], &m, &mut leido).unwrap();
                let e = error(&px, &leido);
                assert!(e < tope, "{w}x{h} calidad {calidad} (dura {dura}): error medio {e:.2} (tope {tope})");
            }
        }
    }

    #[test]
    fn rgb_es_el_mismo_con_los_canales_al_reves() {
        let px = imagen(48, 32);
        let al_reves: Vec<u32> = px.iter().map(|p| (p & 0xFF) << 16 | p & 0xFF00 | p >> 16 & 0xFF).collect();
        let (mut a, mut b) = (vec![0u8; cota(48, 32)], vec![0u8; cota(48, 32)]);
        let na = codificar(&px, 48, 32, 48, false, 85, &mut a).unwrap();
        let nb = codificar(&al_reves, 48, 32, 48, true, 85, &mut b).unwrap();
        assert_eq!(a[..na], b[..nb]);
    }

    #[test]
    fn el_paso_de_fila_y_lo_que_no_cabe() {
        let px = imagen(64, 8);
        // La misma imagen dentro de una mas ancha: con su paso, los mismos bytes.
        let mut ancha = vec![0xFF00FFu32; 100 * 8];
        for y in 0..8 {
            ancha[100 * y..100 * y + 64].copy_from_slice(&px[64 * y..64 * y + 64]);
        }
        let (mut a, mut b) = (vec![0u8; cota(64, 8)], vec![0u8; cota(64, 8)]);
        let na = codificar(&px, 64, 8, 64, false, 75, &mut a).unwrap();
        let nb = codificar(&ancha, 64, 8, 100, false, 75, &mut b).unwrap();
        assert_eq!(a[..na], b[..nb]);
        assert_eq!(codificar(&px, 64, 8, 64, false, 75, &mut a[..100]), Err(Error::NoCabe));
        assert_eq!(codificar(&px, 0, 8, 64, false, 75, &mut a), Err(Error::Medidas));
        assert_eq!(codificar(&px[..10], 64, 8, 64, false, 75, &mut a), Err(Error::Corto));
    }

    /// Para mirarlo fuera: `BMO_JPEG_DIR=/tmp cargo test` deja uno en disco.
    #[test]
    fn deja_uno_si_se_pide() {
        if let Ok(dir) = std::env::var("BMO_JPEG_DIR") {
            let px = imagen(320, 200);
            let mut out = vec![0u8; cota(320, 200)];
            let n = codificar(&px, 320, 200, 320, false, 90, &mut out).unwrap();
            std::fs::write(std::format!("{dir}/prueba.jpg"), &out[..n]).unwrap();
            let raw: Vec<u8> = px.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
            std::fs::write(std::format!("{dir}/prueba.rgb"), raw).unwrap();
            for (w, h) in [(16u32, 16u32), (320, 200)] {
                let px = suave(w, h);
                let mut out = vec![0u8; cota(w, h)];
                let n = codificar(&px, w, h, w, false, 90, &mut out).unwrap();
                std::fs::write(std::format!("{dir}/suave{w}.jpg"), &out[..n]).unwrap();
                let m = crate::jpeg::medir(&out[..n]).unwrap();
                let mut leido = vec![0u32; (w * h) as usize];
                crate::jpeg::decodificar(&out[..n], &m, &mut leido).unwrap();
                let raw: Vec<u8> = leido.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
                std::fs::write(std::format!("{dir}/suave{w}.casa"), raw).unwrap();
            }
        }
    }
}
