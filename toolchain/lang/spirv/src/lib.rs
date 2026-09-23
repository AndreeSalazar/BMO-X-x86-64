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
//! == Y el juez (casilla S2) ==
//!
//! [`juzgar`]: un modulo leido cabe en el SUBCONJUNTO, o el primer motivo por
//! el que no. Tipos que cuadran, valores definidos antes de usarse, bloques
//! que empiezan y terminan, saltos con estructura. [`censo`] cuenta de que
//! familias es un modulo, para saber que falta sin parar en el primer NO.
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

mod juez;
mod lector;
mod motivo;
pub mod tabla;

pub use juez::{censo, juzgar, Censo, Veredicto};
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
    /// Constantes de especializacion: las fija el pipeline, y sin VERRANO no
    /// hay pipeline.
    Especializacion,
    /// Todo lo demas de la gramatica: el lector sabe su forma, el juez lo
    /// niega por su nombre.
    Otro,
}

impl Familia {
    /// Todas, en el orden de [`Censo::por_familia`].
    pub const TODAS: [Familia; 9] = [
        Familia::Nucleo,
        Familia::Salto,
        Familia::Imagen,
        Familia::Atomico,
        Familia::Barrera,
        Familia::Matriz,
        Familia::Derivada,
        Familia::Especializacion,
        Familia::Otro,
    ];

    pub fn indice(self) -> usize {
        self as usize
    }

    /// Por que el juez la niega.
    pub fn nombre(self) -> &'static str {
        match self {
            Familia::Nucleo => "nucleo",
            Familia::Salto => "switch/kill: control de flujo con tabla, despues",
            Familia::Imagen => "imagenes y muestreadores: con el rasterizador",
            Familia::Atomico => "atomicos: piden invocaciones a la vez (hilos)",
            Familia::Barrera => "barreras de grupo: piden invocaciones a la vez (hilos)",
            Familia::Matriz => "matrices: fuera del subconjunto de computo",
            Familia::Derivada => "derivadas: solo existen en la etapa de fragmentos",
            Familia::Especializacion => "constantes de especializacion: las fija el pipeline (VERRANO)",
            Familia::Otro => "instruccion fuera del subconjunto",
        }
    }
}

/// A que grupo pertenece una instruccion de `GLSL.std.450`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GrupoGlsl {
    /// Resultado y operandos flotantes del mismo tipo.
    Flotante,
    /// Resultado y operandos enteros de la misma medida.
    Entero,
    /// No existen en SSE: llegan con la casilla S3b.
    Trascendente,
}

/// Una fila de [`tabla::GLSL450`].
#[derive(Clone, Copy, Debug)]
pub struct FilaGlsl {
    pub numero: u32,
    pub nombre: &'static str,
    pub operandos: u8,
    pub grupo: GrupoGlsl,
}

/// La fila de una instruccion de `GLSL.std.450`, o `None` si no se conoce.
pub fn glsl(numero: u32) -> Option<&'static FilaGlsl> {
    tabla::GLSL450
        .binary_search_by_key(&numero, |f| f.numero)
        .ok()
        .map(|i| &tabla::GLSL450[i])
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
