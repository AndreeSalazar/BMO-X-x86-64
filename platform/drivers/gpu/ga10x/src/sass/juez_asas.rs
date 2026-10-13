//! **R7: EL ASA DE UN TEX O UN TLD DE APP** -- la que puso el kernel, o (G5,
//! 13-10) una CALCULADA desde la base de unas mips, sujeta a su textura.
//!
//! capa: puro -- lee palabras; no toca la tarjeta
//!
//! [eje]     SEGURIDAD -- un asa es un indice en la piscina de TIC: con una
//!           suya la app leeria descriptores que no son de su textura
//!
//! # Lo que se deja
//!
//! ```text
//!    Rb un asa del kernel   (`Carga::Asa` o `Carga::Base`), sin tocar: el
//!                           cuerpo no escribe en ellas (lo mira R7)
//!    Rb calculada           la ultima escritura de Rb antes del TEX es
//!                           `IADD3 Rb, Rbase, Rk, RZ` con Rbase una BASE del
//!                           kernel, y la ultima de Rk antes de esa,
//!                           `IMNMX.U32 Rk, x, tope` (el MENOR sin signo) con
//!                           `tope <= mips::TOPE`; y ningun salto del cuerpo
//!                           cae entre el IMNMX y el TEX (lo esquivaria)
//! ```
//!
//! Asi el TIC que lee la 3060 es el de la base mas 0..=14: los huecos 1..15
//! de SU textura (`mips`). Una base es `asa(16 k + 1, k)` y la textura `k`
//! tiene sus 16 huecos escritos en cada dibujo; el TSC (bits 20..31) no se
//! mueve: 1 + 14 no llega al bit 20.

use super::juez::{Bodrio, Regla};
use super::juez_leer::{decodificar, RZ};

/// Lo de una palabra sin el control.
const BAJOS: u64 = (1 << 41) - 1;

fn ajeno(instruccion: usize, que: u32, detalle: &'static str) -> Result<(), Bodrio> {
    Err(Bodrio { regla: Regla::R7CuerpoAjeno, instruccion, que, detalle })
}

/// La ultima instruccion antes de `k` que escribe `r`.
fn quien_escribe(codigo: &[(u64, u64)], k: usize, r: u8) -> Option<usize> {
    (0..k).rev().find(|&j| decodificar(codigo[j].0, codigo[j].1).is_some_and(|x| x.escribe.1 > 0 && (x.escribe.0..x.escribe.0.saturating_add(x.escribe.1)).contains(&r)))
}

/// Algun BRA del cuerpo cae en `(desde, hasta]`.
fn salto_entre(codigo: &[(u64, u64)], desde: usize, hasta: usize) -> Option<usize> {
    codigo.iter().enumerate().find_map(|(m, &(blo, bhi))| {
        if blo & 0x1FF != 0x147 {
            return None;
        }
        let d = (((bhi & 0x3_FFFF) << 32 | blo >> 32) << 14) as i64 >> 14;
        let destino = (16 * (m as i64 + 1) + d) / 16;
        (destino > desde as i64 && destino <= hasta as i64).then_some(m)
    })
}

/// **El asa del TEX o TLD `k`** (ver el modulo).
pub(super) fn asa_de_app(codigo: &[(u64, u64)], k: usize, asas: u64, bases: u64) -> Result<(), Bodrio> {
    let rb = (codigo[k].0 >> 32 & 0xFF) as u8;
    if rb < 64 && asas >> rb & 1 == 1 {
        return Ok(());
    }
    let no = || ajeno(k, rb as u32, "un TEX o un TLD con un asa que no puso el kernel (ni calculada desde una base, sujeta)");
    if bases == 0 || rb == RZ {
        return no();
    }
    let Some(j) = quien_escribe(codigo, k, rb) else { return no() };
    let (lo, hi) = codigo[j];
    // IADD3 Rb, Ra, Rk, RZ (la forma de `codifica::iadd3`, sin acarreos).
    let suma = lo & 0x1FF == 0x010 && lo >> 9 & 7 == 1 && hi & BAJOS == (0x07ff_e000 | RZ as u64) && lo >> 12 & 0xF == 7 && lo >> 40 == 0;
    let (base, rk) = ((lo >> 24 & 0xFF) as u8, (lo >> 32 & 0xFF) as u8);
    if !suma || base >= 64 || bases >> base & 1 == 0 {
        return ajeno(j, rb as u32, "un asa calculada que no es IADD3 de una BASE del kernel y un indice");
    }
    let Some(i) = quien_escribe(codigo, j, rk) else {
        return ajeno(j, rk as u32, "el indice de un asa calculada no lo sujeta nadie");
    };
    let (ilo, ihi) = codigo[i];
    let sujeta = ilo & 0x1FF == 0x017 && ilo >> 9 & 7 == 4 && ihi & BAJOS == 7 << 23 && (ilo >> 16 & 0xFF) as u8 == rk;
    if !sujeta || (ilo >> 32) as u32 > crate::mips::TOPE {
        return ajeno(i, rk as u32, "el indice de un asa calculada no lo escribe un IMNMX.U32 con un tope de las mips");
    }
    if let Some(m) = salto_entre(codigo, i, k) {
        return ajeno(m, rb as u32, "un salto cae entre el IMNMX que sujeta la mip y su TEX: lo esquivaria");
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_sm86::codifica as c;

    fn tex(rb: u8) -> (u64, u64) {
        c::tex(8, 2, rb, 1)
    }

    #[test]
    fn la_base_mas_una_mip_sujeta() {
        // R6 la base del kernel; R4 la mip; R5 = R6 + min(R4, 14).
        let bien = [c::imnmx(4, 4, c::Fuente::Imm(14), false, false, 1), c::iadd3(5, 6, c::r(4), 1), tex(5)];
        assert_eq!(asa_de_app(&bien, 2, 1 << 6, 1 << 6), Ok(()));
        // La misma base como asa de la vista (no base): no.
        assert!(asa_de_app(&bien, 2, 1 << 6, 0).is_err());
        // Un tope de mas, con signo, o el mayor: no.
        for mala in [c::imnmx(4, 4, c::Fuente::Imm(15), false, false, 1), c::imnmx(4, 4, c::Fuente::Imm(14), false, true, 1), c::imnmx(4, 4, c::Fuente::Imm(14), true, false, 1)] {
            let x = [mala, bien[1], bien[2]];
            assert!(asa_de_app(&x, 2, 1 << 6, 1 << 6).is_err(), "{mala:x?}");
        }
        // Sin sujetar, o sumando desde lo que no es una base.
        assert!(asa_de_app(&[c::mov(4, c::Fuente::Imm(3), 1), bien[1], bien[2]], 2, 1 << 6, 1 << 6).is_err());
        assert!(asa_de_app(&[bien[0], c::iadd3(5, 7, c::r(4), 1), bien[2]], 2, 1 << 6, 1 << 6).is_err());
        // Algo que pisa la mip entre el IMNMX y el IADD3, o el asa antes del TEX.
        assert!(asa_de_app(&[bien[0], c::mov(4, c::Fuente::Imm(99), 1), bien[1], bien[2]], 3, 1 << 6, 1 << 6).is_err());
        assert!(asa_de_app(&[bien[0], bien[1], c::mov(5, c::Fuente::Imm(99), 1), bien[2]], 3, 1 << 6, 1 << 6).is_err());
        // Un salto que cae entre el IMNMX y el TEX.
        let salto = [c::bra(7, 16, 1), bien[0], bien[1], bien[2]];
        assert!(asa_de_app(&salto, 3, 1 << 6, 1 << 6).is_err());
        // Un asa del kernel tal cual, como siempre.
        assert_eq!(asa_de_app(&[tex(6)], 0, 1 << 6, 0), Ok(()));
    }
}
