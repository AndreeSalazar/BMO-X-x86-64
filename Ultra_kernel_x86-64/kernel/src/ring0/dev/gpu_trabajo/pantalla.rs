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
/// Bit de `arg` (C1, 25-09): comprobar solo [`pa::POCAS`] muestras que rotan,
/// no las 1024. Lo pide el escritorio en los fotogramas de paso.
pub const POCAS: u64 = 1 << 57;
/// Bit de `arg` (D2, 25-09): NO pintar; comparar UNA FILA de la pantalla
/// (`y` en los bits 0..15, el fotograma en 32..55) contra la cuenta de la CPU.
/// `Ok(malos | cpu_us << 32)`.
///
/// ** Una fila por llamada, y no la pantalla entera en una (metal 25-09
/// 13:35): un syscall corre con las interrupciones CERRADAS (`SFMASK`), y los
/// 2.073.600 pixeles con el fractal rehecho son ~2 s. En esos 2 s el reloj dio
/// 2 ticks y el bus USB llego 2300 ms tarde; ceder en cada linea no servia,
/// porque sin tick nadie despierta. Entre dos filas se vuelve a Ring 3 y las
/// interrupciones se abren: el peor hueco es UNA fila (~2 ms).
pub const FILA: u64 = 1 << 58;

/// La pantalla del GOP en la VRAM, si se puede. La usa tambien el volcado.
pub(super) fn la_pantalla() -> Option<pa::Pantalla> {
    let d = crate::ring0::dev::framebuffer::display()?;
    // SAFETY: escrito una vez al arrancar (`info::init_from`), solo se lee.
    let (fb, fmt) = unsafe { (crate::info::FB_ADDR, crate::info::FB_PIXEL_FORMAT) };
    // UEFI: 0 = rojo en el byte 0 (RGB); 1 = azul en el byte 0 (BGR).
    let p = pa::desde_gop(fb, crate::ring0::dev::gpu_libos::bar1(), d.stride, d.width, d.height, fmt == 0)?;
    // Las muestras se leen por el physmap: el framebuffer entero dentro de el.
    (fb + p.bytes() <= crate::ring0::mm::PHYSMAP_SIZE).then_some(p)
}

/// **M5d P.** `arg` = la ficha de S3 (bits 0..31), el fotograma (32..55),
/// [`CARGAR`] y [`POCAS`]. `Ok(pantalla::empaquetar(..))`.
pub fn pantalla(arg: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let (ficha, f) = (arg & 0xFFFF_FFFF, (arg >> 32) as u32 & 0xFF_FFFF);
    if arg & FILA != 0 {
        return fila((arg & 0xFFFF) as u32, f);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_BLUR);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_PANTALLA) };
    if !crate::ring0::dev::gpu_despertar::bar1_fisica() {
        return Err(IOMMU_NO_PANTALLA);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(e) || super::gr_ocupado() {
        return Err(IOMMU_NO_BLUR);
    }
    let r = pantalla_(bar0, ficha as u32, e, f, &p, arg & CARGAR != 0, arg & POCAS != 0);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

/// **La pantalla del GOP mapeada para la 3060**, una vez por arranque (la VRAM
/// de las tablas no cambia). La usa tambien el volcado, que la pone de
/// DESTINO. `Ok(true)` si se mapeo AHORA.
pub(super) fn asegurar_mapa(r: &mut Bar0, p: &pa::Pantalla) -> Result<bool, u32> {
    if MAPEADA.load(Ordering::Acquire) {
        return Ok(false);
    }
    match pa::mapear(r, p) {
        Some((n, bien)) if n == bien => {
            MAPEADA.store(true, Ordering::Release);
            crate::ring0::cabina::count("gpu", "M5d P: la pantalla del GOP MAPEADA para la 3060; KiB", p.bytes() / 1024);
            Ok(true)
        }
        _ => {
            crate::ring0::cabina::warn("gpu", "M5d P: la pantalla no se pudo mapear; VRAM", p.vram);
            Err(IOMMU_NO_PANTALLA)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pantalla_(bar0: u64, ficha: u32, e: u32, f: u32, p: &pa::Pantalla, cargar: bool, pocas: bool) -> Result<u64, u32> {
    let mut r = Bar0(bar0);
    let primera = asegurar_mapa(&mut r, p)?;
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
    //
    // ** POR EL PHYSMAP, no por `display().base` (metal 25-09: #PF "proteccion
    // leyendo desde el KERNEL" en `save mode`, justo tras `color`). `base` es
    // la direccion FISICA del GOP usada como puntero: vale con el CR3 del
    // arranque, pero esto corre en una syscall, con el CR3 del ESCRITORIO, y
    // ahi esa misma direccion (0xD000_0000 = `vmm::FRAMEBUFFER_VA_BASE`) es su
    // pagina de USUARIO de la pantalla: Ring 0 leyendola es lo que CR4.SMAP
    // corta. El physmap es del kernel en todos los CR3. Y va con `clflush`:
    // el espejo es WB y la 3060 escribe la VRAM sin pasar por esta cache.
    let cpu_desde = crate::ring0::task::scheduler::rdtsc();
    // SAFETY: escrito una vez al arrancar (`info::init_from`), solo se lee.
    let fb = crate::ring0::mm::phys_to_virt(unsafe { crate::info::FB_ADDR }) as *const u32;
    // C1: las 1024, o POCAS que rotan con el fotograma (en 64 pasan por todas).
    let (cuantas, esperadas) = if pocas { (pa::POCAS, pa::POCAS) } else { (pa::MUESTRAS, pa::MUESTRAS) };
    let buenos = crate::ring0::dev::framebuffer::display().map_or(0, |d| {
        (0..cuantas)
            .map(|i| if pocas { pa::muestra_de_paso(f, i) } else { i })
            .filter(|&k| {
                let (x, y) = pa::muestra(p, k);
                // SAFETY: (x, y) dentro de la pantalla del GOP (`muestra` no
                // pasa de ancho-1 x alto-1), por el physmap (`la_pantalla` ya
                // exigio el framebuffer dentro de BAR1, por debajo de 16 GiB);
                // lectura de 32 bits alineada.
                let v = unsafe {
                    let q = fb.add(y as usize * d.stride as usize + x as usize);
                    core::arch::x86_64::_mm_clflush(q as *const u8);
                    q.read_volatile()
                };
                v & 0x00FF_FFFF == pa::pixel(p, &m, x, y)
            })
            .count() as u32
    });
    let cpu_us = (crate::ring0::task::scheduler::rdtsc() - cpu_desde) / hz;
    let v = pa::empaquetar(buenos, qmd == pa::PAGA_QMD, fin == pa::PAGA_FIN, lanzado, us as u32, cpu_us as u32);
    if !pa::sano_con(v, esperadas) {
        crate::ring0::cabina::warn("gpu", "M5d P: un fotograma no salio igual que la CPU; muestras buenas", buenos as u64);
    }
    Ok(v)
}

/// **D2, una fila**: la `y` del fotograma `f`, pixel a pixel, contra la CPU.
/// Solo LEE: ni timbre ni tramo. `Ok(malos | cpu_us << 32)`.
fn fila(y: u32, f: u32) -> Result<u64, u32> {
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_PANTALLA) };
    if y >= p.alto {
        return Err(IOMMU_NO_PANTALLA);
    }
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let m = pa::marco(f, p.ancho, p.alto);
    // SAFETY: escrito una vez al arrancar (`info::init_from`), solo se lee.
    let fb = crate::ring0::mm::phys_to_virt(unsafe { crate::info::FB_ADDR }) as *const u32;
    let malos = (0..p.ancho)
        .filter(|&x| {
            // SAFETY: (x, y) dentro de la pantalla (y < alto comprobado), por
            // el physmap (`la_pantalla` exigio el framebuffer dentro); 32 bits
            // alineados. `clflush` una vez por linea de cache (16 pixeles).
            let v = unsafe {
                let q = fb.add(y as usize * p.pitch as usize + x as usize);
                if x % 16 == 0 {
                    core::arch::x86_64::_mm_clflush(q as *const u8);
                }
                q.read_volatile()
            };
            v & 0x00FF_FFFF != pa::pixel(&p, &m, x, y)
        })
        .count() as u64;
    if malos != 0 {
        crate::ring0::cabina::warn("gpu", "D2: una fila de la pantalla no salio igual que la CPU; pixeles malos", malos);
    }
    let us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
    Ok(malos | us.min(u32::MAX as u64) << 32)
}
