use super::super::ComplexColumnLinks;
use tablecraft_core::values::Value;
use tablecraft_schema::Column;

/// Completed bounded stored multivalue collections, scoped to one native column.
/// Values borrow the validated child snapshot; no SQL expansion, lookup display
/// translation, sorting, deduplication or collection mutation is implied.
pub struct MultivalueColumn {
    pub(super) links: ComplexColumnLinks,
    pub(super) value_position: usize,
}
impl MultivalueColumn {
    pub(in crate::table::complex) fn record<'a>(
        &self,
        row: &'a [Value],
    ) -> Result<&'a Value, crate::InspectError> {
        row.get(self.value_position)
            .ok_or(crate::InspectError::Malformed)
    }
    /// Raw checked parent links and child identities, with native schema metadata.
    pub fn links(&self) -> &ComplexColumnLinks {
        &self.links
    }
    /// Exact stored scalar field, distinct from a lookup's displayed text.
    pub fn value_column(&self) -> &Column {
        &self.links.child_snapshot().columns()[self.value_position]
    }
    /// Exact stored value-field text order from the validated child snapshot.
    /// Unknown descriptors remain explicit; non-Text fields return None. No
    /// template/database fallback, record scan or comparison activation occurs.
    pub fn value_text_sort_order(&self) -> Option<crate::NativeSortOrder> {
        self.links.child_snapshot().schema().columns()[self.value_position].text_sort_order()
    }
    /// Borrow one exact stored value by parent/child ordinal, without allocation.
    /// Absent ordinals return None; stored NULL remains Some(Value::Null).
    pub fn value(&self, parent: usize, index: usize) -> Option<&Value> {
        self.links
            .child_row(parent, index)?
            .get(self.value_position)
    }
    /// Stored scalar values in flat snapshot order, retaining duplicates and NULL
    /// cells. An empty parent returns an empty iterator; an absent ordinal is None.
    pub fn values(&self, parent: usize) -> Option<impl ExactSizeIterator<Item = &Value>> {
        Some(
            self.links
                .child_rows(parent)?
                .map(|row| &row[self.value_position]),
        )
    }
}
