//! Original supplied/omitted intent; no expanded default is captured here.
use tablecraft_core::values::Value;

/// Presence for one field in the explicitly selected supplied-column schema.
pub enum InsertFieldValue {
    /// An exact value, including explicit stored NULL.
    Supplied(Value),
    /// Resolve the field's qualified source default, nullable fallback or
    /// native Increment declaration. Omission alone grants no generation policy.
    Omitted,
}

/// One presence-bearing row in native order. Native metadata and original
/// presence are checked together; generation is never inferred from omission alone.
pub struct OmittedRowInsert {
    values: Vec<InsertFieldValue>,
}

impl OmittedRowInsert {
    /// Own original intent without resolving metadata or evaluating defaults.
    pub fn new(values: Vec<InsertFieldValue>) -> Self {
        Self { values }
    }

    /// Original values/presence, retained for independent candidate auditing.
    pub fn values(&self) -> &[InsertFieldValue] {
        &self.values
    }
}
