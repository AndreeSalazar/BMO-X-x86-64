//! **X5: EL CUBO POR LA 3060, SIN WINDOWS** -- un fotograma del cubo del
//! estudio D3D dibujado por el pipeline 3D de la 3060 en una ventana de
//! 1280x720 de la pantalla (el framebuffer del GOP), y leido de vuelta para
//! que el escritorio saque su huella. Que se dibuja: `bmo_cubo::tanda` (las
//! cuentas del juez); como: `bmo_gpu_ga10x::cubo` (programas y ordenes).
//!
//! [carril]  ROJO      la 3060 escribe en la memoria que el monitor ESCANEA:
//!                     solo la ventana, dentro del mapa de `pantalla`
//! [consumo] NADA      corre por orden (`gpu cubo 3060`): un dibujo de unos
//!                     cientos de us, y la lectura de vuelta a pedazos
//!
//! Dos subordenes en `IOMMU_OP_GPU_CUBO`:
//!
//! ```text
//!    DIBUJAR  arg = la ficha de S3 (0..31) y el fotograma (32..40)
//!             Ok(cubo::empaquetar(..)): us de la 3060, triangulos, escalera
//!    LEER     arg = CUBO_LEER | k: los pixeles 2k y 2k+1 de la ventana, fila
//!             a fila, como 0x00RRGGBB (Ok(p0 | p1 << 32)); solo tras DIBUJAR
//!    VERRANO  arg = CUBO_VERRANO | la VA de un paquete del escritorio
//!             (`tuberia::Paquete`: sus dos programas, tomados del BSF, y sus
//!             vertices). La tuberia FIJA de VERRANO V0: el kernel sube el
//!             codigo tal cual, no traduce nada. Ok como DIBUJAR
//! ```
//!
//! La lectura va de dos en dos y no la ventana entera en una llamada por lo
//! mismo que D2 (`pantalla::FILA`): un syscall corre con las interrupciones
//! cerradas, y 921.600 pixeles leidos de la VRAM son demasiado tiempo sin
//! reloj.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bmo_gpu_ga10x::cubo as cu;

use super::pantalla::{asegurar_mapa, la_pantalla, IOMMU_NO_PANTALLA};
use super::{esperando, BLUR_ENTRADA, BLUR_EN_MARCHA, DIAG_3D, FRACTAL_ESPERA_US, IOMMU_NO_BLUR, IOMMU_NO_BLUR_PREPARAR, LIENZO_HECHO};
use crate::ring0::dev::gpu_prestamo::Bar0;

/// El bit de `arg` que pide LEER en vez de DIBUJAR.
pub const CUBO_LEER: u64 = 1 << 63;

/// El bit de `arg` que pide dibujar un paquete de VERRANO V0.
pub const CUBO_VERRANO: u64 = 1 << 62;

/// Ya se dibujo en este arranque (LEER antes no tiene que leer).
static DIBUJADO: AtomicBool = AtomicBool::new(false);
/// Cuantos fotogramas del cubo dibujo la 3060 (para el log).
static VECES: AtomicU32 = AtomicU32::new(0);

/// **`IOMMU_OP_GPU_CUBO`**.
pub fn cubo(arg: u64) -> Result<u64, u32> {
    if arg & CUBO_LEER != 0 {
        return leer(arg & 0xF_FFFF);
    }
    if arg & CUBO_VERRANO != 0 {
        return verrano(arg & 0xFFFF_FFFF_FFFF);
    }
    let (ficha, f) = (arg & 0xFFFF_FFFF, (arg >> 32) as u32 & 0x1FF);
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) || f >= 360 {
        return Err(IOMMU_NO_BLUR);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_PANTALLA) };
    let Some(v) = cu::ventana(&p) else { return Err(IOMMU_NO_PANTALLA) };
    if !crate::ring0::dev::gpu_despertar::bar1_fisica() {
        return Err(IOMMU_NO_PANTALLA);
    }
    // La tanda: las cuentas del juez, en bits para el driver.
    let Some(t) = bmo_cubo::tanda::de_fotograma(f, cu::ANCHO, cu::ALTO) else { return Err(IOMMU_NO_BLUR_PREPARAR) };
    let mut tris = [cu::Triangulo { clip: [[0; 4]; 3], color: [0; 4] }; cu::CABEN];
    for (d, s) in tris.iter_mut().zip(t.tris()) {
        *d = cu::Triangulo { clip: s.clip.map(|v| v.map(f32::to_bits)), color: s.color.map(f32::to_bits) };
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bmo_gpu_ga10x::blur::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    // Lo que el volcado del escritorio tenga en vuelo, antes: si no, su copia
    // podria caer ENCIMA del cubo entre el dibujo y la lectura.
    let tris = &tris[..t.n];
    let r = super::volcado::quieto().and_then(|()| dibujar(bar0, ficha as u32, e, &p, t.n as u32, |r| cu::preparar(r, e, &v, tris)));
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

/// **VERRANO V0**: el paquete del escritorio (sus dos programas y sus
/// vertices) por la tuberia fija.
fn verrano(va: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::tuberia as tu;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) {
        return Err(IOMMU_NO_BLUR);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_PANTALLA) };
    let Some(v) = cu::ventana(&p) else { return Err(IOMMU_NO_PANTALLA) };
    if !crate::ring0::dev::gpu_despertar::bar1_fisica() {
        return Err(IOMMU_NO_PANTALLA);
    }
    // El paquete: un bloque de quien lo pide, entero, dentro del physmap.
    let pid = crate::ring0::task::scheduler::current_pid();
    let dentro = |fisica: u64, bytes: u64| fisica.checked_add(bytes).is_some_and(|fin| fin <= crate::ring0::mm::PHYSMAP_SIZE);
    let Some(f) = crate::ring0::obj::memory::fisica_de(pid, va, tu::CABECERA as u64).filter(|&f| dentro(f, tu::CABECERA as u64)) else {
        crate::ring0::cabina::warn("gpu", "VERRANO: el paquete no es un bloque de quien lo pide", va);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    };
    // SAFETY: `fisica_de` dio CABECERA bytes de un bloque del proceso, dentro
    // del physmap (comprobado); solo se leen.
    let cabecera = unsafe { core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(f) as *const u8, tu::CABECERA) };
    let Some(total) = tu::medida(cabecera) else { return Err(IOMMU_NO_BLUR_PREPARAR) };
    let Some(f) = crate::ring0::obj::memory::fisica_de(pid, va, total as u64).filter(|&f| dentro(f, total as u64)) else {
        return Err(IOMMU_NO_BLUR_PREPARAR);
    };
    // SAFETY: como arriba, con `total` bytes; el escritorio esta dentro de
    // esta llamada y no los toca mientras.
    let bytes = unsafe { core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(f) as *const u8, total) };
    let Some(paquete) = tu::leer(bytes) else {
        crate::ring0::cabina::warn("gpu", "VERRANO: el paquete no se sostiene (programas o vertices); bytes", total as u64);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    };
    if !bmo_gpu_ga10x::computo::ficha_valida(paquete.ficha as u64) {
        return Err(IOMMU_NO_BLUR);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bmo_gpu_ga10x::blur::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let n = (paquete.vertices.len() / tu::BYTES_VERTICE / 3) as u32;
    let r = super::volcado::quieto().and_then(|()| dibujar(bar0, paquete.ficha, e, &p, n, |r| tu::preparar(r, e, &v, &paquete)));
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

/// El dibujo, de X5 o de VERRANO: preparar, el timbre, esperar el semaforo,
/// y la escalera. `n` = los triangulos.
fn dibujar(bar0: u64, ficha: u32, e: u32, p: &bmo_gpu_ga10x::pantalla::Pantalla, n: u32, preparar: impl FnOnce(&mut Bar0) -> bool) -> Result<u64, u32> {
    let mut r = Bar0(bar0);
    asegurar_mapa(&mut r, p)?;
    if !preparar(&mut r) {
        crate::ring0::cabina::warn("gpu", "X5: el tramo del cubo no quedo preparado; no se toca el timbre", n as u64);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = cu::lanzar(&mut r, ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(e), Ordering::Release);
    }
    let mut us = 0;
    while lanzado && us < FRACTAL_ESPERA_US {
        let fin = cu::mirar(&mut r).1;
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if fin == cu::PAGA_FIN {
            break;
        }
        esperando(us);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    // La escalera y el motor grafico, como T1c: los lee `DIAG_3D` (`gpu cubo
    // 3060` los muestra si no se pago).
    let etapas = bmo_gpu_ga10x::raster::etapas(&mut r, cu::SEMAFORO_FIN, cu::PAGA_FIN);
    DIAG_3D[0].store(etapas, Ordering::Release);
    let pagados = bmo_gpu_ga10x::raster::escalones(&mut r);
    DIAG_3D[4].store(pagados as u32, Ordering::Release);
    DIAG_3D[5].store((pagados >> 32) as u32, Ordering::Release);
    for (k, reg) in super::GR_MIRADOS.iter().enumerate() {
        DIAG_3D[1 + k].store(bmo_gpu_ga10x::Registros::leer(&mut r, *reg), Ordering::Release);
    }
    let v = cu::empaquetar(us as u32, n, etapas, lanzado);
    if cu::sano(v) {
        DIBUJADO.store(true, Ordering::Release);
        let n = VECES.fetch_add(1, Ordering::AcqRel) + 1;
        if n == 1 {
            crate::ring0::cabina::count("gpu", "X5: LA 3060 DIBUJO EL CUBO del estudio D3D, sin Windows; us", us);
        }
    } else {
        crate::ring0::cabina::warn("gpu", "X5: el cubo no se pago entero; escalera", etapas as u64);
    }
    Ok(v)
}

/// **LEER**: los pixeles `2k` y `2k + 1` de la ventana (fila a fila), como
/// `0x00RRGGBB`.
fn leer(k: u64) -> Result<u64, u32> {
    if !DIBUJADO.load(Ordering::Acquire) {
        return Err(IOMMU_NO_BLUR);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_PANTALLA) };
    let Some(v) = cu::ventana(&p) else { return Err(IOMMU_NO_PANTALLA) };
    let i = 2 * k as u32;
    if i + 1 >= cu::ANCHO * cu::ALTO {
        return Err(IOMMU_NO_BLUR);
    }
    let (x, y) = (i % cu::ANCHO, i / cu::ANCHO);
    // SAFETY: escrito una vez al arrancar (`info::init_from`), solo se lee.
    let fb = crate::ring0::mm::phys_to_virt(unsafe { crate::info::FB_ADDR }) as *const u32;
    // SAFETY: (x0 + x, y0 + y) y el de al lado dentro de la pantalla (la
    // ventana cabe: `ventana` lo exige, y el ancho de la ventana es par), por
    // el physmap como `pantalla` (el framebuffer entero dentro, `la_pantalla`);
    // 32 bits alineados. `clflush` antes: el espejo es WB y la 3060 escribe la
    // VRAM sin pasar por esta cache.
    let (a, b) = unsafe {
        let q = fb.add((v.y0 + y) as usize * p.pitch as usize + (v.x0 + x) as usize);
        core::arch::x86_64::_mm_clflush(q as *const u8);
        (q.read_volatile(), q.add(1).read_volatile())
    };
    Ok(cu::a_rrggbb(a, v.rgb) as u64 | (cu::a_rrggbb(b, v.rgb) as u64) << 32)
}
