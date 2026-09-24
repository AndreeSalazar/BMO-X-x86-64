//! **LO QUE EL GSP PODRA ESCRIBIR (L0c3a)** -- sus argumentos de LIBOS, sus
//! tres logs, `rmargs`, sus dos colas de mensajes y la pagina de vaciado,
//! prestados a la 3060 PARA ESCRIBIR; y comprobados siguiendo cada puntero por
//! la IOMMU, como los seguira el GSP.
//!
//! [carril]  ROJO      pide 180 marcos NEUTRO y los presta ESCRIBIBLES a un aparato
//! [consumo] NADA      corre por orden (`gpu libos`, y el paso `libos`)
//!
//! [eje]     CORRECCION -- es la primera memoria del PC en la que la 3060 puede
//!           ESCRIBIR; lo que no este aqui sigue siendo un fallo de pagina
//!
//! # Lo que ve la 3060 (IOVAs), ademas de lo de L0c2
//!
//! ```text
//!    0x3C00_0000   los argumentos de LIBOS (1 pag.)  -> en L0c3b, por el buzon
//!    0x3C00_1000   rmargs (1)
//!    0x3C00_2000   la pagina de vaciado (1)          -> en L0c3b, al 0x100C10
//!    0x3C10_0000   LOGINIT, LOGINTR y LOGRM (3 x 16)
//!    0x3D00_0000   GspMem: su tabla y las dos colas (129)
//! ```
//!
//! Todo ESCRIBIBLE, como en nova-core (son `Coherent`, no `ToDevice`): el GSP
//! mueve punteros en los logs y en las colas. Lo que queda fuera de estos 180
//! marcos y de lo de L0c2 sigue siendo, para la 3060, un fallo de pagina.
//!
//! # Como se sabe
//!
//! Se sigue cada puntero que seguira el GSP, empezando por el unico que le
//! daremos (los argumentos de LIBOS), y CADA SALTO por la IOMMU
//! (`iommu::ve_la_gpu`): la entrada, su nombre, su IOVA, cada pagina que dice,
//! la tabla dentro de cada log, `rmargs`, la tabla de GspMem y la cabecera de
//! la cola del CPU. Todos tienen que llevar a los marcos pedidos aqui, y la
//! 3060 tiene que poder escribirlos.
//!
//! # Lo que NO hace
//!
//! No escribe un registro de la 3060 ni arranca nada. Eso es L0c3b.

use core::sync::atomic::{AtomicU64, Ordering};

use bmo_gpu_ga10x::libos as lb;

use crate::ring0::mm::phys;
use crate::ring0::plat::iommu as io;

pub const IOVA_LIBOS: u64 = 0x3C00_0000;
pub const IOVA_RMARGS: u64 = 0x3C00_1000;
pub const IOVA_VACIADO: u64 = 0x3C00_2000;
pub const IOVA_LOGS: u64 = 0x3C10_0000;
pub const IOVA_GSPMEM: u64 = 0x3D00_0000;

const PAGINA: u64 = lb::PAGINA;
/// Los argumentos, `rmargs` y la de vaciado.
const CHICAS: u64 = 3;
const LOGS: u64 = 3 * lb::LOG_PAGINAS;
const LOG_BYTES: u64 = lb::LOG_PAGINAS * PAGINA;

/// Sin el GSP-RM prestado (L0c2) no hay a quien darle esto.
pub const IOMMU_NO_LIBOS_ORDEN: u32 = 34;
/// No hubo marcos para los argumentos, los logs o las colas.
pub const IOMMU_NO_LIBOS_MARCOS: u32 = 35;
/// Un puntero no lleva, por la IOMMU, a donde tiene que llevar.
pub const IOMMU_NO_LIBOS_PUNTERO: u32 = 36;

pub const LIBOS_MOTIVO_SHIFT: u64 = 16;
pub const LIBOS_PRESTADAS_SHIFT: u64 = 24;
pub const LIBOS_PREPARADO: u64 = 1 << 56;
pub const LIBOS_PRESTADO: u64 = 1 << 57;
pub const LIBOS_COMPROBADO: u64 = 1 << 58;
pub const LIBOS_VALIDO: u64 = 1 << 63;

/// Banderas, motivo y paginas (ver `info_libos`).
static ESTADO: AtomicU64 = AtomicU64::new(0);
/// Los punteros seguidos en la ultima comprobacion.
static SEGUIDOS: AtomicU64 = AtomicU64::new(0);
/// Las fisicas de los tres grupos.
static CHICAS_F: AtomicU64 = AtomicU64::new(0);
static LOGS_F: AtomicU64 = AtomicU64::new(0);
static GSPMEM_F: AtomicU64 = AtomicU64::new(0);
/// Lo ya prestado: bit 0 las chicas, 1 los logs, 2 GspMem.
static PRESTADO: AtomicU64 = AtomicU64::new(0);

fn apuntar(f: impl FnOnce(u64) -> u64) {
    let v = ESTADO.load(Ordering::Acquire);
    ESTADO.store(f(v), Ordering::Release);
}

fn no(motivo: u32) -> Result<u64, u32> {
    apuntar(|v| (v & !(0xFF << LIBOS_MOTIVO_SHIFT)) | LIBOS_VALIDO | (motivo as u64 & 0xFF) << LIBOS_MOTIVO_SHIFT);
    crate::ring0::cabina::warn("gpu", "L0c3a: lo que el GSP escribe no se presto; motivo", motivo as u64);
    Err(motivo)
}

fn memoria(fisica: u64, bytes: u64) -> &'static mut [u8] {
    // SAFETY: solo con marcos NEUTRO de este fichero, del medida con que se
    // pidieron, o con lo que `io::ve_la_gpu` dice que la 3060 ve -- que son
    // esos mismos marcos (se comprueba antes de leer).
    unsafe { core::slice::from_raw_parts_mut(crate::ring0::mm::phys_to_virt(fisica) as *mut u8, bytes as usize) }
}

/// Un grupo de marcos, pedido una vez.
fn grupo(celda: &AtomicU64, paginas: u64) -> Option<u64> {
    let f = celda.load(Ordering::Acquire);
    if f != 0 {
        return Some(f);
    }
    // La 3060 los leera y ESCRIBIRA por DMA: NEUTRO.
    let f = phys::alloc_frames_contig_de(paginas, phys::Titular::Neutro)?;
    celda.store(f, Ordering::Release);
    Some(f)
}

/// **`gpu libos`**: preparar, prestar y comprobar. `Ok(punteros seguidos)`.
pub fn libos() -> Result<u64, u32> {
    if ESTADO.load(Ordering::Acquire) & LIBOS_PRESTADO != 0 {
        return comprobar();
    }
    let gsp = crate::ring0::dev::gpu_gsp::info_gsp();
    if gsp & crate::ring0::dev::gpu_gsp::GSP_PRESTADO == 0 {
        return no(IOMMU_NO_LIBOS_ORDEN);
    }
    let (Some(chicas), Some(logs), Some(gspmem)) =
        (grupo(&CHICAS_F, CHICAS), grupo(&LOGS_F, LOGS), grupo(&GSPMEM_F, lb::GSPMEM_PAGINAS))
    else {
        return no(IOMMU_NO_LIBOS_MARCOS);
    };

    if PRESTADO.load(Ordering::Acquire) == 0 {
        memoria(chicas, CHICAS * PAGINA).fill(0);
        memoria(logs, LOGS * PAGINA).fill(0);
        memoria(gspmem, lb::GSPMEM_PAGINAS * PAGINA).fill(0);
        // Cada log: su puntero de escritura a 0 y, desde +8, sus paginas.
        for j in 0..3 {
            let iova = IOVA_LOGS + j * LOG_BYTES;
            lb::tabla(&mut memoria(logs + j * LOG_BYTES, PAGINA)[8..], iova, lb::LOG_PAGINAS);
        }
        // rmargs: donde esta GspMem y sus colas.
        let r = lb::rmargs(IOVA_GSPMEM);
        memoria(chicas + PAGINA, PAGINA)[..r.len()].copy_from_slice(&r);
        // GspMem: su tabla, y la cabecera de la cola del CPU.
        lb::tabla(memoria(gspmem, PAGINA), IOVA_GSPMEM, lb::GSPMEM_PAGINAS);
        let c = lb::cabecera_cpu();
        memoria(gspmem + lb::COLA_CPU, PAGINA)[..c.len()].copy_from_slice(&c);
        // Los argumentos de LIBOS: los cuatro, en el orden de nova-core.
        let args = memoria(chicas, PAGINA);
        let regiones = [
            (IOVA_LOGS, LOG_BYTES),
            (IOVA_LOGS + LOG_BYTES, LOG_BYTES),
            (IOVA_LOGS + 2 * LOG_BYTES, LOG_BYTES),
            (IOVA_RMARGS, PAGINA),
        ];
        for (k, (iova, bytes)) in regiones.iter().enumerate() {
            let a = lb::argumento(lb::NOMBRES[k], *iova, *bytes);
            args[k * lb::ARGUMENTO..(k + 1) * lb::ARGUMENTO].copy_from_slice(&a);
        }
        apuntar(|v| v | LIBOS_VALIDO | LIBOS_PREPARADO);
    }

    // El prestamo, ESCRIBIBLE, por grupos; lo ya prestado no se repite.
    for (bit, iova, fisica, n) in [
        (1u64, IOVA_LIBOS, chicas, CHICAS),
        (2, IOVA_LOGS, logs, LOGS),
        (4, IOVA_GSPMEM, gspmem, lb::GSPMEM_PAGINAS),
    ] {
        if PRESTADO.load(Ordering::Acquire) & bit != 0 {
            continue;
        }
        if let Err(m) = io::prestar_gpu(iova, fisica, n, true) {
            return no(m);
        }
        PRESTADO.fetch_or(bit, Ordering::AcqRel);
    }
    let total = CHICAS + LOGS + lb::GSPMEM_PAGINAS;
    apuntar(|v| (v & !(0xFFFF << LIBOS_PRESTADAS_SHIFT)) | LIBOS_PRESTADO | total << LIBOS_PRESTADAS_SHIFT);
    crate::ring0::cabina::count("gpu", "L0c3a: lo que el GSP escribe, PRESTADO a la 3060 para escribir; paginas", total);
    comprobar()
}

/// **Lo que ve la 3060 en `iova`**, si puede ESCRIBIRLO y es `esperada`.
fn escribible(iova: u64, esperada: u64) -> bool {
    matches!(io::ve_la_gpu(iova), Some((f, true)) if f == esperada)
}

/// Una pagina, leida por donde la vera la 3060.
fn por_la_iommu(iova: u64) -> Option<&'static [u8]> {
    let (f, _) = io::ve_la_gpu(iova)?;
    Some(memoria(f & !(PAGINA - 1), PAGINA))
}

/// **Seguir cada puntero como lo seguira el GSP.** `Ok(punteros seguidos)`.
fn comprobar() -> Result<u64, u32> {
    let (chicas, logs, gspmem) = (
        CHICAS_F.load(Ordering::Acquire),
        LOGS_F.load(Ordering::Acquire),
        GSPMEM_F.load(Ordering::Acquire),
    );
    let mut n = 0u64;
    let mut sigue = |ok: bool| -> Result<(), u32> {
        if ok {
            n += 1;
            Ok(())
        } else {
            Err(IOMMU_NO_LIBOS_PUNTERO)
        }
    };
    let r = (|| -> Result<(), u32> {
        // 1. Los argumentos: lo unico que el GSP recibira por su buzon.
        sigue(escribible(IOVA_LIBOS, chicas))?;
        let args = por_la_iommu(IOVA_LIBOS).ok_or(IOMMU_NO_LIBOS_PUNTERO)?;
        let fisicas = [logs, logs + LOG_BYTES, logs + 2 * LOG_BYTES, chicas + PAGINA];
        for k in 0..4 {
            let (id, iova, bytes, kind, loc) = lb::leer_argumento(&args[k * lb::ARGUMENTO..]);
            sigue(id == lb::id8(lb::NOMBRES[k]) && kind == lb::CONTIGUO && loc == lb::EN_SYSMEM)?;
            for p in 0..bytes.div_ceil(PAGINA) {
                sigue(escribible(iova + p * PAGINA, fisicas[k] + p * PAGINA))?;
            }
            if k < 3 {
                // 2. Cada log lleva dentro la IOVA de cada pagina suya.
                let pag = por_la_iommu(iova).ok_or(IOMMU_NO_LIBOS_PUNTERO)?;
                for p in 0..lb::LOG_PAGINAS {
                    sigue(lb::entrada(pag, 1 + p as usize) == iova + p * PAGINA)?;
                }
            }
        }
        // 3. rmargs: GspMem y sus colas.
        let rmargs = por_la_iommu(lb::leer_argumento(&args[3 * lb::ARGUMENTO..]).1).ok_or(IOMMU_NO_LIBOS_PUNTERO)?;
        let (mem, paginas, cmdq, statq, pila) = lb::leer_rmargs(rmargs);
        sigue(paginas as u64 == lb::GSPMEM_PAGINAS && cmdq == lb::CMDQ_OFFSET && statq == lb::STATQ_OFFSET && pila == 1)?;
        // 4. La tabla de GspMem: cada pagina, suya y escribible.
        let tabla = por_la_iommu(mem).ok_or(IOMMU_NO_LIBOS_PUNTERO)?;
        for p in 0..lb::GSPMEM_PAGINAS {
            let e = lb::entrada(tabla, p as usize);
            sigue(e == mem + p * PAGINA && escribible(e, gspmem + p * PAGINA))?;
        }
        // 5. La cabecera de la cola del CPU, como la leera el GSP.
        let cola = por_la_iommu(mem + lb::COLA_CPU).ok_or(IOMMU_NO_LIBOS_PUNTERO)?;
        sigue(lb::leer_cabecera(cola) == lb::leer_cabecera(&lb::cabecera_cpu()))?;
        // Y la de vaciado, para L0c3b.
        sigue(escribible(IOVA_VACIADO, chicas + 2 * PAGINA))?;
        Ok(())
    })();
    SEGUIDOS.store(n, Ordering::Release);
    if let Err(m) = r {
        apuntar(|v| v & !LIBOS_COMPROBADO);
        return no(m);
    }
    apuntar(|v| (v & !(0xFF << LIBOS_MOTIVO_SHIFT)) | LIBOS_COMPROBADO);
    crate::ring0::cabina::count("gpu", "L0c3a: cada puntero del GSP lleva a lo suyo por la IOMMU; punteros", n);
    Ok(n)
}

/// `INFO_GPU_LIBOS`: `0..15` punteros seguidos | `16..23` el ultimo NO |
/// `24..39` paginas prestadas | 56 preparado | 57 PRESTADO | 58 COMPROBADO |
/// 63 valido.
pub fn info_libos() -> u64 {
    let v = ESTADO.load(Ordering::Acquire);
    if v & LIBOS_VALIDO == 0 {
        return 0;
    }
    v | SEGUIDOS.load(Ordering::Acquire).min(0xFFFF)
}
