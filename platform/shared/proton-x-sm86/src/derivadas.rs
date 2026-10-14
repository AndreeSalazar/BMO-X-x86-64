//! **L2: LAS DERIVADAS EN LA 3060** (14-10) -- `ddx`, `ddy` y la fina en x
//! de un sombreador de pixeles, con los MISMOS bits que la casa
//! (`prometeo::olas`, `Ola::Derivada`):
//!
//! ```text
//!    gruesa x   p1 - p0      SHFL.IDX 0 y 1 del cuadro (c 0x1c03), FADD
//!    gruesa y   p2 - p0      SHFL.IDX 0 y 2 del cuadro, FADD
//!    fina x     la de su fila: SHFL.IDX 0 y 1 de su PAR (c 0x1e01), FADD
//!    fina y     todavia no: pide FSWZADD (su signo lo mide la lente)
//! ```
//!
//! La resta es la de la casa (`f(a(de + paso)) - f(a(de))`): FADD con la
//! segunda negada da los mismos bits. Que el hilo 1 del cuadro sea el de la
//! derecha y el 2 el de abajo -- lo que hace que esto sea ddx y no otra cosa
//! -- lo mide `gpu verrano derivadas` (`bmo_gpu_ga10x::derivadas`).
//!
//! [!] Solo en un sombreador de PIXELES: en el de vertices los hilos no van
//! en cuadros (la casa da 0; la 3060 restaria vertices vecinos). La puerta
//! no deja pasar un `ddx` de vertices (`Programa::deriva` del de vertices).

use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::Reg;
use bmo_sm86::codifica as c;
use bmo_sm86::olas::{shfl, Shfl, CUADRO, PARES};

use super::planifica::Meta;
use super::{Abi, Clase, Emisor, NoEmite};

impl Emisor<'_> {
    /// **`d = ddx(a)` / `ddy(a)`** (gruesa) o `ddx_fine(a)`.
    pub(super) fn derivada(&mut self, d: Reg, a: Reg, y: bool, fina: bool, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        if self.abi != Abi::Registros || (fina && y) {
            return Err(NoEmite::Operacion(k));
        }
        let ra = self.registro(a, paso)?;
        let (cc, segundo) = if fina { (PARES, 1) } else { (CUADRO, if y { 2 } else { 1 }) };
        let (t0, t1) = (self.pedir()?, self.pedir()?);
        paso.extend([t0, t1]);
        for (t, lane) in [(t0, 0), (t1, segundo)] {
            // Desacoplado (lee `ra` tarde y escribe con barrera).
            self.poner_meta(shfl(t, ra, Shfl::Idx, lane, cc, 0), Meta { lee_tarde: true, ..Meta::de(Clase::Tex, Some(t), [Some(ra), None, None]) });
        }
        let x = self.destino(d, k)?;
        self.poner(c::fadd(x, c::r(t1), c::neg(t0), false, 0), Clase::Fma, Some(x), [Some(t1), Some(t0), None]);
        Ok(())
    }
}
