//! **LA gpu fn HECHA PROGRAMA** -- el escritor de PROMETEO de TITAN++: de la
//! IR de una gpu fn (ya juzgada, ya calculada) al Programa de la casa. No
//! sabe de tarjetas: lo que sale es lo mismo para todas.
//!
//! ```text
//!    eval        un valor de la IR, sus operaciones; lo que vale, un `Valor`
//!    straight    el cuerpo: cada bloque con su predicado, cada `if` un
//!                `Elige`, cada `for` un `Bucle` (LB5), los nombres que
//!                cruzan la vuelta en su CASA
//!    celda       la gpu fn de CELDAS (nivel 11): sus valores en la entrada
//!                k, su resultado en la salida 0
//! ```
//!
//! La gpu fn que DIBUJA (LB6) usa este mismo escritor; lo suyo -- sus
//! registros por elementos y componentes -- esta en `dibujo.rs`.
//!
//! Salio de `lib.rs` el 08-10, a mitad de LB6 (*"que administre archivos
//! multiples bien organizado"*): ni un byte de lo que escribe cambio.

use bmo_prometeo::programa::{Comparacion, Op, OpEntera, Reg};
use bmo_prometeo::Programa;
use bmo_titan_front::gpu::campos;
use bmo_titan_front::ir::{End, Function, Module, Op as IrOp, PathStep, Value};
use bmo_titan_front::tree::Ty;
use std::collections::HashMap;

use crate::{kind_of, Failure, Kind};

/// ** LO QUE VALE un nombre dentro de una gpu fn: un f32 o un bool, en UN
/// registro; o (LB6) un REGISTRO de dibujo: su tipo (su numero en el modulo)
/// y sus f32, campo a campo y componente a componente, cada uno en su
/// registro -- sus HOJAS. Elegir un campo no es una operacion: es tomar sus
/// hojas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Valor {
    Uno(Reg, Kind),
    Varios(usize, Vec<Reg>),
}

impl Valor {
    /// Sus registros, en orden.
    pub(crate) fn hojas(&self) -> &[Reg] {
        match self {
            Valor::Uno(r, _) => std::slice::from_ref(r),
            Valor::Varios(_, hojas) => hojas,
        }
    }

    /// Un f32 o un bool, donde no cabe otra cosa (una cuenta, una pregunta).
    fn uno(&self) -> Result<(Reg, Kind), Failure> {
        match self {
            Valor::Uno(r, k) => Ok((*r, *k)),
            Valor::Varios(..) => Err(Failure::writer("un registro donde va un numero: gpu.rs y el calculo tenian que haberlo dicho".into())),
        }
    }

    /// Lo mismo, con otras hojas.
    fn con(&self, hojas: Vec<Reg>) -> Valor {
        match self {
            Valor::Uno(_, k) => Valor::Uno(hojas[0], *k),
            Valor::Varios(t, _) => Valor::Varios(*t, hojas),
        }
    }
}

/// Cuantas hojas lleva un valor de este tipo: 1 un f32 o un bool; un
/// registro de dibujo, los componentes de sus campos (`gpu::campos`).
pub(crate) fn hojas_de(m: &Module, ty: &Ty) -> Result<usize, Failure> {
    match ty {
        Ty::F32 | Ty::Bool => Ok(1),
        _ => campos(&m.types, ty).map(|cs| cs.iter().map(|(_, k)| k).sum()).ok_or_else(|| Failure::writer(format!("un `{}` en una gpu fn: gpu.rs tenia que haberlo dicho", ty.name()))),
    }
}

/// El numero del tipo `ty` en el modulo, si es un registro.
fn tipo_de(m: &Module, ty: &Ty) -> Option<usize> {
    let Ty::Named(n) = ty else { return None };
    m.types.iter().position(|t| &t.name == n)
}

/// **Un valor del tipo `ty` con estas hojas.**
pub(crate) fn valor_de(m: &Module, ty: &Ty, hojas: Vec<Reg>) -> Result<Valor, Failure> {
    match (ty, tipo_de(m, ty)) {
        (Ty::F32 | Ty::Bool, _) => Ok(Valor::Uno(hojas[0], kind_of(ty)?)),
        (_, Some(t)) => Ok(Valor::Varios(t, hojas)),
        _ => Err(Failure::writer(format!("un `{}` en una gpu fn: gpu.rs tenia que haberlo dicho", ty.name()))),
    }
}

/// El campo `nombre` del registro de tipo `t`: desde que hoja empieza,
/// cuantas lleva, y su tipo.
fn campo<'m>(m: &'m Module, t: usize, nombre: &str) -> Result<(usize, usize, &'m Ty), Failure> {
    let mut desde = 0;
    for c in &m.types[t].fields {
        let n = hojas_de(m, &c.ty)?;
        if c.name == nombre {
            return Ok((desde, n, &c.ty));
        }
        desde += n;
    }
    Err(Failure::writer(format!("`{}` no tiene el campo `{}`: el juez tenia que haberlo dicho", m.types[t].name, nombre)))
}

/// El que escribe el Programa: los registros, sus constantes y el sitio de
/// cada operacion.
pub(crate) struct Writer<'m> {
    /// El modulo: de donde sale el cuerpo de una gpu fn llamada (LB5) y los
    /// campos de un registro (LB6).
    pub(crate) m: &'m Module,
    /// Las gpu fn que se estan escribiendo EN LINEA ahora mismo, de fuera a
    /// dentro: una que volviera a entrar no acabaria nunca (`gpu.rs` lo dice
    /// antes; esto es la red).
    pub(crate) dentro: Vec<usize>,
    pub(crate) ops: Vec<Op>,
    pub(crate) donde: Vec<(usize, usize)>,
    pub(crate) iniciales: Vec<f32>,
    consts: HashMap<u32, Reg>,
    pub(crate) aqui: (usize, usize),
}

impl<'m> Writer<'m> {
    /// Uno vacio, para la gpu fn `func` de `m`.
    pub(crate) fn nuevo(m: &'m Module, func: usize) -> Writer<'m> {
        let f = &m.functions[func];
        Writer { m, dentro: vec![func], ops: Vec::new(), donde: Vec::new(), iniciales: Vec::new(), consts: HashMap::new(), aqui: (f.line, 1) }
    }

    pub(crate) fn reg(&mut self) -> Result<Reg, Failure> {
        let r = self.iniciales.len();
        if r >= u16::MAX as usize {
            return Err(Failure::writer("demasiados valores para un Programa".into()));
        }
        self.iniciales.push(0.0);
        Ok(r as Reg)
    }

    /// Un registro con estos BITS al empezar (el emisor los pone como inmediato).
    pub(crate) fn bits(&mut self, b: u32) -> Result<Reg, Failure> {
        if let Some(&r) = self.consts.get(&b) {
            return Ok(r);
        }
        let r = self.reg()?;
        self.iniciales[r as usize] = f32::from_bits(b);
        self.consts.insert(b, r);
        Ok(r)
    }

    pub(crate) fn op(&mut self, o: Op) {
        self.ops.push(o);
        self.donde.push(self.aqui);
    }

    /// `d = c ? a : b` (c es un bool de D3D).
    pub(crate) fn elige(&mut self, c: Reg, a: Reg, b: Reg) -> Result<Reg, Failure> {
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

    /// `c ? a : b` hoja a hoja: un registro se elige entero.
    fn elegir(&mut self, c: Reg, a: &Valor, b: &Valor) -> Result<Valor, Failure> {
        let hojas = a.hojas().iter().zip(b.hojas()).map(|(x, y)| self.elige(c, *x, *y)).collect::<Result<_, _>>()?;
        Ok(a.con(hojas))
    }
}

/// **El inverso EXACTO de `b`**, si `b` es una potencia de dos (con signo)
/// cuyo inverso tambien cabe en un f32: entonces `x / b` y `x * inverso` son
/// el mismo numero real y se redondean igual, en cada `x`. 09-10 (DL10): es
/// el del frontend, que cuenta esa division como una multiplicacion en la
/// obra de la celda -- la misma regla en los dos sitios, por construccion.
pub use bmo_titan_front::gpu::inverso_exacto;

/// Lo que vale cada nombre en un punto de la gpu fn.
pub(crate) type Env = HashMap<usize, Valor>;

fn eval(w: &mut Writer, v: &Value, env: &Env) -> Result<Valor, Failure> {
    let at = v.at();
    Ok(match v {
        Value::F32(bits, _) => Valor::Uno(w.bits(*bits)?, Kind::F32),
        Value::Bool(b, _) => Valor::Uno(if *b { w.cierto()? } else { w.bits(0)? }, Kind::Bool),
        Value::Local(l, _) => env.get(l).cloned().ok_or_else(|| Failure::writer(format!("el nombre %{} se lee sin valor: el juez tenia que haberlo dicho", l)))?,
        Value::Neg(x, _) => {
            // El signo, por sus bits: lo mismo que el calculo, tambien con -0 y NaN.
            let (x, k) = eval(w, x, env)?.uno()?;
            w.aqui = at;
            let signo = w.bits(0x8000_0000)?;
            Valor::Uno(w.entera(x, signo, OpEntera::OX)?, k)
        }
        Value::Not(x, _) => {
            let (x, _) = eval(w, x, env)?.uno()?;
            w.aqui = at;
            let todo = w.cierto()?;
            Valor::Uno(w.entera(x, todo, OpEntera::OX)?, Kind::Bool)
        }
        Value::Bin(o, l, r, _) => {
            let (a, ka) = eval(w, l, env)?.uno()?;
            // La division entre una potencia de dos: por su inverso, exacta.
            if *o == "/" {
                if let Value::F32(b, _) = **r {
                    if let Some(inv) = inverso_exacto(b) {
                        let i = w.bits(inv)?;
                        w.aqui = at;
                        let d = w.reg()?;
                        w.op(Op::Mul { d, a, b: i });
                        return Ok(Valor::Uno(d, Kind::F32));
                    }
                }
            }
            let (b, _) = eval(w, r, env)?.uno()?;
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
            Valor::Uno(d, k)
        }
        // ** OTRA gpu fn (LB5), EN LINEA: sus valores, su cuerpo escrito aqui
        // mismo -- con sus bucles y sus `return` --, y su resultado. Un hilo
        // no tiene pila para llamar, y una gpu fn es pura: en linea da los
        // mismos bits.
        Value::Call(g, args, _) => {
            let m = w.m;
            let callee = &m.functions[*g];
            if !callee.gpu {
                return Err(Failure::writer(format!("una llamada a `{}`, de la CPU, dentro de una gpu fn: gpu.rs tenia que haberlo dicho", callee.name)));
            }
            if w.dentro.contains(g) {
                return Err(Failure::writer(format!("`{}` se llama a si misma: gpu.rs tenia que haberlo dicho", callee.name)));
            }
            let ret = callee.ret.as_ref().ok_or_else(|| Failure::writer(format!("`{}` no devuelve nada", callee.name)))?;
            let mut suyo: Env = HashMap::new();
            for ((local, _), a) in callee.params.iter().zip(args) {
                let v = eval(w, a, env)?;
                suyo.insert(*local, v);
            }
            let aqui = w.aqui;
            w.dentro.push(*g);
            let r = straight(w, callee, suyo)?;
            w.dentro.pop();
            w.aqui = aqui;
            valor_de(m, ret, r)?
        }
        // ** LB6: el campo de un registro de dibujo -- sus hojas, sin
        // operacion ninguna.
        Value::Field(base, nombre, _) => match eval(w, base, env)? {
            Valor::Varios(t, hojas) => {
                let (desde, n, ty) = campo(w.m, t, nombre)?;
                valor_de(w.m, ty, hojas[desde..desde + n].to_vec())?
            }
            Valor::Uno(..) => return Err(Failure::writer(format!("`.{}` de un numero: el juez tenia que haberlo dicho", nombre))),
        },
        // ** LB6: un registro escrito, campo a campo en el orden de su `type`
        // (la IR ya los ordeno): sus hojas, una detras de otra.
        Value::Record(t, items, _) => {
            let mut hojas = Vec::new();
            for i in items {
                hojas.extend_from_slice(eval(w, i, env)?.hojas());
            }
            Valor::Varios(*t, hojas)
        }
        other => return Err(Failure::writer(format!("un valor que una gpu fn no tiene ({:?}): gpu.rs tenia que haberlo dicho", other))),
    })
}

/// **La gpu fn de CELDAS `func` de `m`, hecha el Programa de la casa**
/// (nivel 11): la celda de cada valor en la entrada k, componente 0; su
/// resultado en la salida 0. Devuelve el Programa y el sitio del `.titan` de
/// cada una de sus operaciones.
pub(crate) fn celda(m: &Module, func: usize) -> Result<(Programa, Vec<(usize, usize)>), Failure> {
    let f: &Function = &m.functions[func];
    let params: Vec<Kind> = f.params.iter().map(|(_, t)| kind_of(t)).collect::<Result<_, _>>()?;
    let ret = kind_of(f.ret.as_ref().ok_or_else(|| Failure::writer(format!("`{}` no devuelve nada", f.name)))?)?;
    if params.len() > 32 {
        return Err(Failure::writer(format!("`{}` recibe {} valores: el Programa lee 32 entradas como mucho", f.name, params.len())));
    }
    let mut w = Writer::nuevo(m, func);
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
        env.insert(*local, Valor::Uno(v, *kind));
    }
    let result = straight(&mut w, f, env)?[0];
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
    casas: Vec<(usize, Valor)>,
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

/// El `return` desde dentro de un bucle: si ya volvio, y con que hojas (en
/// registros de toda la gpu fn; empiezan en 0: no volvio todavia).
type Vuelta = Option<(Reg, Vec<Reg>)>;

/// La gpu fn: cada bloque con su predicado, cada nombre elegido con `Elige`
/// donde los caminos se juntan, y el resultado elegido entre los `return`.
/// Sin bucles es LINEA RECTA (cada `if` un `Elige`); cada `for` (LB5) es un
/// `Bucle` de verdad, con sus `if` dentro en linea recta otra vez. Devuelve
/// las HOJAS del resultado: un registro para un f32 o un bool; las de un
/// registro de dibujo (LB6), en orden.
///
/// ** LA CASA: un nombre de antes que el bucle cambia vive, mientras el
/// bucle corre, en sus registros, y cada asignacion los escribe ALLI, con el
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
///
/// Un registro de dibujo es lo mismo, hoja a hoja: cada una en su casa, cada
/// una elegida.
pub(crate) fn straight(w: &mut Writer, f: &Function, entry_env: Env) -> Result<Vec<Reg>, Failure> {
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
    // El `return` dentro de un bucle: el valor, y si ya volvio, en registros
    // de toda la gpu fn (empiezan en 0: no volvio todavia).
    let con_vuelta = (0..n).any(|h| paso_de[h].is_some_and(|s| f.blocks[h..=s].iter().any(|bl| matches!(bl.end, End::Return(_)))));
    let vuelta: Vuelta = if con_vuelta {
        let hecho = w.reg()?;
        let ret = f.ret.as_ref().ok_or_else(|| Failure::writer(format!("`{}` no devuelve nada", f.name)))?;
        let valor = (0..hojas_de(w.m, ret)?).map(|_| w.reg()).collect::<Result<_, _>>()?;
        Some((hecho, valor))
    } else {
        None
    };
    let mut incoming: Vec<Vec<(Reg, Env)>> = vec![Vec::new(); n];
    let mut returns: Vec<(Reg, Vec<Reg>)> = Vec::new();
    let mut lazos: Vec<Lazo> = Vec::new();
    // La casa de cada nombre que un bucle abierto cambia.
    let mut casa_de: HashMap<usize, Valor> = HashMap::new();
    for b in 0..n {
        let (pred, mut env) = if b == 0 {
            (w.cierto()?, entry_env.clone())
        } else {
            let edges = std::mem::take(&mut incoming[b]);
            if edges.is_empty() {
                // Ningun camino llega aqui (lo que sigue a un `return` o a un
                // `break`); el paso de un bucle abierto lo cierra igual.
                if lazos.last().is_some_and(|l| l.paso == b) {
                    cerrar(w, &mut lazos, &mut casa_de, &mut incoming, &vuelta)?;
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
                let mut acc: Option<Valor> = None;
                for (p, e) in &edges {
                    if let Some(v) = e.get(&l) {
                        acc = Some(match acc {
                            None => v.clone(),
                            Some(a) => w.elegir(*p, v, &a)?,
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
            Some(s) => abrir(w, f, b, s, pred, &mut env, &mut lazos, &mut casa_de, vuelta.is_some())?,
            None => pred,
        };
        for o in &f.blocks[b].ops {
            match o {
                IrOp::Let { local, value, .. } | IrOp::Set { local, value, .. } => {
                    let v = eval(w, value, &env)?;
                    asignar(w, pred, *local, v, &mut env, &casa_de)?;
                }
                // ** LB6: un CAMPO de un registro de dibujo cambia: las hojas
                // de ese campo, y las demas como estaban.
                IrOp::SetAt { local, path, value, .. } => {
                    let v = eval(w, value, &env)?;
                    let viejo = env.get(local).cloned().ok_or_else(|| Failure::writer(format!("el nombre %{} cambia sin valor: el juez tenia que haberlo dicho", local)))?;
                    let (desde, cuantas) = ruta(w.m, &viejo, path)?;
                    let mut hojas = viejo.hojas().to_vec();
                    hojas[desde..desde + cuantas].copy_from_slice(v.hojas());
                    let nuevo = viejo.con(hojas);
                    asignar(w, pred, *local, nuevo, &mut env, &casa_de)?;
                }
                IrOp::Drop { .. } => {}
                other => return Err(Failure::writer(format!("una gpu fn con {:?}: gpu.rs tenia que haberlo dicho", other))),
            }
        }
        match &f.blocks[b].end {
            // El paso: la vuelta de atras.
            End::Jump(t) if *t <= b => cerrar(w, &mut lazos, &mut casa_de, &mut incoming, &vuelta)?,
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
                let (c, _) = eval(w, cond, &env)?.uno()?;
                w.aqui = *at;
                w.op(Op::RomperSi { c, si_cero: true });
                incoming[*then].push((pred, env));
            }
            End::Branch { cond, then, other, at } => {
                if lazos.last().is_some_and(|l| *then > l.paso || *other > l.paso) {
                    return Err(Failure::writer("un `if` que sale de un bucle".into()));
                }
                let (c, _) = eval(w, cond, &env)?.uno()?;
                w.aqui = *at;
                let yes = w.entera(pred, c, OpEntera::Y)?;
                let todo = w.cierto()?;
                let not_c = w.entera(c, todo, OpEntera::OX)?;
                let no = w.entera(pred, not_c, OpEntera::Y)?;
                incoming[*then].push((yes, env.clone()));
                incoming[*other].push((no, env));
            }
            End::Return(Some(v)) => {
                let r = eval(w, v, &env)?;
                match (lazos.last(), &vuelta) {
                    (None, _) => returns.push((pred, r.hojas().to_vec())),
                    (Some(l), Some((hecho, valor))) => {
                        // Desde dentro de un bucle: se guarda, y fuera.
                        w.aqui = l.at;
                        let ya = w.entera(*hecho, pred, OpEntera::O)?;
                        for (d, a) in valor.iter().zip(r.hojas()) {
                            w.op(Op::Elige { d: *d, c: pred, a: *a, b: *d });
                        }
                        w.op(Op::Copia { d: *hecho, a: ya });
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
    let (_, last) = returns.last().ok_or_else(|| Failure::writer("una gpu fn sin `return`".into()))?;
    let mut acc = last.clone();
    for (p, r) in returns.iter().rev().skip(1) {
        for (a, x) in acc.iter_mut().zip(r) {
            *a = w.elige(*p, *x, *a)?;
        }
    }
    // Lo que volvio desde dentro de un bucle va primero: paso antes.
    if let Some((hecho, valor)) = &vuelta {
        w.aqui = (f.line, 1);
        for (a, x) in acc.iter_mut().zip(valor) {
            *a = w.elige(*hecho, *x, *a)?;
        }
    }
    Ok(acc)
}

/// **Un nombre toma un valor** en un bloque de predicado `pred`: si cruza la
/// vuelta de un bucle, en su CASA (cada hoja que cambia, con su predicado);
/// si no, el nombre es ese valor y ya.
fn asignar(w: &mut Writer, pred: Reg, local: usize, v: Valor, env: &mut Env, casa_de: &HashMap<usize, Valor>) -> Result<(), Failure> {
    match casa_de.get(&local) {
        // Un nombre que cruza la vuelta: en su casa, con el predicado de
        // este bloque.
        Some(casa) => {
            for (x, c) in v.hojas().iter().zip(casa.hojas()) {
                if x != c {
                    let cierto = w.cierto()?;
                    w.op(if pred == cierto { Op::Copia { d: *c, a: *x } } else { Op::Elige { d: *c, c: pred, a: *x, b: *c } });
                }
            }
            env.insert(local, casa.clone());
        }
        None => {
            env.insert(local, v);
        }
    }
    Ok(())
}

/// Las hojas de `valor` que nombra un camino de campos (`w.color.x`): desde
/// cual y cuantas. Solo campos: dentro de una gpu fn no hay tablas.
fn ruta(m: &Module, valor: &Valor, path: &[PathStep]) -> Result<(usize, usize), Failure> {
    let Valor::Varios(mut t, hojas) = valor else {
        return Err(Failure::writer("una parte de un numero cambia: el juez tenia que haberlo dicho".into()));
    };
    let (mut desde, mut cuantas) = (0, hojas.len());
    for paso in path {
        let PathStep::Field(nombre, _) = paso else {
            return Err(Failure::writer("una celda de una tabla cambia en una gpu fn: gpu.rs tenia que haberlo dicho".into()));
        };
        let (d, n, ty) = campo(m, t, nombre)?;
        desde += d;
        cuantas = n;
        match tipo_de(m, ty) {
            Some(u) => t = u,
            None => break,
        }
    }
    Ok((desde, cuantas))
}

/// **Abre el `for` de cabeza `h` y paso `s`**, entrando con `pred` y `env`:
/// cada nombre de antes que el bucle cambia va a su casa (si un bucle de
/// fuera no se la dio ya), el bucle va dentro de su `Si` si lo de fuera no
/// siempre llega, y dentro de cada vuelta todo empieza cierto. Devuelve el
/// predicado de la cabeza; `env` queda con los nombres de la cabeza.
#[allow(clippy::too_many_arguments)]
fn abrir(w: &mut Writer, f: &Function, h: usize, s: usize, pred: Reg, env: &mut Env, lazos: &mut Vec<Lazo>, casa_de: &mut HashMap<usize, Valor>, con_vuelta: bool) -> Result<Reg, Failure> {
    let at = match &f.blocks[h].end {
        End::Branch { at, .. } => *at,
        _ => return Err(Failure::writer("la cabeza de un bucle sin su pregunta".into())),
    };
    w.aqui = at;
    let fuera = env.clone();
    // Los que cruzan la vuelta: los de antes que el bucle cambia, entero o un
    // campo (TITAN++ no tiene sombras: un `let` de dentro es otro nombre, y
    // muere en la vuelta).
    let mut cambia: Vec<usize> = f.blocks[h..=s]
        .iter()
        .flat_map(|bl| &bl.ops)
        .filter_map(|o| match o {
            IrOp::Set { local, .. } | IrOp::SetAt { local, .. } => Some(*local),
            _ => None,
        })
        .collect();
    cambia.sort_unstable();
    cambia.dedup();
    let mut casas = Vec::new();
    for l in cambia {
        if casa_de.contains_key(&l) {
            continue;
        }
        if let Some(v) = fuera.get(&l) {
            let mut hojas = Vec::with_capacity(v.hojas().len());
            for r in v.hojas() {
                let casa = w.reg()?;
                w.op(Op::Copia { d: casa, a: *r });
                hojas.push(casa);
            }
            let casa = v.con(hojas);
            casas.push((l, casa.clone()));
            casa_de.insert(l, casa.clone());
            env.insert(l, casa);
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
fn cerrar(w: &mut Writer, lazos: &mut Vec<Lazo>, casa_de: &mut HashMap<usize, Valor>, incoming: &mut [Vec<(Reg, Env)>], vuelta: &Vuelta) -> Result<(), Failure> {
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
            let no_hecho = w.entera(*hecho, todo, OpEntera::OX)?;
            pred = w.entera(pred, no_hecho, OpEntera::Y)?;
        } else {
            // El bucle de fuera sale tambien.
            w.op(Op::RomperSi { c: *hecho, si_cero: false });
        }
    }
    let mut env = l.env;
    for (local, casa) in l.casas {
        env.insert(local, casa);
        casa_de.remove(&local);
    }
    incoming[l.salida].push((pred, env));
    Ok(())
}
