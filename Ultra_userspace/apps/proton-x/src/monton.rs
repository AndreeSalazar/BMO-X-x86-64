//! **El monton de PROTON-X**: un bloque, y un cursor que solo avanza.
//!
//! El cargador (`bmo-proton-x`) usa `alloc` -- el `.exe` leido, la imagen, la
//! lista de importaciones -- y en Ring 3 de BMO-X no hay asignador: el kernel
//! da BLOQUES enteros (`KIND_MEMORIA`) y el proceso decide. Para cargar un
//! `.exe` basta lo mas simple que existe: pedir un bloque una vez y repartirlo
//! hacia delante. `dealloc` no devuelve nada; el bloque entero muere con el
//! proceso. El `HeapAlloc` del `.exe` es OTRO monton, el suyo, que si
//! devuelve (P4e: `bmo_proton_x::monton`, en arenas de `Plataforma::memoria`).

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;

pub struct Monton {
    /// (inicio, cursor, fin). Todo a 0 hasta [`Monton::poner`].
    estado: UnsafeCell<(usize, usize, usize)>,
}

// SAFETY: PROTON-X corre en UN hilo (el `.exe` de P1 no crea ninguno).
unsafe impl Sync for Monton {}

impl Monton {
    pub const fn vacio() -> Self {
        Monton { estado: UnsafeCell::new((0, 0, 0)) }
    }

    /// Da al monton el bloque `[base, base + bytes)`.
    ///
    /// # Safety
    /// El bloque es de este proceso, vive hasta que el proceso muere, y nadie
    /// mas lo usa.
    pub unsafe fn poner(&self, base: usize, bytes: usize) {
        *self.estado.get() = (base, base, base + bytes);
    }

    /// Lo gastado, para decirlo.
    pub fn gastado(&self) -> usize {
        let (inicio, cursor, _) = unsafe { *self.estado.get() };
        cursor - inicio
    }
}

unsafe impl GlobalAlloc for Monton {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let e = &mut *self.estado.get();
        // ** Lo GRANDE (64 KiB o mas) empieza en PAGINA (P3b4c, 28-09): el
        // back buffer de la casa es un `Vec<u32>` de aqui, y la 3060 solo
        // dibuja en un destino que empieza en pagina (el kernel lo presta
        // por la IOMMU de pagina en pagina). Cuesta como mucho 4 KiB por
        // cosa grande; lo chico sigue con su alineacion.
        let alinea = if l.size() >= 1 << 16 { l.align().max(4096) } else { l.align() };
        let dir = (e.1 + alinea - 1) & !(alinea - 1);
        match dir.checked_add(l.size()) {
            Some(fin) if e.0 != 0 && fin <= e.2 => {
                e.1 = fin;
                dir as *mut u8
            }
            // Nulo: `alloc` lo convierte en `handle_alloc_error`, que acaba en
            // el `panic_handler` y se dice.
            _ => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, _p: *mut u8, _l: Layout) {}
}
