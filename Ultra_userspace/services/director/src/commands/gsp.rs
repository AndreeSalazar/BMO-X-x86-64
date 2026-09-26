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
    } else if lo == m.wpr2.desde {
        // Tras el booter: la WPR2 va desde donde dijo la WPR meta.
        s.with_ink(INK_GOOD);
        s.text(b"; el BOOTER extendio la WPR2 a 0x");
        s.hex(lo, 9);
        s.text(b", la wpr2 de este reparto: la WPR meta se leyo y cuadra");
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
    fila_radix(s);
    fila_libos(s);
    fila_despierto(s);
}

// == L0c2: EL GSP-RM PRESTADO POR SU RADIX3 (2026-09-24) =========================
//
// El kernel abre fw/gsp/ por su cuenta y lo copia a marcos NEUTRO en trozos de
// 512 KiB -- un syscall cada uno, y el escritorio pinta entre medias --; luego
// arma la radix3 y la WPR meta y lo presta; y al final lo relee TODO por la
// radix3 y la IOMMU, como lo recorrera el GSP, con su BLAKE3. Nada arranca.

pub(crate) const NO_RADIX_NO_CUADRA: u32 = 0x117;

/// **`gpu radix`, y el paso de `save mode`.** `Ok(paginas prestadas)`.
pub(crate) fn radix() -> Result<u64, u32> {
    let g = bmo::info(bmo::INFO_GPU_GSP);
    if g & bmo::GSP_PRESTADO == 0 {
        let n = bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_PREPARAR, 0)?;
        for k in 0..n {
            bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_TROZO, k)?;
            bmo::yield_screen();
        }
        bmo::iommu_orden(bmo::IOMMU_OP_GSP_PRESTAR)?;
    }
    let n = (bmo::info(bmo::INFO_GPU_GSP) >> bmo::GSP_TOTALES_SHIFT) & 0xFF;
    for k in 0..n {
        bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_COMPROBAR, k)?;
        bmo::yield_screen();
    }
    let g = bmo::info(bmo::INFO_GPU_GSP);
    if g & bmo::GSP_CUADRA == 0 || g & bmo::GSP_ES_570 == 0 {
        return Err(NO_RADIX_NO_CUADRA);
    }
    Ok((g >> bmo::GSP_PRESTADAS_SHIFT) & 0xFF_FFFF)
}

/// Lo pregunta `save mode`: prestado, y lo visto por la radix3 es la 570.144.
pub(crate) fn radix_hecho() -> bool {
    let g = bmo::info(bmo::INFO_GPU_GSP);
    g & bmo::GSP_PRESTADO != 0 && g & bmo::GSP_CUADRA != 0 && g & bmo::GSP_ES_570 != 0
}

/// `gpu radix`.
pub(crate) fn orden_radix(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if !super::files::antes_de_arriesgar(dsk, p, b"gpu radix") {
        dsk.field.n = 0;
        return After::Settle;
    }
    paint_status(p, &dsk.run_box, "prestando el GSP-RM a la 3060 por su radix3", INK_DIM);
    let r = radix();
    let g = &mut dsk.out.grid;
    match r {
        Ok(paginas) => {
            g.with_ink(INK_GOOD);
            g.text(b"  GSP-RM PRESTADO: ");
            g.dec(paginas);
            g.text(b" paginas, y por la radix3 la 3060 vera la 570.144 ENTERA\n");
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
    paint_status(p, &dsk.run_box, "radix", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **La fila de L0c2**, si se intento.
pub(crate) fn fila_radix(s: &mut Output) {
    let g = bmo::info(bmo::INFO_GPU_GSP);
    if g & bmo::GSP_VALIDO == 0 {
        return;
    }
    campo(s, b"radix");
    let (copiados, totales, comprobados) = (g & 0xFF, (g >> bmo::GSP_TOTALES_SHIFT) & 0xFF, (g >> bmo::GSP_COMPROBADOS_SHIFT) & 0xFF);
    let bien = g & bmo::GSP_PRESTADO != 0 && g & bmo::GSP_CUADRA != 0 && g & bmo::GSP_ES_570 != 0;
    s.with_ink(if bien { INK_GOOD } else if g & bmo::GSP_PRESTADO != 0 { INK_ERR } else { INK_ECHO });
    if g & bmo::GSP_PRESTADO == 0 {
        s.text(b"sin prestar: ");
        s.dec(copiados);
        s.text(b" de ");
        s.dec(totales);
        s.text(b" trozos copiados");
    } else {
        s.text(if bien {
            b"PRESTADO y la radix3 lleva al GSP-RM de la 570.144 entero" as &[u8]
        } else if comprobados < totales {
            b"PRESTADO, sin comprobar entero por la radix3"
        } else if g & bmo::GSP_CUADRA == 0 {
            b"PRESTADO, pero lo que se ve por la radix3 NO es lo copiado"
        } else {
            b"PRESTADO y cuadra, pero NO es la 570.144"
        });
        s.with_ink(INK_ECHO);
        s.text(b"   ");
        s.dec((g >> bmo::GSP_PRESTADAS_SHIFT) & 0xFF_FFFF);
        s.text(b" paginas; ");
        s.dec(comprobados);
        s.text(b" de ");
        s.dec(totales);
        s.text(b" trozos releidos; blake3 ");
        let h = bmo::info(bmo::INFO_GPU_GSP_HASH).to_le_bytes();
        for b in h {
            s.hex(b as u64, 2);
        }
    }
    let m = (g >> bmo::GSP_MOTIVO_SHIFT) & 0xFF;
    if m != 0 && !bien {
        s.with_ink(INK_ERR);
        s.text(b"; el ultimo NO: ");
        s.text(super::iommu::motivo(m as u32));
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu gsp radix", g, b"");
}

// == L0c3a: LO QUE EL GSP PODRA ESCRIBIR (2026-09-24) ============================
//
// Los argumentos de LIBOS, los tres logs, `rmargs`, las dos colas y la pagina
// de vaciado: la primera memoria del PC que la 3060 puede ESCRIBIR. Un syscall:
// son 180 paginas, no 61 MB. El kernel sigue cada puntero por la IOMMU.

/// **`gpu libos`, y el paso de `save mode`.** `Ok(punteros seguidos)`.
pub(crate) fn libos() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_GSP_LIBOS)
}

/// Lo pregunta `save mode`: prestado, y cada puntero lleva a lo suyo.
pub(crate) fn libos_hecho() -> bool {
    bmo::info(bmo::INFO_GPU_LIBOS) & bmo::LIBOS_COMPROBADO != 0
}

/// `gpu libos`.
pub(crate) fn orden_libos(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if !super::files::antes_de_arriesgar(dsk, p, b"gpu libos") {
        dsk.field.n = 0;
        return After::Settle;
    }
    paint_status(p, &dsk.run_box, "prestando a la 3060 lo que el GSP escribe", INK_DIM);
    let r = libos();
    let g = &mut dsk.out.grid;
    match r {
        Ok(n) => {
            g.with_ink(INK_GOOD);
            g.text(b"  LIBOS PRESTADO para escribir: ");
            g.dec(n);
            g.text(b" punteros seguidos por la IOMMU, y todos llevan a lo suyo\n");
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
    paint_status(p, &dsk.run_box, "libos", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **La fila de L0c3a**, si se intento.
pub(crate) fn fila_libos(s: &mut Output) {
    let l = bmo::info(bmo::INFO_GPU_LIBOS);
    if l & bmo::LIBOS_VALIDO == 0 {
        return;
    }
    campo(s, b"libos");
    let bien = l & bmo::LIBOS_COMPROBADO != 0;
    s.with_ink(if bien { INK_GOOD } else if l & bmo::LIBOS_PRESTADO != 0 { INK_ERR } else { INK_ECHO });
    s.text(if bien {
        b"PRESTADO para escribir, y cada puntero del GSP lleva a lo suyo" as &[u8]
    } else if l & bmo::LIBOS_PRESTADO != 0 {
        b"PRESTADO, pero un puntero NO lleva a donde debe"
    } else {
        b"sin prestar"
    });
    s.with_ink(INK_ECHO);
    s.text(b"   ");
    s.dec((l >> bmo::LIBOS_PRESTADAS_SHIFT) & 0xFFFF);
    s.text(b" paginas (argumentos, rmargs, vaciado, 3 logs, 2 colas); ");
    s.dec(l & 0xFFFF);
    s.text(b" punteros seguidos");
    let m = (l >> bmo::LIBOS_MOTIVO_SHIFT) & 0xFF;
    if m != 0 && !bien {
        s.with_ink(INK_ERR);
        s.text(b"; el ultimo NO: ");
        s.text(super::iommu::motivo(m as u32));
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu gsp libos", l, b"");
}

// == L0c3b: DESPERTAR EL GSP (2026-09-24) ========================================
//
// Tres ordenes del kernel y tres esperas aqui, cediendo el turno: el GSP se
// para tras su arranque vacio, el SEC2 se para con el booter hecho, y el
// RISC-V del GSP se enciende. Salga como salga, lo que el GSP escribio en sus
// logs se guarda en `datos/gsplog.bin`: si algo falla, ahi dice por que.

const RUTA_LOG: &[u8] = b"datos/gsplog.bin";
/// Los tres logs: 3 x 16 paginas.
const LOGS_BYTES: u64 = 3 * 16 * 4096;
/// Donde esta, dentro de `INFO_GPU_GSP_MEM`, el `writePtr` de la cola del GSP:
/// tras los logs, GspMem + la cola del GSP + 16.
const COLA_GSP_ESCRITO: u64 = LOGS_BYTES + 0x41000 + 16;

pub(crate) const NO_GSP_NO_PARA: u32 = 0x118;
pub(crate) const NO_SEC2_NO_ACABA: u32 = 0x119;
pub(crate) const NO_RISCV_DORMIDO: u32 = 0x11A;

/// Esperar, cediendo el turno, hasta `ms`, a que `INFO_GPU_DESPIERTO` cumpla.
fn esperar(ms: u64, cumple: impl Fn(u64) -> bool) -> bool {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let fin = bmo::ciclos() + hz / 1000 * ms;
    loop {
        if cumple(bmo::info(bmo::INFO_GPU_DESPIERTO)) {
            return true;
        }
        if bmo::ciclos() >= fin {
            return false;
        }
        bmo::yield_screen();
    }
}

fn mem(desde: u64) -> u64 {
    bmo::info(bmo::INFO_GPU_GSP_MEM | desde << 8)
}

/// Los tres logs del GSP, crudos, a `datos/gsplog.bin`. `true` si se guardo.
fn guardar_logs() -> bool {
    let Some(bloque) = bmo::Memoria::request(LOGS_BYTES) else { return false };
    // SAFETY: el bloque mide LOGS_BYTES, es de este proceso y se escribe aqui
    // antes de leerse.
    let b = unsafe { core::slice::from_raw_parts_mut(bloque.base(), LOGS_BYTES as usize) };
    for o in (0..LOGS_BYTES).step_by(8) {
        b[o as usize..o as usize + 8].copy_from_slice(&mem(o).to_le_bytes());
    }
    match bmo::Archivo::create(RUTA_LOG) {
        Ok(a) => a.escribir_de(&bloque, 0, LOGS_BYTES) == LOGS_BYTES && a.close(),
        Err(_) => false,
    }
}

fn despertar_sin_logs() -> Result<u64, u32> {
    if bmo::info(bmo::INFO_GPU_DESPIERTO) & bmo::DESPIERTO_VISTO != 0 {
        return Ok(1);
    }
    bmo::iommu_orden(bmo::IOMMU_OP_GSP_DESPERTAR)?;
    if !esperar(2000, |d| d & bmo::DESPIERTO_GSP_PARADO != 0) {
        return Err(NO_GSP_NO_PARA);
    }
    bmo::iommu_orden(bmo::IOMMU_OP_GSP_BOOTER)?;
    // nova-core da 2 s al booter; aqui 5, que sube 60 MB a la WPR2.
    if !esperar(5000, |d| d & bmo::DESPIERTO_SEC2_PARADO != 0) {
        return Err(NO_SEC2_NO_ACABA);
    }
    bmo::iommu_orden(bmo::IOMMU_OP_GSP_ACABAR)?;
    if !esperar(5000, |d| d & bmo::DESPIERTO_RISCV_ACTIVO != 0) {
        return Err(NO_RISCV_DORMIDO);
    }
    Ok(1)
}

/// **`gpu despertar`, y el paso de `save mode`.** Salga como salga, los logs.
pub(crate) fn despertar() -> Result<u64, u32> {
    let r = despertar_sin_logs();
    guardar_logs();
    r
}

/// **LA 3060 VIENE CALIENTE** (25-09): `despertar` se nego porque la WPR2
/// ya venia extendida (67), o el booter devolvio 0x15. Las dos caras de una
/// tarjeta que NO perdio la corriente desde que otro arranco su GSP: Windows
/// (reiniciar desde alli), o un reinicio de BMO-X sin `gpu apagar`.
pub(crate) fn caliente() -> bool {
    let d = bmo::info(bmo::INFO_GPU_DESPIERTO);
    if d & bmo::DESPIERTO_VISTO != 0 {
        return false;
    }
    let motivo = (d >> bmo::DESPIERTO_MOTIVO_SHIFT) & 0xFF;
    motivo == bmo::IOMMU_NO_GPU_CALIENTE as u64
        || d & bmo::DESPIERTO_SEC2_ARRANCADO != 0 && (d >> bmo::DESPIERTO_BUZON_SHIFT) & 0xFFFF_FFFF == 0x15
}

/// Lo que se dice cuando viene caliente: que es y que hacer.
pub(crate) const CALIENTE: &[u8] = b"la 3060 viene CALIENTE: no perdio la corriente desde que Windows (o un reinicio sin `gpu apagar`) arranco su GSP. Apaga 15 s sin corriente y vuelve";

/// Lo pregunta `save mode`: el RISC-V del GSP se vio activo.
pub(crate) fn despierto() -> bool {
    bmo::info(bmo::INFO_GPU_DESPIERTO) & bmo::DESPIERTO_VISTO != 0
}

/// `gpu despertar`.
pub(crate) fn orden_despertar(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if !super::files::antes_de_arriesgar(dsk, p, b"gpu despertar") {
        dsk.field.n = 0;
        return After::Settle;
    }
    paint_status(p, &dsk.run_box, "despertando el GSP: el booter en el SEC2", INK_DIM);
    let r = despertar();
    let g = &mut dsk.out.grid;
    match r {
        Ok(_) => {
            g.with_ink(INK_GOOD);
            g.text(b"  EL GSP DESPERTO: el RISC-V del GSP de tu 3060 esta ACTIVO; sus logs en datos/gsplog.bin\n");
        }
        Err(m) => {
            g.with_ink(INK_ERR);
            g.text(b"  NO: ");
            g.text(super::iommu::motivo(m));
            g.text(b" -- los logs del GSP, en datos/gsplog.bin\n");
        }
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "despertar", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn paso(s: &mut Output, hecho: bool, nombre: &[u8]) {
    s.with_ink(if hecho { INK_GOOD } else { INK_ECHO });
    s.text(if hecho { b" +" as &[u8] } else { b" -" });
    s.text(nombre);
}

/// **Las filas de L0c3b**, si se intento.
/// **La fila `autopsia`** (26-09): las fotos del kernel alrededor del booter,
/// SALGA BIEN O MAL -- un 0 tambien se apunta, porque la causa del 0x15 sale
/// de COMPARAR arranques buenos con malos. Todo va a `datos/`.
fn fila_autopsia(s: &mut Output) {
    let antes = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 4 << 8);
    let parado = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 5 << 8);
    if antes >> 63 == 0 {
        return;
    }
    let handoff = |v: u64| if v & 1 << 26 != 0 { b"PUESTO" as &[u8] } else { b"abajo" };
    campo(s, b"autopsia");
    s.with_ink(INK_ECHO);
    s.text(b"antes del booter: BSI 0x");
    s.hex(antes as u32 as u64, 8);
    s.text(b" handoff ");
    s.text(handoff(antes));
    s.text(b", GFW 0x");
    s.hex(antes >> 32 & 0xFF, 2);
    super::datos::anotar(b"gpu booter bsi antes", antes as u32 as u64, b"");
    if parado >> 63 != 0 {
        let gsp = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 6 << 8);
        let w = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 7 << 8);
        let us = parado >> 32 & 0x7FFF_FFFF;
        s.text(b"; al pararse (");
        s.dec(us);
        s.text(b" us): BSI 0x");
        s.hex(parado as u32 as u64, 8);
        s.text(b" handoff ");
        s.text(handoff(parado));
        s.text(b", GSP MAILBOX0 0x");
        s.hex(gsp as u32 as u64, 8);
        s.text(b" MAILBOX1 0x");
        s.hex(gsp >> 32, 8);
        s.text(b", WPR2 0x");
        s.hex(w as u32 as u64, 8);
        s.text(b"/0x");
        s.hex(w >> 32, 8);
        // CRUDOS: un 0xBADF.... es que el falcon no se dejaba leer.
        let c = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 8 << 8);
        s.text(b", CPUCTL GSP 0x");
        s.hex(c as u32 as u64, 8);
        s.text(b" SEC2 0x");
        s.hex(c >> 32, 8);
        super::datos::anotar(b"gpu booter cpuctl gsp", c as u32 as u64, b"");
        // HASTA DONDE LLEGO: lo que el booter escribio en la WPR meta.
        let mm = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 9 << 8);
        if mm >> 63 != 0 {
            let v = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 10 << 8);
            let n = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 11 << 8);
            // [!] NO es un reloj: en el arranque BUENO del 25-09 (sin
            // frontera) tambien salio 0. El booter trabaja con su copia en
            // la WPR de la VRAM, no con esta.
            s.text(b"; la WPR meta de la RAM (no discrimina: 0 tambien en los buenos): cambio ");
            s.dec((mm as u32).count_ones() as u64);
            s.text(b" palabras (mascara 0x");
            s.hex(mm & 0xFFFF_FFFF, 8);
            s.text(b"), verified 0x");
            s.hex(v, 16);
            s.text(b", bootCount ");
            s.dec(n);
            super::datos::anotar(b"gpu booter meta mascara", mm & 0xFFFF_FFFF, b"");
            super::datos::anotar(b"gpu booter meta verified", v, b"");
            super::datos::anotar(b"gpu booter meta bootcount", n, b"");
        }
        // EL BUS DE LA 3060 y la IOMMU, antes y al pararse: lo que dio el
        // metiche NUEVO, fue `gpu frontera` (antes) o el booter (entre medias).
        let (ba, bp) = (bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 12 << 8), bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 13 << 8));
        let ev = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 14 << 8);
        if ba >> 63 != 0 && bp >> 63 != 0 {
            s.text(b"; el bus de la 3060 antes:");
            super::metiche::bits_de_una(s, ba);
            if bp == ba {
                s.text(b", al pararse IGUAL");
            } else {
                s.text(b", al pararse CAMBIO a:");
                super::metiche::bits_de_una(s, bp);
            }
            s.text(b"; eventos de la IOMMU ");
            s.dec(ev & 0xFFFF_FFFF);
            s.text(b" -> ");
            s.dec(ev >> 32);
            s.text(if ev & 0xFFFF_FFFF == ev >> 32 { b" (el booter no choco con ella)" as &[u8] } else { b" (SUBIERON DURANTE EL BOOTER)" });
            super::datos::anotar(b"gpu booter bus antes", ba, b"");
            super::datos::anotar(b"gpu booter bus parado", bp, b"");
            super::datos::anotar(b"gpu booter iommu eventos", ev, b"");
        }
        super::datos::anotar(b"gpu booter bsi parado", parado as u32 as u64, b"");
        super::datos::anotar(b"gpu booter us", us, b"us");
        super::datos::anotar(b"gpu booter gsp mailbox0", gsp as u32 as u64, b"");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}

pub(crate) fn fila_despierto(s: &mut Output) {
    let d = bmo::info(bmo::INFO_GPU_DESPIERTO);
    if d & bmo::DESPIERTO_VALIDO == 0 {
        return;
    }
    campo(s, b"despierto");
    if d & bmo::DESPIERTO_RISCV_ACTIVO != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"el RISC-V del GSP esta ACTIVO: el GSP-RM de la 570.144 corre en tu 3060 ");
    } else if d & bmo::DESPIERTO_VISTO != 0 && super::gspvaciar::espera_secuenciador() {
        // ** Metal 24-09 08:14: salia en rojo "YA NO lo esta", y es lo que
        // toca: el GSP-RM se para solo tras pedir el secuenciador (L0c4b2).
        s.with_ink(INK_GOOD);
        s.text(b"el RISC-V se vio activo y se PARO SOLO, como debe: espera al secuenciador ");
    } else if d & bmo::DESPIERTO_VISTO != 0 {
        s.with_ink(INK_ERR);
        s.text(b"el RISC-V se vio activo y YA NO lo esta ");
    } else {
        s.with_ink(INK_ECHO);
        s.text(b"el RISC-V del GSP no esta activo ");
    }
    paso(s, d & bmo::DESPIERTO_VACIADO != 0, b"vaciado");
    paso(s, d & bmo::DESPIERTO_GSP_PARADO != 0, b"gsp");
    paso(s, d & bmo::DESPIERTO_BOOTER != 0, b"booter");
    paso(s, d & bmo::DESPIERTO_SEC2_PARADO != 0, b"sec2");
    paso(s, d & bmo::DESPIERTO_OS != 0, b"os");
    paso(s, d & bmo::DESPIERTO_RISCV_ACTIVO != 0, b"riscv");
    s.with_ink(INK_ECHO);
    s.text(b"   firma ");
    s.dec((d >> bmo::DESPIERTO_FIRMA_SHIFT) & 3);
    if d & bmo::DESPIERTO_SEC2_ARRANCADO != 0 {
        s.text(b", MAILBOX0 del SEC2 0x");
        s.hex(d >> bmo::DESPIERTO_BUZON_SHIFT, 8);
        // MAILBOX1: lo otro que devuelve el booter; nova-core lo imprime
        // tambien (metal 24-09 07:48: MAILBOX0 0x15, y sin su pareja).
        let m1 = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 1 << 8) >> 32;
        s.text(b", MAILBOX1 0x");
        s.hex(m1, 8);
        super::datos::anotar(b"gpu sec2 mailbox1", m1, b"");
        // ** 0x15: nueve veces en el metal (24-09 y 25-09), seis seguidas EN
        // FRIO: cortar la corriente NO lo arregla. La causa (25-09, 3 de 3):
        // `fuego`/`frontera` usaban el falcon del GSP antes que el booter; ya
        // no son pasos de `save mode`. El caso: `ga10x/EL_0x15.md`.
        if d >> bmo::DESPIERTO_BUZON_SHIFT & 0xFFFF_FFFF == 0x15 {
            s.with_ink(INK_ERR);
            let con_prueba = bmo::info(bmo::INFO_GPU_FRONTERA) & bmo::FUEGO_INTENTADO != 0 || bmo::info(bmo::INFO_GPU_FUEGO) & bmo::FUEGO_INTENTADO != 0;
            s.text(if con_prueba {
                b" = el booter no cargo, y `gpu fuego`/`gpu frontera` corrieron ANTES en el falcon del GSP (la causa conocida): arranca otra vez sin darlas" as &[u8]
            } else {
                b" = el booter no cargo SIN `fuego` ni `frontera`: un 0x15 NUEVO, no el conocido; pega la fila `autopsia` (EL_0x15.md)"
            });
            s.with_ink(INK_ECHO);
        }
    }
    if d & bmo::DESPIERTO_RISCV_PARADO != 0 && !super::gspvaciar::espera_secuenciador() {
        s.with_ink(INK_ERR);
        s.text(b"; el RISC-V dice PARADO");
    }
    let m = (d >> bmo::DESPIERTO_MOTIVO_SHIFT) & 0xFF;
    if m != 0 && d & bmo::DESPIERTO_VISTO == 0 {
        s.with_ink(INK_ERR);
        s.text(b"; el ultimo NO: ");
        s.text(super::iommu::motivo(m as u32));
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    fila_autopsia(s);
    // Lo que el GSP escribio: los punteros de sus logs y de su cola.
    campo(s, b"gsplog");
    let (ini, intr, rm) = (mem(0), mem(16 * 4096), mem(32 * 4096));
    let cola = mem(COLA_GSP_ESCRITO) & 0xFFFF_FFFF;
    let b = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON);
    let escribio = ini | intr | rm | cola != 0;
    s.with_ink(if escribio { INK_GOOD } else { INK_ECHO });
    s.text(if escribio { b"el GSP ESCRIBIO en tu RAM: " as &[u8] } else { b"el GSP no ha escrito nada: " });
    s.with_ink(INK_PLAIN);
    s.text(b"LOGINIT ");
    s.dec(ini);
    s.text(b", LOGINTR ");
    s.dec(intr);
    s.text(b", LOGRM ");
    s.dec(rm);
    s.text(b"; su cola ");
    s.dec(cola);
    s.text(b" mensajes; MAILBOX0 del GSP 0x");
    s.hex(b & 0xFFFF_FFFF, 8);
    s.with_ink(INK_ECHO);
    s.text(b"   -> datos/gsplog.bin");
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu gsp despierto", d, b"");
}
