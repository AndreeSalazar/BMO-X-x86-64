//! El banco del monton: la lista de libres sobre la memoria del anfitrion
//! (un respaldo de mentira que cuenta lo que se pidio y volvio), y la region
//! fija de PROTON-X usada muchas veces seguidas.

use crate::freelist::{FreelistAllocator, MemBackend, Trozo, ARENA_PRIMERA, ALINEA, GRANDE_DESDE};
use core::cell::Cell;
use std::alloc::{alloc, dealloc, Layout as L};

/// La memoria del anfitrion, alineada a 16 como una pagina del kernel lo
/// esta de sobra. Cuenta lo que se pidio y lo que volvio.
#[derive(Debug, Default)]
struct TestBackend {
    pedidos: Cell<u32>,
    devueltos: Cell<u32>,
    /// Si es `true`, se niega a devolver (como un bloque prestado).
    prestado: Cell<bool>,
}

impl MemBackend for TestBackend {
    unsafe fn alloc_chunk(&self, min_size: usize) -> Option<Trozo> {
        let ptr = alloc(L::from_size_align(min_size, 16).ok()?);
        if ptr.is_null() {
            return None;
        }
        self.pedidos.set(self.pedidos.get() + 1);
        Some(Trozo { base: ptr, handle: 0x100 + self.pedidos.get() as u64, medida: min_size })
    }

    unsafe fn free_chunk(&self, ptr: *mut u8, size: usize, _handle: u64) -> bool {
        if self.prestado.get() {
            return false;
        }
        dealloc(ptr, L::from_size_align(size, 16).unwrap());
        self.devueltos.set(self.devueltos.get() + 1);
        true
    }
}
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

/// **El monton de PROTON-X** (03-10): una region fija de 1 MiB, y diez
/// veces su medida en temporales que se piden y se sueltan. El cursor que
/// solo avanzaba moria a la decima parte; la lista los reusa.
#[test]
fn una_region_fija_reusa_lo_soltado() {
    use crate::region::Region;
    let mem = std::vec![0u128; (1 << 20) / 16];
    let r: FreelistAllocator<Region> = FreelistAllocator::new_with(Region::vacia());
    unsafe { r.backend().poner(mem.as_ptr() as usize, 1 << 20, 0x42) };
    let mut vivos = std::vec::Vec::new();
    for i in 0..2000usize {
        let n = 1 + (i * 37) % 9000;
        let p = r.allocate(n);
        assert!(!p.is_null(), "vuelta {i}: {n} B no cupieron con {} en uso", r.en_uso());
        unsafe { ptr::write_bytes(p, i as u8, n) };
        // Uno de cada diez se queda vivo un rato, como un objeto de D3D12.
        if i % 10 == 0 {
            vivos.push((p, n, i as u8));
        } else {
            r.deallocate(p, l());
        }
        if vivos.len() > 20 {
            let (q, m, b) = vivos.remove(0);
            assert!(unsafe { core::slice::from_raw_parts(q, m) }.iter().all(|&x| x == b), "nadie piso lo vivo");
            r.deallocate(q, l());
        }
    }
    assert!(r.pico() < 1 << 20);
    for (q, _, _) in vivos {
        r.deallocate(q, l());
    }
    assert_eq!(r.en_uso(), 0, "todo volvio");
    // Y lo grande (mas de la mitad de la region) sale de la lista entera.
    assert!(!r.allocate(900 * 1024).is_null());
    assert!(r.allocate(200 * 1024).is_null(), "lleno de verdad: nulo, no otro bloque");
}

/// Una region sin poner da nulo, no un puntero a cero.
#[test]
fn una_region_sin_poner_da_nulo() {
    use crate::region::Region;
    let r: FreelistAllocator<Region> = FreelistAllocator::new_with(Region::vacia());
    assert!(r.allocate(8).is_null());
}

/// La pagina para lo grande (lo que la 3060 dibuja empieza en pagina).
#[test]
fn una_region_alinea_a_pagina() {
    use crate::region::Region;
    let mem = std::vec![0u128; (1 << 20) / 16];
    let r: FreelistAllocator<Region> = FreelistAllocator::new_with(Region::vacia());
    unsafe { r.backend().poner(mem.as_ptr() as usize, 1 << 20, 0x42) };
    for _ in 0..4 {
        let p = r.allocate_aligned(70_000, 4096);
        assert_eq!(p as usize % 4096, 0);
        assert!(r.usable(p) >= 70_000);
    }
}

#[test]
fn mas_del_tope_es_nulo() {
    let alloc = test_allocator();
    assert!(alloc.allocate(65 * 1024 * 1024).is_null());
}

/// **El ticket** (03-10): un trozo de una region dice el HANDLE de su bloque
/// y su desplazamiento -- lo que `ARCH_OP_LEER_EN` necesita para leer un
/// fichero directo ahi. La primera version guardaba un 0.
#[test]
fn una_region_da_su_ticket() {
    use crate::region::Region;
    let mem = std::vec![0u128; (1 << 20) / 16];
    let r: FreelistAllocator<Region> = FreelistAllocator::new_with(Region::vacia());
    unsafe { r.backend().poner(mem.as_ptr() as usize, 1 << 20, 0xB10C) };
    assert_eq!(r.backend().handle(), 0xB10C);
    let p = r.allocate(4096);
    let (h, desde) = r.bloque_de(p, 4096).expect("es de la region");
    assert_eq!(h, 0xB10C, "el handle del bloque, no un 0");
    assert_eq!(desde, p as u64 - mem.as_ptr() as u64);
    assert!(r.bloque_de(p, 2 << 20).is_none(), "lo que se sale de la region no");
}

/// **La region que CRECE** (07-10, Cyberpunk en el metal: 64 MiB llenos a
/// los 17,7 s). Llena, sigue por su tramo en trozos de `TROZO_CRECE`,
/// pedidos a `hacer` en orden y sin pisarse; lo de antes sigue vivo e
/// intacto, y lo soltado se reusa. Sin tramo, nulo (lo de antes). Y el tramo
/// tiene fin: pasado, nulo.
#[test]
fn una_region_que_crece_sigue_por_su_tramo() {
    use crate::region::{Region, TROZO_CRECE};
    use std::sync::Mutex;
    // El "kernel": una memoria del anfitrion donde caen los trozos.
    static HECHOS: Mutex<std::vec::Vec<(u64, u64)>> = Mutex::new(std::vec::Vec::new());
    fn hacer(va: u64, n: u64) -> bool {
        HECHOS.lock().unwrap().push((va, n));
        true
    }
    let mem = std::vec![0u128; (1 << 20) / 16];
    let tramo = std::vec![0u128; 3 * TROZO_CRECE / 16];
    let r: FreelistAllocator<Region> = FreelistAllocator::new_with(Region::vacia());
    unsafe {
        r.backend().poner(mem.as_ptr() as usize, 1 << 20, 0x42);
        r.backend().poner_crecer(tramo.as_ptr() as usize, 2 * TROZO_CRECE + 4096, hacer);
    }
    // Lleno la region con trozos de 100 KiB, marcados.
    let mut vivos = std::vec::Vec::new();
    for i in 0..9u8 {
        let p = r.allocate(100 * 1024);
        assert!(!p.is_null());
        unsafe { ptr::write_bytes(p, i, 100 * 1024) };
        vivos.push((p, i));
    }
    assert!(HECHOS.lock().unwrap().is_empty(), "aun cabe en la region");
    // 3,5 MiB, lo que pidio Cyberpunk: ya no cabe; crece.
    let grande = r.allocate(3_670_016);
    assert!(!grande.is_null(), "crece en vez de nulo");
    let base = tramo.as_ptr() as usize;
    assert!((grande as usize) >= base && (grande as usize) < base + TROZO_CRECE, "en el tramo");
    assert_eq!(*HECHOS.lock().unwrap(), [(base as u64, TROZO_CRECE as u64)]);
    assert_eq!((r.backend().crecido(), r.backend().bytes()), (TROZO_CRECE, (1 << 20) + TROZO_CRECE));
    unsafe { ptr::write_bytes(grande, 0xEE, 3_670_016) };
    for &(p, i) in &vivos {
        assert!(unsafe { core::slice::from_raw_parts(p, 100 * 1024) }.iter().all(|&x| x == i), "lo de antes, intacto");
    }
    // Mas de lo que queda del primer trozo: el segundo, detras.
    let a = r.allocate(40 << 20);
    let b = r.allocate(40 << 20);
    assert!(!a.is_null() && !b.is_null());
    assert_eq!(HECHOS.lock().unwrap().len(), 2);
    assert_eq!(HECHOS.lock().unwrap()[1], ((base + TROZO_CRECE) as u64, TROZO_CRECE as u64));
    // Lo soltado se reusa sin crecer.
    r.deallocate(a, l());
    assert!(!r.allocate(30 << 20).is_null());
    assert_eq!(HECHOS.lock().unwrap().len(), 2);
    // El tramo se acabo: nulo, no mas alla.
    assert!(r.allocate(40 << 20).is_null(), "un tercer trozo no cabe en el tramo");
    assert_eq!(HECHOS.lock().unwrap().len(), 2);
}

/// **72 MiB de una vez** (07-10, Cyberpunk en el metal: `memory allocation
/// of 75497472 bytes failed` con el tramo de crecer casi vacio). Una region
/// que crece da un trozo MAS GRANDE que un bloque del kernel, en un solo
/// trozo de su tramo, y se puede escribir entero. La prueba del NO: la misma
/// region sin tramo, nulo (ahi si manda el tope de un bloque).
#[test]
fn una_region_que_crece_da_mas_que_un_bloque() {
    use crate::region::Region;
    use std::sync::Mutex;
    static HECHOS: Mutex<std::vec::Vec<(u64, u64)>> = Mutex::new(std::vec::Vec::new());
    fn hacer(va: u64, n: u64) -> bool {
        HECHOS.lock().unwrap().push((va, n));
        true
    }
    const PIDE: usize = 75_497_472;
    let mem = std::vec![0u128; (1 << 20) / 16];
    // La prueba del NO, primero: sin tramo, el tope de un bloque.
    let fija: FreelistAllocator<Region> = FreelistAllocator::new_with(Region::vacia());
    unsafe { fija.backend().poner(mem.as_ptr() as usize, 1 << 20, 0x42) };
    assert!(fija.allocate(PIDE).is_null(), "sin crecer, 72 MiB no caben");
    // Con tramo (80 MiB de memoria del anfitrion).
    let mem2 = std::vec![0u128; (1 << 20) / 16];
    let tramo = std::vec![0u128; (80 << 20) / 16];
    let r: FreelistAllocator<Region> = FreelistAllocator::new_with(Region::vacia());
    unsafe {
        r.backend().poner(mem2.as_ptr() as usize, 1 << 20, 0x43);
        r.backend().poner_crecer(tramo.as_ptr() as usize, 80 << 20, hacer);
    }
    let p = r.allocate(PIDE);
    assert!(!p.is_null(), "72 MiB de una vez, de su tramo");
    let base = tramo.as_ptr() as usize;
    assert!((p as usize) >= base && (p as usize) + PIDE <= base + (80 << 20), "dentro del tramo");
    unsafe { ptr::write_bytes(p, 0xA5, PIDE) };
    let hechos = HECHOS.lock().unwrap().clone();
    assert_eq!(hechos.len(), 1, "un solo trozo");
    assert_eq!(hechos[0].0, base as u64);
    assert!(hechos[0].1 as usize >= PIDE && hechos[0].1 as usize <= 80 << 20);
    // Y lo que no cabe en el tramo, nulo (no mas alla).
    assert!(r.allocate(PIDE).is_null(), "otro de 72 MiB ya no cabe en 80");
}

/// Si el kernel dice NO (sin RAM), nulo, y no cuenta como crecido.
#[test]
fn una_region_que_no_puede_crecer_da_nulo() {
    use crate::region::Region;
    fn no(_: u64, _: u64) -> bool {
        false
    }
    let mem = std::vec![0u128; (1 << 20) / 16];
    let tramo = std::vec![0u128; 16];
    let r: FreelistAllocator<Region> = FreelistAllocator::new_with(Region::vacia());
    unsafe {
        r.backend().poner(mem.as_ptr() as usize, 1 << 20, 0x42);
        r.backend().poner_crecer(tramo.as_ptr() as usize, 1 << 40, no);
    }
    assert!(r.allocate(2 << 20).is_null());
    assert_eq!(r.backend().crecido(), 0);
}
