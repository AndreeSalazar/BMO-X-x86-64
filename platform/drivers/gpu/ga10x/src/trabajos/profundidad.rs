//! **P3b4c: LA PROFUNDIDAD EN LA 3060** -- la prueba de Z la hace el
//! hardware, no la CPU. BMOX-12 dibuja con `DepthEnable` y `LESS` (D32): sin
//! esto, el cubo solo sale bien porque es convexo y se le descartan las
//! caras de atras; con dos objetos, o sin descarte, saldria MAL.
//!
//! capa: puro -- el mapa, la superficie y las ordenes; la VRAM y los
//! registros los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- la Z es de la tarjeta: la CPU dice la regla
//!           (`D3D12_COMPARISON_FUNC`, si se escribe, con que se limpia) y la
//!           3060 compara cada pixel
//!
//! # Donde vive
//!
//! ```text
//!    VRAM     0x0A00_0000, 4 MiB (tras los buferes del GR, 0x0800_0000 +
//!             26 MiB): la superficie ZF32 de 1280x720, BLOQUE-LINEAL
//!    VA       0x8_0000_0000 (la PD1 del tramo; tras el destino, 0x7)
//!    TABLAS   la PD0 y las 2 PT en VRAM 0x0480_0000 (tras las del destino)
//!    PTE      de VRAM con kind GENERIC_MEMORY (0x06, `dev_mmu.h` de
//!             NVIDIA para tu102/ga100): la Z siempre es bloque-lineal, y
//!             ZF32 no tiene kind propio -- la de siempre (0) es PITCH
//! ```
//!
//! # La superficie
//!
//! Un GOB son 64 bytes x 8 filas; el bloque, 1 GOB de ancho y 16 de alto
//! (128 filas). 1280 x 4 = 5120 bytes = 80 GOBs de ancho; 720 filas son 5,6
//! bloques: se redondea a 6 (768 filas). 5120 x 768 = 3.932.160 bytes.
//!
//! # Como se sabe (el metal)
//!
//! `gpu verrano bmox12 30 z`: el cubo SIN descarte de caras y CON Z. La
//! imagen buena es la de D3D12 (sus huellas: las caras de atras quedan
//! detras). Sin Z, o con una Z que no funciona, las de atras se pintan
//! encima en el orden en que llegan: DISTINTO.

use crate::cubo::{Ordenes, ALTO, ANCHO};
use crate::mmu::{indices, pde_vram, pte_vram};
use crate::vram::{a_cero, escribir64, leer64};
use crate::Registros;

/// La superficie ZF32, en VRAM.
pub const VRAM: u64 = 0x0A00_0000;
/// Donde la ve la 3060.
pub const VA: u64 = 0x8_0000_0000;
/// La PD0 y las PT, en VRAM.
pub const TABLAS: u64 = 0x0480_0000;
/// PT de 2 MiB: la superficie mide 3,75 MiB.
pub const PTS: usize = 2;
/// `NV_MMU_PTE_KIND_GENERIC_MEMORY` (`dev_mmu.h`, tu102 y ga100).
pub const KIND_GENERICO: u64 = 0x06;
/// El kind va en los bits 56..63 de una PTE de la version 2 (nouveau,
/// `gp100_vmm_pgt_pte`: `map->type |= (u64)map->kind << 56`).
const KIND_BIT: u32 = 56;
const PAGINA: u64 = 0x1000;

/// Un GOB: 64 bytes de ancho y 8 filas.
pub const GOB_ANCHO: u32 = 64;
pub const GOB_ALTO: u32 = 8;
/// El bloque: 2^4 = 16 GOBs de alto (128 filas), 1 de ancho y 1 de hondo.
pub const BLOQUE_ALTO_LOG2: u32 = 4;
/// Las filas de un bloque.
pub const FILAS_BLOQUE: u32 = GOB_ALTO << BLOQUE_ALTO_LOG2;
/// Bytes de una fila de la superficie (4 por pixel).
pub const FILA: u32 = 4 * ANCHO;
/// Las filas, redondeadas a bloques enteros.
pub const FILAS: u32 = ALTO.div_ceil(FILAS_BLOQUE) * FILAS_BLOQUE;
/// Lo que mide la superficie.
pub const BYTES: u64 = FILA as u64 * FILAS as u64;

/// `SET_ZT_A` (y detras B, FORMAT, BLOCK_SIZE y ARRAY_PITCH), `SET_ZT_SIZE_A`
/// (y B, C), y el resto de lo de la Z (`clc797.h` de NVIDIA).
pub const SET_ZT_A: u32 = 0x0fe0;
pub const SET_ZT_SIZE_A: u32 = 0x1228;
pub const SET_ZT_SELECT: u32 = 0x1538;
pub const SET_ZT_LAYER: u32 = 0x179c;
pub const SET_ZCULL: u32 = 0x1968;
pub const SET_DEPTH_TEST: u32 = 0x12cc;
pub const SET_DEPTH_WRITE: u32 = 0x12e8;
pub const SET_DEPTH_FUNC: u32 = 0x130c;
pub const SET_Z_CLEAR_VALUE: u32 = 0x0d90;
/// `SET_ZT_FORMAT_V_ZF32`.
pub const ZF32: u32 = 0x0A;
/// `SET_ZT_SIZE_C`: una capa (`THIRD_DIMENSION` 1, `CONTROL`
/// `ARRAY_SIZE_IS_ONE`).
pub const UNA_CAPA: u32 = 1 | 1 << 16;
/// `CLEAR_SURFACE_Z_ENABLE`.
pub const LIMPIAR_Z: u32 = 1;
/// 1.0f, lo que D3D pone de costumbre en `ClearDepthStencilView`.
pub const UNO: u32 = 0x3F80_0000;

/// **La regla de Z de un dibujo**, como la da D3D12.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Z {
    /// `D3D12_COMPARISON_FUNC`: 1 nunca, 2 menor, 3 igual, 4 menor o igual,
    /// 5 mayor, 6 distinto, 7 mayor o igual, 8 siempre. `SET_DEPTH_FUNC`
    /// tiene los MISMOS numeros en su lado D3D (`V_D3D_*`).
    pub funcion: u32,
    /// `DepthWriteMask` ALL.
    pub escribir: bool,
    /// Limpiar la Z antes de dibujar, con estos bits (un f32). Lo que la app
    /// pidio con `ClearDepthStencilView` desde su ultimo dibujo.
    pub limpiar: Option<u32>,
}

impl Z {
    /// Una funcion de D3D, y un valor de limpieza entre 0 y 1 (D3D lo exige).
    pub fn valida(&self) -> bool {
        (1..=8).contains(&self.funcion) && self.limpiar.is_none_or(|b| (0.0..=1.0).contains(&f32::from_bits(b)))
    }
}

/// La PTE de una pagina de la superficie.
pub const fn pte(pagina: u64) -> u64 {
    pte_vram(pagina) | KIND_GENERICO << KIND_BIT
}

/// La entrada de la PD1 del tramo que cuelga [`VA`].
pub const fn entrada_pd1() -> u64 {
    crate::vram::TABLAS[1] + 8 * indices(VA)[2] as u64
}

/// **Mapear la superficie** en [`VA`]: la PD0 y las PT a cero, las PTE de
/// VRAM con kind generico, las PDE y al final la de la PD1 (de la hoja a la
/// raiz), todo RELEIDO. Una vez: la superficie no se mueve. `None` si la
/// entrada de la PD1 ya es de otro. `(escrituras, releidas)`.
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
    for q in 0..BYTES.div_ceil(PAGINA) {
        poner(r, pt((q / 512) as usize) + 8 * (q % 512), pte(VRAM + q * PAGINA));
    }
    let i0 = indices(VA)[3] as u64;
    for k in 0..PTS {
        poner(r, TABLAS + 16 * (i0 + k as u64) + 8, pde_vram(pt(k)));
    }
    poner(r, entrada_pd1(), pde_vram(TABLAS));
    Some((n, bien))
}

/// **Las ordenes de la Z**, detras del estado de `hasta_el_dibujo` (que la
/// deja APAGADA: `SET_ZT_SELECT 0`, `SET_DEPTH_TEST 0`): la superficie, la
/// regla, y la limpieza si la hay. El orden de NVK (`nvk_cmd_draw.c`):
/// A, B, FORMAT, BLOCK_SIZE, ARRAY_PITCH; SELECT; SIZE A, B, C; LAYER.
pub(crate) fn ordenes(e: &mut Ordenes, z: &Z) {
    let bloque = BLOQUE_ALTO_LOG2 << 4;
    e.m(SET_ZT_A, &[(VA >> 32) as u32, VA as u32, ZF32, bloque, (BYTES >> 2) as u32]);
    e.m(SET_ZT_SELECT, &[1]);
    e.m(SET_ZT_SIZE_A, &[ANCHO, ALTO, UNA_CAPA]);
    e.m(SET_ZT_LAYER, &[0]);
    // Sin ZCULL: pide su propia memoria, y aqui no se le da.
    e.m(SET_ZCULL, &[0]);
    if let Some(v) = z.limpiar {
        e.m(SET_Z_CLEAR_VALUE, &[v]);
        e.m(crate::tresde::SET_CLEAR_SURFACE_CONTROL, &[0]);
        e.m(crate::tresde::CLEAR_SURFACE, &[LIMPIAR_Z]);
        e.m(crate::tresde::WAIT_FOR_IDLE, &[0]);
    }
    e.m(SET_DEPTH_TEST, &[1]);
    e.m(SET_DEPTH_FUNC, &[z.funcion]);
    e.m(SET_DEPTH_WRITE, &[z.escribir as u32]);
}

const _: () = assert!(VA % (2 << 20) == 0 && indices(VA)[2] != indices(crate::destino::VA)[2]);
const _: () = assert!(BYTES <= PTS as u64 * (2 << 20) && BYTES == 3_932_160 && FILA % GOB_ANCHO == 0);
// Tras las tablas del destino, y antes de los buferes del GR; la superficie,
// tras los buferes del GR (0x0800_0000 + 26560 KiB).
const _: () = assert!(TABLAS >= crate::destino::TABLAS + (1 + crate::destino::PTS as u64) * PAGINA && TABLAS + (1 + PTS as u64) * PAGINA <= 0x0800_0000);
const _: () = assert!(VRAM >= crate::gr::VRAM + 26560 * 1024 && VRAM % (2 << 20) == 0);

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_superficie_y_su_mapa() {
        assert_eq!((FILA, FILAS, FILAS_BLOQUE), (5120, 768, 128));
        assert_eq!(BYTES.div_ceil(PAGINA), 960);
        // La PTE: valida, VRAM (apertura 0), la pagina y el kind 6 arriba.
        let p = pte(VRAM);
        assert_eq!(p & 1, 1);
        assert_eq!(p >> 56, 6);
        assert_eq!(p & ((1 << 56) - 1), pte_vram(VRAM));
        assert_eq!(indices(VA)[2], 64);
    }

    #[test]
    fn las_ordenes_de_la_z() {
        let mut e = crate::cubo::hasta_el_dibujo_de(&crate::destino::Destino { fila: 5120, ancho: 1280, alto: 720, rgb: false }.ventana(), false, false, None, 64);
        let antes = e.n;
        ordenes(&mut e, &Z { funcion: 2, escribir: true, limpiar: Some(UNO) });
        let o = &e.o[antes..e.n];
        let cab = |m: u32, n: u32| crate::copia::cabecera_en(crate::tresde::SUBCANAL, m, n);
        assert_eq!(&o[..6], &[cab(SET_ZT_A, 5), 8, 0, ZF32, 0x40, (BYTES >> 2) as u32]);
        assert!(o.windows(2).any(|w| w == [cab(SET_Z_CLEAR_VALUE, 1), UNO]));
        assert!(o.windows(2).any(|w| w == [cab(crate::tresde::CLEAR_SURFACE, 1), LIMPIAR_Z]));
        assert!(o.ends_with(&[cab(SET_DEPTH_TEST, 1), 1, cab(SET_DEPTH_FUNC, 1), 2, cab(SET_DEPTH_WRITE, 1), 1]));
        assert!(Z { funcion: 2, escribir: true, limpiar: Some(UNO) }.valida());
        assert!(!Z { funcion: 9, escribir: true, limpiar: None }.valida());
        assert!(!Z { funcion: 2, escribir: true, limpiar: Some(2.0f32.to_bits()) }.valida());
    }
}
