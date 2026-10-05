//! # E1 -- el programa que lee de fuera, CORRIENDO en la maquina
//!
//! `docs/plan/PLAN_LA_ENTRADA.md`, R3; la escalera es la de
//! `docs/maestro/TITAN_MAESTRO.md` 7.3. Un programa con `lee()` no se puede
//! correr al compilar -- lo que se teclee no se sabe todavia --, asi que el
//! calculo lo deja con todos sus bloques (`calc.rs`) y aqui se EMITE de verdad:
//!
//! ```text
//!    cada fn         un marco en `rbp`: cada local, su sitio
//!      int, si/no    8 bytes
//!      texto         su largo (8 bytes) y su bufer de 128: una linea de
//!                    consola, lo que `lee()` trae como mucho
//!    cada bloque     sus operaciones; un `if` es un `jz`, un bucle un salto
//!                    hacia arriba, una llamada un `call`
//!    print           cada parte por su puerta: un texto escrito va como
//!                    inmediato (`write_const`), uno tecleado por su bufer
//!                    (`write_buffer`), un `int` por `fmt::write_i64`
//!    lee()           `console::read_line` al bufer del local
//! ```
//!
//! ** LO QUE SOLO FALLA AL CORRER, ATRAPA (D4 del plan, la regla 1 de INTI):
//! una suma que no cabe en 64 bits (`T0060`), un `/` o `%` por cero
//! (`T0061`), una division que no es exacta (`T0062`). Al compilar el calculo
//! lo habria dicho; con un valor tecleado se dice AL CORRER, con su linea del
//! `.titan`, y el programa sale. Nunca da la vuelta en silencio.
//!
//! ** Las piezas son de la forja (`bmo_lower`): la puerta de la consola, el
//! formateo y la copia de bytes son las mismas que usan C y COBOL. Esto solo
//! decide QUE se emite para cada cosa de TITAN++ (D2 del plan: el emisor
//! propio; el frontend sigue sin nombrar una maquina).
//!
//! [!] Lo que E1 todavia NO emite lo dice al compilar, con el escalon donde
//! llega -- nunca un `.bex` que haga otra cosa: `dec` al correr y unir textos
//! tecleados (R6), llamadas con valores, tablas, registros, casos (R7).

use crate::Emitted;
use bmo_lower::x86::{self, Jump, RAX, RCX, RDI, RDX, RSI, R8, R9};
use bmo_lower::{console, fmt, memoria, task};
use bmo_titan_front::ir::{End, Function, Module, Op, Value};

const RBP: u8 = 5;
/// Lo mas largo que un texto guarda al correr: una linea de consola.
pub const TEXT_MAX: usize = 127;
/// El sitio de un texto: su largo y su bufer (128, el tope de una linea).
const TEXT_SLOT: i32 = 8 + 128;

/// La clase de un local al correr. El frontend ya la juzgo: aqui solo se
/// lee de la forma del valor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum K {
    Int,
    Bool,
    Text,
}

/// Lo que E1 no sabe todavia: un NO al compilar, con su escalon.
fn later(what: &str, rung: &str, at: (usize, usize)) -> String {
    format!("linea {}: {} al correr llega en {} de PLAN_LA_ENTRADA; hoy solo con valores que se saben al compilar", at.0, what, rung)
}

fn kind(v: &Value, kinds: &[Option<K>]) -> Result<Option<K>, String> {
    Ok(Some(match v {
        Value::Int(..) => K::Int,
        Value::Bool(..) => K::Bool,
        Value::Text(..) | Value::Read(..) => K::Text,
        Value::Local(l, _) => return Ok(kinds.get(*l).copied().flatten()),
        Value::Neg(..) => K::Int,
        Value::Not(..) => K::Bool,
        Value::Bin(op, a, _, at) => match *op {
            "==" | "!=" | "<" | "<=" | ">" | ">=" | "and" | "or" => K::Bool,
            _ => match kind(a, kinds)? {
                Some(K::Text) => return Err(later("unir textos", "R6", *at)),
                Some(k) => k,
                None => return Ok(None),
            },
        },
        Value::Dec(_, _, at) | Value::Round(_, _, at) => return Err(later("un `dec`", "R6", *at)),
        Value::Call(_, _, at) => return Err(later("una llamada que devuelve un valor", "R7", *at)),
        other => return Err(later("esto", "R7", other.at())),
    }))
}

/// La clase de cada local de `f`: de los `let`, hasta que no cambia nada.
fn kinds_of(f: &Function) -> Result<Vec<Option<K>>, String> {
    let mut kinds = vec![None; f.locals.len()];
    loop {
        let mut changed = false;
        for b in &f.blocks {
            for op in &b.ops {
                if let Op::Let { local, value, .. } | Op::Set { local, value, .. } = op {
                    if kinds[*local].is_none() {
                        if let Some(k) = kind(value, &kinds)? {
                            kinds[*local] = Some(k);
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            return Ok(kinds);
        }
    }
}

/// Donde vive cada local, debajo de `rbp`.
struct Frame {
    kinds: Vec<Option<K>>,
    /// El desplazamiento (negativo) del primer byte del sitio de cada local.
    at: Vec<i32>,
    /// Dos bufers de paso: un texto escrito que se compara, o un `lee()`
    /// dentro de un `print`.
    scratch: [i32; 2],
    size: i32,
}

impl Frame {
    fn of(f: &Function) -> Result<Frame, String> {
        let kinds = kinds_of(f)?;
        let mut off = 0;
        let mut at = Vec::with_capacity(kinds.len());
        for k in &kinds {
            off += if *k == Some(K::Text) { TEXT_SLOT } else { 8 };
            at.push(-off);
        }
        let s0 = -(off + TEXT_SLOT);
        let s1 = -(off + 2 * TEXT_SLOT);
        let size = (off + 2 * TEXT_SLOT + 15) / 16 * 16;
        Ok(Frame { kinds, at, scratch: [s0, s1], size })
    }
}

/// `lea <reg>, [rbp + disp32]`: la direccion de un sitio del marco.
fn lea(code: &mut Vec<u8>, reg: u8, disp: i32) {
    code.push(0x48 | if reg >= 8 { 0x04 } else { 0 });
    code.push(0x8D);
    code.push(0x80 | ((reg & 7) << 3) | RBP);
    code.extend_from_slice(&disp.to_le_bytes());
}

/// Un NO al correr, por escribir: donde salta, y lo que dice.
struct Trap {
    field: usize,
    text: String,
}

struct E1<'m> {
    m: &'m Module,
    code: Vec<u8>,
    /// (campo rel32, funcion destino): se resuelven al final.
    calls: Vec<(usize, usize)>,
    traps: Vec<Trap>,
}

impl E1<'_> {
    /// "src/main.titan, linea 4" o "linea 4": el sitio en el `.titan`.
    fn place(&self, at: (usize, usize)) -> String {
        match self.m.sources.place(at.0) {
            Some((k, line)) if self.m.sources.0.len() > 1 => format!("{}, linea {}", self.m.sources.path(k), line),
            Some((_, line)) => format!("linea {}", line),
            None => format!("linea {}", at.0),
        }
    }

    /// Salta al NO si la condicion `cc` (byte de `jcc` corto) NO se da: se
    /// emite el salto corto contrario por encima de un `jmp` largo al NO.
    fn trap_unless(&mut self, skip_cc: u8, code_id: &str, why: &str, at: (usize, usize)) {
        let over = x86::salto_corto(&mut self.code, skip_cc);
        let field = x86::emit_jump(&mut self.code, Jump::Always);
        x86::cierra_salto_corto(&mut self.code, over);
        let text = format!("NO {} al correr, {}: {}\n", code_id, self.place(at), why);
        self.traps.push(Trap { field, text });
    }

    fn overflow(&mut self, at: (usize, usize)) {
        // jno: sin desbordar, sigue
        self.trap_unless(0x71, "T0060", "el resultado no cabe en 64 bits", at);
    }

    /// Un `int` o un si/no, a `rax`.
    fn int(&mut self, v: &Value, fr: &Frame) -> Result<(), String> {
        match v {
            Value::Int(n, _) => x86::mov_r64_imm64(&mut self.code, RAX, *n as u64),
            Value::Bool(b, _) => x86::mov_r32_imm32(&mut self.code, RAX, *b as u32),
            Value::Local(l, _) => x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RBP, fr.at[*l]),
            Value::Neg(a, at) => {
                self.int(a, fr)?;
                x86::neg_r64(&mut self.code, RAX);
                self.overflow(*at);
            }
            Value::Not(a, _) => {
                self.int(a, fr)?;
                x86::mov_r32_imm32(&mut self.code, RCX, 1);
                x86::xor_r64_r64(&mut self.code, RAX, RCX);
            }
            Value::Bin(op @ ("and" | "or"), a, b, _) => {
                // Corto: el lado que no hace falta no se calcula.
                self.int(a, fr)?;
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                let done = x86::emit_jump(&mut self.code, if *op == "and" { Jump::IfZero } else { Jump::IfNotZero });
                self.int(b, fr)?;
                x86::patch_jump(&mut self.code, done);
            }
            Value::Bin(op @ ("==" | "!="), a, b, _) if kind(a, &fr.kinds)? == Some(K::Text) => {
                self.texts_equal(a, b, fr)?;
                if *op == "!=" {
                    x86::mov_r32_imm32(&mut self.code, RCX, 1);
                    x86::xor_r64_r64(&mut self.code, RAX, RCX);
                }
            }
            Value::Bin(op, a, b, at) => {
                self.int(a, fr)?;
                x86::push_r64(&mut self.code, RAX);
                self.int(b, fr)?;
                x86::mov_r64_r64(&mut self.code, RCX, RAX);
                x86::pop_r64(&mut self.code, RAX);
                match *op {
                    "+" => {
                        x86::add_r64_r64(&mut self.code, RAX, RCX);
                        self.overflow(*at);
                    }
                    "-" => {
                        x86::sub_r64_r64(&mut self.code, RAX, RCX);
                        self.overflow(*at);
                    }
                    "*" => {
                        x86::imul_r64_r64(&mut self.code, RAX, RCX);
                        self.overflow(*at);
                    }
                    "/" | "%" => self.divide(op, *at),
                    cmp => {
                        let cc = match cmp {
                            "==" => 0x94,
                            "!=" => 0x95,
                            "<" => 0x9C,
                            ">=" => 0x9D,
                            "<=" => 0x9E,
                            ">" => 0x9F,
                            other => return Err(format!("linea {}: `{}` no es una operacion de E1", at.0, other)),
                        };
                        let c = &mut self.code;
                        x86::cmp_r64_r64(c, RAX, RCX);
                        x86::setcc_low(c, cc, RAX);
                        x86::movzx_r64_low(c, RAX, RAX);
                    }
                }
            }
            other => return Err(later("esto", "R7", other.at())),
        }
        Ok(())
    }

    /// `rax / rcx` o `rax % rcx`: por cero atrapa (T0061), y `/` que no es
    /// exacta tambien (T0062): 7 / 2 entre enteros NO es 3.
    fn divide(&mut self, op: &str, at: (usize, usize)) {
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        self.trap_unless(0x75, "T0061", "una division por cero", at);
        // MIN / -1 no cabe: -1 se hace a mano (el resto es 0, el cociente -x).
        x86::cmp_r64_imm8(&mut self.code, RCX, -1);
        let not_minus_one = x86::emit_jump(&mut self.code, Jump::IfNotZero);
        if op == "/" {
            x86::neg_r64(&mut self.code, RAX);
            self.overflow(at);
        } else {
            x86::zero_r32(&mut self.code, RAX);
        }
        let done = x86::emit_jump(&mut self.code, Jump::Always);
        x86::patch_jump(&mut self.code, not_minus_one);
        x86::cqo(&mut self.code);
        x86::idiv_r64(&mut self.code, RCX);
        if op == "/" {
            x86::test_r64_r64(&mut self.code, RDX, RDX);
            self.trap_unless(0x74, "T0062", "una division que no es exacta (entre enteros, 7 / 2 no es 3)", at);
        } else {
            x86::mov_r64_r64(&mut self.code, RAX, RDX);
        }
        x86::patch_jump(&mut self.code, done);
    }

    /// Un texto escrito, a un bufer del marco: de ocho en ocho bytes, como
    /// inmediatos (sin datos aparte, sin reubicaciones). Deja el largo en su
    /// sitio.
    fn put_literal(&mut self, t: &str, slot: i32, at: (usize, usize)) -> Result<(), String> {
        let bytes = t.as_bytes();
        if bytes.len() > 128 {
            return Err(format!("linea {}: un texto de {} bytes no cabe en el sitio de un texto al correr (128)", at.0, bytes.len()));
        }
        for (i, chunk) in bytes.chunks(8).enumerate() {
            let mut w = [0u8; 8];
            w[..chunk.len()].copy_from_slice(chunk);
            x86::mov_r64_imm64(&mut self.code, RAX, u64::from_le_bytes(w));
            x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, slot + 8 + i as i32 * 8, RAX);
        }
        x86::mov_r64_imm64(&mut self.code, RAX, bytes.len() as u64);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, slot, RAX);
        Ok(())
    }

    /// `lee()` al sitio `slot`: una linea de la consola, sin su salto.
    fn read_into(&mut self, slot: i32) {
        lea(&mut self.code, R8, slot + 8);
        console::read_line(&mut self.code, TEXT_MAX as u8);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, slot, R9);
    }

    /// Un texto a `r8` (sus bytes) y `r9` (su largo). `spare`: que bufer de
    /// paso usar si hace falta uno.
    fn text(&mut self, v: &Value, fr: &Frame, spare: usize) -> Result<(), String> {
        let slot = match v {
            Value::Local(l, _) => fr.at[*l],
            Value::Text(t, at) => {
                let s = fr.scratch[spare];
                self.put_literal(t, s, *at)?;
                s
            }
            Value::Read(_) => {
                let s = fr.scratch[spare];
                self.read_into(s);
                s
            }
            other => return Err(later("este texto", "R6", other.at())),
        };
        lea(&mut self.code, R8, slot + 8);
        x86::mov_r64_at_reg_disp32(&mut self.code, R9, RBP, slot);
        Ok(())
    }

    /// `a == b` entre textos, a `rax` (1 o 0): mismo largo y mismos bytes.
    fn texts_equal(&mut self, a: &Value, b: &Value, fr: &Frame) -> Result<(), String> {
        self.text(a, fr, 0)?;
        x86::push_r64(&mut self.code, R8);
        x86::push_r64(&mut self.code, R9);
        self.text(b, fr, 1)?;
        x86::pop_r64(&mut self.code, RDX);
        x86::pop_r64(&mut self.code, RDI);
        let c = &mut self.code;
        // largos distintos: no son iguales
        x86::cmp_r64_r64(c, RDX, R9);
        let differ = x86::emit_jump(c, Jump::IfNotZero);
        x86::mov_r64_r64(c, RSI, R8);
        memoria::comparar_n(c, false);
        x86::test_r64_r64(c, RAX, RAX);
        x86::setcc_low(c, 0x94, RAX);
        x86::movzx_r64_low(c, RAX, RAX);
        let done = x86::emit_jump(c, Jump::Always);
        x86::patch_jump(c, differ);
        x86::zero_r32(c, RAX);
        x86::patch_jump(c, done);
        Ok(())
    }

    /// Un local recibe un valor: `let` o `x = ...`.
    fn assign(&mut self, local: usize, value: &Value, fr: &Frame) -> Result<(), String> {
        match fr.kinds[local] {
            Some(K::Text) => {
                let slot = fr.at[local];
                match value {
                    Value::Text(t, at) => self.put_literal(t, slot, *at)?,
                    Value::Read(_) => self.read_into(slot),
                    Value::Local(src, _) if *src == local => {}
                    Value::Local(src, _) => {
                        let from = fr.at[*src];
                        let c = &mut self.code;
                        lea(c, RDI, slot + 8);
                        lea(c, RSI, from + 8);
                        x86::mov_r64_at_reg_disp32(c, RCX, RBP, from);
                        x86::mov_at_reg_disp32_from_r64(c, RBP, slot, RCX);
                        memoria::copiar(c);
                    }
                    other => return Err(later("este texto", "R6", other.at())),
                }
            }
            Some(_) => {
                self.int(value, fr)?;
                x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, fr.at[local], RAX);
            }
            None => return Err(format!("linea {}: no se de que clase es este valor al correr", value.at().0)),
        }
        Ok(())
    }

    /// `print(...)`: primero se CALCULA cada parte que se calcula (a la pila,
    /// en orden), y solo entonces se escribe -- si algo atrapa, no queda media
    /// linea escrita delante del NO. Luego cada parte por su camino, y el
    /// salto de linea al final.
    fn write(&mut self, parts: &[Value], fr: &Frame) -> Result<(), String> {
        let mut kinds = Vec::with_capacity(parts.len());
        for p in parts {
            if let Value::Read(at) = p {
                return Err(format!("linea {}: `lee()` dentro de un print: guardalo antes en un `let`, y escribe ese nombre", at.0));
            }
            kinds.push(kind(p, &fr.kinds)?.ok_or_else(|| format!("linea {}: no se de que clase es esta parte al correr", p.at().0))?);
        }
        let held: Vec<usize> = (0..parts.len()).filter(|&i| kinds[i] != K::Text).collect();
        if held.len() > 15 {
            return Err(format!("linea {}: un print con mas de 15 numeros o si/no al correr", parts[0].at().0));
        }
        for &i in &held {
            self.int(&parts[i], fr)?;
            x86::push_r64(&mut self.code, RAX);
        }
        for (i, p) in parts.iter().enumerate() {
            match (p, kinds[i]) {
                (Value::Text(t, _), _) => console::write_const(&mut self.code, t.as_bytes()),
                (_, K::Text) => {
                    self.text(p, fr, 0)?;
                    console::write_buffer(&mut self.code);
                }
                (_, k) => {
                    // su valor, ya calculado: el primero, el mas hondo
                    let j = held.iter().position(|&h| h == i).unwrap_or(0);
                    let depth = ((held.len() - 1 - j) * 8) as i32;
                    x86::mov_r64_at_reg_disp32(&mut self.code, RAX, x86::RSP, depth);
                    if k == K::Int {
                        fmt::write_i64(&mut self.code);
                    } else {
                        x86::test_r64_r64(&mut self.code, RAX, RAX);
                        let no = x86::emit_jump(&mut self.code, Jump::IfZero);
                        console::write_const(&mut self.code, b"true");
                        let done = x86::emit_jump(&mut self.code, Jump::Always);
                        x86::patch_jump(&mut self.code, no);
                        console::write_const(&mut self.code, b"false");
                        x86::patch_jump(&mut self.code, done);
                    }
                }
            }
        }
        if !held.is_empty() {
            x86::add_r64_imm8(&mut self.code, x86::RSP, (held.len() * 8) as i8);
        }
        console::write_const(&mut self.code, b"\n");
        Ok(())
    }

    fn function(&mut self, f: &Function) -> Result<(), String> {
        if !f.params.is_empty() {
            return Err(later(&format!("`fn {}` con valores", f.name), "R7", (f.line, 1)));
        }
        let fr = Frame::of(f)?;
        // el marco
        self.code.push(0x55); // push rbp
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC]); // sub rsp, imm32
        self.code.extend_from_slice(&fr.size.to_le_bytes());
        let mut at = vec![usize::MAX; f.blocks.len()];
        let mut jumps: Vec<(usize, usize)> = Vec::new();
        for (i, b) in f.blocks.iter().enumerate() {
            at[i] = self.code.len();
            for op in &b.ops {
                match op {
                    Op::Let { local, value, .. } | Op::Set { local, value, .. } => self.assign(*local, value, &fr)?,
                    Op::Write { parts, .. } => self.write(parts, &fr)?,
                    Op::Call { func, args, at } => {
                        if !args.is_empty() {
                            return Err(later("una llamada con valores", "R7", *at));
                        }
                        self.code.push(0xE8);
                        self.calls.push((self.code.len(), *func));
                        self.code.extend_from_slice(&[0; 4]);
                    }
                    Op::Drop { .. } => {}
                    Op::SetAt { at, .. } => return Err(later("cambiar una parte de una tabla o un registro", "R7", *at)),
                }
            }
            match &b.end {
                End::Return(None) => {
                    self.code.extend_from_slice(&[0x48, 0x89, 0xEC]); // mov rsp, rbp
                    self.code.push(0x5D); // pop rbp
                    self.code.push(0xC3); // ret
                }
                End::Return(Some(v)) => return Err(later("devolver un valor", "R7", v.at())),
                End::Jump(t) => {
                    if *t != i + 1 {
                        jumps.push((x86::emit_jump(&mut self.code, Jump::Always), *t));
                    }
                }
                End::Branch { cond, then, other, .. } => {
                    self.int(cond, &fr)?;
                    x86::test_r64_r64(&mut self.code, RAX, RAX);
                    jumps.push((x86::emit_jump(&mut self.code, Jump::IfZero), *other));
                    if *then != i + 1 {
                        jumps.push((x86::emit_jump(&mut self.code, Jump::Always), *then));
                    }
                }
            }
        }
        for (field, t) in jumps {
            x86::patch_jump_to(&mut self.code, field, at[t]);
        }
        Ok(())
    }
}

/// ** E1 entero: arrancar en `main`, cada fn con su marco, y los NO del
/// final -- lo que se escribe si algo atrapa al correr.
pub fn emit(m: &Module) -> Result<Emitted, String> {
    let mut e = E1 { m, code: Vec::new(), calls: Vec::new(), traps: Vec::new() };
    e.code.push(0xE8);
    e.calls.push((e.code.len(), m.entry));
    e.code.extend_from_slice(&[0; 4]);
    task::exit(&mut e.code);
    let mut starts = Vec::with_capacity(m.functions.len());
    for f in &m.functions {
        starts.push(e.code.len());
        if f.gpu || f.dispatch.is_some() {
            // E1 todavia no lleva la 3060 ni los traits al correr: se dice.
            return Err(later(&format!("`fn {}` (la 3060 o un trait)", f.name), "R7", (f.line, 1)));
        }
        e.function(f)?;
    }
    for (field, k) in std::mem::take(&mut e.calls) {
        let rel = starts[k] as i64 - (field as i64 + 4);
        e.code[field..field + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }
    for t in std::mem::take(&mut e.traps) {
        x86::patch_jump(&mut e.code, t.field);
        console::write_const(&mut e.code, t.text.as_bytes());
        task::exit(&mut e.code);
    }
    Ok(Emitted { code: e.code, starts })
}
