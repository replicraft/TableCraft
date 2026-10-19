use super::super::MultivalueColumn;
use crate::{InspectError, TableFeatures, TablePageRequest, TableSchema, table::page};
use tablecraft_core::values::Value;

pub(super) enum PageMetadata {
    Schema(TableSchema),
    Features(TableFeatures),
}
impl PageMetadata {
    pub(super) fn schema(&self) -> &TableSchema {
        match self {
            Self::Schema(schema) => schema,
            Self::Features(features) => features.schema(),
        }
    }
    pub(super) fn features(&self) -> Option<&TableFeatures> {
        match self {
            Self::Schema(_) => None,
            Self::Features(features) => Some(features),
        }
    }
}

/// A bounded parent page whose complex cells are explicitly typed collections.
/// Complete parent-key/child reads retain the existing 16,384-row snapshot limits.
/// Selected ordinary values are partial reads; formulas/lookups never execute.
pub struct MultivalueTablePage {
    pub(super) metadata: PageMetadata,
    pub(super) records: page::PageRecords,
    pub(super) request: TablePageRequest,
    pub(super) collections: Vec<(usize, MultivalueColumn)>,
}
impl MultivalueTablePage {
    /// Inert feature declarations when cached-calculation opt-in was selected.
    /// The native schema remains available separately in either mode.
    pub fn features(&self) -> Option<&TableFeatures> {
        self.metadata.features()
    }
    /// Native schema in logical column order, including original complex types.
    pub fn schema(&self) -> &TableSchema {
        self.metadata.schema()
    }
    /// Number of selected parent records; no collection flattening is applied.
    pub fn row_count(&self) -> usize {
        self.records.rows.len()
    }
    /// Original checked physical request.
    pub const fn request(&self) -> TablePageRequest {
        self.request
    }
    /// Whether parent-page enumeration reached EOF and checked the declared count.
    pub const fn reached_end(&self) -> bool {
        self.records.reached_end
    }
    /// Qualified collection metadata for a column, or None for scalar/out-of-range.
    pub fn collection_column(&self, column: usize) -> Option<&MultivalueColumn> {
        self.collections
            .iter()
            .find(|(position, _)| *position == column)
            .map(|(_, value)| value)
    }
    /// Explicit scalar or collection cell. None means an out-of-range coordinate.
    pub fn cell(&self, row: usize, column: usize) -> Option<MultivaluePageValue<'_>> {
        let value = self.records.rows.get(row)?.get(column)?;
        Some(if let Some(collection) = self.collection_column(column) {
            // Successfully read complex snapshots cap parent ordinals at 16,384.
            MultivaluePageValue::Collection(MultivaluePageCell {
                collection,
                parent: self.request.offset() as usize + row,
            })
        } else {
            MultivaluePageValue::Scalar(value)
        })
    }
}

/// A scalar value or explicit collection; no collection is coerced into core NULL.
pub enum MultivaluePageValue<'a> {
    /// Exact native scalar or explicitly permitted cached scalar result.
    Scalar(&'a Value),
    /// Stored foreign key and collection, distinct from SQL scalar semantics.
    Collection(MultivaluePageCell<'a>),
}
/// Borrowed stored collection in one validated parent row/column scope.
pub struct MultivaluePageCell<'a> {
    collection: &'a MultivalueColumn,
    parent: usize,
}
impl MultivaluePageCell<'_> {
    /// Assigned native key, or None for an absent key. Empty assigned collections
    /// retain their positive key; NULL elements remain distinct stored values.
    pub fn key(&self) -> Option<i32> {
        self.collection.links().parents()[self.parent].key()
    }
    /// Stored values in flat-table order, retaining duplicates and NULL elements.
    /// Internal membership inconsistencies are errors, never empty defaults.
    pub fn values(&self) -> Result<impl ExactSizeIterator<Item = &Value>, InspectError> {
        self.collection
            .values(self.parent)
            .ok_or(InspectError::Malformed)
    }
}
