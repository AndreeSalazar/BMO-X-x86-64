//! **EL LECTOR (S1): bytes de SPIR-V a un modulo que se puede recorrer.**
//!
//! Una pasada, sin copiar nada: el [`Modulo`] presta los bytes que se le
//! dieron y la tabla de ids que dio quien llama. Lo que comprueba es la FORMA
//! --cabecera, medida de cada instruccion, ids, cadenas, orden de secciones,
//! funciones que abren y cierran--, no el SIGNIFICADO: que los tipos cuadren o
//! que una instruccion este en el subconjunto es del juez (S2).
//!
//! ** Los bytes son de un TERCERO. Toda lectura de palabra pasa por
//! [`palabra`], que no puede salirse; y ninguna cuenta puede desbordar porque
//! las medidas son `u16` sobre un `usize` que ya se comparo con lo que queda.
//!
//! [consumo]  NADA   una pasada por peticion; sin estado entre llamadas

use crate::{fila, Fallo, Motivo, Seccion, MAGIA};

/// Cuantas capacidades guarda. Un sombreador de verdad declara dos o tres.
pub const MAX_CAPACIDADES: usize = 32;
/// Cuantos puntos de entrada guarda.
pub const MAX_ENTRADAS: usize = 8;
/// Cuantas importaciones de instrucciones extendidas guarda.
pub const MAX_IMPORTACIONES: usize = 8;
/// Cuantas extensiones (`OpExtension`) guarda.
pub const MAX_EXTENSIONES: usize = 8;

const OP_EXTENSION: u16 = 10;
const OP_EXT_INST_IMPORT: u16 = 11;
const OP_MEMORY_MODEL: u16 = 14;
const OP_ENTRY_POINT: u16 = 15;
const OP_EXECUTION_MODE: u16 = 16;
const OP_CAPABILITY: u16 = 17;
/// `ExecutionMode LocalSize`: las tres medidas del grupo de trabajo.
const MODO_LOCAL_SIZE: u32 = 17;

/// La palabra `i` (en palabras) de `bytes`, en little-endian. Quien llama ya
/// comprobo que existe; si no, devuelve 0 en vez de salirse.
fn palabra(bytes: &[u8], i: usize) -> u32 {
    match bytes.get(i * 4..i * 4 + 4) {
        Some(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        None => 0,
    }
}

/// Lo que dice la cabecera.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cabecera {
    pub mayor: u8,
    pub menor: u8,
    /// Quien lo fabrico (registro de Khronos). Solo se informa.
    pub generador: u32,
    /// Todo id del modulo es menor que esto.
    pub bound: u32,
}

/// Una instruccion, prestada del fichero.
#[derive(Clone, Copy, Debug)]
pub struct Instr<'a> {
    pub codigo: u16,
    /// Palabras que ocupa, la cabecera incluida.
    pub palabras: u16,
    /// Donde empieza, en palabras desde el principio del fichero.
    pub desde: usize,
    bytes: &'a [u8],
}

impl<'a> Instr<'a> {
    /// La palabra `i` de la instruccion (0 es la de codigo y medida). Fuera de
    /// rango devuelve 0: quien pregunta por un operando opcional lo mira antes
    /// con [`Instr::palabras`].
    pub fn op(&self, i: usize) -> u32 {
        palabra(self.bytes, i)
    }

    /// La cadena que empieza en la palabra `i`, sin su cero. `None` si no
    /// termina dentro de la instruccion.
    pub fn cadena(&self, i: usize) -> Option<&'a [u8]> {
        let desde = i.checked_mul(4)?;
        let resto = self.bytes.get(desde..)?;
        let fin = resto.iter().position(|&b| b == 0)?;
        Some(&resto[..fin])
    }

    /// El id que define, si define uno.
    pub fn resultado(&self) -> Option<u32> {
        let f = fila(self.codigo)?;
        if !f.resultado {
            return None;
        }
        Some(self.op(if f.tipo { 2 } else { 1 }))
    }

    /// El id de su tipo de resultado, si lleva.
    pub fn tipo(&self) -> Option<u32> {
        let f = fila(self.codigo)?;
        if f.tipo {
            Some(self.op(1))
        } else {
            None
        }
    }
}

/// Un punto de entrada.
#[derive(Clone, Copy, Debug)]
pub struct Entrada<'a> {
    /// `ExecutionModel`: 5 = `GLCompute`.
    pub modelo: u32,
    /// El id de su `OpFunction`.
    pub id: u32,
    pub nombre: &'a [u8],
    /// `LocalSize`, si el modulo lo declara.
    pub local: Option<[u32; 3]>,
}

/// Una importacion de instrucciones extendidas (`OpExtInstImport`).
#[derive(Clone, Copy, Debug)]
pub struct Importacion<'a> {
    pub id: u32,
    pub nombre: &'a [u8],
}

/// Un modulo LEIDO: la forma esta bien. El significado lo juzga S2.
pub struct Modulo<'a, 'b> {
    bytes: &'a [u8],
    ids: &'b [u32],
    pub cabecera: Cabecera,
    capacidades: [u32; MAX_CAPACIDADES],
    n_capacidades: usize,
    extensiones: [&'a [u8]; MAX_EXTENSIONES],
    n_extensiones: usize,
    importaciones: [Importacion<'a>; MAX_IMPORTACIONES],
    n_importaciones: usize,
    /// `(direccionamiento, memoria)` de `OpMemoryModel`.
    pub modelo: (u32, u32),
    entradas: [Entrada<'a>; MAX_ENTRADAS],
    n_entradas: usize,
    /// Cuantas `OpFunction`.
    pub funciones: usize,
    /// Cuantas instrucciones, todas.
    pub instrucciones: usize,
}

impl<'a, 'b> Modulo<'a, 'b> {
    pub fn capacidades(&self) -> &[u32] {
        &self.capacidades[..self.n_capacidades]
    }

    pub fn extensiones(&self) -> &[&'a [u8]] {
        &self.extensiones[..self.n_extensiones]
    }

    pub fn importaciones(&self) -> &[Importacion<'a>] {
        &self.importaciones[..self.n_importaciones]
    }

    pub fn entradas(&self) -> &[Entrada<'a>] {
        &self.entradas[..self.n_entradas]
    }

    /// El id de la importacion `GLSL.std.450`, si la hay.
    pub fn glsl450(&self) -> Option<u32> {
        self.importaciones()
            .iter()
            .find(|i| i.nombre == b"GLSL.std.450")
            .map(|i| i.id)
    }

    /// Todas las instrucciones, en orden.
    pub fn recorrer(&self) -> Instrucciones<'a> {
        Instrucciones { bytes: self.bytes, i: 5 }
    }

    /// La instruccion que define `id`.
    pub fn def(&self, id: u32) -> Option<Instr<'a>> {
        let desde = *self.ids.get(id as usize)? as usize;
        if desde == 0 {
            return None;
        }
        instr_en(self.bytes, desde)
    }
}

/// La instruccion que empieza en la palabra `desde` de un fichero YA LEIDO.
fn instr_en(bytes: &[u8], desde: usize) -> Option<Instr<'_>> {
    let total = bytes.len() / 4;
    if desde >= total {
        return None;
    }
    let w = palabra(bytes, desde);
    let palabras = (w >> 16) as u16;
    if palabras == 0 || desde + palabras as usize > total {
        return None;
    }
    Some(Instr {
        codigo: w as u16,
        palabras,
        desde,
        bytes: &bytes[desde * 4..(desde + palabras as usize) * 4],
    })
}

/// El recorrido de un modulo ya leido.
pub struct Instrucciones<'a> {
    bytes: &'a [u8],
    i: usize,
}

impl<'a> Iterator for Instrucciones<'a> {
    type Item = Instr<'a>;
    fn next(&mut self) -> Option<Instr<'a>> {
        let ins = instr_en(self.bytes, self.i)?;
        self.i += ins.palabras as usize;
        Some(ins)
    }
}

/// **Lee un modulo.** `ids` es la memoria que da quien llama: una palabra por
/// id posible (`bound`), y el lector apunta ahi donde se define cada uno. Se
/// pone a cero entera antes de empezar.
pub fn leer<'a, 'b>(bytes: &'a [u8], ids: &'b mut [u32]) -> Result<Modulo<'a, 'b>, Fallo> {
    let no = |motivo, palabra| Err(Fallo { motivo, palabra });

    // -- La cabecera ---------------------------------------------------------
    if bytes.len() < 20 {
        return no(Motivo::Corto { bytes: bytes.len() }, 0);
    }
    if bytes.len() % 4 != 0 {
        return no(Motivo::NoMultiploDe4 { bytes: bytes.len() }, 0);
    }
    let magia = palabra(bytes, 0);
    if magia == MAGIA.swap_bytes() {
        return no(Motivo::AlReves, 0);
    }
    if magia != MAGIA {
        return no(Motivo::NoEsSpirv { magia }, 0);
    }
    let version = palabra(bytes, 1);
    let mayor = (version >> 16) as u8;
    let menor = (version >> 8) as u8;
    if version & 0xFF00_00FF != 0 || mayor != 1 || menor > 6 {
        return no(Motivo::Version { palabra: version }, 1);
    }
    let bound = palabra(bytes, 3);
    if bound == 0 {
        return no(Motivo::BoundCero, 3);
    }
    let esquema = palabra(bytes, 4);
    if esquema != 0 {
        return no(Motivo::EsquemaNoCero { esquema }, 4);
    }
    if ids.len() < bound as usize {
        return no(Motivo::TablaDeIdsChica { bound, cabe: ids.len() }, 3);
    }
    for x in ids.iter_mut() {
        *x = 0;
    }

    let mut m = Modulo {
        bytes,
        ids: &[],
        cabecera: Cabecera { mayor, menor, generador: palabra(bytes, 2), bound },
        capacidades: [0; MAX_CAPACIDADES],
        n_capacidades: 0,
        extensiones: [&[]; MAX_EXTENSIONES],
        n_extensiones: 0,
        importaciones: [Importacion { id: 0, nombre: &[] }; MAX_IMPORTACIONES],
        n_importaciones: 0,
        modelo: (0, 0),
        entradas: [Entrada { modelo: 0, id: 0, nombre: &[], local: None }; MAX_ENTRADAS],
        n_entradas: 0,
        funciones: 0,
        instrucciones: 0,
    };

    // -- Las instrucciones ---------------------------------------------------
    let total = bytes.len() / 4;
    let mut i = 5;
    let mut ultima = Seccion::Capacidad;
    let mut dentro = false;
    let mut hubo_funcion = false;
    let mut modelos = 0u32;
    let id_valido = |id: u32| id != 0 && id < bound;

    while i < total {
        let w = palabra(bytes, i);
        let palabras = (w >> 16) as u16;
        let codigo = w as u16;
        if palabras == 0 {
            return no(Motivo::InstruccionVacia, i);
        }
        if palabras as usize > total - i {
            return no(Motivo::SeSaleDelFichero { palabras, quedan: total - i }, i);
        }
        let ins = Instr { codigo, palabras, desde: i, bytes: &bytes[i * 4..(i + palabras as usize) * 4] };
        let Some(f) = fila(codigo) else {
            return no(Motivo::SinFila { codigo }, i);
        };
        if palabras < f.minimo as u16 {
            return no(Motivo::Corta { codigo, palabras, minimo: f.minimo }, i);
        }

        // El orden de la disposicion logica.
        match f.seccion {
            Seccion::Funcion => {
                if dentro {
                    return no(Motivo::FuncionDentroDeFuncion, i);
                }
                dentro = true;
                hubo_funcion = true;
                m.funciones += 1;
            }
            Seccion::FinFuncion => {
                if !dentro {
                    return no(Motivo::FinSinFuncion, i);
                }
                dentro = false;
            }
            Seccion::Cuerpo => {
                if !dentro {
                    return no(Motivo::CuerpoFueraDeFuncion { codigo }, i);
                }
            }
            Seccion::Flexible => {
                if !dentro {
                    // Global: va entre los tipos, y no despues de las funciones.
                    if hubo_funcion || ultima > Seccion::Tipo {
                        return no(Motivo::FueraDeOrden { codigo }, i);
                    }
                    ultima = Seccion::Tipo;
                }
            }
            s => {
                if dentro || hubo_funcion || s < ultima {
                    return no(Motivo::FueraDeOrden { codigo }, i);
                }
                ultima = s;
            }
        }

        // El tipo y el id que define.
        if f.tipo && !id_valido(ins.op(1)) {
            return no(Motivo::IdFuera { id: ins.op(1) }, i);
        }
        if f.resultado {
            let id = ins.op(if f.tipo { 2 } else { 1 });
            if !id_valido(id) {
                return no(Motivo::IdFuera { id }, i);
            }
            if ids[id as usize] != 0 {
                return no(Motivo::IdRepetido { id }, i);
            }
            ids[id as usize] = i as u32;
        }

        // La cadena de sitio fijo.
        let cadena = if f.cadena != 0 {
            match ins.cadena(f.cadena as usize) {
                Some(c) => c,
                None => return no(Motivo::CadenaSinCero { codigo }, i),
            }
        } else {
            &[]
        };

        // Lo que el modulo declara de si mismo.
        match codigo {
            OP_CAPABILITY => {
                if m.n_capacidades == MAX_CAPACIDADES {
                    return no(Motivo::Demasiadas { que: "capacidades", tope: MAX_CAPACIDADES }, i);
                }
                m.capacidades[m.n_capacidades] = ins.op(1);
                m.n_capacidades += 1;
            }
            OP_EXTENSION => {
                if m.n_extensiones == MAX_EXTENSIONES {
                    return no(Motivo::Demasiadas { que: "extensiones", tope: MAX_EXTENSIONES }, i);
                }
                m.extensiones[m.n_extensiones] = cadena;
                m.n_extensiones += 1;
            }
            OP_EXT_INST_IMPORT => {
                if m.n_importaciones == MAX_IMPORTACIONES {
                    return no(Motivo::Demasiadas { que: "importaciones", tope: MAX_IMPORTACIONES }, i);
                }
                m.importaciones[m.n_importaciones] = Importacion { id: ins.op(1), nombre: cadena };
                m.n_importaciones += 1;
            }
            OP_MEMORY_MODEL => {
                modelos += 1;
                if modelos > 1 {
                    return no(Motivo::Modelo { veces: modelos }, i);
                }
                m.modelo = (ins.op(1), ins.op(2));
            }
            OP_ENTRY_POINT => {
                if m.n_entradas == MAX_ENTRADAS {
                    return no(Motivo::Demasiadas { que: "puntos de entrada", tope: MAX_ENTRADAS }, i);
                }
                let id = ins.op(2);
                if !id_valido(id) {
                    return no(Motivo::IdFuera { id }, i);
                }
                m.entradas[m.n_entradas] = Entrada { modelo: ins.op(1), id, nombre: cadena, local: None };
                m.n_entradas += 1;
            }
            OP_EXECUTION_MODE if ins.op(2) == MODO_LOCAL_SIZE => {
                if palabras < 6 {
                    return no(Motivo::Corta { codigo, palabras, minimo: 6 }, i);
                }
                let id = ins.op(1);
                let n = m.n_entradas;
                let Some(e) = m.entradas[..n].iter_mut().find(|e| e.id == id) else {
                    return no(Motivo::ModoSinEntrada { id }, i);
                };
                e.local = Some([ins.op(3), ins.op(4), ins.op(5)]);
            }
            _ => {}
        }

        m.instrucciones += 1;
        i += palabras as usize;
    }

    if dentro {
        return no(Motivo::FuncionSinFin, total);
    }
    if modelos != 1 {
        return no(Motivo::Modelo { veces: modelos }, total);
    }
    m.ids = ids;
    Ok(m)
}
