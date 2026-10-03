//! **El monton**: `malloc`, `free`, `calloc` y `realloc` sobre una lista de
//! libres ([`freelist`]) que saca sus bloques de `KIND_MEMORIA` ([`backend`]).
//!
//! El mismo `HEAP` es el `#[global_allocator]` de Rust en un `.bex`: lo que un
//! programa pida con `Vec` o con `malloc` sale del mismo sitio. En las pruebas
//! NO lo es (el arnes corre en el anfitrion), y cada prueba usa su propio
//! asignador sobre la memoria del anfitrion.

pub mod backend;
pub mod freelist;

use backend::SyscallBackend;
use core::alloc::{GlobalAlloc, Layout};
use freelist::{FreelistAllocator, ALINEA};

/// El monton del proceso.
#[cfg_attr(not(test), global_allocator)]
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

#[cfg(test)]
mod tests {
    use super::backend::test_backend::TestBackend;
    use super::freelist::{FreelistAllocator, ARENA_PRIMERA, ALINEA, GRANDE_DESDE};
    use core::alloc::{GlobalAlloc, Layout};
    use core::ptr;

    fn test_allocator() -> FreelistAllocator<TestBackend> {
        FreelistAllocator::new_with(TestBackend::default())
    }

    fn l() -> Layout {
        Layout::from_size_align(1, 8).unwrap()
    }

    #[test]
    fn test_malloc_free() {
        let alloc = test_allocator();
        let ptr = alloc.allocate(64);
        assert!(!ptr.is_null());
        alloc.deallocate(ptr, l());
    }

    #[test]
    fn test_malloc_zero() {
        assert!(test_allocator().allocate(0).is_null());
    }

    #[test]
    fn test_realloc_grow() {
        let alloc = test_allocator();
        unsafe {
            let ptr = alloc.allocate(16);
            ptr::write(ptr, 42u8);
            let new_ptr = alloc.realloc(ptr, l(), 64);
            assert!(!new_ptr.is_null());
            assert_eq!(ptr::read(new_ptr), 42u8);
            alloc.deallocate(new_ptr, l());
        }
    }

    #[test]
    fn test_many_small_allocs() {
        let alloc = test_allocator();
        let mut ptrs = [ptr::null_mut(); 100];
        unsafe {
            for (i, p) in ptrs.iter_mut().enumerate() {
                *p = alloc.allocate(8);
                assert!(!p.is_null());
                ptr::write(*p, i as u8);
            }
            for (i, p) in ptrs.iter().enumerate() {
                assert_eq!(ptr::read(*p), i as u8);
            }
            for p in ptrs {
                alloc.deallocate(p, l());
            }
        }
    }

    #[test]
    fn test_calloc() {
        let alloc = test_allocator();
        unsafe {
            let ptr = alloc.allocate(1024);
            ptr::write_bytes(ptr, 0xff, 1024);
            alloc.deallocate(ptr, l());
            let ptr2 = alloc.allocate(1024);
            assert!(!ptr2.is_null());
            alloc.deallocate(ptr2, l());
        }
    }

    /// Todo sale alineado a 16, sea cual sea la medida pedida.
    #[test]
    fn todo_alineado_a_16() {
        let alloc = test_allocator();
        for n in [1usize, 7, 8, 9, 15, 16, 17, 33, 100, 4095] {
            let p = alloc.allocate(n);
            assert_eq!(p as usize % ALINEA, 0, "malloc({n})");
        }
    }

    /// Una alineacion mayor (una pagina, una linea de cache) se respeta, y
    /// liberarla devuelve el trozo de verdad: lo siguiente reusa el sitio.
    #[test]
    fn alineacion_grande_y_su_free() {
        let alloc = test_allocator();
        unsafe {
            for a in [32usize, 64, 4096] {
                let lay = Layout::from_size_align(100, a).unwrap();
                let p = alloc.alloc(lay);
                assert!(!p.is_null());
                assert_eq!(p as usize % a, 0, "align {a}");
                ptr::write_bytes(p, 0xAB, 100);
                assert!(alloc.usable(p) >= 100);
                alloc.dealloc(p, lay);
            }
            // Todo volvio a juntarse: un trozo de casi la arena entera cabe.
            let q = alloc.allocate(ARENA_PRIMERA - 128);
            assert!(!q.is_null());
            assert_eq!(alloc.bloques_vivos(), 1, "no hizo falta otra arena");
        }
    }

    /// Liberar en CUALQUIER orden junta los trozos: al final la arena es un
    /// solo trozo otra vez (antes, en orden inverso, quedaba en migas).
    #[test]
    fn libres_se_juntan_por_los_dos_lados() {
        let alloc = test_allocator();
        let mut ps = [ptr::null_mut(); 64];
        for p in ps.iter_mut() {
            *p = alloc.allocate(1000);
        }
        // Al reves, y luego los impares antes que los pares.
        for p in ps.iter().rev().step_by(2) {
            alloc.deallocate(*p, l());
        }
        for p in ps.iter().rev().skip(1).step_by(2) {
            alloc.deallocate(*p, l());
        }
        let grande = alloc.allocate(ARENA_PRIMERA - 128);
        assert!(!grande.is_null());
        assert_eq!(alloc.bloques_vivos(), 1);
    }

    /// Las arenas crecen al doble: muchas peticiones chicas no gastan los
    /// ocho bloques del kernel.
    #[test]
    fn las_arenas_crecen_y_gastan_poco() {
        let alloc = test_allocator();
        // 8 MiB en trozos de 4 KiB: con arenas de 1 MiB serian 8 bloques.
        for _ in 0..2048 {
            assert!(!alloc.allocate(4096).is_null());
        }
        assert!(alloc.bloques_vivos() <= 4, "{} bloques", alloc.bloques_vivos());
    }

    /// Lo grande pide su bloque y, al liberarlo, VUELVE al kernel.
    #[test]
    fn lo_grande_vuelve_al_kernel() {
        let alloc = test_allocator();
        let p = alloc.allocate(GRANDE_DESDE);
        assert!(!p.is_null());
        assert_eq!(p as usize % ALINEA, 0);
        unsafe { ptr::write_bytes(p, 1, GRANDE_DESDE) };
        assert_eq!(alloc.bloques_vivos(), 1);
        let (h, desde) = alloc.bloque_de(p, GRANDE_DESDE).expect("es del monton");
        assert!(h >= 0x100 && desde > 0);
        alloc.deallocate(p, l());
        assert_eq!(alloc.bloques_vivos(), 0);
    }

    /// Si el kernel no lo recoge (prestado a otro), se queda apuntado.
    #[test]
    fn lo_prestado_no_se_pierde() {
        let alloc = test_allocator();
        let p = alloc.allocate(GRANDE_DESDE);
        alloc.backend().prestado.set(true);
        alloc.deallocate(p, l());
        assert_eq!(alloc.bloques_vivos(), 1);
        assert_eq!(alloc.backend().devueltos.get(), 0);
    }

    /// `bloque_de` dice el bloque y el desplazamiento, y nada fuera.
    #[test]
    fn bloque_de_un_trozo() {
        let alloc = test_allocator();
        let p = alloc.allocate(256);
        let (h, d) = alloc.bloque_de(p, 256).unwrap();
        assert_eq!(h, 0x101);
        assert!(d >= 40);
        assert!(alloc.bloque_de(p, ARENA_PRIMERA * 4).is_none());
        let fuera = [0u8; 16];
        assert!(alloc.bloque_de(fuera.as_ptr(), 16).is_none());
    }

    #[test]
    fn mas_del_tope_es_nulo() {
        let alloc = test_allocator();
        assert!(alloc.allocate(65 * 1024 * 1024).is_null());
    }
}
