//! **SPIR-V, el formato RECIBIDO.** El frontend de los sombreadores de BMO-X.
//!
//! No es un lenguaje mas: la lista de `lang/` se cerro el 2026-09-17 y SPIR-V
//! entro el 23-09 por otra puerta -- nadie lo ESCRIBE, lo emiten glslang, DXC,
//! Naga o rust-gpu, y BMO-X lo RECIBE. Ver `README.md` y
//! `docs/plan/PLAN_EL_SOMBREADOR.md`.
//!
//! == Lo que hay hoy (casilla S1) ==
//!
//! [`leer`]: bytes de un `.spv` -> un [`Modulo`] recorrible, o un [`Fallo`]
//! que dice POR QUE no y en que palabra. Los bytes son de un TERCERO: nada de
//! lo que traigan puede hacer que esto entre en panico.
//!
//! == Las dos reglas que no se negocian ==
//!
//! 1. **`no_std` y sin `alloc`.** La memoria que hace falta --la tabla de ids--
//!    la da quien llama. Es lo que permite que el MISMO lector corra en el
//!    anfitrion y dentro de una app de BMO-X (S5).
//! 2. **No nombra ninguna maquina.** El emisor de x86-64 sera otro crate
//!    (`emisor-x86_64/`, casilla S4); el guardian `isa` lo vigila.
//!
//! [consumo]  NADA   corre solo cuando alguien le da bytes; no hay estado vivo

#![no_std]

mod lector;
mod motivo;
pub mod tabla;

pub use lector::{leer, Cabecera, Entrada, Importacion, Instr, Instrucciones, Modulo};
pub use motivo::{Fallo, Motivo};

/// La palabra magica de SPIR-V, leida en little-endian.
pub const MAGIA: u32 = 0x0723_0203;

/// En que parte de la disposicion logica de un modulo vive una instruccion
/// (especificacion de SPIR-V, 2.4). El ORDEN de las variantes es el del
/// fichero: las secciones hasta `Tipo` solo pueden ir hacia delante.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Seccion {
    Capacidad,
    Extension,
    Importacion,
    Modelo,
    Entrada,
    Modo,
    /// `OpString`, `OpSource*`: lo que dice de donde salio.
    Fuente,
    /// `OpName`, `OpMemberName`.
    Nombre,
    Procesado,
    /// Decoraciones.
    Anotacion,
    /// Tipos, constantes y variables globales.
    Tipo,
    /// Puede ir entre los tipos (global) y dentro de una funcion: `OpVariable`,
    /// `OpUndef`, `OpLine`, `OpNoLine`, `OpNop`.
    Flexible,
    Funcion,
    FinFuncion,
    /// Solo dentro de una funcion.
    Cuerpo,
}

/// A que familia pertenece una instruccion. El lector las LEE todas; el juez
/// (S2) solo acepta `Nucleo` y niega el resto nombrando la familia.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Familia {
    /// El subconjunto de PLAN_EL_SOMBREADOR, seccion 2.
    Nucleo,
    /// `OpSwitch`, `OpKill`: control de flujo que viene despues.
    Salto,
    Imagen,
    Atomico,
    /// Barreras de grupo: piden invocaciones A LA VEZ, o sea hilos.
    Barrera,
    Matriz,
    /// Derivadas: solo existen en la etapa de fragmentos.
    Derivada,
}

/// Una fila de [`tabla::TABLA`]: lo que el lector necesita saber de una
/// instruccion para recorrerla sin entenderla.
#[derive(Clone, Copy, Debug)]
pub struct Fila {
    pub codigo: u16,
    pub nombre: &'static str,
    /// Lleva un id de TIPO de resultado (la palabra 1).
    pub tipo: bool,
    /// Define un id (la palabra 1, o la 2 si lleva tipo).
    pub resultado: bool,
    /// Palabras minimas, cabecera incluida.
    pub minimo: u8,
    /// Palabra donde empieza una cadena de sitio fijo; 0 = no hay.
    pub cadena: u8,
    pub seccion: Seccion,
    pub familia: Familia,
}

/// La fila de un codigo, o `None` si este lector no lo conoce.
pub fn fila(codigo: u16) -> Option<&'static Fila> {
    tabla::TABLA
        .binary_search_by_key(&codigo, |f| f.codigo)
        .ok()
        .map(|i| &tabla::TABLA[i])
}
