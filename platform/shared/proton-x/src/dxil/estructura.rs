//! **Los saltos del DXIL, vueltos `si` y bucles** (E6b, 02-10).
//!
//! `dxc` escribe una funcion de VARIOS bloques: cada uno acaba en `ret` o
//! en `br` (a uno, o a dos segun un `i1`), y los valores que llegan por
//! varios caminos se juntan con `phi`. El [`Programa`] de la casa no tiene
//! bloques: tiene `Si`/`SiNo`/`FinSi` y `Bucle`/`Romper`/`Continuar`/
//! `FinBucle`. Esto los reconstruye -- lo que hace un driver de D3D12 antes de
//! su compilador, y vkd3d-proton para SPIR-V --:
//!
//! ```text
//!    dominadores        quien esta en TODO camino desde la entrada
//!    post-dominadores   quien esta en TODO camino hasta el `ret`: el sitio
//!                       donde se juntan las dos ramas de un `br` (su union)
//!    bucles             un salto a un bloque que lo domina (la vuelta);
//!                       el cuerpo, lo que llega a la vuelta sin pasar por la
//!                       cabeza; su SALIDA, una sola (si no: todavia no)
//!    phi                una COPIA en cada arista que llega, en paralelo
//!                       (primero a temporales si una copia pisa a otra)
//! ```
//!
//! Y al emitir, un `br` dentro de un bucle a su cabeza es `Continuar`, a su
//! salida `Romper`; uno con dos destinos es un `Si` con sus dos ramas hasta su
//! union. La forma que `dxc` da a un bucle (rotado: la cabeza sale con un
//! `br` condicional y el ultimo bloque decide si vuelve) sale asi sin mas.
//!
//! Lo que un grafo no reducible, un bucle de dos salidas o un salto de dos
//! bucles de golpe piden, se DICE: `NoPrograma::Forma`, con el porque.

use alloc::vec;
use alloc::vec::Vec;

use super::programa::{con_signo, Comparacion, Compilador, NoPrograma, Op, Operandos, Reg, Valor};

/// Como acaba un bloque.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Fin {
    Ret,
    Salto(usize),
    /// `br i1 c, si, no`.
    Si { c: Reg, si: usize, no: usize },
}

/// Un `phi` leido: su registro y lo que llega de cada bloque (el id del valor,
/// que puede ser de mas adelante: se resuelve al acabar).
#[derive(Debug, Clone)]
pub(super) struct Phi {
    d: Reg,
    entrantes: Vec<(usize, usize)>,
}

/// Un bloque leido: sus operaciones (de `c.ops`), como acaba y sus `phi`.
#[derive(Debug, Clone)]
struct Bloque {
    desde: usize,
    hasta: usize,
    fin: Fin,
    phis: Vec<Phi>,
}

/// Los bloques de la funcion mientras se lee.
#[derive(Debug, Default)]
pub(super) struct Bloques {
    declarados: usize,
    hechos: Vec<Bloque>,
    desde: usize,
    phis: Vec<Phi>,
}

impl Bloques {
    pub(super) fn declarar(&mut self, n: usize, desde: usize) {
        self.declarados = n;
        self.desde = desde;
    }

    pub(super) fn cerrar(&mut self, fin: Fin, hasta: usize) -> Result<(), NoPrograma> {
        if self.hechos.len() >= self.declarados.max(1) {
            return Err(NoPrograma::Forma("mas bloques de los que la funcion declara"));
        }
        self.hechos.push(Bloque { desde: self.desde, hasta, fin, phis: core::mem::take(&mut self.phis) });
        self.desde = hasta;
        Ok(())
    }
}

// -- Leer las instrucciones --------------------------------------------------

/// Los bits de un valor en un registro: el suyo, o una constante (un entero
/// o `undef`, que se lee como 0) puesta en uno.
pub(super) fn bits(c: &mut Compilador, id: usize) -> Result<Reg, NoPrograma> {
    let v = match c.valores.get(id).copied() {
        Some(Valor::Float(r) | Valor::Bits(r)) => return Ok(r),
        Some(Valor::Entero(v)) => v as i32 as u32,
        Some(Valor::Indefinido) => 0,
        _ => return Err(NoPrograma::Forma("un operando que no es un numero (ni float, ni entero, ni i1)")),
    };
    literal(c, v)
}

fn literal(c: &mut Compilador, v: u32) -> Result<Reg, NoPrograma> {
    if let Some(&(_, r)) = c.literales.iter().find(|x| x.0 == v) {
        return Ok(r);
    }
    let r = c.registro(f32::from_bits(v))?;
    c.literales.push((v, r));
    Ok(r)
}

/// `br bb` o `br i1 c, si, no`: cierra el bloque.
pub(super) fn br(c: &mut Compilador, o: &mut Operandos) -> Result<(), NoPrograma> {
    let si = o.crudo()? as usize;
    let fin = if o.i < o.ops.len() {
        let no = o.crudo()? as usize;
        let cond = o.solo()?;
        Fin::Si { c: bits(c, cond)?, si, no }
    } else {
        Fin::Salto(si)
    };
    let hasta = c.ops.len();
    c.bloques.cerrar(fin, hasta)
}

/// `phi ty [v0, bb0], ...`: los valores van con SIGNO (pueden ser de mas
/// adelante); su registro, nuevo, se escribe en cada arista.
pub(super) fn phi(c: &mut Compilador, o: &mut Operandos, floats: &[bool]) -> Result<(), NoPrograma> {
    let tipo = o.crudo()? as usize;
    let yo = o.siguiente as i64;
    let mut entrantes = Vec::new();
    while o.i + 1 < o.ops.len() {
        let v = con_signo(o.crudo()?);
        let bb = o.crudo()? as usize;
        let id = yo - v;
        entrantes.push((usize::try_from(id).map_err(|_| NoPrograma::Forma("un phi con un valor imposible"))?, bb));
    }
    let d = c.registro(0.0)?;
    c.bloques.phis.push(Phi { d, entrantes });
    c.valores.push(if floats.get(tipo).copied().unwrap_or(false) { Valor::Float(d) } else { Valor::Bits(d) });
    Ok(())
}

/// `fcmp`/`icmp` (CMP2: `[a, b, predicado, banderas?]`): 0xFFFFFFFF o 0.
pub(super) fn cmp(c: &mut Compilador, o: &mut Operandos) -> Result<(), NoPrograma> {
    let a = o.con_tipo()?;
    let b = o.solo()?;
    let pred = o.crudo()?;
    let (ra, rb) = (bits(c, a)?, bits(c, b)?);
    // Las desordenadas de float son la NEGACION de una ordenada: ULT es
    // !(OGE). UNE es `Distinto` tal cual (cierta con un NaN).
    let (como, entero, negar) = match pred {
        1 => (Comparacion::Igual, false, false),
        2 => (Comparacion::Mayor, false, false),
        3 => (Comparacion::MayorIgual, false, false),
        4 => (Comparacion::Menor, false, false),
        5 => (Comparacion::MenorIgual, false, false),
        10 => (Comparacion::MenorIgual, false, true),
        11 => (Comparacion::Menor, false, true),
        12 => (Comparacion::MayorIgual, false, true),
        13 => (Comparacion::Mayor, false, true),
        14 => (Comparacion::Distinto, false, false),
        32 => (Comparacion::Igual, true, false),
        33 => (Comparacion::Distinto, true, false),
        38 => (Comparacion::Mayor, true, false),
        39 => (Comparacion::MayorIgual, true, false),
        40 => (Comparacion::Menor, true, false),
        41 => (Comparacion::MenorIgual, true, false),
        34..=37 => return Err(NoPrograma::Forma("una comparacion de enteros SIN signo: todavia no")),
        _ => return Err(NoPrograma::Forma("una comparacion de floats ONE, UEQ, ORD, UNO o constante: todavia no")),
    };
    let d = c.registro(0.0)?;
    c.ops.push(Op::Compara { d, a: ra, b: rb, como, entero });
    let d = if negar {
        // !x = (x == 0), como enteros.
        let cero = literal(c, 0)?;
        let n = c.registro(0.0)?;
        c.ops.push(Op::Compara { d: n, a: d, b: cero, como: Comparacion::Igual, entero: true });
        n
    } else {
        d
    };
    c.valores.push(Valor::Bits(d));
    Ok(())
}

/// `select i1 c, a, b` (VSELECT: `[a, b, c]`).
pub(super) fn select(c: &mut Compilador, o: &mut Operandos) -> Result<(), NoPrograma> {
    let a = o.con_tipo()?;
    let b = o.solo()?;
    let cond = o.con_tipo()?;
    let float = matches!(c.valores.get(a), Some(Valor::Float(_)));
    let (ra, rb, rc) = (bits(c, a)?, bits(c, b)?, bits(c, cond)?);
    let d = c.registro(0.0)?;
    c.ops.push(Op::Elige { d, c: rc, a: ra, b: rb });
    c.valores.push(if float { Valor::Float(d) } else { Valor::Bits(d) });
    Ok(())
}

/// Si el BINOP de `o` es de enteros (su primer operando lo es).
pub(super) fn es_entero(c: &Compilador, o: &Operandos) -> Result<bool, NoPrograma> {
    let mut p = o.copia();
    let a = p.con_tipo()?;
    Ok(matches!(c.valores.get(a), Some(Valor::Bits(_) | Valor::Entero(_))))
}

/// `add`/`sub` de enteros: `SumaEntera` (un `sub` de una constante, sumando
/// su contrario). Las demas, todavia no.
pub(super) fn binop_entero(c: &mut Compilador, o: &mut Operandos) -> Result<(), NoPrograma> {
    let a = o.con_tipo()?;
    let b = o.solo()?;
    let opcode = o.crudo()?;
    let ra = bits(c, a)?;
    let rb = match (opcode, c.valores.get(b).copied()) {
        (0, _) => bits(c, b)?,
        (1, Some(Valor::Entero(k))) => literal(c, (k as i32).wrapping_neg() as u32)?,
        _ => return Err(NoPrograma::Forma("una operacion de enteros que no es add, o sub de una constante: todavia no")),
    };
    let d = c.registro(0.0)?;
    c.ops.push(Op::SumaEntera { d, a: ra, b: rb });
    c.valores.push(Valor::Bits(d));
    Ok(())
}

// -- Armar el programa -------------------------------------------------------

/// Lo que se puede emitir antes de decir que el grafo no acaba de
/// estructurarse (un bloque se repite si dos ramas llegan a el sin unirse).
const OPS_MAXIMAS: usize = 1 << 16;

/// Un bucle: su cabeza, sus bloques y su unica salida.
struct Bucle {
    cabeza: usize,
    cuerpo: Vec<bool>,
    salida: usize,
}

struct Armador<'a> {
    bloques: &'a [Bloque],
    ops: &'a [Op],
    /// Los `phi` con sus valores ya resueltos a registro: (destino, [(registro, desde)]).
    phis: Vec<Vec<(Reg, Vec<(Reg, usize)>)>>,
    ipdom: Vec<Option<usize>>,
    bucles: Vec<Bucle>,
    /// Los post-dominadores de cada bucle en SU grafo: sin las aristas a su
    /// cabeza ni a su salida (esas son Continuar y Romper, no caminos).
    ipdom_bucle: Vec<Vec<Option<usize>>>,
    /// Los bucles abiertos al emitir (indices en `bucles`).
    activos: Vec<usize>,
    out: Vec<Op>,
    temporales: Vec<f32>,
    primer_temporal: usize,
}

/// **Armar**: sin saltos, nada que hacer; con ellos, `c.ops` pasa a ser el
/// programa estructurado.
pub(super) fn armar(c: &mut Compilador) -> Result<(), NoPrograma> {
    let bloques = core::mem::take(&mut c.bloques.hechos);
    if bloques.len() <= 1 && bloques.iter().all(|b| b.phis.is_empty()) {
        return Ok(());
    }
    if bloques.len() != c.bloques.declarados {
        return Err(NoPrograma::Forma("una funcion que acaba sin cerrar todos sus bloques"));
    }
    let n = bloques.len();
    let mut phis = Vec::with_capacity(n);
    for b in &bloques {
        let mut v = Vec::new();
        for p in &b.phis {
            let mut e = Vec::new();
            for &(id, desde) in &p.entrantes {
                e.push((bits(c, id)?, desde));
            }
            v.push((p.d, e));
        }
        phis.push(v);
    }
    let succ: Vec<Vec<usize>> = bloques.iter().map(|b| sucesores(b.fin)).collect();
    if succ.iter().flatten().any(|&s| s >= n) {
        return Err(NoPrograma::Forma("un br a un bloque que no existe"));
    }
    let mut pred = vec![Vec::new(); n];
    for (b, ss) in succ.iter().enumerate() {
        for &s in ss {
            pred[s].push(b);
        }
    }
    let idom = dominadores(n + 1, 0, &succ_con_vacio(&succ), &pred_con_vacio(&pred));
    // Post-dominadores: el grafo al reves, desde un `ret` virtual (el n).
    let mut rsucc = pred.clone();
    let mut rpred = succ.clone();
    rsucc.push(Vec::new());
    rpred.push(Vec::new());
    for (b, bl) in bloques.iter().enumerate() {
        if bl.fin == Fin::Ret {
            rsucc[n].push(b);
            rpred[b].push(n);
        }
    }
    let ipdom = dominadores(n + 1, n, &rsucc, &rpred);
    let alcanzables: Vec<bool> = (0..n).map(|b| b == 0 || idom[b].is_some()).collect();
    if (0..n).any(|b| alcanzables[b] && b != 0 && ipdom[b].is_none()) {
        return Err(NoPrograma::Forma("un bucle sin salida (no llega nunca al ret)"));
    }
    let bucles = bucles(n, &succ, &pred, &idom, &alcanzables)?;
    let ipdom_bucle = bucles.iter().map(|l| post_dominadores_de(l, n, &succ)).collect();
    let primer_temporal = c.iniciales.len();
    let mut a = Armador { bloques: &bloques, ops: &c.ops, phis, ipdom, bucles, ipdom_bucle, activos: Vec::new(), out: Vec::new(), temporales: Vec::new(), primer_temporal };
    a.emitir(0, None)?;
    // Un Continuar justo antes de su FinBucle no hace nada.
    let mut out: Vec<Op> = Vec::with_capacity(a.out.len());
    for op in a.out {
        if op == Op::FinBucle && out.last() == Some(&Op::Continuar) {
            out.pop();
        }
        out.push(op);
    }
    let temporales = a.temporales;
    c.iniciales.extend(temporales);
    c.ops = out;
    Ok(())
}

fn sucesores(f: Fin) -> Vec<usize> {
    match f {
        Fin::Ret => Vec::new(),
        Fin::Salto(t) => vec![t],
        Fin::Si { si, no, .. } => vec![si, no],
    }
}

fn succ_con_vacio(s: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut v = s.to_vec();
    v.push(Vec::new());
    v
}

fn pred_con_vacio(p: &[Vec<usize>]) -> Vec<Vec<usize>> {
    succ_con_vacio(p)
}

/// El postorden de lo alcanzable desde `entrada` (una busqueda en
/// profundidad sin recursion).
fn postorden(n: usize, entrada: usize, succ: &[Vec<usize>]) -> Vec<usize> {
    let mut orden = Vec::with_capacity(n);
    let mut visto = vec![false; n];
    let mut pila = vec![(entrada, 0usize)];
    visto[entrada] = true;
    while let Some(&mut (x, ref mut k)) = pila.last_mut() {
        if let Some(&s) = succ[x].get(*k) {
            *k += 1;
            if !visto[s] {
                visto[s] = true;
                pila.push((s, 0));
            }
        } else {
            orden.push(x);
            pila.pop();
        }
    }
    orden
}

/// **Dominadores** (Cooper, Harvey y Kennedy): el dominador inmediato de
/// cada nodo alcanzable desde `entrada`; `None` si no se alcanza (y la
/// entrada, el suyo propio).
fn dominadores(n: usize, entrada: usize, succ: &[Vec<usize>], pred: &[Vec<usize>]) -> Vec<Option<usize>> {
    let orden = postorden(n, entrada, succ);
    let mut num = vec![usize::MAX; n];
    for (i, &x) in orden.iter().enumerate() {
        num[x] = i;
    }
    let mut idom = vec![None; n];
    idom[entrada] = Some(entrada);
    let mut cambia = true;
    while cambia {
        cambia = false;
        for &x in orden.iter().rev().filter(|&&x| x != entrada) {
            let mut nuevo: Option<usize> = None;
            for &p in &pred[x] {
                if idom[p].is_none() {
                    continue;
                }
                nuevo = Some(match nuevo {
                    None => p,
                    Some(mut a) => {
                        let mut b = p;
                        while a != b {
                            while num[a] < num[b] {
                                a = idom[a].unwrap_or(entrada);
                            }
                            while num[b] < num[a] {
                                b = idom[b].unwrap_or(entrada);
                            }
                        }
                        a
                    }
                });
            }
            if nuevo.is_some() && idom[x] != nuevo {
                idom[x] = nuevo;
                cambia = true;
            }
        }
    }
    idom[entrada] = None;
    idom
}

/// Los post-dominadores DENTRO del bucle `l`: su grafo sin las aristas que
/// vuelven a la cabeza ni las que salen; quien se queda sin sucesores acaba
/// en un sumidero (el `n`). La union de un `si` dentro del bucle es esto: un
/// `break` o un `continue` de una rama no la mueve.
fn post_dominadores_de(l: &Bucle, n: usize, succ: &[Vec<usize>]) -> Vec<Option<usize>> {
    let queda = |v: usize| l.cuerpo[v] && v != l.cabeza;
    let mut rsucc = vec![Vec::new(); n + 1];
    let mut rpred = vec![Vec::new(); n + 1];
    for x in (0..n).filter(|&x| l.cuerpo[x]) {
        let mut alguno = false;
        for &v in succ[x].iter().filter(|&&v| queda(v)) {
            rsucc[v].push(x);
            rpred[x].push(v);
            alguno = true;
        }
        if !alguno {
            rsucc[n].push(x);
            rpred[x].push(n);
        }
    }
    dominadores(n + 1, n, &rsucc, &rpred)
}

/// Si `a` domina a `b`.
fn domina(idom: &[Option<usize>], a: usize, mut b: usize) -> bool {
    loop {
        if a == b {
            return true;
        }
        match idom[b] {
            Some(x) => b = x,
            None => return false,
        }
    }
}

/// Los bucles naturales: cada salto a un bloque que lo domina es una vuelta.
/// Un salto hacia atras a uno que NO lo domina es un grafo no reducible.
fn bucles(n: usize, succ: &[Vec<usize>], pred: &[Vec<usize>], idom: &[Option<usize>], alcanzable: &[bool]) -> Result<Vec<Bucle>, NoPrograma> {
    let mut v: Vec<Bucle> = Vec::new();
    for u in (0..n).filter(|&u| alcanzable[u]) {
        for &h in &succ[u] {
            if !domina(idom, h, u) {
                continue;
            }
            let k = match v.iter().position(|b| b.cabeza == h) {
                Some(k) => k,
                None => {
                    let mut cuerpo = vec![false; n];
                    cuerpo[h] = true;
                    v.push(Bucle { cabeza: h, cuerpo, salida: usize::MAX });
                    v.len() - 1
                }
            };
            let mut pila = vec![u];
            while let Some(x) = pila.pop() {
                if !v[k].cuerpo[x] {
                    v[k].cuerpo[x] = true;
                    pila.extend(pred[x].iter().copied().filter(|&p| alcanzable[p]));
                }
            }
        }
    }
    for b in v.iter_mut() {
        let mut salidas: Vec<usize> = (0..n).filter(|&x| b.cuerpo[x]).flat_map(|x| succ[x].iter().copied()).filter(|&s| !b.cuerpo[s]).collect();
        salidas.sort_unstable();
        salidas.dedup();
        match salidas.as_slice() {
            [s] => b.salida = *s,
            [] => return Err(NoPrograma::Forma("un bucle sin salida")),
            _ => return Err(NoPrograma::Forma("un bucle con mas de una salida: todavia no")),
        }
    }
    // Un salto hacia ATRAS (en el orden de la busqueda) a uno que no lo
    // domina: un bucle con dos entradas, un grafo no reducible.
    let orden = postorden(n, 0, succ);
    let mut num = vec![0usize; n];
    for (i, &x) in orden.iter().enumerate() {
        num[x] = i;
    }
    for u in (0..n).filter(|&u| alcanzable[u]) {
        for &h in &succ[u] {
            if num[h] >= num[u] && !domina(idom, h, u) {
                return Err(NoPrograma::Forma("un grafo de bloques no reducible (un bucle con dos entradas)"));
            }
        }
    }
    Ok(v)
}

/// Lo que hace un salto especial: salir del bucle o volver a su cabeza.
#[derive(Clone, Copy, PartialEq)]
enum Especial {
    Romper,
    Continuar,
}

impl Armador<'_> {
    fn temporal(&mut self) -> Result<Reg, NoPrograma> {
        let r = self.primer_temporal + self.temporales.len();
        self.temporales.push(0.0);
        Reg::try_from(r).ok().filter(|&r| r < Reg::MAX - 4).ok_or(NoPrograma::Forma("un sombreador con mas de 65000 valores"))
    }

    fn poner(&mut self, op: Op) -> Result<(), NoPrograma> {
        if self.out.len() >= OPS_MAXIMAS {
            return Err(NoPrograma::Forma("un grafo de bloques que no se deja estructurar (crece sin fin)"));
        }
        self.out.push(op);
        Ok(())
    }

    /// Las copias de la arista `b -> t` (los `phi` de `t`), en paralelo.
    fn copias(&mut self, b: usize, t: usize) -> Result<bool, NoPrograma> {
        let pares: Vec<(Reg, Reg)> = self.phis[t].iter().filter_map(|(d, e)| e.iter().find(|x| x.1 == b).map(|x| (*d, x.0))).collect();
        let pisa = pares.iter().any(|&(_, s)| pares.iter().any(|&(d, _)| d == s));
        if pisa {
            let mut tmp = Vec::with_capacity(pares.len());
            for &(_, s) in &pares {
                let x = self.temporal()?;
                self.poner(Op::Copia { d: x, a: s })?;
                tmp.push(x);
            }
            for (&(d, _), x) in pares.iter().zip(tmp) {
                self.poner(Op::Copia { d, a: x })?;
            }
        } else {
            for &(d, s) in &pares {
                self.poner(Op::Copia { d, a: s })?;
            }
        }
        Ok(!pares.is_empty())
    }

    /// Si `t` es la cabeza o la salida del bucle abierto mas interno. A la de
    /// uno de FUERA no se sabe saltar todavia (un `break` de dos bucles).
    fn especial(&self, t: usize) -> Result<Option<Especial>, NoPrograma> {
        for (k, &l) in self.activos.iter().enumerate().rev() {
            let b = &self.bucles[l];
            let dentro = k + 1 == self.activos.len();
            if t == b.cabeza || t == b.salida {
                if !dentro {
                    return Err(NoPrograma::Forma("un salto que sale de dos bucles de golpe: todavia no"));
                }
                return Ok(Some(if t == b.cabeza { Especial::Continuar } else { Especial::Romper }));
            }
        }
        Ok(None)
    }

    fn hacer(&mut self, e: Especial) -> Result<(), NoPrograma> {
        self.poner(if e == Especial::Romper { Op::Romper } else { Op::Continuar })
    }

    /// La arista `b -> t` sin condicion: sus copias, y o un salto especial
    /// (nada despues) o el bloque por el que se sigue.
    fn arista(&mut self, b: usize, t: usize) -> Result<Option<usize>, NoPrograma> {
        self.copias(b, t)?;
        match self.especial(t)? {
            Some(e) => {
                self.hacer(e)?;
                Ok(None)
            }
            None => Ok(Some(t)),
        }
    }

    /// **Emitir** desde el bloque `b` hasta `parar` (sin emitirlo), o hasta
    /// que el camino acabe (`ret`, Romper, Continuar).
    fn emitir(&mut self, mut b: usize, parar: Option<usize>) -> Result<(), NoPrograma> {
        loop {
            if Some(b) == parar {
                return Ok(());
            }
            if let Some(l) = self.bucles.iter().position(|x| x.cabeza == b).filter(|l| !self.activos.contains(l)) {
                if self.activos.len() >= crate::dxil::programa::ANIDADO_MAXIMO / 2 {
                    return Err(NoPrograma::Forma("bucles anidados de mas"));
                }
                self.poner(Op::Bucle)?;
                self.activos.push(l);
                self.emitir(b, None)?;
                self.activos.pop();
                self.poner(Op::FinBucle)?;
                b = self.bucles[l].salida;
                continue;
            }
            let bl = &self.bloques[b];
            for k in bl.desde..bl.hasta {
                let op = self.ops[k];
                self.poner(op)?;
            }
            let siguiente = match self.bloques[b].fin {
                Fin::Ret => None,
                Fin::Salto(t) => self.arista(b, t)?,
                Fin::Si { c, si, no } => self.condicional(b, c, si, no)?,
            };
            match siguiente {
                Some(n) => b = n,
                None => return Ok(()),
            }
        }
    }

    /// `br i1 c, si, no`.
    fn condicional(&mut self, b: usize, c: Reg, si: usize, no: usize) -> Result<Option<usize>, NoPrograma> {
        let (es_si, es_no) = (self.especial(si)?, self.especial(no)?);
        if es_si.is_some() || es_no.is_some() {
            // Un `break` sin copias es un RomperSi (sin `si` alrededor).
            let sin_copias = |a: &Self, t: usize| a.phis[t].iter().all(|(_, e)| e.iter().all(|x| x.1 != b));
            if es_no.is_none() && es_si == Some(Especial::Romper) && sin_copias(self, si) {
                self.poner(Op::RomperSi { c, si_cero: false })?;
                return self.arista(b, no);
            }
            if es_si.is_none() && es_no == Some(Especial::Romper) && sin_copias(self, no) {
                self.poner(Op::RomperSi { c, si_cero: true })?;
                return self.arista(b, si);
            }
            self.poner(Op::Si { c })?;
            if let Some(e) = es_si {
                self.copias(b, si)?;
                self.hacer(e)?;
            }
            if let Some(e) = es_no {
                self.poner(Op::SiNo)?;
                self.copias(b, no)?;
                self.hacer(e)?;
            }
            self.poner(Op::FinSi)?;
            return match (es_si, es_no) {
                (Some(_), Some(_)) => Ok(None),
                (Some(_), None) => self.arista(b, no),
                _ => self.arista(b, si),
            };
        }
        // Las dos ramas hasta su union: el post-dominador inmediato -- dentro
        // de un bucle, el de SU grafo -- (si no hay, cada rama acaba saliendo
        // o volviendo por su cuenta).
        let union = match self.activos.last() {
            Some(&l) => self.ipdom_bucle[l][b].filter(|&m| m < self.bloques.len() && self.bucles[l].cuerpo[m] && self.bucles[l].cabeza != m),
            None => self.ipdom[b].filter(|&m| m < self.bloques.len()),
        };
        self.poner(Op::Si { c })?;
        self.rama(b, si, union)?;
        let sino = self.out.len();
        self.poner(Op::SiNo)?;
        self.rama(b, no, union)?;
        if self.out.len() == sino + 1 {
            self.out.pop();
        }
        self.poner(Op::FinSi)?;
        Ok(union)
    }

    /// Una rama de un `Si`: la arista y lo de detras, hasta la union.
    fn rama(&mut self, b: usize, t: usize, union: Option<usize>) -> Result<(), NoPrograma> {
        match self.arista(b, t)? {
            Some(n) if Some(n) != union => self.emitir(n, union),
            _ => Ok(()),
        }
    }
}
