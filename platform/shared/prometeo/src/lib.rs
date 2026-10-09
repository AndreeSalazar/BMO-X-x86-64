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
//! # El Programa vive aqui (LB3b, 08-10)
//!
//! Su codigo salio de PROTON-X, y la flecha se dio la vuelta: PROTON-X depende
//! de PROMETEO y lo re-exporta en sus rutas de siempre (`dxil::programa`,
//! `mates`, `bufer`, `textura`...), asi que no cambio una linea de las suyas.
//! Lo que vino es el FORMATO y lo que hace falta para CORRERLO en la CPU, y
//! nada mas:
//!
//! ```text
//!    programa     el formato: Op, Programa, Ranuras... y sus numeros
//!    interprete   correrlo sobre floats: el JUEZ de lo que hagan las tarjetas
//!    olas         lo que hace una operacion de olas, y las derivadas
//!    carriles     una ola de 32 carriles, corriendo junta
//!    despacho     un Dispatch de computo: grupos, barreras, olas
//!    ranuras      las texturas, muestreadores, cbuffers y UAV que lee
//!    mates        las funciones (sin, exp...) con los bits de la casa
//!    bufer        los UAV y los buferes: leer, escribir, atomicas
//!    formato_ia   los formatos DXGI de un texel o un elemento
//!    textura      muestrear: filtros, mips, comparaciones
//!    bc           los bloques comprimidos BC1-BC7
//!    profundidad  la comparacion de D3D12 (la de SampleCmp y la de la trama)
//! ```
//!
//! Lo que se quedo en PROTON-X es lo de Windows: leer DXIL y SM5 y
//! traducirlos a este Programa, la trama (el rasterizador), la mezcla, el
//! stencil, y `nativo` (este Programa a x86-64: la CPU como tarjeta, LB4).
//!
//! # Lo que PROMETEO todavia no es
//!
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
pub mod programa;
pub use programa::Programa;

// -- Correrlo en la CPU: el interprete y lo que necesita (LB3b, 08-10) -------

/// El interprete: un Programa corrido sobre floats, una operacion tras otra.
pub mod interprete;
/// Lo que hace cada operacion de olas, y las derivadas de su cuadro.
pub mod olas;
/// Una ola de carriles corriendo junta (computo, pixeles).
pub mod carriles;
/// `Programa::despachar`: un Dispatch de computo, grupo a grupo.
pub mod despacho;
/// Las ranuras de un Programa: texturas, muestreadores, cbuffers, UAV.
pub mod ranuras;
/// Las funciones matematicas con los bits de la casa.
pub mod mates;
/// ** DL13 (09-10): las Mate de SERIES como RECETAS de cuentas de f32 que la
/// casa corre y cada tarjeta repite: los mismos bits por construccion.
pub mod cuentas;
/// La FMA exacta de f32, sin `libm`.
pub mod fma;
/// Seno, coseno y tangente.
pub mod trigo;
/// exp2, log2 y e^x.
pub mod exponencial;
/// Arcotangente, arcoseno y arcocoseno.
pub mod arcos;
/// Senh, cosh y tanh.
pub mod hiperbolicas;
#[cfg(test)]
mod pruebas_series;
/// Los UAV y los buferes.
pub mod bufer;
/// Los formatos DXGI de un texel o de un elemento.
pub mod formato_ia;
/// Muestrear una textura: filtros, mips, comparaciones.
pub mod textura;
/// Los bloques comprimidos BC1-BC7.
pub mod bc;
/// La prueba de profundidad de D3D12.
pub mod profundidad;

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

/// **Lo que una tarjeta dice de si**: como se la nombra, su lengua, sus
/// techos y el APARATO exacto que es. Datos, no codigo (DL2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ficha {
    /// Como la nombra la casa en un mensaje: el aparato exacto ("la RTX 3060
    /// 12G"), nunca la familia de su ISA.
    pub nombre: &'static str,
    /// Su codigo maquina: el modulo de ISA que lleva dentro ("SASS sm_86").
    pub lengua: &'static str,
    /// Los registros que se le dan a un cuerpo.
    pub registros: u32,
    /// El aparato exacto que es.
    pub aparato: Aparato,
}

/// ** EL APARATO EXACTO (08-10, el propietario: *"que la GPU no sea por ISA
/// sea por especificamente muy precisos no es ISA sino propio ISA modular
/// ... (solo: 3060 12G)"*): una tarjeta es UNA grafica concreta, no una
/// familia de ISA. sm_86 es la lengua de toda Ampere GA10x; una RTX 3060 12G
/// y una 3070 serian DOS tarjetas, cada una con sus techos y su juez, aunque
/// hablen la misma ISA. Asi un NO dice el aparato exacto, y el kernel puede
/// reconocerlo por sus ids ([`Aparato::es`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aparato {
    /// Quien la hace y su modelo exacto ("NVIDIA GeForce RTX 3060 12G").
    pub modelo: &'static str,
    /// Su chip ("GA106").
    pub chip: &'static str,
    /// Por donde la reconoce el kernel en el PCI; `None` si no es una
    /// grafica del PCI (la CPU).
    pub pci: Option<Pci>,
    /// Su memoria propia, en MiB; 0 si usa la de la CPU.
    pub memoria_mib: u32,
}

/// Una grafica en el PCI: su fabricante y los dispositivos que son ella.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pci {
    pub fabricante: u16,
    pub dispositivos: &'static [u16],
}

impl Aparato {
    /// Si la grafica `fabricante:dispositivo` del PCI es ESTE aparato.
    pub fn es(&self, fabricante: u16, dispositivo: u16) -> bool {
        self.pci.is_some_and(|p| p.fabricante == fabricante && p.dispositivos.contains(&dispositivo))
    }
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
