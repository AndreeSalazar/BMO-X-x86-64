//! **Antes de despachar**: los buffers que le dan al sombreador contra la
//! tabla de su modulo. Es lo que la GPU VE sin abrir el SPIR-V: que esten
//! todos, que midan lo que el codigo lee, y que no le den de solo lectura uno
//! en el que escribe. Y la tabla de buffers en el orden que el CODIGO quiere,
//! que lo pone el formato y no quien llama.
//!
//! [consumo]  NADA   una vez por despacho, antes de la primera invocacion

use crate::*;

/// Un buffer que se le da al sombreador.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Given {
    pub set: u32,
    pub binding: u32,
    /// Donde esta, para la tabla del codigo.
    pub addr: u64,
    pub bytes: u64,
    /// `false` si quien lo da lo tiene de solo lectura.
    pub writable: bool,
}

impl<'a> ModuleView<'a> {
    /// **Comprueba** los buffers que se le dan: uno por fila, ninguno de mas,
    /// cada uno con al menos sus bytes fijos, y escribible si el codigo escribe.
    pub fn check(&self, given: &[Given]) -> Result<(), Fault> {
        for g in given {
            let n = given.iter().filter(|h| (h.set, h.binding) == (g.set, g.binding)).count();
            let nuestro = self.bindings().any(|b| (b.set, b.binding) == (g.set, g.binding));
            if n > 1 || !nuestro {
                return Err(Fault::at(What::Extra { set: g.set, binding: g.binding }, 0));
            }
        }
        for b in self.bindings() {
            let (set, binding) = (b.set, b.binding);
            let g = given.iter().find(|g| (g.set, g.binding) == (set, binding)).ok_or(Fault::at(What::Missing { set, binding }, 0))?;
            if g.bytes < b.base_bytes as u64 {
                return Err(Fault::at(What::TooSmall { set, binding, need: b.base_bytes }, 0));
            }
            // ** Y un numero ENTERO de elementos (26-09): el SPIR-V dice cuanto
            // mide cada uno (`stride`), y quien da el buffer --INTI, la API--
            // tiene que dar elementos enteros. Si no, el sombreador leeria el
            // ultimo a medias: memoria que nadie escribio.
            if b.stride > 0 && (g.bytes - b.base_bytes as u64) % b.stride as u64 != 0 {
                return Err(Fault::at(What::Stride { set, binding, stride: b.stride }, 0));
            }
            if b.access & WRITES != 0 && !g.writable {
                return Err(Fault::at(What::ReadOnly { set, binding }, 0));
            }
        }
        Ok(())
    }
}

impl<'a> TargetView<'a> {
    /// **La tabla de buffers del codigo**, `(direccion, bytes)` por ranura, en
    /// `out`. Comprueba antes ([`ModuleView::check`]). Devuelve cuantas
    /// palabras escribio.
    pub fn table(&self, module: &ModuleView, given: &[Given], out: &mut [u64]) -> Result<usize, Fault> {
        module.check(given)?;
        let slots = self.slots();
        if out.len() < 2 * slots.len() {
            return Err(Fault::at(What::NoRoom { need: 2 * slots.len() }, 0));
        }
        for (i, &k) in slots.iter().enumerate() {
            let b = module.binding(k as usize);
            // `check` ya vio que esta.
            let g = given.iter().find(|g| (g.set, g.binding) == (b.set, b.binding)).ok_or(Fault::at(What::Missing { set: b.set, binding: b.binding }, 0))?;
            out[2 * i] = g.addr;
            out[2 * i + 1] = g.bytes;
        }
        Ok(2 * slots.len())
    }
}
