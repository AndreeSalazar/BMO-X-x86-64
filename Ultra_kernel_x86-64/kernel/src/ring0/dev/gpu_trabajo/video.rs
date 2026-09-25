//! **M6 V0: EL VIDEO POR LA 3060** -- un fotograma NV12 de un bloque del
//! escritorio, convertido a RGB y agrandado por la 3060 DIRECTAMENTE en el
//! framebuffer del GOP. Lo que se dibuja y como se comprueba esta en
//! `bmo_gpu_ga10x::video`; aqui se presta el fotograma, se toca el timbre y
//! se leen las muestras.
//!
//! [carril]  ROJO      la 3060 LEE memoria de un proceso (SOLO LECTURA, un
//!                     bloque suyo que el kernel reconoce, `fisica_de`) y
//!                     escribe en la que el monitor escanea
//! [consumo] NADA      corre por orden (`gpu video`); un fotograma son unos
//!                     cientos de us de la 3060
//!
//! # *** El prestamo dura UNA llamada
//!
//! Como el volcado sin armar: se presta al entrar y se devuelve antes de
//! volver a Ring 3, salga como salga. Un fotograma NV12 de 640x360 son 85
//! paginas (~50 us de IOMMU); a cambio, el bloque nunca queda visto por la
//! 3060 si el proceso muere o lo suelta (R-DMA-3). Cuando el video sea un
//! tubo continuo (NVDEC o la antena) el prestamo vivira lo que el proceso,
//! como el volcador armado.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use bmo_gpu_ga10x::pantalla as pa;
use bmo_gpu_ga10x::video as vi;

use super::pantalla::{asegurar_mapa, la_pantalla};
use super::{esperando, BLUR_ENTRADA, BLUR_EN_MARCHA, FRACTAL_ESPERA_US, LIENZO_HECHO, PAGINA};
use crate::ring0::dev::gpu_prestamo::Bar0;
use crate::ring0::plat::iommu as io;

/// El video no se puede: sin el lienzo de M5d (el canal de GR y su ficha),
/// sin la pantalla del GOP en modo fisico, un formato que no vale o que no
/// cabe en la pantalla, un fotograma que no es un bloque de quien lo pide, o
/// uno ya en marcha.
pub const IOMMU_NO_VIDEO: u32 = 85;

/// Bit de `arg` de [`video`]: cargar el programa, el QMD y las ordenes.
pub const CARGAR: u64 = 1 << 63;

/// El formato de la tanda: `ancho | alto << 16 | ficha << 32`; 0 = ninguno.
static FORMATO: AtomicU64 = AtomicU64::new(0);
/// Las tablas del fotograma en la GPU: una vez por arranque.
static MAPEADO: AtomicBool = AtomicBool::new(false);

fn formato() -> Option<(vi::Formato, u32)> {
    let v = FORMATO.load(Ordering::Acquire);
    let f = vi::Formato { ancho: (v & 0xFFFF) as u32, alto: (v >> 16 & 0xFFFF) as u32 };
    (v != 0 && f.valido()).then_some((f, (v >> 32) as u32))
}

/// **El formato de la tanda.** `arg` = ancho (0..15), alto (16..31) y la
/// ficha de S3 (32..63). `Ok(escala | x0 << 8 | y0 << 32)`: como cae en la
/// pantalla.
pub fn video_formato(arg: u64) -> Result<u64, u32> {
    let f = vi::Formato { ancho: (arg & 0xFFFF) as u32, alto: (arg >> 16 & 0xFFFF) as u32 };
    let ficha = arg >> 32;
    if !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_VIDEO);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_VIDEO) };
    let Some(e) = vi::encaje(&f, &p) else { return Err(IOMMU_NO_VIDEO) };
    FORMATO.store(arg, Ordering::Release);
    Ok(e.escala as u64 | (e.x0 as u64) << 8 | (e.y0 as u64) << 32)
}

/// **Un fotograma.** `arg` = la VA del bloque con el fotograma NV12 (del que
/// llama) y [`CARGAR`]. `Ok(video::empaquetar(..))`.
pub fn video(arg: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let bar0 = crate::ring0::dev::gpu::bar0();
    let Some((f, ficha)) = formato() else { return Err(IOMMU_NO_VIDEO) };
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_VIDEO) };
    let Some(e) = vi::encaje(&f, &p) else { return Err(IOMMU_NO_VIDEO) };
    if bar0 == 0 || !crate::ring0::dev::gpu_despertar::bar1_fisica() {
        return Err(IOMMU_NO_VIDEO);
    }
    let pid = crate::ring0::task::scheduler::current_pid();
    let va = arg & !CARGAR;
    let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, va, f.bytes()) else {
        crate::ring0::cabina::warn("gpu", "video: el fotograma no es un bloque de quien lo pide", va);
        return Err(IOMMU_NO_VIDEO);
    };
    if fisica % PAGINA != 0 || fisica + f.bytes() > crate::ring0::mm::PHYSMAP_SIZE {
        return Err(IOMMU_NO_VIDEO);
    }
    let en = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(en) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_VIDEO);
    }
    let r = video_(bar0, ficha, en, fisica, &f, &e, &p, arg & CARGAR != 0);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

fn asegurar_mapas(r: &mut Bar0, p: &pa::Pantalla) -> Result<bool, u32> {
    let pantalla = asegurar_mapa(r, p).map_err(|_| IOMMU_NO_VIDEO)?;
    if MAPEADO.load(Ordering::Acquire) {
        return Ok(pantalla);
    }
    match vi::mapear(r) {
        Some((n, bien)) if n == bien => {
            MAPEADO.store(true, Ordering::Release);
            crate::ring0::cabina::count("gpu", "video: el fotograma MAPEADO para la 3060 (PTE de sistema); paginas", vi::MAX_BYTES / PAGINA);
            Ok(true)
        }
        _ => {
            crate::ring0::cabina::warn("gpu", "video: las tablas del fotograma no se escribieron o no se releyeron", 0);
            Err(IOMMU_NO_VIDEO)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn video_(bar0: u64, ficha: u32, en: u32, fisica: u64, f: &vi::Formato, e: &vi::Encaje, p: &pa::Pantalla, cargar: bool) -> Result<u64, u32> {
    let mut r = Bar0(bar0);
    let nuevo = asegurar_mapas(&mut r, p)?;
    let paginas = f.paginas();
    if io::prestar_gpu(vi::IOVA, fisica, paginas, false).is_err() {
        crate::ring0::cabina::warn("gpu", "video: el fotograma no se pudo prestar a la 3060; fisica", fisica);
        return Err(IOMMU_NO_VIDEO);
    }
    let parametros = vi::parametros(f, e, p);
    let preparado = vi::preparar(&mut r, en, f, &parametros, cargar || nuevo);
    let (mut lanzado, mut qmd, mut fin, mut us) = (false, 0, 0, 0);
    if preparado {
        core::sync::atomic::fence(Ordering::SeqCst);
        let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
        let desde = crate::ring0::task::scheduler::rdtsc();
        lanzado = vi::lanzar(&mut r, ficha, en);
        if lanzado {
            BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(en), Ordering::Release);
        }
        while lanzado && us < FRACTAL_ESPERA_US {
            (_, qmd, fin) = vi::mirar(&mut r);
            us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
            if qmd == vi::PAGA_QMD && fin == vi::PAGA_FIN {
                break;
            }
            esperando(us);
        }
    } else {
        crate::ring0::cabina::warn("gpu", "video: el tramo no quedo preparado; no se toca el timbre", 0);
    }
    // Devuelto SIEMPRE, salga como salga el fotograma.
    if io::devolver_gpu(vi::IOVA, paginas).is_err() {
        crate::ring0::cabina::warn("gpu", "video: el fotograma NO se pudo devolver (la invalidacion no contesto)", paginas);
    }
    if !preparado {
        return Err(IOMMU_NO_VIDEO);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    // Las muestras: el NV12 del bloque y la pantalla, los dos por el physmap
    // (la pantalla con `clflush`: su espejo es WB y la 3060 escribe la VRAM
    // sin pasar por esta cache; ver `pantalla`).
    let cpu_desde = crate::ring0::task::scheduler::rdtsc();
    // SAFETY: `fisica_de` dio `f.bytes()` seguidos de un bloque del proceso,
    // dentro del physmap (comprobado arriba); solo se leen.
    let nv12 = unsafe { core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(fisica) as *const u8, f.bytes() as usize) };
    // SAFETY: escrito una vez al arrancar (`info::init_from`), solo se lee.
    let fb = crate::ring0::mm::phys_to_virt(unsafe { crate::info::FB_ADDR }) as *const u32;
    let buenos = (0..vi::MUESTRAS)
        .filter(|&k| {
            let (dx, dy) = vi::muestra(f, e, k);
            let (x, y) = (e.x0 + dx, e.y0 + dy);
            // SAFETY: (x, y) dentro del rectangulo del video, que `encaje`
            // deja dentro de la pantalla; el framebuffer entero esta en el
            // physmap (`la_pantalla`); lectura de 32 bits alineada.
            let v = unsafe {
                let q = fb.add(y as usize * p.pitch as usize + x as usize);
                core::arch::x86_64::_mm_clflush(q as *const u8);
                q.read_volatile()
            };
            v & 0x00FF_FFFF == vi::pixel(nv12, f, e, p.rgb, dx, dy)
        })
        .count() as u32;
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let cpu_us = (crate::ring0::task::scheduler::rdtsc() - cpu_desde) / hz;
    let v = vi::empaquetar(buenos, qmd == vi::PAGA_QMD, fin == vi::PAGA_FIN, lanzado, us as u32, cpu_us as u32);
    if !vi::sano(v) {
        crate::ring0::cabina::warn("gpu", "video: un fotograma no salio igual que la CPU; muestras buenas", buenos as u64);
    }
    Ok(v)
}
