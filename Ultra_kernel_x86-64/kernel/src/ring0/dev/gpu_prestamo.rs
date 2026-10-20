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
//! bits), prestada SOLO PARA LEER en [`IOVA_PRUEBA`]: el paso `prestar` del
//! arranque, que prueba la traduccion de la IOMMU antes del firmware.

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

// == El acceso a la 3060 para el DMA de sus falcons ============================
//
// `gpu fuego` y `gpu frontera` (M0d3, 24-09), las pruebas a mano del DMA del
// falcon del GSP, se retiraron el 17-10: tocar ese falcon antes del booter
// daba 0x15 (EL_0x15.md) y FWSEC, el booter y el GSP-RM ya prueban ese camino
// en cada arranque. Queda lo comun: BAR0, el reloj y el candado.
//
// ** EL CANDADO: la 3060 TRADUCIDA y releida y el Bus Master YA encendido por
// E2 (`gpu vblank`). Esto no lo enciende: el Bus Master tiene un solo
// propietario, y es quien lo apaga cuando la IOMMU se apaga o la 3060 vuelve a
// ver (`syscall/op_maquina.rs`).

use bmo_gpu_ga10x::falcon::{self as fa, NoFuego};

pub const IOMMU_NO_SIN_BUS_MASTER: u32 = 16;

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

// == L0b: FWSEC-FRTS (2026-09-24) ==============================================
//
// El firmware FIRMADO de la VBIOS corre en el falcon del GSP y aparta la WPR2
// en la VRAM. Es la puerta del booter y del GSP-RM. Por el mismo camino que la
// prueba de la pagina prestada: el ucode se copia de la ROM a paginas NEUTRO, se PRESTA a la
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
    super::gpu_capture::before_frts(&mut Bar0(bar0));
    let w = crate::ring0::dev::gpu::info_wpr2();
    if (w >> 32) as u32 >> 4 != 0 {
        return no(IOMMU_NO_WPR2_YA);
    }
    // ** ESPERAR A LA TARJETA COMO OpenRM (20-10, PLAN_GPU_COMO_LINUX 4c):
    // `gpuWaitForGfwBootComplete_TU102` da hasta 2,05 s a que el falcon del
    // GSP pare (alli corre el arranque de la propia tarjeta) y el GFW diga
    // COMPLETED con su PLM legible. Antes BMO-X miraba UNA vez y decia NO.
    use bmo_gpu_ga10x::falcon::Reloj;
    let mut tr = reloj();
    let desde = tr.us();
    let fb = loop {
        let fb = crate::ring0::dev::gpu::info_fb();
        let gfw = fb & crate::ring0::dev::gpu::GPU_FB_PLM_LEIBLE != 0
            && (fb >> crate::ring0::dev::gpu::GPU_FB_GFW_SHIFT) & 0xFF == 0xFF;
        let parado = matches!(fa::como_va(&mut Bar0(bar0), fa::GSP), Ok((true, _, _)));
        if gfw && parado {
            break fb;
        }
        if tr.us().saturating_sub(desde) >= GFW_ESPERA_US {
            // El motivo 24 no dice cual de las dos condiciones falto.
            crate::ring0::cabina::warn("gpu", "L0b: espera GFW agotada; INFO_FB final", fb);
            use bmo_gpu_ga10x::Registros;
            let mut r = Bar0(bar0);
            crate::ring0::cabina::warn("gpu", "L0b: espera GFW agotada; GSP CPUCTL crudo", r.leer(fa::GSP + fa::CPUCTL) as u64);
            crate::ring0::cabina::warn("gpu", "L0b: espera GFW agotada; PLM crudo", r.leer(bmo_gpu_ga10x::aon::GFW_PLM) as u64);
            return no(IOMMU_NO_GFW);
        }
    };
    let esperado = tr.us() - desde;
    if esperado > 1000 {
        crate::ring0::cabina::count("gpu", "L0b: se espero a la tarjeta (GFW COMPLETED y el GSP parado), como OpenRM; us", esperado);
    }
    mirar_candado_mmu(&mut Bar0(bar0));
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
    // ** EL MAPA DE LA 3060 TERCA (29-09, PLAN_LA_3060): lo que se sabia
    // JUSTO ANTES de FRTS, solo leido, para comparar el arranque bueno con
    // el malo: si el falcon del GSP ya estaba PARADO (NVIDIA lo espera antes
    // de mirar el GFW, `gpuWaitForGfwBootComplete_TU102`; aqui tambien), el
    // espacio VGA crudo del que sale la region, si hay pantalla y si la 3060
    // anuncia FLR. No cambia ningun paso.
    let antes = match fa::como_va(&mut r, fa::GSP) {
        Ok((parado, _, _)) => ANTES_LEIDO | parado as u64,
        Err(_) => 0,
    };
    FWSEC_ANTES.store(
        antes | ((fb & crate::ring0::dev::gpu::GPU_FB_SIN_PANTALLA == 0) as u64) << 2 | anuncia_flr() << 3 | (crate::ring0::dev::gpu::info_vga() & 0xFFFF_FFFF) << 32,
        Ordering::Release,
    );
    // E6: PTIMER en hora antes de FWSEC, como OpenRM (solo con `ptimer`).
    crate::ring0::dev::gpu_despertar::g0_ptimer(&mut r);
    let ucode = &mut bufer()[..t];

    // La orden: FRTS en su region.
    let Ok(parche) = bmo_gpu_ga10x::fwsec::parchear(ucode, &d, frts.desde, frts.hasta - frts.desde) else {
        return no(IOMMU_NO_FWSEC_PARCHE);
    };
    // Para la autopsia: donde quedan, en la DMEM del falcon, la DMEMMAPPER
    // y la orden (el falcon las ve desde `dmem_phys_base`), y la region.
    FWSEC_DMEM.store((d.dmem_phys_base + parche.dmemmapper) as u64 | ((d.dmem_phys_base + parche.orden_en) as u64) << 32, Ordering::Release);
    FWSEC_REGION.store(frts.desde >> 12 | (frts.hasta >> 12) << 32, Ordering::Release);
    FWSEC_AUTOPSIA.store(false, Ordering::Release);
    apuntar(|v| v | FWSEC_PARCHEADO);
    let idx = match firmar_cargar_arrancar(&mut r, &d, ucode, "L0b: FWSEC-FRTS al falcon del GSP") {
        Ok(i) => i,
        Err(m) => return no(m),
    };
    apuntar(|v| v | FWSEC_ARRANCADO);
    FRTS_TSC.store(crate::ring0::task::scheduler::rdtsc(), Ordering::Release);
    FWSEC_AUTOPSIA.store(false, Ordering::Release);
    crate::ring0::cabina::count("gpu", "L0b: FWSEC-FRTS ARRANCADO en el falcon del GSP; firma", idx as u64);
    Ok(frts.desde)
}

/// Lo que `gpuWaitForGfwBootComplete_TU102` le da a la tarjeta: 50 ms de
/// arranque y 2 s para acabar (`FWSECLIC_PROG_*_TIMEOUT`).
const GFW_ESPERA_US: u64 = 2_050_000;

/// La region bloqueada por la VBIOS en la MMU: `(desde, hasta)` en bytes, si
/// se puede leer y es valida. OpenRM 615 pone el techo de la WPR2 en
/// `min(este desde, la VGA)` (`kgspPopulateWprMeta_TU102`).
static MMU_CANDADO: AtomicU64 = AtomicU64::new(0);

/// **MEDIR antes de cambiar** (20-10): BMO-X no la tiene en cuenta. Se lee y
/// se dice; si cae por debajo de la VGA, podria condicionar el techo WPR.
/// No demuestra solapamiento. El reparto no cambia hasta verlo en la 3060.
fn mirar_candado_mmu(r: &mut Bar0) {
    use bmo_gpu_ga10x::Registros;
    use bmo_gpu_ga10x::aon::{self, MMU_LOCK_PLM, MMU_LOCK_LO, MMU_LOCK_HI};
    // Un intento que no puede leer no hereda la region del anterior.
    MMU_CANDADO.store(0, Ordering::Release);
    let plm = r.leer(MMU_LOCK_PLM);
    if bmo_gpu_ga10x::es_error_pri(plm) || plm & 1 == 0 {
        crate::ring0::cabina::info("gpu", "L0b: el candado de la MMU de la VBIOS no se deja leer (PLM)", plm as u64);
        return;
    }
    let lo = r.leer(MMU_LOCK_LO);
    let hi = r.leer(MMU_LOCK_HI);
    let c = match aon::candado_mmu(plm, lo, hi) {
        Ok(Some(c)) => c,
        Ok(None) => {
            MMU_CANDADO.store(1 << 63, Ordering::Release);
            crate::ring0::cabina::info("gpu", "L0b: sin candado MMU (hi < lo); limites crudos", lo as u64 | (hi as u64) << 32);
            return;
        }
        Err(_) => {
            crate::ring0::cabina::warn("gpu", "L0b: candado MMU con lectura PRI; limites crudos, no direcciones", lo as u64 | (hi as u64) << 32);
            return;
        }
    };
    MMU_CANDADO.store(3 << 62 | c.desde >> 12 | (c.hasta >> 12) << 32, Ordering::Release);
    crate::ring0::cabina::count("gpu", "L0b: candado MMU valido; hasta (bytes)", c.hasta);
    let raw_vga = crate::ring0::dev::gpu::info_vga() as u32;
    let vga = (((raw_vga >> 8) & 0xFF_FFFF) as u64) << 16;
    if bmo_gpu_ga10x::es_error_pri(raw_vga) || vga == 0 {
        crate::ring0::cabina::info("gpu", "L0b: candado MMU valido, comparacion VGA sin dato; VGA cruda", raw_vga as u64);
    } else if c.desde < vga {
        crate::ring0::cabina::warn("gpu", "L0b: el candado MMU condicionaria el techo WPR de OpenRM; no demuestra solapamiento; desde", c.desde);
    } else {
        crate::ring0::cabina::count("gpu", "L0b: candado de la MMU de la VBIOS, por encima de la VGA; desde", c.desde);
    }
}

/// Consulta interna: paginas desde/hasta en bits 0..27/32..59; bit 63 lectura
/// valida, bit 62 region presente. Cero indica ausencia de lectura valida.
pub fn info_mmu_candado() -> u64 {
    MMU_CANDADO.load(Ordering::Acquire)
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
    let cargado = fa::resetear_con(r, &mut tr, fa::GSP, boot0, crate::ring0::dev::gpu_despertar::g0_pulso())
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

/// Cuando arranco FWSEC-FRTS por ultima vez (TSC). Lo mira la autopsia del
/// booter: EL MOMENTO, cuanto paso de FRTS al booter (29-09).
static FRTS_TSC: AtomicU64 = AtomicU64::new(0);

/// El TSC del ultimo arranque de FWSEC-FRTS; 0 si no arranco.
pub fn frts_tsc() -> u64 {
    FRTS_TSC.load(Ordering::Acquire)
}

/// Lo que ahora dice el falcon del GSP de FWSEC: PARADO y su codigo de FRTS.
static FWSEC_FOTO: AtomicU64 = AtomicU64::new(0);

/// ** LA AUTOPSIA DE FWSEC-FRTS (29-09). El metal (28-09 23:09), tras el
/// reinicio por el bus del cargador (`siempre`): FWSEC se paro con MAILBOX0 0
/// y su codigo de FRTS (`0x1438 >> 16`) a 0 -- "bien" -- y la WPR2 SIN montar.
/// Nada de lo que se miraba dice por que. Esto mira lo que FWSEC DEJA ESCRITO
/// al pararse, SOLO LEYENDO, una vez por arranque y en los buenos tambien (un
/// fallo se entiende comparando): el registro `0x1438` entero, la WPR2 cruda,
/// la region que se le pidio, y la DMEM de su DMEMMAPPER (la cabecera, con
/// `init_cmd` en +44 y los buferes de entrada y SALIDA) y de su orden. Va a
/// CABINA (`cabina fallos` si fue sin WPR2; `cabina` si fue bien).
///
/// Donde quedan en la DMEM la DMEMMAPPER (0..31) y la orden (32..63).
static FWSEC_DMEM: AtomicU64 = AtomicU64::new(0);
/// La region de FRTS pedida, en paginas: desde (0..31), hasta (32..63).
static FWSEC_REGION: AtomicU64 = AtomicU64::new(0);
static FWSEC_AUTOPSIA: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// Lo de justo ANTES de FRTS (ver `fwsec_correr`): bit 0 el falcon del GSP
/// parado, 1 se pudo leer, 2 hay pantalla, 3 la 3060 anuncia FLR, 4 su
/// capacidad PCIe se hallo; arriba, `0x625F04` crudo.
static FWSEC_ANTES: AtomicU64 = AtomicU64::new(0);
const ANTES_LEIDO: u64 = 1 << 1;

/// **Si la 3060 anuncia Function Level Reset** (R4 del mapa, solo lectura):
/// bit 1 = la capacidad PCIe se hallo (el 4 de `FWSEC_ANTES`), bit 0 =
/// DevCap bit 28. Anunciarlo NO dice que limpie la WPR2.
fn anuncia_flr() -> u64 {
    use crate::ring0::dev::pci::cfg_read32;
    let Some((b, d, f)) = crate::ring0::dev::gpu::bdf() else { return 0 };
    if cfg_read32(b, d, f, 0x04) >> 16 & 0x10 == 0 {
        return 0;
    }
    let mut p = (cfg_read32(b, d, f, 0x34) & 0xFC) as u8;
    // Una cadena que da el aparato: con tope, aunque mienta.
    for _ in 0..48 {
        if p < 0x40 {
            break;
        }
        let c = cfg_read32(b, d, f, p);
        if c & 0xFF == 0x10 {
            return 2 | (cfg_read32(b, d, f, p + 4) >> 28 & 1) as u64;
        }
        p = (c >> 8 & 0xFC) as u8;
    }
    0
}

/// Las capacidades extendidas de la 3060 como mascara (bit = id, las de id
/// < 64; `0x15` = Resizable BAR): lo que el cargador NO restaura tras un
/// reinicio por el bus (solo los 256 bytes de 0xCF8).
fn capacidades_extendidas() -> u64 {
    let Some((b, d, f)) = crate::ring0::dev::gpu::bdf() else { return 0 };
    let mut caps = [crate::ring0::dev::pci::CapExt { id: 0, version: 0, offset: 0 }; 32];
    let n = crate::ring0::dev::pci::caps_extendidas(b, d, f, &mut caps);
    caps[..n].iter().filter(|c| c.id < 64).fold(0, |m, c| m | 1 << c.id)
}

/// **La autopsia**: con el falcon del GSP PARADO tras FWSEC-FRTS.
fn autopsiar_fwsec(r: &mut Bar0) {
    if FWSEC_AUTOPSIA.swap(true, Ordering::AcqRel) {
        return;
    }
    super::gpu_capture::after_frts(r);
    let w = crate::ring0::dev::gpu::info_wpr2();
    let montada = (w >> 32) as u32 >> 4 != 0;
    let decir = |msg: &str, v: u64| if montada { crate::ring0::cabina::info("gpu", msg, v) } else { crate::ring0::cabina::warn("gpu", msg, v) };
    decir(if montada { "L0b autopsia FWSEC (BUENA, con WPR2): 0x1438 entero" } else { "L0b autopsia FWSEC SIN WPR2: 0x1438 entero" }, bmo_gpu_ga10x::Registros::leer(r, 0x0000_1438) as u64);
    decir("L0b autopsia FWSEC: la WPR2 cruda (lo | hi << 32)", w);
    decir("L0b autopsia FWSEC: la region de FRTS pedida, paginas (desde | hasta << 32)", FWSEC_REGION.load(Ordering::Acquire));
    decir(
        "L0b autopsia FWSEC: ANTES de FRTS -- bit 0 el falcon del GSP PARADO, 1 leido, 2 pantalla, 3 FLR anunciado, 4 su capacidad PCIe; arriba 0x625F04 crudo",
        FWSEC_ANTES.load(Ordering::Acquire),
    );
    decir("L0b autopsia FWSEC: capacidades extendidas de la 3060 (bit = id; 0x15 Resizable BAR), las que el cargador NO restaura", capacidades_extendidas());
    let donde = FWSEC_DMEM.load(Ordering::Acquire);
    let mut palabras = [0u32; 16];
    for (que, dmem) in [("L0b autopsia FWSEC: DMEMMAPPER", donde as u32), ("L0b autopsia FWSEC: la ORDEN", (donde >> 32) as u32)] {
        bmo_gpu_ga10x::falcon::leer_dmem(r, fa::GSP, dmem, &mut palabras);
        decir(que, dmem as u64);
        for par in palabras.chunks(2) {
            decir("   dos palabras (la de abajo | la de arriba << 32)", par[0] as u64 | (par[1] as u64) << 32);
        }
    }
}

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
                autopsiar_fwsec(&mut r);
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
