//! **SPIR-V, el formato RECIBIDO.** El frontend de los sombreadores de BMO-X.
//!
//! No es un lenguaje mas: la lista de `lang/` se cerro el 2026-09-17 y SPIR-V
//! entro el 23-09 por otra puerta -- nadie lo ESCRIBE, lo emiten glslang, DXC,
//! Naga o rust-gpu, y BMO-X lo RECIBE. Ver `README.md` y
//! `docs/plan/PLAN_EL_SOMBREADOR.md`.
//!
//! == Lo que hay hoy (casilla S1) ==
//!
//! [`read`]: bytes de un `.spv` -> un [`Module`] recorrible, o un [`Error`]
//! que dice POR QUE no y en que palabra. Los bytes son de un TERCERO: nada de
//! lo que traigan puede hacer que esto entre en panico.
//!
//! == Y el juez (casilla S2) ==
//!
//! [`validate`]: un modulo leido cabe en el SUBCONJUNTO, o el primer motivo por
//! el que no. Tipos que cuadran, valores definidos antes de usarse, bloques
//! que empiezan y terminan, saltos con estructura. [`census`] cuenta de que
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

mod interpreter;
pub mod math;
mod reader;
mod reason;
pub mod table;
mod validator;

pub use validator::{census, validate, Census, Verdict};
pub use reader::{read, Header, EntryPoint, Import, Instruction, Instructions, Module};
pub use interpreter::{workspace_words, Buffer, Interpreter, Stats, Trap};
pub use reason::{Error, Reason};

/// La palabra magica de SPIR-V, leida en little-endian.
pub const MAGIC: u32 = 0x0723_0203;

/// En que parte de la disposicion logica de un modulo vive una instruccion
/// (especificacion de SPIR-V, 2.4). El ORDEN de las variantes es el del
/// fichero: las secciones hasta `Type` solo pueden ir hacia delante.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Section {
    Capability,
    Extension,
    Import,
    MemoryModel,
    EntryPoint,
    ExecutionMode,
    /// `OpString`, `OpSource*`: lo que dice de donde salio.
    Source,
    /// `OpName`, `OpMemberName`.
    Name,
    ModuleProcessed,
    /// Decoraciones.
    Annotation,
    /// Tipos, constantes y variables globales.
    Type,
    /// Puede ir entre los tipos (global) y dentro de una funcion: `OpVariable`,
    /// `OpUndef`, `OpLine`, `OpNoLine`, `OpNop`.
    Flexible,
    Function,
    FunctionEnd,
    /// Solo dentro de una funcion.
    Body,
}

/// A que familia pertenece una instruccion. El lector las LEE todas; el juez
/// (S2) solo acepta `Core` y niega el resto nombrando la familia.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    /// El subconjunto de PLAN_EL_SOMBREADOR, seccion 2.
    Core,
    /// `OpSwitch`, `OpKill`: control de flujo que viene despues.
    ControlFlow,
    Image,
    Atomic,
    /// Barreras de grupo: piden invocaciones A LA VEZ, o sea hilos.
    Barrier,
    Matrix,
    /// Derivadas: solo existen en la etapa de fragmentos.
    Derivative,
    /// Constantes de especializacion: las fija el pipeline, y sin VERRANO no
    /// hay pipeline.
    Specialization,
    /// Todo lo demas de la gramatica: el lector sabe su forma, el juez lo
    /// niega por su nombre.
    Other,
}

impl Family {
    /// Todas, en el orden de [`Census::per_family`].
    pub const ALL: [Family; 9] = [
        Family::Core,
        Family::ControlFlow,
        Family::Image,
        Family::Atomic,
        Family::Barrier,
        Family::Matrix,
        Family::Derivative,
        Family::Specialization,
        Family::Other,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// Por que el juez la niega.
    pub fn name(self) -> &'static str {
        match self {
            Family::Core => "nucleo",
            Family::ControlFlow => "switch/kill: control de flujo con tabla, despues",
            Family::Image => "imagenes y muestreadores: con el rasterizador",
            Family::Atomic => "atomicos: piden invocaciones a la vez (hilos)",
            Family::Barrier => "barreras de grupo: piden invocaciones a la vez (hilos)",
            Family::Matrix => "matrices: fuera del subconjunto de computo",
            Family::Derivative => "derivadas: solo existen en la etapa de fragmentos",
            Family::Specialization => "constantes de especializacion: las fija el pipeline (VERRANO)",
            Family::Other => "instruccion fuera del subconjunto",
        }
    }
}

/// A que grupo pertenece una instruccion de `GLSL.std.450`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GlslGroup {
    /// Resultado y operandos flotantes del mismo tipo.
    Float,
    /// Resultado y operandos enteros de la misma medida.
    Int,
    /// No existen en SSE: llegan con la casilla S3b.
    Transcendental,
}

/// Una fila de [`table::GLSL450`].
#[derive(Clone, Copy, Debug)]
pub struct GlslInfo {
    pub number: u32,
    pub name: &'static str,
    pub operands: u8,
    pub group: GlslGroup,
}

/// La fila de una instruccion de `GLSL.std.450`, o `None` si no se conoce.
pub fn glsl_info(number: u32) -> Option<&'static GlslInfo> {
    table::GLSL450
        .binary_search_by_key(&number, |f| f.number)
        .ok()
        .map(|i| &table::GLSL450[i])
}

/// Una fila de [`table::TABLE`]: lo que el lector necesita saber de una
/// instruccion para recorrerla sin entenderla.
#[derive(Clone, Copy, Debug)]
pub struct OpInfo {
    pub opcode: u16,
    pub name: &'static str,
    /// Lleva un id de TIPO de resultado (la palabra 1).
    pub has_result_type: bool,
    /// Define un id (la palabra 1, o la 2 si lleva tipo).
    pub has_result: bool,
    /// Palabras minimas, cabecera incluida.
    pub min_words: u8,
    /// Palabra donde empieza una cadena de sitio fijo; 0 = no hay.
    pub string_word: u8,
    pub section: Section,
    pub family: Family,
}

/// La fila de un codigo, o `None` si este lector no lo conoce.
pub fn op_info(opcode: u16) -> Option<&'static OpInfo> {
    table::TABLE
        .binary_search_by_key(&opcode, |f| f.opcode)
        .ok()
        .map(|i| &table::TABLE[i])
}
