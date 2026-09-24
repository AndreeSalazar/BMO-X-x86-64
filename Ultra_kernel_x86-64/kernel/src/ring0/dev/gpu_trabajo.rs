//! **M5d: EL TRABAJO EN LA 3060** -- lo que corre en el motor grafico una vez
//! hecho el contexto de oro: la clase de computo y su primer trabajo (S1..S3),
//! el primer sombreador (S4..S6), el lienzo en la RAM del PC (L) y el blur (B).
//! Partido de `gpu_libos.rs` el 24-09 (el censo modular lo paro en 1022
//! lineas de codigo): es TEXTO MOVIDO, las mismas funciones, y usa de alli la
//! cola de RPC (`enviar`), los marcos (`grupo`, `memoria`) y la IOMMU
//! (`escribible`).
//!
//! [carril]  ROJO      presta 32 marcos NEUTRO ESCRIBIBLES a la 3060 (el lienzo y
//!                     la salida del blur) y toca su timbre
//! [consumo] NADA      corre por orden (`gpu computo`, `gpu sombreo`, `gpu lienzo`,
//!                     `gpu blur`, y sus pasos de `save mode`)
//!
//! [eje]     CORRECCION -- cada trabajo se comprueba entero: el semaforo que
//!           solo escribe el GR, y cada palabra o pixel contra la CPU

use core::sync::atomic::{AtomicU64, Ordering};

use super::gpu_libos::{enviar, escribible, grupo, memoria, COPIA_ESPERA_US, GR_TRESDE, PAGINA};
use crate::ring0::plat::iommu as io;

// == M5d S1 y S3: EL COMPUTO Y EL PRIMER TRABAJO DEL GR (2026-09-24) ==========
//
// Tras el oro (VISTO en el metal 24-09 15:51): AMPERE_COMPUTE_B en el canal de
// GR0 por RPC, y UNA vez por arranque el primer trabajo del motor grafico --
// la receta de la copia (`copiar`) con `bmo_gpu_ga10x::computo`: ordenes y
// GPFIFO por PRAMIN en el tramo, GP_PUT del canal de GR0 y la ficha en el
// timbre. Lo paga un semaforo de INFORME: solo lo escribe el GR.

static COMPUTO_PEDIDO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static TRABAJO_GR_HECHO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// S1: sin el oro (G4), o el computo ya se pidio.
pub const IOMMU_NO_COMPUTO: u32 = 71;
/// S3: sin el computo, una ficha que no es del canal de GR0, o ya se hizo.
pub const IOMMU_NO_TRABAJO_GR: u32 = 72;
/// S3: el tramo no se releyo igual: no se toco el timbre.
pub const IOMMU_NO_TRABAJO_GR_PREPARAR: u32 = 73;

/// **M5d S1: AMPERE_COMPUTE_B.** `Ok(pagina | numero << 32)` de la RPC.
pub fn pedir_computo() -> Result<u64, u32> {
    if !GR_TRESDE.load(Ordering::Acquire) || COMPUTO_PEDIDO.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_COMPUTO);
    }
    match enviar(bmo_gpu_ga10x::computo::pedir) {
        Ok(v) => {
            crate::ring0::cabina::count("gpu", "M5d S1: GSP_RM_ALLOC de AMPERE_COMPUTE_B pedido; asa", bmo_gpu_ga10x::computo::COMPUTO as u64);
            Ok(v)
        }
        Err(e) => {
            COMPUTO_PEDIDO.store(false, Ordering::Release);
            Err(e)
        }
    }
}

/// **M5d S3: el primer trabajo del GR.** `ficha` = la de `FichaGr` con la
/// lista de GR0 (la de la tabla de aparatos). `Ok(computo::empaquetar(..))`.
pub fn trabajo_gr(ficha: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::computo as cm;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !COMPUTO_PEDIDO.load(Ordering::Acquire) || !cm::ficha_valida(ficha) {
        return Err(IOMMU_NO_TRABAJO_GR);
    }
    if TRABAJO_GR_HECHO.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_TRABAJO_GR);
    }
    let mut r = crate::ring0::dev::gpu_prestamo::Bar0(bar0);
    if !cm::preparar(&mut r) {
        // Nada llego a la 3060: se puede reintentar.
        TRABAJO_GR_HECHO.store(false, Ordering::Release);
        crate::ring0::cabina::warn("gpu", "M5d S3: el tramo no quedo preparado; no se toca el timbre", 0);
        return Err(IOMMU_NO_TRABAJO_GR_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let lanzado = cm::lanzar(&mut r, ficha as u32);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let (mut gp_get, mut semaforo) = (0, 0);
    let mut us = 0;
    while lanzado && us < COPIA_ESPERA_US {
        (gp_get, semaforo) = cm::mirar(&mut r);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if semaforo == cm::PAGA {
            break;
        }
        core::hint::spin_loop();
    }
    // Como la copia: GP_GET llega despues; hasta 10 ms, sin exigirlo.
    let pagado_en = us;
    while semaforo == cm::PAGA && gp_get == 0 && us < pagado_en + 10_000 {
        gp_get = cm::mirar(&mut r).0;
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        core::hint::spin_loop();
    }
    let v = cm::empaquetar(semaforo, gp_get, lanzado, pagado_en as u32);
    if cm::sano(v) {
        crate::ring0::cabina::count("gpu", "M5d S3: EL MOTOR GRAFICO CORRIO nuestro trabajo; us", pagado_en);
    } else {
        crate::ring0::cabina::warn("gpu", "M5d S3: el GR no pago el semaforo; lo que habia", semaforo as u64);
    }
    Ok(v)
}

// == M5d S4..S6: EL PRIMER SOMBREADOR (2026-09-24) ============================
//
// Tras S3 (VISTO en el metal 24-09 16:06). `bmo_gpu_ga10x::sombreador`: el
// programa (SASS de SM86 de `ptxas`, comprobado con `nvdisasm`), el QMD y las
// ordenes por PRAMIN en el tramo, la entrada 1 del GPFIFO de GR0, GP_PUT = 2 y
// la ficha en el timbre. Se espera el semaforo del QMD (la rejilla acabo) y el
// de informe, y se leen las 32 palabras. Lo UNICO que la GPU toca es el tramo.

static SOMBREO_HECHO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// S4..S6: sin S3, una ficha que no es del canal de GR0, o ya se hizo.
pub const IOMMU_NO_SOMBREO: u32 = 74;
/// S4..S6: el tramo no se releyo igual: no se toco el timbre.
pub const IOMMU_NO_SOMBREO_PREPARAR: u32 = 75;

/// **M5d S4..S6: el primer sombreador.** `ficha` = la de S3. `Ok(sombreador::
/// empaquetar(..))`.
pub fn sombrear(ficha: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::sombreador as sb;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !TRABAJO_GR_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_SOMBREO);
    }
    if SOMBREO_HECHO.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_SOMBREO);
    }
    let mut r = crate::ring0::dev::gpu_prestamo::Bar0(bar0);
    if !sb::preparar(&mut r) {
        SOMBREO_HECHO.store(false, Ordering::Release);
        crate::ring0::cabina::warn("gpu", "M5d S4: el tramo no quedo preparado; no se toca el timbre", 0);
        return Err(IOMMU_NO_SOMBREO_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let lanzado = sb::lanzar(&mut r, ficha as u32);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let (mut gp_get, mut qmd, mut fin) = (0, 0, 0);
    let mut us = 0;
    while lanzado && us < COPIA_ESPERA_US {
        (gp_get, qmd, fin) = sb::mirar(&mut r);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if qmd == sb::PAGA_QMD && fin == sb::PAGA_FIN {
            break;
        }
        core::hint::spin_loop();
    }
    let acabo_en = us;
    let (buenas, limpio) = sb::comprobar(&mut r);
    let v = sb::empaquetar(buenas, limpio, qmd == sb::PAGA_QMD, fin == sb::PAGA_FIN, lanzado, gp_get, acabo_en as u32);
    if sb::sano(v) {
        crate::ring0::cabina::count("gpu", "M5d S6: EL PRIMER SOMBREADOR DE BMO-X CORRIO en la 3060; us", acabo_en);
    } else {
        crate::ring0::cabina::warn("gpu", "M5d S6: el sombreador no salio entero; hilos buenos", buenas as u64);
    }
    Ok(v)
}

// == M5d L: EL LIENZO -- LA 3060 PINTA EN LA RAM DEL PC (2026-09-24) ==========
//
// Tras el primer sombreador (VISTO 24-09 16:21). 64 KiB de RAM del PC (16
// marcos NEUTRO, por `grupo`), a cero y prestados ESCRIBIBLES en
// `lienzo::IOVA`; sus 16 PTE de SISTEMA en la PT del tramo (solo si estaban
// vacias); y un programa de 128 x 128 hilos (`lienzo::CODIGO`) que pinta un
// degradado. La CPU lo comprueba pixel a pixel y el escritorio lo lee de dos
// en dos (`leer_lienzo`).

static LIENZO_F: AtomicU64 = AtomicU64::new(0);
static LIENZO_PRESTADO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static LIENZO_HECHO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// L: sin el primer sombreador, una ficha ajena, o ya se pinto.
pub const IOMMU_NO_LIENZO: u32 = 76;
/// L: el lienzo no se presto, sus PTE no estaban vacias, o el tramo no se
/// releyo: no se toco el timbre.
pub const IOMMU_NO_LIENZO_PREPARAR: u32 = 77;

/// Los pixeles del lienzo, si ya se presto.
fn pixeles_del_lienzo() -> Option<&'static [u32]> {
    let f = LIENZO_F.load(Ordering::Acquire);
    if f == 0 || !LIENZO_PRESTADO.load(Ordering::Acquire) {
        return None;
    }
    let b = memoria(f, bmo_gpu_ga10x::lienzo::PAGINAS * PAGINA);
    // SAFETY: `b` son los marcos del lienzo, alineados a pagina (y por tanto a
    // 4) y de 64 KiB justos.
    Some(unsafe { core::slice::from_raw_parts(b.as_ptr() as *const u32, bmo_gpu_ga10x::lienzo::PIXELES) })
}

/// **M5d L: que la 3060 pinte el lienzo.** `ficha` = la de S3.
/// `Ok(lienzo::empaquetar(..))`.
pub fn pintar_lienzo(ficha: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::lienzo as lz;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !SOMBREO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_LIENZO);
    }
    if LIENZO_HECHO.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_LIENZO);
    }
    let fallo = |m: u32, que: &str, v: u64| {
        LIENZO_HECHO.store(false, Ordering::Release);
        crate::ring0::cabina::warn("gpu", que, v);
        Err(m)
    };
    let mut r = crate::ring0::dev::gpu_prestamo::Bar0(bar0);
    // Una vez por arranque: prestar y mapear.
    if !LIENZO_PRESTADO.load(Ordering::Acquire) {
        let Some(f) = grupo(&LIENZO_F, lz::PAGINAS) else {
            return fallo(IOMMU_NO_LIENZO_PREPARAR, "M5d L: no hubo 16 marcos seguidos para el lienzo", 0);
        };
        memoria(f, lz::PAGINAS * PAGINA).fill(0);
        if io::prestar_gpu(lz::IOVA, f, lz::PAGINAS, true).is_err()
            || !(0..lz::PAGINAS).all(|k| escribible(lz::IOVA + k * PAGINA, f + k * PAGINA))
        {
            return fallo(IOMMU_NO_LIENZO_PREPARAR, "M5d L: el lienzo no se ve por la IOMMU donde se presto; iova", lz::IOVA);
        }
        match lz::mapear(&mut r) {
            Some((n, bien)) if n == bien => {}
            _ => return fallo(IOMMU_NO_LIENZO_PREPARAR, "M5d L: las PTE del lienzo no estaban vacias o no se releyeron", 0),
        }
        LIENZO_PRESTADO.store(true, Ordering::Release);
        crate::ring0::cabina::count("gpu", "M5d L: lienzo de 64 KiB PRESTADO a la 3060 y mapeado; iova", lz::IOVA);
    }
    // Cada vez, de cero: lo que se lea despues lo pinto la 3060.
    memoria(LIENZO_F.load(Ordering::Acquire), lz::PAGINAS * PAGINA).fill(0);
    if !lz::preparar(&mut r) {
        return fallo(IOMMU_NO_LIENZO_PREPARAR, "M5d L: el tramo no quedo preparado; no se toca el timbre", 0);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let lanzado = lz::lanzar(&mut r, ficha as u32);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let (mut gp_get, mut qmd, mut fin) = (0, 0, 0);
    let mut us = 0;
    while lanzado && us < COPIA_ESPERA_US {
        (gp_get, qmd, fin) = lz::mirar(&mut r);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if qmd == lz::PAGA_QMD && fin == lz::PAGA_FIN {
            break;
        }
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let buenos = pixeles_del_lienzo().map_or(0, lz::comprobar);
    let v = lz::empaquetar(buenos, qmd == lz::PAGA_QMD, fin == lz::PAGA_FIN, lanzado, gp_get, us as u32);
    if lz::sano(v) {
        crate::ring0::cabina::count("gpu", "M5d L: LA 3060 PINTO 128x128 pixeles en la RAM del PC; us", us);
    } else {
        crate::ring0::cabina::warn("gpu", "M5d L: el lienzo no salio entero; pixeles buenos", buenos as u64);
    }
    Ok(v)
}

/// **Leer el lienzo, dos pixeles por llamada**: `k` = el par (0..8192); con
/// el bit 32, de la SALIDA del blur; con el 33, del FRACTAL (0..131072). `Ok(pixel 2k | pixel 2k+1 << 32)`. Solo
/// tras `pintar_lienzo` (o tras un blur). Solo lectura.
pub fn leer_lienzo(arg: u64) -> Result<u64, u32> {
    if !LIENZO_HECHO.load(Ordering::Acquire) {
        return Err(IOMMU_NO_LIENZO);
    }
    let (k, de_la_salida, del_fractal) = (arg & 0x3_FFFF, arg >> 32 & 1 != 0, arg >> 33 & 1 != 0);
    let p = if del_fractal {
        pixeles_del_fractal()
    } else if de_la_salida {
        pixeles_del_blur()
    } else {
        pixeles_del_lienzo()
    }
    .ok_or(IOMMU_NO_LIENZO)?;
    let i = (k as usize).checked_mul(2).filter(|&i| i + 1 < p.len()).ok_or(IOMMU_NO_LIENZO)?;
    Ok(p[i] as u64 | (p[i + 1] as u64) << 32)
}

// == M5d B: EL BLUR (2026-09-24) ==============================================
//
// Tras el lienzo. El escritorio sube 128 x 128 pixeles AL LIENZO, dos por
// llamada (`escribir_lienzo`); `blur` presta y mapea la SALIDA (16 marcos mas,
// una vez por arranque), lanza el programa de `bmo_gpu_ga10x::blur` en la
// siguiente entrada del GPFIFO de GR (se puede repetir) y compara cada pixel
// con la misma cuenta en la CPU.

static BLUR_F: AtomicU64 = AtomicU64::new(0);
static BLUR_PRESTADO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// La siguiente entrada del GPFIFO de GR para el blur.
static BLUR_ENTRADA: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(bmo_gpu_ga10x::blur::PRIMERA_ENTRADA);
/// Uno en marcha: ni otro blur ni subir pixeles mientras.
static BLUR_EN_MARCHA: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// B: sin el lienzo, una ficha ajena, el GPFIFO gastado, o uno en marcha.
pub const IOMMU_NO_BLUR: u32 = 78;
/// B: la salida no se presto, sus PTE no estaban vacias, o el tramo no se
/// releyo: no se toco el timbre.
pub const IOMMU_NO_BLUR_PREPARAR: u32 = 79;

fn pixeles_del_blur() -> Option<&'static [u32]> {
    let f = BLUR_F.load(Ordering::Acquire);
    if f == 0 || !BLUR_PRESTADO.load(Ordering::Acquire) {
        return None;
    }
    let b = memoria(f, bmo_gpu_ga10x::lienzo::PAGINAS * PAGINA);
    // SAFETY: como `pixeles_del_lienzo`.
    Some(unsafe { core::slice::from_raw_parts(b.as_ptr() as *const u32, bmo_gpu_ga10x::lienzo::PIXELES) })
}

/// **Subir dos pixeles al lienzo**: `arg` = `blur::subir(k, p0, p1)`. Tras
/// pintarlo y con ningun blur en marcha. `Ok(k)`.
pub fn escribir_lienzo(arg: u64) -> Result<u64, u32> {
    if !LIENZO_HECHO.load(Ordering::Acquire) || BLUR_EN_MARCHA.load(Ordering::Acquire) {
        return Err(IOMMU_NO_BLUR);
    }
    let (k, p0, p1) = bmo_gpu_ga10x::blur::bajar(arg);
    let f = LIENZO_F.load(Ordering::Acquire);
    if f == 0 || 2 * k as usize + 1 >= bmo_gpu_ga10x::lienzo::PIXELES {
        return Err(IOMMU_NO_BLUR);
    }
    let b = memoria(f, bmo_gpu_ga10x::lienzo::PAGINAS * PAGINA);
    let o = 8 * k as usize;
    b[o..o + 4].copy_from_slice(&p0.to_le_bytes());
    b[o + 4..o + 8].copy_from_slice(&p1.to_le_bytes());
    Ok(k as u64)
}

/// **M5d B: el blur.** `ficha` = la de S3. `Ok(blur::empaquetar(..))`.
pub fn blur(ficha: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_BLUR);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let r = blur_(bar0, ficha as u32, e);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

fn blur_(bar0: u64, ficha: u32, e: u32) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    use bmo_gpu_ga10x::lienzo as lz;
    let mut r = crate::ring0::dev::gpu_prestamo::Bar0(bar0);
    if !BLUR_PRESTADO.load(Ordering::Acquire) {
        let Some(f) = grupo(&BLUR_F, lz::PAGINAS) else {
            crate::ring0::cabina::warn("gpu", "M5d B: no hubo 16 marcos seguidos para la salida del blur", 0);
            return Err(IOMMU_NO_BLUR_PREPARAR);
        };
        memoria(f, lz::PAGINAS * PAGINA).fill(0);
        if io::prestar_gpu(bl::IOVA, f, lz::PAGINAS, true).is_err()
            || !(0..lz::PAGINAS).all(|k| escribible(bl::IOVA + k * PAGINA, f + k * PAGINA))
        {
            crate::ring0::cabina::warn("gpu", "M5d B: la salida no se ve por la IOMMU donde se presto; iova", bl::IOVA);
            return Err(IOMMU_NO_BLUR_PREPARAR);
        }
        match bl::mapear(&mut r) {
            Some((n, bien)) if n == bien => {}
            _ => {
                crate::ring0::cabina::warn("gpu", "M5d B: las PTE de la salida no estaban vacias o no se releyeron", 0);
                return Err(IOMMU_NO_BLUR_PREPARAR);
            }
        }
        BLUR_PRESTADO.store(true, Ordering::Release);
        crate::ring0::cabina::count("gpu", "M5d B: salida de 64 KiB PRESTADA a la 3060 y mapeada; iova", bl::IOVA);
    }
    // Cada vez, de cero: lo que se lea despues lo escribio la 3060.
    memoria(BLUR_F.load(Ordering::Acquire), lz::PAGINAS * PAGINA).fill(0);
    if !bl::preparar(&mut r, e) {
        crate::ring0::cabina::warn("gpu", "M5d B: el tramo no quedo preparado; no se toca el timbre", 0);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let lanzado = bl::lanzar(&mut r, ficha, e);
    if lanzado {
        // La entrada ya es de la 3060: la siguiente vez, la de despues.
        BLUR_ENTRADA.store(e + 1, Ordering::Release);
    }
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let (mut gp_get, mut qmd, mut fin) = (0, 0, 0);
    let mut us = 0;
    while lanzado && us < COPIA_ESPERA_US {
        (gp_get, qmd, fin) = bl::mirar(&mut r);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if qmd == bl::PAGA_QMD && fin == bl::PAGA_FIN {
            break;
        }
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let buenos = match (pixeles_del_lienzo(), pixeles_del_blur()) {
        (Some(src), Some(sal)) => bl::comprobar(src, sal),
        _ => 0,
    };
    let v = bl::empaquetar(buenos, qmd == bl::PAGA_QMD, fin == bl::PAGA_FIN, lanzado, gp_get, us as u32);
    if bl::sano(v) {
        crate::ring0::cabina::count("gpu", "M5d B: LA 3060 DESENFOCO 128x128 pixeles, igual que la CPU; us", us);
    } else {
        crate::ring0::cabina::warn("gpu", "M5d B: el blur no salio igual que la CPU; pixeles buenos", buenos as u64);
    }
    Ok(v)
}

// == M5d F: EL FRACTAL -- LA FUERZA DE LA 3060 (2026-09-24) ===================
//
// Mandelbrot de 512 x 512 (hasta 256 vueltas por pixel, 262144 hilos) en 1 MiB
// de RAM del PC (256 marcos NEUTRO por `grupo`, prestados ESCRIBIBLES en
// `fractal::IOVA`, una vez por arranque). Se cronometra la 3060 (del timbre al
// semaforo) y la CPU haciendo la MISMA cuenta (`fractal::comprobar`), que de
// paso compara cada pixel. Comparte con el blur la cuenta de entradas del
// GPFIFO y el cerrojo de "uno en marcha".

static FRACTAL_F: AtomicU64 = AtomicU64::new(0);
static FRACTAL_PRESTADO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// Lo mas que se espera a la 3060: 262144 hilos de hasta 256 vueltas.
const FRACTAL_ESPERA_US: u64 = 1_000_000;

fn pixeles_del_fractal() -> Option<&'static [u32]> {
    let f = FRACTAL_F.load(Ordering::Acquire);
    if f == 0 || !FRACTAL_PRESTADO.load(Ordering::Acquire) {
        return None;
    }
    let b = memoria(f, bmo_gpu_ga10x::fractal::PAGINAS * PAGINA);
    // SAFETY: los 256 marcos del fractal, alineados a pagina, de 1 MiB justo.
    Some(unsafe { core::slice::from_raw_parts(b.as_ptr() as *const u32, bmo_gpu_ga10x::fractal::PIXELES) })
}

/// **M5d F: el fractal.** `ficha` = la de S3. `Ok(fractal::empaquetar(..))`.
pub fn fractal(ficha: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_BLUR);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let r = fractal_(bar0, ficha as u32, e);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

/// **El MiB grande** (el del fractal y el triangulo): prestado y mapeado UNA
/// vez por arranque.
fn asegurar_mib(r: &mut crate::ring0::dev::gpu_prestamo::Bar0) -> Result<(), u32> {
    use bmo_gpu_ga10x::fractal as fr;
    if FRACTAL_PRESTADO.load(Ordering::Acquire) {
        return Ok(());
    }
    let Some(f) = grupo(&FRACTAL_F, fr::PAGINAS) else {
        crate::ring0::cabina::warn("gpu", "M5d F: no hubo 256 marcos seguidos para el fractal", 0);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    };
    memoria(f, fr::PAGINAS * PAGINA).fill(0);
    if io::prestar_gpu(fr::IOVA, f, fr::PAGINAS, true).is_err()
        || !(0..fr::PAGINAS).all(|k| escribible(fr::IOVA + k * PAGINA, f + k * PAGINA))
    {
        crate::ring0::cabina::warn("gpu", "M5d F: el fractal no se ve por la IOMMU donde se presto; iova", fr::IOVA);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    match fr::mapear(r) {
        Some((n, bien)) if n == bien => {}
        _ => {
            crate::ring0::cabina::warn("gpu", "M5d F: las PTE del fractal no estaban vacias o no se releyeron", 0);
            return Err(IOMMU_NO_BLUR_PREPARAR);
        }
    }
    FRACTAL_PRESTADO.store(true, Ordering::Release);
    crate::ring0::cabina::count("gpu", "M5d F: 1 MiB PRESTADO a la 3060 para el fractal y mapeado; iova", fr::IOVA);
    Ok(())
}

fn fractal_(bar0: u64, ficha: u32, e: u32) -> Result<u64, u32> {
    use bmo_gpu_ga10x::fractal as fr;
    let mut r = crate::ring0::dev::gpu_prestamo::Bar0(bar0);
    asegurar_mib(&mut r)?;
    memoria(FRACTAL_F.load(Ordering::Acquire), fr::PAGINAS * PAGINA).fill(0);
    if !fr::preparar(&mut r, e) {
        crate::ring0::cabina::warn("gpu", "M5d F: el tramo no quedo preparado; no se toca el timbre", 0);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = fr::lanzar(&mut r, ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(e + 1, Ordering::Release);
    }
    let (mut qmd, mut fin) = (0, 0);
    let mut us = 0;
    while lanzado && us < FRACTAL_ESPERA_US {
        (_, qmd, fin) = fr::mirar(&mut r);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if qmd == fr::PAGA_QMD && fin == fr::PAGA_FIN {
            break;
        }
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    // La CPU hace la MISMA cuenta, cronometrada, y compara cada pixel.
    let cpu_desde = crate::ring0::task::scheduler::rdtsc();
    let (buenos, vueltas) = pixeles_del_fractal().map_or((0, 0), fr::comprobar);
    let cpu_us = (crate::ring0::task::scheduler::rdtsc() - cpu_desde) / hz;
    let v = fr::empaquetar(buenos, qmd == fr::PAGA_QMD, fin == fr::PAGA_FIN, lanzado, us as u32, cpu_us as u32);
    if fr::sano(v) {
        crate::ring0::cabina::count("gpu", "M5d F: LA 3060 CALCULO EL FRACTAL de 512x512 igual que la CPU; us", us);
        crate::ring0::cabina::count("gpu", "M5d F: la CPU tardo en lo mismo, us", cpu_us);
        crate::ring0::cabina::count("gpu", "M5d F: vueltas en total", vueltas);
    } else {
        crate::ring0::cabina::warn("gpu", "M5d F: el fractal no salio igual que la CPU; pixeles buenos", buenos as u64);
    }
    Ok(v)
}

// == M5d T0: EL TRIANGULO, POR COMPUTO (2026-09-24) ============================
//
// Las tres funciones de arista en 262144 hilos, en el MISMO MiB que el fractal
// (`asegurar_mib`), en la siguiente entrada del GPFIFO de GR; cronometrado y
// comparado con la CPU como el fractal.

/// **M5d T0: el triangulo.** `ficha` = la de S3. `Ok(triangulo::empaquetar(..))`.
pub fn triangulo(ficha: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_BLUR);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let r = triangulo_(bar0, ficha as u32, e);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

fn triangulo_(bar0: u64, ficha: u32, e: u32) -> Result<u64, u32> {
    use bmo_gpu_ga10x::fractal as fr;
    use bmo_gpu_ga10x::triangulo as tr;
    let mut r = crate::ring0::dev::gpu_prestamo::Bar0(bar0);
    asegurar_mib(&mut r)?;
    memoria(FRACTAL_F.load(Ordering::Acquire), fr::PAGINAS * PAGINA).fill(0);
    if !tr::preparar(&mut r, e) {
        crate::ring0::cabina::warn("gpu", "M5d T0: el tramo no quedo preparado; no se toca el timbre", 0);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = tr::lanzar(&mut r, ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(e + 1, Ordering::Release);
    }
    let (mut qmd, mut fin) = (0, 0);
    let mut us = 0;
    while lanzado && us < FRACTAL_ESPERA_US {
        (_, qmd, fin) = tr::mirar(&mut r);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if qmd == tr::PAGA_QMD && fin == tr::PAGA_FIN {
            break;
        }
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let cpu_desde = crate::ring0::task::scheduler::rdtsc();
    let buenos = pixeles_del_fractal().map_or(0, tr::comprobar);
    let cpu_us = (crate::ring0::task::scheduler::rdtsc() - cpu_desde) / hz;
    let v = tr::empaquetar(buenos, qmd == tr::PAGA_QMD, fin == tr::PAGA_FIN, lanzado, us as u32, cpu_us as u32);
    if tr::sano(v) {
        crate::ring0::cabina::count("gpu", "M5d T0: EL PRIMER TRIANGULO DE LA 3060, igual que la CPU; us", us);
    } else {
        crate::ring0::cabina::warn("gpu", "M5d T0: el triangulo no salio igual que la CPU; pixeles buenos", buenos as u64);
    }
    Ok(v)
}

// == M5 T1a: LA CLASE 3D LIMPIA UN DESTINO (2026-09-24) =======================
//
// AMPERE_B, con su hardware de pixeles (el ROP) y sin programas, limpia el
// MISMO MiB que el fractal como destino de render de 512 x 512. La CPU solo
// cuenta los pixeles que salieron del color de limpieza.

/// **M5 T1a.** `ficha` = la de S3. `Ok(tresde::empaquetar(..))`.
pub fn limpiar_3d(ficha: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_BLUR);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let r = limpiar_3d_(bar0, ficha as u32, e);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

fn limpiar_3d_(bar0: u64, ficha: u32, e: u32) -> Result<u64, u32> {
    use bmo_gpu_ga10x::fractal as fr;
    use bmo_gpu_ga10x::tresde as td;
    let mut r = crate::ring0::dev::gpu_prestamo::Bar0(bar0);
    asegurar_mib(&mut r)?;
    memoria(FRACTAL_F.load(Ordering::Acquire), fr::PAGINAS * PAGINA).fill(0);
    if !td::preparar(&mut r, e) {
        crate::ring0::cabina::warn("gpu", "M5 T1a: el tramo no quedo preparado; no se toca el timbre", 0);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = td::lanzar(&mut r, ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(e + 1, Ordering::Release);
    }
    let mut fin = 0;
    let mut us = 0;
    while lanzado && us < FRACTAL_ESPERA_US {
        fin = td::mirar(&mut r).1;
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if fin == td::PAGA_FIN {
            break;
        }
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let cpu_desde = crate::ring0::task::scheduler::rdtsc();
    let buenos = pixeles_del_fractal().map_or(0, td::comprobar);
    let cpu_us = (crate::ring0::task::scheduler::rdtsc() - cpu_desde) / hz;
    // Sin QMD: el semaforo de la clase 3D cuenta por los dos.
    let pagado = fin == td::PAGA_FIN;
    let v = td::empaquetar(buenos, pagado, pagado, lanzado, us as u32, cpu_us as u32);
    if td::sano(v) {
        crate::ring0::cabina::count("gpu", "M5 T1a: LA CLASE 3D LIMPIO 512x512 con su ROP; us", us);
    } else {
        crate::ring0::cabina::warn("gpu", "M5 T1a: la limpieza 3D no salio entera; pixeles buenos", buenos as u64);
    }
    Ok(v)
}

// == M5d E: LA ESCENA 3D CON LUZ (2026-09-24) ==================================
//
// La esfera iluminada, el suelo con sombra y el cielo: 262144 hilos en el MISMO
// MiB que el fractal; cronometrado y comparado con la CPU como el fractal.

/// **M5d E: la escena.** `ficha` = la de S3. `Ok(escena::empaquetar(..))`.
pub fn escena(ficha: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_BLUR);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let r = escena_(bar0, ficha as u32, e);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

fn escena_(bar0: u64, ficha: u32, e: u32) -> Result<u64, u32> {
    use bmo_gpu_ga10x::escena as es;
    use bmo_gpu_ga10x::fractal as fr;
    let mut r = crate::ring0::dev::gpu_prestamo::Bar0(bar0);
    asegurar_mib(&mut r)?;
    memoria(FRACTAL_F.load(Ordering::Acquire), fr::PAGINAS * PAGINA).fill(0);
    if !es::preparar(&mut r, e) {
        crate::ring0::cabina::warn("gpu", "M5d E: el tramo no quedo preparado; no se toca el timbre", 0);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = es::lanzar(&mut r, ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(e + 1, Ordering::Release);
    }
    let (mut qmd, mut fin) = (0, 0);
    let mut us = 0;
    while lanzado && us < FRACTAL_ESPERA_US {
        (_, qmd, fin) = es::mirar(&mut r);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if qmd == es::PAGA_QMD && fin == es::PAGA_FIN {
            break;
        }
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let cpu_desde = crate::ring0::task::scheduler::rdtsc();
    let buenos = pixeles_del_fractal().map_or(0, es::comprobar);
    let cpu_us = (crate::ring0::task::scheduler::rdtsc() - cpu_desde) / hz;
    let v = es::empaquetar(buenos, qmd == es::PAGA_QMD, fin == es::PAGA_FIN, lanzado, us as u32, cpu_us as u32);
    if es::sano(v) {
        crate::ring0::cabina::count("gpu", "M5d E: LA 3060 DIBUJO LA ESCENA 3D con luz, igual que la CPU; us", us);
    } else {
        crate::ring0::cabina::warn("gpu", "M5d E: la escena no salio igual que la CPU; pixeles buenos", buenos as u64);
    }
    Ok(v)
}

// == M5 T1b + T1c: EL TRIANGULO POR EL RASTERIZADOR (2026-09-24) ==============
//
// Lo de T1a (el destino, limpio a magenta) y encima UN triangulo por el
// pipeline 3D entero: programa de vertice, rasterizador, programa de pixel y
// ROP. La CPU no dibuja: sabe que pixeles tienen el centro dentro y cuenta.

/// **M5 T1c.** `ficha` = la de S3. `Ok(raster::empaquetar(..))`.
pub fn raster(ficha: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::raster as ra;
    dibujo_3d(ficha, &Dibujo3d {
        nombre: "M5 T1c",
        preparar: ra::preparar,
        mirar: ra::mirar,
        paga: ra::PAGA_FIN,
        semaforo: ra::SEMAFORO_FIN,
        comprobar: ra::comprobar,
        verdes: Some(ra::verdes),
    })
}

/// **M5 T2a: el triangulo con color, mezclado por el rasterizador.**
pub fn color_3d(ficha: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::color3d as c3;
    dibujo_3d(ficha, &Dibujo3d {
        nombre: "M5 T2a",
        preparar: c3::preparar,
        mirar: c3::mirar,
        paga: c3::PAGA_FIN,
        semaforo: c3::SEMAFORO_FIN,
        comprobar: c3::comprobar,
        verdes: None,
    })
}

use crate::ring0::dev::gpu_prestamo::Bar0;

/// Un dibujo por el pipeline 3D: sus programas, su semaforo y su juez.
struct Dibujo3d {
    nombre: &'static str,
    preparar: fn(&mut Bar0, u32) -> bool,
    mirar: fn(&mut Bar0) -> (u32, u32),
    paga: u32,
    semaforo: u64,
    comprobar: fn(&[u32]) -> u32,
    /// Los pixeles del color del programa de pixel, para el log si no sale.
    verdes: Option<fn(&[u32]) -> u32>,
}

fn dibujo_3d(ficha: u64, d: &Dibujo3d) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_BLUR);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let r = dibujo_3d_(bar0, ficha as u32, e, d);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

fn dibujo_3d_(bar0: u64, ficha: u32, e: u32, d: &Dibujo3d) -> Result<u64, u32> {
    use bmo_gpu_ga10x::fractal as fr;
    use bmo_gpu_ga10x::raster as ra;
    let mut r = Bar0(bar0);
    asegurar_mib(&mut r)?;
    memoria(FRACTAL_F.load(Ordering::Acquire), fr::PAGINAS * PAGINA).fill(0);
    if !(d.preparar)(&mut r, e) {
        crate::ring0::cabina::warn("gpu", d.nombre, 0);
        crate::ring0::cabina::warn("gpu", "  el tramo 3D no quedo preparado; no se toca el timbre", 0);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = ra::lanzar(&mut r, ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(e + 1, Ordering::Release);
    }
    let mut fin = 0;
    let mut us = 0;
    while lanzado && us < FRACTAL_ESPERA_US {
        fin = (d.mirar)(&mut r).1;
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if fin == d.paga {
            break;
        }
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let cpu_desde = crate::ring0::task::scheduler::rdtsc();
    let buenos = pixeles_del_fractal().map_or(0, d.comprobar);
    let cpu_us = (crate::ring0::task::scheduler::rdtsc() - cpu_desde) / hz;
    let pagado = fin == d.paga;
    // La escalera y el motor grafico, como quedaron: los lee `DIAG_3D`.
    DIAG_3D[0].store(ra::etapas(&mut r, d.semaforo, d.paga) | ra::escalones(&mut r) << 8, Ordering::Release);
    for (k, reg) in GR_MIRADOS.iter().enumerate() {
        DIAG_3D[1 + k].store(bmo_gpu_ga10x::Registros::leer(&mut r, *reg), Ordering::Release);
    }
    let v = ra::empaquetar(buenos, pagado, pagado, lanzado, us as u32, cpu_us as u32);
    if ra::sano(v) {
        crate::ring0::cabina::count("gpu", d.nombre, us);
        crate::ring0::cabina::count("gpu", "  EL RASTERIZADOR DE LA 3060 DIBUJO, igual que el juez; us", us);
    } else {
        crate::ring0::cabina::warn("gpu", d.nombre, buenos as u64);
        crate::ring0::cabina::warn("gpu", "  el dibujo 3D no salio igual que el juez; pixeles buenos", buenos as u64);
        if let Some(f) = d.verdes {
            let verdes = pixeles_del_fractal().map_or(0, f);
            crate::ring0::cabina::warn("gpu", "  pixeles verdes (los del programa de pixel)", verdes as u64);
        }
    }
    Ok(v)
}

/// Los registros del motor grafico que se miran tras un dibujo 3D: su
/// interrupcion (`NV_PGRAPH_INTR`), su excepcion (`NV_PGRAPH_EXCEPTION`) y
/// que unidades siguen ocupadas (`NV_PGRAPH_STATUS`). Solo se LEEN.
const GR_MIRADOS: [u32; 3] = [0x0040_0100, 0x0040_0108, 0x0040_0700];

/// Lo que quedo del ultimo dibujo 3D: la escalera (bit 0 estado, 1 vertices,
/// 2 entero; bits 8..15 los escalones del estado) y los tres registros.
static DIAG_3D: [core::sync::atomic::AtomicU32; 4] = [const { core::sync::atomic::AtomicU32::new(0) }; 4];

/// **Op 0x39**: la palabra `k` de `DIAG_3D`.
pub fn diag_3d(k: u64) -> Result<u64, u32> {
    DIAG_3D.get(k as usize).map(|v| v.load(Ordering::Acquire) as u64).ok_or(IOMMU_NO_BLUR)
}

// == M5d G: LA ESFERA QUE GIRA Y BOTA (2026-09-24) ============================
//
// Movimiento: UN fotograma de 256 x 256 por llamada, con sus parametros; el
// escritorio pide los 32 de la vuelta. Por el camino de `escena` (computo),
// que ya funciona en el metal, mientras T1c se sigue buscando.

/// **M5d G.** `arg` = la ficha de S3 (bits 0..31) y el fotograma (32..39).
/// `Ok(giro::empaquetar(..))`.
pub fn giro(arg: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::blur as bl;
    let (ficha, f) = (arg & 0xFFFF_FFFF, (arg >> 32) as u32 & 0xFF);
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) || f >= bmo_gpu_ga10x::giro::FOTOGRAMAS {
        return Err(IOMMU_NO_BLUR);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bl::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let r = giro_(bar0, ficha as u32, e, f);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

fn giro_(bar0: u64, ficha: u32, e: u32, f: u32) -> Result<u64, u32> {
    use bmo_gpu_ga10x::giro as gi;
    let mut r = Bar0(bar0);
    asegurar_mib(&mut r)?;
    memoria(FRACTAL_F.load(Ordering::Acquire), (gi::PIXELES * 4) as u64).fill(0);
    if !gi::preparar(&mut r, e, f) {
        crate::ring0::cabina::warn("gpu", "M5d G: el tramo no quedo preparado; no se toca el timbre", f as u64);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = gi::lanzar(&mut r, ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(e + 1, Ordering::Release);
    }
    let (mut qmd, mut fin) = (0, 0);
    let mut us = 0;
    while lanzado && us < FRACTAL_ESPERA_US {
        (_, qmd, fin) = gi::mirar(&mut r);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if qmd == gi::PAGA_QMD && fin == gi::PAGA_FIN {
            break;
        }
        core::hint::spin_loop();
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let cpu_desde = crate::ring0::task::scheduler::rdtsc();
    let buenos = pixeles_del_fractal().map_or(0, |p| gi::comprobar(p, f));
    let cpu_us = (crate::ring0::task::scheduler::rdtsc() - cpu_desde) / hz;
    let v = gi::empaquetar(buenos, qmd == gi::PAGA_QMD, fin == gi::PAGA_FIN, lanzado, us as u32, cpu_us as u32);
    if !gi::sano(v) {
        crate::ring0::cabina::warn("gpu", "M5d G: un fotograma no salio igual que la CPU; pixeles buenos", buenos as u64);
    }
    Ok(v)
}
