//! **LA FILA DEL CBUFFER CALCULADA, EN LA 3060** (E8f de
//! `docs/plan/PLAN_LA_LENGUA_DE_LA_3060.md`, DL18 del 09-10): `Op::ConstantesEn`
//! -- los arrays de un cbuffer, las luces y los huesos (`luces.hlsl`) -- con
//! un LDC del banco de la app, sujeto como lo pide R7 y con los bits de la
//! casa:
//!
//! ```text
//!    t = i << 4                   SHF.L.U32: la fila, en bytes
//!    t = min(t, 16 (filas - 1))   IMNMX.U32: el CERROJO -- t no sale de las
//!                                 filas del array, sea cual sea i --
//!    q..q+1   = c[3][t + 16 fila]       LDC.64
//!    q+2..q+3 = c[3][t + 16 fila + 8]   LDC.64 (el mismo t: nadie lo toca)
//!    P1 = i < filas               ISETP.LT.U32: dentro?
//!    q+k = P1 ? q+k : 0           SEL: fuera, 0, como la casa
//! ```
//!
//! El SEL hace falta aunque el IMNMX sujete: la casa da 0 FUERA de las filas
//! del array aunque el banco tenga detras otra cosa (el cbuffer aplanado
//! trae las filas de todos). Y el `<< 4` de un indice enorme da la vuelta
//! (2^28 se vuelve 0): no importa, ese indice esta fuera y el SEL lo
//! deja en 0.
//!
//! El banco es el 3 en los dos ABI: el de E3 (`BANCO_CB`) y el de la app
//! (`BANCO_APP`), que ata el kernel con la medida del cbuffer de la receta
//! (`receta`, +88 bit 1). Con el indice escrito (`color[2]`), el LDC sin
//! indice (Ra = RZ); fuera de las filas, cuatro ceros.

use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::Reg;
use bmo_sm86::codifica::{self as c, Cmp, Fuente, RZ};

use super::planifica::Meta;
use super::{Clase, Emisor, NoEmite, Valor, BANCO_APP};

const P1: u8 = 1;

/// Lo mas que dice el desplazamiento de un LDC (14 bits de palabras).
const DESP_MAX: u32 = 0xFFFC;

impl Emisor<'_> {
    /// **`d..d+4 = cbuffer[fila + i]`** si `i < filas`; si no, 0.
    pub(super) fn constantes_en(&mut self, d: Reg, fila: u16, filas: u16, i: Reg, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        if (0..4).any(|c| self.valor.get(d as usize + c).is_none_or(|x| x.is_some())) {
            return Err(NoEmite::NoSsa(k));
        }
        let escrito = match self.valor(i) {
            Valor::Imm(v) => Some(v),
            _ => None,
        };
        // Todo fuera (sin filas, o un indice escrito que se pasa): ceros.
        if filas == 0 || escrito.is_some_and(|j| j >= filas as u32) {
            for c in 0..4 {
                self.valor[d as usize + c] = Some(Valor::Imm(0));
            }
            return Ok(());
        }
        let q = self.bloque(4, false)?;
        let (ra, ri, desp) = match escrito {
            Some(j) => (RZ, RZ, 16 * (fila as u32 + j)),
            None => {
                let ri = self.registro(i, paso)?;
                let t = self.pedir()?;
                paso.push(t);
                self.poner(c::shl(t, ri, Fuente::Imm(4), 0), Clase::Alu, Some(t), [Some(ri), None, None]);
                self.poner(c::imnmx(t, t, Fuente::Imm(16 * (filas as u32 - 1)), false, false, 0), Clase::Alu, Some(t), [Some(t), None, None]);
                (t, ri, 16 * fila as u32)
            }
        };
        // El desplazamiento de la segunda mitad tiene que caber en el LDC; si
        // no, por la CPU. (Lo que lee CON el indice lo acota el banco: lo
        // mira R7 contra su medida.)
        if desp + 8 > DESP_MAX {
            return Err(NoEmite::Operacion(k));
        }
        for (mitad, x) in [(0u32, q), (8, q + 2)] {
            let lee = [(ra != RZ).then_some(ra), None, None];
            self.poner_meta(c::ldc(x, BANCO_APP, ra, (desp + mitad) as u16, true, 0), Meta { escribe_n: 2, ..Meta::de(Clase::Mufu, Some(x), lee) });
        }
        if escrito.is_none() {
            self.poner_meta(c::isetp(P1, Cmp::Lt, ri, Fuente::Imm(filas as u32), true, 0), Meta { escribe_p: Some(P1), ..Meta::de(Clase::Alu, None, [Some(ri), None, None]) });
            for c in 0..4u8 {
                self.poner_meta(c::sel(q + c, q + c, c::r(RZ), P1, false, 0), Meta { lee_p: Some((P1, false)), ..Meta::de(Clase::Alu, Some(q + c), [Some(q + c), None, None]) });
            }
        }
        for c in 0..4u8 {
            self.valor[d as usize + c as usize] = Some(Valor::Reg(q + c));
        }
        Ok(())
    }
}
