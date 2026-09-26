//! **EL GSP-RM POR DENTRO, SIN TRAERSELO (L0c1)** -- las secciones del ELF de
//! 63 MB, encontradas leyendo SOLO sus cabeceras.
//!
//! capa: puro -- pide bytes a una `Fuente` y devuelve donde esta cada seccion; no toca un registro (L8)
//!
//! [estado]  RAM WPR       los bytes del GSP-RM, en la RAM del PC, que el booter copia DENTRO de la WPR2
//! [eje]     CORRECCION -- la seccion equivocada es otro firmware, o la firma
//!           de otra familia de GPU
//!
//! # Lo que hay dentro de `gsp-570.144.bin` (readelf, 24-09)
//!
//! ```text
//!    .fwimage             0x3C99000 B   el GSP-RM: lo que se presta al GSP
//!                                        por la tabla radix3
//!    .fwversion           8 B
//!    .fwsignature_ga10x   4 KiB         la firma de Ampere GA10x -- la 3060
//!    .fwsignature_ad10x, _gh100, _gb10x ... las de otras familias
//! ```
//!
//! nova-core (`firmware/gsp.rs::elf64_section`) se trae el fichero entero y lo
//! recorre en memoria. Aqui no: son 63 MB, y en BMO-X abrir un archivo con
//! ventana y saltar es lo barato. Por eso esto habla con una [`Fuente`] --en el
//! escritorio, `saltar` + `leer_en`; en las pruebas, un `&[u8]`-- y le pide la
//! cabecera (64 B), la tabla de secciones (64 B cada una) y los nombres.

/// **De donde salen los bytes.** `false` si no se pudo leer ENTERO.
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

/// La imagen que se presta al GSP.
pub const IMAGEN: &[u8] = b".fwimage";
/// La firma de la familia de la 3060.
pub const FIRMA_GA10X: &[u8] = b".fwsignature_ga10x";
/// Mas secciones que esto no es un GSP-RM (tiene 14): se para antes de leer
/// una tabla que un fichero roto diga que mide 65535.
pub const MAX_SECCIONES: u16 = 64;

/// Por que no.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoElf {
    /// No se pudo leer lo que hacia falta.
    Ilegible,
    /// No es un ELF64 little-endian.
    NoEsElf64,
    /// La tabla de secciones no tiene la forma de un ELF64.
    TablaRara,
    /// No hay ninguna seccion con ese nombre.
    NoEsta,
}

/// Una seccion: donde empieza DEL FICHERO, y cuanto mide.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Seccion {
    pub desde: u64,
    pub bytes: u64,
}

fn u16_de(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_de(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u64_de(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3], b[o + 4], b[o + 5], b[o + 6], b[o + 7]])
}

/// La cabecera de una seccion: (nombre en la tabla de nombres, offset, medida).
fn cabecera<F: Fuente>(f: &mut F, tabla: u64, i: u16) -> Result<(u32, u64, u64), NoElf> {
    let mut s = [0u8; 64];
    if !f.leer(tabla + i as u64 * 64, &mut s) {
        return Err(NoElf::Ilegible);
    }
    Ok((u32_de(&s, 0), u64_de(&s, 24), u64_de(&s, 32)))
}

/// **Donde esta la seccion `nombre`** (`elf64_section`).
pub fn seccion<F: Fuente>(f: &mut F, nombre: &[u8]) -> Result<Seccion, NoElf> {
    let mut h = [0u8; 64];
    if !f.leer(0, &mut h) {
        return Err(NoElf::Ilegible);
    }
    // 0x7F 'E' 'L' 'F', clase 2 (64 bits), datos 1 (little-endian).
    if h[0..4] != *b"\x7FELF" || h[4] != 2 || h[5] != 1 {
        return Err(NoElf::NoEsElf64);
    }
    let tabla = u64_de(&h, 0x28);
    let (largo, cuantas, nombres) = (u16_de(&h, 0x3A), u16_de(&h, 0x3C), u16_de(&h, 0x3E));
    if largo != 64 || cuantas == 0 || cuantas > MAX_SECCIONES || nombres >= cuantas {
        return Err(NoElf::TablaRara);
    }
    let (_, nombres_en, nombres_bytes) = cabecera(f, tabla, nombres)?;
    // El nombre buscado y su NUL, que es lo que distingue `.fwimage` de un
    // `.fwimage2` que alguien agregue luego a la familia.
    let mut buf = [0u8; 64];
    let n = nombre.len() + 1;
    if n > buf.len() {
        return Err(NoElf::NoEsta);
    }
    for i in 0..cuantas {
        let (en, desde, bytes) = cabecera(f, tabla, i)?;
        if en as u64 + n as u64 > nombres_bytes {
            continue;
        }
        if !f.leer(nombres_en + en as u64, &mut buf[..n]) {
            return Err(NoElf::Ilegible);
        }
        if &buf[..n - 1] == nombre && buf[n - 1] == 0 {
            return Ok(Seccion { desde, bytes });
        }
    }
    Err(NoElf::NoEsta)
}

// ===================================================================
//  PRUEBAS -- un ELF64 de mentira con las secciones del GSP-RM
// ===================================================================

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec;
    use std::vec::Vec;

    /// Tres secciones: la nula, `.fwimage` de 0x3000 en 0x40, `.fwsignature_ga10x`
    /// de 0x1000, y `.shstrtab`. Como `gsp-570.144.bin`, en chico.
    fn elf() -> Vec<u8> {
        let nombres = b"\0.fwimage\0.fwsignature_ga10x\0.shstrtab\0";
        let img = (0x40u64, 0x3000u64);
        let fir = (0x3040u64, 0x1000u64);
        let str_en = 0x4040u64;
        let tabla = str_en + 0x40;
        let mut f = vec![0u8; (tabla + 4 * 64) as usize];
        f[0..6].copy_from_slice(b"\x7FELF\x02\x01");
        f[0x28..0x30].copy_from_slice(&tabla.to_le_bytes());
        f[0x3A..0x3C].copy_from_slice(&64u16.to_le_bytes());
        f[0x3C..0x3E].copy_from_slice(&4u16.to_le_bytes());
        f[0x3E..0x40].copy_from_slice(&3u16.to_le_bytes());
        f[str_en as usize..str_en as usize + nombres.len()].copy_from_slice(nombres);
        let pon = |f: &mut Vec<u8>, i: u64, nombre: u32, desde: u64, bytes: u64| {
            let o = (tabla + i * 64) as usize;
            f[o..o + 4].copy_from_slice(&nombre.to_le_bytes());
            f[o + 24..o + 32].copy_from_slice(&desde.to_le_bytes());
            f[o + 32..o + 40].copy_from_slice(&bytes.to_le_bytes());
        };
        pon(&mut f, 1, 1, img.0, img.1);
        pon(&mut f, 2, 10, fir.0, fir.1);
        pon(&mut f, 3, 29, str_en, nombres.len() as u64);
        f
    }

    #[test]
    fn la_imagen_y_la_firma_de_la_3060_se_encuentran() {
        let f = elf();
        let mut s: &[u8] = &f;
        assert_eq!(seccion(&mut s, IMAGEN), Ok(Seccion { desde: 0x40, bytes: 0x3000 }));
        assert_eq!(seccion(&mut s, FIRMA_GA10X), Ok(Seccion { desde: 0x3040, bytes: 0x1000 }));
        assert_eq!(seccion(&mut s, b".fwsignature_ad10x"), Err(NoElf::NoEsta));
        assert_eq!(seccion(&mut s, b".fwimag"), Err(NoElf::NoEsta), "un prefijo no es el nombre");
    }

    #[test]
    fn lo_que_no_es_un_elf64_no_se_recorre() {
        let mut f = elf();
        f[4] = 1;
        let mut s: &[u8] = &f;
        assert_eq!(seccion(&mut s, IMAGEN), Err(NoElf::NoEsElf64));
        let mut f = elf();
        f[0x3C..0x3E].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let mut s: &[u8] = &f;
        assert_eq!(seccion(&mut s, IMAGEN), Err(NoElf::TablaRara));
        let f = elf();
        let mut s: &[u8] = &f[..100];
        assert_eq!(seccion(&mut s, IMAGEN), Err(NoElf::Ilegible));
    }

    #[test]
    fn bytes_hostiles_nunca_revientan() {
        let base = elf();
        for i in (0..0x40).chain(0x4040..base.len()) {
            for v in [0x00, 0x7F, 0xFF] {
                let mut f = base.clone();
                f[i] = v;
                let mut s: &[u8] = &f;
                let _ = seccion(&mut s, FIRMA_GA10X);
            }
        }
    }

    /// El de VERDAD (`BMO_FW_GSP`, ver `booter.rs`), leido sin traerselo:
    /// una Fuente que cuenta los bytes que pide.
    #[test]
    fn el_gsp_de_linux_firmware_si_esta() {
        let Ok(dir) = std::env::var("BMO_FW_GSP") else { return };
        use std::io::{Read, Seek, SeekFrom};
        struct Disco(std::fs::File, u64);
        impl Fuente for Disco {
            fn leer(&mut self, desde: u64, dst: &mut [u8]) -> bool {
                self.1 += dst.len() as u64;
                self.0.seek(SeekFrom::Start(desde)).is_ok() && self.0.read_exact(dst).is_ok()
            }
        }
        let mut d = Disco(std::fs::File::open(std::format!("{}/gsp-570.144.bin", dir)).unwrap(), 0);
        assert_eq!(seccion(&mut d, IMAGEN).unwrap(), Seccion { desde: 0x40, bytes: 0x3C99000 });
        assert_eq!(seccion(&mut d, FIRMA_GA10X).unwrap().bytes, 0x1000);
        assert!(d.1 < 8192, "leyo {} bytes de 63 MB", d.1);
    }
}
