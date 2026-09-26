//! **RM** -- LA CONVERSACION CON EL GSP-RM: mensajes, y la lista cerrada de lo que sale.
//!
//! [carril]  AMARILLO
//!
//! Por que va junto: solo sale lo que `contrato` deja; lo que entra se juzga por su cabecera y su suma.
//!
//! Las reglas de todas las carpetas, y el mapa de la tarjeta: `ANATOMIA.md`,
//! al lado de `Cargo.toml`.

/// L0c4a: los mensajes del GSP -- su cabecera, su suma y su nombre (2026-09-24).
pub mod rpc;
/// L0c4b2a: lo que la CPU le escribe al GSP -- SetSystemInfo y SetRegistry (2026-09-24).
pub mod orden;
/// L1a: GET_GSP_STATIC_INFO -- lo que el GSP-RM dice de la 3060 (2026-09-24).
pub mod estatica;
/// L1b: GSP_RM_ALLOC -- nuestro cliente, dispositivo y subdispositivo (2026-09-24).
pub mod objeto;
/// L1b: GSP_RM_CONTROL -- preguntas de control a nuestro subdispositivo (2026-09-24).
pub mod control;
/// La lista CERRADA de lo que sale hacia el GSP-RM, y el NO de lo demas (2026-09-24).
pub mod contrato;
