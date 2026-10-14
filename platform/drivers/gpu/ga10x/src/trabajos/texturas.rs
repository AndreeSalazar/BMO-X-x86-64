//! **P3b4c.8 T0: LAS TEXTURAS DE LA 3060, SUS DESCRIPTORES** -- lo que la
//! 3060 necesita saber de una textura (el TIC, "texture image control") y de
//! un muestreador (el TSC, "texture sampler control") para que un `TEX` del
//! programa de pixel lea de ella. Hoy un PSO que muestrea se dibuja en la CPU
//! (`bmo_proton_x::textura`); esto es el primer escalon para que lo haga la
//! tarjeta.
//!
//! capa: puro -- 32 bytes por descriptor y las ordenes que los apuntan; la
//! VRAM, la IOMMU y el `TEX` del programa vienen en T1..T3
//!
//! [eje]     CORRECCION -- los campos de `gm107_texture.xml.h` de nouveau
//!           (Maxwell en adelante: el mismo formato "TEXHEAD V2" en Ampere)
//!           y los numeros de D3D12 traducidos uno a uno. [!] NADA DE ESTO
//!           HA CORRIDO EN EL METAL: cada numero dice de donde sale, y el
//!           banco comprueba que cada campo cae en sus bits, no que la 3060
//!           lo entienda.
//!
//! # La escalera
//!
//! ```text
//!    T0  [esto] el TIC y el TSC de una textura 2D de 8 bits por canal, pitch,
//!        y de un muestreador de D3D12 (punto/lineal, los cinco modos, borde)
//!    T1  el `TEX` en SASS: su codificacion, sacada de un binario que YA
//!        corre (nvdisasm de un .cubin de sm_86, en el Windows del
//!        propietario), al corpus de oro y al juez (R8: el asa de la textura
//!        la pone el KERNEL, como las cargas del pegamento)
//!    T2  el kernel: las piscinas en VRAM (SET_TEX_HEADER_POOL / SAMPLER_POOL),
//!        los texeles prestados por la IOMMU como el destino, y la receta
//!        (VRN3) con sus texturas
//!    T3  el metal: `gpu verrano textura` IGUAL a `tests/textura.rs`, y
//!        HelloTexture por PROTON-X con la 3060
//! ```
//!
//! # El TIC de una textura PITCH (lo que hace nouveau, `gm107_create_texture_view`)
//!
//! ```text
//!    0   formato: medidas A8B8G8R8 (0x08), los cuatro UNORM (2), de donde
//!        sale cada canal (X Y Z W); con BGRA en memoria, X y Z se cruzan
//!    1   la direccion, bits 31..0
//!    2   la direccion, bits 47..32 | HEADER_VERSION PITCH (2 en 23:21)
//!    3   el paso de fila >> 5 (filas alineadas a 32 B)
//!    4   ancho - 1 | TEXTURE_TYPE TWO_D_NO_MIPMAP (7 en 26:23)
//!        | BORDER_SIZE SAMPLER_COLOR (7 en 31:29): el borde lo da el TSC
//!    5   alto - 1 | NORMALIZED_COORDS (bit 31): D3D muestrea en 0..1
//!    6   0
//!    7   0: el nivel 0 y nada mas (sin mipmaps, como `textura`)
//! ```
//!
//! # El TSC de un muestreador
//!
//! ```text
//!    0   ADDRESS_U (2:0), ADDRESS_V (5:3), ADDRESS_P (8:6)
//!    1   MAG_FILTER (1:0), MIN_FILTER (5:4), MIP_FILTER NONE (1 en 7:6)
//!    2   MIN_LOD 0 y MAX_LOD 0 (4.8 en 11:0 y 23:12): el nivel 0
//!    3   0
//!    4..7  el color del borde, R G B A en float
//! ```

use crate::cubo::Ordenes;
use crate::mmu::{indices, pde_vram, pte_sistema, pte_vram};
use crate::vram::{a_cero, escribir64, leer64};
use crate::Registros;

/// Bytes de un descriptor (TIC o TSC): ocho palabras.
pub const BYTES: usize = 32;

// == El TIC ==================================================================

/// `GM107_TIC2_0_COMPONENTS_SIZES_A8B8G8R8`.
pub const A8B8G8R8: u32 = 0x08;
/// `..._DATA_TYPE_NUM_UNORM`.
pub const UNORM: u32 = 2;
/// De donde sale un canal (`..._X_SOURCE_IN_*`).
pub const DE_R: u32 = 2;
pub const DE_G: u32 = 3;
pub const DE_B: u32 = 4;
pub const DE_A: u32 = 5;
/// `GM107_TIC2_2_HEADER_VERSION_PITCH` (en 23:21).
pub const VERSION_PITCH: u32 = 2;
/// `GM107_TIC2_4_TEXTURE_TYPE_TWO_D_NO_MIPMAP` (en 26:23).
pub const DOS_D_SIN_MIPMAP: u32 = 7;
/// `GM107_TIC2_4_BORDER_SIZE_SAMPLER_COLOR` (7 en 31:29). Con 0
/// (`BORDER_SIZE_ONE`) la 3060 cree que la imagen trae un texel de borde
/// guardado en memoria y la lee corrida uno en x y en y: el barrido del
/// metal del 28-09 dio `11 11 11 22 22 22 33 33 33 -- -- --` donde la casa
/// espera `00 00 00 11 11 11 22 22 22 33 33 33`, las 96 muestras corridas
/// igual (nearest y linear). Nouveau pone esto mismo.
pub const BORDE_DEL_MUESTREADOR: u32 = 7 << 29;
/// `GM107_TIC2_5_NORMALIZED_COORDS`.
pub const NORMALIZADAS: u32 = 1 << 31;

/// **El formato de una textura de la 3060** (13-10: tambien los BC, por la
/// propia 3060). Los numeros, los de `clb097tex.h` (COMPONENTS: A8B8G8R8
/// 0x08, DXT1 0x24, DXT23 0x25, DXT45 0x26, DXN1 0x27, DXN2 0x28, BC7U 0x17)
/// y la tabla de NIL (`nil_formats.csv`, Mesa: los tipos y de donde sale cada
/// canal): BC4 es (R, 0, 0, 1) y BC5 (R, G, 0, 1), como D3D.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Formato {
    Rgba8,
    Bgra8,
    Bc1,
    Bc2,
    Bc3,
    /// BC4 y BC5 SIN signo (los con signo la casa los guarda corridos a 0..1:
    /// la 3060 daria -1..1, el D3D de verdad; se quedan en la CPU).
    Bc4,
    Bc5,
    Bc7,
    /// ** F1 (14-10): las texturas de FLOAT como las guarda la casa: cuatro
    /// floats por texel (`Como::Flotantes4`: RGBA16F, R11G11B10F, RGBA32F...
    /// ya cuantizados) y uno (`Como::Flotante`: R32, una profundidad, que se
    /// lee (r, 0, 0, 1)). `clb097tex.h`: COMPONENTS R32_G32_B32_A32 0x01 y
    /// R32 0x0f, los canales FLOAT (7); NIL: R32_FLOAT es R001.
    Rgba32f,
    R32f,
}

/// `..._DATA_TYPE_NUM_FLOAT`.
pub const FLOAT: u32 = 7;

/// `..._SOURCE_IN_ZERO` y `..._IN_ONE_FLOAT`.
pub const DE_CERO: u32 = 0;
pub const DE_UNO: u32 = 7;

impl Formato {
    pub const TODOS: [Formato; 10] = [Formato::Rgba8, Formato::Bgra8, Formato::Bc1, Formato::Bc2, Formato::Bc3, Formato::Bc4, Formato::Bc5, Formato::Bc7, Formato::Rgba32f, Formato::R32f];

    /// Como viaja en la receta (4 bits).
    pub const fn numero(self) -> u32 {
        match self {
            Formato::Rgba8 => 0,
            Formato::Bgra8 => 1,
            Formato::Bc1 => 2,
            Formato::Bc2 => 3,
            Formato::Bc3 => 4,
            Formato::Bc4 => 5,
            Formato::Bc5 => 6,
            Formato::Bc7 => 7,
            Formato::Rgba32f => 8,
            Formato::R32f => 9,
        }
    }

    pub const fn de_numero(n: u32) -> Option<Formato> {
        if n < 10 {
            Some(Formato::TODOS[n as usize])
        } else {
            None
        }
    }

    /// Texeles por lado de su bloque (1, o 4 en un BC).
    pub const fn bloque(self) -> u32 {
        match self {
            Formato::Rgba8 | Formato::Bgra8 | Formato::Rgba32f | Formato::R32f => 1,
            _ => 4,
        }
    }

    /// F1: de float (el sombreador filtra: `proton-x-sm86/src/flotantes.rs`).
    pub const fn flotante(self) -> bool {
        matches!(self, Formato::Rgba32f | Formato::R32f)
    }

    /// Bytes de un bloque (de un texel en 8 bits por canal).
    pub const fn bytes(self) -> u32 {
        match self {
            Formato::Rgba8 | Formato::Bgra8 | Formato::R32f => 4,
            Formato::Bc1 | Formato::Bc4 => 8,
            _ => 16,
        }
    }

    /// La palabra 0 del TIC: COMPONENTS, los cuatro UNORM y de donde sale
    /// cada canal (X Y Z W).
    pub const fn palabra0(self) -> u32 {
        let (c, x, y, z, w) = match self {
            Formato::Rgba8 => (A8B8G8R8, DE_R, DE_G, DE_B, DE_A),
            Formato::Bgra8 => (A8B8G8R8, DE_B, DE_G, DE_R, DE_A),
            Formato::Bc1 => (0x24, DE_R, DE_G, DE_B, DE_A),
            Formato::Bc2 => (0x25, DE_R, DE_G, DE_B, DE_A),
            Formato::Bc3 => (0x26, DE_R, DE_G, DE_B, DE_A),
            Formato::Bc4 => (0x27, DE_R, DE_CERO, DE_CERO, DE_UNO),
            Formato::Bc5 => (0x28, DE_R, DE_G, DE_CERO, DE_UNO),
            Formato::Bc7 => (0x17, DE_R, DE_G, DE_B, DE_A),
            Formato::Rgba32f => (0x01, DE_R, DE_G, DE_B, DE_A),
            Formato::R32f => (0x0f, DE_R, DE_CERO, DE_CERO, DE_UNO),
        };
        let t = if self.flotante() { FLOAT } else { UNORM };
        c | t << 7 | t << 10 | t << 13 | t << 16 | x << 19 | y << 22 | z << 25 | w << 28
    }
}

/// `TEXHEAD_PITCH_S_R_G_B_CONVERSION` (el 150: el 22 de la palabra 4).
pub const SRGB: u32 = 1 << 22;

/// **Una textura para la 3060**: 2D, PITCH, en `va`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Imagen {
    /// Donde la ve la 3060.
    pub va: u64,
    pub ancho: u32,
    pub alto: u32,
    /// Bytes por fila (de texeles, o de bloques de 4x4 en un BC).
    pub fila: u32,
    pub formato: Formato,
    /// Los canales de color en sRGB: la 3060 los pasa a lineal al leer.
    pub srgb: bool,
}

impl Imagen {
    /// Lo que el TIC puede decir: medidas de 1..=16384 (16 bits menos uno),
    /// filas de al menos su ancho (en bloques) alineadas a 32 B, la
    /// direccion en 48 bits y alineada a 32 B (el paso y la direccion van sin
    /// sus 5 bits bajos).
    pub const fn valida(&self) -> bool {
        self.ancho >= 1
            && self.alto >= 1
            && self.ancho <= 16384
            && self.alto <= 16384
            && self.fila as u64 >= self.ancho.div_ceil(self.formato.bloque()) as u64 * self.formato.bytes() as u64
            && self.fila % 32 == 0
            && self.va % 32 == 0
            && self.va < 1 << 48
    }
}

/// **El TIC de `i`**, o `None` si no se puede decir (`Imagen::valida`).
pub const fn tic(i: &Imagen) -> Option<[u32; 8]> {
    if !i.valida() {
        return None;
    }
    Some([
        i.formato.palabra0(),
        i.va as u32,
        (i.va >> 32) as u32 & 0xFFFF | VERSION_PITCH << 21,
        i.fila >> 5,
        (i.ancho - 1) | DOS_D_SIN_MIPMAP << 23 | BORDE_DEL_MUESTREADOR | if i.srgb { SRGB } else { 0 },
        (i.alto - 1) | NORMALIZADAS,
        0,
        0,
    ])
}

// == El TSC ==================================================================

/// `G80_TSC_WRAP_*`: lo que hace el muestreador fuera de 0..1.
pub const REPETIR: u32 = 0;
pub const ESPEJO: u32 = 1;
pub const SUJETAR: u32 = 2;
pub const BORDE: u32 = 3;
pub const ESPEJO_UNA_VEZ: u32 = 5;
/// `G80_TSC_1_MAG_FILTER_*` / `MIN_FILTER_*`.
pub const PUNTO: u32 = 1;
pub const LINEAL: u32 = 2;
/// `G80_TSC_1_MIP_FILTER_NONE`.
pub const SIN_MIP: u32 = 1;

/// **Un muestreador para la 3060**, con los numeros de D3D12.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Muestreo {
    /// `D3D12_FILTER` reducido: lineal o punto (sin mipmaps).
    pub lineal: bool,
    /// `D3D12_TEXTURE_ADDRESS_MODE` de U y V: 1 WRAP, 2 MIRROR, 3 CLAMP, 4
    /// BORDER, 5 MIRROR_ONCE.
    pub u: u32,
    pub v: u32,
    /// El color del borde, R G B A.
    pub borde: [f32; 4],
}

/// El modo de D3D12 (1..=5) en el de la 3060.
pub const fn direccion(d3d: u32) -> Option<u32> {
    match d3d {
        1 => Some(REPETIR),
        2 => Some(ESPEJO),
        3 => Some(SUJETAR),
        4 => Some(BORDE),
        5 => Some(ESPEJO_UNA_VEZ),
        _ => None,
    }
}

/// **El TSC de `m`**, o `None` si un modo no es de D3D12.
pub fn tsc(m: &Muestreo) -> Option<[u32; 8]> {
    let (u, v) = (direccion(m.u)?, direccion(m.v)?);
    let filtro = if m.lineal { LINEAL } else { PUNTO };
    // P (la tercera coordenada) no existe en 2D: como U, que es lo que hace
    // nouveau con lo que no se usa.
    Some([u | v << 3 | u << 6, filtro | filtro << 4 | SIN_MIP << 6, 0, 0, m.borde[0].to_bits(), m.borde[1].to_bits(), m.borde[2].to_bits(), m.borde[3].to_bits()])
}

// == T1: el TEX ==============================================================
//
// De donde sale (29-09): NO de memoria. `ptxas -arch=sm_86` (CUDA 12.9, de
// PyPI) compilo un `tex.level.2d` con el asa en un REGISTRO (cargada con un
// LDG: el asa "bindless") y `nvdisasm` 13.4 dijo:
//
//    TEX.SCR.B.LZ R6, R4, R4, R0, 2D ;   0x3800000004047361 / 0x004f4400009e0f06
//
// Y cada campo se movio a mano y se volvio a desensamblar:
//
//    0..11    0x361: TEX con el asa en un registro (.B)
//    12..15   el predicado (7 = PT)
//    16..23   Rd: el primer par del resultado (R y G)
//    24..31   Ra: las coordenadas (u en Ra, v en Ra+1)
//    32..39   Rb: el ASA (tic | tsc << 20)
//    59..60   .SCR (los dos a 1, como ptxas)
//    61..63   la dimension: 1 = 2D
//    64..71   Rd2: el segundo par (B y A)
//    72..75   la mascara de canales (0xF = los cuatro)
//    81..83   el predicado de salida (7 = ninguno)
//    84..86   la cache (1 = la de siempre)
//    87..89   el nivel: 1 = .LZ (el 0, sin derivadas: sin mipmaps no hacen
//             falta, y el programa de pixel no las pide)
//    105..    el control, como todas (`con_control`)
//
// Con Rd2 = Rd + 2 los cuatro canales caen SEGUIDOS en Rd..Rd+3: es la unica
// forma que el juez acepta (`sass::juez`).

/// **`TEX.SCR.B.LZ Rd, Ra, Rb, 2D`**: los cuatro canales en `rd..rd+3`, las
/// coordenadas en `ra, ra+1`, el asa en `rb`.
pub const fn tex(rd: u64, ra: u64, rb: u64, control: u64) -> (u64, u64) {
    let lo = 0x361 | 7 << 12 | rd << 16 | ra << 24 | rb << 32 | 0x38 << 56;
    let hi = (rd + 2) | 0xF << 8 | 7 << 17 | 1 << 20 | 1 << 23;
    (lo, crate::cubo::con_control(hi, control))
}

/// **El asa de una textura**: su TIC y su TSC en las piscinas (20 bits el
/// TIC, 12 el TSC, como el descriptor de NVK: `image_index | sampler_index
/// << 20`). [!] Por comprobar en el metal (T3).
pub const fn asa(tic: u32, tsc: u32) -> u32 {
    (tic & 0xF_FFFF) | tsc << 20
}

// == T2: donde viven (el kernel) ==============================================
//
// ```text
//    TEXELES  la RAM de la APP (sus texturas, como las guarda la casa: pitch,
//             filas de 4 x ancho), prestada a la 3060 SOLO LECTURA y solo
//             mientras dibuja, como el destino: IOVA 0x5C00_0000, una RANURA
//             de 32 MiB por textura (hasta [`MAX_TEXTURAS`]; 1 MiB hasta el
//             12-10, 16 hasta el 14-10: la HDR de float de Cyberpunk)
//    VA       0xA_0000_0000 (la PD1 del tramo; tras la sombra, 0x9): las
//             ranuras con PTE de SISTEMA hacia su IOVA, y detras la pagina
//             de las PISCINAS
//    PISCINAS una pagina de VRAM (0x04A0_4000): los TIC en +0, los TSC en
//             +0x800; la textura k usa los TIC 16 k .. 16 k + 15 (G5, 13-10:
//             uno por mip, `mipmap`) y el TSC k (su asa, `asa(16 k, k)`)
//    TABLAS   la PD0 y las 65 PT en VRAM 0x04A0_0000 (tras las de la sombra)
// ```

/// Donde ve la 3060 los texeles y las piscinas.
pub const VA: u64 = 0xA_0000_0000;
/// Donde los ve por la IOMMU.
/// 12-10 (LI2d): tras las CUATRO ranuras de los destinos (era 0x5900_0000).
pub const IOVA: u64 = 0x5C00_0000;
/// La PD0 y las PT, en VRAM.
pub const TABLAS: u64 = 0x04A0_0000;
/// Las texturas de un dibujo, y lo que mide la ranura de cada una.
pub const MAX_TEXTURAS: usize = 4;
/// 14-10: 32 MiB (era 16 desde el 12-10, y 1 antes). El metal del 14-10
/// 07:14 paro la textura HDR de Cyberpunk: 1280x720 RGBA32F con 5 mips son
/// 14,1 MiB la mip 0 y ~18,8 MiB la cadena; con 32 cabe hasta una de float
/// de 2048x1024 con todas sus mips. El dominio de la IOMMU pone una tabla
/// por cada 2 MiB prestados: hasta 64 aqui, dentro de sus 256
/// (`iommu::AREA_PAGINAS`; el metal medio 56 en uso antes de esto).
pub const RANURA: u64 = 32 << 20;
/// Las PT: las de las ranuras (2 MiB cada una) y una para las piscinas.
pub const PTS: usize = (MAX_TEXTURAS as u64 * RANURA / (2 << 20)) as usize + 1;
/// La pagina de las piscinas: en VRAM, y donde la ve la 3060.
pub const PISCINAS: u64 = TABLAS + (1 + PTS as u64) * 0x1000;
pub const PISCINAS_VA: u64 = VA + MAX_TEXTURAS as u64 * RANURA;
/// Los TSC, detras de los TIC.
pub const TSC_DESDE: u64 = 0x800;
const PAGINA: u64 = 0x1000;

/// **Una textura que manda una app** en su receta: donde esta en SU memoria
/// (la mip 0 de su capa, con las demas detras, como las guarda la casa),
/// como es, que mip mira la vista, donde puso la puerta la cola de mips que
/// no caen en 32 B (`mipmap`) y con que se muestrea.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeApp {
    /// Su VA en la app: la de la mip 0.
    pub va: u64,
    /// Las medidas de la mip 0, y su fila en bytes (la casa: `4 x ancho`;
    /// con varias mips, tiene que serlo: las demas van sin relleno).
    pub ancho: u32,
    pub alto: u32,
    pub fila: u32,
    /// ** G5 (13-10): las mips del recurso (1..=`mipmap::MAX_MIPS`) y la que
    /// mira la vista (el hueco 0: lo que lee un `Sample`).
    pub mips: u32,
    pub vista: u32,
    /// ** G5: donde empieza la COLA en los DATOS de la receta (bytes; la
    /// escribe la puerta con filas de 32). Sin cola, 0.
    pub cola_en: u32,
    pub formato: Formato,
    pub srgb: bool,
    pub muestreo: Muestreo,
}

/// Bytes de una textura en la receta.
pub const BYTES_DE_APP: usize = 48;

impl DeApp {
    pub const NINGUNA: DeApp = DeApp { va: 0, ancho: 0, alto: 0, fila: 0, mips: 0, vista: 0, cola_en: 0, formato: Formato::Rgba8, srgb: false, muestreo: Muestreo { lineal: false, u: 1, v: 1, borde: [0.0; 4] } };

    /// La primera mip que va en la cola (`mipmap::cola_desde`): la decide la
    /// direccion, no la app.
    pub const fn cola(&self) -> u32 {
        crate::mipmap::cola_desde(self.va, self.ancho, self.alto, self.fila, self.formato, self.mips)
    }

    /// Lo que se presta de la RAM de la app: las mips de antes de la cola.
    pub const fn bytes(&self) -> u64 {
        crate::mipmap::desde(self.ancho, self.alto, self.fila, self.formato, self.cola())
    }

    /// Lo que mide la cola en los DATOS.
    pub const fn bytes_cola(&self) -> u64 {
        crate::mipmap::bytes_cola(self.ancho, self.alto, self.formato, self.cola(), self.mips)
    }

    /// Cabe y se puede decir: medidas de 1..=16384, de 1 a `MAX_MIPS` mips
    /// (y no mas de las que tiene la cadena), la de la vista entre ellas, lo
    /// prestado en UNA ranura, la cola en 32 B y no mas de `MAX_COLA`, y su
    /// TSC. La cola dentro de los DATOS lo mira `receta::leer`.
    pub fn valida(&self) -> bool {
        use crate::mipmap::{todas, MAX_COLA, MAX_MIPS};
        (1..=16384).contains(&self.ancho)
            && (1..=16384).contains(&self.alto)
            && (1..=MAX_MIPS.min(todas(self.ancho, self.alto))).contains(&self.mips)
            && self.vista < self.mips
            && self.fila >= crate::mipmap::fila_natural(self.ancho, self.formato, 0)
            && (self.mips == 1 || self.fila == crate::mipmap::fila_natural(self.ancho, self.formato, 0))
            && self.va < 1 << 48
            && (self.bytes() == 0 || self.va != 0)
            && self.va % PAGINA + self.bytes() <= RANURA
            && self.cola_en % 32 == 0
            && self.bytes_cola() <= MAX_COLA as u64
            && (self.bytes_cola() > 0 || self.cola_en == 0)
            && tsc(&self.muestreo).is_some()
    }

    /// Las paginas que se prestan (desde la de su primer texel; 0 si todo
    /// va en la cola).
    pub const fn paginas(&self) -> u64 {
        if self.bytes() == 0 {
            0
        } else {
            (self.va % PAGINA + self.bytes()).div_ceil(PAGINA)
        }
    }

    /// **Los 16 TIC de la textura `k`** (`mipmap::tics`): sus mips en su
    /// ranura (que guarda el desplazamiento en la pagina) y su cola en los
    /// DATOS.
    pub fn tics(&self, k: usize) -> Option<[[u32; 8]; crate::mipmap::HUECOS as usize]> {
        let en_ranura = VA + k as u64 * RANURA + self.va % PAGINA;
        crate::mipmap::tics(en_ranura, crate::vram::DATOS_VA + self.cola_en as u64, self.ancho, self.alto, self.fila, self.mips, self.vista, self.cola(), self.formato, self.srgb)
    }

    /// Como viaja: la VA, las medidas, la fila, la bandera (bit 1 lineal, U en
    /// 4..7 y V en 8..11, los numeros de D3D12; 13-10: el FORMATO en 12..15 y
    /// sRGB en el 16), el borde, donde va la cola y (G5) las mips (8 bits) con
    /// la de la vista (<< 8).
    pub fn escribir(&self, out: &mut [u8]) {
        let m = &self.muestreo;
        let bandera = (m.lineal as u32) << 1 | (m.u & 0xF) << 4 | (m.v & 0xF) << 8 | self.formato.numero() << 12 | (self.srgb as u32) << 16;
        let w = [self.va as u32, (self.va >> 32) as u32, self.ancho, self.alto, self.fila, bandera, m.borde[0].to_bits(), m.borde[1].to_bits(), m.borde[2].to_bits(), m.borde[3].to_bits(), self.cola_en, self.mips | self.vista << 8];
        for (k, x) in w.iter().enumerate() {
            out[4 * k..4 * k + 4].copy_from_slice(&x.to_le_bytes());
        }
    }

    /// Leida de sus 48 bytes, o `None` si algo no se sostiene.
    pub fn leer(b: &[u8]) -> Option<DeApp> {
        let w = |k: usize| u32::from_le_bytes([b[4 * k], b[4 * k + 1], b[4 * k + 2], b[4 * k + 3]]);
        if b.len() < BYTES_DE_APP || w(5) >> 17 != 0 || w(5) & 1 != 0 || w(11) >> 16 != 0 {
            return None;
        }
        let bandera = w(5);
        let borde = [f32::from_bits(w(6)), f32::from_bits(w(7)), f32::from_bits(w(8)), f32::from_bits(w(9))];
        let t = DeApp { va: w(0) as u64 | (w(1) as u64) << 32, ancho: w(2), alto: w(3), fila: w(4), mips: w(11) & 0xFF, vista: w(11) >> 8, cola_en: w(10), formato: Formato::de_numero(bandera >> 12 & 0xF)?, srgb: bandera >> 16 & 1 != 0, muestreo: Muestreo { lineal: bandera & 2 != 0, u: bandera >> 4 & 0xF, v: bandera >> 8 & 0xF, borde } };
        t.valida().then_some(t)
    }
}

/// La entrada de la PD1 del tramo que cuelga [`VA`].
pub const fn entrada_pd1() -> u64 {
    crate::vram::TABLAS[1] + 8 * indices(VA)[2] as u64
}

/// **Mapear** las ranuras (PTE de SISTEMA hacia [`IOVA`]: a que RAM va cada
/// una lo pone la IOMMU en cada dibujo) y la pagina de las piscinas (VRAM),
/// una vez por arranque, de la hoja a la raiz y todo RELEIDO; como
/// `destino::mapear`. `None` si la entrada de la PD1 ya es de otro.
pub fn mapear<R: Registros>(r: &mut R) -> Option<(u32, u32)> {
    let pd1 = leer64(r, entrada_pd1());
    if pd1 != 0 && pd1 != pde_vram(TABLAS) {
        return None;
    }
    let pt = |k: usize| TABLAS + PAGINA * (1 + k as u64);
    for k in 0..=PTS {
        a_cero(r, TABLAS + PAGINA * k as u64);
    }
    a_cero(r, PISCINAS);
    let (mut n, mut bien) = (0u32, 0u32);
    let mut poner = |r: &mut R, dir: u64, v: u64| {
        escribir64(r, dir, v);
        n += 1;
        bien += (leer64(r, dir) == v) as u32;
    };
    for q in 0..MAX_TEXTURAS as u64 * RANURA / PAGINA {
        poner(r, pt((q / 512) as usize) + 8 * (q % 512), pte_sistema(IOVA + q * PAGINA));
    }
    poner(r, pt(PTS - 1), pte_vram(PISCINAS));
    let i0 = indices(VA)[3] as u64;
    for k in 0..PTS {
        poner(r, TABLAS + 16 * (i0 + k as u64) + 8, pde_vram(pt(k)));
    }
    poner(r, entrada_pd1(), pde_vram(TABLAS));
    Some((n, bien))
}

/// **Escribir las piscinas** de un dibujo: los 16 TIC de cada textura
/// (G5: `mipmap`, la textura k en `16 k ..`) y su TSC k, en su sitio,
/// RELEIDOS. `false` si una no se puede decir o no quedo.
pub fn escribir_piscinas<R: Registros>(r: &mut R, texturas: &[DeApp]) -> bool {
    if texturas.len() > MAX_TEXTURAS {
        return false;
    }
    let huecos = crate::mipmap::HUECOS as usize;
    texturas.iter().enumerate().all(|(k, t)| match (t.tics(k), tsc(&t.muestreo)) {
        (Some(ti), Some(ts)) => {
            ti.iter().enumerate().all(|(h, x)| crate::copia::escribir(r, PISCINAS + (BYTES * (huecos * k + h)) as u64, x) == 8) && crate::copia::escribir(r, PISCINAS + TSC_DESDE + (BYTES * k) as u64, &ts) == 8
        }
        _ => false,
    })
}

// == Las piscinas ============================================================

/// `SET_TEX_SAMPLER_POOL_A/B/C` y `SET_TEX_HEADER_POOL_A/B/C` (`clc797.h`;
/// en nouveau, `NVC0_3D_TSC_ADDRESS_HIGH` y `TIC_ADDRESS_HIGH`): la
/// direccion (alta, baja) y el indice MAS ALTO.
pub const SET_TEX_SAMPLER_POOL_A: u32 = 0x155c;
pub const SET_TEX_HEADER_POOL_A: u32 = 0x1574;
/// Las dos invalidaciones de las caches de descriptores (`0x1330` y
/// `0x1334`: nouveau las llama TIC_FLUSH y TSC_FLUSH, `clc797.h`
/// INVALIDATE_SAMPLER_CACHE y INVALIDATE_TEXTURE_HEADER_CACHE; se mandan LAS
/// DOS, con 0 = todo, y el orden de los nombres deja de importar).
pub const INVALIDAR_A: u32 = 0x1330;
pub const INVALIDAR_B: u32 = 0x1334;
/// `SET_SAMPLER_BINDING` (`clc797.h`; en nouveau `LINKED_TSC`, que lo
/// escribe a mano al iniciar en vez de fiarse del valor de arranque): de
/// donde saca la 3060 el TSC de un asa. [`INDEPENDIENTES`] = de sus bits
/// 20..31 (el de D3D12: imagenes y muestreadores en heaps SEPARADOS, que un
/// sombreador combina como quiere); el otro modo (1, VIA_HEADER_BINDING) usa
/// el MISMO indice que el TIC -- el de CUDA, MEDIDO en
/// `COMO_LE_HABLA_NVIDIA.md` 3e: sus asas son 1, 2, 3... con los bits 20..39
/// a cero. Con `asa(k, k)` los dos leen lo mismo; se escribe igual, para que
/// el dia que PROTON-X combine la imagen `i` con el muestreador `j` no mande
/// el valor de fabrica.
pub const SET_SAMPLER_BINDING: u32 = 0x1234;
pub const INDEPENDIENTES: u32 = 0;
/// Lo mas que lleva cada piscina (una pagina de 4 KiB: 128 descriptores).
pub const MAX: u32 = 4096 / BYTES as u32;

/// **Las ordenes de las piscinas**: donde estan los TIC y los TSC, cuantos,
/// y las caches de descriptores invalidadas (se acaban de escribir).
pub fn ordenes(e: &mut Ordenes, tics: u64, n_tic: u32, tscs: u64, n_tsc: u32) -> bool {
    if n_tic == 0 || n_tsc == 0 || n_tic > MAX || n_tsc > MAX || tics % BYTES as u64 != 0 || tscs % BYTES as u64 != 0 {
        return false;
    }
    e.m(SET_SAMPLER_BINDING, &[INDEPENDIENTES]);
    e.m(SET_TEX_HEADER_POOL_A, &[(tics >> 32) as u32, tics as u32, n_tic - 1]);
    e.m(SET_TEX_SAMPLER_POOL_A, &[(tscs >> 32) as u32, tscs as u32, n_tsc - 1]);
    e.m(INVALIDAR_A, &[0]);
    e.m(INVALIDAR_B, &[0]);
    true
}

// Nada se pisa: su entrada de la PD1, su IOVA, sus tablas y su pagina.
const _: () = assert!(VA % (2 << 20) == 0 && indices(VA)[2] != indices(crate::sombra::VA)[2] && indices(VA)[2] != indices(crate::destino::VA)[2]);
const _: () = assert!(IOVA >= crate::destino::iova_de(crate::destino::OBJETIVOS) && MAX_TEXTURAS as u64 * RANURA == (PTS as u64 - 1) * (2 << 20));
const _: () = assert!(TABLAS >= crate::sombra::TABLAS + (1 + crate::sombra::PTS as u64) * PAGINA && PISCINAS + PAGINA <= 0x0800_0000);
const _: () = assert!(TSC_DESDE >= (MAX_TEXTURAS * BYTES) as u64 && TSC_DESDE + (MAX_TEXTURAS * BYTES) as u64 <= PAGINA);
const _: () = assert!(indices(PISCINAS_VA)[3] == indices(VA)[3] + PTS - 1 && indices(PISCINAS_VA)[4] == 0);

#[cfg(test)]
mod pruebas {
    use super::*;

    const IMG: Imagen = Imagen { va: 0xA_0000_1000, ancho: 256, alto: 256, fila: 1024, formato: Formato::Rgba8, srgb: false };

    #[test]
    fn el_tic_de_una_textura_pitch() {
        let t = tic(&IMG).unwrap();
        assert_eq!(t[0] & 0x7F, A8B8G8R8);
        for k in 0..4 {
            assert_eq!(t[0] >> (7 + 3 * k) & 7, UNORM, "canal {k} UNORM");
        }
        assert_eq!((t[0] >> 19 & 7, t[0] >> 22 & 7, t[0] >> 25 & 7, t[0] >> 28 & 7), (DE_R, DE_G, DE_B, DE_A));
        assert_eq!(t[1], 0x0000_1000);
        assert_eq!(t[2] & 0xFFFF, 0xA);
        assert_eq!(t[2] >> 21 & 7, VERSION_PITCH);
        assert_eq!(t[3], 1024 >> 5);
        assert_eq!((t[4] & 0xFFFF, t[4] >> 23 & 0xF), (255, DOS_D_SIN_MIPMAP));
        assert_eq!(t[4] >> 29, 7, "el borde lo da el TSC, no la memoria");
        assert_eq!((t[5] & 0xFFFF, t[5] & NORMALIZADAS), (255, NORMALIZADAS));
        assert_eq!((t[6], t[7]), (0, 0), "el nivel 0, nada mas");
    }

    #[test]
    fn bgra_cruza_rojo_y_azul() {
        let t = tic(&Imagen { formato: Formato::Bgra8, ..IMG }).unwrap();
        assert_eq!((t[0] >> 19 & 7, t[0] >> 22 & 7, t[0] >> 25 & 7, t[0] >> 28 & 7), (DE_B, DE_G, DE_R, DE_A));
    }

    /// 13-10: los BC y sRGB, como NIL (`nil_formats.csv`: COMPONENTS, los
    /// UNORM y el swizzle; BC4 (R,0,0,1), BC5 (R,G,0,1)) y `clb097tex.h`.
    #[test]
    fn los_bc_como_nil() {
        let comp = |f: Formato| f.palabra0() & 0x7F;
        let fuentes = |f: Formato| { let w = f.palabra0(); (w >> 19 & 7, w >> 22 & 7, w >> 25 & 7, w >> 28 & 7) };
        assert_eq!([Formato::Bc1, Formato::Bc2, Formato::Bc3, Formato::Bc4, Formato::Bc5, Formato::Bc7].map(comp), [0x24, 0x25, 0x26, 0x27, 0x28, 0x17]);
        assert_eq!(fuentes(Formato::Bc4), (DE_R, 0, 0, 7));
        assert_eq!(fuentes(Formato::Bc5), (DE_R, DE_G, 0, 7));
        assert_eq!(fuentes(Formato::Bc7), (DE_R, DE_G, DE_B, DE_A));
        // F1: los de float, de `clb097tex.h` y NIL (R32_FLOAT es R001).
        assert_eq!([Formato::Rgba32f, Formato::R32f].map(comp), [0x01, 0x0f]);
        assert_eq!(fuentes(Formato::R32f), (DE_R, 0, 0, 7));
        assert_eq!([Formato::Rgba32f, Formato::R32f].map(|f| (f.bytes(), f.bloque())), [(16, 1), (4, 1)]);
        for f in Formato::TODOS {
            assert_eq!(Formato::de_numero(f.numero()), Some(f));
            for k in 0..4 {
                assert_eq!(f.palabra0() >> (7 + 3 * k) & 7, if f.flotante() { FLOAT } else { UNORM });
            }
        }
        // Un BC1 de 64x64: filas de 16 bloques de 8 bytes (128 B), y sRGB.
        let bc = Imagen { formato: Formato::Bc1, fila: 128, ancho: 64, alto: 64, srgb: true, ..IMG };
        let t = tic(&bc).unwrap();
        assert_eq!((t[3], t[4] & SRGB, t[4] & 0xFFFF), (128 >> 5, SRGB, 63));
        assert!(tic(&Imagen { fila: 96, ..bc }).is_none(), "una fila mas corta que sus bloques");
    }

    #[test]
    fn lo_que_no_se_puede_decir() {
        assert!(tic(&Imagen { fila: 1000, ..IMG }).is_none(), "fila sin alinear a 32");
        assert!(tic(&Imagen { fila: 512, ..IMG }).is_none(), "fila mas corta que la imagen");
        assert!(tic(&Imagen { ancho: 0, ..IMG }).is_none());
        assert!(tic(&Imagen { alto: 16385, ..IMG }).is_none());
        assert!(tic(&Imagen { va: IMG.va + 4, ..IMG }).is_none());
        assert!(tic(&Imagen { va: 1 << 48, ..IMG }).is_none());
        assert!(tic(&Imagen { ancho: 16384, alto: 16384, fila: 65536, ..IMG }).is_some());
    }

    #[test]
    fn el_tsc_de_los_muestreadores_de_d3d12() {
        let m = Muestreo { lineal: true, u: 1, v: 4, borde: [0.0, 0.5, 1.0, 1.0] };
        let t = tsc(&m).unwrap();
        assert_eq!((t[0] & 7, t[0] >> 3 & 7, t[0] >> 6 & 7), (REPETIR, BORDE, REPETIR));
        assert_eq!((t[1] & 3, t[1] >> 4 & 3, t[1] >> 6 & 3), (LINEAL, LINEAL, SIN_MIP));
        assert_eq!(t[2], 0, "MIN_LOD = MAX_LOD = 0: el nivel 0");
        assert_eq!(&t[4..], &[0, 0.5f32.to_bits(), 1.0f32.to_bits(), 1.0f32.to_bits()]);
        let p = tsc(&Muestreo { lineal: false, ..m }).unwrap();
        assert_eq!((p[1] & 3, p[1] >> 4 & 3), (PUNTO, PUNTO));
        assert_eq!([1, 2, 3, 4, 5].map(|d| direccion(d).unwrap()), [REPETIR, ESPEJO, SUJETAR, BORDE, ESPEJO_UNA_VEZ]);
        assert!(tsc(&Muestreo { u: 0, ..m }).is_none() && tsc(&Muestreo { v: 6, ..m }).is_none());
    }

    /// Lo que dijo `ptxas`, y cinco mas que `nvdisasm` 13.4 leyo como se
    /// pidieron (29-09): `TEX.SCR.B.LZ R(rd+2), Rrd, Rra, Rrb, 2D`.
    #[test]
    fn el_tex_de_ptxas_y_nvdisasm() {
        let control = 0x004f_4400_009e_0f06u64 >> 41;
        assert_eq!(tex(4, 4, 0, control), (0x3800_0000_0404_7361, 0x004f_4400_009e_0f06), "el de ptxas, bit a bit");
        for ((rd, ra, rb), lo) in [((0, 2, 4), 0x3800_0004_0200_7361u64), ((8, 0, 10), 0x3800_000a_0008_7361), ((12, 14, 1), 0x3800_0001_0e0c_7361), ((20, 10, 30), 0x3800_001e_0a14_7361), ((60, 58, 57), 0x3800_0039_3a3c_7361)] {
            let (l, h) = tex(rd, ra, rb, control);
            assert_eq!((l, h & 0xFF), (lo, rd + 2));
            assert!(crate::sass::juez::conoce(l, h), "el juez lo sabe leer");
        }
        assert_eq!(asa(3, 1), 3 | 1 << 20);
        // El del emisor de PROTON-X (`bmo_sm86::codifica::tex`) es el mismo.
        for (rd, ra, rb) in [(4, 4, 0), (0, 2, 4), (60, 58, 57)] {
            assert_eq!(bmo_sm86::codifica::tex(rd as u8, ra as u8, rb as u8, control), tex(rd, ra, rb, control));
        }
    }

    #[test]
    fn las_ordenes_de_las_piscinas() {
        use crate::copia::cabecera_en;
        let mut e = crate::cubo::hasta_el_dibujo_de(&crate::destino::Destino { fila: 5120, ancho: 1280, alto: 720, flotante: crate::destino::Flotante::No, rgb: false }.ventana(), false, false, None, 64, crate::tuberia::PROGRAMAS_VA);
        let antes = e.n;
        assert!(ordenes(&mut e, 0x2_0001_4000, 1, 0x2_0001_5000, 1));
        let o = &e.o[antes..e.n];
        let c = |m, n| cabecera_en(crate::tresde::SUBCANAL, m, n);
        assert_eq!(&o[..2], &[c(SET_SAMPLER_BINDING, 1), INDEPENDIENTES], "el modo del muestreador, escrito");
        assert_eq!(&o[2..6], &[c(SET_TEX_HEADER_POOL_A, 3), 2, 0x0001_4000, 0]);
        assert_eq!(&o[6..10], &[c(SET_TEX_SAMPLER_POOL_A, 3), 2, 0x0001_5000, 0]);
        assert_eq!(&o[10..], &[c(INVALIDAR_A, 1), 0, c(INVALIDAR_B, 1), 0]);
        assert!(!ordenes(&mut e, 0x2_0001_4000, 0, 0x2_0001_5000, 1), "sin descriptores, nada");
        assert!(!ordenes(&mut e, 0x2_0001_4000, MAX + 1, 0x2_0001_5000, 1), "mas de una pagina");
        assert!(!ordenes(&mut e, 0x2_0001_4004, 1, 0x2_0001_5000, 1), "sin alinear a 32");
    }
}

// == T3: la prueba del metal (`gpu verrano textura`) =========================

/// **La prueba de las texturas en el metal** -- la MISMA que el propietario
/// corrio con CUDA en su Windows (`docs/metal/tex_cuda/`): una 4x4 RGBA8 con
/// texeles `(x*60, y*60, (x+y)*20, 255)`, los 12 puntos y los 8 muestreadores
/// (Point/Linear x Wrap/Mirror/Clamp/Border, borde (0, 0.5, 1, 1)). Aqui la
/// dibuja la 3060 por la PUERTA DE LAS APPS (una receta VRN2 con una
/// textura): cada punto es un cuadro de 64x64 pixeles con el MISMO uv en sus
/// seis vertices (el IPA de un valor constante es exacto), y el pixel de su
/// centro tiene que ser lo que dio la 3060 bajo CUDA, en 8 bits.
pub mod prueba {
    use super::{tex, DeApp, Muestreo};
    use crate::destino::Destino;
    use crate::pegamento::{Carga, Elemento};
    use crate::receta::{self, Receta, MAX_CARGAS, MAX_ELEMENTOS, MAX_GENERICOS, NINGUNA, NINGUNO};
    use crate::tuberia::{carga, espera, Dibujo};

    /// Los 12 puntos (u, v) de la prueba de CUDA, en su orden.
    pub const PUNTOS: [(f32, f32); 12] = [(0.1, 0.1), (0.5, 0.5), (0.375, 0.625), (0.9, 0.2), (1.3, 0.4), (-0.2, 0.7), (0.125, 0.125), (0.13, 0.13), (0.2, 0.9), (0.99, 0.99), (1.0, 1.0), (2.6, -1.4)];
    /// Los 8 muestreadores, en el orden de `SALIDA.TXT`: (lineal, modo D3D12).
    pub const MUESTREADORES: [(bool, u32); 8] = [(false, 1), (false, 2), (false, 3), (false, 4), (true, 1), (true, 2), (true, 3), (true, 4)];
    pub const NOMBRES: [&str; 8] = ["Point Wrap", "Point Mirror", "Point Clamp", "Point Border", "Linear Wrap", "Linear Mirror", "Linear Clamp", "Linear Border"];
    /// El borde de la prueba de CUDA.
    pub const BORDE: [f32; 4] = [0.0, 0.5, 1.0, 1.0];

    /// **Lo que dio la 3060 bajo CUDA** (`docs/metal/tex_cuda/SALIDA.TXT`),
    /// en 8 bits (`round(x * 255)`, ninguno cae en un empate): R | G << 8 |
    /// B << 16 | A << 24. El banco de `bmo-proton-x-sm86` comprueba que el
    /// muestreador de la casa da EXACTAMENTE esto.
    pub const ESPERADO: [[u32; 12]; 8] = [
        [0xFF000000, 0xFF507878, 0xFF3C783C, 0xFF3C00B4, 0xFF283C3C, 0xFF6478B4, 0xFF000000, 0xFF000000, 0xFF3CB400, 0xFF78B4B4, 0xFF000000, 0xFF507878],
        [0xFF000000, 0xFF507878, 0xFF3C783C, 0xFF3C00B4, 0xFF3C3C78, 0xFF287800, 0xFF000000, 0xFF000000, 0xFF3CB400, 0xFF78B4B4, 0xFF78B4B4, 0xFF507878],
        [0xFF000000, 0xFF507878, 0xFF3C783C, 0xFF3C00B4, 0xFF503CB4, 0xFF287800, 0xFF000000, 0xFF000000, 0xFF3CB400, 0xFF78B4B4, 0xFF78B4B4, 0xFF3C00B4],
        [0xFF000000, 0xFF507878, 0xFF3C783C, 0xFF3C00B4, 0xFFFF7F00, 0xFFFF7F00, 0xFF000000, 0xFF000000, 0xFF3CB400, 0xFF78B4B4, 0xFFFF7F00, 0xFFFF7F00],
        [0xFF0C1212, 0xFF3C5A5A, 0xFF3C783C, 0xFF3C12A2, 0xFF24422A, 0xFF648AA2, 0xFF000000, 0xFF010101, 0xFF3CA212, 0xFF416161, 0xFF3C5A5A, 0xFF4C7272],
        [0xFF000000, 0xFF3C5A5A, 0xFF3C783C, 0xFF4212B4, 0xFF44428A, 0xFF348A12, 0xFF000000, 0xFF010101, 0xFF42B412, 0xFF78B4B4, 0xFF78B4B4, 0xFF4C7272],
        [0xFF000000, 0xFF3C5A5A, 0xFF3C783C, 0xFF4212B4, 0xFF5242B4, 0xFF2E8A00, 0xFF000000, 0xFF010101, 0xFF42B412, 0xFF78B4B4, 0xFF78B4B4, 0xFF3C00B4],
        [0xFF311800, 0xFF3C5A5A, 0xFF3C783C, 0xFF551DA2, 0xFFFF7F00, 0xFFFF7F00, 0xFF000000, 0xFF010101, 0xFF55AF10, 0xFFD88E34, 0xFFDD8C2D, 0xFFFF7F00],
    ];

    /// La textura: 4x4, filas de 32 B (el TIC pide pasos de 32; se rellena).
    pub const ANCHO: u32 = 4;
    pub const FILA: u32 = 32;
    pub const TEXELES: usize = (FILA / 4 * ANCHO) as usize;

    /// Sus texeles como van en memoria (R en el byte 0), con el relleno a 0.
    pub fn texeles() -> [u32; TEXELES] {
        let mut t = [0u32; TEXELES];
        for y in 0..4u32 {
            for x in 0..4u32 {
                t[(y * FILA / 4 + x) as usize] = x * 60 | (y * 60) << 8 | ((x + y) * 20) << 16 | 255 << 24;
            }
        }
        t
    }

    /// El cuadro del punto `k`: su esquina en pixeles (una rejilla de 4 x 3
    /// cuadros de 64, con 64 de hueco) y su centro.
    pub const LADO: u32 = 64;
    pub const fn esquina(k: usize) -> (u32, u32) {
        (128 + 256 * (k as u32 % 4), 104 + 192 * (k as u32 / 4))
    }
    pub const fn centro(k: usize) -> (u32, u32) {
        let (x, y) = esquina(k);
        (x + LADO / 2, y + LADO / 2)
    }

    /// El fondo con el que se limpia el destino (ningun esperado lo es).
    pub const FONDO: u32 = 0xFF20_1810;

    /// `MOV Rd, Rs` (0x002 forma 1, la de la lista blanca de R7; la mascara
    /// 0xF en 72..76 como `ptxas`).
    const fn mov_r(rd: u64, rs: u64) -> (u64, u64) {
        (0x202 | 7 << 12 | rd << 16 | rs << 32, crate::cubo::con_control(0xF << 8, crate::cubo::ALU))
    }

    /// `EXIT` esperando las barreras de `mascara`.
    const fn exit(mascara: u64) -> (u64, u64) {
        (crate::cubo::EXIT.0, crate::cubo::con_control(crate::cubo::EXIT.1, espera(mascara)))
    }

    fn bytes(w: &[(u64, u64)], out: &mut [u8]) -> usize {
        for (k, &(lo, hi)) in w.iter().enumerate() {
            out[16 * k..16 * k + 8].copy_from_slice(&lo.to_le_bytes());
            out[16 * k + 8..16 * k + 16].copy_from_slice(&hi.to_le_bytes());
        }
        16 * w.len()
    }

    /// Los DATOS: 12 cuadros de 6 vertices; cada vertice, la posicion en
    /// recorte y (u, v, 0, 0).
    pub const VERTICES: usize = 6 * PUNTOS.len();
    pub const BYTES_DATOS: usize = 32 * VERTICES;

    /// **La receta del muestreador `m`** (0..8): la textura en `va_textura`
    /// (un bloque de quien la manda, alineado a 32), el destino de 1280x720
    /// RGBA8 en `va_destino`. En `caja`; devuelve cuanto mide.
    pub fn receta(m: usize, va_textura: u64, va_destino: u64, datos: &mut [u8; BYTES_DATOS], caja: &mut [u8]) -> Option<usize> {
        let (lineal, modo) = *MUESTREADORES.get(m)?;
        receta_con(lineal, modo, &PUNTOS, va_textura, va_destino, datos, caja)
    }

    /// **EL BARRIDO** (metal 28-09 23:16: los 96 salieron DISTINTOS, y el
    /// primero, `Point Wrap (0.1, 0.1)`, dio el texel (1, 1) en vez del
    /// (0, 0)): Point Clamp en la diagonal, en el CENTRO de 12 franjas
    /// iguales de 0..1 (`(k + 0.5) / 12`). Con la textura de 4 como se cree,
    /// los texeles son 0 0 0 1 1 1 2 2 2 3 3 3; cualquier otra escala (o un
    /// desplazamiento) sale en ese patron y se lee directo.
    pub const BARRIDO: [(f32, f32); 12] = {
        let mut p = [(0.0f32, 0.0f32); 12];
        let mut k = 0;
        while k < 12 {
            let x = (2 * k + 1) as f32 / 24.0;
            p[k] = (x, x);
            k += 1;
        }
        p
    };
    /// Lo que sale del barrido con la textura de 4 (el texel de la diagonal).
    pub const BARRIDO_BIEN: [u32; 12] = [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];

    /// **Que texel es** un pixel leido: `Some(x + 4 y)` si es uno de los 16
    /// (todos distintos), `Some(16)` si es el borde, `Some(17)` si es 0 (el
    /// relleno o memoria vacia); `None` si no es ninguno (mezcla o basura).
    pub fn que_texel(pixel: u32) -> Option<u32> {
        let t = texeles();
        for y in 0..4u32 {
            for x in 0..4u32 {
                if t[(y * FILA / 4 + x) as usize] == pixel {
                    return Some(x + 4 * y);
                }
            }
        }
        match pixel {
            0xFFFF_7F00 => Some(16),
            // El relleno o memoria vacia: TODO a cero, alfa incluido (el texel
            // (0, 0) es negro OPACO, 0xFF000000, y ya salio arriba).
            0 => Some(17),
            _ => None,
        }
    }

    /// La receta con `lineal`, el modo `modo` (D3D12) y los 12 puntos que se
    /// pidan.
    pub fn receta_con(lineal: bool, modo: u32, puntos: &[(f32, f32); 12], va_textura: u64, va_destino: u64, datos: &mut [u8; BYTES_DATOS], caja: &mut [u8]) -> Option<usize> {
        for (k, &(u, v)) in puntos.iter().enumerate() {
            let (x0, y0) = esquina(k);
            let (x1, y1) = (x0 + LADO, y0 + LADO);
            let cx = |x: u32| x as f32 / 640.0 - 1.0;
            let cy = |y: u32| 1.0 - y as f32 / 360.0;
            let esquinas = [(x0, y0), (x1, y0), (x0, y1), (x1, y0), (x1, y1), (x0, y1)];
            for (j, &(x, y)) in esquinas.iter().enumerate() {
                let w = [cx(x), cy(y), 0.5, 1.0, u, v, 0.0, 0.0];
                for (i, f) in w.iter().enumerate() {
                    let o = 32 * (6 * k + j) + 4 * i;
                    datos[o..o + 4].copy_from_slice(&f.to_le_bytes());
                }
            }
        }
        let mut vs = [0u8; 32];
        let mut ps = [0u8; 32];
        // El de vertice deja todo como llega: la posicion en R0..R3 y el uv
        // en R4..R7 (el generico 0). El de pixel: TEX R0..R3 <- (R4, R5) con
        // el asa en R6 (la barrera 1), y el EXIT la espera.
        let nv = bytes(&[mov_r(0, 0), exit(0)], &mut vs);
        let np = bytes(&[tex(0, 4, 6, carga(1)), exit(1 << 1)], &mut ps);
        let dst = Destino { fila: 4 * 1280, ancho: 1280, alto: 720, flotante: crate::destino::Flotante::No, rgb: true };
        let t = DeApp { va: va_textura, ancho: ANCHO, alto: 4, fila: FILA, mips: 1, vista: 0, cola_en: 0, formato: super::Formato::Rgba8, srgb: false, muestreo: Muestreo { lineal, u: modo, v: modo, borde: BORDE } };
        let mut r = Receta {
            n: VERTICES,
            vs: &vs[..nv],
            ps: &ps[..np],
            registros_vs: 8,
            registros_ps: 8,
            salidas: 2,
            posicion: 0,
            filas: 0,
            paso: 32,
            elementos: [NINGUNO; MAX_ELEMENTOS],
            n_elementos: 2,
            cargas_vs: [NINGUNA; MAX_CARGAS],
            n_cargas_vs: 8,
            cargas_ps: [NINGUNA; MAX_CARGAS],
            n_cargas_ps: 3,
            genericos: [None; MAX_GENERICOS],
            n_genericos: 1,
            datos: &datos[..],
            dibujo: Dibujo { indices: None, vertices: VERTICES as u32, destino: Some((va_destino, dst)), color: Some(FONDO), texturas: 1, ..Dibujo::default() },
            texturas: [DeApp::NINGUNA; super::MAX_TEXTURAS],
            termometro_vs: None,
            termometro_ps: None,
        };
        r.elementos[0] = Elemento::memoria(0, 4);
        r.elementos[1] = Elemento::memoria(16, 4);
        for k in 0..8u8 {
            r.cargas_vs[k as usize] = Carga::Entrada { elemento: k / 4, componente: k % 4, reg: k };
        }
        r.cargas_ps[0] = Carga::Entrada { elemento: 0, componente: 0, reg: 4 };
        r.cargas_ps[1] = Carga::Entrada { elemento: 0, componente: 1, reg: 5 };
        r.cargas_ps[2] = Carga::Asa { textura: 0, reg: 6 };
        r.genericos[0] = Some(0);
        r.texturas[0] = t;
        receta::escribir(caja, &r)
    }

    #[cfg(test)]
    mod pruebas {
        extern crate std;
        use super::*;

        /// Las 8 recetas se escriben, se releen, se PEGAN como en el kernel y
        /// el juez las aprueba; los cuadros caben y no se tocan.
        #[test]
        fn las_ocho_recetas_pasan_el_kernel() {
            let mut datos = [0u8; BYTES_DATOS];
            let mut caja = std::vec![0u8; receta::MAX_RECETA];
            let mut taller = std::boxed::Box::new(receta::Taller::nuevo());
            for m in 0..8 {
                let n = receta(m, 0x4000_0000, 0x1000_0000, &mut datos, &mut caja).expect("se escribe");
                let r = receta::leer(&caja[..n]).expect("se relee");
                assert_eq!((r.texturas()[0].muestreo.lineal, r.texturas()[0].muestreo.u), MUESTREADORES[m]);
                receta::pegar(&r, &mut taller).unwrap_or_else(|e| panic!("{}: {e:?}", NOMBRES[m]));
            }
            for k in 0..12 {
                let (x, y) = esquina(k);
                assert!(x + LADO <= 1280 && y + LADO <= 720);
                for j in 0..k {
                    let (a, b) = esquina(j);
                    assert!(a.abs_diff(x) >= LADO || b.abs_diff(y) >= LADO);
                }
            }
            assert!(ESPERADO.iter().flatten().all(|&p| p != FONDO));
            assert_eq!(texeles()[(FILA / 4 + 1) as usize], 60 | 60 << 8 | 40 << 16 | 255 << 24, "el texel (1, 1)");
            // El barrido: los centros de 12 franjas y lo que dan con 4 texeles.
            for (k, &(u, _)) in BARRIDO.iter().enumerate() {
                assert_eq!((u * 4.0) as u32, BARRIDO_BIEN[k]);
            }
            assert!(receta_con(false, 3, &BARRIDO, 0x4000_0000, 0x1000_0000, &mut datos, &mut caja).is_some());
            // Lo que dio el metal (28-09 23:16) es el texel (1, 1).
            assert_eq!(que_texel(0xFF28_3C3C), Some(1 + 4));
            assert_eq!((que_texel(0xFFFF_7F00), que_texel(0), que_texel(0xFF00_0000), que_texel(0xFF12_3456)), (Some(16), Some(17), Some(0), None));
        }
    }
}
