//! **M5d T0: EL TRIANGULO, POR COMPUTO** -- el primer triangulo de BMO-X en la
//! 3060: 262144 hilos, uno por pixel de 512 x 512, deciden con las TRES
//! FUNCIONES DE ARISTA si su pixel esta dentro, y lo pintan con los tres
//! colores de los vertices mezclados (rojo arriba, verde abajo a la derecha,
//! azul abajo a la izquierda). Es la cuenta que hace el rasterizador de una
//! GPU, hecha a mano en un programa de computo.
//!
//! capa: puro -- el programa y la cuenta de referencia; la RAM y los registros
//! los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- ENTERA: las tres aristas suman SIEMPRE el area
//!           doble (A = 172800), asi que el color sale exacto y la CPU lo
//!           rehace bit a bit
//!
//! # Por que por computo y no por el pipeline 3D (T1)
//!
//! El rasterizador de verdad (AMPERE_B: vertices, rasterizador, pixeles)
//! necesita programas de VERTICE y de PIXEL, y `ptxas` solo compila computo.
//! Sus instrucciones (`IPA`, las de atributos) las conoce `nvdisasm`, asi que
//! T1 se puede hacer codificandolas a mano y comprobandolas contra el, con su
//! cabecera de programa (SPH) y el estado 3D. Es el nivel siguiente; este es
//! el triangulo que ya se puede VER.
//!
//! # El programa (PTX de origen, `ptxas -arch=sm_86`: 16 registros)
//!
//! ```text
//!    v0 = (256, 40)   v1 = (472, 440)   v2 = (40, 440)
//!    w0 = E(v1, v2, p)   w1 = E(v2, v0, p)   w2 = E(v0, v1, p)
//!    fuera si alguno < 0: una cuadricula oscura de 16 px
//!    dentro: (w0*255/A) << 16 | (w1*255/A) << 8 | w2*255/A
//! ```
//!
//! 59 instrucciones; `c[0x0][0x28]` y `ULDC.64` en NOP (sin barreras), el STG
//! sin descriptor; las tres divisiones entre A, exactas (reciproca y dos
//! correcciones, como en el blur). Escribe en el MISMO MiB que el fractal.

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::fractal::{LADO, PIXELES, VA};
use crate::lienzo::sombreador_va;
use crate::sombreador::{ordenes_con, qmd_con, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::vram::a_cero;
use crate::Registros;

pub const V0: (i32, i32) = (256, 40);
pub const V1: (i32, i32) = (472, 440);
pub const V2: (i32, i32) = (40, 440);
/// El area doble.
pub const AREA: u32 = 172_800;
pub const FONDO_A: u32 = 0x000B_0912;
pub const FONDO_B: u32 = 0x0010_141C;
pub const REGISTROS: u32 = 32;
pub const PAGA_QMD: u32 = 0x3060_7C10;
pub const PAGA_FIN: u32 = 0x3060_7CF0;
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x80;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x90;

/// La funcion de arista `E(a, b, p)`: positiva a la izquierda de `a -> b`.
pub const fn arista(a: (i32, i32), b: (i32, i32), x: i32, y: i32) -> i32 {
    (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)
}

/// **Lo que pinta `(x, y)`**: la MISMA cuenta que el programa.
pub const fn pixel(x: u32, y: u32) -> u32 {
    let (xi, yi) = (x as i32, y as i32);
    let w0 = arista(V1, V2, xi, yi);
    let w1 = arista(V2, V0, xi, yi);
    let w2 = arista(V0, V1, xi, yi);
    if (w0 | w1 | w2) < 0 {
        return if (x ^ y) & 16 == 0 { FONDO_A } else { FONDO_B };
    }
    canal(w0) << 16 | canal(w1) << 8 | canal(w2)
}

/// Un canal de color: el peso de su vertice, de 0 a 255.
const fn canal(w: i32) -> u32 {
    (w as u32).wrapping_mul(255) / AREA
}

pub fn comprobar(salida: &[u32]) -> u32 {
    salida.iter().take(PIXELES).enumerate().filter(|&(k, &p)| p == pixel(k as u32 % LADO, k as u32 / LADO)).count() as u32
}

/// **El programa**, como lo leyo `nvdisasm`.
pub const CODIGO: [(u64, u64); 59] = [
    (0x0000_0000_0000_7918, 0x000F_E400_0000_0000), // NOP (era IMAD.MOV.U32 R1, RZ, RZ, c[0x0][0x28])
    (0x0002_A300_0004_7906, 0x000E_2200_0020_9000), // I2F.U32.RP R4, 0x2a300
    (0x0000_0000_000D_7919, 0x000E_6200_0000_2100), // S2R R13, SR_TID.X
    (0x0000_00D8_FF05_7424, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R5, RZ, RZ, 0xd8
    (0x0000_0000_0000_7918, 0x000F_E200_0000_0000), // NOP (era ULDC.64 UR4, c[0x0][0x118])
    (0x0000_0190_FF02_7424, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R2, RZ, RZ, 0x190
    (0x0000_0000_0008_7919, 0x000E_A200_0000_2500), // S2R R8, SR_CTAID.X
    (0xFFFF_FE50_FF09_7424, 0x000F_E400_078E_00FF), // IMAD.MOV.U32 R9, RZ, RZ, -0x1b0
    (0x0000_0004_0004_7308, 0x001E_2200_0000_1000), // MUFU.RCP R4, R4
    (0xFFFF_FFD8_0D00_7810, 0x042F_E200_07FF_E0FF), // IADD3 R0, R13.reuse, -0x28, RZ
    (0xFFFE_7000_0D02_7424, 0x000F_C400_078E_0202), // IMAD R2, R13, R2, -0x19000
    (0xFFFE_8CC0_0805_7424, 0x004F_E200_078E_0205), // IMAD R5, R8, R5, -0x17340
    (0x0FFF_FFFE_0403_7810, 0x001F_C600_07FF_E0FF), // IADD3 R3, R4, 0xffffffe, RZ
    (0x0000_0190_0007_7824, 0x100F_E200_078E_0205), // IMAD R7, R0, 0x190, R5.reuse
    (0x0001_5180_0205_7810, 0x000F_E200_07FF_E105), // IADD3 R5, -R2, 0x15180, R5
    (0x0000_00FF_FF02_7224, 0x000F_E400_078E_00FF), // IMAD.MOV.U32 R2, RZ, RZ, RZ
    (0x0002_E680_0800_7424, 0x000F_E200_078E_0209), // IMAD R0, R8, R9, 0x2e680
    (0x0000_0003_0003_7305, 0x000E_2200_0021_F000), // F2I.FTZ.U32.TRUNC.NTZ R3, R3
    (0x0000_00FF_0704_7824, 0x000F_E400_078E_02FF), // IMAD R4, R7, 0xff, RZ
    (0x0000_00FF_0506_7824, 0x000F_E400_078E_02FF), // IMAD R6, R5, 0xff, RZ
    (0xFFFD_5D00_030B_7824, 0x001F_C800_078E_02FF), // IMAD R11, R3, -0x2a300, RZ
    (0x0000_000B_0303_7227, 0x000F_C800_078E_0002), // IMAD.HI.U32 R3, R3, R11, R2
    (0x0000_00FF_0002_7824, 0x000F_E200_078E_02FF), // IMAD R2, R0, 0xff, RZ
    (0x0000_0000_0500_7212, 0x000F_E200_078E_FE07), // LOP3.LUT R0, R5, R0, R7, 0xfe, !PT
    (0x0000_0004_030B_7227, 0x000F_C800_078E_00FF), // IMAD.HI.U32 R11, R3, R4, RZ
    (0x0000_0002_0309_7227, 0x000F_C800_078E_00FF), // IMAD.HI.U32 R9, R3, R2, RZ
    (0xFFFD_5D00_0902_7824, 0x000F_E400_078E_0202), // IMAD R2, R9, -0x2a300, R2
    (0xFFFD_5D00_0B04_7824, 0x000F_E400_078E_0204), // IMAD R4, R11, -0x2a300, R4
    (0x0000_0006_0303_7227, 0x000F_E200_078E_00FF), // IMAD.HI.U32 R3, R3, R6, RZ
    (0x0002_A300_0200_780C, 0x000F_E400_03F8_6070), // ISETP.GE.U32.AND P4, PT, R2, 0x2a300, PT
    (0x0002_A300_0400_780C, 0x000F_E200_03FA_6070), // ISETP.GE.U32.AND P5, PT, R4, 0x2a300, PT
    (0xFFFD_5D00_0306_7824, 0x000F_CA00_078E_0206), // IMAD R6, R3, -0x2a300, R6
    (0x0002_A300_0600_780C, 0x000F_CA00_03F6_6070), // ISETP.GE.U32.AND P3, PT, R6, 0x2a300, PT
    (0xFFFD_5D00_0202_4810, 0x000F_E400_07FF_E0FF), // @P4 IADD3 R2, R2, -0x2a300, RZ
    (0xFFFD_5D00_0404_5810, 0x000F_E400_07FF_E0FF), // @P5 IADD3 R4, R4, -0x2a300, RZ
    (0x0002_A300_0200_780C, 0x000F_E200_03F4_6070), // ISETP.GE.U32.AND P2, PT, R2, 0x2a300, PT
    (0x0000_0200_0802_7824, 0x000F_E200_078E_00FF), // IMAD.SHL.U32 R2, R8, 0x200, RZ
    (0x0002_A300_0400_780C, 0x000F_E400_03F2_6070), // ISETP.GE.U32.AND P1, PT, R4, 0x2a300, PT
    (0xFFFD_5D00_0606_3810, 0x000F_E400_07FF_E0FF), // @P3 IADD3 R6, R6, -0x2a300, RZ
    (0x0000_0001_0909_4810, 0x000F_C400_07FF_E0FF), // @P4 IADD3 R9, R9, 0x1, RZ
    (0x0002_A300_0600_780C, 0x000F_E400_03F0_6070), // ISETP.GE.U32.AND P0, PT, R6, 0x2a300, PT
    (0x0000_0001_0B0B_5810, 0x000F_E400_07FF_E0FF), // @P5 IADD3 R11, R11, 0x1, RZ
    (0x0000_0001_0303_3810, 0x000F_E400_07FF_E0FF), // @P3 IADD3 R3, R3, 0x1, RZ
    (0x0000_0001_0909_2810, 0x000F_E400_07FF_E0FF), // @P2 IADD3 R9, R9, 0x1, RZ
    (0x0000_0001_0B0B_1810, 0x000F_E400_07FF_E0FF), // @P1 IADD3 R11, R11, 0x1, RZ
    (0x0000_00FF_0000_720C, 0x000F_E200_03F8_6270), // ISETP.GE.AND P4, PT, R0, RZ, PT
    (0x0001_0000_0900_7824, 0x000F_E200_078E_00FF), // IMAD.U32 R0, R9, 0x10000, RZ
    (0x0000_000D_0202_7212, 0x000F_E200_078E_FCFF), // LOP3.LUT R2, R2, R13, RZ, 0xfc, !PT
    (0x0000_0100_0B0B_7824, 0x000F_E200_078E_00FF), // IMAD.SHL.U32 R11, R11, 0x100, RZ
    (0x0000_0001_0303_0810, 0x000F_C400_07FF_E0FF), // @P0 IADD3 R3, R3, 0x1, RZ
    (0x0000_0010_0DFF_7812, 0x000F_E400_0780_4808), // LOP3.LUT P0, RZ, R13, 0x10, R8, 0x48, !PT
    (0x0000_0000_030B_7212, 0x000F_E200_078E_FE0B), // LOP3.LUT R11, R3, R0, R11, 0xfe, !PT
    (0x000B_0912_FF00_7424, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R0, RZ, RZ, 0xb0912
    (0x0003_0000_0202_7811, 0x000F_E200_078E_10FF), // LEA R2, R2, 0x30000, 0x2
    (0x0000_0002_FF03_7424, 0x000F_C600_078E_00FF), // IMAD.MOV.U32 R3, RZ, RZ, 0x2
    (0x0010_141C_000B_C807, 0x000F_CA00_0400_0000), // @!P4 SEL R11, R0, 0x10141c, !P0
    (0x0000_000B_0200_7986, 0x000F_E200_0C10_1904), // STG.E [R2.64], R11
    (0x0000_0000_0000_794D, 0x000F_EA00_0380_0000), // EXIT
    (0xFFFF_FFF0_0000_7947, 0x000F_C000_0383_FFFF), // BRA 0x3a0
];
pub const PALABRAS_CODIGO: usize = CODIGO.len() * 4;

pub fn codigo() -> [u32; PALABRAS_CODIGO] {
    let mut w = [0u32; PALABRAS_CODIGO];
    for (k, &(lo, hi)) in CODIGO.iter().enumerate() {
        w[4 * k] = lo as u32;
        w[4 * k + 1] = (lo >> 32) as u32;
        w[4 * k + 2] = hi as u32;
        w[4 * k + 3] = (hi >> 32) as u32;
    }
    w
}

pub fn qmd() -> [u32; QMD_PALABRAS] {
    qmd_con(sombreador_va(PROGRAMA), LADO, LADO, sombreador_va(SEMAFORO_QMD), PAGA_QMD, REGISTROS)
}

pub fn ordenes() -> [u32; ORDENES] {
    ordenes_con(sombreador_va(QMD), sombreador_va(SEMAFORO_FIN), PAGA_FIN)
}

pub fn preparar<R: Registros>(r: &mut R, e: u32) -> bool {
    if !crate::blur::entrada_valida(e) {
        return false;
    }
    let c = codigo();
    let q = qmd();
    let o = ordenes();
    let en = entrada(sombreador_va(EMPUJE), ORDENES as u32);
    let pag = crate::vram::PALABRAS;
    escribir(r, SEMAFORO_QMD, &[0; 8]) == 8
        && a_cero(r, PROGRAMA) as usize == pag
        && a_cero(r, QMD) as usize == pag
        && escribir(r, PROGRAMA, &c) == PALABRAS_CODIGO
        && escribir(r, QMD, &q) == QMD_PALABRAS
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

pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, SEMAFORO_QMD), leer32(r, SEMAFORO_FIN))
}

/// Como el fractal: pixeles, semaforos y los dos tiempos.
pub use crate::fractal::{desempaquetar, empaquetar, sano};

const _: () = assert!(SEMAFORO_QMD != crate::fractal::SEMAFORO_QMD && SEMAFORO_FIN != crate::fractal::SEMAFORO_FIN);
const _: () = assert!(PALABRAS_CODIGO * 4 <= 4096);
const _: () = assert!(VA == crate::fractal::VA);

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_aristas_suman_el_area() {
        for (x, y) in [(0, 0), (256, 240), (511, 511), (100, 400)] {
            let s = arista(V1, V2, x, y) + arista(V2, V0, x, y) + arista(V0, V1, x, y);
            assert_eq!(s, AREA as i32);
        }
        // Los vertices: todo su color.
        assert_eq!(pixel(256, 40), 0x00FF_0000);
        assert_eq!(pixel(472, 440), 0x0000_FF00);
        assert_eq!(pixel(40, 440), 0x0000_00FF);
        // Fuera, la cuadricula.
        assert_eq!(pixel(0, 0), FONDO_A);
        assert_eq!(pixel(16, 0), FONDO_B);
        assert_eq!(pixel(500, 0), FONDO_B); // 500 & 16 = 16
    }

    #[test]
    fn el_programa_lleva_las_constantes() {
        // 0x2a300 = A; 0xd8 = 216; 0x190 = 400; el fondo; la VA del MiB.
        for c in [0x2A300u64, 0xD8, 0x190, 0xB0912, 0x10141C] {
            assert!(CODIGO.iter().any(|&(lo, _)| lo >> 32 == c || (lo >> 32) as u32 as i32 == -(c as i32)), "{c:#x}");
        }
        assert_eq!(AREA, 0x2A300);
        assert_eq!(CODIGO[0].0, 0x7918);
        assert_eq!(CODIGO[4].0, 0x7918);
        assert_eq!(CODIGO[57].0, 0x794D);
    }
}
