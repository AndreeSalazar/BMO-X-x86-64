//! **EL MONTON DE BMO-X** -- repartir bloques de memoria en trozos, y que lo
//! soltado se REUSE.
//!
//! generacion: ninguna
//!
//! [carril]  VERDE     reparte memoria que ya es del proceso; pedir bloques
//!                     al kernel es cosa del respaldo, no de aqui
//! [cuesta]  MAQUINA   un trozo mal dado pisa a su vecino
//! [riesgo]  SILENCIO  equivocarse no falla: un trozo mal contado sigue y da
//!                     memoria de otro. Por eso el banco usa cada region
//!                     muchas veces y mira que nadie pise lo vivo
//! [consumo] NADA      solo cuando alguien pide o suelta memoria
//!
//! ```text
//!    freelist   la lista de libres: alineada a 16, ordenada por direccion
//!               (lo soltado se junta por los dos lados), arenas que doblan
//!    region     el respaldo de UN bloque ya dado, con su handle (el ticket
//!               del kernel): el monton de PROTON-X y de la LUDOTECA
//!    cerrojo    un giro sobre un atomico: lo puro no enlaza `bmo-abi`
//! ```
//!
//! Salio de `bmo-rt` el 03-10 para que Ring 3 lo pudiera enlazar: PROTON-X
//! murio en el metal con su monton de cursor (pedir avanzaba, soltar no hacia
//! nada) lleno de basura, y `bmo-rt` es una herramienta (lleva `_start` y
//! syscalls) que Ring 3 no puede enlazar (L8). Lo comun baja a una capa mas
//! baja; `bmo-rt` lo usa desde aqui con su respaldo de syscalls.

#![cfg_attr(not(test), no_std)]

mod cerrojo;
pub mod freelist;
pub mod region;

pub use freelist::{FreelistAllocator, MemBackend, Trozo, ALINEA};
pub use region::Region;

#[cfg(test)]
mod pruebas;
