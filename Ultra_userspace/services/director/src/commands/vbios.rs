//! **`gpu vbios`: L0a, LO QUE FWSEC NECESITA, PREGUNTADO.** Lee la VBIOS de la
//! 3060 entera, encuentra FWSEC dentro, pregunta que firma pide su fusible y
//! donde ira la WPR2, y guarda la ROM en `datos/vbios.rom`.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: ~130.000
//!                     preguntas de 8 bytes, un cuarto de segundo
//!
//! # Por que la ROM se lee DESDE AQUI y no en el kernel (2026-09-24)
//!
//! Porque es un MiB por MMIO, y dentro de un syscall eso seria un cuarto de
//! segundo con las interrupciones cerradas: sin raton, sin USB. De ocho en ocho
//! bytes (`INFO_GPU_ROM`), entre pregunta y pregunta la maquina sigue viva.
//! Y quien la entiende es `bmo_gpu_ga10x::vbios`, probado en el anfitrion.
//!
//! # Lo que NO hace
//!
//! No carga nada ni escribe un registro: eso es L0b. Esto dice si se PUEDE.

use bmo_gpu_ga10x::vbios::{self as vb, Fwsec, Imagen, NoVbios};
use bmo_userland as bmo;

use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Donde se guarda la ROM leida. 8.3.
const RUTA: &[u8] = b"datos/vbios.rom";

/// Lo que se saco de la ultima lectura.
#[derive(Clone, Copy)]
struct Resumen {
    bytes: usize,
    imagenes: [Imagen; 8],
    n: usize,
    fwsec: Result<Fwsec, NoVbios>,
    fusible_reg: u32,
    fusible: u32,
    guardada: bool,
}

static mut RESUMEN: Option<Resumen> = None;

fn resumen() -> Option<Resumen> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(RESUMEN) }
}

/// **Se leyo y FWSEC tiene firma para este fusible?** Lo pregunta `save mode`.
pub(crate) fn lista() -> bool {
    resumen().map_or(false, |r| match r.fwsec {
        Ok(f) => vb::indice_de_firma(f.desc.signature_versions, vb::version_del_fusible(r.fusible)).is_some(),
        Err(_) => false,
    })
}

/// **Leer, entender y guardar.** `Ok(bytes de la ROM)`.
pub(crate) fn leer() -> Result<u64, u32> {
    let Some(bloque) = bmo::Memoria::request(vb::ROM_MAX as u64) else {
        return Err(NO_SIN_MEMORIA);
    };
    // SAFETY: el bloque mide ROM_MAX bytes, es de este proceso y se escribe
    // aqui antes de leerse.
    let rom = unsafe { core::slice::from_raw_parts_mut(bloque.base(), vb::ROM_MAX) };
    for off in (0..vb::ROM_MAX).step_by(8) {
        let v = bmo::info(bmo::INFO_GPU_ROM | (off as u64) << 8);
        rom[off..off + 8].copy_from_slice(&v.to_le_bytes());
    }
    let mut ims = [Imagen::default(); 8];
    let n = vb::imagenes(rom, &mut ims).unwrap_or(0);
    let bytes = if n > 0 { ims[n - 1].desde + ims[n - 1].bytes } else { 0 };
    let fwsec = vb::fwsec(rom);
    let (mut fusible_reg, mut fusible) = (0, 0);
    if let Ok(f) = fwsec {
        if let Some(reg) = vb::registro_fusible(f.desc.engine_id_mask, f.desc.ucode_id) {
            fusible_reg = reg;
            fusible = bmo::info(bmo::INFO_GPU_FUSIBLE | (reg as u64) << 8) as u32;
        }
    }
    // Se guarda lo que mide la cadena de imagenes, o el MiB si no se entendio:
    // una ROM que no se entiende es justo la que hay que poder mirar fuera.
    let a_guardar = if bytes > 0 { bytes } else { vb::ROM_MAX };
    let guardada = match bmo::Archivo::create(RUTA) {
        Ok(a) => a.escribir_de(&bloque, 0, a_guardar as u64) == a_guardar as u64 && a.close(),
        Err(_) => false,
    };
    // SAFETY: como `resumen`.
    unsafe {
        *core::ptr::addr_of_mut!(RESUMEN) = Some(Resumen { bytes, imagenes: ims, n, fwsec, fusible_reg, fusible, guardada });
    }
    match fwsec {
        Ok(_) => Ok(bytes as u64),
        Err(_) => Err(NO_SIN_FWSEC),
    }
}

/// Motivos del escritorio (fuera del rango del kernel).
pub(crate) const NO_SIN_MEMORIA: u32 = 0x110;
pub(crate) const NO_SIN_FWSEC: u32 = 0x111;
pub(crate) const NO_SIN_FIRMA: u32 = 0x112;

/// El paso de `save mode`: leer, y que haya firma para ESTE fusible.
pub(crate) fn paso() -> Result<u64, u32> {
    let b = leer()?;
    if !lista() {
        return Err(NO_SIN_FIRMA);
    }
    Ok(b)
}

/// `gpu vbios`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "leyendo la VBIOS de la 3060", INK_DIM);
    let r = leer();
    let g = &mut dsk.out.grid;
    match r {
        Ok(b) => {
            g.with_ink(INK_GOOD);
            g.text(b"  VBIOS leida: ");
            g.dec(b / 1024);
            g.text(b" KiB, FWSEC hallado\n");
        }
        Err(m) => {
            g.with_ink(INK_ERR);
            g.text(b"  NO: ");
            g.text(super::iommu::motivo(m));
            g.byte(b'\n');
        }
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "vbios", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn tipo(t: u8) -> &'static [u8] {
    match t {
        vb::TIPO_PCI_AT => b"PCI-AT",
        vb::TIPO_EFI => b"EFI",
        vb::TIPO_NBSI => b"NBSI",
        vb::TIPO_FWSEC => b"FWSEC",
        _ => b"?",
    }
}

/// **Las filas de L0a.** Salen en `gpu` y en el `save`, si se leyo.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = resumen() else { return };
    campo(s, b"vbios");
    s.dec(r.bytes as u64 / 1024);
    s.text(b" KiB en ");
    s.dec(r.n as u64);
    s.text(b" imagenes:");
    for im in &r.imagenes[..r.n] {
        s.byte(b' ');
        s.text(tipo(im.tipo));
        s.byte(b'(');
        s.dec(im.bytes as u64 / 1024);
        s.text(b"K)");
    }
    s.with_ink(INK_ECHO);
    s.text(if r.guardada { b"   -> datos/vbios.rom" as &[u8] } else { b"   (NO se pudo guardar)" });
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    campo(s, b"fwsec");
    match r.fwsec {
        Err(e) => {
            s.with_ink(INK_ERR);
            s.text(match e {
                NoVbios::SinRom => b"la ventana de la ROM no empieza por 0x55AA" as &[u8],
                NoVbios::Corta(_) => b"una imagen se sale de lo leido",
                NoVbios::FaltaImagen(_) => b"falta la PCI-AT o una de las dos imagenes FWSEC",
                NoVbios::SinBit => b"la PCI-AT no trae BIT, o su ficha del falcon",
                NoVbios::TablaFuera => b"la tabla de la PMU cae fuera de su imagen",
                NoVbios::SinFwsec => b"la tabla no trae FWSEC de produccion (0x85)",
                NoVbios::Descriptor(_) => b"el descriptor no es v3, o cae fuera",
                NoVbios::SinDmemmapper => b"sin DMEMMAPPER: no se le puede pedir FRTS",
            });
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }
        Ok(f) => {
            s.with_ink(INK_GOOD);
            s.text(b"v3 en 0x");
            s.hex(f.en_rom as u64, 5);
            s.with_ink(INK_PLAIN);
            s.text(b": IMEM ");
            s.dec(f.desc.imem_load_size as u64);
            s.text(b" B, DMEM ");
            s.dec(f.desc.dmem_load_size as u64);
            s.text(b" B, motor 0x");
            s.hex(f.desc.engine_id_mask as u64, 4);
            s.text(b" ucode ");
            s.dec(f.desc.ucode_id as u64);
            s.text(b", ");
            s.dec(f.desc.signature_count as u64);
            s.text(b" firmas (versiones 0x");
            s.hex(f.desc.signature_versions as u64, 4);
            s.text(b")\n");
            campo(s, b"dmemmap");
            let firma = f.dmemmapper_firma.to_le_bytes();
            s.text(if firma.iter().all(|c| c.is_ascii_graphic()) { &firma[..] } else { b"????" });
            s.text(b" en DMEM+0x");
            s.hex(f.dmemmapper as u64, 4);
            s.text(b"; la orden va en DMEM+0x");
            s.hex(f.orden_en as u64, 4);
            s.text(b" (");
            s.dec(f.orden_medida as u64);
            s.text(b" B), trae la 0x");
            s.hex(f.orden_de_fabrica as u64, 2);
            s.text(b"; FRTS es la 0x");
            s.hex(vb::ORDEN_FRTS as u64, 2);
            s.byte(b'\n');
            campo(s, b"fusible");
            let version = vb::version_del_fusible(r.fusible);
            s.text(b"0x");
            s.hex(r.fusible_reg as u64, 6);
            s.text(b" = 0x");
            s.hex(r.fusible as u64, 8);
            s.text(b" -> version ");
            s.dec(version as u64);
            match vb::indice_de_firma(f.desc.signature_versions, version) {
                Some(i) => {
                    s.with_ink(INK_GOOD);
                    s.text(b"; la firma buena es la ");
                    s.dec(i as u64);
                    s.text(b" de ");
                    s.dec(f.desc.signature_count as u64);
                }
                None => {
                    s.with_ink(INK_ERR);
                    s.text(b"; NINGUNA firma del descriptor es para este fusible: no se carga");
                }
            }
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }
    }
    fila_vram(s);
}

/// La VRAM, donde ira FRTS, el arranque del firmware y la WPR2 de ahora.
fn fila_vram(s: &mut Output) {
    let fb = bmo::info(bmo::INFO_GPU_FB);
    if fb & bmo::GPU_FB_VALIDA == 0 {
        return;
    }
    let mb = (fb & 0xFFFF_FFFF) as u32;
    let f = vb::frts(mb, bmo::info(bmo::INFO_GPU_VGA) as u32, fb & bmo::GPU_FB_SIN_PANTALLA == 0);
    campo(s, b"vram");
    s.dec(mb as u64);
    s.text(b" MiB; la VGA desde 0x");
    s.hex(f.vga, 9);
    s.text(b"; FRTS iria en 0x");
    s.hex(f.desde, 9);
    s.text(b"..0x");
    s.hex(f.hasta, 9);
    s.with_ink(INK_ECHO);
    if fb & bmo::GPU_FB_PLM_LEIBLE != 0 {
        let gfw = (fb >> bmo::GPU_FB_GFW_SHIFT) & 0xFF;
        s.text(if gfw == 0xFF { b"; el firmware de arranque ACABO" as &[u8] } else { b"; el firmware de arranque NO acabo: 0x" });
        if gfw != 0xFF {
            s.hex(gfw, 2);
        }
    } else {
        s.text(b"; su progreso no se deja leer");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    let w = bmo::info(bmo::INFO_GPU_WPR2);
    campo(s, b"wpr2");
    let (lo, hi) = (((w as u32 >> 4) as u64) << 12, (((w >> 32) as u32 >> 4) as u64) << 12);
    if hi == 0 {
        s.text(b"NO hay (lo que FWSEC-FRTS vendra a montar)\n");
    } else {
        s.with_ink(INK_GOOD);
        s.text(b"YA montada: 0x");
        s.hex(lo, 9);
        s.text(b"..0x");
        s.hex(hi, 9);
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
    super::datos::anotar(b"gpu vram mb", mb as u64, b"MiB");
}
