//! # bmo-prometeo -- PROMETEO, la libreria general de la GPU
//!
//! generacion: hijo -- el contrato de una tarjeta y el Programa de la casa; no sabe que GPU hay debajo ni quien lo usa
//! capa: puro -- ni un `unsafe`, ni un aparato: un formato y un contrato
//!
//! [carril]  VERDE     datos y un contrato: aqui no corre nada
//!
//! El propietario, el 08-10: *"libreria "gpu general" = ese mismo se
//! engordara que se llevara todo el emisor de GPU para que aplique"*; y
//! despues: *"TODAS LAS GPU en emisor SON AISLADAS por completo luego el JUEZ
//! procesa cada uno y el principal "SUPREMO JUEZ" [...] para GPU Final"*. LB3
//! de `docs/plan/PLAN_LAS_LIBRERIAS.md` (la lectura, en su seccion 4.1).
//!
//! **El nombre.** Prometeo es el titan que les llevo el fuego a todos. Esta
//! libreria lleva lo que sabe hacer cada GPU a cualquier programa de la casa
//! -- TITAN++ hoy; PROTON-X e ILLAPA despues --, sin que el programa nombre
//! ninguna. Son DOS crates con un nombre, porque las capas no dejan otra cosa
//! (`platform/shared` solo usa lo puro):
//!
//! ```text
//!    bmo-prometeo         ESTE: abajo, puro. El Programa de la casa (el
//!                         formato comun) y el contrato de una tarjeta
//!    bmo-titan-prometeo   arriba, en toolchain/lang/titan/prometeo: el emisor
//!                         de GPU de TITAN++ entero (la gpu fn -> el Programa,
//!                         la bateria, la comparacion con la casa y el
//!                         calculo), que pide las tarjetas por este contrato
//! ```
//!
//! # La cadena, con las palabras del propietario
//!
//! ```text
//!    el PROGRAMA de la casa      el formato comun: lo que se le da a cualquier
//!         |                      tarjeta (aqui)
//!         v
//!    cada GPU, AISLADA           una `Tarjeta` por GPU. Ninguna sabe de las
//!    por completo                otras, PROMETEO no sabe de ninguna, y nada de
//!         |                      una tarjeta se comparte (PLAN_EL_AISLAMIENTO)
//!         |    su EMISOR      el Programa a SU codigo maquina (la 3060: SM86)
//!         |    su JUEZ        el de ESA tarjeta: procesa lo que su emisor
//!         |                   escribio, ESTRICTO, y sin el no hay codigo
//!         |    su SIMULADOR   su codigo en el anfitrion, con los bits de la
//!         v                   casa: el oraculo
//!    el SUPREMO JUEZ             el de la PUERTA del kernel, delante de la GPU
//!                                final: el mismo juez `no_std` de esa tarjeta,
//!                                otra vez, y el que no se puede saltar (J2 de
//!                                PLAN_LA_LENGUA_DE_LA_3060). Hoy juzga lo que
//!                                dibuja VERRANO (`CUBO_VERRANO`); el del
//!                                computo de una app llega con LB8
//! ```
//!
//! # Lo que una tarjeta da, y lo que NO
//!
//! El contrato pide las cuatro piezas que son CODIGO (DL2: un trait para lo
//! que es codigo, y los techos como DATOS, en su `Ficha`):
//!
//! ```text
//!    SABE      su ficha (nombre, lengua, registros); y lo que no sabe hacer,
//!              dicho: un LIMITE a proposito, o un FALLO de su emisor
//!    EMITE     el Programa, en su codigo maquina, para el oraculo o para el
//!              viaje. Nunca a medias (L-d)
//!    JUZGA     su juez, sobre lo que su emisor escribio
//!    SIMULA    su codigo, en el anfitrion, con los bits de la casa (L-f)
//! ```
//!
//! La quinta, ENTREGA (su objetivo en el BSF y su puerta), no esta en el
//! contrato TODAVIA: nadie que use PROMETEO entrega aun a una tarjeta --
//! TITAN++ lleva las celdas en el `.bex`, no el codigo --. Entra con el
//! primero que lo haga (LB6 o LB8), y no antes.
//!
//! # Lo que PROMETEO todavia no es
//!
//! - **El Programa no se ha mudado.** Su codigo sigue en PROTON-X
//!   (`bmo_proton_x::dxil::programa`): PROMETEO lo da con su nombre, y quien
//!   lo pide aqui ya no nombra la capa de Windows. Mudarlo arrastra lo que el
//!   interprete de la casa necesita (`mates`, `bufer`, `textura`, `olas`...),
//!   la zona mas viva del arbol: es LB3b, y la flecha se da la vuelta.
//! - **Lo comun a varias tarjetas** (asignar registros, ordenar por
//!   latencias) entra aqui cuando haya DOS tarjetas que lo usen: eso es
//!   *"se engordara"*. Con una sola, lo comun no se sabe todavia.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// ** EL PROGRAMA DE LA CASA: el formato comun de TODAS las tarjetas -- el que
/// sale de los DXIL y los SM5 de PROTON-X, el que escribe TITAN++ de una gpu
/// fn y el que corre el interprete de la casa (`Programa::correr`).
pub use bmo_proton_x::dxil::programa;
pub use programa::Programa;

/// **Para que se escribe un codigo**: la misma cuenta, dos puertas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Para {
    /// El que corre el SIMULADOR de la tarjeta en el anfitrion: el oraculo.
    /// Sus entradas llegan como las lee el banco de pruebas de la tarjeta.
    Oraculo,
    /// El que VIAJA a la tarjeta: lo que su puerta subira, como la tarjeta lo
    /// espera de verdad.
    Viaje,
}

/// **El codigo de una tarjeta**, con lo que dice de si. PROMETEO no lo lee:
/// lo guarda y se lo devuelve a SU tarjeta, para juzgarlo y para simularlo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Codigo {
    /// Las instrucciones, tal como viajan.
    pub bytes: Vec<u8>,
    /// Cuantas son: lo que se compara con el techo de la tarjeta.
    pub instrucciones: usize,
    /// Los registros que usa.
    pub registros: u32,
}

/// **Por que una tarjeta no escribio un Programa** -- dos cosas que no se
/// mezclan (LB1 de PLAN_LAS_LIBRERIAS).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoEmite {
    /// Lo que esta tarjeta TODAVIA NO SABE hacer, dicho a proposito: la
    /// operacion `op` del Programa, con su QUE y su POR QUE en palabras de la
    /// tarjeta. Es el NO del PROGRAMA, en el sitio de esa operacion.
    Limite { op: usize, que: String, por_que: String },
    /// Un fallo de su emisor: nunca del programa. `op`, si es de una
    /// operacion.
    Fallo { op: Option<usize>, por_que: String },
}

/// **Lo que una tarjeta dice de si**: como se la nombra, su lengua y sus
/// techos. Datos, no codigo (DL2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ficha {
    /// Como la nombra la casa en un mensaje ("la 3060").
    pub nombre: &'static str,
    /// Su codigo maquina, por su nombre ("SM86").
    pub lengua: &'static str,
    /// Los registros que se le dan a un cuerpo.
    pub registros: u32,
}

/// ** UNA TARJETA: lo que se le pide a cada GPU, y todo es de ELLA sola.
///
/// Una tarjeta no comparte su emisor, su juez ni su simulador con otra (L-b):
/// la que llega no toca a las que ya estaban, y PROMETEO no cambia. Es
/// `Sync` porque el calculo de un lenguaje la comparte entre hilos.
pub trait Tarjeta: Sync {
    /// **SABE**: como se llama y sus techos.
    fn ficha(&self) -> Ficha;

    /// **EMITE**: el Programa, en su codigo maquina, `para` el oraculo o para
    /// el viaje. Lo que no sabe hacer lo dice (`NoEmite`): nunca se traduce a
    /// medias (L-d).
    fn emitir(&self, p: &Programa, para: Para) -> Result<Codigo, NoEmite>;

    /// **JUZGA**: SU juez, ESTRICTO, sobre un codigo de SU emisor; su NO, en
    /// sus palabras. Es el mismo juez que el SUPREMO JUEZ vuelve a correr en
    /// la puerta del kernel antes de que la GPU vea un bit (L-e).
    fn juzgar(&self, c: &Codigo, para: Para) -> Result<(), String>;

    /// **SIMULA**: un codigo `Para::Oraculo`, en el anfitrion, con los bits de
    /// la casa (L-f). `entradas[e]` son los cuatro componentes (sus bits) de
    /// la entrada `e` del Programa; devuelve sus `salidas` primeras salidas.
    fn simular(&self, c: &Codigo, entradas: &[[u32; 4]], salidas: usize) -> Result<Vec<[u32; 4]>, String>;
}
