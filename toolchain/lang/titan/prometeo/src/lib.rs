//! # bmo-titan-prometeo -- PROMETEO de TITAN++: el emisor de GPU, para cualquier tarjeta
//!
//! Nivel 11 de TITAN++. Nacio como `bmo-titan-sm86` el 07-10, cuando el
//! propietario quito el SPIR-V (*"gpu - sm86 - luego el juez verifica
//! estrictamente y el 3060"*). Desde LB3 de `docs/plan/PLAN_LAS_LIBRERIAS.md`
//! (08-10) es la mitad de arriba de PROMETEO, la libreria general de la GPU:
//! la *"gpu general"* del propietario, *"que se llevara todo el emisor de GPU
//! para que aplique"*. **No nombra ninguna tarjeta**: las pide por el
//! contrato (`bmo_prometeo::Tarjeta`), y solo quien arma la herramienta
//! (`titan`) dice cuales hay -- hoy, la 3060. La mitad de abajo (el Programa y
//! el contrato) es `platform/shared/prometeo`.
//!
//! ```text
//!    la IR de una gpu fn   (bmo-titan-front: ya juzgada, ya calculada)
//!         |  programa()
//!         v
//!    el PROGRAMA de la casa   el mismo que sale de los DXIL y los SM5 de
//!         |                   PROTON-X, y el que corre el interprete
//!         |
//!         |  CADA tarjeta, AISLADA por completo (el propietario: "TODAS LAS
//!         |  GPU en emisor SON AISLADAS por completo luego el JUEZ procesa
//!         |  cada uno") --
//!         v
//!    su EMISOR      write(): el Programa a SU codigo, para su oraculo y
//!         |         para el viaje (la 3060: el MISMO emisor que usa
//!         |         Cyberpunk, E3..E6)
//!    su JUEZ        judge(): ESTRICTO, sobre lo que su emisor escribio
//!         |
//!    su SIMULADOR   run(): una celda por hilo
//!         v
//!    el ORACULO     las celdas que lleva el .bex
//!
//!    ... y el SUPREMO JUEZ no esta aqui: esta en la puerta de la GPU final
//!    (el kernel), y es el mismo juez de esa tarjeta, otra vez (LB8).
//! ```
//!
//! *** POR QUE SIN SPIR-V: el emisor de la 3060 que existe lee el `Programa`
//! (E3, 28-09). Con un escritor de SPIR-V aparte, cada cosa que aprendiera
//! TITAN++ (bucles, vecinos, computo) se aprenderia DOS veces, y lo que
//! abriera ILLAPA no le serviria a Cyberpunk. Con el Programa, es una vez.
//!
//! ** TRES RESPUESTAS PARA CADA CELDA, en cada build y en CADA tarjeta: su
//! simulador (sobre su codigo), el interprete de la casa (sobre el Programa)
//! y el calculo de TITAN++ (sobre la IR). Si dos no dan los mismos bits (o
//! los dos NaN), no hay `.bex`, y el NO dice la entrada y las respuestas.
//!
//! ** SIN SALTOS, como hacia el escritor de SPIR-V: cada `if` es un `Elige`
//! (una gpu fn es pura y sin bucles; los dos lados dan el mismo resultado
//! bit a bit). Los saltos de verdad (E6) llegaran con los bucles (IL1, LB5).
//!
//! > **08-10, LB5:** llegaron. Cada `for` de una gpu fn (con su N escrito,
//! > DL4) es un `Bucle` de verdad -- `RomperSi` en su cabeza y en cada
//! > `break`, `FinBucle` en su paso --, y lo que cruza la vuelta vive en su
//! > CASA, un registro que cada asignacion escribe con su predicado (ver
//! > `straight`). Los `if` siguen en linea recta, tambien dentro de un
//! > bucle; una gpu fn sin bucles sale como antes, byte a byte.
//!
//! ** LOS BOOL, como D3D: dentro del Programa un bool es 0xFFFFFFFF o 0; en
//! las celdas, 1 o 0 (lo del calculo). Se convierte al entrar y al salir.
//!
//! ** LA DIVISION: una division entre una POTENCIA DE DOS es exacta como
//! multiplicacion por su inverso (`x / 2.0` y `x * 0.5` son el mismo numero
//! real, redondeado igual), y se escribe asi para cualquier tarjeta. La
//! general llega a la tarjeta como `Div`; la que no la hace exacta lo dice
//! como su LIMITE (la 3060: LI2g de `PLAN_EL_LIBRETO.md`), y es el NO del
//! programa, en su linea y su columna (LB1).

use bmo_prometeo::programa::{Comparacion, Op, OpEntera, Reg};
use bmo_prometeo::{Codigo, NoEmite, Para, Programa, Tarjeta};
use bmo_titan_front::calc::DeviceNo;
use bmo_titan_front::ir::{End, Function, Module, Op as IrOp, Value};
use bmo_titan_front::{Code, Message};
use std::collections::HashMap;

/// Lo que un valor ES dentro de una gpu fn: solo hay dos clases.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    F32,
    Bool,
}

/// Un NO del escritor (no del programa: el frontend ya lo juzgo) -- salvo
/// cuando lleva su `Message`: entonces es lo que una tarjeta TODAVIA NO SABE
/// hacer, un limite dicho a proposito (la division general en la 3060, LI2g),
/// y es un NO del PROGRAMA, con su sitio en el modulo (LB1 de
/// `docs/plan/PLAN_LAS_LIBRERIAS.md`). El texto (`.0`) es el de siempre.
#[derive(Debug)]
pub struct Failure(pub String, pub Option<Message>);

impl Failure {
    /// Un fallo del escritor: nunca del programa.
    fn writer(why: String) -> Failure {
        Failure(why, None)
    }

    /// Lo que se le dice al calculo o al build: el limite como el NO del
    /// programa, o el fallo con el nombre de la gpu fn delante.
    pub fn no(self, name: &str) -> DeviceNo {
        match self.1 {
            Some(said) => DeviceNo::Limit(said),
            None => DeviceNo::Failure(format!("`gpu fn {}`: {}", name, self.0)),
        }
    }
}

/// ** LO QUE TITAN++ DICE DESPUES DEL LIMITE DE UNA TARJETA: el QUE y el POR
/// QUE son de la tarjeta (sus palabras); esto es lo de TITAN++, sea cual sea. La
/// division tiene su como: la de una potencia de dos ya llega como
/// multiplicacion (`inverso_exacto`).
const LIMITE_TAMBIEN: &str = "y una gpu fn da los MISMOS bits por su tarjeta, la casa y el calculo (L29)";
const DIVISION_HOY: &str = "Hoy solo divide entre una potencia de dos, que es una multiplicacion exacta; la general espera a LI2g de PLAN_EL_LIBRETO (DL10 de PLAN_LAS_LIBRERIAS)";
const DIVISION_COMO: &str = "entre una potencia de dos se escribe igual (x / 2.0, x / 0.25); las demas, todavia no";
const LIMITE_COMO: &str = "escribela con otras operaciones, o espera a que su tarjeta la sepa hacer";

/// **Una gpu fn escrita para UNA tarjeta**: su Programa, su codigo para el
/// oraculo y para el viaje, de donde salio, y quien lo escribio.
pub struct Kernel<'t> {
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
    /// La tarjeta que lo escribio: la UNICA que lo juzga y lo simula.
    pub tarjeta: &'t dyn Tarjeta,
    /// Lo que corre su simulador (la 3060: entradas en c[1]).
    pub oraculo: Codigo,
    /// Lo que viajaria a la tarjeta (la 3060: entradas ya en registros).
    pub viaje: Codigo,
}

impl Kernel<'_> {
    /// El codigo que viaja, en bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.viaje.bytes
    }
}

/// **Todas las gpu fn de un modulo, en CADA tarjeta de `tarjetas`**,
/// escritas, juzgadas y comprobadas: lo que `titan build` pide antes de
/// escribir un `.bex`. Un NO aqui es del ESCRITOR o del juez, no del
/// programa -- salvo un `DeviceNo::Limit`: lo que una tarjeta todavia no
/// sabe, y eso SI es el NO del programa, en su sitio.
pub fn kernels<'t>(m: &Module, tarjetas: &[&'t dyn Tarjeta]) -> Result<Vec<Kernel<'t>>, DeviceNo> {
    let mut out = Vec::new();
    for (i, f) in m.functions.iter().enumerate().filter(|(_, f)| f.gpu) {
        if tarjetas.is_empty() {
            return Err(DeviceNo::Failure(format!("`gpu fn {}`: {}", f.name, SIN_TARJETAS)));
        }
        for t in tarjetas {
            let k = write(m, i, *t).map_err(|e| e.no(&f.name))?;
            judge(&k).map_err(DeviceNo::Failure)?;
            verify(m, i, &k).map_err(DeviceNo::Failure)?;
            out.push(k);
        }
    }
    Ok(out)
}

/// Lo que se dice si quien arma la herramienta no dio ninguna tarjeta.
const SIN_TARJETAS: &str = "no hay ninguna tarjeta: quien arma la herramienta no dio ninguna libreria de GPU, y una gpu fn no se escribe a ciegas";

fn kind_of(t: &bmo_titan_front::tree::Ty) -> Result<Kind, Failure> {
    match t {
        bmo_titan_front::tree::Ty::F32 => Ok(Kind::F32),
        bmo_titan_front::tree::Ty::Bool => Ok(Kind::Bool),
        other => Err(Failure::writer(format!("una gpu fn con un `{}`: el frontend (gpu.rs) tenia que haberlo dicho", other.name()))),
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
            return Err(Failure::writer("demasiados valores para un Programa".into()));
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
        Value::Local(l, _) => *env.get(l).ok_or_else(|| Failure::writer(format!("el nombre %{} se lee sin valor: el juez tenia que haberlo dicho", l)))?,
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
                // La general: la tarjeta que no la hace exacta lo dice (ver la cabecera).
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
                (o, k) => return Err(Failure::writer(format!("`{}` entre {:?}: el calculo tenia que haberlo dicho", o, k))),
            };
            w.op(o);
            (d, k)
        }
        other => return Err(Failure::writer(format!("un valor que una gpu fn no tiene ({:?}): gpu.rs tenia que haberlo dicho", other))),
    })
}

/// **La gpu fn `func` de `m`, hecha el Programa de la casa.** Devuelve el
/// Programa y el sitio del `.titan` de cada una de sus operaciones. No sabe
/// de tarjetas: es lo mismo para todas.
pub fn programa(m: &Module, func: usize) -> Result<(Programa, Vec<(usize, usize)>), Failure> {
    let f: &Function = &m.functions[func];
    if !f.gpu {
        return Err(Failure::writer(format!("`{}` no es una gpu fn", f.name)));
    }
    let params: Vec<Kind> = f.params.iter().map(|(_, t)| kind_of(t)).collect::<Result<_, _>>()?;
    let ret = kind_of(f.ret.as_ref().ok_or_else(|| Failure::writer(format!("`{}` no devuelve nada", f.name)))?)?;
    if params.len() > 32 {
        return Err(Failure::writer(format!("`{}` recibe {} valores: el Programa lee 32 entradas como mucho", f.name, params.len())));
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

/// ** Un `for` abierto mientras se escribe (LB5): el Programa lo dice como
/// `Bucle` ... `FinBucle`.
struct Lazo {
    /// Su cabeza (la pregunta), su paso (el salto de vuelta) y su salida.
    cabeza: usize,
    paso: usize,
    salida: usize,
    /// Los nombres que este bucle puso en su CASA (los que ya la tenian, de
    /// un bucle de fuera, siguen en la suya).
    casas: Vec<(usize, Reg, Kind)>,
    /// Como se entro: el predicado de fuera y los nombres de entonces.
    pred: Reg,
    env: Env,
    /// Si va dentro de su `Si`: lo de fuera no siempre llega.
    envuelto: bool,
    /// Si dentro hay un `return`.
    vuelve: bool,
    /// Su sitio: el `for`.
    at: (usize, usize),
}

/// La gpu fn: cada bloque con su predicado, cada nombre elegido con `Elige`
/// donde los caminos se juntan, y el resultado elegido entre los `return`.
/// Sin bucles es LINEA RECTA (cada `if` un `Elige`); cada `for` (LB5) es un
/// `Bucle` de verdad, con sus `if` dentro en linea recta otra vez. Devuelve
/// el registro del resultado.
///
/// ** LA CASA: un nombre de antes que el bucle cambia vive, mientras el
/// bucle corre, en UN registro, y cada asignacion lo escribe ALLI, con el
/// predicado de su bloque (`Elige` sobre si mismo; `Copia` si el bloque
/// corre siempre). Asi la casa esta al dia en cada punto de la vuelta: la
/// vuelta de atras no copia nada, y un `break` es un `RomperSi` y ya -- lo
/// que hace cada vuelta crece con lo que se escribe en ella, nunca con
/// cuantos nombres cruzan (la obra de `gpu.rs` la cuenta, y cada tarjeta
/// la corre en su presupuesto).
///
/// ```text
///    antes      casa_v = v        (cada v de antes que el bucle cambia)
///               Si pred           (si lo de fuera no siempre llega)
///    cabeza     Bucle
///               RomperSi no (i < fin)
///    cuerpo     v = ...  ->  casa_v = Elige(p, ..., casa_v)
///               los `if`, en linea recta; un `continue` salta adelante,
///               al paso
///    break      RomperSi p
///    return     valor = Elige(p, r, valor); hecho = hecho | p; RomperSi p
///    paso       FinBucle
///               FinSi
///    salida     v en su casa; si dentro hubo un `return`, lo de despues va
///               con `no hecho` (o un RomperSi hecho, en el bucle de fuera)
/// ```
fn straight(w: &mut Writer, f: &Function, entry_env: Env) -> Result<Reg, Failure> {
    let n = f.blocks.len();
    // ** LOS BUCLES: el salto hacia arriba de cada `for` (su paso) dice su
    // cabeza; todo lo demas salta hacia abajo.
    let arriba = || Failure::writer("un salto hacia arriba que no es el de un `for` con su N escrito: gpu.rs tenia que haberlo dicho (DL4)".into());
    let mut paso_de: Vec<Option<usize>> = vec![None; n];
    for (b, bl) in f.blocks.iter().enumerate() {
        match &bl.end {
            End::Jump(t) if *t <= b => {
                let es_for = matches!(&f.blocks[*t].end, End::Branch { then, other, .. } if *then == *t + 1 && *other == b + 1);
                if !es_for || paso_de[*t].is_some() {
                    return Err(arriba());
                }
                paso_de[*t] = Some(b);
            }
            End::Branch { then, other, .. } if *then <= b || *other <= b => return Err(arriba()),
            _ => {}
        }
    }
    // El `return` dentro de un bucle: el valor, y si ya volvio, en dos
    // registros de toda la gpu fn (empiezan en 0: no volvio todavia).
    let con_vuelta = (0..n).any(|h| paso_de[h].is_some_and(|s| f.blocks[h..=s].iter().any(|bl| matches!(bl.end, End::Return(_)))));
    let vuelta = if con_vuelta { Some((w.reg()?, w.reg()?)) } else { None };
    let mut incoming: Vec<Vec<(Reg, Env)>> = vec![Vec::new(); n];
    let mut returns: Vec<(Reg, Reg)> = Vec::new();
    let mut lazos: Vec<Lazo> = Vec::new();
    // La casa de cada nombre que un bucle abierto cambia.
    let mut casa_de: HashMap<usize, Reg> = HashMap::new();
    for b in 0..n {
        let (pred, mut env) = if b == 0 {
            (w.cierto()?, entry_env.clone())
        } else {
            let edges = std::mem::take(&mut incoming[b]);
            if edges.is_empty() {
                // Ningun camino llega aqui (lo que sigue a un `return` o a un
                // `break`); el paso de un bucle abierto lo cierra igual.
                if lazos.last().is_some_and(|l| l.paso == b) {
                    cerrar(w, &mut lazos, &mut casa_de, &mut incoming, vuelta)?;
                }
                continue;
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
        // La cabeza de un `for`: aqui se abre su `Bucle`.
        let pred = match paso_de[b] {
            Some(s) => {
                let p = abrir(w, f, b, s, pred, &mut env, &mut lazos, &mut casa_de, vuelta.is_some())?;
                p
            }
            None => pred,
        };
        for o in &f.blocks[b].ops {
            match o {
                IrOp::Let { local, value, .. } | IrOp::Set { local, value, .. } => {
                    let (v, k) = eval(w, value, &env)?;
                    match casa_de.get(local) {
                        // Un nombre que cruza la vuelta: en su casa, con el
                        // predicado de este bloque.
                        Some(&casa) => {
                            if v != casa {
                                let cierto = w.cierto()?;
                                w.op(if pred == cierto { Op::Copia { d: casa, a: v } } else { Op::Elige { d: casa, c: pred, a: v, b: casa } });
                            }
                            env.insert(*local, (casa, k));
                        }
                        None => {
                            env.insert(*local, (v, k));
                        }
                    }
                }
                IrOp::Drop { .. } => {}
                other => return Err(Failure::writer(format!("una gpu fn con {:?}: gpu.rs tenia que haberlo dicho", other))),
            }
        }
        match &f.blocks[b].end {
            // El paso: la vuelta de atras.
            End::Jump(t) if *t <= b => cerrar(w, &mut lazos, &mut casa_de, &mut incoming, vuelta)?,
            End::Jump(t) => match lazos.last() {
                // Un `break`: las casas ya estan al dia.
                Some(l) if *t == l.salida => {
                    w.aqui = l.at;
                    w.op(Op::RomperSi { c: pred, si_cero: false });
                }
                Some(l) if *t > l.paso => return Err(Failure::writer("un salto que sale de un bucle sin ser su `break`".into())),
                _ => incoming[*t].push((pred, env)),
            },
            End::Branch { cond, then, at, .. } if lazos.last().is_some_and(|l| l.cabeza == b) => {
                // La pregunta de cada vuelta: si ya no, fuera.
                let (c, _) = eval(w, cond, &env)?;
                w.aqui = *at;
                w.op(Op::RomperSi { c, si_cero: true });
                incoming[*then].push((pred, env));
            }
            End::Branch { cond, then, other, at } => {
                if lazos.last().is_some_and(|l| *then > l.paso || *other > l.paso) {
                    return Err(Failure::writer("un `if` que sale de un bucle".into()));
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
                match (lazos.last(), vuelta) {
                    (None, _) => returns.push((pred, r)),
                    (Some(l), Some((hecho, valor))) => {
                        // Desde dentro de un bucle: se guarda, y fuera.
                        w.aqui = l.at;
                        let ya = w.entera(hecho, pred, OpEntera::O)?;
                        w.op(Op::Elige { d: valor, c: pred, a: r, b: valor });
                        w.op(Op::Copia { d: hecho, a: ya });
                        w.op(Op::RomperSi { c: pred, si_cero: false });
                    }
                    (Some(_), None) => return Err(Failure::writer("un `return` dentro de un bucle sin su registro".into())),
                }
            }
            End::Return(None) => return Err(Failure::writer("un camino sin `return`: el juez tenia que haberlo dicho (T0070)".into())),
        }
    }
    if !lazos.is_empty() {
        return Err(Failure::writer("un bucle sin cerrar".into()));
    }
    let (_, mut acc) = *returns.last().ok_or_else(|| Failure::writer("una gpu fn sin `return`".into()))?;
    for (p, r) in returns.iter().rev().skip(1) {
        acc = w.elige(*p, *r, acc)?;
    }
    // Lo que volvio desde dentro de un bucle va primero: paso antes.
    if let Some((hecho, valor)) = vuelta {
        w.aqui = (f.line, 1);
        acc = w.elige(hecho, valor, acc)?;
    }
    Ok(acc)
}

/// **Abre el `for` de cabeza `h` y paso `s`**, entrando con `pred` y `env`:
/// cada nombre de antes que el bucle cambia va a su casa (si un bucle de
/// fuera no se la dio ya), el bucle va dentro de su `Si` si lo de fuera no
/// siempre llega, y dentro de cada vuelta todo empieza cierto. Devuelve el
/// predicado de la cabeza; `env` queda con los nombres de la cabeza.
#[allow(clippy::too_many_arguments)]
fn abrir(w: &mut Writer, f: &Function, h: usize, s: usize, pred: Reg, env: &mut Env, lazos: &mut Vec<Lazo>, casa_de: &mut HashMap<usize, Reg>, con_vuelta: bool) -> Result<Reg, Failure> {
    let at = match &f.blocks[h].end {
        End::Branch { at, .. } => *at,
        _ => return Err(Failure::writer("la cabeza de un bucle sin su pregunta".into())),
    };
    w.aqui = at;
    let fuera = env.clone();
    // Los que cruzan la vuelta: los de antes que el bucle cambia (TITAN++ no
    // tiene sombras: un `let` de dentro es otro nombre, y muere en la vuelta).
    let mut cambia: Vec<usize> = f.blocks[h..=s].iter().flat_map(|bl| &bl.ops).filter_map(|o| if let IrOp::Set { local, .. } = o { Some(*local) } else { None }).collect();
    cambia.sort_unstable();
    cambia.dedup();
    let mut casas = Vec::new();
    for l in cambia {
        if casa_de.contains_key(&l) {
            continue;
        }
        if let Some(&(r, k)) = fuera.get(&l) {
            let casa = w.reg()?;
            w.op(Op::Copia { d: casa, a: r });
            casas.push((l, casa, k));
            casa_de.insert(l, casa);
            env.insert(l, (casa, k));
        }
    }
    let cierto = w.cierto()?;
    let envuelto = pred != cierto;
    if envuelto {
        w.op(Op::Si { c: pred });
    }
    w.op(Op::Bucle);
    let vuelve = con_vuelta && f.blocks[h..=s].iter().any(|bl| matches!(bl.end, End::Return(_)));
    lazos.push(Lazo { cabeza: h, paso: s, salida: s + 1, casas, pred, env: fuera, envuelto, vuelve, at });
    Ok(cierto)
}

/// **Cierra el bucle de dentro** en su paso: `FinBucle` (y `FinSi`), y la
/// salida con cada nombre que cambio en su casa -- la de este bucle o la de
/// uno de fuera, que siguen al dia.
fn cerrar(w: &mut Writer, lazos: &mut Vec<Lazo>, casa_de: &mut HashMap<usize, Reg>, incoming: &mut [Vec<(Reg, Env)>], vuelta: Option<(Reg, Reg)>) -> Result<(), Failure> {
    let l = lazos.pop().ok_or_else(|| Failure::writer("un salto hacia arriba sin su bucle".into()))?;
    w.aqui = l.at;
    w.op(Op::FinBucle);
    if l.envuelto {
        w.op(Op::FinSi);
    }
    let mut pred = l.pred;
    if let (true, Some((hecho, _))) = (l.vuelve, vuelta) {
        if lazos.is_empty() {
            // Lo de despues, solo si nadie volvio dentro.
            let todo = w.cierto()?;
            let no_hecho = w.entera(hecho, todo, OpEntera::OX)?;
            pred = w.entera(pred, no_hecho, OpEntera::Y)?;
        } else {
            // El bucle de fuera sale tambien.
            w.op(Op::RomperSi { c: hecho, si_cero: false });
        }
    }
    let mut env = l.env;
    for (local, casa, k) in l.casas {
        env.insert(local, (casa, k));
        casa_de.remove(&local);
    }
    incoming[l.salida].push((pred, env));
    Ok(())
}

// ---- a la tarjeta, y su juez ----------------------------------------------------------

/// El sitio del `.titan` de una gpu fn: (fichero, linea de su `gpu fn`).
fn sitio(m: &Module, f: &Function) -> (String, usize) {
    let (file_k, fn_line) = m.sources.place(f.line).unwrap_or((0, f.line));
    let file = if m.sources.0.is_empty() { "main.titan".to_string() } else { m.sources.path(file_k).to_string() };
    (file, fn_line)
}

/// **Escribe la gpu fn `func` de `m` para `tarjeta`**: el Programa, y su
/// codigo en esa tarjeta para su oraculo y para el viaje. Lo que la tarjeta
/// no sabe se dice en el `.titan` -- fichero, linea y columna --: un LIMITE
/// suyo es el NO del programa; un fallo de su emisor, un fallo.
pub fn write<'t>(m: &Module, func: usize, tarjeta: &'t dyn Tarjeta) -> Result<Kernel<'t>, Failure> {
    let f = &m.functions[func];
    let (programa, donde) = programa(m, func)?;
    let params: Vec<Kind> = f.params.iter().map(|(_, t)| kind_of(t)).collect::<Result<_, _>>()?;
    let ret = kind_of(f.ret.as_ref().expect("programa() lo miro"))?;
    let (file, line) = sitio(m, f);
    let en = |para| {
        tarjeta.emitir(&programa, para).map_err(|e| {
            // DONDE, en las lineas del MODULO (las del `at` de la IR: el
            // paquete entero seguido), o la de la gpu fn; el texto dice la
            // del fichero (08-10: antes salia la del modulo con el nombre del
            // fichero, que solo cuadra en la raiz).
            let op = match &e {
                NoEmite::Limite { op, .. } => Some(*op),
                NoEmite::Fallo { op, .. } => *op,
            };
            let at = op.and_then(|i| donde.get(i).copied()).unwrap_or((f.line, 1));
            let (l, c) = (m.sources.place(at.0).map(|(_, l)| l).unwrap_or(at.0), at.1);
            match e {
                NoEmite::Limite { op, que, por_que } => {
                    // ** UN LIMITE, no un fallo: lo que la tarjeta todavia no
                    // sabe, dicho a proposito. Es el NO del programa.
                    let division = matches!(programa.ops.get(op), Some(Op::Div { .. }));
                    let why = if division { format!("{}, {}. {}", por_que, LIMITE_TAMBIEN, DIVISION_HOY) } else { format!("{}, {}", por_que, LIMITE_TAMBIEN) };
                    let said = Message::new(Code::GpuBody, at.0, at.1, &que, &why, if division { DIVISION_COMO } else { LIMITE_COMO });
                    Failure(format!("{}, linea {}, columna {} (gpu fn `{}`): {}", file, l, c, f.name, why), Some(said))
                }
                NoEmite::Fallo { por_que, .. } => Failure::writer(format!("{}, linea {}, columna {} (gpu fn `{}`): {}", file, l, c, f.name, por_que)),
            }
        })
    };
    let oraculo = en(Para::Oraculo)?;
    let viaje = en(Para::Viaje)?;
    Ok(Kernel { name: f.name.clone(), file, line, params, ret, programa, donde, tarjeta, oraculo, viaje })
}

/// **SU juez, ESTRICTO**, sobre los dos codigos: el de la tarjeta que lo
/// escribio y ningun otro (la 3060: las esperas y las barreras de Ampere, los
/// registros, los saltos, y la regla de un cuerpo de app). Su NO, en sus
/// palabras y en la linea de la gpu fn.
pub fn judge(k: &Kernel) -> Result<(), String> {
    let no = |por: String| format!("{}, linea {} (gpu fn `{}`): el juez de {} dijo que no -- {}", k.file, k.line, k.name, k.tarjeta.ficha().nombre, por);
    k.tarjeta.juzgar(&k.oraculo, Para::Oraculo).map_err(&no)?;
    k.tarjeta.juzgar(&k.viaje, Para::Viaje).map_err(&no)
}

// ---- el oraculo: el simulador de la tarjeta, contra la casa y el calculo --------------

/// La celda `i` de cada valor, como la entrada del Programa (componente 0),
/// en bits.
fn entradas(cells: &[Vec<u32>], i: usize) -> Vec<[u32; 4]> {
    cells.iter().map(|c| [c[i], 0, 0, 0]).collect()
}

/// **El ORACULO**: el codigo de la tarjeta, corrido por SU simulador, un hilo
/// por celda. `cells[k]` son las celdas del valor `k` (sus bits).
pub fn run(k: &Kernel, cells: &[Vec<u32>]) -> Result<Vec<u32>, String> {
    let n = cells.first().map(|c| c.len()).unwrap_or(0);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let s = k.tarjeta.simular(&k.oraculo, &entradas(cells, i), 1)?;
        let first = s.first().ok_or_else(|| format!("el simulador de {} no devolvio la salida de `{}`", k.tarjeta.ficha().nombre, k.name))?;
        out.push(first[0]);
    }
    Ok(out)
}

/// La misma gpu fn por el INTERPRETE de la casa, sobre el Programa.
pub fn run_casa(k: &Kernel, cells: &[Vec<u32>]) -> Vec<u32> {
    let n = cells.first().map(|c| c.len()).unwrap_or(0);
    let mut regs = Vec::new();
    (0..n)
        .map(|i| {
            let e: Vec<[f32; 4]> = entradas(cells, i).iter().map(|c| c.map(f32::from_bits)).collect();
            let mut s = [[0.0f32; 4]; 1];
            k.programa.correr(&e, &[], &mut s, &mut regs);
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
    battery_de(params, 0)
}

/// ** LO QUE CORRE LA BATERIA, como mucho (LB5): sus celdas por la OBRA de
/// una (la de `gpu.rs`, que con cada `range` escrito es la misma en todas).
/// Una gpu fn sin bucles (obra de decenas) se prueba como siempre, hasta
/// 4096 casos; una en el tope de una celda (65536), en 18 -- cada valor
/// sigue pasando por sus 18 bordes --, y su bateria no tarda mucho mas que
/// la de las otras.
pub const BATERIA_OBRA: u64 = 1 << 20;

/// La bateria de una gpu fn de esta `obra`: el producto entero si cabe; si
/// no, cada valor recorre los bordes a su propio paso, en tantas celdas como
/// deje [`BATERIA_OBRA`] (nunca menos que los bordes).
pub fn battery_de(params: &[Kind], obra: u64) -> Vec<Vec<u32>> {
    let column = |k: Kind| -> Vec<u32> { if k == Kind::F32 { BORDES.iter().map(|x| x.to_bits()).collect() } else { vec![0, 1] } };
    let columns: Vec<Vec<u32>> = params.iter().map(|k| column(*k)).collect();
    let total: usize = columns.iter().map(|c| c.len()).product();
    let tope = (BATERIA_OBRA / obra.max(1)).clamp(BORDES.len() as u64, 4096) as usize;
    let mut cells: Vec<Vec<u32>> = vec![Vec::new(); params.len()];
    if total <= tope {
        for i in 0..total {
            let mut rest = i;
            for (j, c) in columns.iter().enumerate() {
                cells[j].push(c[rest % c.len()]);
                rest /= c.len();
            }
        }
    } else {
        for i in 0..tope {
            for (j, c) in columns.iter().enumerate() {
                cells[j].push(c[(i * (2 * j + 1) + j) % c.len()]);
            }
        }
    }
    cells
}

/// Los mismos bits, o los dos NaN: una tarjeta no promete la carga de un NaN.
fn same_cell(a: u32, b: u32, k: Kind) -> bool {
    a == b || (k == Kind::F32 && f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan())
}

/// **La comparacion de cada build**: estas celdas por la tarjeta (su
/// simulador), por la casa y por el calculo; la primera que no da lo mismo,
/// dicha con la entrada y las tres respuestas.
fn compare(m: &Module, func: usize, k: &Kernel, cells: &[Vec<u32>], what: &str) -> Result<Vec<u32>, String> {
    let suya = run(k, cells)?;
    let casa = run_casa(k, cells);
    let calc = bmo_titan_front::calc::run_gpu(m, func, cells).map_err(|e| format!("el calculo: {}", e.what))?;
    for i in 0..suya.len() {
        if !same_cell(suya[i], calc[i], k.ret) || !same_cell(casa[i], calc[i], k.ret) {
            let show = |bits: u32, kind: Kind| if kind == Kind::F32 { format!("{:?}", f32::from_bits(bits)) } else { (bits != 0).to_string() };
            let input: Vec<String> = cells.iter().zip(&k.params).map(|(c, kind)| show(c[i], *kind)).collect();
            return Err(format!(
                "{}, linea {} (gpu fn `{}`): con {} ({}) {} da {}, la casa {} y el calculo {} -- una cuenta, varias respuestas: no hay .bex",
                k.file, k.line, k.name, what, input.join(", "), k.tarjeta.ficha().nombre, show(suya[i], k.ret), show(casa[i], k.ret), show(calc[i], k.ret)
            ));
        }
    }
    Ok(suya)
}

/// **La bateria de bordes** de una gpu fn, por los tres.
pub fn verify(m: &Module, func: usize, k: &Kernel) -> Result<usize, String> {
    let cells = battery_de(&k.params, m.functions[func].obra);
    compare(m, func, k, &cells, "la bateria de bordes")?;
    Ok(cells.first().map(|c| c.len()).unwrap_or(0))
}

/// ** EL ORACULO COMO `Device` DEL CALCULO: cada gpu fn se escribe UNA vez en
/// cada tarjeta, se juzga, pasa la bateria, y sus celdas las calculan las
/// tarjetas (las mismas en todas, o no hay `.bex`). Es lo que `titan build`
/// le da al calculo: los resultados que lleva el `.bex` son los del codigo
/// que viajaria a la tarjeta (las de la primera).
pub struct Oracle<'t> {
    tarjetas: Vec<&'t dyn Tarjeta>,
    written: HashMap<usize, Vec<Kernel<'t>>>,
}

impl<'t> Oracle<'t> {
    /// El oraculo, con las tarjetas que diga quien arma la herramienta.
    pub fn new(tarjetas: &[&'t dyn Tarjeta]) -> Self {
        Oracle { tarjetas: tarjetas.to_vec(), written: HashMap::new() }
    }

    /// Cuantas gpu fn escribio (y juzgo) hasta ahora.
    pub fn written(&self) -> usize {
        self.written.len()
    }
}

impl bmo_titan_front::calc::Device for Oracle<'_> {
    fn run(&mut self, m: &Module, func: usize, cells: Vec<Vec<u32>>) -> Result<Vec<u32>, DeviceNo> {
        if !self.written.contains_key(&func) {
            if self.tarjetas.is_empty() {
                return Err(DeviceNo::Failure(SIN_TARJETAS.to_string()));
            }
            let mut ks = Vec::with_capacity(self.tarjetas.len());
            for t in &self.tarjetas {
                // Un limite de la tarjeta es el NO del programa; lo demas, un fallo.
                let k = write(m, func, *t).map_err(|e| match e.1 {
                    Some(said) => DeviceNo::Limit(said),
                    None => DeviceNo::Failure(e.0),
                })?;
                judge(&k).map_err(DeviceNo::Failure)?;
                verify(m, func, &k).map_err(DeviceNo::Failure)?;
                ks.push(k);
            }
            self.written.insert(func, ks);
        }
        // Las celdas REALES del programa, por cada tarjeta, la casa y el calculo.
        let mut first = None;
        for k in &self.written[&func] {
            let celdas = compare(m, func, k, &cells, "las celdas del programa").map_err(DeviceNo::Failure)?;
            first.get_or_insert(celdas);
        }
        Ok(first.expect("al menos una tarjeta: se miro al escribir"))
    }
}

#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_bucles;
