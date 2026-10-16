//! Los bytes tal como estan, sin interpretarlos como instrucciones.
//! Direccion creciente; cada byte en hexadecimal y en ocho bits (bit 7 a 0).
//! Un inmediato little-endian conserva el orden de memoria: no se invierte.

use core::fmt::{self, Write};

pub const POR_FILA: usize = 4;
/// Dieciseis filas: una consulta acotada, sin volcar un ejecutable a la consola.
pub const POR_PAGINA: usize = 64;

/// Un intervalo del FICHERO, con fin exclusivo; nunca una direccion virtual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pagina {
    pub desde: u64,
    pub hasta: u64,
    pub bytes: usize,
    pub siguiente: Option<u64>,
}

/// Recorrer todos los bytes en consultas acotadas. En EOF, pagina vacia;
/// despues del EOF, `None`. La medida es la del fichero recien abierto.
pub fn pagina(total: u64, desde: u64) -> Option<Pagina> {
    let bytes = total.checked_sub(desde)?.min(POR_PAGINA as u64) as usize;
    let hasta = desde + bytes as u64; // <= total, tambien con total == u64::MAX
    Some(Pagina { desde, hasta, bytes, siguiente: (hasta < total).then_some(hasta) })
}

/// Una fila completa, sin monton ni truncamiento silencioso.
pub struct Fila {
    buf: [u8; 72],
    n: usize,
}

impl Fila {
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.n]).unwrap_or("")
    }
}

impl Write for Fila {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if s.len() > self.buf.len() - self.n { return Err(fmt::Error); }
        self.buf[self.n..self.n + s.len()].copy_from_slice(s.as_bytes());
        self.n += s.len();
        Ok(())
    }
}

/// De uno a cuatro bytes; `None` si no caben o su direccion se desborda.
/// No decodifica: incluso datos, bytes cortados y opcodes desconocidos salen.
pub fn fila(en: u64, bytes: &[u8]) -> Option<Fila> {
    if bytes.is_empty() || bytes.len() > POR_FILA { return None; }
    en.checked_add(bytes.len() as u64 - 1)?;
    let mut f = Fila { buf: [0; 72], n: 0 };
    write!(f, "{en:016x}  ").ok()?;
    for k in 0..POR_FILA {
        if let Some(b) = bytes.get(k) { write!(f, "{b:02x} ").ok()?; }
        else { f.write_str("   ").ok()?; }
    }
    f.write_str("| ").ok()?;
    for (k, b) in bytes.iter().enumerate() {
        if k > 0 { f.write_str(" ").ok()?; }
        write!(f, "{b:08b}").ok()?;
    }
    Some(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_cover_the_whole_file_once_including_partial_and_empty() {
        for total in 0..=257u64 {
            let mut en = 0;
            let mut leidos = 0;
            loop {
                let p = pagina(total, en).unwrap();
                assert_eq!(p.desde, leidos);
                assert!(p.bytes <= POR_PAGINA);
                assert_eq!(p.hasta - p.desde, p.bytes as u64);
                leidos += p.bytes as u64;
                match p.siguiente {
                    Some(n) => { assert_eq!(n, leidos); assert!(n > en); en = n; }
                    None => break,
                }
            }
            assert_eq!(leidos, total);
            assert_eq!(pagina(total, total).unwrap().bytes, 0);
            assert!(pagina(total, total + 1).is_none());
        }
        let p = pagina(u64::MAX, u64::MAX - 3).unwrap();
        assert_eq!(p.bytes, 3);
        assert_eq!(p.hasta, u64::MAX);
        assert_eq!(p.siguiente, None);
    }

    #[test]
    fn bytes_keep_memory_order_and_leading_zeroes() {
        assert_eq!(fila(0x1000, &[0x48, 0x83, 0xc0, 1]).unwrap().as_str(),
            "0000000000001000  48 83 c0 01 | 01001000 10000011 11000000 00000001");
        assert_eq!(fila(0x1004, &[0]).unwrap().as_str(),
            "0000000000001004  00          | 00000000");
        assert!(fila(0, &[]).is_none());
        assert!(fila(0, &[0; 5]).is_none());
        assert!(fila(u64::MAX, &[0, 1]).is_none());
        assert!(fila(u64::MAX, &[0xff]).is_some());
    }

    #[test]
    fn every_byte_roundtrips_even_without_an_instruction() {
        for b in 0u8..=255 {
            let f = fila(b as u64, &[b]).unwrap();
            let (hex, bits) = f.as_str().split_once("| ").unwrap();
            assert_eq!(u8::from_str_radix(hex.split_whitespace().nth(1).unwrap(), 16), Ok(b));
            assert_eq!(u8::from_str_radix(bits, 2), Ok(b));
        }
    }
}
