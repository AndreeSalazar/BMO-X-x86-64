//! Regresion del punto de observacion: argumentos escritos, CPU sin arrancar.
//! [carril] AMARILLO
//! [estado]  VOLATIL  banco simulado local; no accede a registros de aparato.
use super::*;
extern crate std;
use std::{vec, vec::Vec};

struct ArranqueObservado {
    base: u32, control: u32, perder: bool, argumentos: [u32; 3],
    escritos: Vec<(u32, u32)>,
}
impl Registros for ArranqueObservado {
    fn leer(&mut self, reg: u32) -> u32 {
        match reg - self.base {
            BOOTVEC => self.argumentos[0], MAILBOX0 => self.argumentos[1],
            MAILBOX1 => self.argumentos[2], CPUCTL => self.control, _ => 0,
        }
    }
    fn escribir(&mut self, reg: u32, v: u32) {
        self.escritos.push((reg - self.base, v));
        if !self.perder {
            match reg - self.base {
                BOOTVEC => self.argumentos[0] = v, MAILBOX0 => self.argumentos[1] = v,
                MAILBOX1 => self.argumentos[2] = v, _ => {},
            }
        }
    }
}
#[test]
fn foto_observada_ve_argumentos_antes_de_start_en_ambos_motores_y_alias() {
    use crate::arranque::hs_observacion::{Foto, Motor};
    for motor in [Motor::Gsp, Motor::Sec2] {
        for alias in [false, true] {
            let mut r = ArranqueObservado { base: motor.base(), control: if alias { 64 } else { 16 },
                perder: false, argumentos: [0; 3], escritos: vec![] };
            let mut visto = false;
            arrancar_con_observador(&mut r, motor.base(), Some(0x100), Some(0x3E007000), Some(0), |r| {
                assert_eq!(r.escritos, [(BOOTVEC, 0x100), (MAILBOX0, 0x3E007000), (MAILBOX1, 0)]);
                let foto = Foto::leer(r, motor);
                assert_eq!(foto.argumentos([Some(0x100), Some(0x3E007000), Some(0)]), (7, 7, 7));
                visto = true;
            }).unwrap();
            assert!(visto);
            assert_eq!(r.escritos.len(), 4, "no repite argumentos ni STARTCPU");
            assert_eq!(r.escritos[3], (if alias { 0x130 } else { 0x100 }, 2));
        }
    }
}
#[test]
fn observacion_no_fabrica_igualdad_y_pri_de_control_sigue_impidiendo_start() {
    use crate::arranque::hs_observacion::{Foto, Motor};
    let mut r = ArranqueObservado { base: SEC2, control: 0xBADF1002, perder: true,
        argumentos: [0, 0, 99], escritos: vec![] };
    let error = arrancar_con_observador(&mut r, SEC2, Some(0x100), Some(0x3E007000), None, |r| {
        assert_eq!(Foto::leer(r, Motor::Sec2).argumentos([Some(0x100), Some(0x3E007000), None]), (3, 3, 0));
    });
    assert_eq!(error, Err(NoFuego::NoContesta(SEC2 + CPUCTL)));
    assert_eq!(r.escritos, [(BOOTVEC, 0x100), (MAILBOX0, 0x3E007000)]);
}
