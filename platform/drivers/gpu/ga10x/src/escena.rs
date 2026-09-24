//! **M5d E: LA ESCENA 3D CON LUZ** -- dibujada por la 3060: una esfera verde
//! con luz difusa y brillo especular, un suelo en perspectiva con cuadros y
//! la sombra de la esfera, y un cielo degradado. 262144 hilos, uno por pixel
//! de 512 x 512: cada uno lanza su rayo, mira si toca la esfera, saca la
//! normal, calcula la luz -- o, si no, el suelo, sus cuadros y la sombra. La
//! CPU NO dibuja: solo rehace la misma cuenta ENTERA para comprobarla.
//!
//! capa: puro -- el programa y la cuenta de referencia (L8)
//!
//! [eje]     CORRECCION -- todo entero: la raiz (digito a digito), las
//!           divisiones (truncando, como `div.s32`) y los desplazamientos
//!           aritmeticos dan lo mismo en la 3060 y en la CPU, bit a bit
//!
//! # La escena (PTX de origen, `ptxas -arch=sm_86`: 15 registros)
//!
//! ```text
//!    esfera   centro (256, 230), radio 150; z = raiz(r^2 - d^2)
//!    luz      L = (-116, -140, 181) (arriba a la izquierda, delante; |L|~256)
//!    difusa   max(0, N.L)            ambiente 40, difusa hasta 216 (/256)
//!    brillo   H = (-63, -76, 236); (N.H)^16 en punto fijo, x 230/256
//!    color    verde NVIDIA (118, 185, 0) por la luz, + el brillo blanco
//!    cielo    y < 300: (10, 12 + y/16, 24 + y/8)
//!    suelo    u = (x-256)*64/(y-290), v = 8192/(y-290): cuadros de
//!             (u>>5) ^ (v>>3); la sombra, una elipse que oscurece a la mitad
//! ```
//!
//! 158 instrucciones, con los dos NOP de siempre (ninguno pone barrera) y el
//! STG sin descriptor. En el MISMO MiB que el fractal.

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::fractal::{LADO, PIXELES};
use crate::lienzo::sombreador_va;
use crate::sombreador::{ordenes_con, qmd_con, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::vram::a_cero;
use crate::Registros;

pub const REGISTROS: u32 = 32;
pub const PAGA_QMD: u32 = 0x3060_E5C0;
pub const PAGA_FIN: u32 = 0x3060_E5F0;
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0xB0;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0xC0;

/// La raiz entera, digito a digito: la MISMA que el programa.
pub const fn raiz(mut n: i32) -> i32 {
    let (mut r, mut bit) = (0i32, 1i32 << 14);
    while bit != 0 {
        let t = r + bit;
        let mayor = n >= t;
        r >>= 1;
        if mayor {
            n -= t;
            r += bit;
        }
        bit >>= 2;
    }
    r
}

/// **Lo que pinta `(x, y)`**: la MISMA cuenta que el programa.
pub const fn pixel(x: u32, y: u32) -> u32 {
    let (xi, yi) = (x as i32, y as i32);
    let (dx, dy) = (xi - 256, yi - 230);
    let d2 = dx * dx + dy * dy;
    if d2 < 22500 {
        let z = raiz(22500 - d2);
        let mut dif = (dx * -116 + dy * -140 + z * 181) / 150;
        if dif < 0 {
            dif = 0;
        }
        let mut s = (dx * -63 + dy * -76 + z * 236) / 150;
        if s < 0 {
            s = 0;
        }
        if s > 256 {
            s = 256;
        }
        s = (s * s) >> 8;
        s = (s * s) >> 8;
        s = (s * s) >> 8;
        s = (s * s) >> 8;
        let brillo = (s * 230) >> 8;
        let luz = ((dif * 216) >> 8) + 40;
        let r = min255(((luz * 118) >> 8) + brillo);
        let g = min255(((luz * 185) >> 8) + brillo);
        let b = min255(brillo);
        return (r as u32) << 16 | (g as u32) << 8 | b as u32;
    }
    if yi < 300 {
        return 10 << 16 | (((yi >> 4) + 12) as u32) << 8 | ((yi >> 3) + 24) as u32;
    }
    let t = yi - 290;
    let u = (dx << 6) / t;
    let v = 8192 / t;
    let claro = ((u >> 5) ^ (v >> 3)) & 1 != 0;
    let (mut r, mut g, mut b) = if claro { (58, 63, 85) } else { (28, 31, 43) };
    let (sx, sy) = (xi - 330, yi - 400);
    if sx * sx + ((sy * sy) << 4) < 25600 {
        r >>= 1;
        g >>= 1;
        b >>= 1;
    }
    (r as u32) << 16 | (g as u32) << 8 | b as u32
}

const fn min255(v: i32) -> i32 {
    if v > 255 {
        255
    } else {
        v
    }
}

pub fn comprobar(salida: &[u32]) -> u32 {
    salida.iter().take(PIXELES).enumerate().filter(|&(k, &p)| p == pixel(k as u32 % LADO, k as u32 / LADO)).count() as u32
}

/// **El programa**, como lo leyo `nvdisasm`.
pub const CODIGO: [(u64, u64); 158] = [
    (0x0000_0000_0000_7918, 0x000F_E400_0000_0000), // NOP (era IMAD.MOV.U32 R1, RZ, RZ, c[0x0][0x28])
    (0x0000_0000_0000_7919, 0x000E_2200_0000_2100), // S2R R0, SR_TID.X
    (0x0000_0000_0000_7918, 0x000F_E200_0000_0000), // NOP (era ULDC.64 UR6, c[0x0][0x118])
    (0x0000_0930_0000_7945, 0x000F_E400_0380_0000), // BSSY B0, 0x970
    (0x0000_0000_0002_7919, 0x000E_6200_0000_2500), // S2R R2, SR_CTAID.X
    (0xFFFF_FF00_0003_7810, 0x001F_E400_07FF_E0FF), // IADD3 R3, R0, -0x100, RZ
    (0xFFFF_FF1A_0206_7810, 0x002F_C600_07FF_E0FF), // IADD3 R6, R2, -0xe6, RZ
    (0x0000_0003_0305_7224, 0x000F_C800_078E_02FF), // IMAD R5, R3, R3, RZ
    (0x0000_0006_0605_7224, 0x000F_CA00_078E_0205), // IMAD R5, R6, R6, R5
    (0x0000_57E4_0500_780C, 0x000F_DA00_03F0_6270), // ISETP.GE.AND P0, PT, R5, 0x57e4, PT
    (0x0000_04A0_0000_0947, 0x000F_EA00_0380_0000), // @P0 BRA 0x550
    (0x0000_57E4_0505_7810, 0x000F_E200_07FF_E1FF), // IADD3 R5, -R5, 0x57e4, RZ
    (0x0000_00FF_FF03_7224, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R3, RZ, RZ, RZ
    (0x0000_4000_0004_7882, 0x000F_C800_0000_0000), // UMOV UR4, 0x4000
    (0x0000_0004_0304_7C10, 0x000F_E400_0FFF_E0FF), // IADD3 R4, R3, UR4, RZ
    (0x0000_0001_FF03_7819, 0x000F_E400_0001_1403), // SHF.R.S32.HI R3, RZ, 0x1, R3
    (0x0000_0004_0500_720C, 0x000F_DA00_03F0_6270), // ISETP.GE.AND P0, PT, R5, R4, PT
    (0x0000_0004_0303_0C10, 0x000F_E200_0FFF_E0FF), // @P0 IADD3 R3, R3, UR4, RZ
    (0x0000_0002_3F04_7899, 0x000F_E200_0801_1404), // USHF.R.S32.HI UR4, URZ, 0x2, UR4
    (0x0000_0001_0505_0824, 0x000F_CA00_078E_0A04), // @P0 IMAD.IADD R5, R5, 0x1, -R4
    (0x0000_0004_FF00_7C0C, 0x000F_DA00_0BF2_5270), // ISETP.NE.AND P1, PT, RZ, UR4, PT
    (0xFFFF_FF80_0000_1947, 0x000F_EA00_0383_FFFF), // @P1 BRA 0xe0
    (0x0000_0096_0007_7906, 0x000E_2200_0020_9000), // I2F.U32.RP R7, 0x96
    (0xFFFF_FFC1_FF09_7424, 0x000F_C800_078E_00FF), // IMAD.MOV.U32 R9, RZ, RZ, -0x3f
    (0x0000_3F00_0009_7424, 0x000F_C800_078E_0209), // IMAD R9, R0, R9, 0x3f00
    (0xFFFF_FFB4_0608_7824, 0x000F_C800_078E_0209), // IMAD R8, R6, -0x4c, R9
    (0x0000_00EC_0309_7824, 0x000F_E200_078E_0208), // IMAD R9, R3, 0xec, R8
    (0x0000_0007_0007_7308, 0x001E_2800_0000_1000), // MUFU.RCP R7, R7
    (0x0000_0009_000A_7213, 0x000F_E400_0000_0000), // IABS R10, R9
    (0x0000_0096_0909_7812, 0x000F_C800_078E_3CFF), // LOP3.LUT R9, R9, 0x96, RZ, 0x3c, !PT
    (0x0000_00FF_0900_720C, 0x000F_E400_03F4_6270), // ISETP.GE.AND P2, PT, R9, RZ, PT
    (0x0FFF_FFFE_0704_7810, 0x001F_C800_07FF_E0FF), // IADD3 R4, R7, 0xffffffe, RZ
    (0x0000_0004_0005_7305, 0x0000_6400_0021_F000), // F2I.FTZ.U32.TRUNC.NTZ R5, R4
    (0x0000_00FF_FF04_7224, 0x001F_E400_078E_00FF), // IMAD.MOV.U32 R4, RZ, RZ, RZ
    (0xFFFF_FF6A_050B_7824, 0x002F_C800_078E_02FF), // IMAD R11, R5, -0x96, RZ
    (0x0000_000B_0508_7227, 0x000F_C800_078E_0004), // IMAD.HI.U32 R8, R5, R11, R4
    (0xFFFF_FF8C_FF05_7424, 0x000F_E400_078E_00FF), // IMAD.MOV.U32 R5, RZ, RZ, -0x74
    (0x0000_000A_0807_7227, 0x000F_C800_078E_00FF), // IMAD.HI.U32 R7, R8, R10, RZ
    (0xFFFF_FF6A_070A_7824, 0x000F_E400_078E_020A), // IMAD R10, R7, -0x96, R10
    (0x0000_7400_0005_7424, 0x000F_C600_078E_0205), // IMAD R5, R0, R5, 0x7400
    (0x0000_0096_0A00_780C, 0x000F_E200_03F0_6070), // ISETP.GE.U32.AND P0, PT, R10, 0x96, PT
    (0xFFFF_FF74_0606_7824, 0x000F_C800_078E_0205), // IMAD R6, R6, -0x8c, R5
    (0x0000_00B5_0306_7824, 0x000F_CA00_078E_0206), // IMAD R6, R3, 0xb5, R6
    (0x0000_0006_0003_7213, 0x000F_E400_0000_0000), // IABS R3, R6
    (0x0000_0096_0606_7812, 0x000F_E400_078E_3CFF), // LOP3.LUT R6, R6, 0x96, RZ, 0x3c, !PT
    (0xFFFF_FF6A_0A0A_0810, 0x000F_E200_07FF_E0FF), // @P0 IADD3 R10, R10, -0x96, RZ
    (0x0000_0003_0808_7227, 0x000F_E200_078E_00FF), // IMAD.HI.U32 R8, R8, R3, RZ
    (0x0000_0001_0707_0810, 0x000F_E400_07FF_E0FF), // @P0 IADD3 R7, R7, 0x1, RZ
    (0x0000_0096_0A00_780C, 0x000F_E200_03F2_6070), // ISETP.GE.U32.AND P1, PT, R10, 0x96, PT
    (0xFFFF_FF6A_0803_7824, 0x000F_CA00_078E_0203), // IMAD R3, R8, -0x96, R3
    (0x0000_0096_0300_780C, 0x000F_CE00_03F0_6070), // ISETP.GE.U32.AND P0, PT, R3, 0x96, PT
    (0x0000_0001_0707_1810, 0x000F_CA00_07FF_E0FF), // @P1 IADD3 R7, R7, 0x1, RZ
    (0x0000_00FF_FF07_A224, 0x000F_E200_078E_0A07), // @!P2 IMAD.MOV R7, RZ, RZ, -R7
    (0xFFFF_FF6A_0303_0810, 0x000F_E400_07FF_E0FF), // @P0 IADD3 R3, R3, -0x96, RZ
    (0x0000_0001_0808_0810, 0x000F_E400_07FF_E0FF), // @P0 IADD3 R8, R8, 0x1, RZ
    (0x0000_0007_FF07_7217, 0x000F_E400_0780_0200), // IMNMX R7, RZ, R7, !PT
    (0x0000_0096_0300_780C, 0x000F_E400_03F2_6070), // ISETP.GE.U32.AND P1, PT, R3, 0x96, PT
    (0x0000_0100_0707_7817, 0x000F_E400_0380_0200), // IMNMX R7, R7, 0x100, PT
    (0x0000_00FF_0600_720C, 0x000F_C600_03F0_6270), // ISETP.GE.AND P0, PT, R6, RZ, PT
    (0x0000_0007_0707_7224, 0x000F_CA00_078E_02FF), // IMAD R7, R7, R7, RZ
    (0x0000_0008_FF07_7819, 0x000F_E400_0001_1407), // SHF.R.S32.HI R7, RZ, 0x8, R7
    (0x0000_0001_0808_1810, 0x000F_C600_07FF_E0FF), // @P1 IADD3 R8, R8, 0x1, RZ
    (0x0000_0007_0707_7224, 0x000F_E400_078E_02FF), // IMAD R7, R7, R7, RZ
    (0x0000_00FF_FF08_8224, 0x000F_C600_078E_0A08), // @!P0 IMAD.MOV R8, RZ, RZ, -R8
    (0x0000_0008_FF07_7819, 0x000F_E400_0001_1407), // SHF.R.S32.HI R7, RZ, 0x8, R7
    (0x0000_0008_FF03_7217, 0x000F_C600_0780_0200), // IMNMX R3, RZ, R8, !PT
    (0x0000_0007_0707_7224, 0x000F_E400_078E_02FF), // IMAD R7, R7, R7, RZ
    (0x0000_00D8_0303_7824, 0x000F_C600_078E_02FF), // IMAD R3, R3, 0xd8, RZ
    (0x0000_0008_FF07_7819, 0x000F_E400_0001_1407), // SHF.R.S32.HI R7, RZ, 0x8, R7
    (0x0000_0028_0303_7811, 0x000F_C600_078F_C2FF), // LEA.HI.SX32 R3, R3, 0x28, 0x18
    (0x0000_0007_0707_7224, 0x000F_E400_078E_02FF), // IMAD R7, R7, R7, RZ
    (0x0000_0076_0304_7824, 0x040F_E400_078E_02FF), // IMAD R4, R3.reuse, 0x76, RZ
    (0x0000_00B9_0306_7824, 0x000F_E200_078E_02FF), // IMAD R6, R3, 0xb9, RZ
    (0x0000_0008_FF07_7819, 0x000F_CA00_0001_1407), // SHF.R.S32.HI R7, RZ, 0x8, R7
    (0x0000_00E6_0707_7824, 0x000F_CA00_078E_02FF), // IMAD R7, R7, 0xe6, RZ
    (0x0000_0008_FF07_7819, 0x000F_C800_0001_1407), // SHF.R.S32.HI R7, RZ, 0x8, R7
    (0x0000_0007_0404_7211, 0x080F_E400_078F_C2FF), // LEA.HI.SX32 R4, R4, R7.reuse, 0x18
    (0x0000_0007_0606_7211, 0x000F_E400_078F_C2FF), // LEA.HI.SX32 R6, R6, R7, 0x18
    (0x0000_00FF_0404_7817, 0x000F_E400_0380_0200), // IMNMX R4, R4, 0xff, PT
    (0x0000_00FF_0606_7817, 0x000F_E400_0380_0200), // IMNMX R6, R6, 0xff, PT
    (0x0000_00FF_0707_7817, 0x000F_E200_0380_0200), // IMNMX R7, R7, 0xff, PT
    (0x0001_0000_0404_7824, 0x000F_E400_078E_00FF), // IMAD.U32 R4, R4, 0x10000, RZ
    (0x0000_0100_0606_7824, 0x000F_CA00_078E_00FF), // IMAD.SHL.U32 R6, R6, 0x100, RZ
    (0x0000_0004_0707_7212, 0x000F_E200_078E_FE06), // LOP3.LUT R7, R7, R4, R6, 0xfe, !PT
    (0x0000_0410_0000_7947, 0x000F_EA00_0380_0000), // BRA 0x960
    (0x0000_012C_0200_780C, 0x000F_DA00_03F0_6270), // ISETP.GE.AND P0, PT, R2, 0x12c, PT
    (0x0000_000C_0204_8811, 0x040F_E400_078F_E2FF), // @!P0 LEA.HI.SX32 R4, R2.reuse, 0xc, 0x1c
    (0x0000_0018_0207_8811, 0x000F_C600_078F_EAFF), // @!P0 LEA.HI.SX32 R7, R2, 0x18, 0x1d
    (0x0000_0100_0404_8824, 0x000F_CA00_078E_00FF), // @!P0 IMAD.SHL.U32 R4, R4, 0x100, RZ
    (0x000A_0000_0407_8812, 0x000F_E200_078E_FE07), // @!P0 LOP3.LUT R7, R4, 0xa0000, R7, 0xfe, !PT
    (0x0000_03B0_0000_8947, 0x000F_EA00_0380_0000), // @!P0 BRA 0x960
    (0xFFFF_FEDE_0206_7810, 0x000F_E200_07FF_E0FF), // IADD3 R6, R2, -0x122, RZ
    (0x0000_0040_0303_7824, 0x000F_C600_078E_00FF), // IMAD.SHL.U32 R3, R3, 0x40, RZ
    (0x0000_0006_0009_7213, 0x080F_E400_0000_0000), // IABS R9, R6.reuse
    (0x0000_0003_000A_7213, 0x000F_E400_0000_0000), // IABS R10, R3
    (0x0000_0009_0007_7306, 0x000E_2200_0020_9400), // I2F.RP R7, R9
    (0x0000_0006_000C_7213, 0x080F_E400_0000_0000), // IABS R12, R6.reuse
    (0x0000_0006_0303_7212, 0x000F_C800_078E_3CFF), // LOP3.LUT R3, R3, R6, RZ, 0x3c, !PT
    (0x0000_00FF_0300_720C, 0x000F_E200_03F0_6270), // ISETP.GE.AND P0, PT, R3, RZ, PT
    (0x0000_0007_0007_7308, 0x001E_2400_0000_1000), // MUFU.RCP R7, R7
    (0x0FFF_FFFE_0704_7810, 0x001F_CC00_07FF_E0FF), // IADD3 R4, R7, 0xffffffe, RZ
    (0x0000_0004_0005_7305, 0x0000_6400_0021_F000), // F2I.FTZ.U32.TRUNC.NTZ R5, R4
    (0x0000_00FF_FF04_7224, 0x001F_E400_078E_00FF), // IMAD.MOV.U32 R4, RZ, RZ, RZ
    (0x0000_00FF_FF08_7224, 0x002F_C800_078E_0A05), // IMAD.MOV R8, RZ, RZ, -R5
    (0x0000_0009_080B_7224, 0x000F_E400_078E_02FF), // IMAD R11, R8, R9, RZ
    (0x0000_00FF_FF08_7224, 0x000F_E400_078E_000A), // IMAD.MOV.U32 R8, RZ, RZ, R10
    (0x0000_000B_0505_7227, 0x000F_C800_078E_0004), // IMAD.HI.U32 R5, R5, R11, R4
    (0x0000_00FF_FF0B_7224, 0x000F_E400_078E_0A0C), // IMAD.MOV R11, RZ, RZ, -R12
    (0x0000_0008_0504_7227, 0x000F_C800_078E_00FF), // IMAD.HI.U32 R4, R5, R8, RZ
    (0x0000_2000_0507_7827, 0x000F_C800_078E_00FF), // IMAD.HI.U32 R7, R5, 0x2000, RZ
    (0x0000_000B_0405_7224, 0x080F_E400_078E_0208), // IMAD R5, R4, R11.reuse, R8
    (0x0000_2000_0708_7424, 0x000F_C600_078E_020B), // IMAD R8, R7, R11, 0x2000
    (0x0000_0005_0900_720C, 0x040F_E400_03F8_4070), // ISETP.GT.U32.AND P4, PT, R9.reuse, R5, PT
    (0x0000_0008_0900_720C, 0x000F_D600_03FA_4070), // ISETP.GT.U32.AND P5, PT, R9, R8, PT
    (0x0000_0001_0505_C824, 0x100F_E200_078E_0A09), // @!P4 IMAD.IADD R5, R5, 0x1, -R9.reuse
    (0x0000_0001_0404_C810, 0x000F_E200_07FF_E0FF), // @!P4 IADD3 R4, R4, 0x1, RZ
    (0x0000_0001_0808_D824, 0x000F_E200_078E_0A09), // @!P5 IMAD.IADD R8, R8, 0x1, -R9
    (0x0000_0001_0707_D810, 0x000F_E400_07FF_E0FF), // @!P5 IADD3 R7, R7, 0x1, RZ
    (0x0000_0009_0500_720C, 0x080F_E400_03F4_6070), // ISETP.GE.U32.AND P2, PT, R5, R9.reuse, PT
    (0x0000_0009_0800_720C, 0x000F_E400_03F6_6070), // ISETP.GE.U32.AND P3, PT, R8, R9, PT
    (0x0000_2000_0608_7812, 0x000F_E400_078E_3CFF), // LOP3.LUT R8, R6, 0x2000, RZ, 0x3c, !PT
    (0xFFFF_FE70_0205_7810, 0x000F_C400_07FF_E0FF), // IADD3 R5, R2, -0x190, RZ
    (0x0000_00FF_0800_720C, 0x000F_C600_03F2_6270), // ISETP.GE.AND P1, PT, R8, RZ, PT
    (0x0000_0005_0503_7224, 0x000F_E200_078E_02FF), // IMAD R3, R5, R5, RZ
    (0xFFFF_FEB6_0005_7810, 0x000F_E400_07FF_E0FF), // IADD3 R5, R0, -0x14a, RZ
    (0x0000_0001_0404_2810, 0x000F_E200_07FF_E0FF), // @P2 IADD3 R4, R4, 0x1, RZ
    (0x0000_0010_0308_7824, 0x000F_E200_078E_00FF), // IMAD.SHL.U32 R8, R3, 0x10, RZ
    (0x0000_0001_0707_3810, 0x000F_E400_07FF_E0FF), // @P3 IADD3 R7, R7, 0x1, RZ
    (0x0000_00FF_0600_720C, 0x000F_E200_03F4_5270), // ISETP.NE.AND P2, PT, R6, RZ, PT
    (0x0000_00FF_FF04_8224, 0x000F_E200_078E_0A04), // @!P0 IMAD.MOV R4, RZ, RZ, -R4
    (0x0000_0006_FF03_7212, 0x000F_E200_078E_33FF), // LOP3.LUT R3, RZ, R6, RZ, 0x33, !PT
    (0x0000_00FF_FF07_9224, 0x000F_C400_078E_0A07), // @!P1 IMAD.MOV R7, RZ, RZ, -R7
    (0x0000_0005_0508_7224, 0x000F_E200_078E_0208), // IMAD R8, R5, R5, R8
    (0x0000_0004_0304_7207, 0x040F_E200_0500_0000), // SEL R4, R3.reuse, R4, !P2
    (0x0000_003A_FF05_7424, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R5, RZ, RZ, 0x3a
    (0x0000_0007_0303_7207, 0x000F_E200_0500_0000), // SEL R3, R3, R7, !P2
    (0x0000_0055_FF07_7424, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R7, RZ, RZ, 0x55
    (0x0000_0005_FF04_7819, 0x000F_E400_0001_1404), // SHF.R.S32.HI R4, RZ, 0x5, R4
    (0x0000_0003_FF03_7819, 0x000F_E400_0001_1403), // SHF.R.S32.HI R3, RZ, 0x3, R3
    (0x0000_6400_0800_780C, 0x000F_E400_03F2_6270), // ISETP.GE.AND P1, PT, R8, 0x6400, PT
    (0x0000_0001_04FF_7812, 0x000F_E200_0780_4803), // LOP3.LUT P0, RZ, R4, 0x1, R3, 0x48, !PT
    (0x0000_003F_FF04_7424, 0x000F_C600_078E_00FF), // IMAD.MOV.U32 R4, RZ, RZ, 0x3f
    (0x0000_001C_0503_7807, 0x000F_E400_0000_0000), // SEL R3, R5, 0x1c, P0
    (0x0000_001F_0404_7807, 0x000F_E400_0000_0000), // SEL R4, R4, 0x1f, P0
    (0x0000_002B_0707_7807, 0x000F_C600_0000_0000), // SEL R7, R7, 0x2b, P0
    (0x0000_0001_FF03_9819, 0x000F_E400_0001_1403), // @!P1 SHF.R.S32.HI R3, RZ, 0x1, R3
    (0x0000_0001_FF04_9819, 0x000F_E400_0001_1404), // @!P1 SHF.R.S32.HI R4, RZ, 0x1, R4
    (0x0000_0001_FF07_9819, 0x000F_C600_0001_1407), // @!P1 SHF.R.S32.HI R7, RZ, 0x1, R7
    (0x0000_0100_0304_7824, 0x000F_C800_078E_0204), // IMAD R4, R3, 0x100, R4
    (0x0000_0100_0407_7824, 0x000F_E400_078E_0007), // IMAD.U32 R7, R4, 0x100, R7
    (0x0000_0000_0000_7941, 0x000F_EA00_0380_0000), // BSYNC B0
    (0x0000_0200_0203_7824, 0x000F_CA00_078E_00FF), // IMAD.SHL.U32 R3, R2, 0x200, RZ
    (0x0000_0000_0303_7212, 0x000F_C800_078E_FCFF), // LOP3.LUT R3, R3, R0, RZ, 0xfc, !PT
    (0x0003_0000_0302_7811, 0x000F_E200_078E_10FF), // LEA R2, R3, 0x30000, 0x2
    (0x0000_0002_FF03_7424, 0x000F_CA00_078E_00FF), // IMAD.MOV.U32 R3, RZ, RZ, 0x2
    (0x0000_0007_0200_7986, 0x000F_E200_0C10_1906), // STG.E [R2.64], R7
    (0x0000_0000_0000_794D, 0x000F_EA00_0380_0000), // EXIT
    (0xFFFF_FFF0_0000_7947, 0x000F_C000_0383_FFFF), // BRA 0x9d0
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

pub use crate::fractal::{desempaquetar, empaquetar, sano};

const _: () = assert!(PALABRAS_CODIGO * 4 <= 4096);
const _: () = assert!(SEMAFORO_QMD != crate::tresde::SEMAFORO_FIN && SEMAFORO_FIN != crate::tresde::SEMAFORO_FIN);
const _: () = assert!(SEMAFORO_QMD > crate::triangulo::SEMAFORO_FIN);

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_raiz_es_exacta() {
        for n in 0..22_501 {
            let r = raiz(n);
            assert!(r * r <= n && (r + 1) * (r + 1) > n, "{n}");
        }
    }

    #[test]
    fn la_escena() {
        // El centro de la esfera, de frente: difusa (181*150/150 = 181) y
        // algo de brillo.
        let c = pixel(256, 230);
        assert!(c >> 8 & 0xFF > 150 && c >> 16 & 0xFF > 90, "{c:#x}");
        // El brillo: hacia la luz, arriba a la izquierda, casi blanco.
        let b = pixel(256 - 35, 230 - 42);
        assert!(b & 0xFF > 150, "{b:#x}");
        // El borde de abajo a la derecha, en penumbra: solo el ambiente.
        let p = pixel(256 + 100, 230 + 100);
        assert_eq!(p & 0xFF, 0);
        // El cielo, arriba a la izquierda.
        assert_eq!(pixel(0, 0), 10 << 16 | 12 << 8 | 24);
        // El suelo tiene los dos colores, y la sombra oscurece.
        let dentro = pixel(330, 400);
        let fuera = pixel(20, 500);
        assert!(dentro & 0xFF <= 43, "{dentro:#x}");
        assert!(fuera & 0xFF == 85 || fuera & 0xFF == 43);
    }

    #[test]
    fn el_programa_lleva_la_escena() {
        // Algunas constantes que tienen que estar dentro: el radio al
        // cuadrado, la luz y el suelo.
        for c in [22500u64, 0x3_0000] {
            assert!(CODIGO.iter().any(|&(lo, _)| lo >> 32 == c), "{c:#x}");
        }
        assert_eq!(CODIGO[0].0, 0x7918);
        assert_eq!(CODIGO[156].0, 0x794D);
    }
}
