//! BLAKE3 -- reexportado desde `bmo-hash`.
//!
//! La implementacion **vivia aqui** y se mudo a `platform/shared/bmo-hash`.
//! El motivo es ESTRATOS: el sistema de ficheros necesita el mismo hash que
//! las firmas del BEF --esa es media garantia del esquema-- pero corre en Ring 0,
//! donde no hay `alloc`, y `bmo-abi` si lo arrastra.
//!
//! Se reexporta en vez de duplicarse. Dos copias del mismo algoritmo son dos
//! copias que pueden separarse, y el dia que se separen el sintoma sera un
//! archivo que "no cuadra" sin que nada apunte al hash.
//!
//! Todo lo que usaba `crate::bef::blake3::hash` sigue funcionando igual.

pub use bmo_hash::{hash, Hasher};

/// Hash BLAKE3 de 256 bits del buffer. Es lo que firma cada region y cada
/// anexo de un BEF2 (`bef2::escritor` lo escribe, `bef2::lector` lo exige).
pub fn blake3_256(bytes: &[u8]) -> [u8; 32] {
    hash(bytes)
}
