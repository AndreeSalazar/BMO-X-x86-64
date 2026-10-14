//! **LOS CUADROS EN SM86** (L2, 14-10) -- los codificadores de lo que habla
//! entre los hilos de un warp: el intercambio (SHFL), con el que salen las
//! derivadas de un sombreador de pixeles (`ddx`, `ddy` y la mip de un
//! `Sample`), y FSWZADD, la resta de un cuadro de 2x2 en una instruccion.
//!
//! [carril]  VERDE     puro: de campos a dos palabras de 64 bits
//!
//! # De donde sale cada bit (nada adivinado)
//!
//! ```text
//!    SHFL      ORO: `ptxas -arch=sm_86 -O3` (CUDA 12.9) sobre `oro_olas.ptx`
//!              (al lado de este crate), leido con `nvdisasm -hex` (13.4).
//!              Los campos, los de `sm70_encode.rs` de NAK (Mesa, 14-10:
//!              `docs/metal/mesa_nak/INFORME_NAK.md`): la forma con la lane y
//!              la c INMEDIATAS es 0xF89; la fuente en 24..32, la c (13 bits)
//!              en 40..53, la lane (5 bits) en 53..58, el modo en 58..60
//!              (IDX 0, UP 1, DOWN 2, BFLY 3) y el predicado de "dentro" en
//!              81..84 (PT). La lane 0 inmediata la escribe `ptxas` con RZ
//!              (0x589); la 0xF89 con lane 0 la lee `nvdisasm` igual
//!              ("SHFL.IDX PT, R4, R0, 0x0, 0x1c03").
//!    FSWZADD   `ptxas` no la escribe en CUDA (no hay cuadros fuera de un
//!              sombreador de pixeles): sus bits son los de NAK (opcode 0x822,
//!              src0 en 24..32, src1 en 64..72, una operacion de 2 bits por
//!              hilo en 32..40 -- la del hilo 0 en los bits altos --, .FTZ en
//!              el 80) y `nvdisasm` 13.4 los LEE como FSWZADD con su patron
//!              (`LEIDAS_FSWZADD`). Lo que NO se sabe: el signo de cada patron
//!              (si "PN" es src0 - src1): lo mide la LENTE en el metal.
//! ```
//!
//! # La semantica de SHFL (la del manual de PTX, `shfl.sync`)
//!
//! Con `c = mascara de segmento << 8 | tope`: el segmento del hilo empieza en
//! `min = hilo & mascara`, acaba en `max = min | (tope & !mascara)`, y el
//! hilo que se lee es `min | (lane & !mascara)` (IDX), `hilo ^ lane` (BFLY),
//! `hilo - lane` (UP) o `hilo + lane` (DOWN); si se sale del segmento, el
//! valor propio. `0x1c03` es el CUADRO (segmentos de 4), `0x1e01` los
//! PARES (segmentos de 2: la fila de un cuadro).

/// El intercambio: de donde lee cada hilo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Shfl {
    Idx = 0,
    Up = 1,
    Down = 2,
    Bfly = 3,
}

/// La c del cuadro (segmentos de 4) y la de los pares (segmentos de 2).
pub const CUADRO: u16 = 0x1c03;
pub const PARES: u16 = 0x1e01;

/// **`SHFL.<modo> PT, Rd, Ra, lane, c`** con la lane y la c inmediatas.
pub fn shfl(rd: u8, a: u8, modo: Shfl, lane: u8, c: u16, control: u64) -> (u64, u64) {
    let lo = 0xF89 | 7 << 12 | (rd as u64) << 16 | (a as u64) << 24 | ((c as u64) & 0x1FFF) << 40 | ((lane as u64) & 0x1F) << 53 | (modo as u64) << 58;
    (lo, 7 << 17 | control << 41)
}

/// **El hilo del que lee `hilo`** con un SHFL (la semantica de arriba): el
/// simulador y el banco.
pub fn de_quien(hilo: u32, modo: Shfl, lane: u32, c: u32) -> u32 {
    let mascara = c >> 8 & 0x1F;
    let tope = c & 0x1F;
    let min = hilo & mascara;
    let max = min | (tope & !mascara);
    let j = match modo {
        Shfl::Idx => min | (lane & !mascara),
        Shfl::Bfly => hilo ^ lane,
        Shfl::Up => hilo.wrapping_sub(lane),
        Shfl::Down => hilo + lane,
    };
    let fuera = match modo {
        Shfl::Up => j < min || j > hilo,
        _ => j > max,
    };
    if fuera { hilo } else { j }
}

/// Lo que hace un hilo del cuadro en FSWZADD (2 bits): sumar, restar
/// ("sub", 1), restar al reves ("subr", 2) o mover (3); nombres de NAK.
pub const SUMA: u8 = 0;
pub const SUB: u8 = 1;
pub const SUBR: u8 = 2;

/// Los patrones de NAK para ddx (sub, subr, sub, subr) y ddy (sub, sub,
/// subr, subr), el hilo 0 en los bits altos.
pub const DDX: u8 = SUB << 6 | SUBR << 4 | SUB << 2 | SUBR;
pub const DDY: u8 = SUB << 6 | SUB << 4 | SUBR << 2 | SUBR;

/// **`FSWZADD Rd, Ra, Rb, patron`** (`ra` el vecino, `rb` el propio, como
/// NAK).
pub fn fswzadd(rd: u8, a: u8, b: u8, patron: u8, ftz: bool, control: u64) -> (u64, u64) {
    let lo = 0x822 | 7 << 12 | (rd as u64) << 16 | (a as u64) << 24 | (patron as u64) << 32;
    (lo, b as u64 | (ftz as u64) << 16 | control << 41)
}

/// Las PALABRAS DE ORO de SHFL: `ptxas` 12.9 sobre `oro_olas.ptx`, leidas con
/// `nvdisasm -hex` 13.4.
pub const ORO_SHFL: &[(&str, u64, u64)] = &[
    ("SHFL.IDX PT, R11, R0, 0x1, 0x1c03", 0x003c0300000b7f89, 0x000e2800000e0000),
    ("SHFL.IDX PT, R6, R0, 0x2, 0x1c03", 0x005c030000067f89, 0x000fe800000e0000),
    ("SHFL.IDX PT, R13, R0, 0x1, 0x1e01", 0x003e0100000d7f89, 0x000e6800000e0000),
    ("SHFL.BFLY PT, R5, R0, 0x1, 0x1c03", 0x0c3c030000057f89, 0x000ea800000e0000),
    ("SHFL.BFLY PT, R7, R0, 0x2, 0x1c03", 0x0c5c030000077f89, 0x000ee800000e0000),
    ("SHFL.BFLY PT, R9, R0, 0x3, 0x1c03", 0x0c7c030000097f89, 0x000f2200000e0000),
];

/// Lo que `nvdisasm` 13.4 LEYO de los bits de FSWZADD armados con los
/// campos de NAK (`nvdisasm -b SM86 -hex`, 14-10): la codificacion es
/// valida; el SIGNO de cada patron no lo dice nadie (la LENTE).
pub const LEIDAS_FSWZADD: &[(&str, u64, u64)] = &[
    ("FSWZADD R4, R2, R3, PNNPPNNP", 0x0000006602047822, 0x000fc80000000003),
    ("FSWZADD R4, R2, R3, PNPNNPNP", 0x0000005a02047822, 0x000fc80000000003),
    ("FSWZADD.FTZ R10, R8, R9, PNNPPNNP", 0x00000066080a7822, 0x000fc80000010009),
    ("FSWZADD R4, R2, R3, PPPPPPPP", 0x0000000002047822, 0x000fc80000000003),
];

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn shfl_como_ptxas() {
        let k = |i: usize| ORO_SHFL[i].2 >> 41;
        let hechas = [
            shfl(11, 0, Shfl::Idx, 1, CUADRO, k(0)),
            shfl(6, 0, Shfl::Idx, 2, CUADRO, k(1)),
            shfl(13, 0, Shfl::Idx, 1, PARES, k(2)),
            shfl(5, 0, Shfl::Bfly, 1, CUADRO, k(3)),
            shfl(7, 0, Shfl::Bfly, 2, CUADRO, k(4)),
            shfl(9, 0, Shfl::Bfly, 3, CUADRO, k(5)),
        ];
        for ((texto, lo, hi), h) in ORO_SHFL.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}: {:#018x} {:#018x}", h.0, h.1);
        }
    }

    #[test]
    fn fswzadd_como_lo_leyo_nvdisasm() {
        assert_eq!((DDX, DDY), (0x66, 0x5A));
        let k = |i: usize| LEIDAS_FSWZADD[i].2 >> 41;
        let hechas = [fswzadd(4, 2, 3, DDX, false, k(0)), fswzadd(4, 2, 3, DDY, false, k(1)), fswzadd(10, 8, 9, DDX, true, k(2)), fswzadd(4, 2, 3, 0, false, k(3))];
        for ((texto, lo, hi), h) in LEIDAS_FSWZADD.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}");
        }
    }

    #[test]
    fn de_quien_lee_cada_hilo() {
        // El cuadro: IDX lee el hilo pedido de SU cuadro.
        for q in [0u32, 4, 28] {
            for k in 0..4 {
                assert_eq!(de_quien(q + k, Shfl::Idx, 0, CUADRO as u32), q);
                assert_eq!(de_quien(q + k, Shfl::Idx, 2, CUADRO as u32), q + 2);
                assert_eq!(de_quien(q + k, Shfl::Bfly, 1, CUADRO as u32), q + (k ^ 1));
                // Los pares: la fila del cuadro.
                assert_eq!(de_quien(q + k, Shfl::Idx, 1, PARES as u32), q + (k & 2) + 1);
            }
        }
    }
}
