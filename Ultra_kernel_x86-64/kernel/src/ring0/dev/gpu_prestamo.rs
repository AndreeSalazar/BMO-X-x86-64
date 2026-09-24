//! **LO QUE BMO-X LE PRESTA A LA 3060** -- memoria que la tarjeta podra leer
//! por DMA a traves de su dominio TRADUCIDO (M0d de `docs/plan/PLAN_LA_3060.md`).
//!
//! [carril]  ROJO      pide marcos NEUTRO y los presta a un aparato
//! [consumo] NADA      corre por orden (`gpu prestar`)
//!
//! [eje]     CORRECCION -- el camino por el que llegara el firmware del GSP
//!
//! # Por que es un fichero aparte (2026-09-24)
//!
//! Porque es la fila `GPU` del censo del NEUTRO: todo marco que un aparato
//! puede tocar por DMA se pide aqui, con su etiqueta, y el guardian del censo
//! lo cuenta. Hoy es UNA pagina de prueba; el dia de L0 seran el firmware del
//! GSP (~69 MB), su tabla radix3 y sus colas -- y vendran por aqui, no
//! repartidos por el kernel.
//!
//! # La pagina de prueba
//!
//! 4 KiB con un patron que se reconoce (`PATRON | indice` por palabra de 32
//! bits), prestada SOLO PARA LEER en [`IOVA_PRUEBA`]. La 3060 todavia no tiene
//! un motor que haga DMA; el primero que la lea sera el DMA de un falcon (M0d3,
//! la prueba de fuego), y leera ESTO. Si lee otra cosa, o si la IOMMU apunta un
//! fallo de pagina en otra direccion, la traduccion esta mal.

use core::sync::atomic::{AtomicU64, Ordering};

/// Donde la ve la 3060: 256 MiB, lejos del 0 (un puntero nulo de la tarjeta
/// no debe caer en algo prestado).
pub const IOVA_PRUEBA: u64 = 0x1000_0000;
/// La palabra `i` de la pagina vale `PATRON | i`.
pub const PATRON: u32 = 0xB0B0_0000;

pub const GPU_PRUEBA_PRESTADA: u64 = 1 << 63;

/// La fisica de la pagina de prueba | 63 prestada.
static PRUEBA: AtomicU64 = AtomicU64::new(0);

/// **`gpu prestar`**: la pagina de prueba, llena y prestada. `Ok(fisica)`.
pub fn prestar_prueba() -> Result<u64, u32> {
    use crate::ring0::mm::phys;
    let p = PRUEBA.load(Ordering::Acquire);
    if p & GPU_PRUEBA_PRESTADA != 0 {
        return Ok(p & !GPU_PRUEBA_PRESTADA);
    }
    let fisica = if p != 0 {
        p
    } else {
        // La 3060 la leera por DMA: NEUTRO.
        let Some(f) = phys::alloc_frames_contig_de(1, phys::Titular::Neutro) else {
            return Err(crate::ring0::plat::iommu::IOMMU_NO_SIN_AREA);
        };
        let v = crate::ring0::mm::phys_to_virt(f) as *mut u32;
        for i in 0..1024u32 {
            // SAFETY: la pagina recien pedida a este fichero, por el physmap.
            unsafe { v.add(i as usize).write_volatile(PATRON | i) };
        }
        PRUEBA.store(f, Ordering::Release);
        f
    };
    crate::ring0::plat::iommu::prestar_gpu(IOVA_PRUEBA, fisica, 1, false)?;
    PRUEBA.store(fisica | GPU_PRUEBA_PRESTADA, Ordering::Release);
    Ok(fisica)
}

/// `INFO_GPU_PRUEBA`: la fisica de la pagina de prueba | 63 prestada.
pub fn info_prueba() -> u64 {
    PRUEBA.load(Ordering::Acquire)
}
