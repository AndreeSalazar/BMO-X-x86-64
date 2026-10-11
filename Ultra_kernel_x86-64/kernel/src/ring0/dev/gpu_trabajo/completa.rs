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
//! # Dos puertas
//!
//! [`completa`] (detras de las dos llaves: el DIRECTOR, `gpu completa`, que
//! juzga cada pasada) y [`completar_de_app`] (VEL2: la puerta ESTRECHA de
//! las apps, como la de las recetas: el programa es el del kernel, el bloque
//! suyo; todas las pasadas en una llamada).
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
/// luz se lee, y la tolerancia (16 bits de arriba de su f32) desde el bit
/// [`TOLERANCIA`]. [`CARGAR`] se acepta y ya no hace falta: el programa se
/// carga SIEMPRE en la primera pasada (VEL2).
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
    let r = pasadas(bar0, ficha, (fisica, paginas), &f, &[parametros], &p);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

/// **LA PUERTA DE LAS APPS (VEL2, 11-10)**: una APP -- sin la autoridad
/// MAQUINA ni la pantalla -- completa SU imagen en la 3060. Es segura por lo
/// mismo que la de las recetas: el PROGRAMA es el del kernel (el de
/// `bmo_gpu_ga10x::completa`, juzgado; la app no manda codigo), el bloque es
/// SUYO (`fisica_de` con su pid) y lo ve la 3060 solo durante la llamada, y
/// el canal de GR se toma con el mismo cerrojo que todo (`gr_ocupado`).
/// `va` = su bloque: la cabecera (`co::cabecera`) y detras el de VERRANO.
/// Todas las pasadas en UNA llamada (el bloque se presta una vez).
/// `Ok(completa::empaquetar(pasadas hechas, ..))`.
pub fn completar_de_app(va: u64) -> Result<u64, u32> {
    let bar0 = crate::ring0::dev::gpu::bar0();
    let Some(ficha) = super::ficha_del_gr() else {
        crate::ring0::cabina::warn("gpu", "VEL2: una app quiere completar y el escritorio no preparo la 3060 (`gpu lienzo`)", va);
        return Err(IOMMU_NO_COMPLETA);
    };
    if bar0 == 0 || crate::ring0::dev::gpu_apagar::despedido() || !crate::ring0::dev::gpu_despertar::bar1_fisica() || !LIENZO_HECHO.load(Ordering::Acquire) {
        return Err(IOMMU_NO_COMPLETA);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_COMPLETA) };
    let pid = crate::ring0::task::scheduler::current_pid();
    let va = va & VA;
    let dentro = |fisica: u64, bytes: u64| fisica % PAGINA == 0 && fisica.checked_add(bytes).is_some_and(|fin| fin <= crate::ring0::mm::PHYSMAP_SIZE);
    // la cabecera: una pagina de QUIEN pide, dentro del physmap
    let Some(fc) = crate::ring0::obj::memory::fisica_de(pid, va, co::CABECERA).filter(|&f| dentro(f, co::CABECERA)) else {
        crate::ring0::cabina::warn("gpu", "VEL2: el bloque no es de quien pide completar", va);
        return Err(IOMMU_NO_COMPLETA);
    };
    // SAFETY: `fisica_de` dio la pagina en un bloque del proceso, dentro del
    // physmap (comprobado); solo se leen 5 palabras alineadas.
    let palabras = unsafe { *(crate::ring0::mm::phys_to_virt(fc) as *const [u32; 5]) };
    let Some((f, n, tolerancia)) = co::cabecera(&palabras) else {
        crate::ring0::cabina::warn("gpu", "VEL2: la cabecera de completar no se sostiene (palabra, medidas, pasadas o tolerancia)", va);
        return Err(IOMMU_NO_COMPLETA);
    };
    let total = co::CABECERA + f.bytes();
    let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, va, total).filter(|&f| dentro(f, total)) else {
        crate::ring0::cabina::warn("gpu", "VEL2: el bloque no es entero de quien pide completar", va);
        return Err(IOMMU_NO_COMPLETA);
    };
    if super::gr_ocupado() {
        return Err(IOMMU_NO_COMPLETA);
    }
    let mut lista = [[0u32; co::N_PARAMETROS]; co::MAX_PASADAS as usize];
    for k in 0..n {
        let (paso, de_b, t) = co::pasada(k, tolerancia);
        lista[k as usize] = co::parametros_desde(&f, paso, de_b, t, co::CABECERA);
    }
    let r = pasadas(bar0, ficha, (fisica, total.div_ceil(PAGINA)), &f, &lista[..n as usize], &p);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

/// **Las pasadas**, una tras otra, con el bloque prestado UNA vez: cada una
/// espera a la anterior (lee lo que escribio). El programa se carga en la
/// primera SIEMPRE: la pagina del programa es de quien la uso el ultimo.
fn pasadas(bar0: u64, ficha: u32, bloque: (u64, u64), f: &co::Formato, lista: &[[u32; co::N_PARAMETROS]], p: &bmo_gpu_ga10x::pantalla::Pantalla) -> Result<u64, u32> {
    let (fisica, paginas) = bloque;
    let mut r = Bar0(bar0);
    asegurar_mapas(&mut r, p).map_err(|_| IOMMU_NO_COMPLETA)?;
    // la 3060 ESCRIBE la otra luz: prestado para escribir
    if io::prestar_gpu(co::IOVA, fisica, paginas, true).is_err() {
        crate::ring0::cabina::warn("gpu", "completa: el bloque no se pudo prestar a la 3060; fisica", fisica);
        return Err(IOMMU_NO_COMPLETA);
    }
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let (mut hechas, mut us_todo, mut sano) = (0u32, 0u64, true);
    let (mut qmd_bien, mut fin_bien, mut lanzado_bien) = (true, true, true);
    for (k, parametros) in lista.iter().enumerate() {
        let en = BLUR_ENTRADA.load(Ordering::Acquire);
        // el programa, en la primera pasada SIEMPRE: otro trabajo pudo
        // dejar el suyo en la pagina desde la ultima vez
        if !bmo_gpu_ga10x::blur::entrada_valida(en) || !co::preparar(&mut r, en, f, parametros, k == 0) {
            crate::ring0::cabina::warn("gpu", "completa: el tramo no quedo preparado; no se toca el timbre", k as u64);
            sano = false;
            break;
        }
        core::sync::atomic::fence(Ordering::SeqCst);
        let desde = crate::ring0::task::scheduler::rdtsc();
        let lanzado = co::lanzar(&mut r, ficha, en);
        if lanzado {
            BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(en), Ordering::Release);
        }
        let (mut qmd, mut fin, mut us) = (0, 0, 0);
        while lanzado && us < FRACTAL_ESPERA_US {
            (_, qmd, fin) = co::mirar(&mut r);
            us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
            if qmd == co::PAGA_QMD && fin == co::PAGA_FIN {
                break;
            }
            esperando(us);
        }
        let pagada = qmd == co::PAGA_QMD && fin == co::PAGA_FIN;
        super::vigilante::vigilar("E7: una pasada de COMPLETAR no volvio en su plazo; us", lanzado, pagada, us, FRACTAL_ESPERA_US);
        us_todo += us;
        (qmd_bien, fin_bien, lanzado_bien) = (qmd_bien && qmd == co::PAGA_QMD, fin_bien && fin == co::PAGA_FIN, lanzado_bien && lanzado);
        if !pagada {
            sano = false;
            break;
        }
        hechas += 1;
    }
    // Devuelto SIEMPRE, salga como salga.
    if io::devolver_gpu(co::IOVA, paginas).is_err() {
        crate::ring0::cabina::warn("gpu", "completa: el bloque NO se pudo devolver (la invalidacion no contesto)", paginas);
    }
    if hechas == 0 && !sano {
        return Err(IOMMU_NO_COMPLETA);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let v = co::empaquetar(hechas, qmd_bien, fin_bien, lanzado_bien, us_todo as u32, 0);
    if !co::sano(v) || !sano {
        crate::ring0::cabina::warn("gpu", "completa: una pasada no se pago; hechas", hechas as u64);
    }
    Ok(v)
}
