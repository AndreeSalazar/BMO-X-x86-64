//! **EL LECTOR (S1): bytes de SPIR-V a un modulo que se puede recorrer.**
//!
//! Una pasada, sin copiar nada: el [`Module`] presta los bytes que se le
//! dieron y la tabla de ids que dio quien llama. Lo que comprueba es la FORMA
//! --cabecera, medida de cada instruccion, ids, cadenas, orden de secciones,
//! funciones que abren y cierran--, no el SIGNIFICADO: que los tipos cuadren o
//! que una instruccion este en el subconjunto es del juez (S2).
//!
//! ** Los bytes son de un TERCERO. Toda lectura de palabra pasa por
//! [`word`], que no puede salirse; y ninguna cuenta puede desbordar porque
//! las medidas son `u16` sobre un `usize` que ya se comparo con lo que queda.
//!
//! [consumo]  NADA   una pasada por peticion; sin estado entre llamadas

use crate::{op_info, Error, Reason, Section, MAGIC};

/// Cuantas capacidades guarda. Un sombreador de verdad declara dos o tres.
pub const MAX_CAPABILITIES: usize = 32;
/// Cuantos puntos de entrada guarda.
pub const MAX_ENTRY_POINTS: usize = 8;
/// Cuantas importaciones de instrucciones extendidas guarda.
pub const MAX_IMPORTS: usize = 8;
/// Cuantas extensiones (`OpExtension`) guarda.
pub const MAX_EXTENSIONS: usize = 8;

const OP_EXTENSION: u16 = 10;
const OP_EXT_INST_IMPORT: u16 = 11;
const OP_EXT_INST: u16 = 12;
const OP_MEMORY_MODEL: u16 = 14;
const OP_ENTRY_POINT: u16 = 15;
const OP_EXECUTION_MODE: u16 = 16;
const OP_CAPABILITY: u16 = 17;
/// `ExecutionMode LocalSize`: las tres medidas del grupo de trabajo.
const MODO_LOCAL_SIZE: u32 = 17;

/// La palabra `i` (en palabras) de `bytes`, en little-endian. Quien llama ya
/// comprobo que existe; si no, devuelve 0 en vez de salirse.
fn word(bytes: &[u8], i: usize) -> u32 {
    match bytes.get(i * 4..i * 4 + 4) {
        Some(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        None => 0,
    }
}

/// Lo que dice la cabecera.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Header {
    pub major: u8,
    pub minor: u8,
    /// Quien lo fabrico (registro de Khronos). Solo se informa.
    pub generator: u32,
    /// Todo id del modulo es menor que esto.
    pub bound: u32,
}

/// Una instruccion, prestada del fichero.
#[derive(Clone, Copy, Debug)]
pub struct Instruction<'a> {
    pub opcode: u16,
    /// Palabras que ocupa, la cabecera incluida.
    pub words: u16,
    /// Donde empieza, en palabras desde el principio del fichero.
    pub offset: usize,
    bytes: &'a [u8],
}

impl<'a> Instruction<'a> {
    /// La palabra `i` de la instruccion (0 es la de codigo y medida). Fuera de
    /// rango devuelve 0: quien pregunta por un operando opcional lo mira antes
    /// con [`Instruction::words`].
    pub fn op(&self, i: usize) -> u32 {
        word(self.bytes, i)
    }

    /// La cadena que empieza en la palabra `i`, sin su cero. `None` si no
    /// termina dentro de la instruccion.
    pub fn string(&self, i: usize) -> Option<&'a [u8]> {
        let offset = i.checked_mul(4)?;
        let resto = self.bytes.get(offset..)?;
        let fin = resto.iter().position(|&b| b == 0)?;
        Some(&resto[..fin])
    }

    /// El id que define, si define uno.
    pub fn result_id(&self) -> Option<u32> {
        let f = op_info(self.opcode)?;
        if !f.has_result {
            return None;
        }
        Some(self.op(if f.has_result_type { 2 } else { 1 }))
    }

    /// El id de su tipo de resultado, si lleva.
    pub fn result_type(&self) -> Option<u32> {
        let f = op_info(self.opcode)?;
        if f.has_result_type {
            Some(self.op(1))
        } else {
            None
        }
    }
}

/// Un punto de entrada.
#[derive(Clone, Copy, Debug)]
pub struct EntryPoint<'a> {
    /// `ExecutionModel`: 5 = `GLCompute`.
    pub model: u32,
    /// El id de su `OpFunction`.
    pub id: u32,
    pub name: &'a [u8],
    /// `LocalSize`, si el modulo lo declara.
    pub local_size: Option<[u32; 3]>,
}

/// Una importacion de instrucciones extendidas (`OpExtInstImport`).
#[derive(Clone, Copy, Debug)]
pub struct Import<'a> {
    pub id: u32,
    pub name: &'a [u8],
}

/// Un modulo LEIDO: la forma esta bien. El significado lo juzga S2.
pub struct Module<'a, 'b> {
    bytes: &'a [u8],
    ids: &'b [u32],
    pub header: Header,
    capabilities: [u32; MAX_CAPABILITIES],
    n_capacidades: usize,
    extensions: [&'a [u8]; MAX_EXTENSIONS],
    n_extensiones: usize,
    imports: [Import<'a>; MAX_IMPORTS],
    n_importaciones: usize,
    /// `(addressing, memory)` de `OpMemoryModel`.
    pub memory_model: (u32, u32),
    entry_points: [EntryPoint<'a>; MAX_ENTRY_POINTS],
    n_entradas: usize,
    /// Cuantas `OpFunction`.
    pub functions: usize,
    /// Cuantas instrucciones, todas.
    pub instruction_count: usize,
}

impl<'a, 'b> Module<'a, 'b> {
    pub fn capabilities(&self) -> &[u32] {
        &self.capabilities[..self.n_capacidades]
    }

    pub fn extensions(&self) -> &[&'a [u8]] {
        &self.extensions[..self.n_extensiones]
    }

    pub fn imports(&self) -> &[Import<'a>] {
        &self.imports[..self.n_importaciones]
    }

    pub fn entry_points(&self) -> &[EntryPoint<'a>] {
        &self.entry_points[..self.n_entradas]
    }

    /// El id de la importacion `GLSL.std.450`, si la hay.
    pub fn glsl450(&self) -> Option<u32> {
        self.imports()
            .iter()
            .find(|i| i.name == b"GLSL.std.450")
            .map(|i| i.id)
    }

    /// Todas las instrucciones, en orden.
    pub fn instructions(&self) -> Instructions<'a> {
        Instructions { bytes: self.bytes, i: 5 }
    }

    /// Las instrucciones desde la que empieza en la palabra `offset`.
    pub fn instructions_from(&self, offset: usize) -> Instructions<'a> {
        Instructions { bytes: self.bytes, i: offset }
    }

    /// La instruccion que define `id`.
    pub fn def(&self, id: u32) -> Option<Instruction<'a>> {
        let offset = *self.ids.get(id as usize)? as usize;
        if offset == 0 {
            return None;
        }
        instr_en(self.bytes, offset)
    }
}

/// La instruccion que empieza en la palabra `offset` de un fichero YA LEIDO.
fn instr_en(bytes: &[u8], offset: usize) -> Option<Instruction<'_>> {
    let total = bytes.len() / 4;
    if offset >= total {
        return None;
    }
    let w = word(bytes, offset);
    let words = (w >> 16) as u16;
    if words == 0 || offset + words as usize > total {
        return None;
    }
    Some(Instruction {
        opcode: w as u16,
        words,
        offset,
        bytes: &bytes[offset * 4..(offset + words as usize) * 4],
    })
}

/// El recorrido de un modulo ya leido.
pub struct Instructions<'a> {
    bytes: &'a [u8],
    i: usize,
}

impl<'a> Iterator for Instructions<'a> {
    type Item = Instruction<'a>;
    fn next(&mut self) -> Option<Instruction<'a>> {
        let ins = instr_en(self.bytes, self.i)?;
        self.i += ins.words as usize;
        Some(ins)
    }
}

/// **Lee un modulo.** `ids` es la memoria que da quien llama: una palabra por
/// id posible (`bound`), y el lector apunta ahi donde se define cada uno. Se
/// pone a cero entera antes de empezar.
pub fn read<'a, 'b>(bytes: &'a [u8], ids: &'b mut [u32]) -> Result<Module<'a, 'b>, Error> {
    let no = |reason, word| Err(Error { reason, word });

    // -- La cabecera ---------------------------------------------------------
    if bytes.len() < 20 {
        return no(Reason::Short { bytes: bytes.len() }, 0);
    }
    if bytes.len() % 4 != 0 {
        return no(Reason::NotWordAligned { bytes: bytes.len() }, 0);
    }
    let magic = word(bytes, 0);
    if magic == MAGIC.swap_bytes() {
        return no(Reason::BigEndian, 0);
    }
    if magic != MAGIC {
        return no(Reason::NotSpirv { magic }, 0);
    }
    let version = word(bytes, 1);
    let major = (version >> 16) as u8;
    let minor = (version >> 8) as u8;
    if version & 0xFF00_00FF != 0 || major != 1 || minor > 6 {
        return no(Reason::Version { word: version }, 1);
    }
    let bound = word(bytes, 3);
    if bound == 0 {
        return no(Reason::ZeroBound, 3);
    }
    let schema = word(bytes, 4);
    if schema != 0 {
        return no(Reason::NonZeroSchema { schema }, 4);
    }
    if ids.len() < bound as usize {
        return no(Reason::IdTableTooSmall { bound, capacity: ids.len() }, 3);
    }
    for x in ids.iter_mut() {
        *x = 0;
    }

    let mut m = Module {
        bytes,
        ids: &[],
        header: Header { major, minor, generator: word(bytes, 2), bound },
        capabilities: [0; MAX_CAPABILITIES],
        n_capacidades: 0,
        extensions: [&[]; MAX_EXTENSIONS],
        n_extensiones: 0,
        imports: [Import { id: 0, name: &[] }; MAX_IMPORTS],
        n_importaciones: 0,
        memory_model: (0, 0),
        entry_points: [EntryPoint { model: 0, id: 0, name: &[], local_size: None }; MAX_ENTRY_POINTS],
        n_entradas: 0,
        functions: 0,
        instruction_count: 0,
    };

    // -- Las instrucciones ---------------------------------------------------
    let total = bytes.len() / 4;
    let mut i = 5;
    let mut ultima = Section::Capability;
    let mut dentro = false;
    let mut hubo_funcion = false;
    let mut modelos = 0u32;
    let id_valido = |id: u32| id != 0 && id < bound;

    while i < total {
        let w = word(bytes, i);
        let words = (w >> 16) as u16;
        let opcode = w as u16;
        if words == 0 {
            return no(Reason::EmptyInstruction, i);
        }
        if words as usize > total - i {
            return no(Reason::PastEnd { words, remaining: total - i }, i);
        }
        let ins = Instruction { opcode, words, offset: i, bytes: &bytes[i * 4..(i + words as usize) * 4] };
        let Some(f) = op_info(opcode) else {
            return no(Reason::UnknownOpcode { opcode }, i);
        };
        if words < f.min_words as u16 {
            return no(Reason::TooShort { opcode, words, min_words: f.min_words }, i);
        }

        // El orden de la disposicion logica.
        match f.section {
            Section::Function => {
                if dentro {
                    return no(Reason::NestedFunction, i);
                }
                dentro = true;
                hubo_funcion = true;
                m.functions += 1;
            }
            Section::FunctionEnd => {
                if !dentro {
                    return no(Reason::StrayFunctionEnd, i);
                }
                dentro = false;
            }
            // ** `OpExtInst` fuera de una funcion es LEGAL con los conjuntos
            // no semanticos (`NonSemantic.*`, SPV_KHR_non_semantic_info): es
            // informacion de depuracion entre los tipos. El juez niega la
            // importacion; el lector no tiene por que negar la forma.
            Section::Body if opcode == OP_EXT_INST && !dentro => {
                if hubo_funcion || ultima > Section::Type {
                    return no(Reason::OutOfOrder { opcode }, i);
                }
                ultima = Section::Type;
            }
            Section::Body => {
                if !dentro {
                    return no(Reason::BodyOutsideFunction { opcode }, i);
                }
            }
            Section::Flexible => {
                if !dentro {
                    // Global: va entre los tipos, y no despues de las funciones.
                    if hubo_funcion || ultima > Section::Type {
                        return no(Reason::OutOfOrder { opcode }, i);
                    }
                    ultima = Section::Type;
                }
            }
            s => {
                if dentro || hubo_funcion || s < ultima {
                    return no(Reason::OutOfOrder { opcode }, i);
                }
                ultima = s;
            }
        }

        // El tipo y el id que define.
        if f.has_result_type && !id_valido(ins.op(1)) {
            return no(Reason::IdOutOfBound { id: ins.op(1) }, i);
        }
        if f.has_result {
            let id = ins.op(if f.has_result_type { 2 } else { 1 });
            if !id_valido(id) {
                return no(Reason::IdOutOfBound { id }, i);
            }
            if ids[id as usize] != 0 {
                return no(Reason::DuplicateId { id }, i);
            }
            ids[id as usize] = i as u32;
        }

        // La cadena de sitio fijo.
        let cadena = if f.string_word != 0 {
            match ins.string(f.string_word as usize) {
                Some(c) => c,
                None => return no(Reason::UnterminatedString { opcode }, i),
            }
        } else {
            &[]
        };

        // Lo que el modulo declara de si mismo.
        match opcode {
            OP_CAPABILITY => {
                if m.n_capacidades == MAX_CAPABILITIES {
                    return no(Reason::TooMany { what: "capacidades", limit: MAX_CAPABILITIES }, i);
                }
                m.capabilities[m.n_capacidades] = ins.op(1);
                m.n_capacidades += 1;
            }
            OP_EXTENSION => {
                if m.n_extensiones == MAX_EXTENSIONS {
                    return no(Reason::TooMany { what: "extensiones", limit: MAX_EXTENSIONS }, i);
                }
                m.extensions[m.n_extensiones] = cadena;
                m.n_extensiones += 1;
            }
            OP_EXT_INST_IMPORT => {
                if m.n_importaciones == MAX_IMPORTS {
                    return no(Reason::TooMany { what: "importaciones", limit: MAX_IMPORTS }, i);
                }
                m.imports[m.n_importaciones] = Import { id: ins.op(1), name: cadena };
                m.n_importaciones += 1;
            }
            OP_MEMORY_MODEL => {
                modelos += 1;
                if modelos > 1 {
                    return no(Reason::MemoryModelCount { count: modelos }, i);
                }
                m.memory_model = (ins.op(1), ins.op(2));
            }
            OP_ENTRY_POINT => {
                if m.n_entradas == MAX_ENTRY_POINTS {
                    return no(Reason::TooMany { what: "puntos de entrada", limit: MAX_ENTRY_POINTS }, i);
                }
                let id = ins.op(2);
                if !id_valido(id) {
                    return no(Reason::IdOutOfBound { id }, i);
                }
                m.entry_points[m.n_entradas] = EntryPoint { model: ins.op(1), id, name: cadena, local_size: None };
                m.n_entradas += 1;
            }
            OP_EXECUTION_MODE if ins.op(2) == MODO_LOCAL_SIZE => {
                if words < 6 {
                    return no(Reason::TooShort { opcode, words, min_words: 6 }, i);
                }
                let id = ins.op(1);
                let n = m.n_entradas;
                let Some(e) = m.entry_points[..n].iter_mut().find(|e| e.id == id) else {
                    return no(Reason::ModeWithoutEntryPoint { id }, i);
                };
                e.local_size = Some([ins.op(3), ins.op(4), ins.op(5)]);
            }
            _ => {}
        }

        m.instruction_count += 1;
        i += words as usize;
    }

    if dentro {
        return no(Reason::UnterminatedFunction, total);
    }
    if modelos != 1 {
        return no(Reason::MemoryModelCount { count: modelos }, total);
    }
    m.ids = ids;
    Ok(m)
}
