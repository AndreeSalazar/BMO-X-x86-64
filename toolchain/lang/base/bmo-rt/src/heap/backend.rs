//! De donde saca el monton sus bloques.
//!
//! [`SyscallBackend`] los pide a `KIND_MEMORIA` y los devuelve con
//! `MEM_OP_SOLTAR`. En las pruebas, `test_backend` usa la memoria del
//! anfitrion y lleva la cuenta de lo que se pidio y se devolvio.

use bmo_monton::{MemBackend, Trozo};

/// Los bloques de `KIND_MEMORIA`.
#[derive(Debug, Clone, Copy)]
pub struct SyscallBackend;

impl SyscallBackend {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SyscallBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MemBackend for SyscallBackend {
    unsafe fn alloc_chunk(&self, min_size: usize) -> Option<Trozo> {
        crate::syscall::memoria_pedir_con_handle(min_size as u64).map(|(base, handle)| Trozo { base, handle, medida: min_size })
    }

    unsafe fn free_chunk(&self, _ptr: *mut u8, _size: usize, handle: u64) -> bool {
        crate::syscall::memoria_soltar(handle)
    }
}
