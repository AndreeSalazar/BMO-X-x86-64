//! # bmo-titan-spirv -- una `gpu fn` de TITAN++, hecha SPIR-V para la 3060
//!
//! Nivel 11, G2 de `docs/plan/PLAN_EL_CENTAURO.md`. TITAN_MAESTRO 7.4: el
//! frontend no sabe de maquinas, y lo que viaja a la GPU es un FORMATO. Este
//! crate escribe ese formato; quien lo JUZGA es el juez de spirv, que no sabe
//! quien lo escribio:
//!
//! ```text
//!    la IR de una gpu fn  (bmo-titan-front: ya juzgada, ya calculada)
//!         |  write()
//!         v
//!    un modulo SPIR-V 1.0 de computo: un hilo por celda
//!         |  judge()
//!         v
//!    bmo-spirv-front::validate_stage(GLCompute)   el juez neutro
//!    bmo-spirv-sm86::check                        lo que la 3060 traga
//!         |  run()  (G3)
//!         v
//!    el interprete de spirv, el ORACULO: las celdas, calculadas
//! ```
//!
//! ** LA FORMA DEL MODULO: la misma que el banco de spirv ya conoce (la que
//! escribe glslang para SPIR-V 1.0). Un buffer por valor de la gpu fn, en la
//! ranura `k` (DescriptorSet 0, Binding k), y uno mas para el resultado; un
//! hilo por celda (`LocalSize 1 1 1`, y se despachan `n` grupos): la celda es
//! `GlobalInvocationId.x`, sin limites que comprobar.
//!
//! ** SIN SALTOS: cada `if` es un `OpSelect`. Una gpu fn es PURA (no escribe
//! nada, no llama a nada) y no tiene bucles, asi que se pueden calcular los
//! dos lados de un `if` y quedarse con el que toca: el resultado es EL MISMO,
//! bit a bit, porque un f32 no se para (dividir entre cero da infinito, igual
//! en los dos lados). Y sale codigo en linea recta: lo que el emisor de SASS
//! de la 3060 (E3, PLAN_LA_LENGUA_DE_LA_3060) ya traduce.
//!
//! El algoritmo: la IR de una gpu fn es un grafo sin ciclos con los bloques en
//! orden. Se recorren en ese orden; cada bloque lleva su PREDICADO ("este
//! camino es el que corre") y el valor de cada nombre; donde dos caminos se
//! juntan, cada nombre se elige con `OpSelect` por el predicado del camino. Al
//! final, el resultado se elige igual entre los `return`.

use bmo_spirv_front::table::op;
use bmo_titan_front::ir::{End, Function, Module, Op, Value};
use std::collections::HashMap;

/// Lo que un valor ES dentro de una gpu fn: solo hay dos clases.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    F32,
    Bool,
}

/// Una gpu fn escrita: el modulo, y la forma de sus buffers.
pub struct Kernel {
    pub name: String,
    pub words: Vec<u32>,
    /// La clase de cada valor, en el orden de sus ranuras (0, 1, ...).
    pub params: Vec<Kind>,
    /// La del resultado, en la ranura `params.len()`.
    pub ret: Kind,
}

impl Kernel {
    pub fn bytes(&self) -> Vec<u8> {
        self.words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    /// Los codigos de sus instrucciones, en orden (sin la cabecera).
    pub fn opcodes(&self) -> Vec<u16> {
        let mut out = Vec::new();
        let mut i = 5;
        while i < self.words.len() {
            out.push((self.words[i] & 0xffff) as u16);
            i += ((self.words[i] >> 16) as usize).max(1);
        }
        out
    }
}

/// **Todas las gpu fn de un modulo**, escritas y juzgadas: lo que `titan
/// build` pide antes de escribir un `.bex`. Un NO aqui es del ESCRITOR.
pub fn kernels(m: &Module) -> Result<Vec<Kernel>, String> {
    let mut out = Vec::new();
    for (i, f) in m.functions.iter().enumerate().filter(|(_, f)| f.gpu) {
        let k = write(m, i).map_err(|e| format!("`gpu fn {}`: {}", f.name, e.0))?;
        judge(&k).map_err(|e| format!("`gpu fn {}`: el juez de spirv dijo que no -- {}", f.name, e))?;
        out.push(k);
    }
    Ok(out)
}

// ---- los numeros de la especificacion que no son instrucciones -----------------

const CAP_SHADER: u32 = 1;
const ADDRESSING_LOGICAL: u32 = 0;
const MEMORY_GLSL450: u32 = 1;
const MODEL_GLCOMPUTE: u32 = 5;
const MODE_LOCAL_SIZE: u32 = 17;
const DEC_BLOCK_BUFFER: u32 = 3;
const DEC_ARRAY_STRIDE: u32 = 6;
const DEC_BUILTIN: u32 = 11;
const DEC_NON_WRITABLE: u32 = 24;
const DEC_NON_READABLE: u32 = 25;
const DEC_BINDING: u32 = 33;
const DEC_DESCRIPTOR_SET: u32 = 34;
const DEC_OFFSET: u32 = 35;
const BUILTIN_GLOBAL_INVOCATION_ID: u32 = 28;
const CLASS_INPUT: u32 = 1;
const CLASS_UNIFORM: u32 = 2;
const CLASS_FUNCTION_NONE: u32 = 0;

fn ins(buf: &mut Vec<u32>, opcode: u16, operands: &[u32]) {
    buf.push(((operands.len() as u32 + 1) << 16) | opcode as u32);
    buf.extend_from_slice(operands);
}

/// Un texto de SPIR-V: sus bytes, un 0, y relleno hasta la palabra.
fn string(s: &str) -> Vec<u32> {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    while b.len() % 4 != 0 {
        b.push(0);
    }
    b.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

/// Por que una gpu fn no se pudo escribir: siempre un fallo del ESCRITOR o
/// del frontend (el programa ya paso por sus jueces), dicho con su nombre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure(pub String);

struct Writer {
    next: u32,
    decorations: Vec<u32>,
    globals: Vec<u32>,
    body: Vec<u32>,
    float: u32,
    uint: u32,
    boolean: u32,
    floats: HashMap<u32, u32>,
    uints: HashMap<u32, u32>,
    bools: [Option<u32>; 2],
}

impl Writer {
    fn id(&mut self) -> u32 {
        let i = self.next;
        self.next += 1;
        i
    }

    fn global(&mut self, opcode: u16, operands: &[u32]) {
        ins(&mut self.globals, opcode, operands);
    }

    fn float_const(&mut self, bits: u32) -> u32 {
        if let Some(&i) = self.floats.get(&bits) {
            return i;
        }
        let i = self.id();
        let t = self.float;
        self.global(op::OpConstant, &[t, i, bits]);
        self.floats.insert(bits, i);
        i
    }

    fn uint_const(&mut self, v: u32) -> u32 {
        if let Some(&i) = self.uints.get(&v) {
            return i;
        }
        let i = self.id();
        let t = self.uint;
        self.global(op::OpConstant, &[t, i, v]);
        self.uints.insert(v, i);
        i
    }

    fn bool_const(&mut self, v: bool) -> u32 {
        if let Some(i) = self.bools[v as usize] {
            return i;
        }
        let i = self.id();
        let t = self.boolean;
        self.global(if v { op::OpConstantTrue } else { op::OpConstantFalse }, &[t, i]);
        self.bools[v as usize] = Some(i);
        i
    }

    /// Una instruccion de valor en el cuerpo: su resultado.
    fn value(&mut self, opcode: u16, ty: u32, operands: &[u32]) -> u32 {
        let i = self.id();
        let mut all = vec![ty, i];
        all.extend_from_slice(operands);
        ins(&mut self.body, opcode, &all);
        i
    }

    fn ty(&self, k: Kind) -> u32 {
        match k {
            Kind::F32 => self.float,
            Kind::Bool => self.boolean,
        }
    }

    fn and(&mut self, a: u32, b: u32) -> u32 {
        let t = self.boolean;
        self.value(op::OpLogicalAnd, t, &[a, b])
    }

    fn select(&mut self, k: Kind, cond: u32, yes: u32, no: u32) -> u32 {
        let t = self.ty(k);
        self.value(op::OpSelect, t, &[cond, yes, no])
    }
}

/// Lo que vale cada nombre de la gpu fn en un punto: (id, clase).
type Env = HashMap<usize, (u32, Kind)>;

fn kind_of(t: &bmo_titan_front::tree::Ty) -> Result<Kind, Failure> {
    match t {
        bmo_titan_front::tree::Ty::F32 => Ok(Kind::F32),
        bmo_titan_front::tree::Ty::Bool => Ok(Kind::Bool),
        other => Err(Failure(format!("una gpu fn con un `{}`: el frontend (gpu.rs) tenia que haberlo dicho", other.name()))),
    }
}

fn eval(w: &mut Writer, v: &Value, env: &Env) -> Result<(u32, Kind), Failure> {
    Ok(match v {
        Value::F32(bits, _) => (w.float_const(*bits), Kind::F32),
        Value::Bool(b, _) => (w.bool_const(*b), Kind::Bool),
        Value::Local(l, _) => *env.get(l).ok_or_else(|| Failure(format!("el nombre %{} se lee sin valor: el juez tenia que haberlo dicho", l)))?,
        Value::Neg(x, _) => {
            let (x, k) = eval(w, x, env)?;
            let t = w.float;
            (w.value(op::OpFNegate, t, &[x]), k)
        }
        Value::Not(x, _) => {
            let (x, _) = eval(w, x, env)?;
            let t = w.boolean;
            (w.value(op::OpLogicalNot, t, &[x]), Kind::Bool)
        }
        Value::Bin(o, l, r, _) => {
            let (a, ka) = eval(w, l, env)?;
            let (b, _) = eval(w, r, env)?;
            let (f, bo) = (w.float, w.boolean);
            match (*o, ka) {
                ("+", Kind::F32) => (w.value(op::OpFAdd, f, &[a, b]), Kind::F32),
                ("-", Kind::F32) => (w.value(op::OpFSub, f, &[a, b]), Kind::F32),
                ("*", Kind::F32) => (w.value(op::OpFMul, f, &[a, b]), Kind::F32),
                ("/", Kind::F32) => (w.value(op::OpFDiv, f, &[a, b]), Kind::F32),
                // Las de Rust y las del calculo: `!=` es cierto con un NaN.
                ("==", Kind::F32) => (w.value(op::OpFOrdEqual, bo, &[a, b]), Kind::Bool),
                ("!=", Kind::F32) => (w.value(op::OpFUnordNotEqual, bo, &[a, b]), Kind::Bool),
                ("<", Kind::F32) => (w.value(op::OpFOrdLessThan, bo, &[a, b]), Kind::Bool),
                ("<=", Kind::F32) => (w.value(op::OpFOrdLessThanEqual, bo, &[a, b]), Kind::Bool),
                (">", Kind::F32) => (w.value(op::OpFOrdGreaterThan, bo, &[a, b]), Kind::Bool),
                (">=", Kind::F32) => (w.value(op::OpFOrdGreaterThanEqual, bo, &[a, b]), Kind::Bool),
                ("and", Kind::Bool) => (w.value(op::OpLogicalAnd, bo, &[a, b]), Kind::Bool),
                ("or", Kind::Bool) => (w.value(op::OpLogicalOr, bo, &[a, b]), Kind::Bool),
                ("==", Kind::Bool) => (w.value(op::OpLogicalEqual, bo, &[a, b]), Kind::Bool),
                ("!=", Kind::Bool) => (w.value(op::OpLogicalNotEqual, bo, &[a, b]), Kind::Bool),
                (o, k) => return Err(Failure(format!("`{}` entre {:?}: el calculo tenia que haberlo dicho", o, k))),
            }
        }
        other => return Err(Failure(format!("un valor que una gpu fn no tiene ({:?}): gpu.rs tenia que haberlo dicho", other))),
    })
}

/// **Escribe la gpu fn `func` de `m` como un modulo SPIR-V de computo.**
pub fn write(m: &Module, func: usize) -> Result<Kernel, Failure> {
    let f: &Function = &m.functions[func];
    if !f.gpu {
        return Err(Failure(format!("`{}` no es una gpu fn", f.name)));
    }
    let params: Vec<Kind> = f.params.iter().map(|(_, t)| kind_of(t)).collect::<Result<_, _>>()?;
    let ret = kind_of(f.ret.as_ref().ok_or_else(|| Failure(format!("`{}` no devuelve nada", f.name)))?)?;
    let mut w = Writer { next: 1, decorations: Vec::new(), globals: Vec::new(), body: Vec::new(), float: 0, uint: 0, boolean: 0, floats: HashMap::new(), uints: HashMap::new(), bools: [None, None] };

    // -- los tipos de siempre ------------------------------------------------------
    let void = w.id();
    w.global(op::OpTypeVoid, &[void]);
    let fn_void = w.id();
    w.global(op::OpTypeFunction, &[fn_void, void]);
    w.uint = w.id();
    let uint = w.uint;
    w.global(op::OpTypeInt, &[uint, 32, 0]);
    w.float = w.id();
    let float = w.float;
    w.global(op::OpTypeFloat, &[float, 32]);
    w.boolean = w.id();
    let boolean = w.boolean;
    w.global(op::OpTypeBool, &[boolean]);
    let v3 = w.id();
    w.global(op::OpTypeVector, &[v3, uint, 3]);
    let p_in_v3 = w.id();
    w.global(op::OpTypePointer, &[p_in_v3, CLASS_INPUT, v3]);
    let p_in_u = w.id();
    w.global(op::OpTypePointer, &[p_in_u, CLASS_INPUT, uint]);
    let gid = w.id();
    w.global(op::OpVariable, &[p_in_v3, gid, CLASS_INPUT]);
    ins(&mut w.decorations, op::OpDecorate, &[gid, DEC_BUILTIN, BUILTIN_GLOBAL_INVOCATION_ID]);
    let p_u_f = w.id();
    w.global(op::OpTypePointer, &[p_u_f, CLASS_UNIFORM, float]);
    let p_u_u = w.id();
    w.global(op::OpTypePointer, &[p_u_u, CLASS_UNIFORM, uint]);

    // -- un buffer por valor, y uno para el resultado ------------------------------
    // Un bool viaja como un uint (0 o 1): un bool de SPIR-V no tiene medida.
    let mut buffers = Vec::new();
    for (k, kind) in params.iter().chain(std::iter::once(&ret)).enumerate() {
        let elem = if *kind == Kind::F32 { float } else { uint };
        let arr = w.id();
        w.global(op::OpTypeRuntimeArray, &[arr, elem]);
        let st = w.id();
        w.global(op::OpTypeStruct, &[st, arr]);
        let p = w.id();
        w.global(op::OpTypePointer, &[p, CLASS_UNIFORM, st]);
        let var = w.id();
        w.global(op::OpVariable, &[p, var, CLASS_UNIFORM]);
        let output = k == params.len();
        let d = &mut w.decorations;
        ins(d, op::OpDecorate, &[arr, DEC_ARRAY_STRIDE, 4]);
        ins(d, op::OpDecorate, &[st, DEC_BLOCK_BUFFER]);
        ins(d, op::OpMemberDecorate, &[st, 0, if output { DEC_NON_READABLE } else { DEC_NON_WRITABLE }]);
        ins(d, op::OpMemberDecorate, &[st, 0, DEC_OFFSET, 0]);
        ins(d, op::OpDecorate, &[var, DEC_DESCRIPTOR_SET, 0]);
        ins(d, op::OpDecorate, &[var, DEC_BINDING, k as u32]);
        buffers.push((var, *kind));
    }

    // -- el cuerpo: la celda, los valores, la gpu fn en linea recta, el resultado ---
    let main = w.id();
    let label = w.id();
    let zero = w.uint_const(0);
    let x_ptr = w.value(op::OpAccessChain, p_in_u, &[gid, zero]);
    let cell = w.value(op::OpLoad, uint, &[x_ptr]);
    let mut env: Env = HashMap::new();
    for (k, ((local, _), kind)) in f.params.iter().zip(&params).enumerate() {
        let (var, _) = buffers[k];
        let v = if *kind == Kind::F32 {
            let ptr = w.value(op::OpAccessChain, p_u_f, &[var, zero, cell]);
            w.value(op::OpLoad, float, &[ptr])
        } else {
            let ptr = w.value(op::OpAccessChain, p_u_u, &[var, zero, cell]);
            let raw = w.value(op::OpLoad, uint, &[ptr]);
            w.value(op::OpINotEqual, boolean, &[raw, zero])
        };
        env.insert(*local, (v, *kind));
    }
    let result = straight(&mut w, f, env)?;
    let (out, _) = buffers[params.len()];
    if ret == Kind::F32 {
        let ptr = w.value(op::OpAccessChain, p_u_f, &[out, zero, cell]);
        ins(&mut w.body, op::OpStore, &[ptr, result]);
    } else {
        let one = w.uint_const(1);
        let raw = w.value(op::OpSelect, uint, &[result, one, zero]);
        let ptr = w.value(op::OpAccessChain, p_u_u, &[out, zero, cell]);
        ins(&mut w.body, op::OpStore, &[ptr, raw]);
    }

    // -- el modulo, en el orden que pide la especificacion -------------------------
    let mut words = vec![0x0723_0203, 0x0001_0000, 0, 0, 0];
    ins(&mut words, op::OpCapability, &[CAP_SHADER]);
    ins(&mut words, op::OpMemoryModel, &[ADDRESSING_LOGICAL, MEMORY_GLSL450]);
    let mut entry = vec![MODEL_GLCOMPUTE, main];
    entry.extend(string("main"));
    entry.push(gid);
    ins(&mut words, op::OpEntryPoint, &entry);
    ins(&mut words, op::OpExecutionMode, &[main, MODE_LOCAL_SIZE, 1, 1, 1]);
    words.extend(&w.decorations);
    words.extend(&w.globals);
    ins(&mut words, op::OpFunction, &[void, main, CLASS_FUNCTION_NONE, fn_void]);
    ins(&mut words, op::OpLabel, &[label]);
    words.extend(&w.body);
    ins(&mut words, op::OpReturn, &[]);
    ins(&mut words, op::OpFunctionEnd, &[]);
    words[3] = w.next;
    Ok(Kernel { name: f.name.clone(), words, params, ret })
}

/// La gpu fn en LINEA RECTA: cada bloque con su predicado, cada nombre elegido
/// con `OpSelect` donde los caminos se juntan, y el resultado elegido entre los
/// `return`. Devuelve el id del resultado.
fn straight(w: &mut Writer, f: &Function, entry_env: Env) -> Result<u32, Failure> {
    let n = f.blocks.len();
    // Lo que llega a cada bloque: (predicado del camino, valores).
    let mut incoming: Vec<Vec<(u32, Env)>> = vec![Vec::new(); n];
    let mut returns: Vec<(u32, u32, Kind)> = Vec::new();
    for b in 0..n {
        let (pred, mut env) = if b == 0 {
            (w.bool_const(true), entry_env.clone())
        } else {
            let edges = std::mem::take(&mut incoming[b]);
            if edges.is_empty() {
                // Ningun camino llega aqui (lo que sigue a un `return`).
                continue;
            }
            let mut pred = edges[0].0;
            for (p, _) in &edges[1..] {
                let t = w.boolean;
                pred = w.value(op::OpLogicalOr, t, &[pred, *p]);
            }
            let mut env: Env = HashMap::new();
            let mut names: Vec<usize> = edges.iter().flat_map(|(_, e)| e.keys().copied()).collect();
            names.sort_unstable();
            names.dedup();
            for l in names {
                let mut acc: Option<(u32, Kind)> = None;
                for (p, e) in &edges {
                    if let Some(&(v, k)) = e.get(&l) {
                        acc = Some(match acc {
                            None => (v, k),
                            Some((a, _)) if a == v => (a, k),
                            Some((a, _)) => (w.select(k, *p, v, a), k),
                        });
                    }
                }
                if let Some(v) = acc {
                    env.insert(l, v);
                }
            }
            (pred, env)
        };
        for o in &f.blocks[b].ops {
            match o {
                Op::Let { local, value, .. } | Op::Set { local, value, .. } => {
                    let v = eval(w, value, &env)?;
                    env.insert(*local, v);
                }
                Op::Drop { .. } => {}
                other => return Err(Failure(format!("una gpu fn con {:?}: gpu.rs tenia que haberlo dicho", other))),
            }
        }
        match &f.blocks[b].end {
            End::Jump(t) => {
                if *t <= b {
                    return Err(Failure("un salto hacia arriba: una gpu fn no tiene bucles".into()));
                }
                incoming[*t].push((pred, env));
            }
            End::Branch { cond, then, other, .. } => {
                if *then <= b || *other <= b {
                    return Err(Failure("un salto hacia arriba: una gpu fn no tiene bucles".into()));
                }
                let (c, _) = eval(w, cond, &env)?;
                let yes = w.and(pred, c);
                let t = w.boolean;
                let not_c = w.value(op::OpLogicalNot, t, &[c]);
                let no = w.and(pred, not_c);
                incoming[*then].push((yes, env.clone()));
                incoming[*other].push((no, env));
            }
            End::Return(Some(v)) => {
                let (r, k) = eval(w, v, &env)?;
                returns.push((pred, r, k));
            }
            End::Return(None) => return Err(Failure("un camino sin `return`: el juez tenia que haberlo dicho (T0070)".into())),
        }
    }
    let (_, mut acc, kind) = *returns.last().ok_or_else(|| Failure("una gpu fn sin `return`".into()))?;
    for (p, r, _) in returns.iter().rev().skip(1) {
        acc = w.select(kind, *p, *r, acc);
    }
    Ok(acc)
}

/// **El juez de spirv sobre lo escrito**: el lector, el validador neutro con
/// la etapa de computo, y el subconjunto de la 3060. Su NO, en sus palabras.
pub fn judge(k: &Kernel) -> Result<bmo_spirv_sm86::Fit, String> {
    let bytes = k.bytes();
    let mut ids = vec![0u32; k.words[3] as usize + 16];
    let m = bmo_spirv_front::read(&bytes, &mut ids).map_err(|e| format!("lector: {}", e))?;
    bmo_spirv_sm86::check(&m, bmo_spirv_front::Stage::GLCompute).map_err(|r| format!("{}", r))
}

/// **El oraculo (G3)**: la gpu fn corrida por el interprete de spirv, una
/// invocacion por celda. `cells[k]` son las celdas del valor `k` (sus bits);
/// todas del mismo largo. Devuelve las del resultado.
pub fn run(k: &Kernel, cells: &[Vec<u32>]) -> Result<Vec<u32>, String> {
    let n = cells.first().map(|c| c.len()).unwrap_or(0);
    let bytes = k.bytes();
    let mut ids = vec![0u32; k.words[3] as usize + 16];
    let m = bmo_spirv_front::read(&bytes, &mut ids).map_err(|e| format!("lector: {}", e))?;
    let mut ws = vec![0u32; bmo_spirv_front::workspace_words(&m)];
    let mut it = bmo_spirv_front::Interpreter::new(&m, &mut ws).map_err(|e| format!("oraculo: {}", e))?;
    let mut data: Vec<Vec<u32>> = cells.to_vec();
    data.push(vec![0u32; n]);
    let mut buffers: Vec<bmo_spirv_front::Buffer> = data.iter_mut().enumerate().map(|(i, d)| bmo_spirv_front::Buffer { set: 0, binding: i as u32, data: d.as_mut_slice() }).collect();
    it.dispatch([n as u32, 1, 1], &mut buffers, 1_000_000).map_err(|t| format!("oraculo: {:?}", t))?;
    drop(buffers);
    Ok(data.pop().unwrap_or_default())
}

/// ** EL ORACULO COMO `Device` DEL CALCULO (G3): cada gpu fn se escribe UNA
/// vez, se juzga, y sus celdas las calcula el interprete de spirv. Es lo que
/// `titan build` le da al calculo: los resultados que lleva el `.bex` son los
/// del oraculo, el mismo que mide a los emisores de la casa.
#[derive(Default)]
pub struct Oracle {
    written: HashMap<usize, Kernel>,
}

impl bmo_titan_front::calc::Device for Oracle {
    fn run(&mut self, m: &Module, func: usize, cells: Vec<Vec<u32>>) -> Result<Vec<u32>, String> {
        if !self.written.contains_key(&func) {
            let k = write(m, func).map_err(|e| e.0)?;
            judge(&k).map_err(|e| format!("el juez de spirv dijo que no: {}", e))?;
            self.written.insert(func, k);
        }
        run(&self.written[&func], &cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(src: &str) -> Module {
        let toml = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";
        bmo_titan_front::lower_package("src/main.titan", src, &mut |p| (p == "Titan.toml").then(|| toml.to_string())).unwrap_or_else(|e| panic!("{:?}", e))
    }

    fn kernel(src: &str, name: &str) -> Kernel {
        let m = module(src);
        let f = m.functions.iter().position(|f| f.name == name).unwrap();
        write(&m, f).unwrap_or_else(|e| panic!("{:?}", e))
    }

    fn f32s(v: &[f32]) -> Vec<u32> {
        v.iter().map(|x| x.to_bits()).collect()
    }

    /// ** Lo que escribe pasa por el juez de spirv entero, y el oraculo da
    /// los mismos bits que el f32 de Rust (lo que hoy usa el calculo).
    #[test]
    fn a_gpu_fn_is_valid_spirv_and_the_oracle_agrees_bit_by_bit() {
        let src = "mod main \"x\"\ngpu fn mezcla(a: f32, b: f32) -> f32\n    return (a + b) / 2.0\nfn main()\n    let xs: [f32; 2] = [1.0, 2.0]\n    let r = mezcla(xs, xs)\n    print(round(r[0], 1))\n";
        let k = kernel(src, "mezcla");
        let fit = judge(&k).unwrap_or_else(|e| panic!("el juez de spirv dijo que no: {}", e));
        assert_eq!(fit.slots, 0b111, "dos valores y el resultado, en las ranuras 0, 1 y 2");
        let a = [1.0f32, 0.1, -3.5, 1e30, 7.25];
        let b = [3.0f32, 0.2, 2.0, 1e30, -0.0];
        let got = run(&k, &[f32s(&a), f32s(&b)]).unwrap();
        let want: Vec<u32> = a.iter().zip(&b).map(|(x, y)| ((x + y) / 2.0).to_bits()).collect();
        assert_eq!(got, want);
    }

    /// ** Un `if` con sus `return`, en linea recta: el mismo resultado que
    /// los saltos, en cada celda -- los dos lados, y el limite.
    #[test]
    fn an_if_becomes_a_select_and_gives_the_same_cells() {
        let src = "mod main \"x\"\ngpu fn activa(x: f32) -> f32\n    if x > 0.0\n        return x\n    let y = x * 0.5\n    if y < -1.0\n        return -1.0\n    else\n        return y\nfn main()\n    let xs: [f32; 1] = [1.0]\n    let r = activa(xs)\n    print(round(r[0], 1))\n";
        let k = kernel(src, "activa");
        judge(&k).unwrap_or_else(|e| panic!("el juez de spirv dijo que no: {}", e));
        let xs = [2.5f32, 0.0, -1.0, -2.0, -4.0, f32::NAN];
        let got = run(&k, &[f32s(&xs)]).unwrap();
        let rust = |x: f32| if x > 0.0 { x } else { let y = x * 0.5; if y < -1.0 { -1.0 } else { y } };
        let want: Vec<u32> = xs.iter().map(|x| rust(*x).to_bits()).collect();
        assert_eq!(got, want);
        // Sin saltos: el modulo es UN bloque, en linea recta.
        let ops = k.opcodes();
        assert!(!ops.contains(&op::OpBranchConditional) && !ops.contains(&op::OpBranch));
        assert_eq!(ops.iter().filter(|o| **o == op::OpLabel).count(), 1);
    }

    /// ** G3: con el oraculo como `Device`, el calculo NO corre la gpu fn: la
    /// escribe, la juzga y la corre spirv -- y lo que el programa escribe es
    /// lo mismo, linea a linea, que con el f32 del calculo.
    #[test]
    fn the_oracle_runs_the_gpu_fn_and_the_program_writes_the_same() {
        let src = "mod main \"x\"\ngpu fn mezcla(a: f32, b: f32) -> f32\n    return (a + b) / 3.0\nfn main()\n    let xs: [f32; 3] = [1.0, 0.1, -7.5]\n    let ys: [f32; 3] = [2.0, 0.2, 1.25]\n    let r = mezcla(xs, ys)\n    for x in r\n        print(round(x, 9))\n";
        let toml = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";
        let mut read = |p: &str| (p == "Titan.toml").then(|| toml.to_string());
        let plain = bmo_titan_front::lower_package("src/main.titan", src, &mut read).unwrap();
        let mut oracle = Oracle::default();
        let ran = bmo_titan_front::lower_package_with("src/main.titan", src, &mut read, Some(&mut oracle)).unwrap();
        assert_eq!(oracle.written.len(), 1, "the oracle wrote and ran the gpu fn");
        assert_eq!(plain.flat, ran.flat);
    }

    #[test]
    fn a_bool_travels_as_a_uint() {
        let src = "mod main \"x\"\ngpu fn grande(x: f32, flojo: bool) -> bool\n    return x > 10.0 and not flojo\nfn main()\n    let xs: [f32; 1] = [1.0]\n    print(1)\n";
        let k = kernel(src, "grande");
        judge(&k).unwrap_or_else(|e| panic!("{}", e));
        let got = run(&k, &[f32s(&[20.0, 20.0, 5.0]), vec![0, 1, 0]]).unwrap();
        assert_eq!(got, vec![1, 0, 0]);
    }
}
