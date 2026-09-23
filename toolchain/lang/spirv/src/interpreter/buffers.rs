//! **El oraculo y los BUFFERS**: leer y escribir un valor donde digan sus
//! decoraciones (`Offset`, `ArrayStride`), que es la disposicion que escribio
//! quien preparo los datos (std140, std430). En la arena los compuestos van
//! juntos; aqui no.
//!
//! [consumo]  NADA   solo mientras el oraculo despacha

use super::*;

impl<'m, 'a, 'b, 'w> Interpreter<'m, 'a, 'b, 'w> {
    // ---- los buffers ---------------------------------------------------------

    pub(super) fn stride(&self, t: u32) -> Paso<u32> {
        decoration(self.m, t, DEC_ARRAY_STRIDE).ok_or(Reason::NoLayout { id: t })
    }

    pub(super) fn member(&self, t: u32, k: u32) -> Paso<u32> {
        let o = self.slot[t as usize] + k;
        match self.arena.get(o as usize) {
            Some(&x) if x != SIN => Ok(x),
            _ => Err(Reason::NoLayout { id: t }),
        }
    }

    pub(super) fn word_at(buf: &Buffer, byte: u32) -> Paso<u32> {
        if byte % 4 != 0 {
            return Err(Reason::OutOfBounds);
        }
        buf.data.get((byte / 4) as usize).copied().ok_or(Reason::OutOfBounds)
    }

    /// Carga un valor de tipo `t` del buffer, desde el byte `byte`, en la arena
    /// en `dest`. Devuelve cuantas palabras escribio.
    pub(super) fn load_buffer(&mut self, buf: &Buffer, byte: u32, t: u32, dest: usize) -> Paso<usize> {
        let d = def(self.m, t).ok_or(Reason::NoLayout { id: t })?;
        match d.opcode {
            op::OpTypeBool | op::OpTypeInt | op::OpTypeFloat => {
                self.arena[dest] = Self::word_at(buf, byte)?;
                Ok(1)
            }
            op::OpTypeVector => {
                for k in 0..d.op(3) {
                    self.arena[dest + k as usize] = Self::word_at(buf, byte + 4 * k)?;
                }
                Ok(d.op(3) as usize)
            }
            op::OpTypeArray => {
                let n = def(self.m, d.op(3)).map(|c| c.op(3)).unwrap_or(0);
                let s = self.stride(t)?;
                let mut w = 0;
                for i in 0..n {
                    w += self.load_buffer(buf, byte + i * s, d.op(2), dest + w)?;
                }
                Ok(w)
            }
            op::OpTypeStruct => {
                let mut w = 0;
                for k in 0..d.words as u32 - 2 {
                    let off = self.member(t, k)?;
                    w += self.load_buffer(buf, byte + off, d.op(2 + k as usize), dest + w)?;
                }
                Ok(w)
            }
            _ => Err(Reason::NotYet { what: "cargar este tipo de un buffer" }),
        }
    }

    /// Guarda en el buffer el valor de tipo `t` que esta en la arena en `src`.
    pub(super) fn store_buffer(&self, buf: &mut Buffer, byte: u32, t: u32, src: usize) -> Paso<usize> {
        let d = def(self.m, t).ok_or(Reason::NoLayout { id: t })?;
        let mut escribir = |b: u32, v: u32| -> Paso<()> {
            if b % 4 != 0 {
                return Err(Reason::OutOfBounds);
            }
            let w = buf.data.get_mut((b / 4) as usize).ok_or(Reason::OutOfBounds)?;
            *w = v;
            Ok(())
        };
        match d.opcode {
            op::OpTypeBool | op::OpTypeInt | op::OpTypeFloat => {
                escribir(byte, self.arena[src])?;
                Ok(1)
            }
            op::OpTypeVector => {
                for k in 0..d.op(3) {
                    escribir(byte + 4 * k, self.arena[src + k as usize])?;
                }
                Ok(d.op(3) as usize)
            }
            op::OpTypeArray => {
                let n = def(self.m, d.op(3)).map(|c| c.op(3)).unwrap_or(0);
                let s = self.stride(t)?;
                let mut w = 0;
                for i in 0..n {
                    w += self.store_buffer(buf, byte + i * s, d.op(2), src + w)?;
                }
                Ok(w)
            }
            op::OpTypeStruct => {
                let mut w = 0;
                for k in 0..d.words as u32 - 2 {
                    let off = self.member(t, k)?;
                    w += self.store_buffer(buf, byte + off, d.op(2 + k as usize), src + w)?;
                }
                Ok(w)
            }
            _ => Err(Reason::NotYet { what: "guardar este tipo en un buffer" }),
        }
    }
}
