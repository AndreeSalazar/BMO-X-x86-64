//! O1: initialize a NEW record in its destination. Building a table, then
//! a temporary record, then copying that record to the local tripled its
//! stack use. Assignment to an existing value still uses its alias-safe path.
use super::{Class, Place, E1};
use bmo_titan_front::ir::{At, Value};

impl E1<'_> {
    pub(super) fn initialize_record(&mut self, dst: Place, record: usize, items: &[Value], at: At) -> Result<(), String> {
        for (k, item) in items.iter().enumerate() {
            let (offset, expected, ty) = self.forms.field_k(record, k);
            let field = dst.at(offset);
            match item {
                Value::Record(inner, values, inner_at) if expected == Class::Record(*inner) => {
                    self.initialize_record(field, *inner, values, *inner_at)?;
                }
                Value::Repeat(value, n, _) if self.repeat_in_place(value, *n, Some(&ty))? => {
                    let (source, class) = self.eval(value)?;
                    let size = self.forms.size(&class);
                    self.each_cell(*n, &[(field, size)], &mut |e, cells| {
                        e.clone_at(cells[0], source, &class);
                        Ok(())
                    })?;
                    if self.forms.has_decp(&ty) { self.fit(field, &ty, at)?; }
                }
                _ => {
                    let (source, actual) = self.eval(item)?;
                    self.convert(field, source, &actual, &expected, Some(&ty), at)?;
                }
            }
        }
        Ok(())
    }
}
