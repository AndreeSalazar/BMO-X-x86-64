//! **LA IOMMU, PREGUNTADA** -- si el firmware la dejo encendida, que sabe
//! hacer, y a quien atiende.
//!
//! [carril]  AMARILLO  LEE la IOMMU por MMIO y el IVRS: ni un bit escrito
//! [prueba]  bmo-iommu-amdvi, bmo-firmware -- lo que significa cada numero
//! [consumo] NADA      corre una vez al arrancar, y una lectura cuando alguien pregunta
//!
//! [eje]     CORRECCION -- es la pregunta de LEY 24 antes de encender nada
//! [riesgo]  AJENO -- los registros son de un aparato que BMO-X no inicializo:
//!           lo que dicen es lo que dejo el firmware
//!
//! # Por que existe (2026-09-23, M0a de `docs/plan/PLAN_LA_3060.md`)
//!
//! El VBLANK por interrupcion de la RTX 3060 llega por MSI, y un MSI es una
//! ESCRITURA de la tarjeta: pide encenderle el Bus Master, que hoy tiene
//! apagado. El propietario decidio que eso va detras de la IOMMU. Y antes de
//! encenderla, se pregunta -- como se pregunto la grafica:
//!
//! ```text
//!    el IVRS          el IVHD que se usa, a que BDF atiende (el mayor mide
//!                     la tabla), los alias, el IOAPIC y el HPET, y la
//!                     memoria que el firmware exige (IVMD)
//!    los registros    control (la dejo el firmware encendida?), funciones,
//!                     la tabla de dispositivos que haya, y el estado
//! ```
//!
//! # Lo que NO hace, a proposito
//!
//! No escribe NADA: ni en sus registros ni en su configuracion PCI. Lo que
//! significa cada numero vive en `bmo-iommu-amdvi` y en `bmo_firmware::ivrs`,
//! que se prueban en el anfitrion; esto es el pegamento.

use bmo_firmware::ivrs;
use bmo_iommu_amdvi as amdvi;
use core::sync::atomic::{AtomicU64, Ordering};

/// Especiales e IVMD que se guardan. Un Zen trae un IOAPIC o dos y un HPET.
pub const MAX_ESPECIALES: usize = 8;
pub const MAX_IVMD: usize = 8;
/// El physmap cubre `0..16 GiB`: unos registros por encima no se leen por el.
const PHYSMAP_TOPE: u64 = 16 << 30;

// -- El contrato, espejo de `bmo_abi::...::informe::IOMMU_*` ------------------

pub const IOMMU_BASE_PAGINAS_MASK: u64 = 0xF_FFFF_FFFF;
pub const IOMMU_BDF_SHIFT: u64 = 36;
pub const IOMMU_TIPO_SHIFT: u64 = 52;
pub const IOMMU_MUDA: u64 = 1 << 62;
pub const IOMMU_HALLADA: u64 = 1 << 63;

pub const IOMMU_CENSO_UNOS_SHIFT: u64 = 16;
pub const IOMMU_CENSO_RANGOS_SHIFT: u64 = 24;
pub const IOMMU_CENSO_ALIAS_SHIFT: u64 = 32;
pub const IOMMU_CENSO_ESPECIALES_SHIFT: u64 = 40;
pub const IOMMU_CENSO_IVMD_SHIFT: u64 = 48;
pub const IOMMU_CENSO_RARAS_SHIFT: u64 = 56;
pub const IOMMU_CENSO_CORTADO: u64 = 1 << 60;
pub const IOMMU_CENSO_TODOS: u64 = 1 << 61;
pub const IOMMU_CENSO_VALIDO: u64 = 1 << 63;

pub const IOMMU_INDICE_SHIFT: u64 = 8;
pub const IOMMU_PARTE_SHIFT: u64 = 12;
pub const IOMMU_VALIDA: u64 = 1 << 63;

/// `0..35` base en paginas de 4 KiB, `36..51` BDF de la IOMMU, `52..59` tipo
/// del IVHD elegido, 62 muda, 63 hallada.
static DONDE: AtomicU64 = AtomicU64::new(0);
static CONTROL: AtomicU64 = AtomicU64::new(0);
static ESTADO: AtomicU64 = AtomicU64::new(0);
static FUNCIONES: AtomicU64 = AtomicU64::new(0);
static TABLA: AtomicU64 = AtomicU64::new(0);
static CENSO: AtomicU64 = AtomicU64::new(0);
static ESPECIALES: [AtomicU64; MAX_ESPECIALES] = [const { AtomicU64::new(0) }; MAX_ESPECIALES];
/// Tres palabras por IVMD: inicio, largo, y `0..15 bdf | 16..31 aux | 32..39
/// tipo | 40..47 banderas | 63 valido`.
static IVMD: [[AtomicU64; 3]; MAX_IVMD] = [const { [const { AtomicU64::new(0) }; 3] }; MAX_IVMD];

fn sat(v: u16, max: u64) -> u64 {
    (v as u64).min(max)
}

/// **La pregunta.** Una vez, al arrancar, despues del censo de la placa.
pub fn sondear(rsdp: u64) {
    let Some(t) = crate::ring0::plat::placa::ivrs(rsdp) else {
        return;
    };
    let Some(bloque) = ivrs::ivhd_elegido(t) else {
        crate::ring0::cabina::warn("iommu", "el IVRS no trae un IVHD que se lea", 0);
        return;
    };
    // La cabecera: los mismos campos que `leer_ivrs`, del bloque ELEGIDO.
    let bdf = u16::from_le_bytes([bloque[4], bloque[5]]);
    let mut b8 = [0u8; 8];
    b8.copy_from_slice(&bloque[8..16]);
    let base = u64::from_le_bytes(b8);

    let mut esp = [ivrs::Especial::default(); MAX_ESPECIALES];
    let (c, n) = ivrs::censar(bloque, &mut esp);
    for (i, e) in esp[..n].iter().enumerate() {
        ESPECIALES[i].store(
            IOMMU_VALIDA | e.bdf as u64 | (e.handle as u64) << 16 | (e.tipo as u64) << 24 | (e.banderas as u64) << 32,
            Ordering::Release,
        );
    }
    let mut m = [ivrs::Ivmd::default(); MAX_IVMD];
    let nm = ivrs::leer_ivmd(t, &mut m);
    for (i, v) in m[..nm].iter().enumerate() {
        IVMD[i][0].store(v.inicio, Ordering::Release);
        IVMD[i][1].store(v.largo, Ordering::Release);
        IVMD[i][2].store(
            IOMMU_VALIDA | v.bdf as u64 | (v.aux as u64) << 16 | (v.tipo as u64) << 32 | (v.banderas as u64) << 40,
            Ordering::Release,
        );
    }
    CENSO.store(
        IOMMU_CENSO_VALIDO
            | c.max_bdf as u64
            | sat(c.unos, 0xFF) << IOMMU_CENSO_UNOS_SHIFT
            | sat(c.rangos, 0xFF) << IOMMU_CENSO_RANGOS_SHIFT
            | sat(c.alias, 0xFF) << IOMMU_CENSO_ALIAS_SHIFT
            | sat(c.especiales, 0xFF) << IOMMU_CENSO_ESPECIALES_SHIFT
            | (nm as u64).min(0xFF) << IOMMU_CENSO_IVMD_SHIFT
            | sat(c.desconocidas.saturating_add(c.por_hid), 0xF) << IOMMU_CENSO_RARAS_SHIFT
            | if c.cortado { IOMMU_CENSO_CORTADO } else { 0 }
            | if c.todos > 0 { IOMMU_CENSO_TODOS } else { 0 },
        Ordering::Release,
    );

    let mut d = IOMMU_HALLADA
        | ((base >> 12) & IOMMU_BASE_PAGINAS_MASK)
        | (bdf as u64) << IOMMU_BDF_SHIFT
        | (bloque[0] as u64) << IOMMU_TIPO_SHIFT;
    if base == 0 || base >= PHYSMAP_TOPE {
        // No se inventa una lectura: se dice que no se leyo.
        DONDE.store(d | IOMMU_MUDA, Ordering::Release);
        crate::ring0::cabina::warn("iommu", "sus registros no caen en el physmap: no se leen", base);
        return;
    }
    let v = crate::ring0::mm::phys_to_virt(base);
    let leer = |reg: u32| -> u64 {
        // SAFETY: registros de 64 bits alineados de la IOMMU, por el physmap
        // (no cacheable por el MTRR de la placa, como el ABAR del AHCI). Solo
        // lecturas, las mismas que hace Linux antes de tocar nada.
        unsafe { ((v + reg as u64) as *const u64).read_volatile() }
    };
    let control = leer(amdvi::CONTROL);
    let funciones = leer(amdvi::FUNCIONES);
    CONTROL.store(control, Ordering::Release);
    FUNCIONES.store(funciones, Ordering::Release);
    ESTADO.store(leer(amdvi::ESTADO), Ordering::Release);
    TABLA.store(leer(amdvi::TABLA_DISPOSITIVOS), Ordering::Release);
    if amdvi::es_muda(control) {
        d |= IOMMU_MUDA;
    }
    DONDE.store(d, Ordering::Release);

    crate::ring0::cabina::addr("iommu", "registros de la IOMMU (IVHD elegido)", base);
    crate::ring0::cabina::bits("iommu", "  ...control, como lo dejo el firmware", control);
    crate::ring0::cabina::bits("iommu", "  ...funciones (EFR)", funciones);
    crate::ring0::cabina::count("iommu", "  ...mayor BDF que nombra el IVRS", c.max_bdf as u64);
    if amdvi::Control(control).encendida() {
        crate::ring0::cabina::warn("iommu", "el firmware la dejo TRADUCIENDO: se hereda, no se pisa", control);
    }
}

pub fn info_donde() -> u64 {
    DONDE.load(Ordering::Acquire)
}
pub fn info_control() -> u64 {
    CONTROL.load(Ordering::Acquire)
}
pub fn info_estado() -> u64 {
    ESTADO.load(Ordering::Acquire)
}
pub fn info_funciones() -> u64 {
    FUNCIONES.load(Ordering::Acquire)
}
pub fn info_tabla() -> u64 {
    TABLA.load(Ordering::Acquire)
}
pub fn info_censo() -> u64 {
    CENSO.load(Ordering::Acquire)
}

/// `INFO_IOMMU_ESPECIAL | i << 8`: el especial `i`, o 0.
pub fn info_especial(sel: u64) -> u64 {
    let i = ((sel >> IOMMU_INDICE_SHIFT) & 0xF) as usize;
    ESPECIALES.get(i).map_or(0, |e| e.load(Ordering::Acquire))
}

/// `INFO_IOMMU_IVMD | i << 8 | parte << 12`: la palabra `parte` (0 inicio, 1
/// largo, 2 lo demas) del IVMD `i`, o 0.
pub fn info_ivmd(sel: u64) -> u64 {
    let i = ((sel >> IOMMU_INDICE_SHIFT) & 0xF) as usize;
    let p = ((sel >> IOMMU_PARTE_SHIFT) & 0x3) as usize;
    match (IVMD.get(i), p) {
        (Some(w), 0..=2) => w[p].load(Ordering::Acquire),
        _ => 0,
    }
}
