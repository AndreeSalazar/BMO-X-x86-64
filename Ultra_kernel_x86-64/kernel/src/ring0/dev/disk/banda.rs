//! **EL METRO DEL DISCO: cuanto lee ESTE disco, medido en esta maquina.**
//!
//! [carril]  AMARILLO  solo LEE, y dentro de la particion de datos
//! [consumo] NADA      corre cuando una persona lo pide (`disco banda`)
//!
//! [eje]     CORRECCION -- una cifra de catalogo no es una medida
//! [exige]   R-DISCO9 (solo se publica lo MEDIDO), LEY 24 (el hardware se
//!           perfila: la cifra de la caja es de OTRO proyecto)
//!
//! # Por que existe (paso D0 del plan del disco, 2026-09-23)
//!
//! `PERFIL/DISCO.txt` no tenia un solo numero medido, y el perfil del Kingston
//! decia `sostenido_mb_s: Cifra::catalogo(450)`. Cualquier paso siguiente --la
//! E/S asincrona, la cola NCQ, los PRD multiples-- se iba a juzgar contra
//! nada: sin un antes medido, "va mas rapido" no se puede comprobar.
//!
//! # Lo que mide, y lo que NO
//!
//! ```text
//!    MIDE      LECTURA secuencial por el camino de verdad del driver: la
//!              ranura 0, una entrada de PRDT (4 MiB), el juez del DMA y las
//!              marcas de vuelo. Es lo que cuesta leer en BMO-X HOY
//!    NO MIDE   la ESCRITURA sostenida del perfil. Esa cifra es la de despues
//!              de agotar la cache SLC: decenas de GB escritos, desgaste y un
//!              sitio donde escribirlos. Es decision del propietario, no de
//!              un metro que se lanza con una orden
//! ```
//!
//! [!] **Un SSD puede contestar sin leer.** Un sector que nunca se escribio (o
//! que se recorto) no tiene NAND detras: el controlador devuelve ceros de su
//! mapa. Por eso el metro cuenta **cuantos sectores traian datos**, y lo dice
//! al lado de la cifra: una banda medida sobre ceros no es la del disco.
//!
//! # Lo que cuesta
//!
//! Tiene el disco para el solo mientras mide (`tomar_disco`), y el que lo pide
//! espera en su syscall. 64 MiB a ~500 MB/s son ~130 ms. Por eso lo pide una
//! persona, avisada antes -- igual que `disco trim ya`.

use super::*;
use core::sync::atomic::AtomicU64;

/// Lo que se lee si no se dice otra cosa.
pub const MIB_POR_DEFECTO: u64 = 64;
/// Techo: 1 GiB son ~2 s con el disco tomado. Mas es un congelon, no un metro.
pub const MIB_MAX: u64 = 1024;

/// Marcos del bufer de la medida. **4 MiB = una entrada de PRDT llena**, que
/// es lo mas que el driver pide en una orden (`MAX_POR_COMANDO`). Si no hay
/// tanto contiguo se prueba con menos, y la medida de la orden sale en el informe.
const MARCOS: [u64; 3] = [1024, 256, 64];

// -- Los motivos, espejo de `bmo_abi::...::disco::DISCO_BANDA_*` -------------

pub const DISCO_BANDA_HECHA: u64 = 0;
pub const DISCO_BANDA_SIN_DISCO: u64 = 1;
pub const DISCO_BANDA_SIN_VOLUMEN: u64 = 2;
pub const DISCO_BANDA_SIN_MEMORIA: u64 = 3;
pub const DISCO_BANDA_FALLO: u64 = 4;
pub const DISCO_BANDA_SIN_RELOJ: u64 = 5;

// -- Los campos, espejo de `bmo_abi::...::informe::DISCO_BANDA_*` ------------

pub const DISCO_BANDA_US_MASK: u64 = 0xFFFF_FFFF;
pub const DISCO_BANDA_MIB_SHIFT: u64 = 32;
pub const DISCO_BANDA_MIB_MASK: u64 = 0xFFFF;
pub const DISCO_BANDA_DATOS_SHIFT: u64 = 48;
pub const DISCO_BANDA_DATOS_MASK: u64 = 0xFF;
pub const DISCO_BANDA_VECES_SHIFT: u64 = 56;
pub const DISCO_BANDA_ORDEN_SECTORES_MASK: u64 = 0xFFFF;
pub const DISCO_BANDA_ORDEN_MEJOR_SHIFT: u64 = 16;
pub const DISCO_BANDA_ORDEN_PEOR_SHIFT: u64 = 40;
pub const DISCO_BANDA_ORDEN_US_MASK: u64 = 0xFF_FFFF;

/// La ultima medida, ya empaquetada como sale por `INFO_DISCO_BANDA`. 0 = nunca.
static BANDA: AtomicU64 = AtomicU64::new(0);
/// Y sus ordenes: `INFO_DISCO_BANDA_ORDEN`.
static ORDEN: AtomicU64 = AtomicU64::new(0);

/// `INFO_DISCO_BANDA`.
pub fn banda() -> u64 {
    BANDA.load(Ordering::Relaxed)
}

/// `INFO_DISCO_BANDA_ORDEN`.
pub fn banda_orden() -> u64 {
    ORDEN.load(Ordering::Relaxed)
}

fn us(ticks: u64, hz: u64) -> u64 {
    ((ticks as u128 * 1_000_000) / hz as u128) as u64
}

/// **Mide.** `mib` = cuanto leer (0 = [`MIB_POR_DEFECTO`]). Devuelve
/// `(motivo, MB/s)` y deja el detalle en `INFO_DISCO_BANDA*`.
pub fn medir(mib: u64) -> (u64, u64) {
    if !is_ready() {
        return (DISCO_BANDA_SIN_DISCO, 0);
    }
    let hz = crate::ring0::task::scheduler::tsc_freq();
    if hz == 0 {
        return (DISCO_BANDA_SIN_RELOJ, 0);
    }
    // ** DONDE: la particion de DATOS, desde su principio. Es la unica zona que
    // BMO-X ya reconocio como suya; leer no rompe nada, pero un metro que
    // pasea por el disco entero acabaria cronometrando la ESP del arranque, y
    // eso no le dice nada a nadie de como lee BMO-X sus programas.
    let Some(part) = data_partition() else {
        return (DISCO_BANDA_SIN_VOLUMEN, 0);
    };
    let mib = if mib == 0 { MIB_POR_DEFECTO } else { mib.min(MIB_MAX) };
    let sectores = (mib * 1024 * 1024 / SECTOR as u64).min(part.sectors());

    // El bufer: contiguo, del AHCI (`Neutro`) y prestado solo mientras dura.
    let Some((buf, marcos)) = MARCOS
        .iter()
        .find_map(|&n| phys::alloc_frames_contig_de(n, phys::Titular::Neutro).map(|b| (b, n)))
    else {
        return (DISCO_BANDA_SIN_MEMORIA, 0);
    };
    let por_orden = ((marcos * mm::PAGE) / SECTOR as u64) as u16;

    crate::ring0::cabina::info("disk", "metro de lectura: MiB pedidos", mib);
    let _testigo = tomar_disco();

    let mut hecho = 0u64;
    let mut con_datos = 0u64;
    let mut ticks = 0u64;
    let mut mejor = u64::MAX;
    let mut peor = 0u64;
    let mut fallo = false;
    while hecho < sectores {
        let n = ((sectores - hecho).min(por_orden as u64)) as u16;
        // ** Solo se cronometra la orden. Mirar si trae datos va FUERA del
        // reloj: es trabajo del metro, no del disco.
        let t0 = crate::ring0::task::scheduler::rdtsc();
        let got = transfer::mandar_lectura(part.first_lba + hecho, n, buf, false);
        let dt = crate::ring0::task::scheduler::rdtsc().wrapping_sub(t0);
        let Some(got) = got.filter(|&g| g > 0) else {
            fallo = true;
            break;
        };
        ticks += dt;
        mejor = mejor.min(dt);
        peor = peor.max(dt);
        con_datos += sectores_con_datos(buf, got);
        hecho += got as u64;
        if got < n {
            break;
        }
    }

    // Se devuelve marco a marco: asi se pidio y asi lo cuenta el titular.
    for i in 0..marcos {
        phys::free_frame_de(buf + i * mm::PAGE, phys::Titular::Neutro);
    }

    if hecho == 0 {
        return (DISCO_BANDA_FALLO, 0);
    }
    let bytes = hecho * SECTOR as u64;
    let total_us = us(ticks, hz).max(1);
    let mb_s = bytes / total_us; // bytes por microsegundo = MB/s
    let pct = con_datos * 100 / hecho;
    let veces = ((BANDA.load(Ordering::Relaxed) >> DISCO_BANDA_VECES_SHIFT) + 1).min(0xFF);

    BANDA.store(
        (total_us & DISCO_BANDA_US_MASK)
            | (((bytes >> 20) & DISCO_BANDA_MIB_MASK) << DISCO_BANDA_MIB_SHIFT)
            | ((pct & DISCO_BANDA_DATOS_MASK) << DISCO_BANDA_DATOS_SHIFT)
            | (veces << DISCO_BANDA_VECES_SHIFT),
        Ordering::Relaxed,
    );
    ORDEN.store(
        (por_orden as u64 & DISCO_BANDA_ORDEN_SECTORES_MASK)
            | ((us(mejor, hz) & DISCO_BANDA_ORDEN_US_MASK) << DISCO_BANDA_ORDEN_MEJOR_SHIFT)
            | ((us(peor, hz) & DISCO_BANDA_ORDEN_US_MASK) << DISCO_BANDA_ORDEN_PEOR_SHIFT),
        Ordering::Relaxed,
    );
    crate::ring0::cabina::info("disk", "metro de lectura: MB/s MEDIDOS", mb_s);
    if pct < 90 {
        crate::ring0::cabina::warn("disk", "metro: pocos sectores con datos (%), el SSD pudo no leer", pct);
    }
    (if fallo { DISCO_BANDA_FALLO } else { DISCO_BANDA_HECHA }, mb_s)
}

/// Cuantos de los `n` sectores que acaban de llegar a `buf` traen algo que no
/// sea cero. Ver la cabecera: un sector a ceros puede no haber tocado la NAND.
fn sectores_con_datos(buf: u64, n: u16) -> u64 {
    let base = mm::phys_to_virt(buf) as *const u64;
    let mut con = 0u64;
    for s in 0..n as usize {
        let p = unsafe { base.add(s * SECTOR / 8) };
        if (0..SECTOR / 8).any(|i| unsafe { p.add(i).read_volatile() } != 0) {
            con += 1;
        }
    }
    con
}
