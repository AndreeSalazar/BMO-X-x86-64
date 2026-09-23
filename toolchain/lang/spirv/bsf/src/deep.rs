//! **La capa 5: que la tabla no mienta.** Las capas 1 a 4 prueban que el BSF
//! es el que escribio el fabricante; esta prueba que el fabricante dijo la
//! verdad. Relee el SPIR-V con el lector y el juez de S1/S2, saca su interfaz
//! y exige que cada fila sea exactamente esa ([`Bsf::deep`]). Y re-emite el
//! codigo para ver que es el que saldria de ese SPIR-V ([`Bsf::reproduce`]):
//! un BSF reproducible es uno en el que nadie pudo colar otro codigo.
//!
//! Cuestan lo que traducir -- es justo lo que el BSF ahorra --, asi que las
//! pide quien quiere pagarlas: la herramienta al fabricar y el banco siempre;
//! la app en el Ryzen, solo para medirlas.
//!
//! Aqui vive tambien lo que convierte lo emitido en un objetivo
//! ([`x86_64_target`]): las RANURAS salen de comparar el orden del emisor con
//! las filas del modulo.
//!
//! [consumo]  NADA   corre cuando alguien lo pide

use bmo_spirv_front::{interface, read, validate, Error, Module};
use bmo_spirv_x86_64::{emit, tables_words, Program};

use crate::*;

/// Quien emite el objetivo x86-64 v1, como lo guarda la fila.
pub const EMITTER: &[u8] = b"bmo-spirv-x86/1";

/// Lo que el SPIR-V dice de si mismo: lo que va en la fila de un modulo.
#[derive(Clone, Copy, Debug)]
pub struct Facts {
    pub model: u8,
    pub local_size: [u32; 3],
    pub capabilities: u64,
    pub caps_high: u16,
    pub bindings: [Binding; MAX_BINDINGS],
    pub n_bindings: usize,
}

impl Facts {
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings[..self.n_bindings]
    }
}

/// **Los hechos de un modulo**, sacados del SPIR-V: lo juzga y lee su interfaz.
pub fn facts(m: &Module) -> Result<Facts, Error> {
    validate(m)?;
    let it = interface(m)?;
    let mut f = Facts {
        model: it.model as u8,
        local_size: it.local_size,
        capabilities: 0,
        caps_high: 0,
        bindings: [Binding::default(); MAX_BINDINGS],
        n_bindings: it.bindings().len(),
    };
    for &c in m.capabilities() {
        if c < 64 {
            f.capabilities |= 1 << c;
        } else {
            f.caps_high += 1;
        }
    }
    for (k, b) in it.bindings().iter().enumerate() {
        f.bindings[k] = b.into();
    }
    Ok(f)
}

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

fn spirv_fault(m: &ModuleView, e: Error) -> Fault {
    Fault::at(What::Spirv(e), m.spirv_offset() + 4 * e.word)
}

impl<'a> Bsf<'a> {
    /// **La tabla del modulo `i` contra su SPIR-V.** `ids` es la tabla de ids
    /// del lector (una palabra por id del modulo).
    pub fn deep(&self, i: usize, ids: &mut [u32]) -> Result<(), Fault> {
        let mv = self.module(i);
        let m = read(mv.spirv()?, ids).map_err(|e| spirv_fault(&mv, e))?;
        let f = facts(&m).map_err(|e| spirv_fault(&mv, e))?;
        let lies = |que| Err(Fault::at(What::Lies(que), mv.row_offset()));
        if f.model != mv.model() {
            return lies("la etapa");
        }
        if f.local_size != mv.local_size() {
            return lies("el LocalSize");
        }
        if (f.capabilities, f.caps_high) != (mv.capabilities(), mv.caps_high()) {
            return lies("las capacidades");
        }
        if f.n_bindings != mv.binding_count() || !mv.bindings().zip(f.bindings()).all(|(a, b)| a == *b) {
            return lies("los buffers");
        }
        Ok(())
    }

    /// **Re-emite** el modulo `i` y exige que cada objetivo x86-64 v1 sea,
    /// byte a byte, lo que sale. Devuelve cuantos objetivos comprobo; los de
    /// otras maquinas no los sabe emitir y no los cuenta.
    pub fn reproduce(&self, i: usize, ids: &mut [u32], tables: &mut [u32], code: &mut [u8]) -> Result<usize, Fault> {
        let mv = self.module(i);
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
}
