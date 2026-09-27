//! # PROTON-X -- el cargador de un `.exe` x86-64 de Windows (P1a)
//!
//! generacion: hija -- lee, coloca, relocaliza y resuelve; no sabe de
//! memoria de verdad ni de saltos
//! capa: puro -- bytes que entran y bytes que salen, probado en el anfitrion
//!
//! El plan: `docs/plan/PLAN_PROTON_X.md`. La regla que manda sobre todo lo
//! demas (seccion 1): **solo x86-64** -- PE32+, maquina AMD64 y codigo
//! MAQUINA. Las instrucciones del juego corren tal cual en el Ryzen; lo que
//! PROTON-X da son sus IMPORTACIONES, y nada mas.
//!
//! # Lo que hace este crate, y lo que deja a quien carga
//!
//! ```text
//!    este crate                           la app de Ring 3 (P1c)
//!    pe::leer        el veredicto y la    lee el fichero
//!                    forma del .exe
//!    colocar         la imagen entera:    pide los bloques, SELLA el codigo
//!                    cabeceras, secciones (MEM_OP_SELLAR: R+X, sin W) y
//!                    y relocalizaciones   deja los datos RW
//!    importaciones   lo que pide, ranura  pone la TABLA DE LA CASA: sus
//!    resolver        a ranura             funciones `extern "win64"`
//!                                         y salta a la entrada
//! ```
//!
//! # Lo que NO hace, y lo dice
//!
//! Una funcion que la tabla no tiene NO se rellena con un stub: [`resolver`]
//! devuelve TODAS las que faltan, con su DLL, y el `.exe` no arranca. Una
//! seccion que se puede escribir Y ejecutar se rechaza (el W^X de la casa).
//! Un `.exe` con TLS se rechaza hasta P4, que es donde se da. Cada negativa
//! es un [`Fallo`] con nombre, nunca un cero.

#![no_std]

extern crate alloc;

pub mod cargar;
pub mod dxbc;
pub mod dxil;
pub mod pe;
pub mod raiz;
pub mod teb;
pub mod trama;
pub mod ventanas;

pub use cargar::{colocar, importaciones, partir, resolver, Funcion, Importacion, Partes, PAGINA};
pub use pe::{leer, Pe, Permiso, Seccion};

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// **Por que un `.exe` no se carga.** Cada variante nombra lo que falla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fallo {
    /// Ni `MZ` ni `PE\0\0` donde tocan.
    NoEsPe,
    /// Un PE, pero FUERA de PROTON-X: 32 bits, ARM, PE32 o .NET. El motivo
    /// es el mismo texto que da `rayosx`.
    Fuera(&'static str),
    /// El fichero se acaba antes de un campo que la cabecera promete.
    Corto(&'static str),
    /// Una seccion que no cabe en la imagen o en el fichero.
    Seccion { nombre: String, motivo: &'static str },
    /// Una seccion que se puede escribir Y ejecutar: el W^X de la casa.
    EscribeYEjecuta(String),
    /// Hay que moverlo de su base y el enlazador le quito las relocalizaciones
    /// (`RELOCS_STRIPPED` en la cabecera COFF).
    SinRelocalizaciones,
    /// Una relocalizacion de un tipo que un PE32+ x86-64 no deberia traer.
    Relocalizacion { rva: u32, tipo: u16 },
    /// Pide TLS (el directorio 9): se da en P4, no antes.
    PideTls,
    /// Una seccion que no es codigo cae en las paginas del codigo: con
    /// bloques enteros no se sella una sin la otra.
    NoSeParte(String),
    /// Lo que el `.exe` importa y la tabla de la casa NO tiene. Todas.
    Faltan(Vec<Importacion>),
}

impl fmt::Display for Fallo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fallo::NoEsPe => write!(f, "no es un PE (ni MZ ni PE donde tocan)"),
            Fallo::Fuera(m) => write!(f, "FUERA de PROTON-X: {m}"),
            Fallo::Corto(que) => write!(f, "el fichero se acaba antes de {que}"),
            Fallo::Seccion { nombre, motivo } => write!(f, "la seccion {nombre}: {motivo}"),
            Fallo::EscribeYEjecuta(n) => write!(f, "la seccion {n} se puede escribir Y ejecutar: el W^X de la casa no lo deja"),
            Fallo::SinRelocalizaciones => write!(f, "hay que moverlo de su base y el enlazador le quito las relocalizaciones (RELOCS_STRIPPED)"),
            Fallo::Relocalizacion { rva, tipo } => write!(f, "relocalizacion de tipo {tipo} en la RVA {rva:#x}: un PE32+ x86-64 solo trae DIR64"),
            Fallo::PideTls => write!(f, "pide TLS (el directorio 9): se da en P4"),
            Fallo::NoSeParte(n) => write!(f, "la seccion {n} no es codigo y cae en las paginas del codigo: no se sella una sin la otra"),
            Fallo::Faltan(v) => {
                write!(f, "no arranca: faltan {} funcion(es) en la tabla de la casa:", v.len())?;
                for i in v {
                    write!(f, " {}!{}", i.dll, i.funcion)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod pruebas;
