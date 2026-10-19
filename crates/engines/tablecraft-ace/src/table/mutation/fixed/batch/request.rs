use super::super::FixedScalarUpdate;

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
