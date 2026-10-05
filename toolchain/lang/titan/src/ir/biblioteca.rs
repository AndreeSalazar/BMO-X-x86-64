//! `ir::biblioteca` -- the library of lists and maps, LOWERED (level 13,
//! `docs/plan/PLAN_LISTAS_Y_MAPAS.md`). Cut out of `ir.rs` on 05-10 (L6a: no
//! file over 1.000 lines).
//!
//! ```text
//!    push(mut l, x)           l = push(l, x)            an `Op::Set`
//!    push(mut nave.carga, x)  nave.carga = push(...)    an `Op::SetAt`
//!    let u = pop(mut l)       u = ultimo(l), and then   two lines
//!                             l = sin_ultimo(l)
//! ```
//!
//! So the checker sees what it always sees -- a local that changes needs
//! `mut` and counts as changed --, the calculation runs it as values, and
//! the emitter does it IN PLACE (`emisor-x86_64/src/e1/coleccion.rs`).

use super::*;

/// What a lent value (or a lent PART of one) is, lowered: its local, where
/// the `mut` was written, the steps into it, and the part read as a value.
pub(super) struct Lent {
    pub local: usize,
    pub path: Vec<PathStep>,
    pub read: Value,
}

impl Lowering<'_> {
    /// `mut l`, `mut nave.carga`, `mut t[i].x`.
    pub(super) fn lent(&mut self, e: &Expr) -> Option<Lent> {
        match e {
            Expr::Lend { name, line, col, .. } => {
                let local = local_of(&mut self.locals, name);
                Some(Lent { local, path: Vec::new(), read: Value::Local(local, (*line, *col)) })
            }
            Expr::Field { base, name, line, col } => {
                let mut l = self.lent(base)?;
                l.path.push(PathStep::Field(name.clone(), (*line, *col)));
                l.read = Value::Field(Box::new(l.read), name.clone(), (*line, *col));
                Some(l)
            }
            Expr::Index { base, index, line, col } => {
                let mut l = self.lent(base)?;
                let i = value(index, &mut self.locals, self.p);
                l.path.push(PathStep::Index(i.clone()));
                l.read = Value::Index(Box::new(l.read), Box::new(i), (*line, *col));
                Some(l)
            }
            _ => None,
        }
    }

    /// The lent value gets `v`: the whole local, or the part.
    pub(super) fn change(&mut self, at_block: usize, l: Lent, v: Value, at: At) {
        let op = if l.path.is_empty() { Op::Set { local: l.local, value: v, at } } else { Op::SetAt { local: l.local, path: l.path, value: v, at } };
        self.blocks[at_block].ops.push(op);
    }

    /// `push(mut l, x)`, `put(mut m, k, v)`, `remove(mut m, k)` and `pop(mut l)`
    /// on a line of their own.
    pub(super) fn mutator(&mut self, c: &crate::tree::Call, at_block: usize) {
        let l = self.lent(&c.args[0]).expect("check: the first value goes lent");
        let lib = match c.callee.as_str() {
            "push" => Lib::Push,
            "put" => Lib::Put,
            "remove" => Lib::Remove,
            _ => Lib::DropLast,
        };
        let here = (c.line, c.col);
        let mut args = vec![l.read.clone()];
        args.extend(c.args[1..].iter().map(|a| value(a, &mut self.locals, self.p)));
        self.change(at_block, l, Value::Lib(lib, args, here), here);
    }

    /// `pop(mut l)` as a VALUE (of a `let`, an `=`, a `match`): what it
    /// gives, and the line that takes it out of `l` -- to push right after.
    pub(super) fn popped(&mut self, e: &Expr) -> Option<(Value, Lent, At)> {
        let Expr::Call { callee, args, line, col } = e else { return None };
        if callee != "pop" || self.p.functions.iter().any(|f| &f.name == callee) {
            return None;
        }
        let l = self.lent(args.first()?)?;
        let here = (*line, *col);
        Some((Value::Lib(Lib::Last, vec![l.read.clone()], here), l, here))
    }

    /// The other half of `pop`: the lent list without its last.
    pub(super) fn drop_last(&mut self, at_block: usize, l: Lent, here: At) {
        let rest = Value::Lib(Lib::DropLast, vec![l.read.clone()], here);
        self.change(at_block, l, rest, here);
    }
}
