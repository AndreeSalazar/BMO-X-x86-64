//! # bmo-titan-sm86 -- una `gpu fn` de TITAN++, hecha SASS de la 3060
//!
//! Nivel 11 de TITAN++. Reemplaza al escritor de SPIR-V (G2 de
//! `docs/plan/PLAN_EL_CENTAURO.md`) por decision del propietario (07-10):
//! *"gpu - sm86 - luego el juez verifica estrictamente y el 3060"*. La cadena:
//!
//! ```text
//!    la IR de una gpu fn   (bmo-titan-front: ya juzgada, ya calculada)
//!         |  programa()
//!         v
//!    el PROGRAMA de la casa   el mismo que sale de los DXIL y los SM5 de
//!                             PROTON-X, y el que corre el interprete
//!         |  write(): bmo_proton_x_sm86::emitir
//!         v
//!    el SASS de SM86          el MISMO emisor que usa Cyberpunk (E3..E6)
//!         |  judge(): el juez del SASS (ga10x), ESTRICTO
//!         v
//!    la libreria de la 3060   R1..R9: esperas, barreras, registros, saltos
//!         |  run(): el simulador de la 3060, una celda por hilo
//!         v
//!    el ORACULO               las celdas que lleva el .bex
//! ```
//!
//! *** POR QUE SIN SPIR-V: el emisor de la 3060 que existe lee el `Programa`
//! (E3, 28-09). Con un escritor de SPIR-V aparte, cada cosa que aprendiera
//! TITAN++ (bucles, vecinos, computo) se aprenderia DOS veces, y lo que
//! abriera ILLAPA no le serviria a Cyberpunk. Con el Programa, es una vez.
//!
//! ** TRES RESPUESTAS PARA CADA CELDA, en cada build: el simulador de la 3060
//! (sobre el SASS), el interprete de la casa (sobre el Programa) y el
//! calculo de TITAN++ (sobre la IR). Si dos no dan los mismos bits (o los dos
//! NaN), no hay `.bex`, y el NO dice la entrada y las respuestas.
//!
//! ** SIN SALTOS, como hacia el escritor de SPIR-V: cada `if` es un `Elige`
//! (una gpu fn es pura y sin bucles; los dos lados dan el mismo resultado
//! bit a bit). Los saltos de verdad (E6) llegaran con los bucles (IL1).
//!
//! ** LOS BOOL, como D3D: dentro del Programa un bool es 0xFFFFFFFF o 0; en
//! las celdas, 1 o 0 (lo del calculo). Se convierte al entrar y al salir.
//!
//! ** LA DIVISION: la de la 3060 (MUFU.RCP y FMUL) no es la exacta, y el
//! emisor la rechaza (LI2g de `PLAN_EL_LIBRETO.md`, decision pendiente). Una
//! division entre una POTENCIA DE DOS si es exacta como multiplicacion por su
//! inverso (`x / 2.0` y `x * 0.5` son el mismo numero real, redondeado igual):
//! esa se escribe asi. Las demas, NO con su linea.

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_de_app, juzgar_drenado, Contexto, RESERVADOS};
use bmo_proton_x::dxil::programa::{Comparacion, Op, OpEntera, Programa, Reg};
use bmo_proton_x_sm86::simula::{correr, Maquina};
use bmo_proton_x_sm86::{emitir_con, Abi, Emitido, NoEmite};
use bmo_titan_front::ir::{End, Function, Module, Op as IrOp, Value};
use std::collections::HashMap;

/// Los registros que se le dan: los de un hueco de la tuberia de la 3060
/// (`tuberia::REGISTROS`).
pub const REGISTROS: u32 = 64;

/// Lo que un valor ES dentro de una gpu fn: solo hay dos clases.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    F32,
    Bool,
}

/// Un NO del escritor (no del programa: el frontend ya lo juzgo).
#[derive(Debug)]
pub struct Failure(pub String);

/// **Una gpu fn escrita**: su Programa, su SASS en los dos ABI, y de donde salio.
pub struct Kernel {
    pub name: String,
    /// El fichero del paquete donde esta la gpu fn, y su linea alli.
    pub file: String,
    pub line: usize,
    /// La clase de cada valor (la entrada `k` del Programa), y la del resultado.
    pub params: Vec<Kind>,
    pub ret: Kind,
    pub programa: Programa,
    /// El sitio del `.titan` de cada operacion del Programa.
    pub donde: Vec<(usize, usize)>,
    /// El SASS con el ABI del banco (entradas en c[1]): lo que corre el oraculo.
    pub banco: Emitido,
    /// El SASS con el ABI de la 3060 (entradas ya en registros): lo que viaja.
    pub app: Emitido,
}

impl Kernel {
    /// Las instrucciones de 128 bits del SASS que viaja, en bytes.
    pub fn bytes(&self) -> Vec<u8> {
        self.app.codigo.iter().flat_map(|(lo, hi)| lo.to_le_bytes().into_iter().chain(hi.to_le_bytes())).collect()
    }
}

/// **Todas las gpu fn de un modulo**, escritas, juzgadas y comprobadas: lo
/// que `titan build` pide antes de escribir un `.bex`. Un NO aqui es del
/// ESCRITOR o del juez, no del programa.
pub fn kernels(m: &Module) -> Result<Vec<Kernel>, String> {
    let mut out = Vec::new();
    for (i, f) in m.functions.iter().enumerate().filter(|(_, f)| f.gpu) {
        let k = write(m, i).map_err(|e| format!("`gpu fn {}`: {}", f.name, e.0))?;
        judge(&k)?;
        verify(m, i, &k)?;
        out.push(k);
    }
    Ok(out)
}

fn kind_of(t: &bmo_titan_front::tree::Ty) -> Result<Kind, Failure> {
    match t {
        bmo_titan_front::tree::Ty::F32 => Ok(Kind::F32),
        bmo_titan_front::tree::Ty::Bool => Ok(Kind::Bool),
        other => Err(Failure(format!("una gpu fn con un `{}`: el frontend (gpu.rs) tenia que haberlo dicho", other.name()))),
    }
}

// ---- de la IR al Programa ----------------------------------------------------------

/// El que escribe el Programa: los registros, sus constantes y el sitio de
/// cada operacion.
struct Writer {
    ops: Vec<Op>,
    donde: Vec<(usize, usize)>,
    iniciales: Vec<f32>,
    consts: HashMap<u32, Reg>,
    aqui: (usize, usize),
}

impl Writer {
    fn reg(&mut self) -> Result<Reg, Failure> {
        let r = self.iniciales.len();
        if r >= u16::MAX as usize {
            return Err(Failure("demasiados valores para un Programa".into()));
        }
        self.iniciales.push(0.0);
        Ok(r as Reg)
    }

    /// Un registro con estos BITS al empezar (el emisor los pone como inmediato).
    fn bits(&mut self, b: u32) -> Result<Reg, Failure> {
        if let Some(&r) = self.consts.get(&b) {
            return Ok(r);
        }
        let r = self.reg()?;
        self.iniciales[r as usize] = f32::from_bits(b);
        self.consts.insert(b, r);
        Ok(r)
    }

    fn op(&mut self, o: Op) {
        self.ops.push(o);
        self.donde.push(self.aqui);
    }

    /// `d = c ? a : b` (c es un bool de D3D).
    fn elige(&mut self, c: Reg, a: Reg, b: Reg) -> Result<Reg, Failure> {
        if a == b {
            return Ok(a);
        }
        let d = self.reg()?;
        self.op(Op::Elige { d, c, a, b });
        Ok(d)
    }

    fn entera(&mut self, a: Reg, b: Reg, op: OpEntera) -> Result<Reg, Failure> {
        let d = self.reg()?;
        self.op(Op::Entera { d, a, b, op });
        Ok(d)
    }

    fn cierto(&mut self) -> Result<Reg, Failure> {
        self.bits(0xFFFF_FFFF)
    }
}

/// **El inverso EXACTO de `b`**, si `b` es una potencia de dos (con signo)
/// cuyo inverso tambien cabe en un f32: entonces `x / b` y `x * inverso` son
/// el mismo numero real y se redondean igual, en cada `x`.
pub fn inverso_exacto(b: u32) -> Option<u32> {
    let v = f32::from_bits(b);
    if !v.is_normal() || b & 0x007F_FFFF != 0 {
        return None;
    }
    let r = 1.0f32 / v;
    (r.is_normal() && (r * v) == 1.0).then(|| r.to_bits())
}

type Env = HashMap<usize, (Reg, Kind)>;

fn eval(w: &mut Writer, v: &Value, env: &Env) -> Result<(Reg, Kind), Failure> {
    let at = v.at();
    Ok(match v {
        Value::F32(bits, _) => (w.bits(*bits)?, Kind::F32),
        Value::Bool(b, _) => (if *b { w.cierto()? } else { w.bits(0)? }, Kind::Bool),
        Value::Local(l, _) => *env.get(l).ok_or_else(|| Failure(format!("el nombre %{} se lee sin valor: el juez tenia que haberlo dicho", l)))?,
        Value::Neg(x, _) => {
            // El signo, por sus bits: lo mismo que el calculo, tambien con -0 y NaN.
            let (x, k) = eval(w, x, env)?;
            w.aqui = at;
            let signo = w.bits(0x8000_0000)?;
            (w.entera(x, signo, OpEntera::OX)?, k)
        }
        Value::Not(x, _) => {
            let (x, _) = eval(w, x, env)?;
            w.aqui = at;
            let todo = w.cierto()?;
            (w.entera(x, todo, OpEntera::OX)?, Kind::Bool)
        }
        Value::Bin(o, l, r, _) => {
            let (a, ka) = eval(w, l, env)?;
            // La division entre una potencia de dos: por su inverso, exacta.
            if *o == "/" {
                if let Value::F32(b, _) = **r {
                    if let Some(inv) = inverso_exacto(b) {
                        let i = w.bits(inv)?;
                        w.aqui = at;
                        let d = w.reg()?;
                        w.op(Op::Mul { d, a, b: i });
                        return Ok((d, Kind::F32));
                    }
                }
            }
            let (b, _) = eval(w, r, env)?;
            w.aqui = at;
            let d = w.reg()?;
            let cmp = |como| Op::Compara { d, a, b, como, entero: false };
            let cmp_bits = |como| Op::Compara { d, a, b, como, entero: true };
            let (o, k) = match (*o, ka) {
                ("+", Kind::F32) => (Op::Add { d, a, b }, Kind::F32),
                ("-", Kind::F32) => (Op::Sub { d, a, b }, Kind::F32),
                ("*", Kind::F32) => (Op::Mul { d, a, b }, Kind::F32),
                // La general: el emisor la rechaza con su NO (ver la cabecera).
                ("/", Kind::F32) => (Op::Div { d, a, b }, Kind::F32),
                // Las de Rust y las del calculo: `!=` es cierto con un NaN (D3D `ne`).
                ("==", Kind::F32) => (cmp(Comparacion::Igual), Kind::Bool),
                ("!=", Kind::F32) => (cmp(Comparacion::Distinto), Kind::Bool),
                ("<", Kind::F32) => (cmp(Comparacion::Menor), Kind::Bool),
                ("<=", Kind::F32) => (cmp(Comparacion::MenorIgual), Kind::Bool),
                (">", Kind::F32) => (cmp(Comparacion::Mayor), Kind::Bool),
                (">=", Kind::F32) => (cmp(Comparacion::MayorIgual), Kind::Bool),
                ("and", Kind::Bool) => (Op::Entera { d, a, b, op: OpEntera::Y }, Kind::Bool),
                ("or", Kind::Bool) => (Op::Entera { d, a, b, op: OpEntera::O }, Kind::Bool),
                ("==", Kind::Bool) => (cmp_bits(Comparacion::Igual), Kind::Bool),
                ("!=", Kind::Bool) => (cmp_bits(Comparacion::Distinto), Kind::Bool),
                (o, k) => return Err(Failure(format!("`{}` entre {:?}: el calculo tenia que haberlo dicho", o, k))),
            };
            w.op(o);
            (d, k)
        }
        other => return Err(Failure(format!("un valor que una gpu fn no tiene ({:?}): gpu.rs tenia que haberlo dicho", other))),
    })
}

/// **La gpu fn `func` de `m`, hecha el Programa de la casa.** Devuelve el
/// Programa y el sitio del `.titan` de cada una de sus operaciones.
pub fn programa(m: &Module, func: usize) -> Result<(Programa, Vec<(usize, usize)>), Failure> {
    let f: &Function = &m.functions[func];
    if !f.gpu {
        return Err(Failure(format!("`{}` no es una gpu fn", f.name)));
    }
    let params: Vec<Kind> = f.params.iter().map(|(_, t)| kind_of(t)).collect::<Result<_, _>>()?;
    let ret = kind_of(f.ret.as_ref().ok_or_else(|| Failure(format!("`{}` no devuelve nada", f.name)))?)?;
    if params.len() > 32 {
        return Err(Failure(format!("`{}` recibe {} valores: el Programa lee 32 entradas como mucho", f.name, params.len())));
    }
    let mut w = Writer { ops: Vec::new(), donde: Vec::new(), iniciales: Vec::new(), consts: HashMap::new(), aqui: (f.line, 1) };
    // -- la celda de cada valor: la entrada k, componente 0 ------------------------
    let mut env: Env = HashMap::new();
    for (k, ((local, _), kind)) in f.params.iter().zip(&params).enumerate() {
        let d = w.reg()?;
        w.op(Op::Entrada { d, elemento: k as u8, componente: 0 });
        let v = if *kind == Kind::Bool {
            // La celda trae 1 o 0; dentro, un bool de D3D.
            let cero = w.bits(0)?;
            let b = w.reg()?;
            w.op(Op::Compara { d: b, a: d, b: cero, como: Comparacion::Distinto, entero: true });
            b
        } else {
            d
        };
        env.insert(*local, (v, *kind));
    }
    let result = straight(&mut w, f, env)?;
    // -- el resultado: la salida 0; un bool sale como 1 o 0 -------------------------
    w.aqui = (f.line, 1);
    let s = if ret == Kind::Bool {
        let (uno, cero) = (w.bits(1)?, w.bits(0)?);
        w.elige(result, uno, cero)?
    } else {
        result
    };
    w.op(Op::Salida { s, elemento: 0, componente: 0 });
    let lee = if params.len() == 32 { u32::MAX } else { (1u32 << params.len()) - 1 };
    let p = Programa {
        ops: w.ops,
        iniciales: w.iniciales,
        entradas: params.len(),
        salidas: 1,
        lee,
        filas_cb: 0,
        ranuras: Default::default(),
        computo: Default::default(),
    };
    Ok((p, w.donde))
}

/// La gpu fn en LINEA RECTA: cada bloque con su predicado, cada nombre elegido
/// con `Elige` donde los caminos se juntan, y el resultado elegido entre los
/// `return`. Devuelve el registro del resultado.
fn straight(w: &mut Writer, f: &Function, entry_env: Env) -> Result<Reg, Failure> {
    let n = f.blocks.len();
    let mut incoming: Vec<Vec<(Reg, Env)>> = vec![Vec::new(); n];
    let mut returns: Vec<(Reg, Reg)> = Vec::new();
    for b in 0..n {
        let (pred, mut env) = if b == 0 {
            (w.cierto()?, entry_env.clone())
        } else {
            let edges = std::mem::take(&mut incoming[b]);
            if edges.is_empty() {
                continue; // ningun camino llega aqui (lo que sigue a un `return`)
            }
            let mut pred = edges[0].0;
            for (p, _) in &edges[1..] {
                pred = w.entera(pred, *p, OpEntera::O)?;
            }
            let mut env: Env = HashMap::new();
            let mut names: Vec<usize> = edges.iter().flat_map(|(_, e)| e.keys().copied()).collect();
            names.sort_unstable();
            names.dedup();
            for l in names {
                let mut acc: Option<(Reg, Kind)> = None;
                for (p, e) in &edges {
                    if let Some(&(v, k)) = e.get(&l) {
                        acc = Some(match acc {
                            None => (v, k),
                            Some((a, _)) => (w.elige(*p, v, a)?, k),
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
                IrOp::Let { local, value, .. } | IrOp::Set { local, value, .. } => {
                    let v = eval(w, value, &env)?;
                    env.insert(*local, v);
                }
                IrOp::Drop { .. } => {}
                other => return Err(Failure(format!("una gpu fn con {:?}: gpu.rs tenia que haberlo dicho", other))),
            }
        }
        match &f.blocks[b].end {
            End::Jump(t) => {
                if *t <= b {
                    return Err(Failure("un salto hacia arriba: una gpu fn no tiene bucles (todavia: IL1)".into()));
                }
                incoming[*t].push((pred, env));
            }
            End::Branch { cond, then, other, at } => {
                if *then <= b || *other <= b {
                    return Err(Failure("un salto hacia arriba: una gpu fn no tiene bucles (todavia: IL1)".into()));
                }
                let (c, _) = eval(w, cond, &env)?;
                w.aqui = *at;
                let yes = w.entera(pred, c, OpEntera::Y)?;
                let todo = w.cierto()?;
                let not_c = w.entera(c, todo, OpEntera::OX)?;
                let no = w.entera(pred, not_c, OpEntera::Y)?;
                incoming[*then].push((yes, env.clone()));
                incoming[*other].push((no, env));
            }
            End::Return(Some(v)) => {
                let (r, _) = eval(w, v, &env)?;
                returns.push((pred, r));
            }
            End::Return(None) => return Err(Failure("un camino sin `return`: el juez tenia que haberlo dicho (T0070)".into())),
        }
    }
    let (_, mut acc) = *returns.last().ok_or_else(|| Failure("una gpu fn sin `return`".into()))?;
    for (p, r) in returns.iter().rev().skip(1) {
        acc = w.elige(*p, *r, acc)?;
    }
    Ok(acc)
}

// ---- al SASS, y el juez ---------------------------------------------------------------

/// El sitio del `.titan` de una gpu fn: (fichero, linea de su `gpu fn`).
fn sitio(m: &Module, f: &Function) -> (String, usize) {
    let (file_k, fn_line) = m.sources.place(f.line).unwrap_or((0, f.line));
    let file = if m.sources.0.is_empty() { "main.titan".to_string() } else { m.sources.path(file_k).to_string() };
    (file, fn_line)
}

/// **Escribe la gpu fn `func` de `m`**: el Programa y su SASS de SM86, con el
/// MISMO emisor que usa PROTON-X, en los dos ABI.
pub fn write(m: &Module, func: usize) -> Result<Kernel, Failure> {
    let f = &m.functions[func];
    let (programa, donde) = programa(m, func)?;
    let params: Vec<Kind> = f.params.iter().map(|(_, t)| kind_of(t)).collect::<Result<_, _>>()?;
    let ret = kind_of(f.ret.as_ref().expect("programa() lo miro"))?;
    let (file, line) = sitio(m, f);
    let en = |abi| {
        emitir_con(&programa, REGISTROS, abi).map_err(|e| {
            let (l, c) = match e {
                NoEmite::Operacion(i) => donde.get(i).copied().unwrap_or((line, 1)),
                _ => (line, 1),
            };
            let por_que = match (e, programa.ops.get(match e { NoEmite::Operacion(i) => i, _ => usize::MAX })) {
                (_, Some(Op::Div { .. })) => "la division de la 3060 (MUFU.RCP y FMUL) no es la exacta: solo entre una potencia de dos, que es una multiplicacion exacta. Las demas esperan a LI2g de PLAN_EL_LIBRETO".to_string(),
                (NoEmite::Registros, _) => format!("no cabe en los {} registros de un hueco de la 3060", REGISTROS),
                (e, _) => format!("el emisor de la 3060 dijo que no: {:?}", e),
            };
            Failure(format!("{}, linea {}, columna {} (gpu fn `{}`): {}", file, l, c, f.name, por_que))
        })
    };
    let banco = en(Abi::Banco)?;
    let app = en(Abi::Registros)?;
    Ok(Kernel { name: f.name.clone(), file, line, params, ret, programa, donde, banco, app })
}

/// **El juez del SASS, ESTRICTO**: la libreria de lo que sabe la 3060 (las
/// esperas y las barreras de Ampere, los registros, los saltos) sobre los dos
/// SASS, y la regla de un cuerpo de app (R7: ni una lectura de memoria que no
/// cargue el pegamento). Su NO, en sus palabras y en la linea de la gpu fn.
pub fn judge(k: &Kernel) -> Result<(), String> {
    let no = |por: String| format!("{}, linea {} (gpu fn `{}`): el juez de la 3060 dijo que no -- {}", k.file, k.line, k.name, por);
    for e in [&k.banco, &k.app] {
        juzgar_drenado(&e.codigo, &Contexto { registros: e.registros + RESERVADOS, sph: None }).map_err(|b| no(format!("{}", b)))?;
    }
    juzgar_cuerpo_de_app(&k.app.codigo, k.app.registros).map_err(|b| no(format!("{}", b)))
}

// ---- el oraculo: el simulador de la 3060, contra la casa y el calculo -----------------

/// La celda `i` de cada valor, como la entrada del Programa (componente 0).
fn entradas(cells: &[Vec<u32>], i: usize) -> Vec<[f32; 4]> {
    cells.iter().map(|c| [f32::from_bits(c[i]), 0.0, 0.0, 0.0]).collect()
}

/// **El ORACULO**: el SASS corrido por el simulador de la 3060, un hilo por
/// celda. `cells[k]` son las celdas del valor `k` (sus bits).
pub fn run(k: &Kernel, cells: &[Vec<u32>]) -> Result<Vec<u32>, String> {
    let n = cells.first().map(|c| c.len()).unwrap_or(0);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let banco: Vec<u8> = entradas(cells, i).iter().flat_map(|e| e.iter().flat_map(|x| x.to_le_bytes())).collect();
        let mut m = Maquina::nueva([&[], &banco, &[], &[], &[], &[], &[], &[]]);
        correr(&k.banco.codigo, &mut m).map_err(|e| format!("el simulador de la 3060: {:?}", e))?;
        out.push(m.r[0]);
    }
    Ok(out)
}

/// La misma gpu fn por el INTERPRETE de la casa, sobre el Programa.
pub fn run_casa(k: &Kernel, cells: &[Vec<u32>]) -> Vec<u32> {
    let n = cells.first().map(|c| c.len()).unwrap_or(0);
    let mut regs = Vec::new();
    (0..n)
        .map(|i| {
            let mut s = [[0.0f32; 4]; 1];
            k.programa.correr(&entradas(cells, i), &[], &mut s, &mut regs);
            s[0][0].to_bits()
        })
        .collect()
}

/// ** SI ESTA BIEN: las celdas que mas fallan en una cuenta de coma flotante.
const BORDES: [f32; 18] = [
    0.0, -0.0,                       // el signo del cero: 1/0 y 1/-0 no son lo mismo
    1.0, -1.0, 0.5, -2.5, 3.0,       // los de todos los dias
    0.1,                             // el que no cabe en base 2
    1.0e-38, f32::MIN_POSITIVE,      // los normales mas chicos
    1.0e-45, -1.0e-45,               // los subnormales: donde se pierde precision
    f32::MAX, f32::MIN, 1.0e30,      // los que desbordan al sumar
    f32::NAN, f32::INFINITY, f32::NEG_INFINITY,
];

/// Las entradas de la bateria: el producto entero si cabe en 4096 casos; si
/// no, cada valor recorre los bordes a su propio paso.
pub fn battery(params: &[Kind]) -> Vec<Vec<u32>> {
    let column = |k: Kind| -> Vec<u32> { if k == Kind::F32 { BORDES.iter().map(|x| x.to_bits()).collect() } else { vec![0, 1] } };
    let columns: Vec<Vec<u32>> = params.iter().map(|k| column(*k)).collect();
    let total: usize = columns.iter().map(|c| c.len()).product();
    let mut cells: Vec<Vec<u32>> = vec![Vec::new(); params.len()];
    if total <= 4096 {
        for i in 0..total {
            let mut rest = i;
            for (j, c) in columns.iter().enumerate() {
                cells[j].push(c[rest % c.len()]);
                rest /= c.len();
            }
        }
    } else {
        for i in 0..4096 {
            for (j, c) in columns.iter().enumerate() {
                cells[j].push(c[(i * (2 * j + 1) + j) % c.len()]);
            }
        }
    }
    cells
}

/// Los mismos bits, o los dos NaN: la 3060 no promete la carga de un NaN.
fn same_cell(a: u32, b: u32, k: Kind) -> bool {
    a == b || (k == Kind::F32 && f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan())
}

/// **La comparacion de cada build**: estas celdas por la 3060 simulada, por
/// la casa y por el calculo; la primera que no da lo mismo, dicha con la
/// entrada y las tres respuestas.
fn compare(m: &Module, func: usize, k: &Kernel, cells: &[Vec<u32>], what: &str) -> Result<Vec<u32>, String> {
    let sass = run(k, cells)?;
    let casa = run_casa(k, cells);
    let calc = bmo_titan_front::calc::run_gpu(m, func, cells).map_err(|e| format!("el calculo: {}", e.what))?;
    for i in 0..sass.len() {
        if !same_cell(sass[i], calc[i], k.ret) || !same_cell(casa[i], calc[i], k.ret) {
            let show = |bits: u32, kind: Kind| if kind == Kind::F32 { format!("{:?}", f32::from_bits(bits)) } else { (bits != 0).to_string() };
            let input: Vec<String> = cells.iter().zip(&k.params).map(|(c, kind)| show(c[i], *kind)).collect();
            return Err(format!(
                "{}, linea {} (gpu fn `{}`): con {} ({}) la 3060 da {}, la casa {} y el calculo {} -- una cuenta, varias respuestas: no hay .bex",
                k.file, k.line, k.name, what, input.join(", "), show(sass[i], k.ret), show(casa[i], k.ret), show(calc[i], k.ret)
            ));
        }
    }
    Ok(sass)
}

/// **La bateria de bordes** de una gpu fn, por los tres.
pub fn verify(m: &Module, func: usize, k: &Kernel) -> Result<usize, String> {
    let cells = battery(&k.params);
    compare(m, func, k, &cells, "la bateria de bordes")?;
    Ok(cells.first().map(|c| c.len()).unwrap_or(0))
}

/// ** EL ORACULO COMO `Device` DEL CALCULO: cada gpu fn se escribe UNA vez,
/// se juzga, pasa la bateria, y sus celdas las calcula la 3060 simulada. Es
/// lo que `titan build` le da al calculo: los resultados que lleva el `.bex`
/// son los del SASS que viajaria a la tarjeta.
#[derive(Default)]
pub struct Oracle {
    written: HashMap<usize, Kernel>,
}

impl Oracle {
    /// Cuantas gpu fn escribio (y juzgo) hasta ahora.
    pub fn written(&self) -> usize {
        self.written.len()
    }
}

impl bmo_titan_front::calc::Device for Oracle {
    fn run(&mut self, m: &Module, func: usize, cells: Vec<Vec<u32>>) -> Result<Vec<u32>, String> {
        if !self.written.contains_key(&func) {
            let k = write(m, func).map_err(|e| e.0)?;
            judge(&k)?;
            verify(m, func, &k)?;
            self.written.insert(func, k);
        }
        // Las celdas REALES del programa, por los tres.
        compare(m, func, &self.written[&func], &cells, "las celdas del programa")
    }
}

#[cfg(test)]
mod pruebas;
