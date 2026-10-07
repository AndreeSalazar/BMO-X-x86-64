//! **Un monton sobre UN bloque ya dado** (03-10): la lista de libres de
//! [`crate::freelist`] encima de una region fija, que no pide ni devuelve
//! nada al kernel.
//!
//! [carril]  VERDE     reparte memoria que ya es del proceso; no toca el kernel
//! [cuesta]  MAQUINA   un trozo mal contado pisa memoria de otro trozo
//! [riesgo]  SILENCIO  un trozo mal contado no falla: da memoria de otro. El
//!                     banco (`pruebas`) usa una region nueve veces seguidas
//!                     y mira que nadie pise lo vivo
//! [consumo] NADA      solo cuando el programa pide o suelta memoria
//!
//! # Por que existe
//!
//! PROTON-X murio en el metal (02-10) con `memory allocation of 48 bytes
//! failed; monton 50331640 B`. Su monton era un CURSOR QUE SOLO AVANZA: pedir
//! movia el cursor y soltar no hacia nada. Valia cuando PROTON-X solo cargaba
//! un `.exe`; hoy encima vive toda la casa --D3D12, los sombreadores, los
//! hilos, los textos--, y cada temporal soltado se perdia. Cyberpunk lleno
//! los 48 MiB de basura al empezar a crear PSO.
//!
//! # Por que una region y no bloques que crecen
//!
//! El kernel da OCHO bloques vivos por proceso, y PROTON-X ya los reparte:
//! las cabeceras, la ventana, el monton de Windows y un bloque por cada
//! sombreador sellado. Un monton que pidiera bloques al crecer competiria con
//! los sombreadores por el cupo. Asi, el monton cuesta UNA peticion, como
//! antes; lo que cambia es que lo soltado se REUSA.

//!
//! # Y si se llena: CRECER por la reserva (07-10)
//!
//! El metal (06-10, Cyberpunk a los 17,7 s, con el `.exe` pidiendo 134 GiB
//! a la ventana sin problema): `memory allocation of 3670016 bytes failed;
//! monton 63546240 B en uso de 67108864`. El tope era el de un BLOQUE (64
//! MiB), no la RAM. Con [`Region::poner_crecer`] la region, llena, sigue en
//! trozos de [`TROZO_CRECE`] de un TRAMO de direcciones que es SOLO suyo
//! (en BMO-X, un pedazo de la ventana de `TASK_OP_RESERVA_*`, que no gasta
//! bloques: solo paginas, juzgadas contra la RAM libre). `hacer` las hace:
//! no puede pedir memoria (corre DENTRO del asignador, con su cerrojo).

use crate::freelist::{MemBackend, Trozo};
use core::cell::UnsafeCell;

/// Lo que crece cada vez como POCO (lo que pide algo mas grande, en un
/// trozo de su medida): lo que cabe en un bloque del kernel
/// (`freelist::BLOQUE_TOPE`).
pub const TROZO_CRECE: usize = 64 << 20;

/// Quien hace las paginas de `[va, va + bytes)`: `true` si las hizo. NO
/// puede pedir memoria al monton.
pub type Hacer = fn(u64, u64) -> bool;

/// **El TICKET** (03-10): una region es un bloque que el kernel dio, y en
/// BMO-X un bloque se nombra por su HANDLE, no por su direccion. El monton lo
/// guarda con el bloque: es lo que contesta `bloque_de`, y con el una lectura
/// de fichero cae DIRECTA en memoria del monton (`ARCH_OP_LEER_EN` pide el
/// handle del bloque y el desplazamiento: contrato en vez de comprobacion).
/// La primera version lo tiraba (handle 0); el propietario pregunto por el.
#[derive(Clone, Copy)]
struct Dada {
    base: usize,
    bytes: usize,
    handle: u64,
    ya_dada: bool,
    /// Para crecer: el tramo `[desde, hasta)`, lo que ya se dio de el y
    /// quien hace las paginas (`None`: no crece, lo de antes).
    crece_desde: usize,
    crece_hasta: usize,
    crecido: usize,
    hacer: Option<Hacer>,
}

const NADA: Dada = Dada { base: 0, bytes: 0, handle: 0, ya_dada: false, crece_desde: 0, crece_hasta: 0, crecido: 0, hacer: None };

/// La region: el bloque, su handle, y si ya se dio al monton.
pub struct Region(UnsafeCell<Dada>);

// SAFETY: la region se pone una vez antes de pedir nada, y despues solo la
// lee el asignador, que tiene su cerrojo.
unsafe impl Sync for Region {}

impl Region {
    /// Sin region todavia: todo lo que se pida sera nulo hasta [`Region::poner`].
    pub const fn vacia() -> Self {
        Region(UnsafeCell::new(NADA))
    }

    /// Da la region `[base, base + bytes)`, que es el bloque `handle` del
    /// kernel.
    ///
    /// # Safety
    /// La memoria es del proceso, vive lo que el, nadie mas la usa, y `base`
    /// va alineada a 16. Se llama UNA vez, antes de pedir nada.
    pub unsafe fn poner(&self, base: usize, bytes: usize, handle: u64) {
        *self.0.get() = Dada { base, bytes, handle, ..NADA };
    }

    /// **Que pueda CRECER** (07-10): llena la region, trozos de
    /// [`TROZO_CRECE`] del tramo `[desde, desde + bytes)` (alineado a
    /// pagina), hechos por `hacer`.
    ///
    /// # Safety
    /// El tramo es SOLO de esta region (nadie mas toma esas direcciones),
    /// y `hacer` deja memoria del proceso, a cero y R+W, que vive lo que el.
    /// Se llama tras [`Region::poner`] y antes de pedir nada.
    pub unsafe fn poner_crecer(&self, desde: usize, bytes: usize, hacer: Hacer) {
        let e = &mut *self.0.get();
        e.crece_desde = desde;
        e.crece_hasta = desde + bytes;
        e.crecido = 0;
        e.hacer = Some(hacer);
    }

    /// Cuanto mide la region, con lo que crecio.
    pub fn bytes(&self) -> usize {
        unsafe { (*self.0.get()).bytes + (*self.0.get()).crecido }
    }

    /// Cuanto crecio por la reserva.
    pub fn crecido(&self) -> usize {
        unsafe { (*self.0.get()).crecido }
    }

    /// El handle del bloque (el ticket); 0 sin region.
    pub fn handle(&self) -> u64 {
        unsafe { (*self.0.get()).handle }
    }
}

impl MemBackend for Region {
    /// La region ENTERA, la primera vez; despues, un trozo del tramo si
    /// puede crecer (sin ticket: no es un bloque del kernel), o nada.
    unsafe fn alloc_chunk(&self, min_size: usize) -> Option<Trozo> {
        let e = &mut *self.0.get();
        if e.base == 0 {
            return None;
        }
        if !e.ya_dada && e.bytes >= min_size {
            e.ya_dada = true;
            return Some(Trozo { base: e.base as *mut u8, handle: e.handle, medida: e.bytes });
        }
        let hacer = e.hacer?;
        let n = (min_size.max(TROZO_CRECE) + 4095) & !4095;
        let va = e.crece_desde + e.crecido;
        if va.checked_add(n)? > e.crece_hasta || !hacer(va as u64, n as u64) {
            return None;
        }
        e.crecido += n;
        Some(Trozo { base: va as *mut u8, handle: 0, medida: n })
    }

    /// Una region no se devuelve: muere con el proceso.
    unsafe fn free_chunk(&self, _ptr: *mut u8, _size: usize, _handle: u64) -> bool {
        false
    }

    /// No hay otro bloque que pedir: lo grande sale de la lista.
    fn grande_aparte(&self) -> bool {
        false
    }

    /// **Una region que crece no tiene el tope de un bloque del kernel**
    /// (07-10, el metal: Cyberpunk pidio 72 MiB de una vez y el monton dijo
    /// nulo con 16 GiB de tramo libres, porque un trozo no podia pasar de
    /// los 64 MiB de un bloque de `KIND_MEMORIA`). Su tope es su tramo.
    fn tope(&self) -> usize {
        let e = unsafe { &*self.0.get() };
        match e.hacer {
            Some(_) => crate::freelist::BLOQUE_TOPE.max(e.crece_hasta - e.crece_desde),
            None => crate::freelist::BLOQUE_TOPE,
        }
    }
}

impl Default for Region {
    fn default() -> Self {
        Self::vacia()
    }
}
