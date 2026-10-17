//! Foto del GSP justo antes de STARTCPU del SEC2: argumentos y controles.
//!
//! capa: puro -- solo lee por Registros; no resetea ni escribe el falcon.
//! [estado]  VOLATIL  los buzones y controles se guardan como foto de un intento.
//! [consumo] NADA -- cuatro lecturas por intento de booter.
//! El 0x15 deja buzones BADF despues; falta saber si ya lo estaban ANTES.

use crate::{es_error_pri, falcon as fa, Registros};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootProbe {
    pub mailboxes: u64,
    pub controls: u64,
}

impl BootProbe {
    /// Un kernel anterior devuelve buzones vivos para selectores desconocidos.
    /// BADF tambien tiene bit 63: exigir el marcador entero de 32 bits.
    pub fn from_report(mailboxes: u64, controls: u64, stamp: u64) -> Option<(Self, u64)> {
        (stamp >> 32 == 0x8000_0000).then_some((Self { mailboxes, controls }, stamp & 0xFFFF_FFFF))
    }

    pub fn read(r: &mut impl Registros) -> Self {
        let low = r.leer(fa::GSP + fa::MAILBOX0);
        let high = r.leer(fa::GSP + fa::MAILBOX1);
        let cpu = r.leer(fa::GSP + fa::CPUCTL);
        let bcr = r.leer(fa::GSP + fa::BCR_CTRL);
        Self { mailboxes: low as u64 | (high as u64) << 32,
            controls: cpu as u64 | (bcr as u64) << 32 }
    }

    /// Un error PRI conserva su valor crudo, pero nunca acredita argumentos.
    pub fn arguments_readable(self) -> bool {
        !es_error_pri(self.mailboxes as u32) && !es_error_pri((self.mailboxes >> 32) as u32)
    }

    pub fn arguments_match(self, expected: u64) -> bool {
        self.arguments_readable() && self.mailboxes == expected
    }
}

/// **LA FOTO HONDA** (17-10): lo que OpenRM 570.144 marca para comparar un
/// arranque bueno con uno malo, leido JUSTO antes de soltar el booter. Solo
/// lecturas; cada palabra cruda (un BADF se queda BADF).
///    [0] HWCFG2 del SEC2 | del GSP << 32 (el bit 12, la memoria limpiandose)
///    [1] DMATRFCMD del SEC2 | ENGINE del SEC2 << 32
///    [2] ENGINE del GSP | PLM de PTIMER << 32
///    [3] PTIMER: TIME_0 | TIME_1 << 32
///    [4] PTIMER: lo que avanzo TIME_0 en 10 us de la CPU
///    [5] ECC de la MMU sin corregir: L2TLB | HUBTLB << 32
///    [6] ECC FILLUNIT | WPR2_ADDR_LO << 32
///    [7] WPR2_ADDR_HI
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FotoHonda(pub [u64; 8]);

pub const ECC_L2TLB: u32 = 0x0010_0E78;
pub const ECC_HUBTLB: u32 = 0x0010_0E8C;
pub const ECC_FILLUNIT: u32 = 0x0010_0EA0;
pub use crate::aon::{WPR2_HI as WPR2_ADDR_HI, WPR2_LO as WPR2_ADDR_LO};

impl FotoHonda {
    pub fn leer(r: &mut impl Registros, t: &mut impl fa::Reloj) -> Self {
        use crate::arranque::ptimer as pt;
        let mut l = |reg: u32| r.leer(reg) as u64;
        let par = |a: u64, b: u64| a | b << 32;
        let w0 = par(l(fa::SEC2 + fa::HWCFG2), l(fa::GSP + fa::HWCFG2));
        let w1 = par(l(fa::SEC2 + fa::DMATRFCMD), l(fa::SEC2 + fa::ENGINE));
        let w2 = par(l(fa::GSP + fa::ENGINE), l(pt::TIME_PLM));
        let t0 = r.leer(pt::TIME_0);
        let w3 = par(t0 as u64, r.leer(pt::TIME_1) as u64);
        let desde = t.us();
        while t.us() < desde + 10 {}
        let w4 = r.leer(pt::TIME_0).wrapping_sub(t0) as u64;
        let mut l = |reg: u32| r.leer(reg) as u64;
        let w5 = par(l(ECC_L2TLB), l(ECC_HUBTLB));
        let w6 = par(l(ECC_FILLUNIT), l(WPR2_ADDR_LO));
        let w7 = l(WPR2_ADDR_HI);
        Self([w0, w1, w2, w3, w4, w5, w6, w7])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bank([u32; 4]);
    impl Registros for Bank {
        fn leer(&mut self, reg: u32) -> u32 {
            self.0[match reg - fa::GSP {
                fa::MAILBOX0 => 0, fa::MAILBOX1 => 1,
                fa::CPUCTL => 2, fa::BCR_CTRL => 3,
                _ => panic!("registro fuera de la foto"),
            }]
        }
        fn escribir(&mut self, _: u32, _: u32) { panic!("la foto no escribe"); }
    }

    #[test]
    fn reads_without_writing_and_preserves_both_address_halves() {
        let p = BootProbe::read(&mut Bank([0x3C00_0000, 2, 0x10, 1]));
        assert!(p.arguments_match(0x0000_0002_3C00_0000));
        assert!(!p.arguments_match(0x3C00_0000));
        assert_eq!(p.controls, 0x0000_0001_0000_0010);
    }

    #[test]
    fn inaccessible_mailboxes_remain_raw_and_never_count_as_arguments() {
        for bad in [0xBADF_1002, 0xBAD0_0100, 0xFFFF_FFFF] {
            for words in [[bad, 0], [0x3C00_0000, bad]] {
                let p = BootProbe::read(&mut Bank([words[0], words[1], 0xBADF_5620, 0]));
                assert!(!p.arguments_readable());
                assert!(!p.arguments_match(p.mailboxes));
                assert_eq!(p.mailboxes, words[0] as u64 | (words[1] as u64) << 32);
            }
        }
    }

    #[test]
    fn cleared_mailboxes_are_readable_but_do_not_confirm_the_libos_pointer() {
        let p = BootProbe::read(&mut Bank([0, 0, 0x10, 1]));
        assert!(p.arguments_readable());
        assert!(!p.arguments_match(0x3C00_0000));
    }

    #[test]
    fn old_kernel_mailboxes_are_not_a_saved_probe() {
        for stamp in [0, 0x3C00_0000, 0xBADF_1002_BADF_1002, u64::MAX] {
            assert!(BootProbe::from_report(0, 0, stamp).is_none());
        }
        let (p, expected) = BootProbe::from_report(0x3C00_0000, 0x10, 0x8000_0000_3C00_0000).unwrap();
        assert!(p.arguments_match(expected));
    }

    struct Todo(u32);
    impl Registros for Todo {
        fn leer(&mut self, reg: u32) -> u32 {
            if reg == crate::arranque::ptimer::TIME_0 {
                self.0 += 320;
                return self.0;
            }
            reg
        }
        fn escribir(&mut self, _: u32, _: u32) { panic!("la foto honda no escribe"); }
    }
    struct Tic(u64);
    impl fa::Reloj for Tic {
        fn us(&mut self) -> u64 { self.0 += 1; self.0 }
    }

    #[test]
    fn la_foto_honda_lee_cada_registro_en_su_sitio_y_no_escribe() {
        let f = FotoHonda::leer(&mut Todo(0), &mut Tic(0));
        assert_eq!(f.0[0], (fa::SEC2 + fa::HWCFG2) as u64 | ((fa::GSP + fa::HWCFG2) as u64) << 32);
        assert_eq!(f.0[1] >> 32, (fa::SEC2 + fa::ENGINE) as u64);
        assert_eq!(f.0[4], 320, "lo que avanzo PTIMER entre las dos lecturas");
        assert_eq!(f.0[6] >> 32, WPR2_ADDR_LO as u64);
        assert_eq!(f.0[7], WPR2_ADDR_HI as u64);
    }
}

