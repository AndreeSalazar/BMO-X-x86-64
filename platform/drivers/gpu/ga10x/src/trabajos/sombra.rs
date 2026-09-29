//! **P3b4c.6b: LA SOMBRA DEL COLOR** -- la 3060 no dibuja con una Z en
//! bloque sobre un color PITCH (el metal, 28-09 20:31: `gpu verrano bmox12
//! 30 z` dio Xid 69 AL DIBUJAR y el canal GR quedo muerto). Con Z, el color
//! va a una SOMBRA bloque-lineal en VRAM, junto a la Z, y el motor de COPIA
//! la lleva despues al destino de verdad (la ventana de la pantalla o el back
//! buffer de la app, los dos PITCH). Es lo que hace NVK
//! (`nvk_rendering_linear` / `nvk_linear_render_copy`).
//!
//! capa: puro -- el mapa, la superficie y las ordenes de la copia; la VRAM,
//! los registros y el timbre los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- el color y la Z, los dos en bloque: la pareja que
//!           la 3060 acepta
//!
//! # Donde vive
//!
//! ```text
//!    VRAM     0x0A40_0000, 4 MiB (tras la Z, 0x0A00_0000 + 4 MiB): la
//!             superficie A8R8G8B8 de 1280x720, BLOQUE-LINEAL como la Z
//!             (el mismo GOB, bloques de 16 GOBs de alto)
//!    VA       0x9_0000_0000 (la PD1 del tramo; tras la Z, 0x8)
//!    TABLAS   la PD0 y las 2 PT en VRAM 0x0490_0000 (tras las de la Z)
//!    PTE      de VRAM con kind GENERIC_MEMORY (0x06): el de todo lo que va
//!             en bloque sin comprimir en Turing y despues
//!             (`tu102_choose_pte_kind` de NVK, `default: 0x06`)
//!    ORDENES  el canal de COPIA: la pagina de ordenes de L1d3 en +0xC00
//!             (el volcado usa +0x800..+0xAD8) y su semaforo en +0x200
//! ```
//!
//! # El dibujo con sombra
//!
//! ```text
//!    1  si nadie LIMPIA el color, el destino -> la sombra (pitch a bloque):
//!       lo que ya habia en el back buffer tiene que seguir debajo
//!    2  el dibujo de la 3060, con el color en la sombra y la Z al lado
//!    3  pagado el dibujo, la sombra -> el destino (bloque a pitch)
//! ```
//!
//! La copia va por el canal de COPIA, no por el GR: es el que ya COPIO en el
//! metal (L1d3, el volcado de cada fotograma). El kernel espera el semaforo
//! del dibujo antes de lanzarla, asi que no hace falta un semaforo entre
//! canales.
//!
//! # La orden (`clc7b5.h`)
//!
//! La del volcado (OFFSET_IN/OUT, PITCH, LINE_LENGTH_IN, LINE_COUNT) mas el
//! lado en bloque: `SET_{SRC,DST}_BLOCK_SIZE`, WIDTH (en BYTES: sin remapeo
//! el motor cuenta bytes, como NVK con `copy_el_size_B` 1), HEIGHT, DEPTH,
//! LAYER, y el origen (`SRC/DST_ORIGIN_X/Y`, los de Pascal y despues). En
//! `LAUNCH_DMA` el lado en bloque lleva su bit de LAYOUT a 0.

use crate::canal::{GPFIFO, GPFIFO_ENTRADAS};
use crate::copia::{cabecera, entrada, escribir, leer32, AMPERE_DMA_COPY_B, LANZAR};
use crate::cubo::{Ordenes, Ventana, ALTO, ANCHO};
use crate::mmu::{indices, pde_vram};
use crate::profundidad::{pte, BLOQUE_ALTO_LOG2, FILAS};
use crate::vram::{a_cero, escribir64, leer64};
use crate::Registros;

/// La superficie del color, en VRAM.
pub const VRAM: u64 = 0x0A40_0000;
/// Donde la ve la 3060.
pub const VA: u64 = 0x9_0000_0000;
/// La PD0 y las PT, en VRAM.
pub const TABLAS: u64 = 0x0490_0000;
/// PT de 2 MiB: la superficie mide 3,75 MiB.
pub const PTS: usize = 2;
const PAGINA: u64 = 0x1000;

/// Bytes de una fila (4 por pixel) y lo que mide: lo mismo que la Z.
pub const FILA: u32 = 4 * ANCHO;
pub const BYTES: u64 = FILA as u64 * FILAS as u64;

/// `SET_COLOR_TARGET_MEMORY`: BLOCK_WIDTH ONE_GOB (0 en 3:0), BLOCK_HEIGHT
/// SIXTEEN_GOBS (4 en 7:4), BLOCK_DEPTH ONE_GOB, LAYOUT BLOCKLINEAR (bit 12
/// a 0). El mismo bloque que la Z (`SET_ZT_BLOCK_SIZE`).
pub const MEMORIA_BLOQUE: u32 = BLOQUE_ALTO_LOG2 << 4;

/// Los metodos de `clc7b5.h` de la copia en bloque.
const SET_OBJECT: u32 = 0x000;
const SET_SEMAPHORE_A: u32 = 0x240;
const LAUNCH_DMA: u32 = 0x300;
const OFFSET_IN_UPPER: u32 = 0x400;
pub const SET_DST_BLOCK_SIZE: u32 = 0x070c;
pub const SET_SRC_BLOCK_SIZE: u32 = 0x0728;
pub const SRC_ORIGIN_X: u32 = 0x0744;
pub const DST_ORIGIN_X: u32 = 0x074c;
/// `SET_*_BLOCK_SIZE`: WIDTH ONE_GOB (3:0), HEIGHT SIXTEEN_GOBS (7:4), DEPTH
/// ONE_GOB (11:8), GOB_HEIGHT FERMI_8 (1 en 15:12).
pub const BLOQUE_COPIA: u32 = BLOQUE_ALTO_LOG2 << 4 | 1 << 12;
/// `LAUNCH_DMA_SRC_MEMORY_LAYOUT_PITCH` y `DST_..._PITCH`.
pub const ORIGEN_PITCH: u32 = 1 << 7;
pub const DESTINO_PITCH: u32 = 1 << 8;
/// `LAUNCH_DMA_MULTI_LINE_ENABLE`.
pub const MULTI_LINE: u32 = 1 << 9;

/// Las ordenes y el semaforo, en las paginas del canal de copia (sin pisar
/// las de L1d3, +0x000, ni las del volcado, +0x800 y el semaforo +0x100).
pub const EMPUJE: u64 = crate::copia::EMPUJE + 0xC00;
pub const SEMAFORO: u64 = crate::copia::SEMAFORO + 0x200;
/// Lo que escribe la 3060 al acabar cada copia.
pub const PAGA_A_LA_SOMBRA: u32 = 0x3060_5B01;
pub const PAGA_AL_DESTINO: u32 = 0x3060_5B02;
/// Palabras de una copia.
pub const ORDENES: usize = 26;

/// La PTE de una pagina de la sombra: VRAM, kind generico (la de la Z).
pub const fn pte_sombra(pagina: u64) -> u64 {
    pte(pagina)
}

/// La entrada de la PD1 del tramo que cuelga [`VA`].
pub const fn entrada_pd1() -> u64 {
    crate::vram::TABLAS[1] + 8 * indices(VA)[2] as u64
}

/// **Mapear la sombra** en [`VA`], como la Z (`profundidad::mapear`): la
/// PD0 y las PT a cero, las PTE, las PDE y al final la de la PD1, todo
/// RELEIDO. Una vez: la superficie no se mueve. `None` si la entrada de la
/// PD1 ya es de otro. `(escrituras, releidas)`.
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
        poner(r, pt((q / 512) as usize) + 8 * (q % 512), pte_sombra(VRAM + q * PAGINA));
    }
    let i0 = indices(VA)[3] as u64;
    for k in 0..PTS {
        poner(r, TABLAS + 16 * (i0 + k as u64) + 8, pde_vram(pt(k)));
    }
    poner(r, entrada_pd1(), pde_vram(TABLAS));
    Some((n, bien))
}

/// **El color a la sombra**, en las ordenes del dibujo: el destino de color
/// 0 pasa a ser la sombra en bloque, con el formato del destino de verdad
/// (`v.rgb`). Va detras de `hasta_el_dibujo_de` (que lo puso en el pitch con
/// la Z apagada) y antes de la Z y de cualquier limpieza.
pub(crate) fn al_color(e: &mut Ordenes, v: &Ventana) {
    let formato = if v.rgb { crate::cubo::FORMATO_RGB } else { crate::tresde::FORMATO };
    // A, B, WIDTH (pixeles en bloque), HEIGHT, FORMAT, MEMORY,
    // THIRD_DIMENSION, ARRAY_PITCH (bytes de la capa >> 2, como NVK).
    e.m(crate::tresde::SET_COLOR_TARGET_A0, &[(VA >> 32) as u32, VA as u32, ANCHO, ALTO, formato, MEMORIA_BLOQUE, 1, (BYTES >> 2) as u32]);
}

/// **La copia** entre la sombra y la ventana `v` (pitch): `a_la_sombra` =
/// de la ventana a la sombra (antes del dibujo); si no, de la sombra a la
/// ventana (despues). 1280x720 pixeles de 4 bytes, y el semaforo.
pub fn copia(v: &Ventana, a_la_sombra: bool) -> [u32; ORDENES] {
    let (origen, destino, paga, layout, bloque) = if a_la_sombra {
        (v.va, VA, PAGA_A_LA_SOMBRA, ORIGEN_PITCH, SET_DST_BLOCK_SIZE)
    } else {
        (VA, v.va, PAGA_AL_DESTINO, DESTINO_PITCH, SET_SRC_BLOCK_SIZE)
    };
    let origen_xy = if a_la_sombra { DST_ORIGIN_X } else { SRC_ORIGIN_X };
    let s = crate::copia::va(SEMAFORO);
    // El paso del lado en bloque no lo mira el motor; se pone el de su fila.
    let (paso_in, paso_out) = if a_la_sombra { (v.fila, FILA) } else { (FILA, v.fila) };
    [
        cabecera(SET_OBJECT, 1),
        AMPERE_DMA_COPY_B,
        cabecera(OFFSET_IN_UPPER, 8),
        (origen >> 32) as u32,
        origen as u32,
        (destino >> 32) as u32,
        destino as u32,
        paso_in,
        paso_out,
        FILA, // LINE_LENGTH_IN, en bytes
        ALTO, // LINE_COUNT
        // El lado en bloque: BLOCK_SIZE, WIDTH (bytes), HEIGHT, DEPTH, LAYER.
        cabecera(bloque, 5),
        BLOQUE_COPIA,
        FILA,
        ALTO,
        1,
        0,
        // Su origen, (0, 0).
        cabecera(origen_xy, 2),
        0,
        0,
        cabecera(SET_SEMAPHORE_A, 3),
        (s >> 32) as u32,
        s as u32,
        paga,
        cabecera(LAUNCH_DMA, 1),
        (LANZAR & !(ORIGEN_PITCH | DESTINO_PITCH)) | layout | MULTI_LINE,
    ]
}

/// **Lo que pide la sombra a un dibujo**: `None` sin Z (el color va al
/// pitch, como siempre); si no, la ventana PITCH donde acaba el color (el
/// destino de la app o la ventana de la pantalla `v`) y si hay que CARGAR la
/// sombra antes: con destino y sin limpieza del color (la pantalla de `gpu
/// verrano` se limpia siempre), lo que ya pinto la app sigue debajo.
pub fn plan(v: &Ventana, d: &crate::tuberia::Dibujo) -> Option<(Ventana, bool)> {
    d.z?;
    Some(match d.destino {
        // Z1: acaba en la pantalla; si la app no limpia, lo de debajo es lo
        // que ya se ve.
        Some(_) if d.pantalla => (*v, d.color.is_none()),
        Some((_, dst)) => (dst.ventana(), d.color.is_none()),
        None => (*v, false),
    })
}

/// Lo que paga la copia en un sentido.
pub const fn paga(a_la_sombra: bool) -> u32 {
    if a_la_sombra { PAGA_A_LA_SOMBRA } else { PAGA_AL_DESTINO }
}

/// **Preparar una copia** en la entrada `e` del GPFIFO de COPIA: su
/// semaforo a cero, sus ordenes y la entrada, todo RELEIDO (son 29
/// palabras). El timbre, con `volcado::lanzar` (la MMU invalidada, GP_PUT y
/// la ficha): el canal y su GPFIFO son los del volcado.
pub fn preparar<R: Registros>(r: &mut R, e: u32, o: &[u32; ORDENES]) -> bool {
    let en = entrada(crate::copia::va(EMPUJE), ORDENES as u32);
    e < GPFIFO_ENTRADAS
        && escribir(r, SEMAFORO, &[0]) == 1
        && escribir(r, EMPUJE, o) == ORDENES
        && escribir(r, GPFIFO + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
}

/// La 3060 ya pago la copia de ese sentido.
pub fn pagada<R: Registros>(r: &mut R, a_la_sombra: bool) -> bool {
    leer32(r, SEMAFORO) == paga(a_la_sombra)
}

// Nada se pisa: su entrada de la PD1, sus tablas y su VRAM son suyas.
const _: () = assert!(VA % (2 << 20) == 0 && indices(VA)[2] != indices(crate::profundidad::VA)[2] && indices(VA)[2] != indices(crate::destino::VA)[2]);
const _: () = assert!(BYTES <= PTS as u64 * (2 << 20) && BYTES == crate::profundidad::BYTES);
const _: () = assert!(TABLAS >= crate::profundidad::TABLAS + (1 + crate::profundidad::PTS as u64) * PAGINA && TABLAS + (1 + PTS as u64) * PAGINA <= 0x0800_0000);
const _: () = assert!(VRAM >= crate::profundidad::VRAM + crate::profundidad::BYTES && VRAM % (2 << 20) == 0);
// Las ordenes caben en su cuarto de pagina, detras de las del volcado.
const _: () = assert!(EMPUJE >= crate::volcado::EMPUJE + 4 * crate::volcado::PALABRAS_TANDA as u64 && EMPUJE + 4 * ORDENES as u64 <= crate::copia::EMPUJE + PAGINA);
const _: () = assert!(SEMAFORO != crate::volcado::SEMAFORO && SEMAFORO < crate::copia::SEMAFORO + PAGINA);

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::copia::cabecera_en;

    #[test]
    fn la_superficie_y_su_mapa() {
        assert_eq!(indices(VA)[2], 72);
        assert_eq!(BYTES.div_ceil(PAGINA), 960);
        let p = pte_sombra(VRAM);
        assert_eq!(p >> 56, 6, "kind generico: en bloque");
        assert_eq!(MEMORIA_BLOQUE & crate::tresde::MEMORIA_PITCH, 0, "LAYOUT a 0: bloque");
        assert_eq!(MEMORIA_BLOQUE, 0x40);
        assert_eq!(BLOQUE_COPIA, 0x1040);
    }

    #[test]
    fn la_copia_de_la_sombra_a_la_ventana() {
        let v = crate::destino::Destino { fila: 5120, ancho: 1280, alto: 720, rgb: false }.ventana();
        let o = copia(&v, false);
        assert_eq!(o[2], cabecera_en(4, OFFSET_IN_UPPER, 8));
        assert_eq!((o[3] as u64) << 32 | o[4] as u64, VA);
        assert_eq!((o[5] as u64) << 32 | o[6] as u64, crate::destino::VA);
        assert_eq!((o[9], o[10]), (5120, 720));
        assert_eq!(o[11], cabecera_en(4, SET_SRC_BLOCK_SIZE, 5), "el ORIGEN es el que va en bloque");
        assert_eq!(o[17], cabecera_en(4, SRC_ORIGIN_X, 2));
        assert_eq!(o[23], PAGA_AL_DESTINO);
        let l = o[ORDENES - 1];
        assert_eq!(l & ORIGEN_PITCH, 0, "origen en bloque");
        assert_eq!(l & DESTINO_PITCH, DESTINO_PITCH, "destino pitch");
        assert_eq!(l & MULTI_LINE, MULTI_LINE);
        assert_eq!(l & 3, 2, "NON_PIPELINED");
        assert_eq!(l >> 3 & 3, 1, "semaforo de una palabra");
    }

    #[test]
    fn el_plan_de_un_dibujo() {
        use crate::tuberia::Dibujo;
        let pantalla = Ventana { x0: 320, y0: 180, va: 0x4_0000_1000, fila: 7680, rgb: false };
        let dst = crate::destino::Destino { fila: 5120, ancho: 1280, alto: 720, rgb: true };
        let z = Some(crate::profundidad::Z { funcion: 2, escribir: true, limpiar: None });
        assert_eq!(plan(&pantalla, &Dibujo::default()), None, "sin Z, sin sombra");
        assert_eq!(plan(&pantalla, &Dibujo { z, ..Dibujo::default() }), Some((pantalla, false)), "la pantalla: se limpia, no se carga");
        let con_destino = Dibujo { z, destino: Some((0x1000_0000, dst)), ..Dibujo::default() };
        assert_eq!(plan(&pantalla, &con_destino), Some((dst.ventana(), true)), "el back buffer sin limpiar: se CARGA");
        assert_eq!(plan(&pantalla, &Dibujo { color: Some(0), ..con_destino }), Some((dst.ventana(), false)));
        // Z1: el back buffer va a la PANTALLA; si la app no limpia, se carga
        // lo que ya se ve (no la RAM de la app, que no se presta).
        let a_pantalla = Dibujo { cadena: true, pantalla: true, ..con_destino };
        assert_eq!(plan(&pantalla, &a_pantalla), Some((pantalla, true)));
        assert_eq!(plan(&pantalla, &Dibujo { color: Some(0), ..a_pantalla }), Some((pantalla, false)));
    }

    #[test]
    fn la_copia_de_la_ventana_a_la_sombra() {
        let v = Ventana { x0: 10, y0: 20, va: 0x4_0001_0000, fila: 7680, rgb: true };
        let o = copia(&v, true);
        assert_eq!((o[3] as u64) << 32 | o[4] as u64, v.va);
        assert_eq!((o[5] as u64) << 32 | o[6] as u64, VA);
        assert_eq!((o[7], o[8]), (7680, FILA));
        assert_eq!(o[11], cabecera_en(4, SET_DST_BLOCK_SIZE, 5));
        assert_eq!(o[17], cabecera_en(4, DST_ORIGIN_X, 2));
        assert_eq!(o[23], PAGA_A_LA_SOMBRA);
        let l = o[ORDENES - 1];
        assert_eq!((l & ORIGEN_PITCH, l & DESTINO_PITCH), (ORIGEN_PITCH, 0));
    }
}
