//! **VE5: COMPLETAR POR LA 3060** -- una pasada del filtro de ESTUDIO VIVO
//! (el que quita el grano guiado por la superficie) sobre un bloque de quien
//! lo pide, por un programa de computo. Lo que se cuenta y como esta en
//! `bmo_gpu_ga10x::completa`; aqui se presta el bloque, se toca el timbre y
//! se espera. Lo que escribio la 3060 lo JUZGA quien presto el bloque (el
//! DIRECTOR, con la cuenta de VERRANO): el kernel no hace esa cuenta.
//!
//! [carril]  ROJO      la 3060 LEE y ESCRIBE memoria de un proceso (un
//!                     bloque suyo que el kernel reconoce, `fisica_de`, y
//!                     solo ese: el mapa de `video` no ve nada mas)
//! [consumo] NADA      corre por orden (`gpu completa`); una pasada a 320x240
//!                     son 1200 paginas prestadas
//!
//! # El prestamo dura UNA llamada
//!
//! Como `imagen`: se presta al entrar y se devuelve antes de volver a Ring 3,
//! salga como salga. El bloque nunca queda visto por la 3060 si el proceso
//! muere o lo suelta (R-DMA-3).

use core::sync::atomic::{AtomicU64, Ordering};

use bmo_gpu_ga10x::completa as co;

use super::pantalla::la_pantalla;
use super::video::asegurar_mapas;
use super::{esperando, BLUR_ENTRADA, BLUR_EN_MARCHA, FRACTAL_ESPERA_US, LIENZO_HECHO, PAGINA};
use crate::ring0::dev::gpu_prestamo::Bar0;
use crate::ring0::plat::iommu as io;

/// Completar no se puede: sin el lienzo de M5d (el canal de GR y su ficha),
/// sin la pantalla del GOP en modo fisico, un formato que no vale o no cabe,
/// un bloque que no es de quien lo pide, un paso de mas, o una ya en marcha.
pub const IOMMU_NO_COMPLETA: u32 = 92;

/// Bits de `arg` de [`completa`]: el paso (los vecinos a `2^paso`), de que
/// luz se lee, si se carga el programa, y la tolerancia (16 bits de arriba
/// de su f32) desde el bit [`TOLERANCIA`].
pub const PASO: u64 = 0x7;
pub const DE_B: u64 = 1 << 3;
pub const CARGAR: u64 = 1 << 4;
pub const TOLERANCIA: u32 = 48;
/// La VA del bloque: alineada a pagina, por debajo del bit 48.
pub const VA: u64 = 0x0000_FFFF_FFFF_F000;

/// El formato de la tanda: `ancho | alto << 16 | ficha << 32`; 0 = ninguno.
static FORMATO: AtomicU64 = AtomicU64::new(0);

fn formato() -> Option<(co::Formato, u32)> {
    let v = FORMATO.load(Ordering::Acquire);
    let f = co::Formato { ancho: (v & 0xFFFF) as u32, alto: (v >> 16 & 0xFFFF) as u32 };
    (v != 0 && f.valido()).then_some((f, (v >> 32) as u32))
}

/// **El formato de la tanda.** `arg` = ancho (0..15), alto (16..31) y la
/// ficha de S3 (32..63). `Ok(bytes del bloque)`.
pub fn completa_formato(arg: u64) -> Result<u64, u32> {
    let f = co::Formato { ancho: (arg & 0xFFFF) as u32, alto: (arg >> 16 & 0xFFFF) as u32 };
    if !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(arg >> 32) || !f.valido() {
        return Err(IOMMU_NO_COMPLETA);
    }
    FORMATO.store(arg, Ordering::Release);
    Ok(f.bytes())
}

/// **Una pasada.** `arg` = la VA del bloque (de quien llama, [`VA`]) con
/// [`PASO`], [`DE_B`], [`CARGAR`] y la tolerancia. `Ok(completa::empaquetar(..))`
/// sin muestras: lo que salio lo juzga quien llama.
pub fn completa(arg: u64) -> Result<u64, u32> {
    let bar0 = crate::ring0::dev::gpu::bar0();
    let Some((f, ficha)) = formato() else { return Err(IOMMU_NO_COMPLETA) };
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_COMPLETA) };
    let paso = (arg & PASO) as u32;
    if bar0 == 0 || !crate::ring0::dev::gpu_despertar::bar1_fisica() || paso > co::MAX_PASO {
        return Err(IOMMU_NO_COMPLETA);
    }
    let pid = crate::ring0::task::scheduler::current_pid();
    let va = arg & VA;
    let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, va, f.bytes()) else {
        crate::ring0::cabina::warn("gpu", "completa: el bloque no es de quien lo pide", va);
        return Err(IOMMU_NO_COMPLETA);
    };
    let paginas = f.paginas();
    if fisica % PAGINA != 0 || fisica + paginas * PAGINA > crate::ring0::mm::PHYSMAP_SIZE {
        return Err(IOMMU_NO_COMPLETA);
    }
    let en = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bmo_gpu_ga10x::blur::entrada_valida(en) || super::gr_ocupado() {
        return Err(IOMMU_NO_COMPLETA);
    }
    let tolerancia = co::tolerancia_de((arg >> TOLERANCIA) as u16);
    let parametros = co::parametros(&f, paso, arg & DE_B != 0, tolerancia);
    let r = completa_(bar0, ficha, en, (fisica, paginas), &f, &parametros, &p, arg & CARGAR != 0);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

#[allow(clippy::too_many_arguments)]
fn completa_(bar0: u64, ficha: u32, en: u32, bloque: (u64, u64), f: &co::Formato, parametros: &[u32; co::N_PARAMETROS], p: &bmo_gpu_ga10x::pantalla::Pantalla, cargar: bool) -> Result<u64, u32> {
    let (fisica, paginas) = bloque;
    let mut r = Bar0(bar0);
    let nuevo = asegurar_mapas(&mut r, p).map_err(|_| IOMMU_NO_COMPLETA)?;
    // la 3060 ESCRIBE la otra luz: prestado para escribir
    if io::prestar_gpu(co::IOVA, fisica, paginas, true).is_err() {
        crate::ring0::cabina::warn("gpu", "completa: el bloque no se pudo prestar a la 3060; fisica", fisica);
        return Err(IOMMU_NO_COMPLETA);
    }
    let preparado = co::preparar(&mut r, en, f, parametros, cargar || nuevo);
    let (mut lanzado, mut qmd, mut fin, mut us) = (false, 0, 0, 0);
    if preparado {
        core::sync::atomic::fence(Ordering::SeqCst);
        let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
        let desde = crate::ring0::task::scheduler::rdtsc();
        lanzado = co::lanzar(&mut r, ficha, en);
        if lanzado {
            BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(en), Ordering::Release);
        }
        while lanzado && us < FRACTAL_ESPERA_US {
            (_, qmd, fin) = co::mirar(&mut r);
            us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
            if qmd == co::PAGA_QMD && fin == co::PAGA_FIN {
                break;
            }
            esperando(us);
        }
        super::vigilante::vigilar("E7: una pasada de COMPLETAR no volvio en su plazo; us", lanzado, qmd == co::PAGA_QMD && fin == co::PAGA_FIN, us, FRACTAL_ESPERA_US);
    } else {
        crate::ring0::cabina::warn("gpu", "completa: el tramo no quedo preparado; no se toca el timbre", 0);
    }
    // Devuelto SIEMPRE, salga como salga la pasada.
    if io::devolver_gpu(co::IOVA, paginas).is_err() {
        crate::ring0::cabina::warn("gpu", "completa: el bloque NO se pudo devolver (la invalidacion no contesto)", paginas);
    }
    if !preparado {
        return Err(IOMMU_NO_COMPLETA);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let v = co::empaquetar(0, qmd == co::PAGA_QMD, fin == co::PAGA_FIN, lanzado, us as u32, 0);
    if !co::sano(v) {
        crate::ring0::cabina::warn("gpu", "completa: la pasada no se pago; us", us);
    }
    Ok(v)
}
