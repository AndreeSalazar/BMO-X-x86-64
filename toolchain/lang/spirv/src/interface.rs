//! **La INTERFAZ de un modulo**: que buffers toca, con que forma y para que.
//!
//! Es lo que el consumidor tiene que saber ANTES de despachar --que le den un
//! buffer por cada `(set, binding)`, que mida al menos lo que el sombreador lee,
//! que no le den de solo lectura uno que escribe-- y que hasta hoy solo sabia el
//! que habia escrito el GLSL. El BSF (S6) la guarda en una tabla para que la
//! GPU la VEA sin abrir el SPIR-V; esto es de donde sale esa tabla, y con lo que
//! el BSF comprueba que su tabla no miente.
//!
//! == De donde sale cada columna ==
//!
//! - `set`, `binding`: las decoraciones de la variable.
//! - `storage`: clase `StorageBuffer`, o `Uniform` con el struct `BufferBlock`
//!   (la forma de Vulkan 1.0, la que emite glslc por defecto). Si no, es un
//!   bloque uniforme.
//! - `base_bytes`, `stride`: las decoraciones `Offset` y `ArrayStride`. Un
//!   arreglo sin medida (`OpTypeRuntimeArray`) solo puede ir al final del
//!   bloque; `base_bytes` es donde empieza y `stride` lo que mide cada elemento.
//! - `access`: lo que el sombreador HACE, no lo que declara. Se sigue cada
//!   `OpLoad`/`OpStore`/atomica hasta su variable. `NonWritable` es una promesa
//!   del autor; esto es un hecho del codigo.
//!
//! Si un puntero no se puede seguir hasta su variable (un parametro de funcion,
//! por ejemplo) la respuesta es la prudente: TODOS los buffers leen y escriben.
//! Decir de menos haria que el consumidor mapeara de solo lectura algo que se
//! escribe; decir de mas solo cuesta una proteccion que no se pone.
//!
//! [consumo]  NADA   se calcula cuando alguien lo pide

use crate::table::op;
use crate::{Error, Module, Reason};

/// Cuantos buffers guarda la interfaz. El mismo techo que el emisor.
pub const MAX_BINDINGS: usize = 16;

/// El sombreador lee de este buffer.
pub const READS: u8 = 1;
/// El sombreador escribe en este buffer.
pub const WRITES: u8 = 2;

const CLASS_UNIFORM: u32 = 2;
const CLASS_STORAGE_BUFFER: u32 = 12;

const DEC_BUFFER_BLOCK: u32 = 3;
const DEC_ARRAY_STRIDE: u32 = 6;
const DEC_MATRIX_STRIDE: u32 = 7;
const DEC_BINDING: u32 = 33;
const DEC_DESCRIPTOR_SET: u32 = 34;
const DEC_OFFSET: u32 = 35;

/// Un buffer de la interfaz.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Binding {
    pub set: u32,
    pub binding: u32,
    /// `true` si es de almacenamiento (se puede escribir); `false` si es un
    /// bloque uniforme.
    pub storage: bool,
    /// [`READS`] | [`WRITES`], segun lo que hace el codigo.
    pub access: u8,
    /// Bytes fijos del bloque: todo, o hasta donde empieza su arreglo sin medida.
    pub base_bytes: u32,
    /// Lo que mide cada elemento del arreglo sin medida; 0 si no lo hay.
    pub stride: u32,
    /// El id de su `OpVariable`.
    pub variable: u32,
}

impl Binding {
    /// Cuantos bytes hacen falta como minimo: la parte fija.
    pub fn min_bytes(&self) -> u32 {
        self.base_bytes
    }
}

/// La interfaz del primer punto de entrada.
#[derive(Clone, Copy, Debug)]
pub struct Interface<'a> {
    pub name: &'a [u8],
    /// `ExecutionModel`: 5 = `GLCompute`.
    pub model: u32,
    pub local_size: [u32; 3],
    bindings: [Binding; MAX_BINDINGS],
    n: usize,
}

impl<'a> Interface<'a> {
    /// Los buffers, ordenados por `(set, binding)`.
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings[..self.n]
    }
}

/// **La interfaz** del primer punto de entrada de `m`. `m` tiene que estar
/// JUZGADO (`validate`): esto no repite las comprobaciones del juez.
pub fn interface<'a>(m: &Module<'a, '_>) -> Result<Interface<'a>, Error> {
    let entry = *m.entry_points().first().ok_or(Error { reason: Reason::NoEntryPoint, word: 0 })?;
    let mut it = Interface {
        name: entry.name,
        model: entry.model,
        local_size: entry.local_size.unwrap_or([1, 1, 1]),
        bindings: [Binding::default(); MAX_BINDINGS],
        n: 0,
    };

    // Las variables de buffer, con su forma.
    for ins in m.instructions() {
        if ins.opcode == op::OpFunction {
            break;
        }
        if ins.opcode != op::OpVariable {
            continue;
        }
        let class = ins.op(3);
        if class != CLASS_UNIFORM && class != CLASS_STORAGE_BUFFER {
            continue;
        }
        let id = ins.op(2);
        let fail = |reason| Error { reason, word: ins.offset };
        let pointee = match m.def(ins.op(1)) {
            Some(p) if p.opcode == op::OpTypePointer => p.op(3),
            _ => return Err(fail(Reason::NotAType { id: ins.op(1) })),
        };
        let set = decoration(m, id, DEC_DESCRIPTOR_SET).ok_or(fail(Reason::NoBinding { id }))?;
        let binding = decoration(m, id, DEC_BINDING).ok_or(fail(Reason::NoBinding { id }))?;
        if it.n == MAX_BINDINGS {
            return Err(fail(Reason::TooMany { what: "buffers en la interfaz", limit: MAX_BINDINGS }));
        }
        if it.bindings().iter().any(|b| b.set == set && b.binding == binding) {
            return Err(fail(Reason::DuplicateId { id }));
        }
        let storage = class == CLASS_STORAGE_BUFFER || decoration(m, pointee, DEC_BUFFER_BLOCK).is_some();
        let (base_bytes, stride) = block_shape(m, pointee).map_err(|t| fail(Reason::NoLayout { id: t }))?;
        it.bindings[it.n] = Binding { set, binding, storage, access: 0, base_bytes, stride, variable: id };
        it.n += 1;
    }

    // Lo que hace el codigo con cada una.
    let mut unknown = false;
    for ins in m.instructions() {
        let (ptr, what) = match ins.opcode {
            op::OpLoad | op::OpAtomicLoad => (ins.op(3), READS),
            op::OpStore | op::OpAtomicStore => (ins.op(1), WRITES),
            op::OpCopyMemory => {
                mark(m, &mut it, ins.op(2), READS, &mut unknown);
                (ins.op(1), WRITES)
            }
            op::OpAtomicExchange..=op::OpAtomicXor => (ins.op(3), READS | WRITES),
            _ => continue,
        };
        mark(m, &mut it, ptr, what, &mut unknown);
    }
    if unknown {
        for b in &mut it.bindings[..it.n] {
            b.access = READS | WRITES;
        }
    }

    // Orden canonico: el mismo modulo da siempre la misma tabla.
    it.bindings[..it.n].sort_unstable_by_key(|b| (b.set, b.binding));
    Ok(it)
}

/// Sigue un puntero hasta su variable y le apunta el acceso. Si no llega a una
/// variable conocida --y no es de la arena-- lo dice en `unknown`.
fn mark(m: &Module, it: &mut Interface, mut ptr: u32, what: u8, unknown: &mut bool) {
    // La clase va en el TIPO del puntero: uno de la arena (`Function`,
    // `Private`, `Input`) no es de nadie de la interfaz, venga de donde venga.
    let class = m.def(ptr).and_then(|d| m.def(d.op(1))).filter(|t| t.opcode == op::OpTypePointer).map(|t| t.op(2));
    if matches!(class, Some(c) if c != CLASS_UNIFORM && c != CLASS_STORAGE_BUFFER) {
        return;
    }
    for _ in 0..64 {
        let Some(d) = m.def(ptr) else { break };
        match d.opcode {
            op::OpAccessChain | op::OpInBoundsAccessChain | op::OpCopyObject => ptr = d.op(3),
            op::OpVariable => {
                let class = d.op(3);
                if class != CLASS_UNIFORM && class != CLASS_STORAGE_BUFFER {
                    return;
                }
                if let Some(b) = it.bindings[..it.n].iter_mut().find(|b| b.variable == ptr) {
                    b.access |= what;
                    return;
                }
                break;
            }
            _ => break,
        }
    }
    *unknown = true;
}

/// `(base_bytes, stride)` del bloque de tipo `t`: `Err(id)` del tipo al que le
/// falta una decoracion, o de un arreglo sin medida que no va al final.
fn block_shape(m: &Module, t: u32) -> Result<(u32, u32), u32> {
    let d = m.def(t).ok_or(t)?;
    if d.opcode != op::OpTypeStruct {
        return Ok((size(m, t)?, 0));
    }
    let members = d.words as u32 - 2;
    let mut end = 0u32;
    for k in 0..members {
        let mt = d.op(2 + k as usize);
        let off = member_decoration(m, t, k, DEC_OFFSET).ok_or(t)?;
        let md = m.def(mt).ok_or(mt)?;
        if md.opcode == op::OpTypeRuntimeArray {
            if k + 1 != members {
                return Err(mt);
            }
            let stride = decoration(m, mt, DEC_ARRAY_STRIDE).ok_or(mt)?;
            return Ok((off.max(end), stride));
        }
        end = end.max(off.checked_add(size(m, mt)?).ok_or(mt)?);
    }
    Ok((end, 0))
}

/// Bytes que ocupa un valor de tipo `t` en un buffer.
fn size(m: &Module, t: u32) -> Result<u32, u32> {
    let d = m.def(t).ok_or(t)?;
    match d.opcode {
        op::OpTypeBool => Ok(4),
        op::OpTypeInt | op::OpTypeFloat => Ok(d.op(2) / 8),
        op::OpTypeVector => size(m, d.op(2))?.checked_mul(d.op(3)).ok_or(t),
        op::OpTypeMatrix => {
            let stride = member_matrix_stride(m, t).ok_or(t)?;
            stride.checked_mul(d.op(3)).ok_or(t)
        }
        op::OpTypeArray => {
            let len = m.def(d.op(3)).map(|c| c.op(3)).ok_or(t)?;
            let stride = decoration(m, t, DEC_ARRAY_STRIDE).ok_or(t)?;
            stride.checked_mul(len).ok_or(t)
        }
        op::OpTypeStruct => match block_shape(m, t)? {
            (bytes, 0) => Ok(bytes),
            _ => Err(t),
        },
        _ => Err(t),
    }
}

/// `MatrixStride` va en el MIEMBRO que contiene la matriz, no en su tipo: se
/// busca cualquier miembro de tipo `t` que lo diga.
fn member_matrix_stride(m: &Module, t: u32) -> Option<u32> {
    let mut ins = m.instructions().take_while(|i| i.opcode != op::OpFunction);
    ins.find_map(|i| {
        if i.opcode != op::OpMemberDecorate || i.op(3) != DEC_MATRIX_STRIDE {
            return None;
        }
        let s = m.def(i.op(1))?;
        (s.op(2 + i.op(2) as usize) == t).then(|| i.op(4))
    })
}

fn decoration(m: &Module, id: u32, dec: u32) -> Option<u32> {
    m.instructions()
        .take_while(|i| i.opcode != op::OpFunction)
        .find(|i| i.opcode == op::OpDecorate && i.op(1) == id && i.op(2) == dec)
        .map(|i| if i.words > 3 { i.op(3) } else { 0 })
}

fn member_decoration(m: &Module, id: u32, member: u32, dec: u32) -> Option<u32> {
    m.instructions()
        .take_while(|i| i.opcode != op::OpFunction)
        .find(|i| i.opcode == op::OpMemberDecorate && i.op(1) == id && i.op(2) == member && i.op(3) == dec)
        .map(|i| i.op(4))
}
