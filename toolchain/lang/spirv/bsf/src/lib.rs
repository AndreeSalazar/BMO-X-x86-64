//! **BSF -- BMO Format Shader.** El sobre de los sombreadores: SPIR-V, lo que
//! el SPIR-V pide de fuera, y el codigo YA TRADUCIDO para cada maquina, en un
//! fichero que el consumidor valida ANTES de ejecutar nada.
//!
//! Es la casilla S6 de `docs/plan/PLAN_EL_SOMBREADOR.md`, y la idea es de
//! Eddi: *"ese mismo prepara todo SPIR-V y listo, la GPU no pierde tiempo"*.
//! El JIT de S5 traduce en BMO-X cada vez que se abre un sombreador; el BSF
//! se fabrica UNA vez al construir y el consumidor solo comprueba y sella.
//!
//! == Lo que ve la GPU antes de ejecutar ==
//!
//! Cada modulo lleva su **interfaz** en una tabla de filas fijas: por cada
//! buffer, `set`, `binding`, si es de almacenamiento, lo que el codigo HACE
//! con el (lee, escribe) y su forma (bytes fijos + paso del arreglo sin
//! medida). Con eso el consumidor comprueba los buffers que le dan
//! ([`ModuleView::check`]) sin abrir el SPIR-V: que esten todos, que midan lo
//! que el sombreador lee, que no le den de solo lectura uno que escribe. Y el
//! orden en que el codigo quiere su tabla de buffers lo pone el formato
//! ([`TargetView::table`]), no quien llama.
//!
//! == Por que "atomico" ==
//!
//! 1. **Filas fijas, little-endian, sin punteros.** Cabecera de 64 bytes;
//!    modulos de 128, buffers de 24, objetivos de 128. Nada se interpreta a
//!    medias.
//! 2. **La disposicion NO se lee: se recalcula.** Donde va cada tabla y cada
//!    blob lo decide el formato (canonico: tablas, y despues cada SPIR-V
//!    seguido del codigo de sus objetivos, cada uno alineado a 16). El lector
//!    recalcula cada desplazamiento y exige que el fichero diga el mismo. No
//!    hay solapes que buscar porque no puede haber otra disposicion, y no hay
//!    un byte sin propietario: todo relleno y todo reservado es CERO.
//! 3. **Un hash por cosa, y uno del indice.** BLAKE3 (el hash unico del
//!    sistema) de la cabecera + todas las filas; de cada SPIR-V; de cada
//!    codigo. Y el codigo lleva el hash del SPIR-V del que salio: codigo de
//!    otro SPIR-V es un BSF que miente, y se rechaza entero.
//! 4. **Por capas, de la mas barata a la mas cara.** Forma -> indice ->
//!    disposicion -> referencias; el hash de un blob, cuando se TOMA (nada
//!    se usa sin comprobar, y no se paga lo que no se usa); y la profunda,
//!    opcional: releer el
//!    SPIR-V con el juez y exigir que la tabla sea exactamente su interfaz
//!    ([`Bsf::deep`]), y re-emitir para ver que el codigo es el que saldria
//!    ([`Bsf::reproduce`]).
//! 5. **Determinista.** Los mismos `.spv` dan los mismos bytes: sin fechas,
//!    sin rutas, sin orden de llegada.
//!
//! == Lo que el BSF NO es ==
//!
//! No es la seguridad: el BSF viaja como anexo de un `.bex` y la FIRMA del
//! `.bex` cubre cada anexo (B7). El codigo precompilado es tan de fiar como
//! el `.text` del programa que lo lleva, porque lo firma la misma llave. Los
//! hashes de aqui cazan lo que la firma no mira: un fabricante con un fallo,
//! un SPIR-V cambiado sin volver a emitir, un fichero cortado.
//!
//! == Cuando no hay codigo para esta maquina ==
//!
//! [`ModuleView::target`] devuelve `None` si ningun objetivo es de esta
//! maquina, de esta ABI y con lo que esta CPU tiene. No es un fallo: el SPIR-V
//! va dentro, y el consumidor cae al JIT de S5.
//!
//! [consumo]  NADA   se lee cuando alguien abre un sombreador
//!
//! capa: puro -- el mismo codigo lo usa el fabricante en el anfitrion y la app
//! de Ring 3 que consume; logica sin hardware, `no_std` sin `alloc` y sin un
//! solo `unsafe` (`forbid` abajo).

#![no_std]
#![forbid(unsafe_code)]

mod check;
mod deep;
mod fault;
mod read;
mod write;

pub use check::Given;
pub use deep::{facts, x86_64_target, Facts, EMITTER};
pub use fault::{Fault, What};
pub use read::{Bsf, ModuleView, TargetView};
pub use write::{size, write, ModuleIn, TargetIn};

/// `"BSF1"` en little-endian.
pub const MAGIC: u32 = u32::from_le_bytes(*b"BSF1");
/// La version del formato. Otra version es otro formato: se rechaza.
pub const VERSION: u16 = 1;

/// Medidas de las filas.
pub const HEADER_BYTES: usize = 64;
pub const MODULE_BYTES: usize = 128;
pub const BINDING_BYTES: usize = 24;
pub const TARGET_BYTES: usize = 128;
/// Cada blob empieza en un multiplo de esto.
pub const BLOB_ALIGN: usize = 16;

/// Techos DICHOS. Un fichero que los pasa no se mira mas.
pub const MAX_MODULES: usize = 64;
pub const MAX_BINDINGS: usize = 16;
pub const MAX_TARGETS: usize = 8;
pub const MAX_NAME: usize = 32;
pub const MAX_EMITTER: usize = 16;
pub const MAX_BYTES: usize = 64 << 20;

/// El sombreador lee de este buffer (lo mismo que `bmo_spirv_front::READS`).
pub const READS: u8 = bmo_spirv_front::READS;
/// El sombreador escribe en este buffer.
pub const WRITES: u8 = bmo_spirv_front::WRITES;

/// Las maquinas. Una maquina nueva es un numero nuevo; el formato no cambia.
pub mod kind {
    /// x86-64, el emisor escalar de S4 (`bmo-spirv-x86-64`).
    pub const X86_64_SCALAR: u16 = 1;
}

/// El contrato de llamada del codigo de un objetivo.
pub mod abi {
    /// x86-64 v1: `init(rdi = marco, rsi = tabla)` una vez por despacho y
    /// `main(rdi, rsi, rdx = ids[13], rcx = combustible) -> eax trampa, edx
    /// palabra` por invocacion, System V; tabla de `(direccion u64, bytes u64)`
    /// en el orden de las ranuras del objetivo.
    pub const X86_64_V1: u16 = 1;
}

/// Lo que el codigo pide a la CPU. El consumidor pasa lo que TIENE y un
/// objetivo que pide de mas no se elige.
pub mod cpu {
    pub const SSE2: u32 = 1 << 0;
    pub const SSE4_1: u32 = 1 << 1;
    pub const AVX2: u32 = 1 << 2;
    pub const FMA: u32 = 1 << 3;
    /// Los bits que este formato conoce. Uno fuera de aqui es un fichero de
    /// otra version.
    pub const KNOWN: u32 = SSE2 | SSE4_1 | AVX2 | FMA;
}

/// Un buffer de la interfaz, como lo guarda la tabla.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Binding {
    pub set: u32,
    pub binding: u32,
    pub storage: bool,
    /// [`READS`] | [`WRITES`].
    pub access: u8,
    pub base_bytes: u32,
    pub stride: u32,
}

impl From<&bmo_spirv_front::Binding> for Binding {
    fn from(b: &bmo_spirv_front::Binding) -> Self {
        Binding { set: b.set, binding: b.binding, storage: b.storage, access: b.access, base_bytes: b.base_bytes, stride: b.stride }
    }
}

pub(crate) const fn align(n: usize) -> usize {
    (n + BLOB_ALIGN - 1) & !(BLOB_ALIGN - 1)
}

pub(crate) fn u16le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

pub(crate) fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

pub(crate) fn u64le(b: &[u8], i: usize) -> u64 {
    let mut w = [0u8; 8];
    w.copy_from_slice(&b[i..i + 8]);
    u64::from_le_bytes(w)
}

/// Donde empiezan los blobs: tras las tres tablas, alineado.
pub(crate) fn blobs_start(n_modules: usize, n_bindings: usize, n_targets: usize) -> usize {
    align(HEADER_BYTES + n_modules * MODULE_BYTES + n_bindings * BINDING_BYTES + n_targets * TARGET_BYTES)
}

/// El hash del INDICE: los primeros 32 bytes de la cabecera y todas las filas.
/// Los 32 ultimos de la cabecera son el propio hash.
pub(crate) fn index_hash(bytes: &[u8], tables_end: usize) -> [u8; 32] {
    let mut h = bmo_hash::Hasher::new();
    h.update(&bytes[..32]);
    h.update(&bytes[HEADER_BYTES..tables_end]);
    h.finalize()
}
