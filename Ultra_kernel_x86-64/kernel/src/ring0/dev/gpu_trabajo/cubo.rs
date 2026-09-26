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
//!             codigo tal cual, no traduce nada. Ok como DIBUJAR, con lo que
//!             costo preparar (`cubo::con_preparar`)
//!    LIGERO   con VERRANO: las ordenes SIN la escalera de T1c (V1)
//!    ANILLO   con VERRANO: EL ANILLO (V1b, `bmo_gpu_ga10x::anillo`): los
//!             vertices en RAM del PC y el fotograma EN VUELO -- se envia y
//!             se vuelve, sin esperar a la 3060 (`cubo::es_en_vuelo`)
//!    VACIAR   arg = CUBO_VACIAR: esperar lo que el anillo dejo en vuelo
//! ```
//!
//! ** EL ANILLO (V1b, 26-09): la CPU ORQUESTA, no espera. El primer
//! fotograma lo ARMA en frio (todo releido, y se espera a que se pague);
//! los siguientes, mientras sean los mismos programas y la misma ventana y
//! nadie mas pase por el GR, solo copian sus vertices a su ranura de RAM,
//! escriben la cola de sus ordenes y tocan el timbre. Cuatro ranuras: se
//! espera solo si la 3060 va CUATRO fotogramas por detras. Cualquier otro
//! trabajo del GR (`gr_ocupado`) y LEER esperan antes a que se vacie: el
//! anillo comparte con ellos el tramo (programas, tabla, ordenes).
//!
//! ** EN CALIENTE (V1, 26-09): un dibujo de VERRANO con los MISMOS
//! programas, triangulos, ventana y modo que el anterior, sin que nadie haya
//! lanzado nada por el GR entre medias y a menos de 100 ms, solo escribe lo
//! que cambia (`tuberia::preparar_caliente`). Lo decide ESTE fichero, no el
//! escritorio: el escritorio no puede pedir "en caliente".
//!
//! La lectura va de dos en dos y no la ventana entera en una llamada por lo
//! mismo que D2 (`pantalla::FILA`): un syscall corre con las interrupciones
//! cerradas, y 921.600 pixeles leidos de la VRAM son demasiado tiempo sin
//! reloj.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use bmo_gpu_ga10x::cubo as cu;

use super::pantalla::{asegurar_mapa, la_pantalla, IOMMU_NO_PANTALLA};
use super::{esperando, gr_ocupado, BLUR_ENTRADA, BLUR_EN_MARCHA, DIAG_3D, FRACTAL_ESPERA_US, IOMMU_NO_BLUR, IOMMU_NO_BLUR_PREPARAR, LIENZO_HECHO};
use crate::ring0::dev::gpu_prestamo::Bar0;

/// El bit de `arg` que pide LEER en vez de DIBUJAR.
pub const CUBO_LEER: u64 = 1 << 63;

/// El bit de `arg` que pide dibujar un paquete de VERRANO V0.
pub const CUBO_VERRANO: u64 = 1 << 62;

/// Con VERRANO: las ordenes sin la escalera (`tuberia::ordenes_con`).
pub const CUBO_LIGERO: u64 = 1 << 61;

/// Con VERRANO: el anillo (V1b).
pub const CUBO_ANILLO: u64 = 1 << 60;

/// Solo: vaciar el anillo.
pub const CUBO_VACIAR: u64 = 1 << 59;

/// **TOMA TU BODRIO** en la puerta: un programa del paquete de VERRANO que
/// el juez del SASS (`bmo_gpu_ga10x::sass::juez`) rechaza no se sube.
pub const IOMMU_NO_BODRIO: u32 = 87;

/// Lo que dejo el ultimo dibujo de VERRANO pagado entero: la huella de lo
/// fijo (0 = nada que reusar), la entrada del GR tras el, y cuando.
static CALIENTE_HUELLA: AtomicU64 = AtomicU64::new(0);
static CALIENTE_ENTRADA: AtomicU32 = AtomicU32::new(u32::MAX);
static CALIENTE_TSC: AtomicU64 = AtomicU64::new(0);
/// La huella de los programas del ultimo dibujo pagado (ya juzgados).
static CALIENTE_PROGRAMAS: AtomicU64 = AtomicU64::new(0);

/// EL ANILLO: su pagina de RAM (los vertices de las cuatro ranuras) y si ya
/// se presto y mapeo (una vez por arranque).
static ANILLO_F: AtomicU64 = AtomicU64::new(0);
static ANILLO_PRESTADO: AtomicBool = AtomicBool::new(false);
/// La huella de lo fijo del anillo armado (0 = sin armar), la entrada del GR
/// tras su ultimo envio, y cuando.
static ANILLO_HUELLA: AtomicU64 = AtomicU64::new(0);
static ANILLO_ENTRADA: AtomicU32 = AtomicU32::new(u32::MAX);
static ANILLO_TSC: AtomicU64 = AtomicU64::new(0);
/// El numero del ultimo fotograma ENVIADO y el del ultimo visto PAGADO.
static ANILLO_NUMERO: AtomicU32 = AtomicU32::new(0);
static ANILLO_PAGADO: AtomicU32 = AtomicU32::new(0);

/// Ya se dibujo en este arranque (LEER antes no tiene que leer).
static DIBUJADO: AtomicBool = AtomicBool::new(false);
/// Cuantos fotogramas del cubo dibujo la 3060 (para el log).
static VECES: AtomicU32 = AtomicU32::new(0);

/// **`IOMMU_OP_GPU_CUBO`**.
pub fn cubo(arg: u64) -> Result<u64, u32> {
    if arg & CUBO_LEER != 0 {
        return leer(arg & 0xF_FFFF);
    }
    if arg == CUBO_VACIAR {
        return vaciar_con_cerrojo();
    }
    if arg & CUBO_VERRANO != 0 {
        return verrano(arg & 0xFFFF_FFFF_FFFF, arg & CUBO_LIGERO != 0, arg & CUBO_ANILLO != 0);
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
    if !bmo_gpu_ga10x::blur::entrada_valida(e) || gr_ocupado() {
        return Err(IOMMU_NO_BLUR);
    }
    // Lo que el volcado del escritorio tenga en vuelo, antes: si no, su copia
    // podria caer ENCIMA del cubo entre el dibujo y la lectura.
    let tris = &tris[..t.n];
    let r = super::volcado::quieto().and_then(|()| dibujar(bar0, ficha as u32, e, &p, t.n as u32, true, |r| cu::preparar(r, e, &v, tris)));
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

/// **VERRANO V0**: el paquete del escritorio (sus dos programas y sus
/// vertices) por la tuberia fija. Con `anillo`, por el anillo (V1b).
fn verrano(va: u64, ligero: bool, anillo: bool) -> Result<u64, u32> {
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
    // ** EL JUEZ EN LA PUERTA (J2, 26-09): lo que el escritorio diga haber
    // juzgado no cuenta; aqui se juzga otra vez, con los registros que las
    // ordenes le dan (`raster::REGISTROS`), y un BODRIO no llega a la 3060.
    // En caliente no hace falta: la huella de lo fijo lleva los programas,
    // y los de un dibujo pagado ya pasaron por aqui. En el anillo, igual:
    // los de un anillo ARMADO ya pasaron por aqui al armarlo.
    let huella_anillo = if anillo { bmo_gpu_ga10x::anillo::huella(&v, &paquete) } else { 0 };
    let ya_juzgados = if anillo { ANILLO_HUELLA.load(Ordering::Acquire) == huella_anillo } else { caliente_posible(&paquete) };
    if !ya_juzgados {
        let r = bmo_gpu_ga10x::raster::REGISTROS;
        for (cual, prog) in [("vertice", paquete.vs), ("pixel", paquete.ps)] {
            if let Err(b) = bmo_gpu_ga10x::sass::juez::juzgar_programa(prog, r) {
                crate::ring0::cabina::warn("gpu", cual, b.instruccion as u64);
                crate::ring0::cabina::warn("gpu", b.regla.nombre(), b.que as u64);
                crate::ring0::cabina::warn("gpu", "[BMO-X Juez V3b]: TOMA TU BODRIO! no sube a la 3060; instruccion", b.instruccion as u64);
                crate::ring0::cabina::warn("gpu", bmo_gpu_ga10x::sass::juez::REMATE, 87);
                return Err(IOMMU_NO_BODRIO);
            }
        }
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if anillo {
        // El anillo toma el cerrojo SIN vaciarse a si mismo: es el unico que
        // puede seguir con fotogramas suyos en vuelo.
        if !bmo_gpu_ga10x::blur::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
            return Err(IOMMU_NO_BLUR);
        }
        let r = en_anillo(bar0, e, &p, &v, &paquete, huella_anillo);
        BLUR_EN_MARCHA.store(false, Ordering::Release);
        return r;
    }
    if !bmo_gpu_ga10x::blur::entrada_valida(e) || gr_ocupado() {
        return Err(IOMMU_NO_BLUR);
    }
    let n = (paquete.vertices.len() / tu::BYTES_VERTICE / 3) as u32;
    // En caliente, o no: lo fijo es lo mismo, nadie lanzo nada por el GR
    // desde nuestro ultimo dibujo (la entrada sigue donde la dejamos) y fue
    // hace menos de 100 ms (entre dos fotogramas de un banco, no entre dos
    // ordenes tecleadas: un preparar de otro trabajo que fallo a medias no
    // mueve la entrada, pero tampoco cabe en ese hueco). Y se olvida ANTES
    // de dibujar: si este sale mal, el siguiente va en frio.
    let huella = tu::huella_fija(&v, &paquete, ligero);
    let tsc = crate::ring0::task::scheduler::tsc_freq().max(1);
    let ahora = crate::ring0::task::scheduler::rdtsc();
    let caliente = CALIENTE_HUELLA.swap(0, Ordering::AcqRel) == huella
        && CALIENTE_ENTRADA.load(Ordering::Acquire) == e
        && ahora.wrapping_sub(CALIENTE_TSC.load(Ordering::Acquire)) < tsc / 10;
    let mut preparar_ciclos = 0u64;
    let preparar = |r: &mut Bar0| {
        let desde = crate::ring0::task::scheduler::rdtsc();
        let bien = if caliente { tu::preparar_caliente(r, e, &v, &paquete, ligero) } else { tu::preparar_con(r, e, &v, &paquete, ligero) };
        preparar_ciclos = crate::ring0::task::scheduler::rdtsc() - desde;
        bien
    };
    let r = super::volcado::quieto().and_then(|()| dibujar(bar0, paquete.ficha, e, &p, n, !ligero, preparar));
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    let r = r.map(|x| cu::con_preparar(x, caliente, preparar_ciclos * 1_000_000 / tsc));
    if let Ok(x) = r {
        if cu::sano(x) {
            CALIENTE_ENTRADA.store(BLUR_ENTRADA.load(Ordering::Acquire), Ordering::Release);
            CALIENTE_TSC.store(crate::ring0::task::scheduler::rdtsc(), Ordering::Release);
            CALIENTE_PROGRAMAS.store(huella_programas(&paquete), Ordering::Release);
            CALIENTE_HUELLA.store(huella, Ordering::Release);
        }
    }
    r
}

/// Si este paquete PODRIA ir en caliente (sus programas son los del ultimo
/// dibujo pagado): solo mira la huella, sin gastarla -- la decision de verdad
/// la toma `verrano` mas abajo, con la entrada y el reloj.
fn caliente_posible(p: &bmo_gpu_ga10x::tuberia::Paquete) -> bool {
    let h = CALIENTE_HUELLA.load(Ordering::Acquire);
    h != 0 && CALIENTE_PROGRAMAS.load(Ordering::Acquire) == huella_programas(p)
}

/// La huella de los DOS programas solos (FNV-1a), para saber sin la
/// ventana si ya se juzgaron.
fn huella_programas(p: &bmo_gpu_ga10x::tuberia::Paquete) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in p.vs.iter().chain([0xA5u8].iter()).chain(p.ps.iter()) {
        h = (h ^ x as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// == V1b: EL ANILLO ===========================================================

/// Si el anillo tiene fotogramas enviados que aun no se vieron pagados.
fn en_vuelo() -> bool {
    !bmo_gpu_ga10x::anillo::ya(ANILLO_PAGADO.load(Ordering::Acquire), ANILLO_NUMERO.load(Ordering::Acquire))
}

/// **Vaciar el anillo** (con el cerrojo del GR ya tomado): esperar a que la
/// 3060 pague el ultimo fotograma enviado. `Ok(us esperados)`. Si no llega
/// en el plazo se da por perdido -- lo dice la cabina -- y el anillo se
/// olvida: nadie se queda esperando a un fotograma que no va a llegar.
fn vaciar(r: &mut Bar0) -> Result<u64, u32> {
    use bmo_gpu_ga10x::anillo as an;
    let numero = ANILLO_NUMERO.load(Ordering::Acquire);
    if an::ya(ANILLO_PAGADO.load(Ordering::Acquire), numero) {
        return Ok(0);
    }
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    loop {
        let pagado = an::pagado(r);
        let us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if an::ya(pagado, numero) {
            ANILLO_PAGADO.store(pagado, Ordering::Release);
            return Ok(us);
        }
        if us >= FRACTAL_ESPERA_US {
            crate::ring0::cabina::warn("gpu", "V1b: el anillo no se vacio: la 3060 no pago el fotograma", numero as u64);
            ANILLO_HUELLA.store(0, Ordering::Release);
            ANILLO_PAGADO.store(numero, Ordering::Release);
            return Err(IOMMU_NO_BLUR);
        }
        esperando(us);
    }
}

/// **VACIAR** (`CUBO_VACIAR`, y LEER): con su cerrojo.
fn vaciar_con_cerrojo() -> Result<u64, u32> {
    if !en_vuelo() {
        return Ok(0);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let r = vaciar(&mut Bar0(bar0));
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

/// **Soltar el anillo** para otro trabajo del GR (`gr_ocupado`, con el
/// cerrojo ya tomado): lo que quedo en vuelo, pagado, y el anillo olvidado
/// -- el otro va a escribir en el mismo tramo (programas, tabla, ordenes).
pub(super) fn soltar_anillo() -> Result<(), u32> {
    if ANILLO_HUELLA.load(Ordering::Acquire) == 0 && !en_vuelo() {
        return Ok(());
    }
    ANILLO_HUELLA.store(0, Ordering::Release);
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return Err(IOMMU_NO_BLUR);
    }
    vaciar(&mut Bar0(bar0)).map(|_| ())
}

/// **La pagina del anillo**: pedida, PRESTADA a la 3060 SOLO LECTURA (lee
/// los vertices; no escribe nada en la RAM del PC) y mapeada, una vez por
/// arranque.
fn asegurar_pagina(r: &mut Bar0) -> Result<(), u32> {
    use crate::ring0::plat::iommu as io;
    use bmo_gpu_ga10x::anillo as an;
    if ANILLO_PRESTADO.load(Ordering::Acquire) {
        return Ok(());
    }
    let Some(f) = super::grupo(&ANILLO_F, an::PAGINAS) else {
        crate::ring0::cabina::warn("gpu", "V1b: no hubo una pagina de RAM para el anillo", 0);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    };
    super::memoria(f, an::PAGINAS * super::PAGINA).fill(0);
    if io::prestar_gpu(an::IOVA, f, an::PAGINAS, false).is_err() || !matches!(io::ve_la_gpu(an::IOVA), Some((g, false)) if g == f) {
        crate::ring0::cabina::warn("gpu", "V1b: el anillo no se ve por la IOMMU donde se presto; iova", an::IOVA);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    match an::mapear(r) {
        Some((n, bien)) if n == bien => {}
        _ => {
            crate::ring0::cabina::warn("gpu", "V1b: la PTE del anillo no estaba vacia o no se releyo", 0);
            return Err(IOMMU_NO_BLUR_PREPARAR);
        }
    }
    ANILLO_PRESTADO.store(true, Ordering::Release);
    crate::ring0::cabina::count("gpu", "V1b: EL ANILLO -- una pagina de RAM PRESTADA a la 3060 (solo lectura) y mapeada; iova", an::IOVA);
    Ok(())
}

/// Los vertices del paquete en la ranura `k` de la pagina del anillo.
fn a_la_ranura(k: u32, vertices: &[u8]) {
    let f = ANILLO_F.load(Ordering::Acquire) + bmo_gpu_ga10x::anillo::PASO * k as u64;
    super::memoria(f, bmo_gpu_ga10x::anillo::PASO)[..vertices.len()].copy_from_slice(vertices);
}

/// **VERRANO por el anillo** (con el cerrojo tomado, SIN vaciar): si el
/// anillo esta armado con lo mismo, el fotograma se ENVIA y se vuelve; si
/// no, se ARMA en frio con este fotograma y se espera.
fn en_anillo(bar0: u64, e: u32, p: &bmo_gpu_ga10x::pantalla::Pantalla, v: &cu::Ventana, paquete: &bmo_gpu_ga10x::tuberia::Paquete, huella: u64) -> Result<u64, u32> {
    let mut r = Bar0(bar0);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let n = (paquete.vertices.len() / bmo_gpu_ga10x::tuberia::BYTES_VERTICE / 3) as u32;
    let mapa_nuevo = asegurar_mapa(&mut r, p)?;
    // Armado con lo mismo, nadie por el GR desde el ultimo envio (la
    // entrada sigue donde la dejo; `gr_ocupado` ademas lo olvida) y hace
    // menos de 100 ms (entre dos fotogramas de un banco). Un mapa de la
    // pantalla recien hecho pide invalidar la MMU: eso lo hace armar.
    let armado = !mapa_nuevo
        && ANILLO_HUELLA.load(Ordering::Acquire) == huella
        && ANILLO_ENTRADA.load(Ordering::Acquire) == e
        && desde.wrapping_sub(ANILLO_TSC.load(Ordering::Acquire)) < crate::ring0::task::scheduler::tsc_freq() / 10;
    if armado {
        enviar_en_anillo(&mut r, e, v, paquete, n, desde, hz)
    } else {
        armar_anillo(&mut r, e, v, paquete, n, huella, desde, hz)
    }
}

/// **Enviar** el fotograma siguiente: su ranura libre (si la 3060 va
/// CUATRO por detras, se la espera), sus vertices a la RAM, la cola, la
/// entrada, GP_PUT y el timbre. Sin releer, sin invalidar, sin esperar.
fn enviar_en_anillo(r: &mut Bar0, e: u32, v: &cu::Ventana, paquete: &bmo_gpu_ga10x::tuberia::Paquete, n: u32, desde: u64, hz: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::anillo as an;
    let numero = ANILLO_NUMERO.load(Ordering::Acquire).wrapping_add(1);
    // La ranura la usaba el fotograma de hace RANURAS: tiene que estar pagado.
    let viejo = numero.wrapping_sub(an::RANURAS);
    let mut espera = 0u64;
    if !an::ya(ANILLO_PAGADO.load(Ordering::Acquire), viejo) {
        let t0 = crate::ring0::task::scheduler::rdtsc();
        loop {
            let pagado = an::pagado(r);
            espera = (crate::ring0::task::scheduler::rdtsc() - t0) / hz;
            if an::ya(pagado, viejo) {
                ANILLO_PAGADO.store(pagado, Ordering::Release);
                break;
            }
            if espera >= FRACTAL_ESPERA_US {
                crate::ring0::cabina::warn("gpu", "V1b: la 3060 no pago el fotograma que ocupaba la ranura; numero", viejo as u64);
                ANILLO_HUELLA.store(0, Ordering::Release);
                ANILLO_PAGADO.store(ANILLO_NUMERO.load(Ordering::Acquire), Ordering::Release);
                DIAG_3D[0].store(0, Ordering::Release);
                diagnostico(r);
                return Ok(cu::empaquetar(espera as u32, n, 0, true));
            }
            esperando(espera);
        }
    }
    a_la_ranura(an::ranura(numero), paquete.vertices);
    core::sync::atomic::fence(Ordering::SeqCst);
    if !an::enviar(r, e, v, n as usize, numero, paquete.ficha) {
        ANILLO_HUELLA.store(0, Ordering::Release);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    let siguiente = bmo_gpu_ga10x::blur::siguiente(e);
    BLUR_ENTRADA.store(siguiente, Ordering::Release);
    ANILLO_ENTRADA.store(siguiente, Ordering::Release);
    ANILLO_NUMERO.store(numero, Ordering::Release);
    let fin = crate::ring0::task::scheduler::rdtsc();
    ANILLO_TSC.store(fin, Ordering::Release);
    let preparar = ((fin - desde) / hz).saturating_sub(espera);
    Ok(cu::con_preparar(cu::en_vuelo(espera as u32, n), true, preparar))
}

/// **Armar** el anillo con este fotograma (el numero 1), en frio: lo que
/// quedara de otro anillo, pagado; la pagina; los programas, la tabla y las
/// cuatro ranuras, releidos; el timbre CON invalidacion de la MMU, y se
/// espera a que se pague. Solo entonces queda armado.
#[allow(clippy::too_many_arguments)]
fn armar_anillo(r: &mut Bar0, e: u32, v: &cu::Ventana, paquete: &bmo_gpu_ga10x::tuberia::Paquete, n: u32, huella: u64, desde: u64, hz: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::anillo as an;
    vaciar(r)?;
    ANILLO_HUELLA.store(0, Ordering::Release);
    // El anillo pisa EMPUJE y TABLA: el caliente de V1 (`ligero`) ya no
    // puede reusar lo suyo. Su entrada lo diria casi siempre, pero tras 512
    // envios la entrada da la vuelta y podria casar.
    CALIENTE_HUELLA.store(0, Ordering::Release);
    super::volcado::quieto()?;
    asegurar_pagina(r)?;
    super::memoria(ANILLO_F.load(Ordering::Acquire), an::PAGINAS * super::PAGINA).fill(0);
    a_la_ranura(an::ranura(1), paquete.vertices);
    if !an::armar(r, e, v, paquete, 1) {
        crate::ring0::cabina::warn("gpu", "V1b: el anillo no quedo armado; no se toca el timbre", n as u64);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    let preparar = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
    core::sync::atomic::fence(Ordering::SeqCst);
    let t0 = crate::ring0::task::scheduler::rdtsc();
    let lanzado = cu::lanzar(r, paquete.ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(e), Ordering::Release);
    }
    let (mut us, mut pagado) = (0, false);
    while lanzado && us < FRACTAL_ESPERA_US {
        pagado = an::ya(an::pagado(r), 1);
        us = (crate::ring0::task::scheduler::rdtsc() - t0) / hz;
        if pagado {
            break;
        }
        esperando(us);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let etapas = if pagado { 0b100 } else { 0 };
    DIAG_3D[0].store(etapas, Ordering::Release);
    if pagado {
        ANILLO_NUMERO.store(1, Ordering::Release);
        ANILLO_PAGADO.store(1, Ordering::Release);
        ANILLO_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(e), Ordering::Release);
        ANILLO_TSC.store(crate::ring0::task::scheduler::rdtsc(), Ordering::Release);
        ANILLO_HUELLA.store(huella, Ordering::Release);
        DIBUJADO.store(true, Ordering::Release);
    } else {
        // Nada que esperar despues: el fotograma se da por perdido.
        ANILLO_NUMERO.store(0, Ordering::Release);
        ANILLO_PAGADO.store(0, Ordering::Release);
        diagnostico(r);
        crate::ring0::cabina::warn("gpu", "V1b: el primer fotograma del anillo no se pago; us", us);
    }
    Ok(cu::con_preparar(cu::empaquetar(us as u32, n, etapas, lanzado), false, preparar))
}

/// Los escalones y los registros del GR, para `DIAG_3D` (`gpu cubo 3060`
/// y el banco los muestran cuando algo no se pago).
fn diagnostico(r: &mut Bar0) {
    let pagados = bmo_gpu_ga10x::raster::escalones(r);
    DIAG_3D[4].store(pagados as u32, Ordering::Release);
    DIAG_3D[5].store((pagados >> 32) as u32, Ordering::Release);
    for (k, reg) in super::GR_MIRADOS.iter().enumerate() {
        DIAG_3D[1 + k].store(bmo_gpu_ga10x::Registros::leer(r, *reg), Ordering::Release);
    }
}

/// El dibujo, de X5 o de VERRANO: preparar, el timbre, esperar el semaforo,
/// y la escalera. `n` = los triangulos. `escalera` = leer los escalones y
/// los registros del GR al acabar (sin ella, VERRANO `ligero`, no hay
/// escalones que leer: son ~40 lecturas por PCIe menos por fotograma).
fn dibujar(bar0: u64, ficha: u32, e: u32, p: &bmo_gpu_ga10x::pantalla::Pantalla, n: u32, escalera: bool, preparar: impl FnOnce(&mut Bar0) -> bool) -> Result<u64, u32> {
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
    if escalera || etapas & 0b100 == 0 {
        diagnostico(&mut r);
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
    // Lo que el anillo dejo en vuelo, pagado ANTES de leer (y lo del volcado:
    // lo que se lee es lo ultimo que dibujo la 3060, no lo que va a dibujar).
    if en_vuelo() {
        vaciar_con_cerrojo()?;
        super::volcado::quieto()?;
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
