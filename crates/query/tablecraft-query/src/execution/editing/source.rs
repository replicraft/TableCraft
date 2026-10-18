use crate::{QueryError, Source};
use tablecraft_core::values::Value;
use tablecraft_db::Snapshot;
use tablecraft_schema::Column;

pub(super) struct SnapshotSource<'a>(pub &'a Snapshot);
impl Source for SnapshotSource<'_> {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if !self.0.policy().names.equals(table, self.0.name()) {
            return Err(QueryError::UnknownName);
        }
        Ok(self.0.columns())
    }
    fn scan(
        &self,
        _table: &str,
        _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        // Update planning visits snapshot rows directly. A hidden subquery or
        // other accidental source scan must fail rather than read extra data.
        Err(QueryError::Unsupported)
    }
}
