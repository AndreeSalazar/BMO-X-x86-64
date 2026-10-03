//! **El monton**: `malloc`, `free`, `calloc` y `realloc` sobre una lista de
//! libres ([`freelist`]) que saca sus bloques de `KIND_MEMORIA` ([`backend`]).
//!
//! El mismo `HEAP` es el `#[global_allocator]` de Rust en un `.bex`: lo que un
//! programa pida con `Vec` o con `malloc` sale del mismo sitio. En las pruebas
//! NO lo es (el arnes corre en el anfitrion), y cada prueba usa su propio
//! asignador sobre la memoria del anfitrion.

pub mod backend;
/// La lista de libres y la region: viven en `bmo-monton` (puro), para que
/// Ring 3 las enlace sin `bmo-rt` (03-10). Se reexportan con su nombre de
/// siempre.
pub use bmo_monton::{freelist, region};

use backend::SyscallBackend;
use core::alloc::{GlobalAlloc, Layout};
use bmo_monton::{FreelistAllocator, ALINEA};

/// El monton del proceso.
#[cfg_attr(all(not(test), feature = "libc"), global_allocator)]
pub static HEAP: FreelistAllocator<SyscallBackend> = FreelistAllocator::new_with(SyscallBackend::new());

/// Lo que C llama `malloc`: alineado a 16 (`max_align_t` en x86-64), o nulo.
pub fn malloc(size: usize) -> *mut u8 {
    HEAP.allocate(size)
}

/// Liberar lo que dio `malloc`, `calloc` o `realloc`. Nulo no hace nada.
pub fn free(ptr: *mut u8) {
    HEAP.deallocate(ptr, Layout::new::<u8>());
}

/// `nmemb * size` bytes a cero. Nulo si la cuenta desborda.
pub fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    let Some(total) = nmemb.checked_mul(size) else { return core::ptr::null_mut() };
    let ptr = malloc(total);
    if !ptr.is_null() {
        unsafe { core::ptr::write_bytes(ptr, 0, total) };
    }
    ptr
}

/// Cambiar de medida conservando lo de dentro.
pub fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8 {
    unsafe { HEAP.realloc(ptr, Layout::from_size_align_unchecked(1, ALINEA), new_size) }
}

/// El bloque del kernel donde vive `[ptr, ptr + n)`, si es del monton.
pub fn bloque_de(ptr: *const u8, n: usize) -> Option<(u64, u64)> {
    HEAP.bloque_de(ptr, n)
}
