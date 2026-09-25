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
    // ** Con el PASE abierto su lienzo esta en la misma IOVA: prestar y
    // devolver aqui se lo quitaria a la 3060 por debajo.
    if crate::ring0::dev::pase_gpu::abierto() || EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_VOLCADO);
    }
    let r = volcado_(bar0, ficha, fisica, &p);
    EN_MARCHA.store(false, Ordering::Release);
    r
}

/// Las tablas del lienzo y la pantalla del GOP, mapeadas para la 3060 (una
/// vez por arranque).
pub(super) fn asegurar_mapas(r: &mut Bar0, p: &pa::Pantalla) -> Result<(), u32> {
    asegurar_mapa(r, p)?;
    if !MAPEADO.load(Ordering::Acquire) {
        match vl::mapear(r, p) {
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
    Ok(())
}

fn volcado_(bar0: u64, ficha: u32, fisica: u64, p: &pa::Pantalla) -> Result<u64, u32> {
    let mut r = Bar0(bar0);
    asegurar_mapas(&mut r, p)?;
    // Con el volcador ARMADO el lienzo ya esta prestado (el mismo, del mismo
    // propietario): ni se presta otra vez ni se devuelve al acabar.
    if ARMADO.load(Ordering::Acquire) {
        if FISICA.load(Ordering::Acquire) != fisica {
            return Err(IOMMU_NO_VOLCADO);
        }
        // La ultima tanda de cada fotograma, pagada: esta copia usa su
        // segmento de ordenes y su semaforo.
        let n = NUMERO.load(Ordering::Acquire);
        if n != 0 && !vl::pagada(&mut r, n) {
            esperar(&mut r, n)?;
        }
        return copiar(&mut r, ficha, fisica, p);
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

// == 1b: EL VOLCADO EN CADA FOTOGRAMA =========================================
//
// `ARMAR` presta el lienzo (SOLO LECTURA) para quedarse: vive lo que el propietario
// de la pantalla lo sea. Lo devuelven `SOLTAR`, soltar la pantalla
// (`obj::fb::release`, al prestarla a un juego) y la estacion `fb` del
// desmontaje (`obj::fb::process_died`), que va ANTES que `memory`: los
// marcos no se liberan con la 3060 viendolos. Los dos por [`suelta_si_es_de`].
//
// Cada fotograma el escritorio manda sus cajas sucias, una por llamada, y la
// ULTIMA cierra la tanda y toca el timbre -- y VUELVE (1c): la CPU sigue con lo
// suyo mientras la 3060 copia. La VALLA (que la 3060 pague el semaforo con el
// NUMERO de la tanda) se espera aparte, con ESPERAR, justo antes de que la CPU
// vuelva a escribir en el lienzo; y la tanda siguiente la espera tambien antes
// de pisar el segmento de ordenes y el semaforo. Una pantalla entera son ~1-2 ms por PCIe; unas letras,
// microsegundos -- contra los ~27 ms de la CPU moviendo 8 MiB.

/// El lienzo prestado para quedarse, de quien, y donde.
static ARMADO: AtomicBool = AtomicBool::new(false);
static PROPIETARIO: AtomicU32 = AtomicU32::new(0);
static FISICA: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
/// La pantalla del armado: `pitch | ancho << 16 | alto << 32` (la VRAM y el
/// color no hacen falta para copiar cajas).
static MEDIDAS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
/// El numero de la ultima tanda.
static NUMERO: AtomicU32 = AtomicU32::new(0);
/// Tandas ENVIADAS (el timbre tocado), para la fila; `TANDAS` cuenta las que
/// se vieron pagadas al esperarlas.
static ENVIADAS: AtomicU32 = AtomicU32::new(0);
static TANDAS: AtomicU32 = AtomicU32::new(0);
/// La tanda en construccion. Solo la toca quien tiene `EN_MARCHA`.
static mut TANDA: vl::Tanda = vl::Tanda::nueva();
/// B2 (25-09): los bytes de las cajas de la tanda que se esta juntando, los
/// de todas las enviadas, y cuantas veces la CPU espero a la anterior.
static POR_ENVIAR: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static BYTES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static ESPERAS: AtomicU32 = AtomicU32::new(0);
/// Lo mas que se espera a que la 3060 pague una tanda.
const TANDA_ESPERA_US: u64 = 50_000;

fn medidas() -> pa::Pantalla {
    let m = MEDIDAS.load(Ordering::Acquire);
    pa::Pantalla { vram: 0, pitch: m as u32 & 0xFFFF, ancho: (m >> 16) as u32 & 0xFFFF, alto: (m >> 32) as u32 & 0xFFFF, rgb: false }
}

/// **`IOMMU_OP_GPU_VOLCADOR`**: `ARMAR`, `CAJA` o `SOLTAR` (bits 63..60).
pub fn volcador(arg: u64) -> Result<u64, u32> {
    let pid = crate::ring0::task::scheduler::current_pid();
    match vl::suborden(arg) {
        vl::ARMAR => armar(pid, vl::lienzo_de(arg)),
        vl::CAJA => caja(pid, arg),
        vl::SOLTAR => soltar(pid).map(|()| 0),
        vl::COMO_VA => Ok(como_va(vl::como_va_de(arg))),
        vl::ESPERAR => valla(pid),
        _ => Err(IOMMU_NO_VOLCADO),
    }
}

fn armar(pid: u32, lienzo: u64) -> Result<u64, u32> {
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_VOLCADO) };
    if ARMADO.load(Ordering::Acquire) {
        if PROPIETARIO.load(Ordering::Acquire) != pid {
            return Err(IOMMU_NO_VOLCADO);
        }
        // El mismo lienzo: ya esta. OTRO (un lienzo nuevo del mismo propietario):
        // se devuelve el viejo antes de prestar el nuevo.
        if crate::ring0::obj::memory::fisica_de(pid, lienzo, p.bytes()) == Some(FISICA.load(Ordering::Acquire)) {
            return Ok(TANDAS.load(Ordering::Acquire) as u64);
        }
        soltar(pid)?;
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0
        || crate::ring0::dev::gpu_libos::timbre_de_copia().is_none()
        || !vl::cabe(&p)
        || !crate::ring0::dev::gpu_despertar::bar1_fisica()
    {
        return Err(IOMMU_NO_VOLCADO);
    }
    let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, lienzo, p.bytes()) else {
        return Err(IOMMU_NO_VOLCADO);
    };
    // ** El pase y el volcador prestan la misma IOVA: uno a la vez.
    if fisica % PAGINA != 0 || crate::ring0::dev::pase_gpu::abierto() || EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_VOLCADO);
    }
    let mut r = Bar0(bar0);
    let hecho = asegurar_mapas(&mut r, &p).and_then(|()| {
        io::prestar_gpu(vl::IOVA, fisica, vl::paginas(&p), false).map_err(|_| IOMMU_NO_VOLCADO)?;
        if !vl::invalidar_mmu(&mut r) {
            let _ = io::devolver_gpu(vl::IOVA, vl::paginas(&p));
            return Err(IOMMU_NO_VOLCADO);
        }
        Ok(())
    });
    if hecho.is_ok() {
        FISICA.store(fisica, Ordering::Release);
        MEDIDAS.store(p.pitch as u64 | (p.ancho as u64) << 16 | (p.alto as u64) << 32, Ordering::Release);
        PROPIETARIO.store(pid, Ordering::Release);
        // SAFETY: con `EN_MARCHA` tomado nadie mas toca la tanda.
        unsafe { *core::ptr::addr_of_mut!(TANDA) = vl::Tanda::nueva() };
        ARMADO.store(true, Ordering::Release);
        crate::ring0::cabina::count("gpu", "volcador ARMADO: la 3060 lleva el escritorio a la pantalla en cada fotograma; pid", pid as u64);
    }
    EN_MARCHA.store(false, Ordering::Release);
    hecho.map(|()| 0)
}

/// **Una caja sucia**; la ultima cierra la tanda y toca el timbre, SIN esperar.
/// `Ok(numero de la tanda)` (0 si no era la ultima).
fn caja(pid: u32, arg: u64) -> Result<u64, u32> {
    if !ARMADO.load(Ordering::Acquire) || PROPIETARIO.load(Ordering::Acquire) != pid {
        return Err(IOMMU_NO_VOLCADO);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    let Some(ficha) = crate::ring0::dev::gpu_libos::timbre_de_copia() else { return Err(IOMMU_NO_VOLCADO) };
    if bar0 == 0 || EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_VOLCADO);
    }
    let (x, y, w, h, ultima) = vl::caja_de(arg);
    let p = medidas();
    // SAFETY: con `EN_MARCHA` tomado nadie mas toca la tanda.
    let t = unsafe { &mut *core::ptr::addr_of_mut!(TANDA) };
    let r = if !t.caja(&p, x, y, w, h) {
        *t = vl::Tanda::nueva();
        POR_ENVIAR.store(0, Ordering::Release);
        Err(IOMMU_NO_VOLCADO)
    } else if !ultima {
        POR_ENVIAR.fetch_add(w as u64 * h as u64 * 4, Ordering::AcqRel);
        Ok(0)
    } else {
        let bytes = POR_ENVIAR.swap(0, Ordering::AcqRel) + w as u64 * h as u64 * 4;
        let mut regs = Bar0(bar0);
        // La de antes, pagada: esta pisa su segmento de ordenes y su semaforo.
        let antes = NUMERO.load(Ordering::Acquire);
        let pagada = antes == 0 || vl::pagada(&mut regs, antes);
        if !pagada {
            ESPERAS.fetch_add(1, Ordering::AcqRel);
        }
        let libre = pagada || esperar(&mut regs, antes).is_ok();
        let numero = antes.wrapping_add(1).max(1);
        let e = ENTRADA.load(Ordering::Acquire);
        let enviada = libre && t.cerrar(numero) && vl::enviar(&mut regs, t, e, ficha);
        *t = vl::Tanda::nueva();
        if enviada {
            NUMERO.store(numero, Ordering::Release);
            ENVIADAS.fetch_add(1, Ordering::AcqRel);
            BYTES.fetch_add(bytes, Ordering::AcqRel);
            ENTRADA.store(vl::siguiente(e), Ordering::Release);
            Ok(numero as u64)
        } else {
            Err(IOMMU_NO_VOLCADO)
        }
    };
    EN_MARCHA.store(false, Ordering::Release);
    r
}

/// **ESPERAR**: la valla de la ultima tanda, desde el escritorio. `Ok(us)`
/// que hubo que esperar (casi siempre 0: la 3060 ya acabo).
fn valla(pid: u32) -> Result<u64, u32> {
    if !ARMADO.load(Ordering::Acquire) || PROPIETARIO.load(Ordering::Acquire) != pid {
        return Err(IOMMU_NO_VOLCADO);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    let numero = NUMERO.load(Ordering::Acquire);
    if bar0 == 0 || numero == 0 {
        return Ok(0);
    }
    let mut r = Bar0(bar0);
    if vl::pagada(&mut r, numero) {
        return Ok(0);
    }
    esperar(&mut r, numero)
}

/// **La valla**: girar hasta que la 3060 pague `numero`. `Ok(us)`.
fn esperar(r: &mut Bar0, numero: u32) -> Result<u64, u32> {
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    loop {
        if vl::pagada(r, numero) {
            TANDAS.fetch_add(1, Ordering::AcqRel);
            return Ok((crate::ring0::task::scheduler::rdtsc() - desde) / hz);
        }
        if (crate::ring0::task::scheduler::rdtsc() - desde) / hz > TANDA_ESPERA_US {
            crate::ring0::cabina::warn("gpu", "volcador: la 3060 no pago la tanda a tiempo; numero", numero as u64);
            return Err(IOMMU_NO_VOLCADO);
        }
        core::hint::spin_loop();
    }
}

/// **Soltar el lienzo**: se espera la ultima tanda y se devuelve el prestamo.
fn soltar(pid: u32) -> Result<(), u32> {
    if !ARMADO.load(Ordering::Acquire) || PROPIETARIO.load(Ordering::Acquire) != pid {
        return Err(IOMMU_NO_VOLCADO);
    }
    let p = medidas();
    let bar0 = crate::ring0::dev::gpu::bar0();
    let numero = NUMERO.load(Ordering::Acquire);
    if bar0 != 0 && numero != 0 && !vl::pagada(&mut Bar0(bar0), numero) {
        // Nada en vuelo cuando se devuelve (la ultima tanda, o su plazo).
        let _ = esperar(&mut Bar0(bar0), numero);
    }
    ARMADO.store(false, Ordering::Release);
    let _ = io::devolver_gpu(vl::IOVA, vl::paginas(&p));
    crate::ring0::cabina::count("gpu", "volcador SUELTO: el lienzo devuelto por la 3060; tandas", TANDAS.load(Ordering::Acquire) as u64);
    Ok(())
}

/// **El propietario de la pantalla la suelta o muere**: si era el del volcador, su
/// lienzo se devuelve AQUI -- al morir, antes de que `memory` libere sus
/// marcos.
pub fn suelta_si_es_de(pid: u32) {
    if ARMADO.load(Ordering::Acquire) && PROPIETARIO.load(Ordering::Acquire) == pid {
        let _ = soltar(pid);
    }
}

/// **X5: nada del volcado en vuelo** -- la ultima tanda pagada (o su plazo).
/// Lo pide quien va a escribir en la pantalla por otro camino (el cubo) y a
/// leerla despues: una copia en vuelo podria caerle encima.
pub(super) fn quieto() -> Result<(), u32> {
    let bar0 = crate::ring0::dev::gpu::bar0();
    let numero = NUMERO.load(Ordering::Acquire);
    if !ARMADO.load(Ordering::Acquire) || bar0 == 0 || numero == 0 {
        return Ok(());
    }
    let mut r = Bar0(bar0);
    if vl::pagada(&mut r, numero) {
        return Ok(());
    }
    esperar(&mut r, numero).map(|_| ())
}

/// El volcador tiene un lienzo prestado: el pase dice "ocupado".
pub(super) fn armado() -> bool {
    ARMADO.load(Ordering::Acquire)
}

/// Para la fila, segun el selector (`vl::COMO_VA_*`): `enviadas | armado <<
/// 32`, los bytes copiados, o las veces que la CPU tuvo que esperar.
pub fn como_va(sel: u64) -> u64 {
    match sel {
        vl::COMO_VA_BYTES => BYTES.load(Ordering::Acquire),
        vl::COMO_VA_ESPERAS => ESPERAS.load(Ordering::Acquire) as u64,
        _ => ENVIADAS.load(Ordering::Acquire) as u64 | (ARMADO.load(Ordering::Acquire) as u64) << 32,
    }
}
