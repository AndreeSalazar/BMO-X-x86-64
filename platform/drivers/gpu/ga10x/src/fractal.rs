//! **M5d F: EL FRACTAL -- LA FUERZA DE LA 3060** -- algo mucho mas pesado que
//! el blur: el conjunto de Mandelbrot de 512 x 512, hasta 256 vueltas por
//! pixel, 262144 hilos a la vez, en 1 MiB de RAM del PC prestado por la IOMMU.
//! El kernel mide cuanto tarda la 3060 y cuanto tarda la CPU en lo MISMO, y
//! compara cada pixel: tienen que salir iguales bit a bit.
//!
//! capa: puro -- el programa, las PTE, la cuenta de referencia; la RAM y los
//! registros los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- aritmetica ENTERA de punto fijo (Q4.28), no coma
//!           flotante: la CPU puede rehacer la cuenta exacta y comparar; con
//!           `float` la 3060 (FMA) y la CPU redondearian distinto
//!
//! # El programa (PTX de origen, `ptxas -arch=sm_86`: 15 registros)
//!
//! ```text
//!    x = tid.x (0..512)  y = ctaid.x (0..512)
//!    cr = -2.5 + x * 3.5/512     ci = -1.75 + y * 3.5/512       (Q4.28)
//!    z = 0; n = 0
//!    bucle ("nounroll"):
//!       si zr*zr + zi*zi > 4 (en 64 bits, Q8.56): salir
//!       zr, zi = (zr*zr >> 28) - (zi*zi >> 28) + cr, (zr*zi >> 27) + ci
//!       n += 1; si n == 256: salir
//!    salida[y][x] = n == 256 ? negro : color(n)
//! ```
//!
//! 44 instrucciones; como en los anteriores, `c[0x0][0x28]` y `ULDC.64 UR4,
//! c[0x0][0x118]` en NOP (ninguna pone barrera), el STG sin descriptor, y el
//! bucle con su barrera de convergencia (`BSSY`/`BSYNC B0`) tal cual.
//!
//! # Donde vive
//!
//! ```text
//!    salida  256 marcos (1 MiB), IOVA 0x3A10_0000, VA 0x2_0003_0000: las
//!            entradas 48..303 de la PT del tramo (tras el lienzo y el blur)
//!    tramo   11 el programa, 13 el QMD, 14 los semaforos (+0x60, +0x70),
//!            15 las ordenes; el GPFIFO de GR, la entrada que diga el kernel
//! ```

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::lienzo::sombreador_va;
use crate::sombreador::{ordenes_con, qmd_con, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::vram::{a_cero, escribir64, leer64, TRAMO_VA};
use crate::Registros;

pub const LADO: u32 = 512;
pub const PIXELES: usize = (LADO * LADO) as usize;
pub const PAGINAS: u64 = (PIXELES * 4 / 4096) as u64;
pub const IOVA: u64 = 0x3A10_0000;
pub const VA: u64 = TRAMO_VA + 0x3_0000;
pub const PT_PRIMERA: usize = 48;
pub const VUELTAS: u32 = 256;
pub const REGISTROS: u32 = 32;
pub const PAGA_QMD: u32 = 0x3060_F4C0;
pub const PAGA_FIN: u32 = 0x3060_F4F0;
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x60;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x70;

/// El paso y la esquina, en Q4.28: 3.5 / 512, -2.5 y -1.75.
pub const PASO: i32 = 1_835_008;
pub const CR0: i32 = -671_088_640;
pub const CI0: i32 = -469_762_048;
/// 4.0 en Q8.56.
pub const CUATRO: i64 = 1 << 58;

/// **Las vueltas de `(x, y)`**: la MISMA cuenta que el programa, entera.
pub fn vueltas(x: u32, y: u32) -> u32 {
    let cr = (x as i32).wrapping_mul(PASO).wrapping_add(CR0);
    let ci = (y as i32).wrapping_mul(PASO).wrapping_add(CI0);
    let (mut zr, mut zi, mut n) = (0i32, 0i32, 0u32);
    loop {
        let (r2, i2) = (zr as i64 * zr as i64, zi as i64 * zi as i64);
        if r2 + i2 > CUATRO {
            return n;
        }
        let nzi = ((zr as i64 * zi as i64) >> 27) as i32;
        zi = nzi.wrapping_add(ci);
        zr = ((r2 >> 28) as i32).wrapping_sub((i2 >> 28) as i32).wrapping_add(cr);
        n += 1;
        if n >= VUELTAS {
            return n;
        }
    }
}

/// El color de `n` vueltas: 0x00RRGGBB, negro si no escapo.
pub const fn color(n: u32) -> u32 {
    if n == VUELTAS {
        return 0;
    }
    let r = (n << 1) & 0xFF;
    let g = if n << 3 > 0xFF { 0xFF } else { n << 3 };
    let b = (n << 4) & 0xFF;
    r << 16 | g << 8 | b
}

pub fn pixel(x: u32, y: u32) -> u32 {
    color(vueltas(x, y))
}

/// **Comprobar** la salida entera. `(pixeles iguales, vueltas en total)`.
pub fn comprobar(salida: &[u32]) -> (u32, u64) {
    let mut buenos = 0;
    let mut total = 0u64;
    for (k, &p) in salida.iter().take(PIXELES).enumerate() {
        let n = vueltas(k as u32 % LADO, k as u32 / LADO);
        total += n as u64;
        buenos += (p == color(n)) as u32;
    }
    (buenos, total)
}

/// **El programa**, como lo leyo `nvdisasm` (ver arriba).
pub const CODIGO: [(u64, u64); 44] = [
    (0x0000_0000_0000_7918, 0x000F_E400_0000_0000), // NOP (era IMAD.MOV.U32 R1, RZ, RZ, c[0x0][0x28])
    (0x0000_0000_0009_7919, 0x000E_2200_0000_2100), // S2R R9, SR_TID.X
    (0x001C_0000_FF08_7424, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R8, RZ, RZ, 0x1c0000
    (0x0000_0170_0000_7945, 0x000F_E200_0380_0000), // BSSY B0, 0x1b0
    (0x0000_0000_0002_7805, 0x000F_E200_0001_FF00), // CS2R R2, SRZ
    (0x0000_0000_000A_7919, 0x000E_6200_0000_2500), // S2R R10, SR_CTAID.X
    (0x0000_00FF_FF0B_7224, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R11, RZ, RZ, RZ
    (0x0000_0000_0000_7918, 0x000F_E200_0000_0000), // NOP (era ULDC.64 UR4, c[0x0][0x118])
    (0xD800_0000_0900_7424, 0x081F_E400_078E_0208), // IMAD R0, R9, R8.reuse, -0x28000000
    (0xE400_0000_0A08_7424, 0x002F_E400_078E_0208), // IMAD R8, R10, R8, -0x1c000000
    (0x0000_0002_0204_7225, 0x000F_C800_078E_02FF), // IMAD.WIDE R4, R2, R2, RZ
    (0x0000_0003_0306_7225, 0x000F_CA00_078E_02FF), // IMAD.WIDE R6, R3, R3, RZ
    (0x0000_0006_040C_7210, 0x000F_C800_07F3_E0FF), // IADD3 R12, P1, R4, R6, RZ
    (0x0000_00FF_0C00_720C, 0x000F_E200_03F0_4070), // ISETP.GT.U32.AND P0, PT, R12, RZ, PT
    (0x0000_0001_050C_7824, 0x000F_CA00_008E_0607), // IMAD.X R12, R5, 0x1, R7, P1
    (0x0400_0000_0C00_780C, 0x000F_DA00_03F0_4300), // ISETP.GT.AND.EX P0, PT, R12, 0x4000000, PT, P0
    (0x0000_0090_0000_0947, 0x000F_EA00_0380_0000), // @P0 BRA 0x1a0
    (0x0000_0001_0B0B_7810, 0x000F_E200_07FF_E0FF), // IADD3 R11, R11, 0x1, RZ
    (0x0000_0003_0202_7225, 0x000F_E200_078E_02FF), // IMAD.WIDE R2, R2, R3, RZ
    (0x0000_001C_0404_7819, 0x000F_E400_0000_1005), // SHF.R.S64 R4, R4, 0x1c, R5
    (0x0000_0100_0B00_780C, 0x000F_E400_03F0_6070), // ISETP.GE.U32.AND P0, PT, R11, 0x100, PT
    (0x0000_001B_0203_7819, 0x000F_E400_0000_1003), // SHF.R.S64 R3, R2, 0x1b, R3
    (0x0000_001C_0607_7819, 0x000F_C600_0000_1007), // SHF.R.S64 R7, R6, 0x1c, R7
    (0x0000_0001_0803_7824, 0x000F_E200_078E_0203), // IMAD.IADD R3, R8, 0x1, R3
    (0x0000_0004_0002_7210, 0x000F_CA00_07FF_E807), // IADD3 R2, R0, R4, -R7
    (0xFFFF_FF00_0000_8947, 0x000F_EA00_0383_FFFF), // @!P0 BRA 0xa0
    (0x0000_0000_0000_7941, 0x000F_EA00_0380_0000), // BSYNC B0
    (0x0000_0008_0B02_7824, 0x040F_E200_078E_00FF), // IMAD.SHL.U32 R2, R11.reuse, 0x8, RZ
    (0x0000_0100_0B00_780C, 0x040F_E200_03F0_5070), // ISETP.NE.U32.AND P0, PT, R11.reuse, 0x100, PT
    (0x0000_0002_0B00_7824, 0x040F_E400_078E_00FF), // IMAD.SHL.U32 R0, R11.reuse, 0x2, RZ
    (0x0000_0200_0A04_7824, 0x000F_E200_078E_00FF), // IMAD.SHL.U32 R4, R10, 0x200, RZ
    (0x0000_00FF_0202_7817, 0x000F_E400_0380_0000), // IMNMX.U32 R2, R2, 0xff, PT
    (0x0000_7054_0003_7816, 0x000F_E200_0000_00FF), // PRMT R3, R0, 0x7054, RZ
    (0x0000_0010_0B00_7824, 0x000F_E200_078E_00FF), // IMAD.SHL.U32 R0, R11, 0x10, RZ
    (0x0000_0009_0404_7212, 0x000F_E200_078E_FCFF), // LOP3.LUT R4, R4, R9, RZ, 0xfc, !PT
    (0x0000_0100_0202_7824, 0x000F_CA00_078E_00FF), // IMAD.SHL.U32 R2, R2, 0x100, RZ
    (0x0000_0002_0303_7212, 0x000F_E400_078E_FCFF), // LOP3.LUT R3, R3, R2, RZ, 0xfc, !PT
    (0x0003_0000_0402_7811, 0x000F_E400_078E_10FF), // LEA R2, R4, 0x30000, 0x2
    (0x0000_00FF_0300_7812, 0x000F_E200_078E_F800), // LOP3.LUT R0, R3, 0xff, R0, 0xf8, !PT
    (0x0000_0002_FF03_7424, 0x000F_C600_078E_00FF), // IMAD.MOV.U32 R3, RZ, RZ, 0x2
    (0x0000_00FF_0005_7207, 0x000F_CA00_0000_0000), // SEL R5, R0, RZ, P0
    (0x0000_0005_0200_7986, 0x000F_E200_0C10_1904), // STG.E [R2.64], R5
    (0x0000_0000_0000_794D, 0x000F_EA00_0380_0000), // EXIT
    (0xFFFF_FFF0_0000_7947, 0x000F_C000_0383_FFFF), // BRA 0x2b0
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

pub const fn pte_en(k: u64) -> u64 {
    crate::vram::TABLAS[3] + 8 * (PT_PRIMERA as u64 + k)
}

/// **Mapear la salida**: 256 PTE de SISTEMA, solo si estaban VACIAS, releidas.
pub fn mapear<R: Registros>(r: &mut R) -> Option<(u32, u32)> {
    if (0..PAGINAS).any(|k| leer64(r, pte_en(k)) != 0) {
        return None;
    }
    let mut bien = 0;
    for k in 0..PAGINAS {
        let v = crate::mmu::pte_sistema(IOVA + k * 4096);
        escribir64(r, pte_en(k), v);
        bien += (leer64(r, pte_en(k)) == v) as u32;
    }
    Some((PAGINAS as u32, bien))
}

/// **Preparar** con la entrada `e` del GPFIFO (la misma cuenta que el blur).
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

/// `buenos (19) | qmd << 19 | fin << 20 | lanzado << 21 | us de la 3060 (21)
/// << 22 | us de la CPU (21) << 43`.
pub const fn empaquetar(buenos: u32, qmd: bool, fin: bool, lanzado: bool, gpu_us: u32, cpu_us: u32) -> u64 {
    let tope = 0x1F_FFFF;
    let g = if gpu_us > tope { tope } else { gpu_us };
    let c = if cpu_us > tope { tope } else { cpu_us };
    (buenos as u64 & 0x7_FFFF) | (qmd as u64) << 19 | (fin as u64) << 20 | (lanzado as u64) << 21 | (g as u64) << 22 | (c as u64) << 43
}

/// `(buenos, qmd, fin, lanzado, us de la 3060, us de la CPU)`.
pub const fn desempaquetar(v: u64) -> (u32, bool, bool, bool, u32, u32) {
    ((v & 0x7_FFFF) as u32, v >> 19 & 1 != 0, v >> 20 & 1 != 0, v >> 21 & 1 != 0, (v >> 22 & 0x1F_FFFF) as u32, (v >> 43 & 0x1F_FFFF) as u32)
}

pub const fn sano(v: u64) -> bool {
    let (buenos, qmd, _, lanzado, _, _) = desempaquetar(v);
    buenos as usize == PIXELES && qmd && lanzado
}

const _: () = assert!(SEMAFORO_QMD != crate::blur::SEMAFORO_QMD && SEMAFORO_FIN != crate::blur::SEMAFORO_FIN);
const _: () = assert!(PT_PRIMERA + PAGINAS as usize <= 512);
const _: () = assert!(PALABRAS_CODIGO * 4 <= 4096);

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_salida_cabe_tras_el_blur() {
        assert_eq!(PAGINAS, 256);
        assert_eq!(VA, 0x2_0003_0000);
        assert_eq!(crate::mmu::indices(VA)[4], PT_PRIMERA);
        assert_eq!(PT_PRIMERA as u64, crate::blur::PT_PRIMERA as u64 + crate::lienzo::PAGINAS);
        assert!(IOVA >= crate::blur::IOVA + crate::lienzo::PAGINAS * 4096);
        // El programa lleva dentro esa VA: `LEA R2, R4, 0x30000, 0x2` y 2 arriba.
        assert!(CODIGO.iter().any(|&(lo, _)| lo >> 32 == VA & 0xFFFF_FFFF));
    }

    #[test]
    fn las_constantes_son_las_del_programa() {
        // IMAD.MOV.U32 R8, RZ, RZ, 0x1c0000 (el paso); IMAD R0 .. -0x28000000; R8 .. -0x1c000000.
        assert_eq!(PASO, 0x1C_0000);
        assert_eq!(CR0, -0x2800_0000);
        assert_eq!(CI0, -0x1C00_0000);
        assert!(CODIGO.iter().any(|&(lo, _)| lo >> 32 == 0x1C_0000));
    }

    #[test]
    fn la_cuenta_de_referencia() {
        // (0, 0) del plano: x = 2.5 * 512 / 3.5 = 365.7; el centro del
        // conjunto no escapa. La esquina (-2.5, -1.75) escapa en la primera.
        assert_eq!(vueltas(366, 256), VUELTAS);
        assert_eq!(vueltas(0, 0), 1);
        assert_eq!(pixel(366, 256), 0);
        assert_eq!(color(1), 0x0002_0810);
        assert_eq!(color(40), 0x0050_FF80);
        let v = empaquetar(262_144, true, true, true, 1500, 900_000);
        assert_eq!(desempaquetar(v), (262_144, true, true, true, 1500, 900_000));
        assert!(sano(v));
        assert_eq!(desempaquetar(empaquetar(0, false, false, true, 5_000_000, 9)).4, 0x1F_FFFF);
    }

    #[test]
    fn el_programa_es_el_de_nvdisasm() {
        assert_eq!(CODIGO[0].0, 0x7918);
        assert_eq!(CODIGO[7].0, 0x7918);
        for &(lo, hi) in &CODIGO {
            if lo & 0xFFF == 0x986 {
                assert_eq!(hi >> (101 - 64) & 1, 0);
            }
        }
        assert_eq!(CODIGO[42].0, 0x794D);
    }
}
