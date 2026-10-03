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

use crate::freelist::{MemBackend, Trozo};
use core::cell::UnsafeCell;

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
}

/// La region: el bloque, su handle, y si ya se dio al monton.
pub struct Region(UnsafeCell<Dada>);

// SAFETY: la region se pone una vez antes de pedir nada, y despues solo la
// lee el asignador, que tiene su cerrojo.
unsafe impl Sync for Region {}

impl Region {
    /// Sin region todavia: todo lo que se pida sera nulo hasta [`Region::poner`].
    pub const fn vacia() -> Self {
        Region(UnsafeCell::new(Dada { base: 0, bytes: 0, handle: 0, ya_dada: false }))
    }

    /// Da la region `[base, base + bytes)`, que es el bloque `handle` del
    /// kernel.
    ///
    /// # Safety
    /// La memoria es del proceso, vive lo que el, nadie mas la usa, y `base`
    /// va alineada a 16. Se llama UNA vez, antes de pedir nada.
    pub unsafe fn poner(&self, base: usize, bytes: usize, handle: u64) {
        *self.0.get() = Dada { base, bytes, handle, ya_dada: false };
    }

    /// Cuanto mide la region.
    pub fn bytes(&self) -> usize {
        unsafe { (*self.0.get()).bytes }
    }

    /// El handle del bloque (el ticket); 0 sin region.
    pub fn handle(&self) -> u64 {
        unsafe { (*self.0.get()).handle }
    }
}

impl MemBackend for Region {
    /// La region ENTERA, la primera vez; despues, nada.
    unsafe fn alloc_chunk(&self, min_size: usize) -> Option<Trozo> {
        let e = &mut *self.0.get();
        if e.base == 0 || e.ya_dada || e.bytes < min_size {
            return None;
        }
        e.ya_dada = true;
        Some(Trozo { base: e.base as *mut u8, handle: e.handle, medida: e.bytes })
    }

    /// Una region no se devuelve: muere con el proceso.
    unsafe fn free_chunk(&self, _ptr: *mut u8, _size: usize, _handle: u64) -> bool {
        false
    }

    /// No hay otro bloque que pedir: lo grande sale de la lista.
    fn grande_aparte(&self) -> bool {
        false
    }
}

impl Default for Region {
    fn default() -> Self {
        Self::vacia()
    }
}
