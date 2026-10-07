//! **El monton de PROTON-X**: un bloque, y la lista de libres de
//! `bmo-monton` encima (03-10). Lo usa tambien la LUDOTECA (`#[path]`).
//!
//! [carril]  VERDE     reparte el bloque del proceso; no pide nada al kernel
//! [cuesta]  MAQUINA   un trozo mal dado pisa a la casa entera
//! [riesgo]  SILENCIO  un trozo mal contado no falla: da memoria de otro. La
//!                     lista se prueba en el anfitrion (`bmo-monton`)
//! [consumo] NADA      solo cuando la casa pide o suelta memoria
//!
//! # Por que cambio
//!
//! Hasta el 03-10 era un CURSOR QUE SOLO AVANZA: pedir lo movia y soltar no
//! hacia nada. Se escribio cuando PROTON-X solo cargaba un `.exe` ("el `.exe`
//! de P1 no crea ninguno"); hoy encima vive la casa entera -- las listas de
//! ordenes de D3D12 que se graban cada fotograma, los sombreadores que se
//! leen y se compilan, los textos de cada aviso --, y cada temporal soltado
//! se perdia. En el metal (02-10) Cyberpunk lo lleno al empezar a crear PSO:
//! `memory allocation of 48 bytes failed; monton 50331640 B`.
//!
//! Las texturas y los buferes del `.exe` NO viven aqui (tanda 44: memoria del
//! proceso, `memoria::pedir_paginas`): esto es la casa por dentro.
//!
//! # Lo que se queda igual
//!
//! UN bloque, pedido una vez (`poner`): el kernel da ocho por proceso y los
//! sombreadores sellados tambien los quieren. Y lo grande (64 KiB o mas)
//! empieza en PAGINA (P3b4c, 28-09): el back buffer de la casa es un
//! `Vec<u32>` de aqui, y la 3060 solo dibuja en un destino que empieza en
//! pagina (el kernel lo presta por la IOMMU de pagina en pagina).

use bmo_monton::{FreelistAllocator, Region};
use core::alloc::{GlobalAlloc, Layout};

/// Desde aqui, un trozo empieza en pagina.
const GRANDE: usize = 1 << 16;

pub struct Monton(FreelistAllocator<Region>);

/// La alineacion de verdad de un trozo de `bytes` que pide `alinea`.
fn alineacion(bytes: usize, alinea: usize) -> usize {
    if bytes >= GRANDE {
        alinea.max(4096)
    } else {
        alinea
    }
}

impl Monton {
    pub const fn vacio() -> Self {
        Monton(FreelistAllocator::new_with(Region::vacia()))
    }

    /// Da al monton el bloque `[base, base + bytes)`, con su HANDLE (el
    /// ticket del kernel: con el, el monton sabe nombrar su bloque).
    ///
    /// # Safety
    /// El bloque es de este proceso, vive hasta que el proceso muere, nadie
    /// mas lo usa, y se pone UNA vez antes de pedir nada.
    pub unsafe fn poner(&self, base: usize, bytes: usize, handle: u64) {
        self.0.backend().poner(base, bytes, handle);
    }

    /// **Que pueda CRECER** (07-10): llenos sus bytes, sigue en trozos de
    /// 64 MiB del tramo `[desde, desde + bytes)`, que es SOLO suyo, hechos
    /// por `hacer` (que no pide memoria: corre dentro del monton).
    ///
    /// # Safety
    /// Lo de `bmo_monton::Region::poner_crecer`; tras [`Monton::poner`].
    #[allow(dead_code)]
    pub unsafe fn crecer(&self, desde: usize, bytes: usize, hacer: bmo_monton::region::Hacer) {
        self.0.backend().poner_crecer(desde, bytes, hacer);
    }

    /// Cuanto crecio por la reserva.
    #[allow(dead_code)]
    pub fn crecido(&self) -> usize {
        self.0.backend().crecido()
    }

    /// Lo que esta dado AHORA (antes era todo lo que se dio alguna vez).
    pub fn gastado(&self) -> usize {
        self.0.en_uso()
    }

    /// Lo mas que llego a estar dado.
    #[allow(dead_code)]
    pub fn pico(&self) -> usize {
        self.0.pico()
    }

    /// Lo que mide el bloque.
    #[allow(dead_code)]
    pub fn medida(&self) -> usize {
        self.0.backend().bytes()
    }
}

unsafe impl GlobalAlloc for Monton {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // Nulo: `alloc` lo convierte en `handle_alloc_error`, que acaba en el
        // `panic_handler` y se dice.
        self.0.allocate_aligned(l.size(), alineacion(l.size(), l.align()))
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        self.0.deallocate(p, l)
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, nuevo: usize) -> *mut u8 {
        let alinea = alineacion(nuevo, l.align());
        // Cabe donde esta, y con la alineacion que pide su medida nueva.
        if nuevo <= self.0.usable(p) && p as usize % alinea == 0 {
            return p;
        }
        let q = self.0.allocate_aligned(nuevo, alinea);
        if !q.is_null() {
            core::ptr::copy_nonoverlapping(p, q, l.size().min(nuevo));
            self.0.deallocate(p, l);
        }
        q
    }
}
