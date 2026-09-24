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
//! No arranca nada: eso es L0c3b. El unico registro de la 3060 que escribe es
//! el TIMBRE del GSP (0x110C00), y solo tras `GSP_INIT_DONE`, para avisar de
//! una RPC (L1a, al final).

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
        // Sin su `writePtr`: tras `gpu sistema` (L0c4b2a) esta en 2, y un
        // `gpu libos` de despues no tiene que llamarlo roto.
        let sin_puntero = |c: (u32, u32, u32, u32, u32, u32, u32)| (c.0, c.1, c.2, c.4, c.5, c.6);
        sigue(sin_puntero(lb::leer_cabecera(cola)) == sin_puntero(lb::leer_cabecera(&lb::cabecera_cpu())))?;
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

/// Los 3 logs y GspMem, uno detras de otro, como los ve `INFO_GPU_GSP_MEM`.
pub const VENTANA_LOGS: u64 = LOGS * PAGINA;
pub const VENTANA_TOTAL: u64 = VENTANA_LOGS + lb::GSPMEM_PAGINAS * PAGINA;

/// **`INFO_GPU_GSP_MEM`**: 8 bytes de lo que el GSP escribe, en `desde`
/// contado sobre `0..0x30000` los logs (LOGINIT, LOGINTR, LOGRM) y detras
/// GspMem. Para leerlo desde el escritorio, como la ROM en L0a. 0 si no hay.
pub fn info_gsp_mem(sel: u64) -> u64 {
    let desde = (sel >> 8) & !7;
    let (f, o) = if desde < VENTANA_LOGS {
        (LOGS_F.load(Ordering::Acquire), desde)
    } else if desde < VENTANA_TOTAL {
        (GSPMEM_F.load(Ordering::Acquire), desde - VENTANA_LOGS)
    } else {
        return 0;
    };
    if f == 0 {
        return 0;
    }
    // SAFETY: un marco NEUTRO de este fichero, dentro de lo que se pidio; lo
    // escribe la 3060 por DMA, y por eso se lee volatile.
    unsafe { (crate::ring0::mm::phys_to_virt(f + o) as *const u64).read_volatile() }
}

// == L0c4b1: EL PUNTERO DE LECTURA DE LA CPU (2026-09-24) =====================
//
// La cola del GSP la escribe el GSP; hasta donde la LEYO la CPU lo dice el
// `readPtr` que vive en la cabecera de la cola de la CPU (`cpuq.rx`, nova-core
// `advance_cpu_read_ptr`). Moverlo es devolverle al GSP los huecos: en el
// metal (24-09 07:34) la cola estaba LLENA de NOCAT, y lo que el GSP diga
// despues no cabe hasta que esto se mueva. Es memoria del PC, no un registro
// de la 3060 -- pero la lee el GSP, y por eso la escribe el kernel.

/// El GSP no esta despierto en este arranque: no hay cola que devolver.
pub const IOMMU_NO_COLA_ANTES: u32 = 46;
/// El puntero pedido no es un hueco de la cola (0..63).
pub const IOMMU_NO_COLA_PUNTERO: u32 = 47;

/// **Mover el `readPtr` de la CPU a `nuevo`.** `Ok(el que habia)`.
pub fn mover_lectura(nuevo: u64) -> Result<u64, u32> {
    use crate::ring0::dev::gpu_despertar as d;
    if d::info_despierto() & d::DESPIERTO_VISTO == 0 {
        return Err(IOMMU_NO_COLA_ANTES);
    }
    let f = GSPMEM_F.load(Ordering::Acquire);
    if f == 0 || nuevo >= lb::MSGQ_PAGINAS {
        return Err(IOMMU_NO_COLA_PUNTERO);
    }
    let p = crate::ring0::mm::phys_to_virt(f + lb::COLA_CPU + lb::RX_HDR_OFF as u64) as *mut u32;
    // Lo leido de los mensajes tiene que estar acabado ANTES de devolver sus
    // huecos: nova-core pone aqui una barrera, y aqui tambien.
    core::sync::atomic::fence(Ordering::SeqCst);
    // SAFETY: `readPtr` de la cola de la CPU, dentro de GspMem (marcos NEUTRO
    // de este fichero); volatile porque lo lee el GSP por DMA.
    let antes = unsafe { p.read_volatile() };
    unsafe { p.write_volatile(nuevo as u32) };
    core::sync::atomic::fence(Ordering::SeqCst);
    Ok(antes as u64)
}

// == L0c4b2a: LOS DOS PRIMEROS MENSAJES DE LA CPU (2026-09-24) ================
//
// `GSP_SET_SYSTEM_INFO` y `SET_REGISTRY`, a la cola de la CPU y ANTES de
// despertar el GSP, como nouveau (`r535_gsp_oneinit`) y OpenRM (`kgspInitRm`):
// el GSP-RM los encuentra al mirar su cola por primera vez. nova-core los manda
// despues, con el RISC-V ya activo, y toca el timbre (`0x110C00`); aqui no hace
// falta timbre -- `despertar` resetea el falcon del GSP despues de esto.
//
// En el metal (24-09 08:14) el GSP llego a su secuenciador SIN ellos, tras 835
// ASSERT. Los bytes los arma `bmo_gpu_ga10x::orden` en su hueco de GspMem, con
// lo que el kernel lee del PCI: el escritorio no manda bytes, solo dice "ya".

/// GspMem, en fisica (`0` = sin `gpu libos`). Para L0c4b2c, que lee el
/// secuenciador de la cola del GSP.
pub(crate) fn gspmem() -> u64 {
    GSPMEM_F.load(Ordering::Acquire)
}

/// Sin `gpu libos` no hay cola de la CPU donde escribir.
pub const IOMMU_NO_SISTEMA_ANTES: u32 = 48;
/// Ya se mandaron, o el GSP ya desperto: van ANTES, y una sola vez.
pub const IOMMU_NO_SISTEMA_YA: u32 = 49;

/// Una BAR de memoria del espacio de configuracion, con su mitad alta si es de
/// 64 bits. `0` si es de E/S.
fn barra(bus: u8, dev: u8, func: u8, off: u8) -> u64 {
    let pci = crate::ring0::dev::pci::cfg_read32;
    let bajo = pci(bus, dev, func, off);
    if bajo & 1 != 0 {
        return 0;
    }
    let base = (bajo & 0xFFFF_FFF0) as u64;
    if (bajo >> 1) & 3 == 2 {
        base | (pci(bus, dev, func, off + 4) as u64) << 32
    } else {
        base
    }
}

/// **Escribir SetSystemInfo y SetRegistry** en las paginas 0 y 1 de la cola
/// de la CPU y mover su `writePtr` a 2. `Ok(2)`.
pub fn escribir_sistema() -> Result<u64, u32> {
    let f = GSPMEM_F.load(Ordering::Acquire);
    if f == 0 {
        return Err(IOMMU_NO_SISTEMA_ANTES);
    }
    let Some((bus, dev, func)) = crate::ring0::dev::gpu::bdf() else { return Err(io::IOMMU_NO_SIN_GPU) };
    let cola = f + lb::COLA_CPU;
    // `writePtr` de la cola de la CPU: +16 de su cabecera (`msgqTxHeader`).
    let escrito = crate::ring0::mm::phys_to_virt(cola + 16) as *mut u32;
    // SAFETY: dentro de GspMem (marcos NEUTRO de este fichero); volatile
    // porque lo lee el GSP por DMA.
    if crate::ring0::dev::gpu_despertar::gsp_tomado() || unsafe { escrito.read_volatile() } != 0 {
        return Err(IOMMU_NO_SISTEMA_YA);
    }
    let pci = |off| crate::ring0::dev::pci::cfg_read32(bus, dev, func, off);
    let s = bmo_gpu_ga10x::orden::Sistema {
        bar0: barra(bus, dev, func, 0x10),
        bar1: barra(bus, dev, func, 0x14),
        bar3: barra(bus, dev, func, 0x1C),
        bdf: (bus as u16) << 8 | (dev as u16) << 3 | func as u16,
        id: pci(0x00),
        subid: pci(0x2C),
        revision: pci(0x08) as u8,
    };
    let hueco = |k: u64| {
        let p = crate::ring0::mm::phys_to_virt(cola + PAGINA + k * PAGINA) as *mut u8;
        // SAFETY: la pagina `k` de datos de la cola de la CPU, dentro de
        // GspMem; las dos (0 y 1) no se pisan y nadie mas las escribe todavia.
        unsafe { core::slice::from_raw_parts_mut(p, PAGINA as usize) }
    };
    let (Some(a), Some(b)) = (bmo_gpu_ga10x::orden::sistema(hueco(0), 0, &s), bmo_gpu_ga10x::orden::registro(hueco(1), 1)) else {
        return Err(IOMMU_NO_SISTEMA_ANTES);
    };
    // El contrato, tambien aqui: lo que sale hacia el GSP pasa por la lista.
    if bmo_gpu_ga10x::contrato::permitido(&hueco(0)[..a]).is_err() || bmo_gpu_ga10x::contrato::permitido(&hueco(1)[..b]).is_err() {
        crate::ring0::cabina::warn("gpu", "contrato: SetSystemInfo o SetRegistry fuera de la lista; no se mandan", 0);
        return Err(IOMMU_NO_SISTEMA_ANTES);
    }
    // Los mensajes enteros ANTES de moverle el puntero (nova-core pone la
    // barrera en `advance_cpu_write_ptr`).
    core::sync::atomic::fence(Ordering::SeqCst);
    // SAFETY: como arriba.
    unsafe { escrito.write_volatile(2) };
    core::sync::atomic::fence(Ordering::SeqCst);
    crate::ring0::cabina::count("gpu", "L0c4b2a: SetSystemInfo y SetRegistry en la cola de la CPU; bytes", (a + b) as u64);
    Ok(2)
}

// == L1a: LA PRIMERA RPC DE VERDAD (2026-09-24) ================================
//
// Con el GSP-RM arrancado (metal 24-09 10:13), la CPU PREGUNTA y el GSP-RM
// CONTESTA: `GET_GSP_STATIC_INFO`, lo que el GSP-RM dice de la 3060. La
// pregunta la arma el kernel (`bmo_gpu_ga10x::estatica`) en la pagina
// siguiente de la cola de la CPU, mueve su `writePtr` y toca el TIMBRE
// (`NV_PGSP_QUEUE_HEAD(0)` = 0x110C00, como `notify_gsp` de nova-core): el
// GSP-RM ya corre y no mira la cola si no se le avisa. La respuesta llega por
// la cola del GSP y la lee el escritorio.
//
// L1b (misma puerta): `GSP_RM_ALLOC` de NUESTRO cliente, dispositivo y
// subdispositivo (`bmo_gpu_ga10x::objeto`), con asas fijas. El escritorio
// dice CUAL (0, 1 o 2), nunca manda bytes: solo salen del kernel preguntas
// de la lista.

/// El GSP-RM no esta arrancado (sin secuenciador corrido) o no hay colas.
pub const IOMMU_NO_RPC_ANTES: u32 = 54;
/// La cola de la CPU esta llena: el GSP-RM no ha leido lo de antes.
pub const IOMMU_NO_RPC_LLENA: u32 = 55;
/// L1b: no es uno de los tres objetos.
pub const IOMMU_NO_RPC_OBJETO: u32 = 56;
/// El mensaje armado no esta en el contrato (`bmo_gpu_ga10x::contrato`): no
/// sale, y el timbre no suena.
pub const IOMMU_NO_RPC_CONTRATO: u32 = 57;
/// No es una de las ordenes de control de la lista.
pub const IOMMU_NO_RPC_CONTROL: u32 = 58;
/// L1c2: sin 3060 que probar, o la prueba de la VRAM ya esta en curso.
pub const IOMMU_NO_VRAM: u32 = 59;

/// El timbre de la cola de la CPU.
const TIMBRE: u32 = bmo_gpu_ga10x::falcon::GSP + 0xC00;
/// El numero de la siguiente pregunta (0 y 1 fueron SetSystemInfo y SetRegistry).
static NUMERO_RPC: AtomicU64 = AtomicU64::new(2);

/// **Preguntar `GET_GSP_STATIC_INFO`.** `Ok(pagina | numero << 32)`.
pub fn preguntar_estatica() -> Result<u64, u32> {
    let r = enviar(bmo_gpu_ga10x::estatica::pregunta)?;
    crate::ring0::cabina::count("gpu", "L1a: GET_GSP_STATIC_INFO preguntada; numero", r >> 32);
    Ok(r)
}

/// **L1b: pedir uno de nuestros objetos** (`que` = 0, 1 o 2).
pub fn pedir_objeto(que: u64) -> Result<u64, u32> {
    let Some(o) = bmo_gpu_ga10x::objeto::Objeto::de(que) else {
        return Err(IOMMU_NO_RPC_OBJETO);
    };
    let r = enviar(|h, n| bmo_gpu_ga10x::objeto::pedir(h, n, o))?;
    crate::ring0::cabina::count("gpu", "L1b: GSP_RM_ALLOC pedido; asa", o.asa() as u64);
    Ok(r)
}

/// **L1b: una orden de control** sobre nuestro subdispositivo (`que` = el
/// indice en `bmo_gpu_ga10x::control::Control::TODOS`).
pub fn pedir_control(que: u64) -> Result<u64, u32> {
    let Some(c) = bmo_gpu_ga10x::control::Control::de(que) else {
        return Err(IOMMU_NO_RPC_CONTROL);
    };
    let r = enviar(|h, n| bmo_gpu_ga10x::control::pedir(h, n, c))?;
    crate::ring0::cabina::count("gpu", "L1b: GSP_RM_CONTROL pedido; cmd", c.forma().0 as u64);
    Ok(r)
}

/// **Una pregunta a la cola de la CPU**: la arma `armar` en la pagina
/// siguiente, mueve el `writePtr` y toca el timbre. `Ok(pagina | numero << 32)`.
fn enviar(armar: impl FnOnce(&mut [u8], u32) -> Option<usize>) -> Result<u64, u32> {
    use crate::ring0::dev::gpu_despertar as d;
    let f = GSPMEM_F.load(Ordering::Acquire);
    let bar0 = crate::ring0::dev::gpu::bar0();
    if f == 0 || bar0 == 0 || d::info_secuencia() >> 24 & d::SEC_HECHO == 0 {
        return Err(IOMMU_NO_RPC_ANTES);
    }
    let cola = f + lb::COLA_CPU;
    let escrito = crate::ring0::mm::phys_to_virt(cola + 16) as *mut u32;
    // Hasta donde leyo el GSP la cola de la CPU: el `readPtr` de la cabecera
    // de SU cola (+32).
    let leido = crate::ring0::mm::phys_to_virt(f + lb::COLA_GSP + lb::RX_HDR_OFF as u64) as *const u32;
    // SAFETY: dentro de GspMem (marcos NEUTRO de este fichero); volatile: la 3060.
    let (wp, rp) = unsafe { (escrito.read_volatile() as u64 % lb::MSGQ_PAGINAS, leido.read_volatile() as u64 % lb::MSGQ_PAGINAS) };
    let siguiente = (wp + 1) % lb::MSGQ_PAGINAS;
    if siguiente == rp {
        return Err(IOMMU_NO_RPC_LLENA);
    }
    let numero = NUMERO_RPC.fetch_add(1, Ordering::AcqRel) as u32;
    let p = crate::ring0::mm::phys_to_virt(cola + PAGINA + wp * PAGINA) as *mut u8;
    // SAFETY: la pagina `wp` de datos de la cola de la CPU, libre (el GSP ya
    // leyo hasta `rp`, y `wp + 1 != rp`); nadie mas la escribe.
    let hueco = unsafe { core::slice::from_raw_parts_mut(p, PAGINA as usize) };
    let Some(n) = armar(hueco, numero) else {
        return Err(IOMMU_NO_RPC_ANTES);
    };
    // ** LA SEGUNDA LLAVE: el mensaje ya armado pasa por el contrato ANTES de
    // mover el `writePtr`. Si no esta en la lista, la pagina queda escrita
    // pero el GSP no la ve (su puntero no avanza) y el timbre no suena.
    if let Err(no) = bmo_gpu_ga10x::contrato::permitido(&hueco[..n]) {
        let f = match no {
            bmo_gpu_ga10x::contrato::No::Funcion(f) => f as u64,
            _ => 0,
        };
        crate::ring0::cabina::warn("gpu", "contrato: un mensaje al GSP fuera de la lista NO sale; funcion", f);
        return Err(IOMMU_NO_RPC_CONTRATO);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    // SAFETY: como arriba.
    unsafe { escrito.write_volatile(siguiente as u32) };
    core::sync::atomic::fence(Ordering::SeqCst);
    bmo_gpu_ga10x::Registros::escribir(&mut crate::ring0::dev::gpu_prestamo::Bar0(bar0), TIMBRE, 0);
    Ok(wp | (numero as u64) << 32)
}

// == L1c2: LA CPU ESCRIBE EN LA VRAM (2026-09-24) ==============================
//
// Por la ventana PRAMIN de BAR0 (`bmo_gpu_ga10x::vram`): una pagina en UNA
// direccion fija (`vram::PRUEBA`, 64 MiB, dentro de lo que el GSP-RM dio como
// usable), guardada antes y devuelta despues, con la ventana como estaba. El
// escritorio no elige la direccion: pide la prueba y lee el resultado.

/// Los 4 KiB de antes, mientras dura la prueba. Estatico: 4 KiB no van en la
/// pila del kernel.
static mut GUARDADO: [u32; bmo_gpu_ga10x::vram::PALABRAS] = [0; bmo_gpu_ga10x::vram::PALABRAS];
static PROBANDO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// **La prueba de la VRAM.** `Ok(vram::empaquetar(..))`.
pub fn probar_vram() -> Result<u64, u32> {
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || PROBANDO.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_VRAM);
    }
    let mut r = crate::ring0::dev::gpu_prestamo::Bar0(bar0);
    // SAFETY: `PROBANDO` deja entrar a uno solo; nadie mas toca GUARDADO.
    let guardado = unsafe { &mut *core::ptr::addr_of_mut!(GUARDADO) };
    let p = bmo_gpu_ga10x::vram::probar(&mut r, guardado);
    PROBANDO.store(false, Ordering::Release);
    crate::ring0::cabina::count("gpu", "L1c2: VRAM por PRAMIN; palabras buenas", p.buenas as u64);
    if p.devueltas as usize != bmo_gpu_ga10x::vram::PALABRAS || !p.ventana_devuelta {
        crate::ring0::cabina::warn("gpu", "L1c2: la VRAM o la ventana NO quedaron como estaban; devueltas", p.devueltas as u64);
    }
    Ok(bmo_gpu_ga10x::vram::empaquetar(&p))
}
