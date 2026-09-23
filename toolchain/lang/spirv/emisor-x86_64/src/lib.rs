//! **SPIR-V a x86-64: el emisor del SOMBREADOR (casilla S4).**
//!
//! Traduce un sombreador de computo que el juez acepto a codigo de x86-64 que
//! hace LO MISMO que el oraculo, bit a bit. El banco lo comprueba ejecutando
//! lo emitido en el emulador de `bmo-lower` contra `bmo_spirv_front::Interpreter`
//! (la prueba DIFERENCIAL de PLAN_EL_SOMBREADOR).
//!
//! `no_std` y sin `alloc`, como el frontend: el codigo se escribe en el
//! buffer que da quien llama, y las dos tablas que hacen falta (donde vive
//! cada id y donde cae cada etiqueta) tambien. Es lo que deja que el MISMO
//! emisor corra en el anfitrion (AOT) y dentro de una app de BMO-X (JIT, S5).
//!
//! La API habla ingles (decision del propietario, 23-09); los comentarios y
//! los textos de pantalla, castellano.
//!
//! [consumo]  NADA   emite cuando se le pide; no hay estado vivo

#![no_std]

mod asm;
mod emit;
mod layout;

use bmo_spirv_front::{validate, Error, Module, Reason};

/// Palabras del bloque de ids que `main` recibe en `rdx`.
pub const IDS_WORDS: usize = 13;
/// Cuantos buffers distintos puede declarar un sombreador.
pub const MAX_BUFFERS: usize = 16;

/// Los codigos que `main` devuelve en `eax` cuando PARA.
pub mod trap {
    pub const DIVISION_BY_ZERO: u32 = 1;
    pub const DIVISION_OVERFLOW: u32 = 2;
    pub const SHIFT_TOO_LARGE: u32 = 3;
    pub const OUT_OF_BOUNDS: u32 = 4;
    pub const OUT_OF_FUEL: u32 = 5;
    pub const UNREACHABLE: u32 = 6;
}

/// El motivo de un codigo de trampa: el MISMO `Reason` que da el oraculo.
pub fn trap_reason(code: u32) -> Option<Reason> {
    Some(match code {
        trap::DIVISION_BY_ZERO => Reason::DivisionByZero,
        trap::DIVISION_OVERFLOW => Reason::DivisionOverflow,
        trap::SHIFT_TOO_LARGE => Reason::ShiftTooLarge,
        trap::OUT_OF_BOUNDS => Reason::OutOfBounds,
        trap::OUT_OF_FUEL => Reason::OutOfFuel,
        trap::UNREACHABLE => Reason::ReachedUnreachable,
        _ => return None,
    })
}

/// Lo emitido.
#[derive(Clone, Copy, Debug)]
pub struct Program {
    /// Bytes de codigo escritos.
    pub code_len: usize,
    /// `init(rdi = marco, rsi = buffers)`: una vez por despacho.
    pub init: usize,
    /// `main(rdi, rsi, rdx = ids, rcx = combustible) -> eax`: una vez por invocacion.
    pub main: usize,
    /// Palabras del marco (la memoria de una invocacion).
    pub frame_words: usize,
    /// `(DescriptorSet, Binding)` de cada buffer, en el orden de la tabla.
    pub buffers: [(u32, u32); MAX_BUFFERS],
    pub n_buffers: usize,
    /// `LocalSize` del punto de entrada.
    pub local_size: [u32; 3],
}

/// Cuantas palabras de tabla necesita `emit` para este modulo.
pub fn tables_words(m: &Module) -> usize {
    2 * m.header.bound as usize
}

/// **Emite** el punto de entrada de un modulo. Juzga primero: solo se emite lo
/// que cabe en el subconjunto. `code` demasiado chico se dice con cuanto hace
/// falta (`WorkspaceTooSmall`).
pub fn emit(m: &Module, tables: &mut [u32], code: &mut [u8]) -> Result<Program, Error> {
    validate(m)?;
    let bound = m.header.bound as usize;
    let need = tables_words(m);
    if tables.len() < need {
        return Err(Error { reason: Reason::WorkspaceTooSmall { need, have: tables.len() }, word: 0 });
    }
    let (slots, resto) = tables.split_at_mut(bound);
    let labels = &mut resto[..bound];
    slots.fill(0);
    labels.fill(0);
    let frame_words = layout::plan(m, slots);
    let entry = m.entry_points()[0];
    let slots: &[u32] = slots;

    // Primera pasada: cuenta, y fija donde cae cada etiqueta.
    let mut e = emit::Emitter::new(m, slots, &mut *labels, asm::Writer::counting());
    e.program(entry.id).map_err(|(reason, word)| Error { reason, word })?;
    let size = e.w.pos;
    if code.len() < size {
        return Err(Error { reason: Reason::WorkspaceTooSmall { need: size, have: code.len() }, word: 0 });
    }
    // Segunda: los mismos bytes, con los destinos buenos.
    let mut e = emit::Emitter::new(m, slots, labels, asm::Writer::writing(code));
    let (init, main) = e.program(entry.id).map_err(|(reason, word)| Error { reason, word })?;
    Ok(Program {
        code_len: e.w.pos,
        init,
        main,
        frame_words,
        buffers: e.buffers,
        n_buffers: e.n_buffers,
        local_size: entry.local_size.unwrap_or([1, 1, 1]),
    })
}
