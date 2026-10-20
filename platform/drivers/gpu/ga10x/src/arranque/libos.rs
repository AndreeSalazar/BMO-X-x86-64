//! **LO QUE EL GSP ESCRIBE EN LA RAM DEL PC (L0c3a)** -- sus argumentos de
//! LIBOS, sus tres logs, su `rmargs` y sus dos colas de mensajes, en los bytes
//! exactos de la version 570.144.
//!
//! capa: puro -- arma bytes y dice donde va cada cosa; no toca un registro (L8)
//!
//! [estado]  RAM           lo que el GSP escribe en la RAM del PC: argumentos, logs y colas
//! [eje]     CORRECCION -- un offset mal puesto aqui es un GSP que escribe su
//!           primer mensaje donde nadie lo va a buscar, o un fallo de pagina
//!
//! # La forma (nova-core de Linux 7.3: `gsp.rs`, `gsp/fw.rs`, `gsp/cmdq.rs`)
//!
//! ```text
//!    libos      1 pagina: `LibosMemoryRegionInitArgument` de 32 B cada uno
//!                 id8   el nombre al reves en un u64 ("LOGINIT", "RMARGS"...)
//!                 pa    donde lo vera el GSP (IOVA)
//!                 size  cuanto mide
//!                 kind  1 contiguo    loc  1 en la RAM del PC
//!    log x3     16 paginas: en +0 el puntero de escritura (lo mueve el GSP);
//!               desde +8, una entrada por pagina con su IOVA
//!    rmargs     1 pagina: `GSP_ARGUMENTS_CACHED` -- donde esta GspMem, cuantas
//!               paginas mide y donde empieza cada cola; `bDmemStack` = 1
//!    GspMem     129 paginas: la 0 es su tabla (una IOVA por pagina); luego la
//!               cola CPU->GSP y la GSP->CPU, cada una con su cabecera de
//!               32 + 4 B y 63 paginas de mensajes, alineada a pagina
//! ```
//!
//! [!] En `rmargs` los offsets de las colas se cuentan DESPUES de la pagina de
//! tabla (`POST_PTE_OFFSET` de nova-core): 0x1000 y 0x41000, no 0x2000 y
//! 0x42000.

pub const PAGINA: u64 = 4096;
/// `RM_LOG_BUFFER_NUM_PAGES`.
pub const LOG_PAGINAS: u64 = 0x10;
/// `MSGQ_NUM_PAGES`: los mensajes de una cola.
pub const MSGQ_PAGINAS: u64 = 0x3F;
/// Una cola: su pagina de cabeceras y sus mensajes.
pub const COLA_BYTES: u64 = (1 + MSGQ_PAGINAS) * PAGINA;
/// GspMem: la tabla, la cola del CPU y la del GSP.
pub const GSPMEM_PAGINAS: u64 = 1 + 2 * (1 + MSGQ_PAGINAS);
/// `CMDQ_OFFSET` y `STATQ_OFFSET`, contados tras la pagina de tabla.
pub const CMDQ_OFFSET: u64 = PAGINA;
pub const STATQ_OFFSET: u64 = PAGINA + COLA_BYTES;
/// Donde empieza cada cola dentro de GspMem.
pub const COLA_CPU: u64 = PAGINA;
pub const COLA_GSP: u64 = PAGINA + COLA_BYTES;
/// `offset_of!(Msgq, rx)`: detras de los 32 B de `msgqTxHeader`.
pub const RX_HDR_OFF: u32 = 32;

/// Lo que mide un `LibosMemoryRegionInitArgument`.
pub const ARGUMENTO: usize = 32;
/// `LIBOS_MEMORY_REGION_CONTIGUOUS` y `LIBOS_MEMORY_REGION_LOC_SYSMEM`.
pub const CONTIGUO: u8 = 1;
pub const EN_SYSMEM: u8 = 1;
/// Los cuatro, en el orden de nova-core (`Gsp::new`).
pub const NOMBRES: [&[u8]; 4] = [b"LOGINIT", b"LOGINTR", b"LOGRM", b"RMARGS"];

fn poner(b: &mut [u8], o: usize, v: &[u8]) {
    b[o..o + v.len()].copy_from_slice(v);
}
fn u32_de(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u64_de(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3], b[o + 4], b[o + 5], b[o + 6], b[o + 7]])
}

/// **El `ID8` de un nombre** (`id8` de nova-core): sus bytes AL REVES, el
/// ultimo en el byte bajo. "LOGINIT" = 0x004C4F47494E4954.
pub const fn id8(nombre: &[u8]) -> u64 {
    let mut v = 0u64;
    let mut i = 0;
    while i < nombre.len() && i < 8 {
        v |= (nombre[nombre.len() - 1 - i] as u64) << (8 * i);
        i += 1;
    }
    v
}

/// **Una entrada de los argumentos de LIBOS.**
pub fn argumento(nombre: &[u8], iova: u64, bytes: u64) -> [u8; ARGUMENTO] {
    let mut b = [0u8; ARGUMENTO];
    poner(&mut b, 0, &id8(nombre).to_le_bytes());
    poner(&mut b, 8, &iova.to_le_bytes());
    poner(&mut b, 16, &bytes.to_le_bytes());
    b[24] = CONTIGUO;
    b[25] = EN_SYSMEM;
    b
}

/// Una entrada leida: `(id8, iova, bytes, kind, loc)`.
pub fn leer_argumento(b: &[u8]) -> (u64, u64, u64, u8, u8) {
    (u64_de(b, 0), u64_de(b, 8), u64_de(b, 16), b[24], b[25])
}

/// **Las entradas de una tabla de paginas contigua** (`PteArray::init`): la
/// IOVA de cada pagina, en fila, desde `b`.
pub fn tabla(b: &mut [u8], iova: u64, paginas: u64) {
    for i in 0..paginas as usize {
        poner(b, i * 8, &(iova + i as u64 * PAGINA).to_le_bytes());
    }
}

/// La entrada `i` de una tabla.
pub fn entrada(b: &[u8], i: usize) -> u64 {
    u64_de(b, i * 8)
}

/// Lo que mide `GSP_ARGUMENTS_CACHED` (r570.144).
pub const RMARGS_BYTES: usize = 72;

/// **`rmargs`**: donde esta GspMem y sus colas; el resto a 0 menos `bDmemStack`.
pub fn rmargs(gspmem: u64) -> [u8; RMARGS_BYTES] {
    let mut b = [0u8; RMARGS_BYTES];
    poner(&mut b, 0, &gspmem.to_le_bytes());
    poner(&mut b, 8, &(GSPMEM_PAGINAS as u32).to_le_bytes());
    poner(&mut b, 16, &CMDQ_OFFSET.to_le_bytes());
    poner(&mut b, 24, &STATQ_OFFSET.to_le_bytes());
    // 32..44 srInitArguments, 44 gpuInstance: a 0.
    b[48] = 1; // bDmemStack
    // 56..72 profilerArgs: a 0.
    b
}

/// `rmargs` leido: `(gspmem, paginas, cmdq, statq, bDmemStack)`.
pub fn leer_rmargs(b: &[u8]) -> (u64, u32, u64, u64, u8) {
    (u64_de(b, 0), u32_de(b, 8), u64_de(b, 16), u64_de(b, 24), b[48])
}

/// **La cabecera de la cola del CPU** (`MsgqTxHeader::new`); la del GSP la
/// escribe el GSP.
pub fn cabecera_cpu() -> [u8; 32] {
    let mut b = [0u8; 32];
    let campos = [0u32, COLA_BYTES as u32, PAGINA as u32, MSGQ_PAGINAS as u32, 0, 1, RX_HDR_OFF, PAGINA as u32];
    for (k, v) in campos.iter().enumerate() {
        poner(&mut b, k * 4, &v.to_le_bytes());
    }
    b
}

/// La cabecera leida: `(size, msgSize, msgCount, writePtr, flags, rxHdrOff, entryOff)`.
pub fn leer_cabecera(b: &[u8]) -> (u32, u32, u32, u32, u32, u32, u32) {
    (u32_de(b, 4), u32_de(b, 8), u32_de(b, 12), u32_de(b, 16), u32_de(b, 20), u32_de(b, 24), u32_de(b, 28))
}

// ===================================================================
//  615.78.08 (F2a de PLAN_GPU_COMO_LINUX, 20-10)
// ===================================================================
//
// Lo mismo, en los bytes de la 615.78.08 (`gsp_init_args.h` y
// `msgq_priv.h` de OpenRM 615.78.08). Los argumentos de LIBOS, los logs y
// las medidas de GspMem NO cambian (129 paginas, colas en 0x1000 y
// 0x41000). Cambian dos cosas:
//
// ```text
//    rmargs   184 B: magic 'GSP ', su medida y `flags` delante; las colas
//             dicen la medida de su elemento (MCTP+NVDM = 24 B), 4 KiB a
//             64 KiB, y sus alineaciones; la pila en DMEM es un BIT de flags
//    cola     `msgqTxHeader` v2.0: version 2.0, size, msgSize, msgCount,
//             entryOff y tres reservados -- sin writePtr ni rxHdrOff: los
//             punteros viven en REGISTROS del GSP, no en la cola
// ```

/// Lo que mide `GSP_ARGUMENTS_CACHED` (615.78.08).
pub const RMARGS_615_BYTES: usize = 184;
/// `GSP_ARGUMENTS_MAGIC_VALUE`: "GSP " en little-endian.
pub const RMARGS_615_MAGIC: u32 = u32::from_le_bytes(*b"GSP ");
/// `GSP_ARGUMENTS_FLAG_STACK_IN_DMEM`.
pub const RMARGS_615_PILA_EN_DMEM: u64 = 0x02;
/// `GSP_MSG_QUEUE_ELEMENT_SIZE_NO_ENCRYPTION`: MCTP (16) + NVDM (8).
pub const ELEMENTO_615_CABECERA: u32 = 24;
/// `queueElementSizeMin` / `Max`: una pagina, dieciseis.
pub const ELEMENTO_615_MIN: u32 = PAGINA as u32;
pub const ELEMENTO_615_MAX: u32 = 16 * PAGINA as u32;
/// `queueHeaderAlign` y `queueElementAlign` (log2: 4 y 12).
pub const COLA_615_ALINEA_CABECERA: u32 = 4;
pub const COLA_615_ALINEA_ELEMENTO: u32 = 12;

/// **`rmargs` de 615**: magic, medida, la pila en DMEM, y GspMem con sus
/// colas. Lo demas (perfilador, heap de sysmem, bindata...) a 0, como
/// OpenRM en una GA102 con VRAM.
pub fn rmargs_615(gspmem: u64) -> [u8; RMARGS_615_BYTES] {
    let mut b = [0u8; RMARGS_615_BYTES];
    poner(&mut b, 0, &RMARGS_615_MAGIC.to_le_bytes());
    poner(&mut b, 4, &(RMARGS_615_BYTES as u16).to_le_bytes());
    poner(&mut b, 8, &RMARGS_615_PILA_EN_DMEM.to_le_bytes());
    // 16..80 MESSAGE_QUEUE_INIT_ARGUMENTS; `flags` (16) a 0: sin cifrado.
    poner(&mut b, 24, &gspmem.to_le_bytes());
    poner(&mut b, 32, &CMDQ_OFFSET.to_le_bytes());
    poner(&mut b, 40, &STATQ_OFFSET.to_le_bytes());
    poner(&mut b, 48, &ELEMENTO_615_CABECERA.to_le_bytes());
    poner(&mut b, 52, &ELEMENTO_615_MIN.to_le_bytes());
    poner(&mut b, 56, &ELEMENTO_615_MAX.to_le_bytes());
    poner(&mut b, 60, &COLA_615_ALINEA_CABECERA.to_le_bytes());
    poner(&mut b, 64, &COLA_615_ALINEA_ELEMENTO.to_le_bytes());
    poner(&mut b, 68, &(GSPMEM_PAGINAS as u32).to_le_bytes());
    // 80..96 srInitArguments, 100 gpuInstance, 104.. los args: a 0.
    b
}

/// `rmargs` de 615 leido, en la forma del de 570: `(gspmem, paginas, cmdq,
/// statq, pila en DMEM)`, o `None` si no lleva la magic ni su medida.
pub fn leer_rmargs_615(b: &[u8]) -> Option<(u64, u32, u64, u64, u8)> {
    if b.len() < RMARGS_615_BYTES || u32_de(b, 0) != RMARGS_615_MAGIC || u16::from_le_bytes([b[4], b[5]]) as usize != RMARGS_615_BYTES {
        return None;
    }
    let pila = (u64_de(b, 8) & RMARGS_615_PILA_EN_DMEM != 0) as u8;
    Some((u64_de(b, 24), u32_de(b, 68), u64_de(b, 32), u64_de(b, 40), pila))
}

/// `MSGQ_VERSION_MAJOR` y `_MINOR` de 615.
pub const MSGQ_615_VERSION: (u16, u16) = (2, 0);

/// **La cabecera de la cola del CPU de 615** (`msgqTxCreate`): la primera
/// entrada a una pagina de la cabecera, y 63 entradas de 4 KiB.
pub fn cabecera_cpu_615() -> [u8; 32] {
    let mut b = [0u8; 32];
    poner(&mut b, 0, &MSGQ_615_VERSION.0.to_le_bytes());
    poner(&mut b, 2, &MSGQ_615_VERSION.1.to_le_bytes());
    let entrada = PAGINA as u32;
    let campos = [COLA_BYTES as u32, ELEMENTO_615_MIN, (COLA_BYTES as u32 - entrada) / ELEMENTO_615_MIN, entrada];
    for (k, v) in campos.iter().enumerate() {
        poner(&mut b, 4 + k * 4, &v.to_le_bytes());
    }
    b
}

/// La cabecera de 615 leida: `((major, minor), size, msgSize, msgCount, entryOff)`.
pub fn leer_cabecera_615(b: &[u8]) -> ((u16, u16), u32, u32, u32, u32) {
    (
        (u16::from_le_bytes([b[0], b[1]]), u16::from_le_bytes([b[2], b[3]])),
        u32_de(b, 4),
        u32_de(b, 8),
        u32_de(b, 12),
        u32_de(b, 16),
    )
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_medidas_de_nova_core() {
        assert_eq!(COLA_BYTES, 0x40000);
        assert_eq!(GSPMEM_PAGINAS, 129);
        assert_eq!((CMDQ_OFFSET, STATQ_OFFSET), (0x1000, 0x41000));
        assert_eq!((COLA_CPU, COLA_GSP), (0x1000, 0x41000));
        assert_eq!(COLA_GSP + COLA_BYTES, GSPMEM_PAGINAS * PAGINA, "GspMem acaba donde acaba la cola del GSP");
    }

    #[test]
    fn id8_va_al_reves() {
        assert_eq!(id8(b"LOGINIT"), 0x004C_4F47_494E_4954);
        assert_eq!(id8(b"RMARGS").to_le_bytes(), *b"SGRAMR\0\0");
        assert_eq!(id8(b"LOGRM").to_le_bytes()[0], b'M');
    }

    #[test]
    fn una_entrada_de_libos() {
        let a = argumento(b"LOGRM", 0x3C30_0000, 0x10000);
        assert_eq!(leer_argumento(&a), (id8(b"LOGRM"), 0x3C30_0000, 0x10000, 1, 1));
        assert!(a[26..].iter().all(|&x| x == 0));
    }

    #[test]
    fn rmargs_apunta_a_gspmem_y_sus_colas() {
        let r = rmargs(0x3D00_0000);
        assert_eq!(leer_rmargs(&r), (0x3D00_0000, 129, 0x1000, 0x41000, 1));
        assert_eq!(&r[12..16], &[0; 4], "el relleno tras pageTableEntryCount");
        assert!(r[32..48].iter().all(|&x| x == 0) && r[49..].iter().all(|&x| x == 0));
    }

    #[test]
    fn la_cabecera_de_la_cola_del_cpu() {
        let c = cabecera_cpu();
        assert_eq!(leer_cabecera(&c), (0x40000, 0x1000, 63, 0, 1, 32, 0x1000));
        assert_eq!(&c[..4], &[0; 4], "version 0");
    }

    #[test]
    fn rmargs_de_615_en_los_bytes_de_openrm() {
        let r = rmargs_615(0x3D00_0000);
        // offsetof de GSP_ARGUMENTS_CACHED y MESSAGE_QUEUE_INIT_ARGUMENTS (615.78.08).
        assert_eq!(&r[0..4], b"GSP ");
        assert_eq!(u16::from_le_bytes([r[4], r[5]]), 184);
        assert_eq!(u64_de(&r, 8), 0x02, "solo STACK_IN_DMEM");
        assert_eq!(u64_de(&r, 16), 0, "la cola sin cifrado");
        assert_eq!((u32_de(&r, 48), u32_de(&r, 52), u32_de(&r, 56)), (24, 0x1000, 0x10000));
        assert_eq!((u32_de(&r, 60), u32_de(&r, 64)), (4, 12));
        assert!(r[72..].iter().all(|&x| x == 0), "srInit, gpuInstance y los args a 0");
        assert_eq!(leer_rmargs_615(&r), Some((0x3D00_0000, 129, 0x1000, 0x41000, 1)));
        // Un rmargs de 570 no pasa por uno de 615.
        let mut viejo = [0u8; RMARGS_615_BYTES];
        viejo[..RMARGS_BYTES].copy_from_slice(&rmargs(0x3D00_0000));
        assert_eq!(leer_rmargs_615(&viejo), None);
    }

    #[test]
    fn la_cabecera_de_la_cola_de_615() {
        let c = cabecera_cpu_615();
        assert_eq!(leer_cabecera_615(&c), ((2, 0), 0x40000, 0x1000, 63, 0x1000));
        assert!(c[20..].iter().all(|&x| x == 0), "los tres reservados");
        // msgqTxCreate: entryOff = NV_ALIGN_UP(32, 1 << 12); msgCount = (size - entryOff) / msgSize.
        assert_eq!(MSGQ_PAGINAS as u32, (0x40000 - 0x1000) / 0x1000);
    }

    #[test]
    fn la_tabla_de_un_log() {
        let mut p = [0u8; 4096];
        tabla(&mut p[8..], 0x3C10_0000, LOG_PAGINAS);
        assert_eq!(entrada(&p, 0), 0, "el puntero de escritura empieza a 0");
        assert_eq!(entrada(&p, 1), 0x3C10_0000);
        assert_eq!(entrada(&p, 16), 0x3C10_0000 + 15 * 4096);
        assert_eq!(entrada(&p, 17), 0);
    }
}
