//! **LAS PAGINAS**: por donde escribe el programa, y donde no puede.
//!
//! Aparte de `mod.rs` desde el 2026-09-19: la proteccion de solo lectura hizo
//! crecer el emulador por encima de las 1.000 lineas de L6a, y se pago sacando
//! la pregunta entera a su fichero, no subiendo el techo.

use super::Machine;

impl Machine {
    /// Un byte de memoria, para que los tests puedan mirar si el emisor
    /// escribio donde no debia. Sin esto, un desbordamiento de buffer solo se
    /// ve cuando ya ha corrompido otra cosa.
    pub fn read_u8_pub(&self, addr: u64) -> u8 {
        self.read_u8_mem(addr)
    }

    /// **Lo que un banco deja en memoria ANTES de correr** (2026-09-26: las
    /// matrices del cubo para las de cuatro `flotante32` de INTI). Va por el
    /// mismo camino que el programa, asi que respeta las paginas de solo
    /// lectura igual.
    pub fn pon_u64(&mut self, addr: u64, value: u64) {
        self.write_u64(addr, value);
    }

    pub(super) fn write_u64(&mut self, addr: u64, value: u64) {
        for i in 0..8 {
            self.escribe(addr + i, ((value >> (i * 8)) & 0xFF) as u8);
        }
    }

    /// **El UNICO sitio por donde el PROGRAMA escribe memoria.** Si cae en una
    /// pagina de solo lectura, revienta como el Ryzen: con la direccion y el
    /// `rip`. (Lo que escribe el KERNEL emulado --`sistema.rs`, el reparto del
    /// DIRECTOR-- va por su camino, igual que en metal va por la fisica.)
    pub(super) fn escribe(&mut self, addr: u64, b: u8) {
        self.mira_trampa(addr);
        if self.solo_lectura.iter().any(|&(d, h)| addr >= d && addr < h) {
            panic!(
                "#PF: el programa escribe en una pagina de SOLO LECTURA en {addr:#x} \
                 (rip {:#x}) -- en el Ryzen esto es un fallo de pagina",
                self.rip
            );
        }
        if let Some(v) = &mut self.escritas {
            v.push(addr);
        }
        self.mem.insert(addr, b);
    }
}

/// **Un fallo de pagina, emulado** (B2, 14-10): lo que la autopsia del kernel
/// lee de un `#PF`. `rip` es el offset en el codigo de la instruccion que
/// toco la trampa; `regs`, los dieciseis en el orden de x86 (0 `rax`, 1
/// `rcx`...), de antes de ejecutarla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FalloEmu {
    pub rip: usize,
    pub addr: u64,
    pub regs: [u64; 16],
}

impl Machine {
    /// Si `addr` cae en la trampa, se apunta (la primera): `run` para al
    /// acabar la instruccion. Va en `&self` porque leer memoria lo es.
    pub(super) fn mira_trampa(&self, addr: u64) {
        if let Some((d, h)) = self.trampa {
            if addr >= d && addr < h && self.toco.get().is_none() {
                self.toco.set(Some(addr));
            }
        }
    }
}

impl Machine {
    /// Escribe bytes en memoria: lo que una prueba siembra ENTRE dos llamadas
    /// (2026-09-23, el emisor de SPIR-V).
    pub fn escribir(&mut self, addr: u64, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.escribe(addr + i as u64, *b);
        }
    }
}
