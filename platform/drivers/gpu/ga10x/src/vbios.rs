//! **LA VBIOS DE LA 3060, LEIDA (L0a)** -- donde esta FWSEC, como se firma y
//! donde pondra la WPR2. Solo lee bytes que ya se leyeron.
//!
//! capa: puro -- recibe la ROM entera como bytes y devuelve lo que dice; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- un solo byte mal contado aqui es firmware ajeno
//!           ejecutandose con la firma equivocada
//!
//! # Por que existe (2026-09-24)
//!
//! L0 de `docs/plan/PLAN_LA_3060.md` empieza por FWSEC-FRTS: el firmware
//! FIRMADO por Nvidia que viene DENTRO de la VBIOS de la tarjeta, corre en el
//! falcon del GSP (el mismo de la prueba de fuego) y aparta la WPR2 en la VRAM.
//! Antes de cargarlo, se PREGUNTA (LEY 24): donde esta, cuanto mide, que firma
//! pide el fusible de ESTA tarjeta, y si trae la interfaz para pedirle FRTS.
//!
//! # El camino (nova-core, Linux v6.17: `vbios.rs`, `firmware.rs`,
//! `firmware/fwsec.rs`, `fb.rs`), leido el 24-09
//!
//! ```text
//!    la ROM (BAR0 + 0x300000)  una cadena de IMAGENES PCI: 0x55AA, PCIR (o
//!                              NPDS), su medida en bloques de 512, su TIPO
//!                              (0x00 PCI-AT, 0x03 EFI, 0x70 NBSI, 0xE0 FWSEC)
//!                              y la ultima; NPDE, si esta, manda en la medida
//!    la PCI-AT                 la cabecera BIT (FF B8 "BIT" 00) y su ficha
//!                              0x70: un puntero a los DATOS DEL FALCON
//!    el puntero                cuenta desde el principio de la PCI-AT, saltando
//!                              la PCI-AT y la primera FWSEC: cae en la tabla de
//!                              la PMU, en una de las dos imagenes FWSEC
//!    la tabla de la PMU        la entrada 0x85 (FWSEC de PRODUCCION) apunta al
//!                              descriptor, en la SEGUNDA imagen FWSEC
//!    el descriptor v3          44 bytes: medidas de IMEM y DMEM, donde va la
//!                              firma, que motor y que ucode la comprueban, y
//!                              cuantas firmas de 384 bytes vienen detras
//!    la interfaz (APPIF)       en la DMEM: la entrada 4, DMEMMAPPER, dice donde
//!                              va la ORDEN (FRTS = 0x15)
//! ```
//!
//! # La firma se elige por el FUSIBLE, no por el fichero
//!
//! El fallo 4 de FastOS. El registro `FUSE_OPT_FPF_<motor>_UCODE<id>_VERSION`
//! dice que version quemo la fabrica; la firma buena es la que ocupa, entre las
//! que trae el descriptor (`signature_versions`), el puesto de ese bit:
//! [`indice_de_firma`].

use crate::es_error_pri;

/// La ventana de la ROM en BAR0, y lo mas que se recorre (`BIOS_MAX_SCAN_LEN`).
pub const ROM: u32 = 0x0030_0000;
pub const ROM_MAX: usize = 0x10_0000;

pub const TIPO_PCI_AT: u8 = 0x00;
pub const TIPO_EFI: u8 = 0x03;
pub const TIPO_NBSI: u8 = 0x70;
pub const TIPO_FWSEC: u8 = 0xE0;

const FICHA_FALCON: u8 = 0x70;
const APP_FWSEC_PROD: u8 = 0x85;
const APPIF_DMEMMAPPER: u32 = 0x4;
/// La orden que aparta la WPR2 (`NVFW_FALCON_APPIF_DMEMMAPPER_CMD_FRTS`).
pub const ORDEN_FRTS: u32 = 0x15;
/// Bytes de una firma RSA-3K (`BCRT30_RSA3K_SIG_SIZE`).
pub const FIRMA: usize = 384;
/// Bytes del descriptor v3 (`FalconUCodeDescV3`, `repr(C)`).
pub const DESCRIPTOR: usize = 44;

/// Por que no.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoVbios {
    /// No empieza por una imagen PCI (0x55AA): la ROM no se leyo, o no es.
    SinRom,
    /// Una imagen dice medir algo que se sale de lo leido.
    Corta(usize),
    /// Falta la PCI-AT, o una de las dos FWSEC.
    FaltaImagen(u8),
    /// La PCI-AT no trae BIT, o el BIT no trae la ficha del falcon.
    SinBit,
    /// El puntero o la tabla de la PMU caen fuera de su imagen.
    TablaFuera,
    /// La tabla no trae FWSEC de produccion (0x85).
    SinFwsec,
    /// El descriptor no es v3, o cae fuera.
    Descriptor(u32),
    /// La interfaz de la DMEM no trae DMEMMAPPER: no se le puede pedir FRTS.
    SinDmemmapper,
}

/// Una imagen de la ROM.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Imagen {
    pub desde: usize,
    pub bytes: usize,
    pub tipo: u8,
    pub ultima: bool,
}

fn u16_en(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn u32_en(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?]))
}

/// La imagen que empieza en `desde`.
fn imagen_en(rom: &[u8], desde: usize) -> Result<Imagen, NoVbios> {
    let d = rom.get(desde..).ok_or(NoVbios::Corta(desde))?;
    match u16_en(d, 0) {
        Some(0xAA55) | Some(0xBB77) | Some(0x4E56) => {}
        _ => return Err(if desde == 0 { NoVbios::SinRom } else { NoVbios::Corta(desde) }),
    }
    let pcir = u16_en(d, 24).ok_or(NoVbios::Corta(desde))? as usize;
    let p = d.get(pcir..pcir + 24).ok_or(NoVbios::Corta(desde))?;
    if &p[0..4] != b"PCIR" && &p[0..4] != b"NPDS" {
        return Err(NoVbios::Corta(desde));
    }
    let largo_pcir = u16_en(p, 10).unwrap_or(0) as usize;
    let mut bytes = u16_en(p, 16).unwrap_or(0) as usize * 512;
    let tipo = p[20];
    let mut ultima = p[21] & 0x80 != 0;
    // El NPDE, si esta, manda (`NpdeStruct::find_in_data`).
    let npde = (pcir + largo_pcir + 0x0F) & !0x0F;
    if let Some(n) = d.get(npde..npde + 11) {
        if &n[0..4] == b"NPDE" {
            let sub = u16_en(n, 8).unwrap_or(0) as usize;
            if sub != 0 {
                bytes = sub * 512;
                ultima = n[10] & 0x80 != 0;
            }
        }
    }
    if tipo == TIPO_NBSI {
        ultima = true;
    }
    if bytes == 0 || desde + bytes > rom.len() {
        return Err(NoVbios::Corta(desde));
    }
    Ok(Imagen { desde, bytes, tipo, ultima })
}

/// **Las imagenes de la ROM**, en orden. Devuelve cuantas cupieron en `fuera`.
pub fn imagenes(rom: &[u8], fuera: &mut [Imagen]) -> Result<usize, NoVbios> {
    let mut n = 0;
    let mut desde = 0;
    while n < fuera.len() && desde < ROM_MAX.min(rom.len()) {
        let im = imagen_en(rom, desde)?;
        fuera[n] = im;
        n += 1;
        if im.ultima {
            break;
        }
        desde = (desde + im.bytes).next_multiple_of(512);
    }
    Ok(n)
}

/// **El descriptor v3 de FWSEC**, tal cual (`FalconUCodeDescV3`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Descriptor {
    pub hdr: u32,
    pub pkc_data_offset: u32,
    pub interface_offset: u32,
    pub imem_phys_base: u32,
    pub imem_load_size: u32,
    pub imem_virt_base: u32,
    pub dmem_phys_base: u32,
    pub dmem_load_size: u32,
    pub engine_id_mask: u16,
    pub ucode_id: u8,
    pub signature_count: u8,
    pub signature_versions: u16,
}

impl Descriptor {
    pub const fn version(&self) -> u32 {
        (self.hdr >> 8) & 0xFF
    }
    /// Lo que mide la cabecera ENTERA (con las firmas): el ucode va detras.
    pub const fn medida(&self) -> usize {
        (self.hdr >> 16) as usize
    }
}

/// **FWSEC, hallado.** Todo relativo a la ROM entera.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Fwsec {
    pub desc: Descriptor,
    /// Donde empieza el descriptor en la ROM.
    pub en_rom: usize,
    /// Donde empiezan las firmas, y donde el ucode (IMEM y luego DMEM).
    pub firmas: usize,
    pub ucode: usize,
    /// La DMEMMAPPER: su base en la DMEM, su firma de 4 letras, y donde va
    /// la orden (relativo a la DMEM).
    pub dmemmapper: u32,
    pub dmemmapper_firma: u32,
    pub orden_en: u32,
    pub orden_medida: u32,
    /// La orden con la que viene: la que BMO-X cambiara por FRTS.
    pub orden_de_fabrica: u32,
}

/// **Los 44 bytes de un descriptor v3, leidos.** Lo usa tambien el kernel,
/// que lo vuelve a leer de la ROM por su cuenta antes de cargar nada.
pub fn descriptor(d: &[u8]) -> Option<Descriptor> {
    if d.len() < DESCRIPTOR {
        return None;
    }
    let w = |k: usize| u32_en(d, k).unwrap_or(0);
    Some(Descriptor {
        hdr: w(0),
        pkc_data_offset: w(8),
        interface_offset: w(12),
        imem_phys_base: w(16),
        imem_load_size: w(20),
        imem_virt_base: w(24),
        dmem_phys_base: w(28),
        dmem_load_size: w(32),
        engine_id_mask: u16_en(d, 36)?,
        ucode_id: d[38],
        signature_count: d[39],
        signature_versions: u16_en(d, 40)?,
    })
}

/// **Encontrar FWSEC** en la ROM, por el camino de nova-core.
pub fn fwsec(rom: &[u8]) -> Result<Fwsec, NoVbios> {
    let mut ims = [Imagen::default(); 8];
    let n = imagenes(rom, &mut ims)?;
    let ims = &ims[..n];
    let pci_at = ims.iter().find(|i| i.tipo == TIPO_PCI_AT).ok_or(NoVbios::FaltaImagen(TIPO_PCI_AT))?;
    let mut fw = ims.iter().filter(|i| i.tipo == TIPO_FWSEC);
    let primera = fw.next().ok_or(NoVbios::FaltaImagen(TIPO_FWSEC))?;
    let segunda = fw.next().ok_or(NoVbios::FaltaImagen(TIPO_FWSEC))?;
    let at = &rom[pci_at.desde..pci_at.desde + pci_at.bytes];

    // -- El BIT y su ficha 0x70 --
    let bit = at.windows(6).position(|w| w == [0xFF, 0xB8, b'B', b'I', b'T', 0]).ok_or(NoVbios::SinBit)?;
    let cab = at.get(bit..bit + 12).ok_or(NoVbios::SinBit)?;
    let (largo_cab, largo_ficha, fichas) = (cab[8] as usize, cab[9] as usize, cab[10] as usize);
    let mut datos = None;
    for i in 0..fichas {
        let f = bit + largo_cab + i * largo_ficha;
        let Some(ficha) = at.get(f..f + 6) else { break };
        if ficha[0] == FICHA_FALCON {
            datos = u16_en(ficha, 4);
            break;
        }
    }
    let datos = datos.ok_or(NoVbios::SinBit)? as usize;
    let puntero = u32_en(at, datos).ok_or(NoVbios::SinBit)? as usize;

    // -- La tabla de la PMU, en la primera o en la segunda FWSEC --
    let mut o = puntero.checked_sub(pci_at.bytes).ok_or(NoVbios::TablaFuera)?;
    let tabla_im = if o < primera.bytes {
        primera
    } else {
        o -= primera.bytes;
        segunda
    };
    let t = rom.get(tabla_im.desde + o..tabla_im.desde + tabla_im.bytes).ok_or(NoVbios::TablaFuera)?;
    let (largo_t, largo_e, entradas) = (*t.get(1).ok_or(NoVbios::TablaFuera)? as usize, t[2] as usize, t[3] as usize);
    if largo_e < 6 || largo_t + entradas * largo_e > t.len() {
        return Err(NoVbios::TablaFuera);
    }
    let mut ucode_ptr = None;
    for i in 0..entradas {
        let e = &t[largo_t + i * largo_e..];
        if e[0] == APP_FWSEC_PROD {
            ucode_ptr = u32_en(e, 2);
            break;
        }
    }
    let ucode_ptr = ucode_ptr.ok_or(NoVbios::SinFwsec)? as usize;
    let od = ucode_ptr
        .checked_sub(pci_at.bytes)
        .and_then(|x| x.checked_sub(primera.bytes))
        .ok_or(NoVbios::Descriptor(0))?;
    let en_rom = segunda.desde + od;
    let fin_seg = segunda.desde + segunda.bytes;

    // -- El descriptor v3 --
    let d = rom.get(en_rom..en_rom + DESCRIPTOR).filter(|_| en_rom + DESCRIPTOR <= fin_seg).ok_or(NoVbios::Descriptor(0))?;
    let desc = descriptor(d).ok_or(NoVbios::Descriptor(0))?;
    if desc.version() != 3 {
        return Err(NoVbios::Descriptor(desc.hdr));
    }
    let firmas = en_rom + DESCRIPTOR;
    let ucode = en_rom + desc.medida();
    let largo = desc.imem_load_size as usize + desc.dmem_load_size as usize;
    if firmas + desc.signature_count as usize * FIRMA > fin_seg || ucode + largo > fin_seg || desc.medida() < DESCRIPTOR {
        return Err(NoVbios::Descriptor(desc.hdr));
    }

    // -- La interfaz de la DMEM: la DMEMMAPPER --
    let dmem = ucode + desc.imem_load_size as usize;
    let fin_dmem = dmem + desc.dmem_load_size as usize;
    let h = dmem + desc.interface_offset as usize;
    let hdr = rom.get(h..h + 4).filter(|_| h + 4 <= fin_dmem).ok_or(NoVbios::SinDmemmapper)?;
    if hdr[0] != 1 {
        return Err(NoVbios::SinDmemmapper);
    }
    let (largo_h, largo_a, apps) = (hdr[1] as usize, hdr[2] as usize, hdr[3] as usize);
    for i in 0..apps {
        let a = h + largo_h + i * largo_a;
        let (Some(id), Some(base)) = (u32_en(rom, a), u32_en(rom, a + 4)) else { break };
        if a + 8 > fin_dmem || id != APPIF_DMEMMAPPER {
            continue;
        }
        let m = dmem + base as usize;
        if m + 64 > fin_dmem {
            return Err(NoVbios::SinDmemmapper);
        }
        let r = |k: usize| u32_en(rom, m + k).unwrap_or(0);
        return Ok(Fwsec {
            desc,
            en_rom,
            firmas,
            ucode,
            dmemmapper: base,
            dmemmapper_firma: r(0),
            orden_en: r(8),
            orden_medida: r(12),
            orden_de_fabrica: r(44),
        });
    }
    Err(NoVbios::SinDmemmapper)
}

// -- El fusible, y la firma que pide ------------------------------------------

/// El primer registro de fusibles de cada motor (`NV_FUSE_OPT_FPF_*_UCODE1_VERSION`).
pub const FUSIBLE_SEC2: u32 = 0x0082_4140;
pub const FUSIBLE_NVDEC: u32 = 0x0082_4100;
pub const FUSIBLE_GSP: u32 = 0x0082_41C0;

/// **Que registro de fusibles lee este descriptor** (`signature_reg_fuse_version_ga102`).
pub const fn registro_fusible(engine_id_mask: u16, ucode_id: u8) -> Option<u32> {
    if ucode_id == 0 || ucode_id > 16 {
        return None;
    }
    let base = if engine_id_mask & 0x0001 != 0 {
        FUSIBLE_SEC2
    } else if engine_id_mask & 0x0004 != 0 {
        FUSIBLE_NVDEC
    } else if engine_id_mask & 0x0400 != 0 {
        FUSIBLE_GSP
    } else {
        return None;
    };
    Some(base + (ucode_id as u32 - 1) * 4)
}

/// La version QUEMADA: el puesto del bit mas alto (0 si no hay ninguno).
pub const fn version_del_fusible(crudo: u32) -> u32 {
    if es_error_pri(crudo) {
        return 0;
    }
    32 - crudo.leading_zeros()
}

/// **Cual de las firmas del descriptor pide ESTE fusible.** `None` si
/// ninguna: entonces no se carga, porque la ROM de la tarjeta la rechazaria.
pub const fn indice_de_firma(signature_versions: u16, version: u32) -> Option<u32> {
    if version >= 16 {
        return None;
    }
    let bit = 1u32 << version;
    let v = signature_versions as u32;
    if v & bit == 0 {
        return None;
    }
    Some((v & (bit - 1)).count_ones())
}

// -- Donde va la WPR2 (`FbLayout`) ---------------------------------------------

/// Lo que FRTS apartara, contado desde el principio de la VRAM.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Frts {
    pub vram: u64,
    pub vga: u64,
    pub desde: u64,
    pub hasta: u64,
}

/// **La cuenta de nova-core.** `vram_mb` de `0x1183A4`; `vga_crudo` de
/// `0x625F04` (bit 3 valido, bits 8..31 la base / 64 KiB); `pantalla` si el
/// fusible de la pantalla no la apago.
pub const fn frts(vram_mb: u32, vga_crudo: u32, pantalla: bool) -> Frts {
    const MIB: u64 = 1 << 20;
    const K128: u64 = 128 * 1024;
    let vram = vram_mb as u64 * MIB;
    let base = vram.saturating_sub(MIB);
    let vga = if pantalla && vga_crudo & (1 << 3) != 0 {
        let a = ((vga_crudo >> 8) as u64) << 16;
        if a < base { vram.saturating_sub(K128) } else { a }
    } else {
        base
    };
    let desde = (vga & !(K128 - 1)).saturating_sub(MIB);
    Frts { vram, vga, desde, hasta: desde + MIB }
}

// ===================================================================
//  PRUEBAS -- una ROM de mentira armada byte a byte
// ===================================================================

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec;
    use std::vec::Vec;

    fn p16(b: &mut [u8], o: usize, v: u16) {
        b[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn p32(b: &mut [u8], o: usize, v: u32) {
        b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// Una imagen de `bloques` x 512 con su PCIR en 0x40.
    fn imagen(bloques: u16, tipo: u8, ultima: bool) -> Vec<u8> {
        let mut b = vec![0u8; bloques as usize * 512];
        p16(&mut b, 0, 0xAA55);
        p16(&mut b, 24, 0x40);
        b[0x40..0x44].copy_from_slice(b"PCIR");
        p16(&mut b, 0x40 + 10, 24);
        p16(&mut b, 0x40 + 16, bloques);
        b[0x40 + 20] = tipo;
        b[0x40 + 21] = if ultima { 0x80 } else { 0 };
        b
    }

    const IMEM: u32 = 0x400;
    const DMEM: u32 = 0x300;

    /// PCI-AT (2 bloques) + FWSEC 1 (2) + FWSEC 2 (8) + EFI (1, la ultima).
    fn rom() -> Vec<u8> {
        let mut at = imagen(2, TIPO_PCI_AT, false);
        // BIT en 0x100: cabecera de 12, fichas de 6, dos fichas.
        at[0x100..0x106].copy_from_slice(&[0xFF, 0xB8, b'B', b'I', b'T', 0]);
        at[0x108] = 12;
        at[0x109] = 6;
        at[0x10A] = 2;
        at[0x10C] = 0x32; // otra ficha
        at[0x112] = FICHA_FALCON;
        p16(&mut at, 0x112 + 4, 0x180); // sus datos en 0x180
        // El puntero: la tabla de la PMU en la SEGUNDA FWSEC, a +0x100.
        p32(&mut at, 0x180, 1024 + 1024 + 0x100);

        let f1 = imagen(2, TIPO_FWSEC, false);
        let mut f2 = imagen(8, TIPO_FWSEC, false);
        // La tabla de la PMU: cabecera 4, entradas de 6, dos entradas.
        let t = 0x100;
        f2[t] = 1;
        f2[t + 1] = 4;
        f2[t + 2] = 6;
        f2[t + 3] = 2;
        f2[t + 4] = 0x45; // FWSEC de depuracion: no es esta
        f2[t + 10] = APP_FWSEC_PROD;
        let d = 0x200; // el descriptor, en la segunda FWSEC
        p32(&mut f2, t + 12, (1024 + 1024 + d) as u32);
        // El descriptor v3: medida 44 + 2 firmas.
        let medida = (DESCRIPTOR + 2 * FIRMA) as u32;
        p32(&mut f2, d, medida << 16 | 3 << 8 | 1);
        p32(&mut f2, d + 8, 0x10); // pkc_data_offset
        p32(&mut f2, d + 12, 0x40); // interface_offset
        p32(&mut f2, d + 20, IMEM);
        p32(&mut f2, d + 32, DMEM);
        p16(&mut f2, d + 36, 0x0400); // GSP
        f2[d + 38] = 1; // ucode 1
        f2[d + 39] = 2; // dos firmas
        p16(&mut f2, d + 40, 0b0110); // versiones 1 y 2
        // La DMEM: la interfaz en +0x40, la DMEMMAPPER en +0x80.
        let dm = d + medida as usize + IMEM as usize;
        f2[dm + 0x40] = 1;
        f2[dm + 0x41] = 4;
        f2[dm + 0x42] = 8;
        f2[dm + 0x43] = 2;
        p32(&mut f2, dm + 0x44, 0x9); // otra app
        p32(&mut f2, dm + 0x4C, APPIF_DMEMMAPPER);
        p32(&mut f2, dm + 0x50, 0x80);
        f2[dm + 0x80..dm + 0x84].copy_from_slice(b"DMAP");
        p32(&mut f2, dm + 0x80 + 8, 0x200); // la orden va en DMEM + 0x200
        p32(&mut f2, dm + 0x80 + 12, 0x100);
        p32(&mut f2, dm + 0x80 + 44, 0x19); // de fabrica: SB

        let efi = imagen(1, TIPO_EFI, true);
        [at, f1, f2, efi].concat()
    }

    #[test]
    fn las_imagenes_de_la_rom_en_orden() {
        let r = rom();
        let mut ims = [Imagen::default(); 8];
        let n = imagenes(&r, &mut ims).unwrap();
        assert_eq!(n, 4);
        let tipos: Vec<u8> = ims[..n].iter().map(|i| i.tipo).collect();
        assert_eq!(tipos, [TIPO_PCI_AT, TIPO_FWSEC, TIPO_FWSEC, TIPO_EFI]);
        assert_eq!(ims[2].desde, 2048);
        assert_eq!(ims[2].bytes, 4096);
        assert!(ims[3].ultima);
    }

    #[test]
    fn fwsec_se_encuentra_por_el_camino_de_nova_core() {
        let r = rom();
        let f = fwsec(&r).expect("hallado");
        assert_eq!(f.en_rom, 2048 + 0x200);
        assert_eq!(f.desc.version(), 3);
        assert_eq!((f.desc.imem_load_size, f.desc.dmem_load_size), (IMEM, DMEM));
        assert_eq!((f.desc.engine_id_mask, f.desc.ucode_id, f.desc.signature_count), (0x400, 1, 2));
        assert_eq!(f.firmas, f.en_rom + DESCRIPTOR);
        assert_eq!(f.ucode, f.en_rom + DESCRIPTOR + 2 * FIRMA);
        assert_eq!(f.dmemmapper, 0x80);
        assert_eq!(&f.dmemmapper_firma.to_le_bytes(), b"DMAP");
        assert_eq!((f.orden_en, f.orden_medida, f.orden_de_fabrica), (0x200, 0x100, 0x19));
    }

    #[test]
    fn el_npde_manda_en_la_medida() {
        let mut im = imagen(1, TIPO_FWSEC, true);
        // PCIR de 24 en 0x40: el NPDE va en (0x40 + 24 + 15) & !15 = 0x60.
        im[0x60..0x64].copy_from_slice(b"NPDE");
        p16(&mut im, 0x60 + 8, 1);
        im[0x60 + 10] = 0x80;
        im[0x40 + 21] = 0; // la PCIR dice que no es la ultima; el NPDE, que si
        let mut ims = [Imagen::default(); 2];
        assert_eq!(imagenes(&im, &mut ims), Ok(1));
        assert!(ims[0].ultima);
    }

    #[test]
    fn una_rom_que_no_es_rom_no_se_lee() {
        let mut ims = [Imagen::default(); 2];
        assert_eq!(imagenes(&[0xFF; 1024], &mut ims), Err(NoVbios::SinRom));
        assert_eq!(fwsec(&[0xBA; 64]), Err(NoVbios::SinRom));
        let mut r = rom();
        r.truncate(3000); // la segunda FWSEC dice medir mas de lo leido
        assert_eq!(fwsec(&r), Err(NoVbios::Corta(2048)));
    }

    #[test]
    fn sin_la_entrada_de_produccion_no_hay_fwsec() {
        let mut r = rom();
        r[2048 + 0x100 + 10] = 0x05; // la 0x85 pasa a ser otra cosa
        assert_eq!(fwsec(&r), Err(NoVbios::SinFwsec));
    }

    #[test]
    fn un_descriptor_que_no_es_v3_no_se_cree() {
        let mut r = rom();
        r[2048 + 0x200 + 1] = 2;
        assert!(matches!(fwsec(&r), Err(NoVbios::Descriptor(_))));
    }

    #[test]
    fn bytes_hostiles_nunca_revientan() {
        // Cada byte de la ROM buena, cambiado a cada valor frontera: el
        // analizador puede decir que no, pero nunca salirse de la rebanada.
        let base = rom();
        for i in (0..base.len()).step_by(7) {
            for v in [0x00, 0x7F, 0x80, 0xFF] {
                let mut r = base.clone();
                r[i] = v;
                let _ = fwsec(&r);
            }
        }
    }

    #[test]
    fn la_firma_se_elige_por_el_fusible() {
        // versiones 1 y 2 presentes (0b0110): el fusible en 2 pide la SEGUNDA.
        assert_eq!(version_del_fusible(0b10), 2);
        assert_eq!(indice_de_firma(0b0110, 2), Some(1));
        assert_eq!(indice_de_firma(0b0110, 1), Some(0));
        assert_eq!(indice_de_firma(0b0110, 3), None, "una version que no viene no se inventa");
        assert_eq!(version_del_fusible(0), 0);
        assert_eq!(version_del_fusible(0xBADF_1100), 0);
        assert_eq!(registro_fusible(0x400, 1), Some(0x8241C0));
        assert_eq!(registro_fusible(0x001, 3), Some(0x824148));
        assert_eq!(registro_fusible(0x400, 0), None);
    }

    #[test]
    fn la_frts_de_nova_core() {
        // 12 GiB, la VGA en la ultima MiB (valida): FRTS 1 MiB por debajo,
        // alineado a 128 KiB.
        // El registro: bits 8..31 = la base / 64 KiB, bit 3 valido.
        let vga = (((12 * 1024 - 1) * 16) << 8) | 1 << 3;
        let f = frts(12 * 1024, vga, true);
        assert_eq!(f.vram, 12 << 30);
        assert_eq!(f.vga, (12 << 30) - (1 << 20));
        // Y el camino del registro de verdad: una VGA en 12 GiB - 512 KiB.
        let arriba = (((12 * 1024 * 16) - 8) << 8) | 1 << 3;
        assert_eq!(frts(12 * 1024, arriba, true).vga, (12 << 30) - 512 * 1024);
        assert_eq!(f.hasta, f.vga);
        assert_eq!(f.hasta - f.desde, 1 << 20);
        // Sin pantalla, la VGA es la ultima MiB igual.
        assert_eq!(frts(12 * 1024, 0, false).vga, (12 << 30) - (1 << 20));
        // Una VGA por debajo de la ultima MiB: los ultimos 128 KiB.
        assert_eq!(frts(12 * 1024, (1 << 8) | 1 << 3, true).vga, (12 << 30) - 128 * 1024);
    }
}
