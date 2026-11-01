//! Exact source-local requests shared below engineering and normal packaging.
use super::rows::RowAssignment;
#[cfg(feature = "journal")]
use crate::{EditError, header::check};
#[cfg(feature = "journal")]
use tablecraft_core::limits::ResourceBudget;
use tablecraft_core::values::Value;

/// An exact expected/replacement carrier for one snapshot-local live row.
///
/// The target native column determines eligibility. NULL transitions and Boolean
/// masks retain their own checks; no SQL coercion or variable-length encoding is
/// implied. Values are deliberately
/// excluded from Debug/Display implementations.
pub struct FixedScalarUpdate {
    row: usize,
    expected: Value,
    value: Value,
}
impl FixedScalarUpdate {
    /// Address a live row of the captured image with exact expected/replacement values.
    pub fn new(row: usize, expected: Value, value: Value) -> Self {
        Self {
            row,
            expected,
            value,
        }
    }
    /// Sequential live-row ordinal in this snapshot only.
    pub const fn row(&self) -> usize {
        self.row
    }
    /// Original carrier, compared without numeric or temporal normalization.
    pub fn expected(&self) -> &Value {
        &self.expected
    }
    /// Replacement carrier; it must exactly match the qualified field type.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Request metadata only: preserve written order while refusing duplicate
    /// ordinals with the same bounded directory used by Int32 intents.
    #[cfg(feature = "journal")]
    pub(crate) fn validate_intent_requests(
        updates: &[Self],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        super::rows::unique(updates, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(())
    }
}

impl RowAssignment for FixedScalarUpdate {
    fn row(&self) -> usize {
        self.row()
    }
}

/// One column's assignments, addressed in the original captured image.
///
/// Borrowed input is not retained by the returned candidate. Table/column group
/// identities must be unique ignoring ASCII case, and each group must contain
/// distinct live-row ordinals. Expected values always name the original image.
pub struct FixedColumnUpdates<'a> {
    table: &'a str,
    column: &'a str,
    updates: &'a [FixedScalarUpdate],
}
impl<'a> FixedColumnUpdates<'a> {
    /// Group exact expected/replacement values for one column of a stable image.
    pub const fn new(table: &'a str, column: &'a str, updates: &'a [FixedScalarUpdate]) -> Self {
        Self {
            table,
            column,
            updates,
        }
    }
    /// Requested native table name.
    pub const fn table(&self) -> &'a str {
        self.table
    }
    /// Requested native column name.
    pub const fn column(&self) -> &'a str {
        self.column
    }
    /// Assignments against the original snapshot, including exact no-ops.
    pub const fn updates(&self) -> &'a [FixedScalarUpdate] {
        self.updates
    }
}

impl crate::NativeImage {
    /// Current fixed batch column-group ceiling, not a format maximum.
    pub const MAX_FIXED_BATCH_GROUPS: usize = crate::limits::MAX_COLUMNS;
    /// Current cumulative fixed assignment ceiling.
    pub const MAX_FIXED_BATCH_ASSIGNMENTS: usize = crate::limits::MAX_LIVE_ROWS;
}
