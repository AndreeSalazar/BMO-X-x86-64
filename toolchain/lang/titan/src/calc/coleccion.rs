//! `calc::coleccion` -- THE LISTS AND THE MAPS, run when compiling (level 13,
//! `docs/plan/PLAN_LISTAS_Y_MAPAS.md`).
//!
//! ```text
//!    [int]          Const::List: the type of its cells and the cells
//!    {text: int}    Const::Map: the types of keys and values, and the
//!                   entries in the order they went in (D4)
//!    get / pop      a case of `Opcion`: Hay(v) or NoHay (D3)
//! ```
//!
//! A list carries the TYPE of what it holds so that `push(mut l, 2)` into a
//! `[dec]` keeps a decimal, as `let x: dec = 2` does. `[1, 2]` written is a
//! table until it goes where a list is said (`let l: [int] = [1, 2]`, a
//! parameter, a field, `l = [1, 2]` into a list): there it becomes one.
//!
//! A list or a map is a VALUE like any other (D2): `let b = a` is another
//! one, and `push(mut a, x)` changes only `a` -- here it is `a = push(a, x)`
//! (`ir.rs`), and the old list is taken, not copied, to build the new one.

use super::*;

/// May a value of class `got` go where the collection `want` is said? Cell
/// by cell as `fits` says (an int where a dec goes), and `[]` / `{}` (ANY)
/// wherever a list or a map goes.
pub(super) fn fits_collection(want: &Class, got: &Class) -> bool {
    let cell = |w: &Class, g: &Class| *g == Class::Any || fits(w, g);
    match (want, got) {
        (Class::List(w), Class::List(g) | Class::Table(g, _)) => cell(w, g),
        (Class::Map(wk, wv), Class::Map(gk, gv)) => cell(wk, gk) && cell(wv, gv),
        (Class::Opt(w), Class::Opt(g)) => cell(w, g),
        _ => false,
    }
}

/// Is there a `[]` or a `{}` in this class that nothing said the type of?
pub(super) fn has_any(c: &Class) -> bool {
    match c {
        Class::Any => true,
        Class::List(c) | Class::Opt(c) | Class::Table(c, _) => has_any(c),
        Class::Map(k, v) => has_any(k) || has_any(v),
        _ => false,
    }
}

/// May a value of this class be the KEY of a map? A number, a text, a yes/no,
/// or a record of those: what has one equality nobody can doubt. Not a
/// decimal (1.0 and 1.00 are the same number, and would be two keys to a
/// careless machine), not a list (it changes).
pub(super) fn key_class(c: &Class, d: Defs) -> bool {
    match c {
        Class::Int | Class::Text | Class::Bool => true,
        Class::Record(t) => d.types[*t].fields.iter().all(|f| key_class(&of_ty(&f.ty, d), d)),
        _ => false,
    }
}

/// The type of a value, written: what a map written (`{"a": 1}`) holds.
fn ty_of(c: &Const, d: Defs) -> Ty {
    match c {
        Const::Int(_) => Ty::Int,
        Const::Dec(..) => Ty::Dec,
        Const::Text(_) => Ty::Text,
        Const::Bool(_) => Ty::Bool,
        Const::F32(_) => Ty::F32,
        Const::Record(t, _) => Ty::Named(d.types[*t].name.clone()),
        Const::Variant(e, ..) => Ty::Named(d.enums[*e].name.clone()),
        Const::Table(items) => Ty::Table(Box::new(items.first().map_or(Ty::Int, |i| ty_of(i, d))), items.len()),
        Const::List(t, _) => Ty::List(Box::new(t.clone())),
        Const::Map(k, v, _) => Ty::Map(Box::new(k.clone()), Box::new(v.clone())),
    }
}

/// `{"a": 1, "b": 2.5}`: the map, of the types its first entry says (a
/// decimal anywhere makes the values decimals, as in a table). A key written
/// twice keeps its first place and its last value, as `put` would.
pub(super) fn map_written(items: Vec<(Const, Const)>, d: Defs, at: At) -> Result<Const, Message> {
    let Some((k0, _)) = items.first() else {
        return Ok(Const::Map(Ty::Named("?".into()), Ty::Named("?".into()), Vec::new()));
    };
    let kt = ty_of(k0, d);
    let vt = if items.iter().any(|(_, v)| matches!(v, Const::Dec(..))) { Ty::Dec } else { ty_of(&items[0].1, d) };
    let mut out: Vec<(Const, Const)> = Vec::with_capacity(items.len());
    for (k, v) in items {
        let v = fit_into(v, Some(&vt), d, at)?;
        match out.iter_mut().find(|(j, _)| same(j, &k)) {
            Some(e) => e.1 = v,
            None => out.push((k, v)),
        }
    }
    Ok(Const::Map(kt, vt, out))
}

/// A value going where a list, a map or an `Opcion` is said: it takes that
/// type, cell by cell, entry by entry.
pub(super) fn fit_collection(c: Const, ty: &Ty, d: Defs, at: At) -> Result<Const, Message> {
    match (c, ty) {
        (Const::Table(items) | Const::List(_, items), Ty::List(t)) => items.into_iter().map(|i| fit_into(i, Some(t), d, at)).collect::<Result<Vec<_>, _>>().map(|v| Const::List((**t).clone(), v)),
        (Const::Map(_, _, items), Ty::Map(kt, vt)) => {
            let mut out = Vec::with_capacity(items.len());
            for (k, v) in items {
                out.push((fit_into(k, Some(kt), d, at)?, fit_into(v, Some(vt), d, at)?));
            }
            Ok(Const::Map((**kt).clone(), (**vt).clone(), out))
        }
        (Const::Variant(e, v, items), Ty::Opt(t)) => items.into_iter().map(|i| fit_into(i, Some(t), d, at)).collect::<Result<Vec<_>, _>>().map(|x| Const::Variant(e, v, x)),
        (c, _) => Ok(c),
    }
}

/// `l = [1, 2]` into a local that holds a list: the new value takes the old
/// one's type (a table becomes a list of it; a map, a map of it).
pub(super) fn like(old: &Const, v: Const, d: Defs, at: At) -> Result<Const, Message> {
    match old {
        Const::List(t, _) => fit_collection(v, &Ty::List(Box::new(t.clone())), d, at),
        Const::Map(k, w, _) => fit_collection(v, &Ty::Map(Box::new(k.clone()), Box::new(w.clone())), d, at),
        _ => Ok(v),
    }
}

/// `Hay(v)` or `NoHay`: the case of the prelude's `Opcion`.
fn opcion(d: Defs, found: Option<Const>) -> Const {
    let e = d.enums.iter().position(|e| crate::prelude::is_opcion(&e.name)).expect("prelude: get and pop bring Opcion");
    match found {
        Some(v) => Const::Variant(e, 0, vec![v]),
        None => Const::Variant(e, 1, Vec::new()),
    }
}

impl Run<'_, '_> {
    /// One fn of the library of lists and maps. The ones that CHANGE their
    /// first value come as `l = push(l, x)`: the others are calculated
    /// first, and then the old `l` is TAKEN (the `=` gives it its new value
    /// right after), so a push costs one cell, not a copy of the list.
    pub(super) fn lib(&mut self, lib: Lib, args: &[Value], at: At, known: &mut Vec<Option<Const>>) -> Result<Const, Message> {
        let d = self.m.defs();
        let mut rest = Vec::with_capacity(args.len().saturating_sub(1));
        for a in &args[1..] {
            rest.push(self.ev(a, known)?);
        }
        let changes = matches!(lib, Lib::Push | Lib::DropLast | Lib::Put | Lib::Remove);
        let first = match &args[0] {
            Value::Local(l, _) if changes => known[*l].take().expect("juez: every local read has a value"),
            other => self.ev(other, known)?,
        };
        let mut rest = rest.into_iter();
        let mut next = || rest.next().expect("check: the library fn has its values");
        Ok(match (lib, first) {
            (Lib::Push, Const::List(t, mut items)) => {
                items.push(fit_into(next(), Some(&t), d, at)?);
                Const::List(t, items)
            }
            (Lib::Last, Const::List(_, items)) => opcion(d, items.last().cloned()),
            (Lib::DropLast, Const::List(t, mut items)) => {
                items.pop();
                Const::List(t, items)
            }
            (Lib::Put, Const::Map(kt, vt, mut items)) => {
                let k = fit_into(next(), Some(&kt), d, at)?;
                let v = fit_into(next(), Some(&vt), d, at)?;
                match items.iter_mut().find(|(j, _)| same(j, &k)) {
                    Some(e) => e.1 = v,
                    None => items.push((k, v)),
                }
                Const::Map(kt, vt, items)
            }
            (Lib::Remove, Const::Map(kt, vt, mut items)) => {
                let k = next();
                items.retain(|(j, _)| !same(j, &k));
                Const::Map(kt, vt, items)
            }
            (Lib::Get, Const::Map(_, _, items)) => {
                let k = next();
                opcion(d, items.into_iter().find(|(j, _)| same(j, &k)).map(|(_, v)| v))
            }
            (Lib::Has, Const::Map(_, _, items)) => {
                let k = next();
                Const::Bool(items.iter().any(|(j, _)| same(j, &k)))
            }
            (Lib::Turn, c @ (Const::Table(_) | Const::List(..))) => cell(&c, &next(), at)?.clone(),
            (Lib::Turn, Const::Map(_, _, items)) => match next() {
                Const::Int(i) => items[i as usize].0.clone(),
                _ => unreachable!("ir: a turn counts with an int"),
            },
            (lib, other) => unreachable!("classes: `{}` on {:?}", lib.name(), other),
        })
    }
}
