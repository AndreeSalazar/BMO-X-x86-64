//! **FWSEC-FRTS, PREPARADO (L0b)** -- el ucode de la VBIOS con la orden
//! cambiada a FRTS y la firma del fusible puesta. Sobre bytes: el kernel los
//! copia de la ROM, esto los parchea, y el kernel los presta a la 3060.
//!
//! capa: puro -- cambia bytes de un bufer; no toca un registro (L8)
//!
//! [estado]  ROM WPR       FWSEC sale de la VBIOS y MONTA la WPR2 (FRTS): la region que sobrevive a un reinicio
//! [eje]     CORRECCION -- un byte mal puesto aqui es un firmware firmado que
//!           la ROM del falcon rechaza, o una WPR2 donde no se dijo
//!
//! # Lo que se cambia, y nada mas (nova-core, `firmware/fwsec.rs`, 24-09)
//!
//! ```text
//!    DMEM + interface_offset   la cabecera APPIF (v1): entradas de 8 bytes,
//!                              `id` y `dmem_base`; la 4 es DMEMMAPPER
//!    DMEM + dmem_base          la DMEMMAPPER: en +8 donde va la orden, en
//!                              +12 cuanto cabe, en +44 la orden a correr
//!    DMEM + orden_en           FrtsCmd, 44 bytes empaquetados:
//!                                ReadVbios  ver 1, hdr 24, addr 0, size 0, flags 2
//!                                FrtsRegion ver 1, hdr 20, addr y size en paginas
//!                                           de 4 KiB, tipo 2 (en la VRAM)
//!    init_cmd                  0x15 (FRTS)
//!    DMEM + pkc_data_offset    la firma RSA-3K que pide el fusible (384 bytes)
//! ```
//!
//! Donde "DMEM" es el bufer desde `imem_load_size`: la DMEM va detras de la
//! IMEM en el mismo ucode, y asi la carga el DMA.

use crate::vbios::{Descriptor, FIRMA, ORDEN_FRTS};

const APPIF_DMEMMAPPER: u32 = 0x4;
/// Lo que mide `FrtsCmd`: `ReadVbios` (24) + `FrtsRegion` (20).
pub const ORDEN_FRTS_BYTES: usize = 44;
/// Lo que mide `ReadVbios` sola (la orden SB no lleva region).
pub const LEER_VBIOS_BYTES: usize = 24;
const REGION_EN_LA_VRAM: u32 = 2;

/// Por que no.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoParche {
    /// El bufer es mas chico que IMEM + DMEM.
    Corto,
    /// La cabecera APPIF no es v1, o cae fuera.
    SinAppif,
    /// No hay DMEMMAPPER entre las apps.
    SinDmemmapper,
    /// La orden no cabe donde la DMEMMAPPER dice.
    OrdenNoCabe,
    /// La FRTS no va a pagina, o no cabe en 32 bits de paginas.
    FrtsRara,
    /// La firma se sale de la DMEM.
    FirmaFuera,
}

fn u32_en(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn poner32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// Lo que se cambio, para decirlo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Parche {
    /// Donde quedo la DMEMMAPPER y la orden, relativo a la DMEM.
    pub dmemmapper: u32,
    pub orden_en: u32,
    /// La orden que traia de fabrica.
    pub antes: u32,
}

/// **Cambiar la orden a FRTS** en `ucode` (IMEM y luego DMEM), con la region
/// `frts_desde..frts_desde + frts_bytes` de la VRAM.
pub fn parchear(ucode: &mut [u8], d: &Descriptor, frts_desde: u64, frts_bytes: u64) -> Result<Parche, NoParche> {
    if frts_desde % 4096 != 0 || frts_bytes % 4096 != 0 || (frts_desde >> 12) > u32::MAX as u64 || frts_bytes == 0 {
        return Err(NoParche::FrtsRara);
    }
    parchear_orden(ucode, d, ORDEN_FRTS, Some((frts_desde, frts_bytes)))
}

/// **Cambiar la orden a SB** (L0c5, al APAGAR el GSP): la misma
/// `ReadVbios` y la orden `0x19`, sin region -- como `nvkm_gsp_fwsec_patch`
/// de nouveau, que solo escribe la region para FRTS.
pub fn parchear_sb(ucode: &mut [u8], d: &Descriptor) -> Result<Parche, NoParche> {
    parchear_orden(ucode, d, crate::descarga::ORDEN_SB, None)
}

/// Lo comun: la DMEMMAPPER, su `ReadVbios` y su orden; la region de FRTS
/// solo si la hay.
fn parchear_orden(ucode: &mut [u8], d: &Descriptor, orden: u32, frts: Option<(u64, u64)>) -> Result<Parche, NoParche> {
    let imem = d.imem_load_size as usize;
    let fin = imem + d.dmem_load_size as usize;
    if ucode.len() < fin {
        return Err(NoParche::Corto);
    }
    let h = imem + d.interface_offset as usize;
    let cab = ucode.get(h..h + 4).filter(|_| h + 4 <= fin).ok_or(NoParche::SinAppif)?;
    if cab[0] != 1 {
        return Err(NoParche::SinAppif);
    }
    let (largo_h, largo_a, apps) = (cab[1] as usize, cab[2] as usize, cab[3] as usize);
    for i in 0..apps {
        let a = h + largo_h + i * largo_a;
        if a + 8 > fin {
            break;
        }
        if u32_en(ucode, a) != Some(APPIF_DMEMMAPPER) {
            continue;
        }
        let base = u32_en(ucode, a + 4).ok_or(NoParche::SinDmemmapper)?;
        let m = imem + base as usize;
        if m + 48 > fin {
            return Err(NoParche::SinDmemmapper);
        }
        let orden_en = u32_en(ucode, m + 8).unwrap_or(0);
        let cabe = u32_en(ucode, m + 12).unwrap_or(0) as usize;
        let o = imem + orden_en as usize;
        let mide = if frts.is_some() { ORDEN_FRTS_BYTES } else { LEER_VBIOS_BYTES };
        if cabe < mide || o + mide > fin {
            return Err(NoParche::OrdenNoCabe);
        }
        let antes = u32_en(ucode, m + 44).unwrap_or(0);
        // ReadVbios: ver, hdr, addr (u64), size, flags.
        poner32(ucode, o, 1);
        poner32(ucode, o + 4, 24);
        poner32(ucode, o + 8, 0);
        poner32(ucode, o + 12, 0);
        poner32(ucode, o + 16, 0);
        poner32(ucode, o + 20, 2);
        // FrtsRegion: ver, hdr, addr y size en paginas, tipo.
        if let Some((frts_desde, frts_bytes)) = frts {
            poner32(ucode, o + 24, 1);
            poner32(ucode, o + 28, 20);
            poner32(ucode, o + 32, (frts_desde >> 12) as u32);
            poner32(ucode, o + 36, (frts_bytes >> 12) as u32);
            poner32(ucode, o + 40, REGION_EN_LA_VRAM);
        }
        poner32(ucode, m + 44, orden);
        return Ok(Parche { dmemmapper: base, orden_en, antes });
    }
    Err(NoParche::SinDmemmapper)
}

/// **Poner la firma** en `DMEM + pkc_data_offset` (`patch_signature`).
pub fn poner_firma(ucode: &mut [u8], d: &Descriptor, firma: &[u8; FIRMA]) -> Result<(), NoParche> {
    let o = d.imem_load_size as usize + d.pkc_data_offset as usize;
    let fin = d.imem_load_size as usize + d.dmem_load_size as usize;
    if o + FIRMA > fin || o + FIRMA > ucode.len() {
        return Err(NoParche::FirmaFuera);
    }
    ucode[o..o + FIRMA].copy_from_slice(firma);
    Ok(())
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec;

    fn desc() -> Descriptor {
        Descriptor {
            hdr: 3 << 8,
            pkc_data_offset: 0x100,
            interface_offset: 0x20,
            imem_load_size: 0x200,
            dmem_load_size: 0x800,
            engine_id_mask: 0x400,
            ucode_id: 9,
            signature_count: 3,
            signature_versions: 7,
            ..Descriptor::default()
        }
    }

    /// IMEM de 0x200 y una DMEM con APPIF en 0x20, dos apps, la DMEMMAPPER en
    /// 0x560 y la orden en 0x7C0 (64 B) -- lo que dijo la VBIOS del Ryzen.
    fn ucode() -> std::vec::Vec<u8> {
        let mut u = vec![0xEEu8; 0x200 + 0x800];
        let d = 0x200;
        u[d + 0x20..d + 0x24].copy_from_slice(&[1, 4, 8, 2]);
        poner32(&mut u, d + 0x24, 0x7);
        poner32(&mut u, d + 0x2C, APPIF_DMEMMAPPER);
        poner32(&mut u, d + 0x30, 0x560);
        u[d + 0x560..d + 0x564].copy_from_slice(b"DMAP");
        poner32(&mut u, d + 0x560 + 8, 0x7C0);
        poner32(&mut u, d + 0x560 + 12, 64);
        poner32(&mut u, d + 0x560 + 44, 0);
        u
    }

    #[test]
    fn la_orden_pasa_a_ser_frts_con_la_region_en_paginas() {
        let mut u = ucode();
        let p = parchear(&mut u, &desc(), 0x2_FFE0_0000, 1 << 20).unwrap();
        assert_eq!(p, Parche { dmemmapper: 0x560, orden_en: 0x7C0, antes: 0 });
        let o = 0x200 + 0x7C0;
        let w = |k: usize| u32::from_le_bytes(u[o + k..o + k + 4].try_into().unwrap());
        assert_eq!([w(0), w(4), w(8), w(12), w(16), w(20)], [1, 24, 0, 0, 0, 2], "ReadVbios");
        assert_eq!([w(24), w(28), w(32), w(36), w(40)], [1, 20, 0x2FFE00, 0x100, 2], "FrtsRegion");
        assert_eq!(u32_en(&u, 0x200 + 0x560 + 44), Some(ORDEN_FRTS));
        assert_eq!(u[o + 44], 0xEE, "ni un byte mas alla de los 44");
        assert_eq!(&u[0x200 + 0x560..0x200 + 0x564], b"DMAP", "la firma de la DMEMMAPPER no se toca");
    }

    #[test]
    fn la_orden_sb_lleva_solo_leer_vbios() {
        // Al APAGAR (L0c5): la orden 0x19 y la ReadVbios, y NI UN byte de
        // region -- lo que hubiera ahi se queda.
        let mut u = ucode();
        let p = parchear_sb(&mut u, &desc()).unwrap();
        assert_eq!(p.orden_en, 0x7C0);
        let o = 0x200 + 0x7C0;
        let w = |k: usize| u32::from_le_bytes(u[o + k..o + k + 4].try_into().unwrap());
        assert_eq!([w(0), w(4), w(8), w(12), w(16), w(20)], [1, 24, 0, 0, 0, 2], "ReadVbios");
        assert_eq!(u[o + 24], 0xEE, "sin region");
        assert_eq!(u32_en(&u, 0x200 + 0x560 + 44), Some(0x19));
        // Y sobre un ucode ya parcheado para FRTS, cambia la orden y deja la
        // region como estaba (FWSEC-SB no la lee).
        let mut u = ucode();
        parchear(&mut u, &desc(), 0x2_FFE0_0000, 1 << 20).unwrap();
        let p = parchear_sb(&mut u, &desc()).unwrap();
        assert_eq!((p.antes, u32_en(&u, 0x200 + 0x560 + 44)), (ORDEN_FRTS, Some(0x19)));
    }

    #[test]
    fn la_firma_va_en_la_dmem_donde_dice_el_descriptor() {
        let mut u = ucode();
        poner_firma(&mut u, &desc(), &[0x5A; FIRMA]).unwrap();
        assert!(u[0x300..0x300 + FIRMA].iter().all(|&b| b == 0x5A));
        assert_eq!(u[0x2FF], 0xEE);
        assert_eq!(u[0x300 + FIRMA], 0xEE);
        let mut d = desc();
        d.pkc_data_offset = 0x800 - 10;
        assert_eq!(poner_firma(&mut u, &d, &[0; FIRMA]), Err(NoParche::FirmaFuera));
    }

    #[test]
    fn lo_que_no_cuadra_no_se_parchea() {
        let mut u = ucode();
        assert_eq!(parchear(&mut u[..0x400], &desc(), 0, 4096), Err(NoParche::Corto));
        assert_eq!(parchear(&mut u, &desc(), 0x1234, 4096), Err(NoParche::FrtsRara));
        let mut sin = ucode();
        poner32(&mut sin, 0x200 + 0x2C, 0x9); // la DMEMMAPPER pasa a ser otra app
        assert_eq!(parchear(&mut sin, &desc(), 0, 4096), Err(NoParche::SinDmemmapper));
        let mut chica = ucode();
        poner32(&mut chica, 0x200 + 0x560 + 12, 40); // no caben 44
        assert_eq!(parchear(&mut chica, &desc(), 0, 4096), Err(NoParche::OrdenNoCabe));
        let mut v2 = ucode();
        v2[0x200 + 0x20] = 2;
        assert_eq!(parchear(&mut v2, &desc(), 0, 4096), Err(NoParche::SinAppif));
    }

    #[test]
    fn bytes_hostiles_nunca_revientan() {
        let base = ucode();
        for i in (0x200..base.len()).step_by(3) {
            for v in [0x00, 0x7F, 0xFF] {
                let mut u = base.clone();
                u[i] = v;
                let _ = parchear(&mut u, &desc(), 0x1000, 0x1000);
            }
        }
    }
}
