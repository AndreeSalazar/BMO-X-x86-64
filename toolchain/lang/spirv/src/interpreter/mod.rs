//! **EL ORACULO (S3): ejecuta un sombreador de computo, invocacion a invocacion.**
//!
//! Es lento A PROPOSITO: es la DEFINICION de lo que un sombreador hace. El
//! emisor de x86-64 (S4) se juzgara contra esto, bit a bit.
//!
//! == La memoria: la da quien llama ==
//!
//! `no_std` y sin `alloc`, como el lector y el juez: [`workspace_words`] dice
//! cuantas palabras hacen falta y [`Interpreter::new`] las recibe. Dentro van
//! tres tablas de `bound` palabras (donde vive cada id, cuanto mide, y en que
//! invocacion se escribio por ultima vez) y la ARENA: un sitio fijo para cada
//! valor y para cada variable. Fijo porque SPIR-V prohibe la recursion en los
//! sombreadores: ningun id puede estar vivo dos veces a la vez.
//!
//! == Los punteros ==
//!
//! Un puntero son dos palabras: el ESPACIO (0 = la arena, `k + 1` = el buffer
//! `k`) y el DESPLAZAMIENTO (palabras en la arena, BYTES en un buffer). En la
//! arena los compuestos van juntos, sin huecos; en un buffer, donde digan sus
//! decoraciones `Offset` y `ArrayStride` -- que es la disposicion que escribio
//! quien preparo los datos (std140, std430).
//!
//! == Lo que SPIR-V deja indefinido, aqui PARA ==
//!
//! Division entera por cero, `INT_MIN / -1`, desplazar 32 bits o mas, salirse
//! de un buffer, leer un valor que esta invocacion no definio, un bucle sin fin
//! (combustible). Cada uno es un [`Trap`] con su motivo, la palabra del fichero
//! y la invocacion. Un oraculo que diera "lo que salga" no seria un oraculo.
//!
//! Lo que SI se define aqui en vez de parar (y el emisor tiene que igualarlo):
//! la aritmetica de [`crate::math`], y que una variable de funcion sin
//! inicializar vale 0.
//!
//! [consumo]  NADA   corre solo mientras alguien despacha; sin estado vivo fuera

mod buffers;
mod values;

use crate::table::op;
use crate::{op_info, validate, EntryPoint, Error, Instruction, Module, Reason};

/// Un id cuyo valor no cambia entre invocaciones (constantes, punteros a
/// variables globales).
const PERMANENT: u32 = u32::MAX;
/// El espacio de un puntero a la arena.
const ARENA: u32 = 0;
/// Lo que no tiene disposicion en un buffer.
const SIN: u32 = u32::MAX;

const CLASS_INPUT: u32 = 1;
const CLASS_UNIFORM: u32 = 2;
const CLASS_PRIVATE: u32 = 6;
const CLASS_STORAGE_BUFFER: u32 = 12;

const DEC_ARRAY_STRIDE: u32 = 6;
const DEC_BUILTIN: u32 = 11;
const DEC_BINDING: u32 = 33;
const DEC_DESCRIPTOR_SET: u32 = 34;
const DEC_OFFSET: u32 = 35;

const BUILTIN_NUM_WORKGROUPS: u32 = 24;
const BUILTIN_WORKGROUP_ID: u32 = 26;
const BUILTIN_LOCAL_INVOCATION_ID: u32 = 27;
const BUILTIN_GLOBAL_INVOCATION_ID: u32 = 28;
const BUILTIN_LOCAL_INVOCATION_INDEX: u32 = 29;

/// Cuantas llamadas anidadas se guardan. Sin recursion, es la profundidad del
/// arbol de llamadas; un sombreador de verdad no pasa de un punado.
const MAX_DEPTH: usize = 32;

/// Un buffer que el sombreador lee o escribe, por su `(DescriptorSet, Binding)`.
pub struct Buffer<'d> {
    pub set: u32,
    pub binding: u32,
    pub data: &'d mut [u32],
}

/// El oraculo paro: por que, donde y en que invocacion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trap {
    pub reason: Reason,
    /// La palabra del fichero de la instruccion que paro.
    pub word: usize,
    /// `GlobalInvocationId` de la invocacion.
    pub invocation: [u32; 3],
}

/// Lo que se ejecuto.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub invocations: u64,
    pub instructions: u64,
}

#[derive(Clone, Copy, Default)]
struct Frame {
    ret: usize,
    result: u32,
    current: u32,
    previous: u32,
}

/// Lo que coloca la planificacion de la arena.
enum Placed {
    Value { id: u32, off: u32, len: u32 },
    Storage { id: u32, off: u32 },
    Members { id: u32, off: u32, n: u32 },
}

// ---- preguntas sobre tipos, que no necesitan la arena -------------------------

fn def<'a>(m: &Module<'a, '_>, id: u32) -> Option<Instruction<'a>> {
    m.def(id)
}

/// Cuantas palabras mide un valor de tipo `t` en la arena (juntas, sin huecos).
fn dense(m: &Module, t: u32) -> u32 {
    let Some(d) = def(m, t) else { return 0 };
    match d.opcode {
        op::OpTypeBool | op::OpTypeInt | op::OpTypeFloat => 1,
        op::OpTypeVector => d.op(3),
        op::OpTypeArray => {
            let n = def(m, d.op(3)).map(|c| c.op(3)).unwrap_or(0);
            n.saturating_mul(dense(m, d.op(2)))
        }
        op::OpTypeStruct => (2..d.words as usize).map(|k| dense(m, d.op(k))).sum(),
        op::OpTypePointer => 2,
        _ => 0,
    }
}

/// `(clase, apuntado)` de un tipo puntero.
fn pointer(m: &Module, t: u32) -> (u32, u32) {
    match def(m, t) {
        Some(d) if d.opcode == op::OpTypePointer => (d.op(2), d.op(3)),
        _ => (0, 0),
    }
}

/// El tipo de un valor.
fn type_of(m: &Module, id: u32) -> u32 {
    def(m, id).map(|d| d.op(1)).unwrap_or(0)
}

/// El valor de una decoracion sobre `id`.
fn decoration(m: &Module, id: u32, dec: u32) -> Option<u32> {
    m.instructions()
        .take_while(|i| i.opcode != op::OpFunction)
        .find(|i| i.opcode == op::OpDecorate && i.op(1) == id && i.op(2) == dec)
        .map(|i| i.op(3))
}

fn member_offset(m: &Module, id: u32, member: u32) -> Option<u32> {
    m.instructions()
        .take_while(|i| i.opcode != op::OpFunction)
        .find(|i| i.opcode == op::OpMemberDecorate && i.op(1) == id && i.op(2) == member && i.op(3) == DEC_OFFSET)
        .map(|i| i.op(4))
}

/// **Planifica la arena**: cada valor y cada variable, en orden. Devuelve
/// cuantas palabras mide.
fn plan(m: &Module, mut f: impl FnMut(Placed)) -> usize {
    let mut next: u32 = 0;
    for ins in m.instructions() {
        match ins.opcode {
            op::OpTypeStruct => {
                let n = ins.words as u32 - 2;
                f(Placed::Members { id: ins.op(1), off: next, n });
                next += n;
            }
            op::OpFunction => {}
            _ => {
                let Some(info) = op_info(ins.opcode) else { continue };
                if !info.has_result_type {
                    continue;
                }
                let id = ins.op(2);
                let len = dense(m, ins.op(1));
                f(Placed::Value { id, off: next, len });
                next += len;
                if ins.opcode == op::OpVariable {
                    let (class, pointee) = pointer(m, ins.op(1));
                    if class != CLASS_UNIFORM && class != CLASS_STORAGE_BUFFER {
                        f(Placed::Storage { id, off: next });
                        next += dense(m, pointee);
                    }
                }
            }
        }
    }
    next as usize
}

/// **Cuantas palabras de trabajo necesita el oraculo para este modulo.**
pub fn workspace_words(m: &Module) -> usize {
    3 * m.header.bound as usize + plan(m, |_| {})
}

/// El oraculo, preparado para un modulo.
pub struct Interpreter<'m, 'a, 'b, 'w> {
    m: &'m Module<'a, 'b>,
    entry: EntryPoint<'a>,
    slot: &'w mut [u32],
    len: &'w mut [u32],
    stamp: &'w mut [u32],
    arena: &'w mut [u32],
    epoch: u32,
}

type Paso<T> = Result<T, Reason>;

impl<'m, 'a, 'b, 'w> Interpreter<'m, 'a, 'b, 'w> {
    /// **Prepara el oraculo.** Juzga el modulo (solo ejecuta lo que cabe),
    /// reparte la arena y escribe las constantes.
    pub fn new(m: &'m Module<'a, 'b>, workspace: &'w mut [u32]) -> Result<Self, Error> {
        validate(m)?;
        let need = workspace_words(m);
        if workspace.len() < need {
            return Err(Error { reason: Reason::WorkspaceTooSmall { need, have: workspace.len() }, word: 0 });
        }
        let bound = m.header.bound as usize;
        let (slot, resto) = workspace.split_at_mut(bound);
        let (len, resto) = resto.split_at_mut(bound);
        let (stamp, arena) = resto.split_at_mut(bound);
        slot.fill(0);
        len.fill(0);
        stamp.fill(0);
        arena.fill(0);
        plan(m, |p| match p {
            Placed::Value { id, off, len: n } => {
                slot[id as usize] = off;
                len[id as usize] = n;
            }
            Placed::Storage { id, off } => {
                // El puntero a una variable de la arena no cambia nunca.
                let s = slot[id as usize] as usize;
                arena[s] = ARENA;
                arena[s + 1] = off;
                stamp[id as usize] = PERMANENT;
            }
            Placed::Members { id, off, n } => {
                slot[id as usize] = off;
                len[id as usize] = n;
                for k in 0..n {
                    arena[(off + k) as usize] = member_offset(m, id, k).unwrap_or(SIN);
                }
            }
        });
        let entry = m.entry_points()[0];
        let mut it = Interpreter { m, entry, slot, len, stamp, arena, epoch: 0 };
        it.constants();
        Ok(it)
    }

    /// Las constantes, una vez: no cambian entre invocaciones.
    fn constants(&mut self) {
        let m = self.m;
        for ins in m.instructions().take_while(|i| i.opcode != op::OpFunction) {
            let id = ins.op(2);
            match ins.opcode {
                op::OpConstant => self.arena[self.slot[id as usize] as usize] = ins.op(3),
                op::OpConstantTrue => self.arena[self.slot[id as usize] as usize] = 1,
                op::OpConstantFalse | op::OpConstantNull | op::OpUndef => {}
                op::OpConstantComposite => {
                    let mut d = self.slot[id as usize] as usize;
                    for k in 3..ins.words as usize {
                        let p = ins.op(k) as usize;
                        let (o, n) = (self.slot[p] as usize, self.len[p] as usize);
                        self.arena.copy_within(o..o + n, d);
                        d += n;
                    }
                }
                _ => continue,
            }
            self.stamp[id as usize] = PERMANENT;
        }
    }

    // ---- la arena ------------------------------------------------------------

    fn ready(&self, id: u32) -> Paso<()> {
        let s = *self.stamp.get(id as usize).ok_or(Reason::UndefinedValue { id })?;
        if s == PERMANENT || s == self.epoch {
            Ok(())
        } else {
            Err(Reason::UndefinedValue { id })
        }
    }

    /// Los componentes (hasta 4) de un escalar o un vector.
    fn comps(&self, id: u32) -> Paso<([u32; 4], usize)> {
        self.ready(id)?;
        let o = self.slot[id as usize] as usize;
        let n = (self.len[id as usize] as usize).min(4);
        let mut v = [0u32; 4];
        v[..n].copy_from_slice(&self.arena[o..o + n]);
        Ok((v, n))
    }

    fn scalar(&self, id: u32) -> Paso<u32> {
        Ok(self.comps(id)?.0[0])
    }

    fn put(&mut self, id: u32, v: &[u32]) {
        let o = self.slot[id as usize] as usize;
        self.arena[o..o + v.len()].copy_from_slice(v);
        self.stamp[id as usize] = self.epoch;
    }

    /// Copia el valor `from` (entero, sea lo que sea) a la arena en `to`.
    fn copy_to(&mut self, from: u32, to: usize) -> Paso<usize> {
        self.ready(from)?;
        let (o, n) = (self.slot[from as usize] as usize, self.len[from as usize] as usize);
        self.arena.copy_within(o..o + n, to);
        Ok(n)
    }

    fn copy_value(&mut self, from: u32, to: u32) -> Paso<()> {
        let d = self.slot[to as usize] as usize;
        self.copy_to(from, d)?;
        self.stamp[to as usize] = self.epoch;
        Ok(())
    }

    fn ptr(&self, id: u32) -> Paso<(u32, u32)> {
        self.ready(id)?;
        let o = self.slot[id as usize] as usize;
        Ok((self.arena[o], self.arena[o + 1]))
    }

    // ---- despachar -----------------------------------------------------------

    /// **Despacha** `groups` grupos de trabajo sobre `buffers`. `fuel` es el
    /// maximo de instrucciones por invocacion: un bucle que no termina para
    /// con `OutOfFuel` en vez de colgar al que llama.
    pub fn dispatch(&mut self, groups: [u32; 3], buffers: &mut [Buffer], fuel: u64) -> Result<Stats, Trap> {
        let nada = [0u32; 3];
        self.bind(buffers).map_err(|(reason, word)| Trap { reason, word, invocation: nada })?;
        let ls = self.entry.local_size.unwrap_or([1, 1, 1]);
        let mut stats = Stats::default();
        for wz in 0..groups[2] {
            for wy in 0..groups[1] {
                for wx in 0..groups[0] {
                    for lz in 0..ls[2] {
                        for ly in 0..ls[1] {
                            for lx in 0..ls[0] {
                                let local = [lx, ly, lz];
                                let group = [wx, wy, wz];
                                let global = [wx * ls[0] + lx, wy * ls[1] + ly, wz * ls[2] + lz];
                                let index = lz * ls[0] * ls[1] + ly * ls[0] + lx;
                                let n = self
                                    .invocation(global, local, group, groups, index, buffers, fuel)
                                    .map_err(|(reason, word)| Trap { reason, word, invocation: global })?;
                                stats.invocations += 1;
                                stats.instructions += n;
                            }
                        }
                    }
                }
            }
        }
        Ok(stats)
    }

    /// Los punteros a los buffers: `(set, binding)` -> el indice del que se dio.
    fn bind(&mut self, buffers: &[Buffer]) -> Result<(), (Reason, usize)> {
        let m = self.m;
        for ins in m.instructions().take_while(|i| i.opcode != op::OpFunction) {
            if ins.opcode != op::OpVariable || !matches!(ins.op(3), CLASS_UNIFORM | CLASS_STORAGE_BUFFER) {
                continue;
            }
            let id = ins.op(2);
            let set = decoration(m, id, DEC_DESCRIPTOR_SET).unwrap_or(0);
            let binding = decoration(m, id, DEC_BINDING).unwrap_or(0);
            let k = buffers
                .iter()
                .position(|b| b.set == set && b.binding == binding)
                .ok_or((Reason::NoBuffer { set, binding }, ins.offset))?;
            let s = self.slot[id as usize] as usize;
            self.arena[s] = k as u32 + 1;
            self.arena[s + 1] = 0;
            self.stamp[id as usize] = PERMANENT;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn invocation(
        &mut self,
        global: [u32; 3],
        local: [u32; 3],
        group: [u32; 3],
        groups: [u32; 3],
        index: u32,
        buffers: &mut [Buffer],
        fuel: u64,
    ) -> Result<u64, (Reason, usize)> {
        // Una epoca nueva: todo lo escrito en la anterior deja de estar definido.
        self.epoch = self.epoch.wrapping_add(1);
        if self.epoch == PERMANENT || self.epoch == 0 {
            for s in self.stamp.iter_mut() {
                if *s != PERMANENT {
                    *s = 0;
                }
            }
            self.epoch = 1;
        }
        // Las globales: los BuiltIn de esta invocacion y las privadas desde cero.
        let m = self.m;
        for ins in m.instructions().take_while(|i| i.opcode != op::OpFunction) {
            if ins.opcode != op::OpVariable {
                continue;
            }
            let id = ins.op(2);
            let dir = self.arena[self.slot[id as usize] as usize + 1] as usize;
            match ins.op(3) {
                CLASS_INPUT => {
                    let v = match decoration(m, id, DEC_BUILTIN) {
                        Some(BUILTIN_GLOBAL_INVOCATION_ID) => global,
                        Some(BUILTIN_LOCAL_INVOCATION_ID) => local,
                        Some(BUILTIN_WORKGROUP_ID) => group,
                        Some(BUILTIN_NUM_WORKGROUPS) => groups,
                        Some(BUILTIN_LOCAL_INVOCATION_INDEX) => [index, 0, 0],
                        _ => [0, 0, 0],
                    };
                    let n = dense(m, pointer(m, ins.op(1)).1) as usize;
                    self.arena[dir..dir + n.min(3)].copy_from_slice(&v[..n.min(3)]);
                }
                CLASS_PRIVATE => {
                    let n = dense(m, pointer(m, ins.op(1)).1) as usize;
                    if ins.words > 4 {
                        self.copy_to(ins.op(4), dir).map_err(|r| (r, ins.offset))?;
                    } else {
                        self.arena[dir..dir + n].fill(0);
                    }
                }
                _ => {}
            }
        }
        let f = m.def(self.entry.id).map(|d| d.offset).unwrap_or(0);
        self.run(f, buffers, fuel)
    }

    /// El primer bloque de la funcion que empieza en `f`, tras sus parametros.
    fn body_of(&self, f: usize) -> Option<Instruction<'a>> {
        self.m.instructions_from(f).skip(1).find(|i| i.opcode != op::OpFunctionParameter)
    }

    /// **Ejecuta** desde la funcion de entrada hasta su `OpReturn`.
    fn run(&mut self, f: usize, buffers: &mut [Buffer], fuel: u64) -> Result<u64, (Reason, usize)> {
        let m = self.m;
        let mut frames = [Frame::default(); MAX_DEPTH];
        let mut depth = 0usize;
        let first = self.body_of(f).ok_or((Reason::NoBody, f))?;
        let mut pc = first.offset;
        let mut current = first.op(1);
        let mut previous = 0u32;
        let mut n: u64 = 0;
        loop {
            let ins = m.instructions_from(pc).next().ok_or((Reason::NoBody, pc))?;
            n += 1;
            if n > fuel {
                return Err((Reason::OutOfFuel, pc));
            }
            let next = pc + ins.words as usize;
            let a_label = move |id: u32| m.def(id).map(|d| d.offset).ok_or((Reason::NotALabel { id }, pc));
            match ins.opcode {
                op::OpLabel | op::OpSelectionMerge | op::OpLoopMerge | op::OpLine | op::OpNoLine | op::OpNop => {
                    pc = next;
                }
                op::OpBranch => {
                    previous = current;
                    current = ins.op(1);
                    pc = a_label(current)?;
                }
                op::OpBranchConditional => {
                    let c = self.scalar(ins.op(1)).map_err(|r| (r, pc))?;
                    previous = current;
                    current = if c != 0 { ins.op(2) } else { ins.op(3) };
                    pc = a_label(current)?;
                }
                op::OpPhi => {
                    let mut k = 3;
                    let mut hecho = false;
                    while k + 1 < ins.words as usize {
                        if ins.op(k + 1) == previous {
                            self.copy_value(ins.op(k), ins.op(2)).map_err(|r| (r, pc))?;
                            hecho = true;
                            break;
                        }
                        k += 2;
                    }
                    if !hecho {
                        return Err((Reason::PhiWithoutPredecessor, pc));
                    }
                    pc = next;
                }
                op::OpFunctionCall => {
                    if depth == MAX_DEPTH {
                        return Err((Reason::CallTooDeep, pc));
                    }
                    let callee = m.def(ins.op(3)).ok_or((Reason::Undefined { id: ins.op(3) }, pc))?;
                    // Los argumentos a los parametros.
                    let mut k = 4;
                    for p in m.instructions_from(callee.offset).skip(1) {
                        if p.opcode != op::OpFunctionParameter {
                            break;
                        }
                        self.copy_value(ins.op(k), p.op(2)).map_err(|r| (r, pc))?;
                        k += 1;
                    }
                    frames[depth] = Frame { ret: next, result: ins.op(2), current, previous };
                    depth += 1;
                    let body = self.body_of(callee.offset).ok_or((Reason::NoBody, callee.offset))?;
                    pc = body.offset;
                    current = body.op(1);
                    previous = 0;
                }
                op::OpReturn | op::OpReturnValue => {
                    if depth == 0 {
                        return Ok(n);
                    }
                    depth -= 1;
                    let fr = frames[depth];
                    if ins.opcode == op::OpReturnValue {
                        self.copy_value(ins.op(1), fr.result).map_err(|r| (r, pc))?;
                    }
                    pc = fr.ret;
                    current = fr.current;
                    previous = fr.previous;
                }
                op::OpUnreachable => return Err((Reason::ReachedUnreachable, pc)),
                _ => {
                    self.step(&ins, buffers).map_err(|r| (r, pc))?;
                    pc = next;
                }
            }
        }
    }
}
