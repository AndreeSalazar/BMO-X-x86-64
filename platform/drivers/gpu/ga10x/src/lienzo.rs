//! **M5d L: EL LIENZO -- LA 3060 PINTA EN LA RAM DEL PC** -- tras el primer
//! sombreador (VISTO en el metal el 24-09 a las 16:21: 32 de 32 hilos, los
//! dos semaforos PAGADOS). Un programa de 128 x 128 hilos pinta un degradado
//! de 128 x 128 pixeles en 64 KiB de RAM del PC, prestados a la 3060 por la
//! IOMMU; la CPU lo lee y el escritorio lo muestra.
//!
//! capa: puro -- el programa, las PTE y lo que se espera; la RAM y los
//! registros los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- el SASS sale de `ptxas`/`nvdisasm` (como el primer
//!           sombreador); el QMD y las ordenes son los de `sombreador`
//!
//! # El programa (PTX de origen, `ptxas -arch=sm_86`: 8 registros)
//!
//! ```text
//!    x = tid.x (0..128)   y = ctaid.x (0..128)
//!    dir = 0x2_0001_0000 + ((y << 7) | x) * 4
//!    [dir] = x << 17 | y << 9 | (x + y)      -- 0x00RRGGBB: R 2x, G 2y, B x+y
//! ```
//!
//! Como el primero, las dos lecturas del bufer de constantes de CUDA (la pila
//! y `ULDC.64 UR4, c[0x0][0x118]`, que va DESPUES del ultimo uso de UR4/UR5)
//! quedan en NOP con sus bits de planificacion; `nvdisasm -b SM86` lo lee:
//! `NOP; S2UR UR5, SR_CTAID.X; S2R R4, SR_TID.X; USHF.L.U32 UR4, UR5, 0x7;
//! IMAD.SHL.U32 R3, R4, 0x20000; LOP3.LUT R0, R4, UR4; USHF.L.U32 UR4, UR5,
//! 0x9; IADD3 R4, R4, UR5; IMAD.SHL.U32 R0, R0, 0x4; LOP3.LUT R5, R4, UR4,
//! R3, 0xfe; IMAD.MOV.U32 R3, RZ, RZ, 0x2; NOP; LOP3.LUT R2, R0, 0x10000;
//! STG.E [R2.64], R5; EXIT; BRA 0xf0`.
//!
//! # Donde vive
//!
//! ```text
//!    RAM del PC   16 marcos seguidos, prestados ESCRIBIBLES en la IOVA
//!                 0x3A02_0000 (los metodos de copia y de GR van en 0x3A00_0000
//!                 y 0x3A01_0000)
//!    VA           0x2_0001_0000: las entradas 16..31 de la PT del tramo (la
//!                 0..15 son el tramo), PTE de SISTEMA coherente y VOL
//!    tramo        11 el programa (encima del primero: ya corrio), 13 el QMD,
//!                 14 los semaforos, 15 las ordenes; GPFIFO de GR, entrada 2
//! ```

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::sombreador::{self, ordenes_con, qmd_con, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::vram::{a_cero, escribir64, leer64, TRAMO_VA};
use crate::Registros;

/// Pixeles por lado.
pub const LADO: u32 = 128;
pub const PIXELES: usize = (LADO * LADO) as usize;
/// Paginas de 4 KiB: 128 x 128 x 4 B.
pub const PAGINAS: u64 = (PIXELES * 4 / 4096) as u64;
/// Donde lo ve la 3060 por la IOMMU.
pub const IOVA: u64 = 0x3A02_0000;
/// Donde lo ve la GPU: tras el tramo, en la misma PT.
pub const VA: u64 = TRAMO_VA + 0x1_0000;
/// La primera entrada de la PT del tramo que usa.
pub const PT_PRIMERA: usize = 16;

/// Lo que pinta el hilo `(x, y)`: 0x00RRGGBB.
pub const fn color(x: u32, y: u32) -> u32 {
    x << 17 | y << 9 | (x + y)
}

/// Lo que paga el QMD y lo que paga el informe.
pub const PAGA_QMD: u32 = 0x3060_1140;
pub const PAGA_FIN: u32 = 0x3060_11F0;
/// La entrada del GPFIFO de GR: la 2 (S3 la 0, el primer sombreador la 1).
pub const ENTRADA_GPFIFO: u64 = 2;

/// **El programa**, como lo leyo `nvdisasm` (ver arriba).
pub const CODIGO: [(u64, u64); 16] = [
    (0x0000_0000_0000_7918, 0x000F_E400_0000_0000), // NOP (era MOV R1, c[0x0][0x28])
    (0x0000_0000_0005_79C3, 0x000E_2200_0000_2500), // S2UR UR5, SR_CTAID.X
    (0x0000_0000_0004_7919, 0x000E_6200_0000_2100), // S2R R4, SR_TID.X
    (0x0000_0007_0504_7899, 0x001F_E200_0800_063F), // USHF.L.U32 UR4, UR5, 0x7, URZ
    (0x0002_0000_0403_7824, 0x002F_CA00_078E_00FF), // IMAD.SHL.U32 R3, R4, 0x20000, RZ
    (0x0000_0004_0400_7C12, 0x040F_E200_0F8E_FCFF), // LOP3.LUT R0, R4, UR4, RZ, 0xfc
    (0x0000_0009_0504_7899, 0x000F_E200_0800_063F), // USHF.L.U32 UR4, UR5, 0x9, URZ
    (0x0000_0005_0404_7C10, 0x000F_C600_0FFF_E0FF), // IADD3 R4, R4, UR5, RZ
    (0x0000_0004_0000_7824, 0x000F_E400_078E_00FF), // IMAD.SHL.U32 R0, R0, 0x4, RZ
    (0x0000_0004_0405_7C12, 0x000F_E200_0F8E_FE03), // LOP3.LUT R5, R4, UR4, R3, 0xfe
    (0x0000_0002_FF03_7424, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R3, RZ, RZ, 0x2
    (0x0000_0000_0000_7918, 0x000F_E200_0000_0000), // NOP (era ULDC.64 UR4, c[0x0][0x118])
    (0x0001_0000_0002_7812, 0x000F_CA00_078E_FCFF), // LOP3.LUT R2, R0, 0x10000, RZ, 0xfc
    (0x0000_0005_0200_7986, 0x000F_E200_0C10_1904), // STG.E [R2.64], R5
    (0x0000_0000_0000_794D, 0x000F_EA00_0380_0000), // EXIT
    (0xFFFF_FFF0_0000_7947, 0x000F_C000_0383_FFFF), // BRA 0xf0 (a si misma)
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

pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x20;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x30;

pub fn qmd() -> [u32; QMD_PALABRAS] {
    qmd_con(sombreador_va(PROGRAMA), LADO, LADO, sombreador_va(SEMAFORO_QMD), PAGA_QMD)
}

pub fn ordenes() -> [u32; ORDENES] {
    ordenes_con(sombreador_va(QMD), sombreador_va(SEMAFORO_FIN), PAGA_FIN)
}

const fn sombreador_va(vram: u64) -> u64 {
    crate::computo::va(vram)
}

/// Donde va la PTE `k` del lienzo.
pub const fn pte_en(k: u64) -> u64 {
    crate::vram::TABLAS[3] + 8 * (PT_PRIMERA as u64 + k)
}

/// **Mapear el lienzo**: las 16 PTE de SISTEMA, solo si estaban VACIAS (no se
/// pisa nada), releidas. `(escritas, releidas iguales)`.
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

/// **Preparar**: los semaforos del lienzo a cero, el programa (encima del
/// primero), el QMD, las ordenes y la entrada 2 del GPFIFO, todo RELEIDO.
pub fn preparar<R: Registros>(r: &mut R) -> bool {
    let c = codigo();
    let q = qmd();
    let o = ordenes();
    let e = entrada(sombreador_va(EMPUJE), ORDENES as u32);
    let pag = crate::vram::PALABRAS;
    escribir(r, SEMAFORO_QMD, &[0, 0, 0, 0, 0, 0, 0, 0]) == 8
        && a_cero(r, PROGRAMA) as usize == pag
        && a_cero(r, QMD) as usize == pag
        && escribir(r, PROGRAMA, &c) == PALABRAS_CODIGO
        && escribir(r, QMD, &q) == QMD_PALABRAS
        && escribir(r, EMPUJE, &o) == ORDENES
        && escribir(r, GR.gpfifo + 8 * ENTRADA_GPFIFO, &[e as u32, (e >> 32) as u32]) == 2
}

/// **Lanzar**: la MMU invalidada, GP_PUT = 3 y la ficha en el timbre.
pub fn lanzar<R: Registros>(r: &mut R, ficha: u32) -> bool {
    let puesto = invalidar(r) && escribir(r, GR.userd + GP_PUT, &[ENTRADA_GPFIFO as u32 + 1]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

/// `(GP_GET, semaforo del QMD, semaforo de informe)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, SEMAFORO_QMD), leer32(r, SEMAFORO_FIN))
}

/// **Comprobar lo pintado** (los pixeles, en la RAM del PC): cuantos son los
/// que tocan.
pub fn comprobar(pixeles: &[u32]) -> u32 {
    pixeles
        .iter()
        .take(PIXELES)
        .enumerate()
        .filter(|&(k, &p)| p == color(k as u32 % LADO, k as u32 / LADO))
        .count() as u32
}

/// `buenos | qmd << 15 | fin << 16 | lanzado << 17 | GP_GET << 24 | us << 32`.
pub const fn empaquetar(buenos: u32, qmd: bool, fin: bool, lanzado: bool, gp_get: u32, us: u32) -> u64 {
    (buenos as u64 & 0x7FFF) | (qmd as u64) << 15 | (fin as u64) << 16 | (lanzado as u64) << 17 | (gp_get as u64 & 0xFF) << 24 | (us as u64) << 32
}

/// `(buenos, qmd, fin, lanzado, GP_GET, us)`. 16384 no cabe en 15 bits: el
/// kernel manda `buenos` 16384 como 0x4000, que SI cabe (0x7FFF es 32767).
pub const fn desempaquetar(v: u64) -> (u32, bool, bool, bool, u32, u32) {
    ((v & 0x7FFF) as u32, v >> 15 & 1 != 0, v >> 16 & 1 != 0, v >> 17 & 1 != 0, (v >> 24 & 0xFF) as u32, (v >> 32) as u32)
}

/// El lienzo salio entero.
pub const fn sano(v: u64) -> bool {
    let (buenos, qmd, _, lanzado, _, _) = desempaquetar(v);
    buenos as usize == PIXELES && qmd && lanzado
}

/// Que no choca con nada de lo que ya usa el sombreador.
const _: () = assert!(SEMAFORO_QMD != sombreador::SEMAFORO_QMD && SEMAFORO_FIN != sombreador::SEMAFORO_FIN);

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_lienzo_cabe_tras_el_tramo() {
        assert_eq!(PAGINAS, 16);
        assert_eq!(VA, 0x2_0001_0000);
        // Las entradas 16..31 de la misma PT: ni el tramo (0..15) ni fuera.
        assert_eq!(crate::mmu::indices(VA)[4], PT_PRIMERA);
        assert_eq!(crate::mmu::indices(VA)[..4], crate::mmu::indices(TRAMO_VA)[..4]);
        assert!(PT_PRIMERA as u64 + PAGINAS <= 512);
        // Y el programa lleva dentro esa VA: 0x10000 en el LOP3 y 2 arriba.
        assert_eq!(CODIGO[12].0 >> 32, VA & 0xFFFF_FFFF);
        assert_eq!(CODIGO[10].0 >> 32, VA >> 32);
    }

    #[test]
    fn la_pte_de_sistema() {
        let v = crate::mmu::pte_sistema(IOVA);
        assert_eq!(v & 0xF, 1 | 2 << 1 | 1 << 3);
        assert_eq!((v >> 8) << 12, IOVA);
    }

    #[test]
    fn el_programa_es_el_de_nvdisasm() {
        let c = codigo();
        assert_eq!(c.len(), 64);
        assert_eq!(CODIGO[13], (0x0000_0005_0200_7986, 0x000F_E200_0C10_1904));
        assert_eq!(CODIGO[13].1 >> (101 - 64) & 1, 0);
        for k in [0, 11] {
            assert_eq!(CODIGO[k].0, 0x7918);
            assert_eq!(CODIGO[k].1 & ((1 << 41) - 1), 0);
        }
    }

    #[test]
    fn el_qmd_es_de_128_por_128() {
        let q = qmd();
        let f = |hi, lo| crate::sombreador::leer_campo(&q, hi, lo);
        assert_eq!((f(415, 384), f(607, 592)), (128, 128));
        assert_eq!(f(1567, 1536) | f(1584, 1568) << 32, sombreador_va(PROGRAMA));
        assert_eq!(f(863, 832), PAGA_QMD as u64);
        // Y el primer sombreador sigue igual.
        let q1 = crate::sombreador::qmd();
        assert_eq!(crate::sombreador::leer_campo(&q1, 415, 384), 1);
        assert_eq!(crate::sombreador::leer_campo(&q1, 607, 592), 32);
    }

    #[test]
    fn el_degradado_se_comprueba() {
        let mut p = [0u32; PIXELES];
        assert_eq!(comprobar(&p), 1); // solo (0, 0) es negro
        for (k, x) in p.iter_mut().enumerate() {
            *x = color(k as u32 % LADO, k as u32 / LADO);
        }
        assert_eq!(comprobar(&p), PIXELES as u32);
        assert_eq!(color(127, 127), 0x00FE_FEFE);
        let v = empaquetar(PIXELES as u32, true, true, true, 3, 40);
        assert_eq!(desempaquetar(v), (16384, true, true, true, 3, 40));
        assert!(sano(v));
    }
}
