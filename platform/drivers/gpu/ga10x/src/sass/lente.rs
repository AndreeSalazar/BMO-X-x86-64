//! La pregunta: que campos puede leer el juez en estos 16 bytes SM86.
//! [carril] VERDE -- interpreta copias; no toca la tarjeta
//! [consumo] NADA -- solo bajo consulta
//!
//! Reconocida no significa aprobada: falta juzgar el programa y su contexto.
//! No elimina cabeceras SPH ni rellena instrucciones cortadas.

use super::juez_leer::decodificar;

pub const INSTRUCTION_BYTES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Misaligned, Truncated, OffsetOverflow }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub offset: u64,
    pub lo: u64,
    pub hi: u64,
    pub opcode: u16,
    pub form: u8,
    pub guard: u8,
    /// Solo cuando el decodificador del juez reconoce la palabra completa.
    pub decoded: Option<Decoded>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub stall: u32,
    pub write_barrier: u32,
    pub read_barrier: u32,
    pub wait_mask: u32,
    pub writes: (u8, u8),
    pub reads: [(u8, u8); 4],
    pub branch: bool,
    pub exit: bool,
    pub memory: bool,
}

/// La primera palabra en orden de memoria es `lo`. `offset` es relativo al
/// comienzo del CODIGO, no al BSF, SPH o una direccion virtual supuesta.
pub fn read(bytes: &[u8], offset: u64) -> Result<Instruction, Error> {
    if offset % INSTRUCTION_BYTES as u64 != 0 { return Err(Error::Misaligned); }
    offset.checked_add(INSTRUCTION_BYTES as u64 - 1).ok_or(Error::OffsetOverflow)?;
    let bytes = bytes.get(..INSTRUCTION_BYTES).ok_or(Error::Truncated)?;
    let lo = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let hi = u64::from_le_bytes(bytes[8..].try_into().unwrap());
    let decoded = decodificar(lo, hi).map(|i| Decoded {
        stall: i.espera, write_barrier: i.bar_escritura, read_barrier: i.bar_lectura,
        wait_mask: i.mascara, writes: i.escribe, reads: i.lee,
        branch: i.salto, exit: i.fin, memory: i.memoria,
    });
    Ok(Instruction { offset, lo, hi, opcode: (lo & 0x1ff) as u16, form: (lo >> 9 & 7) as u8, guard: (lo >> 12 & 15) as u8, decoded })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stored_exit_word_keeps_all_128_bits_and_control() {
        // Palabra de oro EXIT, conservada por el codificador de la casa.
        let (lo, hi) = (0x000000000000794du64, 0x000fc00003800000u64);
        assert_eq!(bmo_sm86::codifica::exit(0x7e0), (lo, hi));
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&lo.to_le_bytes()); bytes[8..].copy_from_slice(&hi.to_le_bytes());
        let i = read(&bytes, 32).unwrap();
        assert_eq!((i.offset, i.lo, i.hi, i.opcode, i.guard), (32, lo, hi, 0x14d, 7));
        let d = i.decoded.unwrap();
        assert!(d.exit); assert!(!d.branch);
        assert_eq!((d.stall, d.write_barrier, d.read_barrier, d.wait_mask), (0, 7, 7, 0));
    }
    #[test]
    fn unknown_is_kept_but_never_given_invented_semantics() {
        let bytes = [0; 16];
        let i = read(&bytes, 0).unwrap();
        assert_eq!((i.lo, i.hi), (0, 0)); assert_eq!(i.decoded, None);
    }
    #[test]
    fn truncated_unaligned_and_extreme_offsets_are_explicit() {
        for n in 0..16 { assert_eq!(read(&[0; 16][..n], 0), Err(Error::Truncated)); }
        assert_eq!(read(&[0; 16], 1), Err(Error::Misaligned));
        assert!(read(&[0; 16], u64::MAX - 15).is_ok());
    }
}
