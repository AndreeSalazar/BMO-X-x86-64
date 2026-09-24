//! **`gpu gsp`: L0c1, EL FIRMWARE DEL GSP, PREGUNTADO.** Lee de `fw/gsp/` los
//! cuatro binarios que la VBIOS no trae, los entiende, pregunta que firma del
//! booter pide el fusible del SEC2, y reparte la VRAM como nova-core.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: ~130 KiB leidos
//!                     enteros y unos pocos KiB de los 63 MB del GSP-RM
//!
//! # Lo que NO hace
//!
//! No presta nada a la 3060 ni escribe un registro: eso es L0c2 (prestar el
//! GSP-RM por la radix3) y L0c3 (el booter en el SEC2). Esto dice si se PUEDE,
//! y cuanto habra que prestar.
//!
//! # Por que el GSP-RM se lee a saltos (2026-09-24)
//!
//! Son 63 MB, y abrir en BMO-X por el camino de siempre se trae el fichero
//! ENTERO a RAM contigua. `Archivo::reflejar` lo abre por una ventana de 64 KiB
//! y `bmo_gpu_ga10x::elf` pide solo la cabecera y la tabla de secciones.

use bmo_gpu_ga10x::booter::{self as bt, Booter, NoBooter, Riscv};
use bmo_gpu_ga10x::elf::{self, Fuente, NoElf, Seccion};
use bmo_gpu_ga10x::vbios as vb;
use bmo_gpu_ga10x::wpr::{self, Mapa, Radix3};
use bmo_userland as bmo;

use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Donde los deja el build (`build\firmware.ps1`). 8.3.
const CARGADOR: &[u8] = b"fw/gsp/boot_ld.bin";
const DESCARGADOR: &[u8] = b"fw/gsp/boot_ul.bin";
const BOOTLOADER: &[u8] = b"fw/gsp/bootldr.bin";
const GSP_RM: &[u8] = b"fw/gsp/gsp.bin";
/// Mas que esto no es un booter ni un bootloader (miden 24..61 KiB).
const CHICO_MAX: u64 = 256 * 1024;

/// Por que no, con el fichero delante.
#[derive(Clone, Copy)]
enum Falta {
    /// No se pudo abrir: el motivo del kernel.
    NoEsta(u32),
    /// Se abrio y no se entendio.
    Booter(NoBooter),
    Elf(NoElf),
    /// Mide mas de lo que un binario chico de NVIDIA mide.
    Grande(u64),
    /// No hubo memoria para traerlo.
    SinMemoria,
}

#[derive(Clone, Copy)]
struct Resumen {
    cargador: Result<(Booter, u64), Falta>,
    descargador: Result<(Booter, u64), Falta>,
    bootloader: Result<(Riscv, u64), Falta>,
    imagen: Result<Seccion, Falta>,
    firma: Result<Seccion, Falta>,
    /// Los bytes que se leyeron del GSP-RM para encontrar las dos secciones.
    leidos: u64,
    fusible_reg: u32,
    fusible: u32,
}

static mut RESUMEN: Option<Resumen> = None;

fn resumen() -> Option<Resumen> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(RESUMEN) }
}

/// Un binario chico, entero, y lo que dice de si mismo.
fn chico<T>(ruta: &[u8], entender: fn(&[u8]) -> Result<T, NoBooter>) -> Result<(T, u64), Falta> {
    let a = bmo::Archivo::leer_de(ruta).map_err(Falta::NoEsta)?;
    let n = a.size();
    if n > CHICO_MAX {
        return Err(Falta::Grande(n));
    }
    let bloque = bmo::Memoria::request(n.max(1)).ok_or(Falta::SinMemoria)?;
    let leidos = a.leer_en(&bloque, 0, n);
    // SAFETY: el bloque mide al menos `n` bytes, es de este proceso y
    // `leer_en` acaba de escribir `leidos` de ellos.
    let f = unsafe { core::slice::from_raw_parts(bloque.base(), leidos as usize) };
    entender(f).map(|t| (t, n)).map_err(Falta::Booter)
}

/// El GSP-RM a saltos: la [`Fuente`] de `bmo_gpu_ga10x::elf`, sobre un
/// archivo reflejado. Cuenta lo que lee, para decirlo.
struct Reflejo(bmo::Archivo, u64);

impl Fuente for Reflejo {
    fn leer(&mut self, desde: u64, dst: &mut [u8]) -> bool {
        self.1 += dst.len() as u64;
        self.0.saltar(desde) == desde && self.0.read(dst) == dst.len()
    }
}

/// **Leer y entender los cuatro.** `Ok(paginas a prestar)` si esta todo.
pub(crate) fn leer() -> Result<u64, u32> {
    let cargador = chico(CARGADOR, bt::booter);
    let descargador = chico(DESCARGADOR, bt::booter);
    let bootloader = chico(BOOTLOADER, bt::riscv);
    let (imagen, firma, leidos) = match bmo::Archivo::reflejar(GSP_RM) {
        Err(m) => (Err(Falta::NoEsta(m)), Err(Falta::NoEsta(m)), 0),
        Ok(a) => {
            let mut r = Reflejo(a, 0);
            let i = elf::seccion(&mut r, elf::IMAGEN).map_err(Falta::Elf);
            let f = elf::seccion(&mut r, elf::FIRMA_GA10X).map_err(Falta::Elf);
            (i, f, r.1)
        }
    };
    let (mut fusible_reg, mut fusible) = (0, 0);
    if let Ok((b, _)) = cargador {
        if let Some(reg) = vb::registro_fusible(b.engine_id_mask, b.ucode_id) {
            fusible_reg = reg;
            fusible = bmo::info(bmo::INFO_GPU_FUSIBLE | (reg as u64) << 8) as u32;
        }
    }
    let r = Resumen { cargador, descargador, bootloader, imagen, firma, leidos, fusible_reg, fusible };
    // SAFETY: como `resumen`.
    unsafe {
        *core::ptr::addr_of_mut!(RESUMEN) = Some(r);
    }
    listo(&r).ok_or(NO_GSP_INCOMPLETO)
}

/// El reparto de la VRAM con lo que dijeron el bootloader y el GSP-RM.
fn mapa(r: &Resumen) -> Option<Mapa> {
    let fb = bmo::info(bmo::INFO_GPU_FB);
    if fb & bmo::GPU_FB_VALIDA == 0 {
        return None;
    }
    let f = vb::frts((fb & 0xFFFF_FFFF) as u32, bmo::info(bmo::INFO_GPU_VGA) as u32, fb & bmo::GPU_FB_SIN_PANTALLA == 0);
    let (bl, _) = r.bootloader.ok()?;
    wpr::mapa(&f, bl.bin.bytes as u64, r.imagen.ok()?.bytes)
}

/// **Esta todo para L0c2?** Los cuatro entendidos, una firma del booter para
/// ESTE fusible, y una VRAM que se deja repartir. `Some(paginas a prestar)`.
fn listo(r: &Resumen) -> Option<u64> {
    let (b, _) = r.cargador.ok()?;
    r.descargador.ok()?;
    b.indice_de_firma(r.fusible)?;
    r.firma.ok()?;
    mapa(r)?;
    let (bl, _) = r.bootloader.ok()?;
    let paginas = Radix3::de(r.imagen.ok()?.bytes).paginas() + (bl.bin.bytes as u64 + 4095) / 4096 + 1;
    Some(paginas)
}

/// Lo pregunta `save mode`.
pub(crate) fn hecho() -> bool {
    resumen().map_or(false, |r| listo(&r).is_some())
}

/// Motivo del escritorio (fuera del rango del kernel; `vbios.rs` va hasta 0x115).
pub(crate) const NO_GSP_INCOMPLETO: u32 = 0x116;

/// `gpu gsp`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "leyendo el firmware del GSP de fw/gsp/", INK_DIM);
    let r = leer();
    let g = &mut dsk.out.grid;
    match r {
        Ok(paginas) => {
            g.with_ink(INK_GOOD);
            g.text(b"  FIRMWARE DEL GSP entendido: habra que prestar ");
            g.dec(paginas);
            g.text(b" paginas (");
            g.dec(paginas * 4096 >> 20);
            g.text(b" MiB) a la 3060\n");
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
    paint_status(p, &dsk.run_box, "gsp", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn falta(s: &mut Output, ruta: &[u8], f: Falta) {
    s.with_ink(INK_ERR);
    s.text(ruta);
    match f {
        Falta::NoEsta(m) => {
            s.text(b": no se pudo abrir (motivo ");
            s.dec(m as u64);
            s.text(b"; el build lo baja: `build\\firmware.ps1`)");
        }
        Falta::SinMemoria => s.text(b": no hubo memoria para traerlo"),
        Falta::Grande(n) => {
            s.text(b": mide ");
            s.dec(n);
            s.text(b" B, demasiado para ser el que se espera");
        }
        Falta::Booter(e) => s.text(match e {
            NoBooter::Corto => b": mas corto que su cabecera" as &[u8],
            NoBooter::SinMagia => b": no empieza por 0x10DE, no es de NVIDIA",
            NoBooter::Fuera => b": su cabecera promete algo fuera del fichero",
            NoBooter::FirmaRara => b": los parametros de su firma no cuadran",
            NoBooter::SinFirma => b": sin firma",
        }),
        Falta::Elf(e) => s.text(match e {
            NoElf::Ilegible => b": no se pudo leer su cabecera" as &[u8],
            NoElf::NoEsElf64 => b": no es un ELF64",
            NoElf::TablaRara => b": su tabla de secciones no es de un ELF64",
            NoElf::NoEsta => b": no trae la seccion que hace falta (.fwimage o la firma ga10x)",
        }),
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}

fn rango(s: &mut Output, r: wpr::Rango) {
    s.text(b"0x");
    s.hex(r.desde, 9);
    s.text(b"..0x");
    s.hex(r.hasta, 9);
}

/// **Las filas de L0c1.** Salen en `gpu` y en el `save`, si se leyo.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = resumen() else { return };
    campo(s, b"booter");
    match r.cargador {
        Err(f) => falta(s, CARGADOR, f),
        Ok((b, n)) => {
            s.text(b"boot_ld ");
            s.dec(n);
            s.text(b" B: motor 0x");
            s.hex(b.engine_id_mask as u64, 4);
            s.text(b" ucode ");
            s.dec(b.ucode_id as u64);
            s.text(b", ");
            s.dec(b.firmas as u64);
            s.text(b" firmas de ");
            s.dec(b.firma_bytes as u64);
            s.text(b" B; IMEM 0x");
            s.hex(b.imem.bytes as u64, 4);
            s.text(b" B desde +0x");
            s.hex(b.imem.origen as u64, 4);
            s.text(b", DMEM 0x");
            s.hex(b.dmem.bytes as u64, 4);
            s.text(b" B, la firma en DMEM+0x");
            s.hex(b.pkc_data_offset() as u64, 3);
            s.byte(b'\n');
            campo(s, b"fusible");
            s.text(b"0x");
            s.hex(r.fusible_reg as u64, 6);
            s.text(b" = 0x");
            s.hex(r.fusible as u64, 8);
            s.text(b" -> version ");
            s.dec(vb::version_del_fusible(r.fusible) as u64);
            s.text(b" (el fichero dice ");
            s.dec(b.fuse_ver as u64);
            s.byte(b')');
            match b.indice_de_firma(r.fusible) {
                Some(i) => {
                    s.with_ink(INK_GOOD);
                    s.text(b"; la firma buena del booter es la ");
                    s.dec(i as u64);
                    s.text(b" de ");
                    s.dec(b.firmas as u64);
                }
                None => {
                    s.with_ink(INK_ERR);
                    s.text(b"; NINGUNA firma del booter es para este fusible: el SEC2 la rechazaria");
                }
            }
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }
    }
    campo(s, b"unload");
    match r.descargador {
        Err(f) => falta(s, DESCARGADOR, f),
        Ok((b, n)) => {
            s.text(b"boot_ul ");
            s.dec(n);
            s.text(b" B: ");
            s.dec(b.firmas as u64);
            s.text(b" firmas; el camino de vuelta, para apagar el GSP sin reiniciar\n");
        }
    }
    campo(s, b"bootldr");
    match r.bootloader {
        Err(f) => falta(s, BOOTLOADER, f),
        Ok((bl, n)) => {
            s.text(b"bootldr ");
            s.dec(n);
            s.text(b" B (RISC-V v");
            s.dec(bl.version as u64);
            s.text(b"): ucode 0x");
            s.hex(bl.bin.bytes as u64, 4);
            s.text(b" B; codigo +0x");
            s.hex(bl.codigo as u64, 4);
            s.text(b", datos +0x");
            s.hex(bl.datos as u64, 4);
            s.text(b", manifiesto +0x");
            s.hex(bl.manifiesto as u64, 4);
            s.byte(b'\n');
        }
    }
    campo(s, b"gsp-rm");
    match (r.imagen, r.firma) {
        (Err(f), _) | (_, Err(f)) => falta(s, GSP_RM, f),
        (Ok(i), Ok(f)) => {
            let rx = Radix3::de(i.bytes);
            s.text(b".fwimage ");
            s.dec(i.bytes >> 20);
            s.text(b" MiB (0x");
            s.hex(i.bytes, 7);
            s.text(b" B) = ");
            s.dec(rx.imagen);
            s.text(b" paginas + ");
            s.dec(rx.nivel2 + rx.nivel1 + 1);
            s.text(b" de radix3; firma ga10x ");
            s.dec(f.bytes);
            s.text(b" B");
            s.with_ink(INK_ECHO);
            s.text(b"   (leidos ");
            s.dec(r.leidos);
            s.text(b" B de el)");
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }
    }
    let Some(m) = mapa(&r) else { return };
    campo(s, b"mapa");
    s.text(b"wpr2 ");
    rango(s, m.wpr2);
    s.text(b" (");
    s.dec(m.wpr2.bytes() >> 20);
    s.text(b" MiB): heap ");
    s.dec(m.heap.bytes() >> 20);
    s.text(b" MiB desde 0x");
    s.hex(m.heap.desde, 9);
    s.text(b", elf desde 0x");
    s.hex(m.elf.desde, 9);
    s.text(b", boot desde 0x");
    s.hex(m.boot.desde, 9);
    s.byte(b'\n');
    // Lo que FWSEC monto (L0b) tiene que ser el frts de este reparto.
    let w = bmo::info(bmo::INFO_GPU_WPR2);
    let lo = ((w as u32 >> 4) as u64) << 12;
    campo(s, b"cuadra");
    s.text(b"frts ");
    rango(s, m.frts);
    if (w >> 32) as u32 >> 4 == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"; la WPR2 todavia NO esta montada (`gpu fwsec`)");
    } else if lo == m.frts.desde {
        s.with_ink(INK_GOOD);
        s.text(b"; es donde FWSEC monto la WPR2: el reparto cuadra");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"; la WPR2 de FWSEC empieza en OTRO sitio: 0x");
        s.hex(lo, 9);
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    if let Some(p) = listo(&r) {
        super::datos::anotar(b"gpu gsp paginas", p, b"");
    }
}
