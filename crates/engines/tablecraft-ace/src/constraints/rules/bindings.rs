use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, evaluation::Bindings};
use tablecraft_schema::Column;
use tablecraft_sql::select::FieldReference;

pub(super) struct RowBindings<'a> {
    pub columns: &'a [Column],
    pub row: &'a [Value],
    pub table: &'a str,
    pub field: Option<usize>,
}
impl Bindings for RowBindings<'_> {
    fn resolve(&self, field: &FieldReference) -> Result<&Value, EvalError> {
        if field
            .qualifier
            .as_ref()
            .is_some_and(|name| !name.as_str().eq_ignore_ascii_case(self.table))
        {
            return Err(EvalError::UnboundField);
        }
        let position = self
            .columns
            .iter()
            .position(|column| column.name().eq_ignore_ascii_case(field.name.as_str()))
            .ok_or(EvalError::UnboundField)?;
        if self.field.is_some_and(|selected| selected != position) {
            return Err(EvalError::UnboundField);
        }
        self.row.get(position).ok_or(EvalError::UnboundField)
    }
}
