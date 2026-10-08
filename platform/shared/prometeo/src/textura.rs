//! **LAS TEXTURAS** (camino de `HelloTexture`, 29-09) -- lo que lee un
//! sombreador con `Sample`: una imagen de 8 bits por canal y un
//! MUESTREADOR (como se filtra y que pasa fuera de 0..1), con las reglas de
//! D3D11/12 (la especificacion funcional de D3D11, seccion 7.18).
//!
//! capa: puro -- ni un aparato, ni `std`: soft-float en Ring 3
//!
//! ```text
//!    PUNTO     el texel de floor(u * ancho), floor(v * alto)
//!    LINEAL    los cuatro de alrededor de (u * ancho - 0.5, v * alto - 0.5),
//!              COMO LA 3060 (abajo)
//!    fuera     REPETIR, ESPEJO, SUJETAR, BORDE (su color) y ESPEJO UNA VEZ
//! ```
//!
//! # Como filtra la 3060, MEDIDO (29-09)
//!
//! `docs/metal/tex_cuda/SALIDA.TXT`: 96 muestras de la 3060 misma (CUDA,
//! Point/Linear x Wrap/Mirror/Clamp/Border x 12 puntos de una 4x4 RGBA8). La
//! version de antes (la fraccion truncada a 8 bits y el filtro en float)
//! igualaba 60; esta, las 96 BIT A BIT (`tests/metal_textura.rs`):
//!
//! ```text
//!    la coordenada   (u * ancho - 0.5) en punto fijo con 8 bits de fraccion,
//!                    REDONDEADA (no truncada): i = parte entera, a = fraccion
//!    los pesos       cada esquina, round(wu * wv / 256), en 1/256: el
//!                    producto tambien se cuantiza a 8 bits
//!    los texeles     en 16 bits (byte * 257); el borde, cuantizado ANTES al
//!                    formato: floor(c * 255) (0,5 pedido -> 127/255)
//!    el resultado    round(suma / 256) / 65535
//! ```
//!
//! [!] El borde: con 0, 0,5 y 1 se sabe que 0,5 va a 127 (no a 128); si la
//! 3060 trunca o redondea hacia abajo en otros valores, lo dira otra medida.
//!
//! Sin anisotropia: se dice al leer el muestreador y el lote va igual, con
//! lo de aqui.
//!
//! # La mip (D4.4, 05-10)
//!
//! Hasta el 05-10 `Sample` leia siempre la mip mas detallada de la vista
//! (las derivadas daban 0): un juego con texturas lejanas leia la mip 0 y
//! salia con ruido (aliasing) y mas lento. Ahora, como la especificacion de
//! D3D11 (7.18.11), con los gradientes de las coordenadas en el cuadro de
//! 2x2 del pixel (`olas::gradientes`):
//!
//! ```text
//!    lambda   log2 del lado mayor del pixel en texeles de la mip mas
//!             detallada de la vista: ISOTROPICO (el anisotropico, igual)
//!    + sesgo  el del sombreador (SampleBias) y el del muestreador (MipLODBias)
//!    sujeto   a [max(MinLOD, clamp del sombreador), MaxLOD], a las mips de
//!             la vista (MostDetailedMip, MipLevels) y al ResourceMinLODClamp
//!    filtro   el de cerca (MAG) si no pasa de 0; el de lejos (MIN) si pasa
//!    mip      PUNTO, la mas cercana: floor(lambda + 0,5); LINEAL, las dos de
//!             alrededor, mezcladas con la fraccion cuantizada a 1/256
//! ```
//!
//! La precision: lambda = log2(lado^2) / 2, con lado^2 en f64 redondeado a
//! f32 y el log2 de `crate::mates` (serie en f64, sin `libm`): el error es
//! de unos 4e-8 (medio ULP de f32 en lado^2, pasado por log2). Una potencia
//! de dos da su lambda EXACTO. Lo que no hace, dicho: un cubo sigue en la mip
//! de la vista (sus gradientes son de la direccion, no de la cara), y una 3D
//! no mira el gradiente de w.
//!
//! 02-10: ademas de RGBA/BGRA de 8 bits, los floats (R32, profundidades) y
//! los BC1..BC7 (se descomprime el bloque del texel: ver [`crate::bc`]), las
//! vistas sRGB (a lineal antes de filtrar) y el mapeo de componentes de la
//! vista.

use crate::bc::{self, Bc};

/// `DXGI_FORMAT_R8G8B8A8_UNORM` y `B8G8R8A8_UNORM`.
pub const R8G8B8A8_UNORM: u32 = 28;
pub const B8G8R8A8_UNORM: u32 = 87;

/// **Como estan los texeles en memoria** (02-10: ademas de 8 bits por
/// canal, los floats de una profundidad o un R32, y los bloques BC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Como {
    /// 4 bytes por texel, R en el byte bajo.
    Rgba8,
    /// 4 bytes por texel, B en el byte bajo.
    Bgra8,
    /// Un float de 32 bits por texel (R32, una profundidad): se lee
    /// `(r, 0, 0, 1)`, como D3D.
    Flotante,
    /// N5.16 (05-10): cuatro floats de 32 bits por texel, (r, g, b, a) tal
    /// cual: como guarda la casa los render targets de float (RGBA16F,
    /// R11G11B10F, RGBA32F, R10G10B10A2...), ya cuantizados a su formato.
    Flotantes4,
    /// Bloques de 4x4 comprimidos, tal cual (8 o 16 bytes: 2 o 4 palabras),
    /// fila de bloques tras fila de bloques; se descomprime el bloque al leer.
    Bloques(Bc),
    /// 05-10: el plano de STENCIL de un D24S8 o un D32S8X24 (un byte por
    /// texel, cuatro por palabra: el texel `i` es el byte `i % 4` de la
    /// palabra `i / 4`), visto por un SRV X24_TYPELESS_G8_UINT o
    /// X32_TYPELESS_G8X24_UINT: `Load` da `(0, stencil, 0, 1)`, enteros.
    Stencil8,
}

/// **Que mira una vista** (la D3D12_SRV_DIMENSION, reducida): una 1D o
/// 2D, un array de ellas, una 3D, un cubo o un array de cubos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clase {
    Plana,
    Array,
    Volumen,
    Cubo,
    CuboArray,
}

/// **Una textura, vista por un SRV**: TODOS sus subrecursos como los guarda
/// la casa (en orden de subrecurso: `mip + capa * mips`; cada uno, filas de
/// texeles o de bloques sin relleno, y en 3D sus rebanadas seguidas), en
/// palabras de 4 bytes; y lo que mira la vista (D4.4: tambien sus mips).
#[derive(Clone, Copy, Debug)]
pub struct Textura<'a> {
    pub texeles: &'a [u32],
    /// De la mip 0.
    pub ancho: u32,
    pub alto: u32,
    pub como: Como,
    /// Una vista `*_SRGB`: el color (no el alfa) se pasa a lineal al leer
    /// cada texel, ANTES de filtrar, como D3D.
    pub srgb: bool,
    /// El `Shader4ComponentMapping` de la vista: 3 bits por canal de salida
    /// (0..3 el canal R, G, B o A; 4 un 0; 5 un 1). [`Textura::MAPEO`] es
    /// el de siempre.
    pub mapeo: u32,
    /// Las mips, las capas (un cubo, 6 por cubo; 1 en 3D) y la
    /// profundidad de la mip 0 (1 si no es 3D) de la textura ENTERA.
    pub mips: u32,
    pub capas: u32,
    pub hondo: u32,
    /// La vista: su clase, su mip mas detallada y su primera capa (o cara).
    pub clase: Clase,
    pub mip: u32,
    pub capa: u32,
    /// D4.4: las mips de la vista desde `mip` (su MipLevels; 0 o `u32::MAX`,
    /// todas las que quedan) y su ResourceMinLODClamp (en mips del RECURSO:
    /// ninguna lectura baja de ahi).
    pub niveles: u32,
    pub lod_min: f32,
    /// D2.7 (06-10): la vista lee la textura con OTRO formato del mismo
    /// tamanio de elemento (un R32_UINT sobre un R11G11B10F o un RGBA8, un
    /// RGBA8_SINT sobre un RGBA8_TYPELESS): `(guardado, vista)`. `Load` toma
    /// los BYTES del texel (en uno de [`Como::Flotantes4`], los de sus floats
    /// en `guardado`; en los demas, su palabra tal cual) y los lee en
    /// `vista`. `None`: la vista lee lo guardado como es.
    pub vista: Option<(u32, u32)>,
}

/// **Un plano**: una rebanada 2D de un subrecurso, lo que de verdad se
/// filtra.
#[derive(Clone, Copy, Debug)]
struct Plano<'a> {
    texeles: &'a [u32],
    ancho: u32,
    alto: u32,
    como: Como,
    srgb: bool,
}

/// sRGB (8 bits) a lineal en 16 bits: `round(65535 * lineal(c / 255))` con
/// la curva de sRGB (la recta hasta 0,04045 y la potencia de 2,4). Tabla
/// GENERADA (no hay `powf` en Ring 3).
const SRGB_A_LINEAL: [u16; 256] = [
    0, 20, 40, 60, 80, 99, 119, 139, 159, 179, 199, 219, 241, 264, 288, 313,
    340, 367, 396, 427, 458, 491, 526, 562, 599, 637, 677, 718, 761, 805, 851, 898,
    947, 997, 1048, 1101, 1156, 1212, 1270, 1330, 1391, 1453, 1517, 1583, 1651, 1720, 1790, 1863,
    1937, 2013, 2090, 2170, 2250, 2333, 2418, 2504, 2592, 2681, 2773, 2866, 2961, 3058, 3157, 3258,
    3360, 3464, 3570, 3678, 3788, 3900, 4014, 4129, 4247, 4366, 4488, 4611, 4736, 4864, 4993, 5124,
    5257, 5392, 5530, 5669, 5810, 5953, 6099, 6246, 6395, 6547, 6700, 6856, 7014, 7174, 7335, 7500,
    7666, 7834, 8004, 8177, 8352, 8528, 8708, 8889, 9072, 9258, 9445, 9635, 9828, 10022, 10219, 10417,
    10619, 10822, 11028, 11235, 11446, 11658, 11873, 12090, 12309, 12530, 12754, 12980, 13209, 13440, 13673, 13909,
    14146, 14387, 14629, 14874, 15122, 15371, 15623, 15878, 16135, 16394, 16656, 16920, 17187, 17456, 17727, 18001,
    18277, 18556, 18837, 19121, 19407, 19696, 19987, 20281, 20577, 20876, 21177, 21481, 21787, 22096, 22407, 22721,
    23038, 23357, 23678, 24002, 24329, 24658, 24990, 25325, 25662, 26001, 26344, 26688, 27036, 27386, 27739, 28094,
    28452, 28813, 29176, 29542, 29911, 30282, 30656, 31033, 31412, 31794, 32179, 32567, 32957, 33350, 33745, 34143,
    34544, 34948, 35355, 35764, 36176, 36591, 37008, 37429, 37852, 38278, 38706, 39138, 39572, 40009, 40449, 40891,
    41337, 41785, 42236, 42690, 43147, 43606, 44069, 44534, 45002, 45473, 45947, 46423, 46903, 47385, 47871, 48359,
    48850, 49344, 49841, 50341, 50844, 51349, 51858, 52369, 52884, 53401, 53921, 54445, 54971, 55500, 56032, 56567,
    57105, 57646, 58190, 58737, 59287, 59840, 60396, 60955, 61517, 62082, 62650, 63221, 63795, 64372, 64952, 65535,
];

/// **Lineal a sRGB de 8 bits** (N5.16, 05-10): la vuelta de la tabla de
/// arriba, para presentar un back buffer de float (scRGB, lineal) en una
/// ventana de 8 bits. El codigo cuyo lineal queda MAS CERCA; lo de fuera de
/// [0, 1] se recorta (una pantalla SDR no tiene mas), y un NaN es negro.
pub fn lineal_a_srgb8(x: f32) -> u8 {
    let v = if x > 0.0 { if x < 1.0 { (x * 65535.0 + 0.5) as u32 } else { 65535 } } else { 0 };
    let i = SRGB_A_LINEAL.partition_point(|&t| (t as u32) < v);
    match i {
        0 => 0,
        256 => 255,
        i => {
            let (a, b) = (SRGB_A_LINEAL[i - 1] as u32, SRGB_A_LINEAL[i] as u32);
            if v - a <= b - v { (i - 1) as u8 } else { i as u8 }
        }
    }
}
/// Como se filtra (`D3D12_FILTER`: cada uno de MIN, MAG y MIP).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filtro {
    Punto,
    Lineal,
}

/// Que pasa fuera de 0..1 (`D3D12_TEXTURE_ADDRESS_MODE`: 1..5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direccion {
    Repetir,
    Espejo,
    Sujetar,
    Borde,
    EspejoUnaVez,
}

/// **Un muestreador.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Muestreador {
    pub filtro: Filtro,
    pub u: Direccion,
    pub v: Direccion,
    /// El color del borde (R, G, B, A), para [`Direccion::Borde`].
    pub borde: [f32; 4],
    /// 03-10: un muestreador de COMPARACION (las sombras): su
    /// `D3D12_COMPARISON_FUNC` (2 menor, 4 menor o igual...); 0, no lo es.
    pub comparacion: u32,
    /// D4.4: como elige la mip (`filtro` es el de CERCA, el MAG).
    pub lod: Lod,
}

/// **Como elige la mip un muestreador** (D4.4, 05-10; ver la cabecera).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lod {
    /// El filtro de LEJOS (MIN, con lambda > 0); `None`, el mismo de cerca.
    pub min: Option<Filtro>,
    /// Entre mips: PUNTO la mas cercana; LINEAL, las dos de alrededor.
    pub mip: Filtro,
    /// MipLODBias, MinLOD y MaxLOD (en mips de la vista).
    pub sesgo: f32,
    pub minimo: f32,
    pub maximo: f32,
}

impl Lod {
    /// El de un muestreador hecho a mano: la mip mas cercana, sin sesgo ni
    /// limites, y el mismo filtro de cerca y de lejos.
    pub const DE_SIEMPRE: Lod = Lod { min: None, mip: Filtro::Punto, sesgo: 0.0, minimo: 0.0, maximo: f32::MAX };
}

impl Muestreador {
    /// **De un `D3D12_STATIC_SAMPLER_DESC`** (13 palabras): Filter,
    /// AddressU, V, W, MipLODBias, MaxAnisotropy, ComparisonFunc,
    /// BorderColor, MinLOD, MaxLOD, ShaderRegister, RegisterSpace,
    /// ShaderVisibility. `Err` = lo que no se sabe (se dice, no se inventa).
    pub fn de_estatico(p: &[u32; 13]) -> Result<Self, &'static str> {
        let filtro = filtro(p[0])?;
        let borde = match p[7] {
            0 => [0.0, 0.0, 0.0, 0.0],
            1 => [0.0, 0.0, 0.0, 1.0],
            2 => [1.0, 1.0, 1.0, 1.0],
            _ => return Err("un color de borde que no es de D3D12"),
        };
        let comparacion = if p[0] & COMPARACION != 0 { p[6] } else { 0 };
        let lod = lod(p[0], f32::from_bits(p[4]), f32::from_bits(p[8]), f32::from_bits(p[9]));
        Ok(Muestreador { filtro, u: direccion(p[1])?, v: direccion(p[2])?, borde, comparacion, lod })
    }

    /// **De un `D3D12_SAMPLER_DESC`** (CreateSampler: 13 palabras tambien --
    /// el borde, cuatro floats en 8..11, y MinLOD/MaxLOD en 12..13 van
    /// aparte: aqui llegan Filter, AddressU, V y los cuatro del borde; el
    /// sesgo y los limites de la mip, con [`Muestreador::con_lod`]).
    pub fn de_descriptor(filtro_d3d: u32, u: u32, v: u32, borde: [f32; 4], comparacion: u32) -> Result<Self, &'static str> {
        let comparacion = if filtro_d3d & COMPARACION != 0 { comparacion } else { 0 };
        Ok(Muestreador { filtro: filtro(filtro_d3d)?, u: direccion(u)?, v: direccion(v)?, borde, comparacion, lod: lod(filtro_d3d, 0.0, 0.0, f32::MAX) })
    }

    /// D4.4: con este MipLODBias, MinLOD y MaxLOD (un NaN, el de siempre).
    pub fn con_lod(mut self, sesgo: f32, minimo: f32, maximo: f32) -> Self {
        let l = lod(0, sesgo, minimo, maximo);
        (self.lod.sesgo, self.lod.minimo, self.lod.maximo) = (l.sesgo, l.minimo, l.maximo);
        self
    }
}

/// D4.4: lo de la mip de un `D3D12_FILTER` (MIN 0x10, MIP 0x01; el
/// anisotropico, 0x40, todo lineal) con su sesgo y sus limites.
fn lod(f: u32, sesgo: f32, minimo: f32, maximo: f32) -> Lod {
    let lineal = |bit: u32| if f & (bit | 0x40) != 0 { Filtro::Lineal } else { Filtro::Punto };
    let o = |x: f32, si_nan: f32| if x.is_nan() { si_nan } else { x };
    Lod { min: Some(lineal(0x10)), mip: lineal(0x01), sesgo: o(sesgo, 0.0), minimo: o(minimo, 0.0), maximo: o(maximo, f32::MAX) }
}

/// El bit de COMPARACION de un `D3D12_FILTER` (0x80..0xD5).
const COMPARACION: u32 = 0x80;

/// `D3D12_FILTER`: los bits de MIN, MAG y MIP (0x01 MIP, 0x04 MAG, 0x10
/// MIN) lineales o no; 0x80 comparacion; 0x40/0x55 anisotropico.
///
/// 03-10: el ANISOTROPICO (el de casi todas las texturas de Cyberpunk) se
/// lee LINEAL (trilineal desde D4.4: la mip por derivadas, ISOTROPICA; la
/// anisotropia, todavia no); antes se negaba y la textura salia negra. Este
/// es el de MAG (el de cerca); el de MIN y el de MIP, en [`Lod`]. Los de
/// comparacion, min y max (0x100, 0x180) filtran igual.
fn filtro(f: u32) -> Result<Filtro, &'static str> {
    if f & 0x40 != 0 {
        return Ok(Filtro::Lineal);
    }
    Ok(if f & 0x04 != 0 { Filtro::Lineal } else { Filtro::Punto })
}

fn direccion(d: u32) -> Result<Direccion, &'static str> {
    Ok(match d {
        1 => Direccion::Repetir,
        2 => Direccion::Espejo,
        3 => Direccion::Sujetar,
        4 => Direccion::Borde,
        5 => Direccion::EspejoUnaVez,
        _ => return Err("un modo de direccion que no es de D3D12"),
    })
}

/// `floor` sin `libm` (soft-float en Ring 3): por la conversion a entero.
pub fn suelo(x: f32) -> f32 {
    if !(x.abs() < 8_388_608.0) {
        return x; // ya es entero (o NaN, o infinito)
    }
    let t = x as i32 as f32;
    if t > x {
        t - 1.0
    } else {
        t
    }
}

/// El texel `i` de `n` segun la direccion; `None` = el borde.
fn dentro(i: i64, n: i64, d: Direccion) -> Option<i64> {
    match d {
        Direccion::Repetir => Some(i.rem_euclid(n)),
        Direccion::Sujetar => Some(i.clamp(0, n - 1)),
        Direccion::Borde => (0..n).contains(&i).then_some(i),
        Direccion::Espejo => {
            let m = i.rem_euclid(2 * n);
            Some(if m < n { m } else { 2 * n - 1 - m })
        }
        Direccion::EspejoUnaVez => {
            // `!i` es `-i - 1` sin desbordar con i64::MIN (X2, 05-10).
            let a = if i < 0 { !i } else { i };
            Some(a.min(n - 1))
        }
    }
}

/// Un canal de 8 bits en 16 (`byte * 257`: 255 -> 65535), como filtra la 3060.
fn canal16(b: u32) -> u32 {
    (b & 0xFF) * 257
}

/// El color del borde en 16 bits, CUANTIZADO al formato de 8 (la 3060:
/// 0,5 -> 127/255, medido).
fn borde16(c: f32) -> u32 {
    let c = if c.is_nan() { 0.0 } else { c.clamp(0.0, 1.0) };
    suelo(c * 255.0) as u32 * 257
}

/// **La coordenada en punto fijo**, 8 bits de fraccion y REDONDEADA:
/// `(i, a)` con `a` en 0..256.
fn fijo(x: f32) -> (i64, u32) {
    let f = suelo(x * 256.0 + 0.5) as i64;
    (f.div_euclid(256), f.rem_euclid(256) as u32)
}

impl<'a> Textura<'a> {
    /// D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING: R, G, B, A tal cual.
    pub const MAPEO: u32 = 0x1688;

    /// Una 2D de un nivel y 8 bits por canal, lineal y con el mapeo de siempre.
    pub const fn rgba(texeles: &'a [u32], ancho: u32, alto: u32, bgra: bool) -> Self {
        Textura { texeles, ancho, alto, como: if bgra { Como::Bgra8 } else { Como::Rgba8 }, srgb: false, mapeo: Self::MAPEO, mips: 1, capas: 1, hondo: 1, clase: Clase::Plana, mip: 0, capa: 0, niveles: u32::MAX, lod_min: 0.0, vista: None }
    }

    /// D4.4: las mips que ve la vista (desde `mip`), al menos una.
    pub fn niveles_vista(&self) -> u32 {
        let quedan = self.mips.max(1).saturating_sub(self.mip).max(1);
        if self.niveles == 0 {
            quedan
        } else {
            self.niveles.min(quedan)
        }
    }

    /// Si es BGRA de 8 bits (si no, se tiene por RGBA).
    pub fn bgra(&self) -> bool {
        self.como == Como::Bgra8
    }

    /// `(ancho, alto, profundidad)` de la mip `m`.
    pub fn medidas_de(&self, m: u32) -> (u32, u32, u32) {
        ((self.ancho >> m).max(1), (self.alto >> m).max(1), (self.hondo >> m).max(1))
    }

    /// Las palabras de una rebanada de la mip `m`, y de toda la mip.
    fn palabras(&self, m: u32) -> (usize, usize) {
        let (w, h, d) = self.medidas_de(m);
        let rebanada = match self.como {
            Como::Bloques(b) => w.div_ceil(4) as usize * h.div_ceil(4) as usize * b.bytes() / 4,
            Como::Flotantes4 => w as usize * h as usize * 4,
            Como::Stencil8 => (w as usize * h as usize).div_ceil(4),
            _ => w as usize * h as usize,
        };
        (rebanada, rebanada * d as usize)
    }

    /// **El plano** de la mip `m`, capa `c` y rebanada `z` (todo dentro), o
    /// `None` si la memoria no llega (una textura mal dada se lee a ceros).
    fn plano(&self, m: u32, c: u32, z: u32) -> Option<Plano<'a>> {
        let (m, c) = (m.min(self.mips.max(1) - 1), c.min(self.capas.max(1) - 1));
        let capa: usize = (0..self.mips.max(1)).map(|k| self.palabras(k).1).sum();
        let antes: usize = (0..m).map(|k| self.palabras(k).1).sum();
        let (rebanada, _) = self.palabras(m);
        let (w, h, d) = self.medidas_de(m);
        let desde = c as usize * capa + antes + z.min(d - 1) as usize * rebanada;
        let texeles = self.texeles.get(desde..desde + rebanada)?;
        Some(Plano { texeles, ancho: w, alto: h, como: self.como, srgb: self.srgb })
    }

    /// **`Sample(s, (u, v))`** en la mip y la capa de la vista, con su mapeo.
    pub fn muestrear(&self, m: &Muestreador, u: f32, v: f32) -> [f32; 4] {
        match self.plano(self.mip, self.capa, 0) {
            Some(p) => mapear(self.mapeo, p.muestrear(m, u, v, [0, 0])),
            None => [0.0; 4],
        }
    }

    /// **`Sample`/`SampleLevel` de cualquier vista** (02-10): `c` son las
    /// coordenadas del sombreador (u, v; la tercera, la capa de un array, la
    /// w de una 3D o, con u y v, la direccion de un cubo; la cuarta, el cubo
    /// de un array de cubos), `nivel` la mip pedida (SampleLevel; `None`, la
    /// mas detallada de la vista) y `desp` el desplazamiento en texeles.
    ///
    /// D4.4: `SampleLevel` elige como un muestreo (con el sesgo del
    /// muestreador, sus limites y su filtro de mip: LINEAL mezcla dos).
    /// [!] Un cubo, sin filtrar entre caras (cada cara se sujeta en su borde).
    pub fn muestrear_en(&self, m: &Muestreador, c: [f32; 4], nivel: Option<f32>, desp: [i8; 3]) -> [f32; 4] {
        match nivel {
            Some(l) => self.muestrear_lod(m, c, l + m.lod.sesgo, f32::NEG_INFINITY, desp),
            None => self.en_su_mip(m, c, desp),
        }
    }

    /// Lo de [`Textura::muestrear_en`] en la mip `mip` de la vista, con el
    /// filtro de `m` tal cual.
    fn en_su_mip(&self, m: &Muestreador, c: [f32; 4], desp: [i8; 3]) -> [f32; 4] {
        let mip = self.mip;
        let o = [desp[0] as i64, desp[1] as i64];
        let capa = |x: f32, cuantas: u32| if x.is_nan() { 0 } else { suelo(x + 0.5).clamp(0.0, cuantas.saturating_sub(1) as f32) as u32 };
        let c_ = match self.clase {
            Clase::Plana => self.plano(mip, self.capa, 0).map(|p| p.muestrear(m, c[0], c[1], o)),
            Clase::Array => self.plano(mip, self.capa + capa(c[2], self.capas - self.capa.min(self.capas)), 0).map(|p| p.muestrear(m, c[0], c[1], o)),
            Clase::Volumen => self.volumen(m, mip, c, o, desp[2] as i64),
            Clase::Cubo | Clase::CuboArray => {
                let cubo = if self.clase == Clase::CuboArray { capa(c[3], (self.capas - self.capa.min(self.capas)) / 6) } else { 0 };
                let (cara, u, v) = cara_de_cubo(c[0], c[1], c[2]);
                let s = Muestreador { u: Direccion::Sujetar, v: Direccion::Sujetar, ..*m };
                self.plano(mip, self.capa + 6 * cubo + cara, 0).map(|p| p.muestrear(&s, u, v, [0, 0]))
            }
        };
        mapear(self.mapeo, c_.unwrap_or([0.0; 4]))
    }

    /// **`Gather`** (03-10): el canal `canal` de los CUATRO texeles del
    /// cuadro de 2x2 que mezclaria un filtro lineal en `c`, en el orden de
    /// D3D: x (-u, +v), y (+u, +v), z (+u, -v), w (-u, -v). Cada uno, por
    /// el muestreador (su direccion y su borde) en su centro.
    pub fn juntar(&self, m: &Muestreador, c: [f32; 4], canal: usize, desp: [i8; 3]) -> [f32; 4] {
        let ((x0, y0), _) = self.cuadro(c);
        let (w, h, _) = self.medidas_de(self.mip);
        let p = Muestreador { filtro: Filtro::Punto, ..*m };
        let en = |dx: f32, dy: f32| self.muestrear_en(&p, [(x0 + dx + 0.5) / w as f32, (y0 + dy + 0.5) / h as f32, c[2], c[3]], None, desp)[canal & 3];
        [en(0.0, 1.0), en(1.0, 1.0), en(1.0, 0.0), en(0.0, 0.0)]
    }

    /// El texel de arriba a la izquierda del cuadro de 2x2 de `c` (en la mip
    /// de la vista) y cuanto se mete el punto en el (para los pesos).
    fn cuadro(&self, c: [f32; 4]) -> ((f32, f32), (f32, f32)) {
        let (w, h, _) = self.medidas_de(self.mip);
        let (x, y) = (c[0] * w as f32 - 0.5, c[1] * h as f32 - 0.5);
        let (x0, y0) = (suelo(x), suelo(y));
        ((x0, y0), (x - x0, y - y0))
    }

    /// **`SampleCmp`** (03-10, las sombras): `referencia` contra el canal 0
    /// de cada texel con la funcion del muestreador (`comparacion`; sin ella,
    /// MENOR O IGUAL), 1 si pasa y 0 si no; con filtro lineal, los cuatro del
    /// cuadro con sus pesos (el PCF de 2x2 de una GPU).
    pub fn comparar(&self, m: &Muestreador, c: [f32; 4], referencia: f32, desp: [i8; 3]) -> f32 {
        let f = if m.comparacion == 0 { 4 } else { m.comparacion };
        let pasa = |t: f32| if (crate::profundidad::Profundidad { funcion: f, escribir: false }).pasa(referencia, t) { 1.0 } else { 0.0 };
        if m.filtro == Filtro::Punto {
            let p = Muestreador { filtro: Filtro::Punto, ..*m };
            return pasa(self.muestrear_en(&p, c, None, desp)[0]);
        }
        let g = self.juntar(m, c, 0, desp).map(pasa);
        let (_, (fx, fy)) = self.cuadro(c);
        let arriba = g[3] + (g[2] - g[3]) * fx;
        let abajo = g[0] + (g[1] - g[0]) * fx;
        arriba + (abajo - arriba) * fy
    }

    /// **D4.4: lambda, el LOD de unos gradientes** `g` = (ddx u, ddx v,
    /// ddy u, ddy v), sin sesgo ni sujetar (lo de `CalculateLevelOfDetail
    /// Unclamped`): log2 del lado mayor del pixel en texeles de la mip mas
    /// detallada de la vista, sin raiz: `log2(max(lado_x^2, lado_y^2)) / 2`.
    /// Sin gradientes (todo 0: un carril solo), -infinito.
    pub fn lambda(&self, g: [f32; 4]) -> f32 {
        let (w, h, _) = self.medidas_de(self.mip);
        let lado = |du: f32, dv: f32| {
            let (a, b) = (du as f64 * w as f64, dv as f64 * h as f64);
            a * a + b * b
        };
        crate::mates::log2(lado(g[0], g[1]).max(lado(g[2], g[3])) as f32) * 0.5
    }

    /// D4.4: un lambda (con los sesgos ya sumados) SUJETO como D3D, en mips
    /// de la vista: a [max(MinLOD, `clamp`), MaxLOD], a las de la vista y no
    /// por debajo del ResourceMinLODClamp. Un NaN, la mas detallada.
    fn sujetar(&self, m: &Muestreador, l: f32, clamp: f32) -> f32 {
        let l = if l.is_nan() { f32::NEG_INFINITY } else { l };
        let l = l.max(m.lod.minimo.max(clamp)).min(m.lod.maximo);
        let piso = (self.lod_min - self.mip as f32).max(0.0);
        l.max(piso).min((self.niveles_vista() - 1) as f32)
    }

    /// **D4.4: la mip (o las dos) de un lambda** con sus sesgos, en mips de
    /// la vista, y el muestreador con el filtro que toca: el de cerca si el
    /// sujeto no pasa de 0, el de lejos si pasa. PUNTO: la mas cercana; LINEAL:
    /// las dos de alrededor y el peso de la segunda, en 1/256 como `fijo`.
    fn elegir(&self, m: &Muestreador, lambda: f32, clamp: f32) -> (Muestreador, u32, u32, f32) {
        let l = self.sujetar(m, lambda, clamp);
        let filtro = if l > 0.0 { m.lod.min.unwrap_or(m.filtro) } else { m.filtro };
        let m = Muestreador { filtro, ..*m };
        let ultima = self.niveles_vista() - 1;
        if m.lod.mip == Filtro::Punto {
            let k = (suelo(l + 0.5) as u32).min(ultima);
            return (m, k, k, 0.0);
        }
        let (k, f) = fijo(l);
        let k = (k.max(0) as u32).min(ultima);
        if k == ultima || f == 0 {
            return (m, k, k, 0.0);
        }
        (m, k, k + 1, f as f32 / 256.0)
    }

    /// D4.4: muestrear con el lambda `lambda` (sus sesgos ya sumados) y el
    /// clamp del sombreador; con dos mips, mezcladas.
    fn muestrear_lod(&self, m: &Muestreador, c: [f32; 4], lambda: f32, clamp: f32, desp: [i8; 3]) -> [f32; 4] {
        let (m, a, b, f) = self.elegir(m, lambda, clamp);
        let en = |k: u32| Textura { mip: self.mip + k, ..*self }.en_su_mip(&m, c, desp);
        let x = en(a);
        if a == b {
            return x;
        }
        let y = en(b);
        core::array::from_fn(|q| x[q] + (y[q] - x[q]) * f)
    }

    /// **D4.4: `Sample`, `SampleBias` y `SampleGrad`**: en la mip de los
    /// gradientes `g`, con el sesgo del sombreador y su clamp (`sesgo_clamp`)
    /// y lo del muestreador. Un cubo, en la mip de la vista (ver la cabecera).
    pub fn muestrear_grad(&self, m: &Muestreador, c: [f32; 4], g: [f32; 4], sesgo_clamp: [f32; 2], desp: [i8; 3]) -> [f32; 4] {
        if matches!(self.clase, Clase::Cubo | Clase::CuboArray) {
            return self.en_su_mip(m, c, desp);
        }
        self.muestrear_lod(m, c, self.lambda(g) + sesgo_clamp[0] + m.lod.sesgo, sesgo_clamp[1], desp)
    }

    /// **D4.4: `SampleCmp`**: [`Textura::comparar`] en la mip (o las dos,
    /// mezcladas) de los gradientes `g`, como [`Textura::muestrear_grad`].
    #[allow(clippy::too_many_arguments)]
    pub fn comparar_grad(&self, m: &Muestreador, c: [f32; 4], referencia: f32, g: [f32; 4], sesgo_clamp: [f32; 2], desp: [i8; 3]) -> f32 {
        if matches!(self.clase, Clase::Cubo | Clase::CuboArray) {
            return self.comparar(m, c, referencia, desp);
        }
        let (m, a, b, f) = self.elegir(m, self.lambda(g) + sesgo_clamp[0] + m.lod.sesgo, sesgo_clamp[1]);
        let en = |k: u32| Textura { mip: self.mip + k, ..*self }.comparar(&m, c, referencia, desp);
        let x = en(a);
        if a == b {
            x
        } else {
            x + (en(b) - x) * f
        }
    }

    /// **D4.4: `CalculateLevelOfDetail`** (`sujeta`) y su `Unclamped`: el
    /// lambda de los gradientes `g` con el sesgo del muestreador; sujeto, como
    /// lo sujeta un muestreo, sin redondear (con mip PUNTO o LINEAL, igual).
    pub fn lod(&self, m: &Muestreador, g: [f32; 4], sujeta: bool) -> f32 {
        let l = self.lambda(g) + m.lod.sesgo;
        if sujeta {
            self.sujetar(m, l, f32::NEG_INFINITY)
        } else {
            l
        }
    }

    /// Una 3D: las dos rebanadas de alrededor de `w`, mezcladas si el
    /// filtro es lineal (sujetas en los extremos).
    fn volumen(&self, m: &Muestreador, mip: u32, c: [f32; 4], o: [i64; 2], oz: i64) -> Option<[f32; 4]> {
        let (_, _, d) = self.medidas_de(mip.min(self.mips.max(1) - 1));
        let w = if c[2].is_nan() { 0.0 } else { c[2] * d as f32 };
        let z = |k: i64| (k + oz).clamp(0, d as i64 - 1) as u32;
        match m.filtro {
            Filtro::Punto => Some(self.plano(mip, 0, z(suelo(w) as i64))?.muestrear(m, c[0], c[1], o)),
            Filtro::Lineal => {
                let (k, f) = fijo(w - 0.5);
                let (a, b) = (self.plano(mip, 0, z(k))?.muestrear(m, c[0], c[1], o), self.plano(mip, 0, z(k + 1))?.muestrear(m, c[0], c[1], o));
                let f = f as f32 / 256.0;
                Some(core::array::from_fn(|q| a[q] + (b[q] - a[q]) * f))
            }
        }
    }

    /// **`Load` (`tN.Load(int3(x, y, mip))`)**: el texel de coordenadas
    /// ENTERAS `c` (x, y, y la capa de un array o la rebanada de una 3D) de
    /// la mip `mip` de la vista, sin filtrar ni muestreador; fuera, ceros
    /// (como D3D). `enteros`: un formato entero (`.i32`): los canales de 8
    /// bits, como enteros (bits); si no, como floats 0..1.
    pub fn cargar(&self, c: [i32; 3], mip: i32, desp: [i8; 3], enteros: bool) -> [u32; 4] {
        let m = self.mip as i64 + mip as i64;
        if m < 0 || m >= self.mips as i64 {
            return [0; 4];
        }
        let (w, h, d) = self.medidas_de(m as u32);
        let (x, y, z) = (c[0] as i64 + desp[0] as i64, c[1] as i64 + desp[1] as i64, c[2] as i64 + desp[2] as i64);
        let (capa, rebanada) = match self.clase {
            Clase::Volumen => (0, z),
            Clase::Array | Clase::CuboArray | Clase::Cubo => (self.capa as i64 + z, 0),
            Clase::Plana => (self.capa as i64, 0),
        };
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 || rebanada < 0 || rebanada >= d as i64 || capa < 0 || capa >= self.capas as i64 {
            return [0; 4];
        }
        let Some(p) = self.plano(m as u32, capa as u32, rebanada as u32) else { return [0; 4] };
        let crudo = match (self.vista, p.como) {
            (Some((g, v)), Como::Flotantes4) => {
                use crate::formato_ia::{empaquetar, es_entero, leer};
                match empaquetar(g, p.flotante(x, y).map(f32::to_bits), es_entero(g)) {
                    Some(b) => leer(v, &b).map(f32::to_bits),
                    None => [0; 4],
                }
            }
            (Some((_, v)), Como::Rgba8 | Como::Bgra8 | Como::Flotante) => crate::formato_ia::leer(v, &p.palabra(x, y).to_le_bytes()).map(f32::to_bits),
            _ => self.cargar_guardado(&p, x, y, w, enteros),
        };
        // El mapeo, sobre los bits: un 1 es 1 en un entero, 1.0 en un float.
        let entero_visto = self.vista.is_some_and(|(_, v)| crate::formato_ia::es_entero(v));
        let uno = if entero_visto || (enteros && self.vista.is_none() && !matches!(p.como, Como::Flotante | Como::Flotantes4)) { 1 } else { 1.0f32.to_bits() };
        if self.mapeo & 0xFFF == Self::MAPEO & 0xFFF {
            return crudo;
        }
        core::array::from_fn(|k| match (self.mapeo >> (3 * k)) & 7 {
            s @ 0..=3 => crudo[s as usize],
            5 => uno,
            _ => 0,
        })
    }

    /// El texel `(x, y)` de `p` como lo guarda la casa (lo de [`Textura::cargar`]
    /// sin otra vista).
    fn cargar_guardado(&self, p: &Plano, x: i64, y: i64, w: u32, enteros: bool) -> [u32; 4] {
        match p.como {
            Como::Flotante => [p.texeles.get((y * w as i64 + x) as usize).copied().unwrap_or(0), 0, 0, 1.0f32.to_bits()],
            Como::Flotantes4 => p.flotante(x, y).map(f32::to_bits),
            Como::Stencil8 => {
                let i = (y * w as i64 + x) as usize;
                [0, p.texeles.get(i / 4).map_or(0, |q| q >> (8 * (i % 4)) & 0xFF), 0, 1]
            }
            _ if enteros => {
                let q = p.palabra(x, y);
                let (r, g, b, a) = if p.como == Como::Bgra8 { (q >> 16, q >> 8, q, q >> 24) } else { (q, q >> 8, q >> 16, q >> 24) };
                [r & 0xFF, g & 0xFF, b & 0xFF, a & 0xFF]
            }
            _ => p.texel(x, y).map(|v| (v as f32 / 65535.0).to_bits()),
        }
    }

    /// **`GetDimensions(mip)`** de la vista, como DXIL: `(ancho, alto,
    /// profundidad o capas, mips)` (en x lo que tenga cada clase; las mips,
    /// siempre en w).
    pub fn medidas(&self, mip: u32) -> [u32; 4] {
        let m = self.mip.saturating_add(mip);
        // D4.4: las de la VISTA (su MipLevels), no todas las que quedan.
        let mips = self.niveles_vista();
        if mip >= mips || m >= self.mips {
            return [0, 0, 0, mips];
        }
        let (w, h, d) = self.medidas_de(m);
        let capas = self.capas.saturating_sub(self.capa);
        match self.clase {
            Clase::Plana | Clase::Cubo => [w, h, 0, mips],
            Clase::Array => [w, h, capas, mips],
            Clase::Volumen => [w, h, d, mips],
            Clase::CuboArray => [w, h, capas / 6, mips],
        }
    }
}

/// **La cara de un cubo** que mira la direccion `(x, y, z)`, y sus `(u, v)`
/// en ella (el eje mayor, como D3D: +X, -X, +Y, -Y, +Z, -Z).
pub fn cara_de_cubo(x: f32, y: f32, z: f32) -> (u32, f32, f32) {
    let (ax, ay, az) = (x.abs(), y.abs(), z.abs());
    let (cara, sc, tc, ma) = if ax >= ay && ax >= az {
        if x >= 0.0 {
            (0, -z, -y, ax)
        } else {
            (1, z, -y, ax)
        }
    } else if ay >= az {
        if y >= 0.0 {
            (2, x, z, ay)
        } else {
            (3, x, -z, ay)
        }
    } else if z >= 0.0 {
        (4, x, -y, az)
    } else {
        (5, -x, -y, az)
    };
    if !(ma > 0.0) {
        return (0, 0.5, 0.5);
    }
    (cara, (sc / ma + 1.0) * 0.5, (tc / ma + 1.0) * 0.5)
}

impl Plano<'_> {
    /// La palabra de 4 bytes del texel `(x, y)` en 8 bits por canal
    /// (`0xAABBGGRR`); un BC, descomprimiendo su bloque.
    fn palabra(&self, x: i64, y: i64) -> u32 {
        match self.como {
            Como::Bloques(tipo) => {
                let n = tipo.bytes() / 4;
                let fila = (self.ancho as usize).div_ceil(4);
                let i = ((y / 4) as usize * fila + (x / 4) as usize) * n;
                let Some(w) = self.texeles.get(i..i + n) else { return 0 };
                let mut b = [0u8; 16];
                for (k, p) in w.iter().enumerate() {
                    b[4 * k..4 * k + 4].copy_from_slice(&p.to_le_bytes());
                }
                bc::bloque(tipo, &b[..4 * n])[((y % 4) * 4 + x % 4) as usize]
            }
            _ => self.texeles.get((y * self.ancho as i64 + x) as usize).copied().unwrap_or(0),
        }
    }

    /// El texel `(x, y)` ya dentro, como (R, G, B, A) en 16 bits.
    fn texel(&self, x: i64, y: i64) -> [u32; 4] {
        let p = self.palabra(x, y);
        let (r, g, b, a) = if self.como == Como::Bgra8 { (p >> 16, p >> 8, p, p >> 24) } else { (p, p >> 8, p >> 16, p >> 24) };
        let color = |c: u32| if self.srgb { SRGB_A_LINEAL[(c & 0xFF) as usize] as u32 } else { canal16(c) };
        [color(r), color(g), color(b), canal16(a)]
    }

    /// El texel `(x, y)` de una de floats: `(r, 0, 0, 1)` de un R32, los
    /// cuatro de una de `Flotantes4`.
    fn flotante(&self, x: i64, y: i64) -> [f32; 4] {
        let i = (y * self.ancho as i64 + x) as usize;
        if self.como == Como::Flotantes4 {
            return match self.texeles.get(4 * i..4 * i + 4) {
                Some(t) => [f32::from_bits(t[0]), f32::from_bits(t[1]), f32::from_bits(t[2]), f32::from_bits(t[3])],
                None => [0.0; 4],
            };
        }
        let r = self.texeles.get(i).map_or(0.0, |&p| f32::from_bits(p));
        [r, 0.0, 0.0, 1.0]
    }

    /// El texel `(i, j)` con las direcciones del muestreador (o su borde),
    /// en 16 bits.
    fn leer(&self, m: &Muestreador, i: i64, j: i64) -> [u32; 4] {
        match (dentro(i, self.ancho as i64, m.u), dentro(j, self.alto as i64, m.v)) {
            (Some(x), Some(y)) => self.texel(x, y),
            _ => m.borde.map(borde16),
        }
    }

    /// Como [`Self::leer`], en float (una de floats; el borde, tal cual).
    fn leer_f(&self, m: &Muestreador, i: i64, j: i64) -> [f32; 4] {
        match (dentro(i, self.ancho as i64, m.u), dentro(j, self.alto as i64, m.v)) {
            (Some(x), Some(y)) => self.flotante(x, y),
            _ => m.borde,
        }
    }

    /// `Sample` en este plano, sin mapeo; `o`, el desplazamiento en texeles.
    fn muestrear(&self, m: &Muestreador, u: f32, v: f32, o: [i64; 2]) -> [f32; 4] {
        if self.ancho == 0 || self.alto == 0 || u.is_nan() || v.is_nan() {
            return [0.0; 4];
        }
        let (su, sv) = (u * self.ancho as f32, v * self.alto as f32);
        let a_float = |c: [u32; 4]| c.map(|x| x as f32 / 65535.0);
        let flot = matches!(self.como, Como::Flotante | Como::Flotantes4);
        match m.filtro {
            Filtro::Punto => {
                // X2 (05-10): `wrapping_add`: con una coordenada enorme (o
                // infinita: `as i64` satura) y un desplazamiento, el `+` del
                // banco paraba (debug); en el metal (release) daba la vuelta.
                // Lo mismo que en el metal, sin parar.
                let (i, j) = ((suelo(su) as i64).wrapping_add(o[0]), (suelo(sv) as i64).wrapping_add(o[1]));
                if flot {
                    self.leer_f(m, i, j)
                } else {
                    a_float(self.leer(m, i, j))
                }
            }
            Filtro::Lineal => {
                let ((i0, a), (j0, b)) = (fijo(su - 0.5), fijo(sv - 0.5));
                let (i0, j0) = (i0 + o[0], j0 + o[1]);
                // Los pesos de las cuatro esquinas, cuantizados a 1/256.
                let w = |x: u32| (x + 128) / 256;
                let pesos = [w((256 - a) * (256 - b)), w(a * (256 - b)), w((256 - a) * b), w(a * b)];
                let puntos = [(i0, j0), (i0 + 1, j0), (i0, j0 + 1), (i0 + 1, j0 + 1)];
                if flot {
                    let e = puntos.map(|(i, j)| self.leer_f(m, i, j));
                    return core::array::from_fn(|k| (0..4).map(|q| e[q][k] * pesos[q] as f32).sum::<f32>() / 256.0);
                }
                let esquinas = puntos.map(|(i, j)| self.leer(m, i, j));
                a_float(core::array::from_fn(|k| {
                    let suma: u32 = (0..4).map(|q| esquinas[q][k] * pesos[q]).sum();
                    (suma + 128) / 256
                }))
            }
        }
    }
}

/// El mapeo de componentes de una vista sobre `(r, g, b, a)`.
fn mapear(mapeo: u32, c: [f32; 4]) -> [f32; 4] {
    if mapeo & 0xFFF == Textura::MAPEO & 0xFFF {
        return c;
    }
    core::array::from_fn(|k| match (mapeo >> (3 * k)) & 7 {
        s @ 0..=3 => c[s as usize],
        5 => 1.0,
        _ => 0.0,
    })
}

/// **Lo que un dibujo le da a sus sombreadores**: las texturas por su
/// registro (tN) y los muestreadores por el suyo (sN).
#[derive(Clone, Copy, Debug)]
pub struct Recursos<'a> {
    pub texturas: &'a [Option<Textura<'a>>],
    pub muestreadores: &'a [Option<Muestreador>],
    /// N5.3: los SRV que son BUFERES, en la misma ranura que las texturas
    /// (un SRV es una cosa u otra: la otra se queda en `None`).
    pub buferes: &'a [Option<crate::bufer::Bufer<'a>>],
    /// N5.4: las texturas de un rango con el registro CALCULADO (un array de
    /// texturas, o bindless), que se buscan al correr: ver [`Dinamicas`].
    pub dinamicas: Option<Dinamicas<'a>>,
}

/// **Quien busca una textura por su registro calculado** (N5.4, 05-10):
/// `(rango, registro)` -- el rango, el de `Ranuras::dinamicas` del programa;
/// el registro, el absoluto que calculo el sombreador (la base del rango
/// incluida) -- y su textura, o `None` (se lee como un SRV nulo: ceros).
/// Lo da quien dibuja (la casa, que lo busca en la root signature y el
/// monton), una vez por textura distinta y no por pixel si sabe guardarlo.
/// Sus texturas son `'static` (la memoria del proceso): asi `Recursos` sigue
/// siendo covariante y el interprete puede prestar la elegida un momento.
///
/// 15 de la pila A (07-10): y, si lo hay, quien busca un BUFER (un array de
/// `StructuredBuffer`/`ByteAddressBuffer`/`Buffer` con el registro
/// calculado: los buferes "bindless").
#[derive(Clone, Copy)]
pub struct Dinamicas<'a>(pub &'a dyn Fn(u8, u32) -> Option<Textura<'static>>, pub Option<&'a dyn Fn(u8, u32) -> Option<crate::bufer::Bufer<'static>>>);

/// **Lo ELEGIDO por un `EligeTextura`** (N5.4; 15, 07-10): la textura o el
/// bufer del registro calculado. Lo que lee `DINAMICA` lo lee de aqui.
#[derive(Debug, Clone, Copy)]
pub struct Elegida {
    pub textura: Option<Textura<'static>>,
    pub bufer: Option<crate::bufer::Bufer<'static>>,
}

impl core::fmt::Debug for Dinamicas<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Dinamicas(..)")
    }
}

impl<'a> Recursos<'a> {
    pub const NINGUNO: Recursos<'static> = Recursos { texturas: &[], muestreadores: &[], buferes: &[], dinamicas: None };

    /// **La textura del registro `registro` del rango dinamico `rango`**, o
    /// `None` (sin quien busque, o sin textura alli).
    pub fn dinamica(&self, rango: u8, registro: u32) -> Option<Textura<'static>> {
        self.dinamicas.and_then(|d| (d.0)(rango, registro))
    }

    /// **Lo del registro `registro` del rango dinamico `rango`** (15,
    /// 07-10): su textura y su bufer (uno de los dos, o ninguno).
    pub fn elegir(&self, rango: u8, registro: u32) -> Option<Elegida> {
        let d = self.dinamicas?;
        Some(Elegida { textura: (d.0)(rango, registro), bufer: d.1.and_then(|b| b(rango, registro)) })
    }

    /// `Load` del bufer tN (ver [`crate::bufer::Bufer::cargar`]); sin
    /// bufer, ceros.
    pub fn cargar_bufer(&self, t: u8, modo: crate::bufer::Modo, i: u32, desp: u32) -> [u32; 4] {
        match self.buferes.get(t as usize) {
            Some(Some(b)) => b.cargar(modo, i, desp),
            _ => [0; 4],
        }
    }

    /// `GetDimensions` del bufer tN; sin bufer, ceros.
    pub fn medidas_bufer(&self, t: u8, modo: crate::bufer::Modo) -> [u32; 4] {
        match self.buferes.get(t as usize) {
            Some(Some(b)) => b.medidas(modo),
            _ => [0; 4],
        }
    }

    /// `Sample(tN, sM, (u, v))`; sin textura o sin muestreador, ceros.
    pub fn muestrear(&self, t: u8, s: u8, u: f32, v: f32) -> [f32; 4] {
        match (self.texturas.get(t as usize), self.muestreadores.get(s as usize)) {
            (Some(Some(tx)), Some(Some(m))) => tx.muestrear(m, u, v),
            _ => [0.0; 4],
        }
    }

    /// `Sample`/`SampleLevel` de cualquier vista (ver [`Textura::muestrear_en`]).
    pub fn muestrear_en(&self, t: u8, s: u8, c: [f32; 4], nivel: Option<f32>, desp: [i8; 3]) -> [f32; 4] {
        match (self.texturas.get(t as usize), self.muestreadores.get(s as usize)) {
            (Some(Some(tx)), Some(Some(m))) => tx.muestrear_en(m, c, nivel, desp),
            _ => [0.0; 4],
        }
    }

    /// D4.4: `Sample` con gradientes (ver [`Textura::muestrear_grad`]).
    pub fn muestrear_grad(&self, t: u8, s: u8, c: [f32; 4], g: [f32; 4], sesgo_clamp: [f32; 2], desp: [i8; 3]) -> [f32; 4] {
        match (self.texturas.get(t as usize), self.muestreadores.get(s as usize)) {
            (Some(Some(tx)), Some(Some(m))) => tx.muestrear_grad(m, c, g, sesgo_clamp, desp),
            _ => [0.0; 4],
        }
    }

    /// D4.4: `SampleCmp` con gradientes (ver [`Textura::comparar_grad`]).
    #[allow(clippy::too_many_arguments)]
    pub fn comparar_grad(&self, t: u8, s: u8, c: [f32; 4], referencia: f32, g: [f32; 4], sesgo_clamp: [f32; 2], desp: [i8; 3]) -> f32 {
        match (self.texturas.get(t as usize), self.muestreadores.get(s as usize)) {
            (Some(Some(tx)), Some(Some(m))) => tx.comparar_grad(m, c, referencia, g, sesgo_clamp, desp),
            _ => 0.0,
        }
    }

    /// D4.4: `CalculateLevelOfDetail` (ver [`Textura::lod`]); sin textura o
    /// sin muestreador, 0.
    pub fn lod(&self, t: u8, s: u8, g: [f32; 4], sujeta: bool) -> f32 {
        match (self.texturas.get(t as usize), self.muestreadores.get(s as usize)) {
            (Some(Some(tx)), Some(Some(m))) => tx.lod(m, g, sujeta),
            _ => 0.0,
        }
    }

    /// **D4.4: si la mip por derivadas cambia lo que lee este dibujo**:
    /// alguna vista con mas de una mip, o algun muestreador con el filtro de
    /// lejos distinto del de cerca. La puerta de la 3060 lo pregunta (su TEX
    /// lee la mip 0).
    pub fn mip_importa(&self) -> bool {
        self.texturas.iter().flatten().any(|t| t.niveles_vista() > 1) || self.muestreadores.iter().flatten().any(|m| m.lod.min.is_some_and(|f| f != m.filtro))
    }

    /// `Gather` de tN con sN (ver [`Textura::juntar`]); sin ellos, ceros.
    pub fn juntar(&self, t: u8, s: u8, c: [f32; 4], canal: usize, desp: [i8; 3]) -> [f32; 4] {
        match (self.texturas.get(t as usize), self.muestreadores.get(s as usize)) {
            (Some(Some(tx)), Some(Some(m))) => tx.juntar(m, c, canal, desp),
            _ => [0.0; 4],
        }
    }

    /// `SampleCmp` de tN con sN (ver [`Textura::comparar`]); sin ellos, 0.
    pub fn comparar(&self, t: u8, s: u8, c: [f32; 4], referencia: f32, desp: [i8; 3]) -> f32 {
        match (self.texturas.get(t as usize), self.muestreadores.get(s as usize)) {
            (Some(Some(tx)), Some(Some(m))) => tx.comparar(m, c, referencia, desp),
            _ => 0.0,
        }
    }

    /// `Load` de tN (ver [`Textura::cargar`]); sin textura, ceros.
    pub fn cargar(&self, t: u8, c: [i32; 3], mip: i32, desp: [i8; 3], enteros: bool) -> [u32; 4] {
        match self.texturas.get(t as usize) {
            Some(Some(tx)) => tx.cargar(c, mip, desp, enteros),
            _ => [0; 4],
        }
    }

    /// `GetDimensions` de tN; sin textura, ceros (como un SRV nulo).
    pub fn medidas(&self, t: u8, mip: u32) -> [u32; 4] {
        match self.texturas.get(t as usize) {
            Some(Some(tx)) => tx.medidas(mip),
            _ => [0; 4],
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const PUNTO_BORDE: Muestreador = Muestreador { filtro: Filtro::Punto, u: Direccion::Borde, v: Direccion::Borde, borde: [0.0, 0.0, 0.0, 0.0], comparacion: 0, lod: Lod::DE_SIEMPRE };

    #[test]
    fn suelo_como_floor() {
        for x in [-2.5f32, -2.0, -0.5, -0.0, 0.0, 0.25, 0.999, 1.0, 7.5, 1e9, -1e9] {
            assert_eq!(suelo(x), x.floor(), "{x}");
        }
    }

    #[test]
    fn punto_el_texel_de_floor_y_el_borde_fuera() {
        // 2x2 RGBA: rojo, verde / azul, blanco.
        let t = [0xFF00_00FF, 0xFF00_FF00, 0xFFFF_0000, 0xFFFF_FFFF];
        let tx = Textura::rgba(&t, 2, 2, false);
        assert_eq!(tx.muestrear(&PUNTO_BORDE, 0.25, 0.25), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(tx.muestrear(&PUNTO_BORDE, 0.75, 0.25), [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(tx.muestrear(&PUNTO_BORDE, 0.25, 0.75), [0.0, 0.0, 1.0, 1.0]);
        // El centro exacto (0.5) es del texel 1.
        assert_eq!(tx.muestrear(&PUNTO_BORDE, 0.5, 0.5), [1.0, 1.0, 1.0, 1.0]);
        // Fuera: el borde (negro transparente).
        assert_eq!(tx.muestrear(&PUNTO_BORDE, 1.25, 0.25), [0.0; 4]);
        assert_eq!(tx.muestrear(&PUNTO_BORDE, -0.01, 0.25), [0.0; 4]);
        // Repetir y sujetar.
        let rep = Muestreador { u: Direccion::Repetir, v: Direccion::Repetir, ..PUNTO_BORDE };
        assert_eq!(tx.muestrear(&rep, 1.25, 0.25), [1.0, 0.0, 0.0, 1.0]);
        let suj = Muestreador { u: Direccion::Sujetar, v: Direccion::Sujetar, ..PUNTO_BORDE };
        assert_eq!(tx.muestrear(&suj, 7.0, -3.0), [0.0, 1.0, 0.0, 1.0]);
        // BGRA: el mismo pixel en memoria dice otro color.
        let bg = Textura { como: Como::Bgra8, ..tx };
        assert_eq!(bg.muestrear(&PUNTO_BORDE, 0.25, 0.25), [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn lineal_mezcla_los_cuatro() {
        let t = [0xFF00_0000, 0xFFFF_FFFF, 0xFF00_0000, 0xFFFF_FFFF];
        let tx = Textura::rgba(&t, 2, 2, false);
        let m = Muestreador { filtro: Filtro::Lineal, u: Direccion::Sujetar, v: Direccion::Sujetar, borde: [0.0; 4], comparacion: 0, lod: Lod::DE_SIEMPRE };
        // Justo entre los dos centros: la mitad, como la da la 3060 (en 16
        // bits, redondeada: 32768/65535, no 0,5 exacto).
        assert_eq!(tx.muestrear(&m, 0.5, 0.5)[0], 32768.0 / 65535.0);
        // En el centro de un texel: el texel.
        assert_eq!(tx.muestrear(&m, 0.25, 0.25)[0], 0.0);
        assert_eq!(tx.muestrear(&m, 0.75, 0.75)[0], 1.0);
    }

    #[test]
    fn lineal_a_srgb8_es_la_vuelta_de_la_tabla() {
        for c in 0..=255u8 {
            assert_eq!(lineal_a_srgb8(SRGB_A_LINEAL[c as usize] as f32 / 65535.0), c, "ida y vuelta de {c}");
        }
        // La curva: 0,5 lineal es 188 en sRGB (round(255 * 0,7354)); 0,2 es 124.
        assert_eq!((lineal_a_srgb8(0.5), lineal_a_srgb8(0.2)), (188, 124));
        assert_eq!((lineal_a_srgb8(-1.0), lineal_a_srgb8(7.0), lineal_a_srgb8(f32::NAN)), (0, 255, 0));
    }

    #[test]
    fn bc_srgb_mapeo_floats_y_cubos() {
        // Un BC1 de 4x4 de un rojo 565 (c0 = c1, indices a 0): 255, 0, 0.
        let bloque = [0x00u8, 0xF8, 0x00, 0xF8, 0, 0, 0, 0];
        let w = [u32::from_le_bytes(bloque[..4].try_into().unwrap()), u32::from_le_bytes(bloque[4..].try_into().unwrap())];
        let bc = Textura { como: Como::Bloques(Bc::Bc1), ..Textura::rgba(&w, 4, 4, false) };
        assert_eq!(bc.muestrear(&PUNTO_BORDE, 0.6, 0.3), [1.0, 0.0, 0.0, 1.0]);
        // De 2x2 (la mip de una textura mayor): un bloque igual.
        let chica = Textura { ancho: 2, alto: 2, ..bc };
        assert_eq!(chica.muestrear(&PUNTO_BORDE, 0.9, 0.9), [1.0, 0.0, 0.0, 1.0]);
        // sRGB: 128 es 0,2158 en lineal (16 bits); el alfa no se toca.
        let gris = [0x80_80_80_80u32];
        let s = Textura { srgb: true, ..Textura::rgba(&gris, 1, 1, false) };
        let c = s.muestrear(&PUNTO_BORDE, 0.5, 0.5);
        assert_eq!(c[0], 14146.0 / 65535.0);
        assert_eq!(c[3], 128.0 * 257.0 / 65535.0);
        // El mapeo: (B, G, R, 1) -- 2 | 1 << 3 | 0 << 6 | 5 << 9 (y el bit 12).
        let rgba = [0x40_30_20_10u32];
        let m = Textura { mapeo: 2 | 1 << 3 | 5 << 9 | 1 << 12, ..Textura::rgba(&rgba, 1, 1, false) };
        let c = m.muestrear(&PUNTO_BORDE, 0.5, 0.5);
        assert_eq!(c.map(|x| (x * 255.0 + 0.5) as u32), [0x30, 0x20, 0x10, 255]);
        // Un float (R32): (r, 0, 0, 1), y el lineal mezcla en float.
        let f = [0.25f32.to_bits(), 0.75f32.to_bits()];
        let fl = Textura { como: Como::Flotante, ..Textura::rgba(&f, 2, 1, false) };
        assert_eq!(fl.muestrear(&PUNTO_BORDE, 0.75, 0.5), [0.75, 0.0, 0.0, 1.0]);
        let lin = Muestreador { filtro: Filtro::Lineal, u: Direccion::Sujetar, v: Direccion::Sujetar, borde: [0.0; 4], comparacion: 0, lod: Lod::DE_SIEMPRE };
        assert_eq!(fl.muestrear(&lin, 0.5, 0.5)[0], 0.5);
        // N5.16: cuatro floats por texel (un HDR): mas de 1 y negativos, tal
        // cual, y el lineal mezcla en float.
        let h: alloc::vec::Vec<u32> = [[3.5f32, -0.5, 0.0, 1.0], [1.5, 0.5, 8.0, 0.5]].iter().flatten().map(|x| x.to_bits()).collect();
        let hdr = Textura { como: Como::Flotantes4, ..Textura::rgba(&h, 2, 1, false) };
        assert_eq!(hdr.muestrear(&PUNTO_BORDE, 0.25, 0.5), [3.5, -0.5, 0.0, 1.0]);
        assert_eq!(hdr.muestrear(&lin, 0.5, 0.5), [2.5, 0.0, 4.0, 0.75]);
        // 05-10: el plano de stencil, un byte por texel: Load da el de (x, y)
        // en G, entero (5 x 1: el quinto en la segunda palabra).
        let st = [0x44_33_22_11u32, 0x55];
        let p = Textura { como: Como::Stencil8, ..Textura::rgba(&st, 5, 1, false) };
        assert_eq!(p.cargar([2, 0, 0], 0, [0; 3], true), [0, 0x33, 0, 1]);
        assert_eq!(p.cargar([4, 0, 0], 0, [0; 3], true), [0, 0x55, 0, 1]);
        // Las caras de un cubo: el eje mayor y su signo.
        assert_eq!(cara_de_cubo(1.0, 0.0, 0.0), (0, 0.5, 0.5));
        assert_eq!(cara_de_cubo(-1.0, 0.5, 0.0), (1, 0.5, 0.25));
        assert_eq!(cara_de_cubo(0.0, 1.0, 0.5), (2, 0.5, 0.75));
        assert_eq!(cara_de_cubo(0.0, -1.0, 0.5), (3, 0.5, 0.25));
        assert_eq!(cara_de_cubo(0.5, 0.0, 1.0), (4, 0.75, 0.5));
        assert_eq!(cara_de_cubo(0.5, 0.0, -1.0), (5, 0.25, 0.5));
    }

    #[test]
    fn el_muestreador_de_hellotexture() {
        // D3D12_FILTER_MIN_MAG_MIP_POINT, BORDER x3, TRANSPARENT_BLACK.
        let m = Muestreador::de_estatico(&[0, 4, 4, 4, 0, 0, 1, 0, 0, 0x7F7F_FFFF, 0, 0, 5]).unwrap();
        // D4.4: con lo de su mip (MIN y MIP punto, sin sesgo ni limites).
        assert_eq!(m, Muestreador { lod: Lod { min: Some(Filtro::Punto), ..Lod::DE_SIEMPRE }, ..PUNTO_BORDE });
        assert_eq!(Muestreador::de_estatico(&[0x15, 3, 3, 3, 0, 0, 1, 2, 0, 0, 0, 0, 0]).unwrap().filtro, Filtro::Lineal);
        // 03-10: el anisotropico se lee lineal (antes se negaba: negro), y el
        // de comparacion guarda su funcion (aqui LESS_EQUAL, 4).
        assert_eq!(Muestreador::de_estatico(&[0x55, 1, 1, 1, 0, 16, 1, 0, 0, 0, 0, 0, 0]).unwrap().filtro, Filtro::Lineal);
        let c = Muestreador::de_estatico(&[0x95, 1, 1, 1, 0, 0, 4, 0, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!((c.filtro, c.comparacion), (Filtro::Lineal, 4));
        assert_eq!(Muestreador::de_estatico(&[0x80, 1, 1, 1, 0, 0, 4, 0, 0, 0, 0, 0, 0]).unwrap().comparacion, 4);
        assert_eq!(m.comparacion, 0, "sin el bit 0x80, no compara");
    }
}
