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
    fn la_tabla_de_un_log() {
        let mut p = [0u8; 4096];
        tabla(&mut p[8..], 0x3C10_0000, LOG_PAGINAS);
        assert_eq!(entrada(&p, 0), 0, "el puntero de escritura empieza a 0");
        assert_eq!(entrada(&p, 1), 0x3C10_0000);
        assert_eq!(entrada(&p, 16), 0x3C10_0000 + 15 * 4096);
        assert_eq!(entrada(&p, 17), 0);
    }
}
