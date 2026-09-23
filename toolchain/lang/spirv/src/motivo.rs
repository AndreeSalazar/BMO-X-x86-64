//! **Por que NO.** Cada forma de que un `.spv` no se pueda leer, con su nombre.
//!
//! Un NO sin motivo es un SILENCIO (L6i): quien tiene un sombreador que no
//! carga necesita saber si el fichero esta roto, si es de otra version o si
//! usa algo que este lector no conoce -- son tres arreglos distintos.
//!
//! [consumo]  NADA   solo datos

use core::fmt;

use crate::Familia;

/// Lo que fallo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Motivo {
    /// Menos de las cinco palabras de la cabecera.
    Corto { bytes: usize },
    /// SPIR-V es una lista de palabras de 32 bits.
    NoMultiploDe4 { bytes: usize },
    /// La primera palabra no es la magia.
    NoEsSpirv { magia: u32 },
    /// La magia al reves: un modulo en big-endian. Es SPIR-V legal, pero nada
    /// de lo que BMO-X recibe lo emite, y darle la vuelta es trabajo sin cliente.
    AlReves,
    /// Version que no es 1.0 a 1.6, o palabra de version mal formada.
    Version { palabra: u32 },
    /// `bound` es 0: no cabe ni un id.
    BoundCero,
    /// El esquema esta reservado y vale 0.
    EsquemaNoCero { esquema: u32 },
    /// La tabla de ids que dio quien llama es mas chica que `bound`.
    TablaDeIdsChica { bound: u32, cabe: usize },
    /// Una instruccion que dice medir 0 palabras: el recorrido no avanzaria.
    InstruccionVacia,
    /// Una instruccion que dice medir mas de lo que queda del fichero.
    SeSaleDelFichero { palabras: u16, quedan: usize },
    /// Un codigo que la tabla no tiene. Se dice el numero.
    SinFila { codigo: u16 },
    /// Menos palabras de las que la instruccion necesita.
    Corta { codigo: u16, palabras: u16, minimo: u8 },
    /// Un id que es 0 o no es menor que `bound`.
    IdFuera { id: u32 },
    /// Dos instrucciones definen el mismo id.
    IdRepetido { id: u32 },
    /// Una cadena que no termina en su cero dentro de su instruccion.
    CadenaSinCero { codigo: u16 },
    /// `OpMemoryModel` tiene que estar exactamente una vez.
    Modelo { veces: u32 },
    /// Una instruccion de una seccion anterior a la ultima vista.
    FueraDeOrden { codigo: u16 },
    /// `OpFunction` sin cerrar la anterior.
    FuncionDentroDeFuncion,
    /// `OpFunctionEnd` sin `OpFunction`.
    FinSinFuncion,
    /// Una instruccion de cuerpo fuera de toda funcion.
    CuerpoFueraDeFuncion { codigo: u16 },
    /// El fichero se acaba dentro de una funcion.
    FuncionSinFin,
    /// Un modo de ejecucion para un id que no es punto de entrada.
    ModoSinEntrada { id: u32 },
    /// Mas de las que este lector guarda. Es un techo DICHO, no un error del fichero.
    Demasiadas { que: &'static str, tope: usize },

    // ---- EL JUEZ (S2): el modulo esta bien formado, pero no cabe ----------
    /// Una capacidad que no es `Shader`.
    CapacidadFuera { capacidad: u32 },
    /// Una extension que el subconjunto no conoce.
    ExtensionFuera,
    /// Instrucciones extendidas que no son `GLSL.std.450`.
    ImportacionFuera,
    /// Direccionamiento o memoria que no son `Logical` + `GLSL450`.
    ModeloFuera { direccionamiento: u32, memoria: u32 },
    /// Ningun punto de entrada.
    SinEntrada,
    /// Una etapa que no es `GLCompute`.
    EtapaFuera { modelo: u32 },
    /// Un punto de entrada de computo sin `LocalSize`.
    SinLocalSize,
    /// Un modo de ejecucion que no es `LocalSize`.
    ModoFuera { modo: u32 },
    /// Una instruccion de una familia que no es el nucleo.
    FamiliaFuera { familia: Familia, codigo: u16 },
    /// Una instruccion del nucleo que el subconjunto aun asi no acepta.
    InstruccionFuera { codigo: u16, porque: &'static str },
    /// Una instruccion de `GLSL.std.450` que no esta en la tabla.
    ExtInstFuera { numero: u32 },
    /// Una de `GLSL.std.450` que llega con S3b (seno, coseno, exp, log, pow).
    ExtInstLuego { numero: u32 },
    /// Un tipo que no cabe: entero o flotante que no es de 32 bits, vector de
    /// mas de 4...
    TipoFuera { porque: &'static str },
    /// Una clase de almacenamiento fuera del subconjunto.
    ClaseFuera { clase: u32 },
    /// Una variable de entrada que no es un `BuiltIn`.
    EntradaSinBuiltIn { id: u32 },
    /// Un `BuiltIn` que el computo no tiene.
    BuiltInFuera { builtin: u32 },
    /// Un buffer sin `Binding` o sin `DescriptorSet`: nadie sabria cual es.
    SinBinding { id: u32 },
    /// Se esperaba un tipo.
    NoEsTipo { id: u32 },
    /// Se esperaba un valor.
    NoEsValor { id: u32 },
    /// Se esperaba una constante.
    NoEsConstante { id: u32 },
    /// Se esperaba una etiqueta de esta misma funcion.
    NoEsEtiqueta { id: u32 },
    /// Un id que nadie define.
    NoDefinido { id: u32 },
    /// Un valor usado antes de definirse, o que es de otra funcion.
    UsoAntesDeDefinir { id: u32 },
    /// Los tipos de una instruccion no cuadran.
    TipoNoCuadra { codigo: u16 },
    /// Un indice de composicion fuera de su medida.
    IndiceFuera { codigo: u16 },
    /// Escribir en una variable de entrada.
    EscrituraEnEntrada,
    /// Una funcion sin ningun bloque.
    SinCuerpo,
    /// Una instruccion fuera de un bloque (antes de su `OpLabel`).
    FueraDeBloque { codigo: u16 },
    /// Un bloque que no termina en un salto o un retorno.
    BloqueSinTerminar,
    /// `OpPhi` que no esta al principio de su bloque.
    PhiFueraDeSitio,
    /// `OpVariable` de funcion que no esta al principio del primer bloque.
    VariableFueraDeSitio,
    /// Una instruccion de fusion que no va justo antes de su salto.
    MergeFueraDeSitio,
    /// Un salto condicional sin construccion que lo encierre.
    SaltoSinEstructura,
    /// Un punto de entrada que no es `void main()`.
    EntradaNoCuadra,
}

impl Motivo {
    /// El motivo en palabras, sin los numeros.
    pub fn nombre(&self) -> &'static str {
        match self {
            Motivo::Corto { .. } => "mas corto que la cabecera de SPIR-V (5 palabras)",
            Motivo::NoMultiploDe4 { .. } => "no es una lista de palabras de 32 bits",
            Motivo::NoEsSpirv { .. } => "no empieza por la magia de SPIR-V",
            Motivo::AlReves => "SPIR-V en big-endian: legal, pero nada de lo que BMO-X recibe lo emite",
            Motivo::Version { .. } => "version de SPIR-V que no es 1.0 a 1.6",
            Motivo::BoundCero => "bound 0: el modulo no puede definir ningun id",
            Motivo::EsquemaNoCero { .. } => "esquema distinto de 0 (esta reservado)",
            Motivo::TablaDeIdsChica { .. } => "la tabla de ids que se dio es mas chica que el bound",
            Motivo::InstruccionVacia => "instruccion de 0 palabras",
            Motivo::SeSaleDelFichero { .. } => "instruccion que se sale del fichero",
            Motivo::SinFila { .. } => "instruccion que este lector no conoce",
            Motivo::Corta { .. } => "instruccion con menos palabras de las que necesita",
            Motivo::IdFuera { .. } => "id 0 o no menor que el bound",
            Motivo::IdRepetido { .. } => "id definido dos veces",
            Motivo::CadenaSinCero { .. } => "cadena sin su cero dentro de la instruccion",
            Motivo::Modelo { .. } => "OpMemoryModel tiene que estar exactamente una vez",
            Motivo::FueraDeOrden { .. } => "instruccion fuera del orden de secciones del modulo",
            Motivo::FuncionDentroDeFuncion => "OpFunction dentro de otra funcion",
            Motivo::FinSinFuncion => "OpFunctionEnd sin OpFunction",
            Motivo::CuerpoFueraDeFuncion { .. } => "instruccion de cuerpo fuera de una funcion",
            Motivo::FuncionSinFin => "el fichero se acaba dentro de una funcion",
            Motivo::ModoSinEntrada { .. } => "modo de ejecucion para algo que no es punto de entrada",
            Motivo::Demasiadas { .. } => "mas de las que este lector guarda",
            Motivo::CapacidadFuera { .. } => "capacidad fuera del subconjunto (solo Shader)",
            Motivo::ExtensionFuera => "extension fuera del subconjunto",
            Motivo::ImportacionFuera => "instrucciones extendidas que no son GLSL.std.450",
            Motivo::ModeloFuera { .. } => "modelo que no es Logical + GLSL450",
            Motivo::SinEntrada => "ningun punto de entrada",
            Motivo::EtapaFuera { .. } => "etapa que no es de computo (GLCompute)",
            Motivo::SinLocalSize => "punto de entrada de computo sin LocalSize",
            Motivo::ModoFuera { .. } => "modo de ejecucion que no es LocalSize",
            Motivo::FamiliaFuera { familia, .. } => familia.nombre(),
            Motivo::InstruccionFuera { porque, .. } => porque,
            Motivo::ExtInstFuera { .. } => "instruccion de GLSL.std.450 fuera del subconjunto",
            Motivo::ExtInstLuego { .. } => "instruccion de GLSL.std.450 que llega con S3b",
            Motivo::TipoFuera { porque } => porque,
            Motivo::ClaseFuera { .. } => "clase de almacenamiento fuera del subconjunto",
            Motivo::EntradaSinBuiltIn { .. } => "variable de entrada que no es un BuiltIn",
            Motivo::BuiltInFuera { .. } => "BuiltIn que el computo no tiene",
            Motivo::SinBinding { .. } => "buffer sin Binding o sin DescriptorSet",
            Motivo::NoEsTipo { .. } => "se esperaba un tipo",
            Motivo::NoEsValor { .. } => "se esperaba un valor",
            Motivo::NoEsConstante { .. } => "se esperaba una constante",
            Motivo::NoEsEtiqueta { .. } => "se esperaba una etiqueta de esta funcion",
            Motivo::NoDefinido { .. } => "id que nadie define",
            Motivo::UsoAntesDeDefinir { .. } => "valor usado antes de definirse o de otra funcion",
            Motivo::TipoNoCuadra { .. } => "los tipos no cuadran",
            Motivo::IndiceFuera { .. } => "indice fuera de la medida del compuesto",
            Motivo::EscrituraEnEntrada => "escritura en una variable de entrada",
            Motivo::SinCuerpo => "funcion sin bloques",
            Motivo::FueraDeBloque { .. } => "instruccion fuera de un bloque",
            Motivo::BloqueSinTerminar => "bloque que no termina en un salto o un retorno",
            Motivo::PhiFueraDeSitio => "OpPhi que no esta al principio de su bloque",
            Motivo::VariableFueraDeSitio => "OpVariable fuera del principio del primer bloque",
            Motivo::MergeFueraDeSitio => "instruccion de fusion que no va justo antes de su salto",
            Motivo::SaltoSinEstructura => "salto condicional sin construccion que lo encierre",
            Motivo::EntradaNoCuadra => "el punto de entrada no es void main()",
        }
    }
}

/// El nombre de un codigo, o "codigo N" si la tabla no lo tiene.
struct NombreOp(u16);

fn nombre_op(codigo: u16) -> NombreOp {
    NombreOp(codigo)
}

impl fmt::Display for NombreOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match crate::fila(self.0) {
            Some(fila) => f.write_str(fila.nombre),
            None => write!(f, "codigo {}", self.0),
        }
    }
}

/// Un NO: el motivo y la palabra del fichero donde se vio.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fallo {
    pub motivo: Motivo,
    /// Desplazamiento en PALABRAS desde el principio del fichero.
    pub palabra: usize,
}

impl fmt::Display for Fallo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "palabra {}: {}", self.palabra, self.motivo.nombre())?;
        match self.motivo {
            Motivo::Corto { bytes } | Motivo::NoMultiploDe4 { bytes } => write!(f, " ({} bytes)", bytes),
            Motivo::NoEsSpirv { magia } => write!(f, " (0x{:08x})", magia),
            Motivo::Version { palabra } => write!(f, " (0x{:08x})", palabra),
            Motivo::EsquemaNoCero { esquema } => write!(f, " ({})", esquema),
            Motivo::TablaDeIdsChica { bound, cabe } => write!(f, " (bound {}, caben {})", bound, cabe),
            Motivo::SeSaleDelFichero { palabras, quedan } => {
                write!(f, " (dice {} palabras, quedan {})", palabras, quedan)
            }
            Motivo::SinFila { codigo } => write!(f, " (codigo {})", codigo),
            Motivo::Corta { codigo, palabras, minimo } => {
                write!(f, " (codigo {}: {} palabras, minimo {})", codigo, palabras, minimo)
            }
            Motivo::IdFuera { id }
            | Motivo::IdRepetido { id }
            | Motivo::ModoSinEntrada { id }
            | Motivo::EntradaSinBuiltIn { id }
            | Motivo::SinBinding { id }
            | Motivo::NoEsTipo { id }
            | Motivo::NoEsValor { id }
            | Motivo::NoEsConstante { id }
            | Motivo::NoEsEtiqueta { id }
            | Motivo::NoDefinido { id }
            | Motivo::UsoAntesDeDefinir { id } => write!(f, " (id {})", id),
            Motivo::CadenaSinCero { codigo }
            | Motivo::FueraDeOrden { codigo }
            | Motivo::CuerpoFueraDeFuncion { codigo }
            | Motivo::InstruccionFuera { codigo, .. }
            | Motivo::TipoNoCuadra { codigo }
            | Motivo::IndiceFuera { codigo }
            | Motivo::FueraDeBloque { codigo } => write!(f, " ({})", nombre_op(codigo)),
            Motivo::FamiliaFuera { codigo, .. } => write!(f, " ({})", nombre_op(codigo)),
            Motivo::CapacidadFuera { capacidad } => write!(f, " (capacidad {})", capacidad),
            Motivo::ModeloFuera { direccionamiento, memoria } => {
                write!(f, " (direccionamiento {}, memoria {})", direccionamiento, memoria)
            }
            Motivo::EtapaFuera { modelo } => write!(f, " (modelo {})", modelo),
            Motivo::ModoFuera { modo } => write!(f, " (modo {})", modo),
            Motivo::ExtInstFuera { numero } | Motivo::ExtInstLuego { numero } => {
                write!(f, " (numero {})", numero)
            }
            Motivo::ClaseFuera { clase } => match clase {
                0 => f.write_str(" (UniformConstant: imagenes y muestreadores)"),
                3 => f.write_str(" (Output: solo existe en vertices y fragmentos)"),
                4 => f.write_str(" (Workgroup: memoria de grupo, pide hilos)"),
                9 => f.write_str(" (PushConstant: las pone el pipeline, VERRANO)"),
                c => write!(f, " (clase {})", c),
            },
            Motivo::BuiltInFuera { builtin } => write!(f, " (BuiltIn {})", builtin),
            Motivo::Modelo { veces } => write!(f, " (esta {} veces)", veces),
            Motivo::Demasiadas { que, tope } => write!(f, " ({}: tope {})", que, tope),
            _ => Ok(()),
        }
    }
}
