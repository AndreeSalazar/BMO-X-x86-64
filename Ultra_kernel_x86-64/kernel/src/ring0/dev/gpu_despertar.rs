//! **DESPERTAR EL GSP (L0c3b)** -- la pagina de vaciado registrada, el GSP con
//! sus argumentos de LIBOS en el buzon, y el booter FIRMADO en el SEC2 con la
//! WPR meta en el suyo. Si todo va bien, el booter sube el GSP-RM a la WPR2 y
//! enciende el RISC-V del GSP.
//!
//! [carril]  ROJO      arranca firmware firmado en dos falcons de la 3060
//! [consumo] NADA      corre por orden (`gpu despertar`, y el paso `despertar`)
//!
//! [eje]     CORRECCION -- es el paso de nova-core (`gsp/hal/tu102.rs::boot` y
//!           `gsp/boot.rs`) en el mismo orden; uno cambiado es un GSP mudo
//!
//! # El orden (nova-core, Linux 7.3, GA102: sin el bootloader de FWSEC)
//!
//! ```text
//!    0  la pagina de vaciado en 0x100C40 / 0x100C10    (`SysmemFlush`)
//!    -  FWSEC-FRTS                                      ya: L0b monto la WPR2
//!    1  GSP: reset; MAILBOX0/1 = los argumentos de LIBOS (0x3C000000);
//!       STARTCPU y esperar a que se pare               (`gsp_falcon.boot`)
//!    2  SEC2: reset, FBIF, la app 0 del booter a la IMEM SEGURA con su
//!       etiqueta (+0x100), sus datos a la DMEM, el BROM con la firma del
//!       fusible, BOOTVEC 0x100, MAILBOX0/1 = la WPR meta (0x3E007000),
//!       STARTCPU                                       (`BooterFirmware::run`)
//!    3  el SEC2 se para con MAILBOX0 = 0; el registro OS del GSP = la
//!       `app_version` del bootloader                    (`write_os_version`)
//!    4  el RISC-V del GSP ACTIVO (hasta 5 s)            (`is_riscv_active`)
//! ```
//!
//! Tres ordenes y no una, por la maquina, como FWSEC: DESPERTAR hace 0 y 1 y
//! vuelve en cuanto el GSP arranca; BOOTER hace 2 con el GSP ya parado y
//! vuelve en cuanto arranca el SEC2; ACABAR hace 3. Quien espera entre medias
//! es el escritorio, cediendo el turno -- nova-core da 2 s a cada falcon, y
//! 2 s dentro de un syscall son 500 latidos del bus USB. El 4 lo mira el
//! escritorio por `INFO_GPU_DESPIERTO`, que lee los falcons en vivo.
//!
//! # Lo que se puede ver despues
//!
//! El GSP escribe en sus logs y en su cola (L0c3a): `INFO_GPU_GSP_MEM` los
//! deja leer de 8 en 8 bytes, y el escritorio guarda LOGINIT en el disco.

use core::sync::atomic::{AtomicU64, Ordering};

use bmo_gpu_ga10x::falcon as fa;
use bmo_gpu_ga10x::{booter, vbios as vb, Registros};

use crate::ring0::dev::gpu_gsp::{self, Fichero};
use crate::ring0::dev::gpu_libos;
use crate::ring0::dev::gpu_prestamo::{self as pr, Bar0};
use crate::ring0::mm::phys;
use crate::ring0::plat::iommu as io;

/// Donde lo deja el build. Lo abre `syscall/op_gsp.rs` (L8b).
pub const RUTA_BOOTER: &str = "fw/gsp/boot_ld.bin";
/// Donde ve la 3060 el ucode firmado del booter.
pub const IOVA_BOOTER: u64 = 0x3B00_0000;
/// La WPR meta: la pagina 7 de lo auxiliar de L0c2.
pub const IOVA_WPR_META: u64 = gpu_gsp::IOVA_GSP_AUX + 7 * 4096;

const PAGINA: u64 = 4096;
/// 0..16 el ucode (lo que se presta); 16..32 el fichero entero, sin prestar.
const BOOTER_PAGINAS: u64 = 32;
const UCODE_BYTES: u64 = 16 * PAGINA;

/// `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` (direccion >> 8) y su `_HI` (>> 40).
pub const VACIADO: u32 = 0x0010_0C10;
pub const VACIADO_HI: u32 = 0x0010_0C40;

pub const IOMMU_NO_DESPERTAR_ANTES: u32 = 37;
pub const IOMMU_NO_BOOTER: u32 = 38;
pub const IOMMU_NO_BOOTER_FIRMA: u32 = 39;
pub const IOMMU_NO_VACIADO: u32 = 40;
pub const IOMMU_NO_GSP_FALCON: u32 = 41;
pub const IOMMU_NO_SEC2: u32 = 42;
pub const IOMMU_NO_SEC2_NO_PARA: u32 = 43;
pub const IOMMU_NO_BOOTER_MAL: u32 = 44;
pub const IOMMU_NO_YA_DESPIERTO: u32 = 45;

pub const DESPIERTO_VACIADO: u64 = 1 << 0;
pub const DESPIERTO_GSP_ARRANCADO: u64 = 1 << 1;
pub const DESPIERTO_GSP_PARADO: u64 = 1 << 2;
pub const DESPIERTO_BOOTER: u64 = 1 << 3;
pub const DESPIERTO_SEC2_ARRANCADO: u64 = 1 << 4;
pub const DESPIERTO_SEC2_PARADO: u64 = 1 << 5;
pub const DESPIERTO_OS: u64 = 1 << 6;
pub const DESPIERTO_RISCV_ACTIVO: u64 = 1 << 7;
pub const DESPIERTO_RISCV_PARADO: u64 = 1 << 8;
pub const DESPIERTO_VISTO: u64 = 1 << 9;
pub const DESPIERTO_FIRMA_SHIFT: u64 = 10;
pub const DESPIERTO_VALIDO: u64 = 1 << 15;
pub const DESPIERTO_MOTIVO_SHIFT: u64 = 16;
pub const DESPIERTO_BUZON_SHIFT: u64 = 32;

/// Banderas, firma y motivo; lo vivo se lee en `info_despierto`.
static ESTADO: AtomicU64 = AtomicU64::new(0);
static BOOTER_F: AtomicU64 = AtomicU64::new(0);

fn apuntar(f: impl FnOnce(u64) -> u64) {
    let v = ESTADO.load(Ordering::Acquire);
    ESTADO.store(f(v), Ordering::Release);
}

fn no(motivo: u32) -> Result<u64, u32> {
    apuntar(|v| (v & !(0xFF << DESPIERTO_MOTIVO_SHIFT)) | DESPIERTO_VALIDO | (motivo as u64 & 0xFF) << DESPIERTO_MOTIVO_SHIFT);
    crate::ring0::cabina::warn("gpu", "L0c3b: el GSP no se desperto; motivo", motivo as u64);
    Err(motivo)
}

fn memoria(fisica: u64, bytes: u64) -> &'static mut [u8] {
    // SAFETY: los marcos NEUTRO de `BOOTER_F`, de este fichero y del medida
    // con que se pidieron.
    unsafe { core::slice::from_raw_parts_mut(crate::ring0::mm::phys_to_virt(fisica) as *mut u8, bytes as usize) }
}

/// Lo de antes, hecho EN ESTE ARRANQUE: la WPR2 de FWSEC, el GSP-RM por su
/// radix3 y lo que el GSP escribe; y el candado de todo DMA de la 3060.
fn antes() -> Result<Bar0, u32> {
    let bar0 = pr::candado_dma()?;
    let wpr2 = (crate::ring0::dev::gpu::info_wpr2() >> 32) as u32 >> 4 != 0;
    let g = gpu_gsp::info_gsp();
    let radix = g & gpu_gsp::GSP_CUADRA != 0 && g & gpu_gsp::GSP_ES_570 != 0;
    let libos = gpu_libos::info_libos() & gpu_libos::LIBOS_COMPROBADO != 0;
    if gpu_gsp::app_version().is_none() || !wpr2 || !radix || !libos {
        return Err(IOMMU_NO_DESPERTAR_ANTES);
    }
    Ok(Bar0(bar0))
}

/// **DESPERTAR**: 0 y 1 del orden. `Ok(IOVA de los argumentos)` en cuanto el
/// GSP arranca; si se paro, lo dice `INFO_GPU_DESPIERTO`.
pub fn despertar() -> Result<u64, u32> {
    if ESTADO.load(Ordering::Acquire) & DESPIERTO_SEC2_ARRANCADO != 0 {
        // El booter ya corrio en este arranque: la WPR2 ya tiene el GSP-RM, y
        // correrlo otra vez sin el booter_unload no se hace.
        return no(IOMMU_NO_YA_DESPIERTO);
    }
    let mut r = match antes() {
        Ok(r) => r,
        Err(m) => return no(m),
    };
    apuntar(|x| x | DESPIERTO_VALIDO);

    // 0. La pagina de vaciado, y que se relea.
    let v = gpu_libos::IOVA_VACIADO;
    r.escribir(VACIADO_HI, (v >> 40) as u32);
    r.escribir(VACIADO, (v >> 8) as u32);
    if r.leer(VACIADO) != (v >> 8) as u32 {
        return no(IOMMU_NO_VACIADO);
    }
    apuntar(|x| x | DESPIERTO_VACIADO);

    // 1. El GSP, con sus argumentos de LIBOS en el buzon, arranca (y se para
    // solo: aun no tiene codigo; el que lo pondra es el booter).
    crate::ring0::cabina::info("gpu", "L0c3b: el GSP con sus argumentos de LIBOS en el buzon", gpu_libos::IOVA_LIBOS);
    let mut t = pr::reloj();
    let boot0 = r.leer(bmo_gpu_ga10x::BOOT_0);
    let l = gpu_libos::IOVA_LIBOS;
    if let Err(e) = fa::resetear(&mut r, &mut t, fa::GSP, boot0)
        .and_then(|_| fa::arrancar_con(&mut r, fa::GSP, None, Some(l as u32), Some((l >> 32) as u32)))
    {
        crate::ring0::cabina::warn("gpu", "L0c3b: el falcon del GSP no dejo resetearse; motivo", pr::motivo(e));
        return no(IOMMU_NO_GSP_FALCON);
    }
    apuntar(|x| x | DESPIERTO_GSP_ARRANCADO);
    Ok(l)
}

/// **BOOTER**: 2 del orden, con el GSP ya parado. `Ok(la firma usada)` en
/// cuanto el SEC2 arranca; si se paro, lo dice `INFO_GPU_DESPIERTO`.
pub fn booter(booter_fichero: Option<&mut dyn Fichero>) -> Result<u64, u32> {
    let e = ESTADO.load(Ordering::Acquire);
    if e & DESPIERTO_SEC2_ARRANCADO != 0 {
        return no(IOMMU_NO_YA_DESPIERTO);
    }
    let mut r = match antes() {
        Ok(r) => r,
        Err(m) => return no(m),
    };
    if e & DESPIERTO_GSP_ARRANCADO == 0 || !matches!(fa::como_va(&mut r, fa::GSP), Ok((true, _, _))) {
        return no(IOMMU_NO_GSP_FALCON);
    }

    // El booter: leido, firmado con la firma del FUSIBLE del SEC2, prestado.
    let Some(f) = booter_fichero else { return no(IOMMU_NO_BOOTER) };
    if BOOTER_F.load(Ordering::Acquire) == 0 {
        // La 3060 lo leera por DMA: NEUTRO.
        let Some(p) = phys::alloc_frames_contig_de(BOOTER_PAGINAS, phys::Titular::Neutro) else {
            return no(IOMMU_NO_BOOTER);
        };
        BOOTER_F.store(p, Ordering::Release);
    }
    let base = BOOTER_F.load(Ordering::Acquire);
    let todo = memoria(base, BOOTER_PAGINAS * PAGINA);
    todo.fill(0);
    let (ucode, fichero) = todo.split_at_mut(UCODE_BYTES as usize);
    let medida = f.medida();
    if medida > fichero.len() as u64 || f.leer(0, &mut fichero[..medida as usize]) != medida as usize {
        return no(IOMMU_NO_BOOTER);
    }
    let fichero = &fichero[..medida as usize];
    let Ok(b) = booter::booter(fichero) else { return no(IOMMU_NO_BOOTER) };
    if b.bin.bytes as u64 > UCODE_BYTES || b.engine_id_mask & 1 == 0 || b.dmem.origen % 256 != 0 {
        return no(IOMMU_NO_BOOTER);
    }
    let Some(reg) = vb::registro_fusible(b.engine_id_mask, b.ucode_id) else { return no(IOMMU_NO_BOOTER_FIRMA) };
    let Some(idx) = b.indice_de_firma(r.leer(reg)) else { return no(IOMMU_NO_BOOTER_FIRMA) };
    ucode[..b.bin.bytes as usize].copy_from_slice(b.bin.ucode(fichero));
    if booter::firmar(fichero, &b, idx, ucode).is_err() {
        return no(IOMMU_NO_BOOTER_FIRMA);
    }
    if e & DESPIERTO_BOOTER == 0 {
        if let Err(m) = io::prestar_gpu(IOVA_BOOTER, base, UCODE_BYTES / PAGINA, false) {
            return no(m);
        }
    }
    apuntar(|v| (v & !(3 << DESPIERTO_FIRMA_SHIFT)) | DESPIERTO_BOOTER | (idx as u64 & 3) << DESPIERTO_FIRMA_SHIFT);

    // 2. El booter en el SEC2, con la WPR meta en el buzon.
    crate::ring0::cabina::info("gpu", "L0c3b: el booter al SEC2; la WPR meta en", IOVA_WPR_META);
    let mut t = pr::reloj();
    let boot0 = r.leer(bmo_gpu_ga10x::BOOT_0);
    let a256 = |n: u32| n.next_multiple_of(256);
    let cargado = fa::resetear(&mut r, &mut t, fa::SEC2, boot0)
        .and_then(|_| fa::preparar_fbif(&mut r, fa::SEC2))
        .and_then(|_| {
            fa::copiar_etiquetado(&mut r, &mut t, fa::SEC2, IOVA_BOOTER, b.imem.origen, b.imem.destino, a256(b.imem.bytes), true, true, 50_000)
        })
        .and_then(|_| {
            fa::copiar(&mut r, &mut t, fa::SEC2, IOVA_BOOTER + b.dmem.origen as u64, b.dmem.destino, a256(b.dmem.bytes), false, false, 50_000)
        });
    if let Err(e) = cargado {
        crate::ring0::cabina::warn("gpu", "L0c3b: el SEC2 no dejo cargar el booter; motivo", pr::motivo(e));
        return no(IOMMU_NO_SEC2);
    }
    fa::brom(&mut r, fa::SEC2, b.pkc_data_offset(), b.engine_id_mask, b.ucode_id);
    let m = IOVA_WPR_META;
    if fa::arrancar_con(&mut r, fa::SEC2, Some(b.arranque()), Some(m as u32), Some((m >> 32) as u32)).is_err() {
        return no(IOMMU_NO_SEC2);
    }
    apuntar(|x| x | DESPIERTO_SEC2_ARRANCADO);
    crate::ring0::cabina::count("gpu", "L0c3b: el BOOTER ARRANCADO en el SEC2; firma", idx as u64);
    Ok(idx as u64)
}

/// **ACABAR**: el SEC2 parado con MAILBOX0 = 0, y el OS del GSP escrito.
/// `Ok(MAILBOX1 del SEC2)`.
pub fn acabar() -> Result<u64, u32> {
    if ESTADO.load(Ordering::Acquire) & DESPIERTO_SEC2_ARRANCADO == 0 {
        return no(IOMMU_NO_DESPERTAR_ANTES);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return no(io::IOMMU_NO_SIN_GPU);
    }
    let mut r = Bar0(bar0);
    let Ok((parado, m0, m1)) = fa::como_va(&mut r, fa::SEC2) else { return no(IOMMU_NO_SEC2) };
    if !parado {
        return no(IOMMU_NO_SEC2_NO_PARA);
    }
    if m0 != 0 {
        return no(IOMMU_NO_BOOTER_MAL);
    }
    let app = gpu_gsp::app_version().unwrap_or(0);
    r.escribir(fa::GSP + fa::OS, app);
    apuntar(|x| x | DESPIERTO_OS);
    crate::ring0::cabina::count("gpu", "L0c3b: el booter acabo con MAILBOX0 = 0; OS del GSP", app as u64);
    Ok(m1 as u64)
}

/// `INFO_GPU_DESPIERTO`: 0 vaciado | 1 GSP arrancado | 2 GSP PARADO (vivo) |
/// 3 booter firmado y prestado | 4 SEC2 arrancado | 5 SEC2 PARADO (vivo) | 6
/// OS escrito | 7 RISC-V ACTIVO (vivo) | 8 RISC-V parado (vivo) | 9 se VIO
/// activo | `10..11` la firma | 15 valido | `16..23` el ultimo NO | `32..63`
/// MAILBOX0 del SEC2 (vivo, si arranco).
pub fn info_despierto() -> u64 {
    let mut v = ESTADO.load(Ordering::Acquire);
    if v & DESPIERTO_VALIDO == 0 {
        return 0;
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return v;
    }
    let mut r = Bar0(bar0);
    if v & DESPIERTO_GSP_ARRANCADO != 0 {
        if let Ok((true, _, _)) = fa::como_va(&mut r, fa::GSP) {
            v |= DESPIERTO_GSP_PARADO;
        }
    }
    if v & DESPIERTO_SEC2_ARRANCADO != 0 {
        if let Ok((parado, m0, _)) = fa::como_va(&mut r, fa::SEC2) {
            v |= if parado { DESPIERTO_SEC2_PARADO } else { 0 } | (m0 as u64) << DESPIERTO_BUZON_SHIFT;
        }
        if let Ok((activo, parado)) = fa::riscv(&mut r, fa::GSP) {
            v |= if activo { DESPIERTO_RISCV_ACTIVO } else { 0 } | if parado { DESPIERTO_RISCV_PARADO } else { 0 };
            if activo && ESTADO.load(Ordering::Acquire) & DESPIERTO_VISTO == 0 {
                ESTADO.fetch_or(DESPIERTO_VISTO, Ordering::AcqRel);
                crate::ring0::cabina::count("gpu", "L0c3b: el RISC-V del GSP ESTA ACTIVO", 1);
                v |= DESPIERTO_VISTO;
            }
        }
    }
    v
}

/// `INFO_GPU_DESPIERTO_BUZON`: MAILBOX0 | MAILBOX1 << 32 del GSP (vivo).
pub fn info_despierto_buzon() -> u64 {
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || ESTADO.load(Ordering::Acquire) & DESPIERTO_GSP_ARRANCADO == 0 {
        return 0;
    }
    match fa::como_va(&mut Bar0(bar0), fa::GSP) {
        Ok((_, m0, m1)) => m0 as u64 | (m1 as u64) << 32,
        Err(_) => 0,
    }
}
