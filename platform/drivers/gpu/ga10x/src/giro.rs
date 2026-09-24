//! **M5d G: UNA ESFERA QUE GIRA Y BOTA, CON LUZ** -- movimiento 3D dibujado por
//! la 3060, fotograma a fotograma: una pelota de gajos (verde NVIDIA y blanco)
//! que gira sobre su eje vertical, bota dos veces por vuelta y lleva su sombra
//! en un suelo que avanza. 32 fotogramas de 256 x 256; cada uno es UN trabajo
//! de la 3060 con sus parametros, y la CPU rehace la cuenta de cada uno y la
//! compara bit a bit: la CPU NO dibuja.
//!
//! capa: puro -- el programa, los parametros de cada fotograma y la cuenta de
//! referencia; la RAM y los registros los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- todo entero, como la escena: la raiz digito a
//!           digito, el giro en Q14 con desplazamiento aritmetico, las
//!           divisiones truncando
//!
//! # Por que por computo (24-09)
//!
//! El triangulo por el rasterizador (T1c) aun se cuelga en el metal. El
//! propietario pidio "algo 3D con movimiento" mientras tanto: esto va por el
//! camino que YA funciona en su 3060 (el de `escena`), con dos cosas nuevas:
//! el programa LEE sus parametros de memoria (5 `LDG.E.STRONG.SYS`, que no se
//! quedan con el fotograma anterior en cache) y se lanza una vez por
//! fotograma.
//!
//! # La cuenta (PTX de origen, `ptxas -arch=sm_86`: 18 registros)
//!
//! ```text
//!    parametros  cy (el bote), cos y sin del giro en Q14, la medida de la
//!                sombra, el avance del suelo -- en VA 0x2_0000_E400
//!    esfera      centro (128, cy), radio 56; z = raiz(56^2 - d^2)
//!    giro        rx = (dx cos - z sin) >> 14,  rz = (dx sin + z cos) >> 14
//!    gajos       (rx < 0) ^ (rz < 0) ^ (|rx| > |rz|): 8 gajos, verde o blanco
//!    luz         la de la escena (difusa + brillo ^16), /56
//!    cielo       y < 160;  suelo en perspectiva que avanza, y la sombra
//!                (128, 222) que crece cuando la pelota baja
//! ```

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::lienzo::sombreador_va;
use crate::sombreador::{ordenes_con, qmd_con, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::vram::a_cero;
use crate::Registros;

/// El lado de un fotograma, y sus pixeles.
pub const LADO: u32 = 256;
pub const PIXELES: usize = (LADO * LADO) as usize;
/// Cuantos fotogramas da una vuelta.
pub const FOTOGRAMAS: u32 = 32;
pub const REGISTROS: u32 = 32;
pub const PAGA_QMD: u32 = 0x3060_6100;
pub const PAGA_FIN: u32 = 0x3060_61F0;
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x100;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x110;
/// Donde lee el programa sus 5 parametros (VA 0x2_0000_E400).
pub const PARAMETROS: u64 = SEMAFOROS + 0x400;

/// **Los parametros de cada fotograma**: `[cy, cos, sin, sombra, avance]`
/// (el giro en Q14, 11,25 grados por fotograma; dos botes por vuelta). Salen
/// de `sin`/`cos` redondeados; la CPU y la 3060 usan ESTA tabla, asi que da
/// igual como se sacaron.
pub const TABLA: [[i32; 5]; 32] = [
    [165, 16384, 0, 2600, 0],
    [148, 16069, 3196, 2260, 2],
    [132, 15137, 6270, 1940, 4],
    [118, 13623, 9102, 1660, 6],
    [105, 11585, 11585, 1400, 8],
    [94, 9102, 13623, 1180, 10],
    [86, 6270, 15137, 1020, 12],
    [82, 3196, 16069, 940, 14],
    [80, 0, 16384, 900, 16],
    [82, -3196, 16069, 940, 18],
    [86, -6270, 15137, 1020, 20],
    [94, -9102, 13623, 1180, 22],
    [105, -11585, 11585, 1400, 24],
    [118, -13623, 9102, 1660, 26],
    [132, -15137, 6270, 1940, 28],
    [148, -16069, 3196, 2260, 30],
    [165, -16384, 0, 2600, 32],
    [148, -16069, -3196, 2260, 34],
    [132, -15137, -6270, 1940, 36],
    [118, -13623, -9102, 1660, 38],
    [105, -11585, -11585, 1400, 40],
    [94, -9102, -13623, 1180, 42],
    [86, -6270, -15137, 1020, 44],
    [82, -3196, -16069, 940, 46],
    [80, 0, -16384, 900, 48],
    [82, 3196, -16069, 940, 50],
    [86, 6270, -15137, 1020, 52],
    [94, 9102, -13623, 1180, 54],
    [105, 11585, -11585, 1400, 56],
    [118, 13623, -9102, 1660, 58],
    [132, 15137, -6270, 1940, 60],
    [148, 16069, -3196, 2260, 62],
];

pub const fn parametros(f: u32) -> [i32; 5] {
    TABLA[(f % FOTOGRAMAS) as usize]
}

/// La raiz entera, digito a digito: la MISMA que el programa.
const fn raiz(n: i32) -> i32 {
    crate::escena::raiz(n)
}

/// **Lo que pinta `(x, y)` en el fotograma de `p`**: la MISMA cuenta que el
/// programa, instruccion a instruccion.
pub const fn pixel(x: u32, y: u32, p: &[i32; 5]) -> u32 {
    let [cy, c, s, sombra, avance] = *p;
    let (dx, dy) = (x as i32 - 128, y as i32 - cy);
    let d2 = dx * dx + dy * dy;
    if d2 < 3136 {
        let z = raiz(3136 - d2);
        let rx = (dx * c - z * s) >> 14;
        let rz = (dx * s + z * c) >> 14;
        let verde = (rx < 0) ^ (rz < 0) ^ (rx.abs() > rz.abs());
        let base = if verde { [118, 185, 0] } else { [235, 235, 235] };
        let mut lam = (dx * -116 + dy * -140 + z * 181) / 56;
        if lam < 0 {
            lam = 0;
        }
        let mut h = (dx * -63 + dy * -76 + z * 236) / 56;
        h = if h < 0 { 0 } else if h > 256 { 256 } else { h };
        let mut k = 0;
        while k < 4 {
            h = (h * h) >> 8;
            k += 1;
        }
        let brillo = (h * 230) >> 8;
        let luz = ((lam * 216) >> 8) + 40;
        let r = min255(((luz * base[0]) >> 8) + brillo);
        let g = min255(((luz * base[1]) >> 8) + brillo);
        let b = min255(((luz * base[2]) >> 8) + brillo);
        return (r as u32) << 16 | (g as u32) << 8 | b as u32;
    }
    let yi = y as i32;
    if yi < 160 {
        return 0x000A_0000 | (((yi >> 3) + 12) as u32) << 8 | ((yi >> 2) + 24) as u32;
    }
    let fondo = yi - 150;
    let u = ((dx << 6) / fondo) >> 5;
    let v = (4096 / fondo + avance) >> 3;
    let claro = (u ^ v) & 1 != 0;
    let (mut r, mut g, mut b) = if claro { (58, 63, 85) } else { (28, 31, 43) };
    let sy = yi - 222;
    if dx * dx + ((sy * sy) << 4) < sombra {
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

/// Cuantos pixeles del fotograma `f` son lo que tienen que ser.
pub fn comprobar(salida: &[u32], f: u32) -> u32 {
    let p = parametros(f);
    salida.iter().take(PIXELES).enumerate().filter(|&(k, &v)| v == pixel(k as u32 % LADO, k as u32 / LADO, &p)).count() as u32
}

/// **El programa** (182 instrucciones): los dos NOP de siempre (el `c[0x0][0x28]`
/// y el `ULDC.64`, sin barreras) y los LDG/STG sin descriptor (bit 101).
pub const CODIGO: [(u64, u64); 182] = [
    (0x0000000000007918, 0x000fe40000000000), // NOP (era IMAD.MOV.U32 R1, RZ, RZ, c[0x0][0x28])
    (0x00000000ff087424, 0x000fe200078e00ff), // IMAD.MOV.U32 R8, RZ, RZ, 0x0
    (0x0000000000007918, 0x000fe20000000000), // NOP (era ULDC.64 UR6, c[0x0][0x118])
    (0x00000002ff097424, 0x000fca00078e00ff), // IMAD.MOV.U32 R9, RZ, RZ, 0x2
    (0x00e40006080b7981, 0x000ea8000c1f5900), // LDG.E.STRONG.SYS R11, [R8.64+0xe400]
    (0x00e4040608037981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R3, [R8.64+0xe404]
    (0x00e4080608057981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R5, [R8.64+0xe408]
    (0x00e40c0608067981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R6, [R8.64+0xe40c]
    (0x00e4100608077981, 0x000162000c1f5900), // LDG.E.STRONG.SYS R7, [R8.64+0xe410]
    (0x00000a5000007945, 0x000fe60003800000), // BSSY B0, 0xaf0
    (0x0000000000007919, 0x000e680000002100), // S2R R0, SR_TID.X
    (0x0000000000027919, 0x000ea20000002500), // S2R R2, SR_CTAID.X
    (0xffffff8000047810, 0x002fca0007ffe0ff), // IADD3 R4, R0, -0x80, RZ
    (0x00000004040a7224, 0x000fe400078e02ff), // IMAD R10, R4, R4, RZ
    (0x00000001020b7824, 0x004fc800078e0a0b), // IMAD.IADD R11, R2, 0x1, -R11
    (0x0000000b0b0a7224, 0x000fca00078e020a), // IMAD R10, R11, R11, R10
    (0x00000c400a00780c, 0x000fda0003f06270), // ISETP.GE.AND P0, PT, R10, 0xc40, PT
    (0x000005b000000947, 0x000fea0003800000), // @P0 BRA 0x6d0
    (0x00000c400a0a7810, 0x001fe20007ffe1ff), // IADD3 R10, -R10, 0xc40, RZ
    (0x000000ffff067224, 0x020fe200078e00ff), // IMAD.MOV.U32 R6, RZ, RZ, RZ
    (0x0000400000047882, 0x000fc80000000000), // UMOV UR4, 0x4000
    (0x0000000406077c10, 0x000fe4000fffe0ff), // IADD3 R7, R6, UR4, RZ
    (0x00000001ff067819, 0x000fe40000011406), // SHF.R.S32.HI R6, RZ, 0x1, R6
    (0x000000070a00720c, 0x000fda0003f06270), // ISETP.GE.AND P0, PT, R10, R7, PT
    (0x0000000406060c10, 0x000fe2000fffe0ff), // @P0 IADD3 R6, R6, UR4, RZ
    (0x000000023f047899, 0x000fe20008011404), // USHF.R.S32.HI UR4, URZ, 0x2, UR4
    (0x000000010a0a0824, 0x000fca00078e0a07), // @P0 IMAD.IADD R10, R10, 0x1, -R7
    (0x00000004ff007c0c, 0x000fda000bf25270), // ISETP.NE.AND P1, PT, RZ, UR4, PT
    (0xffffff8000001947, 0x000fea000383ffff), // @P1 BRA 0x150
    (0x0000003800077906, 0x000e220000209000), // I2F.U32.RP R7, 0x38
    (0xffffffc1ff0d7424, 0x000fc800078e00ff), // IMAD.MOV.U32 R13, RZ, RZ, -0x3f
    (0x00001f80000a7424, 0x000fc800078e020d), // IMAD R10, R0, R13, 0x1f80
    (0xffffffb40b0d7824, 0x000fc800078e020a), // IMAD R13, R11, -0x4c, R10
    (0x000000ec060d7824, 0x000fe200078e020d), // IMAD R13, R6, 0xec, R13
    (0x0000000700077308, 0x001e240000001000), // MUFU.RCP R7, R7
    (0x0ffffffe07087810, 0x001fe40007ffe0ff), // IADD3 R8, R7, 0xffffffe, RZ
    (0x0000000d00077213, 0x000fc80000000000), // IABS R7, R13
    (0x0000000800097305, 0x000062000021f000), // F2I.FTZ.U32.TRUNC.NTZ R9, R8
    (0x000000380d0d7812, 0x000fc800078e3cff), // LOP3.LUT R13, R13, 0x38, RZ, 0x3c, !PT
    (0x000000ff0d00720c, 0x000fe20003f46270), // ISETP.GE.AND P2, PT, R13, RZ, PT
    (0x000000ffff087224, 0x001fe400078e00ff), // IMAD.MOV.U32 R8, RZ, RZ, RZ
    (0xffffffc8090f7824, 0x002fc800078e02ff), // IMAD R15, R9, -0x38, RZ
    (0x0000000f09097227, 0x000fc800078e0008), // IMAD.HI.U32 R9, R9, R15, R8
    (0xffffff8cff0f7424, 0x000fe400078e00ff), // IMAD.MOV.U32 R15, RZ, RZ, -0x74
    (0x00000007090a7227, 0x000fc800078e00ff), // IMAD.HI.U32 R10, R9, R7, RZ
    (0xffffffc80a077824, 0x000fe400078e0207), // IMAD R7, R10, -0x38, R7
    (0x00003a0000087424, 0x000fc600078e020f), // IMAD R8, R0, R15, 0x3a00
    (0x000000380700780c, 0x000fe20003f06070), // ISETP.GE.U32.AND P0, PT, R7, 0x38, PT
    (0xffffff740b0b7824, 0x000fc800078e0208), // IMAD R11, R11, -0x8c, R8
    (0x000000b5060b7824, 0x000fca00078e020b), // IMAD R11, R6, 0xb5, R11
    (0x0000000b00087213, 0x000fe40000000000), // IABS R8, R11
    (0x000000380b0b7812, 0x000fe400078e3cff), // LOP3.LUT R11, R11, 0x38, RZ, 0x3c, !PT
    (0xffffffc807070810, 0x000fe20007ffe0ff), // @P0 IADD3 R7, R7, -0x38, RZ
    (0x0000000809097227, 0x000fe200078e00ff), // IMAD.HI.U32 R9, R9, R8, RZ
    (0x000000010a0a0810, 0x000fe40007ffe0ff), // @P0 IADD3 R10, R10, 0x1, RZ
    (0x000000380700780c, 0x000fe20003f26070), // ISETP.GE.U32.AND P1, PT, R7, 0x38, PT
    (0xffffffc809077824, 0x000fca00078e0208), // IMAD R7, R9, -0x38, R8
    (0x000000380700780c, 0x000fce0003f06070), // ISETP.GE.U32.AND P0, PT, R7, 0x38, PT
    (0x000000010a0a1810, 0x000fca0007ffe0ff), // @P1 IADD3 R10, R10, 0x1, RZ
    (0x000000ffff0aa224, 0x000fe200078e0a0a), // @!P2 IMAD.MOV R10, RZ, RZ, -R10
    (0xffffffc807070810, 0x000fe40007ffe0ff), // @P0 IADD3 R7, R7, -0x38, RZ
    (0x000000ff0b00720c, 0x000fe40003f46270), // ISETP.GE.AND P2, PT, R11, RZ, PT
    (0x0000000aff0a7217, 0x000fe40007800200), // IMNMX R10, RZ, R10, !PT
    (0x000000380700780c, 0x000fe20003f26070), // ISETP.GE.U32.AND P1, PT, R7, 0x38, PT
    (0x0000000605077224, 0x040fe200078e02ff), // IMAD R7, R5.reuse, R6, RZ
    (0x000001000a0a7817, 0x000fe20003800200), // IMNMX R10, R10, 0x100, PT
    (0x0000000405057224, 0x080fe200078e02ff), // IMAD R5, R5, R4.reuse, RZ
    (0x0000000109090810, 0x000fe20007ffe0ff), // @P0 IADD3 R9, R9, 0x1, RZ
    (0x0000000403077224, 0x000fc400078e0a07), // IMAD R7, R3, R4, -R7
    (0x0000000a0a0a7224, 0x000fe400078e02ff), // IMAD R10, R10, R10, RZ
    (0x0000000603057224, 0x000fe200078e0205), // IMAD R5, R3, R6, R5
    (0x0000000eff077819, 0x000fe40000011407), // SHF.R.S32.HI R7, RZ, 0xe, R7
    (0x00000008ff0a7819, 0x000fe4000001140a), // SHF.R.S32.HI R10, RZ, 0x8, R10
    (0x0000000109091810, 0x000fe40007ffe0ff), // @P1 IADD3 R9, R9, 0x1, RZ
    (0x0000000eff057819, 0x000fe20000011405), // SHF.R.S32.HI R5, RZ, 0xe, R5
    (0x0000000a0a0a7224, 0x000fe200078e02ff), // IMAD R10, R10, R10, RZ
    (0x0000000700037213, 0x000fe20000000000), // IABS R3, R7
    (0x000000ffff09a224, 0x000fe200078e0a09), // @!P2 IMAD.MOV R9, RZ, RZ, -R9
    (0x000000ff0500720c, 0x000fc40003f06270), // ISETP.GE.AND P0, PT, R5, RZ, PT
    (0x00000008ff0a7819, 0x000fe4000001140a), // SHF.R.S32.HI R10, RZ, 0x8, R10
    (0x000000ff0700720c, 0x000fe40004701a70), // ISETP.LT.XOR P0, PT, R7, RZ, !P0
    (0x0000000500047213, 0x000fe20000000000), // IABS R4, R5
    (0x0000000a0a0a7224, 0x000fe200078e02ff), // IMAD R10, R10, R10, RZ
    (0x00000009ff097217, 0x000fe40007800200), // IMNMX R9, RZ, R9, !PT
    (0x000000040300720c, 0x000fe20000704a70), // ISETP.GT.XOR P0, PT, R3, R4, P0
    (0x000000ebff047424, 0x000fe200078e00ff), // IMAD.MOV.U32 R4, RZ, RZ, 0xeb
    (0x00000008ff0a7819, 0x000fe2000001140a), // SHF.R.S32.HI R10, RZ, 0x8, R10
    (0x000000d809037824, 0x000fe200078e02ff), // IMAD R3, R9, 0xd8, RZ
    (0x000000ebff087807, 0x000fc40000000000), // SEL R8, RZ, 0xeb, P0
    (0x0000007604067807, 0x000fe20004000000), // SEL R6, R4, 0x76, !P0
    (0x0000000a0a0a7224, 0x000fe200078e02ff), // IMAD R10, R10, R10, RZ
    (0x0000002803037811, 0x000fe400078fc2ff), // LEA.HI.SX32 R3, R3, 0x28, 0x18
    (0x000000b904047807, 0x000fe40004000000), // SEL R4, R4, 0xb9, !P0
    (0x00000008ff0a7819, 0x000fe2000001140a), // SHF.R.S32.HI R10, RZ, 0x8, R10
    (0x0000000306057224, 0x080fe400078e02ff), // IMAD R5, R6, R3.reuse, RZ
    (0x0000000304077224, 0x000fe400078e02ff), // IMAD R7, R4, R3, RZ
    (0x000000e60a0a7824, 0x000fc400078e02ff), // IMAD R10, R10, 0xe6, RZ
    (0x0000000308037224, 0x000fc600078e02ff), // IMAD R3, R8, R3, RZ
    (0x00000008ff0a7819, 0x000fc8000001140a), // SHF.R.S32.HI R10, RZ, 0x8, R10
    (0x0000000a05057211, 0x080fe400078fc2ff), // LEA.HI.SX32 R5, R5, R10.reuse, 0x18
    (0x0000000a07077211, 0x080fe400078fc2ff), // LEA.HI.SX32 R7, R7, R10.reuse, 0x18
    (0x000000ff05057817, 0x000fe40003800200), // IMNMX R5, R5, 0xff, PT
    (0x000000ff07077817, 0x000fe40003800200), // IMNMX R7, R7, 0xff, PT
    (0x0000000a03037211, 0x000fe200078fc2ff), // LEA.HI.SX32 R3, R3, R10, 0x18
    (0x0001000005047824, 0x000fe400078e00ff), // IMAD.U32 R4, R5, 0x10000, RZ
    (0x0000010007077824, 0x000fe200078e00ff), // IMAD.SHL.U32 R7, R7, 0x100, RZ
    (0x000000ff03037817, 0x000fc80003800200), // IMNMX R3, R3, 0xff, PT
    (0x0000000403057212, 0x000fe200078efe07), // LOP3.LUT R5, R3, R4, R7, 0xfe, !PT
    (0x0000041000007947, 0x000fea0003800000), // BRA 0xae0
    (0x000000a00200780c, 0x001fda0003f06270), // ISETP.GE.AND P0, PT, R2, 0xa0, PT
    (0x0000000c02038811, 0x060fe400078feaff), // @!P0 LEA.HI.SX32 R3, R2.reuse, 0xc, 0x1d
    (0x0000001802058811, 0x000fc600078ff2ff), // @!P0 LEA.HI.SX32 R5, R2, 0x18, 0x1e
    (0x0000010003088824, 0x000fca00078e00ff), // @!P0 IMAD.SHL.U32 R8, R3, 0x100, RZ
    (0x000a000008058812, 0x000fe200078efe05), // @!P0 LOP3.LUT R5, R8, 0xa0000, R5, 0xfe, !PT
    (0x000003b000008947, 0x000fea0003800000), // @!P0 BRA 0xae0
    (0xffffff6a02037810, 0x000fe20007ffe0ff), // IADD3 R3, R2, -0x96, RZ
    (0x00000040040a7824, 0x000fc600078e00ff), // IMAD.SHL.U32 R10, R4, 0x40, RZ
    (0x00000003000c7213, 0x080fe40000000000), // IABS R12, R3.reuse
    (0x00000003000d7213, 0x080fe40000000000), // IABS R13, R3.reuse
    (0x0000000c00057306, 0x000e220000209400), // I2F.RP R5, R12
    (0x0000000a000e7213, 0x000fe40000000000), // IABS R14, R10
    (0x000000030a0a7212, 0x000fca00078e3cff), // LOP3.LUT R10, R10, R3, RZ, 0x3c, !PT
    (0x0000000500057308, 0x001e240000001000), // MUFU.RCP R5, R5
    (0x0ffffffe05087810, 0x001fe20007ffe0ff), // IADD3 R8, R5, 0xffffffe, RZ
    (0x000000ffff057224, 0x000fca00078e0a0d), // IMAD.MOV R5, RZ, RZ, -R13
    (0x0000000800097305, 0x000064000021f000), // F2I.FTZ.U32.TRUNC.NTZ R9, R8
    (0x000000ffff087224, 0x001fe400078e00ff), // IMAD.MOV.U32 R8, RZ, RZ, RZ
    (0x000000ffff0b7224, 0x002fc800078e0a09), // IMAD.MOV R11, RZ, RZ, -R9
    (0x0000000c0b0b7224, 0x000fc800078e02ff), // IMAD R11, R11, R12, RZ
    (0x0000000b09097227, 0x000fcc00078e0008), // IMAD.HI.U32 R9, R9, R11, R8
    (0x0000100009087827, 0x000fc800078e00ff), // IMAD.HI.U32 R8, R9, 0x1000, RZ
    (0x00001000080b7424, 0x000fe400078e0205), // IMAD R11, R8, R5, 0x1000
    (0x0000000e09097227, 0x000fc600078e00ff), // IMAD.HI.U32 R9, R9, R14, RZ
    (0x0000000b0c00720c, 0x000fe20003f04070), // ISETP.GT.U32.AND P0, PT, R12, R11, PT
    (0x0000000509057224, 0x000fca00078e020e), // IMAD R5, R9, R5, R14
    (0x000000050c00720c, 0x000fce0003f64070), // ISETP.GT.U32.AND P3, PT, R12, R5, PT
    (0x000000010b0b8824, 0x100fe200078e0a0c), // @!P0 IMAD.IADD R11, R11, 0x1, -R12.reuse
    (0x0000000108088810, 0x000fe40007ffe0ff), // @!P0 IADD3 R8, R8, 0x1, RZ
    (0x000000ff0a00720c, 0x000fe40003f06270), // ISETP.GE.AND P0, PT, R10, RZ, PT
    (0x0000000c0b00720c, 0x000fe20003f86070), // ISETP.GE.U32.AND P4, PT, R11, R12, PT
    (0x000000010505b824, 0x000fe200078e0a0c), // @!P3 IMAD.IADD R5, R5, 0x1, -R12
    (0x00001000030b7812, 0x000fe400078e3cff), // LOP3.LUT R11, R3, 0x1000, RZ, 0x3c, !PT
    (0x000000010909b810, 0x000fe40007ffe0ff), // @!P3 IADD3 R9, R9, 0x1, RZ
    (0x000000ff0b00720c, 0x000fc40003f26270), // ISETP.GE.AND P1, PT, R11, RZ, PT
    (0x0000000c0500720c, 0x000fe40003f46070), // ISETP.GE.U32.AND P2, PT, R5, R12, PT
    (0xffffff2202057810, 0x000fe40007ffe0ff), // IADD3 R5, R2, -0xde, RZ
    (0x000000ff0300720c, 0x000fe40003f65270), // ISETP.NE.AND P3, PT, R3, RZ, PT
    (0x0000000108084810, 0x000fe20007ffe0ff), // @P4 IADD3 R8, R8, 0x1, RZ
    (0x00000005050a7224, 0x000fc800078e02ff), // IMAD R10, R5, R5, RZ
    (0x000000ffff057224, 0x000fe200078e0008), // IMAD.MOV.U32 R5, RZ, RZ, R8
    (0x00000003ff087212, 0x000fe200078e33ff), // LOP3.LUT R8, RZ, R3, RZ, 0x33, !PT
    (0x000000100a037824, 0x000fe200078e00ff), // IMAD.SHL.U32 R3, R10, 0x10, RZ
    (0x0000000109092810, 0x000fe20007ffe0ff), // @P2 IADD3 R9, R9, 0x1, RZ
    (0x000000ffff059224, 0x000fe400078e0a05), // @!P1 IMAD.MOV R5, RZ, RZ, -R5
    (0x0000000404037224, 0x000fe400078e0203), // IMAD R3, R4, R4, R3
    (0x000000ffff098224, 0x000fe200078e0a09), // @!P0 IMAD.MOV R9, RZ, RZ, -R9
    (0x00000005080a7207, 0x040fe20005800000), // SEL R10, R8.reuse, R5, !P3
    (0x0000003aff057424, 0x000fe200078e00ff), // IMAD.MOV.U32 R5, RZ, RZ, 0x3a
    (0x000000060300720c, 0x000fe20003f26270), // ISETP.GE.AND P1, PT, R3, R6, PT
    (0x00000055ff067424, 0x000fe200078e00ff), // IMAD.MOV.U32 R6, RZ, RZ, 0x55
    (0x0000000908087207, 0x000fe20005800000), // SEL R8, R8, R9, !P3
    (0x0000000107077824, 0x000fc600078e020a), // IMAD.IADD R7, R7, 0x1, R10
    (0x00000005ff037819, 0x000fe40000011408), // SHF.R.S32.HI R3, RZ, 0x5, R8
    (0x00000003ff047819, 0x000fc80000011407), // SHF.R.S32.HI R4, RZ, 0x3, R7
    (0x0000000103ff7812, 0x000fe20007804804), // LOP3.LUT P0, RZ, R3, 0x1, R4, 0x48, !PT
    (0x0000003fff047424, 0x000fc600078e00ff), // IMAD.MOV.U32 R4, RZ, RZ, 0x3f
    (0x0000001c05037807, 0x000fe40000000000), // SEL R3, R5, 0x1c, P0
    (0x0000001f04047807, 0x000fe40000000000), // SEL R4, R4, 0x1f, P0
    (0x0000002b06057807, 0x000fe40000000000), // SEL R5, R6, 0x2b, P0
    (0x00000001ff039819, 0x000fe40000011403), // @!P1 SHF.R.S32.HI R3, RZ, 0x1, R3
    (0x00000001ff049819, 0x000fe40000011404), // @!P1 SHF.R.S32.HI R4, RZ, 0x1, R4
    (0x00000001ff059819, 0x000fc60000011405), // @!P1 SHF.R.S32.HI R5, RZ, 0x1, R5
    (0x0000010003047824, 0x000fc800078e0204), // IMAD R4, R3, 0x100, R4
    (0x0000010004057824, 0x000fe400078e0005), // IMAD.U32 R5, R4, 0x100, R5
    (0x0000000000007941, 0x000fea0003800000), // BSYNC B0
    (0x0000010002037824, 0x000fca00078e00ff), // IMAD.SHL.U32 R3, R2, 0x100, RZ
    (0x0000000003037212, 0x000fc800078efcff), // LOP3.LUT R3, R3, R0, RZ, 0xfc, !PT
    (0x0003000003027811, 0x000fe200078e10ff), // LEA R2, R3, 0x30000, 0x2
    (0x00000002ff037424, 0x000fca00078e00ff), // IMAD.MOV.U32 R3, RZ, RZ, 0x2
    (0x0000000502007986, 0x000fe2000c101906), // STG.E [R2.64], R5
    (0x000000000000794d, 0x000fea0003800000), // EXIT
    (0xfffffff000007947, 0x000fc0000383ffff), // BRA 0xb50
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

/// 256 hilos por bloque, 256 bloques: uno por pixel.
pub fn qmd() -> [u32; QMD_PALABRAS] {
    qmd_con(sombreador_va(PROGRAMA), LADO, LADO, sombreador_va(SEMAFORO_QMD), PAGA_QMD, REGISTROS)
}

pub fn ordenes() -> [u32; ORDENES] {
    ordenes_con(sombreador_va(QMD), sombreador_va(SEMAFORO_FIN), PAGA_FIN)
}

/// **Preparar el fotograma `f`** con la entrada `e` del GPFIFO de GR: sus
/// parametros, el programa, el QMD, las ordenes y la entrada.
pub fn preparar<R: Registros>(r: &mut R, e: u32, f: u32) -> bool {
    if !crate::blur::entrada_valida(e) {
        return false;
    }
    let p = parametros(f).map(|v| v as u32);
    let c = codigo();
    let q = qmd();
    let o = ordenes();
    let en = entrada(sombreador_va(EMPUJE), ORDENES as u32);
    let pag = crate::vram::PALABRAS;
    escribir(r, SEMAFORO_QMD, &[0; 8]) == 8
        && escribir(r, PARAMETROS, &p) == 5
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

pub use crate::fractal::{desempaquetar, empaquetar};

/// Sano: los dos semaforos pagados y el fotograma entero igual.
pub const fn sano(v: u64) -> bool {
    let (buenos, qmd, fin, lanzado, _, _) = desempaquetar(v);
    buenos as usize == PIXELES && qmd && fin && lanzado
}

const _: () = assert!(PALABRAS_CODIGO * 4 <= 4096);
const _: () = assert!(PARAMETROS >= SEMAFOROS + 0x200 && PARAMETROS + 20 <= SEMAFOROS + 4096);
const _: () = assert!(SEMAFORO_QMD > crate::color3d::SEMAFORO_FIN && SEMAFORO_FIN + 16 <= PARAMETROS);
const _: () = assert!(crate::lienzo::sombreador_va(PARAMETROS) == 0x2_0000_E400);

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_programa_lee_sus_parametros_donde_se_escriben() {
        // Los 5 LDG: [R8.64 + 0xe400 + 4k], con R8:R9 = 0x2_0000_0000.
        let ldg: [u64; 5] = core::array::from_fn(|k| 0xE400 + 4 * k as u64);
        let mut n = 0;
        for &(lo, hi) in CODIGO.iter() {
            if lo & 0xFFF == 0x981 {
                assert_eq!(lo >> 40, ldg[n], "LDG {n}");
                assert_eq!(hi >> 37 & 1, 0, "LDG sin descriptor");
                n += 1;
            }
        }
        assert_eq!(n, 5);
        assert_eq!(CODIGO[3].0 >> 32, 2); // MOV R9, 0x2
    }

    #[test]
    fn la_tabla_cierra_la_vuelta() {
        // El fotograma 0 y el que seguiria al ultimo son el mismo: el bucle no salta.
        assert_eq!(TABLA[0], [165, 16384, 0, 2600, 0]);
        let u = TABLA[FOTOGRAMAS as usize - 1];
        assert_eq!((u[0], u[4] + 2), (148, 2 * FOTOGRAMAS as i32));
        // El suelo avanza 64 en una vuelta: 8 cuadros, par: el damero casa.
        assert_eq!((2 * FOTOGRAMAS as i32 >> 3) % 2, 0);
    }

    #[test]
    fn la_pelota_se_ve() {
        let p = parametros(0);
        // El centro de la pelota, iluminado; lejos, cielo y suelo.
        let c = pixel(128, 165, &p);
        assert!(c != pixel(128, 20, &p) && c != pixel(5, 250, &p));
        // Los gajos: a la izquierda y a la derecha del centro, colores distintos.
        assert_ne!(pixel(110, 165, &p) & 0xFF, pixel(146, 165, &p) & 0xFF);
        // Gira: el mismo pixel cambia de gajo entre fotogramas.
        let q = parametros(4);
        assert!((0..256).any(|x| pixel(x, 165, &p) != pixel(x, 165, &q)));
    }

    #[test]
    fn se_comprueba() {
        let mut img = [0u32; PIXELES];
        for (k, v) in img.iter_mut().enumerate() {
            *v = pixel(k as u32 % LADO, k as u32 / LADO, &parametros(7));
        }
        assert_eq!(comprobar(&img, 7), PIXELES as u32);
        assert!(comprobar(&img, 8) < PIXELES as u32);
    }
}
