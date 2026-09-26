//! **D2: UNA IMAGEN DE 32 BITS POR LA 3060** -- un fotograma `0x00RRGGBB`
//! (el de DOOM: 320 x 200) de un bloque del escritorio, agrandado por la
//! 3060 DIRECTAMENTE en el framebuffer del GOP. Lo que se dibuja y como se
//! comprueba esta en `bmo_gpu_ga10x::imagen`; aqui se presta el fotograma, se
//! toca el timbre y se leen las muestras. Es `video` con otro formato y otro
//! programa, a proposito: el mismo prestamo, el mismo mapa, la misma espera.
//!
//! [carril]  ROJO      la 3060 LEE memoria de un proceso (SOLO LECTURA, un
//!                     bloque suyo que el kernel reconoce, `fisica_de`) y
//!                     escribe en la que el monitor escanea
//! [consumo] NADA      corre por orden (`gpu imagen`); un fotograma de DOOM son
//!                     63 paginas prestadas y unos cientos de us de la 3060
//!
//! # El prestamo dura UNA llamada
//!
//! Como `video`: se presta al entrar y se devuelve antes de volver a Ring 3,
//! salga como salga. El bloque nunca queda visto por la 3060 si el proceso
//! muere o lo suelta (R-DMA-3). Cuando DOOM mande sus fotogramas en vivo (D2c)
//! el prestamo podra vivir lo que el proceso, como el volcador armado.

use core::sync::atomic::{AtomicU64, Ordering};

use bmo_gpu_ga10x::imagen as im;
use bmo_gpu_ga10x::pantalla as pa;

use super::pantalla::la_pantalla;
use super::video::asegurar_mapas;
use super::{esperando, BLUR_ENTRADA, BLUR_EN_MARCHA, FRACTAL_ESPERA_US, LIENZO_HECHO, PAGINA};
use crate::ring0::dev::gpu_prestamo::Bar0;
use crate::ring0::plat::iommu as io;

/// La imagen no se puede: sin el lienzo de M5d (el canal de GR y su ficha),
/// sin la pantalla del GOP en modo fisico, un formato que no vale o que no
/// cabe en la pantalla, un fotograma que no es un bloque de quien lo pide, o
/// uno ya en marcha.
pub const IOMMU_NO_IMAGEN: u32 = 88;

/// Bit de `arg` de [`imagen`]: cargar el programa, el QMD y las ordenes.
pub const CARGAR: u64 = 1 << 63;
/// Bit de `arg` de [`imagen`] (D2c): la VA es de un PRESTAMO que el que llama
/// TOMO (el fotograma que DOOM le ofrecio), no de un bloque suyo.
pub const PRESTADO: u64 = 1 << 62;

/// El formato de la tanda: `ancho | alto << 16 | ficha << 32`; 0 = ninguno.
static FORMATO: AtomicU64 = AtomicU64::new(0);

fn formato() -> Option<(im::Formato, u32)> {
    let v = FORMATO.load(Ordering::Acquire);
    let f = im::Formato { ancho: (v & 0xFFFF) as u32, alto: (v >> 16 & 0xFFFF) as u32 };
    (v != 0 && f.valido()).then_some((f, (v >> 32) as u32))
}

/// **El formato de la tanda.** `arg` = ancho (0..15), alto (16..31) y la
/// ficha de S3 (32..63). `Ok(escala | x0 << 8 | y0 << 32)`: como cae en la
/// pantalla.
pub fn imagen_formato(arg: u64) -> Result<u64, u32> {
    let f = im::Formato { ancho: (arg & 0xFFFF) as u32, alto: (arg >> 16 & 0xFFFF) as u32 };
    let ficha = arg >> 32;
    if !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_IMAGEN);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_IMAGEN) };
    let Some(e) = im::encaje(&f, &p) else { return Err(IOMMU_NO_IMAGEN) };
    FORMATO.store(arg, Ordering::Release);
    Ok(e.escala as u64 | (e.x0 as u64) << 8 | (e.y0 as u64) << 32)
}

/// **Un fotograma.** `arg` = la VA del bloque con la imagen (del que llama)
/// y [`CARGAR`]. `Ok(imagen::empaquetar(..))`.
pub fn imagen(arg: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let bar0 = crate::ring0::dev::gpu::bar0();
    let Some((f, ficha)) = formato() else { return Err(IOMMU_NO_IMAGEN) };
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_IMAGEN) };
    let Some(e) = im::encaje(&f, &p) else { return Err(IOMMU_NO_IMAGEN) };
    if bar0 == 0 || !crate::ring0::dev::gpu_despertar::bar1_fisica() {
        return Err(IOMMU_NO_IMAGEN);
    }
    let pid = crate::ring0::task::scheduler::current_pid();
    let va = arg & !(CARGAR | PRESTADO);
    // ** De donde sale el fotograma: un bloque propio (la carta, un fichero) o
    // un PRESTAMO tomado (D2c: el de DOOM, que empieza donde cayo su `malloc`).
    // En los dos, la fisica de una PAGINA y cuanto anda el fotograma dentro.
    let (fisica, dentro) = if arg & PRESTADO != 0 {
        let Some(t) = crate::ring0::obj::loan::fisica_tomada(pid, va, f.bytes()) else {
            crate::ring0::cabina::warn("gpu", "imagen: el fotograma no es un prestamo tomado por quien lo pide, o sus marcos no van seguidos", va);
            return Err(IOMMU_NO_IMAGEN);
        };
        t
    } else {
        let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, va, f.bytes()) else {
            crate::ring0::cabina::warn("gpu", "imagen: el fotograma no es un bloque de quien lo pide", va);
            return Err(IOMMU_NO_IMAGEN);
        };
        if fisica % PAGINA != 0 {
            return Err(IOMMU_NO_IMAGEN);
        }
        (fisica, 0)
    };
    let Some(paginas) = im::paginas(&f, dentro) else { return Err(IOMMU_NO_IMAGEN) };
    if fisica + paginas * PAGINA > crate::ring0::mm::PHYSMAP_SIZE {
        return Err(IOMMU_NO_IMAGEN);
    }
    let en = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(en) || super::gr_ocupado() {
        return Err(IOMMU_NO_IMAGEN);
    }
    let r = imagen_(bar0, ficha, en, (fisica, dentro, paginas), &f, &e, &p, arg & CARGAR != 0);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

#[allow(clippy::too_many_arguments)]
/// `origen` = (la fisica de la primera pagina, donde empieza el fotograma
/// dentro de ella, cuantas paginas se prestan).
fn imagen_(bar0: u64, ficha: u32, en: u32, origen: (u64, u64, u64), f: &im::Formato, e: &im::Encaje, p: &pa::Pantalla, cargar: bool) -> Result<u64, u32> {
    let (fisica, dentro, paginas) = origen;
    let mut r = Bar0(bar0);
    let nuevo = asegurar_mapas(&mut r, p).map_err(|_| IOMMU_NO_IMAGEN)?;
    if io::prestar_gpu(im::IOVA, fisica, paginas, false).is_err() {
        crate::ring0::cabina::warn("gpu", "imagen: el fotograma no se pudo prestar a la 3060; fisica", fisica);
        return Err(IOMMU_NO_IMAGEN);
    }
    let parametros = im::parametros_desde(f, e, p, dentro);
    let preparado = im::preparar(&mut r, en, f, &parametros, cargar || nuevo);
    let (mut lanzado, mut qmd, mut fin, mut us) = (false, 0, 0, 0);
    if preparado {
        core::sync::atomic::fence(Ordering::SeqCst);
        let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
        let desde = crate::ring0::task::scheduler::rdtsc();
        lanzado = im::lanzar(&mut r, ficha, en);
        if lanzado {
            BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(en), Ordering::Release);
        }
        while lanzado && us < FRACTAL_ESPERA_US {
            (_, qmd, fin) = im::mirar(&mut r);
            us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
            if qmd == im::PAGA_QMD && fin == im::PAGA_FIN {
                break;
            }
            esperando(us);
        }
    } else {
        crate::ring0::cabina::warn("gpu", "imagen: el tramo no quedo preparado; no se toca el timbre", 0);
    }
    // Devuelto SIEMPRE, salga como salga el fotograma.
    if io::devolver_gpu(im::IOVA, paginas).is_err() {
        crate::ring0::cabina::warn("gpu", "imagen: el fotograma NO se pudo devolver (la invalidacion no contesto)", paginas);
    }
    if !preparado {
        return Err(IOMMU_NO_IMAGEN);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    // Las muestras: la imagen del bloque y la pantalla, las dos por el
    // physmap (la pantalla con `clflush`, como `video`).
    let cpu_desde = crate::ring0::task::scheduler::rdtsc();
    // SAFETY: `fisica_de` o `fisica_tomada` dieron `paginas` marcos SEGUIDOS
    // desde `fisica` (un bloque del proceso, o un prestamo que tomo), dentro
    // del physmap (comprobado arriba); `dentro` es multiplo de 4 y el
    // fotograma cabe en ellas (`im::paginas`); solo se leen.
    let origen = unsafe { core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(fisica + dentro) as *const u32, (f.ancho * f.alto) as usize) };
    // SAFETY: escrito una vez al arrancar (`info::init_from`), solo se lee.
    let fb = crate::ring0::mm::phys_to_virt(unsafe { crate::info::FB_ADDR }) as *const u32;
    let buenos = (0..im::MUESTRAS)
        .filter(|&k| {
            let (dx, dy) = im::muestra(f, e, k);
            let (x, y) = (e.x0 + dx, e.y0 + dy);
            // SAFETY: (x, y) dentro del rectangulo de la imagen, que
            // `encaje` deja dentro de la pantalla; el framebuffer entero
            // esta en el physmap (`la_pantalla`); lectura de 32 bits alineada.
            let v = unsafe {
                let q = fb.add(y as usize * p.pitch as usize + x as usize);
                core::arch::x86_64::_mm_clflush(q as *const u8);
                q.read_volatile()
            };
            v & 0x00FF_FFFF == im::pixel(origen, f, e, p.rgb, dx, dy)
        })
        .count() as u32;
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let cpu_us = (crate::ring0::task::scheduler::rdtsc() - cpu_desde) / hz;
    let v = im::empaquetar(buenos, qmd == im::PAGA_QMD, fin == im::PAGA_FIN, lanzado, us as u32, cpu_us as u32);
    if !im::sano(v) {
        crate::ring0::cabina::warn("gpu", "imagen: un fotograma no salio igual que la CPU; muestras buenas", buenos as u64);
    }
    Ok(v)
}
