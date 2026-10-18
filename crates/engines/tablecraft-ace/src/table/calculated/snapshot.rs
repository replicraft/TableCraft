use super::super::TableFeatures;
use crate::NativeColumn;

/// Explicit stored Int32 calculation results, without recalculation or freshness
/// guarantees. This is not an ordinary scalar query snapshot or an edit handle.
pub struct StoredInt32Calculation {
    pub(super) features: TableFeatures,
    pub(super) position: usize,
    pub(super) values: Vec<Option<i32>>,
}
impl StoredInt32Calculation {
    /// Original inert declarations, including expression/result-type properties.
    pub fn features(&self) -> &TableFeatures {
        &self.features
    }
    /// Exact native calculated column whose storage was inspected.
    pub fn column(&self) -> &NativeColumn {
        &self.features.schema().columns()[self.position]
    }
    /// Stored values in sequential live-root order; ordinals are snapshot-local.
    /// NULL is returned only for absent storage or the qualified empty wrapper.
    pub fn values(&self) -> &[Option<i32>] {
        &self.values
    }
}
