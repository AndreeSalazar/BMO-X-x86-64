//! **LA MEMORIA COMPARTIDA DEL GRUPO EN LA 3060** (LI2f, 12-10) --
//! `groupshared` de HLSL: `Op::LeeCompartida`, `Op::EscribeCompartida` y
//! `Op::AtomicoCompartido`, con LDS, STS y ATOMS (`bmo_sm86::computo`, de
//! oro), y su prologo.
//!
//! ```text
//!    la casa        la palabra base + i de su array de n; fuera (i >= n), 0 al
//!                   leer y nada al escribir. La compartida empieza A CERO en
//!                   cada grupo (`Programa::despachar`)
//!    el prologo     la 3060 NO la pone a cero: antes del cuerpo, cada hilo
//!                   (su SV_GroupIndex t) escribe 0 en t, t + N, t + 2N... y
//!                   una BAR.SYNC; hasta 16 vueltas
//!    cada acceso    vale:  ISETP.LT.U32 P2 = i < n
//!                   la direccion, la cadena que reconoce el juez:
//!                      IADD3     x = i + base
//!                      IMNMX.U32 a = min(x, tope)      tope = base + n - 1
//!                      IMAD      a = a * 4 + RZ
//!                      LDS / @P2 STS / @P2 ATOMS  [a]
//!                   leer: SEL v = P2 ? v : 0
//! ```
//!
//! La SEGURIDAD es la cadena: `4 * tope + 4` no pasa de la compartida que el
//! kernel le da al grupo (`computo.compartida`, en el QMD), valga `i` lo que
//! valga. Que `i` caiga en SU array (y no en otro del mismo grupo) es el
//! predicado: correccion, como en los UAV.

use alloc::vec::Vec;

use bmo_proton_x::bufer::Atomo as AtomoCasa;
use bmo_proton_x::dxil::programa::Reg;
use bmo_sm86::codifica::{self as c, Cmp, Fuente, RZ};
use bmo_sm86::computo::{self as g, Atomo};

use super::planifica::Meta;
use super::{Clase, Emisor, NoEmite};

/// Si la palabra vale (P0 es del `si` fundido: no se toca).
const P_VALE: u8 = 2;
/// Las vueltas del prologo, como mucho (con mas, la compartida es grande
/// para su grupo: por la CPU).
pub const VUELTAS: u32 = 16;

/// El atomico de la 3060 para uno de la casa, y si es con signo. El CAS es
/// otra instruccion (`None`).
fn atomo(como: AtomoCasa) -> Option<(Atomo, bool)> {
    Some(match como {
        AtomoCasa::Suma => (Atomo::Add, false),
        AtomoCasa::Y => (Atomo::And, false),
        AtomoCasa::O => (Atomo::Or, false),
        AtomoCasa::Xor => (Atomo::Xor, false),
        AtomoCasa::MinConSigno => (Atomo::Min, true),
        AtomoCasa::MaxConSigno => (Atomo::Max, true),
        AtomoCasa::MinSinSigno => (Atomo::Min, false),
        AtomoCasa::MaxSinSigno => (Atomo::Max, false),
        AtomoCasa::Cambia => (Atomo::Exch, false),
        AtomoCasa::CambiaSiIgual => return None,
    })
}

/// Lo que hace la memoria con la direccion `a` ya sujeta.
enum Acceso {
    Lee(u8),
    Escribe(u8),
    Atomo { d: u8, como: AtomoCasa, v: u8 },
}

impl Emisor<'_> {
    /// Las palabras de la compartida del programa (0: no tiene).
    fn palabras_compartidas(&self) -> u32 {
        self.p.computo.compartida
    }

    /// La cadena (ver el modulo) para la palabra `x` -- ya con su base --, con
    /// `tope` la ultima que se puede tocar, y el acceso detras. `guarda`: el
    /// predicado de un STS o un ATOMS (`None`: sin guarda).
    fn sujeta_y_toca(&mut self, a: u8, x: u8, tope: u32, guarda: Option<u8>, acceso: Acceso) {
        self.poner(c::imnmx(a, x, Fuente::Imm(tope), false, false, 0), Clase::Alu, Some(a), [Some(x), None, None]);
        self.poner(c::imad(a, a, Fuente::Imm(4), RZ, 0), Clase::Fma, Some(a), [Some(a), None, None]);
        let g_ = guarda.unwrap_or(7);
        let lee_p = guarda.map(|p| (p, true));
        match acceso {
            Acceso::Lee(v) => self.poner_meta(g::lds(v, a, 0, 0), Meta { lee_tarde: true, ..Meta::de(Clase::Tex, Some(v), [Some(a), None, None]) }),
            Acceso::Escribe(s) => {
                let mut w = g::sts(a, 0, s, 0);
                w.0 = w.0 & !(0xF << 12) | (g_ as u64) << 12;
                self.poner_meta(w, Meta { lee_p, lee_tarde: true, ..Meta::de(Clase::Tex, None, [Some(a), (s != RZ).then_some(s), None]) });
            }
            Acceso::Atomo { d, como, v } => {
                let mut w = match atomo(como) {
                    Some((op, signo)) => g::atoms(op, signo, d, a, v, 0),
                    // El CAS de la casa compara con 0 (`AtomicoCompartido` no
                    // lleva `igual`): Rb = RZ.
                    None => g::atoms_cas(d, a, RZ, v, 0),
                };
                w.0 = w.0 & !(0xF << 12) | (g_ as u64) << 12;
                self.poner_meta(w, Meta { lee_p, lee_tarde: true, ..Meta::de(Clase::Tex, Some(d), [Some(a), Some(v), None]) });
            }
        }
    }

    /// **El prologo**: la compartida a cero, cada hilo sus palabras, y la
    /// barrera. Solo si el programa la usa.
    pub(super) fn prologo_compartida(&mut self) -> Result<(), NoEmite> {
        let usa = self.p.ops.iter().position(|o| matches!(o, bmo_proton_x::dxil::programa::Op::LeeCompartida { .. } | bmo_proton_x::dxil::programa::Op::EscribeCompartida { .. } | bmo_proton_x::dxil::programa::Op::AtomicoCompartido { .. }));
        let Some(k) = usa else { return Ok(()) };
        let w = self.palabras_compartidas();
        let h = self.hilos(k)?;
        let n = h[0] * h[1] * h[2];
        if w == 0 || w.div_ceil(n) > VUELTAS {
            return Err(NoEmite::Operacion(k));
        }
        let t = self.pedir()?;
        self.indice(t, h)?;
        let (x, a) = (self.pedir()?, self.pedir()?);
        for r in 0..w.div_ceil(n) {
            let guarda = if (r + 1) * n > w {
                self.poner(c::iadd3(x, t, Fuente::Imm(r * n), 0), Clase::Alu, Some(x), [Some(t), None, None]);
                self.poner_meta(c::isetp(P_VALE, Cmp::Lt, x, Fuente::Imm(w), true, 0), Meta { escribe_p: Some(P_VALE), ..Meta::de(Clase::Alu, None, [Some(x), None, None]) });
                Some(P_VALE)
            } else {
                self.poner(c::iadd3(x, t, Fuente::Imm(r * n), 0), Clase::Alu, Some(x), [Some(t), None, None]);
                None
            };
            self.sujeta_y_toca(a, x, w - 1, guarda, Acceso::Escribe(RZ));
        }
        for r in [t, x, a] {
            self.soltar(r);
        }
        self.poner_meta(g::bar_sync(0), Meta { salto: true, ..Meta::de(Clase::Nada, None, [None; 3]) });
        Ok(())
    }

    /// P2 = `i < n`, y `x` = i + base; `None` si el array no cabe.
    fn guia_compartida(&mut self, base: u32, n: u32, i: Reg, k: usize, paso: &mut Vec<u8>) -> Result<(u8, u8, u32), NoEmite> {
        self.hilos(k)?;
        let w = self.palabras_compartidas();
        if n == 0 || base.checked_add(n).is_none_or(|f| f > w) {
            return Err(NoEmite::Operacion(k));
        }
        let ri = self.registro(i, paso)?;
        let (x, a) = (self.pedir()?, self.pedir()?);
        paso.extend([x, a]);
        self.poner_meta(c::isetp(P_VALE, Cmp::Lt, ri, Fuente::Imm(n), true, 0), Meta { escribe_p: Some(P_VALE), ..Meta::de(Clase::Alu, None, [Some(ri), None, None]) });
        self.poner(c::iadd3(x, ri, Fuente::Imm(base), 0), Clase::Alu, Some(x), [Some(ri), None, None]);
        Ok((x, a, base + n - 1))
    }

    /// **`d` = la palabra `base + i`** (de `n`).
    pub(super) fn lee_compartida(&mut self, d: Reg, base: u32, n: u32, i: Reg, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        let (x, a, tope) = self.guia_compartida(base, n, i, k, paso)?;
        let v = self.destino(d, k)?;
        self.sujeta_y_toca(a, x, tope, None, Acceso::Lee(v));
        self.poner_meta(c::sel(v, v, c::r(RZ), P_VALE, false, 0), Meta { lee_p: Some((P_VALE, false)), ..Meta::de(Clase::Alu, Some(v), [Some(v), None, None]) });
        Ok(())
    }

    /// **La palabra `base + i` = `s`**.
    pub(super) fn escribe_compartida(&mut self, base: u32, n: u32, i: Reg, s: Reg, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        let rs = self.registro(s, paso)?;
        let (x, a, tope) = self.guia_compartida(base, n, i, k, paso)?;
        self.sujeta_y_toca(a, x, tope, Some(P_VALE), Acceso::Escribe(rs));
        Ok(())
    }

    /// **`d` = lo de antes; la palabra `base + i` = `como(antes, v)`**.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn atomico_compartido(&mut self, d: Reg, base: u32, n: u32, i: Reg, v: Reg, como: AtomoCasa, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        let rv = self.registro(v, paso)?;
        let (x, a, tope) = self.guia_compartida(base, n, i, k, paso)?;
        let rd = self.destino(d, k)?;
        self.sujeta_y_toca(a, x, tope, Some(P_VALE), Acceso::Atomo { d: rd, como, v: rv });
        self.poner_meta(c::sel(rd, rd, c::r(RZ), P_VALE, false, 0), Meta { lee_p: Some((P_VALE, false)), ..Meta::de(Clase::Alu, Some(rd), [Some(rd), None, None]) });
        Ok(())
    }
}
