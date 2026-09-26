//! **M5 T1a: LA CLASE 3D ESCRIBE PIXELES** -- el primer escalon del triangulo
//! por el rasterizador de hardware: AMPERE_B limpia un destino de render con
//! su propio hardware de pixeles (el ROP), sin programas. Si los 262144
//! pixeles salen del color de limpieza, el camino 3D escribe en memoria.
//!
//! capa: puro -- las ordenes y lo que se espera; la RAM y los registros los
//! toca el kernel (L8)
//!
//! [eje]     CORRECCION -- cada metodo y cada campo, de `clc797.h` de NVIDIA
//!           (open-gpu-doc)
//!
//! # Los escalones
//!
//! ```text
//!    T1a  limpiar el destino con el ROP, sin programas             (aqui)
//!    T1b  los programas de vertice y de pixel, con su cabecera (SPH)
//!    T1c  el triangulo por el rasterizador, con `bmo-dibujo` de juez
//! ```
//!
//! # El destino
//!
//! El MISMO MiB que el fractal: 512 x 512, PITCH, A8R8G8B8 (en memoria
//! `0xAARRGGBB`, el formato del escritorio). En PITCH el "ancho" son los
//! BYTES de una fila: 2048.

use crate::canal::GR;
use crate::copia::{cabecera_en, entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::fractal::{LADO, PIXELES, VA};
use crate::lienzo::sombreador_va;
use crate::sombreador::{EMPUJE, SEMAFOROS};
use crate::Registros;

/// La clase 3D va en el subcanal 0 (el computo, en el 1).
pub const SUBCANAL: u32 = 0;

// Los metodos de `clc797.h`.
pub const SET_OBJECT: u32 = 0x0000;
pub const WAIT_FOR_IDLE: u32 = 0x0110;
pub const SET_COLOR_TARGET_A0: u32 = 0x0800;
pub const SET_COLOR_CLEAR_VALUE0: u32 = 0x0d80;
pub const SET_WINDOW_OFFSET_X: u32 = 0x0df8;
pub const SET_SCISSOR_ENABLE0: u32 = 0x0e00;
pub const SET_SURFACE_CLIP_HORIZONTAL: u32 = 0x0ff4;
pub const SET_CLEAR_SURFACE_CONTROL: u32 = 0x10f8;
/// `SET_CLEAR_RECT_HORIZONTAL` (xmin | xmax << 16) y, detras,
/// `SET_CLEAR_RECT_VERTICAL` (ymin | ymax << 16): el rectangulo que limpia
/// `CLEAR_SURFACE` si `SET_CLEAR_SURFACE_CONTROL` dice [`USAR_RECT`]
/// (`clc797.h`; el maximo, como NVK: x + ancho).
pub const SET_CLEAR_RECT_HORIZONTAL: u32 = 0x0d6c;
/// `SET_CLEAR_SURFACE_CONTROL_USE_CLEAR_RECT` (bit 4).
pub const USAR_RECT: u32 = 1 << 4;
pub const SET_CT_SELECT: u32 = 0x121c;
pub const CLEAR_SURFACE: u32 = 0x19d0;
pub const SET_CT_WRITE0: u32 = 0x1a00;
pub const SET_REPORT_SEMAPHORE_A: u32 = 0x1b00;

/// `SET_COLOR_TARGET_FORMAT_V_A8R8G8B8`.
pub const FORMATO: u32 = 0xCF;
/// `SET_COLOR_TARGET_MEMORY_LAYOUT_PITCH` (bit 12).
pub const MEMORIA_PITCH: u32 = 1 << 12;
/// `SET_CT_WRITE`: R, G, B y A (bits 0, 4, 8, 12).
pub const ESCRIBIR_RGBA: u32 = 0x1111;
/// `CLEAR_SURFACE`: R, G, B y A (bits 2..5), destino 0.
pub const LIMPIAR_RGBA: u32 = 0b1111 << 2;
/// `SET_REPORT_SEMAPHORE_D`: RELEASE, tras TODAS las escrituras (bit 4), en
/// todo el pipeline (15 en 15:12), una palabra (bit 28).
pub const INFORME: u32 = 1 << 28 | 0xF << 12 | 1 << 4;
/// El mismo informe en CUATRO palabras (STRUCTURE_SIZE = FOUR_WORDS, bit
/// 28 a 0, `clc797.h`): la paga en la primera y, en la tercera y la cuarta,
/// el RELOJ de la 3060 en ns (el mismo formato que lee nouveau para sus
/// `PIPE_QUERY_TIMESTAMP`). 16 bytes, alineados a 16.
pub const INFORME_CON_RELOJ: u32 = INFORME & !(1 << 28);

/// El color de limpieza: R = 1, G = 0, B = 1, A = 1 (floats exactos: sin
/// redondeo al pasar a 8 bits). En memoria, `0xFFFF00FF`.
pub const LIMPIEZA: [u32; 4] = [0x3F80_0000, 0, 0x3F80_0000, 0x3F80_0000];
pub const PIXEL_LIMPIO: u32 = 0xFFFF_00FF;

pub const PAGA_FIN: u32 = 0x3060_3D1A;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0xA0;
pub const ORDENES: usize = 39;

/// **Las ordenes** (subcanal 0).
pub fn ordenes() -> [u32; ORDENES] {
    let c = |m, n| cabecera_en(SUBCANAL, m, n);
    let s = sombreador_va(SEMAFORO_FIN);
    let fila = LADO * 4;
    [
        c(SET_OBJECT, 1),
        crate::gr::AMPERE_B,
        // A, B, WIDTH, HEIGHT, FORMAT, MEMORY, THIRD_DIMENSION, ARRAY_PITCH.
        c(SET_COLOR_TARGET_A0, 8),
        (VA >> 32) as u32,
        VA as u32,
        fila,
        LADO,
        FORMATO,
        MEMORIA_PITCH,
        1,
        0,
        // Un destino, el 0.
        c(SET_CT_SELECT, 1),
        1,
        // Recorte de la superficie: x/ancho, y/alto.
        c(SET_SURFACE_CLIP_HORIZONTAL, 2),
        LADO << 16,
        LADO << 16,
        c(SET_WINDOW_OFFSET_X, 2),
        0,
        0,
        c(SET_SCISSOR_ENABLE0, 1),
        0,
        c(SET_CT_WRITE0, 1),
        ESCRIBIR_RGBA,
        c(SET_COLOR_CLEAR_VALUE0, 4),
        LIMPIEZA[0],
        LIMPIEZA[1],
        LIMPIEZA[2],
        LIMPIEZA[3],
        c(SET_CLEAR_SURFACE_CONTROL, 1),
        0,
        c(CLEAR_SURFACE, 1),
        LIMPIAR_RGBA,
        c(WAIT_FOR_IDLE, 1),
        0,
        c(SET_REPORT_SEMAPHORE_A, 4),
        (s >> 32) as u32,
        s as u32,
        PAGA_FIN,
        INFORME,
    ]
}

/// **Preparar** con la entrada `e` del GPFIFO de GR: el semaforo a cero, las
/// ordenes y la entrada, releidos.
pub fn preparar<R: Registros>(r: &mut R, e: u32) -> bool {
    if !crate::blur::entrada_valida(e) {
        return false;
    }
    let o = ordenes();
    let en = entrada(sombreador_va(EMPUJE), ORDENES as u32);
    escribir(r, SEMAFORO_FIN, &[0; 4]) == 4
        && escribir(r, EMPUJE, &o) == ORDENES
        && escribir(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
}

pub fn lanzar<R: Registros>(r: &mut R, ficha: u32, e: u32) -> bool {
    let puesto = crate::blur::entrada_valida(e) && invalidar(r) && escribir(r, GR.userd + GP_PUT, &[crate::blur::siguiente(e)]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

/// `(GP_GET, semaforo)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, SEMAFORO_FIN))
}

/// Cuantos pixeles son del color de limpieza.
pub fn comprobar(salida: &[u32]) -> u32 {
    salida.iter().take(PIXELES).filter(|&&p| p == PIXEL_LIMPIO).count() as u32
}

/// El formato del fractal: pixeles, semaforos y los dos tiempos (aqui, el
/// de la CPU es solo el de comprobar).
pub use crate::fractal::{desempaquetar, empaquetar, sano};

const _: () = assert!(SEMAFORO_FIN != crate::triangulo::SEMAFORO_FIN && SEMAFORO_FIN != crate::triangulo::SEMAFORO_QMD);

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_metodos_son_los_de_clc797() {
        // (0x0800 + j*64) con j = 0, y los que siguen de 4 en 4.
        assert_eq!(SET_COLOR_TARGET_A0 + 0x1C, 0x081C);
        assert_eq!(SET_COLOR_CLEAR_VALUE0, 0x0d80);
        assert_eq!(CLEAR_SURFACE, 0x19d0);
        assert_eq!(INFORME, 0x1000_F010);
    }

    #[test]
    fn las_ordenes() {
        let o = ordenes();
        assert_eq!(o.len(), ORDENES);
        // SET_OBJECT en el subcanal 0: 1 << 29 | 1 << 16.
        assert_eq!((o[0], o[1]), (0x2001_0000, 0xC797));
        assert_eq!(o[2], cabecera_en(0, 0x0800, 8));
        assert_eq!(((o[3] as u64) << 32) | o[4] as u64, VA);
        assert_eq!((o[5], o[6], o[7], o[8]), (2048, 512, 0xCF, 1 << 12));
        assert_eq!(o[31], LIMPIAR_RGBA);
        assert_eq!(o[38], INFORME);
        // 1.0 es 1.0 en 8 bits: 0xFF, sin redondeo.
        assert_eq!(f32::from_bits(LIMPIEZA[0]), 1.0);
    }

    #[test]
    fn se_comprueba() {
        let mut p = [PIXEL_LIMPIO; 16];
        assert_eq!(comprobar(&p), 16);
        p[3] = 0;
        assert_eq!(comprobar(&p), 15);
    }
}
