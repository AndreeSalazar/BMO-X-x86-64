//! **LOS FICHEROS DEL GSP, ABIERTOS PARA `dev/gpu_gsp.rs`** (L0c2) -- el
//! puente entre quien sabe de FAT32 y quien presta memoria a la 3060.
//!
//! [carril]  AMARILLO  abre y lee fw/gsp/ para quien los presta
//! [consumo] NADA      corre solo cuando el escritorio pide `gpu radix`
//!
//! # Por que esto vive en `syscall/` y no en `dev/` (2026-09-24)
//!
//! Porque `dev` esta DEBAJO de `fsys`: el disco es un aparato, y FAT32 lo usa.
//! La primera version de L0c2 abria `fw/gsp/gsp.bin` desde `dev/gpu_gsp.rs`
//! y el guardian de capas (L8b) lo paro en el build: `dev <-> fsys` en los dos
//! sentidos. El arreglo es el de la casa -- el dato SUBE como parametro --:
//! `dev/gpu_gsp.rs` pide un `Fichero` y `syscall`, que conecta a los dos, se
//! lo da abierto.
//!
//! # El cursor se queda aqui
//!
//! El de FAT32 solo avanza: llegar al byte N es seguir la cadena. Los 122
//! trozos del `.fwimage` van en orden, asi que el mismo cursor sirve de uno al
//! siguiente sin volver a recorrer 60 MB de cadena en cada syscall. Solo se
//! vuelve al principio si alguien pide hacia atras (el ELF, al prepararlo).

use crate::ring0::dev::gpu_gsp::{self, Fichero};
use crate::ring0::fsys::fs;

/// Un fichero de FAT32 abierto por rangos, con su cursor.
#[derive(Clone, Copy)]
struct Fat {
    inicio: bmo_fat32::Cursor,
    cur: bmo_fat32::Cursor,
    medida: u32,
}

impl Fat {
    fn abrir(ruta: &str) -> Option<Self> {
        let (c, m) = fs::abrir_rangos(ruta).ok()?;
        Some(Fat { inicio: c, cur: c, medida: m })
    }
}

impl Fichero for Fat {
    fn leer(&mut self, desde: u64, dst: &mut [u8]) -> usize {
        if (desde as usize) < self.cur.base() {
            self.cur = self.inicio;
        }
        fs::leer_rango(&mut self.cur, desde as usize, self.medida, dst)
    }
    fn medida(&self) -> u64 {
        self.medida as u64
    }
}

/// `gsp.bin`, abierto en PREPARAR y seguido por los TROZOS. Solo lo toca el
/// escritorio (el syscall de la IOMMU es suyo), de una orden en una.
static mut GSP: Option<Fat> = None;

/// **PREPARAR**: abre los dos y se los da.
pub(super) fn preparar() -> Result<u64, u32> {
    let mut bl = Fat::abrir(gpu_gsp::RUTA_BOOTLOADER);
    let mut gsp = Fat::abrir(gpu_gsp::RUTA_GSP);
    let r = gpu_gsp::preparar(
        bl.as_mut().map(|f| f as &mut dyn Fichero),
        gsp.as_mut().map(|f| f as &mut dyn Fichero),
    );
    // Los trozos empiezan por el principio: el ELF dejo el cursor al final.
    // SAFETY: ver `GSP`.
    unsafe { *core::ptr::addr_of_mut!(GSP) = gsp.map(|f| Fat { cur: f.inicio, ..f }) };
    r
}

/// **BOOTER** (L0c3b): abre `boot_ld.bin` y se lo da.
pub(super) fn booter() -> Result<u64, u32> {
    let mut b = Fat::abrir(crate::ring0::dev::gpu_despertar::RUTA_BOOTER);
    crate::ring0::dev::gpu_despertar::booter(b.as_mut().map(|f| f as &mut dyn Fichero))
}

/// **TROZO k**, con el mismo `gsp.bin` y su cursor.
pub(super) fn trozo(k: u64) -> Result<u64, u32> {
    // SAFETY: ver `GSP`.
    let gsp = unsafe { (*core::ptr::addr_of_mut!(GSP)).as_mut() };
    gpu_gsp::trozo(k, gsp.map(|f| f as &mut dyn Fichero))
}
