//! **EL COMPUTO DE UNA APP en SM86** (LI2f, 12-10) -- los codificadores de
//! lo que pide un sombreador de computo de D3D12: los ids de hilo y de grupo
//! (S2R), la barrera del grupo (BAR.SYNC), la memoria compartida (STS, LDS),
//! la global (LDG, STG) y los atomicos de los `Interlocked*` de HLSL (ATOMS,
//! ATOMG), en las dos memorias.
//!
//! [carril]  VERDE     puro: de campos a dos palabras de 64 bits
//!
//! # De donde sale cada bit (nada adivinado)
//!
//! Como todo `codifica`: su PALABRA DE ORO, de `ptxas -arch=sm_86 -O3`
//! (CUDA 12.9, de PyPI) sobre `ga10x/sombreadores/oro_computo.ptx`, leida con
//! `nvdisasm -hex` (13.4, de PyPI) -- el 12-10, en el anfitrion, con las
//! mismas herramientas que las demas --. La prueba de abajo compara los 128
//! bits de cada una, control incluido. Lo que se deduce de ellas, dicho:
//!
//! ```text
//!    S2R          el registro especial en 72..80: TID.X/Y/Z 0x21..0x23,
//!                 CTAID.X/Y/Z 0x25..0x27 (los seis, de oro)
//!    STS / LDS    [Ra + imm]: Ra en 24..32 y el inmediato en 40..64 (de 24
//!                 bits); el valor de STS en 32..40
//!    ATOMS/ATOMG  la operacion en 87..91 (ADD 0, MIN 1, MAX 2, AND 5, OR 6,
//!                 XOR 7, EXCH 8) y CON SIGNO en el 73; el CAS es otro opcode
//!                 (0x38d y 0x3a9) con el tercer registro en 64..72
//!    LDG / STG    `.E` (64 bits de direccion), 32 bits de dato, la guarda en
//!                 12..16 (`@P`, `@!P`), [Ra.64 + imm] con imm en 40..64; el
//!                 LDG lleva un 0x04 FIJO en 32..40 (el `.STRONG.SYS` del
//!                 kernel, `tuberia::ldg`, un 0x06: es de la forma)
//! ```
//!
//! [!] NADA DE ESTO HA CORRIDO EN EL METAL todavia desde PROTON-X: lo dira el
//! primer computo de una app. Lo que si corrio: S2R TID.X y CTAID.X y STG
//! (los trabajos del kernel: `blur`, `fractal`, la libreta).

/// Un registro especial que lee S2R.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Sr {
    TidX = 0x21,
    TidY = 0x22,
    TidZ = 0x23,
    CtaidX = 0x25,
    CtaidY = 0x26,
    CtaidZ = 0x27,
}

impl Sr {
    pub const TID: [Sr; 3] = [Sr::TidX, Sr::TidY, Sr::TidZ];
    pub const CTAID: [Sr; 3] = [Sr::CtaidX, Sr::CtaidY, Sr::CtaidZ];

    pub fn de(n: u8) -> Option<Sr> {
        Sr::TID.iter().chain(Sr::CTAID.iter()).copied().find(|s| *s as u8 == n)
    }
}

/// Una operacion atomica (`Interlocked*` de HLSL).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Atomo {
    Add = 0,
    Min = 1,
    Max = 2,
    And = 5,
    Or = 6,
    Xor = 7,
    Exch = 8,
}

impl Atomo {
    pub fn de(n: u8) -> Option<Atomo> {
        [Atomo::Add, Atomo::Min, Atomo::Max, Atomo::And, Atomo::Or, Atomo::Xor, Atomo::Exch].into_iter().find(|a| *a as u8 == n)
    }
}

/// El predicado PT (siempre), como guarda.
pub const SIEMPRE: u8 = 7;

fn palabra(lo: u64, hi: u64, control: u64) -> (u64, u64) {
    (lo, (hi & ((1 << 41) - 1)) | control << 41)
}

/// `S2R Rd, SR`.
pub fn s2r(rd: u8, sr: Sr, control: u64) -> (u64, u64) {
    palabra(0x7919 | (rd as u64) << 16, (sr as u64) << 8, control)
}

/// `BAR.SYNC.DEFER_BLOCKING 0x0`: la barrera del grupo.
pub fn bar_sync(control: u64) -> (u64, u64) {
    palabra(0x7b1d, 1 << 16, control)
}

/// `STS [Ra + imm], Rb`.
pub fn sts(ra: u8, imm: u32, rb: u8, control: u64) -> (u64, u64) {
    palabra(0x7388 | (ra as u64) << 24 | (rb as u64) << 32 | (imm as u64 & 0xFF_FFFF) << 40, 0x800, control)
}

/// `LDS Rd, [Ra + imm]`.
pub fn lds(rd: u8, ra: u8, imm: u32, control: u64) -> (u64, u64) {
    palabra(0x7984 | (rd as u64) << 16 | (ra as u64) << 24 | (imm as u64 & 0xFF_FFFF) << 40, 0x800, control)
}

/// `ATOMS.op[.S32] Rd, [Ra], Rb`: el valor de antes en Rd.
pub fn atoms(op: Atomo, con_signo: bool, rd: u8, ra: u8, rb: u8, control: u64) -> (u64, u64) {
    palabra(0x738c | (rd as u64) << 16 | (ra as u64) << 24 | (rb as u64) << 32, (op as u64) << 23 | (con_signo as u64) << 9, control)
}

/// `ATOMS.CAS Rd, [Ra], Rb, Rc`: si lo de `[Ra]` es Rb, pone Rc.
pub fn atoms_cas(rd: u8, ra: u8, rb: u8, rc: u8, control: u64) -> (u64, u64) {
    palabra(0x738d | (rd as u64) << 16 | (ra as u64) << 24 | (rb as u64) << 32, rc as u64, control)
}

/// `ATOMG.E.op[.S32].STRONG.GPU PT, Rd, [Ra.64], Rb`.
pub fn atomg(op: Atomo, con_signo: bool, rd: u8, ra: u8, rb: u8, control: u64) -> (u64, u64) {
    palabra(0x79a8 | (rd as u64) << 16 | (ra as u64) << 24 | (rb as u64) << 32, 0x081e_e1c4 & !(0xF << 23) | (op as u64) << 23 | (con_signo as u64) << 9, control)
}

/// `ATOMG.E.CAS.STRONG.GPU PT, Rd, [Ra.64], Rb, Rc`.
pub fn atomg_cas(rd: u8, ra: u8, rb: u8, rc: u8, control: u64) -> (u64, u64) {
    palabra(0x73a9 | (rd as u64) << 16 | (ra as u64) << 24 | (rb as u64) << 32, 0x001e_e100 | rc as u64, control)
}

/// `[@[!]Pg] LDG.E Rd, [Ra.64 + imm]` (`guarda`: 0..6 un predicado, +8
/// negado, [`SIEMPRE`] sin guarda).
pub fn ldg(guarda: u8, rd: u8, ra: u8, imm: u32, control: u64) -> (u64, u64) {
    palabra(0x0981 | (guarda as u64 & 0xF) << 12 | (rd as u64) << 16 | (ra as u64) << 24 | 0x04 << 32 | (imm as u64 & 0xFF_FFFF) << 40, 0x0c1e_1900, control)
}

/// `[@[!]Pg] STG.E [Ra.64 + imm], Rb`.
pub fn stg(guarda: u8, ra: u8, imm: u32, rb: u8, control: u64) -> (u64, u64) {
    palabra(0x0986 | (guarda as u64 & 0xF) << 12 | (ra as u64) << 24 | (rb as u64) << 32 | (imm as u64 & 0xFF_FFFF) << 40, 0x0c10_1904, control)
}

/// Las PALABRAS DE ORO del computo (12-10): `ptxas -arch=sm_86 -O3` (CUDA
/// 12.9) sobre `ga10x/sombreadores/oro_computo.ptx`, leido con `nvdisasm
/// -hex` (13.4). El texto, el de `nvdisasm`.
pub const ORO_COMPUTO: &[(&str, u64, u64)] = &[
    ("S2R R5, SR_TID.X", 0x0000000000057919, 0x000e220000002100),
    ("S2R R7, SR_TID.Y", 0x0000000000077919, 0x000e680000002200),
    ("S2R R9, SR_TID.Z", 0x0000000000097919, 0x000ea80000002300),
    ("S2R R11, SR_CTAID.X", 0x00000000000b7919, 0x000ee80000002500),
    ("S2R R13, SR_CTAID.Y", 0x00000000000d7919, 0x000f280000002600),
    ("S2R R15, SR_CTAID.Z", 0x00000000000f7919, 0x000f680000002700),
    ("BAR.SYNC.DEFER_BLOCKING 0x0", 0x0000000000007b1d, 0x000fec0000010000),
    ("STS [R0], R7", 0x0000000700007388, 0x000fe80000000800),
    ("LDS R5, [R0+0x8]", 0x0000080000057984, 0x000e280000000800),
    ("ATOMS.ADD R7, [R5], R24", 0x000000180507738c, 0x000e280000000000),
    ("ATOMS.MIN R9, [R5], R24", 0x000000180509738c, 0x000e680000800000),
    ("ATOMS.MAX R11, [R5], R24", 0x00000018050b738c, 0x000ea80001000000),
    ("ATOMS.MIN.S32 R13, [R5], R24", 0x00000018050d738c, 0x000ee80000800200),
    ("ATOMS.MAX.S32 R15, [R5], R24", 0x00000018050f738c, 0x000f280001000200),
    ("ATOMS.AND R17, [R5], R24", 0x000000180511738c, 0x000f680002800000),
    ("ATOMS.OR R19, [R5], R24", 0x000000180513738c, 0x000ea80003000000),
    ("ATOMS.XOR R21, [R5], R24", 0x000000180515738c, 0x000ee80003800000),
    ("ATOMS.EXCH R23, [R5], R24", 0x000000180517738c, 0x000f280004000000),
    ("ATOMS.CAS R25, [R5], R24, R25", 0x000000180519738d, 0x000f680000000019),
    ("ATOMG.E.ADD.STRONG.GPU PT, R7, [R2.64], R18", 0x00000012020779a8, 0x000ea200081ee1c4),
    ("ATOMG.E.MIN.STRONG.GPU PT, R9, [R2.64], R18", 0x00000012020979a8, 0x000ea800089ee1c4),
    ("ATOMG.E.MAX.STRONG.GPU PT, R11, [R2.64], R18", 0x00000012020b79a8, 0x000ea800091ee1c4),
    ("ATOMG.E.MIN.S32.STRONG.GPU PT, R13, [R2.64], R18", 0x00000012020d79a8, 0x000ee800089ee3c4),
    ("ATOMG.E.MAX.S32.STRONG.GPU PT, R15, [R2.64], R18", 0x00000012020f79a8, 0x000f2800091ee3c4),
    ("ATOMG.E.AND.STRONG.GPU PT, R17, [R2.64], R18", 0x00000012021179a8, 0x000f28000a9ee1c4),
    ("ATOMG.E.OR.STRONG.GPU PT, R7, [R2.64], R18", 0x00000012020779a8, 0x001f28000b1ee1c4),
    ("ATOMG.E.XOR.STRONG.GPU PT, R9, [R2.64], R18", 0x00000012020979a8, 0x002f28000b9ee1c4),
    ("ATOMG.E.EXCH.STRONG.GPU PT, R11, [R2.64], R18", 0x00000012020b79a8, 0x004ea8000c1ee1c4),
    ("ATOMG.E.CAS.STRONG.GPU PT, R13, [R2], R18, R19", 0x00000012020d73a9, 0x008ea800001ee113),
    ("LDG.E R3, [R2.64+0x10]", 0x0000100402037981, 0x000ea2000c1e1900),
    ("STG.E [R2.64+0xc], R11", 0x00000c0b02007986, 0x008fe8000c101904),
    ("@!P0 ATOMG.E.ADD.STRONG.GPU PT, R7, [R2.64], R5", 0x00000005020789a8, 0x00116200081ee1c4),
];

#[cfg(test)]
mod pruebas {
    use super::*;

    /// *** Cada codificador da su palabra de oro, los 128 bits.
    #[test]
    fn el_computo_como_ptxas() {
        let k = |i: usize| ORO_COMPUTO[i].2 >> 41;
        let a = [Atomo::Add, Atomo::Min, Atomo::Max, Atomo::Min, Atomo::Max, Atomo::And, Atomo::Or, Atomo::Xor, Atomo::Exch];
        let signo = [false, false, false, true, true, false, false, false, false];
        let mut hechas = [
            s2r(5, Sr::TidX, k(0)),
            s2r(7, Sr::TidY, k(1)),
            s2r(9, Sr::TidZ, k(2)),
            s2r(11, Sr::CtaidX, k(3)),
            s2r(13, Sr::CtaidY, k(4)),
            s2r(15, Sr::CtaidZ, k(5)),
            bar_sync(k(6)),
            sts(0, 0, 7, k(7)),
            lds(5, 0, 8, k(8)),
        ]
        .to_vec();
        let ds = [7u8, 9, 11, 13, 15, 17, 19, 21, 23];
        for j in 0..9 {
            hechas.push(atoms(a[j], signo[j], ds[j], 5, 24, k(9 + j)));
        }
        hechas.push(atoms_cas(25, 5, 24, 25, k(18)));
        let dg = [7u8, 9, 11, 13, 15, 17, 7, 9, 11];
        for j in 0..9 {
            hechas.push(atomg(a[j], signo[j], dg[j], 2, 18, k(19 + j)));
        }
        hechas.push(atomg_cas(13, 2, 18, 19, k(28)));
        hechas.push(ldg(SIEMPRE, 3, 2, 0x10, k(29)));
        hechas.push(stg(SIEMPRE, 2, 0xc, 11, k(30)));
        let mut g = atomg(Atomo::Add, false, 7, 2, 5, k(31));
        g.0 = g.0 & !(0xF << 12) | 8 << 12; // la guarda @!P0, como la pone el emisor
        hechas.push(g);
        assert_eq!(hechas.len(), ORO_COMPUTO.len());
        for ((texto, lo, hi), h) in ORO_COMPUTO.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}: {:#018x} {:#018x}", h.0, h.1);
        }
    }

    /// La guarda de LDG y STG: `@P3` y `@!P1`, en sus cuatro bits.
    #[test]
    fn la_guarda_de_ldg_y_stg() {
        assert_eq!(stg(3, 2, 0, 5, 0).0 >> 12 & 0xF, 3);
        assert_eq!(ldg(8 + 1, 4, 2, 0, 0).0 >> 12 & 0xF, 9);
        assert_eq!(stg(SIEMPRE, 2, 0xc, 11, 0).0, ORO_COMPUTO[30].1);
    }
}
