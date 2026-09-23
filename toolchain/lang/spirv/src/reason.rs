//! **Por que NO.** Cada forma de que un `.spv` no se pueda leer, con su nombre.
//!
//! Un NO sin motivo es un SILENCIO (L6i): quien tiene un sombreador que no
//! carga necesita saber si el fichero esta roto, si es de otra version o si
//! usa algo que este lector no conoce -- son tres arreglos distintos.
//!
//! [consumo]  NADA   solo datos

use core::fmt;

use crate::Family;

/// Lo que fallo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reason {
    /// Menos de las cinco palabras de la cabecera.
    Short { bytes: usize },
    /// SPIR-V es una lista de palabras de 32 bits.
    NotWordAligned { bytes: usize },
    /// La primera palabra no es la magia.
    NotSpirv { magic: u32 },
    /// La magia al reves: un modulo en big-endian. Es SPIR-V legal, pero nada
    /// de lo que BMO-X recibe lo emite, y darle la vuelta es trabajo sin cliente.
    BigEndian,
    /// Version que no es 1.0 a 1.6, o palabra de version mal formada.
    Version { word: u32 },
    /// `bound` es 0: no cabe ni un id.
    ZeroBound,
    /// El esquema esta reservado y vale 0.
    NonZeroSchema { schema: u32 },
    /// La tabla de ids que dio quien llama es mas chica que `bound`.
    IdTableTooSmall { bound: u32, capacity: usize },
    /// Una instruccion que dice medir 0 palabras: el recorrido no avanzaria.
    EmptyInstruction,
    /// Una instruccion que dice medir mas de lo que queda del fichero.
    PastEnd { words: u16, remaining: usize },
    /// Un codigo que la tabla no tiene. Se dice el numero.
    UnknownOpcode { opcode: u16 },
    /// Menos palabras de las que la instruccion necesita.
    TooShort { opcode: u16, words: u16, min_words: u8 },
    /// Un id que es 0 o no es menor que `bound`.
    IdOutOfBound { id: u32 },
    /// Dos instrucciones definen el mismo id.
    DuplicateId { id: u32 },
    /// Una cadena que no termina en su cero dentro de su instruccion.
    UnterminatedString { opcode: u16 },
    /// `OpMemoryModel` tiene que estar exactamente una vez.
    MemoryModelCount { count: u32 },
    /// Una instruccion de una seccion anterior a la ultima vista.
    OutOfOrder { opcode: u16 },
    /// `OpFunction` sin cerrar la anterior.
    NestedFunction,
    /// `OpFunctionEnd` sin `OpFunction`.
    StrayFunctionEnd,
    /// Una instruccion de cuerpo fuera de toda funcion.
    BodyOutsideFunction { opcode: u16 },
    /// El fichero se acaba dentro de una funcion.
    UnterminatedFunction,
    /// Un modo de ejecucion para un id que no es punto de entrada.
    ModeWithoutEntryPoint { id: u32 },
    /// Mas de las que este lector guarda. Es un techo DICHO, no un error del fichero.
    TooMany { what: &'static str, limit: usize },

    // ---- EL JUEZ (S2): el modulo esta bien formado, pero no cabe ----------
    /// Una capacidad que no es `Shader`.
    UnsupportedCapability { capability: u32 },
    /// Una extension que el subconjunto no conoce.
    UnsupportedExtension,
    /// Instrucciones extendidas que no son `GLSL.std.450`.
    UnsupportedImport,
    /// Direccionamiento o memoria que no son `Logical` + `GLSL450`.
    UnsupportedMemoryModel { addressing: u32, memory: u32 },
    /// Ningun punto de entrada.
    NoEntryPoint,
    /// Una etapa que no es `GLCompute`.
    UnsupportedStage { model: u32 },
    /// Un punto de entrada de computo sin `LocalSize`.
    NoLocalSize,
    /// Un modo de ejecucion que no es `LocalSize`.
    UnsupportedMode { mode: u32 },
    /// Una instruccion de una familia que no es el nucleo.
    UnsupportedFamily { family: Family, opcode: u16 },
    /// Una instruccion del nucleo que el subconjunto aun asi no acepta.
    UnsupportedInstruction { opcode: u16, why: &'static str },
    /// Una instruccion de `GLSL.std.450` que no esta en la tabla.
    UnsupportedGlsl { number: u32 },
    /// Una de `GLSL.std.450` que llega con S3b (seno, coseno, exp, log, pow).
    GlslLater { number: u32 },
    /// Un tipo que no cabe: entero o flotante que no es de 32 bits, vector de
    /// mas de 4...
    UnsupportedType { why: &'static str },
    /// Una clase de almacenamiento fuera del subconjunto.
    UnsupportedStorageClass { class: u32 },
    /// Una variable de entrada que no es un `BuiltIn`.
    InputWithoutBuiltIn { id: u32 },
    /// Un `BuiltIn` que el computo no tiene.
    UnsupportedBuiltIn { builtin: u32 },
    /// Un buffer sin `Binding` o sin `DescriptorSet`: nadie sabria cual es.
    NoBinding { id: u32 },
    /// Se esperaba un tipo.
    NotAType { id: u32 },
    /// Se esperaba un valor.
    NotAValue { id: u32 },
    /// Se esperaba una constante.
    NotAConstant { id: u32 },
    /// Se esperaba una etiqueta de esta misma funcion.
    NotALabel { id: u32 },
    /// Un id que nadie define.
    Undefined { id: u32 },
    /// Un valor usado antes de definirse, o que es de otra funcion.
    UsedBeforeDefined { id: u32 },
    /// Los tipos de una instruccion no cuadran.
    TypeMismatch { opcode: u16 },
    /// Un indice de composicion fuera de su medida.
    IndexOutOfRange { opcode: u16 },
    /// Escribir en una variable de entrada.
    WriteToInput,
    /// Una funcion sin ningun bloque.
    NoBody,
    /// Una instruccion fuera de un bloque (antes de su `OpLabel`).
    OutsideBlock { opcode: u16 },
    /// Un bloque que no termina en un salto o un retorno.
    UnterminatedBlock,
    /// `OpPhi` que no esta al principio de su bloque.
    MisplacedPhi,
    /// `OpVariable` de funcion que no esta al principio del primer bloque.
    MisplacedVariable,
    /// Una instruccion de fusion que no va justo antes de su salto.
    MisplacedMerge,
    /// Un salto condicional sin construccion que lo encierre.
    Unstructured,
    /// Un punto de entrada que no es `void main()`.
    EntryPointNotVoidMain,

    // ---- EL ORACULO (S3): lo que SPIR-V deja indefinido, aqui PARA ---------
    /// La memoria de trabajo que se dio no alcanza (`workspace_words`).
    WorkspaceTooSmall { need: usize, have: usize },
    /// El sombreador pide un buffer que no se le dio.
    NoBuffer { set: u32, binding: u32 },
    /// Un buffer sin `Offset` o sin `ArrayStride`: no se sabe donde cae nada.
    NoLayout { id: u32 },
    /// Division o resto entero por cero.
    DivisionByZero,
    /// `INT_MIN / -1`: el cociente no cabe.
    DivisionOverflow,
    /// Desplazar 32 bits o mas.
    ShiftTooLarge,
    /// Leer o escribir fuera de un buffer, o un indice fuera de un arreglo.
    OutOfBounds,
    /// Leer un valor que en ESTA invocacion no se definio (lo que el juez no
    /// ve: la dominancia).
    UndefinedValue { id: u32 },
    /// Mas instrucciones que el combustible dado: un bucle que no termina.
    OutOfFuel,
    /// Mas llamadas anidadas que las que el oraculo guarda.
    CallTooDeep,
    /// Se ejecuto `OpUnreachable`.
    ReachedUnreachable,
    /// Un `OpPhi` sin la pareja del bloque del que se viene.
    PhiWithoutPredecessor,
    /// Algo que el oraculo aun no sabe hacer, dicho.
    NotYet { what: &'static str },
}

impl Reason {
    /// El motivo en palabras, sin los numeros.
    pub fn name(&self) -> &'static str {
        match self {
            Reason::Short { .. } => "mas corto que la cabecera de SPIR-V (5 palabras)",
            Reason::NotWordAligned { .. } => "no es una lista de palabras de 32 bits",
            Reason::NotSpirv { .. } => "no empieza por la magia de SPIR-V",
            Reason::BigEndian => "SPIR-V en big-endian: legal, pero nada de lo que BMO-X recibe lo emite",
            Reason::Version { .. } => "version de SPIR-V que no es 1.0 a 1.6",
            Reason::ZeroBound => "bound 0: el modulo no puede definir ningun id",
            Reason::NonZeroSchema { .. } => "esquema distinto de 0 (esta reservado)",
            Reason::IdTableTooSmall { .. } => "la tabla de ids que se dio es mas chica que el bound",
            Reason::EmptyInstruction => "instruccion de 0 palabras",
            Reason::PastEnd { .. } => "instruccion que se sale del fichero",
            Reason::UnknownOpcode { .. } => "instruccion que este lector no conoce",
            Reason::TooShort { .. } => "instruccion con menos palabras de las que necesita",
            Reason::IdOutOfBound { .. } => "id 0 o no menor que el bound",
            Reason::DuplicateId { .. } => "id definido dos veces",
            Reason::UnterminatedString { .. } => "cadena sin su cero dentro de la instruccion",
            Reason::MemoryModelCount { .. } => "OpMemoryModel tiene que estar exactamente una vez",
            Reason::OutOfOrder { .. } => "instruccion fuera del orden de secciones del modulo",
            Reason::NestedFunction => "OpFunction dentro de otra funcion",
            Reason::StrayFunctionEnd => "OpFunctionEnd sin OpFunction",
            Reason::BodyOutsideFunction { .. } => "instruccion de cuerpo fuera de una funcion",
            Reason::UnterminatedFunction => "el fichero se acaba dentro de una funcion",
            Reason::ModeWithoutEntryPoint { .. } => "modo de ejecucion para algo que no es punto de entrada",
            Reason::TooMany { .. } => "mas de las que este lector guarda",
            Reason::UnsupportedCapability { .. } => "capacidad fuera del subconjunto (solo Shader)",
            Reason::UnsupportedExtension => "extension fuera del subconjunto",
            Reason::UnsupportedImport => "instrucciones extendidas que no son GLSL.std.450",
            Reason::UnsupportedMemoryModel { .. } => "modelo que no es Logical + GLSL450",
            Reason::NoEntryPoint => "ningun punto de entrada",
            Reason::UnsupportedStage { .. } => "etapa que no es de computo (GLCompute)",
            Reason::NoLocalSize => "punto de entrada de computo sin LocalSize",
            Reason::UnsupportedMode { .. } => "modo de ejecucion que no es LocalSize",
            Reason::UnsupportedFamily { family, .. } => family.name(),
            Reason::UnsupportedInstruction { why, .. } => why,
            Reason::UnsupportedGlsl { .. } => "instruccion de GLSL.std.450 fuera del subconjunto",
            Reason::GlslLater { .. } => "instruccion de GLSL.std.450 que llega con S3b",
            Reason::UnsupportedType { why } => why,
            Reason::UnsupportedStorageClass { .. } => "clase de almacenamiento fuera del subconjunto",
            Reason::InputWithoutBuiltIn { .. } => "variable de entrada que no es un BuiltIn",
            Reason::UnsupportedBuiltIn { .. } => "BuiltIn que el computo no tiene",
            Reason::NoBinding { .. } => "buffer sin Binding o sin DescriptorSet",
            Reason::NotAType { .. } => "se esperaba un tipo",
            Reason::NotAValue { .. } => "se esperaba un valor",
            Reason::NotAConstant { .. } => "se esperaba una constante",
            Reason::NotALabel { .. } => "se esperaba una etiqueta de esta funcion",
            Reason::Undefined { .. } => "id que nadie define",
            Reason::UsedBeforeDefined { .. } => "valor usado antes de definirse o de otra funcion",
            Reason::TypeMismatch { .. } => "los tipos no cuadran",
            Reason::IndexOutOfRange { .. } => "indice fuera de la medida del compuesto",
            Reason::WriteToInput => "escritura en una variable de entrada",
            Reason::NoBody => "funcion sin bloques",
            Reason::OutsideBlock { .. } => "instruccion fuera de un bloque",
            Reason::UnterminatedBlock => "bloque que no termina en un salto o un retorno",
            Reason::MisplacedPhi => "OpPhi que no esta al principio de su bloque",
            Reason::MisplacedVariable => "OpVariable fuera del principio del primer bloque",
            Reason::MisplacedMerge => "instruccion de fusion que no va justo antes de su salto",
            Reason::Unstructured => "salto condicional sin construccion que lo encierre",
            Reason::EntryPointNotVoidMain => "el punto de entrada no es void main()",
            Reason::WorkspaceTooSmall { .. } => "la memoria de trabajo no alcanza",
            Reason::NoBuffer { .. } => "el sombreador pide un buffer que no se le dio",
            Reason::NoLayout { .. } => "buffer sin Offset o sin ArrayStride",
            Reason::DivisionByZero => "division entera por cero",
            Reason::DivisionOverflow => "division entera que desborda (INT_MIN / -1)",
            Reason::ShiftTooLarge => "desplazamiento de 32 bits o mas",
            Reason::OutOfBounds => "acceso fuera de un buffer o de un arreglo",
            Reason::UndefinedValue { .. } => "valor que esta invocacion no definio",
            Reason::OutOfFuel => "se acabo el combustible: un bucle que no termina",
            Reason::CallTooDeep => "demasiadas llamadas anidadas",
            Reason::ReachedUnreachable => "se ejecuto OpUnreachable",
            Reason::PhiWithoutPredecessor => "OpPhi sin la pareja del bloque de origen",
            Reason::NotYet { what } => what,
        }
    }
}

/// El nombre de un codigo, o "codigo N" si la tabla no lo tiene.
struct NombreOp(u16);

fn nombre_op(opcode: u16) -> NombreOp {
    NombreOp(opcode)
}

impl fmt::Display for NombreOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match crate::op_info(self.0) {
            Some(op_info) => f.write_str(op_info.name),
            None => write!(f, "codigo {}", self.0),
        }
    }
}

/// Un NO: el motivo y la palabra del fichero donde se vio.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Error {
    pub reason: Reason,
    /// Desplazamiento en PALABRAS desde el principio del fichero.
    pub word: usize,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "palabra {}: {}", self.word, self.reason.name())?;
        match self.reason {
            Reason::Short { bytes } | Reason::NotWordAligned { bytes } => write!(f, " ({} bytes)", bytes),
            Reason::NotSpirv { magic } => write!(f, " (0x{:08x})", magic),
            Reason::Version { word } => write!(f, " (0x{:08x})", word),
            Reason::NonZeroSchema { schema } => write!(f, " ({})", schema),
            Reason::IdTableTooSmall { bound, capacity } => write!(f, " (bound {}, caben {})", bound, capacity),
            Reason::PastEnd { words, remaining } => {
                write!(f, " (dice {} palabras, quedan {})", words, remaining)
            }
            Reason::UnknownOpcode { opcode } => write!(f, " (codigo {})", opcode),
            Reason::TooShort { opcode, words, min_words } => {
                write!(f, " (codigo {}: {} palabras, minimo {})", opcode, words, min_words)
            }
            Reason::IdOutOfBound { id }
            | Reason::DuplicateId { id }
            | Reason::ModeWithoutEntryPoint { id }
            | Reason::InputWithoutBuiltIn { id }
            | Reason::NoBinding { id }
            | Reason::NotAType { id }
            | Reason::NotAValue { id }
            | Reason::NotAConstant { id }
            | Reason::NotALabel { id }
            | Reason::Undefined { id }
            | Reason::UsedBeforeDefined { id } => write!(f, " (id {})", id),
            Reason::UnterminatedString { opcode }
            | Reason::OutOfOrder { opcode }
            | Reason::BodyOutsideFunction { opcode }
            | Reason::UnsupportedInstruction { opcode, .. }
            | Reason::TypeMismatch { opcode }
            | Reason::IndexOutOfRange { opcode }
            | Reason::OutsideBlock { opcode } => write!(f, " ({})", nombre_op(opcode)),
            Reason::UnsupportedFamily { opcode, .. } => write!(f, " ({})", nombre_op(opcode)),
            Reason::UnsupportedCapability { capability } => write!(f, " (capacidad {})", capability),
            Reason::UnsupportedMemoryModel { addressing, memory } => {
                write!(f, " (direccionamiento {}, memoria {})", addressing, memory)
            }
            Reason::UnsupportedStage { model } => write!(f, " (modelo {})", model),
            Reason::UnsupportedMode { mode } => write!(f, " (modo {})", mode),
            Reason::UnsupportedGlsl { number } | Reason::GlslLater { number } => {
                write!(f, " (numero {})", number)
            }
            Reason::UnsupportedStorageClass { class } => match class {
                0 => f.write_str(" (UniformConstant: imagenes y muestreadores)"),
                3 => f.write_str(" (Output: solo existe en vertices y fragmentos)"),
                4 => f.write_str(" (Workgroup: memoria de grupo, pide hilos)"),
                9 => f.write_str(" (PushConstant: las pone el pipeline, VERRANO)"),
                c => write!(f, " (clase {})", c),
            },
            Reason::UnsupportedBuiltIn { builtin } => write!(f, " (BuiltIn {})", builtin),
            Reason::WorkspaceTooSmall { need, have } => write!(f, " (hacen falta {} palabras, hay {})", need, have),
            Reason::NoBuffer { set, binding } => write!(f, " (set {}, binding {})", set, binding),
            Reason::NoLayout { id } | Reason::UndefinedValue { id } => write!(f, " (id {})", id),
            Reason::MemoryModelCount { count } => write!(f, " (esta {} veces)", count),
            Reason::TooMany { what, limit } => write!(f, " ({}: tope {})", what, limit),
            _ => Ok(()),
        }
    }
}
