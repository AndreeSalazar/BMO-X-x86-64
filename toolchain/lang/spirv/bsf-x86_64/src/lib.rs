//! **El adaptador del emisor x86-64 al BSF** -- lo que antes vivia DENTRO del
//! sobre (`bmo-bsf/src/deep.rs`) y lo obligaba a enlazar un emisor.
//!
//! [consumo]  NADA   corre cuando alguien fabrica o re-emite un BSF
//!
//! capa: puro -- `no_std`, sin `alloc`, sin `unsafe`
//!
//! # Por que aparte (26-09, `docs/plan/PLAN_EL_AISLAMIENTO.md` A1)
//!
//! El sobre es de TODOS los que escriben en el, y cada uno escribe en su
//! `kind`: `X86_64_SCALAR` sale de aqui; `SM86` (la RTX 3060 12G) sale de
//! `bmo-gpu-ga10x` y lo juzga su juez. Si el sobre conociera a un emisor, una
//! GPU nueva tendria que tocar el sobre -- y un cambio en el sobre es un
//! cambio para todas. Asi, una GPU nueva trae su emisor, su juez y su
//! adaptador, y el sobre no se entera.
//!
//! - [`x86_64_target`]: lo emitido como objetivo x86-64 v1 (las RANURAS salen
//!   de comparar el orden del emisor con las filas del modulo).
//! - [`reproducir`]: re-emite un modulo y exige que cada objetivo x86-64 v1
//!   sea, byte a byte, lo que sale. Un BSF reproducible es uno en el que nadie
//!   pudo colar otro codigo.

#![no_std]
#![forbid(unsafe_code)]

use bmo_bsf::*;
use bmo_spirv_front::read;
use bmo_spirv_x86_64::{emit, tables_words, Program};

/// Quien emite el objetivo x86-64 v1, como lo guarda la fila.
pub const EMITTER: &[u8] = b"bmo-spirv-x86/1";

/// **Lo emitido, como objetivo x86-64 v1**. `slots` es donde se escriben las
/// ranuras; el objetivo las toma prestadas de ahi.
pub fn x86_64_target<'a>(p: &Program, code: &'a [u8], bindings: &[Binding], slots: &'a mut [u8; MAX_BINDINGS]) -> Result<TargetIn<'a>, Fault> {
    if p.n_buffers > MAX_BINDINGS {
        return Err(Fault::at(What::Target, 0));
    }
    for (i, &(set, binding)) in p.buffers[..p.n_buffers].iter().enumerate() {
        let k = bindings.iter().position(|b| (b.set, b.binding) == (set, binding)).ok_or(Fault::at(What::Lies("el codigo usa un buffer que la tabla no tiene"), 0))?;
        slots[i] = k as u8;
    }
    let slots: &'a [u8; MAX_BINDINGS] = slots;
    Ok(TargetIn {
        kind: kind::X86_64_SCALAR,
        abi: abi::X86_64_V1,
        requires: cpu::SSE2,
        code: &code[..p.code_len],
        init: p.init as u32,
        main: p.main as u32,
        frame_words: p.frame_words as u32,
        slots: &slots[..p.n_buffers],
        emitter: EMITTER,
    })
}

/// **Re-emite** el modulo `i` de `bsf` y exige que cada objetivo x86-64 v1
/// sea, byte a byte, lo que sale. Devuelve cuantos objetivos comprobo; los de
/// otras maquinas no son de este emisor y no los cuenta.
pub fn reproducir(bsf: &Bsf, i: usize, ids: &mut [u32], tables: &mut [u32], code: &mut [u8]) -> Result<usize, Fault> {
    let mv = bsf.module(i);
    let mut n = 0;
    for t in mv.targets() {
        if (t.kind(), t.abi()) != (kind::X86_64_SCALAR, abi::X86_64_V1) {
            continue;
        }
        let m = read(mv.spirv()?, &mut *ids).map_err(|e| spirv_fault(&mv, e))?;
        let need = tables_words(&m);
        let tables = tables.get_mut(..need).ok_or(Fault::at(What::NoRoom { need: 4 * need }, 0))?;
        let p = emit(&m, tables, &mut *code).map_err(|e| spirv_fault(&mv, e))?;
        let rows: [Binding; MAX_BINDINGS] = core::array::from_fn(|k| if k < mv.binding_count() { mv.binding(k) } else { Binding::default() });
        let mut slots = [0xFF; MAX_BINDINGS];
        let want = x86_64_target(&p, &code[..p.code_len], &rows[..mv.binding_count()], &mut slots)?;
        let lies = |que| Err(Fault::at(What::Lies(que), mv.row_offset()));
        if want.code != t.code()? {
            return lies("el codigo no es el que sale de su SPIR-V");
        }
        if (want.init as usize, want.main as usize, want.frame_words as usize) != (t.init(), t.main(), t.frame_words()) {
            return lies("las entradas o el marco");
        }
        if want.slots != t.slots() {
            return lies("las ranuras");
        }
        n += 1;
    }
    Ok(n)
}
