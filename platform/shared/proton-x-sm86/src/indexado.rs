//! **LOS ARRAYS, EN LA 3060** (E8b de `docs/plan/PLAN_LA_LENGUA_DE_LA_3060.md`,
//! 09-10): `Op::LeeIndexado` y `Op::EscribeIndexado` (N5.10 de
//! PLAN_LAS_TRES_GRANDES: `alloca`, los arrays globales constantes, y los
//! temporales indexables de SM5) SIN MEMORIA -- la lista blanca de R7 no deja
//! a un cuerpo de app ni LDL ni STL --: el array son sus registros, y el
//! indice se mira contra cada uno.
//!
//! ```text
//!    leer a[i]       por cada j: P1 = (i == j), x = P1 ? a[j] : x -- la
//!                    primera elige contra RZ: fuera del array (tambien
//!                    los bits de un negativo o de un NaN), 0, como la
//!                    casa --
//!    escribir a[i]   por cada j: P1 = (i == j), a[j] = P1 ? s : a[j]
//!                    -- fuera, nada --
//!    indice escrito  (el del DXIL de `a[2]`): la copia y ya
//! ```
//!
//! El indice se compara por sus BITS (`ISETP.U32`), como la casa
//! (`bits(regs, i)`). Cada elemento de un array que se escribe es una
//! VARIABLE -- su registro desde que nace, con su inicial --: lo dice el
//! analisis (`saltos.rs`), que cuenta cada lectura y cada escritura indexada
//! como una de TODO el array.
//!
//! Cuesta dos instrucciones por elemento: para los arrays de un sombreador
//! (unos pocos), cabe; uno de cientos no cabria en un hueco de la tuberia
//! (las 128 de la puerta, con el pegamento), y lo dice el pegamento: el PSO
//! va por la CPU.

use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::Reg;
use bmo_sm86::codifica::{self as c, Cmp, Fuente, RZ};

use super::planifica::Meta;
use super::{reg_de, Clase, Emisor, NoEmite, Valor};

const P1: u8 = 1;

impl Emisor<'_> {
    /// El indice, si es una constante escrita (sus bits): un registro que
    /// nadie escribe vale su inicial (`valor`, como en `fuente`).
    fn indice_escrito(&mut self, i: Reg) -> Option<u32> {
        match self.valor(i) {
            Valor::Imm(v) => Some(v),
            _ => None,
        }
    }

    /// El array cabe en los registros del Programa: la casa solo toca el
    /// elemento del indice, pero aqui se miran TODOS; si alguno no existe,
    /// la operacion no se emite (va por la CPU).
    fn cabe(&self, base: Reg, n: u16, k: usize) -> Result<(), NoEmite> {
        if base as usize + n as usize <= self.valor.len() {
            Ok(())
        } else {
            Err(NoEmite::Operacion(k))
        }
    }

    /// **`d = a[i]`** (`a` los `n` registros desde `base`).
    pub(super) fn lee_indexado(&mut self, d: Reg, base: Reg, n: u16, i: Reg, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        self.cabe(base, n, k)?;
        if let Some(j) = self.indice_escrito(i) {
            let f = if j < n as u32 { self.fuente(base + j as Reg) } else { Fuente::Imm(0) };
            let x = self.destino(d, k)?;
            if f != c::r(x) {
                self.poner(c::mov(x, f, 0), Clase::Alu, Some(x), [reg_de(f), None, None]);
            }
            return Ok(());
        }
        // Sin elementos, todo indice esta fuera: 0.
        if n == 0 {
            let x = self.destino(d, k)?;
            self.poner(c::mov(x, Fuente::Imm(0), 0), Clase::Alu, Some(x), [None; 3]);
            return Ok(());
        }
        let ri = self.registro(i, paso)?;
        // Lo elegido hasta ahora (con un solo elemento, no hace falta).
        let t = if n > 1 {
            let t = self.pedir()?;
            paso.push(t);
            t
        } else {
            RZ
        };
        for j in 0..n {
            self.poner_meta(c::isetp(P1, Cmp::Eq, ri, Fuente::Imm(j as u32), true, 0), Meta { escribe_p: Some(P1), ..Meta::de(Clase::Alu, None, [Some(ri), None, None]) });
            // La ultima escribe el destino (que puede ser el registro de i).
            let x = if j + 1 == n { self.destino(d, k)? } else { t };
            // La primera elige contra RZ (fuera, 0); las demas, contra lo
            // elegido.
            let (antes, lee) = if j == 0 { (RZ, None) } else { (t, Some(t)) };
            let w = match self.fuente(base + j) {
                Fuente::R { r, .. } => (c::sel(x, r, c::r(antes), P1, false, 0), [Some(r), lee, None]),
                f => (c::sel(x, antes, f, P1, true, 0), [lee, None, None]),
            };
            self.poner_meta(w.0, Meta { lee_p: Some((P1, false)), ..Meta::de(Clase::Alu, Some(x), w.1) });
        }
        Ok(())
    }

    /// **`a[i] = s`**: cada elemento, una variable, se queda o toma `s`.
    pub(super) fn escribe_indexado(&mut self, base: Reg, n: u16, i: Reg, s: Reg, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        self.cabe(base, n, k)?;
        let rs = self.registro(s, paso)?;
        if let Some(j) = self.indice_escrito(i) {
            if j < n as u32 {
                let x = self.elemento(base + j as Reg, k)?;
                if x != rs {
                    self.poner(c::mov(x, c::r(rs), 0), Clase::Alu, Some(x), [Some(rs), None, None]);
                }
            }
            return Ok(());
        }
        let mut ri = self.registro(i, paso)?;
        // El indice es un elemento de este array (`a[a[0]] = s`): sus bits se
        // miran UNA vez, como la casa; si no, el elemento que cambia en
        // medio de la cadena la desviaria.
        if (base as usize..base as usize + n as usize).contains(&(i as usize)) {
            let t = self.pedir()?;
            paso.push(t);
            self.poner(c::mov(t, c::r(ri), 0), Clase::Alu, Some(t), [Some(ri), None, None]);
            ri = t;
        }
        for j in 0..n {
            let x = self.elemento(base + j, k)?;
            self.poner_meta(c::isetp(P1, Cmp::Eq, ri, Fuente::Imm(j as u32), true, 0), Meta { escribe_p: Some(P1), ..Meta::de(Clase::Alu, None, [Some(ri), None, None]) });
            self.poner_meta(c::sel(x, rs, c::r(x), P1, false, 0), Meta { lee_p: Some((P1, false)), ..Meta::de(Clase::Alu, Some(x), [Some(rs), Some(x), None]) });
        }
        Ok(())
    }

    /// El registro de un elemento de un array que se escribe: una variable,
    /// que ya nacio (`saltos.rs` lo hace variable). Si no lo es -- el
    /// analisis no lo vio --, la operacion no se emite: mejor por la CPU que
    /// mal en la 3060.
    fn elemento(&self, r: Reg, k: usize) -> Result<u8, NoEmite> {
        match self.valor.get(r as usize).copied().flatten() {
            Some(Valor::Reg(x)) if self.variable.get(r as usize) == Some(&true) => Ok(x),
            _ => Err(NoEmite::Operacion(k)),
        }
    }
}
