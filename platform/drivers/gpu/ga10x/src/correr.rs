//! **CORRER EL SECUENCIADOR DEL GSP (L0c4b2c)** -- las ordenes de
//! `GSP_RUN_CPU_SEQUENCER`, hechas como las hace nova-core
//! (`gsp/sequencer.rs`), pero por TRAMOS: cada llamada corre hasta un
//! presupuesto de tiempo y dice donde se quedo, para que el kernel no se
//! quede segundos esperando a la 3060 con todo lo demas parado.
//!
//! capa: puro -- recibe los registros y el reloj como rasgos; no sabe de BAR0 (L8)
//!
//! [eje]     CORRECCION -- es la primera vez que la CPU escribe registros de
//!           la 3060 porque lo pide su firmware: nada fuera de lo permitido
//!
//! # Lo que se deja tocar
//!
//! SOLO el falcon del GSP (`0x110000..=0x111FFF`, alineado a 4) y el registro
//! de la BSI que dice que el GSP-RM volvio. Asi lo trajo el metal (24-09
//! 08:59): `toca falcon GSP x416`, las 416 ordenes con registro. Una orden
//! fuera de ahi para TODO el secuenciador antes de escribir el primer bit
//! (`validar`).
//!
//! # CORE_RESUME (lo unico que no es un registro)
//!
//! ```text
//!    fase 0  reset del GSP PARA EL RISC-V (OpenRM `kflcnResetIntoRiscv`);
//!            sus argumentos de LIBOS a MAILBOX0/1; arrancar el SEC2
//!    fase 1  esperar el bit 26 de `NV_PGC6_BSI_SECURE_SCRATCH_14` (0x1180F8):
//!            el GSP-RM volvio (2 s)
//!    fase 2  MAILBOX0 del SEC2 a 0; el OS del GSP = la version del
//!            bootloader; y esperar el RISC-V del GSP activo
//! ```

use crate::falcon::{self as fa, NoFuego, Reloj};
use crate::secuenciador::Orden;
use crate::{es_error_pri, Registros};

/// Lo que se deja tocar: el falcon del GSP.
pub const DESDE: u32 = fa::GSP;
pub const HASTA: u32 = fa::GSP + 0x1FFF;

/// `NV_PGC6_BSI_SECURE_SCRATCH_14`, y su bit `boot_stage_3_handoff`.
pub const BSI_14: u32 = 0x0011_80F8;
pub const VUELTA: u32 = 1 << 26;

/// Los plazos de nova-core: REG_POLL sin plazo = 4 s; esperar la parada, 2 s;
/// la vuelta del GSP-RM, 2 s. El RISC-V activo tras la vuelta: 100 ms (nova-core
/// lo mira una vez).
pub const PLAZO_POLL_US: u64 = 4_000_000;
pub const PLAZO_PARADA_US: u64 = 2_000_000;
pub const PLAZO_VUELTA_US: u64 = 2_000_000;
pub const PLAZO_RISCV_US: u64 = 100_000;

/// Lo que CORE_RESUME necesita saber.
#[derive(Clone, Copy, Debug)]
pub struct Contexto {
    /// `NV_PMC_BOOT_0`, para el reset del falcon.
    pub boot0: u32,
    /// La IOVA de los argumentos de LIBOS (a MAILBOX0/1 del GSP).
    pub libos: u64,
    /// La version del bootloader (al registro OS del GSP).
    pub os: u32,
}

/// Por que se paro.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Falla {
    /// La orden `i` toca un registro fuera del falcon del GSP (lo dice `validar`).
    Fuera(usize, u32),
    /// La orden `i` espero mas que su plazo; lo ultimo que leyo.
    Plazo(usize, u32),
    /// El registro de la orden `i` no contesta.
    NoContesta(usize, u32),
    /// Un falcon no se dejo en la orden `i`.
    Falcon(usize, NoFuego),
    /// El SEC2 acabo CORE_RESUME con este MAILBOX0.
    Sec2(u32),
}

/// Como acabo un tramo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tramo {
    /// Todas corridas.
    Hecho,
    /// Se acabo el presupuesto: llamar otra vez.
    Sigue,
    Falla(Falla),
}

/// **Toda orden dentro de lo permitido?** `Err` con la primera que no.
pub fn validar(ordenes: &[Orden]) -> Result<(), Falla> {
    for (i, o) in ordenes.iter().enumerate() {
        if let Some(reg) = o.registro() {
            if !(DESDE..=HASTA).contains(&reg) || reg % 4 != 0 {
                return Err(Falla::Fuera(i, reg));
            }
        }
    }
    Ok(())
}

/// **Donde va el secuenciador**: la orden siguiente, la fase de CORE_RESUME y
/// desde cuando se espera lo que se esta esperando (en us del reloj).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Corredor {
    pub i: usize,
    pub fase: u8,
    pub desde: Option<u64>,
}

impl Corredor {
    /// **Correr un tramo**: desde la orden `i` hasta acabar, fallar, o pasar
    /// `presupuesto_us`. Lo que se espera y no llega no bloquea: el tramo acaba
    /// en `Sigue` y la espera sigue contando en la llamada siguiente.
    pub fn correr(&mut self, ordenes: &[Orden], r: &mut impl Registros, t: &mut impl Reloj, c: &Contexto, presupuesto_us: u64) -> Tramo {
        if self.i == 0 && self.fase == 0 && self.desde.is_none() {
            if let Err(f) = validar(ordenes) {
                return Tramo::Falla(f);
            }
        }
        let inicio = t.us();
        while self.i < ordenes.len() {
            match self.una(ordenes[self.i], r, t, c) {
                Ok(true) => {
                    self.i += 1;
                    self.fase = 0;
                    self.desde = None;
                }
                Ok(false) => {}
                Err(f) => return Tramo::Falla(f),
            }
            if t.us().saturating_sub(inicio) >= presupuesto_us && self.i < ordenes.len() {
                return Tramo::Sigue;
            }
        }
        Tramo::Hecho
    }

    fn leer(&self, r: &mut impl Registros, reg: u32) -> Result<u32, Falla> {
        let v = r.leer(reg);
        if es_error_pri(v) {
            Err(Falla::NoContesta(self.i, reg))
        } else {
            Ok(v)
        }
    }

    /// Lo esperado: `Ok(true)` si ya, `Ok(false)` si todavia y hay plazo.
    fn espera(&mut self, t: &mut impl Reloj, plazo_us: u64, ya: bool, visto: u32) -> Result<bool, Falla> {
        if ya {
            return Ok(true);
        }
        let ahora = t.us();
        let desde = *self.desde.get_or_insert(ahora);
        if ahora.saturating_sub(desde) > plazo_us {
            Err(Falla::Plazo(self.i, visto))
        } else {
            Ok(false)
        }
    }

    fn falcon<T>(&self, x: Result<T, NoFuego>) -> Result<T, Falla> {
        x.map_err(|e| Falla::Falcon(self.i, e))
    }

    /// Una orden. `Ok(true)` = hecha.
    fn una(&mut self, o: Orden, r: &mut impl Registros, t: &mut impl Reloj, c: &Contexto) -> Result<bool, Falla> {
        match o {
            Orden::Escribir { reg, valor } => {
                r.escribir(reg, valor);
                Ok(true)
            }
            Orden::Modificar { reg, mascara, valor } => {
                let v = self.leer(r, reg)?;
                r.escribir(reg, (v & !mascara) | valor);
                Ok(true)
            }
            Orden::Esperar { reg, mascara, valor, plazo_us } => {
                let v = self.leer(r, reg)?;
                let plazo = if plazo_us == 0 { PLAZO_POLL_US } else { plazo_us as u64 };
                self.espera(t, plazo, v & mascara == valor, v)
            }
            Orden::Retraso { us } => {
                let ahora = t.us();
                let desde = *self.desde.get_or_insert(ahora);
                Ok(ahora.saturating_sub(desde) >= us as u64)
            }
            Orden::Guardar { reg, .. } => {
                self.leer(r, reg)?;
                Ok(true)
            }
            Orden::Resetear => {
                self.falcon(fa::resetear(r, t, fa::GSP, c.boot0))?;
                self.falcon(fa::reset_dma(r, fa::GSP))?;
                Ok(true)
            }
            Orden::Arrancar => {
                self.falcon(fa::arrancar_con(r, fa::GSP, None, None, None))?;
                Ok(true)
            }
            Orden::EsperarParada => {
                let (parado, _, _) = self.falcon(fa::como_va(r, fa::GSP))?;
                self.espera(t, PLAZO_PARADA_US, parado, r.leer(fa::GSP + fa::CPUCTL))
            }
            Orden::Reanudar => self.reanudar(r, t, c),
        }
    }

    fn reanudar(&mut self, r: &mut impl Registros, t: &mut impl Reloj, c: &Contexto) -> Result<bool, Falla> {
        if self.fase == 0 {
            // ** Al RISC-V, como OpenRM 570.144; nova-core resetea a FALCON.
            self.falcon(fa::resetear_en_riscv(r, t, fa::GSP))?;
            r.escribir(fa::GSP + fa::MAILBOX0, c.libos as u32);
            r.escribir(fa::GSP + fa::MAILBOX1, (c.libos >> 32) as u32);
            self.falcon(fa::arrancar_con(r, fa::SEC2, None, None, None))?;
            self.fase = 1;
            self.desde = None;
        }
        if self.fase == 1 {
            let v = self.leer(r, BSI_14)?;
            if !self.espera(t, PLAZO_VUELTA_US, v & VUELTA != 0, v)? {
                return Ok(false);
            }
            let (_, m0, _) = self.falcon(fa::como_va(r, fa::SEC2))?;
            if m0 != 0 {
                return Err(Falla::Sec2(m0));
            }
            r.escribir(fa::GSP + fa::OS, c.os);
            self.fase = 2;
            self.desde = None;
        }
        let (activo, _) = self.falcon(fa::riscv(r, fa::GSP))?;
        self.espera(t, PLAZO_RISCV_US, activo, r.leer(fa::GSP + fa::RISCV_CPUCTL))
    }
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    /// Una 3060 de mentira: los registros que se leen dan lo que se les dijo
    /// (o 0), y se apunta cada escritura. El DMA del falcon acaba al instante,
    /// el falcon se para cuando se le pide, y el GSP-RM "vuelve" al arrancar
    /// el SEC2.
    #[derive(Default)]
    struct Placa {
        regs: Vec<(u32, u32)>,
        escritos: Vec<(u32, u32)>,
        sec2_mbox0: u32,
        nunca_vuelve: bool,
    }

    impl Placa {
        fn reg(&self, r: u32) -> u32 {
            self.regs.iter().rev().find(|(k, _)| *k == r).map_or(0, |e| e.1)
        }
    }

    impl Registros for Placa {
        fn leer(&mut self, reg: u32) -> u32 {
            match reg {
                r if r == fa::GSP + fa::HWCFG2 || r == fa::SEC2 + fa::HWCFG2 => 1 << 31,
                r if r == fa::GSP + fa::BCR_CTRL || r == fa::SEC2 + fa::BCR_CTRL => 1,
                r if r == fa::GSP + fa::CPUCTL => 1 << 4, // parado
                r if r == fa::SEC2 + fa::MAILBOX0 => self.sec2_mbox0,
                BSI_14 => {
                    let arrancado = self.escritos.iter().any(|&(k, _)| k == fa::SEC2 + fa::CPUCTL);
                    if arrancado && !self.nunca_vuelve {
                        VUELTA
                    } else {
                        0
                    }
                }
                r if r == fa::GSP + fa::RISCV_CPUCTL => 1 << 7, // activo
                r => self.reg(r),
            }
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            self.escritos.push((reg, v));
            self.regs.push((reg, v));
        }
    }

    /// Un reloj que avanza 10 us cada vez que se mira.
    struct Tic(u64);
    impl Reloj for Tic {
        fn us(&mut self) -> u64 {
            self.0 += 10;
            self.0
        }
    }

    const C: Contexto = Contexto { boot0: 0xB760_00A1, libos: 0x3C00_0000, os: 0x1234 };

    fn todo(ordenes: &[Orden], p: &mut Placa) -> Tramo {
        let (mut c, mut t) = (Corredor::default(), Tic(0));
        loop {
            match c.correr(ordenes, p, &mut t, &C, 1000) {
                Tramo::Sigue => continue,
                fin => return fin,
            }
        }
    }

    #[test]
    fn corre_como_el_del_metal() {
        let ordenes = [
            Orden::Escribir { reg: fa::GSP + 0x040, valor: 0 },
            Orden::Resetear,
            Orden::Escribir { reg: fa::GSP + 0x600, valor: 0x114 },
            Orden::Esperar { reg: fa::GSP + 0x118, mascara: 1, valor: 0, plazo_us: 0 },
            Orden::Modificar { reg: fa::GSP + 0x600, mascara: 0xF0, valor: 0x20 },
            Orden::Retraso { us: 50 },
            Orden::Arrancar,
            Orden::EsperarParada,
            Orden::Reanudar,
        ];
        let mut p = Placa::default();
        assert_eq!(todo(&ordenes, &mut p), Tramo::Hecho);
        assert!(p.escritos.contains(&(fa::GSP + 0x600, 0x124)), "MODIFICAR: (0x114 & !0xF0) | 0x20");
        assert!(p.escritos.contains(&(fa::GSP + fa::MAILBOX0, 0x3C00_0000)), "CORE_RESUME: LIBOS al buzon");
        assert!(p.escritos.contains(&(fa::GSP + fa::OS, 0x1234)), "y el OS del GSP");
        assert!(p.escritos.iter().any(|&(k, _)| k == fa::SEC2 + fa::CPUCTL), "arranco el SEC2");
        let bcr = p.escritos.iter().rposition(|&(k, _)| k == fa::GSP + fa::BCR_CTRL).unwrap();
        assert_eq!(p.escritos[bcr].1, 0x111, "CORE_RESUME deja el GSP en RISC-V: nucleo, valido y BRFETCH");
        let sec2 = p.escritos.iter().position(|&(k, _)| k == fa::SEC2 + fa::CPUCTL).unwrap();
        assert!(bcr < sec2, "antes de arrancar el SEC2");
    }

    #[test]
    fn nada_fuera_del_falcon_del_gsp_ni_un_bit() {
        let ordenes = [
            Orden::Escribir { reg: fa::GSP + 0x040, valor: 0 },
            Orden::Escribir { reg: 0x0000_0200, valor: 0 }, // PMC_ENABLE: no
        ];
        let mut p = Placa::default();
        assert_eq!(todo(&ordenes, &mut p), Tramo::Falla(Falla::Fuera(1, 0x200)));
        assert!(p.escritos.is_empty(), "se valida TODO antes de la primera escritura");
        assert_eq!(validar(&[Orden::Escribir { reg: fa::GSP + 2, valor: 0 }]), Err(Falla::Fuera(0, fa::GSP + 2)), "sin alinear");
        assert_eq!(validar(&[Orden::Escribir { reg: fa::SEC2, valor: 0 }]), Err(Falla::Fuera(0, fa::SEC2)), "ni el SEC2");
    }

    #[test]
    fn una_espera_que_no_llega_acaba_en_su_plazo() {
        let ordenes = [Orden::Esperar { reg: fa::GSP + 0x118, mascara: 1, valor: 1, plazo_us: 500 }];
        assert_eq!(todo(&ordenes, &mut Placa::default()), Tramo::Falla(Falla::Plazo(0, 0)));
    }

    #[test]
    fn por_tramos_sigue_donde_se_quedo() {
        let ordenes = [Orden::Retraso { us: 5000 }, Orden::Escribir { reg: fa::GSP + 0x104, valor: 0x100 }];
        let (mut c, mut t, mut p) = (Corredor::default(), Tic(0), Placa::default());
        assert_eq!(c.correr(&ordenes, &mut p, &mut t, &C, 1000), Tramo::Sigue);
        assert_eq!(c.i, 0, "el retraso sigue");
        assert!(p.escritos.is_empty());
        let mut n = 1;
        while c.correr(&ordenes, &mut p, &mut t, &C, 1000) == Tramo::Sigue {
            n += 1;
        }
        assert!(n >= 4, "5 ms en tramos de 1 ms: cuatro siguen y el quinto acaba");
        assert_eq!(p.escritos, [(fa::GSP + 0x104, 0x100)]);
    }

    #[test]
    fn core_resume_dice_lo_que_falla() {
        let mut p = Placa { sec2_mbox0: 0x15, ..Default::default() };
        assert_eq!(todo(&[Orden::Reanudar], &mut p), Tramo::Falla(Falla::Sec2(0x15)));
        let mut p = Placa { nunca_vuelve: true, ..Default::default() };
        assert_eq!(todo(&[Orden::Reanudar], &mut p), Tramo::Falla(Falla::Plazo(0, 0)), "el GSP-RM no vuelve en 2 s");
    }
}
