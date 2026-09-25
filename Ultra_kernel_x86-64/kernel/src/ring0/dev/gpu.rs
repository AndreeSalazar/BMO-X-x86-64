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
/// Donde esta en el bus: `bus << 8 | dev << 3 | func`, con el bit 63 si se
/// hallo. Lo pide la IOMMU para cegarla (M0e).
static BDF: AtomicU64 = AtomicU64::new(0);

/// **El BDF de la NVIDIA**, si la sonda la hallo.
pub fn bdf() -> Option<(u8, u8, u8)> {
    let v = BDF.load(Ordering::Acquire);
    if v >> 63 == 0 {
        return None;
    }
    Some(((v >> 8) as u8, ((v >> 3) & 0x1F) as u8, (v & 7) as u8))
}

/// BAR0 por el physmap (`0` = no hay grafica que leer). Para E2 (`dev/vblank.rs`).
pub fn bar0() -> u64 {
    BAR0.load(Ordering::Acquire)
}

/// La cabeza que pinta, si la sonda hallo un Ampere con un modo de verdad.
pub fn cabeza() -> Option<u32> {
    let c = CHIP.load(Ordering::Acquire);
    if c & GPU_AMPERE == 0 || MODO.load(Ordering::Acquire) & GPU_MODO_VALIDO == 0 {
        return None;
    }
    Some(((c >> GPU_CABEZA_SHIFT) & 0x7) as u32)
}

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
pub const GPU_LINEA_PARADAS_SHIFT: u64 = 32;
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
    BDF.store(1 << 63 | (bus as u64) << 8 | (dev as u64) << 3 | func as u64, Ordering::Release);
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
    // ** LA FOTO AL LLEGAR: la WPR2 y el enlace crudos al sondear, SOLO para
    // mirarlos. NO es un veredicto: el metal (24-09 14:09) trajo aqui una
    // WPR2 "con techo" y aun asi FWSEC-FRTS la encontro vacia y la monto
    // limpia -- al sondear el firmware de arranque de la tarjeta aun no ha
    // acabado y estos registros no dicen nada todavia.
    FRIO_WPR2.store(info_wpr2() | 1 << 63, Ordering::Release);
    FRIO_ENLACE.store(info_salud(1 << 8), Ordering::Release);

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
    // Las veces que el rayo se hallo PARADO van en los bits 32..47, se lea
    // ahora o no: un rayo que se paro una vez es un dato aunque hoy se mueva.
    let paradas = paradas().min(0xFFFF) << GPU_LINEA_PARADAS_SHIFT;
    let Some((modo, l)) = rayo_ahora() else { return paradas };
    paradas | GPU_LINEA_VALIDA | l as u64 | if modo.en_vblank(l) { GPU_LINEA_VBLANK } else { 0 }
}

// == EL RAYO PARADO (2026-09-24) ================================================
//
// Todo lo que espera al rayo cree que se MUEVE: si la cabeza deja de barrer
// -- la tarjeta se para, o pierde lo que lee -- la linea se queda quieta, y
// quien pregunta "cuanto falta" recibe "un poco" para siempre. El Ryzen se
// quedo congelado en la intro del escritorio el 24-09, y un rayo quieto es la
// forma exacta de ese cuelgue. Asi que el kernel lo MIRA: la misma linea que
// hace mas de 20 ms puede ser casualidad (justo un cuadro despues), asi que se
// CONFIRMA releyendo 60 us, en los que un rayo vivo baja ~4 lineas. Si no se
// movio, esta PARADO: se contesta "no hay rayo" y el que copia copia sin
// esperar. Se cuenta, y se dice una vez en CABINA.

/// La ultima linea vista + 1 (0 = ninguna) y cuando, en TSC.
static VISTA: AtomicU64 = AtomicU64::new(0);
static VISTA_TSC: AtomicU64 = AtomicU64::new(0);
/// Veces que se confirmo el rayo PARADO.
static PARADAS: AtomicU64 = AtomicU64::new(0);

fn parado(bar0: u64, cabeza: u32, l: u16) -> bool {
    use crate::ring0::task::scheduler::{rdtsc, tsc_freq};
    let t = rdtsc();
    if VISTA.load(Ordering::Acquire) != l as u64 + 1 {
        VISTA.store(l as u64 + 1, Ordering::Release);
        VISTA_TSC.store(t, Ordering::Release);
        return false;
    }
    let hz = tsc_freq().max(1);
    if t.saturating_sub(VISTA_TSC.load(Ordering::Acquire)) < hz / 50 {
        return false;
    }
    let fin = t.saturating_add(hz / 1_000_000 * 60);
    while rdtsc() < fin {
        let v = leer(bar0, ga10x::linea(cabeza));
        if !ga10x::es_error_pri(v) && v as u16 != l {
            VISTA.store(v as u64 + 1, Ordering::Release);
            VISTA_TSC.store(rdtsc(), Ordering::Release);
            return false;
        }
        core::hint::spin_loop();
    }
    if PARADAS.fetch_add(1, Ordering::AcqRel) == 0 {
        crate::ring0::cabina::warn("gpu", "el RAYO esta PARADO: la linea no se mueve; nadie le espera", l as u64);
    }
    true
}

/// Veces que el rayo se encontro PARADO (`INFO_GPU_LINEA`, bits 32..47).
pub fn paradas() -> u64 {
    PARADAS.load(Ordering::Acquire)
}

/// El modo guardado y la linea que barre AHORA. `None` sin grafica que leer,
/// o con el rayo PARADO.
fn rayo_ahora() -> Option<(ga10x::Modo, u16)> {
    let bar0 = BAR0.load(Ordering::Acquire);
    let c = CHIP.load(Ordering::Acquire);
    let m = MODO.load(Ordering::Acquire);
    if bar0 == 0 || c & GPU_AMPERE == 0 || m & GPU_MODO_VALIDO == 0 {
        return None;
    }
    let cabeza = ((c >> GPU_CABEZA_SHIFT) & 0x7) as u32;
    let v = leer(bar0, ga10x::linea(cabeza));
    if ga10x::es_error_pri(v) {
        return None;
    }
    if parado(bar0, cabeza, v as u16) {
        return None;
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
    Some((modo, v as u16))
}

// -- ** EL VOLCADO DETRAS DEL RAYO (E1, 2026-09-23) ---------------------------

pub const GPU_ESPERA_Y0_SHIFT: u64 = 8;
pub const GPU_ESPERA_Y1_SHIFT: u64 = 20;
pub const GPU_ESPERA_FILAS_MASK: u64 = 0xFFF;
pub const GPU_ESPERA_NS_FILA_SHIFT: u64 = 32;
pub const GPU_ESPERA_NS_FILA_MASK: u64 = 0xFFFF;
pub const GPU_ESPERA_NS_MASK: u64 = 0xFFFF_FFFF;
pub const GPU_ESPERA_NO_CABE: u64 = 1 << 61;
pub const GPU_ESPERA_VALIDA: u64 = 1 << 63;

/// **`INFO_GPU_ESPERA`: cuanto esperar para copiar las filas `[y0, y1)` sin
/// que el rayo las barra a medio copiar.** Lo pregunta quien vuelca, una vez
/// por caja; lo que tarda su copia por fila lo MIDE el (`ns_fila`), y el
/// kernel pone lo que solo el sabe: donde va el rayo ahora y lo que dura una
/// linea MEDIDA. La cuenta es de `bmo_gpu_ga10x::Modo::espera`.
///
/// `0` = no hay rayo que mirar (otra placa, o no se midio): se copia sin mas.
pub fn info_espera(sel: u64) -> u64 {
    let t = TIEMPO.load(Ordering::Acquire);
    if t & GPU_TIEMPO_MEDIDO == 0 {
        return 0;
    }
    let Some((modo, l)) = rayo_ahora() else { return 0 };
    let periodo = t & 0xFFFF_FFFF;
    let linea_ns = (periodo / modo.vtotal.max(1) as u64).max(1);
    let y0 = ((sel >> GPU_ESPERA_Y0_SHIFT) & GPU_ESPERA_FILAS_MASK) as u16;
    let y1 = ((sel >> GPU_ESPERA_Y1_SHIFT) & GPU_ESPERA_FILAS_MASK) as u16;
    let ns_fila = (sel >> GPU_ESPERA_NS_FILA_SHIFT) & GPU_ESPERA_NS_FILA_MASK;
    let filas = y1.saturating_sub(y0) as u64;
    let copia = (filas * ns_fila).div_ceil(linea_ns) as u32;
    let Some(e) = modo.espera(l, y0, y1, copia) else { return 0 };
    let ns = (e.lineas as u64 * linea_ns).min(GPU_ESPERA_NS_MASK);
    GPU_ESPERA_VALIDA | ns | if e.cabe { 0 } else { GPU_ESPERA_NO_CABE }
}

// -- ** L0a: LO QUE FWSEC NECESITA, PREGUNTADO (2026-09-24) -------------------
//
// Todo lectura, como el resto de este fichero. La ROM se sirve de OCHO en ocho
// bytes y la recorre el escritorio: leer su MiB entero aqui dentro seria un
// cuarto de segundo con las interrupciones cerradas. Quien la entiende es
// `bmo_gpu_ga10x::vbios`, en el escritorio.

pub const GPU_FB_GFW_SHIFT: u64 = 32;
pub const GPU_FB_PLM_LEIBLE: u64 = 1 << 40;
pub const GPU_FB_SIN_PANTALLA: u64 = 1 << 41;
pub const GPU_FB_VALIDA: u64 = 1 << 63;

/// `INFO_GPU_ROM`: ocho bytes de la ROM desde el offset de los bits 8..27
/// (alineado a 8). `0` sin grafica o fuera de la ventana de 1 MiB.
pub fn info_rom(sel: u64) -> u64 {
    let bar0 = BAR0.load(Ordering::Acquire);
    let off = ((sel >> 8) & 0xF_FFF8) as u32;
    if bar0 == 0 || off as usize >= ga10x::vbios::ROM_MAX {
        return 0;
    }
    let lo = leer(bar0, ga10x::vbios::ROM + off) as u64;
    let hi = leer(bar0, ga10x::vbios::ROM + off + 4) as u64;
    lo | hi << 32
}

/// `INFO_GPU_FUSIBLE`: el registro de fusibles de los bits 8..39, crudo. Solo
/// los de version de ucode (`0x824100..0x824200`): lo demas contesta 0.
pub fn info_fusible(sel: u64) -> u64 {
    let bar0 = BAR0.load(Ordering::Acquire);
    let reg = ((sel >> 8) & 0xFFFF_FFFC) as u32;
    if bar0 == 0 || !(ga10x::vbios::FUSIBLE_NVDEC..ga10x::vbios::FUSIBLE_NVDEC + 0x100).contains(&reg) {
        return 0;
    }
    leer(bar0, reg) as u64
}

/// `INFO_GPU_FB`: `0..31` VRAM usable en MiB | `32..39` el progreso del
/// arranque del firmware (0xFF acabado) | 40 se pudo leer | 41 la pantalla
/// apagada por fusible | 63 valida. El progreso solo se lee si su mascara de
/// privilegio lo deja, como nova-core (`gfw.rs`).
pub fn info_fb() -> u64 {
    let bar0 = BAR0.load(Ordering::Acquire);
    if bar0 == 0 || CHIP.load(Ordering::Acquire) & GPU_AMPERE == 0 {
        return 0;
    }
    let mb = leer(bar0, 0x0011_83A4);
    let mut x = GPU_FB_VALIDA | if ga10x::es_error_pri(mb) { 0 } else { mb as u64 };
    let plm = leer(bar0, 0x0011_8128);
    if !ga10x::es_error_pri(plm) && plm & 1 != 0 {
        x |= GPU_FB_PLM_LEIBLE | ((leer(bar0, 0x0011_8234) & 0xFF) as u64) << GPU_FB_GFW_SHIFT;
    }
    if leer(bar0, 0x0082_0C04) & 1 != 0 {
        x |= GPU_FB_SIN_PANTALLA;
    }
    x
}

/// `INFO_GPU_VGA`: `NV_PDISP_VGA_WORKSPACE_BASE` crudo.
pub fn info_vga() -> u64 {
    let bar0 = BAR0.load(Ordering::Acquire);
    if bar0 == 0 { 0 } else { leer(bar0, 0x0062_5F04) as u64 }
}

/// `INFO_GPU_WPR2`: los dos limites de la WPR2 crudos (`0x1FA824` abajo,
/// `0x1FA828` arriba << 32). Arriba a 0 = no hay WPR2.
pub fn info_wpr2() -> u64 {
    let bar0 = BAR0.load(Ordering::Acquire);
    if bar0 == 0 {
        return 0;
    }
    leer(bar0, 0x001F_A824) as u64 | (leer(bar0, 0x001F_A828) as u64) << 32
}

// == LA SALUD, EN SOLO LECTURA (2026-09-24) ===================================
//
// Para el panel del escritorio: la temperatura del chip por su sensor
// (`bmo_gpu_ga10x::salud::TERMICO`, como nouveau) y el enlace PCIe por la
// capacidad PCI Express. Dos lecturas; ni un registro se escribe.

/// La WPR2 cruda al sondear (bit 63: se tomo), y el enlace (`LNKSTA | LNKCAP
/// << 32`) al sondear.
static FRIO_WPR2: AtomicU64 = AtomicU64::new(0);
static FRIO_ENLACE: AtomicU64 = AtomicU64::new(0);

/// ** LO QUE HIZO EL CARGADOR (25-09): si la 3060 llego CALIENTE y la
/// reinicio por el bus antes de `ExitBootServices` (`s1_cpu::gpu_reinicio`).
/// Las banderas `GPU_REINICIO_*` de `boot_context`, y las dos lecturas crudas
/// (RISC-V del GSP abajo, WPR2 arriba) antes y despues.
static CARGADOR: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

/// Lo apunta `phase::main` desde el `BootContext`, antes de `sondear`.
pub fn cargador(banderas: u64, antes: u64, despues: u64) {
    CARGADOR[0].store(banderas, Ordering::Release);
    CARGADOR[1].store(antes, Ordering::Release);
    CARGADOR[2].store(despues, Ordering::Release);
    if banderas & boot_context::GPU_REINICIO_HECHO != 0 {
        crate::ring0::cabina::warn("gpu", "el cargador REINICIO la 3060 por el bus (venia caliente); banderas", banderas);
    }
}

/// `INFO_GPU_SALUD`: selector 0 el sensor crudo, 1 `LNKSTA | LNKCAP << 32`;
/// 2 la WPR2 cruda AL SONDEAR (bit 63: se tomo); 3 el enlace AL SONDEAR;
/// 4, 5 y 6 lo que hizo el cargador (banderas, lecturas antes, despues).
pub fn info_salud(sel: u64) -> u64 {
    match sel >> 8 {
        2 => return FRIO_WPR2.load(Ordering::Acquire),
        3 => return FRIO_ENLACE.load(Ordering::Acquire),
        4..=6 => return CARGADOR[(sel >> 8) as usize - 4].load(Ordering::Acquire),
        _ => {}
    }
    let bar0 = bar0();
    let Some((b, d, f)) = bdf() else { return 0 };
    if sel >> 8 == 0 {
        if bar0 == 0 {
            return 0;
        }
        // Por el mismo `Bar0` que lee BOOT_0: leido y nada mas.
        let mut r = crate::ring0::dev::gpu_prestamo::Bar0(bar0);
        return bmo_gpu_ga10x::Registros::leer(&mut r, bmo_gpu_ga10x::salud::TERMICO) as u64;
    }
    let pci = crate::ring0::dev::pci::cfg_read32;
    // La lista de capacidades: el bit 4 del estado dice que la hay.
    if pci(b, d, f, 0x04) >> 16 & 0x10 == 0 {
        return 0;
    }
    let mut p = (pci(b, d, f, 0x34) & 0xFC) as u8;
    let mut vueltas = 0;
    // `p + 0x10` en un `u8`: una capacidad no empieza pasado 0xEC.
    while p != 0 && p <= 0xEC && vueltas < 48 {
        let c = pci(b, d, f, p);
        if c & 0xFF == 0x10 {
            let cap = pci(b, d, f, p + 0x0C) as u64;
            let sta = (pci(b, d, f, p + 0x10) >> 16) as u64;
            return sta | cap << 32;
        }
        p = ((c >> 8) & 0xFC) as u8;
        vueltas += 1;
    }
    0
}
