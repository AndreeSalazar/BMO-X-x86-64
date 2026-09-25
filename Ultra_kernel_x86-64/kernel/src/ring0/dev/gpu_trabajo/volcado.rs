//! **EL VOLCADO POR LA 3060** (compositor por GPU, paso 1; 2026-09-25) -- el
//! motor de COPIA lleva el lienzo del escritorio (su RAM) a la pantalla del
//! GOP (VRAM) en una orden. Lo que se copia y como se comprueba esta en
//! `bmo_gpu_ga10x::volcado`; aqui se presta el lienzo, se toca el timbre y se
//! leen las muestras.
//!
//! [carril]  ROJO      la 3060 LEE memoria de un proceso y escribe en la que
//!                     el monitor escanea: el prestamo es SOLO LECTURA, de un
//!                     bloque suyo que el kernel reconoce (`fisica_de`), y dura
//!                     lo que dura UNA copia
//! [consumo] NADA      corre por orden (`gpu volcado`, `save mode`); una copia
//!                     de la pantalla entera son unos pocos ms de la 3060
//!
//! # *** Por que el prestamo dura UNA copia
//!
//! El lienzo es de un PROCESO (el escritorio). Si el prestamo sobreviviera al
//! proceso, sus marcos pasarian a otro y la 3060 los seguiria viendo: es el
//! R-DMA-3 de la casa (un marco reasignado con DMA dentro). Prestar 2025
//! paginas y devolverlas cuesta ~1 ms; para una verificacion es barato. Para
//! volcar CADA fotograma (el paso 1b) el prestamo tendra que vivir lo que el
//! proceso, y morir con el en el desmontaje -- eso es otro commit y otra
//! verificacion.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bmo_gpu_ga10x::pantalla as pa;
use bmo_gpu_ga10x::volcado as vl;

use super::pantalla::{asegurar_mapa, la_pantalla};
use super::{COPIA_ESPERA_US, PAGINA};
use crate::ring0::dev::gpu_prestamo::Bar0;
use crate::ring0::plat::iommu as io;

/// El volcado no se puede: sin la copia de L1d3 (su timbre), sin la pantalla
/// del GOP en modo fisico, un lienzo que no es un bloque del que llama o no
/// mide la pantalla, o uno ya en marcha.
pub const IOMMU_NO_VOLCADO: u32 = 83;

/// Las tablas del lienzo en la GPU: una vez por arranque (el mapa no depende
/// de donde este el lienzo: eso lo pone la IOMMU en cada prestamo).
static MAPEADO: AtomicBool = AtomicBool::new(false);
static EN_MARCHA: AtomicBool = AtomicBool::new(false);
/// La siguiente entrada del GPFIFO de copia.
static ENTRADA: AtomicU32 = AtomicU32::new(vl::PRIMERA_ENTRADA);

/// **Un volcado del escritorio por la 3060.** `lienzo` = la VA del lienzo del
/// escritorio (el bloque del que llama, de la medida de la pantalla).
/// `Ok(volcado::empaquetar(..))`.
pub fn volcado(lienzo: u64) -> Result<u64, u32> {
    let bar0 = crate::ring0::dev::gpu::bar0();
    let Some(ficha) = crate::ring0::dev::gpu_libos::timbre_de_copia() else {
        return Err(IOMMU_NO_VOLCADO);
    };
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_VOLCADO) };
    if bar0 == 0 || !vl::cabe(&p) || !crate::ring0::dev::gpu_despertar::bar1_fisica() {
        return Err(IOMMU_NO_VOLCADO);
    }
    let pid = crate::ring0::task::scheduler::current_pid();
    let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, lienzo, p.bytes()) else {
        crate::ring0::cabina::warn("gpu", "volcado: el lienzo no es un bloque de quien lo pide", lienzo);
        return Err(IOMMU_NO_VOLCADO);
    };
    if fisica % PAGINA != 0 || fisica + p.bytes() > crate::ring0::mm::PHYSMAP_SIZE {
        return Err(IOMMU_NO_VOLCADO);
    }
    if EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_VOLCADO);
    }
    let r = volcado_(bar0, ficha, fisica, &p);
    EN_MARCHA.store(false, Ordering::Release);
    r
}

fn volcado_(bar0: u64, ficha: u32, fisica: u64, p: &pa::Pantalla) -> Result<u64, u32> {
    let mut r = Bar0(bar0);
    asegurar_mapa(&mut r, p)?;
    if !MAPEADO.load(Ordering::Acquire) {
        match vl::mapear(&mut r, p) {
            Some((n, bien)) if n == bien => {
                MAPEADO.store(true, Ordering::Release);
                crate::ring0::cabina::count("gpu", "volcado: el lienzo MAPEADO para la 3060 (PTE de sistema); paginas", vl::paginas(p));
            }
            _ => {
                crate::ring0::cabina::warn("gpu", "volcado: las tablas del lienzo no se escribieron o no se releyeron", 0);
                return Err(IOMMU_NO_VOLCADO);
            }
        }
    }
    let paginas = vl::paginas(p);
    if io::prestar_gpu(vl::IOVA, fisica, paginas, false).is_err() {
        crate::ring0::cabina::warn("gpu", "volcado: el lienzo no se pudo prestar a la 3060; fisica", fisica);
        return Err(IOMMU_NO_VOLCADO);
    }
    let r_ = copiar(&mut r, ficha, fisica, p);
    // Devuelto SIEMPRE, salga como salga la copia.
    if io::devolver_gpu(vl::IOVA, paginas).is_err() {
        crate::ring0::cabina::warn("gpu", "volcado: el lienzo NO se pudo devolver (la invalidacion no contesto)", paginas);
    }
    r_
}

/// El pixel `(x, y)` en `base` (physmap), con el paso de la pantalla.
fn en(base: u64, p: &pa::Pantalla, x: u32, y: u32) -> *mut u32 {
    (base + 4 * (y as u64 * p.pitch as u64 + x as u64)) as *mut u32
}

fn copiar(r: &mut Bar0, ficha: u32, fisica: u64, p: &pa::Pantalla) -> Result<u64, u32> {
    use core::arch::x86_64::_mm_clflush;
    let lienzo = crate::ring0::mm::phys_to_virt(fisica);
    // SAFETY: escrito una vez al arrancar (`info::init_from`), solo se lee.
    let fb = crate::ring0::mm::phys_to_virt(unsafe { crate::info::FB_ADDR });
    // ** Las muestras con el color CONTRARIO al del lienzo, por el physmap y
    // empujadas fuera de la cache (el espejo es WB; sin `clflush` la linea
    // podria bajar a la VRAM DESPUES de la copia y taparla).
    for k in 0..pa::MUESTRAS {
        let (x, y) = pa::muestra(p, k);
        // SAFETY: (x, y) dentro de la pantalla (`muestra`), en el lienzo (un
        // bloque del proceso de `stride x alto`, `fisica_de`) y en el
        // framebuffer (`la_pantalla` lo exige dentro del physmap).
        unsafe {
            let q = en(fb, p, x, y);
            q.write_volatile(vl::contrario(en(lienzo, p, x, y).read_volatile()));
            _mm_clflush(q as *const u8);
        }
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let e = ENTRADA.load(Ordering::Acquire);
    if !vl::preparar(r, e, p) {
        crate::ring0::cabina::warn("gpu", "volcado: el canal de copia no quedo preparado; no se toca el timbre", e as u64);
        return Err(IOMMU_NO_VOLCADO);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = vl::lanzar(r, ficha, e);
    if lanzado {
        ENTRADA.store(vl::siguiente(e), Ordering::Release);
    }
    let (mut semaforo, mut us) = (0, 0);
    while lanzado && us < COPIA_ESPERA_US {
        semaforo = vl::mirar(r).1;
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if semaforo == vl::PAGA {
            break;
        }
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let buenas = (0..pa::MUESTRAS)
        .filter(|&k| {
            let (x, y) = pa::muestra(p, k);
            // SAFETY: como arriba; `clflush` antes de leer lo que escribio la 3060.
            unsafe {
                let q = en(fb, p, x, y);
                _mm_clflush(q as *const u8);
                core::sync::atomic::fence(Ordering::SeqCst);
                q.read_volatile() & 0x00FF_FFFF == en(lienzo, p, x, y).read_volatile() & 0x00FF_FFFF
            }
        })
        .count() as u32;
    let v = vl::empaquetar(buenas, semaforo == vl::PAGA, lanzado, us as u32);
    if vl::sano(v) {
        crate::ring0::cabina::count("gpu", "volcado: LA 3060 LLEVO EL ESCRITORIO A LA PANTALLA; us", us);
    } else {
        crate::ring0::cabina::warn("gpu", "volcado: la copia no salio entera; muestras buenas", buenas as u64);
    }
    Ok(v)
}
