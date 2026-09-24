//! **LO QUE EL IVRS DICE DE CADA IOMMU** -- a quien traduce, con que alias,
//! que interrupciones no son de un aparato PCI, y que memoria exige el
//! firmware que se deje pasar.
//!
//! # Por que existe (2026-09-23, M0a de `docs/plan/PLAN_LA_3060.md`)
//!
//! `leer_ivrs` (en `lib.rs`) dice SI hay IOMMU y DONDE viven sus registros.
//! Para ENCENDERLA hace falta lo que viene detras de cada cabecera, y sin eso
//! la primera traduccion rompe algo que funcionaba:
//!
//! ```text
//!    entradas de dispositivo   a que BDF atiende esta IOMMU, y el mayor de
//!                              ellos: la medida de la tabla de dispositivos
//!    ALIAS                     un aparato detras de un puente PCIe->PCI
//!                              pide con el BDF del PUENTE, no con el suyo
//!    ESPECIALES                el IOAPIC y el HPET no son funciones PCI,
//!                              pero sus interrupciones pasan por la IOMMU:
//!                              remapear sin saber su BDF las calla
//!    IVMD                      memoria que el firmware usa por DMA (USB
//!                              heredado, el PSP): o se deja pasar, o la
//!                              placa hace cosas raras en cuanto se traduce
//! ```
//!
//! Las constantes y las medidas son las de Linux (`drivers/iommu/amd/init.c`,
//! `IVHD_DEV_*`, `ivhd_entry_length`, `get_ivhd_header_size`, `struct
//! ivmd_header`): se leyeron el 23-09, no se suponen.
//!
//! Solo LEE bytes: el kernel se los da, y aqui no se toca un registro.

/// Las tres formas de un bloque IVHD. Describen la misma IOMMU con mas o
/// menos detalle; el firmware puede traer varias para la MISMA.
pub const IVHD_10: u8 = 0x10;
pub const IVHD_11: u8 = 0x11;
pub const IVHD_40: u8 = 0x40;
/// Bloques IVMD: memoria con reglas. Para todos los aparatos, para uno, o
/// para un rango de BDF.
pub const IVMD_TODOS: u8 = 0x20;
pub const IVMD_UNO: u8 = 0x21;
pub const IVMD_RANGO: u8 = 0x22;

/// Banderas de un IVMD (`IVMD_FLAG_*`).
pub const IVMD_UNIDAD: u8 = 0x01;
pub const IVMD_LEE: u8 = 0x02;
pub const IVMD_ESCRIBE: u8 = 0x04;
pub const IVMD_EXCLUSION: u8 = 0x08;

/// Tipos de entrada de dispositivo (`IVHD_DEV_*`).
const DEV_RELLENO: u8 = 0x00;
const DEV_TODOS: u8 = 0x01;
const DEV_UNO: u8 = 0x02;
const DEV_RANGO_DESDE: u8 = 0x03;
const DEV_RANGO_HASTA: u8 = 0x04;
const DEV_ALIAS: u8 = 0x42;
const DEV_ALIAS_RANGO: u8 = 0x43;
const DEV_EXT_UNO: u8 = 0x46;
const DEV_EXT_RANGO: u8 = 0x47;
const DEV_ESPECIAL: u8 = 0x48;
const DEV_ACPI_HID: u8 = 0xF0;

/// Lo que es un `ESPECIAL` (`IVHD_SPECIAL_*`).
pub const ESPECIAL_IOAPIC: u8 = 1;
pub const ESPECIAL_HPET: u8 = 2;

/// Cabecera de un IVRS antes del primer bloque (igual que `IVRS_CABECERA`).
const IVRS_CABECERA: usize = crate::IVRS_CABECERA;

pub fn es_ivhd(tipo: u8) -> bool {
    matches!(tipo, IVHD_10 | IVHD_11 | IVHD_40)
}

pub fn es_ivmd(tipo: u8) -> bool {
    matches!(tipo, IVMD_TODOS | IVMD_UNO | IVMD_RANGO)
}

/// Bytes de la cabecera de un IVHD antes de sus entradas: 24 el 0x10, 40 los
/// otros dos (traen la copia del registro de funciones extendidas).
pub fn cabecera_ivhd(tipo: u8) -> Option<usize> {
    match tipo {
        IVHD_10 => Some(24),
        IVHD_11 | IVHD_40 => Some(40),
        _ => None,
    }
}

fn u16_en(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_en(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

fn u64_en(b: &[u8], i: usize) -> u64 {
    let mut x = [0u8; 8];
    x.copy_from_slice(&b[i..i + 8]);
    u64::from_le_bytes(x)
}

/// **Cada bloque del IVRS, en orden**: `(tipo, bytes del bloque)`. Llama a
/// `f` con cada uno y para en el primero que no tiene forma. Como `leer_ivrs`,
/// termina aunque el firmware mienta: un largo menor que una cabecera corta.
pub fn por_bloque<'a>(ivrs: &'a [u8], mut f: impl FnMut(u8, &'a [u8])) {
    if ivrs.len() < IVRS_CABECERA {
        return;
    }
    let largo = (u32_en(ivrs, 4) as usize).min(ivrs.len());
    let mut o = IVRS_CABECERA;
    while o + 4 <= largo {
        let tipo = ivrs[o];
        let l = u16_en(ivrs, o + 2) as usize;
        if l < 8 || o + l > largo {
            break;
        }
        f(tipo, &ivrs[o..o + l]);
        o += l;
    }
}

/// **El IVHD que se usa.** El firmware puede describir la MISMA IOMMU con un
/// 0x10 y un 0x11 (o un 0x40): se toma el de tipo mas alto, que trae mas
/// detalle -- es lo que hace Linux. Esto vale para una placa con UNA IOMMU,
/// que es la del Ryzen; con varias, cada una tiene su BDF y habria que
/// elegir por BDF.
pub fn ivhd_elegido(ivrs: &[u8]) -> Option<&[u8]> {
    let mut mejor: Option<&[u8]> = None;
    por_bloque(ivrs, |tipo, b| {
        if es_ivhd(tipo) && b.len() >= 24 && mejor.map_or(true, |m| tipo > m[0]) {
            mejor = Some(b);
        }
    });
    mejor
}

/// Un aparato que no es una funcion PCI pero cuyas interrupciones pasan por
/// la IOMMU: el IOAPIC o el HPET, con el BDF con el que piden.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Especial {
    /// [`ESPECIAL_IOAPIC`] o [`ESPECIAL_HPET`]; otro numero, sin nombre.
    pub tipo: u8,
    /// El id del IOAPIC (el de la MADT) o el numero del HPET.
    pub handle: u8,
    /// El BDF con el que pide.
    pub bdf: u16,
    /// Las banderas de la entrada (`ACPI_DEVFLAG_*`): que se deja pasar.
    pub banderas: u8,
}

/// **El censo de las entradas de UN IVHD.** No las guarda: las cuenta, y
/// dice el mayor BDF que nombran, que es lo que mide la tabla de
/// dispositivos.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Censo {
    pub tipo: u8,
    /// Entradas `ALL`: todos los BDF del segmento.
    pub todos: u16,
    /// Un BDF suelto (normal o extendido).
    pub unos: u16,
    /// Rangos cerrados (desde + hasta).
    pub rangos: u16,
    /// Alias: sueltos y rangos. Cada uno es un aparato que pide con OTRO BDF.
    pub alias: u16,
    /// IOAPIC y HPET. Los primeros [`censar`] los copia en su salida.
    pub especiales: u16,
    /// Aparatos nombrados por su HID de ACPI.
    pub por_hid: u16,
    /// Tipos que esta lectura no conoce: se saltan por su medida.
    pub desconocidas: u16,
    /// El mayor BDF nombrado, alias incluidos.
    pub max_bdf: u16,
    /// Una entrada se salia del bloque, o no tenia medida: se paro ahi.
    pub cortado: bool,
    /// La copia del registro de funciones extendidas (EFR) que traen el 0x11
    /// y el 0x40. `None` en el 0x10.
    pub efr: Option<u64>,
}

/// Que clase de entrada produjo un [`Tramo`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Que {
    /// `ALL`: todos los BDF.
    Todos,
    /// Un BDF suelto (normal o extendido).
    Uno,
    /// Un rango cerrado (desde + hasta).
    Rango,
    /// Un alias o un rango con alias: `alias` es el BDF con el que piden.
    Alias,
    /// El IOAPIC o el HPET.
    Especial(Especial),
    /// Un aparato nombrado por su HID de ACPI.
    PorHid,
}

/// **Un tramo de BDF con las banderas que el IVHD pide para el.** Lo que
/// hace falta para llenar la tabla de dispositivos como Linux
/// (`set_dev_entry_from_acpi_range`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tramo {
    pub que: Que,
    pub desde: u16,
    pub hasta: u16,
    /// `ACPI_DEVFLAG_*`: INIT, EXTINT, NMI, SYSMGT, LINT0, LINT1.
    pub banderas: u8,
    /// El BDF con el que piden, si es un alias. Tambien lleva las banderas.
    pub alias: Option<u16>,
}

/// Lo que el recorrido no pudo convertir en tramo.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Resto {
    /// Tipos que esta lectura no conoce: se saltaron por su medida.
    pub desconocidas: u16,
    /// Una entrada se salia del bloque, o no tenia medida: se paro ahi.
    pub cortado: bool,
}

/// **Recorre las entradas de un IVHD** (el bloque entero, cabecera
/// incluida) y da cada tramo con sus banderas. Un rango se da al llegar a su
/// `hasta`, con las banderas de su `desde`, como Linux.
pub fn por_entrada(bloque: &[u8], mut f: impl FnMut(Tramo)) -> Resto {
    let mut r = Resto::default();
    let Some(cab) = bloque.first().and_then(|&t| cabecera_ivhd(t)) else {
        r.cortado = true;
        return r;
    };
    if bloque.len() < cab {
        r.cortado = true;
        return r;
    }
    let largo = (u16_en(bloque, 2) as usize).min(bloque.len());
    // El rango abierto: desde, banderas, y su alias si lo lleva.
    let mut abierto: Option<(u16, u8, Option<u16>)> = None;
    let mut o = cab;
    while o + 4 <= largo {
        let tipo = bloque[o];
        let l = if tipo < 0x80 {
            4usize << (tipo >> 6)
        } else if tipo == DEV_ACPI_HID && o + 22 <= largo {
            bloque[o + 21] as usize + 22
        } else {
            0
        };
        if l == 0 || o + l > largo {
            r.cortado = true;
            break;
        }
        let bdf = u16_en(bloque, o + 1);
        let banderas = bloque[o + 3];
        let ext = if l >= 8 { u32_en(bloque, o + 4) } else { 0 };
        let uno = |que, b: u16, alias| Tramo { que, desde: b, hasta: b, banderas, alias };
        match tipo {
            DEV_RELLENO => {}
            DEV_TODOS => f(Tramo { que: Que::Todos, desde: 0, hasta: 0xFFFF, banderas, alias: None }),
            DEV_UNO | DEV_EXT_UNO => f(uno(Que::Uno, bdf, None)),
            DEV_RANGO_DESDE | DEV_EXT_RANGO => abierto = Some((bdf, banderas, None)),
            DEV_ALIAS_RANGO => abierto = Some((bdf, banderas, Some((ext >> 8) as u16))),
            DEV_RANGO_HASTA => {
                if let Some((desde, b, alias)) = abierto.take() {
                    let que = if alias.is_some() { Que::Alias } else { Que::Rango };
                    f(Tramo { que, desde, hasta: bdf.max(desde), banderas: b, alias });
                }
            }
            DEV_ALIAS => f(uno(Que::Alias, bdf, Some((ext >> 8) as u16))),
            DEV_ESPECIAL => {
                let e = Especial { tipo: (ext >> 24) as u8, handle: ext as u8, bdf: (ext >> 8) as u16, banderas };
                f(uno(Que::Especial(e), e.bdf, None));
            }
            DEV_ACPI_HID => f(uno(Que::PorHid, bdf, None)),
            _ => r.desconocidas += 1,
        }
        o += l;
    }
    r
}

/// **Cuenta las entradas de un IVHD** y copia los especiales en
/// `especiales`. Devuelve el censo y cuantos especiales copio. Es
/// [`por_entrada`] contado.
pub fn censar(bloque: &[u8], especiales: &mut [Especial]) -> (Censo, usize) {
    let mut c = Censo::default();
    let mut n = 0usize;
    if bloque.len() < 24 {
        c.cortado = true;
        return (c, 0);
    }
    c.tipo = bloque[0];
    if cabecera_ivhd(c.tipo) == Some(40) && bloque.len() >= 40 && u16_en(bloque, 2) >= 40 {
        c.efr = Some(u64_en(bloque, 24));
    }
    let r = por_entrada(bloque, |t| {
        c.max_bdf = c.max_bdf.max(t.hasta).max(t.alias.unwrap_or(0));
        match t.que {
            Que::Todos => c.todos += 1,
            Que::Uno => c.unos += 1,
            Que::Rango => c.rangos += 1,
            Que::Alias => c.alias += 1,
            Que::PorHid => c.por_hid += 1,
            Que::Especial(e) => {
                c.especiales += 1;
                if n < especiales.len() {
                    especiales[n] = e;
                    n += 1;
                }
            }
        }
    });
    c.desconocidas = r.desconocidas;
    c.cortado = r.cortado;
    (c, n)
}

/// Un bloque IVMD: memoria que el firmware declara con reglas.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Ivmd {
    /// [`IVMD_TODOS`], [`IVMD_UNO`] o [`IVMD_RANGO`].
    pub tipo: u8,
    /// `IVMD_*`: unidad (identidad), lee, escribe, exclusion.
    pub banderas: u8,
    /// El BDF (o el primero del rango).
    pub bdf: u16,
    /// El ultimo BDF del rango en [`IVMD_RANGO`]; nada en los otros.
    pub aux: u16,
    /// La memoria, fisica.
    pub inicio: u64,
    pub largo: u64,
}

/// **Los IVMD del IVRS.** Devuelve cuantos escribio.
pub fn leer_ivmd(ivrs: &[u8], salida: &mut [Ivmd]) -> usize {
    let mut n = 0usize;
    por_bloque(ivrs, |tipo, b| {
        if !es_ivmd(tipo) || b.len() < 32 || n >= salida.len() {
            return;
        }
        salida[n] = Ivmd {
            tipo,
            banderas: b[1],
            bdf: u16_en(b, 4),
            aux: u16_en(b, 6),
            inicio: u64_en(b, 16),
            largo: u64_en(b, 24),
        };
        n += 1;
    });
    n
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::vec::Vec;

    fn ivhd(tipo: u8, bdf: u16, entradas: &[u8]) -> Vec<u8> {
        let cab = cabecera_ivhd(tipo).unwrap();
        let mut b = vec![0u8; cab];
        b[0] = tipo;
        b[4..6].copy_from_slice(&bdf.to_le_bytes());
        b[8..16].copy_from_slice(&0xFEB8_0000u64.to_le_bytes());
        if cab == 40 {
            b[24..32].copy_from_slice(&0x0000_0000_0000_00EFu64.to_le_bytes());
        }
        b.extend_from_slice(entradas);
        let l = b.len() as u16;
        b[2..4].copy_from_slice(&l.to_le_bytes());
        b
    }

    fn ivmd(tipo: u8, banderas: u8, bdf: u16, inicio: u64, largo: u64) -> Vec<u8> {
        let mut b = vec![0u8; 32];
        b[0] = tipo;
        b[1] = banderas;
        b[2..4].copy_from_slice(&32u16.to_le_bytes());
        b[4..6].copy_from_slice(&bdf.to_le_bytes());
        b[16..24].copy_from_slice(&inicio.to_le_bytes());
        b[24..32].copy_from_slice(&largo.to_le_bytes());
        b
    }

    fn ivrs(bloques: &[Vec<u8>]) -> Vec<u8> {
        let mut t = vec![0u8; IVRS_CABECERA];
        t[0..4].copy_from_slice(b"IVRS");
        for b in bloques {
            t.extend_from_slice(b);
        }
        let l = t.len() as u32;
        t[4..8].copy_from_slice(&l.to_le_bytes());
        t
    }

    /// Las entradas de un Zen tipico: la raiz suelta, un rango, un alias
    /// detras de un puente, el IOAPIC y el HPET, y relleno.
    fn entradas() -> Vec<u8> {
        let mut e = Vec::new();
        e.extend_from_slice(&[DEV_UNO, 0x00, 0x00, 0x00]); // 00:00.0
        e.extend_from_slice(&[DEV_RANGO_DESDE, 0x08, 0x00, 0x00]); // 00:01.0
        e.extend_from_slice(&[DEV_RANGO_HASTA, 0xFF, 0x2B, 0x00]); // 2B:1F.7
        // Alias: 03:00.0 pide como 02:00.0 (ext = alias << 8).
        e.extend_from_slice(&[DEV_ALIAS, 0x00, 0x03, 0x00]);
        e.extend_from_slice(&(0x0200u32 << 8).to_le_bytes());
        // IOAPIC con id 0x21 en 00:14.0 (0xA0), y un HPET 0 en el mismo BDF.
        e.extend_from_slice(&[DEV_ESPECIAL, 0x00, 0x00, 0xD7]);
        e.extend_from_slice(&(0x21u32 | (0x00A0 << 8) | ((ESPECIAL_IOAPIC as u32) << 24)).to_le_bytes());
        e.extend_from_slice(&[DEV_ESPECIAL, 0x00, 0x00, 0x00]);
        e.extend_from_slice(&((0x00A0u32 << 8) | ((ESPECIAL_HPET as u32) << 24)).to_le_bytes());
        e.extend_from_slice(&[DEV_RELLENO, 0, 0, 0]);
        e
    }

    #[test]
    fn un_ivmd_no_es_una_iommu() {
        let t = ivrs(&[ivhd(IVHD_10, 0x0002, &entradas()), ivmd(IVMD_TODOS, IVMD_UNIDAD | IVMD_LEE | IVMD_ESCRIBE, 0, 0x9F00_0000, 0x10_0000)]);
        let mut v = [crate::Ivhd { tipo: 0, banderas: 0, largo: 0, id_dispositivo: 0, base_mmio: 0, segmento: 0 }; 4];
        assert_eq!(crate::leer_ivrs(&t, &mut v), 1, "el IVMD no se cuenta como IOMMU");
        assert_eq!(v[0].base_mmio, 0xFEB8_0000);
    }

    #[test]
    fn censo_de_las_entradas() {
        let t = ivrs(&[ivhd(IVHD_10, 0x0002, &entradas())]);
        let b = ivhd_elegido(&t).expect("hay IVHD");
        let mut esp = [Especial::default(); 4];
        let (c, n) = censar(b, &mut esp);
        assert_eq!((c.unos, c.rangos, c.alias, c.especiales, c.desconocidas), (1, 1, 1, 2, 0));
        assert!(!c.cortado);
        assert_eq!(c.max_bdf, 0x2BFF, "el fin del rango");
        assert_eq!(c.efr, None, "el 0x10 no trae EFR");
        assert_eq!(n, 2);
        assert_eq!(esp[0], Especial { tipo: ESPECIAL_IOAPIC, handle: 0x21, bdf: 0x00A0, banderas: 0xD7 });
        assert_eq!(esp[1].tipo, ESPECIAL_HPET);
    }

    #[test]
    fn se_elige_el_ivhd_con_mas_detalle() {
        let t = ivrs(&[ivhd(IVHD_10, 0x0002, &entradas()), ivhd(IVHD_11, 0x0002, &entradas())]);
        let b = ivhd_elegido(&t).unwrap();
        assert_eq!(b[0], IVHD_11);
        let (c, _) = censar(b, &mut []);
        assert_eq!(c.efr, Some(0xEF), "el 0x11 trae la copia del EFR");
        assert_eq!(c.especiales, 2, "sin sitio para copiarlos, se cuentan igual");
    }

    #[test]
    fn una_entrada_que_se_sale_corta_el_censo() {
        // Un alias (8 bytes) al que le faltan 4.
        let mut b = ivhd(IVHD_10, 2, &[DEV_UNO, 1, 0, 0, DEV_ALIAS, 0, 3, 0]);
        let l = b.len() as u16;
        b[2..4].copy_from_slice(&l.to_le_bytes());
        let (c, _) = censar(&b, &mut []);
        assert_eq!(c.unos, 1);
        assert!(c.cortado);
        // Un largo de cero en la tabla no deja el bucle girando.
        let mut t = ivrs(&[b]);
        t[IVRS_CABECERA + 2] = 0;
        t[IVRS_CABECERA + 3] = 0;
        assert_eq!(ivhd_elegido(&t), None);
    }

    #[test]
    fn cada_tramo_con_sus_banderas() {
        let t = ivrs(&[ivhd(IVHD_10, 0x0002, &entradas())]);
        let b = ivhd_elegido(&t).unwrap();
        let mut v = Vec::new();
        let r = por_entrada(b, |x| v.push(x));
        assert!(!r.cortado);
        assert_eq!(v.len(), 5, "uno, rango, alias, IOAPIC, HPET");
        assert_eq!((v[1].que, v[1].desde, v[1].hasta), (Que::Rango, 0x0008, 0x2BFF));
        assert_eq!((v[2].que, v[2].desde, v[2].alias), (Que::Alias, 0x0300, Some(0x0200)));
        assert_eq!((v[3].desde, v[3].banderas), (0x00A0, 0xD7), "el IOAPIC, con sus banderas");
        assert!(matches!(v[3].que, Que::Especial(e) if e.tipo == ESPECIAL_IOAPIC));
    }

    #[test]
    fn los_ivmd_se_leen() {
        let t = ivrs(&[
            ivhd(IVHD_10, 2, &[]),
            ivmd(IVMD_UNO, IVMD_UNIDAD | IVMD_LEE, 0x0098, 0xA000_0000, 0x2000),
            ivmd(IVMD_RANGO, IVMD_EXCLUSION, 0x0100, 0xB000_0000, 0x1000),
        ]);
        let mut m = [Ivmd::default(); 4];
        assert_eq!(leer_ivmd(&t, &mut m), 2);
        assert_eq!((m[0].tipo, m[0].bdf, m[0].inicio, m[0].largo), (IVMD_UNO, 0x0098, 0xA000_0000, 0x2000));
        assert_eq!(m[1].banderas, IVMD_EXCLUSION);
    }
}
