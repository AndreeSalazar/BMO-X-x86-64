//! **M5d B: EL BLUR -- LA 3060 DESENFOCA UN TROZO DE TU PANTALLA** -- tras el
//! lienzo. El escritorio sube 128 x 128 pixeles al lienzo (`lienzo::VA`); un
//! programa de 128 x 128 hilos hace, cada uno, la media de los 7 x 7 de su
//! alrededor (con los bordes repetidos) y la escribe en OTRO lienzo de 64 KiB
//! (`VA`). El kernel lo comprueba pixel a pixel contra la MISMA cuenta en la
//! CPU ([`desenfocar`]) y el escritorio muestra el antes y el despues.
//!
//! capa: puro -- el programa, las PTE, la cuenta de referencia; la RAM y los
//! registros los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- el SASS de `ptxas` (con los bucles SIN desenrollar:
//!           desenrollados eran 8.8 KiB y no caben en una pagina del tramo),
//!           comprobado con `nvdisasm`; la division entre 49 que emite es
//!           EXACTA (la reciproca y dos correcciones), como `/` en la CPU
//!
//! # El programa (PTX de origen, `ptxas -arch=sm_86`: 16 registros)
//!
//! ```text
//!    x = tid.x   y = ctaid.x   (r, g, b) = 0
//!    para dy en -3..=3, dx en -3..=3:        (.pragma "nounroll")
//!       p = lienzo[clamp(y+dy, 0, 127)][clamp(x+dx, 0, 127)]
//!       r += p >> 16 & 0xFF;  g += p >> 8 & 0xFF;  b += p & 0xFF
//!    salida[y][x] = (r/49) << 16 | (g/49) << 8 | b/49
//! ```
//!
//! Como en los anteriores, `c[0x0][0x28]` y `ULDC.64 UR6, c[0x0][0x118]`
//! quedan en NOP; el segundo CONSERVA su espera de la barrera 0 (la de S2UR) y
//! no pone ninguna: la planificacion es la de `ptxas`. Ni el LDG ni el STG usan
//! descriptor (bit 101 a 0).
//!
//! # Donde vive
//!
//! ```text
//!    entrada  el lienzo (IOVA 0x3A02_0000, VA 0x2_0001_0000)
//!    salida   16 marcos mas, IOVA 0x3A03_0000, VA 0x2_0002_0000: las
//!             entradas 32..47 de la PT del tramo
//!    tramo    11 el programa, 13 el QMD, 14 los semaforos (+0x40, +0x50),
//!             15 las ordenes; GPFIFO de GR, la entrada que diga el kernel
//!             (3 la primera vez, y una mas cada vez)
//! ```

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::lienzo::{sombreador_va, LADO, PAGINAS, PIXELES};
use crate::sombreador::{ordenes_con, qmd_con, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::vram::{a_cero, escribir64, leer64, TRAMO_VA};
use crate::Registros;

/// El radio: 7 x 7.
pub const RADIO: i32 = 3;
pub const MUESTRAS: u32 = ((2 * RADIO + 1) * (2 * RADIO + 1)) as u32;
/// La salida: donde la ve la 3060 por la IOMMU, y donde la ve la GPU.
pub const IOVA: u64 = 0x3A03_0000;
pub const VA: u64 = TRAMO_VA + 0x2_0000;
pub const PT_PRIMERA: usize = 32;
/// Registros por hilo: los 16 de `ptxas`, con margen.
pub const REGISTROS: u32 = 32;
pub const PAGA_QMD: u32 = 0x3060_B1C0;
pub const PAGA_FIN: u32 = 0x3060_B1F0;
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x40;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x50;
/// La primera entrada del GPFIFO de GR que usa (S3 0, el sombreador 1, el
/// lienzo 2), y la ultima que se deja usar.
pub const PRIMERA_ENTRADA: u32 = 3;
pub const ULTIMA_ENTRADA: u32 = crate::canal::GPFIFO_ENTRADAS - 1;

/// **La cuenta de referencia**: lo que tiene que salir en `(x, y)`.
pub fn desenfocar(src: &[u32], x: u32, y: u32) -> u32 {
    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
    for dy in -RADIO..=RADIO {
        let yy = (y as i32 + dy).clamp(0, LADO as i32 - 1) as u32;
        for dx in -RADIO..=RADIO {
            let xx = (x as i32 + dx).clamp(0, LADO as i32 - 1) as u32;
            let p = src[(yy * LADO + xx) as usize];
            r += p >> 16 & 0xFF;
            g += p >> 8 & 0xFF;
            b += p & 0xFF;
        }
    }
    (r / MUESTRAS) << 16 | (g / MUESTRAS) << 8 | b / MUESTRAS
}

/// **Comprobar**: cuantos pixeles de `salida` son los de la cuenta.
pub fn comprobar(src: &[u32], salida: &[u32]) -> u32 {
    if src.len() < PIXELES || salida.len() < PIXELES {
        return 0;
    }
    (0..PIXELES as u32).filter(|&k| salida[k as usize] == desenfocar(src, k % LADO, k / LADO)).count() as u32
}

/// **El programa**, como lo leyo `nvdisasm` (ver arriba).
pub const CODIGO: [(u64, u64); 71] = [
    (0x0000_0000_0000_7918, 0x000F_E400_0000_0000), // NOP (era IMAD.MOV.U32 R1, RZ, RZ, c[0x0][0x28])
    (0x0000_0000_0004_79C3, 0x000E_2200_0000_2500), // S2UR UR4, SR_CTAID.X
    (0x0000_0000_000B_7919, 0x000E_6200_0000_2100), // S2R R11, SR_TID.X
    (0x0000_00FF_FF00_7224, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R0, RZ, RZ, RZ
    (0x0000_0000_0004_7805, 0x000F_E200_0001_FF00), // CS2R R4, SRZ
    (0xFFFF_FFFD_FF0A_7424, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R10, RZ, RZ, -0x3
    (0x0000_0000_0000_7918, 0x001F_C800_0000_0000), // NOP (era ULDC.64 UR6, c[0x0][0x118])
    (0x0000_0004_0A02_7C10, 0x040F_E200_0FFF_E0FF), // IADD3 R2, R10.reuse, UR4, RZ
    (0xFFFF_FFFD_FF06_7424, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R6, RZ, RZ, -0x3
    (0x0000_0001_0A0A_7810, 0x000F_E400_07FF_E0FF), // IADD3 R10, R10, 0x1, RZ
    (0x0000_0002_FF02_7217, 0x000F_E400_0780_0200), // IMNMX R2, RZ, R2, !PT
    (0x0000_0003_0A00_780C, 0x000F_C400_03F2_4270), // ISETP.GT.AND P1, PT, R10, 0x3, PT
    (0x0000_007F_0202_7817, 0x000F_CA00_0380_0200), // IMNMX R2, R2, 0x7f, PT
    (0x0000_0080_020D_7824, 0x000F_E400_078E_00FF), // IMAD.SHL.U32 R13, R2, 0x80, RZ
    (0x0000_0001_0B02_7824, 0x002F_E400_078E_0206), // IMAD.IADD R2, R11, 0x1, R6
    (0x0000_0002_FF03_7424, 0x000F_C600_078E_00FF), // IMAD.MOV.U32 R3, RZ, RZ, 0x2
    (0x0000_0002_FF02_7217, 0x000F_C800_0780_0200), // IMNMX R2, RZ, R2, !PT
    (0x0000_007F_0202_7817, 0x000F_C800_0380_0200), // IMNMX R2, R2, 0x7f, PT
    (0x0000_0002_0D02_7212, 0x000F_CA00_078E_FCFF), // LOP3.LUT R2, R13, R2, RZ, 0xfc, !PT
    (0x0000_0004_0202_7824, 0x000F_CA00_078E_00FF), // IMAD.SHL.U32 R2, R2, 0x4, RZ
    (0x0001_0000_0202_7812, 0x000F_CC00_078E_FCFF), // LOP3.LUT R2, R2, 0x10000, RZ, 0xfc, !PT
    (0x0000_0006_0202_7981, 0x000E_A200_0C1E_1900), // LDG.E R2, [R2.64]
    (0x0000_0001_0606_7810, 0x000F_C800_07FF_E0FF), // IADD3 R6, R6, 0x1, RZ
    (0x0000_0003_0600_780C, 0x000F_E400_03F0_4270), // ISETP.GT.AND P0, PT, R6, 0x3, PT
    (0x0000_00FF_0208_7812, 0x044F_E400_078E_C0FF), // LOP3.LUT R8, R2.reuse, 0xff, RZ, 0xc0, !PT
    (0x0000_7771_0207_7816, 0x040F_E400_0000_00FF), // PRMT R7, R2.reuse, 0x7771, RZ
    (0x0000_7772_0209_7816, 0x000F_E200_0000_00FF), // PRMT R9, R2, 0x7772, RZ
    (0x0000_0001_0805_7824, 0x000F_E400_078E_0205), // IMAD.IADD R5, R8, 0x1, R5
    (0x0000_0001_0704_7824, 0x000F_E400_078E_0204), // IMAD.IADD R4, R7, 0x1, R4
    (0x0000_0001_0900_7824, 0x000F_C400_078E_0200), // IMAD.IADD R0, R9, 0x1, R0
    (0xFFFF_FEF0_0000_8947, 0x000F_EA00_0383_FFFF), // @!P0 BRA 0xe0
    (0xFFFF_FE70_0000_9947, 0x000F_EA00_0383_FFFF), // @!P1 BRA 0x70
    (0x0000_0031_0006_7906, 0x000E_2200_0020_9000), // I2F.U32.RP R6, 0x31
    (0x0000_0007_0404_7899, 0x000F_CE00_0800_063F), // USHF.L.U32 UR4, UR4, 0x7, URZ
    (0x0000_0006_0006_7308, 0x001E_2400_0000_1000), // MUFU.RCP R6, R6
    (0x0FFF_FFFE_0602_7810, 0x001F_CC00_07FF_E0FF), // IADD3 R2, R6, 0xffffffe, RZ
    (0x0000_0002_0003_7305, 0x0000_6400_0021_F000), // F2I.FTZ.U32.TRUNC.NTZ R3, R2
    (0x0000_00FF_FF02_7224, 0x001F_E400_078E_00FF), // IMAD.MOV.U32 R2, RZ, RZ, RZ
    (0xFFFF_FFCF_0307_7824, 0x002F_C800_078E_02FF), // IMAD R7, R3, -0x31, RZ
    (0x0000_0007_0303_7227, 0x000F_CC00_078E_0002), // IMAD.HI.U32 R3, R3, R7, R2
    (0x0000_0000_0307_7227, 0x000F_C800_078E_00FF), // IMAD.HI.U32 R7, R3, R0, RZ
    (0x0000_0004_0309_7227, 0x000F_C800_078E_00FF), // IMAD.HI.U32 R9, R3, R4, RZ
    (0xFFFF_FFCF_0700_7824, 0x000F_E400_078E_0200), // IMAD R0, R7, -0x31, R0
    (0xFFFF_FFCF_0904_7824, 0x000F_E400_078E_0204), // IMAD R4, R9, -0x31, R4
    (0x0000_0005_0308_7227, 0x000F_E200_078E_00FF), // IMAD.HI.U32 R8, R3, R5, RZ
    (0x0000_0031_0000_780C, 0x000F_E400_03F0_6070), // ISETP.GE.U32.AND P0, PT, R0, 0x31, PT
    (0x0000_0031_0400_780C, 0x000F_E200_03F4_6070), // ISETP.GE.U32.AND P2, PT, R4, 0x31, PT
    (0xFFFF_FFCF_0805_7824, 0x000F_E400_078E_0205), // IMAD R5, R8, -0x31, R5
    (0x0000_0002_FF03_7424, 0x000F_C600_078E_00FF), // IMAD.MOV.U32 R3, RZ, RZ, 0x2
    (0x0000_0031_0500_780C, 0x000F_CA00_03F8_6070), // ISETP.GE.U32.AND P4, PT, R5, 0x31, PT
    (0xFFFF_FFCF_0000_0810, 0x000F_E400_07FF_E0FF), // @P0 IADD3 R0, R0, -0x31, RZ
    (0xFFFF_FFCF_0404_2810, 0x000F_E400_07FF_E0FF), // @P2 IADD3 R4, R4, -0x31, RZ
    (0x0000_0031_0000_780C, 0x000F_E400_03F2_6070), // ISETP.GE.U32.AND P1, PT, R0, 0x31, PT
    (0x0000_0031_0400_780C, 0x000F_E400_03F6_6070), // ISETP.GE.U32.AND P3, PT, R4, 0x31, PT
    (0xFFFF_FFCF_0505_4810, 0x000F_E400_07FF_E0FF), // @P4 IADD3 R5, R5, -0x31, RZ
    (0x0000_0001_0707_0810, 0x000F_C400_07FF_E0FF), // @P0 IADD3 R7, R7, 0x1, RZ
    (0x0000_0031_0500_780C, 0x000F_E400_03FA_6070), // ISETP.GE.U32.AND P5, PT, R5, 0x31, PT
    (0x0000_0001_0909_2810, 0x000F_E400_07FF_E0FF), // @P2 IADD3 R9, R9, 0x1, RZ
    (0x0000_0004_0B00_7C12, 0x000F_E400_0F8E_FCFF), // LOP3.LUT R0, R11, UR4, RZ, 0xfc, !PT
    (0x0000_0001_0707_1810, 0x000F_E400_07FF_E0FF), // @P1 IADD3 R7, R7, 0x1, RZ
    (0x0000_0001_0909_3810, 0x000F_E200_07FF_E0FF), // @P3 IADD3 R9, R9, 0x1, RZ
    (0x0000_0004_0000_7824, 0x000F_E200_078E_00FF), // IMAD.SHL.U32 R0, R0, 0x4, RZ
    (0x0000_0001_0808_4810, 0x000F_E200_07FF_E0FF), // @P4 IADD3 R8, R8, 0x1, RZ
    (0x0001_0000_0707_7824, 0x000F_C400_078E_00FF), // IMAD.U32 R7, R7, 0x10000, RZ
    (0x0000_0100_0909_7824, 0x000F_E200_078E_00FF), // IMAD.SHL.U32 R9, R9, 0x100, RZ
    (0x0000_0001_0808_5810, 0x000F_E400_07FF_E0FF), // @P5 IADD3 R8, R8, 0x1, RZ
    (0x0002_0000_0002_7812, 0x000F_E400_078E_FCFF), // LOP3.LUT R2, R0, 0x20000, RZ, 0xfc, !PT
    (0x0000_0007_0807_7212, 0x000F_CA00_078E_FE09), // LOP3.LUT R7, R8, R7, R9, 0xfe, !PT
    (0x0000_0007_0200_7986, 0x000F_E200_0C10_1906), // STG.E [R2.64], R7
    (0x0000_0000_0000_794D, 0x000F_EA00_0380_0000), // EXIT
    (0xFFFF_FFF0_0000_7947, 0x000F_C000_0383_FFFF), // BRA 0x460 (a si misma)
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

/// Donde va la PTE `k` de la salida.
pub const fn pte_en(k: u64) -> u64 {
    crate::vram::TABLAS[3] + 8 * (PT_PRIMERA as u64 + k)
}

/// **Mapear la salida**: 16 PTE de SISTEMA, solo si estaban VACIAS, releidas.
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

/// Una entrada del GPFIFO valida para los trabajos que van rotando.
///
/// ** 24-09: el anillo DA LA VUELTA. Antes se paraba en la 511 ("el GPFIFO
/// ya se gasto"): con `gpu giro` a 32 entradas por vuelta, eran ~14 vueltas
/// por arranque. Las entradas 0..2 (S3, el sombreador y el lienzo) solo se
/// usan UNA vez por arranque y ANTES que todas estas (el kernel lo guarda
/// con sus `*_HECHO`), asi que pisarlas al dar la vuelta no repite nada: el
/// GP_GET ya paso por ellas.
pub const fn entrada_valida(e: u32) -> bool {
    e <= ULTIMA_ENTRADA
}

/// **La entrada que va detras de `e`**, y el GP_PUT tras escribir `e`: al
/// final del anillo, la 0.
pub const fn siguiente(e: u32) -> u32 {
    (e + 1) % crate::canal::GPFIFO_ENTRADAS
}

/// **Preparar** con la entrada `e` del GPFIFO: sus semaforos a cero, el
/// programa, el QMD y las ordenes, todo RELEIDO.
pub fn preparar<R: Registros>(r: &mut R, e: u32) -> bool {
    if !entrada_valida(e) {
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

/// **Lanzar**: la MMU invalidada, GP_PUT = `e` + 1 y la ficha en el timbre.
pub fn lanzar<R: Registros>(r: &mut R, ficha: u32, e: u32) -> bool {
    let puesto = entrada_valida(e) && invalidar(r) && escribir(r, GR.userd + GP_PUT, &[siguiente(e)]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

/// `(GP_GET, semaforo del QMD, semaforo de informe)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, SEMAFORO_QMD), leer32(r, SEMAFORO_FIN))
}

/// Como `lienzo::empaquetar`, con la entrada del GPFIFO arriba.
pub const fn empaquetar(buenos: u32, qmd: bool, fin: bool, lanzado: bool, gp_get: u32, us: u32) -> u64 {
    crate::lienzo::empaquetar(buenos, qmd, fin, lanzado, gp_get, us)
}

pub const fn desempaquetar(v: u64) -> (u32, bool, bool, bool, u32, u32) {
    crate::lienzo::desempaquetar(v)
}

pub const fn sano(v: u64) -> bool {
    crate::lienzo::sano(v)
}

/// Un pixel del escritorio y su lugar, en un argumento: `k` (el par, 0..8192)
/// en 61..48 y los dos pixeles de 24 bits abajo.
pub const fn subir(k: u32, p0: u32, p1: u32) -> u64 {
    (k as u64 & 0x1FFF) << 48 | (p1 as u64 & 0xFF_FFFF) << 24 | (p0 as u64 & 0xFF_FFFF)
}

/// `(k, p0, p1)`.
pub const fn bajar(v: u64) -> (u32, u32, u32) {
    ((v >> 48 & 0x1FFF) as u32, (v & 0xFF_FFFF) as u32, (v >> 24 & 0xFF_FFFF) as u32)
}

const _: () = assert!(SEMAFORO_QMD != crate::lienzo::SEMAFORO_QMD && SEMAFORO_FIN != crate::lienzo::SEMAFORO_FIN);
const _: () = assert!(PALABRAS_CODIGO * 4 <= 4096);

#[cfg(test)]
mod pruebas {
    use super::*;
    extern crate std;

    #[test]
    fn la_salida_cabe_tras_el_lienzo() {
        assert_eq!(VA, 0x2_0002_0000);
        assert_eq!(crate::mmu::indices(VA)[4], PT_PRIMERA);
        assert_eq!(crate::mmu::indices(VA)[..4], crate::mmu::indices(TRAMO_VA)[..4]);
        assert_eq!(PT_PRIMERA as u64, crate::lienzo::PT_PRIMERA as u64 + PAGINAS);
        assert!(IOVA >= crate::lienzo::IOVA + PAGINAS * 4096);
    }

    #[test]
    fn el_programa_lee_del_lienzo_y_escribe_en_la_salida() {
        // LOP3.LUT R2, R2, 0x10000 (el lienzo) y LOP3.LUT R2, R0, 0x20000 (la salida).
        assert!(CODIGO.iter().any(|&(lo, _)| lo == 0x0001_0000_0202_7812));
        assert!(CODIGO.iter().any(|&(lo, _)| lo >> 32 == VA & 0xFFFF_FFFF && lo & 0xFFFF == 0x7812));
        assert_eq!(crate::lienzo::VA & 0xFFFF_FFFF, 0x1_0000);
        // Ningun LDG/STG con descriptor; los dos NOP, sin barrera propia.
        for &(lo, hi) in &CODIGO {
            if lo & 0xFFF == 0x981 || lo & 0xFFF == 0x986 {
                assert_eq!(hi >> (101 - 64) & 1, 0);
            }
            if lo == 0x7918 {
                assert_eq!(hi >> (41 + 5) & 7, 7);
            }
        }
        // Termina en EXIT y BRA a si misma.
        assert_eq!(CODIGO[69].0, 0x794D);
        assert_eq!(CODIGO[70].0 & 0xFFFF, 0x7947);
    }

    #[test]
    fn la_cuenta_de_referencia() {
        let mut src = std::vec![0u32; PIXELES];
        // Uniforme: sale igual.
        src.iter_mut().for_each(|p| *p = 0x0040_8020);
        assert_eq!(desenfocar(&src, 0, 0), 0x0040_8020);
        assert_eq!(desenfocar(&src, 64, 64), 0x0040_8020);
        // Un punto blanco: se reparte entre 49.
        src.iter_mut().for_each(|p| *p = 0);
        src[(64 * LADO + 64) as usize] = 0x00FF_FFFF;
        assert_eq!(desenfocar(&src, 64, 64), 0x0005_0505);
        assert_eq!(desenfocar(&src, 67, 61), 0x0005_0505);
        assert_eq!(desenfocar(&src, 68, 64), 0);
        let mut sal = std::vec![0u32; PIXELES];
        for k in 0..PIXELES as u32 {
            sal[k as usize] = desenfocar(&src, k % LADO, k / LADO);
        }
        assert_eq!(comprobar(&src, &sal), PIXELES as u32);
        sal[5] ^= 1;
        assert_eq!(comprobar(&src, &sal), PIXELES as u32 - 1);
    }

    #[test]
    fn el_qmd_y_las_entradas() {
        let q = qmd();
        let f = |hi, lo| crate::sombreador::leer_campo(&q, hi, lo);
        assert_eq!((f(415, 384), f(607, 592), f(656, 648)), (128, 128, 32));
        assert!(entrada_valida(3) && entrada_valida(511) && !entrada_valida(512));
        // El anillo da la vuelta: tras la 511, la 0.
        assert_eq!((siguiente(3), siguiente(510), siguiente(511)), (4, 511, 0));
        assert_eq!(bajar(subir(8191, 0x00AB_CDEF, 0x0012_3456)), (8191, 0x00AB_CDEF, 0x0012_3456));
    }
}
