//! Observacion externa HS: lista cerrada, solo lecturas, sin inferir autenticacion.
//! capa: puro -- el kernel proporciona Registros; no hay MMIO ni reservas aqui.
//! [carril] VERDE
//! [estado]  VOLATIL  crudos de una fase, no una instantanea atomica del motor.
use crate::{es_error_pri, falcon as f, Registros};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motor { Gsp, Sec2 }
impl Motor {
    pub const fn base(self) -> u32 { match self { Self::Gsp => f::GSP, Self::Sec2 => f::SEC2 } }
}
pub const CAMPOS: usize = 21;
pub const REGISTROS: [u32; CAMPOS] = [
    f::MAILBOX0, f::MAILBOX1, f::CPUCTL, f::BCR_CTRL, f::RISCV_CPUCTL,
    f::HWCFG, f::HWCFG1, f::HWCFG2, f::ENGINE, f::DMACTL, f::TRANSCFG,
    f::DMATRFBASE, f::DMATRFBASE1, f::DMATRFMOFFS, f::DMATRFFBOFFS,
    f::DMATRFCMD, f::BOOTVEC, f::BROM_PARAADDR, f::BROM_ENGIDMASK,
    f::BROM_UCODE_ID, f::MOD_SEL,
];
pub const NOMBRES: [&str; CAMPOS] = [
    "MAILBOX0", "MAILBOX1", "CPUCTL", "BCR_CTRL", "RISCV_CPUCTL",
    "HWCFG", "HWCFG1", "HWCFG2", "ENGINE", "DMACTL", "TRANSCFG",
    "DMATRFBASE", "DMATRFBASE1", "DMATRFMOFFS", "DMATRFFBOFFS",
    "DMATRFCMD", "BOOTVEC", "BROM_PARAADDR", "BROM_ENGIDMASK",
    "BROM_UCODE_ID", "MOD_SEL",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Foto { pub motor: Motor, pub crudos: [u32; CAMPOS], pub pri: u32 }
impl Foto {
    pub fn leer(r: &mut impl Registros, motor: Motor) -> Self {
        let mut foto = Self { motor, crudos: [0; CAMPOS], pri: 0 };
        for (i, offset) in REGISTROS.into_iter().enumerate() {
            let v = r.leer(motor.base() + offset);
            foto.crudos[i] = v;
            if es_error_pri(v) { foto.pri |= 1 << i; }
        }
        foto
    }
    /// Some(0) es cero leido; None es PRI, nunca un estado positivo.
    pub fn valor(&self, campo: usize) -> Option<u32> {
        self.crudos.get(campo).copied().filter(|_| self.pri & (1 << campo) == 0)
    }
    pub fn dma(&self) -> Option<(bool, bool)> {
        self.valor(15).map(|v| (v & 1 != 0, v & 2 != 0))
    }
    pub fn nucleo_riscv(&self) -> Option<bool> { self.valor(3).map(|v| v & 16 != 0) }
    pub fn actividad_riscv(&self) -> Option<(bool, bool)> {
        self.valor(4).map(|v| (v & 128 != 0, v & 16 != 0))
    }
}

pub const fn fallo(e: f::NoFuego) -> (&'static str, u32) {
    match e {
        f::NoFuego::NoContesta(reg) => ("HS fallo: registro inaccesible por PRI; direccion", reg),
        f::NoFuego::NoLimpia => ("HS fallo: memoria TCM no termino de limpiarse", 0),
        f::NoFuego::NoNucleo => ("HS fallo: seleccion de nucleo no confirmada", 0),
        f::NoFuego::DmemChica(n) => ("HS fallo: capacidad DMEM insuficiente", n),
        f::NoFuego::DmaNoAcaba(n) => ("HS fallo: DMA no llego a IDLE; offset del bloque", n),
        f::NoFuego::DmaColaLlena(n) => ("HS fallo: cola DMA FULL; offset aun no publicado", n),
        f::NoFuego::DmaFuera => ("HS fallo: intervalo DMA/TCM invalido; ver entrada, no retorno de firmware", 0),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    struct Banco { i: usize, motor: Motor, dato: u32 }
    impl Registros for Banco {
        fn leer(&mut self, reg: u32) -> u32 {
            assert_eq!(reg, self.motor.base()+REGISTROS[self.i]);
            self.i += 1;
            self.dato
        }
        fn escribir(&mut self, _: u32, _: u32) { panic!("observar HS no escribe"); }
    }
    #[test]
    fn ambos_motores_usan_la_lista_cerrada_sin_lecturas_de_tcm() {
        for motor in [Motor::Gsp, Motor::Sec2] {
            let mut b = Banco { i: 0, motor, dato: 0 };
            let f = Foto::leer(&mut b, motor);
            assert_eq!(b.i, CAMPOS);
            assert_eq!(f.pri, 0);
            assert_eq!(f.valor(0), Some(0));
            assert_eq!(f.valor(CAMPOS), None);
            assert_eq!(f.dma(), Some((false, false)));
        }
    }
    #[test]
    fn pri_no_se_convierte_en_full_idle_actividad_o_seleccion() {
        for dato in [0xBADF1002, 0xBADF5620, 0xBAD00100, u32::MAX] {
            let f = Foto::leer(&mut Banco { i: 0, motor: Motor::Gsp, dato }, Motor::Gsp);
            assert_eq!(f.crudos, [dato; CAMPOS]);
            assert_eq!(f.pri, (1 << CAMPOS)-1);
            assert_eq!(f.dma(), None);
            assert_eq!(f.nucleo_riscv(), None);
            assert_eq!(f.actividad_riscv(), None);
        }
    }
    #[test]
    fn seleccion_actividad_halt_y_full_idle_son_datos_distintos() {
        let mut f = Foto { motor: Motor::Sec2, crudos: [0; CAMPOS], pri: 0 };
        f.crudos[3] = 0x111; f.crudos[4] = 16; f.crudos[15] = 3;
        assert_eq!(f.nucleo_riscv(), Some(true));
        assert_eq!(f.actividad_riscv(), Some((false, true)));
        assert_eq!(f.dma(), Some((true, true)));
        f.crudos[4] = 128;
        assert_eq!(f.actividad_riscv(), Some((true, false)));
    }
}
