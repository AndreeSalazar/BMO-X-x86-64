//! **EL EMISOR (S4): cada funcion del sombreador, a x86-64.**
//!
//! == La forma del codigo ==
//!
//! ```text
//!   0      la SALIDA de las trampas   (rsp <- r14, pop r14, pop r15, ret)
//!   init   una vez por despacho: constantes y punteros a variables y buffers
//!   main   una vez por invocacion: BuiltIn y privadas, `call` a la entrada
//!   ...    cada OpFunction, bloque a bloque
//! ```
//!
//! `main(rdi = marco, rsi = tabla de buffers, rdx = ids, rcx = combustible)`
//! devuelve `eax` = 0, o el codigo de una trampa con `edx` = la palabra del
//! fichero donde paro. La tabla de buffers son pares `(direccion, bytes)` de
//! 64 bits, en el orden de `Program::buffers`. Los ids son 13 palabras:
//! GlobalInvocationId, LocalInvocationId, WorkgroupId, NumWorkgroups (tres
//! cada una) y LocalInvocationIndex.
//!
//! == La forma de calcular: el MARCO, sin registros vivos ==
//!
//! Cada valor vive en su sitio fijo del marco (`[rdi + 4 * slot]`) y cada
//! instruccion carga sus operandos, calcula y guarda. Es lento y es CORRECTO:
//! la velocidad (valores en registros, carriles de S7) se mide despues contra
//! esto, y no al reves.
//!
//! == Lo que para, igual que el oraculo ==
//!
//! Division por cero, `INT_MIN / -1`, desplazar 32 o mas, salirse de un
//! buffer o de un arreglo, `OpUnreachable`, y el combustible -- que aqui se
//! gasta en cada salto HACIA ATRAS (el oraculo lo gasta por instruccion: los
//! dos paran, no en el mismo numero).
//!
//! [!] Lo que NO vigila, y lo dice: un valor que la invocacion no definio (la
//! dominancia). Eso lo caza el oraculo; aqui seria un sello por id y por
//! invocacion en cada lectura.
//!
//! [consumo]  NADA   emite y se va

mod memory;
mod values;

use crate::asm::{cc, Alu, Writer, R14, R15, R8, R9, R10, RAX, RCX, RDI, RDX, RSP};
use crate::layout::{self, dense, is_buffer, pointer, CLASS_INPUT, CLASS_PRIVATE, DEC_BINDING, DEC_BUILTIN, DEC_DESCRIPTOR_SET};
use crate::{trap, MAX_BUFFERS};
use bmo_spirv_front::table::op;
use bmo_spirv_front::{Instruction, Module, Reason};

pub(crate) type Paso<T> = Result<T, Reason>;

/// Donde cae cada BuiltIn en el bloque de ids (en bytes).
fn builtin_offset(b: u32) -> Option<i32> {
    match b {
        28 => Some(0),  // GlobalInvocationId
        27 => Some(12), // LocalInvocationId
        26 => Some(24), // WorkgroupId
        24 => Some(36), // NumWorkgroups
        29 => Some(48), // LocalInvocationIndex
        _ => None,
    }
}

pub(crate) struct Emitter<'m, 'a, 'b, 't, 'c> {
    pub m: &'m Module<'a, 'b>,
    pub slots: &'t [u32],
    pub labels: &'t mut [u32],
    pub w: Writer<'c>,
    pub trap_exit: usize,
    /// La palabra del fichero de la instruccion que se emite (para las trampas).
    pub word: usize,
    current: u32,
    function: u32,
    pub buffers: [(u32, u32); MAX_BUFFERS],
    pub n_buffers: usize,
}

impl<'m, 'a, 'b, 't, 'c> Emitter<'m, 'a, 'b, 't, 'c> {
    pub fn new(m: &'m Module<'a, 'b>, slots: &'t [u32], labels: &'t mut [u32], w: Writer<'c>) -> Self {
        Emitter { m, slots, labels, w, trap_exit: 0, word: 0, current: 0, function: 0, buffers: [(0, 0); MAX_BUFFERS], n_buffers: 0 }
    }

    /// El desplazamiento en bytes del sitio de `id` en el marco.
    pub fn at(&self, id: u32) -> i32 {
        (self.slots[id as usize] * 4) as i32
    }

    pub fn len(&self, t: u32) -> u32 {
        dense(self.m, t)
    }

    /// Salta a la trampa `code` si se cumple `c`.
    pub fn trap_if(&mut self, c: u8, code: u32) {
        let skip = self.w.jcc_fwd(c ^ 1);
        self.trap_now(code);
        self.w.here(skip);
    }

    pub fn trap_now(&mut self, code: u32) {
        self.w.mov_imm(RAX, code);
        self.w.mov_imm(RDX, self.word as u32);
        self.w.jmp_to(self.trap_exit);
    }

    /// Copia `n` palabras del marco (`from` -> `to`, en bytes).
    pub fn copy(&mut self, from: i32, to: i32, n: u32) {
        if n <= 64 {
            for k in 0..n as i32 {
                self.w.load(false, RAX, RDI, from + 4 * k);
                self.w.store(false, RDI, to + 4 * k, RAX);
            }
            return;
        }
        // Un bucle: r9 <- origen, r10 <- destino, ecx cuenta.
        self.w.mov(true, R9, RDI);
        self.w.alu_imm(true, Alu::Add, R9, from as u32);
        self.w.mov(true, R10, RDI);
        self.w.alu_imm(true, Alu::Add, R10, to as u32);
        self.w.mov_imm(RCX, n);
        let top = self.w.pos;
        self.w.load(false, RAX, R9, 0);
        self.w.store(false, R10, 0, RAX);
        self.w.alu_imm(true, Alu::Add, R9, 4);
        self.w.alu_imm(true, Alu::Add, R10, 4);
        self.w.alu_imm(false, Alu::Sub, RCX, 1);
        self.w.jcc_to(cc::NE, top);
    }

    /// Pone a cero `n` palabras del marco.
    pub fn zero(&mut self, to: i32, n: u32) {
        for k in 0..n as i32 {
            self.w.store_imm(RDI, to + 4 * k, 0);
        }
    }

    // ---- el programa entero --------------------------------------------------

    /// Emite todo. Devuelve `(init, main)`.
    pub fn program(&mut self, entry: u32) -> Result<(usize, usize), (Reason, usize)> {
        // La salida de las trampas: deshace lo que `main` apilo.
        self.trap_exit = self.w.pos;
        self.w.mov(true, RSP, R14);
        self.w.pop(R14);
        self.w.pop(R15);
        self.w.ret();

        let init = self.w.pos;
        self.init().map_err(|r| (r, self.word))?;
        self.w.ret();

        let main = self.w.pos;
        self.w.push(R15);
        self.w.push(R14);
        self.w.mov(true, R14, RSP);
        self.w.mov(true, R15, RCX);
        self.w.mov(true, R8, RDX);
        self.invocation().map_err(|r| (r, self.word))?;
        let destino = self.labels[entry as usize] as usize;
        self.w.call_to(destino);
        self.w.alu(false, Alu::Xor, RAX, RAX);
        self.w.pop(R14);
        self.w.pop(R15);
        self.w.ret();

        let m = self.m;
        for ins in m.instructions() {
            if ins.opcode == op::OpFunction {
                self.function(&ins).map_err(|r| (r, self.word))?;
            }
        }
        Ok((init, main))
    }

    /// Una vez por despacho: constantes y punteros.
    fn init(&mut self) -> Paso<()> {
        let m = self.m;
        for ins in m.instructions().take_while(|i| i.opcode != op::OpFunction) {
            self.word = ins.offset;
            let id = ins.op(2);
            match ins.opcode {
                op::OpConstant => {
                    let a = self.at(id);
                    self.w.store_imm(RDI, a, ins.op(3));
                }
                op::OpConstantTrue => {
                    let a = self.at(id);
                    self.w.store_imm(RDI, a, 1);
                }
                op::OpConstantFalse | op::OpConstantNull | op::OpUndef => {
                    let (a, n) = (self.at(id), self.len(ins.op(1)));
                    self.zero(a, n);
                }
                op::OpConstantComposite => {
                    let mut d = self.at(id);
                    for k in 3..ins.words as usize {
                        let p = ins.op(k);
                        let n = self.len(layout::type_of(m, p));
                        let src = self.at(p);
                        self.copy(src, d, n);
                        d += 4 * n as i32;
                    }
                }
                op::OpVariable => {
                    let a = self.at(id);
                    let (class, _) = pointer(m, ins.op(1));
                    if is_buffer(class) {
                        let set = layout::decoration(m, id, DEC_DESCRIPTOR_SET).unwrap_or(0);
                        let binding = layout::decoration(m, id, DEC_BINDING).unwrap_or(0);
                        let k = self.buffer_index(set, binding)?;
                        self.w.store_imm(RDI, a, k + 1);
                        self.w.store_imm(RDI, a + 4, 0);
                    } else {
                        self.w.store_imm(RDI, a, 0);
                        self.w.store_imm(RDI, a + 4, self.slots[id as usize] + 2);
                    }
                }
                _ => {}
            }
        }
        // ** Y los punteros de las variables DE FUNCION, que tambien son fijos
        // (sin recursion, cada una tiene un solo almacen). Estaban fuera: solo
        // se escribian los de las globales, y una variable local apuntaba a la
        // palabra 0 del marco -- lo cazo la prueba diferencial en el primer
        // bucle de `glslc -O0`, que guarda todo en variables locales.
        for ins in m.instructions().skip_while(|i| i.opcode != op::OpFunction) {
            if ins.opcode == op::OpVariable {
                let id = ins.op(2);
                let a = self.at(id);
                self.w.store_imm(RDI, a, 0);
                self.w.store_imm(RDI, a + 4, self.slots[id as usize] + 2);
            }
        }
        Ok(())
    }

    fn buffer_index(&mut self, set: u32, binding: u32) -> Paso<u32> {
        if let Some(k) = self.buffers[..self.n_buffers].iter().position(|&b| b == (set, binding)) {
            return Ok(k as u32);
        }
        if self.n_buffers == MAX_BUFFERS {
            return Err(Reason::TooMany { what: "buffers", limit: MAX_BUFFERS });
        }
        self.buffers[self.n_buffers] = (set, binding);
        self.n_buffers += 1;
        Ok(self.n_buffers as u32 - 1)
    }

    /// Una vez por invocacion: los BuiltIn y las variables privadas.
    fn invocation(&mut self) -> Paso<()> {
        let m = self.m;
        for ins in m.instructions().take_while(|i| i.opcode != op::OpFunction) {
            if ins.opcode != op::OpVariable {
                continue;
            }
            self.word = ins.offset;
            let id = ins.op(2);
            let (class, pointee) = pointer(m, ins.op(1));
            let almacen = self.at(id) + 8;
            let n = self.len(pointee);
            match class {
                CLASS_INPUT => {
                    let b = layout::decoration(m, id, DEC_BUILTIN).unwrap_or(u32::MAX);
                    let off = builtin_offset(b).ok_or(Reason::NotYet { what: "este BuiltIn en el emisor" })?;
                    for k in 0..n.min(3) as i32 {
                        self.w.load(false, RAX, R8, off + 4 * k);
                        self.w.store(false, RDI, almacen + 4 * k, RAX);
                    }
                }
                CLASS_PRIVATE => {
                    if ins.words > 4 {
                        let src = self.at(ins.op(4));
                        self.copy(src, almacen, n);
                    } else {
                        self.zero(almacen, n);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    // ---- una funcion ---------------------------------------------------------

    fn function(&mut self, f: &Instruction) -> Paso<()> {
        let m = self.m;
        self.labels[f.op(2) as usize] = self.w.pos as u32;
        self.function = f.op(2);
        for ins in m.instructions_from(f.offset).skip(1) {
            if ins.opcode == op::OpFunctionEnd {
                break;
            }
            self.word = ins.offset;
            self.instruction(&ins)?;
        }
        Ok(())
    }

    fn instruction(&mut self, ins: &Instruction) -> Paso<()> {
        let m = self.m;
        match ins.opcode {
            op::OpFunctionParameter
            | op::OpPhi
            | op::OpLine
            | op::OpNoLine
            | op::OpNop
            | op::OpSelectionMerge
            | op::OpLoopMerge => Ok(()),
            op::OpLabel => {
                self.labels[ins.op(1) as usize] = self.w.pos as u32;
                self.current = ins.op(1);
                Ok(())
            }
            op::OpBranch => self.edge(ins.op(1)),
            op::OpBranchConditional => {
                let c = self.at(ins.op(1));
                self.w.cmp_mem_imm(RDI, c, 0);
                let falso = self.w.jcc_fwd(cc::E);
                self.edge(ins.op(2))?;
                self.w.here(falso);
                self.edge(ins.op(3))
            }
            op::OpReturn => {
                self.w.ret();
                Ok(())
            }
            op::OpReturnValue => {
                let n = self.len(layout::type_of(m, ins.op(1)));
                let (src, dst) = (self.at(ins.op(1)), self.at(self.function));
                self.copy(src, dst, n);
                self.w.ret();
                Ok(())
            }
            op::OpUnreachable => {
                self.trap_now(trap::UNREACHABLE);
                Ok(())
            }
            op::OpFunctionCall => {
                let callee = m.def(ins.op(3)).ok_or(Reason::Undefined { id: ins.op(3) })?;
                let mut k = 4;
                for p in m.instructions_from(callee.offset).skip(1) {
                    if p.opcode != op::OpFunctionParameter {
                        break;
                    }
                    let n = self.len(p.op(1));
                    let (src, dst) = (self.at(ins.op(k)), self.at(p.op(2)));
                    self.copy(src, dst, n);
                    k += 1;
                }
                let destino = self.labels[ins.op(3) as usize] as usize;
                self.w.call_to(destino);
                let n = self.len(ins.op(1));
                if n > 0 {
                    let (src, dst) = (self.at(ins.op(3)), self.at(ins.op(2)));
                    self.copy(src, dst, n);
                }
                Ok(())
            }
            _ => self.value(ins),
        }
    }

    /// **Un borde**: las copias de los `OpPhi` del destino (a sus sombras
    /// primero, para que no se pisen entre si), el combustible si el salto va
    /// hacia atras, y el salto.
    fn edge(&mut self, to: u32) -> Paso<()> {
        let m = self.m;
        let destino = m.def(to).ok_or(Reason::NotALabel { id: to })?;
        let from = self.current;
        for fase in 0..2 {
            for p in m.instructions_from(destino.offset).skip(1) {
                match p.opcode {
                    op::OpLine | op::OpNoLine => continue,
                    op::OpPhi => {}
                    _ => break,
                }
                let n = self.len(p.op(1));
                let valor = self.at(p.op(2));
                let sombra = valor + 4 * n as i32;
                if fase == 1 {
                    self.copy(sombra, valor, n);
                    continue;
                }
                let mut k = 3;
                while k + 1 < p.words as usize {
                    if p.op(k + 1) == from {
                        let src = self.at(p.op(k));
                        self.copy(src, sombra, n);
                        break;
                    }
                    k += 2;
                }
            }
        }
        if destino.offset <= self.word {
            self.w.alu_imm(true, Alu::Sub, R15, 1);
            self.trap_if(cc::E, trap::OUT_OF_FUEL);
        }
        let d = self.labels[to as usize] as usize;
        self.w.jmp_to(d);
        Ok(())
    }
}
