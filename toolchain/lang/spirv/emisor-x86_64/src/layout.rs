//! **La disposicion**: donde vive cada valor en el MARCO, y donde cae cada
//! escalar dentro de un BUFFER.
//!
//! El marco es la memoria de una invocacion (`rdi` apunta a el): un sitio
//! FIJO por id, porque SPIR-V prohibe la recursion. Lo mismo que el oraculo,
//! calculado aqui para el emisor. Ademas, en el sitio del id de cada:
//!
//! - `OpVariable` de la arena: el puntero `[0, off]` y, justo detras, su
//!   ALMACEN (`off` = sitio + 2);
//! - `OpPhi`: el valor y, justo detras, su SOMBRA (la copia del borde, para
//!   que dos `OpPhi` que se leen entre si no se pisen);
//! - `OpFunction`: el sitio de lo que DEVUELVE.
//!
//! [consumo]  NADA   se calcula al emitir

use bmo_spirv_front::table::op;
use bmo_spirv_front::{op_info, Module};

pub const CLASS_INPUT: u32 = 1;
pub const CLASS_UNIFORM: u32 = 2;
pub const CLASS_PRIVATE: u32 = 6;
pub const CLASS_STORAGE_BUFFER: u32 = 12;

pub const DEC_ARRAY_STRIDE: u32 = 6;
pub const DEC_BUILTIN: u32 = 11;
pub const DEC_BINDING: u32 = 33;
pub const DEC_DESCRIPTOR_SET: u32 = 34;
pub const DEC_OFFSET: u32 = 35;

/// Un puntero de esta clase apunta a un buffer (su desplazamiento son BYTES).
pub fn is_buffer(class: u32) -> bool {
    class == CLASS_UNIFORM || class == CLASS_STORAGE_BUFFER
}

/// Cuantas palabras mide un valor de tipo `t` en el marco (juntas).
pub fn dense(m: &Module, t: u32) -> u32 {
    let Some(d) = m.def(t) else { return 0 };
    match d.opcode {
        op::OpTypeBool | op::OpTypeInt | op::OpTypeFloat => 1,
        op::OpTypeVector => d.op(3),
        op::OpTypeArray => array_len(m, t).saturating_mul(dense(m, d.op(2))),
        op::OpTypeStruct => (2..d.words as usize).map(|k| dense(m, d.op(k))).sum(),
        op::OpTypePointer => 2,
        _ => 0,
    }
}

/// La longitud de un `OpTypeArray` (su constante).
pub fn array_len(m: &Module, t: u32) -> u32 {
    m.def(t).and_then(|d| m.def(d.op(3))).map(|c| c.op(3)).unwrap_or(0)
}

/// `(clase, apuntado)` de un tipo puntero.
pub fn pointer(m: &Module, t: u32) -> (u32, u32) {
    match m.def(t) {
        Some(d) if d.opcode == op::OpTypePointer => (d.op(2), d.op(3)),
        _ => (0, 0),
    }
}

/// El tipo de un valor.
pub fn type_of(m: &Module, id: u32) -> u32 {
    m.def(id).map(|d| d.op(1)).unwrap_or(0)
}

pub fn decoration(m: &Module, id: u32, dec: u32) -> Option<u32> {
    m.instructions()
        .take_while(|i| i.opcode != op::OpFunction)
        .find(|i| i.opcode == op::OpDecorate && i.op(1) == id && i.op(2) == dec)
        .map(|i| i.op(3))
}

pub fn member_offset(m: &Module, id: u32, member: u32) -> Option<u32> {
    m.instructions()
        .take_while(|i| i.opcode != op::OpFunction)
        .find(|i| i.opcode == op::OpMemberDecorate && i.op(1) == id && i.op(2) == member && i.op(3) == DEC_OFFSET)
        .map(|i| i.op(4))
}

/// Un paso por un compuesto en el MARCO con un indice CONSTANTE: cuantas
/// palabras avanzar y el tipo al que se llega.
pub fn dense_step(m: &Module, t: u32, i: u32) -> Option<(u32, u32)> {
    let d = m.def(t)?;
    match d.opcode {
        op::OpTypeVector if i < d.op(3) => Some((i, d.op(2))),
        op::OpTypeArray if i < array_len(m, t) => Some((i * dense(m, d.op(2)), d.op(2))),
        op::OpTypeStruct if (i as usize) + 2 < d.words as usize => {
            let antes: u32 = (0..i as usize).map(|k| dense(m, d.op(2 + k))).sum();
            Some((antes, d.op(2 + i as usize)))
        }
        _ => None,
    }
}

/// **Reparte el marco**: escribe en `slots[id]` la palabra donde vive cada
/// valor. Devuelve cuantas palabras mide el marco.
pub fn plan(m: &Module, slots: &mut [u32]) -> usize {
    let mut next: u32 = 0;
    for ins in m.instructions() {
        let Some(info) = op_info(ins.opcode) else { continue };
        if ins.opcode == op::OpFunction {
            // El sitio de lo que devuelve.
            let id = ins.op(2);
            slots[id as usize] = next;
            next += dense(m, ins.op(1));
            continue;
        }
        if !info.has_result_type {
            continue;
        }
        let id = ins.op(2);
        let len = dense(m, ins.op(1));
        slots[id as usize] = next;
        next += len;
        match ins.opcode {
            op::OpPhi => next += len, // la sombra
            op::OpVariable => {
                let (class, pointee) = pointer(m, ins.op(1));
                if !is_buffer(class) {
                    next += dense(m, pointee);
                }
            }
            _ => {}
        }
    }
    next as usize
}

/// Una hoja escalar de un valor guardado en un buffer: su byte (relativo al
/// valor) y su palabra dentro del valor en el marco.
#[derive(Clone, Copy)]
pub struct Leaf {
    pub byte: u32,
    pub word: u32,
}

/// **Las hojas de un tipo en un buffer**, en orden, llamando a `f` por cada
/// una. `Err` si falta una decoracion (el mismo `NoLayout` que el oraculo) o si
/// hay mas de `limit` (un arreglo enorme cargado de golpe: se dice).
pub fn leaves(m: &Module, t: u32, byte: u32, word: &mut u32, limit: &mut u32, f: &mut dyn FnMut(Leaf)) -> Result<(), u32> {
    let d = m.def(t).ok_or(t)?;
    match d.opcode {
        op::OpTypeBool | op::OpTypeInt | op::OpTypeFloat => {
            if *limit == 0 {
                return Err(u32::MAX);
            }
            *limit -= 1;
            f(Leaf { byte, word: *word });
            *word += 1;
            Ok(())
        }
        op::OpTypeVector => {
            for k in 0..d.op(3) {
                leaves(m, d.op(2), byte + 4 * k, word, limit, f)?;
            }
            Ok(())
        }
        op::OpTypeArray => {
            let stride = decoration(m, t, DEC_ARRAY_STRIDE).ok_or(t)?;
            for i in 0..array_len(m, t) {
                leaves(m, d.op(2), byte + i * stride, word, limit, f)?;
            }
            Ok(())
        }
        op::OpTypeStruct => {
            for k in 0..d.words as u32 - 2 {
                let off = member_offset(m, t, k).ok_or(t)?;
                leaves(m, d.op(2 + k as usize), byte + off, word, limit, f)?;
            }
            Ok(())
        }
        _ => Err(t),
    }
}
