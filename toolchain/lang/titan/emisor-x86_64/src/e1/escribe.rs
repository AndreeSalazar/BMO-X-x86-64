//! **LO QUE SE ESCRIBE Y LO QUE SE LEE** -- un valor en la consola, letra a
//! letra como `Const::show` del calculo (la vara):
//!
//! ```text
//!    int, dec      sus cifras: 12.50, -0.05, 3
//!    texto         tal cual; DENTRO de una tabla o un registro, entre
//!                  comillas y con sus escapes ("a\"b"), como `{:?}` de Rust
//!    [T; n]        [1, 2, 3]
//!    type          Nave { x: 1, fuel: 12.50 }
//!    enum          Nada, o Circulo(2.0)
//! ```
//!
//! Y lo que entra: `lee()` (una linea de la consola) y `numero(t)` (el caso
//! `Es(n)` o `NoEs`, con la regla de `prelude::parse`).

use super::forma::short;
use super::{later, Helper, Place, E1, READ_MAX, TEXT_CAP};
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R10, R11, R8, R9};
use bmo_lower::{console, fmt, memoria};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::Value;

const RBP: u8 = super::RBP;

impl E1<'_> {
    pub fn show(&mut self, p: Place, c: &Class, inside: bool) -> Result<(), String> {
        match c {
            Class::Int | Class::Byte => {
                self.load(p, RAX);
                fmt::write_i64(&mut self.code);
            }
            Class::Bool => {
                self.load(p, RAX);
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                let no = self.jcc(0x84);
                console::write_const(&mut self.code, b"true");
                let done = self.jmp();
                self.here(no);
                console::write_const(&mut self.code, b"false");
                self.here(done);
            }
            Class::Dec => {
                self.load(p, RAX);
                self.load(p.at(8), RCX);
                self.call_helper(Helper::DecShow);
            }
            Class::Text => {
                self.addr(p.at(8), R8);
                self.load(p, R9);
                if inside {
                    self.call_helper(Helper::WriteQuoted);
                } else {
                    console::write_buffer(&mut self.code);
                }
            }
            Class::List(_) | Class::Map(..) | Class::Opt(_) | Class::Any => self.show_collection(p, c)?,
            Class::Table(inner, n) => {
                console::write_const(&mut self.code, b"[");
                if *n > 0 {
                    let sz = self.forms.size(inner);
                    let inner = (**inner).clone();
                    self.show(p, &inner, true)?;
                    self.each_cell(n - 1, &[(p.at(sz), sz)], &mut |e, cells| {
                        console::write_const(&mut e.code, b", ");
                        e.show(cells[0], &inner, true)
                    })?;
                }
                console::write_const(&mut self.code, b"]");
            }
            Class::Record(t) => {
                let name = short(&self.m.types[*t].name).to_string();
                console::write_const(&mut self.code, format!("{} {{ ", name).as_bytes());
                for k in 0..self.m.types[*t].fields.len() {
                    let fname = self.m.types[*t].fields[k].name.clone();
                    let sep = if k > 0 { ", " } else { "" };
                    console::write_const(&mut self.code, format!("{}{}: ", sep, fname).as_bytes());
                    let (off, fc, _) = self.forms.field_k(*t, k);
                    self.show(p.at(off), &fc, true)?;
                }
                console::write_const(&mut self.code, b" }");
            }
            Class::Enum(e) => {
                let mut ends = Vec::new();
                for v in 0..self.m.enums[*e].cases.len() {
                    self.load(p, RAX);
                    self.imm(RCX, v as i64);
                    x86::cmp_r64_r64(&mut self.code, RAX, RCX);
                    let next = self.jcc(0x85);
                    let case = short(&self.m.enums[*e].cases[v].name).to_string();
                    console::write_const(&mut self.code, case.as_bytes());
                    let k = self.m.enums[*e].cases[v].fields.len();
                    if k > 0 {
                        console::write_const(&mut self.code, b"(");
                        for j in 0..k {
                            if j > 0 {
                                console::write_const(&mut self.code, b", ");
                            }
                            let (off, fc, _) = self.forms.case_field(*e, v, j);
                            self.show(p.at(off), &fc, true)?;
                        }
                        console::write_const(&mut self.code, b")");
                    }
                    ends.push(self.jmp());
                    self.here(next);
                }
                for f in ends {
                    self.here(f);
                }
            }
            Class::Trait(k) => {
                let mut ends = Vec::new();
                for (ty, tc) in self.forms.trait_types(*k) {
                    let id = self.forms.type_id(&ty);
                    self.load(p, RAX);
                    self.imm(RCX, id);
                    x86::cmp_r64_r64(&mut self.code, RAX, RCX);
                    let next = self.jcc(0x85);
                    self.show(p.at(8), &tc, inside)?;
                    ends.push(self.jmp());
                    self.here(next);
                }
                for f in ends {
                    self.here(f);
                }
            }
            Class::F32 => return Err(later("escribir un f32", "el f32 vive en la 3060: lo que se muestra es un `dec`", (0, 0))),
        }
        Ok(())
    }

    /// `lee()` a un sitio de texto: una linea de la consola, sin su salto.
    /// La consola guarda READ_MAX bytes y tira los de mas: una linea que
    /// LLENA el sitio puede venir cortada, y cortar en silencio no se hace
    /// -- es un NO (las de READ_MAX - 1 bytes o menos llegan enteras).
    pub fn read_into(&mut self, t: Place, at: (usize, usize)) {
        self.addr(t.at(8), R8);
        console::read_line(&mut self.code, READ_MAX);
        x86::cmp_r64_imm8(&mut self.code, R9, READ_MAX as i8);
        self.trap(0x83, "T0060", &format!("la linea tecleada pasa de {} bytes", READ_MAX - 1), at);
        self.store(t, R9);
    }

    /// `numero(t)`: `Es(n)` o `NoEs` (`prelude::parse`, la misma regla).
    pub fn number(&mut self, inner: &Value, e: usize) -> Result<(Place, Class), String> {
        let (p, _) = self.eval(inner)?;
        self.addr(p.at(8), R8);
        self.load(p, R9);
        self.call_helper(Helper::ParseInt);
        let c = Class::Enum(e);
        let t = self.temp(self.forms.size(&c));
        x86::test_r64_r64(&mut self.code, R10, R10);
        let no = self.jcc(0x85);
        self.store(t.at(8), RAX);
        self.store_imm(t, 0);
        let done = self.jmp();
        self.here(no);
        self.store_imm(t, 1);
        self.here(done);
        Ok((t, c))
    }

    /// `a + b` entre textos: los dos seguidos, si caben.
    pub fn join(&mut self, pa: Place, pb: Place, at: (usize, usize)) -> Place {
        let t = self.temp(8 + TEXT_CAP as i32);
        self.load(pa, RCX);
        self.load(pb, RDX);
        x86::add_r64_r64(&mut self.code, RCX, RDX);
        x86::cmp_r64_imm32(&mut self.code, RCX, TEXT_CAP as i32);
        self.trap(0x8F, "T0060", &format!("el texto pasaria de {} bytes, lo mas que guarda un texto al correr", TEXT_CAP), at);
        self.store(t, RCX);
        self.addr(t.at(8), RDI);
        self.addr(pa.at(8), RSI);
        self.load(pa, RCX);
        memoria::copiar(&mut self.code);
        self.addr(pb.at(8), RSI);
        self.load(pb, RCX);
        memoria::copiar(&mut self.code);
        t
    }

    // -- las subrutinas ------------------------------------------------

    /// (rax, rcx) como `show_dec`: 12.50, -0.05, 3.
    pub fn h_dec_show(&mut self) {
        // el marco: el signo -8, la escala -16; el texto se escribe hacia
        // atras desde rbp - 16
        self.code.push(0x55);
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]);
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC, 64, 0, 0, 0]);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, -16, RCX);
        x86::mov_r64_r64(&mut self.code, R9, RAX);
        x86::shr_r64_imm8(&mut self.code, R9, 63);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, -8, R9);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let pos = self.jcc(0x89);
        x86::neg_r64(&mut self.code, RAX);
        self.here(pos);
        self.lea(R8, RBP, -16);
        x86::zero_r32(&mut self.code, R11);
        let top = self.code.len();
        x86::zero_r32(&mut self.code, RDX);
        self.imm(RCX, 10);
        x86::div_r64(&mut self.code, RCX);
        x86::add_r64_imm8(&mut self.code, RDX, b'0' as i8);
        x86::dec_r64(&mut self.code, R8);
        x86::mov_byte_at_reg_from_low(&mut self.code, R8, RDX);
        x86::inc_r64(&mut self.code, R11);
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, RBP, -16);
        x86::cmp_r64_r64(&mut self.code, R11, RCX);
        let no_dot = self.jcc(0x85);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let no_dot2 = self.jcc(0x84);
        x86::dec_r64(&mut self.code, R8);
        x86::mov_byte_at_reg_imm8(&mut self.code, R8, b'.');
        self.here(no_dot);
        self.here(no_dot2);
        // sigue mientras queden cifras, o mientras no haya llegado al punto
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let more = self.jcc(0x85);
        x86::patch_jump_to(&mut self.code, more, top);
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, RBP, -16);
        x86::cmp_r64_r64(&mut self.code, R11, RCX);
        let more2 = self.jcc(0x8E);
        x86::patch_jump_to(&mut self.code, more2, top);
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, RBP, -8);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let unsigned = self.jcc(0x84);
        x86::dec_r64(&mut self.code, R8);
        x86::mov_byte_at_reg_imm8(&mut self.code, R8, b'-');
        self.here(unsigned);
        self.lea(R9, RBP, -16);
        x86::sub_r64_r64(&mut self.code, R9, R8);
        console::write_buffer(&mut self.code);
        self.code.extend_from_slice(&[0x48, 0x89, 0xEC]);
        self.code.push(0x5D);
        self.code.push(0xC3);
    }

    /// (r8, r9) -> rax, r10 = 0 si es un numero entero (`prelude::parse`):
    /// sin blancos alrededor, un `+` o `-`, una cifra o mas, en 64 bits.
    pub fn h_parse_int(&mut self) {
        x86::zero_r32(&mut self.code, R10);
        x86::zero_r32(&mut self.code, RSI);
        x86::zero_r32(&mut self.code, RDI);
        let blank = |e: &mut Self, skip: &mut Vec<usize>| {
            for b in [b' ', b'\t', b'\r', b'\n'] {
                x86::cmp_r64_imm8(&mut e.code, RAX, b as i8);
                skip.push(e.jcc(0x84));
            }
        };
        // los blancos de delante
        let front = self.code.len();
        x86::test_r64_r64(&mut self.code, R9, R9);
        let fail0 = self.jcc(0x84);
        x86::movzx_r32_byte_at_reg(&mut self.code, RAX, R8);
        let mut skips = Vec::new();
        blank(self, &mut skips);
        let front_done = self.jmp();
        for s in skips {
            self.here(s);
        }
        x86::inc_r64(&mut self.code, R8);
        x86::dec_r64(&mut self.code, R9);
        let b1 = self.jmp();
        x86::patch_jump_to(&mut self.code, b1, front);
        self.here(front_done);
        // los de detras (queda al menos un byte que no es blanco)
        let back = self.code.len();
        x86::mov_r64_r64(&mut self.code, R11, R9);
        x86::dec_r64(&mut self.code, R11);
        x86::movzx_r32_byte_base_index(&mut self.code, RAX, R8, R11);
        let mut skips = Vec::new();
        blank(self, &mut skips);
        let back_done = self.jmp();
        for s in skips {
            self.here(s);
        }
        x86::dec_r64(&mut self.code, R9);
        let b2 = self.jmp();
        x86::patch_jump_to(&mut self.code, b2, back);
        self.here(back_done);
        // el signo
        x86::movzx_r32_byte_at_reg(&mut self.code, RAX, R8);
        x86::cmp_r64_imm8(&mut self.code, RAX, b'-' as i8);
        let not_minus = self.jcc(0x85);
        self.imm(RSI, 1);
        let skip_sign = self.jmp();
        self.here(not_minus);
        x86::cmp_r64_imm8(&mut self.code, RAX, b'+' as i8);
        let digits = self.jcc(0x85);
        self.here(skip_sign);
        x86::inc_r64(&mut self.code, R8);
        x86::dec_r64(&mut self.code, R9);
        self.here(digits);
        x86::test_r64_r64(&mut self.code, R9, R9);
        let fail1 = self.jcc(0x84);
        // las cifras, contando en NEGATIVO (asi cabe MIN)
        let top = self.code.len();
        x86::test_r64_r64(&mut self.code, R9, R9);
        let end = self.jcc(0x84);
        x86::movzx_r32_byte_at_reg(&mut self.code, RAX, R8);
        x86::sub_r64_imm8(&mut self.code, RAX, b'0' as i8);
        x86::cmp_r64_imm8(&mut self.code, RAX, 9);
        let fail2 = self.jcc(0x87);
        self.imm(RCX, 10);
        x86::imul_r64_r64(&mut self.code, RDI, RCX);
        let fail3 = self.jcc(0x80);
        x86::sub_r64_r64(&mut self.code, RDI, RAX);
        let fail4 = self.jcc(0x80);
        x86::inc_r64(&mut self.code, R8);
        x86::dec_r64(&mut self.code, R9);
        let b3 = self.jmp();
        x86::patch_jump_to(&mut self.code, b3, top);
        self.here(end);
        x86::mov_r64_r64(&mut self.code, RAX, RDI);
        x86::test_r64_r64(&mut self.code, RSI, RSI);
        let negative = self.jcc(0x85);
        self.imm(RCX, i64::MIN);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let fail5 = self.jcc(0x84);
        x86::neg_r64(&mut self.code, RAX);
        self.here(negative);
        self.code.push(0xC3);
        for f in [fail0, fail1, fail2, fail3, fail4, fail5] {
            self.here(f);
        }
        self.imm(R10, 1);
        self.code.push(0xC3);
    }

    /// (r8, r9) entre comillas y con sus escapes, como `{:?}` de Rust.
    pub fn h_write_quoted(&mut self) {
        self.code.push(0x55);
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]);
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC, 0x00, 0x08, 0, 0]); // sub rsp, 2048
        self.lea(RDI, RBP, -2048);
        x86::mov_byte_at_reg_imm8(&mut self.code, RDI, b'"');
        x86::inc_r64(&mut self.code, RDI);
        let top = self.code.len();
        x86::test_r64_r64(&mut self.code, R9, R9);
        let close = self.jcc(0x84);
        x86::movzx_r32_byte_at_reg(&mut self.code, RAX, R8);
        let mut done = Vec::new();
        // los que llevan su barra: \" \\ \n \r \t \0
        for (b, esc) in [(b'"', b'"'), (b'\\', b'\\'), (b'\n', b'n'), (b'\r', b'r'), (b'\t', b't'), (0u8, b'0')] {
            x86::cmp_r64_imm8(&mut self.code, RAX, b as i8);
            let next = self.jcc(0x85);
            x86::mov_byte_at_reg_imm8(&mut self.code, RDI, b'\\');
            x86::inc_r64(&mut self.code, RDI);
            x86::mov_byte_at_reg_imm8(&mut self.code, RDI, esc);
            x86::inc_r64(&mut self.code, RDI);
            done.push(self.jmp());
            self.here(next);
        }
        // los de control: \u{1b}
        x86::cmp_r64_imm8(&mut self.code, RAX, 0x20);
        let control = self.jcc(0x82);
        x86::cmp_r64_imm8(&mut self.code, RAX, 0x7F);
        let control2 = self.jcc(0x84);
        x86::mov_byte_at_reg_from_low(&mut self.code, RDI, RAX);
        x86::inc_r64(&mut self.code, RDI);
        done.push(self.jmp());
        self.here(control);
        self.here(control2);
        for b in *b"\\u{" {
            x86::mov_byte_at_reg_imm8(&mut self.code, RDI, b);
            x86::inc_r64(&mut self.code, RDI);
        }
        x86::cmp_r64_imm8(&mut self.code, RAX, 16);
        let one_digit = self.jcc(0x82);
        x86::mov_r64_r64(&mut self.code, RCX, RAX);
        x86::shr_r64_imm8(&mut self.code, RCX, 4);
        self.hex_digit();
        self.here(one_digit);
        x86::mov_r64_r64(&mut self.code, RCX, RAX);
        self.imm(RDX, 15);
        // rcx & 15: and con un registro
        self.code.extend_from_slice(&[0x48, 0x21, 0xD1]); // and rcx, rdx
        self.hex_digit();
        x86::mov_byte_at_reg_imm8(&mut self.code, RDI, b'}');
        x86::inc_r64(&mut self.code, RDI);
        for d in done {
            self.here(d);
        }
        x86::inc_r64(&mut self.code, R8);
        x86::dec_r64(&mut self.code, R9);
        let back = self.jmp();
        x86::patch_jump_to(&mut self.code, back, top);
        self.here(close);
        x86::mov_byte_at_reg_imm8(&mut self.code, RDI, b'"');
        x86::inc_r64(&mut self.code, RDI);
        self.lea(R8, RBP, -2048);
        x86::mov_r64_r64(&mut self.code, R9, RDI);
        x86::sub_r64_r64(&mut self.code, R9, R8);
        console::write_buffer(&mut self.code);
        self.code.extend_from_slice(&[0x48, 0x89, 0xEC]);
        self.code.push(0x5D);
        self.code.push(0xC3);
    }

    /// La cifra hexadecimal de rcx (0-15), en minuscula, a [rdi]; rdi avanza.
    fn hex_digit(&mut self) {
        x86::cmp_r64_imm8(&mut self.code, RCX, 10);
        let digit = self.jcc(0x82);
        x86::add_r64_imm8(&mut self.code, RCX, (b'a' - b'0' - 10) as i8);
        self.here(digit);
        x86::add_r64_imm8(&mut self.code, RCX, b'0' as i8);
        x86::mov_byte_at_reg_from_low(&mut self.code, RDI, RCX);
        x86::inc_r64(&mut self.code, RDI);
    }
}
