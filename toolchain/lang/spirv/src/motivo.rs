//! **Por que NO.** Cada forma de que un `.spv` no se pueda leer, con su nombre.
//!
//! Un NO sin motivo es un SILENCIO (L6i): quien tiene un sombreador que no
//! carga necesita saber si el fichero esta roto, si es de otra version o si
//! usa algo que este lector no conoce -- son tres arreglos distintos.
//!
//! [consumo]  NADA   solo datos

use core::fmt;

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
            Motivo::IdFuera { id } | Motivo::IdRepetido { id } | Motivo::ModoSinEntrada { id } => {
                write!(f, " (id {})", id)
            }
            Motivo::CadenaSinCero { codigo }
            | Motivo::FueraDeOrden { codigo }
            | Motivo::CuerpoFueraDeFuncion { codigo } => write!(f, " (codigo {})", codigo),
            Motivo::Modelo { veces } => write!(f, " (esta {} veces)", veces),
            Motivo::Demasiadas { que, tope } => write!(f, " ({}: tope {})", que, tope),
            _ => Ok(()),
        }
    }
}
