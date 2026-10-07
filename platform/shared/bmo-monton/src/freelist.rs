//! **La lista de libres**: el monton sobre los bloques de `KIND_MEMORIA`, con
//! la politica que pide el kernel de BMO-X y no la de un `brk` que aqui no
//! existe. El respaldo (de donde salen los bloques) lo pone quien lo usa:
//! `bmo-rt` pide al kernel; [`crate::Region`] reparte uno ya dado.
//!
//! ```text
//!    el kernel   da BLOQUES enteros y contiguos (TASK_OP_MEMORIA_PEDIR), como
//!                mucho OCHO vivos por proceso y de 64 MiB cada uno; y desde el
//!                2026-09-21 los recoge enteros (MEM_OP_SOLTAR)
//!    las arenas  la primera de 1 MiB y cada una el doble que la anterior,
//!                hasta 64 MiB: con ocho peticiones se llega a ~200 MiB, y un
//!                programa chico gasta UNA
//!    lo grande   lo que pasa de 16 MiB pide su propio bloque, y al liberarlo
//!                vuelve al kernel: un WAD o un fondo no se quedan de por vida
//!    alineado    a 16, lo que C promete en x86-64 (`max_align_t`) y lo que
//!                quiere un `movaps`; mas, con un desplazamiento apuntado
//! ```
//!
//! La lista de libres va ORDENADA POR DIRECCION: al liberar, un trozo se junta
//! con el de delante y con el de detras. Hasta el 03-10 solo miraba el de
//! detras, y un `free` en orden inverso dejaba la arena en migas.
//!
//! La forma de un trozo: una palabra de cabecera (su medida, multiplo de 16,
//! con banderas en los cuatro bits bajos) y detras lo del usuario. Libre, los
//! ocho primeros bytes de lo del usuario guardan el siguiente libre.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;
use crate::cerrojo::Cerrojo;

/// A lo que se alinea todo trozo.
pub const ALINEA: usize = 16;

/// El trozo mas chico: cabecera, el puntero de la lista y relleno a 16.
const MIN_BLOCK: usize = 32;

/// La primera arena; cada una nueva mide el doble.
pub const ARENA_PRIMERA: usize = 1024 * 1024;

/// El tope de un bloque de `KIND_MEMORIA` (`ring0::obj::loan`).
pub const BLOQUE_TOPE: usize = 64 * 1024 * 1024;

/// Desde aqui, un trozo pide su propio bloque.
pub const GRANDE_DESDE: usize = 16 * 1024 * 1024;

/// Cuantos bloques se apuntan. El kernel deja ocho vivos; el doble deja sitio
/// a que cambie sin que esto sea lo primero que se rompa.
pub const BLOQUES: usize = 16;

/// La palabra de cabecera.
pub const HEADER_SIZE: usize = core::mem::size_of::<usize>();

/// Donde empieza el primer trozo de un bloque: detras de su ficha y de forma
/// que lo del usuario caiga alineado a [`ALINEA`] (el bloque lo esta: es una
/// pagina del kernel, o 16 en las pruebas).
const PRIMER_TROZO: usize = 40;
const _: () = assert!(PRIMER_TROZO >= core::mem::size_of::<Ficha>());
const _: () = assert!((PRIMER_TROZO + HEADER_SIZE) % ALINEA == 0);

/// La marca al principio de cada bloque, para verlo en un volcado.
const MAGIA: u64 = 0x5254_4E41_B4D0_424D;

/// La palabra de cabecera de un trozo.
#[repr(transparent)]
pub struct BlockHeader(usize);

impl BlockHeader {
    /// Libre: esta en la lista.
    const FREE: usize = 1;
    /// Es un bloque entero del kernel: liberarlo es devolverlo.
    const GRANDE: usize = 2;
    const BANDERAS: usize = 0xF;

    pub const fn new(size: usize, free: bool) -> Self {
        Self(size | if free { Self::FREE } else { 0 })
    }

    pub fn size(&self) -> usize {
        self.0 & !Self::BANDERAS
    }

    pub fn is_free(&self) -> bool {
        self.0 & Self::FREE != 0
    }

    fn grande(&self) -> bool {
        self.0 & Self::GRANDE != 0
    }
}

/// Lo que va delante de lo del usuario cuando se pidio mas alineado que
/// [`ALINEA`]: cuanto hay que retroceder hasta lo de verdad. Va donde iria la
/// cabecera, y el bit 2 la distingue de una.
const DESPLAZADO: usize = 4;

/// La ficha al principio de cada bloque.
#[allow(dead_code)]
#[repr(C)]
struct Ficha {
    magia: u64,
    medida: usize,
    handle: u64,
}

/// Un bloque que el kernel dio: donde, cuanto, su handle, y si es de un solo
/// trozo grande.
#[derive(Clone, Copy)]
pub struct Bloque {
    pub base: *mut u8,
    pub medida: usize,
    pub handle: u64,
    grande: bool,
}

const SIN_BLOQUE: Bloque = Bloque { base: ptr::null_mut(), medida: 0, handle: 0, grande: false };

/// Un bloque que da el respaldo: donde, su handle, y cuanto mide DE VERDAD
/// (al menos lo pedido; una region fija lo da entero de una vez).
#[derive(Clone, Copy)]
pub struct Trozo {
    pub base: *mut u8,
    pub handle: u64,
    pub medida: usize,
}

/// De donde salen los bloques: el kernel, una region fija ya dada, o la
/// memoria del anfitrion en las pruebas.
pub trait MemBackend {
    /// Un bloque de al menos `min_size` bytes.
    unsafe fn alloc_chunk(&self, min_size: usize) -> Option<Trozo>;
    /// Devolverlo entero. `false` si no se recogio (sigue prestado, o el
    /// respaldo no devuelve).
    unsafe fn free_chunk(&self, ptr: *mut u8, size: usize, handle: u64) -> bool;
    /// Si lo grande ([`GRANDE_DESDE`]) pide su propio bloque. Una region fija
    /// dice que no: no hay otro bloque que pedir, y lo grande sale de la lista
    /// como lo demas.
    fn grande_aparte(&self) -> bool {
        true
    }
    /// **Lo mas que mide UN bloque** de este respaldo: el de un bloque de
    /// `KIND_MEMORIA` ([`BLOQUE_TOPE`]), salvo que el respaldo sepa dar mas
    /// (una region que crece por la reserva: su tramo entero).
    fn tope(&self) -> usize {
        BLOQUE_TOPE
    }
}

struct HeapInner {
    free_head: *mut u8,
    bloques: [Bloque; BLOQUES],
    arenas: u32,
    /// Bytes en trozos dados ahora (con su cabecera), y lo mas que llego.
    en_uso: usize,
    pico: usize,
}

pub struct FreelistAllocator<B: MemBackend> {
    inner: UnsafeCell<HeapInner>,
    backend: B,
    lock: Cerrojo,
}

unsafe impl<B: MemBackend> Send for FreelistAllocator<B> {}
unsafe impl<B: MemBackend> Sync for FreelistAllocator<B> {}

const fn sube(v: usize, a: usize) -> usize {
    (v + a - 1) & !(a - 1)
}

impl<B: MemBackend> FreelistAllocator<B> {
    pub const fn new_with(backend: B) -> Self {
        Self {
            inner: UnsafeCell::new(HeapInner { free_head: ptr::null_mut(), bloques: [SIN_BLOQUE; BLOQUES], arenas: 0, en_uso: 0, pico: 0 }),
            backend,
            lock: Cerrojo::new(),
        }
    }

    /// `size` bytes alineados a [`ALINEA`], o nulo.
    pub fn allocate(&self, size: usize) -> *mut u8 {
        if size == 0 || size > self.backend.tope() {
            return ptr::null_mut();
        }
        let needed = sube(HEADER_SIZE + size, ALINEA).max(MIN_BLOCK);
        self.lock.lock();
        let r = unsafe {
            if needed >= GRANDE_DESDE && self.backend.grande_aparte() {
                self.allocate_large(needed)
            } else {
                self.allocate_from_freelist(needed)
            }
        };
        self.lock.unlock();
        r
    }

    /// `size` bytes alineados a `align` (potencia de dos), o nulo.
    pub fn allocate_aligned(&self, size: usize, align: usize) -> *mut u8 {
        if align <= ALINEA {
            return self.allocate(size);
        }
        let Some(pedir) = size.checked_add(align) else { return ptr::null_mut() };
        let crudo = self.allocate(pedir);
        if crudo.is_null() {
            return crudo;
        }
        let p = sube(crudo as usize, align) as *mut u8;
        if p != crudo {
            // Cabe: los dos van alineados a 16 y son distintos, asi que hay al
            // menos 16 bytes delante de `p` dentro del trozo.
            unsafe { p.sub(HEADER_SIZE).cast::<usize>().write((p as usize - crudo as usize) | DESPLAZADO) };
        }
        p
    }

    pub fn deallocate(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }
        self.lock.lock();
        unsafe { self.deallocate_inner(Self::real(ptr)) };
        self.lock.unlock();
    }

    /// Cuantos bytes de usuario caben desde `ptr` hasta el final de su trozo.
    pub fn usable(&self, ptr: *mut u8) -> usize {
        if ptr.is_null() {
            return 0;
        }
        unsafe {
            let real = Self::real(ptr);
            let hdr = real.sub(HEADER_SIZE);
            hdr as usize + (*hdr.cast::<BlockHeader>()).size() - ptr as usize
        }
    }

    /// **El bloque del kernel que contiene `[ptr, ptr + n)`**: `(handle,
    /// desplazamiento dentro de el)`. Es lo que deja leer un fichero DIRECTO a
    /// memoria del monton con `ARCH_OP_LEER_EN`, sin copia ni bufer aparte.
    pub fn bloque_de(&self, ptr: *const u8, n: usize) -> Option<(u64, u64)> {
        let p = ptr as usize;
        self.lock.lock();
        let r = self.inner().bloques.iter().find_map(|b| {
            let base = b.base as usize;
            (b.medida != 0 && p >= base && p.checked_add(n)? <= base + b.medida).then(|| (b.handle, (p - base) as u64))
        });
        self.lock.unlock();
        r
    }

    /// De donde saca los bloques.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Bytes en trozos dados ahora (cabeceras incluidas).
    pub fn en_uso(&self) -> usize {
        self.inner().en_uso
    }

    /// Lo mas que llego [`FreelistAllocator::en_uso`].
    pub fn pico(&self) -> usize {
        self.inner().pico
    }

    /// Cuantos bloques del kernel tiene apuntados ahora.
    pub fn bloques_vivos(&self) -> usize {
        self.inner().bloques.iter().filter(|b| b.medida != 0).count()
    }
}

impl<B: MemBackend> FreelistAllocator<B> {
    #[allow(clippy::mut_from_ref)]
    fn inner(&self) -> &mut HeapInner {
        unsafe { &mut *self.inner.get() }
    }

    /// Lo del usuario de verdad, deshaciendo un [`DESPLAZADO`].
    unsafe fn real(ptr: *mut u8) -> *mut u8 {
        let w = ptr.sub(HEADER_SIZE).cast::<usize>().read();
        if w & DESPLAZADO != 0 {
            ptr.sub(w & !BlockHeader::BANDERAS)
        } else {
            ptr
        }
    }

    fn apuntar(&self, b: Bloque) -> bool {
        match self.inner().bloques.iter_mut().find(|x| x.medida == 0) {
            Some(x) => {
                *x = b;
                true
            }
            None => false,
        }
    }

    /// Un bloque nuevo del kernel con su ficha puesta.
    unsafe fn pedir_bloque(&self, medida: usize, grande: bool) -> Option<Bloque> {
        if !self.inner().bloques.iter().any(|x| x.medida == 0) {
            return None;
        }
        let t = self.backend.alloc_chunk(medida)?;
        let (base, handle) = (t.base, t.handle);
        if base.is_null() || t.medida < medida {
            return None;
        }
        let medida = t.medida;
        base.cast::<Ficha>().write(Ficha { magia: MAGIA, medida, handle });
        let b = Bloque { base, medida, handle, grande };
        self.apuntar(b);
        Some(b)
    }

    unsafe fn allocate_from_freelist(&self, needed: usize) -> *mut u8 {
        let mut prev: *mut u8 = ptr::null_mut();
        let mut curr = self.inner().free_head;
        while !curr.is_null() {
            let block_size = (*curr.cast::<BlockHeader>()).size();
            let next_free = self.read_next_free(curr);
            if block_size >= needed {
                let remaining = block_size - needed;
                let (siguiente, dado) = if remaining >= MIN_BLOCK {
                    let resto = curr.add(needed);
                    resto.cast::<BlockHeader>().write(BlockHeader::new(remaining, true));
                    self.write_next_free(resto, next_free);
                    curr.cast::<BlockHeader>().write(BlockHeader::new(needed, false));
                    (resto, needed)
                } else {
                    curr.cast::<BlockHeader>().write(BlockHeader::new(block_size, false));
                    (next_free, block_size)
                };
                let inner = self.inner();
                inner.en_uso += dado;
                inner.pico = inner.pico.max(inner.en_uso);
                if prev.is_null() {
                    self.inner().free_head = siguiente;
                } else {
                    self.write_next_free(prev, siguiente);
                }
                return curr.add(HEADER_SIZE);
            }
            prev = curr;
            curr = next_free;
        }

        // Nada cabe: una arena nueva, el doble que la anterior.
        let doble = ARENA_PRIMERA << self.inner().arenas.min(6);
        let medida = doble.max(sube(PRIMER_TROZO + needed, ALINEA)).min(self.backend.tope());
        if PRIMER_TROZO + needed > medida {
            return ptr::null_mut();
        }
        let Some(b) = self.pedir_bloque(medida, false) else { return ptr::null_mut() };
        self.inner().arenas += 1;
        let trozo = b.base.add(PRIMER_TROZO);
        let cabe = (b.medida - PRIMER_TROZO) & !(ALINEA - 1);
        trozo.cast::<BlockHeader>().write(BlockHeader::new(cabe, false));
        // Entra en la lista como un trozo libre (sin contarlo como devuelto).
        self.inner().en_uso += cabe;
        self.deallocate_inner(trozo.add(HEADER_SIZE));
        self.allocate_from_freelist(needed)
    }

    unsafe fn deallocate_inner(&self, ptr: *mut u8) {
        let mut hdr_ptr = ptr.sub(HEADER_SIZE);
        let hdr = &*hdr_ptr.cast::<BlockHeader>();
        debug_assert!(!hdr.is_free(), "doble free");
        self.inner().en_uso -= hdr.size();
        if hdr.grande() {
            self.soltar_grande(hdr_ptr);
            return;
        }
        let mut size = hdr.size();

        // El sitio en la lista ordenada: entre `prev` y `curr`.
        let inner = self.inner();
        let mut prev: *mut u8 = ptr::null_mut();
        let mut curr = inner.free_head;
        while !curr.is_null() && (curr as usize) < hdr_ptr as usize {
            prev = curr;
            curr = self.read_next_free(curr);
        }

        // Con el de detras, si toca.
        let mut next = curr;
        if !curr.is_null() && hdr_ptr.add(size) == curr {
            size += (*curr.cast::<BlockHeader>()).size();
            next = self.read_next_free(curr);
        }
        // Con el de delante, si toca.
        if !prev.is_null() && prev.add((*prev.cast::<BlockHeader>()).size()) == hdr_ptr {
            size += (*prev.cast::<BlockHeader>()).size();
            hdr_ptr = prev;
            hdr_ptr.cast::<BlockHeader>().write(BlockHeader::new(size, true));
            self.write_next_free(hdr_ptr, next);
            return;
        }
        hdr_ptr.cast::<BlockHeader>().write(BlockHeader::new(size, true));
        self.write_next_free(hdr_ptr, next);
        if prev.is_null() {
            inner.free_head = hdr_ptr;
        } else {
            self.write_next_free(prev, hdr_ptr);
        }
    }

    /// Un trozo con su propio bloque: el bloque entero, con la cabecera donde
    /// iria el primer trozo de una arena.
    unsafe fn allocate_large(&self, needed: usize) -> *mut u8 {
        let medida = sube(PRIMER_TROZO + needed, ALINEA);
        if medida > self.backend.tope() {
            return ptr::null_mut();
        }
        let Some(b) = self.pedir_bloque(medida, true) else { return ptr::null_mut() };
        let trozo = b.base.add(PRIMER_TROZO);
        trozo.cast::<BlockHeader>().write(BlockHeader(needed | BlockHeader::GRANDE));
        let inner = self.inner();
        inner.en_uso += needed;
        inner.pico = inner.pico.max(inner.en_uso);
        trozo.add(HEADER_SIZE)
    }

    /// Devolver al kernel el bloque de un trozo grande. Si no lo recoge
    /// (sigue prestado a otro), se queda apuntado: perder la cuenta de memoria
    /// que otro lee seria peor que no reusarla.
    unsafe fn soltar_grande(&self, hdr_ptr: *mut u8) {
        let base = hdr_ptr.sub(PRIMER_TROZO);
        let inner = self.inner();
        if let Some(b) = inner.bloques.iter_mut().find(|b| b.base == base && b.grande) {
            if self.backend.free_chunk(b.base, b.medida, b.handle) {
                *b = SIN_BLOQUE;
            }
        }
    }

    fn read_next_free(&self, hdr: *mut u8) -> *mut u8 {
        unsafe { hdr.add(HEADER_SIZE).cast::<*mut u8>().read() }
    }

    fn write_next_free(&self, hdr: *mut u8, next: *mut u8) {
        unsafe { hdr.add(HEADER_SIZE).cast::<*mut u8>().write(next) }
    }
}

unsafe impl<B: MemBackend> GlobalAlloc for FreelistAllocator<B> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate_aligned(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.deallocate(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ptr.is_null() {
            return self.allocate_aligned(new_size, layout.align());
        }
        if new_size == 0 {
            self.dealloc(ptr, layout);
            return ptr::null_mut();
        }
        let cabe = self.usable(ptr);
        if new_size <= cabe {
            return ptr;
        }
        let new_ptr = self.allocate_aligned(new_size, layout.align());
        if new_ptr.is_null() {
            return ptr::null_mut();
        }
        ptr::copy_nonoverlapping(ptr, new_ptr, cabe.min(new_size));
        self.dealloc(ptr, layout);
        new_ptr
    }
}
