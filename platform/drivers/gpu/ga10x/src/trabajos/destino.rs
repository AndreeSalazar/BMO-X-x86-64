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
pub const PTS: usize = 4;
pub const MAX_BYTES: u64 = PTS as u64 * (2 << 20);
/// Lo mas largo de un lado: 4096 (los campos de 16 bits del recorte dan
/// mas; el mapa, [`MAX_BYTES`], es el que de verdad acota).
pub const MAX_LADO: u32 = 4096;
const PAGINA: u64 = 0x1000;

/// Un destino: la RAM de la app, fila a fila.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Destino {
    /// Bytes por fila.
    pub fila: u32,
    pub ancho: u32,
    pub alto: u32,
    /// El rojo en el byte 0 (R8G8B8A8); si no, B8G8R8A8.
    pub rgb: bool,
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
        self.ancho >= 1 && self.alto >= 1 && self.ancho <= MAX_LADO && self.alto <= MAX_LADO && self.fila >= 4 * self.ancho && self.fila % 128 == 0 && self.bytes() <= MAX_BYTES
    }

    /// La de VERRANO: 1280x720 (la de la sombra y la de la Z).
    pub const fn es_la_de_verrano(&self) -> bool {
        self.ancho == crate::cubo::ANCHO && self.alto == crate::cubo::ALTO
    }

    /// Como lo ven las ordenes de VERRANO: una ventana en el (0, 0) de [`VA`],
    /// de SU medida.
    pub const fn ventana(&self) -> Ventana {
        Ventana { x0: 0, y0: 0, va: VA, fila: self.fila, rgb: self.rgb, ancho: self.ancho, alto: self.alto }
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
    for k in 0..=PTS {
        a_cero(r, TABLAS + PAGINA * k as u64);
    }
    let (mut n, mut bien) = (0u32, 0u32);
    let mut poner = |r: &mut R, dir: u64, v: u64| {
        escribir64(r, dir, v);
        n += 1;
        bien += (leer64(r, dir) == v) as u32;
    };
    for q in 0..MAX_BYTES / PAGINA {
        poner(r, pt((q / 512) as usize) + 8 * (q % 512), pte_sistema(IOVA + q * PAGINA));
    }
    let i0 = indices(VA)[3] as u64;
    for k in 0..PTS {
        poner(r, TABLAS + 16 * (i0 + k as u64) + 8, pde_vram(pt(k)));
    }
    poner(r, entrada_pd1(), pde_vram(TABLAS));
    Some((n, bien))
}

// Nada se pisa: su entrada de la PD1, su IOVA y sus tablas son suyas.
const _: () = assert!(VA % (2 << 20) == 0 && indices(VA)[2] != indices(crate::pantalla::VA)[2]);
const _: () = assert!(indices(VA)[2] != indices(crate::volcado::VA)[2] && indices(VA)[2] != indices(crate::video::VA)[2]);
const _: () = assert!(IOVA >= crate::video::IOVA + crate::video::MAX_BYTES);
const _: () = assert!(TABLAS >= crate::video::TABLAS + (1 + crate::video::PTS as u64) * PAGINA && TABLAS + (1 + PTS as u64) * PAGINA <= 0x0800_0000);

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_medida_de_verrano_y_las_demas() {
        let d = Destino { fila: 1280 * 4, ancho: 1280, alto: 720, rgb: false };
        assert!(d.valido() && d.es_la_de_verrano());
        assert_eq!(d.paginas(), 900);
        assert_eq!(d.ventana(), Ventana { x0: 0, y0: 0, va: VA, fila: 5120, rgb: false, ancho: 1280, alto: 720 });
        assert_eq!(indices(VA)[2], 56);
        // ** Q0a1: otras medidas, si caben en el mapa.
        for (an, al) in [(1920, 1080), (640, 360), (320, 200), (1, 1), (4096, 512), (1898, 1064)] {
            let fila = (4 * an as u32).next_multiple_of(128);
            let o = Destino { fila, ancho: an, alto: al, rgb: false };
            assert!(o.valido(), "{}x{}", an, al);
            assert!(!o.es_la_de_verrano() || (an, al) == (1280, 720));
            assert_eq!((o.ventana().ancho, o.ventana().alto), (an, al));
        }
        assert!(!Destino { fila: 1280 * 4 + 4, ..d }.valido(), "filas sin alinear");
        assert!(!Destino { fila: 4 * 1279, ancho: 1280, ..d }.valido(), "una fila mas corta que la medida");
        assert!(!Destino { ancho: 0, ..d }.valido() && !Destino { alto: 0, ..d }.valido(), "vacio");
        assert!(!Destino { ancho: 4097, fila: 4097 * 4 + 124, alto: 1, rgb: false }.valido(), "un lado de mas");
        assert!(!Destino { ancho: 2048, fila: 8192, alto: 1100, rgb: false }.valido(), "mas que el mapa (8 MiB)");
    }
}
