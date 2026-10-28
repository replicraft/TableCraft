use super::{ReadOptions, projected_selection, read_selected};
use crate::{IndexReadRequest, InspectError, NativeReadSource, NativeRowReference, TableSnapshot};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Indexed values paired with their validated original logical DATA roots.
/// References retain the actual returned index order, including selected subsets.
/// They are meaningful only within the snapshot's schema and unchanged caller-owned
/// source binding, never across independent files or revisions.
pub struct IndexedTableSnapshot {
    pub(super) snapshot: TableSnapshot,
    pub(super) references: Vec<NativeRowReference>,
}

impl IndexedTableSnapshot {
    /// Checked scalar projection in the requested native index order.
    pub fn snapshot(&self) -> &TableSnapshot {
        &self.snapshot
    }

    /// One logical root per returned row, before following any forwarding pointer.
    pub fn row_references(&self) -> &[NativeRowReference] {
        &self.references
    }

    /// Move the paired snapshot and logical roots without cloning scalar values.
    pub fn into_parts(self) -> (TableSnapshot, Vec<NativeRowReference>) {
        (self.snapshot, self.references)
    }
}

/// Read an explicit indexed projection and retain its validated logical row roots.
/// Uses the same key/row, ownership, selection, batch and cumulative resource checks
/// as `read_table_index_with_request`. Selected reads do not claim complete counts;
/// unsupported profiles never fall back. References do not authorize mutation.
pub fn read_table_index_with_references<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    request: IndexReadRequest<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<IndexedTableSnapshot, InspectError> {
    if request.maximum_rows == 0 || request.maximum_rows > crate::TableSnapshotLimits::MAX_ROWS {
        return Err(InspectError::InvalidSnapshotLimits);
    }
    let mut references = Vec::new();
    let snapshot = read_selected(
        source,
        scope,
        table,
        index,
        ReadOptions {
            selection: projected_selection(request.selection),
            columns: request.columns,
            maximum_rows: request.maximum_rows,
            legacy_table_limit: false,
            table_order: false,
            eligible_rows: None,
            row_references: Some(&mut references),
            stored_features: None,
        },
        budget,
        cancelled,
    )?;
    if references.len() != snapshot.rows().len() {
        return Err(InspectError::Malformed);
    }
    Ok(IndexedTableSnapshot {
        snapshot,
        references,
    })
}
