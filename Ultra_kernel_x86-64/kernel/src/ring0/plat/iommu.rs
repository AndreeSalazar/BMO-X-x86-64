//! **LA IOMMU, PREGUNTADA Y ENCENDIDA** -- si el firmware la dejo encendida,
//! que sabe hacer, a quien atiende; y desde M0c, encenderla por orden.
//!
//! [carril]  ROJO      ESCRIBE en la IOMMU (M0c): la frontera del DMA de todo aparato
//! [prueba]  bmo-iommu-amdvi, bmo-firmware -- lo que significa cada numero
//! [consumo] NADA      corre una vez al arrancar, y cuando el propietario lo teclea
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
//! # Lo que escribe, y cuando
//!
//! Al arrancar, NADA en la IOMMU: solo lee (M0a) y arma sus tablas en RAM
//! sin entregarlas (M0b). Escribe en sus registros UNICAMENTE por
//! `encender`/`apagar` (M0c), que llegan por `TASK_OP_IOMMU` desde el
//! escritorio. Ni una escritura en su configuracion PCI. Lo que significa cada
//! numero vive en `bmo-iommu-amdvi` y en `bmo_firmware::ivrs`, que se prueban
//! en el anfitrion; esto es el pegamento.

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
    // ** M0b: si se puede encender, sus tablas se ARMAN ya -- y no se le
    // entregan. Ver `armar`.
    if amdvi::veredicto(control, funciones) == amdvi::Veredicto::SePuede {
        armar(bloque, c.max_bdf);
    }
}

// == M0b: LAS TABLAS, ARMADAS EN RAM Y SIN ENTREGAR (2026-09-24) ==============
//
// La tabla de dispositivos (hasta 2 MiB), la cola de ordenes y el registro de
// eventos (8 KiB cada uno) se piden y se llenan AQUI, al arrancar, con lo que
// dice `bmo_iommu_amdvi::tablas` -- y NINGUN registro de la IOMMU se toca. Lo
// que el metal contesta con esto, antes de encender nada en M0c:
//
//    - que hay 2 MiB CONTIGUOS para la tabla (los pide el campo de medida)
//    - que lo escrito por el physmap se LEE igual, entrada a entrada
//    - cuantas entradas llevan banderas del IVHD (INIT/NMI/LINT/SysMgt)
//
// Cada BDF queda DE PASO (V + TV + IR + IW, modo 0): lo que M0c entregara para
// encender sin que ningun aparato note nada. El dominio es el 1: Linux no usa
// el 0 (`pdom_id_alloc` empieza en 1).

/// Entradas de cada cola: 512 x 16 B = 8 KiB (`CMD_BUFFER_ENTRIES`,
/// `EVTLOG_SIZE_DEF`).
const ENTRADAS_COLA: u32 = 512;
const BYTES_COLA: u64 = ENTRADAS_COLA as u64 * amdvi::tablas::ORDEN as u64;
/// El dominio de las entradas de paso.
const DOMINIO_PASO: u16 = 1;

pub const IOMMU_ARMADO_PAGINAS_SHIFT: u64 = 36;
pub const IOMMU_ARMADO_BANDERAS_SHIFT: u64 = 48;
pub const IOMMU_ARMADO_COMPROBADO: u64 = 1 << 62;
pub const IOMMU_ARMADO_SI: u64 = 1 << 63;

/// `0..35` base de la tabla en paginas | `36..47` sus paginas | `48..61`
/// entradas con banderas del IVHD | 62 releida igual | 63 armada.
static ARMADO: AtomicU64 = AtomicU64::new(0);
/// `0..35` base de las colas en paginas (ordenes, y a +8 KiB eventos) |
/// `36..51` entradas por cola | 63 armadas.
static COLAS: AtomicU64 = AtomicU64::new(0);

fn armar(bloque: &[u8], max_bdf: u16) {
    use crate::ring0::mm::phys;
    use amdvi::tablas::{self as t, Dte};
    let bytes = amdvi::tabla_para(max_bdf);
    let paginas = bytes / 4096;
    // La IOMMU lee estas tablas por DMA: son NEUTRO, como los buferes del disco.
    let Some(base) = phys::alloc_frames_contig_de(paginas, phys::Titular::Neutro) else {
        crate::ring0::cabina::warn("iommu", "no hay paginas CONTIGUAS para la tabla de dispositivos", paginas);
        return;
    };
    // Ordenes (8 KiB) + eventos (8 KiB) + una pagina para el SEMAFORO: donde
    // la IOMMU escribe el dato de COMPLETION_WAIT (M0c).
    let Some(colas) = phys::alloc_frames_contig_de(2 * BYTES_COLA / 4096 + 1, phys::Titular::Neutro) else {
        crate::ring0::cabina::warn("iommu", "no hay paginas contiguas para las colas", 0);
        return;
    };
    let v = crate::ring0::mm::phys_to_virt(base);
    let vc = crate::ring0::mm::phys_to_virt(colas);
    // SAFETY: `paginas` marcos contiguos recien entregados por el asignador
    // a este fichero, por el physmap; nadie mas los tiene. Y las dos colas,
    // igual. La IOMMU no sabe todavia que existen: ningun registro apunta aqui.
    let tabla = unsafe {
        core::ptr::write_bytes(vc as *mut u8, 0, (2 * BYTES_COLA + 4096) as usize);
        core::slice::from_raw_parts_mut(v as *mut u64, (bytes / 8) as usize)
    };
    let n = t::llenar(tabla, Dte::de_paso(DOMINIO_PASO));
    let mut con_banderas = 0u64;
    let r = ivrs::por_entrada(bloque, |tr| {
        if tr.banderas == 0 {
            return;
        }
        let mut marca = |b: u16| {
            if let Some(e) = t::leer(tabla, b) {
                t::poner(tabla, b, e.con_banderas_ivhd(tr.banderas));
                con_banderas += 1;
            }
        };
        let mut b = tr.desde as u32;
        while b <= tr.hasta as u32 {
            marca(b as u16);
            b += 1;
        }
        if let Some(a) = tr.alias {
            marca(a);
        }
    });
    // Releer: cada entrada tiene que ser de paso, con banderas o sin ellas.
    let mut bien = true;
    for b in 0..n {
        match t::leer(tabla, b as u16) {
            Some(e) if e.valida() && e.traduce() && e.lee() && e.escribe() && e.dominio() == DOMINIO_PASO => {}
            _ => {
                bien = false;
                break;
            }
        }
    }
    // Y los registros que M0c escribira, comprobados contra su lectura.
    let reg_tabla = t::registro_tabla(base, bytes);
    let reg_ordenes = t::registro_cola(colas, ENTRADAS_COLA);
    bien &= reg_tabla.and_then(amdvi::Tabla::de_registro) == Some(amdvi::Tabla { base, bytes })
        && reg_ordenes.and_then(amdvi::Cola::de_registro) == Some(amdvi::Cola { base: colas, entradas: ENTRADAS_COLA });
    ARMADO.store(
        IOMMU_ARMADO_SI
            | (base >> 12) & IOMMU_BASE_PAGINAS_MASK
            | (paginas & 0xFFF) << IOMMU_ARMADO_PAGINAS_SHIFT
            | con_banderas.min(0x3FFF) << IOMMU_ARMADO_BANDERAS_SHIFT
            | if bien { IOMMU_ARMADO_COMPROBADO } else { 0 },
        Ordering::Release,
    );
    COLAS.store(IOMMU_ARMADO_SI | (colas >> 12) & IOMMU_BASE_PAGINAS_MASK | (ENTRADAS_COLA as u64) << 36, Ordering::Release);
    crate::ring0::cabina::addr("iommu", "M0b: tabla de dispositivos ARMADA (sin entregar)", base);
    crate::ring0::cabina::count("iommu", "  ...entradas de paso", n as u64);
    crate::ring0::cabina::count("iommu", "  ...con banderas del IVHD", con_banderas);
    if !bien || r.cortado {
        crate::ring0::cabina::warn("iommu", "M0b: la tabla releida NO dice lo que se escribio", base);
    }
}

pub fn info_armado() -> u64 {
    ARMADO.load(Ordering::Acquire)
}
pub fn info_colas() -> u64 {
    COLAS.load(Ordering::Acquire)
}

pub fn info_donde() -> u64 {
    DONDE.load(Ordering::Acquire)
}
/// Los registros EN VIVO si se pueden leer; si no, la foto del arranque. Desde
/// M0c el control cambia despues de arrancar, y una foto vieja mentiria.
fn vivo(reg: u32, foto: &AtomicU64) -> u64 {
    match registros() {
        // SAFETY: registros de la IOMMU por el physmap, como en `sondear`.
        Some(v) => unsafe { ((v + reg as u64) as *const u64).read_volatile() },
        None => foto.load(Ordering::Acquire),
    }
}
pub fn info_control() -> u64 {
    vivo(amdvi::CONTROL, &CONTROL)
}
pub fn info_estado() -> u64 {
    vivo(amdvi::ESTADO, &ESTADO)
}
pub fn info_funciones() -> u64 {
    FUNCIONES.load(Ordering::Acquire)
}
pub fn info_tabla() -> u64 {
    vivo(amdvi::TABLA_DISPOSITIVOS, &TABLA)
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

// == M0c: ENCENDER, CON LA VUELTA ATRAS DENTRO (2026-09-24) ====================
//
// La primera ESCRITURA en la IOMMU. Solo por orden del propietario (`iommu
// encender`, `TASK_OP_IOMMU`), nunca al arrancar: si algo sale mal, un
// reinicio lo borra todo, porque nada de esto queda escrito en ningun sitio.
//
//    1. entregar la tabla de M0b, la cola de ordenes y el registro de eventos
//    2. encender la cola y los eventos, y luego la IOMMU (todo DE PASO: ningun
//       aparato deberia notar nada)
//    3. INVALIDATE_IOMMU_ALL y COMPLETION_WAIT: "cuando acabes, escribe esto"
//    4. si el dato llega, la IOMMU OBEDECE. Si en 10 ms no llega, se APAGA
//       otra vez con el control que tenia, y se dice por que
//
// El orden es el de Linux (`iommu_enable_command_buffer`,
// `iommu_enable_event_buffer`, `iommu_enable`, `amd_iommu_flush_all_caches`).

/// Motivos del NO de `TASK_OP_IOMMU`. Espejo de `bmo_abi::...::IOMMU_NO_*`.
pub const IOMMU_NO_ESCRITORIO: u32 = 1;
pub const IOMMU_NO_TABLAS: u32 = 2;
pub const IOMMU_NO_YA_ENCENDIDA: u32 = 3;
pub const IOMMU_NO_CONTESTA: u32 = 4;
pub const IOMMU_NO_APAGADA: u32 = 5;

pub const IOMMU_VIVA_EVENTOS_SHIFT: u64 = 32;
pub const IOMMU_VIVA_MOTIVO_SHIFT: u64 = 48;
pub const IOMMU_VIVA_INTENTOS_SHIFT: u64 = 56;
pub const IOMMU_VIVA_CONTESTO: u64 = 1 << 62;
pub const IOMMU_VIVA_ENCENDIDA: u64 = 1 << 63;

/// `0..31` us que tardo el COMPLETION_WAIT | `32..47` eventos en el registro |
/// `48..55` el ultimo motivo del NO | `56..59` intentos | 62 contesto |
/// 63 ENCENDIDA por BMO-X ahora.
static VIVA: AtomicU64 = AtomicU64::new(0);
/// El control que dejo el firmware: lo que se restaura al apagar.
static CONTROL_ORIGINAL: AtomicU64 = AtomicU64::new(0);

/// El dato que la IOMMU escribe al acabar. Cualquiera distinto de 0 vale; este
/// se reconoce en un volcado.
const SEMAFORO_DATO: u64 = 0xB0B0_1000_0000_0001;
const ESPERA_MAX_MS: u64 = 10;
const CONTROL_EN: u64 = 1 << 0;
const CONTROL_EVENTOS: u64 = 1 << 2;
const CONTROL_ORDENES: u64 = 1 << 12;

/// La base virtual de sus registros, si se pueden tocar.
fn registros() -> Option<u64> {
    let d = DONDE.load(Ordering::Acquire);
    if d & IOMMU_HALLADA == 0 || d & IOMMU_MUDA != 0 {
        return None;
    }
    Some(crate::ring0::mm::phys_to_virt((d & IOMMU_BASE_PAGINAS_MASK) << 12))
}

fn apuntar_viva(f: impl FnOnce(u64) -> u64) {
    let v = VIVA.load(Ordering::Acquire);
    VIVA.store(f(v), Ordering::Release);
}

fn fallo(motivo: u32) -> Result<u64, u32> {
    apuntar_viva(|v| (v & !(0xFF << IOMMU_VIVA_MOTIVO_SHIFT)) | (motivo as u64) << IOMMU_VIVA_MOTIVO_SHIFT);
    Err(motivo)
}

/// **Encender.** `Ok(us | eventos << 32)`, o el motivo del NO.
pub fn encender() -> Result<u64, u32> {
    use amdvi::tablas::{self as t, Orden};
    apuntar_viva(|v| {
        let n = ((v >> IOMMU_VIVA_INTENTOS_SHIFT) & 0xF).saturating_add(1).min(0xF);
        (v & !(0xF << IOMMU_VIVA_INTENTOS_SHIFT)) | n << IOMMU_VIVA_INTENTOS_SHIFT
    });
    let a = ARMADO.load(Ordering::Acquire);
    let c = COLAS.load(Ordering::Acquire);
    let Some(v) = registros() else { return fallo(IOMMU_NO_TABLAS) };
    if a & IOMMU_ARMADO_COMPROBADO == 0 || c & IOMMU_ARMADO_SI == 0 {
        return fallo(IOMMU_NO_TABLAS);
    }
    // SAFETY: los registros de la IOMMU por el physmap, no cacheable por el
    // MTRR (como el ABAR del AHCI). Lo que se escribe es lo de Linux.
    let rd = |r: u32| unsafe { ((v + r as u64) as *const u64).read_volatile() };
    let wr = |r: u32, x: u64| unsafe { ((v + r as u64) as *mut u64).write_volatile(x) };
    let control = rd(amdvi::CONTROL);
    if amdvi::Control(control).encendida() {
        return fallo(IOMMU_NO_YA_ENCENDIDA);
    }
    let tabla = (a & IOMMU_BASE_PAGINAS_MASK) << 12;
    let bytes = ((a >> IOMMU_ARMADO_PAGINAS_SHIFT) & 0xFFF) * 4096;
    let ordenes = (c & IOMMU_BASE_PAGINAS_MASK) << 12;
    let eventos = ordenes + BYTES_COLA;
    let semaforo = eventos + BYTES_COLA;
    let (Some(r_tabla), Some(r_ord), Some(r_ev), Some(esperar)) = (
        t::registro_tabla(tabla, bytes),
        t::registro_cola(ordenes, ENTRADAS_COLA),
        t::registro_cola(eventos, ENTRADAS_COLA),
        Orden::esperar(semaforo, SEMAFORO_DATO),
    ) else {
        return fallo(IOMMU_NO_TABLAS);
    };
    let vs = crate::ring0::mm::phys_to_virt(semaforo);
    CONTROL_ORIGINAL.store(control, Ordering::Release);
    crate::ring0::cabina::bits("iommu", "M0c: ENCENDER, control antes", control);

    // 1. Entregar.
    wr(amdvi::TABLA_DISPOSITIVOS, r_tabla);
    wr(amdvi::COLA_ORDENES, r_ord);
    wr(amdvi::ORDENES_CABEZA, 0);
    wr(amdvi::ORDENES_COLA, 0);
    wr(amdvi::REGISTRO_EVENTOS, r_ev);
    wr(amdvi::EVENTOS_CABEZA, 0);
    wr(amdvi::EVENTOS_COLA, 0);
    // 2. Colas, y luego la IOMMU.
    wr(amdvi::CONTROL, control | CONTROL_ORDENES | CONTROL_EVENTOS);
    wr(amdvi::CONTROL, control | CONTROL_ORDENES | CONTROL_EVENTOS | CONTROL_EN);
    // 3. Las dos ordenes, en el anillo, y la cola que las entrega.
    // SAFETY: el semaforo y la cola son paginas NEUTRO de este fichero (`armar`).
    unsafe {
        (vs as *mut u64).write_volatile(0);
        let vo = crate::ring0::mm::phys_to_virt(ordenes) as *mut u32;
        for (i, w) in Orden::invalidar_todo().0.iter().chain(esperar.0.iter()).enumerate() {
            vo.add(i).write_volatile(*w);
        }
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    wr(amdvi::ORDENES_COLA, 2 * t::ORDEN as u64);
    // 4. Esperar el dato.
    use crate::ring0::task::scheduler::{rdtsc, tsc_freq};
    let hz = tsc_freq().max(1);
    let t0 = rdtsc();
    let fin = t0.saturating_add(hz / 1000 * ESPERA_MAX_MS);
    let mut llego = false;
    while rdtsc() < fin {
        // SAFETY: la pagina del semaforo, arriba.
        if unsafe { (vs as *const u64).read_volatile() } == SEMAFORO_DATO {
            llego = true;
            break;
        }
        core::hint::spin_loop();
    }
    let us = rdtsc().saturating_sub(t0) * 1_000_000 / hz;
    if !llego {
        // La vuelta atras: el control como estaba, y la IOMMU sin traducir.
        wr(amdvi::CONTROL, control);
        crate::ring0::cabina::warn("iommu", "M0c: el COMPLETION_WAIT no llego en 10 ms: APAGADA otra vez", us);
        return fallo(IOMMU_NO_CONTESTA);
    }
    let pendientes = (rd(amdvi::EVENTOS_COLA).wrapping_sub(rd(amdvi::EVENTOS_CABEZA)) & 0x7FFF0) / t::ORDEN as u64;
    VIVA.store(
        IOMMU_VIVA_ENCENDIDA
            | IOMMU_VIVA_CONTESTO
            | (VIVA.load(Ordering::Acquire) & (0xF << IOMMU_VIVA_INTENTOS_SHIFT))
            | us.min(0xFFFF_FFFF)
            | pendientes.min(0xFFFF) << IOMMU_VIVA_EVENTOS_SHIFT,
        Ordering::Release,
    );
    crate::ring0::cabina::count("iommu", "M0c: ENCENDIDA y obedece; el COMPLETION_WAIT tardo, us", us);
    if pendientes > 0 {
        crate::ring0::cabina::warn("iommu", "M0c: hay EVENTOS en su registro", pendientes);
    }
    Ok(us.min(0xFFFF_FFFF) | pendientes.min(0xFFFF) << 32)
}

/// **Apagar**: el control como lo dejo el firmware. Solo si la encendio
/// BMO-X: una que encendio otro no se apaga desde aqui.
pub fn apagar() -> Result<u64, u32> {
    if VIVA.load(Ordering::Acquire) & IOMMU_VIVA_ENCENDIDA == 0 {
        return fallo(IOMMU_NO_APAGADA);
    }
    let Some(v) = registros() else { return fallo(IOMMU_NO_TABLAS) };
    let original = CONTROL_ORIGINAL.load(Ordering::Acquire);
    // SAFETY: como en `encender`.
    unsafe { ((v + amdvi::CONTROL as u64) as *mut u64).write_volatile(original) };
    apuntar_viva(|x| x & !IOMMU_VIVA_ENCENDIDA);
    crate::ring0::cabina::bits("iommu", "M0c: APAGADA por orden, control devuelto", original);
    Ok(0)
}

pub fn info_viva() -> u64 {
    VIVA.load(Ordering::Acquire)
}
