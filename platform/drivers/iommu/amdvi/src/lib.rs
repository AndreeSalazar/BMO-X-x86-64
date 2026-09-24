//! **LO QUE LA IOMMU DE AMD CONTESTA** -- si esta encendida, que sabe hacer y
//! que tablas tiene, leido por MMIO.
//!
//! capa: puro -- recibe numeros que el kernel ya leyo y devuelve lo que significan; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- aqui no se optimiza nada, se lee bien
//!
//! # Por que existe (2026-09-23, M0a de `docs/plan/PLAN_LA_3060.md`)
//!
//! El propietario eligio el camino: antes de que la RTX 3060 haga DMA (un MSI
//! ya lo es), la IOMMU encendida. Y antes de encenderla, se PREGUNTA (LEY 24),
//! como se pregunto la grafica:
//!
//! ```text
//!    0x0018  CONTROL    la dejo el firmware encendida? (pre-boot DMA
//!                       protection): entonces se HEREDA, no se pisa
//!    0x0030  EFR        que sabe: cuantos niveles de pagina, remapeo de
//!                       interrupciones, x2APIC, NX...
//!    0x0000  TABLA      si ya hay tabla de dispositivos, donde y cuanta
//!    0x2020  ESTADO     si sus colas corren
//! ```
//!
//! Los offsets y los bits son los de Linux (`drivers/iommu/amd/
//! amd_iommu_types.h`: `MMIO_*_OFFSET`, `CONTROL_*`, `MMIO_STATUS_*`,
//! `FEATURE_*`), leidos el 23-09. El bit 4 de ESTADO (`CmdBufRun`) no lo
//! nombra Linux; es del manual de AMD y cae en el hueco entre el 3 y el 5.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// Las tablas que la IOMMU lee de la RAM, armadas y probadas aqui (M0b).
pub mod tablas;

// -- Los registros (MMIO) ----------------------------------------------------

pub const TABLA_DISPOSITIVOS: u32 = 0x0000;
pub const COLA_ORDENES: u32 = 0x0008;
pub const REGISTRO_EVENTOS: u32 = 0x0010;
pub const CONTROL: u32 = 0x0018;
pub const EXCLUSION_BASE: u32 = 0x0020;
pub const EXCLUSION_LIMITE: u32 = 0x0028;
pub const FUNCIONES: u32 = 0x0030;
pub const ORDENES_CABEZA: u32 = 0x2000;
pub const ORDENES_COLA: u32 = 0x2008;
pub const EVENTOS_CABEZA: u32 = 0x2010;
pub const EVENTOS_COLA: u32 = 0x2018;
pub const ESTADO: u32 = 0x2020;

/// Bytes de una entrada de la tabla de dispositivos (`DEV_TABLE_ENTRY_SIZE`).
pub const ENTRADA_DISPOSITIVO: u64 = 32;

/// **Un registro que no contesta**: todo unos. Una IOMMU apagada por la
/// placa, o una direccion de MMIO que no es la suya.
pub const fn es_muda(v: u64) -> bool {
    v == u64::MAX
}

// -- CONTROL -------------------------------------------------------------------

/// El registro de control, crudo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Control(pub u64);

impl Control {
    const fn bit(self, b: u32) -> bool {
        self.0 >> b & 1 != 0
    }
    /// `CONTROL_IOMMU_EN`: TRADUCE. Si el firmware la dejo asi, hay tablas
    /// suyas en memoria que un aparato esta usando AHORA.
    pub const fn encendida(self) -> bool {
        self.bit(0)
    }
    /// `CONTROL_EVT_LOG_EN`: apunta los fallos en su registro de eventos.
    pub const fn eventos(self) -> bool {
        self.bit(2)
    }
    /// `CONTROL_CMDBUF_EN`: lee su cola de ordenes.
    pub const fn ordenes(self) -> bool {
        self.bit(12)
    }
    /// `CONTROL_GA_EN`: remapeo de interrupciones con el formato de 128 bits.
    pub const fn ga(self) -> bool {
        self.bit(17)
    }
    /// `CONTROL_XT_EN`: interrupciones x2APIC (IDs de 32 bits).
    pub const fn xt(self) -> bool {
        self.bit(50)
    }
}

// -- ESTADO --------------------------------------------------------------------

/// El registro de estado, crudo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Estado(pub u64);

impl Estado {
    /// `MMIO_STATUS_EVT_OVERFLOW_MASK`: se perdieron eventos.
    pub const fn eventos_desbordados(self) -> bool {
        self.0 & 1 != 0
    }
    /// `MMIO_STATUS_EVT_RUN_MASK`: el registro de eventos esta en marcha.
    pub const fn eventos_corren(self) -> bool {
        self.0 >> 3 & 1 != 0
    }
    /// `CmdBufRun` (bit 4, del manual de AMD): la cola de ordenes corre.
    pub const fn ordenes_corren(self) -> bool {
        self.0 >> 4 & 1 != 0
    }
}

// -- FUNCIONES (EFR) -------------------------------------------------------------

/// El registro de funciones extendidas (`MMIO_EXT_FEATURES`), crudo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Funciones(pub u64);

impl Funciones {
    const fn bit(self, b: u32) -> bool {
        self.0 >> b & 1 != 0
    }
    /// `FEATURE_PREFETCH`.
    pub const fn prefetch(self) -> bool {
        self.bit(0)
    }
    /// `FEATURE_PPR`: peticiones de pagina (fallos que el aparato reintenta).
    pub const fn ppr(self) -> bool {
        self.bit(1)
    }
    /// `FEATURE_X2APIC`: sabe mandar interrupciones a IDs de x2APIC.
    pub const fn x2apic(self) -> bool {
        self.bit(2)
    }
    /// `FEATURE_NX`: el bit de no ejecutar en sus tablas.
    pub const fn nx(self) -> bool {
        self.bit(3)
    }
    /// `FEATURE_GT`: traduccion de invitado (dos niveles).
    pub const fn gt(self) -> bool {
        self.bit(4)
    }
    /// `FEATURE_IA`: la orden INVALIDATE_IOMMU_ALL.
    pub const fn invalidar_todo(self) -> bool {
        self.bit(6)
    }
    /// `FEATURE_GA`: remapeo de interrupciones con el formato de 128 bits.
    pub const fn ga(self) -> bool {
        self.bit(7)
    }
    /// `FEATURE_HE`: sabe apagar funciones por hardware.
    pub const fn he(self) -> bool {
        self.bit(8)
    }
    /// `FEATURE_PC`: contadores de rendimiento.
    pub const fn contadores(self) -> bool {
        self.bit(9)
    }
    /// **Cuantos niveles de pagina traduce como mucho** (`FEATURE_HATS`,
    /// bits 10-11): 4 + el campo. Linux: `amd_iommu_hpt_level = hats + 4`.
    /// Con 4 niveles cubre 48 bits de direccion: sobra para esta placa.
    pub const fn niveles(self) -> u8 {
        4 + ((self.0 >> 10) & 3) as u8
    }
}

// -- Las tablas que apunta cada registro ---------------------------------------

/// **La tabla de dispositivos** que dice el registro 0x0000: base en los bits
/// 12..51, y la medida en paginas de 4 KiB MENOS UNA en los bits 0..8.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tabla {
    pub base: u64,
    pub bytes: u64,
}

impl Tabla {
    /// `None` = no hay tabla (base 0): nadie la armo.
    pub const fn de_registro(v: u64) -> Option<Tabla> {
        let base = v & 0x000F_FFFF_FFFF_F000;
        if base == 0 {
            return None;
        }
        Some(Tabla { base, bytes: ((v & 0x1FF) + 1) * 4096 })
    }
    /// Cuantos BDF caben.
    pub const fn entradas(&self) -> u64 {
        self.bytes / ENTRADA_DISPOSITIVO
    }
}

/// **Una cola** (ordenes o eventos): base en los bits 12..51 y `2^n`
/// entradas de 16 bytes con `n` en los bits 56..59 (`MMIO_CMD_SIZE_SHIFT`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cola {
    pub base: u64,
    pub entradas: u32,
}

impl Cola {
    pub const fn de_registro(v: u64) -> Option<Cola> {
        let base = v & 0x000F_FFFF_FFFF_F000;
        if base == 0 {
            return None;
        }
        Some(Cola { base, entradas: 1 << ((v >> 56) & 0xF) })
    }
}

/// **Lo que tiene que medir la tabla de dispositivos** para cubrir hasta el
/// BDF `max_bdf`: en bytes, redondeado a pagina. La mayor son 2 MiB (65.536
/// BDF), y el campo de medida no da mas.
pub const fn tabla_para(max_bdf: u16) -> u64 {
    let b = (max_bdf as u64 + 1) * ENTRADA_DISPOSITIVO;
    (b + 4095) / 4096 * 4096
}

/// **Si se puede encender, y si no, por que.** Lo que la sonda sabe decir
/// antes de escribir nada.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Veredicto {
    /// Apagada y contesta. Los niveles no se miran: `niveles()` nunca da
    /// menos de 4, que son los de las tablas de BMO-X.
    SePuede,
    /// Los registros contestan todo unos: no es la IOMMU, o esta apagada por
    /// la placa (BIOS).
    Muda,
    /// El firmware la dejo TRADUCIENDO: hay que heredar sus tablas.
    EncendidaPorElFirmware,
}

pub const fn veredicto(control: u64, funciones: u64) -> Veredicto {
    if es_muda(control) || es_muda(funciones) {
        return Veredicto::Muda;
    }
    if Control(control).encendida() {
        return Veredicto::EncendidaPorElFirmware;
    }
    Veredicto::SePuede
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn control_apagada_y_encendida() {
        assert!(!Control(0).encendida());
        let c = Control(1 | 1 << 2 | 1 << 12);
        assert!(c.encendida() && c.eventos() && c.ordenes() && !c.ga());
        assert!(Control(1 << 50).xt());
    }

    #[test]
    fn funciones_y_niveles() {
        // PREFETCH, X2APIC, NX, IA, GA, HATS = 01 (5 niveles).
        let f = Funciones(1 | 1 << 2 | 1 << 3 | 1 << 6 | 1 << 7 | 1 << 10);
        assert!(f.prefetch() && f.x2apic() && f.nx() && f.invalidar_todo() && f.ga());
        assert!(!f.ppr() && !f.gt());
        assert_eq!(f.niveles(), 5);
        assert_eq!(Funciones(0).niveles(), 4);
    }

    #[test]
    fn la_tabla_de_dispositivos() {
        assert_eq!(Tabla::de_registro(0), None, "sin base no hay tabla");
        // 2 MiB = 512 paginas: el campo dice 511.
        let t = Tabla::de_registro(0x1_2340_0000 | 0x1FF).unwrap();
        assert_eq!(t.base, 0x1_2340_0000);
        assert_eq!(t.bytes, 2 * 1024 * 1024);
        assert_eq!(t.entradas(), 65536);
    }

    #[test]
    fn las_colas() {
        // 512 ordenes = 2^9 (`MMIO_CMD_SIZE_512`).
        let c = Cola::de_registro(0x9 << 56 | 0x8000_0000).unwrap();
        assert_eq!((c.base, c.entradas), (0x8000_0000, 512));
        assert_eq!(Cola::de_registro(0x9 << 56), None);
    }

    #[test]
    fn la_tabla_que_hace_falta() {
        assert_eq!(tabla_para(0), 4096);
        assert_eq!(tabla_para(127), 4096, "128 BDF x 32 = una pagina justa");
        assert_eq!(tabla_para(128), 8192);
        assert_eq!(tabla_para(0x2BFF), 360_448, "hasta el bus 0x2B: 352 KiB");
        assert_eq!(tabla_para(0xFFFF), 2 * 1024 * 1024);
    }

    #[test]
    fn el_veredicto() {
        assert_eq!(veredicto(0, 0), Veredicto::SePuede);
        assert_eq!(veredicto(u64::MAX, 0), Veredicto::Muda);
        assert_eq!(veredicto(1, 0), Veredicto::EncendidaPorElFirmware);
        assert!(!es_muda(0xBAD0_0000));
    }
}
