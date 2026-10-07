//! # PROTON-X -- el cargador de un `.exe` x86-64 de Windows (P1a)
//!
//! generacion: hijo -- lee, coloca, relocaliza y resuelve; no sabe de
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

pub mod bandas;
pub mod bc;
pub mod bufer;
// A9b (06-10): el mapa de la CPU (el Enlace de un PSO) en bytes y de vuelta.
pub mod cifra;
pub mod cargar;
/// E2.5 (05-10): los pixeles en cuadros de 2x2 y en olas (las de un
/// sombreador que usa `Wave*` y `Quad*`).
pub mod cuadros;
pub mod desenrollar;
/// Los formatos de un vertice: de los bytes a lo que lee el sombreador (03-10).
pub mod formato_ia;
pub mod direcciones;
pub mod dll;
pub mod donde;
pub mod dxbc;
pub mod dxil;
pub mod en_vivo;
pub mod ficheros;
pub mod formato;
pub mod pcm;
pub mod pe;
pub mod proceso;
pub mod procesadores;
pub mod raiz;
pub mod regiones;
pub mod registro;
pub mod resumen;
pub mod seh;
pub mod sm5;
pub mod sombras;
pub mod stencil;
pub mod hilos;
pub mod hora;
pub mod lote;
pub mod mates;
pub mod mezcla;
pub mod nulo;
pub mod mensajes;
pub mod monton;
pub mod nativo;
/// E2.3b (05-10): el computo traducido a x86-64, con saltos y barreras.
pub mod nativo_computo;
// L6a (06-10): el Dispatch del computo traducido, con sus olas (A10).
pub mod nativo_despacho;
/// X2 (05-10): lo que el codigo traducido LLAMA (texturas, matematica).
pub mod nativo_llamadas;
pub mod teb;
pub mod texto;
pub mod textura;
pub mod tls;
pub mod trama;
pub mod ventanas;

pub use cargar::{colocar, colocar_en, importaciones, importaciones_de_seccion, partir, resolver, retrasadas_de_seccion, tramos, Funcion, Importacion, Partes, Tramo, PAGINA};
pub use pe::{leer, leer_cabeceras, Pe, Permiso, Seccion};

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
    /// Su directorio de TLS (el 9) no cuadra: fuera de la imagen o sin fin.
    Tls(&'static str),
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
            Fallo::Tls(m) => write!(f, "su TLS (el directorio 9): {m}"),
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
#[cfg(test)]
mod pruebas_pe;
#[cfg(test)]
mod pruebas_windows;
#[cfg(test)]
mod pruebas_sm5;
#[cfg(test)]
mod pruebas_seh;
#[cfg(test)]
mod pruebas_saltos;
#[cfg(test)]
mod pruebas_espacios;
#[cfg(test)]
mod pruebas_computo;
#[cfg(test)]
mod pruebas_geometria;
#[cfg(test)]
mod pruebas_pixeles;
#[cfg(test)]
mod pruebas_uav;
#[cfg(test)]
mod pruebas_olas;
#[cfg(test)]
mod pruebas_niveles;
#[cfg(test)]
mod pruebas_nulo;
#[cfg(test)]
mod pruebas_cifra;
#[cfg(test)]
mod pruebas_bandas;
#[cfg(test)]
mod pruebas_turno;
