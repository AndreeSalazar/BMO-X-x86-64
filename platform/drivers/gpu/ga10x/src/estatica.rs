//! **LO QUE EL GSP-RM DICE DE TU 3060 (L1a)** -- `GET_GSP_STATIC_INFO`: la
//! primera RPC de verdad, pregunta y respuesta. La pregunta son los 1656 B de
//! `GspStaticConfigInfo_t` a cero; la respuesta, los mismos llenos.
//!
//! capa: puro -- arma la pregunta y lee la respuesta; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- un offset corrido es un nombre de GPU con basura, o
//!           unas asas del RM que no abren nada en L1b
//!
//! # Lo que se lee (r570.144, offsets sacados de las bindings de nova-core)
//!
//! ```text
//!    +0x158  fbRegionInfoParams   numFBRegions y 16 regiones de 48 B:
//!                                 base, limit, reserved, performance,
//!                                 supportCompressed, supportISO, bProtected
//!    +0x4C8  fb_length            la VRAM, en bytes
//!    +0x4D8  fb_bus_width         el bus, en bits
//!    +0x4DC  fb_ram_type
//!    +0x4E8  l2_cache_size
//!    +0x4EC  gpuNameString[64]    "NVIDIA GeForce RTX 3060"
//!    +0x52C  gpuShortNameString[64]
//!    +0x600  bar1PdeBase, bar2PdeBase
//!    +0x640  hInternalClient, hInternalDevice, hInternalSubdevice
//!    +0x64E  bIsGpuUefi, bIsEfiInit
//! ```
//!
//! "Usable" es lo que nova-core deja usar (`usable_fb_regions`): ni reservada
//! ni protegida, y con compresion e ISO.

use crate::orden;

/// `NV_VGPU_MSG_FUNCTION_GET_GSP_STATIC_INFO`.
pub const GET_GSP_STATIC_INFO: u32 = 65;
/// Lo que mide `GspStaticConfigInfo_t`.
pub const BYTES: usize = 1656;
pub const MAX_REGIONES: usize = 16;

/// Una region de la VRAM, como la dice el GSP-RM.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Region {
    pub base: u64,
    pub limit: u64,
    pub usable: bool,
}

/// Lo que se lee de la respuesta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Estatica {
    pub nombre: [u8; 64],
    pub corto: [u8; 64],
    pub vram: u64,
    pub bus_bits: u32,
    pub ram_tipo: u32,
    pub l2: u32,
    pub bar1_pde: u64,
    pub bar2_pde: u64,
    /// Las asas internas del RM: con ellas se piden objetos en L1b.
    pub cliente: u32,
    pub dispositivo: u32,
    pub subdispositivo: u32,
    pub uefi: bool,
    pub efi_init: bool,
    pub regiones: [Region; MAX_REGIONES],
    pub n_regiones: usize,
}

fn u32_de(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn u64_de(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

/// **La pregunta** en `hueco`: la RPC con sus 1656 B a cero.
pub fn pregunta(hueco: &mut [u8], numero: u32) -> Option<usize> {
    orden::componer(hueco, numero, GET_GSP_STATIC_INFO, BYTES, |_| {})
}

/// **Leer la respuesta**: los datos del mensaje (tras sus 80 B de cabecera).
pub fn leer(d: &[u8]) -> Option<Estatica> {
    if d.len() < BYTES {
        return None;
    }
    let mut nombre = [0u8; 64];
    nombre.copy_from_slice(&d[0x4EC..0x4EC + 64]);
    let mut corto = [0u8; 64];
    corto.copy_from_slice(&d[0x52C..0x52C + 64]);
    let n = (u32_de(d, 0x158) as usize).min(MAX_REGIONES);
    let mut regiones = [Region::default(); MAX_REGIONES];
    for (k, r) in regiones.iter_mut().enumerate().take(n) {
        let o = 0x158 + 8 + 48 * k;
        let (reservada, comprimible, iso, protegida) = (u64_de(d, o + 16), d[o + 28], d[o + 29], d[o + 30]);
        *r = Region { base: u64_de(d, o), limit: u64_de(d, o + 8), usable: reservada == 0 && protegida == 0 && comprimible != 0 && iso != 0 };
    }
    Some(Estatica {
        nombre,
        corto,
        vram: u64_de(d, 0x4C8),
        bus_bits: u32_de(d, 0x4D8),
        ram_tipo: u32_de(d, 0x4DC),
        l2: u32_de(d, 0x4E8),
        bar1_pde: u64_de(d, 0x600),
        bar2_pde: u64_de(d, 0x608),
        cliente: u32_de(d, 0x640),
        dispositivo: u32_de(d, 0x644),
        subdispositivo: u32_de(d, 0x648),
        uefi: d[0x64E] != 0,
        efi_init: d[0x64F] != 0,
        regiones,
        n_regiones: n,
    })
}

/// El texto de un nombre: hasta su primer 0.
pub fn texto(b: &[u8]) -> &[u8] {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    &b[..n]
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::{Mensaje, Suma, CABECERA};

    #[test]
    fn la_pregunta_cabe_en_una_pagina_y_suma_cero() {
        let mut h = [0xAAu8; 4096];
        let n = pregunta(&mut h, 2).unwrap();
        assert_eq!(n, 80 + 1656);
        let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado());
        assert_eq!((m.funcion, m.numero, m.datos()), (65, 2, 1656));
        let mut s = Suma::default();
        s.mas(&h[..m.bytes_sumados()]);
        assert_eq!(s.valor(), 0);
        assert!(h[CABECERA..n].iter().all(|&b| b == 0), "la pregunta va a cero");
    }

    #[test]
    fn la_respuesta_se_lee_donde_dicen_las_bindings() {
        let mut d = [0u8; BYTES];
        d[0x4EC..0x4EC + 23].copy_from_slice(b"NVIDIA GeForce RTX 3060");
        d[0x52C..0x52C + 5].copy_from_slice(b"GA106");
        d[0x4C8..0x4D0].copy_from_slice(&(12u64 << 30).to_le_bytes());
        d[0x4D8..0x4DC].copy_from_slice(&192u32.to_le_bytes());
        d[0x640..0x644].copy_from_slice(&0xC1D0_0001u32.to_le_bytes());
        d[0x644..0x648].copy_from_slice(&0x5C00_0002u32.to_le_bytes());
        d[0x648..0x64C].copy_from_slice(&0x5C00_0003u32.to_le_bytes());
        d[0x64E] = 1;
        // Dos regiones: la primera usable, la segunda protegida.
        d[0x158..0x15C].copy_from_slice(&2u32.to_le_bytes());
        let r0 = 0x158 + 8;
        d[r0 + 8..r0 + 16].copy_from_slice(&0x2F3F_FFFFFu64.to_le_bytes());
        d[r0 + 28] = 1;
        d[r0 + 29] = 1;
        let r1 = r0 + 48;
        d[r1..r1 + 8].copy_from_slice(&0x2F40_00000u64.to_le_bytes());
        d[r1 + 28] = 1;
        d[r1 + 29] = 1;
        d[r1 + 30] = 1;
        let e = leer(&d).unwrap();
        assert_eq!(texto(&e.nombre), b"NVIDIA GeForce RTX 3060");
        assert_eq!(texto(&e.corto), b"GA106");
        assert_eq!((e.vram, e.bus_bits), (12 << 30, 192));
        assert_eq!((e.cliente, e.dispositivo, e.subdispositivo), (0xC1D0_0001, 0x5C00_0002, 0x5C00_0003));
        assert!(e.uefi && !e.efi_init);
        assert_eq!(e.n_regiones, 2);
        assert_eq!(e.regiones[0], Region { base: 0, limit: 0x2F3F_FFFFF, usable: true });
        assert!(!e.regiones[1].usable, "protegida");
        assert_eq!(leer(&d[..100]), None);
    }
}
