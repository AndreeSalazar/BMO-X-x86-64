//! **SASS** -- EL IDIOMA DE LA 3060: el juez de lo que se le da a ejecutar
//! (y, con `PLAN_LA_LENGUA_DE_LA_3060.md`, su emisor). El codificador (E2)
//! vive en `platform/shared/bmo-sm86`, puro: aqui solo se le juzga, en pruebas.
//!
//! [carril]  VERDE
//!
//! Por que va aparte: es lo unico del crate que no toca la tarjeta ni sus
//! ordenes -- lee programas de SM86 y dice SI o NO. Lo llaman el build (antes
//! de fabricar un BSF) y el kernel (antes de subir un programa).

pub mod juez;
pub mod corpus;
#[cfg(test)]
mod juez_saltos;
#[cfg(test)]
mod juez_kill;
#[cfg(test)]
mod juez_lecturas;
