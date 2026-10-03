//! De donde saca el monton sus bloques.
//!
//! [`SyscallBackend`] los pide a `KIND_MEMORIA` y los devuelve con
//! `MEM_OP_SOLTAR`. En las pruebas, `test_backend` usa la memoria del
//! anfitrion y lleva la cuenta de lo que se pidio y se devolvio.

use super::freelist::MemBackend;

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
    unsafe fn alloc_chunk(&self, min_size: usize) -> Option<(*mut u8, u64)> {
        crate::syscall::memoria_pedir_con_handle(min_size as u64)
    }

    unsafe fn free_chunk(&self, _ptr: *mut u8, _size: usize, handle: u64) -> bool {
        crate::syscall::memoria_soltar(handle)
    }
}

#[cfg(test)]
pub mod test_backend {
    use super::super::freelist::MemBackend;
    use core::cell::Cell;
    use std::alloc::{alloc, dealloc, Layout};

    /// La memoria del anfitrion, alineada a 16 como una pagina del kernel lo
    /// esta de sobra. Cuenta lo que se pidio y lo que volvio.
    #[derive(Debug, Default)]
    pub struct TestBackend {
        pub pedidos: Cell<u32>,
        pub devueltos: Cell<u32>,
        /// Si es `true`, se niega a devolver (como un bloque prestado).
        pub prestado: Cell<bool>,
    }

    impl MemBackend for TestBackend {
        unsafe fn alloc_chunk(&self, min_size: usize) -> Option<(*mut u8, u64)> {
            let ptr = alloc(Layout::from_size_align(min_size, 16).ok()?);
            if ptr.is_null() {
                return None;
            }
            self.pedidos.set(self.pedidos.get() + 1);
            Some((ptr, 0x100 + self.pedidos.get() as u64))
        }

        unsafe fn free_chunk(&self, ptr: *mut u8, size: usize, _handle: u64) -> bool {
            if self.prestado.get() {
                return false;
            }
            dealloc(ptr, Layout::from_size_align(size, 16).unwrap());
            self.devueltos.set(self.devueltos.get() + 1);
            true
        }
    }
}
