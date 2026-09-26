//! **M5d S4..S6: EL PRIMER SOMBREADOR DE BMO-X EN LA 3060** -- un programa de
//! computo de 32 hilos que escribe, cada uno, `0x3060_0000 + su id` en su
//! palabra del tramo. Tras el primer trabajo del GR (S3, VISTO en el metal el
//! 24-09 a las 16:06: el semaforo de informe PAGADO en 45 us).
//!
//! capa: puro -- el programa, el QMD y las ordenes; los registros los toca el
//! kernel con un `Registros` (L8)
//!
//! [eje]     CORRECCION -- cada bit del programa sale del ENSAMBLADOR de NVIDIA
//!           y se comprobo con SU desensamblador; el QMD, de `clc7c0qmd.h`
//!
//! # El programa (SASS de SM86, 16 B por instruccion)
//!
//! Salio de este PTX, con `ptxas -arch=sm_86` (CUDA 12.9: "Used 8 registers,
//! used 0 barriers"):
//!
//! ```text
//!    mov.u32 %r1, %tid.x;           shl.b32 %r3, %r1, 2;
//!    or.b32  %r4, %r3, 0xA000;      mov.b64 %rd2, {%r4, 2};
//!    add.s32 %r2, %r1, 0x30600000;  st.global.u32 [%rd2], %r2;   ret;
//! ```
//!
//! y se tocaron DOS instrucciones: `MOV R1, c[0x0][0x28]` (la pila de CUDA)
//! y `ULDC.64 UR4, c[0x0][0x118]` (el descriptor de memoria de CUDA) leen el
//! bufer de constantes 0, que llena el driver de CUDA y aqui no existe. Las
//! dos quedaron en NOP con SUS bits de planificacion, y el desensamblador de
//! NVIDIA (`nvdisasm -b SM86`, 13.4) lo lee asi:
//!
//! ```text
//!    NOP ; S2R R5, SR_TID.X ; IMAD.MOV.U32 R3, RZ, RZ, 0x2 ; NOP ;
//!    SHF.L.U32 R2, R5, 0x2, RZ ; IADD3 R5, R5, 0x30600000, RZ ;
//!    LOP3.LUT R2, R2, 0xa000, RZ, 0xfc, !PT ; STG.E [R2.64], R5 ;
//!    EXIT ; BRA 0x90
//! ```
//!
//! Ni R1 ni UR4 se usan despues: el STG no lleva descriptor (el bit 101 de su
//! codificacion, el que lo pondria, esta a 0 -- comprobado cambiandolo y
//! desensamblando: sale `desc[UR4]`).
//!
//! # El lanzamiento
//!
//! Un QMD V03_00 (256 B, `clc7c0qmd.h`): 1 x 1 x 1 bloques de 32 x 1 x 1
//! hilos, el programa en su VA, sin memoria compartida ni local, las caches
//! invalidadas, y el semaforo RELEASE0 que la 3060 escribe AL ACABAR la
//! rejilla. Las ordenes (subcanal 1): las ventanas compartida y local lejos
//! de todo, `SEND_PCAS_A` (el QMD >> 8), `SEND_SIGNALING_PCAS2_B` (3,
//! INVALIDATE_COPY_SCHEDULE), `WAIT_FOR_IDLE` y un semaforo de informe.
//!
//! # Donde vive (el tramo)
//!
//! ```text
//!    10 lo que ESCRIBE el sombreador   11 el programa   13 el QMD
//!    14 los dos semaforos             15 las ordenes   GPFIFO GR, entrada 1
//! ```

use crate::canal::GR;
use crate::computo::{self, va, AMPERE_COMPUTE_B, SUBCANAL};
use crate::copia::{cabecera_en, entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::vram::{a_cero, ventana, TRAMO, VENTANA, VENTANA_REG};
use crate::Registros;

const PAGINA: u64 = 0x1000;
pub const SALIDA: u64 = TRAMO + 10 * PAGINA;
pub const PROGRAMA: u64 = TRAMO + 11 * PAGINA;
pub const QMD: u64 = TRAMO + 13 * PAGINA;
pub const SEMAFOROS: u64 = TRAMO + 14 * PAGINA;
pub const EMPUJE: u64 = TRAMO + 15 * PAGINA;
/// El semaforo del QMD (al acabar la rejilla) y el de informe (tras el
/// `WAIT_FOR_IDLE`).
pub const SEMAFORO_QMD: u64 = SEMAFOROS;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x10;

/// Hilos: un bloque de 32 (un warp).
pub const HILOS: u32 = 32;
/// Lo que escribe el hilo `k`.
pub const fn valor(k: u32) -> u32 {
    0x3060_0000 + k
}
/// Lo que paga el QMD y lo que paga el informe.
pub const PAGA_QMD: u32 = 0x3060_5AD0;
pub const PAGA_FIN: u32 = 0x3060_F1E0;

/// **El programa**: `(bajo, alto)` de cada instruccion, como lo leyo
/// `nvdisasm` (ver arriba).
pub const CODIGO: [(u64, u64); 10] = [
    (0x0000_0000_0000_7918, 0x000F_E400_0000_0000), // NOP (era MOV R1, c[0x0][0x28])
    (0x0000_0000_0005_7919, 0x000E_2200_0000_2100), // S2R R5, SR_TID.X
    (0x0000_0002_FF03_7424, 0x000F_E200_078E_00FF), // IMAD.MOV.U32 R3, RZ, RZ, 0x2
    (0x0000_0000_0000_7918, 0x000F_E200_0000_0000), // NOP (era ULDC.64 UR4, c[0x0][0x118])
    (0x0000_0002_0502_7819, 0x041F_E400_0000_06FF), // SHF.L.U32 R2, R5, 0x2, RZ
    (0x3060_0000_0505_7810, 0x000F_E400_07FF_E0FF), // IADD3 R5, R5, 0x30600000, RZ
    (0x0000_A000_0202_7812, 0x000F_CA00_078E_FCFF), // LOP3.LUT R2, R2, 0xa000, RZ, 0xfc, !PT
    (0x0000_0005_0200_7986, 0x000F_E200_0C10_1904), // STG.E [R2.64], R5
    (0x0000_0000_0000_794D, 0x000F_EA00_0380_0000), // EXIT
    (0xFFFF_FFF0_0000_7947, 0x000F_C000_0383_FFFF), // BRA 0x90 (a si misma)
];
/// El programa en palabras de 32 bits, como va a la VRAM.
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

/// Registros por hilo que se reservan: los 8 de `ptxas`, con margen.
pub const REGISTROS: u32 = 16;

/// Palabras del QMD.
pub const QMD_PALABRAS: usize = 64;

/// Poner `v` en los bits `lo..=hi` del QMD (los `MW(hi:lo)` de NVIDIA).
pub fn campo(q: &mut [u32; QMD_PALABRAS], hi: u32, lo: u32, v: u64) {
    for b in lo..=hi {
        let bit = (v >> (b - lo)) & 1;
        let (w, s) = ((b / 32) as usize, b % 32);
        q[w] = q[w] & !(1 << s) | (bit as u32) << s;
    }
}

/// Leer los bits `lo..=hi` del QMD.
pub fn leer_campo(q: &[u32; QMD_PALABRAS], hi: u32, lo: u32) -> u64 {
    (lo..=hi).fold(0u64, |v, b| v | (((q[(b / 32) as usize] >> (b % 32)) & 1) as u64) << (b - lo))
}

/// **El QMD V03_00** del primer sombreador.
pub fn qmd() -> [u32; QMD_PALABRAS] {
    qmd_con(va(PROGRAMA), HILOS, 1, va(SEMAFORO_QMD), PAGA_QMD, REGISTROS)
}

/// **Un QMD V03_00**, campo a campo (`NVC7C0_QMDV03_00_*`): `bloques` x 1 x 1
/// de `hilos` x 1 x 1, el programa en `programa` y RELEASE0 con `paga` en
/// `sem` al acabar la rejilla.
pub fn qmd_con(programa: u64, hilos: u32, bloques: u32, sem: u64, paga: u32, registros: u32) -> [u32; QMD_PALABRAS] {
    qmd_rejilla(programa, hilos, bloques, 1, sem, paga, registros)
}

/// **Un QMD V03_00 con rejilla de DOS dimensiones**: `bloques` x `filas` x 1
/// (M5d P: la pantalla entera, una fila de bloques por linea).
pub fn qmd_rejilla(programa: u64, hilos: u32, bloques: u32, filas: u32, sem: u64, paga: u32, registros: u32) -> [u32; QMD_PALABRAS] {
    let mut q = [0u32; QMD_PALABRAS];
    campo(&mut q, 133, 128, 0x3F); // QMD_GROUP_ID (el de NVK)
    campo(&mut q, 134, 134, 1); // SM_GLOBAL_CACHING_ENABLE
    for b in 186..=191 {
        campo(&mut q, b, b, 1); // INVALIDATE_* (texturas, datos, instrucciones, constantes)
    }
    campo(&mut q, 369, 368, 1); // CWD_MEMBAR_TYPE L1_SYSMEMBAR
    campo(&mut q, 378, 378, 1); // API_VISIBLE_CALL_LIMIT NO_CHECK
    campo(&mut q, 415, 384, bloques as u64); // CTA_RASTER_WIDTH
    campo(&mut q, 431, 416, filas as u64); // CTA_RASTER_HEIGHT
    campo(&mut q, 463, 448, 1); // CTA_RASTER_DEPTH
    // Memoria compartida: 0 B; la config de la SM, 8 KiB (8K / 4K + 1).
    campo(&mut q, 567, 562, 3); // MIN_SM_CONFIG_SHARED_MEM_SIZE
    campo(&mut q, 574, 569, 3); // MAX_SM_CONFIG_SHARED_MEM_SIZE
    campo(&mut q, 662, 657, 3); // TARGET_SM_CONFIG_SHARED_MEM_SIZE
    campo(&mut q, 579, 576, 0); // QMD_VERSION
    campo(&mut q, 583, 580, 3); // QMD_MAJOR_VERSION
    campo(&mut q, 607, 592, hilos as u64); // CTA_THREAD_DIMENSION0
    campo(&mut q, 623, 608, 1); // CTA_THREAD_DIMENSION1
    campo(&mut q, 639, 624, 1); // CTA_THREAD_DIMENSION2
    campo(&mut q, 656, 648, registros as u64); // REGISTER_COUNT_V
    // RELEASE0: una palabra, con SYSMEMBAR, al acabar la rejilla.
    campo(&mut q, 799, 768, sem & 0xFFFF_FFFF);
    campo(&mut q, 807, 800, sem >> 32);
    campo(&mut q, 819, 819, 1); // RELEASE0_MEMBAR_TYPE FE_SYSMEMBAR
    campo(&mut q, 823, 823, 1); // RELEASE0_ENABLE
    campo(&mut q, 831, 830, 1); // RELEASE0_STRUCTURE_SIZE ONE_WORD
    campo(&mut q, 863, 832, paga as u64);
    campo(&mut q, 1567, 1536, programa & 0xFFFF_FFFF); // PROGRAM_ADDRESS_LOWER
    campo(&mut q, 1584, 1568, programa >> 32); // PROGRAM_ADDRESS_UPPER
    q
}

/// Los metodos de `clc7c0.h`.
const SET_OBJECT: u32 = 0x0000;
const WAIT_FOR_IDLE: u32 = 0x0110;
const SET_SHADER_SHARED_MEMORY_WINDOW_A: u32 = 0x02A0;
const SEND_PCAS_A: u32 = 0x02B4;
const SEND_SIGNALING_PCAS2_B: u32 = 0x02C0;
const SET_SHADER_LOCAL_MEMORY_WINDOW_A: u32 = 0x07B0;
const SET_REPORT_SEMAPHORE_A: u32 = 0x1B00;
/// `PCAS_ACTION_INVALIDATE_COPY_SCHEDULE`.
const INVALIDAR_Y_LANZAR: u32 = 3;

/// Palabras de ordenes.
pub const ORDENES: usize = 19;

/// **Las ordenes**: las ventanas (la compartida en 0xFE_0000_0000 y la local
/// en 0xFF_0000_0000, como NVK: lejos del tramo y de los buferes de GR), el
/// QMD, esperar a que el GR acabe, y el semaforo de informe.
pub fn ordenes() -> [u32; ORDENES] {
    ordenes_con(va(QMD), va(SEMAFORO_FIN), PAGA_FIN)
}

/// Las mismas ordenes para otro QMD y otro semaforo de informe.
pub fn ordenes_con(q: u64, s: u64, paga: u32) -> [u32; ORDENES] {
    let c = |m, n| cabecera_en(SUBCANAL, m, n);
    [
        c(SET_OBJECT, 1),
        AMPERE_COMPUTE_B,
        c(SET_SHADER_SHARED_MEMORY_WINDOW_A, 2),
        0xFE,
        0,
        c(SET_SHADER_LOCAL_MEMORY_WINDOW_A, 2),
        0xFF,
        0,
        c(SEND_PCAS_A, 1),
        (q >> 8) as u32,
        c(SEND_SIGNALING_PCAS2_B, 1),
        INVALIDAR_Y_LANZAR,
        c(WAIT_FOR_IDLE, 1),
        0,
        c(SET_REPORT_SEMAPHORE_A, 4),
        (s >> 32) as u32,
        s as u32,
        paga,
        computo::INFORME,
    ]
}

/// La entrada del GPFIFO: la 1 (S3 uso la 0).
pub const ENTRADA_GPFIFO: u64 = 1;

/// **Preparar**: salida y semaforos a cero, el programa (y el resto de su
/// pagina a cero), el QMD y las ordenes, y la entrada 1 del GPFIFO, todo
/// RELEIDO.
pub fn preparar<R: Registros>(r: &mut R) -> bool {
    let c = codigo();
    let q = qmd();
    let o = ordenes();
    let e = entrada(va(EMPUJE), ORDENES as u32);
    let pag = crate::vram::PALABRAS;
    a_cero(r, SALIDA) as usize == pag
        && a_cero(r, SEMAFOROS) as usize == pag
        && a_cero(r, PROGRAMA) as usize == pag
        && a_cero(r, QMD) as usize == pag
        && escribir(r, PROGRAMA, &c) == PALABRAS_CODIGO
        && escribir(r, QMD, &q) == QMD_PALABRAS
        && escribir(r, EMPUJE, &o) == ORDENES
        && escribir(r, GR.gpfifo + 8 * ENTRADA_GPFIFO, &[e as u32, (e >> 32) as u32]) == 2
}

/// **Lanzar**: la MMU invalidada, GP_PUT = 2 y la ficha en el timbre.
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

/// **Comprobar**: cuantos hilos escribieron SU valor; y que nada paso de ellos
/// (la palabra 32, a cero).
pub fn comprobar<R: Registros>(r: &mut R) -> (u32, bool) {
    let (base, off) = ventana(SALIDA);
    let antes = r.leer(VENTANA_REG);
    r.escribir(VENTANA_REG, base);
    let n = (0..HILOS).filter(|&k| r.leer(VENTANA + off + 4 * k) == valor(k)).count() as u32;
    let limpio = r.leer(VENTANA + off + 4 * HILOS) == 0;
    r.escribir(VENTANA_REG, antes);
    (n, limpio)
}

/// `buenas | limpio << 6 | qmd << 7 | fin << 8 | lanzado << 9 | GP_GET << 16
/// | us << 32`.
pub const fn empaquetar(buenas: u32, limpio: bool, qmd: bool, fin: bool, lanzado: bool, gp_get: u32, us: u32) -> u64 {
    (buenas as u64 & 0x3F)
        | (limpio as u64) << 6
        | (qmd as u64) << 7
        | (fin as u64) << 8
        | (lanzado as u64) << 9
        | (gp_get as u64 & 0xFF) << 16
        | (us as u64) << 32
}

/// `(buenas, limpio, qmd, fin, lanzado, GP_GET, us)`.
pub const fn desempaquetar(v: u64) -> (u32, bool, bool, bool, bool, u32, u32) {
    (
        (v & 0x3F) as u32,
        v >> 6 & 1 != 0,
        v >> 7 & 1 != 0,
        v >> 8 & 1 != 0,
        v >> 9 & 1 != 0,
        (v >> 16 & 0xFF) as u32,
        (v >> 32) as u32,
    )
}

/// El sombreador corrio entero: los 32 hilos escribieron lo suyo, nada mas,
/// y la 3060 pago el semaforo del QMD.
pub const fn sano(v: u64) -> bool {
    let (buenas, limpio, qmd, _, lanzado, _, _) = desempaquetar(v);
    buenas == HILOS && limpio && qmd && lanzado
}

#[cfg(test)]
mod pruebas {
    use super::*;
    extern crate std;

    struct Falsa {
        ventana: u32,
        tramo: std::vec::Vec<u32>,
        timbre: Option<u32>,
    }

    impl Falsa {
        fn celda(&mut self, reg: u32) -> Option<&mut u32> {
            let dir = ((self.ventana as u64) << 16) + (reg - VENTANA) as u64;
            let k = dir.checked_sub(TRAMO)? / 4;
            self.tramo.get_mut(k as usize)
        }
        fn en(&self, dir: u64) -> u32 {
            self.tramo[((dir - TRAMO) / 4) as usize]
        }
        fn poner(&mut self, dir: u64, v: u32) {
            self.tramo[((dir - TRAMO) / 4) as usize] = v;
        }
    }

    impl Registros for Falsa {
        fn leer(&mut self, reg: u32) -> u32 {
            match reg {
                VENTANA_REG => self.ventana,
                _ => self.celda(reg).map_or(0, |c| *c),
            }
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            match reg {
                VENTANA_REG => self.ventana = v,
                TIMBRE => self.timbre = Some(v),
                crate::copia::INVALIDAR_PDB | crate::copia::INVALIDAR_PDB_HI | crate::copia::INVALIDAR => {}
                _ => {
                    if let Some(c) = self.celda(reg) {
                        *c = v;
                    }
                }
            }
        }
    }

    #[test]
    fn todo_cabe_en_el_tramo_y_no_pisa_lo_de_antes() {
        let fin = TRAMO + 16 * PAGINA;
        for d in [SALIDA, PROGRAMA, QMD, SEMAFOROS, EMPUJE] {
            assert!(d >= TRAMO && d + PAGINA <= fin);
            // Ni la copia (3, 4, 8, 12) ni S3 (7, 9) ni los canales (0, 1, 2, 5, 6).
            assert!(![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 12].contains(&((d - TRAMO) / PAGINA)));
        }
        // El QMD alineado a 256 B; el programa, a 128.
        assert_eq!(va(QMD) % 256, 0);
        assert_eq!(va(PROGRAMA) % 128, 0);
        // Y la salida es la que el programa lleva dentro: 0x2_0000_A000.
        assert_eq!(va(SALIDA), 0x2_0000_A000);
    }

    #[test]
    fn el_programa_es_el_de_nvdisasm() {
        let c = codigo();
        assert_eq!(c.len(), 40);
        // La STG: 0x0000000502007986 / 0x000fe2000c101904, el bit 101 a 0.
        assert_eq!((c[28], c[29], c[30], c[31]), (0x0200_7986, 0x5, 0x0C10_1904, 0x000F_E200));
        assert_eq!(CODIGO[7].1 >> (101 - 64) & 1, 0);
        // Los dos NOP conservan los bits de planificacion (41..63 del alto).
        assert_eq!(CODIGO[0].1 & ((1 << 41) - 1), 0);
        assert_eq!(CODIGO[3].1 & ((1 << 41) - 1), 0);
        // El valor y la direccion que lleva dentro son los que se comprueban.
        assert_eq!(CODIGO[5].0 >> 32, valor(0) as u64);
        assert_eq!(CODIGO[6].0 >> 32, va(SALIDA) & 0xFFFF_FFFF);
        assert_eq!(CODIGO[2].0 >> 32, va(SALIDA) >> 32);
    }

    #[test]
    fn el_qmd_dice_lo_que_debe() {
        let q = qmd();
        let f = |hi, lo| leer_campo(&q, hi, lo);
        assert_eq!((f(583, 580), f(579, 576)), (3, 0));
        assert_eq!((f(607, 592), f(623, 608), f(639, 624)), (32, 1, 1));
        assert_eq!((f(415, 384), f(431, 416), f(463, 448)), (1, 1, 1));
        assert_eq!(f(1567, 1536) | f(1584, 1568) << 32, va(PROGRAMA));
        assert_eq!(f(799, 768) | f(807, 800) << 32, va(SEMAFORO_QMD));
        assert_eq!((f(823, 823), f(831, 830), f(863, 832)), (1, 1, PAGA_QMD as u64));
        assert_eq!(f(656, 648), 16);
        assert_eq!(f(561, 544), 0); // SHARED_MEMORY_SIZE
        assert_eq!(f(759, 736), 0); // SHADER_LOCAL_MEMORY_LOW_SIZE
        // Un campo que cruza dos palabras se lee igual que se escribio.
        let mut z = [0u32; QMD_PALABRAS];
        campo(&mut z, 40, 20, 0x1A_BCDE);
        assert_eq!(leer_campo(&z, 40, 20), 0x1A_BCDE);
    }

    #[test]
    fn las_ordenes_lanzan_el_qmd() {
        let o = ordenes();
        assert_eq!(o[0], 0x2001_2000);
        assert_eq!(o[8], cabecera_en(1, 0x02B4, 1));
        assert_eq!(o[9] as u64, va(QMD) >> 8);
        assert_eq!((o[10], o[11]), (cabecera_en(1, 0x02C0, 1), 3));
        assert_eq!(o[12], cabecera_en(1, 0x0110, 1));
        assert_eq!(((o[15] as u64) << 32) | o[16] as u64, va(SEMAFORO_FIN));
    }

    #[test]
    fn preparar_lanzar_y_comprobar_en_una_3060_de_mentira() {
        let mut f = Falsa { ventana: 0xFFF0, tramo: std::vec![0xDEAD_BEEF; 16 * 1024], timbre: None };
        assert!(preparar(&mut f));
        assert_eq!(f.en(PROGRAMA + 28 * 4), 0x0200_7986);
        assert_eq!(f.en(PROGRAMA + 40 * 4), 0);
        assert_eq!(f.en(QMD + 4 * 18), qmd()[18]);
        let e = entrada(va(EMPUJE), ORDENES as u32);
        assert_eq!(f.en(GR.gpfifo + 8), e as u32);
        // La entrada 0 (la de S3) no se toca.
        assert_eq!(f.en(GR.gpfifo), 0xDEAD_BEEF);
        assert!(lanzar(&mut f, 2));
        assert_eq!((f.en(GR.userd + GP_PUT), f.timbre), (2, Some(2)));
        // La 3060 de mentira "corre" el sombreador.
        assert_eq!(comprobar(&mut f), (0, true));
        for k in 0..HILOS {
            f.poner(SALIDA + 4 * k as u64, valor(k));
        }
        assert_eq!(comprobar(&mut f), (32, true));
        f.poner(SALIDA + 4 * 32, 1);
        assert_eq!(comprobar(&mut f), (32, false));
        assert_eq!(f.ventana, 0xFFF0);
    }

    #[test]
    fn el_resultado_se_empaqueta() {
        let v = empaquetar(32, true, true, true, true, 2, 9);
        assert_eq!(desempaquetar(v), (32, true, true, true, true, 2, 9));
        assert!(sano(v));
        assert!(!sano(empaquetar(31, true, true, true, true, 2, 9)));
        assert!(!sano(empaquetar(32, true, false, true, true, 2, 9)));
    }
}
