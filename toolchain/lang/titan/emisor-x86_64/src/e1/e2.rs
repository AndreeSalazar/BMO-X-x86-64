//! ** E2: EL EMISOR CON REGISTROS VIRTUALES (B5 de `docs/plan/PLAN_TITAN_AFINADO.md`).
//!
//! El propietario, 16-10: *"no es siempre parche, siempre cambiar piezas [...]
//! romper y mejorar para superarlos"*. E1 emite cada valor a un sitio de la
//! pila y lo vuelve a leer; los veteranos (GCC, LLVM) traducen primero a una
//! forma con REGISTROS VIRTUALES -- tantos como haga falta --, miran cuales
//! viven a la vez y los REPARTEN entre los registros de verdad. Esto es esa
//! pieza, para la casa:
//!
//! ```text
//!    1  bajar     la IR de una fn a instrucciones de tres direcciones sobre
//!                 registros virtuales (`Baja`): un local, un registro; cada
//!                 paso de una cuenta, otro; `and`/`or` como saltos
//!    2  vida      que registros viven en cada instruccion (flujo hacia
//!                 atras, con los bucles), y cuales cruzan una llamada
//!    3  reparto   barrido lineal: el que cruza una llamada, a un PRESERVADO
//!                 (rbx, r12, r13, r14); el que no, a uno que no hace falta
//!                 guardar (rsi, rdi, r10). Si no caben: la fn la emite E1
//!    4  emitir    x86-64 sobre los registros reales, con los MISMOS NO que
//!                 E1 (el desborde, la division, la pila de las llamadas)
//! ```
//!
//! Por ahora E2 toma las fn LIGERAS (`llamada::ligera`) cuyo cuerpo solo
//! hace cuentas de enteros, compara y llama a otras ligeras: lo demas sigue
//! por E1, fn a fn. Cada pieza nueva que aprenda se mide en el duelo.
//!
//! [!] Los registros que E2 no reparte son los de paso: `rax`, `rcx`, `rdx`
//! (la division y el resultado), `r8`, `r9` (los valores de una llamada),
//! `r11` y `r15` (el bloque del programa).

use super::E1;
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R10, R11, R8, R9};
use bmo_titan_front::ir::{End, Function, Op, Value};
use bmo_titan_front::calc::Class;
use bmo_titan_front::tree::Ty;

type At = (usize, usize);
/// Un registro virtual.
type V = usize;

#[derive(Clone, Copy, Debug)]
enum Opnd {
    V(V),
    K(i64),
}

#[derive(Debug)]
enum I {
    /// Lo que llega en el registro `k` de una llamada ligera.
    Param(V, usize),
    Label(usize),
    K(V, i64),
    Mov(V, V),
    /// `d = a op b` con su NO (`seguro`: el divisor se demostro 1 o mas).
    Bin(&'static str, V, V, Opnd, At, bool),
    /// Una llamada a una fn ligera; su resultado, si lo da y se usa.
    Call(Option<V>, usize, Vec<Opnd>, At),
    /// Si `a op b`, salta a la etiqueta; si no, sigue.
    Br(&'static str, V, Opnd, usize),
    Jmp(usize),
    Ret(Option<Opnd>),
    // ** LA REGION (B5c): un bucle de una fn de E1, en registros
    /// El local `l`, de su sitio de E1, al entrar.
    Carga(V, usize),
    /// El puntero y lo que mide la lista `l`, al entrar (no cambian dentro).
    Base(V, usize),
    Largo(V, usize),
    /// `d = l[i]` (de 8 bytes, o de 1); `mira`: si se mira que este dentro.
    Idx { d: V, base: V, len: V, i: V, byte: bool, mira: bool, at: At },
    /// `l[i] = v`; `mira`: si se mira que este dentro.
    Pon { base: V, len: V, i: V, v: Opnd, byte: bool, mira: bool, at: At },
    /// `byte(v)`: v va de 0 a 255, o T0060.
    MiraByte(V, At),
    /// Sale de la region al bloque `t` de E1: cada local que cambio, a su
    /// sitio (y el que vive en un registro de E1, aunque no cambie: ese
    /// registro se presto dentro).
    Sale(usize, Vec<(usize, V, bool)>),
}

/// Los valores de una llamada ligera (`llamada::ARGS`), en orden.
const ARGS: [u8; 4] = [RDX, RCX, R8, R9];
/// Los que no hace falta guardar: se reparten a quien no cruza una llamada.
const DE_PASO: [u8; 3] = [RSI, RDI, R10];

/// 1. BAJAR: la IR de la fn, a instrucciones sobre registros virtuales.
struct Baja<'a> {
    m: &'a bmo_titan_front::ir::Module,
    code: Vec<I>,
    nv: usize,
    local: Vec<Option<V>>,
    etiquetas: usize,
    /// B2: lo menos que vale cada local, si se demostro (`afinado::minimos`).
    minimo: Vec<Option<i64>>,
    /// En una REGION: lo que E1 sabe de cada local al entrar, las listas
    /// (base, largo, de bytes), lo que cambia dentro, lo que se carga al
    /// entrar, y si algo no se pudo.
    region: Option<Region>,
}

struct Region {
    known: Vec<Option<Class>>,
    listas: std::collections::HashMap<usize, (V, V, bool)>,
    cambian: std::collections::BTreeSet<usize>,
    entrada: Vec<I>,
    mal: bool,
}

impl Baja<'_> {
    fn nuevo(&mut self) -> V {
        self.nv += 1;
        self.nv - 1
    }

    fn etiqueta(&mut self) -> usize {
        self.etiquetas += 1;
        self.etiquetas - 1
    }

    fn de_local(&mut self, l: usize) -> V {
        if let Some(v) = self.local[l] {
            return v;
        }
        let v = self.nuevo();
        self.local[l] = Some(v);
        // en una region, el que ya tenia valor al entrar se carga de su sitio
        if let Some(r) = &mut self.region {
            match &r.known[l] {
                Some(Class::Int | Class::Byte) => r.entrada.push(I::Carga(v, l)),
                None => {}
                Some(_) => r.mal = true,
            }
        }
        v
    }

    /// La lista `l` de una region: su base, lo que mide y si es de bytes.
    fn lista(&mut self, l: usize) -> Option<(V, V, bool)> {
        if let Some(x) = self.region.as_ref()?.listas.get(&l) {
            return Some(*x);
        }
        let byte = match &self.region.as_ref()?.known[l] {
            Some(Class::List(c)) if **c == Class::Int => false,
            Some(Class::List(c)) if **c == Class::Byte => true,
            _ => return None,
        };
        let (base, len) = (self.nuevo(), self.nuevo());
        let r = self.region.as_mut()?;
        r.entrada.push(I::Base(base, l));
        r.entrada.push(I::Largo(len, l));
        r.listas.insert(l, (base, len, byte));
        Some((base, len, byte))
    }

    /// Un local que cambia dentro de la region.
    fn cambia(&mut self, l: usize) {
        if let Some(r) = &mut self.region {
            r.cambian.insert(l);
        }
    }

    /// Un registro con el valor (un numero escrito se escribe en uno nuevo).
    fn en_registro(&mut self, o: Opnd) -> V {
        match o {
            Opnd::V(v) => v,
            Opnd::K(k) => {
                let v = self.nuevo();
                self.code.push(I::K(v, k));
                v
            }
        }
    }

    /// Un valor entero; `None` si E2 todavia no lo sabe bajar.
    fn valor(&mut self, v: &Value) -> Option<Opnd> {
        match v {
            Value::Int(k, _) => Some(Opnd::K(*k)),
            Value::Local(l, _) => Some(Opnd::V(self.de_local(*l))),
            Value::Bin("+" | "-" | "*" | "/" | "%", ..) | Value::Call(..) => {
                let d = self.nuevo();
                self.valor_en(d, v)?;
                Some(Opnd::V(d))
            }
            // ** de la region: una celda, lo que mide, una vuelta de `for`
            Value::Index(b, i, at) => {
                let Value::Local(l, _) = &**b else { return None };
                let (base, len, byte) = self.lista(*l)?;
                let i = self.valor(i)?;
                let i = self.en_registro(i);
                let d = self.nuevo();
                self.code.push(I::Idx { d, base, len, i, byte, mira: true, at: *at });
                Some(Opnd::V(d))
            }
            Value::Lib(bmo_titan_front::ir::Lib::Turn, args, at) => {
                let [Value::Local(l, _), i] = &args[..] else { return None };
                let (base, len, byte) = self.lista(*l)?;
                let i = self.valor(i)?;
                let i = self.en_registro(i);
                let d = self.nuevo();
                self.code.push(I::Idx { d, base, len, i, byte, mira: false, at: *at });
                Some(Opnd::V(d))
            }
            Value::Len(b, _) => {
                let Value::Local(l, _) = &**b else { return None };
                Some(Opnd::V(self.lista(*l)?.1))
            }
            Value::Byte(inner, at) => {
                let o = self.valor(inner)?;
                let v = self.en_registro(o);
                self.code.push(I::MiraByte(v, *at));
                Some(Opnd::V(v))
            }
            _ => None,
        }
    }

    /// Un valor entero, DERECHO a `d` (sin un registro de mas en medio).
    fn valor_en(&mut self, d: V, v: &Value) -> Option<()> {
        match v {
            Value::Bin(op @ ("+" | "-" | "*" | "/" | "%"), a, b, at) => {
                let cota_a = super::afinado::cota(a, &self.minimo);
                let seguro = super::afinado::cota(b, &self.minimo).is_some_and(|m| m >= 1);
                // B2: `x % 2^k` de un x que se demuestra no negativo, un `and`
                let y = matches!((*op, &**b), ("%", Value::Int(k, _)) if *k > 0 && (*k as u64).is_power_of_two() && *k <= i32::MAX as i64) && cota_a.is_some_and(|m| m >= 0);
                let av = self.valor(a)?;
                let bv = self.valor(b)?;
                let av = self.en_registro(av);
                if y {
                    let Opnd::K(k) = bv else { return None };
                    self.code.push(I::Bin("&", d, av, Opnd::K(k - 1), *at, true));
                } else {
                    self.code.push(I::Bin(op, d, av, bv, *at, seguro));
                }
            }
            Value::Call(func, args, at) if super::llamada::ligera(self.m, *func) && self.m.functions[*func].ret.is_some() => {
                let mut vs = Vec::with_capacity(args.len());
                for a in args {
                    vs.push(self.valor(a)?);
                }
                self.code.push(I::Call(Some(d), *func, vs, *at));
            }
            Value::Int(..) | Value::Local(..) | Value::Index(..) | Value::Lib(..) | Value::Len(..) | Value::Byte(..) => match self.valor(v)? {
                Opnd::K(k) => self.code.push(I::K(d, k)),
                Opnd::V(s) => self.code.push(I::Mov(d, s)),
            },
            _ => return None,
        }
        Some(())
    }

    /// Una condicion: a `si` si se cumple, a `no` si no.
    fn cond(&mut self, c: &Value, si: usize, no: usize) -> Option<()> {
        match c {
            Value::Bin("and", a, b, _) => {
                let medio = self.etiqueta();
                self.cond(a, medio, no)?;
                self.code.push(I::Label(medio));
                self.cond(b, si, no)
            }
            Value::Bin("or", a, b, _) => {
                let medio = self.etiqueta();
                self.cond(a, si, medio)?;
                self.code.push(I::Label(medio));
                self.cond(b, si, no)
            }
            Value::Not(a, _) => self.cond(a, no, si),
            Value::Bool(b, _) => {
                self.code.push(I::Jmp(if *b { si } else { no }));
                Some(())
            }
            Value::Bin(op @ ("<" | "<=" | ">" | ">=" | "==" | "!="), a, b, _) => {
                let a = self.valor(a)?;
                let b = self.valor(b)?;
                let a = self.en_registro(a);
                self.code.push(I::Br(op, a, b, si));
                self.code.push(I::Jmp(no));
                Some(())
            }
            _ => None,
        }
    }

    fn funcion(&mut self, f: &Function) -> Option<()> {
        for (k, (l, _)) in f.params.iter().enumerate() {
            let v = self.de_local(*l);
            self.code.push(I::Param(v, k));
        }
        // los bloques muertos tambien (un salto decidido al compilar aun
        // puede nombrarlos): no cuestan nada que corra
        for (k, b) in f.blocks.iter().enumerate() {
            self.code.push(I::Label(k));
            for op in &b.ops {
                match op {
                    Op::Let { local, value, ty, .. } if ty.as_ref().is_none_or(|t| matches!(t, Ty::Int)) => {
                        let d = self.de_local(*local);
                        self.valor_en(d, value)?;
                    }
                    Op::Set { local, value, .. } => {
                        let d = self.de_local(*local);
                        self.valor_en(d, value)?;
                    }
                    Op::Drop { .. } => {}
                    Op::Call { func, args, at } if super::llamada::ligera(self.m, *func) => {
                        let mut vs = Vec::with_capacity(args.len());
                        for a in args {
                            vs.push(self.valor(a)?);
                        }
                        self.code.push(I::Call(None, *func, vs, *at));
                    }
                    _ => return None,
                }
            }
            match &b.end {
                End::Return(v) => {
                    let o = match v {
                        Some(v) => Some(self.valor(v)?),
                        None => None,
                    };
                    self.code.push(I::Ret(o));
                }
                End::Jump(t) => self.code.push(I::Jmp(*t)),
                End::Branch { cond, then, other, .. } => self.cond(cond, *then, *other)?,
            }
        }
        Some(())
    }

    /// ** B5c: LA REGION -- los bloques `h..=b` de una fn de E1 (un bucle:
    /// `h` su cabeza). Lo que salta fuera SALE: cada local que cambio, a su
    /// sitio, y al bloque de E1. La vuelta de un bucle de dentro mira su
    /// condicion abajo (el bucle girado de B3).
    fn region(&mut self, f: &Function, h: usize, b: usize) -> Option<()> {
        let dentro = |t: usize| (h..=b).contains(&t);
        let mut salidas: Vec<(usize, usize)> = Vec::new();
        let mut a_fuera = |me: &mut Self, t: usize| -> usize {
            if dentro(t) {
                return t;
            }
            if let Some(&(_, e)) = salidas.iter().find(|(x, _)| *x == t) {
                return e;
            }
            let e = me.etiqueta();
            salidas.push((t, e));
            e
        };
        for k in h..=b {
            let blk = &f.blocks[k];
            self.code.push(I::Label(k));
            for op in &blk.ops {
                match op {
                    Op::Let { local, value, ty, .. } if ty.as_ref().is_none_or(|t| matches!(t, Ty::Int)) => {
                        let d = self.de_local(*local);
                        self.cambia(*local);
                        self.valor_en(d, value)?;
                    }
                    Op::Set { local, value, .. } => {
                        if !matches!(self.region.as_ref()?.known[*local], Some(Class::Int) | None) {
                            return None;
                        }
                        let d = self.de_local(*local);
                        self.cambia(*local);
                        self.valor_en(d, value)?;
                    }
                    Op::SetAt { local, path, value, at } => {
                        let [bmo_titan_front::ir::PathStep::Index(i)] = &path[..] else { return None };
                        let (base, len, byte) = self.lista(*local)?;
                        let i = self.valor(i)?;
                        let i = self.en_registro(i);
                        let v = self.valor(value)?;
                        self.code.push(I::Pon { base, len, i, v, byte, mira: true, at: *at });
                    }
                    // morir: solo un entero (lo demas se suelta, y eso es de E1)
                    Op::Drop { local, .. } => {
                        if !matches!(self.region.as_ref()?.known[*local], Some(Class::Int | Class::Byte) | None) {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
            match &blk.end {
                End::Return(_) => return None,
                End::Jump(t) if dentro(*t) && *t <= k => {
                    // la vuelta: la condicion de la cabeza, aqui (B3)
                    match &f.blocks[*t] {
                        cab if cab.ops.is_empty() => match &cab.end {
                            End::Branch { cond, then, other, .. } => {
                                let (si, no) = (a_fuera(self, *then), a_fuera(self, *other));
                                self.cond(cond, si, no)?;
                            }
                            _ => self.code.push(I::Jmp(*t)),
                        },
                        _ => self.code.push(I::Jmp(*t)),
                    }
                }
                End::Jump(t) => {
                    let e = a_fuera(self, *t);
                    self.code.push(I::Jmp(e));
                }
                End::Branch { cond, then, other, .. } => {
                    let (si, no) = (a_fuera(self, *then), a_fuera(self, *other));
                    self.cond(cond, si, no)?;
                }
            }
        }
        // las salidas, al final: cada una deja lo que cambio en su sitio
        let r = self.region.as_ref()?;
        if r.mal {
            return None;
        }
        let cargados: Vec<usize> = r.entrada.iter().filter_map(|i| if let I::Carga(_, l) = i { Some(*l) } else { None }).collect();
        // lo que nace dentro del bucle muere en el (su `let` es de la vuelta):
        // solo vuelve a su sitio lo que ya vivia al entrar
        let cambian: Vec<(usize, V, bool)> = (0..self.local.len())
            .filter(|&l| (r.cambian.contains(&l) && r.known[l].is_some()) || cargados.contains(&l))
            .filter_map(|l| self.local[l].map(|v| (l, v, r.cambian.contains(&l))))
            .collect();
        for (t, e) in salidas {
            self.code.push(I::Label(e));
            self.code.push(I::Sale(t, cambian.clone()));
        }
        // y lo que se carga al entrar, delante de todo
        let entrada = std::mem::take(&mut self.region.as_mut()?.entrada);
        self.code.splice(0..0, entrada);
        Some(())
    }
}

/// Lo que una instruccion usa y lo que define.
fn usa_y_define(i: &I) -> (Vec<V>, Option<V>) {
    let op = |o: &Opnd| if let Opnd::V(v) = o { vec![*v] } else { Vec::new() };
    match i {
        I::Param(v, _) | I::K(v, _) => (Vec::new(), Some(*v)),
        I::Label(_) | I::Jmp(_) | I::Ret(None) => (Vec::new(), None),
        I::Mov(d, s) => (vec![*s], Some(*d)),
        I::Bin(_, d, a, b, _, _) => {
            let mut u = vec![*a];
            u.extend(op(b));
            (u, Some(*d))
        }
        I::Call(d, _, args, _) => (args.iter().flat_map(op).collect(), *d),
        I::Carga(v, _) | I::Base(v, _) | I::Largo(v, _) => (Vec::new(), Some(*v)),
        I::Idx { d, base, len, i, .. } => (vec![*base, *len, *i], Some(*d)),
        I::Pon { base, len, i, v, .. } => {
            let mut u = vec![*base, *len, *i];
            u.extend(op(v));
            (u, None)
        }
        I::MiraByte(v, _) => (vec![*v], None),
        I::Sale(_, cambian) => (cambian.iter().map(|(_, v, _)| *v).collect(), None),
        I::Br(_, a, b, _) => {
            let mut u = vec![*a];
            u.extend(op(b));
            (u, None)
        }
        I::Ret(Some(o)) => (op(o), None),
    }
}

/// 2 y 3. LA VIDA Y EL REPARTO: el registro real de cada virtual, o `None`
/// si no caben (y la fn la emite E1).
fn reparte(code: &[I], nv: usize, preservados: &[u8], de_paso: &[u8], scratch: bool) -> Option<Vec<u8>> {
    let n = code.len();
    let donde: std::collections::HashMap<usize, usize> = code.iter().enumerate().filter_map(|(k, i)| if let I::Label(l) = i { Some((*l, k)) } else { None }).collect();
    let sucesores: Vec<Vec<usize>> = code
        .iter()
        .enumerate()
        .map(|(k, i)| match i {
            I::Jmp(l) => vec![donde[l]],
            I::Br(_, _, _, l) => vec![donde[l], k + 1],
            I::Ret(_) | I::Sale(..) => Vec::new(),
            _ => vec![k + 1],
        })
        .collect();
    let ud: Vec<(Vec<V>, Option<V>)> = code.iter().map(usa_y_define).collect();
    // vivos al entrar y al salir de cada instruccion, hasta que nada cambie
    let mut entra = vec![vec![false; nv]; n];
    let mut sale = vec![vec![false; nv]; n];
    loop {
        let mut cambio = false;
        for k in (0..n).rev() {
            let mut s = vec![false; nv];
            for &t in &sucesores[k] {
                if t < n {
                    for v in 0..nv {
                        s[v] |= entra[t][v];
                    }
                }
            }
            let mut e = s.clone();
            if let Some(d) = ud[k].1 {
                e[d] = false;
            }
            for &u in &ud[k].0 {
                e[u] = true;
            }
            if e != entra[k] || s != sale[k] {
                entra[k] = e;
                sale[k] = s;
                cambio = true;
            }
        }
        if !cambio {
            break;
        }
    }
    // el tramo de cada virtual, y si cruza una llamada
    let mut tramo: Vec<Option<(usize, usize)>> = vec![None; nv];
    let mut cruza = vec![false; nv];
    for k in 0..n {
        for v in 0..nv {
            let aqui = entra[k][v] || sale[k][v] || ud[k].1 == Some(v) || ud[k].0.contains(&v);
            if aqui {
                tramo[v] = Some(match tramo[v] {
                    None => (k, k),
                    Some((a, _)) => (a, k),
                });
            }
            if matches!(code[k], I::Call(..)) && sale[k][v] && ud[k].1 != Some(v) {
                cruza[v] = true;
            }
        }
    }
    // ** el registro que PIDE cada virtual: el resultado de una llamada y lo
    // que se devuelve, `rax`; el valor `k` de una llamada, su registro
    // (`ARGS`). Lo toma solo si en todo su tramo nada lo pisa: ni otra
    // llamada, ni una division (rax, rdx, rcx), ni una resta al reves (rax)
    let mut pide: Vec<Option<u8>> = vec![None; nv];
    for i in code {
        match i {
            I::Call(d, _, args, _) => {
                for (k, a) in args.iter().enumerate() {
                    if let Opnd::V(v) = a {
                        pide[*v] = Some(ARGS[k]);
                    }
                }
                if let Some(d) = d {
                    pide[*d].get_or_insert(RAX);
                }
            }
            I::Ret(Some(Opnd::V(v))) => {
                pide[*v].get_or_insert(RAX);
            }
            _ => {}
        }
    }
    let pisa = |v: V, r: u8| -> bool {
        let (a, b) = tramo[v].unwrap();
        (a..=b).any(|k| match &code[k] {
            // la llamada que lo produce (en `a`) o que lo consume (en `b`) no lo pisa
            I::Call(..) => k != a && k != b,
            I::Bin("/" | "%", ..) => matches!(r, RAX | RDX | RCX),
            I::Bin("-", ..) => r == RAX,
            _ => false,
        })
    };
    // barrido lineal
    let mut orden: Vec<V> = (0..nv).filter(|&v| tramo[v].is_some()).collect();
    orden.sort_by_key(|&v| tramo[v].unwrap().0);
    let mut reg = vec![0u8; nv];
    let mut activos: Vec<V> = Vec::new();
    for v in orden {
        let inicio = tramo[v].unwrap().0;
        activos.retain(|&w| tramo[w].unwrap().1 >= inicio);
        let ocupados: Vec<u8> = activos.iter().map(|&w| reg[w]).collect();
        let libres = |pool: &[u8]| pool.iter().copied().find(|r| !ocupados.contains(r));
        let pedido = pide[v].filter(|&r| !cruza[v] && !ocupados.contains(&r) && !pisa(v, r));
        let r = match pedido {
            Some(r) => r,
            None if cruza[v] => libres(preservados)?,
            // en una region, tambien los de la division, si en su tramo no
            // hay division (ni resta al reves) que los pise
            None => libres(de_paso)
                .or_else(|| if scratch { [RCX, RDX, RAX].into_iter().find(|&r| !ocupados.contains(&r) && !pisa(v, r)) } else { None })
                .or_else(|| libres(preservados))?,
        };
        reg[v] = r;
        activos.push(v);
    }
    Some(reg)
}

impl E1<'_> {
    /// ** E2: la fn `f`, si E2 la sabe emitir. Da lo que gasta de pila cada
    /// llamada (como `function`), o `None` -- y nada emitido -- para que la
    /// emita E1.
    pub(super) fn e2(&mut self, f: &Function) -> Result<Option<i32>, String> {
        let Some(k) = self.m.functions.iter().position(|g| std::ptr::eq(g, f)) else { return Ok(None) };
        if !super::llamada::ligera(self.m, k) {
            return Ok(None);
        }
        let mut b = Baja { m: self.m, code: Vec::new(), nv: 0, local: vec![None; f.locals.len()], etiquetas: f.blocks.len(), minimo: Vec::new(), region: None };
        if b.funcion(f).is_none() {
            return Ok(None);
        }
        let preservados: &[u8] = if self.libres { &[14, 3, 12, 13] } else { &[14] };
        quita_lo_repetido(&mut b.code);
        let Some(reg) = reparte(&b.code, b.nv, preservados, &DE_PASO, false) else { return Ok(None) };
        let mut guardar: Vec<u8> = preservados.iter().copied().filter(|r| reg.iter().enumerate().any(|(v, x)| x == r && usado(&b.code, v))).collect();
        guardar.sort();
        // lo que gasta: los preservados guardados y la vuelta
        let coste = 8 * guardar.len() as i32 + 8;
        for &r in &guardar {
            x86::push_r64(&mut self.code, r);
        }
        self.emite(&b.code, &reg, &guardar);
        self.guardado = None;
        Ok(Some(coste))
    }

    /// Las instrucciones repartidas, a x86-64. Da los saltos que SALEN (a
    /// bloques de E1: `(campo, bloque)`).
    fn emite(&mut self, code: &[I], reg: &[u8], guardar: &[u8]) -> Vec<(usize, usize)> {
        let mut sitio = std::collections::HashMap::new();
        let mut saltos: Vec<(usize, usize)> = Vec::new();
        let mut fuera: Vec<(usize, usize)> = Vec::new();
        let mut k = 0;
        while k < code.len() {
            match &code[k] {
                I::Param(..) => {
                    // todos los que llegan a la vez: un movimiento paralelo
                    let mut movs = Vec::new();
                    while let Some(I::Param(v, a)) = code.get(k) {
                        movs.push((reg[*v], Opnd8::R(ARGS[*a])));
                        k += 1;
                    }
                    self.paralelo(movs);
                    continue;
                }
                I::Label(l) => {
                    sitio.insert(*l, self.code.len());
                }
                I::K(v, n) => self.imm(reg[*v], *n),
                I::Mov(d, s) => self.mov_si(reg[*d], reg[*s]),
                I::Bin(op, d, a, o, at, seguro) => self.e2_cuenta(op, reg[*d], reg[*a], o.map_reg(reg), *at, *seguro),
                I::Call(d, func, args, at) => {
                    let movs = args.iter().enumerate().map(|(j, a)| (ARGS[j], a.map_reg(&reg))).collect();
                    self.paralelo(movs);
                    self.salta_a(*func, *at);
                    if let Some(d) = d {
                        self.mov_si(reg[*d], RAX);
                    }
                }
                I::Br(op, a, o, l) => {
                    self.e2_cmp(reg[*a], o.map_reg(&reg));
                    // `Br` + `Jmp(no)` + la etiqueta `si` detras: el salto AL
                    // REVES a `no`, y se sigue en `si`
                    if let (Some(I::Jmp(no)), Some(I::Label(sig))) = (code.get(k + 1), code.get(k + 2)) {
                        if sig == l {
                            let j = self.jcc(cc(op) ^ 1);
                            saltos.push((j, *no));
                            k += 2;
                            continue;
                        }
                    }
                    let j = self.jcc(cc(op));
                    saltos.push((j, *l));
                }
                I::Jmp(l) => {
                    if !matches!(code.get(k + 1), Some(I::Label(x)) if x == l) {
                        let j = self.jmp();
                        saltos.push((j, *l));
                    }
                }
                I::Carga(..) | I::Base(..) | I::Largo(..) => {
                    // lo que entra a la region: primero lo que viene de un
                    // registro (un movimiento paralelo), luego lo de la pila
                    let mut movs = Vec::new();
                    let mut pila = Vec::new();
                    while let Some(i @ (I::Carga(..) | I::Base(..) | I::Largo(..))) = code.get(k) {
                        let (v, p) = match i {
                            I::Carga(v, l) => (*v, self.local(*l)),
                            I::Base(v, l) => (*v, self.local(*l)),
                            I::Largo(v, l) => (*v, self.local(*l).at(8)),
                            _ => unreachable!(),
                        };
                        match p.base {
                            super::Base::Reg(r) => movs.push((reg[v], Opnd8::R(r))),
                            _ => pila.push((reg[v], p)),
                        }
                        k += 1;
                    }
                    self.paralelo(movs);
                    for (r, p) in pila {
                        self.load(p, r);
                    }
                    continue;
                }
                I::Idx { d, base, len, i, byte, mira, at } => {
                    if *mira {
                        x86::cmp_r64_r64(&mut self.code, reg[*i], reg[*len]);
                        self.trap(0x83, "T0072", "una celda fuera de la lista", *at);
                    }
                    self.celda_sib(reg[*d], reg[*base], reg[*i], *byte, false);
                }
                I::Pon { base, len, i, v, byte, mira, at } => {
                    if *mira {
                        x86::cmp_r64_r64(&mut self.code, reg[*i], reg[*len]);
                        self.trap(0x83, "T0072", "una celda fuera de la lista", *at);
                    }
                    let r = match v.map_reg(reg) {
                        Opnd8::R(r) => r,
                        Opnd8::K(n) => {
                            self.imm(R11, n);
                            R11
                        }
                    };
                    self.celda_sib(r, reg[*base], reg[*i], *byte, true);
                }
                I::MiraByte(v, at) => {
                    x86::cmp_r64_imm32(&mut self.code, reg[*v], 255);
                    self.trap(0x87, "T0060", "byte(x) no cabe en un byte: va de 0 a 255", *at);
                }
                I::Sale(t, cambian) => {
                    // cada local que cambio, a su sitio: lo de la pila primero
                    // (el contador de un `for` el ultimo: su vuelta vive en el
                    // mismo sitio), luego los registros de E1, en paralelo
                    let mut orden: Vec<&(usize, V, bool)> = cambian.iter().collect();
                    orden.sort_by_key(|(l, _, _)| self.f.vuelta.get(*l).is_some_and(|x| x.is_some()));
                    let mut movs: Vec<(u8, Opnd8)> = Vec::new();
                    for &&(l, v, cambio) in &orden {
                        let p = self.local(l);
                        match p.base {
                            super::Base::Reg(r) => {
                                movs.retain(|(d, _)| *d != r);
                                movs.push((r, Opnd8::R(reg[v])));
                            }
                            _ if cambio => self.store(p, reg[v]),
                            _ => {}
                        }
                    }
                    self.paralelo(movs);
                    for &r in guardar.iter().rev() {
                        x86::pop_r64(&mut self.code, r);
                    }
                    let j = self.jmp();
                    fuera.push((j, *t));
                }
                I::Ret(o) => {
                    match o {
                        Some(Opnd::V(v)) => self.mov_si(RAX, reg[*v]),
                        Some(Opnd::K(n)) => self.imm(RAX, *n),
                        None => {}
                    }
                    for &r in guardar.iter().rev() {
                        x86::pop_r64(&mut self.code, r);
                    }
                    self.code.push(0xC3);
                }
            }
            k += 1;
        }
        for (j, l) in saltos {
            x86::patch_jump_to(&mut self.code, j, sitio[&l]);
        }
        self.guardado = None;
        fuera
    }

    /// `d = [base + i*8]` (o el byte, sin signo), o al reves si `pon`.
    fn celda_sib(&mut self, r: u8, base: u8, i: u8, byte: bool, pon: bool) {
        let (w, op): (u8, &[u8]) = match (byte, pon) {
            (false, false) => (0x48, &[0x8B]),
            (false, true) => (0x48, &[0x89]),
            (true, false) => (0x40, &[0x0F, 0xB6]),
            (true, true) => (0x40, &[0x88]),
        };
        self.code.push(w | ((r >> 3) & 1) << 2 | ((i >> 3) & 1) << 1 | ((base >> 3) & 1));
        self.code.extend_from_slice(op);
        let escala = if byte { 0 } else { 3 };
        let sib = escala << 6 | (i & 7) << 3 | (base & 7);
        if base & 7 == 5 {
            // rbp / r13 de base: con un desplazamiento de 0
            self.code.extend_from_slice(&[0x44 | (r & 7) << 3, sib, 0]);
        } else {
            self.code.extend_from_slice(&[0x04 | (r & 7) << 3, sib]);
        }
    }

    /// ** Un MOVIMIENTO PARALELO: cada destino recibe lo que su fuente tenia
    /// ANTES de empezar (`g(b, a)` cruza dos registros). Primero los que no
    /// pisan la fuente de otro; un ciclo se rompe con `r11`; los numeros, al
    /// final.
    fn paralelo(&mut self, movs: Vec<(u8, Opnd8)>) {
        let mut regs: Vec<(u8, u8)> = movs.iter().filter_map(|&(d, s)| if let Opnd8::R(r) = s { (d != r).then_some((d, r)) } else { None }).collect();
        while !regs.is_empty() {
            if let Some(k) = (0..regs.len()).find(|&k| !regs.iter().any(|&(_, s)| s == regs[k].0)) {
                let (d, s) = regs.remove(k);
                x86::mov_r64_r64(&mut self.code, d, s);
            } else {
                // un ciclo: la fuente del primero a `r11`
                let (_, s) = regs[0];
                x86::mov_r64_r64(&mut self.code, R11, s);
                for m in regs.iter_mut() {
                    if m.1 == s {
                        m.1 = R11;
                    }
                }
            }
        }
        for (d, s) in movs {
            if let Opnd8::K(n) = s {
                self.imm(d, n);
            }
        }
    }

    fn mov_si(&mut self, d: u8, s: u8) {
        if d != s {
            x86::mov_r64_r64(&mut self.code, d, s);
        }
    }

    /// `cmp a, b`.
    fn e2_cmp(&mut self, a: u8, b: Opnd8) {
        match b {
            Opnd8::R(r) => x86::cmp_r64_r64(&mut self.code, a, r),
            Opnd8::K(n) => match i32::try_from(n) {
                Ok(n) => x86::cmp_r64_imm32(&mut self.code, a, n),
                Err(_) => {
                    self.imm(R11, n);
                    x86::cmp_r64_r64(&mut self.code, a, R11);
                }
            },
        }
    }

    /// `d = a op b`, con los NO de E1 (`numero.rs`).
    fn e2_cuenta(&mut self, op: &str, d: u8, a: u8, b: Opnd8, at: At, demostrado: bool) {
        const NO_CABE: &str = "el resultado no cabe en 64 bits";
        match op {
            "&" => {
                // B2: el resto de una potencia de dos de un no negativo
                let Opnd8::K(m) = b else { return };
                self.mov_si(d, a);
                x86::and_r64_imm32(&mut self.code, d, m as u32);
            }
            "+" | "-" | "*" => {
                let b = match b {
                    Opnd8::K(n) if i32::try_from(n).is_err() => {
                        self.imm(R11, n);
                        Opnd8::R(R11)
                    }
                    b => b,
                };
                match b {
                    Opnd8::R(r) if r == d && d != a => {
                        if op == "-" {
                            // d = a - d: por `rax`
                            x86::mov_r64_r64(&mut self.code, RAX, a);
                            x86::sub_r64_r64(&mut self.code, RAX, r);
                            self.trap(0x80, "T0060", NO_CABE, at);
                            x86::mov_r64_r64(&mut self.code, d, RAX);
                            return;
                        }
                        // + y * dan lo mismo al reves
                        self.e2_op(op, d, Opnd8::R(a));
                    }
                    b => {
                        self.mov_si(d, a);
                        self.e2_op(op, d, b);
                    }
                }
                self.trap(0x80, "T0060", NO_CABE, at);
            }
            _ => {
                // la division: el divisor en un registro que no sea rax ni rdx
                let (div, seguro) = match b {
                    Opnd8::R(r) => (r, demostrado),
                    Opnd8::K(n) => {
                        self.imm(RCX, n);
                        (RCX, n != 0 && n != -1)
                    }
                };
                let mut hecho = None;
                if !seguro {
                    x86::test_r64_r64(&mut self.code, div, div);
                    self.trap(0x84, "T0061", "una division por cero", at);
                    x86::cmp_r64_imm8(&mut self.code, div, -1);
                    let normal = self.jcc(0x85);
                    if op == "/" {
                        x86::mov_r64_r64(&mut self.code, RAX, a);
                        self.imm(R11, i64::MIN);
                        x86::cmp_r64_r64(&mut self.code, RAX, R11);
                        self.trap(0x84, "T0060", NO_CABE, at);
                        x86::neg_r64(&mut self.code, RAX);
                        x86::mov_r64_r64(&mut self.code, d, RAX);
                    } else {
                        self.imm(d, 0);
                    }
                    hecho = Some(self.jmp());
                    self.here(normal);
                }
                x86::mov_r64_r64(&mut self.code, RAX, a);
                x86::cqo(&mut self.code);
                x86::idiv_r64(&mut self.code, div);
                if op == "/" {
                    x86::test_r64_r64(&mut self.code, RDX, RDX);
                    self.trap(0x85, "T0062", "una division que no es exacta (entre enteros, 7 / 2 no es 3)", at);
                    x86::mov_r64_r64(&mut self.code, d, RAX);
                } else {
                    x86::mov_r64_r64(&mut self.code, d, RDX);
                }
                if let Some(h) = hecho {
                    self.here(h);
                }
            }
        }
    }

    /// `d op= b` (+, -, *), sin el `jo`.
    fn e2_op(&mut self, op: &str, d: u8, b: Opnd8) {
        match (op, b) {
            ("+", Opnd8::R(r)) => x86::add_r64_r64(&mut self.code, d, r),
            ("-", Opnd8::R(r)) => x86::sub_r64_r64(&mut self.code, d, r),
            (_, Opnd8::R(r)) => x86::imul_r64_r64(&mut self.code, d, r),
            ("+", Opnd8::K(n)) => self.suma_inmediata(d, n),
            ("-", Opnd8::K(n)) => match n.checked_neg() {
                Some(m) if i32::try_from(m).is_ok() => self.suma_inmediata(d, m),
                _ => {
                    self.imm(R11, n);
                    x86::sub_r64_r64(&mut self.code, d, R11);
                }
            },
            (_, Opnd8::K(n)) => {
                // imul d, d, imm8 / imm32
                let rex = 0x48 | ((d >> 3) & 1) << 2 | ((d >> 3) & 1);
                let modrm = 0xC0 | (d & 7) << 3 | (d & 7);
                match i8::try_from(n) {
                    Ok(b) => self.code.extend_from_slice(&[rex, 0x6B, modrm, b as u8]),
                    Err(_) => {
                        self.code.extend_from_slice(&[rex, 0x69, modrm]);
                        self.code.extend_from_slice(&(n as i32).to_le_bytes());
                    }
                }
            }
        }
    }
}

/// Un valor ya repartido: un registro real o un numero.
#[derive(Clone, Copy)]
enum Opnd8 {
    R(u8),
    K(i64),
}

impl Opnd {
    fn map_reg(&self, reg: &[u8]) -> Opnd8 {
        match self {
            Opnd::V(v) => Opnd8::R(reg[*v]),
            Opnd::K(n) => Opnd8::K(*n),
        }
    }
}

/// El salto de cuando `a op b` SE CUMPLE.
fn cc(op: &str) -> u8 {
    match op {
        "<" => 0x8C,
        "<=" => 0x8E,
        ">" => 0x8F,
        ">=" => 0x8D,
        "==" => 0x84,
        _ => 0x85,
    }
}

/// Si el virtual `v` aparece en alguna instruccion (y por eso tiene registro).
fn usado(code: &[I], v: V) -> bool {
    code.iter().any(|i| {
        let (u, d) = usa_y_define(i);
        d == Some(v) || u.contains(&v)
    })
}

impl E1<'_> {
    /// ** B5c: si el bloque `h` es la cabeza de un bucle que E2 sabe hacer
    /// ENTERO (cuentas, comparaciones, celdas de listas que no crecen), lo
    /// emite E2 en registros. Da el ultimo bloque del bucle y los saltos que
    /// salen (a bloques de E1), o `None` y nada emitido.
    pub(super) fn e2_region(&mut self, f: &Function, h: usize) -> Result<Option<(usize, Vec<(usize, usize)>)>, String> {
        let Some(b) = (h..f.blocks.len()).filter(|&k| f.blocks[k].end.targets().contains(&h)).max() else { return Ok(None) };
        // nadie de fuera entra en medio del bucle
        for (k, blk) in f.blocks.iter().enumerate() {
            if !(h..=b).contains(&k) && blk.end.targets().iter().any(|&t| t > h && t <= b) {
                return Ok(None);
            }
        }
        let region = Region { known: self.f.known.clone(), listas: Default::default(), cambian: Default::default(), entrada: Vec::new(), mal: false };
        let mut baja = Baja { m: self.m, code: Vec::new(), nv: 0, local: vec![None; f.locals.len()], etiquetas: f.blocks.len(), minimo: self.f.minimo.clone(), region: Some(region) };
        if baja.region(f, h, b).is_none() {
            return Ok(None);
        }
        // los de paso, y los de E1 de esta fn (sus locales se cargan y se
        // devuelven a la entrada y a la salida)
        quita_lo_repetido(&mut baja.code);
        // un registro de E1 se presta solo si su local entra (y vuelve a el
        // al salir); los demas guardan un local que la region no toca
        let entran: Vec<usize> = baja.code.iter().filter_map(|i| if let I::Carga(_, l) = i { Some(*l) } else { None }).collect();
        let mut de_paso = vec![RSI, RDI, R8, R9, R10];
        de_paso.extend(self.f.registro.iter().filter(|x| entran.iter().any(|&l| matches!(self.local(l).base, super::Base::Reg(r) if r == x.1))).map(|x| x.1));
        // y los preservados que esta fn no usa: se guardan al entrar al bucle
        // y se devuelven al salir
        let suyos: Vec<u8> = self.f.registro.iter().map(|x| x.1).collect();
        let extra: Vec<u8> = (if self.libres { vec![14u8, 3, 12, 13] } else { vec![14u8] }).into_iter().filter(|r| !suyos.contains(r)).collect();
        let Some(reg) = reparte(&baja.code, baja.nv, &extra, &de_paso, true) else { return Ok(None) };
        let mut guardar: Vec<u8> = extra.iter().copied().filter(|r| reg.iter().enumerate().any(|(v, x)| x == r && usado(&baja.code, v))).collect();
        guardar.sort();
        for &r in &guardar {
            x86::push_r64(&mut self.code, r);
        }
        let fuera = self.emite(&baja.code, &reg, &guardar);
        Ok(Some((b, fuera)))
    }
}

/// ** B6: LO QUE YA SE HIZO NO SE REPITE (las "expresiones disponibles" de
/// GCC y LLVM). Por cada sitio del codigo, lo que se sabe seguro venga de
/// donde venga: que la celda `i` de una lista ya se miro (`i < largo`), y
/// que registro tiene ya el valor de `l[i]`. Se sabe al leerla o escribirla,
/// y se olvida cuando `i`, el largo o quien la tiene cambian, o cuando se
/// escribe en esa lista (en cualquier celda). Donde se sabe, la celda no se
/// mira otra vez y no se lee otra vez: `while j >= 0 and a[j] > v` y el
/// `a[j]` del cuerpo son la misma.
fn quita_lo_repetido(code: &mut [I]) {
    use std::collections::BTreeSet;
    // (indice, largo) miradas; (base, indice, quien la tiene) leidas
    type Sabe = (BTreeSet<(V, V)>, BTreeSet<(V, V, V)>);
    let n = code.len();
    let donde: std::collections::HashMap<usize, usize> = code.iter().enumerate().filter_map(|(k, i)| if let I::Label(l) = i { Some((*l, k)) } else { None }).collect();
    let mut antes: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, i) in code.iter().enumerate() {
        let sig: Vec<usize> = match i {
            I::Jmp(l) => vec![donde[l]],
            I::Br(_, _, _, l) => vec![donde[l], k + 1],
            I::Ret(_) | I::Sale(..) => Vec::new(),
            _ => vec![k + 1],
        };
        for t in sig {
            if t < n {
                antes[t].push(k);
            }
        }
    }
    let paso = |i: &I, mut s: Sabe| -> Sabe {
        if let (_, Some(d)) = usa_y_define(i) {
            s.0.retain(|&(a, b)| a != d && b != d);
            s.1.retain(|&(a, b, c)| a != d && b != d && c != d);
        }
        match i {
            I::Idx { d, base, len, i, byte: false, .. } => {
                s.0.insert((*i, *len));
                if d != i && d != base {
                    s.1.insert((*base, *i, *d));
                }
            }
            I::Idx { len, i, .. } => {
                s.0.insert((*i, *len));
            }
            I::Pon { base, len, i, v, byte, .. } => {
                s.0.insert((*i, *len));
                s.1.retain(|&(b, _, _)| b != *base);
                if let (Opnd::V(v), false) = (v, byte) {
                    s.1.insert((*base, *i, *v));
                }
            }
            _ => {}
        }
        s
    };
    // hacia delante, hasta que nada cambie: lo que se sabe al entrar en cada
    // instruccion es lo que se sabe en TODAS las que llevan a ella
    let mut entra: Vec<Option<Sabe>> = vec![None; n];
    entra[0] = Some(Default::default());
    loop {
        let mut cambio = false;
        for k in 0..n {
            let nuevo: Option<Sabe> = if k == 0 {
                Some(Default::default())
            } else {
                let mut acc: Option<Sabe> = None;
                for &p in &antes[k] {
                    if let Some(e) = &entra[p] {
                        let sale = paso(&code[p], e.clone());
                        acc = Some(match acc {
                            None => sale,
                            Some(a) => (a.0.intersection(&sale.0).copied().collect(), a.1.intersection(&sale.1).copied().collect()),
                        });
                    }
                }
                acc
            };
            if nuevo != entra[k] {
                entra[k] = nuevo;
                cambio = true;
            }
        }
        if !cambio {
            break;
        }
    }
    for k in 0..n {
        let Some(s) = entra[k].clone() else { continue };
        match &mut code[k] {
            I::Idx { d, base, len, i, byte: false, .. } if s.1.iter().any(|&(b, x, _)| b == *base && x == *i) && s.0.contains(&(*i, *len)) => {
                let quien = s.1.iter().find(|&&(b, x, _)| b == *base && x == *i).unwrap().2;
                code[k] = I::Mov(*d, quien);
            }
            I::Idx { len, i, mira, .. } | I::Pon { len, i, mira, .. } if s.0.contains(&(*i, *len)) => *mira = false,
            _ => {}
        }
    }
}
