//! **LA LAMINA EN LA MAQUINA** (A4b de `docs/plan/EL_FOCO.md`, 14-10) -- la
//! RAM que un programa de TITAN++ tiene en la mano.
//!
//! [carril]  VERDE     su asa en el marco, sus bytes en un bloque del kernel
//! [consumo] NADA      pedir y soltar, una puerta cada uno
//!
//! ```text
//!    el asa (32 B)   +0   donde empiezan sus bytes
//!                    +8   cuantos bytes (len)
//!                    +16  cuantos caben (los mismos: no crece)
//!                    +24  su asa del KERNEL; 0 = no hay (soltada o entregada)
//! ```
//!
//! Las tres primeras palabras son las de una lista (`coleccion.rs`), a
//! proposito: `l[i]` lee y escribe UN byte como la celda de una `[byte]`,
//! con el mismo `movzx` y la misma vara, y `len(l)` es la misma palabra.
//! Lo que cambia es de donde salen los bytes -- de la PUERTA DE LA RAM
//! (`ram.rs`), no del monton -- y como se van:
//!
//! ```text
//!    pedir         `lamina(n)`: la puerta; si el kernel dice que no, el
//!                  programa lo dice (T0060) en su linea
//!    soltar        al morir su propietario (`drop_at`): MEM_OP_SOLTAR, y
//!                  el asa a cero. Un asa a cero no se suelta: nadie suelta
//!                  dos veces
//!    entregar      `f(take l)`: la fn que la recibe COPIA el asa (no pide
//!                  nada) y quien la entrego deja la suya a cero al volver
//!    copiar        nunca: el frontend lo dice (T0077) antes de llegar aqui;
//!                  si llegara, el programa lo dice y sale
//! ```

use super::{Helper, Place, E1};
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX};
use bmo_lower::{console, task};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::Value;

/// Lo que mide el asa de una lamina.
pub(crate) const ASA: i32 = 32;
const KERNEL: i32 = 24;

impl E1<'_> {
    /// `lamina(n)`: `n` bytes por la puerta, y su asa en un sitio de paso.
    pub(super) fn lamina_nueva(&mut self, n: &Value, at: (usize, usize)) -> Result<(Place, Class), String> {
        let (pn, _) = self.eval(n)?;
        let t = self.temp(ASA);
        self.load(pn, RDX);
        x86::cmp_r64_imm8(&mut self.code, RDX, 0);
        self.trap(0x8E, "T0060", "lamina(n) pide al menos un byte", at);
        self.pide_lamina();
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        self.trap(0x85, "T0060", "el kernel no dio la lamina (sin RAM, o ya hay todas las que se pueden tener a la vez)", at);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        self.trap(0x84, "T0060", "el kernel no dio la lamina (sin RAM, o ya hay todas las que se pueden tener a la vez)", at);
        self.store(t, RDX);
        self.store(t.at(KERNEL), RCX);
        self.load(pn, RAX);
        self.store(t.at(8), RAX);
        self.store(t.at(16), RAX);
        self.own(t, &Class::Lamina);
        Ok((t, Class::Lamina))
    }

    /// `l[i]`: el sitio de su byte `i`, o T0072 fuera de ella.
    pub(super) fn lamina_index(&mut self, h: Place, idx: &Value, at: (usize, usize)) -> Result<Place, String> {
        self.a_rax(idx)?;
        self.dentro_de(h.at(8), "una celda fuera de la lamina", at);
        let cell = self.list_cell(&Class::Byte);
        Ok(self.celda_rax(h, cell))
    }

    /// Muere su propietario: el bloque, de vuelta al kernel, y el asa a cero.
    pub(super) fn suelta_lamina(&mut self, p: Place) {
        self.load(p.at(KERNEL), RDI);
        x86::test_r64_r64(&mut self.code, RDI, RDI);
        let nada = self.jcc(0x84);
        self.call_helper(Helper::SueltaLamina);
        for k in 0..ASA / 8 {
            self.store_imm(p.at(8 * k), 0);
        }
        self.here(nada);
    }

    /// Entregada (`take`): quien la entrego ya no tiene nada que soltar.
    pub(super) fn olvida_lamina(&mut self, p: Place) {
        for k in 0..ASA / 8 {
            self.store_imm(p.at(8 * k), 0);
        }
    }

    /// Una copia de una lamina no existe: si el emisor llegara a pedirla, el
    /// programa lo dice y sale en vez de tener dos asas del mismo bloque.
    pub(super) fn copia_de_lamina(&mut self) {
        console::write_const(&mut self.code, b"NO al correr: una lamina se iba a COPIAR (fallo del compilador: avisa con este programa)\n");
        task::exit(&mut self.code);
    }
}
