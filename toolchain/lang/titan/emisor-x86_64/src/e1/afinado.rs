//! ** LO QUE AFINA E1 SIN QUE TITAN++ SEPA DE CPU (`docs/plan/PLAN_TITAN_AFINADO.md`).
//!
//! La ley del propietario (16-10): TITAN++ dice QUE se hace; el emisor decide
//! COMO. Cada cosa de aqui hace el mismo programa con menos instrucciones, y
//! se mide en el DUELO (`metro --duelo`: el mismo programa en C y en TITAN++).
//!
//! ```text
//!    AF-1  lo que una fn solo LEE le llega como puntero, sin copia
//!    AF1   `if` / `while` sobre una comparacion de enteros (y su `and`):
//!          el salto con el `cmp` mismo, sin el si/no de en medio
//! ```
//! (AF0, los numeros cortos, vive en `E1::imm`.)

use super::E1;
use bmo_lower::x86::{self, RAX, RCX};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{End, Function, Lib, Op, PathStep, Value};
use bmo_titan_front::tree::Mode;

impl<'a> E1<'a> {
    /// ** AF1: el SALTO de un `if` o un `while` cuya condicion es comparar
    /// dos numeros enteros (`<`, `<=`, `>`, `>=`, `==`, `!=`): el `cmp` y el
    /// salto de cuando NO se cumple, sin el si/no de en medio. Da el sitio
    /// del salto a parchear, o `None` si la condicion es otra cosa (y nada
    /// emitido). Si calcular un lado dejo algo con monton que soltar, se
    /// suelta ANTES del `cmp` (soltar llama y pisa las banderas): sus dos
    /// valores ya estan en su sitio de la pila, que no se suelta.
    /// Si la condicion se puede saltar asi (`salto_directo`): una
    /// comparacion de enteros, o un `and` de dos que se pueden.
    pub(super) fn se_funde(&self, cond: &Value) -> Result<bool, String> {
        let Value::Bin(op, a, b, _) = cond else { return Ok(false) };
        if *op == "and" {
            return Ok(self.se_funde(a)? && self.se_funde(b)?);
        }
        if !matches!(*op, "<" | "<=" | ">" | ">=" | "==" | "!=") {
            return Ok(false);
        }
        let entero = |c: &Class| matches!(c, Class::Int | Class::Byte);
        Ok(entero(&self.class_of(a)?) && entero(&self.class_of(b)?))
    }

    /// Los saltos de cuando NO se cumple (`se_funde` dijo que si). Un `and`
    /// salta en el primero que no se cumpla, y el de la derecha solo se
    /// calcula si el de la izquierda se cumplio: el mismo atajo de siempre.
    pub(super) fn salto_directo(&mut self, cond: &Value) -> Result<Vec<usize>, String> {
        let Value::Bin(op, a, b, _) = cond else { return Ok(Vec::new()) };
        if *op == "and" {
            let mut v = self.salto_directo(a)?;
            v.extend(self.salto_directo(b)?);
            return Ok(v);
        }
        let no_se_cumple = match *op {
            "<" => 0x8D,
            "<=" => 0x8F,
            ">" => 0x8E,
            ">=" => 0x8C,
            "==" => 0x85,
            _ => 0x84,
        };
        // AF2b: contra un numero escrito, el numero va DENTRO del `cmp`
        if let Value::Int(v, _) = &**b {
            if let Ok(v) = i32::try_from(*v) {
                let (mut pa, _) = self.eval(a)?;
                if !self.owned.is_empty() {
                    let t = self.temp(8);
                    self.load(pa, RAX);
                    self.store(t, RAX);
                    pa = t;
                    self.drop_owned_from(0);
                }
                self.load(pa, RAX);
                x86::cmp_r64_imm32(&mut self.code, RAX, v);
                return Ok(vec![self.jcc(no_se_cumple)]);
            }
        }
        // `a` se guarda aparte solo si calcular `b` puede tocar lo que
        // leyo (una llamada); un local o un numero escrito, no
        let simple = |v: &Value| matches!(v, Value::Local(..) | Value::Int(..) | Value::Bool(..));
        let (mut pa, _) = self.eval(a)?;
        if !simple(b) {
            let t = self.temp(8);
            self.load(pa, RAX);
            self.store(t, RAX);
            pa = t;
        }
        let (mut pb, _) = self.eval(b)?;
        if !self.owned.is_empty() {
            let t = self.temp(8);
            self.load(pb, RAX);
            self.store(t, RAX);
            pb = t;
            self.drop_owned_from(0);
        }
        self.load(pa, RAX);
        self.load(pb, RCX);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        Ok(vec![self.jcc(no_se_cumple)])
    }
}

/// ** PRESTADO PARA LEER (16-10, LE2 de PL18 de PLAN_ECCE). `fn f(n: T)`
/// es una COPIA (nivel 7), y E1 la hacia entera al entrar: una `[byte]` de
/// 20 KB pasada a una fn pequenia costaba 200 mil pasos por llamada (medido:
/// la letra de ECCE tardaba 100 millones en pintar una cabecera). Si la fn
/// NUNCA puede cambiar lo que le llega ni quedarselo, la copia no se
/// distingue del original: va un PUNTERO, como con `mut`, y no se suelta al
/// volver (es del que llama).
///
/// Cuando, y solo cuando: llega copiado; es una lista, una tabla o un mapa
/// (con monton) sin `dec` dentro (su `fit` es la copia misma); y en todo el
/// cuerpo no se le da otro valor, ni se presta (`mut`) ni se entrega
/// (`take`), ni se GUARDA entero en otro sitio (un `let`, un `=`, un
/// `return`, dentro de una tabla, un registro, un caso, un mapa o un
/// `push`). Leerlo -- una celda, `len`, `for`, compararlo, pasarlo COPIADO a
/// otra fn -- no lo cambia. Quien llama no puede tocarlo mientras: la fn es
/// lo unico que corre, y prestar y leer lo mismo en una llamada es T0076.
pub(super) fn solo_se_lee(f: &Function, i: usize, c: &Class) -> bool {
    if f.modes.get(i).copied().unwrap_or(Mode::Copy) != Mode::Copy || f.dispatch.is_some() || f.gpu {
        return false;
    }
    if !matches!(c, Class::List(_) | Class::Map(..) | Class::Table(..)) || format!("{:?}", f.params[i].1).contains("Dec") || format!("{c:?}").contains("Dec") {
        return false;
    }
    let l = f.params[i].0;
    let toca = |v: &Value| se_queda(v, l, true);
    let lee = |v: &Value| se_queda(v, l, false);
    for b in &f.blocks {
        for op in &b.ops {
            let mal = match op {
                Op::Let { local, value, .. } | Op::Set { local, value, .. } => *local == l || toca(value),
                Op::SetAt { local, path, value, .. } => *local == l || toca(value) || path.iter().any(|p| matches!(p, PathStep::Index(v) if lee(v))),
                Op::Write { parts: vs, .. } | Op::Call { args: vs, .. } | Op::Director { args: vs, .. } => vs.iter().any(lee),
                Op::Drop { .. } => false,
            };
            if mal {
                return false;
            }
        }
        let mal = match &b.end {
            End::Return(Some(v)) => toca(v),
            End::Branch { cond, .. } => lee(cond),
            _ => false,
        };
        if mal {
            return false;
        }
    }
    true
}

/// Si el valor `v` presta o entrega el local `l`, o lo GUARDA entero (`aqui`:
/// `v` mismo va a un sitio que se queda con el).
fn se_queda(v: &Value, l: usize, aqui: bool) -> bool {
    let guarda = |v: &Value| se_queda(v, l, true);
    let lee = |v: &Value| se_queda(v, l, false);
    match v {
        Value::Local(k, _) => aqui && *k == l,
        Value::Lend(_, k, _) => *k == l,
        Value::Int(..) | Value::Text(..) | Value::Bool(..) | Value::Dec(..) | Value::F32(..) | Value::Read(..) => false,
        Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) => items.iter().any(guarda),
        Value::Map(items, _) => items.iter().any(|(k, w)| guarda(k) || guarda(w)),
        Value::Repeat(w, _, _) => guarda(w),
        Value::Lib(lib, args, _) => match lib {
            // leer: la celda de un `for`, `get`, `has`, el ultimo
            Lib::Turn | Lib::Get | Lib::Has | Lib::Last => args.iter().any(lee),
            _ => args.iter().enumerate().any(|(k, a)| if k == 0 { lee(a) } else { guarda(a) }),
        },
        Value::Call(_, args, _) | Value::Director(_, args, _) => args.iter().any(lee),
        Value::Index(b, i, _) | Value::Bin(_, b, i, _) => lee(b) || lee(i),
        Value::Field(w, _, _) | Value::Len(w, _) | Value::Round(w, _, _) | Value::Is(w, _, _, _) | Value::Payload(w, _, _, _, _) | Value::Number(w, _, _) | Value::Byte(w, _) | Value::Lamina(w, _) | Value::Neg(w, _) | Value::Not(w, _) => lee(w),
    }
}

