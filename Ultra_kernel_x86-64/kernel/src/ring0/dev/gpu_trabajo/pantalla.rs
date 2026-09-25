//! **M5d P: LA 3060 PINTA LA PANTALLA ENTERA** -- un fotograma por llamada,
//! escrito por la 3060 DIRECTAMENTE en el framebuffer del GOP (en su VRAM).
//! Lo que se dibuja y como se comprueba esta en `bmo_gpu_ga10x::pantalla`;
//! aqui se toman las medidas del GOP, se mapea una vez, se toca el timbre y
//! se leen de vuelta las muestras.
//!
//! [carril]  ROJO      la 3060 escribe en la memoria que el monitor ESCANEA:
//!                     solo el framebuffer del GOP, mapeado por su medida
//! [consumo] NADA      corre por orden (`gpu pantalla`, `save mode`); un
//!                     fotograma son unos pocos ms de la 3060

use core::sync::atomic::{AtomicBool, Ordering};

use bmo_gpu_ga10x::pantalla as pa;

use super::{esperando, BLUR_ENTRADA, BLUR_EN_MARCHA, FRACTAL_ESPERA_US, IOMMU_NO_BLUR, IOMMU_NO_BLUR_PREPARAR, LIENZO_HECHO};
use crate::ring0::dev::gpu_prestamo::Bar0;

/// La pantalla no se puede pintar por la GPU: BAR1 no esta en modo fisico
/// (sin `gpu init`, o el GSP-RM la tiene), el framebuffer no cae dentro de
/// BAR1 y por debajo de lo nuestro, o no se pudo mapear.
pub const IOMMU_NO_PANTALLA: u32 = 81;

/// Mapeada en este arranque (la VRAM de las tablas no cambia).
static MAPEADA: AtomicBool = AtomicBool::new(false);

/// Bit de `arg`: cargar tambien el programa, el QMD y las ordenes.
pub const CARGAR: u64 = 1 << 56;

/// La pantalla del GOP en la VRAM, si se puede.
fn la_pantalla() -> Option<pa::Pantalla> {
    let d = crate::ring0::dev::framebuffer::display()?;
    // SAFETY: escrito una vez al arrancar (`info::init_from`), solo se lee.
    let (fb, fmt) = unsafe { (crate::info::FB_ADDR, crate::info::FB_PIXEL_FORMAT) };
    // UEFI: 0 = rojo en el byte 0 (RGB); 1 = azul en el byte 0 (BGR).
    pa::desde_gop(fb, crate::ring0::dev::gpu_libos::bar1(), d.stride, d.width, d.height, fmt == 0)
}

/// **M5d P.** `arg` = la ficha de S3 (bits 0..31), el fotograma (32..55) y
/// [`CARGAR`]. `Ok(pantalla::empaquetar(..))`.
pub fn pantalla(arg: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let (ficha, f) = (arg & 0xFFFF_FFFF, (arg >> 32) as u32 & 0xFF_FFFF);
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_BLUR);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_PANTALLA) };
    if !crate::ring0::dev::gpu_despertar::bar1_fisica() {
        return Err(IOMMU_NO_PANTALLA);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let r = pantalla_(bar0, ficha as u32, e, f, &p, arg & CARGAR != 0);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

fn pantalla_(bar0: u64, ficha: u32, e: u32, f: u32, p: &pa::Pantalla, cargar: bool) -> Result<u64, u32> {
    let mut r = Bar0(bar0);
    let primera = !MAPEADA.load(Ordering::Acquire);
    if primera {
        match pa::mapear(&mut r, p) {
            Some((n, bien)) if n == bien => {
                MAPEADA.store(true, Ordering::Release);
                crate::ring0::cabina::count("gpu", "M5d P: la pantalla del GOP MAPEADA para la 3060; KiB", p.bytes() / 1024);
            }
            _ => {
                crate::ring0::cabina::warn("gpu", "M5d P: la pantalla no se pudo mapear; VRAM", p.vram);
                return Err(IOMMU_NO_PANTALLA);
            }
        }
    }
    let m = pa::marco(f, p.ancho, p.alto);
    if !pa::preparar(&mut r, e, p, &m, cargar || primera) {
        crate::ring0::cabina::warn("gpu", "M5d P: el tramo no quedo preparado; no se toca el timbre", f as u64);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = pa::lanzar(&mut r, ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(e), Ordering::Release);
    }
    let (mut qmd, mut fin) = (0, 0);
    let mut us = 0;
    while lanzado && us < FRACTAL_ESPERA_US {
        (_, qmd, fin) = pa::mirar(&mut r);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if qmd == pa::PAGA_QMD && fin == pa::PAGA_FIN {
            break;
        }
        esperando(us);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    // Las muestras, leidas de donde mira el monitor.
    let cpu_desde = crate::ring0::task::scheduler::rdtsc();
    let buenos = crate::ring0::dev::framebuffer::display().map_or(0, |d| {
        (0..pa::MUESTRAS)
            .filter(|&k| {
                let (x, y) = pa::muestra(p, k);
                // SAFETY: (x, y) dentro de la pantalla del GOP (`muestra` no
                // pasa de ancho-1 x alto-1); lectura de 32 bits alineada.
                let v = unsafe { d.base.add(y as usize * d.stride as usize + x as usize).read_volatile() };
                v & 0x00FF_FFFF == pa::pixel(p, &m, x, y)
            })
            .count() as u32
    });
    let cpu_us = (crate::ring0::task::scheduler::rdtsc() - cpu_desde) / hz;
    let v = pa::empaquetar(buenos, qmd == pa::PAGA_QMD, fin == pa::PAGA_FIN, lanzado, us as u32, cpu_us as u32);
    if !pa::sano(v) {
        crate::ring0::cabina::warn("gpu", "M5d P: un fotograma no salio igual que la CPU; muestras buenas", buenos as u64);
    }
    Ok(v)
}
