//! Lectura a saltos, extraida de ga10x::elf sin cambiar su contrato.
//! capa: puro -- bytes del llamador, nunca direcciones que el lector siga
//! [carril] VERDE
//! [consumo] NADA -- solo cuando se pide una lectura
//! [cuesta] NADA -- una copia ilegible se rechaza

/// De donde salen los bytes. `false` si no se pudo leer ENTERO.
pub trait Fuente {
    fn leer(&mut self, desde: u64, dst: &mut [u8]) -> bool;
}

impl Fuente for &[u8] {
    fn leer(&mut self, desde: u64, dst: &mut [u8]) -> bool {
        let Ok(o) = usize::try_from(desde) else { return false };
        match self.get(o..o.saturating_add(dst.len())) {
            Some(s) if s.len() == dst.len() => {
                dst.copy_from_slice(s);
                true
            }
            _ => false,
        }
    }
}
