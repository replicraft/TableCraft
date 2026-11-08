use crate::{QueryError, Source, source::TextCollationId};
use tablecraft_core::values::Value;
use tablecraft_db::Snapshot;
use tablecraft_schema::Column;

pub(in crate::execution) struct SnapshotSource<'a>(pub &'a Snapshot);
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

/// UPDATE carries borrowed field policies alongside its existing frozen rows.
/// DELETE keeps using the unchanged policy-free snapshot adapter above.
pub(super) struct UpdateSource<'a> {
    pub snapshot: &'a Snapshot,
    pub text_collations: &'a [Option<TextCollationId>],
}

impl Source for UpdateSource<'_> {
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
