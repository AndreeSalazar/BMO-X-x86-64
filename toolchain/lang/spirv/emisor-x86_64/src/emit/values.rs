//! **Las instrucciones de VALOR**: aritmetica, comparaciones, conversiones,
//! compuestos y `GLSL.std.450`. Cada una, componente a componente, desenrollada
//! (un vector tiene como mucho cuatro).
//!
//! La regla que manda aqui: **hacer lo que hace `bmo_spirv_front::math`**, en
//! el mismo orden de operaciones y con las mismas decisiones donde SPIR-V deja
//! el hueco (min/max con NaN, saturar al convertir, `mix` sin fusionar). La
//! prueba diferencial compara bits, no parecidos.
//!
//! [consumo]  NADA   emite y se va

use super::{Emitter, Paso};
use crate::asm::{cc, Alu, R9, RAX, RCX, RDI, RDX};
use crate::layout::{dense_step, type_of};
use crate::trap;
use bmo_spirv_front::table::{glsl as g, op};
use bmo_spirv_front::{glsl_info, GlslGroup, Instruction, Reason};

const ONE: u32 = 0x3F80_0000; // 1.0f32

impl<'m, 'a, 'b, 't, 'c> Emitter<'m, 'a, 'b, 't, 'c> {
    pub(super) fn value(&mut self, ins: &Instruction) -> Paso<()> {
        let m = self.m;
        let opcode = ins.opcode;
        let r = ins.op(2);
        let n = self.len(ins.op(1)).min(4) as i32;
        let (ra, a, b) = (self.at(r), self.at(ins.op(3)), self.at(ins.op(4)));
        match opcode {
            op::OpIAdd
            | op::OpISub
            | op::OpIMul
            | op::OpUDiv
            | op::OpSDiv
            | op::OpUMod
            | op::OpSRem
            | op::OpSMod
            | op::OpShiftRightLogical
            | op::OpShiftRightArithmetic
            | op::OpShiftLeftLogical
            | op::OpBitwiseOr
            | op::OpBitwiseXor
            | op::OpBitwiseAnd => {
                for k in 0..n {
                    self.w.load(false, RAX, RDI, a + 4 * k);
                    self.w.load(false, RCX, RDI, b + 4 * k);
                    self.int_binary(opcode);
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpSNegate | op::OpNot => {
                for k in 0..n {
                    self.w.load(false, RAX, RDI, a + 4 * k);
                    self.w.f7(false, if opcode == op::OpNot { 2 } else { 3 }, RAX);
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpFAdd | op::OpFSub | op::OpFMul | op::OpFDiv => {
                let code = match opcode {
                    op::OpFAdd => 0x58,
                    op::OpFSub => 0x5C,
                    op::OpFMul => 0x59,
                    _ => 0x5E,
                };
                for k in 0..n {
                    self.w.movss_load(0, RDI, a + 4 * k);
                    self.w.movss_load(1, RDI, b + 4 * k);
                    self.w.ss(code, 0, 1);
                    self.w.movss_store(RDI, ra + 4 * k, 0);
                }
            }
            op::OpFRem | op::OpFMod => {
                // x - y * trunc(x / y)   /   x - y * floor(x / y)
                for k in 0..n {
                    self.w.movss_load(0, RDI, a + 4 * k);
                    self.w.movss_load(1, RDI, b + 4 * k);
                    self.w.ss(0x10, 2, 0);
                    self.w.ss(0x5E, 2, 1);
                    if opcode == op::OpFRem {
                        self.trunc_x(2, 3);
                    } else {
                        self.floor_x(2, 3);
                    }
                    self.w.ss(0x59, 1, 3);
                    self.w.ss(0x5C, 0, 1);
                    self.w.movss_store(RDI, ra + 4 * k, 0);
                }
            }
            op::OpFNegate => {
                for k in 0..n {
                    self.w.load(false, RAX, RDI, a + 4 * k);
                    self.w.alu_imm(false, Alu::Xor, RAX, 0x8000_0000);
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpVectorTimesScalar => {
                self.w.movss_load(1, RDI, b);
                for k in 0..n {
                    self.w.movss_load(0, RDI, a + 4 * k);
                    self.w.ss(0x59, 0, 1);
                    self.w.movss_store(RDI, ra + 4 * k, 0);
                }
            }
            op::OpDot => {
                let na = self.len(type_of(m, ins.op(3))) as i32;
                self.w.movss_load(0, RDI, a);
                self.w.movss_load(1, RDI, b);
                self.w.ss(0x59, 0, 1);
                for k in 1..na {
                    self.w.movss_load(2, RDI, a + 4 * k);
                    self.w.movss_load(1, RDI, b + 4 * k);
                    self.w.ss(0x59, 2, 1);
                    self.w.ss(0x58, 0, 2);
                }
                self.w.movss_store(RDI, ra, 0);
            }
            op::OpIEqual..=op::OpSLessThanEqual => {
                let c = match opcode {
                    op::OpIEqual => cc::E,
                    op::OpINotEqual => cc::NE,
                    op::OpUGreaterThan => cc::A,
                    op::OpSGreaterThan => cc::G,
                    op::OpUGreaterThanEqual => cc::AE,
                    op::OpSGreaterThanEqual => cc::GE,
                    op::OpULessThan => cc::B,
                    op::OpSLessThan => cc::L,
                    op::OpULessThanEqual => cc::BE,
                    _ => cc::LE,
                };
                for k in 0..n {
                    self.w.load(false, RAX, RDI, a + 4 * k);
                    self.w.load(false, RCX, RDI, b + 4 * k);
                    self.w.alu(false, Alu::Cmp, RAX, RCX);
                    self.w.setcc_eax(c);
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpFOrdEqual..=op::OpFUnordGreaterThanEqual => {
                for k in 0..n {
                    self.w.movss_load(0, RDI, a + 4 * k);
                    self.w.movss_load(1, RDI, b + 4 * k);
                    self.float_compare(opcode);
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpIsNan | op::OpIsInf => {
                for k in 0..n {
                    if opcode == op::OpIsNan {
                        self.w.movss_load(0, RDI, a + 4 * k);
                        self.w.ucomiss(0, 0);
                        self.w.setcc_eax(cc::P);
                    } else {
                        self.w.load(false, RAX, RDI, a + 4 * k);
                        self.w.alu_imm(false, Alu::And, RAX, 0x7FFF_FFFF);
                        self.w.alu_imm(false, Alu::Cmp, RAX, 0x7F80_0000);
                        self.w.setcc_eax(cc::E);
                    }
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpLogicalEqual | op::OpLogicalNotEqual | op::OpLogicalOr | op::OpLogicalAnd => {
                for k in 0..n {
                    self.w.load(false, RAX, RDI, a + 4 * k);
                    self.w.load(false, RCX, RDI, b + 4 * k);
                    match opcode {
                        op::OpLogicalOr => self.w.alu(false, Alu::Or, RAX, RCX),
                        op::OpLogicalAnd => self.w.alu(false, Alu::And, RAX, RCX),
                        _ => {
                            self.w.alu(false, Alu::Cmp, RAX, RCX);
                            self.w.setcc_eax(if opcode == op::OpLogicalEqual { cc::E } else { cc::NE });
                        }
                    }
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpLogicalNot => {
                for k in 0..n {
                    self.w.load(false, RAX, RDI, a + 4 * k);
                    self.w.alu_imm(false, Alu::Xor, RAX, 1);
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpAny | op::OpAll => {
                let na = self.len(type_of(m, ins.op(3))) as i32;
                self.w.load(false, RAX, RDI, a);
                for k in 1..na {
                    self.w.load(false, RCX, RDI, a + 4 * k);
                    self.w.alu(false, if opcode == op::OpAny { Alu::Or } else { Alu::And }, RAX, RCX);
                }
                self.w.store(false, RDI, ra, RAX);
            }
            op::OpSelect => {
                let cn = self.len(type_of(m, ins.op(3)));
                let (x, y) = (self.at(ins.op(4)), self.at(ins.op(5)));
                for k in 0..n {
                    let c = if cn == 1 { a } else { a + 4 * k };
                    self.w.load(false, RAX, RDI, x + 4 * k);
                    self.w.cmp_mem_imm(RDI, c, 0);
                    let si = self.w.jcc_fwd(cc::NE);
                    self.w.load(false, RAX, RDI, y + 4 * k);
                    self.w.here(si);
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpConvertFToU | op::OpConvertFToS => {
                for k in 0..n {
                    self.w.movss_load(0, RDI, a + 4 * k);
                    self.float_to_int(opcode == op::OpConvertFToS);
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpConvertSToF | op::OpConvertUToF => {
                for k in 0..n {
                    self.w.load(false, RAX, RDI, a + 4 * k);
                    // Sin signo: la palabra ya esta extendida con ceros en rax,
                    // y convertir los 64 bits da el valor sin signo exacto.
                    self.w.cvtsi2ss(opcode == op::OpConvertUToF, 0, RAX);
                    self.w.movss_store(RDI, ra + 4 * k, 0);
                }
            }
            // Todo es de 32 bits: estas no cambian ningun bit.
            op::OpUConvert | op::OpSConvert | op::OpFConvert | op::OpBitcast | op::OpCopyObject => {
                let n = self.len(ins.op(1));
                self.copy(a, ra, n);
            }
            op::OpUndef => {
                let n = self.len(ins.op(1));
                self.zero(ra, n);
            }
            op::OpCompositeConstruct => {
                let mut d = ra;
                for k in 3..ins.words as usize {
                    let p = ins.op(k);
                    let np = self.len(type_of(m, p));
                    let src = self.at(p);
                    self.copy(src, d, np);
                    d += 4 * np as i32;
                }
            }
            op::OpCompositeExtract => {
                let mut t = type_of(m, ins.op(3));
                let mut off = 0u32;
                for k in 4..ins.words as usize {
                    let (o, s) = dense_step(m, t, ins.op(k)).ok_or(Reason::OutOfBounds)?;
                    off += o;
                    t = s;
                }
                let nr = self.len(ins.op(1));
                self.copy(a + 4 * off as i32, ra, nr);
            }
            op::OpCompositeInsert => {
                let nr = self.len(ins.op(1));
                self.copy(b, ra, nr);
                let mut t = type_of(m, ins.op(4));
                let mut off = 0u32;
                for k in 5..ins.words as usize {
                    let (o, s) = dense_step(m, t, ins.op(k)).ok_or(Reason::OutOfBounds)?;
                    off += o;
                    t = s;
                }
                let no = self.len(type_of(m, ins.op(3)));
                self.copy(a, ra + 4 * off as i32, no);
            }
            op::OpVectorShuffle => {
                let na = self.len(type_of(m, ins.op(3)));
                for k in 0..n {
                    let i = ins.op(5 + k as usize);
                    if i == u32::MAX {
                        self.w.store_imm(RDI, ra + 4 * k, 0);
                        continue;
                    }
                    let src = if i < na { a + 4 * i as i32 } else { b + 4 * (i - na) as i32 };
                    self.w.load(false, RAX, RDI, src);
                    self.w.store(false, RDI, ra + 4 * k, RAX);
                }
            }
            op::OpVectorExtractDynamic => {
                let nv = self.len(type_of(m, ins.op(3)));
                self.w.load(false, RDX, RDI, b);
                self.w.alu_imm(false, Alu::Cmp, RDX, nv);
                self.trap_if(cc::AE, trap::OUT_OF_BOUNDS);
                self.w.shift_imm(false, 4, RDX, 2);
                self.w.mov(true, RCX, RDI);
                self.w.alu(true, Alu::Add, RCX, RDX);
                self.w.load(false, RAX, RCX, a);
                self.w.store(false, RDI, ra, RAX);
            }
            op::OpVectorInsertDynamic => {
                let nv = self.len(ins.op(1));
                self.copy(a, ra, nv);
                let idx = self.at(ins.op(5));
                self.w.load(false, RDX, RDI, idx);
                self.w.alu_imm(false, Alu::Cmp, RDX, nv);
                self.trap_if(cc::AE, trap::OUT_OF_BOUNDS);
                self.w.shift_imm(false, 4, RDX, 2);
                self.w.mov(true, RCX, RDI);
                self.w.alu(true, Alu::Add, RCX, RDX);
                self.w.load(false, RAX, RDI, b);
                self.w.store(false, RCX, ra, RAX);
            }
            op::OpExtInst => self.glsl(ins, n)?,
            op::OpVariable
            | op::OpLoad
            | op::OpStore
            | op::OpCopyMemory
            | op::OpAccessChain
            | op::OpInBoundsAccessChain
            | op::OpArrayLength => self.memory(ins)?,
            _ => return Err(Reason::NotYet { what: "instruccion que el emisor aun no traduce" }),
        }
        Ok(())
    }

    /// `eax op= ecx` con las trampas del oraculo.
    fn int_binary(&mut self, opcode: u16) {
        let w = &mut self.w;
        match opcode {
            op::OpIAdd => w.alu(false, Alu::Add, RAX, RCX),
            op::OpISub => w.alu(false, Alu::Sub, RAX, RCX),
            op::OpIMul => w.imul(false, RAX, RCX),
            op::OpBitwiseOr => w.alu(false, Alu::Or, RAX, RCX),
            op::OpBitwiseXor => w.alu(false, Alu::Xor, RAX, RCX),
            op::OpBitwiseAnd => w.alu(false, Alu::And, RAX, RCX),
            op::OpShiftRightLogical | op::OpShiftRightArithmetic | op::OpShiftLeftLogical => {
                w.alu_imm(false, Alu::Cmp, RCX, 32);
                self.trap_if(cc::AE, trap::SHIFT_TOO_LARGE);
                let ext = match opcode {
                    op::OpShiftRightLogical => 5,
                    op::OpShiftRightArithmetic => 7,
                    _ => 4,
                };
                self.w.shift_cl(ext, RAX);
            }
            op::OpUDiv | op::OpUMod => {
                w.test(RCX, RCX);
                self.trap_if(cc::E, trap::DIVISION_BY_ZERO);
                self.w.alu(false, Alu::Xor, RDX, RDX);
                self.w.f7(true, 6, RCX);
                if opcode == op::OpUMod {
                    self.w.mov(false, RAX, RDX);
                }
            }
            _ => {
                // SDiv / SRem / SMod: con signo, en 64 bits (no desborda).
                w.test(RCX, RCX);
                self.trap_if(cc::E, trap::DIVISION_BY_ZERO);
                self.w.alu_imm(false, Alu::Cmp, RAX, 0x8000_0000);
                let no_min = self.w.jcc_fwd(cc::NE);
                self.w.alu_imm(false, Alu::Cmp, RCX, 0xFFFF_FFFF);
                self.trap_if(cc::E, trap::DIVISION_OVERFLOW);
                self.w.here(no_min);
                self.w.movsxd(RAX, RAX);
                self.w.movsxd(RCX, RCX);
                self.w.cqo();
                self.w.f7(true, 7, RCX);
                match opcode {
                    op::OpSDiv => {}
                    op::OpSRem => self.w.mov(false, RAX, RDX),
                    _ => {
                        // SMod: el signo del divisor. r != 0 y signo(r) != signo(y) -> r += y.
                        self.w.test(RDX, RDX);
                        let cero = self.w.jcc_fwd(cc::E);
                        self.w.mov(false, R9, RDX);
                        self.w.alu(false, Alu::Xor, R9, RCX);
                        let igual = self.w.jcc_fwd(cc::S ^ 1);
                        self.w.alu(false, Alu::Add, RDX, RCX);
                        self.w.here(igual);
                        self.w.here(cero);
                        self.w.mov(false, RAX, RDX);
                    }
                }
            }
        }
    }

    /// `xmm0` contra `xmm1` -> `eax` = 0 o 1.
    fn float_compare(&mut self, opcode: u16) {
        // Las no ordenadas son la NEGACION de la ordenada contraria.
        let (base, negar) = match opcode {
            op::OpFUnordEqual => (op::OpFOrdNotEqual, true),
            op::OpFUnordNotEqual => (op::OpFOrdEqual, true),
            op::OpFUnordLessThan => (op::OpFOrdGreaterThanEqual, true),
            op::OpFUnordGreaterThan => (op::OpFOrdLessThanEqual, true),
            op::OpFUnordLessThanEqual => (op::OpFOrdGreaterThan, true),
            op::OpFUnordGreaterThanEqual => (op::OpFOrdLessThan, true),
            o => (o, false),
        };
        let w = &mut self.w;
        match base {
            op::OpFOrdEqual | op::OpFOrdNotEqual => {
                w.ucomiss(0, 1);
                w.setcc_eax(if base == op::OpFOrdEqual { cc::E } else { cc::NE });
                w.setcc_ecx(cc::NP);
                w.alu(false, Alu::And, RAX, RCX);
            }
            op::OpFOrdLessThan => {
                w.ucomiss(1, 0);
                w.setcc_eax(cc::A);
            }
            op::OpFOrdLessThanEqual => {
                w.ucomiss(1, 0);
                w.setcc_eax(cc::AE);
            }
            op::OpFOrdGreaterThan => {
                w.ucomiss(0, 1);
                w.setcc_eax(cc::A);
            }
            _ => {
                w.ucomiss(0, 1);
                w.setcc_eax(cc::AE);
            }
        }
        if negar {
            self.w.alu_imm(false, Alu::Xor, RAX, 1);
        }
    }

    /// `xmm0` a entero en `eax`, SATURANDO como `math::to_u32` / `to_i32`.
    fn float_to_int(&mut self, signed: bool) {
        let w = &mut self.w;
        w.ucomiss(0, 0);
        let nan = w.jcc_fwd(cc::P);
        let (lo, hi, minimo, maximo) = if signed {
            (0xCF00_0000u32, 0x4F00_0000u32, 0x8000_0000u32, 0x7FFF_FFFFu32)
        } else {
            (0u32, 0x4F80_0000u32, 0u32, 0xFFFF_FFFFu32)
        };
        w.mov_imm(RAX, lo);
        w.movd_to_xmm(false, 1, RAX);
        w.ucomiss(0, 1);
        let bajo = w.jcc_fwd(cc::B);
        w.mov_imm(RAX, hi);
        w.movd_to_xmm(false, 1, RAX);
        w.ucomiss(0, 1);
        let alto = w.jcc_fwd(cc::AE);
        w.cvttss2si(!signed, RAX, 0);
        let fin1 = w.jmp_fwd();
        w.here(nan);
        w.mov_imm(RAX, 0);
        let fin2 = w.jmp_fwd();
        w.here(bajo);
        w.mov_imm(RAX, minimo);
        let fin3 = w.jmp_fwd();
        w.here(alto);
        w.mov_imm(RAX, maximo);
        w.here(fin1);
        w.here(fin2);
        w.here(fin3);
    }

    /// `trunc` de `math`, sobre los bits de `eax`. Toca `ecx`, `edx`.
    fn trunc_eax(&mut self) {
        let w = &mut self.w;
        w.mov(false, RCX, RAX);
        w.shift_imm(false, 5, RCX, 23);
        w.alu_imm(false, Alu::And, RCX, 0xFF);
        w.alu_imm(false, Alu::Sub, RCX, 127);
        w.alu_imm(false, Alu::Cmp, RCX, 23);
        let entero = w.jcc_fwd(cc::GE);
        w.test(RCX, RCX);
        let signo = w.jcc_fwd(cc::S);
        w.mov_imm(RDX, 0x007F_FFFF);
        w.shift_cl(5, RDX);
        w.f7(false, 2, RDX);
        w.alu(false, Alu::And, RAX, RDX);
        let fin = w.jmp_fwd();
        w.here(signo);
        w.alu_imm(false, Alu::And, RAX, 0x8000_0000);
        w.here(entero);
        w.here(fin);
    }

    /// `dst = trunc(src)` (xmm).
    fn trunc_x(&mut self, src: u8, dst: u8) {
        self.w.movd_from_xmm(false, RAX, src);
        self.trunc_eax();
        self.w.movd_to_xmm(false, dst, RAX);
    }

    /// `dst = floor(src)`: `t = trunc(x); x < t ? t - 1 : t`. Usa xmm7.
    fn floor_x(&mut self, src: u8, dst: u8) {
        self.trunc_x(src, dst);
        self.w.ucomiss(dst, src);
        let no = self.w.jcc_fwd(cc::BE);
        self.w.mov_imm(RAX, ONE);
        self.w.movd_to_xmm(false, 7, RAX);
        self.w.ss(0x5C, dst, 7);
        self.w.here(no);
    }

    /// `dst = ceil(src)`: `x > t ? t + 1 : t`. Usa xmm7.
    fn ceil_x(&mut self, src: u8, dst: u8) {
        self.trunc_x(src, dst);
        self.w.ucomiss(src, dst);
        let no = self.w.jcc_fwd(cc::BE);
        self.w.mov_imm(RAX, ONE);
        self.w.movd_to_xmm(false, 7, RAX);
        self.w.ss(0x58, dst, 7);
        self.w.here(no);
    }

    /// `GLSL.std.450`, componente a componente.
    fn glsl(&mut self, ins: &Instruction, n: i32) -> Paso<()> {
        let numero = ins.op(4);
        let fila = glsl_info(numero).ok_or(Reason::UnsupportedGlsl { number: numero })?;
        let ra = self.at(ins.op(2));
        let x = self.at(ins.op(5));
        let y = if fila.operands > 1 { self.at(ins.op(6)) } else { 0 };
        let z = if fila.operands > 2 { self.at(ins.op(7)) } else { 0 };
        if fila.group == GlslGroup::Transcendental {
            // S4b: en doble, con las rutinas de `trascendentes.rs`, como `math`.
            let (sincos, exp, ln) = self.routines.ok_or(Reason::NotYet { what: "rutinas trascendentes sin emitir" })?;
            for k in 0..n {
                self.w.movss_load(0, RDI, x + 4 * k);
                self.w.cvtss2sd(0, 0);
                match numero {
                    g::Sin => self.w.call_to(sincos),
                    g::Cos => {
                        self.w.call_to(sincos);
                        self.w.sd(0x10, 0, 1);
                    }
                    g::Exp => self.w.call_to(exp),
                    g::Log => self.w.call_to(ln),
                    g::Pow => {
                        // exp(y * ln(x))
                        self.w.call_to(ln);
                        self.w.movss_load(1, RDI, y + 4 * k);
                        self.w.cvtss2sd(1, 1);
                        self.w.sd(0x59, 1, 0);
                        self.w.sd(0x10, 0, 1);
                        self.w.call_to(exp);
                    }
                    _ => return Err(Reason::UnsupportedGlsl { number: numero }),
                }
                self.w.cvtsd2ss(0, 0);
                self.w.movss_store(RDI, ra + 4 * k, 0);
            }
            return Ok(());
        }
        for k in 0..n {
            let (xk, yk, zk, rk) = (x + 4 * k, y + 4 * k, z + 4 * k, ra + 4 * k);
            if fila.group == GlslGroup::Int {
                self.glsl_int(numero, xk, yk, zk);
                self.w.store(false, RDI, rk, RAX);
                continue;
            }
            self.w.movss_load(0, RDI, xk);
            match numero {
                g::FAbs => {
                    self.w.load(false, RAX, RDI, xk);
                    self.w.alu_imm(false, Alu::And, RAX, 0x7FFF_FFFF);
                    self.w.movd_to_xmm(false, 0, RAX);
                }
                g::Floor => {
                    self.floor_x(0, 1);
                    self.w.ss(0x10, 0, 1);
                }
                g::Ceil => {
                    self.ceil_x(0, 1);
                    self.w.ss(0x10, 0, 1);
                }
                g::Fract => {
                    self.floor_x(0, 1);
                    self.w.ss(0x5C, 0, 1);
                }
                g::Sqrt => self.w.ss(0x51, 0, 0),
                g::InverseSqrt => {
                    self.w.ss(0x51, 1, 0);
                    self.w.mov_imm(RAX, ONE);
                    self.w.movd_to_xmm(false, 0, RAX);
                    self.w.ss(0x5E, 0, 1);
                }
                g::FMin | g::FMax => {
                    self.w.movss_load(1, RDI, yk);
                    self.w.ss(if numero == g::FMin { 0x5D } else { 0x5F }, 0, 1);
                }
                g::FClamp => {
                    self.w.movss_load(1, RDI, yk);
                    self.w.ss(0x5F, 0, 1);
                    self.w.movss_load(1, RDI, zk);
                    self.w.ss(0x5D, 0, 1);
                }
                g::FMix => {
                    // x * (1 - a) + y * a
                    self.w.movss_load(2, RDI, zk);
                    self.w.mov_imm(RAX, ONE);
                    self.w.movd_to_xmm(false, 3, RAX);
                    self.w.ss(0x5C, 3, 2);
                    self.w.ss(0x59, 0, 3);
                    self.w.movss_load(1, RDI, yk);
                    self.w.ss(0x59, 1, 2);
                    self.w.ss(0x58, 0, 1);
                }
                g::Step => {
                    // step(edge, x) = x < edge ? 0 : 1; aqui xmm0 = edge.
                    self.w.movss_load(1, RDI, yk);
                    self.w.ucomiss(0, 1);
                    self.w.setcc_eax(cc::A);
                    self.w.alu_imm(false, Alu::Xor, RAX, 1);
                    self.w.imul_imm64(RAX, RAX, ONE);
                    self.w.movd_to_xmm(false, 0, RAX);
                }
                g::Fma => {
                    self.w.movss_load(1, RDI, yk);
                    self.w.movss_load(2, RDI, zk);
                    self.fma();
                }
                _ => return Err(Reason::UnsupportedGlsl { number: numero }),
            }
            self.w.movss_store(RDI, rk, 0);
        }
        Ok(())
    }

    fn glsl_int(&mut self, numero: u32, x: i32, y: i32, z: i32) {
        let w = &mut self.w;
        w.load(false, RAX, RDI, x);
        match numero {
            g::SAbs => {
                w.mov(false, RCX, RAX);
                w.shift_imm(false, 7, RCX, 31);
                w.alu(false, Alu::Xor, RAX, RCX);
                w.alu(false, Alu::Sub, RAX, RCX);
            }
            g::UMin | g::SMin | g::UMax | g::SMax => {
                w.load(false, RCX, RDI, y);
                let quedo = match numero {
                    g::UMin => cc::BE,
                    g::SMin => cc::LE,
                    g::UMax => cc::AE,
                    _ => cc::GE,
                };
                self.min_max(quedo);
            }
            _ => {
                // UClamp / SClamp: x.max(lo).min(hi)
                let con_signo = numero == g::SClamp;
                w.load(false, RCX, RDI, y);
                self.min_max(if con_signo { cc::GE } else { cc::AE });
                self.w.load(false, RCX, RDI, z);
                self.min_max(if con_signo { cc::LE } else { cc::BE });
            }
        }
    }

    /// `eax` se queda si `cmp eax, ecx` cumple `quedo`; si no, `eax = ecx`.
    fn min_max(&mut self, quedo: u8) {
        self.w.alu(false, Alu::Cmp, RAX, RCX);
        let si = self.w.jcc_fwd(quedo);
        self.w.mov(false, RAX, RCX);
        self.w.here(si);
    }

    /// `xmm0 = fma(xmm0, xmm1, xmm2)` como `math::fma`: en doble, con
    /// "redondeo a impar". Usa xmm3..xmm7, rax, rcx, rdx.
    fn fma(&mut self) {
        let w = &mut self.w;
        w.cvtss2sd(0, 0);
        w.cvtss2sd(1, 1);
        w.cvtss2sd(2, 2);
        w.sd(0x59, 0, 1); // p = a * b (exacto)
        w.sd(0x10, 3, 0);
        w.sd(0x58, 3, 2); // s = p + c
        w.movd_from_xmm(true, RAX, 3);
        w.mov(true, RDX, RAX);
        w.shift_imm(true, 5, RDX, 52);
        w.alu_imm(false, Alu::And, RDX, 0x7FF);
        w.alu_imm(false, Alu::Cmp, RDX, 0x7FF);
        let no_finito = w.jcc_fwd(cc::E);
        w.sd(0x10, 4, 3);
        w.sd(0x5C, 4, 0); // bb = s - p
        w.sd(0x10, 5, 3);
        w.sd(0x5C, 5, 4); // s - bb
        w.sd(0x10, 6, 0);
        w.sd(0x5C, 6, 5); // p - (s - bb)
        w.sd(0x10, 7, 2);
        w.sd(0x5C, 7, 4); // c - bb
        w.sd(0x58, 6, 7); // err
        w.xorpd(7, 7);
        w.comisd(6, 7);
        let exacto = w.jcc_fwd(cc::E);
        w.mov(false, RCX, RAX);
        w.alu_imm(false, Alu::And, RCX, 1);
        let impar = w.jcc_fwd(cc::NE);
        w.mov(true, RDX, RAX);
        w.comisd(6, 7);
        w.setcc_ecx(cc::A); // err > 0
        w.comisd(3, 7);
        w.setcc_eax(cc::A); // s > 0
        w.alu(false, Alu::Cmp, RAX, RCX);
        let bajar = w.jcc_fwd(cc::NE);
        w.alu_imm(true, Alu::Add, RDX, 1);
        let listo = w.jmp_fwd();
        w.here(bajar);
        w.alu_imm(true, Alu::Sub, RDX, 1);
        w.here(listo);
        w.movd_to_xmm(true, 3, RDX);
        w.here(no_finito);
        w.here(exacto);
        w.here(impar);
        w.cvtsd2ss(0, 3);
    }
}
