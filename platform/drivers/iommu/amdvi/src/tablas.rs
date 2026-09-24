//! **LAS TABLAS QUE LEE LA IOMMU** -- la entrada de cada aparato, las ordenes,
//! los eventos y los registros que las entregan. Armadas y probadas aqui,
//! ANTES de tocar el metal (M0b de `docs/plan/PLAN_LA_3060.md`).
//!
//! # Lo que la IOMMU lee de la RAM
//!
//! ```text
//!    tabla de dispositivos   32 bytes por BDF: si traduce, con que paginas,
//!                            y si deja pasar INIT/NMI/LINT (las banderas
//!                            del IVHD)
//!    cola de ordenes         16 bytes por orden: invalidar lo que guardo en
//!                            su cache, y ESPERAR (escribe un dato cuando
//!                            acabo lo anterior)
//!    registro de eventos     16 bytes por evento: un aparato que se salio,
//!                            una orden mala -- con su BDF y su direccion
//! ```
//!
//! Cada bit sale de Linux (`amd_iommu_types.h`: `DTE_*`, `DEV_ENTRY_*`,
//! `CMD_*`, `EVENT_*`; `iommu.c`: `build_completion_wait`, `build_inv_dte`,
//! `build_inv_all`, `set_dte_passthrough`, `amd_iommu_set_dte_v1`,
//! `iommu_print_event`; `init.c`: `set_dev_entry_from_acpi_range`,
//! `init_device_table_dma`, `iommu_set_device_table`), leido el 24-09.
//!
//! # Tres formas de una entrada, y la de por defecto
//!
//! ```text
//!    BLOQUEADA   V + TV, sin IR ni IW: todo DMA se niega. Es como Linux
//!                arranca TODAS las entradas (`init_device_table_dma`)
//!    DE PASO     V + TV + IR + IW, modo 0: sin traducir, todo pasa. Es la
//!                de M0c: encender sin que ningun aparato note nada
//!    TRADUCIDA   V + TV + IR + IW + modo + raiz: el aparato ve SOLO lo que
//!                sus tablas de pagina le prestan (M0d)
//! ```
//!
//! [!] Una entrada VIVA no se reescribe a trozos: la IOMMU lee 256 bits y
//! puede cazar la mitad vieja y la mitad nueva. Aqui solo se ARMAN; como se
//! cambia una en marcha (y que invalidar despues) es de M0d.

// -- La entrada de dispositivo (DTE) -------------------------------------------

/// Palabra 0.
const V: u64 = 1 << 0;
const TV: u64 = 1 << 1;
const MODO_SHIFT: u32 = 9;
const RAIZ_MASK: u64 = 0x000F_FFFF_FFFF_F000;
const IR: u64 = 1 << 61;
const IW: u64 = 1 << 62;
/// Palabra 1.
const DOMINIO_MASK: u64 = 0xFFFF;
const SYSMGT_SHIFT: u32 = 40;
/// Palabra 2 (`DEV_ENTRY_*_PASS`: bit - 128).
const INIT_PASA: u64 = 1 << 56;
const EINT_PASA: u64 = 1 << 57;
const NMI_PASA: u64 = 1 << 58;
const LINT0_PASA: u64 = 1 << 62;
const LINT1_PASA: u64 = 1 << 63;

/// Banderas de una entrada del IVHD (`ACPI_DEVFLAG_*`).
pub const ACPI_INIT: u8 = 0x01;
pub const ACPI_EXTINT: u8 = 0x02;
pub const ACPI_NMI: u8 = 0x04;
pub const ACPI_SYSMGT1: u8 = 0x10;
pub const ACPI_SYSMGT2: u8 = 0x20;
pub const ACPI_LINT0: u8 = 0x40;
pub const ACPI_LINT1: u8 = 0x80;

/// **La entrada de un BDF**, los 256 bits tal cual se escriben.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Dte(pub [u64; 4]);

impl Dte {
    /// Todo a cero: `V = 0`. La IOMMU no mira nada de este BDF.
    pub const fn vacia() -> Dte {
        Dte([0; 4])
    }

    /// **Todo DMA negado.** `V + TV` sin permisos.
    pub const fn bloqueada(dominio: u16) -> Dte {
        Dte([V | TV, dominio as u64, 0, 0])
    }

    /// **Sin traducir, todo pasa** (`set_dte_passthrough`).
    pub const fn de_paso(dominio: u16) -> Dte {
        Dte([V | TV | IR | IW, dominio as u64, 0, 0])
    }

    /// **Traducida por unas tablas de pagina** de `niveles` niveles con la
    /// raiz en `raiz` (`amd_iommu_set_dte_v1`). `None` si la raiz no esta
    /// alineada a pagina o los niveles no son 1..=6.
    pub const fn traducida(dominio: u16, raiz: u64, niveles: u8) -> Option<Dte> {
        if raiz & 0xFFF != 0 || raiz & !RAIZ_MASK != 0 || niveles == 0 || niveles > 6 {
            return None;
        }
        Some(Dte([V | TV | IR | IW | (niveles as u64) << MODO_SHIFT | raiz, dominio as u64, 0, 0]))
    }

    /// **Lo que el IVHD pide para este BDF** (`set_dev_entry_from_acpi_range`):
    /// que INIT, NMI, LINT... pasen, y el campo SysMgt. Con la errata 63:
    /// SysMgt = 01 obliga a IW.
    pub const fn con_banderas_ivhd(self, b: u8) -> Dte {
        let mut d = self.0;
        if b & ACPI_INIT != 0 {
            d[2] |= INIT_PASA;
        }
        if b & ACPI_EXTINT != 0 {
            d[2] |= EINT_PASA;
        }
        if b & ACPI_NMI != 0 {
            d[2] |= NMI_PASA;
        }
        if b & ACPI_SYSMGT1 != 0 {
            d[1] |= 1 << SYSMGT_SHIFT;
        }
        if b & ACPI_SYSMGT2 != 0 {
            d[1] |= 2 << SYSMGT_SHIFT;
        }
        if b & ACPI_LINT0 != 0 {
            d[2] |= LINT0_PASA;
        }
        if b & ACPI_LINT1 != 0 {
            d[2] |= LINT1_PASA;
        }
        if (d[1] >> SYSMGT_SHIFT) & 3 == 1 {
            d[0] |= IW;
        }
        Dte(d)
    }

    pub const fn valida(&self) -> bool {
        self.0[0] & V != 0
    }
    pub const fn traduce(&self) -> bool {
        self.0[0] & TV != 0
    }
    pub const fn lee(&self) -> bool {
        self.0[0] & IR != 0
    }
    pub const fn escribe(&self) -> bool {
        self.0[0] & IW != 0
    }
    pub const fn niveles(&self) -> u8 {
        ((self.0[0] >> MODO_SHIFT) & 7) as u8
    }
    pub const fn raiz(&self) -> u64 {
        self.0[0] & RAIZ_MASK
    }
    pub const fn dominio(&self) -> u16 {
        (self.0[1] & DOMINIO_MASK) as u16
    }
}

/// **Escribe la entrada del BDF `bdf`** en una tabla vista como palabras de
/// 64 bits (4 por BDF). `false` si el BDF no cabe.
pub fn poner(tabla: &mut [u64], bdf: u16, e: Dte) -> bool {
    let i = bdf as usize * 4;
    let Some(w) = tabla.get_mut(i..i + 4) else { return false };
    w.copy_from_slice(&e.0);
    true
}

/// La entrada del BDF `bdf`, si cabe.
pub fn leer(tabla: &[u64], bdf: u16) -> Option<Dte> {
    let i = bdf as usize * 4;
    let w = tabla.get(i..i + 4)?;
    Some(Dte([w[0], w[1], w[2], w[3]]))
}

/// **La misma entrada en TODOS los BDF que caben.** Devuelve cuantos.
pub fn llenar(tabla: &mut [u64], e: Dte) -> usize {
    let mut n = 0;
    for w in tabla.chunks_exact_mut(4) {
        w.copy_from_slice(&e.0);
        n += 1;
    }
    n
}

// -- Los registros que entregan las tablas --------------------------------------

/// **El valor del registro 0x0000**: base alineada a pagina y la medida en
/// paginas MENOS UNA (`iommu_set_device_table`). `None` si la base no esta
/// alineada o la medida no es de 1 a 512 paginas.
pub const fn registro_tabla(base: u64, bytes: u64) -> Option<u64> {
    if base & 0xFFF != 0 || base & !RAIZ_MASK != 0 || bytes == 0 || bytes % 4096 != 0 || bytes > 2 * 1024 * 1024 {
        return None;
    }
    Some(base | (bytes / 4096 - 1))
}

/// **El valor del registro de una cola** (0x0008 ordenes, 0x0010 eventos):
/// base alineada a pagina y `log2(entradas)` en los bits 56..59
/// (`MMIO_CMD_SIZE_512` = 9). Entre 256 y 32.768 entradas, potencia de dos.
pub const fn registro_cola(base: u64, entradas: u32) -> Option<u64> {
    if base & 0xFFF != 0 || base & !RAIZ_MASK != 0 || !entradas.is_power_of_two() || entradas < 256 || entradas > 32768 {
        return None;
    }
    Some(base | (entradas.trailing_zeros() as u64) << 56)
}

// -- Las ordenes -------------------------------------------------------------------

/// Bytes de una orden y de un evento.
pub const ORDEN: u32 = 16;

const TIPO_SHIFT: u32 = 28;
const ORDEN_ESPERAR: u32 = 0x01;
const ORDEN_INVALIDAR_ENTRADA: u32 = 0x02;
const ORDEN_INVALIDAR_TODO: u32 = 0x08;
/// `CMD_COMPL_WAIT_STORE_MASK`: al acabar, ESCRIBE el dato en la direccion.
const ESPERAR_ESCRIBE: u32 = 0x01;

/// Una orden, las cuatro palabras de 32 bits tal cual van a la cola.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Orden(pub [u32; 4]);

impl Orden {
    pub const fn tipo(&self) -> u32 {
        self.0[1] >> TIPO_SHIFT
    }

    /// **COMPLETION_WAIT**: cuando todo lo de antes acabo, la IOMMU escribe
    /// `dato` en `dir` (alineada a 8). Es la unica forma de saber que una
    /// invalidacion ya surtio efecto. `None` si `dir` no esta alineada.
    pub const fn esperar(dir: u64, dato: u64) -> Option<Orden> {
        if dir & 7 != 0 || dir & !0x000F_FFFF_FFFF_FFFF != 0 {
            return None;
        }
        Some(Orden([
            dir as u32 | ESPERAR_ESCRIBE,
            (dir >> 32) as u32 | ORDEN_ESPERAR << TIPO_SHIFT,
            dato as u32,
            (dato >> 32) as u32,
        ]))
    }

    /// **INVALIDATE_DEVTAB_ENTRY**: olvida lo que guardo de la entrada de
    /// `bdf`. Tras cambiar una entrada, siempre.
    pub const fn invalidar_entrada(bdf: u16) -> Orden {
        Orden([bdf as u32, ORDEN_INVALIDAR_ENTRADA << TIPO_SHIFT, 0, 0])
    }

    /// **INVALIDATE_IOMMU_ALL**: olvida todo. Solo si el EFR trae `IA`
    /// (`Funciones::invalidar_todo`); el Ryzen del 24-09 lo trae.
    pub const fn invalidar_todo() -> Orden {
        Orden([0, ORDEN_INVALIDAR_TODO << TIPO_SHIFT, 0, 0])
    }
}

/// **Una cola en anillo**, contada en bytes como sus registros de cabeza y
/// cola (bits 4..18). La IOMMU avanza la CABEZA al leer; quien escribe
/// avanza la COLA. Como Linux, se deja siempre un hueco de 0x20: una cola
/// llena y una vacia no se pueden confundir.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Anillo {
    pub bytes: u32,
}

impl Anillo {
    /// Donde va la siguiente orden despues de `cola`.
    pub const fn siguiente(&self, cola: u32) -> u32 {
        (cola + ORDEN) % self.bytes
    }

    /// **Cabe otra orden?** (`__iommu_queue_command_sync`: `left <= 0x20`
    /// es esperar.)
    pub const fn hay_sitio(&self, cabeza: u32, cola: u32) -> bool {
        let libre = cabeza.wrapping_sub(self.siguiente(cola)) % self.bytes;
        libre > 0x20
    }

    /// Ordenes pendientes de leer por la IOMMU.
    pub const fn pendientes(&self, cabeza: u32, cola: u32) -> u32 {
        cola.wrapping_sub(cabeza) % self.bytes / ORDEN
    }
}

// -- Los eventos ------------------------------------------------------------------

/// Un evento del registro, las cuatro palabras de 32 bits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Evento(pub [u32; 4]);

impl Evento {
    /// `0` = la IOMMU todavia no lo escribio entero (Linux reintenta).
    pub const fn tipo(&self) -> u32 {
        (self.0[1] >> 28) & 0xF
    }
    pub const fn bdf(&self) -> u16 {
        self.0[0] as u16
    }
    /// El dominio (o el PASID), 20 bits.
    pub const fn dominio(&self) -> u32 {
        (self.0[0] & 0xF_0000) | (self.0[1] & 0xFFFF)
    }
    pub const fn banderas(&self) -> u32 {
        (self.0[1] >> 16) & 0xFFF
    }
    pub const fn direccion(&self) -> u64 {
        (self.0[3] as u64) << 32 | self.0[2] as u64
    }
    /// **Que fue**, en palabras (`EVENT_TYPE_*`).
    pub const fn nombre(&self) -> &'static str {
        match self.tipo() {
            0x1 => "entrada de dispositivo ILEGAL",
            0x2 => "FALLO de pagina de un aparato",
            0x3 => "error de hardware leyendo la tabla de dispositivos",
            0x4 => "error de hardware leyendo una tabla de paginas",
            0x5 => "orden ILEGAL en la cola",
            0x6 => "error de hardware leyendo la cola de ordenes",
            0x7 => "una invalidacion de IOTLB no llego a tiempo",
            0x8 => "peticion de un aparato que no debia",
            0x9 => "peticion de pagina invalida",
            0xD => "fallo de RMP (SEV-SNP)",
            0xE => "error de hardware en la RMP",
            0 => "a medio escribir",
            _ => "tipo que esta lectura no conoce",
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::{Cola, Tabla};

    #[test]
    fn las_tres_formas_de_una_entrada() {
        let b = Dte::bloqueada(0);
        assert!(b.valida() && b.traduce() && !b.lee() && !b.escribe(), "bloqueada: V + TV sin permisos");
        let p = Dte::de_paso(7);
        assert!(p.valida() && p.traduce() && p.lee() && p.escribe());
        assert_eq!((p.niveles(), p.dominio()), (0, 7), "de paso = modo 0");
        let t = Dte::traducida(3, 0x1234_5000, 4).unwrap();
        assert_eq!((t.niveles(), t.raiz(), t.dominio()), (4, 0x1234_5000, 3));
        assert_eq!(Dte::traducida(3, 0x1234_5008, 4), None, "raiz sin alinear");
        assert_eq!(Dte::traducida(3, 0x1000, 7), None, "no hay 7 niveles");
        assert!(!Dte::vacia().valida());
    }

    #[test]
    fn las_banderas_del_ivhd() {
        // Las del IOAPIC del Ryzen (24-09): 0xD7 = INIT EXTINT NMI SYSMGT1 LINT0 LINT1.
        let d = Dte::de_paso(0).con_banderas_ivhd(0xD7);
        assert_eq!(d.0[2] >> 56, 0b1100_0111, "INIT, EINT, NMI, LINT0, LINT1");
        assert_eq!((d.0[1] >> 40) & 3, 1, "SysMgt = 01");
        // Errata 63: SysMgt = 01 obliga a IW, aunque la entrada estuviera bloqueada.
        let b = Dte::bloqueada(0).con_banderas_ivhd(ACPI_SYSMGT1);
        assert!(b.escribe() && !b.lee());
        let b2 = Dte::bloqueada(0).con_banderas_ivhd(ACPI_SYSMGT1 | ACPI_SYSMGT2);
        assert!(!b2.escribe(), "SysMgt = 11 no es la errata");
    }

    #[test]
    fn la_tabla_se_llena_y_se_lee() {
        let mut t = vec![0u64; 4 * 256];
        assert_eq!(llenar(&mut t, Dte::bloqueada(0)), 256);
        assert!(poner(&mut t, 0x10, Dte::de_paso(1)));
        assert!(!poner(&mut t, 256, Dte::de_paso(1)), "fuera de la tabla");
        assert!(leer(&t, 0x10).unwrap().escribe());
        assert!(!leer(&t, 0x11).unwrap().escribe());
        assert_eq!(leer(&t, 256), None);
    }

    #[test]
    fn los_registros_vuelven_a_leerse_igual() {
        let r = registro_tabla(0x4000_0000, 2 * 1024 * 1024).unwrap();
        assert_eq!(Tabla::de_registro(r), Some(Tabla { base: 0x4000_0000, bytes: 2 * 1024 * 1024 }));
        assert_eq!(registro_tabla(0x4000_0800, 4096), None);
        assert_eq!(registro_tabla(0x4000_0000, 4 * 1024 * 1024), None, "mas de 512 paginas no cabe en el campo");
        let c = registro_cola(0x5000_0000, 512).unwrap();
        assert_eq!(c >> 56, 9, "MMIO_CMD_SIZE_512");
        assert_eq!(Cola::de_registro(c), Some(Cola { base: 0x5000_0000, entradas: 512 }));
        assert_eq!(registro_cola(0x5000_0000, 500), None);
        assert_eq!(registro_cola(0x5000_0000, 128), None);
    }

    #[test]
    fn las_ordenes() {
        let e = Orden::esperar(0x1_2345_6788, 0xDEAD_BEEF_0000_0001).unwrap();
        assert_eq!(e.tipo(), 1);
        assert_eq!(e.0, [0x2345_6789, 0x1000_0001, 0x0000_0001, 0xDEAD_BEEF]);
        assert_eq!(Orden::esperar(0x1004, 0), None, "direccion sin alinear a 8");
        assert_eq!(Orden::invalidar_entrada(0x2B03).0, [0x2B03, 0x2000_0000, 0, 0]);
        assert_eq!(Orden::invalidar_todo().tipo(), 8);
    }

    #[test]
    fn el_anillo() {
        let a = Anillo { bytes: 8192 };
        assert!(a.hay_sitio(0, 0), "vacio");
        assert_eq!(a.siguiente(8192 - 16), 0, "da la vuelta");
        assert_eq!(a.pendientes(0, 48), 3);
        assert_eq!(a.pendientes(8192 - 16, 16), 2, "pendientes a traves de la vuelta");
        // Lleno: la cola a 0x20 bytes de alcanzar a la cabeza.
        assert!(!a.hay_sitio(0x40, 0x10));
        assert!(a.hay_sitio(0x40, 0x00));
    }

    #[test]
    fn un_evento_de_fallo() {
        // Fallo de pagina del 2B:00.3 escribiendo en 0xDEAD_B000, dominio 5.
        let e = Evento([0x2B03, (0x2 << 28) | (0x020 << 16) | 5, 0xDEAD_B000, 0]);
        assert_eq!((e.tipo(), e.bdf(), e.dominio(), e.banderas()), (2, 0x2B03, 5, 0x020));
        assert_eq!(e.direccion(), 0xDEAD_B000);
        assert_eq!(e.nombre(), "FALLO de pagina de un aparato");
        assert_eq!(Evento([0; 4]).nombre(), "a medio escribir");
    }
}
