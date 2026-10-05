//! **Las texturas de verdad** (02-10, lo que Cyberpunk crea al montar su
//! D3D12): de CUALQUIER formato, con sus MIPMAPS, sus CAPAS (arrays y
//! cubos: un cubo son 6 capas) y en 1D, 2D o 3D. Hasta hoy la casa sabia de
//! una textura 2D de un nivel, RGBA/BGRA de 8 bits o D32, y lo demas era
//! E_INVALIDARG.
//!
//! capa: puro -- cuentas sobre la descripcion y bytes; la memoria la pide
//! quien crea el recurso
//!
//! ```text
//!    la FORMA       lo del D3D12_RESOURCE_DESC: dimension, ancho, alto,
//!                   profundidad o capas, mips (0 = la cadena entera),
//!                   formato
//!    un SUBRECURSO  el indice de D3D12: mip + capa * mips (en 3D, la mip,
//!                   con TODA su profundidad)
//!    lo INTERNO     como lo guarda la casa, todo seguido y en orden de
//!                   subrecurso: los BC TAL CUAL (bloques de 4x4: la 3060
//!                   los lee asi, y descomprimidos ocuparian de 4 a 8 veces
//!                   mas); lo demas en 4 bytes por texel (RGBA8, o BGRA8, o
//!                   el float de 32 bits de una profundidad o un R32)
//!    la HUELLA      como queda un subrecurso en un bufer (GetCopyableFootprints):
//!                   en su formato NATIVO, cada fila a 256 bytes y cada
//!                   subrecurso a 512, como D3D12
//! ```
//!
//! Lo que pierde la conversion a 8 bits por canal (un HDR de 16 bits, un
//! R10G10B10A2, un entero): se VE, sin su rango ni su precision. La copia
//! de vuelta a un bufer (leer) solo es exacta donde lo interno es lo nativo
//! (RGBA8, BGRA8, R32 y D32, y los BC); de lo demas se dice.

use alloc::vec::Vec;

use bmo_proton_x::bc::Bc;

/// D3D12_RESOURCE_DIMENSION.
pub(crate) const DIM_TEXTURA1D: u32 = 2;
pub(crate) const DIM_TEXTURA2D: u32 = 3;
pub(crate) const DIM_TEXTURA3D: u32 = 4;

/// D3D12_TEXTURE_DATA_PITCH_ALIGNMENT y D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT.
pub(crate) const PASO_FILA: u64 = 256;
pub(crate) const ALINEA_SUB: u64 = 512;

/// **Como guarda la casa los texeles de un formato.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Almacen {
    /// 4 bytes por texel, R en el byte bajo (`0xAABBGGRR`).
    Rgba8,
    /// 4 bytes por texel, B en el byte bajo (los B8G8R8A8/X8 de DXGI).
    Bgra8,
    /// El float de 32 bits de una profundidad o de un R32: se lee como R.
    Flotante,
    /// N5.16 (05-10): cuatro floats de 32 bits por texel (16 bytes), los
    /// de los formatos de FLOAT de color y los de 10 bits (el HDR: RGBA16F,
    /// R11G11B10F, RGBA32F, RG16F, R10G10B10A2...), ya cuantizados a su
    /// formato NATIVO ([`Almacen::nativo`]): se leen y se escriben sin
    /// perder nada, y vuelven a su formato exactos.
    Flotantes4,
    /// Bloques comprimidos, tal cual llegan.
    Bloques(Bc),
}

impl Almacen {
    /// El de un DXGI_FORMAT.
    pub fn de(formato: u32) -> Almacen {
        if let Some(b) = Bc::de_dxgi(formato) {
            return Almacen::Bloques(b);
        }
        match formato {
            87 | 88 | 90..=93 => Almacen::Bgra8,
            // R32 (TYPELESS, FLOAT, UINT, SINT), D32, D24S8 y su familia,
            // D32S8X24 y la suya, D16 y R16 TYPELESS: profundidades y floats.
            19..=22 | 39..=47 | 53 | 55 => Almacen::Flotante,
            // N5.16: los de float y los de 10 bits (y sus TYPELESS).
            1..=3 | 5 | 6 | 9 | 10 | 15 | 16 | 23 | 24 | 26 | 33 | 34 | 54 => Almacen::Flotantes4,
            _ => Almacen::Rgba8,
        }
    }

    /// **El formato con que se cuantiza y se copia** uno de [`Almacen::Flotantes4`]:
    /// el mismo, o el de float (o UNORM, los de 10 bits) de un TYPELESS.
    pub fn nativo(formato: u32) -> u32 {
        match formato {
            1 => 2,
            5 => 6,
            9 => 10,
            15 => 16,
            23 => 24,
            33 => 34,
            f => f,
        }
    }

    /// `(bytes, lado)` de un elemento interno: un texel o un bloque de 4x4.
    pub fn elemento(self) -> (u64, u64) {
        match self {
            Almacen::Bloques(b) => (b.bytes() as u64, 4),
            Almacen::Flotantes4 => (16, 1),
            _ => (4, 1),
        }
    }
}

/// **La forma de una textura**, leida de su D3D12_RESOURCE_DESC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Forma {
    pub dimension: u32,
    pub ancho: u32,
    pub alto: u32,
    /// Profundidad (3D) o capas (1D/2D; un cubo, 6 por cubo).
    pub hondo: u32,
    pub mips: u32,
    pub formato: u32,
}

impl Forma {
    /// Una 2D de un nivel y una capa (un render target, un back buffer).
    pub const fn plana(ancho: u32, alto: u32, formato: u32) -> Forma {
        Forma { dimension: DIM_TEXTURA2D, ancho, alto, hondo: 1, mips: 1, formato }
    }

    /// **De un D3D12_RESOURCE_DESC** (o DESC1: empieza igual): Dimension +0,
    /// Width +16, Height +24, DepthOrArraySize +28, MipLevels +30, Format +32.
    /// `None` si no es una textura que se pueda tener (medidas imposibles).
    ///
    /// # Safety
    /// `d` son los 56 bytes de una descripcion del `.exe`.
    pub unsafe fn de(d: *const u8) -> Option<Forma> {
        let u32_ = |o: usize| (d.add(o) as *const u32).read_unaligned();
        let u16_ = |o: usize| (d.add(o) as *const u16).read_unaligned() as u32;
        let dimension = u32_(0);
        let ancho = (d.add(16) as *const u64).read_unaligned();
        if !(DIM_TEXTURA1D..=DIM_TEXTURA3D).contains(&dimension) || ancho == 0 || ancho > 16384 {
            return None;
        }
        let alto = if dimension == DIM_TEXTURA1D { 1 } else { u32_(24) };
        let hondo = u16_(28).max(1);
        if alto == 0 || alto > 16384 || (dimension == DIM_TEXTURA3D && hondo > 2048) {
            return None;
        }
        let ancho = ancho as u32;
        let maximo = 32 - ancho.max(alto).max(if dimension == DIM_TEXTURA3D { hondo } else { 1 }).leading_zeros();
        let mips = match u16_(30) {
            0 => maximo,
            m => m.min(maximo),
        };
        Some(Forma { dimension, ancho, alto, hondo, mips, formato: u32_(32) })
    }

    /// Las capas (en 3D, una: la profundidad es de cada mip).
    pub fn capas(&self) -> u32 {
        if self.dimension == DIM_TEXTURA3D {
            1
        } else {
            self.hondo
        }
    }

    pub fn subrecursos(&self) -> u32 {
        self.mips * self.capas()
    }

    /// `(mip, capa)` del subrecurso `i`.
    pub fn sub(&self, i: u32) -> (u32, u32) {
        (i % self.mips, i / self.mips)
    }

    /// `(ancho, alto, profundidad)` de la mip `m`.
    pub fn medidas(&self, m: u32) -> (u32, u32, u32) {
        let d = if self.dimension == DIM_TEXTURA3D { (self.hondo >> m).max(1) } else { 1 };
        ((self.ancho >> m).max(1), (self.alto >> m).max(1), d)
    }
}

/// **Un subrecurso, por dentro.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sub {
    pub ancho: u32,
    pub alto: u32,
    pub hondo: u32,
    /// Desde el principio de lo interno, en bytes.
    pub desde: u64,
    /// Bytes de una fila de elementos (texeles o bloques), sin relleno.
    pub fila: u64,
    /// Filas de elementos de una rebanada.
    pub filas: u32,
}

impl Sub {
    pub fn bytes(&self) -> u64 {
        self.fila * self.filas as u64 * self.hondo as u64
    }
}

/// **La disposicion interna**: cada subrecurso en su sitio, y el total.
pub fn disposicion(f: &Forma) -> (Vec<Sub>, u64) {
    let (bytes, lado) = Almacen::de(f.formato).elemento();
    let mut v = Vec::with_capacity(f.subrecursos() as usize);
    let mut total = 0u64;
    for i in 0..f.subrecursos() {
        let (m, _) = f.sub(i);
        let (ancho, alto, hondo) = f.medidas(m);
        let fila = (ancho as u64).div_ceil(lado) * bytes;
        let filas = (alto as u64).div_ceil(lado) as u32;
        let s = Sub { ancho, alto, hondo, desde: total, fila, filas };
        total += s.bytes();
        v.push(s);
    }
    (v, total)
}

/// **La huella NATIVA de un subrecurso** en un bufer (lo que da
/// GetCopyableFootprints).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Huella {
    /// Desde el principio de la primera huella (sin el `desde` del `.exe`).
    pub desde: u64,
    /// En texeles, redondeado al bloque (un BC de 2x2 da 4x4).
    pub ancho: u32,
    pub alto: u32,
    pub hondo: u32,
    /// Bytes de fila en el bufer: la fila, a 256.
    pub paso: u64,
    /// Filas de elementos de una rebanada.
    pub filas: u32,
    /// Bytes de una fila, sin el relleno.
    pub fila: u64,
}

/// **Las huellas de `n` subrecursos desde `primero`**, y lo que ocupan
/// todas (sin el relleno de la ultima fila, como D3D12).
pub fn huellas(f: &Forma, primero: u32, n: u32) -> (Vec<Huella>, u64) {
    let (bytes, lado) = crate::d3d12_medidas::elemento(f.formato);
    let mut v = Vec::with_capacity(n as usize);
    let (mut siguiente, mut total) = (0u64, 0u64);
    for i in primero..primero.saturating_add(n) {
        let (m, _) = f.sub(i);
        let (ancho, alto, hondo) = f.medidas(m);
        let (cols, filas) = ((ancho as u64).div_ceil(lado), (alto as u64).div_ceil(lado));
        let fila = cols * bytes;
        let paso = fila.div_ceil(PASO_FILA) * PASO_FILA;
        let desde = siguiente.div_ceil(ALINEA_SUB) * ALINEA_SUB;
        let h = Huella { desde, ancho: (cols * lado) as u32, alto: (filas * lado) as u32, hondo, paso, filas: filas as u32, fila };
        total = desde + paso * (filas * hondo as u64 - 1) + fila;
        siguiente = desde + paso * filas * hondo as u64;
        v.push(h);
    }
    (v, total)
}

/// Un medio float (IEEE 754 de 16 bits) a f32.
pub fn medio(h: u32) -> f32 {
    let (s, e, m) = (h >> 15 & 1, h >> 10 & 0x1F, h & 0x3FF);
    let v = match e {
        0 => m as f32 / 16_777_216.0,
        0x1F => {
            if m == 0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => f32::from_bits((e + 112) << 23 | m << 13),
    };
    if s == 1 {
        -v
    } else {
        v
    }
}

/// Un float chico sin signo de `m` bits de mantisa y 5 de exponente (los
/// de R11G11B10_FLOAT) a f32.
fn chico(v: u32, m: u32) -> f32 {
    let (e, f) = (v >> m & 0x1F, v & ((1 << m) - 1));
    match e {
        0 => f as f32 / (1u32 << m) as f32 / 16384.0,
        0x1F => f32::INFINITY,
        _ => f32::from_bits((e + 112) << 23 | f << (23 - m)),
    }
}

/// Un float 0..1 a un byte (lo de fuera, sujeto; NaN, 0).
fn byte(x: f32) -> u32 {
    if x.is_nan() || x <= 0.0 {
        0
    } else if x >= 1.0 {
        255
    } else {
        (x * 255.0 + 0.5) as u32
    }
}

fn rgba(r: u32, g: u32, b: u32, a: u32) -> u32 {
    a << 24 | b << 16 | g << 8 | r
}

/// **Un texel NATIVO de `formato` (sus bytes en `s`) a lo interno**: 4
/// bytes, como dice [`Almacen::de`]. Los BC no pasan por aqui.
pub fn a_interno(formato: u32, s: &[u8]) -> u32 {
    let b = |k: usize| s.get(k).copied().unwrap_or(0) as u32;
    let u16_ = |k: usize| b(k) | b(k + 1) << 8;
    let u32_ = |k: usize| u16_(k) | u16_(k + 2) << 16;
    let f32_ = |k: usize| f32::from_bits(u32_(k));
    match formato {
        // Lo que ya es lo interno: RGBA8 y BGRA8 (X8: alfa 1), R32 y D32.
        27..=32 | 87 | 90 | 91 => u32_(0),
        88 | 92 | 93 => u32_(0) | 0xFF00_0000,
        39..=43 => u32_(0),
        // D24S8: la profundidad (24 bits UNORM) a float; D32S8X24: su float.
        44..=47 => ((u32_(0) & 0xFF_FFFF) as f32 / 16_777_215.0).to_bits(),
        19..=22 => u32_(0),
        // D16 y R16 TYPELESS: a float.
        53 | 55 => (u16_(0) as f32 / 65535.0).to_bits(),
        // 128, 96 y 64 bits de floats.
        2 | 6 => rgba(byte(f32_(0)), byte(f32_(4)), byte(f32_(8)), if formato == 2 { byte(f32_(12)) } else { 255 }),
        16 => rgba(byte(f32_(0)), byte(f32_(4)), 0, 255),
        10 => rgba(byte(medio(u16_(0))), byte(medio(u16_(2))), byte(medio(u16_(4))), byte(medio(u16_(6)))),
        34 => rgba(byte(medio(u16_(0))), byte(medio(u16_(2))), 0, 255),
        54 => rgba(byte(medio(u16_(0))), 0, 0, 255),
        // UNORM de 16 bits: el byte alto.
        11 => rgba(b(1), b(3), b(5), b(7)),
        35 => rgba(b(1), b(3), 0, 255),
        56 => rgba(b(1), 0, 0, 255),
        // R10G10B10A2 y R11G11B10_FLOAT.
        23..=25 => {
            let v = u32_(0);
            rgba((v & 0x3FF) >> 2, (v >> 10 & 0x3FF) >> 2, (v >> 20 & 0x3FF) >> 2, (v >> 30) * 85)
        }
        26 => {
            let v = u32_(0);
            rgba(byte(chico(v & 0x7FF, 6)), byte(chico(v >> 11 & 0x7FF, 6)), byte(chico(v >> 22, 5)), 255)
        }
        // R9G9B9E5: tres mantisas de 9 bits y un exponente comun.
        67 => {
            let v = u32_(0);
            let e = (v >> 27) as i32 - 15 - 9;
            let x = |m: u32| byte(m as f32 * f32::from_bits(((e + 127).clamp(1, 254) as u32) << 23));
            rgba(x(v & 0x1FF), x(v >> 9 & 0x1FF), x(v >> 18 & 0x1FF), 255)
        }
        // 8 y 16 bits por canal sin float: el byte (bajo los de 8, alto
        // los de 16; los enteros y SNORM, lo que caiga).
        48..=52 => rgba(b(0), b(1), 0, 255),
        60..=64 => rgba(b(0), 0, 0, 255),
        65 => rgba(0, 0, 0, b(0)),
        9 | 12..=14 => rgba(b(0), b(2), b(4), b(6)),
        33 | 36..=38 => rgba(b(0), b(2), 0, 255),
        57..=59 => rgba(b(0), 0, 0, 255),
        1 | 3 | 4 => rgba(b(0), b(4), b(8), b(12)),
        5 | 7 | 8 => rgba(b(0), b(4), b(8), 255),
        15 | 17 | 18 => rgba(b(0), b(4), 0, 255),
        // B5G6R5, B5G5R5A1, B4G4R4A4.
        85 => {
            let v = u16_(0);
            rgba((v >> 11) * 255 / 31, (v >> 5 & 0x3F) * 255 / 63, (v & 0x1F) * 255 / 31, 255)
        }
        86 => {
            let v = u16_(0);
            rgba((v >> 10 & 0x1F) * 255 / 31, (v >> 5 & 0x1F) * 255 / 31, (v & 0x1F) * 255 / 31, (v >> 15) * 255)
        }
        115 => {
            let v = u16_(0);
            rgba((v >> 8 & 0xF) * 17, (v >> 4 & 0xF) * 17, (v & 0xF) * 17, (v >> 12) * 17)
        }
        _ => u32_(0),
    }
}

/// Lo interno ES lo nativo (4 bytes que se copian tal cual, o bloques):
/// la copia a un bufer es exacta.
pub fn interno_es_nativo(formato: u32) -> bool {
    matches!(formato, 27..=32 | 87 | 88 | 90..=93 | 39..=43) || Bc::de_dxgi(formato).is_some()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn desc(dimension: u32, ancho: u64, alto: u32, hondo: u16, mips: u16, formato: u32) -> [u8; 56] {
        let mut d = [0u8; 56];
        d[0..4].copy_from_slice(&dimension.to_le_bytes());
        d[16..24].copy_from_slice(&ancho.to_le_bytes());
        d[24..28].copy_from_slice(&alto.to_le_bytes());
        d[28..30].copy_from_slice(&hondo.to_le_bytes());
        d[30..32].copy_from_slice(&mips.to_le_bytes());
        d[32..36].copy_from_slice(&formato.to_le_bytes());
        d
    }

    fn forma(d: &[u8; 56]) -> Forma {
        // SAFETY: 56 bytes.
        unsafe { Forma::de(d.as_ptr()) }.unwrap()
    }

    #[test]
    fn la_forma_mips_capas_y_subrecursos() {
        // 1024x512 con la cadena entera: 11 mips (hasta 1x1).
        let f = forma(&desc(3, 1024, 512, 1, 0, 28));
        assert_eq!((f.mips, f.capas(), f.subrecursos()), (11, 1, 11));
        assert_eq!(f.medidas(10), (1, 1, 1));
        // Un cubo con 3 mips: 6 capas, 18 subrecursos; el 7 es mip 1 de la cara 2.
        let c = forma(&desc(3, 256, 256, 6, 3, 71));
        assert_eq!((c.capas(), c.subrecursos(), c.sub(7)), (6, 18, (1, 2)));
        // 3D: la profundidad baja con las mips; una "capa".
        let t = forma(&desc(4, 64, 32, 16, 0, 10));
        assert_eq!((t.mips, t.capas(), t.medidas(1), t.medidas(5)), (7, 1, (32, 16, 8), (2, 1, 1)));
        // Imposibles: ancho 0, dimension de bufer.
        let mut malo = desc(3, 0, 4, 1, 1, 28);
        // SAFETY: 56 bytes.
        assert!(unsafe { Forma::de(malo.as_ptr()) }.is_none());
        malo = desc(1, 64, 1, 1, 1, 0);
        assert!(unsafe { Forma::de(malo.as_ptr()) }.is_none());
    }

    #[test]
    fn lo_interno_bc_tal_cual_y_lo_demas_a_4_bytes() {
        // BC1 de 8x8 con 4 mips: 4 bloques (32 B), 1 (8), 1 (8), 1 (8).
        let (s, total) = disposicion(&forma(&desc(3, 8, 8, 1, 4, 71)));
        assert_eq!(s.iter().map(|x| (x.desde, x.bytes())).collect::<Vec<_>>(), [(0, 32), (32, 8), (40, 8), (48, 8)]);
        assert_eq!(total, 56);
        // RGBA16F de 4x2, dos capas: por dentro CUATRO floats por texel
        // (16 bytes, N5.16: el HDR no se aplasta a 8 bits).
        let (s, total) = disposicion(&forma(&desc(3, 4, 2, 2, 1, 10)));
        assert_eq!((s.len(), s[1].desde, total), (2, 128, 256));
        assert_eq!(Almacen::de(10), Almacen::Flotantes4);
        assert_eq!(Almacen::de(28), Almacen::Rgba8);
        assert_eq!(Almacen::de(87), Almacen::Bgra8);
        assert_eq!(Almacen::de(45), Almacen::Flotante);
        assert_eq!(Almacen::de(98), Almacen::Bloques(Bc::Bc7));
    }

    #[test]
    fn las_huellas_como_d3d12() {
        // 100x10 RGBA8: fila 400 B, paso 512; total = 512 * 9 + 400.
        let (h, total) = huellas(&forma(&desc(3, 100, 10, 1, 1, 28)), 0, 1);
        assert_eq!((h[0].paso, h[0].filas, h[0].fila, total), (512, 10, 400, 512 * 9 + 400));
        // BC7 de 6x6 con 2 mips: la mip 0 son 2x2 bloques (fila 32 B, paso
        // 256, 2 filas); la mip 1 (3x3) un bloque, a 512 de alineacion.
        let (h, total) = huellas(&forma(&desc(3, 6, 6, 1, 2, 98)), 0, 2);
        assert_eq!((h[0].ancho, h[0].alto, h[0].filas, h[0].fila), (8, 8, 2, 32));
        assert_eq!((h[1].desde, h[1].ancho, h[1].filas, h[1].fila), (512, 4, 1, 16));
        assert_eq!(total, 512 + 16);
        // Desde el subrecurso 1: empieza en 0.
        let (h, _) = huellas(&forma(&desc(3, 6, 6, 1, 2, 98)), 1, 1);
        assert_eq!(h[0].desde, 0);
        // 3D: cada rebanada, filas * paso.
        let (h, total) = huellas(&forma(&desc(4, 4, 4, 4, 1, 28)), 0, 1);
        assert_eq!((h[0].hondo, total), (4, 256 * 15 + 16));
    }

    #[test]
    fn los_formatos_sin_comprimir_a_8_bits() {
        // RGBA16F: 1, 0.5, 0, 2 (sujeto) -> 255, 128, 0, 255.
        let s = [0x00, 0x3C, 0x00, 0x38, 0x00, 0x00, 0x00, 0x40];
        assert_eq!(a_interno(10, &s), 0xFF00_80FF);
        // R10G10B10A2: R 1023, G 0, B 512, A 3.
        let v: u32 = 1023 | 512 << 20 | 3 << 30;
        assert_eq!(a_interno(24, &v.to_le_bytes()), 0xFF80_00FF);
        // R11G11B10_FLOAT: R 1.0 (exp 15, mantisa 0 en 6 bits).
        assert_eq!(a_interno(26, &(15u32 << 6).to_le_bytes()) & 0xFF, 255);
        // D24S8: profundidad 1.0.
        assert_eq!(f32::from_bits(a_interno(45, &0xFFFF_FFFFu32.to_le_bytes())), 1.0);
        // R8 y B5G6R5 blanco.
        assert_eq!(a_interno(61, &[200]), 0xFF00_00C8);
        assert_eq!(a_interno(85, &0xFFFFu16.to_le_bytes()), 0xFFFF_FFFF);
        // BGRX: alfa a 1.
        assert_eq!(a_interno(88, &[1, 2, 3, 0]), 0xFF03_0201);
        assert!(interno_es_nativo(28) && interno_es_nativo(71) && !interno_es_nativo(10));
        assert_eq!(medio(0x3C00), 1.0);
        assert_eq!(medio(0xC000), -2.0);
    }
}
