//! **LAS FORMAS** -- cuanto ocupa en memoria cada clase de valor de TITAN++
//! cuando corre (E1), y donde vive cada parte.
//!
//! ```text
//!    int, si/no       8 bytes
//!    dec              16: sus cifras (i64) y su escala (cuantas son
//!                     decimales) -- la escala viaja con el valor, como en
//!                     el calculo (`Const::Dec`): 1.5 * 2 da 3.0
//!    texto            8 de largo y TEXT_CAP de bytes
//!    [T; n]           n veces la forma de T, seguidas
//!    type             sus campos, en el orden en que se declaran
//!    enum             8 de caso y el mayor de sus casos
//!    trait            8 de TIPO y el mayor de los tipos que lo cumplen:
//!                     quien lo recibe no sabe cual es, y lo mira al correr
//! ```
//!
//! La clase la da el frontend (`calc::Class`): aqui no se decide nada de lo
//! que un valor ES, solo donde cabe.

use super::TEXT_CAP;
use bmo_titan_front::calc::{of_ty, Class};
use bmo_titan_front::ir::Module;
use bmo_titan_front::tree::Ty;

pub(crate) struct Forms<'m> {
    m: &'m Module,
    /// Los tipos que cumplen algun trait: su posicion es su numero al correr.
    ids: Vec<String>,
}

impl<'m> Forms<'m> {
    pub fn new(m: &'m Module) -> Forms<'m> {
        let mut ids: Vec<String> = m.impls.iter().map(|(_, t)| t.clone()).collect();
        ids.sort();
        ids.dedup();
        Forms { m, ids }
    }

    pub fn class(&self, t: &Ty) -> Class {
        of_ty(t, self.m.defs())
    }

    pub fn size(&self, c: &Class) -> i32 {
        match c {
            Class::Int | Class::Bool | Class::F32 => 8,
            Class::Dec => 16,
            Class::Text => 8 + TEXT_CAP as i32,
            Class::Table(inner, n) => self.size(inner) * *n as i32,
            Class::Record(t) => self.m.types[*t].fields.iter().map(|f| self.size(&self.class(&f.ty))).sum(),
            Class::Enum(e) => 8 + (0..self.m.enums[*e].cases.len()).map(|v| self.case_size(*e, v)).max().unwrap_or(0),
            Class::Trait(k) => 8 + self.trait_types(*k).iter().map(|(_, c)| self.size(c)).max().unwrap_or(0),
        }
    }

    fn case_size(&self, e: usize, v: usize) -> i32 {
        self.m.enums[e].cases[v].fields.iter().map(|t| self.size(&self.class(t))).sum()
    }

    /// El campo `name` del registro `t`: donde empieza, su clase y su tipo.
    pub fn field(&self, t: usize, name: &str) -> (i32, Class, Ty) {
        let k = self.m.types[t].fields.iter().position(|f| f.name == name).expect("classes: the field exists");
        self.field_k(t, k)
    }

    pub fn field_k(&self, t: usize, k: usize) -> (i32, Class, Ty) {
        let fields = &self.m.types[t].fields;
        let off = fields[..k].iter().map(|f| self.size(&self.class(&f.ty))).sum();
        (off, self.class(&fields[k].ty), fields[k].ty.clone())
    }

    /// El valor `k` que lleva el caso `v` del enum `e`: donde empieza
    /// (contando el caso, 8 bytes, delante), su clase y su tipo.
    pub fn case_field(&self, e: usize, v: usize, k: usize) -> (i32, Class, Ty) {
        let fields = &self.m.enums[e].cases[v].fields;
        let off = 8 + fields[..k].iter().map(|t| self.size(&self.class(t))).sum::<i32>();
        (off, self.class(&fields[k]), fields[k].clone())
    }

    /// Los tipos que cumplen el trait `k`, y su clase.
    pub fn trait_types(&self, k: usize) -> Vec<(String, Class)> {
        let name = &self.m.traits[k].name;
        self.m.impls.iter().filter(|(t, _)| t == name).map(|(_, ty)| (ty.clone(), self.class_of_name(ty))).collect()
    }

    pub fn class_of_name(&self, name: &str) -> Class {
        match name {
            "int" => Class::Int,
            "dec" => Class::Dec,
            "text" => Class::Text,
            "bool" => Class::Bool,
            other => self.class(&Ty::Named(other.to_string())),
        }
    }

    /// El nombre del tipo de un valor de esta clase: lo que elige la fn de un
    /// trait (`calc::type_of`).
    pub fn type_name(&self, c: &Class) -> Option<String> {
        Some(match c {
            Class::Int => "int".into(),
            Class::Dec => "dec".into(),
            Class::Text => "text".into(),
            Class::Bool => "bool".into(),
            Class::Record(t) => self.m.types[*t].name.clone(),
            Class::Enum(e) => self.m.enums[*e].name.clone(),
            _ => return None,
        })
    }

    pub fn type_id(&self, name: &str) -> i64 {
        self.ids.iter().position(|t| t == name).map_or(-1, |i| i as i64)
    }

    /// Lleva un `dec(p, s)` dentro? Entonces hay que mirar sus cifras cada
    /// vez que recibe un valor (el PIC de COBOL, T0074).
    pub fn has_decp(&self, t: &Ty) -> bool {
        match t {
            Ty::DecP(..) => true,
            Ty::Table(inner, _) => self.has_decp(inner),
            Ty::Named(n) => match self.class(t) {
                Class::Record(r) => self.m.types[r].fields.iter().any(|f| self.has_decp(&f.ty)),
                Class::Enum(e) => self.m.enums[e].cases.iter().flat_map(|c| &c.fields).any(|f| self.has_decp(f)),
                _ => {
                    let _ = n;
                    false
                }
            },
            _ => false,
        }
    }
}

/// Un nombre como lo escribe `print`: sin el modulo del que viene.
pub(crate) fn short(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}
