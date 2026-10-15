//! **BMO LAMINA** -- la RAM que se presta entre dos lados, con UN trato y UN
//! censo. La LAMINA ABSOLUTA de `docs/plan/EL_FOCO.md` (seccion 0, 14-10).
//!
//! generacion: abuelo -- lo que toda lamina PROMETE; no depende de nadie
//! capa: puro -- ni un `unsafe` ni un aparato: palabras atomicas que da quien llama
//!
//! (El censo NOMBRA las clases y sus ficheros como datos, en cadenas: no
//! importa ninguno. La flecha de dependencia baja: VERRANO, hijo, depende de
//! esto, y no al reves -- L7 de `censo-modular`.)
//!
//! # El propietario, 14-10
//!
//! *"RAM = convertir como LAMINA ABSOLUTA, LAMINA ese mismo para administrar y
//! llamar, no quiero tener que lidiar como hizo C con C++"*; y despues: *"la RAM
//! se convierte ZONA DE MAPA DE GUERRA / QUIROFANO EXTREMO porque alli es donde
//! se administra todo [...] LAMINA es inteligente por si sola y el kernel ya
//! sabe que hacer"*.
//!
//! # Lo que ya decian los documentos (y por eso esto no es una idea nueva)
//!
//! ```text
//!    LA_RAM.md           la RAM no es una BODEGA, es un QUIROFANO: entra lo de
//!    (09-08)             la operacion en curso. Siete reglas: malloc no miente,
//!                        lo entregado es ENTERO, copiar se cuenta, el destino
//!                        es una CAPABILITY. Y la Parte IX: SEIS herramientas
//!                        para transportar un dato, y de cada transporte hay
//!                        que poder decir cual es y por que
//!    MONTON.md (20-08)   pedir de un monton es TENERLO EN LA MANO; el estado
//!                        vive dentro del bloque, no en una global escondida
//!    EL_FOCO 1e (10-10)  la LAMINA QUIROFANO: un dato, un sitio; se mira UNA
//!                        palabra; nunca medio fotograma; sin cerrojo; la CPU
//!                        orquesta y la GPU ejecuta; al ritmo, no a la siesta
//! ```
//!
//! Lo que faltaba no era otra regla: era que la RAM COMPARTIDA las cumpliera
//! con un solo trato. Contados el 14-10, hay OCHO bloques que un lado presta a
//! otro, y cada uno trae su cabecera escrita a mano ([`censo::CENSO`]).
//!
//! # El MAPA DE GUERRA: por que la lamina es "inteligente por si sola"
//!
//! Un bloque prestado dice QUE ES en su primera palabra (la magia). Con eso, el
//! que lo toma no adivina: busca la magia en el censo y la [`censo::Clase`] le
//! dice quien escribe, quien lee, que herramienta de LA_RAM es, que trato
//! sigue y si lo lee Ring 0 (y entonces se COPIA una vez y se juzga la copia).
//! El DIRECTOR ya lo hacia a mano con dos clases (`scene/surface.rs`: BVER va
//! a VERRANO, lo demas se mira como superficie); el censo lo hace para todas:
//!
//! ```text
//!    la palabra 0   ->  censo::por_magia  ->  la Clase   ->  quien, como, juez
//! ```
//!
//! Y el kernel, el escritorio o la app saben que hacer porque esta ESCRITO, no
//! porque alguien se acuerde. Un bloque cuya magia no esta en el censo no es
//! una lamina: el guardian `toolchain/tools/laminas` para el build.
//!
//! # El QUIROFANO: lo que NO se le hace a la RAM aqui
//!
//! ```text
//!    malloc/free sueltos      una lamina se pide con su medida y se TIENE: el
//!                             que no la tiene en la mano no la toca
//!    new/delete, destructores cada lamina tiene UN propietario y la recoge el
//!                             kernel al morir este (`reap_dead`); nadie libera
//!                             dos veces lo que no es suyo
//!    cerrojos entre procesos  una app colgada deja su ultimo fotograma, y ya
//!    leer medio dato          el SELLO por ranura ([`trato`]) lo ve, siempre
//! ```
//!
//! # Lo que hay aqui
//!
//! - [`trato`] -- la cabecera comun (magia, version, medida de la cabecera), el
//!   SELLO por ranura y las DOS ranuras con su secuencia. Salio de
//!   `bmo_verrano::lamina` (26-09), donde la prueba de dos hilos cazo el medio
//!   fotograma; BVER lo usa sin mover un byte.
//! - [`censo`] -- las clases de bloque prestado, con su magia.

#![no_std]
#![forbid(unsafe_code)]

pub mod censo;
pub mod trato;
