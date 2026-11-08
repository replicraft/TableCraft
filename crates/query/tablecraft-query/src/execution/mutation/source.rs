use crate::{
    QueryError, Source,
    execution::cell::CellValue,
    source::{TextCollationId, TextPolicy},
};
use tablecraft_core::values::Value;
use tablecraft_db::Snapshot;
use tablecraft_expressions::numeric::OperandDeclaration;
use tablecraft_schema::Column;

/// Borrowed field policies accompany the frozen mutation snapshot.
pub(in crate::execution) struct MutationSource<'a> {
    pub snapshot: &'a Snapshot,
    pub text_collations: &'a [Option<TextCollationId>],
}

impl Source for MutationSource<'_> {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if !self
            .snapshot
            .policy()
            .names
            .equals(table, self.snapshot.name())
        {
            return Err(QueryError::UnknownName);
        }
        Ok(self.snapshot.columns())
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        self.columns(table)?;
        Ok(self.text_collations)
    }

    fn scan(
        &self,
        _table: &str,
        _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        Err(QueryError::Unsupported)
    }
}

impl MutationSource<'_> {
    pub(in crate::execution) fn cell(
        &self,
        position: usize,
        value: Value,
        has_resolver: bool,
    ) -> CellValue {
        let cell = CellValue::scalar(value);
        if !has_resolver && self.text_collations.is_empty() {
            return cell;
        }

        let policy = self
            .text_collations
            .get(position)
            .copied()
            .flatten()
            .map_or(TextPolicy::Default, TextPolicy::Column);
        cell.with_policy(policy)
            .with_complete_policy(true)
            .with_declaration(Some(OperandDeclaration::Scalar))
    }
}
