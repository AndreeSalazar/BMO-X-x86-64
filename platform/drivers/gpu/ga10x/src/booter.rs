//! **LOS BINARIOS DE NVIDIA QUE LA VBIOS NO TRAE, LEIDOS (L0c1)** -- el
//! booter del SEC2 y el bootloader RISC-V del GSP, entendidos sobre bytes.
//!
//! capa: puro -- recibe el fichero entero y devuelve donde esta cada cosa; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- un offset mal leido aqui es un firmware firmado que
//!           la ROM del SEC2 rechaza, o una firma puesta encima del codigo
//!
//! # El formato (nova-core, `firmware.rs` y `firmware/booter.rs` de Linux 6.18)
//!
//! Los tres chicos de `linux-firmware/nvidia/ga102/gsp/` empiezan igual:
//!
//! ```text
//!    BinHdr (24 B)   magia 0x10DE, version, medida, cabecera, datos, bytes
//!                    -- `datos..datos+bytes` es el ucode que se carga
//! ```
//!
//! Y en `cabecera` cada uno lleva lo suyo:
//!
//! ```text
//!    booter_load / booter_unload   HsHeaderV2 (36 B): donde estan las firmas,
//!                                  el punto de parche, los parametros de la
//!                                  firma y la cabecera de carga (IMEM/DMEM)
//!    bootloader                    RmRiscvUCodeDesc (56 B): donde empiezan el
//!                                  codigo, los datos y el manifiesto
//! ```
//!
//! [!] Los offsets de HsHeaderV2 son DEL FICHERO; el punto de parche y los de
//! la cabecera de carga son DEL UCODE (desde `datos`). nova-core los mezcla sin
//! decirlo y aqui se dicen: es justo donde un parche cae un KiB mas alla.
//!
//! # La firma del booter NO se elige como la de FWSEC
//!
//! FWSEC cuenta bits en `signature_versions` (`vbios::indice_de_firma`). El
//! booter RESTA: indice = `fuse_ver` del fichero - version quemada en el
//! fusible, y un fusible a 0 pide la ULTIMA (`FUSE_VERSION_USE_LAST_SIG`).

use crate::vbios::version_del_fusible;

/// `bin_magic` de los binarios de NVIDIA.
pub const MAGIA: u32 = 0x10DE;
/// Lo que mide `BinHdr`.
pub const BIN_HDR: usize = 24;
/// Lo que mide `HsHeaderV2`.
pub const HS_HDR: usize = 36;
/// Lo que mide `RmRiscvUCodeDesc`.
pub const RISCV_DESC: usize = 56;

/// Por que no.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoBooter {
    /// Mas corto que su propia cabecera.
    Corto,
    /// No empieza por 0x10DE: no es un binario de NVIDIA.
    SinMagia,
    /// Algo de lo que la cabecera promete cae fuera del fichero.
    Fuera,
    /// Los parametros de la firma no caben en lo que dice el formato.
    FirmaRara,
    /// Ninguna firma sirve para la version del fusible.
    SinFirma,
}

fn u32_en(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o.checked_add(4)?)?.try_into().ok()?))
}

/// Un rango `desde..desde+bytes` dentro de `medida`, sin desbordar.
const fn cabe(desde: u32, bytes: u32, medida: u32) -> bool {
    match desde.checked_add(bytes) {
        Some(fin) => fin <= medida,
        None => false,
    }
}

// -- BinHdr ------------------------------------------------------------------

/// `BinHdr`: donde esta la cabecera propia y donde el ucode.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Bin {
    pub version: u32,
    pub cabecera: u32,
    pub datos: u32,
    pub bytes: u32,
}

impl Bin {
    /// El ucode: lo que se carga en el falcon, o lo que se presta al GSP.
    pub fn ucode<'a>(&self, f: &'a [u8]) -> &'a [u8] {
        &f[self.datos as usize..(self.datos + self.bytes) as usize]
    }
}

/// **Leer el `BinHdr`**, y que lo que promete este DENTRO del fichero.
pub fn bin(f: &[u8]) -> Result<Bin, NoBooter> {
    if f.len() < BIN_HDR {
        return Err(NoBooter::Corto);
    }
    let w = |o| u32_en(f, o).unwrap_or(0);
    if w(0) != MAGIA {
        return Err(NoBooter::SinMagia);
    }
    let medida = f.len().min(u32::MAX as usize) as u32;
    let b = Bin { version: w(4), cabecera: w(12), datos: w(16), bytes: w(20) };
    if !cabe(b.datos, b.bytes, medida) || b.cabecera >= medida {
        return Err(NoBooter::Fuera);
    }
    Ok(b)
}

// -- El booter (SEC2) ---------------------------------------------------------

/// Una carga por DMA: de `origen` en el ucode a `destino` en la IMEM/DMEM.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Tramo {
    pub origen: u32,
    pub destino: u32,
    pub bytes: u32,
}

/// **Lo que dice un booter** (`booter_load` o `booter_unload`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Booter {
    pub bin: Bin,
    /// Donde empieza la primera firma, DEL FICHERO.
    pub firmas_en: u32,
    pub firmas: u32,
    pub firma_bytes: u32,
    /// Donde va la firma, DEL UCODE.
    pub parche: u32,
    /// `HsSignatureParams`.
    pub fuse_ver: u32,
    pub engine_id_mask: u16,
    pub ucode_id: u8,
    /// Lo que va a la IMEM (la app 0) y a la DMEM (los datos del OS).
    pub imem: Tramo,
    pub dmem: Tramo,
}

impl Booter {
    /// `BROM_PARAADDR`: la firma, contada desde el principio de la DMEM.
    pub const fn pkc_data_offset(&self) -> u32 {
        self.parche.wrapping_sub(self.dmem.origen)
    }

    /// Donde arranca el falcon (`boot_addr`): el principio de la app 0.
    pub const fn arranque(&self) -> u32 {
        self.imem.origen
    }

    /// **Cual de las firmas pide ESTE fusible** (su valor crudo). `None` si
    /// ninguna: la ROM del SEC2 rechazaria cualquiera.
    pub const fn indice_de_firma(&self, fusible_crudo: u32) -> Option<u32> {
        if self.firmas == 0 {
            return None;
        }
        let hw = version_del_fusible(fusible_crudo);
        let i = if hw == 0 {
            self.firmas - 1
        } else {
            match self.fuse_ver.checked_sub(hw) {
                Some(i) => i,
                None => return None,
            }
        };
        if i < self.firmas { Some(i) } else { None }
    }

    /// Los bytes de la firma `i`, del fichero.
    pub fn firma<'a>(&self, f: &'a [u8], i: u32) -> Option<&'a [u8]> {
        if i >= self.firmas {
            return None;
        }
        let o = self.firmas_en as usize + (i * self.firma_bytes) as usize;
        f.get(o..o + self.firma_bytes as usize)
    }
}

/// **Leer un booter** (`HsFirmwareV2` + `HsSignatureParams` + `HsLoadHeaderV2`).
pub fn booter(f: &[u8]) -> Result<Booter, NoBooter> {
    let b = bin(f)?;
    let medida = f.len().min(u32::MAX as usize) as u32;
    let h = b.cabecera as usize;
    if !cabe(b.cabecera, HS_HDR as u32, medida) {
        return Err(NoBooter::Fuera);
    }
    let w = |o: usize| u32_en(f, o).ok_or(NoBooter::Fuera);
    let (sig_prod, sig_bytes) = (w(h)?, w(h + 4)?);
    let (patch_loc_en, patch_sig_en) = (w(h + 8)? as usize, w(h + 12)? as usize);
    let (meta_en, meta_bytes) = (w(h + 16)?, w(h + 20)?);
    let (num_sig_en, carga_en) = (w(h + 24)? as usize, w(h + 28)? as usize);

    let firmas = w(num_sig_en)?;
    let parche = w(patch_loc_en)?;
    let firmas_en = sig_prod.checked_add(w(patch_sig_en)?).ok_or(NoBooter::Fuera)?;
    let firma_bytes = if firmas == 0 { 0 } else { sig_bytes / firmas };
    if firmas != 0 && (firma_bytes == 0 || !cabe(firmas_en, sig_bytes, medida)) {
        return Err(NoBooter::Fuera);
    }

    if meta_bytes != 12 || !cabe(meta_en, 12, medida) {
        return Err(NoBooter::FirmaRara);
    }
    let m = meta_en as usize;
    let (fuse_ver, mascara, id) = (w(m)?, w(m + 4)?, w(m + 8)?);
    if mascara > u16::MAX as u32 || id == 0 || id > 16 {
        return Err(NoBooter::FirmaRara);
    }

    // HsLoadHeaderV2: codigo, datos y cuantas apps; detras, la app 0.
    let (datos_en, datos_bytes, apps) = (w(carga_en + 8)?, w(carga_en + 12)?, w(carga_en + 16)?);
    if apps == 0 {
        return Err(NoBooter::Fuera);
    }
    let (app_en, app_bytes) = (w(carga_en + 20)?, w(carga_en + 24)?);

    let bb = Booter {
        bin: b,
        firmas_en,
        firmas,
        firma_bytes,
        parche,
        fuse_ver,
        engine_id_mask: mascara as u16,
        ucode_id: id as u8,
        imem: Tramo { origen: app_en, destino: 0, bytes: app_bytes },
        dmem: Tramo { origen: datos_en, destino: 0, bytes: datos_bytes },
    };
    // Todo lo que se carga y el parche, DENTRO del ucode; y la firma, en la DMEM.
    let u = b.bytes;
    if !cabe(app_en, app_bytes, u) || !cabe(datos_en, datos_bytes, u) || !cabe(parche, firma_bytes, u) {
        return Err(NoBooter::Fuera);
    }
    if parche < datos_en || !cabe(parche - datos_en, firma_bytes, datos_bytes) {
        return Err(NoBooter::FirmaRara);
    }
    Ok(bb)
}

/// **Poner la firma `i`** en su sitio del ucode (`patch_signature`). `ucode`
/// es la copia que se va a prestar, no el fichero.
pub fn firmar(f: &[u8], b: &Booter, i: u32, ucode: &mut [u8]) -> Result<(), NoBooter> {
    let firma = b.firma(f, i).ok_or(NoBooter::SinFirma)?;
    let o = b.parche as usize;
    let d = ucode.get_mut(o..o + firma.len()).ok_or(NoBooter::Fuera)?;
    d.copy_from_slice(firma);
    Ok(())
}

// -- El bootloader RISC-V (GSP) ------------------------------------------------

/// **Lo que dice el bootloader** (`RmRiscvUCodeDesc`): lo que va, tal cual,
/// en `GspFwWprMeta`. Los offsets son DEL UCODE.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Riscv {
    pub bin: Bin,
    pub version: u32,
    pub codigo: u32,
    pub datos: u32,
    pub manifiesto: u32,
    pub app_version: u32,
}

/// **Leer el bootloader.** De los 14 campos, los que nova-core usa.
pub fn riscv(f: &[u8]) -> Result<Riscv, NoBooter> {
    let b = bin(f)?;
    let medida = f.len().min(u32::MAX as usize) as u32;
    if !cabe(b.cabecera, RISCV_DESC as u32, medida) {
        return Err(NoBooter::Fuera);
    }
    let h = b.cabecera as usize;
    let w = |k: usize| u32_en(f, h + 4 * k).unwrap_or(0);
    let r = Riscv {
        bin: b,
        version: w(0),
        app_version: w(7),
        manifiesto: w(8),
        datos: w(10),
        codigo: w(12),
    };
    // El codigo y los datos del monitor, dentro del ucode.
    if !cabe(r.codigo, w(13), b.bytes) || !cabe(r.datos, w(11), b.bytes) || !cabe(r.manifiesto, w(9), b.bytes) {
        return Err(NoBooter::Fuera);
    }
    Ok(r)
}

// ===================================================================
//  PRUEBAS -- los numeros de booter_load-570.144 y bootloader-570.144,
//  armados byte a byte; y los ficheros de verdad si estan a mano
// ===================================================================

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec;
    use std::vec::Vec;

    fn p32(b: &mut [u8], o: usize, v: u32) {
        b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// Un booter con la forma de `booter_load-570.144.bin`: ucode en 0x378 de
    /// 0xEC00, HsHeaderV2 en 0x18, dos firmas de 384 B, parche en 0x8A10,
    /// SEC2 ucode 3, app 0 en 0x100 de 0x8900 y datos en 0x8A00 de 0x6200.
    fn booter_de_mentira() -> Vec<u8> {
        let mut f = vec![0u8; 0x378 + 0xEC00];
        p32(&mut f, 0, MAGIA);
        p32(&mut f, 4, 1);
        let n = f.len() as u32;
        p32(&mut f, 8, n);
        p32(&mut f, 12, 0x18);
        p32(&mut f, 16, 0x378);
        p32(&mut f, 20, 0xEC00);
        for (k, v) in [0x3C, 0x300, 0x33C, 0x340, 0x344, 0xC, 0x350, 0x354, 0x24].iter().enumerate() {
            p32(&mut f, 0x18 + 4 * k, *v);
        }
        for i in 0..0x300 {
            f[0x3C + i] = if i < 384 { 0xA1 } else { 0xB2 };
        }
        p32(&mut f, 0x33C, 0x8A10);
        p32(&mut f, 0x340, 0);
        p32(&mut f, 0x344, 1);
        p32(&mut f, 0x348, 0x1);
        p32(&mut f, 0x34C, 3);
        p32(&mut f, 0x350, 2);
        for (k, v) in [0, 0x100, 0x8A00, 0x6200, 1, 0x100, 0x8900].iter().enumerate() {
            p32(&mut f, 0x354 + 4 * k, *v);
        }
        f
    }

    #[test]
    fn el_booter_se_lee_como_lo_lee_nova_core() {
        let f = booter_de_mentira();
        let b = booter(&f).unwrap();
        assert_eq!((b.firmas, b.firma_bytes, b.firmas_en), (2, 384, 0x3C));
        assert_eq!((b.parche, b.fuse_ver, b.engine_id_mask, b.ucode_id), (0x8A10, 1, 1, 3));
        assert_eq!(b.imem, Tramo { origen: 0x100, destino: 0, bytes: 0x8900 });
        assert_eq!(b.dmem, Tramo { origen: 0x8A00, destino: 0, bytes: 0x6200 });
        assert_eq!(b.pkc_data_offset(), 0x10);
        assert_eq!(b.arranque(), 0x100);
        assert_eq!(b.bin.ucode(&f).len(), 0xEC00);
    }

    #[test]
    fn la_firma_del_booter_se_elige_restando() {
        let b = booter(&booter_de_mentira()).unwrap();
        assert_eq!(b.indice_de_firma(0), Some(1), "fusible a 0: la ultima");
        assert_eq!(b.indice_de_firma(0b1), Some(0), "version 1 quemada, fuse_ver 1: la 0");
        assert_eq!(b.indice_de_firma(0b11), None, "version 2 > fuse_ver 1: ninguna");
        assert_eq!(b.indice_de_firma(0xBADF_1100), Some(1), "un error PRI cuenta como 0");
    }

    #[test]
    fn firmar_pone_los_384_bytes_en_el_parche_y_nada_mas() {
        let f = booter_de_mentira();
        let b = booter(&f).unwrap();
        let mut u = b.bin.ucode(&f).to_vec();
        firmar(&f, &b, 1, &mut u).unwrap();
        assert!(u[0x8A10..0x8A10 + 384].iter().all(|&x| x == 0xB2));
        assert_eq!((u[0x8A0F], u[0x8A10 + 384]), (0, 0));
        assert_eq!(firmar(&f, &b, 2, &mut u), Err(NoBooter::SinFirma));
    }

    #[test]
    fn lo_que_no_cuadra_no_se_acepta() {
        assert_eq!(booter(&[0u8; 8]), Err(NoBooter::Corto));
        let mut f = booter_de_mentira();
        f[0] = 0xDF;
        assert_eq!(booter(&f), Err(NoBooter::SinMagia));
        let mut f = booter_de_mentira();
        p32(&mut f, 0x33C, 0xEC00 - 10); // el parche se sale del ucode
        assert_eq!(booter(&f), Err(NoBooter::Fuera));
        let mut f = booter_de_mentira();
        p32(&mut f, 0x33C, 0x100); // el parche en la IMEM, no en la DMEM
        assert_eq!(booter(&f), Err(NoBooter::FirmaRara));
        let mut f = booter_de_mentira();
        p32(&mut f, 0x34C, 17);
        assert_eq!(booter(&f), Err(NoBooter::FirmaRara));
    }

    #[test]
    fn bytes_hostiles_nunca_revientan() {
        let base = booter_de_mentira();
        for i in (0..0x380).chain(0x8A00..0x8A20) {
            for v in [0x00, 0x7F, 0xFF] {
                let mut f = base.clone();
                f[i] = v;
                let _ = booter(&f);
                let _ = riscv(&f);
            }
        }
    }

    /// `bootloader-570.144.bin`: ucode en 0x6C de 0x6000, desc en 0x18.
    fn bootloader_de_mentira() -> Vec<u8> {
        let mut f = vec![0u8; 0x6C + 0x6000];
        p32(&mut f, 0, MAGIA);
        p32(&mut f, 12, 0x18);
        p32(&mut f, 16, 0x6C);
        p32(&mut f, 20, 0x6000);
        let d = [5, 0x5000, 0x880, 0x5880, 0x10, 0, 0, 0, 0, 0x800, 0x800, 0x1000, 0x1800, 0x2900];
        for (k, v) in d.iter().enumerate() {
            p32(&mut f, 0x18 + 4 * k, *v);
        }
        f
    }

    #[test]
    fn el_bootloader_dice_codigo_datos_y_manifiesto() {
        let r = riscv(&bootloader_de_mentira()).unwrap();
        assert_eq!((r.version, r.codigo, r.datos, r.manifiesto), (5, 0x1800, 0x800, 0));
        assert_eq!(r.bin.bytes, 0x6000);
        let mut f = bootloader_de_mentira();
        p32(&mut f, 0x18 + 4 * 13, 0x6000); // el codigo se sale
        assert_eq!(riscv(&f), Err(NoBooter::Fuera));
    }

    /// Los de VERDAD, si alguien los dejo en `BMO_FW_GSP` (la carpeta de
    /// `BMO-externo\firmware\nvidia\ga102\gsp`). Sin ella, no prueba nada.
    #[test]
    fn los_ficheros_de_linux_firmware_si_estan() {
        let Ok(dir) = std::env::var("BMO_FW_GSP") else { return };
        let leer = |n: &str| std::fs::read(std::format!("{}/{}-570.144.bin", dir, n)).unwrap();
        let f = leer("booter_load");
        let b = booter(&f).unwrap();
        assert_eq!((b.firmas, b.firma_bytes, b.ucode_id, b.pkc_data_offset()), (2, 384, 3, 0x10));
        let u = booter(&leer("booter_unload")).unwrap();
        assert_eq!((u.firmas, u.engine_id_mask), (2, 1));
        let r = riscv(&leer("bootloader")).unwrap();
        assert_eq!((r.codigo, r.datos, r.bin.bytes), (0x1800, 0x800, 0x6000));
    }
}
