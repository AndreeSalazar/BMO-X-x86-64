//! **La MEMORIA**: variables de funcion, cargar, guardar, copiar, cadenas de
//! acceso y la longitud de un arreglo de tiempo de ejecucion.
//!
//! Un puntero son dos palabras en el marco, como en el oraculo: el ESPACIO (0 =
//! el marco, `k + 1` = el buffer `k`) y el DESPLAZAMIENTO (palabras en el
//! marco, BYTES en un buffer). La clase de almacenamiento del TIPO del puntero
//! dice cual de los dos es, asi que se sabe al emitir.
//!
//! En un buffer cada palabra se comprueba contra su medida antes de tocarla
//! (`OutOfBounds`, como el oraculo, en el MISMO orden: una escritura que se
//! sale a mitad deja escritas las de antes, igual en los dos).
//!
//! Registros: `rax` la entrada de la tabla, `r9` los bytes del buffer, `r10`
//! su direccion, `rcx` el desplazamiento, `rdx`/`r11` de paso.
//!
//! [consumo]  NADA   emite y se va

use super::{Emitter, Paso};
use crate::asm::{cc, Alu, R10, R11, R9, RAX, RCX, RDI, RDX, RSI};
use crate::layout::{self, array_len, dense, is_buffer, pointer, type_of, Leaf, DEC_ARRAY_STRIDE};
use crate::trap;
use bmo_spirv_front::table::op;
use bmo_spirv_front::{Instruction, Reason};

/// Cuantas hojas escalares se desenrollan en una carga o un guardado de un
/// buffer. Mas que esto es un arreglo entero de golpe: se dice.
const MAX_HOJAS: u32 = 256;

impl<'m, 'a, 'b, 't, 'c> Emitter<'m, 'a, 'b, 't, 'c> {
    pub(super) fn memory(&mut self, ins: &Instruction) -> Paso<()> {
        let m = self.m;
        match ins.opcode {
            op::OpVariable => {
                // De funcion: su inicializador, o 0. El puntero ya lo puso `init`.
                let almacen = self.at(ins.op(2)) + 8;
                let n = dense(m, pointer(m, ins.op(1)).1);
                if ins.words > 4 {
                    let src = self.at(ins.op(4));
                    self.copy(src, almacen, n);
                } else {
                    self.zero(almacen, n);
                }
            }
            op::OpLoad => {
                let p = ins.op(3);
                let (class, t) = pointer(m, type_of(m, p));
                let dst = self.at(ins.op(2));
                if is_buffer(class) {
                    self.buffer_base(p);
                    self.buffer_leaves(t, |e, h| {
                        e.w.load(false, RAX, R11, h.byte as i32);
                        e.w.store(false, RDI, dst + 4 * h.word as i32, RAX);
                    })?;
                } else {
                    self.frame_pointer(p);
                    let n = dense(m, t);
                    self.copy_via(RCX, 0, RDI, dst, n);
                }
            }
            op::OpStore => {
                let p = ins.op(1);
                let v = ins.op(2);
                let (class, t) = pointer(m, type_of(m, p));
                let src = self.at(v);
                if is_buffer(class) {
                    self.buffer_base(p);
                    self.buffer_leaves(t, |e, h| {
                        e.w.load(false, RAX, RDI, src + 4 * h.word as i32);
                        e.w.store(false, R11, h.byte as i32, RAX);
                    })?;
                } else {
                    self.frame_pointer(p);
                    let n = dense(m, t);
                    self.copy_via(RDI, src, RCX, 0, n);
                }
            }
            op::OpCopyMemory => {
                let (dc, t) = pointer(m, type_of(m, ins.op(1)));
                let (sc, _) = pointer(m, type_of(m, ins.op(2)));
                if is_buffer(dc) || is_buffer(sc) {
                    return Err(Reason::NotYet { what: "OpCopyMemory con un buffer, en el emisor" });
                }
                let n = dense(m, t);
                self.frame_pointer(ins.op(2));
                self.w.mov(true, RDX, RCX);
                self.frame_pointer(ins.op(1));
                self.copy_via(RDX, 0, RCX, 0, n);
            }
            op::OpAccessChain | op::OpInBoundsAccessChain => self.access_chain(ins)?,
            op::OpArrayLength => self.array_length(ins)?,
            _ => return Err(Reason::NotYet { what: "instruccion de memoria que el emisor aun no traduce" }),
        }
        Ok(())
    }

    /// `rcx` = la direccion de lo que apunta un puntero al MARCO.
    fn frame_pointer(&mut self, p: u32) {
        let a = self.at(p);
        self.w.load(false, RCX, RDI, a + 4);
        self.w.shift_imm(false, 4, RCX, 2);
        self.w.alu(true, Alu::Add, RCX, RDI);
    }

    /// Copia `n` palabras de `[from_base + from]` a `[to_base + to]`. Las bases
    /// no pueden ser `rax` (se usa de paso) ni `r9`/`r10` (bucle).
    fn copy_via(&mut self, from_base: u8, from: i32, to_base: u8, to: i32, n: u32) {
        if n <= 64 {
            for k in 0..n as i32 {
                self.w.load(false, RAX, from_base, from + 4 * k);
                self.w.store(false, to_base, to + 4 * k, RAX);
            }
            return;
        }
        self.w.mov(true, R9, from_base);
        self.w.alu_imm(true, Alu::Add, R9, from as u32);
        self.w.mov(true, R10, to_base);
        self.w.alu_imm(true, Alu::Add, R10, to as u32);
        self.w.mov_imm(R11, n);
        let top = self.w.pos;
        self.w.load(false, RAX, R9, 0);
        self.w.store(false, R10, 0, RAX);
        self.w.alu_imm(true, Alu::Add, R9, 4);
        self.w.alu_imm(true, Alu::Add, R10, 4);
        self.w.alu_imm(false, Alu::Sub, R11, 1);
        self.w.jcc_to(cc::NE, top);
    }

    /// Para un puntero a un BUFFER: `r9` = sus bytes, `r10` = su direccion,
    /// `rcx` = el desplazamiento (alineado a 4, o para).
    fn buffer_base(&mut self, p: u32) {
        let a = self.at(p);
        self.w.load(false, RAX, RDI, a);
        self.w.load(false, RCX, RDI, a + 4);
        self.w.alu_imm(false, Alu::Sub, RAX, 1);
        self.w.shift_imm(false, 4, RAX, 4);
        self.w.alu(true, Alu::Add, RAX, RSI);
        self.w.load(true, R9, RAX, 8);
        self.w.load(true, R10, RAX, 0);
        self.w.mov(false, RDX, RCX);
        self.w.alu_imm(false, Alu::And, RDX, 3);
        self.trap_if(cc::NE, trap::OUT_OF_BOUNDS);
    }

    /// Por cada hoja escalar de `t` en el buffer: comprueba que cabe, deja en
    /// `r11` la direccion del valor, y llama a `f`.
    fn buffer_leaves(&mut self, t: u32, mut f: impl FnMut(&mut Self, Leaf)) -> Paso<()> {
        let m = self.m;
        let mut hojas = [Leaf { byte: 0, word: 0 }; MAX_HOJAS as usize];
        let mut n = 0usize;
        let mut word = 0u32;
        let mut limite = MAX_HOJAS;
        layout::leaves(m, t, 0, &mut word, &mut limite, &mut |h| {
            hojas[n] = h;
            n += 1;
        })
        .map_err(|id| {
            if id == u32::MAX {
                Reason::NotYet { what: "cargar o guardar un arreglo enorme de un buffer de golpe" }
            } else {
                Reason::NoLayout { id }
            }
        })?;
        for h in &hojas[..n] {
            if h.byte % 4 != 0 {
                self.trap_now(trap::OUT_OF_BOUNDS);
                continue;
            }
            // rcx + byte + 4 <= bytes del buffer
            self.w.mov(false, RDX, RCX);
            self.w.alu_imm(true, Alu::Add, RDX, h.byte + 4);
            self.w.alu(true, Alu::Cmp, RDX, R9);
            self.trap_if(cc::A, trap::OUT_OF_BOUNDS);
            self.w.mov(true, R11, R10);
            self.w.alu(true, Alu::Add, R11, RCX);
            f(self, *h);
        }
        Ok(())
    }

    fn access_chain(&mut self, ins: &Instruction) -> Paso<()> {
        let m = self.m;
        let base = ins.op(3);
        let (class, mut t) = pointer(m, type_of(m, base));
        let buffer = is_buffer(class);
        let a = self.at(base);
        let ra = self.at(ins.op(2));
        // rcx acumula el desplazamiento en 64 bits; el espacio se copia al final.
        self.w.load(false, RCX, RDI, a + 4);
        for k in 4..ins.words as usize {
            let idx = ins.op(k);
            let d = m.def(t).ok_or(Reason::OutOfBounds)?;
            match d.opcode {
                op::OpTypeVector | op::OpTypeArray | op::OpTypeRuntimeArray => {
                    let elem = d.op(2);
                    let at_idx = self.at(idx);
                    self.w.load(false, RDX, RDI, at_idx);
                    if d.opcode != op::OpTypeRuntimeArray {
                        let largo = if d.opcode == op::OpTypeVector { d.op(3) } else { array_len(m, t) };
                        self.w.alu_imm(false, Alu::Cmp, RDX, largo);
                        self.trap_if(cc::AE, trap::OUT_OF_BOUNDS);
                    }
                    let paso = if !buffer {
                        dense(m, elem)
                    } else if d.opcode == op::OpTypeVector {
                        4
                    } else {
                        layout::decoration(m, t, DEC_ARRAY_STRIDE).ok_or(Reason::NoLayout { id: t })?
                    };
                    self.w.imul_imm64(RDX, RDX, paso);
                    self.w.alu(true, Alu::Add, RCX, RDX);
                    t = elem;
                }
                op::OpTypeStruct => {
                    let v = m.def(idx).map(|c| c.op(3)).unwrap_or(0);
                    let off = if buffer {
                        layout::member_offset(m, t, v).ok_or(Reason::NoLayout { id: t })?
                    } else {
                        layout::dense_step(m, t, v).ok_or(Reason::OutOfBounds)?.0
                    };
                    self.w.alu_imm(true, Alu::Add, RCX, off);
                    t = d.op(2 + v as usize);
                }
                _ => return Err(Reason::OutOfBounds),
            }
        }
        if buffer {
            // El desplazamiento de un buffer son 32 bits: lo que no cabe, para.
            self.w.mov(true, RDX, RCX);
            self.w.shift_imm(true, 5, RDX, 32);
            self.trap_if(cc::NE, trap::OUT_OF_BOUNDS);
        }
        self.w.load(false, RAX, RDI, a);
        self.w.store(false, RDI, ra, RAX);
        self.w.store(false, RDI, ra + 4, RCX);
        Ok(())
    }

    fn array_length(&mut self, ins: &Instruction) -> Paso<()> {
        let m = self.m;
        let p = ins.op(3);
        let (class, s) = pointer(m, type_of(m, p));
        if !is_buffer(class) {
            return Err(Reason::NotYet { what: "OpArrayLength fuera de un buffer" });
        }
        let miembro = ins.op(4);
        let mo = layout::member_offset(m, s, miembro).ok_or(Reason::NoLayout { id: s })?;
        let rt = m.def(s).map(|d| d.op(2 + miembro as usize)).unwrap_or(0);
        let stride = layout::decoration(m, rt, DEC_ARRAY_STRIDE).ok_or(Reason::NoLayout { id: rt })?.max(1);
        let ra = self.at(ins.op(2));
        self.buffer_base(p);
        // (bytes - off - mo) / stride, sin bajar de cero.
        self.w.mov(true, RAX, R9);
        self.w.alu(true, Alu::Sub, RAX, RCX);
        let cero1 = self.w.jcc_fwd(cc::B);
        self.w.alu_imm(true, Alu::Sub, RAX, mo);
        let cero2 = self.w.jcc_fwd(cc::B);
        self.w.alu(false, Alu::Xor, RDX, RDX);
        self.w.mov_imm(R10, stride);
        self.w.f7(true, 6, R10);
        let fin = self.w.jmp_fwd();
        self.w.here(cero1);
        self.w.here(cero2);
        self.w.mov_imm(RAX, 0);
        self.w.here(fin);
        self.w.store(false, RDI, ra, RAX);
        Ok(())
    }
}
