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

    // Lo que dijo FWSEC, antes de quitarle su falcon.
    pr::fotografiar_fwsec();
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
    // Se paro: queda dicho. Luego el RISC-V lo arranca otra vez y en vivo ya
    // no se ve (metal 24-09 07:10: la fila decia `-gsp` con el GSP despierto).
    apuntar(|x| x | DESPIERTO_GSP_PARADO);

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

/// **El falcon del GSP ya es del GSP-RM?** Desde que L0c3b lo reseteo, lo que
/// diga en vivo no es de FWSEC.
pub fn gsp_tomado() -> bool {
    ESTADO.load(Ordering::Acquire) & DESPIERTO_GSP_ARRANCADO != 0
}

/// `INFO_GPU_DESPIERTO_BUZON`: MAILBOX0 | MAILBOX1 << 32 (vivo) del GSP, o
/// con selector 1 (`1 << 8`) del SEC2, si el booter arranco; con selector 2,
/// como va el secuenciador (L0c4b2c, `info_secuencia`).
///
/// ** El SEC2 lo trajo el metal (24-09 07:48): el booter se paro con MAILBOX0
/// = 0x15 donde tres veces antes dio 0. nova-core imprime los DOS buzones al
/// fallar; aqui solo se veia el primero.
pub fn info_despierto_buzon(sel: u64) -> u64 {
    if sel >> 8 == 2 {
        return info_secuencia();
    }
    if sel >> 8 == 3 {
        return info_bar1();
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    let (falcon, hace_falta) = if sel >> 8 == 1 { (fa::SEC2, DESPIERTO_SEC2_ARRANCADO) } else { (fa::GSP, DESPIERTO_GSP_ARRANCADO) };
    if bar0 == 0 || ESTADO.load(Ordering::Acquire) & hace_falta == 0 {
        return 0;
    }
    match fa::como_va(&mut Bar0(bar0), falcon) {
        Ok((_, m0, m1)) => m0 as u64 | (m1 as u64) << 32,
        Err(_) => 0,
    }
}

// == L0c4b2c: CORRER EL SECUENCIADOR (2026-09-24) =============================
//
// El GSP-RM se para tras pedir `GSP_RUN_CPU_SEQUENCER` y espera a que la CPU
// haga sus ordenes. El metal (24-09 08:59) trajo 420: 416 con registro, TODAS
// en el falcon del GSP, y al final CORE_RESET/START/WAIT_FOR_HALT/RESUME.
//
// El kernel lee el mensaje EL MISMO de la cola del GSP (el escritorio solo
// dice "sigue"), comprueba su suma, lo entiende con `bmo_gpu_ga10x::
// secuenciador` y valida TODAS las ordenes contra lo permitido (el falcon del
// GSP) antes de la primera escritura. Luego lo corre `bmo_gpu_ga10x::correr`
// en tramos de 1 ms: cada llamada sigue donde se quedo la anterior, y las
// esperas cuentan su plazo entre llamadas. Acabado, devuelve los huecos del
// mensaje al GSP. Una sola vez por arranque: a medias no se repite.

use bmo_gpu_ga10x::correr::{Contexto, Corredor, Falla, Tramo};
use bmo_gpu_ga10x::secuenciador::{self as sq, Orden};

/// El GSP no desperto, o lo primero de su cola no es un secuenciador entero.
pub const IOMMU_NO_SEC_ANTES: u32 = 50;
/// Una orden fallo o se sale del falcon del GSP: el detalle en `info_secuencia`.
pub const IOMMU_NO_SEC_FALLO: u32 = 51;
/// Ya se corrio en este arranque.
pub const IOMMU_NO_SEC_YA: u32 = 52;

const MAX_ORDENES: usize = 1024;
const MAX_BYTES: usize = 16 * PAGINA as usize;
const TRAMO_US: u64 = 1000;

/// `info_secuencia`, bits 24..31: como va.
pub const SEC_CARGADO: u64 = 0x40;
pub const SEC_HECHO: u64 = 0x80;
/// ...o por que se paro (`Falla`).
pub const SEC_FUERA: u64 = 1;
pub const SEC_PLAZO: u64 = 2;
pub const SEC_NO_CONTESTA: u64 = 3;
pub const SEC_FALCON: u64 = 4;
pub const SEC_SEC2: u64 = 5;

/// `i | fase << 16 | como va << 24 | dato << 32`.
static SEC: AtomicU64 = AtomicU64::new(0);
/// Desde cuando se espera lo que se espera (us + 1; 0 = nada).
static SEC_DESDE: AtomicU64 = AtomicU64::new(0);
/// `ordenes | paginas del mensaje << 16 | su pagina << 24`.
static SEC_MSG: AtomicU64 = AtomicU64::new(0);
static mut SEC_ORDENES: [Orden; MAX_ORDENES] = [Orden::Resetear; MAX_ORDENES];
static mut SEC_DATOS: [u8; MAX_BYTES] = [0; MAX_BYTES];

fn sec_no(codigo: u64, dato: u32, motivo: u32) -> Result<u64, u32> {
    let v = SEC.load(Ordering::Acquire) & 0xFF_FFFF;
    SEC.store(v | codigo << 24 | (dato as u64) << 32, Ordering::Release);
    crate::ring0::cabina::warn("gpu", "L0c4b2c: el secuenciador se paro; como va", SEC.load(Ordering::Acquire));
    Err(motivo)
}

/// **Cargar el secuenciador** de la cola del GSP: su suma, sus ordenes. `Ok(n)`.
fn cargar() -> Result<usize, u32> {
    let f = gpu_libos::gspmem();
    if f == 0 {
        return Err(IOMMU_NO_SEC_ANTES);
    }
    let ver = |o: u64| crate::ring0::mm::phys_to_virt(f + o);
    // El readPtr de la CPU (cabecera rx de la cola de la CPU).
    // SAFETY: dentro de GspMem (marcos NEUTRO de `gpu_libos`); volatile: la 3060.
    let p = unsafe { (ver(bmo_gpu_ga10x::libos::COLA_CPU + 32) as *const u32).read_volatile() } as u64 % 63;
    let byte = |o: usize| -> u8 {
        let d = bmo_gpu_ga10x::libos::COLA_GSP + PAGINA + (p * PAGINA + o as u64) % (63 * PAGINA);
        // SAFETY: como arriba, dentro de las 63 paginas de la cola del GSP.
        unsafe { (ver(d) as *const u8).read_volatile() }
    };
    let mut c = [0u8; bmo_gpu_ga10x::rpc::CABECERA];
    for (k, b) in c.iter_mut().enumerate() {
        *b = byte(k);
    }
    let m = bmo_gpu_ga10x::rpc::Mensaje::de(&c);
    if !m.bien_formado() || m.funcion != bmo_gpu_ga10x::rpc::SECUENCIADOR || m.datos() > MAX_BYTES {
        return Err(IOMMU_NO_SEC_ANTES);
    }
    // SAFETY: SEC_DATOS y SEC_ORDENES solo se tocan aqui y en `secuenciar`,
    // desde la syscall del escritorio (un solo hilo).
    let datos = unsafe { &mut *core::ptr::addr_of_mut!(SEC_DATOS) };
    let mut s = bmo_gpu_ga10x::rpc::Suma::default();
    s.mas(&c);
    for k in 0..m.datos() {
        datos[k] = byte(bmo_gpu_ga10x::rpc::CABECERA + k);
    }
    s.mas(&datos[..m.datos()]);
    if s.valor() != 0 {
        return Err(IOMMU_NO_SEC_ANTES);
    }
    // SAFETY: como arriba.
    let ordenes = unsafe { &mut *core::ptr::addr_of_mut!(SEC_ORDENES) };
    let mut n = 0;
    for x in sq::ordenes(&datos[..m.datos()]) {
        match x {
            Ok(o) if n < MAX_ORDENES => {
                ordenes[n] = o;
                n += 1;
            }
            _ => return Err(IOMMU_NO_SEC_ANTES),
        }
    }
    SEC_MSG.store(n as u64 | (m.paginas as u64) << 16 | p << 24, Ordering::Release);
    SEC.store(SEC_CARGADO << 24, Ordering::Release);
    crate::ring0::cabina::count("gpu", "L0c4b2c: secuenciador cargado de la cola del GSP; ordenes", n as u64);
    Ok(n)
}

/// **Un tramo del secuenciador.** `Ok(info_secuencia)`: con `SEC_HECHO`, el
/// GSP-RM volvio y los huecos del mensaje ya son suyos; si no, llamar otra vez.
pub fn secuenciar() -> Result<u64, u32> {
    if ESTADO.load(Ordering::Acquire) & DESPIERTO_VISTO == 0 {
        return Err(IOMMU_NO_SEC_ANTES);
    }
    let v = SEC.load(Ordering::Acquire);
    let como = v >> 24 & 0xFF;
    if como & SEC_HECHO != 0 {
        return Err(IOMMU_NO_SEC_YA);
    }
    if como & 0x3F != 0 {
        return Err(IOMMU_NO_SEC_FALLO);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return Err(io::IOMMU_NO_SIN_GPU);
    }
    if como & SEC_CARGADO == 0 {
        // BAR1 tal como la dejo el GOP, ANTES de que el GSP-RM vuelva: para
        // devolversela a la pantalla (`devolver_bar1`).
        if BAR1_ANTES.load(Ordering::Acquire) == 0 {
            let b = Bar0(bar0).leer(BAR1_BLOCK);
            BAR1_ANTES.store(1 << 63 | b as u64, Ordering::Release);
        }
        // Sin mensaje todavia NO se queda apuntado: no se ha escrito nada, y
        // se puede volver a pedir cuando llegue. Lo que se queda es una falla
        // a medias, que no se repite.
        cargar()?;
    }
    let msg = SEC_MSG.load(Ordering::Acquire);
    let n = (msg & 0xFFFF) as usize;
    // SAFETY: como en `cargar`.
    let ordenes = unsafe { &(&*core::ptr::addr_of!(SEC_ORDENES))[..n] };
    let v = SEC.load(Ordering::Acquire);
    let desde = SEC_DESDE.load(Ordering::Acquire);
    let mut c = Corredor { i: (v & 0xFFFF) as usize, fase: (v >> 16 & 0xFF) as u8, desde: desde.checked_sub(1) };
    let mut r = Bar0(bar0);
    let ctx = Contexto {
        boot0: r.leer(bmo_gpu_ga10x::BOOT_0),
        libos: gpu_libos::IOVA_LIBOS,
        os: gpu_gsp::app_version().unwrap_or(0),
    };
    let tramo = c.correr(ordenes, &mut r, &mut pr::reloj(), &ctx, TRAMO_US);
    SEC.store(c.i as u64 | (c.fase as u64) << 16 | SEC_CARGADO << 24, Ordering::Release);
    SEC_DESDE.store(c.desde.map_or(0, |d| d + 1), Ordering::Release);
    match tramo {
        Tramo::Sigue => Ok(info_secuencia()),
        Tramo::Hecho => {
            // El mensaje, consumido: sus huecos vuelven al GSP (nova-core lo
            // consume al recibirlo, antes de correrlo).
            let (paginas, p) = (msg >> 16 & 0xFF, msg >> 24 & 0xFF);
            let _ = gpu_libos::mover_lectura((p + paginas) % 63);
            SEC.fetch_or(SEC_HECHO << 24, Ordering::AcqRel);
            crate::ring0::cabina::count("gpu", "L0c4b2c: SECUENCIADOR CORRIDO y el GSP-RM volvio; ordenes", n as u64);
            Ok(info_secuencia())
        }
        Tramo::Falla(f) => match f {
            Falla::Fuera(_, reg) => sec_no(SEC_FUERA, reg, IOMMU_NO_SEC_FALLO),
            Falla::Plazo(_, visto) => sec_no(SEC_PLAZO, visto, IOMMU_NO_SEC_FALLO),
            Falla::NoContesta(_, reg) => sec_no(SEC_NO_CONTESTA, reg, IOMMU_NO_SEC_FALLO),
            Falla::Falcon(_, e) => sec_no(SEC_FALCON, pr::motivo(e) as u32, IOMMU_NO_SEC_FALLO),
            Falla::Sec2(m0) => sec_no(SEC_SEC2, m0, IOMMU_NO_SEC_FALLO),
        },
    }
}

/// `NV_VIRTUAL_FUNCTION_PRIV_FUNC_BAR1_BLOCK` y `_BAR2_BLOCK` (Turing en
/// adelante: nouveau `tu102_bar_bar1_init`, 0xB80F40 y 0xB80F48). El bit 31
/// dice que la BAR es VIRTUAL, por las tablas de pagina de alguien.
const BAR1_BLOCK: u32 = 0x00B8_0F40;
const BAR2_BLOCK: u32 = 0x00B8_0F48;

/// Si se esta enlazando BAR1: los bits 0..1 (nouveau `tu102_bar_bar1_wait`).
const BAR_ENLACE: u32 = 0x00B8_0F50;
/// `BAR1_BLOCK` antes del secuenciador; bit 63 = apuntado.
static BAR1_ANTES: AtomicU64 = AtomicU64::new(0);
/// No hay BAR1 que devolver: el GSP-RM no arranco, o no se apunto la de antes.
pub const IOMMU_NO_BAR1: u32 = 53;

/// **Devolverle BAR1 a la pantalla** (L0c4b3a): escribir en `BAR1_BLOCK` el
/// valor que tenia ANTES del secuenciador y esperar a que se enlace (2 ms).
/// `Ok(el de ahora | el que habia puesto el GSP-RM << 32)`; si ya era el de
/// antes, no se escribe nada.
///
/// ** Por que (metal 24-09 09:54): tras `GSP_INIT_DONE` la pantalla se quedo
/// quieta con la CPU viva, y la copia al GOP paso de ~3000 a 381 ps/pixel. El
/// GOP se pinta por BAR1, y el GSP-RM la toma al arrancar. Mientras BMO-X no le
/// pida al GSP-RM su propio hueco de BAR1 (L1), se le devuelve la del GOP: el
/// GSP-RM, parado esperando RPC, no la esta usando. Solo se escribe el valor
/// que el mismo registro tenia: nada que venga del escritorio.
pub fn devolver_bar1() -> Result<u64, u32> {
    let a = BAR1_ANTES.load(Ordering::Acquire);
    if SEC.load(Ordering::Acquire) >> 24 & SEC_HECHO == 0 || a >> 63 == 0 {
        return Err(IOMMU_NO_BAR1);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return Err(io::IOMMU_NO_SIN_GPU);
    }
    let mut r = Bar0(bar0);
    let (antes, ahora) = (a as u32, r.leer(BAR1_BLOCK));
    if ahora == antes {
        return Ok(ahora as u64 | (ahora as u64) << 32);
    }
    r.escribir(BAR1_BLOCK, antes);
    let mut t = pr::reloj();
    let fin = fa::Reloj::us(&mut t) + 2000;
    while r.leer(BAR_ENLACE) & 3 != 0 && fa::Reloj::us(&mut t) < fin {}
    crate::ring0::cabina::count("gpu", "L0c4b3a: BAR1 devuelta a la pantalla; la del GSP-RM era", ahora as u64);
    Ok(r.leer(BAR1_BLOCK) as u64 | (ahora as u64) << 32)
}

/// `INFO_GPU_DESPIERTO_BUZON` con selector 3: `BAR1_BLOCK | BAR2_BLOCK << 32`,
/// en vivo; solo lectura.
///
/// ** Lo trajo el metal (24-09 09:54): tras `GSP_INIT_DONE` la pantalla se
/// quedo QUIETA con la CPU viva, y la copia al GOP paso de ~3000 a 381
/// ps/pixel. El GOP se pinta por BAR1: si el GSP-RM la puso virtual, lo que
/// pinta la CPU ya no cae donde mira la pantalla. Esto lo dice antes y despues.
pub fn info_bar1() -> u64 {
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return 0;
    }
    let mut r = Bar0(bar0);
    r.leer(BAR1_BLOCK) as u64 | (r.leer(BAR2_BLOCK) as u64) << 32
}

/// `INFO_GPU_DESPIERTO_BUZON` con selector 2: `i | fase << 16 | como va << 24
/// | dato << 32` -- `i` la orden en curso (o la que fallo), y el dato de la
/// falla (el registro, o MAILBOX0 del SEC2, o el motivo del falcon).
pub fn info_secuencia() -> u64 {
    SEC.load(Ordering::Acquire)
}
