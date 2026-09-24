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

// == M0d3: LA PRUEBA DE FUEGO, y LA FRONTERA (2026-09-24) =====================
//
// `gpu fuego`: el DMA del falcon del GSP trae la pagina de prueba desde
// `IOVA_PRUEBA` a su DMEM, y se compara palabra a palabra. Si las 1024
// cuadran, la 3060 LEYO la RAM del PC a traves de la IOMMU -- solo lo
// prestado.
//
// `gpu frontera`: lo mismo desde `IOVA_NO_PRESTADA`. Lo correcto es que NO
// llegue nada y que la IOMMU apunte un FALLO DE PAGINA con el BDF de la 3060 y
// esa direccion. Es la mitad que dice que la venda existe.
//
// ** EL CANDADO: la 3060 TRADUCIDA y releida, la pagina prestada, y el Bus
// Master YA encendido por E2 (`gpu vblank`). Esto no lo enciende: el Bus
// Master tiene un solo propietario, y es quien lo apaga cuando la IOMMU se apaga o
// la 3060 vuelve a ver (`syscall/op_maquina.rs`).

use bmo_gpu_ga10x::falcon::{self as fa, NoFuego};

/// Una direccion del aparato que NO se presta nunca: la de la frontera.
pub const IOVA_NO_PRESTADA: u64 = 0x2000_0000;

pub const IOMMU_NO_SIN_BUS_MASTER: u32 = 16;
pub const IOMMU_NO_FUEGO: u32 = 17;
pub const IOMMU_NO_SIN_PRUEBA: u32 = 18;

pub const FUEGO_PRIMERA_SHIFT: u64 = 11;
pub const FUEGO_MOTIVO_SHIFT: u64 = 22;
pub const FUEGO_SEGURIDAD_SHIFT: u64 = 26;
pub const FUEGO_DMEM_SHIFT: u64 = 28;
pub const FUEGO_EVENTOS_SHIFT: u64 = 36;
pub const FUEGO_HECHO: u64 = 1 << 62;
pub const FUEGO_INTENTADO: u64 = 1 << 63;

pub const FRONTERA_EVENTO: u64 = 1 << 4;
pub const FRONTERA_BDF: u64 = 1 << 5;
pub const FRONTERA_DIR: u64 = 1 << 6;
pub const FRONTERA_DMA_ACABO: u64 = 1 << 7;
pub const FRONTERA_MOTIVO_SHIFT: u64 = 8;

/// `INFO_GPU_FUEGO`: `0..10` palabras que cuadran | `11..21` la primera que no
/// | `22..25` el motivo (`NoFuego` + 1, 6 = sin empezar) | `26..27` seguridad
/// del falcon | `28..35` su DMEM en KiB | `36..43` eventos nuevos de la IOMMU |
/// 62 HECHO (1024 de 1024 y ningun evento) | 63 intentado.
static FUEGO: AtomicU64 = AtomicU64::new(0);
/// `INFO_GPU_FUEGO_LEIDO`: la primera palabra que no cuadro | us del DMA << 32.
static FUEGO_LEIDO: AtomicU64 = AtomicU64::new(0);
/// `INFO_GPU_FRONTERA`: `0..3` tipo del evento | 4 hubo evento nuevo | 5 con el
/// BDF de la 3060 | 6 con la direccion no prestada | 7 el DMA acabo | `8..11`
/// motivo | 62 HECHO (evento, BDF y direccion) | 63 intentada.
static FRONTERA: AtomicU64 = AtomicU64::new(0);

/// Los registros de la 3060 por BAR0 en el physmap (como `dev/vblank.rs`).
struct Bar0(u64);

impl bmo_gpu_ga10x::Registros for Bar0 {
    fn leer(&mut self, reg: u32) -> u32 {
        // SAFETY: BAR0 de la grafica por el physmap, no cacheable (MTRR);
        // registros de 32 bits alineados de sus 16 MiB, los de nova-core.
        unsafe { ((self.0 + reg as u64) as *const u32).read_volatile() }
    }
    fn escribir(&mut self, reg: u32, x: u32) {
        // SAFETY: como `leer`. Solo los del falcon de `bmo_gpu_ga10x::falcon`.
        unsafe { ((self.0 + reg as u64) as *mut u32).write_volatile(x) }
    }
}

/// El reloj del falcon: el TSC en microsegundos.
struct Tsc(u64);

impl fa::Reloj for Tsc {
    fn us(&mut self) -> u64 {
        crate::ring0::task::scheduler::rdtsc() / self.0
    }
}

fn motivo(e: NoFuego) -> u64 {
    match e {
        NoFuego::NoContesta(_) => 1,
        NoFuego::NoLimpia => 2,
        NoFuego::NoNucleo => 3,
        NoFuego::DmemChica(_) => 4,
        NoFuego::DmaNoAcaba(_) => 5,
    }
}

/// **El candado del fuego.** `Ok(bar0)` o el motivo del NO.
fn candado() -> Result<u64, u32> {
    use crate::ring0::plat::iommu as io;
    let g = io::info_gpu();
    if g & io::IOMMU_GPU_TRADUCIDA == 0 || g & io::IOMMU_GPU_RELEIDA == 0 {
        return Err(io::IOMMU_NO_NO_TRADUCIDA);
    }
    if PRUEBA.load(Ordering::Acquire) & GPU_PRUEBA_PRESTADA == 0 {
        return Err(IOMMU_NO_SIN_PRUEBA);
    }
    let Some((b, d, f)) = crate::ring0::dev::gpu::bdf() else { return Err(io::IOMMU_NO_SIN_GPU) };
    if crate::ring0::dev::pci::cfg_read32(b, d, f, 0) & 0xFFFF != 0x10DE || (g & 0xFFFF) as u16 != (b as u16) << 8 | (d as u16) << 3 | f as u16 {
        return Err(io::IOMMU_NO_SIN_GPU);
    }
    if !crate::ring0::dev::vblank::activo() || crate::ring0::dev::pci::comando(b, d, f) & 0b100 == 0 {
        return Err(IOMMU_NO_SIN_BUS_MASTER);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return Err(io::IOMMU_NO_SIN_GPU);
    }
    Ok(bar0)
}

fn eventos() -> u64 {
    let e = crate::ring0::plat::iommu::info_evento();
    if e & crate::ring0::plat::iommu::IOMMU_EVENTO_HAY != 0 { e & 0xFFFF } else { 0 }
}

fn reloj() -> Tsc {
    Tsc((crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1))
}

/// **`gpu fuego`**. `Ok(palabras que cuadran)` -- 1024 es el exito.
pub fn fuego() -> Result<u64, u32> {
    let bar0 = candado()?;
    let (mut r, mut t) = (Bar0(bar0), reloj());
    let boot0 = bmo_gpu_ga10x::Registros::leer(&mut r, bmo_gpu_ga10x::BOOT_0);
    let antes = eventos();
    FUEGO.store(FUEGO_INTENTADO | 6 << FUEGO_MOTIVO_SHIFT, Ordering::Release);
    crate::ring0::cabina::info("gpu", "M0d3: la PRUEBA DE FUEGO -- el falcon del GSP trae la pagina prestada por DMA", IOVA_PRUEBA);
    let ficha = match fa::ficha(&mut r, fa::GSP) {
        Ok(f) => f,
        Err(e) => return fallo_fuego(e, 0),
    };
    let fija = (ficha.seguridad() as u64 & 3) << FUEGO_SEGURIDAD_SHIFT | ((ficha.dmem / 1024).min(255) as u64) << FUEGO_DMEM_SHIFT;
    if ficha.dmem < 4096 {
        return fallo_fuego(NoFuego::DmemChica(ficha.dmem), fija);
    }
    if let Err(e) = fa::resetear(&mut r, &mut t, fa::GSP, boot0) {
        return fallo_fuego(e, fija);
    }
    let t0 = fa::Reloj::us(&mut t);
    let dma = fa::traer(&mut r, &mut t, fa::GSP, IOVA_PRUEBA, 0, 4096, 50_000);
    let us = fa::Reloj::us(&mut t) - t0;
    let mut leido = [0u32; 1024];
    if dma.is_ok() {
        fa::leer_dmem(&mut r, fa::GSP, 0, &mut leido);
    }
    // Y se deja como estaba: el reset borra su DMEM.
    let _ = fa::resetear(&mut r, &mut t, fa::GSP, boot0);
    if let Err(e) = dma {
        return fallo_fuego(e, fija);
    }
    let c = fa::contar(&leido, PATRON);
    let nuevos = eventos().saturating_sub(antes);
    let (idx, mal) = c.primera_mal.unwrap_or((0, 0));
    let hecho = c.bien == 1024 && nuevos == 0;
    FUEGO.store(
        FUEGO_INTENTADO
            | if hecho { FUEGO_HECHO } else { 0 }
            | c.bien as u64
            | (idx as u64 & 0x7FF) << FUEGO_PRIMERA_SHIFT
            | fija
            | nuevos.min(0xFF) << FUEGO_EVENTOS_SHIFT,
        Ordering::Release,
    );
    FUEGO_LEIDO.store(mal as u64 | us.min(0xFFFF_FFFF) << 32, Ordering::Release);
    if hecho {
        crate::ring0::cabina::count("gpu", "M0d3: la 3060 LEYO la RAM del PC por la IOMMU: palabras que cuadran", 1024);
    } else {
        crate::ring0::cabina::warn("gpu", "M0d3: el DMA acabo pero la DMEM NO es la pagina prestada; cuadran", c.bien as u64);
    }
    Ok(c.bien as u64)
}

fn fallo_fuego(e: NoFuego, fija: u64) -> Result<u64, u32> {
    FUEGO.store(FUEGO_INTENTADO | fija | motivo(e) << FUEGO_MOTIVO_SHIFT, Ordering::Release);
    crate::ring0::cabina::warn("gpu", "M0d3: el falcon del GSP no dejo hacer el DMA; motivo", motivo(e));
    Err(IOMMU_NO_FUEGO)
}

/// **`gpu frontera`**. `Ok(1)` si la IOMMU paro el DMA con su evento.
pub fn frontera() -> Result<u64, u32> {
    use crate::ring0::plat::iommu as io;
    let bar0 = candado()?;
    let (mut r, mut t) = (Bar0(bar0), reloj());
    let boot0 = bmo_gpu_ga10x::Registros::leer(&mut r, bmo_gpu_ga10x::BOOT_0);
    let antes = eventos();
    FRONTERA.store(FUEGO_INTENTADO, Ordering::Release);
    crate::ring0::cabina::info("gpu", "M0d3: la FRONTERA -- el falcon pide una direccion NO prestada", IOVA_NO_PRESTADA);
    if let Err(e) = fa::resetear(&mut r, &mut t, fa::GSP, boot0) {
        FRONTERA.store(FUEGO_INTENTADO | motivo(e) << FRONTERA_MOTIVO_SHIFT, Ordering::Release);
        return Err(IOMMU_NO_FUEGO);
    }
    let dma = fa::traer(&mut r, &mut t, fa::GSP, IOVA_NO_PRESTADA, 0, fa::TROZO, 50_000);
    let _ = fa::resetear(&mut r, &mut t, fa::GSP, boot0);
    // La IOMMU escribe el evento por su cuenta: se le dan unos ms.
    let fin = fa::Reloj::us(&mut t) + 5_000;
    while eventos() <= antes && fa::Reloj::us(&mut t) < fin {}
    let e = io::info_evento();
    let nuevo = eventos() > antes;
    let bdf_gpu = io::info_gpu() & 0xFFFF;
    let bdf_ok = nuevo && (e >> io::IOMMU_EVENTO_BDF_SHIFT) & 0xFFFF == bdf_gpu;
    let dir_ok = nuevo && io::info_evento_dir() & !0xFFF == IOVA_NO_PRESTADA;
    let hecho = nuevo && bdf_ok && dir_ok;
    FRONTERA.store(
        FUEGO_INTENTADO
            | if hecho { FUEGO_HECHO } else { 0 }
            | (e >> io::IOMMU_EVENTO_TIPO_SHIFT) & 0xF
            | if nuevo { FRONTERA_EVENTO } else { 0 }
            | if bdf_ok { FRONTERA_BDF } else { 0 }
            | if dir_ok { FRONTERA_DIR } else { 0 }
            | if dma.is_ok() { FRONTERA_DMA_ACABO } else { 0 }
            | dma.err().map_or(0, motivo) << FRONTERA_MOTIVO_SHIFT,
        Ordering::Release,
    );
    if hecho {
        crate::ring0::cabina::count("gpu", "M0d3: la FRONTERA aguanta -- la IOMMU paro el DMA de la 3060 en", IOVA_NO_PRESTADA);
    } else {
        crate::ring0::cabina::warn("gpu", "M0d3: la FRONTERA no se vio: sin evento con el BDF y la direccion de la 3060", e);
    }
    Ok(hecho as u64)
}

pub fn info_fuego() -> u64 {
    FUEGO.load(Ordering::Acquire)
}
pub fn info_fuego_leido() -> u64 {
    FUEGO_LEIDO.load(Ordering::Acquire)
}
pub fn info_frontera() -> u64 {
    FRONTERA.load(Ordering::Acquire)
}
