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
pub(crate) struct Bar0(pub(crate) u64);

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
pub(crate) struct Tsc(u64);

impl fa::Reloj for Tsc {
    fn us(&mut self) -> u64 {
        crate::ring0::task::scheduler::rdtsc() / self.0
    }
}

pub(crate) fn motivo(e: NoFuego) -> u64 {
    match e {
        NoFuego::NoContesta(_) => 1,
        NoFuego::NoLimpia => 2,
        NoFuego::NoNucleo => 3,
        NoFuego::DmemChica(_) => 4,
        NoFuego::DmaNoAcaba(_) => 5,
    }
}

/// **El candado del fuego.** `Ok(bar0)` o el motivo del NO.
///
/// ** Y NUNCA con el GSP en marcha (26-09, EL_0x15.md): `fuego` y `frontera`
/// RESETEAN el falcon del GSP; con el booter ya dado, lo matarian. Y el falcon
/// que usan queda apuntado: el caso del 0x15 mostro que tocarlo antes del
/// booter es lo que lo tumbaba (3 de 3 arranques en frio buenos sin ellas).
fn candado() -> Result<u64, u32> {
    if PRUEBA.load(Ordering::Acquire) & GPU_PRUEBA_PRESTADA == 0 {
        return Err(IOMMU_NO_SIN_PRUEBA);
    }
    if crate::ring0::dev::gpu_despertar::gsp_tocado() {
        crate::ring0::cabina::warn("gpu", "M0d3 NEGADA: el booter ya corrio; resetear el falcon del GSP lo mataria", 0);
        return Err(crate::ring0::dev::gpu_despertar::IOMMU_NO_YA_DESPIERTO);
    }
    let bar0 = candado_dma()?;
    FALCON_GSP_USADO.store(true, Ordering::Release);
    Ok(bar0)
}

/// `fuego` o `frontera` usaron el DMA del falcon del GSP en este arranque.
static FALCON_GSP_USADO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Si el falcon del GSP ya hizo DMA para una prueba en este arranque: el
/// booter que venga detras puede dar 0x15 (EL_0x15.md).
pub fn falcon_gsp_usado() -> bool {
    FALCON_GSP_USADO.load(Ordering::Acquire)
}

/// **El candado de todo DMA de la 3060**: TRADUCIDA y releida, una NVIDIA en
/// su BDF, y el Bus Master de E2 encendido (que no se enciende aqui).
pub(crate) fn candado_dma() -> Result<u64, u32> {
    use crate::ring0::plat::iommu as io;
    let g = io::info_gpu();
    if g & io::IOMMU_GPU_TRADUCIDA == 0 || g & io::IOMMU_GPU_RELEIDA == 0 {
        return Err(io::IOMMU_NO_NO_TRADUCIDA);
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

/// Cuantos eventos lleva apuntados la IOMMU (0 si ninguno).
pub(crate) fn eventos() -> u64 {
    let e = crate::ring0::plat::iommu::info_evento();
    if e & crate::ring0::plat::iommu::IOMMU_EVENTO_HAY != 0 { e & 0xFFFF } else { 0 }
}

pub(crate) fn reloj() -> Tsc {
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

// == L0b: FWSEC-FRTS (2026-09-24) ==============================================
//
// El firmware FIRMADO de la VBIOS corre en el falcon del GSP y aparta la WPR2
// en la VRAM. Es la puerta del booter y del GSP-RM. Por el mismo camino que la
// prueba de fuego: el ucode se copia de la ROM a paginas NEUTRO, se PRESTA a la
// 3060 solo para leer en `IOVA_FWSEC`, y el DMA del falcon lo trae -- ahora a la
// IMEM y a la DMEM, en modo seguro, con la firma que pide el fusible puesta.
//
// Tres ordenes y no una, por la maquina: la ROM se lee de 4 KiB en 4 KiB (un
// syscall cada trozo), y el arranque vuelve en cuanto el falcon ARRANCA -- quien
// espera a que se pare es el escritorio, cediendo el turno, y no el kernel con
// las interrupciones cerradas. Hasta 2 s, dice nova-core.
//
//    PREPARAR(en_rom)  el kernel relee el descriptor de la ROM por su cuenta
//                      y lo juzga: v3, del GSP, medidas que caben
//    TROZO(k)          4 KiB de ucode, de la ROM al bufer
//    CORRER            la orden FRTS y la firma (`bmo_gpu_ga10x::fwsec`), el
//                      prestamo, reset, IMEM y DMEM por DMA seguro, BROM y
//                      STARTCPU. `INFO_GPU_FWSEC` dice despues si se paro, su
//                      MAILBOX0, el codigo de FRTS y si hay WPR2

use bmo_gpu_ga10x::vbios as vb;

/// Donde ve la 3060 el ucode de FWSEC: lejos de la pagina de prueba.
pub const IOVA_FWSEC: u64 = 0x1100_0000;
/// 32 paginas = 128 KiB: el ucode del Ryzen mide 58 KiB.
const FWSEC_PAGINAS: u64 = 32;

pub const IOMMU_NO_FWSEC_DESC: u32 = 19;
pub const IOMMU_NO_FWSEC_SIN_PREPARAR: u32 = 20;
pub const IOMMU_NO_FWSEC_FIRMA: u32 = 21;
pub const IOMMU_NO_FWSEC_PARCHE: u32 = 22;
pub const IOMMU_NO_WPR2_YA: u32 = 23;
pub const IOMMU_NO_GFW: u32 = 24;
/// La tarjeta no es la 3060 12G: su VRAM no son 12288 MiB
/// (`bmo_gpu_ga10x::identidad`). Nada de lo que sigue se escribio para otra.
pub const IOMMU_NO_OTRA_TARJETA: u32 = 84;
pub const IOMMU_NO_FWSEC_FALCON: u32 = 25;

pub const FWSEC_TOTALES_SHIFT: u64 = 8;
pub const FWSEC_FIRMA_SHIFT: u64 = 16;
pub const FWSEC_PARCHEADO: u64 = 1 << 18;
pub const FWSEC_PRESTADO: u64 = 1 << 19;
pub const FWSEC_ARRANCADO: u64 = 1 << 20;
pub const FWSEC_PARADO: u64 = 1 << 21;
pub const FWSEC_WPR2: u64 = 1 << 22;
pub const FWSEC_ERROR_SHIFT: u64 = 32;
pub const FWSEC_MOTIVO_SHIFT: u64 = 48;
pub const FWSEC_PREPARADO: u64 = 1 << 62;
pub const FWSEC_VALIDO: u64 = 1 << 63;

/// La fisica del bufer (0 = aun no).
static FWSEC_BUF: AtomicU64 = AtomicU64::new(0);
/// Donde empieza el descriptor en la ROM.
static FWSEC_EN_ROM: AtomicU64 = AtomicU64::new(0);
/// Los trozos de 4 KiB ya copiados (un bit cada uno).
static FWSEC_TROZOS: AtomicU64 = AtomicU64::new(0);
/// El estado sin lo que se lee en vivo (ver `info_fwsec`).
static FWSEC_ESTADO: AtomicU64 = AtomicU64::new(0);
/// El descriptor, juzgado en PREPARAR. El escritorio es el unico que llega
/// aqui (solo quien tiene la pantalla), y de uno en uno.
static mut FWSEC_DESC: Option<vb::Descriptor> = None;

fn desc() -> Option<vb::Descriptor> {
    // SAFETY: ver `FWSEC_DESC`.
    unsafe { *core::ptr::addr_of!(FWSEC_DESC) }
}

fn apuntar(f: impl FnOnce(u64) -> u64) {
    let v = FWSEC_ESTADO.load(Ordering::Acquire);
    FWSEC_ESTADO.store(f(v), Ordering::Release);
}

fn no(motivo: u32) -> Result<u64, u32> {
    apuntar(|v| (v & !(0xFF << FWSEC_MOTIVO_SHIFT)) | (motivo as u64 & 0xFF) << FWSEC_MOTIVO_SHIFT);
    Err(motivo)
}

/// Un byte de la ROM, por su palabra.
fn rom_byte(r: &mut Bar0, p: usize) -> u8 {
    let w = bmo_gpu_ga10x::Registros::leer(r, vb::ROM + (p as u32 & !3));
    (w >> ((p & 3) * 8)) as u8
}

/// `n` bytes de la ROM desde `p` a `dst`: por palabras si `p` va a 4.
fn rom_a(r: &mut Bar0, p: usize, dst: &mut [u8]) {
    if p % 4 == 0 {
        for (i, c) in dst.chunks_mut(4).enumerate() {
            let w = bmo_gpu_ga10x::Registros::leer(r, vb::ROM + (p + i * 4) as u32).to_le_bytes();
            c.copy_from_slice(&w[..c.len()]);
        }
    } else {
        for (i, b) in dst.iter_mut().enumerate() {
            *b = rom_byte(r, p + i);
        }
    }
}

fn total(d: &vb::Descriptor) -> usize {
    d.imem_load_size as usize + (d.dmem_load_size as usize).next_multiple_of(256)
}

fn bufer() -> &'static mut [u8] {
    let f = FWSEC_BUF.load(Ordering::Acquire);
    // SAFETY: las paginas NEUTRO de `fwsec_preparar`, de este fichero; la 3060
    // solo las LEE (prestadas sin escritura).
    unsafe { core::slice::from_raw_parts_mut(crate::ring0::mm::phys_to_virt(f) as *mut u8, (FWSEC_PAGINAS * 4096) as usize) }
}

/// **PREPARAR**: juzgar el descriptor que empieza en `en_rom`. `Ok(trozos)`.
pub fn fwsec_preparar(en_rom: u64) -> Result<u64, u32> {
    use crate::ring0::mm::phys;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return no(crate::ring0::plat::iommu::IOMMU_NO_SIN_GPU);
    }
    let e = en_rom as usize;
    if e + vb::DESCRIPTOR > vb::ROM_MAX {
        return no(IOMMU_NO_FWSEC_DESC);
    }
    let mut r = Bar0(bar0);
    let mut b = [0u8; vb::DESCRIPTOR];
    rom_a(&mut r, e, &mut b);
    let Some(d) = vb::descriptor(&b) else { return no(IOMMU_NO_FWSEC_DESC) };
    let t = total(&d);
    let bien = d.version() == 3
        && d.engine_id_mask & 0x0400 != 0
        && d.signature_count > 0
        && d.imem_load_size % 256 == 0
        && d.dmem_load_size > 0
        && t <= (FWSEC_PAGINAS * 4096) as usize
        && d.medida() >= vb::DESCRIPTOR + d.signature_count as usize * vb::FIRMA
        && e + d.medida() + t <= vb::ROM_MAX;
    if !bien {
        return no(IOMMU_NO_FWSEC_DESC);
    }
    if FWSEC_BUF.load(Ordering::Acquire) == 0 {
        // La 3060 lo leera por DMA: NEUTRO.
        let Some(f) = phys::alloc_frames_contig_de(FWSEC_PAGINAS, phys::Titular::Neutro) else {
            return no(crate::ring0::plat::iommu::IOMMU_NO_SIN_AREA);
        };
        FWSEC_BUF.store(f, Ordering::Release);
    }
    bufer().fill(0);
    // SAFETY: ver `FWSEC_DESC`.
    unsafe { *core::ptr::addr_of_mut!(FWSEC_DESC) = Some(d) };
    FWSEC_EN_ROM.store(en_rom, Ordering::Release);
    FWSEC_TROZOS.store(0, Ordering::Release);
    let n = t.div_ceil(4096) as u64;
    apuntar(|v| (v & (FWSEC_PRESTADO | 0xFF << FWSEC_MOTIVO_SHIFT)) | FWSEC_VALIDO | FWSEC_PREPARADO | n << FWSEC_TOTALES_SHIFT);
    crate::ring0::cabina::count("gpu", "L0b: FWSEC juzgado en la ROM; trozos de 4 KiB", n);
    Ok(n)
}

/// **TROZO k**: 4 KiB de ucode, de la ROM al bufer. `Ok(bits de los copiados)`.
pub fn fwsec_trozo(k: u64) -> Result<u64, u32> {
    let Some(d) = desc() else { return no(IOMMU_NO_FWSEC_SIN_PREPARAR) };
    let t = total(&d);
    let n = t.div_ceil(4096) as u64;
    if k >= n {
        return no(IOMMU_NO_FWSEC_SIN_PREPARAR);
    }
    let mut r = Bar0(crate::ring0::dev::gpu::bar0());
    let desde = k as usize * 4096;
    let hasta = (desde + 4096).min(d.imem_load_size as usize + d.dmem_load_size as usize);
    let origen = FWSEC_EN_ROM.load(Ordering::Acquire) as usize + d.medida() + desde;
    if hasta > desde {
        rom_a(&mut r, origen, &mut bufer()[desde..hasta]);
    }
    let bits = FWSEC_TROZOS.load(Ordering::Acquire) | 1 << k;
    FWSEC_TROZOS.store(bits, Ordering::Release);
    Ok(bits)
}

/// **CORRER**: parchear, firmar, prestar, cargar y ARRANCAR. `Ok(frts)` en
/// cuanto el falcon arranca; si acabo bien lo dice `INFO_GPU_FWSEC`.
pub fn fwsec_correr() -> Result<u64, u32> {
    let Some(d) = desc() else { return no(IOMMU_NO_FWSEC_SIN_PREPARAR) };
    let t = total(&d);
    let n = t.div_ceil(4096) as u64;
    if FWSEC_TROZOS.load(Ordering::Acquire) != (1u64 << n) - 1 {
        return no(IOMMU_NO_FWSEC_SIN_PREPARAR);
    }
    let bar0 = match candado_dma() {
        Ok(b) => b,
        Err(m) => return no(m),
    };
    let w = crate::ring0::dev::gpu::info_wpr2();
    if (w >> 32) as u32 >> 4 != 0 {
        return no(IOMMU_NO_WPR2_YA);
    }
    let fb = crate::ring0::dev::gpu::info_fb();
    if fb & crate::ring0::dev::gpu::GPU_FB_PLM_LEIBLE == 0 || (fb >> crate::ring0::dev::gpu::GPU_FB_GFW_SHIFT) & 0xFF != 0xFF {
        return no(IOMMU_NO_GFW);
    }
    // ** LA 3060 12G, Y SOLO ELLA (25-09): con el GFW acabado, la VRAM ya se
    // puede leer; FRTS y todo lo de despues se calcula desde ella.
    if !bmo_gpu_ga10x::identidad::vram_es_la_suya(fb as u32) {
        crate::ring0::cabina::warn("gpu", "no es la 3060 12G: la VRAM no son 12288 MiB, sino", fb & 0xFFFF_FFFF);
        return no(IOMMU_NO_OTRA_TARJETA);
    }
    let frts = vb::frts(
        fb as u32,
        crate::ring0::dev::gpu::info_vga() as u32,
        fb & crate::ring0::dev::gpu::GPU_FB_SIN_PANTALLA == 0,
    );
    let mut r = Bar0(bar0);
    let ucode = &mut bufer()[..t];

    // La orden: FRTS en su region.
    if bmo_gpu_ga10x::fwsec::parchear(ucode, &d, frts.desde, frts.hasta - frts.desde).is_err() {
        return no(IOMMU_NO_FWSEC_PARCHE);
    }
    apuntar(|v| v | FWSEC_PARCHEADO);
    let idx = match firmar_cargar_arrancar(&mut r, &d, ucode, "L0b: FWSEC-FRTS al falcon del GSP") {
        Ok(i) => i,
        Err(m) => return no(m),
    };
    apuntar(|v| v | FWSEC_ARRANCADO);
    crate::ring0::cabina::count("gpu", "L0b: FWSEC-FRTS ARRANCADO en el falcon del GSP; firma", idx as u64);
    Ok(frts.desde)
}

/// La firma que pide el fusible, el prestamo (una vez), la carga en el falcon
/// del GSP y el arranque: lo mismo para FRTS (al arrancar) y para SB (L0c5, al
/// apagar). `Ok(indice de la firma)`; el `Err` es el motivo, sin apuntar.
fn firmar_cargar_arrancar(r: &mut Bar0, d: &vb::Descriptor, ucode: &mut [u8], que: &str) -> Result<u32, u32> {
    use crate::ring0::plat::iommu as io;
    // La firma: la que pide el FUSIBLE (el fallo 4 de FastOS).
    let Some(reg) = vb::registro_fusible(d.engine_id_mask, d.ucode_id) else { return Err(IOMMU_NO_FWSEC_FIRMA) };
    let version = vb::version_del_fusible(bmo_gpu_ga10x::Registros::leer(r, reg));
    let Some(idx) = vb::indice_de_firma(d.signature_versions, version).filter(|&i| i < d.signature_count as u32) else {
        return Err(IOMMU_NO_FWSEC_FIRMA);
    };
    let mut firma = [0u8; vb::FIRMA];
    let en_rom = FWSEC_EN_ROM.load(Ordering::Acquire) as usize;
    rom_a(r, en_rom + vb::DESCRIPTOR + idx as usize * vb::FIRMA, &mut firma);
    if bmo_gpu_ga10x::fwsec::poner_firma(ucode, d, &firma).is_err() {
        return Err(IOMMU_NO_FWSEC_PARCHE);
    }
    apuntar(|v| (v & !(3 << FWSEC_FIRMA_SHIFT)) | (idx as u64 & 3) << FWSEC_FIRMA_SHIFT);

    // El prestamo, solo para leer, una vez.
    if FWSEC_ESTADO.load(Ordering::Acquire) & FWSEC_PRESTADO == 0 {
        if let Err(m) = io::prestar_gpu(IOVA_FWSEC, FWSEC_BUF.load(Ordering::Acquire), FWSEC_PAGINAS, false) {
            return Err(m);
        }
        apuntar(|v| v | FWSEC_PRESTADO);
    }

    // El falcon: como `FwsecFirmware::run` de nova-core, sin esperar al final.
    crate::ring0::cabina::info("gpu", que, 0);
    let mut tr = reloj();
    let boot0 = bmo_gpu_ga10x::Registros::leer(r, bmo_gpu_ga10x::BOOT_0);
    let cargado = fa::resetear(r, &mut tr, fa::GSP, boot0)
        .and_then(|_| fa::preparar_fbif(r, fa::GSP))
        .and_then(|_| fa::copiar(r, &mut tr, fa::GSP, IOVA_FWSEC, d.imem_phys_base, d.imem_load_size, true, true, 50_000))
        .and_then(|_| {
            fa::copiar(
                r,
                &mut tr,
                fa::GSP,
                IOVA_FWSEC + d.imem_load_size as u64,
                d.dmem_phys_base,
                (d.dmem_load_size).next_multiple_of(256),
                false,
                true,
                50_000,
            )
        });
    if let Err(e) = cargado {
        crate::ring0::cabina::warn("gpu", "L0b: el falcon no dejo cargar FWSEC; motivo", motivo(e));
        return Err(IOMMU_NO_FWSEC_FALCON);
    }
    fa::brom(r, fa::GSP, d.pkc_data_offset, d.engine_id_mask, d.ucode_id);
    if fa::arrancar(r, fa::GSP, 0).is_err() {
        return Err(IOMMU_NO_FWSEC_FALCON);
    }
    Ok(idx)
}

/// **FWSEC-SB** (L0c5, al APAGAR el GSP): el MISMO FWSEC de FRTS, que sigue
/// en su bufer desde el arranque, parcheado con la orden SB (0x19) y firmado
/// otra vez. Sin las condiciones de FRTS: aqui la WPR2 SI esta montada. Quien
/// llama ya se aseguro de que el GSP-RM se suspendio. `Ok(firma)` en cuanto el
/// falcon arranca; su error, en `descarga::SB_ERROR`.
pub fn fwsec_sb() -> Result<u64, u32> {
    let Some(d) = desc() else { return no(IOMMU_NO_FWSEC_SIN_PREPARAR) };
    let t = total(&d);
    let n = t.div_ceil(4096) as u64;
    if FWSEC_TROZOS.load(Ordering::Acquire) != (1u64 << n) - 1 || FWSEC_ESTADO.load(Ordering::Acquire) & FWSEC_ARRANCADO == 0 {
        return no(IOMMU_NO_FWSEC_SIN_PREPARAR);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return no(crate::ring0::plat::iommu::IOMMU_NO_SIN_GPU);
    }
    let mut r = Bar0(bar0);
    let ucode = &mut bufer()[..t];
    if bmo_gpu_ga10x::fwsec::parchear_sb(ucode, &d).is_err() {
        return no(IOMMU_NO_FWSEC_PARCHE);
    }
    match firmar_cargar_arrancar(&mut r, &d, ucode, "L0c5: FWSEC-SB al falcon del GSP (apagar)") {
        Ok(idx) => {
            crate::ring0::cabina::count("gpu", "L0c5: FWSEC-SB ARRANCADO en el falcon del GSP; firma", idx as u64);
            Ok(idx as u64)
        }
        Err(m) => no(m),
    }
}

/// `INFO_GPU_FWSEC`: `0..7` trozos copiados | `8..15` trozos totales | `16..17`
/// la firma usada | 18 parcheado | 19 prestado | 20 arrancado | 21 PARADO
/// (en vivo) | 22 hay WPR2 (en vivo) | `32..47` el codigo de FRTS de
/// `0x1438` (en vivo) | `48..55` el ultimo NO | 62 preparado | 63 valido.
pub fn info_fwsec() -> u64 {
    let mut v = FWSEC_ESTADO.load(Ordering::Acquire);
    if v & FWSEC_VALIDO == 0 {
        return 0;
    }
    v |= FWSEC_TROZOS.load(Ordering::Acquire).count_ones() as u64 & 0xFF;
    // ** El falcon del GSP ya no es de FWSEC (L0c3b, metal 24-09 07:10): tras
    // despertar el GSP, leerlo en vivo decia "ARRANCADO y todavia corriendo"
    // de un FWSEC que acabo hace rato. Se contesta con la FOTO de antes.
    if crate::ring0::dev::gpu_despertar::gsp_tomado() {
        v |= FWSEC_FOTO.load(Ordering::Acquire);
    } else {
        v |= fwsec_en_vivo(v);
    }
    if (crate::ring0::dev::gpu::info_wpr2() >> 32) as u32 >> 4 != 0 {
        v |= FWSEC_WPR2;
    }
    v
}

/// Lo que ahora dice el falcon del GSP de FWSEC: PARADO y su codigo de FRTS.
static FWSEC_FOTO: AtomicU64 = AtomicU64::new(0);

/// **La foto de FWSEC**, antes de que L0c3b le quite el falcon del GSP.
pub fn fotografiar_fwsec() {
    let v = FWSEC_ESTADO.load(Ordering::Acquire);
    if v & FWSEC_VALIDO != 0 {
        fwsec_en_vivo(v);
    }
}

fn fwsec_en_vivo(estado: u64) -> u64 {
    let mut v = 0;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 != 0 && estado & FWSEC_ARRANCADO != 0 {
        let mut r = Bar0(bar0);
        if let Ok((parado, _, _)) = fa::como_va(&mut r, fa::GSP) {
            if parado {
                v |= FWSEC_PARADO;
            }
        }
        let e = bmo_gpu_ga10x::Registros::leer(&mut r, 0x0000_1438);
        if !bmo_gpu_ga10x::es_error_pri(e) {
            v |= ((e >> 16) as u64 & 0xFFFF) << FWSEC_ERROR_SHIFT;
        }
        FWSEC_FOTO.store(v, Ordering::Release);
    }
    v
}

/// `INFO_GPU_FWSEC_BUZON`: MAILBOX0 | MAILBOX1 << 32 del falcon del GSP, si
/// FWSEC arranco.
pub fn info_fwsec_buzon() -> u64 {
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || FWSEC_ESTADO.load(Ordering::Acquire) & FWSEC_ARRANCADO == 0 {
        return 0;
    }
    if crate::ring0::dev::gpu_despertar::gsp_tomado() {
        // Los buzones del GSP ya son del GSP-RM; los de FWSEC acabaron en 0.
        return 0;
    }
    match fa::como_va(&mut Bar0(bar0), fa::GSP) {
        Ok((_, m0, m1)) => m0 as u64 | (m1 as u64) << 32,
        Err(_) => 0,
    }
}
