//! **E2 -- LA 3060 AVISA DEL VBLANK**, por MSI y detras de un CANDADO.
//!
//! [carril]  ROJO      la primera ESCRITURA en la grafica: su Bus Master, su
//!                     MSI y el aviso de UNA cabeza; y atiende su interrupcion
//! [consumo] NADA      corre por orden (`gpu vblank`), y despues ~60 avisos
//!                     por segundo que son tres lecturas y cuatro escrituras
//!
//! [eje]     CORRECCION -- el paso E2 de `docs/plan/PLAN_LA_3060.md`
//! [riesgo]  AJENO -- la tarjeta la dejo asi el GOP; aqui solo se enciende un
//!           aviso encima de lo que ya barre
//!
//! # Por que existe (2026-09-24)
//!
//! Hasta hoy BMO-X solo LEIA la 3060 (`dev/gpu.rs`). E2 es la primera vez que
//! le escribe: que avise cuando acaba de barrer la pantalla, para que el
//! compositor pueda dormir hasta ese instante en vez de preguntar la linea (E3).
//!
//! # EL CANDADO, que es la razon de que E2 exista ahora y no antes
//!
//! Un MSI es una ESCRITURA de la tarjeta (a `0xFEE.....`), y para eso necesita
//! el Bus Master -- que es el mismo bit que le daria DMA a TODA la RAM. La
//! regla de las etapas lo pone en MID, detras de la IOMMU. Asi que el Bus Master
//! se enciende SOLO si, en ese instante:
//!
//! ```text
//!    la IOMMU esta ENCENDIDA                     (M0c)
//!    la entrada de la 3060 es BLOQUEADA y se
//!    releyo asi, y es la de ESTE BDF             (M0e: `gpu cegar`)
//!    en ese BDF sigue habiendo una NVIDIA
//! ```
//!
//! Con la entrada bloqueada, el DMA no pasa y el MSI si (las palabras de
//! interrupcion se conservan). Y el candado no se comprueba solo al abrir:
//! `iommu apagar` y `gpu ver` APAGAN E2 antes de quitarle la venda
//! (`syscall/op_maquina.rs`). Nunca hay un instante con el Bus Master
//! encendido y la 3060 viendo la RAM.
//!
//! # Lo que se escribe, y en que orden
//!
//! El orden de los registros de la tarjeta vive en `bmo_gpu_ga10x::vblank`,
//! probado en el anfitrion. Aqui, el pegamento:
//!
//! ```text
//!    1. callar la cima y la hoja de la pantalla (por si el GOP dejo algo)
//!    2. el MSI: a DONDE (el LAPIC del BSP, vector 50)
//!    3. el Bus Master, por `pci::enable_mem_bus_master` (el portero la adopta)
//!    4. armar: hojas bloqueadas, el aviso de la cabeza, la hoja, la cima
//! ```
//!
//! # Y si algo va mal DENTRO de la interrupcion
//!
//! Desde el manejador no se toca la configuracion PCI (dos puertos, y otro
//! nucleo podria estar a medias con ellos): se CALLA la cima por MMIO y se
//! apunta el motivo. El Bus Master lo retira la proxima orden (`gpu vblank
//! off`), y mientras tanto la 3060 sigue ciega. Dos motivos:
//!
//! ```text
//!    TORMENTA     mas de 1.000 avisos en un segundo (lo esperado: ~60)
//!    NO CONTESTA  un registro devolvio un error del anillo PRIV
//! ```

use bmo_gpu_ga10x::vblank::{self as v, Registros};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// -- El contrato, espejo de `bmo_abi::...::informe::E2_*` ----------------------

/// `INFO_GPU_VBLANK`: `0..31` VBLANKs de NUESTRA cabeza por interrupcion |
/// `32..55` entradas al vector (saturado) | 62 callada desde la interrupcion |
/// 63 armado.
pub const E2_ENTRADAS_SHIFT: u64 = 32;
pub const E2_CALLADA: u64 = 1 << 62;
pub const E2_ARMADO: u64 = 1 << 63;

/// `INFO_GPU_E2`: la ESCALERA, leida al preguntar. Cada peldano es un sitio
/// donde el aviso se puede quedar; en el metal, el primero que falte dice donde.
pub const E2_MSI: u64 = 1 << 0;
pub const E2_MSI_MASCARA: u64 = 1 << 1;
pub const E2_BME: u64 = 1 << 2;
pub const E2_CIEGA: u64 = 1 << 3;
pub const E2_ENCENDIDO: u64 = 1 << 4;
pub const E2_EVENTO: u64 = 1 << 5;
pub const E2_HOJA: u64 = 1 << 6;
pub const E2_CIMA: u64 = 1 << 7;
pub const E2_VECTOR: u64 = 1 << 8;
pub const E2_AJENOS_SHIFT: u64 = 16;
pub const E2_MOTIVO_SHIFT: u64 = 32;
pub const E2_OTRAS_SHIFT: u64 = 40;
pub const E2_CABEZA_SHIFT: u64 = 48;
pub const E2_VALIDA: u64 = 1 << 63;

/// Por que se apago (bits 32..39 de `INFO_GPU_E2`).
pub const E2_APAGADO_ORDEN: u64 = 1;
pub const E2_APAGADO_TORMENTA: u64 = 2;
pub const E2_APAGADO_CANDADO: u64 = 3;
pub const E2_APAGADO_NO_CONTESTA: u64 = 4;

/// Motivos del NO de `IOMMU_OP_E2_ENCENDER`, en las banderas de `ERROR_NEGADO`.
/// Siguen la cuenta de `plat/iommu.rs` (hasta el 6): es la misma puerta.
pub const IOMMU_NO_GPU_VE: u32 = 7;
pub const IOMMU_NO_SIN_MSI: u32 = 8;
pub const IOMMU_NO_SIN_CABEZA: u32 = 9;
pub const IOMMU_NO_SIN_VECTOR: u32 = 10;
pub const IOMMU_NO_E2_NO_ARMA: u32 = 11;

/// Mas avisos que esto en un segundo es una TORMENTA: la cima se calla.
const TORMENTA_MAX: u64 = 1000;

/// Sobre esta llave dormira quien espere el VBLANK (E3). Hoy no la usa nadie.
pub const LLAVE: u64 = 0x0B1A_0000_0000_0001;

// -- El estado ------------------------------------------------------------------

/// El vector 50 esta en la IDT (se instala AL ARRANCAR, ver `preparar`).
static VECTOR_LISTO: AtomicBool = AtomicBool::new(false);
/// `0..2` cabeza | `8..39` la mascara que habia antes | 62 callada | 63 armado.
static ESTADO: AtomicU64 = AtomicU64::new(0);
const ESTADO_MASCARA_SHIFT: u64 = 8;
static VBLANKS: AtomicU64 = AtomicU64::new(0);
static ENTRADAS: AtomicU64 = AtomicU64::new(0);
static AJENOS: AtomicU64 = AtomicU64::new(0);
static OTRAS: AtomicU64 = AtomicU64::new(0);
static MOTIVO: AtomicU64 = AtomicU64::new(0);
static VENTANA_TSC: AtomicU64 = AtomicU64::new(0);
static EN_VENTANA: AtomicU64 = AtomicU64::new(0);

/// Los registros de la 3060, por BAR0 en el physmap: valen bajo cualquier CR3.
struct Bar0(u64);

impl Registros for Bar0 {
    fn leer(&mut self, reg: u32) -> u32 {
        // SAFETY: BAR0 de la grafica por el physmap (no cacheable por el MTRR
        // de la placa, como en `dev/gpu.rs`); registros de 32 bits alineados de
        // los 16 MiB de BAR0, los que nouveau usa en el GA106.
        unsafe { ((self.0 + reg as u64) as *const u32).read_volatile() }
    }
    fn escribir(&mut self, reg: u32, x: u32) {
        // SAFETY: como `leer`. Solo los registros de `bmo_gpu_ga10x::vblank`.
        unsafe { ((self.0 + reg as u64) as *mut u32).write_volatile(x) }
    }
}

/// **Al arrancar**: el vector 50 en la IDT, si hay una NVIDIA. Sin MSI
/// programado nadie puede dispararlo. Va aqui y no en `encender` porque la IDT
/// vive donde solo se alcanza al arrancar: dentro de un syscall esa direccion
/// no existe (el mismo fallo que `placa` pago el 24-09).
pub fn preparar() {
    if crate::ring0::dev::gpu::bdf().is_none() {
        return;
    }
    let idt = crate::info::idt_ptr();
    if idt != 0 && crate::ring0::plat::irq::instalar_gpu(idt) {
        VECTOR_LISTO.store(true, Ordering::Release);
    }
}

fn armado() -> bool {
    ESTADO.load(Ordering::Acquire) & (E2_ARMADO | E2_CALLADA) != 0
}

/// **ENCENDER E2.** `Ok(bdf << 32 | cabeza)`. Desde un syscall (IF=0): el
/// primer aviso entra al volver a Ring 3.
pub fn encender() -> Result<u64, u32> {
    use crate::ring0::plat::iommu as io;
    let e = ESTADO.load(Ordering::Acquire);
    if e & E2_ARMADO != 0 {
        return Ok(e & 0x7);
    }
    if !VECTOR_LISTO.load(Ordering::Acquire) {
        return Err(IOMMU_NO_SIN_VECTOR);
    }
    let Some((b, d, f)) = crate::ring0::dev::gpu::bdf() else { return Err(io::IOMMU_NO_SIN_GPU) };
    if crate::ring0::dev::pci::cfg_read32(b, d, f, 0) & 0xFFFF != 0x10DE {
        crate::ring0::cabina::warn("gpu", "E2: en el BDF de la sonda ya no hay una NVIDIA: no se toca", 0);
        return Err(io::IOMMU_NO_SIN_GPU);
    }
    let bdf = (b as u16) << 8 | (d as u16) << 3 | f as u16;

    // == EL CANDADO ==
    if io::info_viva() & io::IOMMU_VIVA_ENCENDIDA == 0 {
        crate::ring0::cabina::warn("gpu", "E2: la IOMMU esta APAGADA: el Bus Master de la 3060 no se enciende", 0);
        return Err(io::IOMMU_NO_APAGADA);
    }
    let g = io::info_gpu();
    if g & io::IOMMU_GPU_CIEGA == 0 || g & io::IOMMU_GPU_RELEIDA == 0 || (g & 0xFFFF) as u16 != bdf {
        crate::ring0::cabina::warn("gpu", "E2: la 3060 NO esta ciega en la IOMMU: el candado no abre", g);
        return Err(IOMMU_NO_GPU_VE);
    }

    let (Some(cabeza), bar0) = (crate::ring0::dev::gpu::cabeza(), crate::ring0::dev::gpu::bar0()) else {
        return Err(IOMMU_NO_SIN_CABEZA);
    };
    if bar0 == 0 {
        return Err(IOMMU_NO_SIN_CABEZA);
    }
    let mut r = Bar0(bar0);

    // 1. Nada avisa mientras se prepara.
    v::callar(&mut r);
    // 2. A DONDE: el LAPIC del BSP. Preguntado, no supuesto (LEY 24).
    let bsp = crate::ring0::plat::smp::tramp::BSP_APIC.load(Ordering::Relaxed);
    let destino = if bsp == u32::MAX { crate::ring0::plat::smp::tramp::apic_id() } else { bsp } as u8;
    let vector = crate::ring0::plat::irq::VECTOR_GPU as u8;
    if !crate::ring0::dev::pci::msi_activar(b, d, f, vector, destino) {
        crate::ring0::cabina::warn("gpu", "E2: la 3060 no anuncia MSI: no se enciende nada", 0);
        return Err(IOMMU_NO_SIN_MSI);
    }
    // 3. El Bus Master, por el UNICO sitio que lo enciende.
    crate::ring0::dev::pci::enable_mem_bus_master(b, d, f);
    crate::ring0::cabina::count("gpu", "E2: Bus Master de la 3060 ENCENDIDO, CIEGA en la IOMMU; BDF", bdf as u64);
    // 4. Que avise.
    match v::armar(&mut r, cabeza) {
        Ok(a) => {
            VENTANA_TSC.store(0, Ordering::Relaxed);
            MOTIVO.store(0, Ordering::Relaxed);
            ESTADO.store(
                E2_ARMADO | a.cabeza as u64 | (a.mascara_antes as u64) << ESTADO_MASCARA_SHIFT,
                Ordering::Release,
            );
            crate::ring0::cabina::count("gpu", "E2: el VBLANK de la 3060 AVISA por MSI; cabeza", cabeza as u64);
            Ok((bdf as u64) << 32 | cabeza as u64)
        }
        Err(e) => {
            let _ = crate::ring0::dev::pci::bus_master_apagar(b, d, f);
            crate::ring0::cabina::warn("gpu", "E2: la pantalla no acepto el aviso: Bus Master retirado", e as u64);
            Err(IOMMU_NO_E2_NO_ARMA)
        }
    }
}

/// **APAGAR E2** desde un syscall: la cabeza como estaba y el Bus Master
/// RETIRADO. `true` si estaba encendido (o callado por la interrupcion).
pub fn apagar(motivo: u64) -> bool {
    let e = ESTADO.swap(0, Ordering::AcqRel);
    if e & (E2_ARMADO | E2_CALLADA) == 0 {
        return false;
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 != 0 {
        let a = v::Armado {
            cabeza: (e & 0x7) as u32,
            mascara_antes: (e >> ESTADO_MASCARA_SHIFT) as u32,
        };
        v::desarmar(&mut Bar0(bar0), a);
    }
    if let Some((b, d, f)) = crate::ring0::dev::gpu::bdf() {
        if !crate::ring0::dev::pci::bus_master_apagar(b, d, f) {
            crate::ring0::cabina::warn("gpu", "E2: se pidio retirar el Bus Master de la 3060 y lo MANTIENE", 0);
        }
    }
    // El motivo de la interrupcion (tormenta, no contesta) no se pisa.
    if e & E2_CALLADA == 0 {
        MOTIVO.store(motivo, Ordering::Release);
    }
    crate::ring0::cabina::count("gpu", "E2: APAGADO -- aviso quitado y Bus Master retirado; motivo", motivo);
    true
}

/// **EL AVISO.** Desde `plat/irq.rs`, con la tarea que fuera parada: solo
/// MMIO, nada de configuracion PCI ni de CABINA.
pub fn atender_irq() {
    ENTRADAS.fetch_add(1, Ordering::Relaxed);
    let e = ESTADO.load(Ordering::Acquire);
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return;
    }
    let mut r = Bar0(bar0);
    if e & E2_ARMADO == 0 {
        // Un aviso sin E2 armado: que no se repita.
        v::callar(&mut r);
        return;
    }
    let a = v::atender(&mut r, (e & 0x7) as u32);
    if a.vblank {
        VBLANKS.fetch_add(1, Ordering::Relaxed);
    }
    if a.otras_cabezas != 0 {
        OTRAS.fetch_add(a.otras_cabezas as u64, Ordering::Relaxed);
    }
    if a.ajeno != 0 || a.hoja_ajena != 0 {
        AJENOS.fetch_add(1, Ordering::Relaxed);
    }
    if a.no_contesta {
        callar_desde_irq(&mut r, E2_APAGADO_NO_CONTESTA);
        return;
    }
    if tormenta() {
        callar_desde_irq(&mut r, E2_APAGADO_TORMENTA);
        return;
    }
    v::rearmar(&mut r);
}

fn tormenta() -> bool {
    use crate::ring0::task::scheduler::{rdtsc, tsc_freq};
    let t = rdtsc();
    let hz = tsc_freq().max(1);
    if t.saturating_sub(VENTANA_TSC.load(Ordering::Relaxed)) >= hz {
        VENTANA_TSC.store(t, Ordering::Relaxed);
        EN_VENTANA.store(1, Ordering::Relaxed);
        return false;
    }
    EN_VENTANA.fetch_add(1, Ordering::Relaxed) + 1 > TORMENTA_MAX
}

fn callar_desde_irq(r: &mut Bar0, motivo: u64) {
    v::callar(r);
    let e = ESTADO.load(Ordering::Acquire);
    ESTADO.store((e & !E2_ARMADO) | E2_CALLADA, Ordering::Release);
    MOTIVO.store(motivo, Ordering::Release);
}

// -- Lo que sube a Ring 3 --------------------------------------------------------

/// `INFO_GPU_VBLANK`.
pub fn info_vblank() -> u64 {
    let e = ESTADO.load(Ordering::Acquire);
    (VBLANKS.load(Ordering::Relaxed) & 0xFFFF_FFFF)
        | ENTRADAS.load(Ordering::Relaxed).min(0xFF_FFFF) << E2_ENTRADAS_SHIFT
        | e & (E2_ARMADO | E2_CALLADA)
}

/// `INFO_GPU_E2`: la escalera, leida AHORA.
pub fn info_e2() -> u64 {
    use crate::ring0::plat::iommu as io;
    let Some((b, d, f)) = crate::ring0::dev::gpu::bdf() else { return 0 };
    let mut x = E2_VALIDA;
    let m = crate::ring0::dev::pci::msi_leer(b, d, f);
    if m.enable {
        x |= E2_MSI;
    }
    if m.enmascarado {
        x |= E2_MSI_MASCARA;
    }
    if crate::ring0::dev::pci::comando(b, d, f) & 0b100 != 0 {
        x |= E2_BME;
    }
    if io::info_gpu() & io::IOMMU_GPU_CIEGA != 0 {
        x |= E2_CIEGA;
    }
    if VECTOR_LISTO.load(Ordering::Acquire) {
        x |= E2_VECTOR;
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if let (Some(c), true) = (crate::ring0::dev::gpu::cabeza(), bar0 != 0) {
        let mut r = Bar0(bar0);
        let leido = |r: &mut Bar0, reg: u32, bit: u32| {
            let val = r.leer(reg);
            !bmo_gpu_ga10x::es_error_pri(val) && val & bit != 0
        };
        if leido(&mut r, v::cabeza_encendido(c), v::VBLANK) {
            x |= E2_ENCENDIDO;
        }
        if leido(&mut r, v::cabeza_evento(c), v::VBLANK) {
            x |= E2_EVENTO;
        }
        if leido(&mut r, v::hoja_estado(v::HOJA_PANTALLA), v::BIT_PANTALLA) {
            x |= E2_HOJA;
        }
        if leido(&mut r, v::CIMA_ESTADO, v::CIMA_PANTALLA) {
            x |= E2_CIMA;
        }
        x |= (c as u64 & 0x7) << E2_CABEZA_SHIFT;
    }
    x | AJENOS.load(Ordering::Relaxed).min(0xFFFF) << E2_AJENOS_SHIFT
        | (MOTIVO.load(Ordering::Relaxed) & 0xFF) << E2_MOTIVO_SHIFT
        | OTRAS.load(Ordering::Relaxed).min(0xFF) << E2_OTRAS_SHIFT
}

/// Esta encendido (o callado y pendiente de apagar)? Para `op_maquina`.
pub fn activo() -> bool {
    armado()
}
