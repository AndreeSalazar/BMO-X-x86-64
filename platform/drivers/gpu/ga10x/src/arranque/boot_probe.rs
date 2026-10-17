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
}
