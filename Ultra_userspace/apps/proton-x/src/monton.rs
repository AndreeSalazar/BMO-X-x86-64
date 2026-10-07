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

// == LAS ARENAS DE LOS SUB-DIRECTORES (H4.3, 07-10) =======================
//
// Un obrero que corre una parte en Ring 3 (`obreros.rs`) no puede tocar la
// lista de libres de arriba como uno mas: si se le acaba, CRECE por la
// reserva, y eso es un syscall -- en un obrero, un #UD con el cerrojo
// tomado, y el proceso entero se quedaria girando. Asi que lo que pide una
// parte sale de SU arena (un cursor que solo avanza, dentro de su hueco) y
// soltarlo no hace nada: la arena vuelve a cero en el siguiente reparto.
//
// Quien es quien se sabe por la PILA: cada parte corre con la pila de su
// hueco (`bmo_orquesta::ring3::Pedido::pila`), asi que `rsp` dentro del
// hueco `k` es la parte `k`. Sin arenas puestas (la LUDOTECA), `base` es 0
// y esto no mira nada mas.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

/// Cuantas arenas (las partes de un reparto; la 0 es la de la casa y no
/// tiene).
pub const ARENAS_MAX: usize = 16;

pub struct Arenas {
    base: AtomicU64,
    hueco: AtomicU64,
    cursor: [AtomicU64; ARENAS_MAX],
    tope: [AtomicU64; ARENAS_MAX],
    agotada: [AtomicBool; ARENAS_MAX],
}

pub static ARENAS: Arenas = Arenas {
    base: AtomicU64::new(0),
    hueco: AtomicU64::new(0),
    cursor: [const { AtomicU64::new(0) }; ARENAS_MAX],
    tope: [const { AtomicU64::new(0) }; ARENAS_MAX],
    agotada: [const { AtomicBool::new(false) }; ARENAS_MAX],
};

impl Arenas {
    /// Los huecos: el `k` es `[base + k * hueco, base + (k + 1) * hueco)`;
    /// su arena empieza abajo y su pila esta arriba.
    #[allow(dead_code)]
    pub fn poner(&self, base: u64, hueco: u64) {
        self.hueco.store(hueco, Relaxed);
        self.base.store(base, Relaxed);
    }

    /// Antes de cada reparto: la arena `k` a cero, con `bytes` hechos.
    #[allow(dead_code)]
    pub fn preparar(&self, k: usize, bytes: u64) {
        self.cursor[k].store(0, Relaxed);
        self.tope[k].store(bytes, Relaxed);
        self.agotada[k].store(false, Relaxed);
    }

    /// La parte se quedo sin arena (crecera para el siguiente reparto).
    #[allow(dead_code)]
    pub fn agotada(&self, k: usize) -> bool {
        self.agotada[k].load(Relaxed)
    }

    /// La parte que corre en este nucleo (la de la pila), o `None`.
    #[inline]
    fn de_rsp(&self) -> Option<usize> {
        let base = self.base.load(Relaxed);
        if base == 0 {
            return None;
        }
        let rsp: u64;
        // SAFETY: leer rsp.
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags)) };
        let k = rsp.checked_sub(base)? / self.hueco.load(Relaxed);
        (k >= 1 && (k as usize) < ARENAS_MAX).then_some(k as usize)
    }

    #[inline]
    fn es_suya(&self, p: *mut u8) -> bool {
        let base = self.base.load(Relaxed);
        base != 0 && (p as u64).wrapping_sub(base) < self.hueco.load(Relaxed) * ARENAS_MAX as u64
    }

    fn pedir(&self, k: usize, bytes: usize, alinea: usize) -> *mut u8 {
        let abajo = self.base.load(Relaxed) + k as u64 * self.hueco.load(Relaxed);
        let desde = (abajo + self.cursor[k].load(Relaxed)).next_multiple_of(alinea.max(16) as u64);
        let hasta = desde + bytes as u64;
        if hasta > abajo + self.tope[k].load(Relaxed) {
            self.agotada[k].store(true, Relaxed);
            return core::ptr::null_mut();
        }
        self.cursor[k].store(hasta - abajo, Relaxed);
        desde as *mut u8
    }
}

/// Lo que esta corriendo en este nucleo es una parte de un reparto (el
/// `panic_handler` no hace nada mas que un `ud2`: decirlo es un syscall).
#[allow(dead_code)]
pub fn en_parte() -> bool {
    ARENAS.de_rsp().is_some()
}

unsafe impl GlobalAlloc for Monton {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if let Some(k) = ARENAS.de_rsp() {
            return ARENAS.pedir(k, l.size(), l.align());
        }
        // Nulo: `alloc` lo convierte en `handle_alloc_error`, que acaba en el
        // `panic_handler` y se dice.
        self.0.allocate_aligned(l.size(), alineacion(l.size(), l.align()))
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // Lo de una arena vuelve con el siguiente reparto.
        if ARENAS.es_suya(p) {
            return;
        }
        self.0.deallocate(p, l)
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, nuevo: usize) -> *mut u8 {
        // De una arena, o pedido desde una parte: uno nuevo y copiar (cada
        // `alloc` va a donde le toca).
        if ARENAS.es_suya(p) || ARENAS.de_rsp().is_some() {
            let q = self.alloc(Layout::from_size_align_unchecked(nuevo, l.align()));
            if !q.is_null() {
                core::ptr::copy_nonoverlapping(p, q, l.size().min(nuevo));
                self.dealloc(p, l);
            }
            return q;
        }
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
