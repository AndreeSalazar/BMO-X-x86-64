//! **P3b4b (3): EL DESTINO DE LA APP** -- la 3060 dibuja en la RAM de un
//! proceso (el back buffer de un juego), no en un recuadro fijo de la
//! pantalla. La app presenta como siempre (su ventana, el escritorio):
//! lo unico que cambia es QUIEN pinto los pixeles.
//!
//! capa: puro -- el mapa, las medidas y lo que se valida; la RAM, la IOMMU
//! y los registros los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- el mismo camino que `volcado` (VISTO en el metal:
//!           una VA propia con sus tablas en VRAM, PTE de SISTEMA hacia una
//!           IOVA fija, y la IOMMU dice a que RAM va esa IOVA en cada dibujo)
//!
//! # Donde vive
//!
//! ```text
//!    VA       0x7_0000_0000 (la PD1 del tramo; 0x4 pantalla, 0x5 volcado,
//!             0x6 video): 4 PT de 2 MiB, hasta 1920x1080
//!    IOVA     0x5800_0000 (tras la de video), prestada ESCRIBIBLE solo
//!             mientras se dibuja
//!    TABLAS   la PD0 y las PT en VRAM 0x0470_0000 (tras las de video)
//! ```
//!
//! Hasta el 10-10 la medida era la de VERRANO (1280x720: el viewport y el
//! recorte de sus ordenes). ** Q0a1 de `docs/plan/EL_FOCO.md`: CUALQUIERA
//! que quepa en el mapa (de 1x1 hasta [`MAX_BYTES`], 1920x1080 incluida), y
//! las ordenes sacan de ella el viewport y el recorte (`cubo::Ventana`). Con
//! la Z no: la sombra en bloque y el bufer de Z miden 1280x720 (`Dibujo`,
//! `tuberia::cabe`).

use crate::cubo::Ventana;
use crate::mmu::{indices, pde_vram, pte_sistema};
use crate::vram::{a_cero, escribir64, leer64};
use crate::Registros;

pub const VA: u64 = 0x7_0000_0000;
pub const IOVA: u64 = 0x5800_0000;
pub const TABLAS: u64 = 0x0470_0000;
/// LI2d-HDR (12-10): 8 PT, 16 MiB -- un render target de float de 1280x720
/// son 14,1 MiB (16 bytes por pixel). Llega justo hasta la IOVA de las
/// texturas (`texturas::IOVA`), sin pisarla.
pub const PTS: usize = 8;
pub const MAX_BYTES: u64 = PTS as u64 * (2 << 20);
/// ** LI2d (12-10): VARIOS RENDER TARGETS -- el G-buffer de un motor
/// diferido escribe 3 o 4 a la vez. Cada uno tiene SU ranura de
/// [`MAX_BYTES`], seguidas en la VA y en la IOVA: el `k` en [`va_de`] y
/// [`iova_de`]. D3D12 deja 8; aqui 4 (lo que mide un G-buffer): cada ranura
/// son 8 tablas de la IOMMU, y las del dominio de la 3060 son 128. Lo que
/// pide mas, por la CPU con su porque.
pub const OBJETIVOS: usize = 4;
/// Las PT de todas las ranuras.
pub const PTS_TODAS: usize = PTS * OBJETIVOS;

/// Donde ve la 3060 el render target `k` (el 0, [`VA`]).
pub const fn va_de(k: usize) -> u64 {
    VA + k as u64 * MAX_BYTES
}

/// Donde lo presta la IOMMU (el 0, [`IOVA`]).
pub const fn iova_de(k: usize) -> u64 {
    IOVA + k as u64 * MAX_BYTES
}
/// Lo mas largo de un lado: 4096 (los campos de 16 bits del recorte dan
/// mas; el mapa, [`MAX_BYTES`], es el que de verdad acota).
pub const MAX_LADO: u32 = 4096;
const PAGINA: u64 = 0x1000;
/// **Q0a1b (12-10): donde puede EMPEZAR un destino**: a 128 B, no a pagina.
/// La 3060 ya dibuja en un color pitch que empieza a mitad de pagina: la
/// ventana del cubo en la pantalla (`cubo::ventana`, x0 a 32 pixeles = 128
/// B), visto en el metal desde X5. Un render target del juego que empieza a
/// mitad de pagina (un mip de 80x45 en ...d400) va igual: su ventana empieza
/// en [`VA`] + lo que se mete en la pagina, y la IOMMU presta desde la
/// pagina de su base.
pub const ALINEA: u64 = 128;

/// **Como guarda el destino cada pixel** (LI2d-HDR, 12-10). Un render target
/// de float del juego lo guarda la casa en floats de 32 bits YA redondeados a
/// su formato (`formato_ia::cuantizar`, N5.16): cuatro por pixel, o uno en un
/// R32_FLOAT. La 3060 escribe ESO mismo (`RF32_GF32_BF32_AF32` o `RF32` de
/// `clc797.h`), y el redondeo al formato lo pone el pegamento del de pixel
/// (`pegamento::Redondeo`): la casa y la 3060 comparten la memoria tal cual.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Flotante {
    /// 8 bits por canal (B8G8R8A8, o R8G8B8A8 con `rgb`).
    #[default]
    No,
    /// Cuatro floats de 32 bits por pixel.
    Rgba32,
    /// Un float de 32 bits por pixel.
    R32,
}

impl Flotante {
    /// Bytes de un pixel.
    pub const fn bytes(self) -> u32 {
        match self {
            Flotante::Rgba32 => 16,
            Flotante::No | Flotante::R32 => 4,
        }
    }

    /// Su numero en la receta (los bits 1..3 de la bandera del destino).
    pub const fn numero(self) -> u32 {
        match self {
            Flotante::No => 0,
            Flotante::Rgba32 => 1,
            Flotante::R32 => 2,
        }
    }

    pub const fn de_numero(n: u32) -> Option<Flotante> {
        match n {
            0 => Some(Flotante::No),
            1 => Some(Flotante::Rgba32),
            2 => Some(Flotante::R32),
            _ => None,
        }
    }

    /// `SET_COLOR_TARGET_FORMAT` (`clc797.h`): `rgb` solo cuenta en 8 bits.
    pub const fn formato(self, rgb: bool) -> u32 {
        match self {
            Flotante::No if rgb => crate::cubo::FORMATO_RGB,
            Flotante::No => crate::tresde::FORMATO,
            Flotante::Rgba32 => FORMATO_RGBA32F,
            Flotante::R32 => FORMATO_R32F,
        }
    }
}

/// `SET_COLOR_TARGET_FORMAT_V_RF32_GF32_BF32_AF32` y `_V_RF32` (`clc797.h`).
pub const FORMATO_RGBA32F: u32 = 0xC0;
pub const FORMATO_R32F: u32 = 0xE5;

/// Un destino: la RAM de la app, fila a fila.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Destino {
    /// Bytes por fila.
    pub fila: u32,
    pub ancho: u32,
    pub alto: u32,
    /// El rojo en el byte 0 (R8G8B8A8); si no, B8G8R8A8.
    pub rgb: bool,
    /// LI2d-HDR: de float, o de 8 bits por canal.
    pub flotante: Flotante,
}

impl Destino {
    pub const fn bytes(&self) -> u64 {
        self.fila as u64 * self.alto as u64
    }

    pub const fn paginas(&self) -> u64 {
        self.bytes().div_ceil(PAGINA)
    }

    /// Se puede dibujar ahi: una medida de 1 a [`MAX_LADO`] por lado, filas
    /// alineadas a 128 B (el destino de color de la 3060) y dentro del mapa.
    pub const fn valido(&self) -> bool {
        self.ancho >= 1 && self.alto >= 1 && self.ancho <= MAX_LADO && self.alto <= MAX_LADO && self.fila as u64 >= self.flotante.bytes() as u64 * self.ancho as u64 && self.fila % 128 == 0 && self.bytes() <= MAX_BYTES
    }

    /// **Q0a1b**: [`valido`](Self::valido) y que la VA de la app empiece a
    /// [`ALINEA`], con lo que se mete en la pagina DENTRO del mapa.
    pub const fn valido_en(&self, va_app: u64) -> bool {
        self.valido() && va_app % ALINEA == 0 && va_app % PAGINA + self.bytes() <= MAX_BYTES
    }

    /// Las paginas que se prestan para un destino en `va_app`: desde la de
    /// su base hasta la de su ultimo byte.
    pub const fn paginas_en(&self, va_app: u64) -> u64 {
        (va_app % PAGINA + self.bytes()).div_ceil(PAGINA)
    }

    /// La de VERRANO: 1280x720 (la de la sombra y la de la Z).
    pub const fn es_la_de_verrano(&self) -> bool {
        self.ancho == crate::cubo::ANCHO && self.alto == crate::cubo::ALTO
    }

    /// Como lo ven las ordenes de VERRANO: una ventana en el (0, 0) de [`VA`],
    /// de SU medida.
    pub const fn ventana(&self) -> Ventana {
        self.ventana_en(0)
    }

    /// **Q0a1b**: la ventana de un destino en `va_app`: empieza en [`VA`] +
    /// lo que `va_app` se mete en su pagina (la IOMMU presta desde la pagina
    /// de la base: `IOVA` es esa pagina).
    pub const fn ventana_en(&self, va_app: u64) -> Ventana {
        self.ventana_de(0, va_app)
    }

    /// **LI2d**: la ventana del render target `k` (su ranura, [`va_de`]).
    pub const fn ventana_de(&self, k: usize, va_app: u64) -> Ventana {
        Ventana { x0: 0, y0: 0, va: va_de(k) + va_app % PAGINA, fila: self.fila, rgb: self.rgb, flotante: self.flotante, ancho: self.ancho, alto: self.alto }
    }
}

/// La entrada de la PD1 del tramo que cuelga [`VA`].
pub const fn entrada_pd1() -> u64 {
    crate::vram::TABLAS[1] + 8 * indices(VA)[2] as u64
}

/// **Mapear** [`MAX_BYTES`] en [`VA`] con PTE de SISTEMA hacia [`IOVA`] (una
/// vez por arranque: a que RAM va la IOVA lo pone la IOMMU en cada dibujo).
/// La PD0 y las PT a cero, las PTE, las PDE y al final la de la PD1, todo
/// RELEIDO; solo si la entrada de la PD1 esta vacia o ya es la nuestra.
pub fn mapear<R: Registros>(r: &mut R) -> Option<(u32, u32)> {
    let pd1 = leer64(r, entrada_pd1());
    if pd1 != 0 && pd1 != pde_vram(TABLAS) {
        return None;
    }
    let pt = |k: usize| TABLAS + PAGINA * (1 + k as u64);
    for k in 0..=PTS_TODAS {
        a_cero(r, TABLAS + PAGINA * k as u64);
    }
    let (mut n, mut bien) = (0u32, 0u32);
    let mut poner = |r: &mut R, dir: u64, v: u64| {
        escribir64(r, dir, v);
        n += 1;
        bien += (leer64(r, dir) == v) as u32;
    };
    // LI2d: las ranuras de los OBJETIVOS, seguidas.
    for q in 0..OBJETIVOS as u64 * MAX_BYTES / PAGINA {
        poner(r, pt((q / 512) as usize) + 8 * (q % 512), pte_sistema(IOVA + q * PAGINA));
    }
    let i0 = indices(VA)[3] as u64;
    for k in 0..PTS_TODAS {
        poner(r, TABLAS + 16 * (i0 + k as u64) + 8, pde_vram(pt(k)));
    }
    poner(r, entrada_pd1(), pde_vram(TABLAS));
    Some((n, bien))
}

// Nada se pisa: su entrada de la PD1, su IOVA y sus tablas son suyas.
const _: () = assert!(VA % (2 << 20) == 0 && indices(VA)[2] != indices(crate::pantalla::VA)[2]);
const _: () = assert!(indices(VA)[2] != indices(crate::volcado::VA)[2] && indices(VA)[2] != indices(crate::video::VA)[2]);
const _: () = assert!(IOVA >= crate::video::IOVA + crate::video::MAX_BYTES);
const _: () = assert!(iova_de(OBJETIVOS) <= crate::texturas::IOVA, "los destinos no pisan las texturas");
const _: () = assert!(TABLAS >= crate::video::TABLAS + (1 + crate::video::PTS as u64) * PAGINA && TABLAS + (1 + PTS_TODAS as u64) * PAGINA <= 0x0800_0000);
const _: () = assert!(indices(VA)[3] as usize + PTS_TODAS <= 512 && (va_de(OBJETIVOS) - 1) >> 32 == VA >> 32, "en la PD0, y con los 32 bits altos de VA");

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_medida_de_verrano_y_las_demas() {
        let d = Destino { fila: 1280 * 4, ancho: 1280, alto: 720, flotante: crate::destino::Flotante::No, rgb: false };
        assert!(d.valido() && d.es_la_de_verrano());
        assert_eq!(d.paginas(), 900);
        assert_eq!(d.ventana(), Ventana { x0: 0, y0: 0, va: VA, fila: 5120, flotante: crate::destino::Flotante::No, rgb: false, ancho: 1280, alto: 720 });
        assert_eq!(indices(VA)[2], 56);
        // ** Q0a1: otras medidas, si caben en el mapa.
        for (an, al) in [(1920, 1080), (640, 360), (320, 200), (1, 1), (4096, 512), (1898, 1064)] {
            let fila = (4 * an as u32).next_multiple_of(128);
            let o = Destino { fila, ancho: an, alto: al, flotante: crate::destino::Flotante::No, rgb: false };
            assert!(o.valido(), "{}x{}", an, al);
            assert!(!o.es_la_de_verrano() || (an, al) == (1280, 720));
            assert_eq!((o.ventana().ancho, o.ventana().alto), (an, al));
        }
        assert!(!Destino { fila: 1280 * 4 + 4, ..d }.valido(), "filas sin alinear");
        assert!(!Destino { fila: 4 * 1279, ancho: 1280, ..d }.valido(), "una fila mas corta que la medida");
        assert!(!Destino { ancho: 0, ..d }.valido() && !Destino { alto: 0, ..d }.valido(), "vacio");
        assert!(!Destino { ancho: 4097, fila: 4097 * 4 + 124, alto: 1, flotante: crate::destino::Flotante::No, rgb: false }.valido(), "un lado de mas");
        assert!(!Destino { ancho: 4096, fila: 16384, alto: 1100, flotante: crate::destino::Flotante::No, rgb: false }.valido(), "mas que el mapa (16 MiB)");
        // ** LI2d-HDR: el render target de float de 1280x720 (16 B por pixel)
        // cabe; una fila mas corta que 16 B por pixel, no; 1920x1080 de float
        // (31,6 MiB) no cabe en el mapa todavia.
        let f = Destino { fila: 1280 * 16, flotante: Flotante::Rgba32, ..d };
        assert!(f.valido() && f.es_la_de_verrano());
        assert_eq!(f.ventana().flotante, Flotante::Rgba32);
        assert!(!Destino { fila: 1280 * 4, ..f }.valido(), "16 B por pixel");
        assert!(!Destino { fila: 1920 * 16, ancho: 1920, alto: 1080, ..f }.valido(), "1920x1080 de float no cabe en 16 MiB");
        assert!(Destino { flotante: Flotante::R32, ..d }.valido(), "un R32 son 4 B por pixel");
        assert_eq!((Flotante::Rgba32.formato(false), Flotante::R32.formato(true), Flotante::No.formato(true)), (0xC0, 0xE5, 0xD5));
        for f in [Flotante::No, Flotante::Rgba32, Flotante::R32] {
            assert_eq!(Flotante::de_numero(f.numero()), Some(f));
        }
        assert_eq!(Flotante::de_numero(3), None);
    }
}
