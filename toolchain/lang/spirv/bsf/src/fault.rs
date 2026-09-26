//! **Por que NO.** Cada forma de que un BSF no se use, con su nombre y el byte
//! donde se vio. Un NO sin motivo es un SILENCIO (L6i).
//!
//! [consumo]  NADA   solo datos

use core::fmt;

/// Lo que fallo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum What {
    /// Menos bytes que una cabecera.
    Short,
    /// La primera palabra no es `"BSF1"`.
    Magic,
    /// Otra version del formato.
    Version(u16),
    /// La cabecera dice otra medida de cabecera.
    HeaderBytes,
    /// La cabecera dice otra medida de fichero de la que llego.
    TotalBytes { declared: u32, actual: usize },
    /// Mas modulos, buffers u objetivos de los que el formato admite, o cero modulos.
    Limits,
    /// El hash del indice no cuadra: alguien toco una fila o la cabecera.
    IndexHash,
    /// Un byte reservado o de relleno que no es cero.
    NotZero,
    /// Un desplazamiento que no es el canonico.
    Layout,
    /// Un nombre vacio, largo, no ASCII imprimible o repetido.
    Name,
    /// Las filas de un modulo no son las suyas o no van en orden.
    Order,
    /// Un buffer con un acceso o una clase que no existen.
    Access,
    /// Un objetivo que no conoce: bits de CPU desconocidos, ranuras mal hechas.
    Target,
    /// Una entrada (`init`, `main`) fuera de su codigo.
    Entry,
    /// El SPIR-V no empieza por su magia.
    SpirvMagic,
    /// El SPIR-V no es el del hash.
    SpirvHash,
    /// El codigo no es el del hash.
    CodeHash,
    /// El codigo salio de OTRO SPIR-V.
    StaleCode,
    /// Comprobacion profunda: el SPIR-V no se lee o no cabe.
    Spirv(bmo_spirv_front::Error),
    /// Comprobacion profunda: la tabla no dice lo que dice el SPIR-V.
    Lies(&'static str),
    /// Al escribir: el buffer de salida es chico.
    NoRoom { need: usize },
    /// Al despachar: falta el buffer de una fila.
    Missing { set: u32, binding: u32 },
    /// Al despachar: un buffer que el modulo no tiene.
    Extra { set: u32, binding: u32 },
    /// Al despachar: un buffer mas chico que lo que el sombreador lee.
    TooSmall { set: u32, binding: u32, need: u32 },
    /// Al despachar: de solo lectura uno en el que el sombreador escribe.
    ReadOnly { set: u32, binding: u32 },
    /// Al despachar: lo que pasa de los bytes fijos no es un numero ENTERO de
    /// elementos (`stride`). El sombreador leeria medio elemento del final.
    Stride { set: u32, binding: u32, stride: u32 },
}

/// Un NO, con donde.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fault {
    pub what: What,
    /// El byte del fichero donde se vio (0 si no es de un byte).
    pub at: usize,
}

impl Fault {
    /// Un fallo en `at` (lo usan tambien los adaptadores de cada emisor).
    pub const fn at(what: What, at: usize) -> Self {
        Fault { what, at }
    }
}

impl fmt::Display for What {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            What::Short => write!(f, "mas corto que una cabecera"),
            What::Magic => write!(f, "no empieza por BSF1"),
            What::Version(v) => write!(f, "version {} (esta es la 1)", v),
            What::HeaderBytes => write!(f, "la cabecera no mide 64"),
            What::TotalBytes { declared, actual } => write!(f, "dice medir {} y llegaron {}", declared, actual),
            What::Limits => write!(f, "cuentas fuera de los techos"),
            What::IndexHash => write!(f, "el hash del indice no cuadra"),
            What::NotZero => write!(f, "un byte reservado no es cero"),
            What::Layout => write!(f, "un desplazamiento no es el canonico"),
            What::Name => write!(f, "nombre de modulo mal hecho o repetido"),
            What::Order => write!(f, "filas fuera de su sitio o de su orden"),
            What::Access => write!(f, "buffer con acceso o clase que no existen"),
            What::Target => write!(f, "objetivo mal hecho"),
            What::Entry => write!(f, "entrada fuera del codigo"),
            What::SpirvMagic => write!(f, "el SPIR-V no empieza por su magia"),
            What::SpirvHash => write!(f, "el SPIR-V no es el del hash"),
            What::CodeHash => write!(f, "el codigo no es el del hash"),
            What::StaleCode => write!(f, "el codigo salio de otro SPIR-V"),
            What::Spirv(e) => write!(f, "SPIR-V, {}", e),
            What::Lies(que) => write!(f, "la tabla miente: {}", que),
            What::NoRoom { need } => write!(f, "hacen falta {} bytes para escribirlo", need),
            What::Missing { set, binding } => write!(f, "falta el buffer set {} binding {}", set, binding),
            What::Extra { set, binding } => write!(f, "sobra el buffer set {} binding {}", set, binding),
            What::TooSmall { set, binding, need } => write!(f, "buffer set {} binding {} mide menos de {}", set, binding, need),
            What::ReadOnly { set, binding } => write!(f, "buffer set {} binding {} es de solo lectura y el sombreador escribe", set, binding),
            What::Stride { set, binding, stride } => write!(f, "buffer set {} binding {} no es un numero entero de elementos de {} bytes", set, binding, stride),
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BSF: {} (byte {})", self.what, self.at)
    }
}
