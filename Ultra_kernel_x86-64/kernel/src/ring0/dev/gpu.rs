//! **LA GRAFICA, PREGUNTADA** -- quien es, que modo barre, y si su VBLANK se
//! puede ver sin firmware.
//!
//! [carril]  AMARILLO  LEE la grafica por MMIO: ni un bit escrito
//! [consumo] NADA      corre una vez al arrancar, y una lectura cuando alguien pregunta
//!
//! [eje]     CORRECCION -- es la pregunta de LEY 24 antes de construir nada
//! [riesgo]  AJENO -- lo que se lee es de un aparato que BMO-X no inicializo:
//!           lo dejo asi el firmware (el GOP del UEFI), y un error del anillo
//!           PRIV (`0xBAD.....`) tiene cara de numero
//!
//! # Por que existe (2026-09-23)
//!
//! `docs/maestro/GPU_NVIDIA_MAESTRO.md` (6b): la pantalla de la RTX 3060 se
//! puede mover sin el firmware del GSP, y lo que le falta al compositor es el
//! VBLANK. Antes de escribir un driver, este fichero contesta si SE PUEDE,
//! leyendo lo que la tarjeta ya hace con el modo que dejo el GOP:
//!
//! ```text
//!    la identidad (BOOT_0)       es un Ampere? cual?
//!    la cabeza que pinta         su modo: totales y borrado
//!    su linea, cronometrada      el refresco MEDIDO, dos vueltas de la linea
//! ```
//!
//! Si la linea avanza y da la vuelta, el VBLANK se puede esperar por MMIO y
//! sin firmware: "se puede" lo dice el metal, no este comentario.
//!
//! # Lo que NO hace, a proposito
//!
//! No escribe NADA en la tarjeta: ni configuracion PCI (no enciende el maestro
//! de bus: no hay DMA que hacer), ni registros. Lo que decodifica cada numero
//! vive en `bmo-gpu-ga10x`, que se prueba en el anfitrion; esto es el pegamento.

use bmo_gpu_ga10x as ga10x;
use core::sync::atomic::{AtomicU64, Ordering};

/// NVIDIA en el registro de fabricante.
const NVIDIA: u32 = 0x10DE;
/// Clase PCI 0x03: una controladora de pantalla.
const CLASE_PANTALLA: u8 = 0x03;
/// Lo mas que se cronometra la linea al arrancar. Tres vueltas a 60 Hz son
/// 50 ms; con la cabeza parada, esto es lo que se pierde para saberlo.
const MEDIR_MAX_MS: u64 = 80;

/// BAR0 por el physmap. `0` = no hay grafica que leer.
static BAR0: AtomicU64 = AtomicU64::new(0);

// -- El contrato, espejo de `bmo_abi::...::informe::GPU_*` --------------------

pub const GPU_BOOT0_MASK: u64 = 0xFFFF_FFFF;
pub const GPU_DEVICE_SHIFT: u64 = 32;
pub const GPU_CABEZAS_SHIFT: u64 = 48;
pub const GPU_CABEZA_SHIFT: u64 = 56;
pub const GPU_AMPERE: u64 = 1 << 62;
pub const GPU_HALLADA: u64 = 1 << 63;
pub const GPU_MODO_VALIDO: u64 = 1 << 63;
pub const GPU_TIEMPO_MEDIDO: u64 = 1 << 63;
pub const GPU_LINEA_VBLANK: u64 = 1 << 16;
pub const GPU_LINEA_VALIDA: u64 = 1 << 63;

static CHIP: AtomicU64 = AtomicU64::new(0);
/// `0..15` px visibles, `16..31` lineas visibles, `32..47` htotal, `48..62` vtotal, 63 valido.
static MODO: AtomicU64 = AtomicU64::new(0);
/// `0..15` inicio del VBLANK, `16..31` su fin, `32..62` reloj de pixel en kHz.
static BORRADO: AtomicU64 = AtomicU64::new(0);
/// `0..31` periodo MEDIDO en ns, `32..47` vueltas vistas, `48..62` cambios de linea (saturado), 63 medido.
static TIEMPO: AtomicU64 = AtomicU64::new(0);

fn leer(bar0: u64, reg: u32) -> u32 {
    // SAFETY: BAR0 de la grafica por el physmap, que el MTRR de la placa hace
    // no cacheable (como el ABAR del AHCI). Solo lecturas de registros de 32
    // bits alineados que nouveau lee igual.
    unsafe { ((bar0 + reg as u64) as *const u32).read_volatile() }
}

/// **La busca y la pregunta.** Una vez, al arrancar.
pub fn sondear() {
    let Some((bus, dev, func, device)) = buscar() else {
        crate::ring0::cabina::info("gpu", "no hay grafica NVIDIA en el bus", 0);
        return;
    };
    let pci = crate::ring0::dev::pci::cfg_read32;
    let bar = pci(bus, dev, func, 0x10);
    // BAR0 de una NVIDIA es de memoria y de 32 bits; si no lo es, no se
    // supone nada.
    if bar & 1 != 0 || (bar & 0xFFFF_FFF0) == 0 {
        crate::ring0::cabina::warn("gpu", "la NVIDIA no tiene BAR0 de memoria: no se lee", bar as u64);
        return;
    }
    let fisica = (bar & 0xFFFF_FFF0) as u64;
    let cmd = pci(bus, dev, func, 0x04);
    if cmd & (1 << 1) == 0 {
        // La decodificacion de memoria APAGADA: leer daria todo unos. No se
        // enciende: este fichero no escribe.
        crate::ring0::cabina::warn("gpu", "la NVIDIA tiene la memoria APAGADA: no se lee", cmd as u64);
        return;
    }
    let bar0 = crate::ring0::mm::phys_to_virt(fisica);
    BAR0.store(bar0, Ordering::Release);

    let chip = ga10x::Chip(leer(bar0, ga10x::BOOT_0));
    let mut c = GPU_HALLADA | (chip.0 as u64 & GPU_BOOT0_MASK) | ((device as u64) << GPU_DEVICE_SHIFT);
    crate::ring0::cabina::id("gpu", "NVIDIA en PCI, BOOT_0", chip.0 as u64);
    if !chip.es_ampere() {
        CHIP.store(c, Ordering::Release);
        crate::ring0::cabina::warn("gpu", "BOOT_0 no es un Ampere: no se sigue leyendo", chip.0 as u64);
        return;
    }
    c |= GPU_AMPERE;

    // Las cabezas que existen, y la primera con un modo de verdad: la que
    // dejo encendida el GOP.
    let mascara = leer(bar0, ga10x::CABEZAS);
    if !ga10x::es_error_pri(mascara) {
        c |= ((mascara & 0xFF) as u64) << GPU_CABEZAS_SHIFT;
        for cabeza in 0..8u32 {
            if mascara & (1 << cabeza) == 0 {
                continue;
            }
            let Some(m) = modo(bar0, cabeza) else { continue };
            c |= (cabeza as u64) << GPU_CABEZA_SHIFT;
            guardar_modo(&m);
            medir(bar0, cabeza, &m);
            break;
        }
    }
    CHIP.store(c, Ordering::Release);
}

fn modo(bar0: u64, cabeza: u32) -> Option<ga10x::Modo> {
    ga10x::Modo::de_registros(
        leer(bar0, ga10x::totales(cabeza)),
        leer(bar0, ga10x::fin_borrado(cabeza)),
        leer(bar0, ga10x::inicio_borrado(cabeza)),
        leer(bar0, ga10x::reloj(cabeza)),
    )
}

fn guardar_modo(m: &ga10x::Modo) {
    MODO.store(
        GPU_MODO_VALIDO
            | m.pixeles_visibles() as u64
            | (m.lineas_visibles() as u64) << 16
            | (m.htotal as u64) << 32
            | ((m.vtotal as u64) & 0x7FFF) << 48,
        Ordering::Release,
    );
    BORRADO.store(
        m.vinicio as u64 | (m.vfin as u64) << 16 | ((m.reloj_hz as u64 / 1000) & 0x7FFF_FFFF) << 32,
        Ordering::Release,
    );
}

/// **Cronometra la linea** hasta tres vueltas o [`MEDIR_MAX_MS`]. Es el unico
/// sitio que espera, y solo al arrancar.
fn medir(bar0: u64, cabeza: u32, m: &ga10x::Modo) {
    use crate::ring0::task::scheduler::{rdtsc, tsc_freq};
    let hz = tsc_freq();
    if hz == 0 {
        return;
    }
    let reg = ga10x::linea(cabeza);
    let mut med = ga10x::Medidor::default();
    let fin = rdtsc().saturating_add(hz / 1000 * MEDIR_MAX_MS);
    while med.vueltas < 3 {
        let t = rdtsc();
        if t >= fin {
            break;
        }
        let v = leer(bar0, reg);
        if ga10x::es_error_pri(v) {
            break;
        }
        let l = v as u16;
        if l >= m.vtotal {
            // Una linea fuera del modo no es una linea: no se cuenta.
            continue;
        }
        med.muestra(l, t);
    }
    let ns = med.periodo().map(|p| p.saturating_mul(1_000_000_000) / hz);
    TIEMPO.store(
        ns.map_or(0, |n| GPU_TIEMPO_MEDIDO | (n & 0xFFFF_FFFF))
            | ((med.vueltas as u64) & 0xFFFF) << 32
            | ((med.cambios as u64).min(0x7FFF)) << 48,
        Ordering::Release,
    );
    match ns {
        Some(n) => crate::ring0::cabina::count("gpu", "cuadro MEDIDO por la linea que barre, us", n / 1000),
        None => crate::ring0::cabina::warn("gpu", "la linea NO dio la vuelta: sin VBLANK por MMIO, cambios", med.cambios as u64),
    }
}

fn buscar() -> Option<(u8, u8, u8, u16)> {
    let pci = crate::ring0::dev::pci::cfg_read32;
    for bus in 0u16..=255 {
        let bus = bus as u8;
        for dev in 0u8..32 {
            if pci(bus, dev, 0, 0) == 0xFFFF_FFFF {
                continue;
            }
            let multi = (pci(bus, dev, 0, 0x0C) >> 16) & 0x80 != 0;
            for func in 0..if multi { 8u8 } else { 1 } {
                let vd = pci(bus, dev, func, 0);
                if vd & 0xFFFF != NVIDIA {
                    continue;
                }
                if (pci(bus, dev, func, 0x08) >> 24) as u8 == CLASE_PANTALLA {
                    return Some((bus, dev, func, (vd >> 16) as u16));
                }
            }
        }
    }
    None
}

// -- Lo que sube a Ring 3 ----------------------------------------------------

/// `INFO_GPU_CHIP`.
pub fn info_chip() -> u64 {
    CHIP.load(Ordering::Acquire)
}
/// `INFO_GPU_MODO`.
pub fn info_modo() -> u64 {
    MODO.load(Ordering::Acquire)
}
/// `INFO_GPU_BORRADO`.
pub fn info_borrado() -> u64 {
    BORRADO.load(Ordering::Acquire)
}
/// `INFO_GPU_TIEMPO`.
pub fn info_tiempo() -> u64 {
    TIEMPO.load(Ordering::Acquire)
}

/// `INFO_GPU_LINEA`: la linea que barre AHORA, leida al preguntar. Una lectura
/// de MMIO por el physmap, que todo espacio comparte: vale bajo cualquier CR3.
pub fn info_linea() -> u64 {
    let bar0 = BAR0.load(Ordering::Acquire);
    let c = CHIP.load(Ordering::Acquire);
    let m = MODO.load(Ordering::Acquire);
    if bar0 == 0 || c & GPU_AMPERE == 0 || m & GPU_MODO_VALIDO == 0 {
        return 0;
    }
    let cabeza = ((c >> GPU_CABEZA_SHIFT) & 0x7) as u32;
    let v = leer(bar0, ga10x::linea(cabeza));
    if ga10x::es_error_pri(v) {
        return 0;
    }
    let b = BORRADO.load(Ordering::Acquire);
    let modo = ga10x::Modo {
        htotal: (m >> 32) as u16,
        vtotal: ((m >> 48) & 0x7FFF) as u16,
        hinicio: 0,
        hfin: 0,
        vinicio: b as u16,
        vfin: (b >> 16) as u16,
        reloj_hz: 0,
    };
    let l = v as u16;
    GPU_LINEA_VALIDA | l as u64 | if modo.en_vblank(l) { GPU_LINEA_VBLANK } else { 0 }
}
