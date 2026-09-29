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
//!    0   formato: tamanos A8B8G8R8 (0x08), los cuatro UNORM (2), de donde
//!        sale cada canal (X Y Z W); con BGRA en memoria, X y Z se cruzan
//!    1   la direccion, bits 31..0
//!    2   la direccion, bits 47..32 | HEADER_VERSION PITCH (2 en 23:21)
//!    3   el paso de fila >> 5 (filas alineadas a 32 B)
//!    4   ancho - 1 | TEXTURE_TYPE TWO_D_NO_MIPMAP (7 en 26:23)
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
/// `GM107_TIC2_5_NORMALIZED_COORDS`.
pub const NORMALIZADAS: u32 = 1 << 31;

/// **Una textura para la 3060**: 2D, 8 bits por canal, PITCH, en `va`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Imagen {
    /// Donde la ve la 3060.
    pub va: u64,
    pub ancho: u32,
    pub alto: u32,
    /// Bytes por fila.
    pub fila: u32,
    /// `B8G8R8A8` en memoria (si no, `R8G8B8A8`).
    pub bgra: bool,
}

impl Imagen {
    /// Lo que el TIC puede decir: medidas de 1..=16384 (16 bits menos uno),
    /// filas de al menos `4 x ancho` alineadas a 32 B, la direccion en 48
    /// bits y alineada a 32 B (el paso y la direccion van sin sus 5 bits
    /// bajos).
    pub const fn valida(&self) -> bool {
        self.ancho >= 1
            && self.alto >= 1
            && self.ancho <= 16384
            && self.alto <= 16384
            && self.fila as u64 >= 4 * self.ancho as u64
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
    let (x, z) = if i.bgra { (DE_B, DE_R) } else { (DE_R, DE_B) };
    let formato = A8B8G8R8 | UNORM << 7 | UNORM << 10 | UNORM << 13 | UNORM << 16 | x << 19 | DE_G << 22 | z << 25 | DE_A << 28;
    Some([
        formato,
        i.va as u32,
        (i.va >> 32) as u32 & 0xFFFF | VERSION_PITCH << 21,
        i.fila >> 5,
        (i.ancho - 1) | DOS_D_SIN_MIPMAP << 23,
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
/// Lo mas que lleva cada piscina (una pagina de 4 KiB: 128 descriptores).
pub const MAX: u32 = 4096 / BYTES as u32;

/// **Las ordenes de las piscinas**: donde estan los TIC y los TSC, cuantos,
/// y las caches de descriptores invalidadas (se acaban de escribir).
pub fn ordenes(e: &mut Ordenes, tics: u64, n_tic: u32, tscs: u64, n_tsc: u32) -> bool {
    if n_tic == 0 || n_tsc == 0 || n_tic > MAX || n_tsc > MAX || tics % BYTES as u64 != 0 || tscs % BYTES as u64 != 0 {
        return false;
    }
    e.m(SET_TEX_HEADER_POOL_A, &[(tics >> 32) as u32, tics as u32, n_tic - 1]);
    e.m(SET_TEX_SAMPLER_POOL_A, &[(tscs >> 32) as u32, tscs as u32, n_tsc - 1]);
    e.m(INVALIDAR_A, &[0]);
    e.m(INVALIDAR_B, &[0]);
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const IMG: Imagen = Imagen { va: 0xA_0000_1000, ancho: 256, alto: 256, fila: 1024, bgra: false };

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
        assert_eq!((t[5] & 0xFFFF, t[5] & NORMALIZADAS), (255, NORMALIZADAS));
        assert_eq!((t[6], t[7]), (0, 0), "el nivel 0, nada mas");
    }

    #[test]
    fn bgra_cruza_rojo_y_azul() {
        let t = tic(&Imagen { bgra: true, ..IMG }).unwrap();
        assert_eq!((t[0] >> 19 & 7, t[0] >> 22 & 7, t[0] >> 25 & 7, t[0] >> 28 & 7), (DE_B, DE_G, DE_R, DE_A));
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
    }

    #[test]
    fn las_ordenes_de_las_piscinas() {
        use crate::copia::cabecera_en;
        let mut e = crate::cubo::hasta_el_dibujo_de(&crate::destino::Destino { fila: 5120, ancho: 1280, alto: 720, rgb: false }.ventana(), false, false, None, 64);
        let antes = e.n;
        assert!(ordenes(&mut e, 0x2_0001_4000, 1, 0x2_0001_5000, 1));
        let o = &e.o[antes..e.n];
        let c = |m, n| cabecera_en(crate::tresde::SUBCANAL, m, n);
        assert_eq!(&o[..4], &[c(SET_TEX_HEADER_POOL_A, 3), 2, 0x0001_4000, 0]);
        assert_eq!(&o[4..8], &[c(SET_TEX_SAMPLER_POOL_A, 3), 2, 0x0001_5000, 0]);
        assert_eq!(&o[8..], &[c(INVALIDAR_A, 1), 0, c(INVALIDAR_B, 1), 0]);
        assert!(!ordenes(&mut e, 0x2_0001_4000, 0, 0x2_0001_5000, 1), "sin descriptores, nada");
        assert!(!ordenes(&mut e, 0x2_0001_4000, MAX + 1, 0x2_0001_5000, 1), "mas de una pagina");
        assert!(!ordenes(&mut e, 0x2_0001_4004, 1, 0x2_0001_5000, 1), "sin alinear a 32");
    }
}
