//! O1: a field borrowed from its owner, without inventing a new IR value.
//! Field(Lend(owner)) keeps ordinary field typing and reads. Only complete
//! owners and field chains qualify; indexing and partial take remain separate.
use super::{At, Value};
use crate::tree::Mode;

pub struct FieldLoan<'a> {
    pub mode: Mode,
    pub local: usize,
    pub at: At,
    pub fields: Vec<(&'a str, At)>,
}

impl Value {
    /// Whole-value loans retain their mode. Only `mut` can continue through
    /// fields; callers use the owner for exclusivity and the path for storage.
    pub fn field_loan(&self) -> Option<FieldLoan<'_>> {
        match self {
            Value::Lend(mode, local, at) => Some(FieldLoan {
                mode: *mode, local: *local, at: *at, fields: Vec::new(),
            }),
            Value::Field(base, name, at) => {
                let mut loan = base.field_loan()?;
                if loan.mode != Mode::Mut { return None; }
                loan.fields.push((name, *at));
                Some(loan)
            }
            _ => None,
        }
    }
}
