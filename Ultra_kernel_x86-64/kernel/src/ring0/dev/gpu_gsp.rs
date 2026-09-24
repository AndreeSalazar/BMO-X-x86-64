//! **EL GSP-RM, PRESTADO A LA 3060 (L0c2)** -- sus 61 MB copiados del disco a
//! marcos NEUTRO, su tabla radix3, el bootloader, la firma y la WPR meta,
//! prestados por la IOMMU; y COMPROBADO recorriendo la radix3 como la recorrera
//! el GSP, por la misma traduccion.
//!
//! [carril]  ROJO      pide ~61 MiB de marcos NEUTRO y los presta a un aparato
//! [consumo] NADA      corre por orden (`gpu radix`, y el paso `radix`)
//!
//! [eje]     CORRECCION -- una pagina fuera de sitio aqui es un GSP-RM que el
//!           bootloader rechaza por firma, sin decir cual
//!
//! # Lo que ve la 3060 (IOVAs) al acabar
//!
//! ```text
//!    0x1000_0000   la pagina de prueba (M0d)            ya estaba
//!    0x1100_0000   el ucode de FWSEC (L0b)              ya estaba
//!    0x2000_0000   NADA: la frontera (M0d3)             se queda sin prestar
//!    0x3E00_0000   el bootloader (6 pag.), la firma ga10x (1) y la WPR meta
//!                  (1, la unica que puede ESCRIBIR)
//!    0x3F00_0000   la radix3: nivel 0, nivel 1, 31 paginas de nivel 2
//!    0x4000_0000   el .fwimage: 15.513 paginas, en bloques de 2 MiB
//! ```
//!
//! # Cuatro ordenes, como FWSEC, y por lo mismo
//!
//! ```text
//!    PREPARAR     el kernel abre fw/gsp/ POR SU CUENTA (no se fia de lo que
//!                 leyo el escritorio), encuentra las secciones y pide marcos
//!    TROZO(k)     512 KiB del .fwimage, del disco a sus marcos, y su BLAKE3
//!    PRESTAR      la radix3, la WPR meta (`bmo_gpu_ga10x::wpr`) y el prestamo
//!    COMPROBAR(k) 512 KiB leidos POR LA RADIX3 y la IOMMU, y su BLAKE3
//! ```
//!
//! Cada trozo es un syscall: 122 de unos milisegundos, y entre medias el
//! escritorio pinta. Uno de 61 MB serian segundos con la maquina sorda.
//!
//! # Como se sabe
//!
//! El BLAKE3 de lo que se ve POR LA RADIX3 tiene que ser el de lo que se
//! copio, y el de lo que se copio el del `.fwimage` de la 570.144, calculado
//! en el anfitrion con el mismo `bmo-hash` ([`HASH_570`]). Si cuadran los tres,
//! el GSP vera su firmware entero, en orden, y solo para leer.
//!
//! # Lo que NO hace
//!
//! No arranca nada: ni el SEC2 ni el GSP. Eso es L0c3.

use core::sync::atomic::{AtomicU64, Ordering};

use bmo_gpu_ga10x::{booter, elf, wpr};

use crate::ring0::fsys::fs;
use crate::ring0::mm::phys;
use crate::ring0::plat::iommu as io;

pub const IOVA_GSP_AUX: u64 = 0x3E00_0000;
pub const IOVA_GSP_RADIX: u64 = 0x3F00_0000;
pub const IOVA_GSP_IMAGEN: u64 = 0x4000_0000;

const RUTA_GSP: &str = "fw/gsp/gsp.bin";
const RUTA_BOOTLOADER: &str = "fw/gsp/bootldr.bin";

const PAGINA: u64 = 4096;
/// Las paginas AUX: 0..6 el bootloader, 6 la firma, 7 la WPR meta; 8..16 la
/// ventana por la que el kernel lee el ELF y el bootloader, que NO se presta.
const AUX_PAGINAS: u64 = 16;
const AUX_BOOTLOADER_MAX: u64 = 6;
const AUX_FIRMA: u64 = 6;
const AUX_META: u64 = 7;
const AUX_VENTANA: u64 = 8;
/// El .fwimage va en bloques de 2 MiB contiguos: 61 MB contiguos se pueden
/// negar en una RAM que lleva un rato encendida, y la radix3 no los necesita.
const BLOQUE_PAGINAS: u64 = 512;
const MAX_BLOQUES: usize = 40;
/// La radix3 de 80 MiB de imagen cabe en 1 + 1 + 40.
const RADIX_MAX: u64 = 48;
/// Lo que mueve cada TROZO y cada COMPROBAR: 128 paginas.
pub const TROZO: u64 = 512 * 1024;

/// El BLAKE3 del `.fwimage` de `gsp-570.144.bin` (0x3C99000 B desde 0x40),
/// calculado en el anfitrion con `bmo-hash` el 24-09.
pub const HASH_570: [u8; 32] = [
    0xe7, 0x85, 0x6e, 0xe2, 0xb3, 0x87, 0x91, 0x7b, 0xfc, 0x11, 0x2d, 0x6b, 0x70, 0x5c, 0x1e, 0x7e,
    0xde, 0xfd, 0x32, 0xe5, 0x87, 0xeb, 0x58, 0x9e, 0x17, 0x81, 0xe9, 0x60, 0x93, 0x72, 0xee, 0xa0,
];

/// No esta `fw/gsp/gsp.bin` o `bootldr.bin` en el volumen de datos.
pub const IOMMU_NO_GSP_FICHERO: u32 = 26;
/// Estan, pero no son lo que se espera (el ELF, sus secciones, el bootloader).
pub const IOMMU_NO_GSP_FORMATO: u32 = 27;
/// No hubo marcos para la imagen, la radix3 o lo auxiliar.
pub const IOMMU_NO_GSP_MARCOS: u32 = 28;
/// Fuera de orden: sin PREPARAR, un trozo saltado, o prestar sin copiar todo.
pub const IOMMU_NO_GSP_ORDEN: u32 = 29;
/// El disco devolvio menos de lo pedido.
pub const IOMMU_NO_GSP_DISCO: u32 = 30;
/// Ya esta prestado: no se escribe encima de lo que la 3060 puede leer.
pub const IOMMU_NO_GSP_YA_PRESTADO: u32 = 31;
/// La VRAM no se deja repartir (`wpr::mapa`).
pub const IOMMU_NO_GSP_SIN_VRAM: u32 = 32;
/// La radix3 no lleva, por la IOMMU, a donde tiene que llevar.
pub const IOMMU_NO_GSP_RADIX: u32 = 33;

pub const GSP_TOTALES_SHIFT: u64 = 8;
pub const GSP_COMPROBADOS_SHIFT: u64 = 16;
pub const GSP_MOTIVO_SHIFT: u64 = 24;
pub const GSP_PRESTADAS_SHIFT: u64 = 32;
pub const GSP_PREPARADO: u64 = 1 << 56;
pub const GSP_COPIADO: u64 = 1 << 57;
pub const GSP_PRESTADO: u64 = 1 << 58;
pub const GSP_CUADRA: u64 = 1 << 59;
pub const GSP_ES_570: u64 = 1 << 60;
pub const GSP_VALIDO: u64 = 1 << 63;

/// Banderas, motivo y paginas prestadas (ver `info_gsp`).
static ESTADO: AtomicU64 = AtomicU64::new(0);
static COPIADOS: AtomicU64 = AtomicU64::new(0);
static COMPROBADOS: AtomicU64 = AtomicU64::new(0);
/// Las fisicas: lo auxiliar, la radix3 y cada bloque de la imagen.
static AUX: AtomicU64 = AtomicU64::new(0);
static RADIX: AtomicU64 = AtomicU64::new(0);
static BLOQUES: [AtomicU64; MAX_BLOQUES] = [const { AtomicU64::new(0) }; MAX_BLOQUES];
/// Lo ya prestado: un bit por bloque; 40 la radix3, 41 lo auxiliar de solo
/// lectura, 42 la WPR meta. Un prestamo a medias se retoma sin repetir.
static PRESTADO: AtomicU64 = AtomicU64::new(0);
const PRESTADO_RADIX: u64 = 1 << 40;
const PRESTADO_AUX: u64 = 1 << 41;
const PRESTADO_META: u64 = 1 << 42;

/// Lo que PREPARAR entendio. Solo llega aqui el escritorio (quien tiene la
/// pantalla), de una orden en una: un solo escritor.
#[derive(Clone, Copy)]
struct Plan {
    imagen: elf::Seccion,
    firma: elf::Seccion,
    bootloader: booter::Riscv,
    medida: u32,
    inicio: bmo_fat32::Cursor,
}

static mut PLAN: Option<Plan> = None;
/// Por donde va la copia en `gsp.bin` (el cursor de FAT32 solo avanza).
static mut CURSOR: Option<bmo_fat32::Cursor> = None;
static mut HASH_COPIA: Option<bmo_hash::Hasher> = None;
static mut HASH_RADIX: Option<bmo_hash::Hasher> = None;
static mut DIGESTO_COPIA: [u8; 32] = [0; 32];
static mut DIGESTO_RADIX: [u8; 32] = [0; 32];

fn plan() -> Option<Plan> {
    // SAFETY: ver `PLAN`.
    unsafe { *core::ptr::addr_of!(PLAN) }
}

fn apuntar(f: impl FnOnce(u64) -> u64) {
    let v = ESTADO.load(Ordering::Acquire);
    ESTADO.store(f(v), Ordering::Release);
}

fn no(motivo: u32) -> Result<u64, u32> {
    apuntar(|v| (v & !(0xFF << GSP_MOTIVO_SHIFT)) | GSP_VALIDO | (motivo as u64 & 0xFF) << GSP_MOTIVO_SHIFT);
    crate::ring0::cabina::warn("gpu", "L0c2: el GSP-RM no se presto; motivo", motivo as u64);
    Err(motivo)
}

/// `bytes` de memoria fisica por el physmap.
fn memoria(fisica: u64, bytes: u64) -> &'static mut [u8] {
    // SAFETY: solo se llama con marcos NEUTRO de este fichero (AUX, RADIX y
    // BLOQUES), del medida con que se pidieron.
    unsafe { core::slice::from_raw_parts_mut(crate::ring0::mm::phys_to_virt(fisica) as *mut u8, bytes as usize) }
}

fn paginas_de(bytes: u64) -> u64 {
    bytes.div_ceil(PAGINA)
}

fn trozos(p: &Plan) -> u64 {
    p.imagen.bytes.div_ceil(TROZO)
}

/// La fisica de la pagina `p` de la imagen.
fn fisica_de(p: u64) -> u64 {
    BLOQUES[(p / BLOQUE_PAGINAS) as usize].load(Ordering::Acquire) + (p % BLOQUE_PAGINAS) * PAGINA
}

/// **El ELF leido por una ventana de 32 KiB** (`bmo_gpu_ga10x::elf::Fuente`).
/// El cursor de FAT32 solo avanza: si se pide algo de atras, se vuelve al
/// principio, como `obj/file.rs::reflejar`. Con la ventana son dos vueltas
/// para todo el ELF, no una por seccion.
struct Ventana<'a> {
    inicio: bmo_fat32::Cursor,
    cur: bmo_fat32::Cursor,
    medida: u32,
    buf: &'a mut [u8],
    desde: u64,
    largo: usize,
}

impl Ventana<'_> {
    /// `dst` directamente desde el disco, sin pasar por la ventana.
    fn rango(&mut self, desde: u64, dst: &mut [u8]) -> bool {
        if (desde as usize) < self.cur.base() {
            self.cur = self.inicio;
        }
        fs::leer_rango(&mut self.cur, desde as usize, self.medida, dst) == dst.len()
    }
}

impl elf::Fuente for Ventana<'_> {
    fn leer(&mut self, desde: u64, dst: &mut [u8]) -> bool {
        if dst.len() > self.buf.len() {
            return false;
        }
        let fin = desde + dst.len() as u64;
        if desde < self.desde || fin > self.desde + self.largo as u64 {
            if (desde as usize) < self.cur.base() {
                self.cur = self.inicio;
            }
            let n = fs::leer_rango(&mut self.cur, desde as usize, self.medida, self.buf);
            self.desde = desde;
            self.largo = n;
            if fin > desde + n as u64 {
                return false;
            }
        }
        let o = (desde - self.desde) as usize;
        dst.copy_from_slice(&self.buf[o..o + dst.len()]);
        true
    }
}

/// **PREPARAR.** `Ok(trozos de 512 KiB a copiar)`.
pub fn preparar() -> Result<u64, u32> {
    if ESTADO.load(Ordering::Acquire) & GSP_PRESTADO != 0 {
        // Ya prestado: nada que repetir, y lo que la 3060 lee no se toca.
        return Ok(plan().map_or(0, |p| trozos(&p)));
    }
    if AUX.load(Ordering::Acquire) == 0 {
        // La 3060 lo leera por DMA: NEUTRO.
        let Some(f) = phys::alloc_frames_contig_de(AUX_PAGINAS, phys::Titular::Neutro) else {
            return no(IOMMU_NO_GSP_MARCOS);
        };
        AUX.store(f, Ordering::Release);
    }
    let aux = memoria(AUX.load(Ordering::Acquire), AUX_PAGINAS * PAGINA);
    aux.fill(0);
    let (prestable, ventana) = aux.split_at_mut((AUX_VENTANA * PAGINA) as usize);

    // El bootloader, entero por la ventana; su ucode, a las paginas 0..6.
    let Ok((mut cbl, mbl)) = fs::abrir_rangos(RUTA_BOOTLOADER) else { return no(IOMMU_NO_GSP_FICHERO) };
    if mbl as usize > ventana.len() {
        return no(IOMMU_NO_GSP_FORMATO);
    }
    if fs::leer_rango(&mut cbl, 0, mbl, &mut ventana[..mbl as usize]) != mbl as usize {
        return no(IOMMU_NO_GSP_DISCO);
    }
    let Ok(bl) = booter::riscv(&ventana[..mbl as usize]) else { return no(IOMMU_NO_GSP_FORMATO) };
    if bl.bin.bytes as u64 > AUX_BOOTLOADER_MAX * PAGINA {
        return no(IOMMU_NO_GSP_FORMATO);
    }
    prestable[..bl.bin.bytes as usize].copy_from_slice(bl.bin.ucode(&ventana[..mbl as usize]));

    // El GSP-RM: sus dos secciones, y la firma a la pagina 6.
    let Ok((cg, mg)) = fs::abrir_rangos(RUTA_GSP) else { return no(IOMMU_NO_GSP_FICHERO) };
    let mut v = Ventana { inicio: cg, cur: cg, medida: mg, buf: ventana, desde: 0, largo: 0 };
    let (Ok(imagen), Ok(firma)) = (elf::seccion(&mut v, elf::IMAGEN), elf::seccion(&mut v, elf::FIRMA_GA10X)) else {
        return no(IOMMU_NO_GSP_FORMATO);
    };
    let dentro = |s: elf::Seccion| s.bytes > 0 && s.desde.checked_add(s.bytes).map_or(false, |f| f <= mg as u64);
    if !dentro(imagen) || !dentro(firma) || firma.bytes > PAGINA || paginas_de(imagen.bytes) > MAX_BLOQUES as u64 * BLOQUE_PAGINAS {
        return no(IOMMU_NO_GSP_FORMATO);
    }
    let f0 = (AUX_FIRMA * PAGINA) as usize;
    if !v.rango(firma.desde, &mut prestable[f0..f0 + firma.bytes as usize]) {
        return no(IOMMU_NO_GSP_DISCO);
    }

    // Los marcos de la imagen (una vez) y los de la radix3.
    let paginas = paginas_de(imagen.bytes);
    for b in 0..paginas.div_ceil(BLOQUE_PAGINAS) as usize {
        if BLOQUES[b].load(Ordering::Acquire) != 0 {
            continue;
        }
        // Bloques ENTEROS aunque el ultimo sobre: si otro GSP-RM mas grande
        // se prepara despues, su ultimo bloque no se sale de lo pedido. La
        // 3060 lo leera por DMA: NEUTRO.
        let Some(f) = phys::alloc_frames_contig_de(BLOQUE_PAGINAS, phys::Titular::Neutro) else {
            return no(IOMMU_NO_GSP_MARCOS);
        };
        BLOQUES[b].store(f, Ordering::Release);
    }
    let rx = wpr::Radix3::de(imagen.bytes);
    if rx.paginas_tabla() > RADIX_MAX {
        return no(IOMMU_NO_GSP_FORMATO);
    }
    if RADIX.load(Ordering::Acquire) == 0 {
        // La 3060 la recorrera por DMA: NEUTRO.
        let Some(f) = phys::alloc_frames_contig_de(RADIX_MAX, phys::Titular::Neutro) else {
            return no(IOMMU_NO_GSP_MARCOS);
        };
        RADIX.store(f, Ordering::Release);
    }

    let p = Plan { imagen, firma, bootloader: bl, medida: mg, inicio: cg };
    // SAFETY: ver `PLAN`.
    unsafe {
        *core::ptr::addr_of_mut!(PLAN) = Some(p);
        *core::ptr::addr_of_mut!(CURSOR) = Some(cg);
        *core::ptr::addr_of_mut!(HASH_COPIA) = Some(bmo_hash::Hasher::new());
    }
    COPIADOS.store(0, Ordering::Release);
    COMPROBADOS.store(0, Ordering::Release);
    let n = trozos(&p);
    ESTADO.store(GSP_VALIDO | GSP_PREPARADO | n.min(0xFF) << GSP_TOTALES_SHIFT, Ordering::Release);
    crate::ring0::cabina::count("gpu", "L0c2: GSP-RM entendido por el kernel; trozos de 512 KiB", n);
    Ok(n)
}

/// **TROZO k**: 512 KiB del `.fwimage`, del disco a sus marcos. En orden: el
/// cursor de FAT32 solo avanza, y el BLAKE3 tambien. `Ok(copiados)`.
pub fn trozo(k: u64) -> Result<u64, u32> {
    if ESTADO.load(Ordering::Acquire) & GSP_PRESTADO != 0 {
        return no(IOMMU_NO_GSP_YA_PRESTADO);
    }
    let Some(p) = plan() else { return no(IOMMU_NO_GSP_ORDEN) };
    let n = trozos(&p);
    if k >= n || (k != 0 && k != COPIADOS.load(Ordering::Acquire)) {
        return no(IOMMU_NO_GSP_ORDEN);
    }
    // SAFETY: ver `PLAN`.
    let (cur, hash) = unsafe {
        if k == 0 {
            *core::ptr::addr_of_mut!(CURSOR) = Some(p.inicio);
            *core::ptr::addr_of_mut!(HASH_COPIA) = Some(bmo_hash::Hasher::new());
            apuntar(|v| v & !GSP_COPIADO);
        }
        (&mut *core::ptr::addr_of_mut!(CURSOR), &mut *core::ptr::addr_of_mut!(HASH_COPIA))
    };
    let (Some(cur), Some(hash)) = (cur.as_mut(), hash.as_mut()) else { return no(IOMMU_NO_GSP_ORDEN) };
    let desde = k * TROZO;
    let bytes = (p.imagen.bytes - desde).min(TROZO);
    // 128 paginas alineadas a 128 caen dentro de un bloque de 512.
    let dst = memoria(fisica_de(desde / PAGINA), paginas_de(bytes) * PAGINA);
    let (datos, cola) = dst.split_at_mut(bytes as usize);
    if fs::leer_rango(cur, (p.imagen.desde + desde) as usize, p.medida, datos) != datos.len() {
        return no(IOMMU_NO_GSP_DISCO);
    }
    // Lo que sobra de la ultima pagina, a 0: la radix3 la presta entera.
    cola.fill(0);
    hash.update(datos);
    COPIADOS.store(k + 1, Ordering::Release);
    if k + 1 == n {
        // SAFETY: ver `PLAN`.
        unsafe {
            if let Some(h) = (*core::ptr::addr_of_mut!(HASH_COPIA)).take() {
                *core::ptr::addr_of_mut!(DIGESTO_COPIA) = h.finalize();
            }
        }
        apuntar(|v| v | GSP_COPIADO);
        crate::ring0::cabina::count("gpu", "L0c2: el GSP-RM COPIADO del disco a marcos NEUTRO; bytes", p.imagen.bytes);
    }
    Ok(k + 1)
}

/// **PRESTAR**: la radix3, la WPR meta y el prestamo. `Ok(paginas prestadas)`.
pub fn prestar() -> Result<u64, u32> {
    let Some(p) = plan() else { return no(IOMMU_NO_GSP_ORDEN) };
    if ESTADO.load(Ordering::Acquire) & GSP_COPIADO == 0 {
        return no(IOMMU_NO_GSP_ORDEN);
    }
    let rx = wpr::Radix3::de(p.imagen.bytes);
    let paginas = paginas_de(p.imagen.bytes);
    let aux = AUX.load(Ordering::Acquire);
    let radix = RADIX.load(Ordering::Acquire);
    let hecho = PRESTADO.load(Ordering::Acquire);

    if hecho == 0 {
        // La radix3, palabra a palabra: lo que el GSP recorrera.
        for t in 0..rx.paginas_tabla() {
            let pag = memoria(radix + t * PAGINA, PAGINA);
            for i in 0..512u64 {
                let w = rx.palabra(IOVA_GSP_IMAGEN, IOVA_GSP_RADIX, t, i).to_le_bytes();
                pag[(i * 8) as usize..(i * 8 + 8) as usize].copy_from_slice(&w);
            }
        }
        // La WPR meta: el reparto de la VRAM y donde vera cada cosa.
        let fb = crate::ring0::dev::gpu::info_fb();
        let frts = bmo_gpu_ga10x::vbios::frts(
            fb as u32,
            crate::ring0::dev::gpu::info_vga() as u32,
            fb & crate::ring0::dev::gpu::GPU_FB_SIN_PANTALLA == 0,
        );
        let Some(m) = wpr::mapa(&frts, p.bootloader.bin.bytes as u64, p.imagen.bytes) else {
            return no(IOMMU_NO_GSP_SIN_VRAM);
        };
        let prestado = wpr::Prestado {
            radix3: IOVA_GSP_RADIX,
            bootloader: IOVA_GSP_AUX,
            firma: IOVA_GSP_AUX + AUX_FIRMA * PAGINA,
        };
        let meta = wpr::wpr_meta(&m, &p.bootloader, p.imagen.bytes, p.firma.bytes, &prestado);
        let pag = memoria(aux + AUX_META * PAGINA, PAGINA);
        pag.fill(0);
        pag[..meta.len()].copy_from_slice(&meta);
    }

    // El prestamo, pieza a pieza; lo ya prestado no se repite.
    let marca = |bit: u64, iova: u64, fisica: u64, n: u64, escribe: bool| -> Result<(), u32> {
        if PRESTADO.load(Ordering::Acquire) & bit != 0 {
            return Ok(());
        }
        io::prestar_gpu(iova, fisica, n, escribe)?;
        PRESTADO.fetch_or(bit, Ordering::AcqRel);
        Ok(())
    };
    for b in 0..paginas.div_ceil(BLOQUE_PAGINAS) {
        let n = (paginas - b * BLOQUE_PAGINAS).min(BLOQUE_PAGINAS);
        let fisica = BLOQUES[b as usize].load(Ordering::Acquire);
        if let Err(m) = marca(1 << b, IOVA_GSP_IMAGEN + b * BLOQUE_PAGINAS * PAGINA, fisica, n, false) {
            return no(m);
        }
    }
    let res = marca(PRESTADO_RADIX, IOVA_GSP_RADIX, radix, rx.paginas_tabla(), false)
        .and_then(|_| marca(PRESTADO_AUX, IOVA_GSP_AUX, aux, AUX_META, false))
        .and_then(|_| marca(PRESTADO_META, IOVA_GSP_AUX + AUX_META * PAGINA, aux + AUX_META * PAGINA, 1, true));
    if let Err(m) = res {
        return no(m);
    }
    let total = paginas + rx.paginas_tabla() + AUX_META + 1;
    apuntar(|v| (v & !(0xFF_FFFF << GSP_PRESTADAS_SHIFT)) | GSP_PRESTADO | total.min(0xFF_FFFF) << GSP_PRESTADAS_SHIFT);
    // SAFETY: ver `PLAN`.
    unsafe { *core::ptr::addr_of_mut!(HASH_RADIX) = Some(bmo_hash::Hasher::new()) };
    COMPROBADOS.store(0, Ordering::Release);
    crate::ring0::cabina::count("gpu", "L0c2: el GSP-RM y su radix3 PRESTADOS a la 3060; paginas", total);
    Ok(total)
}

/// Una palabra de 64 bits en la fisica `f`.
fn palabra_en(f: u64) -> u64 {
    // SAFETY: `f` sale de `io::ve_la_gpu` sobre una IOVA de la radix3, o sea
    // un marco NEUTRO de este fichero; alineada a 8.
    unsafe { (crate::ring0::mm::phys_to_virt(f & !7) as *const u64).read_volatile() }
}

/// **La pagina `p` de la imagen, como la encontrara el GSP**: nivel 0, nivel
/// 1, nivel 2 y la pagina, cada salto por la IOMMU. `Some(fisica)` si lleva a
/// donde tiene que llevar y la 3060 no puede escribirla.
fn por_la_radix(p: u64) -> Option<u64> {
    let solo_leer = |iova: u64| io::ve_la_gpu(iova).filter(|&(_, escribe)| !escribe).map(|(f, _)| f);
    let l1 = palabra_en(solo_leer(IOVA_GSP_RADIX)?);
    let j = p / 512;
    let l2 = palabra_en(solo_leer(l1 + (j / 512) * PAGINA)? + (j % 512) * 8);
    let pagina = palabra_en(solo_leer(l2)? + (p % 512) * 8);
    if pagina != IOVA_GSP_IMAGEN + p * PAGINA {
        return None;
    }
    solo_leer(pagina)
}

/// **COMPROBAR k**: 512 KiB leidos por la radix3 y la IOMMU. `Ok(comprobados)`.
pub fn comprobar(k: u64) -> Result<u64, u32> {
    let Some(p) = plan() else { return no(IOMMU_NO_GSP_ORDEN) };
    if ESTADO.load(Ordering::Acquire) & GSP_PRESTADO == 0 {
        return no(IOMMU_NO_GSP_ORDEN);
    }
    let n = trozos(&p);
    if k >= n || (k != 0 && k != COMPROBADOS.load(Ordering::Acquire)) {
        return no(IOMMU_NO_GSP_ORDEN);
    }
    // SAFETY: ver `PLAN`.
    let hash = unsafe {
        if k == 0 {
            *core::ptr::addr_of_mut!(HASH_RADIX) = Some(bmo_hash::Hasher::new());
            apuntar(|v| v & !(GSP_CUADRA | GSP_ES_570));
        }
        &mut *core::ptr::addr_of_mut!(HASH_RADIX)
    };
    let Some(hash) = hash.as_mut() else { return no(IOMMU_NO_GSP_ORDEN) };
    let desde = k * TROZO;
    let hasta = (desde + TROZO).min(p.imagen.bytes);
    let mut a = desde;
    while a < hasta {
        let pag = a / PAGINA;
        let Some(f) = por_la_radix(pag) else { return no(IOMMU_NO_GSP_RADIX) };
        let bytes = (hasta - a).min(PAGINA);
        hash.update(memoria(f, bytes));
        a += bytes;
    }
    COMPROBADOS.store(k + 1, Ordering::Release);
    if k + 1 == n {
        // SAFETY: ver `PLAN`.
        let (radix, copia) = unsafe {
            let d = match (*core::ptr::addr_of_mut!(HASH_RADIX)).take() {
                Some(h) => h.finalize(),
                None => [0; 32],
            };
            *core::ptr::addr_of_mut!(DIGESTO_RADIX) = d;
            (d, *core::ptr::addr_of!(DIGESTO_COPIA))
        };
        let cuadra = radix == copia;
        let es_570 = radix == HASH_570;
        apuntar(|v| v | if cuadra { GSP_CUADRA } else { 0 } | if es_570 { GSP_ES_570 } else { 0 });
        if cuadra && es_570 {
            crate::ring0::cabina::count("gpu", "L0c2: la radix3 lleva al GSP-RM de la 570.144 ENTERO, por la IOMMU; bytes", p.imagen.bytes);
        } else {
            crate::ring0::cabina::warn("gpu", "L0c2: lo que se ve por la radix3 NO es lo copiado, o no es la 570.144", cuadra as u64);
        }
    }
    Ok(k + 1)
}

/// `INFO_GPU_GSP`: `0..7` trozos copiados | `8..15` trozos totales | `16..23`
/// comprobados | `24..31` el ultimo NO | `32..55` paginas prestadas | 56
/// preparado | 57 copiado | 58 PRESTADO | 59 CUADRA (radix3 = copia) | 60 es
/// la 570.144 | 63 valido.
pub fn info_gsp() -> u64 {
    let v = ESTADO.load(Ordering::Acquire);
    if v & GSP_VALIDO == 0 {
        return 0;
    }
    v | COPIADOS.load(Ordering::Acquire).min(0xFF) | COMPROBADOS.load(Ordering::Acquire).min(0xFF) << GSP_COMPROBADOS_SHIFT
}

/// `INFO_GPU_GSP_HASH`: los 8 primeros bytes del BLAKE3 visto por la radix3
/// (o del copiado, si aun no se comprobo), como u64 little-endian.
pub fn info_gsp_hash() -> u64 {
    // SAFETY: ver `PLAN`; solo se leen.
    let (r, c) = unsafe { (*core::ptr::addr_of!(DIGESTO_RADIX), *core::ptr::addr_of!(DIGESTO_COPIA)) };
    let d = if r != [0; 32] { r } else { c };
    u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]])
}
