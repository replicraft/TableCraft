use super::super::TableFeatures;
use crate::NativeColumn;
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

/// Explicit stored scalar calculation results. Expressions are never executed
/// and cached values may be stale. This is not a query source or edit handle.
pub struct StoredCalculation {
    pub(super) features: TableFeatures,
    pub(super) position: usize,
    pub(super) kind: ScalarType,
    pub(super) values: Vec<Value>,
}
impl StoredCalculation {
    /// Original inert expression/result-type declarations.
    pub fn features(&self) -> &TableFeatures {
        &self.features
    }
    /// Native field whose stored values were read.
    pub fn column(&self) -> &NativeColumn {
        &self.features.schema().columns()[self.position]
    }
    /// Explicitly qualified result carrier; NULL retains its own value variant.
    pub const fn data_type(&self) -> ScalarType {
        self.kind
    }
    /// Exact stored values in sequential live-root order, without recalculation.
    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

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
