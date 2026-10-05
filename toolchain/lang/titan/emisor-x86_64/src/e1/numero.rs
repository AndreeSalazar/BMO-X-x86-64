//! **LOS NUMEROS AL CORRER** -- `int` y `dec`, con la semantica de
//! `calc/numero.rs` (la vara):
//!
//! ```text
//!    int          + - * con `jo`: desbordar ATRAPA (T0060); / y % miran el
//!                 cero (T0061), y `/` que no es exacta atrapa (T0062)
//!    dec          cifras y escala. Sumar alinea las escalas; multiplicar las
//!                 suma y quita los ceros de mas; dividir busca el decimal
//!                 EXACTO con menos cifras (o T0062; dentro de `round`, 18
//!                 decimales cortados y `round` decide)
//!    round(x, n)  la mitad se aleja del cero, como el ROUNDED de COBOL
//! ```
//!
//! Las subrutinas (una vez por programa, si se usan) dejan su estado en
//! `r10`: 0 bien, 1 no cabe, 2 no es exacta, 3 entre cero, 4 no cabe en su
//! `dec(p, s)` -- y quien llama atrapa con SU linea (`trap_status`).
//!
//! [!] Donde el calculo usa 128 bits para un paso intermedio (alinear dos
//! escalas muy distintas, multiplicar antes de quitar ceros), aqui ese paso
//! va en 64 y, si no cabe, ATRAPA con T0060 en vez de seguir: nunca un numero
//! distinto, como mucho un NO donde el calculo habria llegado.

use super::{Helper, Place, E1};
use bmo_lower::x86::{self, RAX, RCX, RDX, R10, R11, R8, R9};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::Value;


impl E1<'_> {
    pub fn bin(&mut self, op: &str, a: &Value, b: &Value, at: (usize, usize)) -> Result<(Place, Class), String> {
        if op == "and" || op == "or" {
            // corto: el lado que no hace falta no se calcula
            let (pa, _) = self.eval(a)?;
            let t = self.temp(8);
            self.load(pa, RAX);
            self.store(t, RAX);
            x86::test_r64_r64(&mut self.code, RAX, RAX);
            let done = self.jcc(if op == "and" { 0x84 } else { 0x85 });
            let (pb, _) = self.eval(b)?;
            self.load(pb, RAX);
            self.store(t, RAX);
            self.here(done);
            return Ok((t, Class::Bool));
        }
        let (pa, ca) = self.eval(a)?;
        let (pb, cb) = self.eval(b)?;
        match op {
            "==" | "!=" => {
                let t = self.equal(pa, &ca, pb, &cb, at)?;
                if op == "!=" {
                    self.load(t, RAX);
                    self.imm(RCX, 1);
                    x86::xor_r64_r64(&mut self.code, RAX, RCX);
                    self.store(t, RAX);
                }
                Ok((t, Class::Bool))
            }
            "<" | "<=" | ">" | ">=" => {
                if ca == Class::Int && cb == Class::Int {
                    self.load(pa, RAX);
                    self.load(pb, RDX);
                } else {
                    self.load_dec(pa, &ca, RAX, RCX);
                    self.load_dec(pb, &cb, RDX, R8);
                    self.call_helper(Helper::DecCmp);
                    x86::zero_r32(&mut self.code, RDX);
                }
                x86::cmp_r64_r64(&mut self.code, RAX, RDX);
                let cc = match op {
                    "<" => 0x9C,
                    "<=" => 0x9E,
                    ">" => 0x9F,
                    _ => 0x9D,
                };
                x86::setcc_low(&mut self.code, cc, RAX);
                x86::movzx_r64_low(&mut self.code, RAX, RAX);
                let t = self.temp(8);
                self.store(t, RAX);
                Ok((t, Class::Bool))
            }
            "+" if ca == Class::Text && cb == Class::Text => Ok((self.join(pa, pb, at), Class::Text)),
            _ if ca == Class::Int && cb == Class::Int && !(op == "/" && self.lenient > 0) => Ok((self.int_op(op, pa, pb, at)?, Class::Int)),
            "+" | "-" | "*" | "/" => {
                self.load_dec(pa, &ca, RAX, RCX);
                self.load_dec(pb, &cb, RDX, R8);
                match op {
                    "+" | "-" => {
                        self.imm(R9, (op == "-") as i64);
                        self.call_helper(Helper::DecAdd);
                        self.trap_status(at);
                    }
                    "*" => {
                        self.call_helper(Helper::DecMul);
                        self.trap_status(at);
                    }
                    _ => {
                        self.imm(R9, (self.lenient > 0) as i64);
                        self.call_helper(Helper::DecDiv);
                        self.trap_status(at);
                    }
                }
                let t = self.temp(16);
                self.store(t, RAX);
                self.store(t.at(8), RCX);
                Ok((t, Class::Dec))
            }
            other => Err(format!("linea {}: `{}` entre {:?} y {:?} no se emite al correr", at.0, other, ca, cb)),
        }
    }

    fn int_op(&mut self, op: &str, pa: Place, pb: Place, at: (usize, usize)) -> Result<Place, String> {
        self.load(pa, RAX);
        self.load(pb, RCX);
        match op {
            "+" | "-" | "*" => {
                match op {
                    "+" => x86::add_r64_r64(&mut self.code, RAX, RCX),
                    "-" => x86::sub_r64_r64(&mut self.code, RAX, RCX),
                    _ => x86::imul_r64_r64(&mut self.code, RAX, RCX),
                }
                self.trap(0x80, "T0060", "el resultado no cabe en 64 bits", at);
            }
            "/" | "%" => {
                x86::test_r64_r64(&mut self.code, RCX, RCX);
                self.trap(0x84, "T0061", "una division por cero", at);
                // -1 a mano: MIN / -1 no cabe, y el resto es 0
                x86::cmp_r64_imm8(&mut self.code, RCX, -1);
                let normal = self.jcc(0x85);
                if op == "/" {
                    self.imm(RDX, i64::MIN);
                    x86::cmp_r64_r64(&mut self.code, RAX, RDX);
                    self.trap(0x84, "T0060", "el resultado no cabe en 64 bits", at);
                    x86::neg_r64(&mut self.code, RAX);
                } else {
                    x86::zero_r32(&mut self.code, RAX);
                }
                let done = self.jmp();
                self.here(normal);
                x86::cqo(&mut self.code);
                x86::idiv_r64(&mut self.code, RCX);
                if op == "/" {
                    x86::test_r64_r64(&mut self.code, RDX, RDX);
                    self.trap(0x85, "T0062", "una division que no es exacta (entre enteros, 7 / 2 no es 3)", at);
                } else {
                    x86::mov_r64_r64(&mut self.code, RAX, RDX);
                }
                self.here(done);
            }
            other => return Err(format!("linea {}: `{}` entre enteros no se emite al correr", at.0, other)),
        }
        let t = self.temp(8);
        self.store(t, RAX);
        Ok(t)
    }

    /// `-x`: MIN no tiene opuesto que quepa (T0060).
    pub fn neg(&mut self, a: &Value, at: (usize, usize)) -> Result<(Place, Class), String> {
        let (p, c) = self.eval(a)?;
        self.load(p, RAX);
        self.imm(RCX, i64::MIN);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        self.trap(0x84, "T0060", "el resultado no cabe en 64 bits", at);
        x86::neg_r64(&mut self.code, RAX);
        let size = if c == Class::Dec { 16 } else { 8 };
        let t = self.temp(size);
        self.store(t, RAX);
        if c == Class::Dec {
            self.load(p.at(8), RAX);
            self.store(t.at(8), RAX);
        }
        Ok((t, c))
    }

    /// `round(x, n)`: lo de dentro con `lenient` (una division que no acaba
    /// llega a 18 decimales), y la mitad lejos del cero.
    pub fn round(&mut self, inner: &Value, n: u32, at: (usize, usize)) -> Result<(Place, Class), String> {
        self.lenient += 1;
        let r = self.eval(inner);
        self.lenient -= 1;
        let (p, c) = r?;
        if c == Class::F32 {
            return Err(super::later("round de un f32", "el f32 vive en la 3060: G4 de PLAN_EL_CENTAURO", at));
        }
        self.load_dec(p, &c, RAX, RCX);
        self.imm(RDX, n as i64);
        self.call_helper(Helper::DecRound);
        self.trap_status(at);
        let t = self.temp(16);
        self.store(t, RAX);
        self.store(t.at(8), RCX);
        Ok((t, Class::Dec))
    }

    // -- las subrutinas ------------------------------------------------

    pub fn helper(&mut self, h: Helper) {
        match h {
            Helper::Pow10 => self.h_pow10(),
            Helper::DecCmp => self.h_cmp(),
            Helper::DecAdd => self.h_add(),
            Helper::DecMul => self.h_mul(),
            Helper::DecDiv => self.h_div(),
            Helper::DecRound => self.h_round(),
            Helper::DecFit => self.h_fit(),
            Helper::DecShow => self.h_dec_show(),
            Helper::ParseInt => self.h_parse_int(),
            Helper::WriteQuoted => self.h_write_quoted(),
            Helper::WriteInt => {
                bmo_lower::fmt::write_i64(&mut self.code);
                self.ret();
            }
        }
    }

    fn set_status(&mut self, k: i64) {
        self.imm(R10, k);
    }

    fn ret(&mut self) {
        self.code.push(0xC3);
    }

    /// rax * 10^rcx -> rax; r10 = 1 si no cabe. Ensucia rcx, r11.
    fn h_pow10(&mut self) {
        x86::zero_r32(&mut self.code, R10);
        let top = self.code.len();
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let done = self.jcc(0x84);
        self.imm(R11, 10);
        x86::imul_r64_r64(&mut self.code, RAX, R11);
        let ovf = self.jcc(0x80);
        x86::dec_r64(&mut self.code, RCX);
        let back = self.jmp();
        x86::patch_jump_to(&mut self.code, back, top);
        self.here(ovf);
        self.set_status(1);
        self.here(done);
        self.ret();
    }

    /// round((rax, rcx), rdx): a rdx decimales, la mitad lejos del cero.
    fn h_round(&mut self) {
        x86::zero_r32(&mut self.code, R10);
        x86::cmp_r64_imm8(&mut self.code, RDX, 18);
        let ovf = self.jcc(0x8F);
        x86::cmp_r64_r64(&mut self.code, RCX, RDX);
        let cut = self.jcc(0x8F);
        // con menos decimales de los pedidos: se rellena con ceros
        x86::push_r64(&mut self.code, RDX);
        x86::mov_r64_r64(&mut self.code, R11, RDX);
        x86::sub_r64_r64(&mut self.code, R11, RCX);
        x86::mov_r64_r64(&mut self.code, RCX, R11);
        self.call_helper(Helper::Pow10);
        x86::pop_r64(&mut self.code, RCX);
        self.ret();
        self.here(cut);
        x86::push_r64(&mut self.code, RDX);
        x86::push_r64(&mut self.code, RAX);
        x86::mov_r64_r64(&mut self.code, R11, RCX);
        x86::sub_r64_r64(&mut self.code, R11, RDX);
        x86::mov_r64_r64(&mut self.code, RCX, R11);
        self.imm(RAX, 1);
        self.call_helper(Helper::Pow10);
        x86::mov_r64_r64(&mut self.code, R9, RAX);
        x86::pop_r64(&mut self.code, RAX);
        x86::cqo(&mut self.code);
        x86::idiv_r64(&mut self.code, R9);
        // |resto| * 2 >= q: se aleja del cero, hacia el lado del signo
        x86::mov_r64_r64(&mut self.code, R11, RDX);
        x86::test_r64_r64(&mut self.code, R11, R11);
        let abs_ok = self.jcc(0x89);
        x86::neg_r64(&mut self.code, R11);
        self.here(abs_ok);
        x86::add_r64_r64(&mut self.code, R11, R11);
        x86::cmp_r64_r64(&mut self.code, R11, R9);
        let keep = self.jcc(0x8C);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let down = self.jcc(0x88);
        x86::inc_r64(&mut self.code, RAX);
        let done = self.jmp();
        self.here(down);
        x86::dec_r64(&mut self.code, RAX);
        self.here(done);
        self.here(keep);
        x86::pop_r64(&mut self.code, RCX);
        x86::zero_r32(&mut self.code, R10);
        self.ret();
        self.here(ovf);
        self.set_status(1);
        self.ret();
    }

    /// El PIC: (rax, rcx) a `dec(rdx, r8)` -- sobran decimales que no son
    /// cero, o cifras de mas: r10 = 4 (T0074).
    fn h_fit(&mut self) {
        x86::zero_r32(&mut self.code, R10);
        let trim = self.code.len();
        x86::cmp_r64_r64(&mut self.code, RCX, R8);
        let trimmed = self.jcc(0x8E);
        x86::mov_r64_r64(&mut self.code, R11, RAX);
        x86::push_r64(&mut self.code, RDX);
        x86::cqo(&mut self.code);
        self.imm(R9, 10);
        x86::idiv_r64(&mut self.code, R9);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        x86::pop_r64(&mut self.code, RDX);
        let restore = self.jcc(0x85);
        x86::dec_r64(&mut self.code, RCX);
        let back = self.jmp();
        x86::patch_jump_to(&mut self.code, back, trim);
        self.here(restore);
        x86::mov_r64_r64(&mut self.code, RAX, R11);
        self.here(trimmed);
        x86::cmp_r64_r64(&mut self.code, RCX, R8);
        let size1 = self.jcc(0x8F);
        // relleno: rax * 10^(s - sc)
        x86::push_r64(&mut self.code, RDX);
        x86::mov_r64_r64(&mut self.code, R11, R8);
        x86::sub_r64_r64(&mut self.code, R11, RCX);
        x86::mov_r64_r64(&mut self.code, RCX, R11);
        self.call_helper(Helper::Pow10);
        x86::pop_r64(&mut self.code, RDX);
        x86::test_r64_r64(&mut self.code, R10, R10);
        let size2 = self.jcc(0x85);
        x86::mov_r64_r64(&mut self.code, RCX, R8);
        // |rax| < 10^p
        x86::push_r64(&mut self.code, RAX);
        x86::push_r64(&mut self.code, RCX);
        x86::mov_r64_r64(&mut self.code, RCX, RDX);
        self.imm(RAX, 1);
        self.call_helper(Helper::Pow10);
        x86::mov_r64_r64(&mut self.code, R9, RAX);
        x86::mov_r64_r64(&mut self.code, R11, R10);
        x86::pop_r64(&mut self.code, RCX);
        x86::pop_r64(&mut self.code, RAX);
        x86::zero_r32(&mut self.code, R10);
        x86::test_r64_r64(&mut self.code, R11, R11);
        let fits = self.jcc(0x85); // p tan grande que 10^p no cabe: cabe todo
        x86::mov_r64_r64(&mut self.code, R11, RAX);
        x86::test_r64_r64(&mut self.code, R11, R11);
        let pos = self.jcc(0x89);
        x86::neg_r64(&mut self.code, R11);
        self.here(pos);
        x86::cmp_r64_r64(&mut self.code, R11, R9);
        let size3 = self.jcc(0x83);
        self.here(fits);
        self.ret();
        for f in [size1, size2, size3] {
            self.here(f);
        }
        self.set_status(4);
        self.ret();
    }
}
