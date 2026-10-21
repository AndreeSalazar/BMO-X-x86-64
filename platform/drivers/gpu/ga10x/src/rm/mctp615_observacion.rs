//! Fotos externas de las colas 615, sin consumir mensajes ni inferir autenticacion.
//! capa: puro -- registros prestados y tiempos aportados por el llamador.
//! [carril] VERDE
//! [estado]  VOLATIL  seis plazos por intercambio, sin reservas ni escrituras de aparato.
use crate::{es_error_pri, falcon as f, Registros};
use super::{disponibles, TX_HEAD, TX_TAIL, RX_HEAD, RX_TAIL};

pub const REGISTROS: [u32; 8] = [TX_HEAD, TX_TAIL, RX_HEAD, RX_TAIL,
    f::GSP + f::BCR_CTRL, f::GSP + f::RISCV_CPUCTL,
    f::GSP + f::MAILBOX0, f::GSP + f::MAILBOX1];
pub const PLAZOS_US: [u64; 6] = [0, 100_000, 1_000_000, 5_000_000, 15_000_000, 29_000_000];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Foto { pub crudos: [u32; 8], pub pri: u32 }
impl Foto {
    pub fn leer(r: &mut impl Registros) -> Self {
        let mut s = Self { crudos: [0; 8], pri: 0 };
        for (i, dir) in REGISTROS.iter().enumerate() {
            s.crudos[i] = r.leer(*dir);
            if es_error_pri(s.crudos[i]) { s.pri |= 1 << i; }
        }
        s
    }
    /// Solo diferencia contadores coherentes de PRI o distancia imposible.
    /// Un TX sin pendientes no acredita ejecucion ni respuesta de INIT.
    pub fn pendientes(&self) -> (Option<u32>, Option<u32>) {
        (disponibles(self.crudos[0], self.crudos[1]).ok(),
         disponibles(self.crudos[2], self.crudos[3]).ok())
    }
}

#[derive(Default)]
pub struct Seguimiento { inicio: Option<u64>, siguiente: usize }
impl Seguimiento {
    pub const fn nuevo() -> Self { Self { inicio: None, siguiente: 0 } }
    pub fn transcurrido(&self, ahora_us: u64) -> u64 {
        self.inicio.map(|inicio| ahora_us.saturating_sub(inicio)).unwrap_or(0)
    }
    /// Una sola foto al saltar plazos; no inventa fotos de tiempos omitidos.
    pub fn toca(&mut self, ahora_us: u64) -> Option<u64> {
        let inicio = *self.inicio.get_or_insert(ahora_us);
        let transcurrido = ahora_us.saturating_sub(inicio);
        if self.siguiente >= PLAZOS_US.len() || transcurrido < PLAZOS_US[self.siguiente] {
            return None;
        }
        while self.siguiente < PLAZOS_US.len() && transcurrido >= PLAZOS_US[self.siguiente] {
            self.siguiente += 1;
        }
        Some(transcurrido)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    struct Banco { visitas: std::vec::Vec<u32>, datos: [u32; 8] }
    impl Registros for Banco {
        fn leer(&mut self, dir: u32) -> u32 {
            self.visitas.push(dir);
            self.datos[REGISTROS.iter().position(|d| *d == dir).unwrap()]
        }
        fn escribir(&mut self, _: u32, _: u32) { panic!("observador escribio") }
    }
    #[test]
    fn lista_cerrada_solo_lectura_y_pri_no_parece_cola_vacia() {
        let mut b = Banco { visitas: std::vec::Vec::new(), datos: [1, 0, 0, 0, 0xBAD0_0001, 0, 0, 0] };
        let a = Foto::leer(&mut b);
        assert_eq!(b.visitas, REGISTROS);
        assert_eq!(a.pri, 16);
        assert_eq!(a.pendientes(), (Some(1), Some(0)));
        b.datos[1] = 0xBADF_1002;
        b.datos[2] = 64;
        assert_eq!(Foto::leer(&mut b).pendientes(), (None, None));
        b.datos = [1, 1, 3, 2, 0, 0, 0, 0];
        assert_eq!(Foto::leer(&mut b).pendientes(), (Some(0), Some(1)));
    }
    #[test]
    fn espera_acotada_incluye_29s_y_no_reconstruye_el_pasado() {
        let mut s = Seguimiento::default();
        let base = 500;
        for t in PLAZOS_US { assert_eq!(s.toca(base + t), Some(t)); assert_eq!(s.toca(base + t), None); }
        assert_eq!(s.toca(u64::MAX), None);
        let mut salto = Seguimiento::default();
        assert_eq!(salto.toca(100), Some(0));
        assert_eq!(salto.toca(20_000_100), Some(20_000_000));
        assert_eq!(salto.toca(20_000_101), None);
        assert_eq!(salto.toca(29_000_100), Some(29_000_000));
        assert_eq!(salto.toca(1), None);
    }
}
