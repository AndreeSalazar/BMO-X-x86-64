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
//! ** EL SOBRE NO CONOCE EMISORES (26-09, `PLAN_EL_AISLAMIENTO.md` A1).
//! Hasta hoy aqui vivian `x86_64_target` y `reproduce`, y por eso este crate
//! enlazaba el emisor x86-64: el sobre de la 3060 arrastraba un emisor de
//! CPU. Ahora cada emisor trae su adaptador al sobre -- el de x86-64 en
//! `bmo-bsf-x86-64`, el SASS de la 3060 en `bmo-gpu-ga10x` -- y aqui queda lo
//! que es de TODOS: el formato y el frontend comun (el SPIR-V).
//!
//! [consumo]  NADA   corre cuando alguien lo pide

use bmo_spirv_front::{interface, read, validate, Error, Module};

use crate::*;

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

/// Un error del SPIR-V, con su sitio dentro del BSF (lo usan tambien los
/// adaptadores de cada emisor).
pub fn spirv_fault(m: &ModuleView, e: Error) -> Fault {
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
}
