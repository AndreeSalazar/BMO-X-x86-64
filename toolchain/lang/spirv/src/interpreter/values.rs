//! **El oraculo, una instruccion de VALOR**: aritmetica, comparaciones,
//! conversiones, compuestos, memoria y `GLSL.std.450`. Lo que no es flujo de
//! control (eso vive en `run`, en `mod.rs`).
//!
//! [consumo]  NADA   solo mientras el oraculo despacha

use super::*;
use crate::table::glsl as g;
use crate::{glsl_info, math, GlslGroup};

impl<'m, 'a, 'b, 'w> Interpreter<'m, 'a, 'b, 'w> {
    // ---- una instruccion de valor ----------------------------------------------

    pub(super) fn step(&mut self, ins: &Instruction, buffers: &mut [Buffer]) -> Paso<()> {
        let m = self.m;
        let opcode = ins.opcode;
        let r = ins.op(2);
        let rn = (self.len.get(r as usize).copied().unwrap_or(0) as usize).min(4);
        let f = f32::from_bits;
        let b = |x: f32| x.to_bits();
        let bool_ = |c: bool| c as u32;
        match opcode {
            // -- enteros, componente a componente --
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
                let (x, _) = self.comps(ins.op(3))?;
                let (y, _) = self.comps(ins.op(4))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    v[k] = int_binary(opcode, x[k], y[k])?;
                }
                self.put(r, &v[..rn]);
            }
            op::OpSNegate | op::OpNot => {
                let (x, _) = self.comps(ins.op(3))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    v[k] = if opcode == op::OpNot { !x[k] } else { x[k].wrapping_neg() };
                }
                self.put(r, &v[..rn]);
            }
            // -- flotantes --
            op::OpFAdd | op::OpFSub | op::OpFMul | op::OpFDiv | op::OpFRem | op::OpFMod => {
                let (x, _) = self.comps(ins.op(3))?;
                let (y, _) = self.comps(ins.op(4))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    let (p, q) = (f(x[k]), f(y[k]));
                    v[k] = b(match opcode {
                        op::OpFAdd => p + q,
                        op::OpFSub => p - q,
                        op::OpFMul => p * q,
                        op::OpFDiv => p / q,
                        op::OpFRem => math::rem(p, q),
                        _ => math::modulo(p, q),
                    });
                }
                self.put(r, &v[..rn]);
            }
            op::OpFNegate => {
                let (x, _) = self.comps(ins.op(3))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    v[k] = b(math::neg(f(x[k])));
                }
                self.put(r, &v[..rn]);
            }
            op::OpVectorTimesScalar => {
                let (x, _) = self.comps(ins.op(3))?;
                let s = f(self.scalar(ins.op(4))?);
                let mut v = [0u32; 4];
                for k in 0..rn {
                    v[k] = b(f(x[k]) * s);
                }
                self.put(r, &v[..rn]);
            }
            op::OpDot => {
                let (x, n) = self.comps(ins.op(3))?;
                let (y, _) = self.comps(ins.op(4))?;
                // En orden: ((x0*y0 + x1*y1) + x2*y2) + x3*y3. Sin fusionar.
                let mut acc = f(x[0]) * f(y[0]);
                for k in 1..n {
                    acc = acc + f(x[k]) * f(y[k]);
                }
                self.put(r, &[b(acc)]);
            }
            // -- comparaciones --
            op::OpIEqual..=op::OpSLessThanEqual => {
                let (x, _) = self.comps(ins.op(3))?;
                let (y, _) = self.comps(ins.op(4))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    let (p, q) = (x[k], y[k]);
                    let (sp, sq) = (p as i32, q as i32);
                    v[k] = bool_(match opcode {
                        op::OpIEqual => p == q,
                        op::OpINotEqual => p != q,
                        op::OpUGreaterThan => p > q,
                        op::OpSGreaterThan => sp > sq,
                        op::OpUGreaterThanEqual => p >= q,
                        op::OpSGreaterThanEqual => sp >= sq,
                        op::OpULessThan => p < q,
                        op::OpSLessThan => sp < sq,
                        op::OpULessThanEqual => p <= q,
                        _ => sp <= sq,
                    });
                }
                self.put(r, &v[..rn]);
            }
            op::OpFOrdEqual..=op::OpFUnordGreaterThanEqual => {
                let (x, _) = self.comps(ins.op(3))?;
                let (y, _) = self.comps(ins.op(4))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    let (p, q) = (f(x[k]), f(y[k]));
                    let nan = p.is_nan() || q.is_nan();
                    v[k] = bool_(match opcode {
                        op::OpFOrdEqual => !nan && p == q,
                        op::OpFUnordEqual => nan || p == q,
                        op::OpFOrdNotEqual => !nan && p != q,
                        op::OpFUnordNotEqual => nan || p != q,
                        op::OpFOrdLessThan => !nan && p < q,
                        op::OpFUnordLessThan => nan || p < q,
                        op::OpFOrdGreaterThan => !nan && p > q,
                        op::OpFUnordGreaterThan => nan || p > q,
                        op::OpFOrdLessThanEqual => !nan && p <= q,
                        op::OpFUnordLessThanEqual => nan || p <= q,
                        op::OpFOrdGreaterThanEqual => !nan && p >= q,
                        _ => nan || p >= q,
                    });
                }
                self.put(r, &v[..rn]);
            }
            op::OpIsNan | op::OpIsInf => {
                let (x, _) = self.comps(ins.op(3))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    let p = f(x[k]);
                    v[k] = bool_(if opcode == op::OpIsNan { p.is_nan() } else { p.is_infinite() });
                }
                self.put(r, &v[..rn]);
            }
            op::OpLogicalEqual | op::OpLogicalNotEqual | op::OpLogicalOr | op::OpLogicalAnd => {
                let (x, _) = self.comps(ins.op(3))?;
                let (y, _) = self.comps(ins.op(4))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    let (p, q) = (x[k] != 0, y[k] != 0);
                    v[k] = bool_(match opcode {
                        op::OpLogicalEqual => p == q,
                        op::OpLogicalNotEqual => p != q,
                        op::OpLogicalOr => p || q,
                        _ => p && q,
                    });
                }
                self.put(r, &v[..rn]);
            }
            op::OpLogicalNot => {
                let (x, _) = self.comps(ins.op(3))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    v[k] = bool_(x[k] == 0);
                }
                self.put(r, &v[..rn]);
            }
            op::OpAny | op::OpAll => {
                let (x, n) = self.comps(ins.op(3))?;
                let c = if opcode == op::OpAny { x[..n].iter().any(|&c| c != 0) } else { x[..n].iter().all(|&c| c != 0) };
                self.put(r, &[bool_(c)]);
            }
            op::OpSelect => {
                let (c, cn) = self.comps(ins.op(3))?;
                let (x, _) = self.comps(ins.op(4))?;
                let (y, _) = self.comps(ins.op(5))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    let s = if cn == 1 { c[0] } else { c[k] };
                    v[k] = if s != 0 { x[k] } else { y[k] };
                }
                self.put(r, &v[..rn]);
            }
            // -- conversiones --
            op::OpConvertFToU | op::OpConvertFToS | op::OpConvertSToF | op::OpConvertUToF => {
                let (x, _) = self.comps(ins.op(3))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    v[k] = match opcode {
                        op::OpConvertFToU => math::to_u32(f(x[k])),
                        op::OpConvertFToS => math::to_i32(f(x[k])) as u32,
                        op::OpConvertSToF => b(x[k] as i32 as f32),
                        _ => b(x[k] as f32),
                    };
                }
                self.put(r, &v[..rn]);
            }
            // Todo es de 32 bits: estas no cambian ningun bit.
            op::OpUConvert | op::OpSConvert | op::OpFConvert | op::OpBitcast | op::OpCopyObject => {
                self.copy_value(ins.op(3), r)?;
            }
            op::OpUndef => {
                let o = self.slot[r as usize] as usize;
                let n = self.len[r as usize] as usize;
                self.arena[o..o + n].fill(0);
                self.stamp[r as usize] = self.epoch;
            }
            // -- compuestos --
            op::OpCompositeConstruct => {
                let mut d = self.slot[r as usize] as usize;
                for k in 3..ins.words as usize {
                    d += self.copy_to(ins.op(k), d)?;
                }
                self.stamp[r as usize] = self.epoch;
            }
            op::OpCompositeExtract => {
                let c = ins.op(3);
                self.ready(c)?;
                let (mut off, mut t) = (0u32, type_of(m, c));
                for k in 4..ins.words as usize {
                    let (o, s) = dense_step(m, t, ins.op(k)).ok_or(Reason::OutOfBounds)?;
                    off += o;
                    t = s;
                }
                let src = (self.slot[c as usize] + off) as usize;
                let n = self.len[r as usize] as usize;
                let d = self.slot[r as usize] as usize;
                self.arena.copy_within(src..src + n, d);
                self.stamp[r as usize] = self.epoch;
            }
            op::OpCompositeInsert => {
                let d = self.slot[r as usize] as usize;
                self.copy_to(ins.op(4), d)?;
                let (mut off, mut t) = (0u32, type_of(m, ins.op(4)));
                for k in 5..ins.words as usize {
                    let (o, s) = dense_step(m, t, ins.op(k)).ok_or(Reason::OutOfBounds)?;
                    off += o;
                    t = s;
                }
                self.copy_to(ins.op(3), d + off as usize)?;
                self.stamp[r as usize] = self.epoch;
            }
            op::OpVectorShuffle => {
                let (x, nx) = self.comps(ins.op(3))?;
                let (y, _) = self.comps(ins.op(4))?;
                let mut v = [0u32; 4];
                for k in 0..rn {
                    let i = ins.op(5 + k) as usize;
                    v[k] = if i < nx { x[i] } else if i < nx + 4 { y[i - nx] } else { 0 };
                }
                self.put(r, &v[..rn]);
            }
            op::OpVectorExtractDynamic => {
                let (x, n) = self.comps(ins.op(3))?;
                let i = self.scalar(ins.op(4))? as usize;
                if i >= n {
                    return Err(Reason::OutOfBounds);
                }
                self.put(r, &[x[i]]);
            }
            op::OpVectorInsertDynamic => {
                let (mut x, n) = self.comps(ins.op(3))?;
                let c = self.scalar(ins.op(4))?;
                let i = self.scalar(ins.op(5))? as usize;
                if i >= n {
                    return Err(Reason::OutOfBounds);
                }
                x[i] = c;
                self.put(r, &x[..n]);
            }
            // -- memoria --
            op::OpVariable => {
                // Una variable de funcion: su inicializador, o 0.
                let dir = self.arena[self.slot[r as usize] as usize + 1] as usize;
                let n = dense(m, pointer(m, ins.op(1)).1) as usize;
                if ins.words > 4 {
                    self.copy_to(ins.op(4), dir)?;
                } else {
                    self.arena[dir..dir + n].fill(0);
                }
            }
            op::OpLoad => {
                let (space, off) = self.ptr(ins.op(3))?;
                let d = self.slot[r as usize] as usize;
                let n = self.len[r as usize] as usize;
                if space == ARENA {
                    self.arena.copy_within(off as usize..off as usize + n, d);
                } else {
                    let buf = buffers.get((space - 1) as usize).ok_or(Reason::OutOfBounds)?;
                    self.load_buffer(buf, off, ins.op(1), d)?;
                }
                self.stamp[r as usize] = self.epoch;
            }
            op::OpStore => {
                let (space, off) = self.ptr(ins.op(1))?;
                let v = ins.op(2);
                self.ready(v)?;
                if space == ARENA {
                    self.copy_to(v, off as usize)?;
                } else {
                    let t = type_of(m, v);
                    let src = self.slot[v as usize] as usize;
                    let buf = buffers.get_mut((space - 1) as usize).ok_or(Reason::OutOfBounds)?;
                    self.store_buffer(buf, off, t, src)?;
                }
            }
            op::OpCopyMemory => {
                let (ds, doff) = self.ptr(ins.op(1))?;
                let (ss, soff) = self.ptr(ins.op(2))?;
                let t = pointer(m, type_of(m, ins.op(2))).1;
                let n = dense(m, t) as usize;
                match (ds == ARENA, ss == ARENA) {
                    (true, true) => self.arena.copy_within(soff as usize..soff as usize + n, doff as usize),
                    (true, false) => {
                        let buf = buffers.get((ss - 1) as usize).ok_or(Reason::OutOfBounds)?;
                        self.load_buffer(buf, soff, t, doff as usize)?;
                    }
                    (false, true) => {
                        let buf = buffers.get_mut((ds - 1) as usize).ok_or(Reason::OutOfBounds)?;
                        self.store_buffer(buf, doff, t, soff as usize)?;
                    }
                    (false, false) => return Err(Reason::NotYet { what: "OpCopyMemory entre dos buffers" }),
                }
            }
            op::OpAccessChain | op::OpInBoundsAccessChain => {
                let base = ins.op(3);
                let (space, mut off) = self.ptr(base)?;
                let mut t = pointer(m, type_of(m, base)).1;
                for k in 4..ins.words as usize {
                    let i = self.scalar(ins.op(k))?;
                    let d = def(m, t).ok_or(Reason::OutOfBounds)?;
                    if space == ARENA {
                        let (o, s) = dense_step(m, t, i).ok_or(Reason::OutOfBounds)?;
                        off += o;
                        t = s;
                    } else {
                        match d.opcode {
                            op::OpTypeVector => {
                                if i >= d.op(3) {
                                    return Err(Reason::OutOfBounds);
                                }
                                off += 4 * i;
                                t = d.op(2);
                            }
                            op::OpTypeArray | op::OpTypeRuntimeArray => {
                                if d.opcode == op::OpTypeArray {
                                    let n = def(m, d.op(3)).map(|c| c.op(3)).unwrap_or(0);
                                    if i >= n {
                                        return Err(Reason::OutOfBounds);
                                    }
                                }
                                off = off.checked_add(i.checked_mul(self.stride(t)?).ok_or(Reason::OutOfBounds)?)
                                    .ok_or(Reason::OutOfBounds)?;
                                t = d.op(2);
                            }
                            op::OpTypeStruct => {
                                off += self.member(t, i)?;
                                t = d.op(2 + i as usize);
                            }
                            _ => return Err(Reason::OutOfBounds),
                        }
                    }
                }
                self.put(r, &[space, off]);
            }
            op::OpArrayLength => {
                let (space, off) = self.ptr(ins.op(3))?;
                if space == ARENA {
                    return Err(Reason::NotYet { what: "OpArrayLength fuera de un buffer" });
                }
                let s = pointer(m, type_of(m, ins.op(3))).1;
                let mo = self.member(s, ins.op(4))?;
                let rt = def(m, s).map(|d| d.op(2 + ins.op(4) as usize)).unwrap_or(0);
                let stride = self.stride(rt)?.max(1);
                let bytes = buffers.get((space - 1) as usize).map(|b| b.data.len() as u32 * 4).unwrap_or(0);
                let n = bytes.saturating_sub(off + mo) / stride;
                self.put(r, &[n]);
            }
            // -- GLSL.std.450 --
            op::OpExtInst => {
                let numero = ins.op(4);
                let fila = glsl_info(numero).ok_or(Reason::UnsupportedGlsl { number: numero })?;
                let mut a = [[0u32; 4]; 3];
                for k in 0..fila.operands as usize {
                    a[k] = self.comps(ins.op(5 + k))?.0;
                }
                let mut v = [0u32; 4];
                for k in 0..rn {
                    v[k] = match fila.group {
                        GlslGroup::Float => b(glsl_float(numero, f(a[0][k]), f(a[1][k]), f(a[2][k]))?),
                        GlslGroup::Int => glsl_int(numero, a[0][k], a[1][k], a[2][k])?,
                        GlslGroup::Transcendental => return Err(Reason::GlslLater { number: numero }),
                    };
                }
                self.put(r, &v[..rn]);
            }
            _ => return Err(Reason::NotYet { what: "instruccion que el oraculo aun no ejecuta" }),
        }
        Ok(())
    }
}

/// Un paso por un compuesto en la ARENA (juntos, sin huecos): cuantas palabras
/// avanzar y el tipo al que se llega.
fn dense_step(m: &Module, t: u32, i: u32) -> Option<(u32, u32)> {
    let d = def(m, t)?;
    match d.opcode {
        op::OpTypeVector if i < d.op(3) => Some((i, d.op(2))),
        op::OpTypeArray => {
            let n = def(m, d.op(3))?.op(3);
            if i < n {
                Some((i * dense(m, d.op(2)), d.op(2)))
            } else {
                None
            }
        }
        op::OpTypeStruct if (i as usize) + 2 < d.words as usize => {
            let antes: u32 = (0..i as usize).map(|k| dense(m, d.op(2 + k))).sum();
            Some((antes, d.op(2 + i as usize)))
        }
        _ => None,
    }
}

fn int_binary(opcode: u16, x: u32, y: u32) -> Result<u32, Reason> {
    let (sx, sy) = (x as i32, y as i32);
    Ok(match opcode {
        op::OpIAdd => x.wrapping_add(y),
        op::OpISub => x.wrapping_sub(y),
        op::OpIMul => x.wrapping_mul(y),
        op::OpUDiv | op::OpUMod => {
            if y == 0 {
                return Err(Reason::DivisionByZero);
            }
            if opcode == op::OpUDiv {
                x / y
            } else {
                x % y
            }
        }
        op::OpSDiv | op::OpSRem | op::OpSMod => {
            if sy == 0 {
                return Err(Reason::DivisionByZero);
            }
            if sx == i32::MIN && sy == -1 {
                return Err(Reason::DivisionOverflow);
            }
            match opcode {
                op::OpSDiv => (sx / sy) as u32,
                op::OpSRem => (sx % sy) as u32,
                _ => {
                    // El signo del divisor.
                    let mut r = sx % sy;
                    if r != 0 && ((r < 0) != (sy < 0)) {
                        r += sy;
                    }
                    r as u32
                }
            }
        }
        op::OpShiftRightLogical | op::OpShiftRightArithmetic | op::OpShiftLeftLogical => {
            if y >= 32 {
                return Err(Reason::ShiftTooLarge);
            }
            match opcode {
                op::OpShiftRightLogical => x >> y,
                op::OpShiftRightArithmetic => (sx >> y) as u32,
                _ => x << y,
            }
        }
        op::OpBitwiseOr => x | y,
        op::OpBitwiseXor => x ^ y,
        _ => x & y,
    })
}

fn glsl_float(numero: u32, x: f32, y: f32, z: f32) -> Result<f32, Reason> {
    Ok(match numero {
        g::FAbs => math::abs(x),
        g::Floor => math::floor(x),
        g::Ceil => math::ceil(x),
        g::Fract => math::fract(x),
        g::Sqrt => math::sqrt(x),
        g::InverseSqrt => math::inverse_sqrt(x),
        g::FMin => math::min(x, y),
        g::FMax => math::max(x, y),
        g::FClamp => math::clamp(x, y, z),
        g::FMix => math::mix(x, y, z),
        g::Step => math::step(x, y),
        g::Fma => math::fma(x, y, z),
        _ => return Err(Reason::UnsupportedGlsl { number: numero }),
    })
}

fn glsl_int(numero: u32, x: u32, y: u32, z: u32) -> Result<u32, Reason> {
    let (sx, sy, sz) = (x as i32, y as i32, z as i32);
    Ok(match numero {
        g::SAbs => sx.wrapping_abs() as u32,
        g::UMin => x.min(y),
        g::SMin => sx.min(sy) as u32,
        g::UMax => x.max(y),
        g::SMax => sx.max(sy) as u32,
        g::UClamp => x.max(y).min(z),
        g::SClamp => sx.max(sy).min(sz) as u32,
        _ => return Err(Reason::UnsupportedGlsl { number: numero }),
    })
}
