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
/// La WPR2 ya empieza mas abajo que la de FWSEC-FRTS antes de nuestro
/// booter: eso solo lo hace un booter, y otro no cargaria.
pub const IOMMU_NO_GPU_CALIENTE: u32 = 67;
/// E7 (`junto`): FWSEC-FRTS, corrido DENTRO de `despertar`, no se paro en 3
/// s, o se paro con su buzon o su codigo distintos de 0, o sin la WPR2.
pub const IOMMU_NO_FWSEC_JUNTO: u32 = 95;

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

// == G0: LAS PRUEBAS CONTRA EL 0x15 (17-10, GCB2c de PLAN_GSP_CONTROL_BMO) ==
//
// Diferencias con OpenRM que se prueban una a una sin recompilar: el
// propietario escribe sus palabras en `g0.txt` (o `gpu g0 ...`) y el
// siguiente arranque las aplica. Sin el fichero, el camino de siempre.
// (E2, `sin-d`, se retiro tras probarse: el 0x15 salio igual.)
//
//    riscv     E3: el GSP como OpenRM 570.144 (kgspBootstrap_TU102)
//    seguido   E4: el booter cargado antes de despertar, sin hueco
//    reset     E5: el RESET de GSP y SEC2 con diez lecturas por flanco
//    ptimer    E6: PTIMER en hora antes de FWSEC (tmrSetCurrentTime_GV100)
//    junto     E7: FWSEC, el GSP y el booter en UNA llamada (kgspBootstrap_TU102)
//
// ** DESDE EL 20-10, SIN `g0.txt` CORRE OpenRM ENTERO (`g0::OPENRM`: riscv,
// seguido, reset y junto). El propietario: *"inspirar por completo el
// OpenRM de ULTRA"* -- el driver con el que su 3060 arranca en ULTRA LinuX.
// `gpu g0 normal` (un `g0.txt` sin palabras) vuelve al camino de antes.

/// Donde vive la eleccion. 8.3, junto a `modo.txt`.
pub const RUTA_G0: &str = "informe/estado/g0.txt";
/// Lo pedido y lo usado (`bmo_nvidia_gsp_control::g0`).
static G0: AtomicU64 = AtomicU64::new(0);

/// **Lo que dice `g0.txt`**, una vez: la fase de arranque lo lee con los
/// datos montados y pasa sus bytes (`None` si no esta). El driver no abre
/// ficheros: L8b, `dev` no conecta con `fsys`.
pub fn fijar_g0(texto: Option<&[u8]>) {
    use bmo_nvidia_gsp_control::g0;
    let mut v = g0::MIRADO;
    match texto {
        Some(t) => v |= g0::FICHERO | g0::leer(t),
        None => v |= g0::OPENRM,
    }
    G0.store(v, Ordering::Release);
    crate::ring0::cabina::info("gpu", "G0: lo que corre (bit 0 riscv, 1 seguido, 3 reset, 4 ptimer, 5 junto; 62 hay g0.txt; sin el, OpenRM entero)", v);
}

fn g0_pide(bit: u64) -> bool {
    G0.load(Ordering::Acquire) & bit != 0
}

/// E5: como se sostiene el RESET de los falcons en el camino del arranque
/// (FWSEC, GSP y booter). Con `reset`, como OpenRM; lo apunta como USADO.
pub fn g0_pulso() -> fa::Pulso {
    use bmo_nvidia_gsp_control::g0;
    if g0_pide(g0::RESET) {
        G0.fetch_or(g0::RESET_USADO, Ordering::AcqRel);
        fa::Pulso::Lecturas
    } else {
        fa::Pulso::Espera
    }
}

/// E6: PTIMER en hora, con la del reloj de la placa, antes de FWSEC-FRTS.
/// Solo con `ptimer`; si el PLM no deja, no se escribe y la caja negra lo dice.
pub(crate) fn g0_ptimer(r: &mut Bar0) {
    use bmo_nvidia_gsp_control::g0;
    if !g0_pide(g0::PTIMER) {
        return;
    }
    let Some(f) = bmo_rtc::desempaquetar(crate::ring0::dev::clock::ahora()) else {
        crate::ring0::cabina::warn("gpu", "G0 E6: sin hora de la placa; PTIMER no se toca", 0);
        return;
    };
    let ns = bmo_gpu_ga10x::arranque::ptimer::ns_desde_1970(f.anio, f.mes, f.dia, f.hora, f.minuto, f.segundo);
    if bmo_gpu_ga10x::arranque::ptimer::poner_hora(r, ns) {
        G0.fetch_or(g0::PTIMER_USADO, Ordering::AcqRel);
        crate::ring0::cabina::count("gpu", "G0 E6: PTIMER en hora como OpenRM; segundos desde 1970", ns / 1_000_000_000);
    } else {
        crate::ring0::cabina::warn("gpu", "G0 E6: el PLM de PTIMER no deja escribir al nivel 0; no se toca", 0);
    }
}

/// E4: `despertar` sigue con el booter en la misma llamada.
pub fn g0_seguido() -> bool {
    g0_pide(bmo_nvidia_gsp_control::g0::SEGUIDO)
}

/// E7: y ademas FWSEC-FRTS va dentro de esa llamada, delante.
pub fn g0_junto() -> bool {
    g0_pide(bmo_nvidia_gsp_control::g0::JUNTO) && g0_seguido()
}

/// `INFO_GPU_G0`: lo pedido (0..7), lo usado (8..15), palabras desconocidas
/// (16..23), hay fichero (62) y se miro (63).
pub fn info_g0() -> u64 {
    G0.load(Ordering::Acquire)
}
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
    antes_con(true)
}

/// `antes`, sin pedir la WPR2 si `con_wpr2` es falso: E7 corre FWSEC-FRTS
/// en la misma llamada, asi que al entrar aun no esta.
fn antes_con(con_wpr2: bool) -> Result<Bar0, u32> {
    let bar0 = pr::candado_dma()?;
    let wpr2 = (crate::ring0::dev::gpu::info_wpr2() >> 32) as u32 >> 4 != 0;
    let g = gpu_gsp::info_gsp();
    // La version emparejada: CUADRA y es una version conocida (570 o 615).
    let radix = g & gpu_gsp::GSP_CUADRA != 0 && gpu_gsp::version().is_some();
    let libos = gpu_libos::info_libos() & gpu_libos::LIBOS_COMPROBADO != 0;
    if gpu_gsp::app_version().is_none() || (con_wpr2 && !wpr2) || !radix || !libos {
        return Err(IOMMU_NO_DESPERTAR_ANTES);
    }
    Ok(Bar0(bar0))
}

/// **DESPERTAR**: 0 y 1 del orden. `Ok(IOVA de los argumentos)` en cuanto el
/// GSP arranca; si se paro, lo dice `INFO_GPU_DESPIERTO`.
/// Si el camino del booter ya empezo en este arranque (el GSP o el SEC2
/// arrancados): desde ahi, nadie resetea el falcon del GSP.
pub fn gsp_tocado() -> bool {
    ESTADO.load(Ordering::Acquire) & (DESPIERTO_GSP_ARRANCADO | DESPIERTO_SEC2_ARRANCADO) != 0
}

pub fn despertar() -> Result<u64, u32> {
    if ESTADO.load(Ordering::Acquire) & DESPIERTO_SEC2_ARRANCADO != 0 {
        // El booter ya corrio en este arranque: la WPR2 ya tiene el GSP-RM, y
        // correrlo otra vez sin el booter_unload no se hace. El motivo de
        // ANTES se queda (metal 24-09 13:52: el 45 de la repeticion de `save
        // mode` tapaba el 0x15 del booter).
        return Err(IOMMU_NO_YA_DESPIERTO);
    }
    let mut r = match antes() {
        Ok(r) => r,
        Err(m) => return no(m),
    };
    // ** LA WPR2 YA EXTENDIDA antes de nuestro booter: solo un booter la baja
    // de `frts.desde`, asi que alguien lo corrio antes y el nuestro no
    // cargaria. OJO: esto NO explica el 0x15 del metal (24-09 13:52): ese dia
    // FWSEC-FRTS corrio -- y solo corre con la WPR2 vacia --, asi que la
    // tarjeta llego limpia. El 0x15 sigue sin causa conocida.
    let fb = crate::ring0::dev::gpu::info_fb();
    let frts = bmo_gpu_ga10x::vbios::frts(
        fb as u32,
        crate::ring0::dev::gpu::info_vga() as u32,
        fb & crate::ring0::dev::gpu::GPU_FB_SIN_PANTALLA == 0,
    );
    let abajo = ((crate::ring0::dev::gpu::info_wpr2() as u32 >> 4) as u64) << 12;
    if abajo != 0 && abajo < frts.desde {
        crate::ring0::cabina::warn("gpu", "L0c3b: la WPR2 ya viene EXTENDIDA de un arranque anterior; empieza en", abajo);
        return no(IOMMU_NO_GPU_CALIENTE);
    }
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
    foto_preparacion(&mut r, 1);
    // 1. El GSP, con sus argumentos de LIBOS en el buzon, arranca (y se para
    // solo: aun no tiene codigo; el que lo pondra es el booter).
    crate::ring0::cabina::info("gpu", "L0c3b: el GSP con sus argumentos de LIBOS en el buzon", gpu_libos::IOVA_LIBOS);
    let mut t = pr::reloj();
    let boot0 = r.leer(bmo_gpu_ga10x::BOOT_0);
    let l = gpu_libos::IOVA_LIBOS;
    let hecho = if g0_pide(bmo_nvidia_gsp_control::g0::RISCV) {
        // ** E3, G0-MODO (17-10): lo que hace OpenRM 570.144 en
        // `kgspBootstrap_TU102` -- `kflcnResetIntoRiscv` (reset y BCR_CTRL =
        // RISC-V, VALID, BRFETCH) y los argumentos de LIBOS en los buzones
        // (`kgspProgramLibosBootArgsAddr`), SIN arrancar el falcon vacio.
        crate::ring0::cabina::info("gpu", "G0 E3: el GSP como OpenRM 570.144 -- nucleo RISC-V y buzones, sin STARTCPU", 0);
        fa::resetear_en_riscv_con(&mut r, &mut t, fa::GSP, g0_pulso()).map(|_| {
            foto_preparacion(&mut r, 2);
            r.escribir(fa::GSP + fa::MAILBOX0, l as u32);
            r.escribir(fa::GSP + fa::MAILBOX1, (l >> 32) as u32);
            foto_preparacion(&mut r, 3);
            G0.fetch_or(bmo_nvidia_gsp_control::g0::RISCV_USADO, Ordering::AcqRel);
        })
    } else {
        fa::resetear_con(&mut r, &mut t, fa::GSP, boot0, g0_pulso())
            .and_then(|_| fa::arrancar_con(&mut r, fa::GSP, None, Some(l as u32), Some((l >> 32) as u32)))
    };
    if let Err(e) = hecho {
        crate::ring0::cabina::warn("gpu", "L0c3b: el falcon del GSP no dejo resetearse; motivo", pr::motivo(e));
        return no(IOMMU_NO_GSP_FALCON);
    }
    apuntar(|x| x | DESPIERTO_GSP_ARRANCADO);
    GSP_TSC.store(crate::ring0::task::scheduler::rdtsc(), Ordering::Release);
    Ok(l)
}

/// G0, observacion antes de STARTCPU del SEC2: 1 tras FWSEC, 2 tras BCR,
/// 3 tras LIBOS. No interpreta un PRI Falcon como fallo del nucleo RISC-V.
/// No modifica la receta ni convierte estas lecturas en nuevas condiciones.
fn foto_preparacion(r: &mut Bar0, fase: u64) {
    let desde = crate::ring0::task::scheduler::rdtsc();
    let f = fa::foto_nucleos(r, fa::GSP);
    let hasta = crate::ring0::task::scheduler::rdtsc();
    let c = |s: &'static str, v| crate::ring0::cabina::info("gpu", s, v);
    c("G0 foto preparacion: fase (1 FWSEC, 2 BCR, 3 LIBOS)", fase);
    c("G0 foto: TSC antes de las lecturas", desde);
    c("G0 foto: TSC despues de las lecturas", hasta);
    c("G0 foto: BCR | RISCV_CPUCTL << 32, crudos", f.bcr as u64 | (f.riscv_cpuctl as u64) << 32);
    c("G0 foto: FALCON_CPUCTL crudo", f.falcon_cpuctl as u64);
    c("G0 foto: MAILBOX0 | MAILBOX1 << 32, crudos", f.mailbox0 as u64 | (f.mailbox1 as u64) << 32);
    // Bits separados de validez y valor: un PRI no se disfraza de cero medido.
    let mut v = 0;
    if let Some(riscv) = f.seleccionado() { v |= 1 | (riscv as u64) << 1; }
    if let Some((activo, parado)) = f.actividad() { v |= 4 | (activo as u64) << 3 | (parado as u64) << 4; }
    if let Some(args) = f.argumentos() { v |= 32 | ((args == gpu_libos::IOVA_LIBOS) as u64) << 6; }
    c("G0 foto: BCR valido/seleccion, RV valido/activo/halt, args validos/iguales (bits 0..6)", v);
}

/// **E4, SEGUIDO** (17-10): `despertar` y el booter en la MISMA llamada, como
/// OpenRM y nova-core, sin volver a Ring 3 en medio (el hueco medido era de
/// 120 ms). El fichero del booter llega ya abierto, antes de despertar.
pub fn despertar_seguido(booter_fichero: Option<&mut dyn Fichero>) -> Result<u64, u32> {
    // Metal 17-10 09:04: con solo juntar las dos llamadas el hueco seguia en
    // 118 ms -- no era Ring 3, era el booter: leerlo, firmarlo, copiarlo al
    // SEC2 y la huella de los 61 MB que vigila. Todo eso va ANTES de tocar el
    // GSP; entre el GSP arrancado y el booter suelto solo queda `wbinvd`.
    if ESTADO.load(Ordering::Acquire) & DESPIERTO_SEC2_ARRANCADO != 0 {
        return Err(IOMMU_NO_YA_DESPIERTO);
    }
    let mut r = match antes() {
        Ok(r) => r,
        Err(m) => return no(m),
    };
    let cargado = cargar_booter(&mut r, booter_fichero)?;
    let vigia = huellas();
    despertar()?;
    if !g0_pide(bmo_nvidia_gsp_control::g0::RISCV_USADO) {
        // El falcon vacio se para en microsegundos; 10 ms de techo.
        let mut t = pr::reloj();
        use bmo_gpu_ga10x::falcon::Reloj;
        let fin = t.us() + 10_000;
        while !matches!(fa::como_va(&mut r, fa::GSP), Ok((true, _, _))) {
            if t.us() >= fin {
                return no(IOMMU_NO_GSP_FALCON);
            }
        }
        apuntar(|x| x | DESPIERTO_GSP_PARADO);
    }
    G0.fetch_or(bmo_nvidia_gsp_control::g0::SEGUIDO_USADO, Ordering::AcqRel);
    crate::ring0::cabina::info("gpu", "G0 E4: el booter ya en el SEC2 antes de despertar; se suelta seguido", 0);
    soltar(&mut r, cargado, vigia)
}

/// **E7, JUNTO** (20-10, PLAN_GPU_COMO_LINUX 4b): FWSEC-FRTS, el GSP y el
/// booter en UNA llamada, en el orden de OpenRM (`kgspBootstrap_TU102`):
///
/// ```text
///    antes      el booter leido y firmado, y la huella de los 61 MB (lo lento)
///    1. FWSEC   en el falcon del GSP; se ESPERA aqui a que pare, con la WPR2
///    2. el GSP  reset a RISC-V y LIBOS en los buzones (`despertar`, E3)
///    3. SEC2    reset, el booter por DMA, el BROM (`kgspExecuteBooterLoad`)
///    4. soltar  wbinvd y STARTCPU del SEC2
/// ```
///
/// Sin Ring 3 ni un `save` en medio: los ~522 ms de FWSEC al booter se
/// quedan en lo que tarda FWSEC. Si la WPR2 ya estaba (`gpu fwsec` a mano),
/// se sigue sin FWSEC y sin apuntar E7 como usado.
pub fn fwsec_y_despertar(booter_fichero: Option<&mut dyn Fichero>) -> Result<u64, u32> {
    use bmo_gpu_ga10x::falcon::Reloj;
    use bmo_nvidia_gsp_control::g0;
    if ESTADO.load(Ordering::Acquire) & DESPIERTO_SEC2_ARRANCADO != 0 {
        return Err(IOMMU_NO_YA_DESPIERTO);
    }
    let wpr2_ya = (crate::ring0::dev::gpu::info_wpr2() >> 32) as u32 >> 4 != 0;
    let mut r = match antes_con(wpr2_ya) {
        Ok(r) => r,
        Err(m) => return no(m),
    };
    let leido = leer_booter(&mut r, booter_fichero)?;
    let vigia = huellas();
    if wpr2_ya {
        crate::ring0::cabina::warn("gpu", "G0 E7: la WPR2 ya estaba (FWSEC corrio aparte): sin junto", 0);
    } else {
        // 1. FWSEC-FRTS, y su final esperado AQUI, como `kgspExecuteFwsec`.
        if let Err(m) = pr::fwsec_correr() {
            return no(m);
        }
        let mut t = pr::reloj();
        let desde = t.us();
        loop {
            if let Ok((true, _, _)) = fa::como_va(&mut r, fa::GSP) {
                break;
            }
            if t.us() >= desde + 3_000_000 {
                return no(IOMMU_NO_FWSEC_JUNTO);
            }
        }
        // `info_fwsec` lo lee como lo leia el director: su foto y su autopsia.
        let f = pr::info_fwsec();
        let buzon = pr::info_fwsec_buzon() as u32;
        let wpr2 = (crate::ring0::dev::gpu::info_wpr2() >> 32) as u32 >> 4 != 0;
        if buzon != 0 || (f >> pr::FWSEC_ERROR_SHIFT) & 0xFFFF != 0 || !wpr2 {
            return no(IOMMU_NO_FWSEC_JUNTO);
        }
        G0.fetch_or(g0::JUNTO_USADO, Ordering::AcqRel);
        crate::ring0::cabina::count("gpu", "G0 E7: FWSEC-FRTS corrio dentro de despertar y monto la WPR2; us", t.us() - desde);
    }
    // 2. El GSP como OpenRM (con `riscv`, sin arrancarlo).
    despertar()?;
    if !g0_pide(g0::RISCV_USADO) {
        let mut t = pr::reloj();
        let fin = t.us() + 10_000;
        while !matches!(fa::como_va(&mut r, fa::GSP), Ok((true, _, _))) {
            if t.us() >= fin {
                return no(IOMMU_NO_GSP_FALCON);
            }
        }
        apuntar(|x| x | DESPIERTO_GSP_PARADO);
    }
    // 3 y 4. El booter al SEC2 y soltarlo.
    let cargado = meter_booter(&mut r, leido)?;
    G0.fetch_or(g0::SEGUIDO_USADO, Ordering::AcqRel);
    soltar(&mut r, cargado, vigia)
}

/// **BOOTER**: 2 del orden, con el GSP ya parado. `Ok(la firma usada)` en
/// cuanto el SEC2 arranca; si se paro, lo dice `INFO_GPU_DESPIERTO`.
pub fn booter(booter_fichero: Option<&mut dyn Fichero>) -> Result<u64, u32> {
    let e = ESTADO.load(Ordering::Acquire);
    if e & DESPIERTO_SEC2_ARRANCADO != 0 {
        return Err(IOMMU_NO_YA_DESPIERTO);
    }
    let mut r = match antes() {
        Ok(r) => r,
        Err(m) => return no(m),
    };
    // E3: con el GSP preparado como OpenRM no hay falcon arrancado que
    // esperar parado; el booter va directo.
    let riscv = g0_pide(bmo_nvidia_gsp_control::g0::RISCV_USADO);
    if e & DESPIERTO_GSP_ARRANCADO == 0 || (!riscv && !matches!(fa::como_va(&mut r, fa::GSP), Ok((true, _, _)))) {
        return no(IOMMU_NO_GSP_FALCON);
    }
    // Se paro: queda dicho. Luego el RISC-V lo arranca otra vez y en vivo ya
    // no se ve (metal 24-09 07:10: la fila decia `-gsp` con el GSP despierto).
    if !riscv {
        apuntar(|x| x | DESPIERTO_GSP_PARADO);
    }

    let cargado = cargar_booter(&mut r, booter_fichero)?;
    soltar(&mut r, cargado, huellas())
}

/// Lo que deja `cargar_booter`: el booter firmado y ya en el SEC2, sin soltar.
struct Cargado {
    idx: u32,
    arranque: u32,
    hs: super::gpu_hs::Tramo,
}

/// El booter leido, firmado con la firma del fusible, prestado y copiado al
/// SEC2. No toca el GSP: E4 lo hace antes de despertarlo.
fn cargar_booter(r: &mut Bar0, booter_fichero: Option<&mut dyn Fichero>) -> Result<Cargado, u32> {
    let l = leer_booter(r, booter_fichero)?;
    meter_booter(r, l)
}

/// Lo que deja `leer_booter`: el booter firmado y prestado, aun FUERA del SEC2.
struct Leido {
    b: booter::Booter,
    idx: u32,
    fusible: u32,
    crudo: u32,
}

/// Lo lento del booter (E7 lo hace antes de FWSEC): leerlo del disco,
/// firmarlo con la firma del fusible y prestarlo. No toca ningun falcon.
fn leer_booter(r: &mut Bar0, booter_fichero: Option<&mut dyn Fichero>) -> Result<Leido, u32> {
    let no = |m: u32| no(m).and(Err::<Leido, u32>(m));
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
    if !b.carga_valida(UCODE_BYTES, 65536, 65536) {
        return no(IOMMU_NO_BOOTER);
    }
    let Some(reg) = vb::registro_fusible(b.engine_id_mask, b.ucode_id) else { return no(IOMMU_NO_BOOTER_FIRMA) };
    let crudo = r.leer(reg);
    let Some(idx) = b.indice_de_firma(crudo) else { return no(IOMMU_NO_BOOTER_FIRMA) };
    ucode[..b.bin.bytes as usize].copy_from_slice(b.bin.ucode(fichero));
    if booter::firmar(fichero, &b, idx, ucode).is_err() {
        return no(IOMMU_NO_BOOTER_FIRMA);
    }
    // Prestado UNA vez por arranque: prestar encima se negaria.
    if !BOOTER_PRESTADO.swap(true, Ordering::AcqRel) {
        if let Err(m) = io::prestar_gpu(IOVA_BOOTER, base, UCODE_BYTES / PAGINA, false) {
            BOOTER_PRESTADO.store(false, Ordering::Release);
            return no(m);
        }
    }
    apuntar(|v| (v & !(3 << DESPIERTO_FIRMA_SHIFT)) | DESPIERTO_BOOTER | (idx as u64 & 3) << DESPIERTO_FIRMA_SHIFT);
    Ok(Leido { b, idx: idx as u32, fusible: reg, crudo })
}

/// El booter ya leido, al SEC2 (como `kgspExecuteBooterLoad_TU102`: reset
/// del SEC2, FBIF, IMEM y DMEM por DMA, y los registros del BROM). Rapido: E7
/// lo hace entre el GSP preparado y STARTCPU.
fn meter_booter(r: &mut Bar0, l: Leido) -> Result<Cargado, u32> {
    let no = |m: u32| no(m).and(Err::<Cargado, u32>(m));
    let (b, idx) = (l.b, l.idx);
    let hs = super::gpu_hs::Tramo::nuevo(bmo_gpu_ga10x::arranque::hs_observacion::Motor::Sec2,
        "HS entrada: BOOTER_LOAD; id", idx, l.fusible, l.crudo);
    hs.dato("HS entrada: ucode bytes", b.bin.bytes as u64);
    hs.huella(memoria(BOOTER_F.load(Ordering::Acquire), b.bin.bytes as u64));
    hs.dato("HS entrada: firma DMEM offset | bytes << 32", b.pkc_data_offset() as u64 | (b.firma_bytes as u64) << 32);

    // 2. El booter en el SEC2, con la WPR meta en el buzon.
    crate::ring0::cabina::info("gpu", "L0c3b: el booter al SEC2; la WPR meta en", IOVA_WPR_META);
    let mut t = pr::reloj();
    let boot0 = r.leer(bmo_gpu_ga10x::BOOT_0);
    let a256 = |n: u32| n.next_multiple_of(256);
    let cargado = hs.ejecutar(r, "HS fase: reset SEC2; id", |r| fa::resetear_con(r, &mut t, fa::SEC2, boot0, g0_pulso()))
        .and_then(|_| hs.ejecutar(r, "HS fase: validar TCM y preparar FBIF; id", |r| {
            let caps = fa::ficha(r, fa::SEC2)?;
            if !b.carga_valida(UCODE_BYTES, caps.imem, caps.dmem) {
                return Err(fa::NoFuego::DmaFuera);
            }
            crate::ring0::cabina::info("gpu", "G0 contrato 615: carga HS validada contra IMEM/DMEM SEC2", caps.imem as u64 | (caps.dmem as u64) << 32);
            fa::preparar_fbif(r, fa::SEC2)
        }))
        .and_then(|_| {
            hs.dma(r, &mut t, IOVA_BOOTER, b.imem.origen, b.imem.destino, a256(b.imem.bytes), true, true)
        })
        .and_then(|_| {
            hs.dma(r, &mut t, IOVA_BOOTER + b.dmem.origen as u64, 0, b.dmem.destino, a256(b.dmem.bytes), false, false)
        });
    if let Err(e) = cargado {
        crate::ring0::cabina::warn("gpu", "L0c3b: el SEC2 no dejo cargar el booter; motivo", pr::motivo(e));
        return no(IOMMU_NO_SEC2);
    }
    fa::brom(r, fa::SEC2, b.pkc_data_offset(), b.engine_id_mask, b.ucode_id);
    hs.paso("HS fase: BROM programado, autenticacion aun no acreditada; id");
    Ok(Cargado { idx, arranque: b.arranque(), hs })
}

/// Suelta el booter ya cargado: la foto de antes, `wbinvd` y STARTCPU.
/// `vigia` es la huella de lo que la 3060 solo lee, tomada por quien llama.
fn soltar(r: &mut Bar0, c: Cargado, vigia: (u64, u64)) -> Result<u64, u32> {
    let idx = c.idx;
    let m = IOVA_WPR_META;
    // La foto [0] de la autopsia, y el reloj del booter.
    AUTOPSIA[0].store(crate::ring0::dev::gpu::info_bsi(), Ordering::Release);
    AUTOPSIA[8].store(bus_3060(), Ordering::Release);
    AUTOPSIA[10].store(crate::ring0::dev::gpu_prestamo::eventos(), Ordering::Release);
    // ** H3, LA PRUEBA DE LA CACHE (29-09, `PLAN_LA_3060.md`). El metal de
    // hoy dio el 0x15 con `GSP MAILBOX0 0xBADF1002` tres veces de cuatro, y
    // dos de ellas tras APAGAR con ErP: no es la tarjeta caliente. Lo que el
    // booter y el GSP leen de la RAM (la WPR meta, los argumentos de LIBOS,
    // la radix3 de 60 MiB) lo acaba de escribir la CPU, y el L3 son 32 MiB:
    // si la 3060 lee sin espiar la cache (no-snoop), puede leer lo VIEJO. Un
    // `wbinvd` escribe TODA la cache a la RAM antes de soltar al booter.
    // Es UN cambio, solo en este camino, y la fila `despierto` dira si el
    // 0x15 se va. Cuesta unos ms una vez por arranque.
    // SAFETY: Ring 0; `wbinvd` solo escribe y vacia caches, no toca memoria
    // que no sea suya ni registros.
    // EL VIGIA (29-09): la huella de lo que la 3060 solo lee, ANTES. Va
    // antes del `wbinvd` y del reloj: no retrasa al booter respecto a ellos.
    let (inmutable, libos) = vigia;
    AUTOPSIA[12].store(inmutable, Ordering::Release);
    AUTOPSIA[14].store(libos, Ordering::Release);
    crate::ring0::cabina::info("gpu", "L0c3b: el vigia: huella de lo que la 3060 solo lee, antes del booter", inmutable);
    unsafe { core::arch::asm!("wbinvd", options(nostack, preserves_flags)) };
    crate::ring0::cabina::info("gpu", "L0c3b: H3, la cache de la CPU escrita a la RAM (wbinvd) antes del booter", 0);
    let ahora = crate::ring0::task::scheduler::rdtsc();
    BOOTER_TSC.store(ahora, Ordering::Release);
    let intento = INTENTOS.fetch_add(1, Ordering::AcqRel) + 1;
    crate::ring0::cabina::count("gpu", "L0c3b: intento del booter (GCB2a)", intento);
    // [11] H4 y H5: cuanto espero el GSP parado, y el disco al soltarlo.
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let hueco = ahora.saturating_sub(GSP_TSC.load(Ordering::Acquire)) / hz;
    AUTOPSIA[11].store(hueco.min(0xFFFF_FFFF) | (crate::ring0::dev::disk::entradas_irq() as u64 & 0xFFFF) << 32, Ordering::Release);
    // [16] EL MOMENTO (29-09, el propietario: *"en que momento pide GSP para
    // arrancar?"*): los ms desde que arranco FWSEC-FRTS hasta soltar el
    // booter. En nova-core, milisegundos; en `save mode`, la copia de los 60
    // MB y un `save` por paso en medio.
    let frts = crate::ring0::dev::gpu_prestamo::frts_tsc();
    if frts != 0 {
        AUTOPSIA[16].store(ahora.saturating_sub(frts) / hz / 1000 | 1 << 63, Ordering::Release);
    }
    // G0: cuatro lecturas, sin reset ni escritura. La foto DESPUES ya existe.
    let probe = bmo_gpu_ga10x::arranque::boot_probe::BootProbe::read(r);
    AUTOPSIA[17].store(probe.mailboxes, Ordering::Release);
    AUTOPSIA[18].store(probe.controls, Ordering::Release);
    AUTOPSIA[19].store(gpu_libos::IOVA_LIBOS | 1 << 63, Ordering::Release);
    fotografiar_honda(r);
    super::gpu_capture::record(r, intento, bmo_nvidia_gsp_control::capture::BEFORE_BOOTER);
    c.hs.dato("HS entrada: intento booter al soltar", intento);
    c.hs.dato("HS entrada: BOOTVEC solicitado", c.arranque as u64);
    c.hs.dato("HS entrada: buzones SEC2 solicitados (WPR meta IOVA)", m);
    c.hs.foto(r, "HS foto externa previa a STARTCPU; id");
    if c.hs.ejecutar(r, "HS fase: solicitar STARTCPU SEC2; id", |r| fa::arrancar_con(r, fa::SEC2, Some(c.arranque), Some(m as u32), Some((m >> 32) as u32))).is_err() {
        return no(IOMMU_NO_SEC2);
    }
    c.hs.paso("HS fase: STARTCPU solicitado; no prueba autenticacion; id");
    apuntar(|x| x | DESPIERTO_SEC2_ARRANCADO);
    crate::ring0::cabina::count("gpu", "L0c3b: el BOOTER ARRANCADO en el SEC2; firma", idx as u64);
    Ok(idx as u64)
}

// == LA AUTOPSIA DEL BOOTER (26-09) ==========================================
//
// Cuatro 0x15 y dos hipotesis caidas (la WPR2 caliente; "reiniciaste sin
// cortar la corriente" cuando la tarjeta llego fria). Antes de cambiar nada,
// medir: tres fotos, SOLO lecturas, sin tocar el camino del booter.
//
//    [0]  justo ANTES de arrancar el booter: `gpu::info_bsi`
//    [1]  la PRIMERA vez que se ve el SEC2 parado: `info_bsi` | los us desde
//         el arranque del booter << 32 (resolucion: la de quien pregunta)
//    [2]  en ese instante: MAILBOX0 | MAILBOX1 << 32 del falcon del GSP (lo
//         escribimos con los argumentos de LIBOS; si esta a 0, alguien lo
//         borro: el GSP se reseteo o su FMC corrio -- nova-core: "GSP-FMC
//         normally clears the boot parameters address from the mailboxes")
//    [3]  en ese instante: la WPR2 (`INFO_GPU_WPR2`)
//    [4]  en ese instante: CPUCTL del GSP | CPUCTL del SEC2 << 32, CRUDOS
//    [5]  la WPR meta: que palabras cambio el booter (mascara de 32) | se
//         tomo << 63 -- HASTA DONDE LLEGO (`gpu_gsp::meta_cambios`)
//    [6]  su `verified` (byte 248)
//    [7]  su `bootCount` (byte 200)
//    [8]  el bus de la 3060 ANTES del booter: sus bits de error de PCI
//         (`metiche::una`), y [9] los mismos al pararse
//    [10] los eventos de la IOMMU antes | al pararse << 32
//
// ** [8..10] (26-09, metal 20:59 y 21:08): el metiche vio a la 3060 con
// `aborto-recibido` NUEVO en la sesion. `gpu frontera` lo provoca a proposito
// (el DMA a una direccion NO prestada) y corre ANTES del booter: si [8] ya lo
// trae, es nuestro y del booter no dice nada; si aparece entre [8] y [9], o
// suben los eventos de la IOMMU, el booter choco con el bus o con la IOMMU.
//
// Y todo va tambien a CABINA: queda en la caja negra y en el serie aunque
// nadie pegue el informe.
//
// ** CRUDOS (26-09): el 25-09 19:59 [2] salio 0xFFFFFFFF/0xFFFFFFFF, y ese
// era el centinela de "el registro contesto con error de PRI" -- el GSP no se
// dejaba leer al pararse el SEC2. El numero crudo (0xBADF....) dice POR QUE:
// ahora se guarda tal cual, sin juicio.
//
// La comparacion que decide: si los arranques que dan 0x15 traen el bit
// `BSI_HANDOFF` puesto ANTES del booter y los que dan 0 no, la causa es ese
// estado que sobrevive al reinicio, y el arreglo es exigirlo abajo (o
// reiniciar la tarjeta) antes de gastar el booter.

//    [11] (29-09) `hueco | disco << 32 | disco al pararse << 48`: los us que
//         el GSP espero PARADO entre `despertar` y el booter (H4: el hueco
//         lo pone Ring 3 y cambia de un arranque a otro, con un `save` en
//         medio), y las entradas del vector del DISCO al soltar el booter y
//         al verlo parado, 16 bits cada una (H5: el DMA del disco, por la
//         misma IOMMU, mientras el booter lee 60 MiB). Solo lecturas.
//
// ** Por que (29-09): H3 cayo y el 0x15 volvio SIN `fuego` ni `frontera`,
// con la tarjeta fria: 06:25 y 10:24 buenos, 10:09 y cuatro seguidos desde
// las 10:54 malos, con `al llegar` y `cargador` IGUALES. Lo que si cambia de
// un arranque a otro es el tiempo y el trafico de alrededor.

//    [12] y [13] (29-09, EL VIGIA) los 8 primeros bytes del BLAKE3 de TODO lo
//         que la 3060 solo lee -- el GSP-RM, su radix3, el bootloader, la
//         firma y el ucode del booter -- justo ANTES de soltar el booter, y
//         al verlo parado. [14] y [15], lo mismo de los argumentos de LIBOS
//         y `rmargs`. Iguales = la RAM no se movio mientras el booter
//         trabajaba; distintos = algo escribio donde nadie debia.

//    [16] (29-09) los ms desde que arranco FWSEC-FRTS hasta soltar el
//         booter, bit 63 = se tomo. EL MOMENTO.

// [17..19]: GSP MAILBOX0/1, CPUCTL/BCR_CTRL y LIBOS esperado | foto << 63.
static AUTOPSIA: [AtomicU64; 20] = [const { AtomicU64::new(0) }; 20];

// ** LA FOTO HONDA (17-10): E2..E6 no cambiaron el 0x15; lo que lo decide
// esta dentro de la tarjeta. Justo antes de soltar el booter, solo leido, lo
// que OpenRM 570.144 mira (`boot_probe::FotoHonda`): la limpieza de memoria
// de SEC2 y GSP, el DMA del SEC2, los ENGINE, PTIMER y si avanza, los ECC
// de la MMU y la WPR2. La causa sale de comparar buenos y malos.
// Selectores 24..31 de `INFO_GPU_DESPIERTO_BUZON`; el 32, si se tomo.
static HONDA: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];

fn fotografiar_honda(r: &mut Bar0) {
    let f = bmo_gpu_ga10x::arranque::boot_probe::FotoHonda::leer(r, &mut pr::reloj());
    for (h, v) in HONDA.iter().zip(f.0) {
        h.store(v, Ordering::Release);
    }
    HONDA[8].store(1, Ordering::Release);
    let c = |t: &'static str, v: u64| crate::ring0::cabina::count("gpu", t, v);
    c("  ...HONDA: HWCFG2 SEC2 | GSP << 32", f.0[0]);
    c("  ...HONDA: DMATRFCMD SEC2 | ENGINE SEC2 << 32", f.0[1]);
    c("  ...HONDA: ENGINE GSP | PLM PTIMER << 32", f.0[2]);
    c("  ...HONDA: PTIMER TIME_0 | TIME_1 << 32", f.0[3]);
    c("  ...HONDA: PTIMER avanzo en 10 us (ns)", f.0[4]);
    c("  ...HONDA: ECC L2TLB | HUBTLB << 32", f.0[5]);
    c("  ...HONDA: ECC FILLUNIT | WPR2_LO << 32", f.0[6]);
    c("  ...HONDA: WPR2_HI", f.0[7]);
}

/// El ucode del booter ya prestado a la 3060 en este arranque.
static BOOTER_PRESTADO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// **El vigia**: `(huella de lo que solo se lee, huella de LIBOS)`, los 8
/// primeros bytes de cada BLAKE3; 0 si esa parte aun no existe.
fn huellas() -> (u64, u64) {
    let ocho = |h: bmo_hash::Hasher| {
        let d = h.finalize();
        u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]) | 1
    };
    let mut h = bmo_hash::Hasher::new();
    let gsp = gpu_gsp::huella_prestada(&mut h);
    let base = BOOTER_F.load(Ordering::Acquire);
    if base != 0 {
        h.update(memoria(base, UCODE_BYTES));
    }
    let a = if gsp { ocho(h) } else { 0 };
    let mut h = bmo_hash::Hasher::new();
    let b = if gpu_libos::huella_argumentos(&mut h) { ocho(h) } else { 0 };
    (a, b)
}
/// Cuando arranco el falcon del GSP en `despertar` (TSC).
static GSP_TSC: AtomicU64 = AtomicU64::new(0);

/// El bus de la 3060 (`metiche::una`), 0 si no hay BDF.
fn bus_3060() -> u64 {
    crate::ring0::dev::gpu::bdf().map_or(0, |(b, d, f)| crate::ring0::dev::metiche::una(b, d, f))
}
static BOOTER_TSC: AtomicU64 = AtomicU64::new(0);
/// GCB2a: cuantas veces se SOLTO el booter desde que arranco la maquina. Es
/// del arranque, no del intento.
static INTENTOS: AtomicU64 = AtomicU64::new(0);

/// `INFO_GPU_INTENTO`: el numero del ultimo intento del booter; 0 = ninguno.
pub fn info_intento() -> u64 {
    INTENTOS.load(Ordering::Acquire)
}

/// La foto del SEC2 recien parado, UNA vez por arranque.
fn autopsiar(r: &mut Bar0) {
    super::gpu_hs::parado(r, bmo_gpu_ga10x::arranque::hs_observacion::Motor::Sec2);
    // El diario es por intento, independiente de la antigua primera autopsia.
    super::gpu_capture::record(r, info_intento(), bmo_nvidia_gsp_control::capture::HALTED);
    if AUTOPSIA[1].load(Ordering::Acquire) != 0 {
        return;
    }
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let us = crate::ring0::task::scheduler::rdtsc().saturating_sub(BOOTER_TSC.load(Ordering::Acquire)) / hz;
    use bmo_gpu_ga10x::Registros;
    let (gm0, gm1) = (r.leer(fa::GSP + fa::MAILBOX0), r.leer(fa::GSP + fa::MAILBOX1));
    AUTOPSIA[2].store(gm0 as u64 | (gm1 as u64) << 32, Ordering::Release);
    let (gc, sc) = (r.leer(fa::GSP + fa::CPUCTL), r.leer(fa::SEC2 + fa::CPUCTL));
    AUTOPSIA[4].store(gc as u64 | (sc as u64) << 32, Ordering::Release);
    if let Some((mascara, cuenta, verificado)) = gpu_gsp::meta_cambios() {
        AUTOPSIA[5].store(mascara as u64 | 1 << 63, Ordering::Release);
        AUTOPSIA[6].store(verificado, Ordering::Release);
        AUTOPSIA[7].store(cuenta, Ordering::Release);
    }
    // A la caja negra: sin depender de que alguien pegue el informe.
    let c = |t: &'static str, v: u64| crate::ring0::cabina::count("gpu", t, v);
    c("AUTOPSIA del booter: MAILBOX0 del SEC2", r.leer(fa::SEC2 + fa::MAILBOX0) as u64);
    c("  ...us desde que arranco (techo: lo que tardo alguien en mirar)", us);
    c("  ...BSI_14 antes | ahora << 32", AUTOPSIA[0].load(Ordering::Acquire) & 0xFFFF_FFFF | (crate::ring0::dev::gpu::info_bsi() & 0xFFFF_FFFF) << 32);
    c("  ...MAILBOX0 del GSP, crudo", gm0 as u64);
    c("  ...GSP MAILBOX0/1 ANTES del booter", AUTOPSIA[17].load(Ordering::Acquire));
    c("  ...GSP CPUCTL/BCR_CTRL ANTES del booter", AUTOPSIA[18].load(Ordering::Acquire));
    c("  ...CPUCTL GSP | SEC2 << 32, crudos", gc as u64 | (sc as u64) << 32);
    c("  ...la WPR meta: palabras que cambio el booter (mascara)", AUTOPSIA[5].load(Ordering::Acquire) & 0xFFFF_FFFF);
    c("  ...su verified", AUTOPSIA[6].load(Ordering::Acquire));
    AUTOPSIA[9].store(bus_3060(), Ordering::Release);
    let h = AUTOPSIA[11].load(Ordering::Acquire) & 0xFFFF_FFFF_FFFF;
    AUTOPSIA[11].store(h | (crate::ring0::dev::disk::entradas_irq() as u64 & 0xFFFF) << 48, Ordering::Release);
    c("  ...us del GSP parado antes del booter | disco al soltar << 32 | al pararse << 48", AUTOPSIA[11].load(Ordering::Acquire));
    // EL VIGIA, DESPUES: la misma huella, al ver el booter parado.
    let (inmutable, libos) = huellas();
    AUTOPSIA[13].store(inmutable, Ordering::Release);
    AUTOPSIA[15].store(libos, Ordering::Release);
    let igual = inmutable == AUTOPSIA[12].load(Ordering::Acquire) && libos == AUTOPSIA[14].load(Ordering::Acquire);
    if igual {
        c("  ...el vigia: lo que la 3060 solo lee sigue IGUAL que antes del booter", inmutable);
    } else {
        crate::ring0::cabina::fault("gpu", "L0c3b: el vigia: la RAM que lee el booter CAMBIO mientras trabajaba", inmutable);
    }
    let ev = AUTOPSIA[10].load(Ordering::Acquire) & 0xFFFF_FFFF;
    AUTOPSIA[10].store(ev | crate::ring0::dev::gpu_prestamo::eventos() << 32, Ordering::Release);
    c("  ...el bus de la 3060 antes del booter (metiche::una)", AUTOPSIA[8].load(Ordering::Acquire));
    c("  ...y al pararse", AUTOPSIA[9].load(Ordering::Acquire));
    c("  ...eventos de la IOMMU antes | al pararse << 32", AUTOPSIA[10].load(Ordering::Acquire));
    AUTOPSIA[3].store(crate::ring0::dev::gpu::info_wpr2(), Ordering::Release);
    AUTOPSIA[1].store(crate::ring0::dev::gpu::info_bsi() & 0xFFFF_FFFF | us.min(0x7FFF_FFFF) << 32 | 1 << 63, Ordering::Release);
}

/// Donde deja el build el booter de DESCARGA (L0c5).
pub const RUTA_DESCARGADOR: &str = "fw/gsp/boot_ul.bin";

/// **EL BOOTER DE DESCARGA** (L0c5, al APAGAR): el paso 5 de
/// `bmo_gpu_ga10x::descarga`, con el GSP-RM ya despedido y FWSEC-SB hecho
/// (lo comprueba `gpu_apagar`). El mismo camino que `booter` -- el mismo
/// bufer, ya prestado, la firma que pide el FUSIBLE --, con `boot_ul.bin` y
/// los dos buzones a 0xFF (`tu102_gsp_fini` de nouveau). `Ok(firma)` en
/// cuanto el SEC2 arranca; acaba con la WPR2 abajo.
pub fn descargador(fichero: Option<&mut dyn Fichero>) -> Result<u64, u32> {
    use bmo_gpu_ga10x::descarga as dc;
    let e = ESTADO.load(Ordering::Acquire);
    // Solo se descarga lo que ESTE arranque cargo: el bufer y su prestamo son
    // los del booter de subida.
    if e & DESPIERTO_BOOTER == 0 || e & DESPIERTO_SEC2_ARRANCADO == 0 {
        return Err(IOMMU_NO_DESPERTAR_ANTES);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    let base = BOOTER_F.load(Ordering::Acquire);
    if bar0 == 0 || base == 0 {
        return Err(IOMMU_NO_DESPERTAR_ANTES);
    }
    let mut r = Bar0(bar0);
    let Some(f) = fichero else { return Err(IOMMU_NO_BOOTER) };
    let todo = memoria(base, BOOTER_PAGINAS * PAGINA);
    todo.fill(0);
    let (ucode, fichero) = todo.split_at_mut(UCODE_BYTES as usize);
    let medida = f.medida();
    if medida > fichero.len() as u64 || f.leer(0, &mut fichero[..medida as usize]) != medida as usize {
        return Err(IOMMU_NO_BOOTER);
    }
    let fichero = &fichero[..medida as usize];
    let Ok(b) = booter::booter(fichero) else { return Err(IOMMU_NO_BOOTER) };
    if !b.carga_valida(UCODE_BYTES, 65536, 65536) {
        return Err(IOMMU_NO_BOOTER);
    }
    let Some(reg) = vb::registro_fusible(b.engine_id_mask, b.ucode_id) else { return Err(IOMMU_NO_BOOTER_FIRMA) };
    let crudo = r.leer(reg);
    let Some(idx) = b.indice_de_firma(crudo) else { return Err(IOMMU_NO_BOOTER_FIRMA) };
    let hs = super::gpu_hs::Tramo::nuevo(bmo_gpu_ga10x::arranque::hs_observacion::Motor::Sec2,
        "HS entrada: BOOTER_UNLOAD; id", idx, reg, crudo);
    ucode[..b.bin.bytes as usize].copy_from_slice(b.bin.ucode(fichero));
    if booter::firmar(fichero, &b, idx, ucode).is_err() {
        return Err(IOMMU_NO_BOOTER_FIRMA);
    }
    hs.huella(&ucode[..b.bin.bytes as usize]);
    crate::ring0::cabina::info("gpu", "L0c5: el booter de DESCARGA al SEC2; firma", idx as u64);
    let mut t = pr::reloj();
    let boot0 = r.leer(bmo_gpu_ga10x::BOOT_0);
    let a256 = |n: u32| n.next_multiple_of(256);
    let cargado = hs.ejecutar(&mut r, "HS fase: reset SEC2 descarga; id", |r| fa::resetear(r, &mut t, fa::SEC2, boot0))
        .and_then(|_| hs.ejecutar(&mut r, "HS fase: validar TCM y preparar FBIF descarga; id", |r| {
            let caps = fa::ficha(r, fa::SEC2)?;
            if !b.carga_valida(UCODE_BYTES, caps.imem, caps.dmem) {
                return Err(fa::NoFuego::DmaFuera);
            }
            fa::preparar_fbif(r, fa::SEC2)
        }))
        .and_then(|_| {
            hs.dma(&mut r, &mut t, IOVA_BOOTER, b.imem.origen, b.imem.destino, a256(b.imem.bytes), true, true)
        })
        .and_then(|_| {
            hs.dma(&mut r, &mut t, IOVA_BOOTER + b.dmem.origen as u64, 0, b.dmem.destino, a256(b.dmem.bytes), false, false)
        });
    if let Err(e) = cargado {
        crate::ring0::cabina::warn("gpu", "L0c5: el SEC2 no dejo cargar el booter de descarga; motivo", pr::motivo(e));
        return Err(IOMMU_NO_SEC2);
    }
    fa::brom(&mut r, fa::SEC2, b.pkc_data_offset(), b.engine_id_mask, b.ucode_id);
    let m = dc::BUZON_DESCARGA;
    hs.foto(&mut r, "HS foto externa previa a STARTCPU descarga; id");
    if hs.ejecutar(&mut r, "HS fase: solicitar STARTCPU descarga; id", |r| fa::arrancar_con(r, fa::SEC2, Some(b.arranque()), Some(m), Some(m))).is_err() {
        return Err(IOMMU_NO_SEC2);
    }
    crate::ring0::cabina::count("gpu", "L0c5: el booter de DESCARGA ARRANCADO en el SEC2; firma", idx as u64);
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
    autopsiar(&mut r);
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
            // Solo en el arranque que cargo el booter, no tras la descarga.
            if parado && BOOTER_TSC.load(Ordering::Acquire) != 0 {
                autopsiar(&mut r);
            }
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
/// como va el secuenciador (L0c4b2c, `info_secuencia`); 3 BAR1; 4..14 la
/// AUTOPSIA DEL BOOTER (ver `autopsiar`); 21..23 la foto previa del GSP.
///
/// ** El SEC2 lo trajo el metal (24-09 07:48): el booter se paro con MAILBOX0
/// = 0x15 donde tres veces antes dio 0. nova-core imprime los DOS buzones al
/// fallar; aqui solo se veia el primero.
pub fn info_despierto_buzon(sel: u64) -> u64 {
    let kind = (sel >> 8) & 255;
    let base = bmo_nvidia_gsp_control::capture::SELECTOR;
    if kind == base || kind == base + 1 || kind == base + 3 { return super::gpu_capture::info(sel); }
    if kind == base + 2 {
        // Director pide la foto al vencer SU espera. Solo si un booter se
        // solto y han transcurrido al menos sus 5 s; no cambia ese plazo.
        let start = BOOTER_TSC.load(Ordering::Acquire);
        let hz = crate::ring0::task::scheduler::tsc_freq();
        let bar = super::gpu::bar0();
        if ESTADO.load(Ordering::Acquire) & DESPIERTO_SEC2_ARRANCADO != 0
            && start != 0 && bar != 0 && hz != 0
            && crate::ring0::task::scheduler::rdtsc().saturating_sub(start) >= hz.saturating_mul(5) {
            super::gpu_capture::record(&mut Bar0(bar), info_intento(), bmo_nvidia_gsp_control::capture::WAIT_EXPIRED);
        }
        return 0;
    }
    if sel >> 8 == 2 {
        return info_secuencia();
    }
    if sel >> 8 == 3 {
        return info_bar1();
    }
    if (4..=23).contains(&(sel >> 8)) {
        return AUTOPSIA[(sel >> 8) as usize - 4].load(Ordering::Acquire);
    }
    if (24..24 + HONDA.len() as u64).contains(&(sel >> 8)) {
        return HONDA[(sel >> 8) as usize - 24].load(Ordering::Acquire);
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
    // En 615 no hay secuenciador: el GSP-RM pide `EXEC_HS_BINARY` por GMCAPI (F2b).
    if gpu_gsp::version() == Some(gpu_gsp::Version::V615) {
        return super::gpu_615::paso();
    }
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

/// **BAR1 en modo FISICO** (el bit 31 de `BAR1_BLOCK` a 0, como la deja el
/// GOP y como la devuelve `devolver_bar1`): la direccion `d` de BAR1 es la
/// `d` de la VRAM. M5d P lo pide antes de pintar la pantalla por la GPU.
pub fn bar1_fisica() -> bool {
    let bar0 = crate::ring0::dev::gpu::bar0();
    bar0 != 0 && {
        let v = Bar0(bar0).leer(BAR1_BLOCK);
        !bmo_gpu_ga10x::es_error_pri(v) && v >> 31 == 0
    }
}

/// Si se esta enlazando BAR1: los bits 0..1 (nouveau `tu102_bar_bar1_wait`).
const BAR_ENLACE: u32 = 0x00B8_0F50;
/// `BAR1_BLOCK` antes del secuenciador; bit 63 = apuntado.
static BAR1_ANTES: AtomicU64 = AtomicU64::new(0);
/// Foto previa a GMCAPI 615; solo guarda un valor legible del aparato.
pub(super) fn guardar_bar1() -> bool {
    if BAR1_ANTES.load(Ordering::Acquire) != 0 { return true; }
    let bar = crate::ring0::dev::gpu::bar0();
    if bar == 0 { return false; }
    let v = Bar0(bar).leer(BAR1_BLOCK);
    if bmo_gpu_ga10x::es_error_pri(v) { return false; }
    BAR1_ANTES.store(1 << 63 | v as u64, Ordering::Release);
    true
}
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
    if info_secuencia() >> 24 & SEC_HECHO == 0 || a >> 63 == 0 {
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
    if gpu_gsp::version() == Some(gpu_gsp::Version::V615) { return super::gpu_615::info(); }
    SEC.load(Ordering::Acquire)
}
