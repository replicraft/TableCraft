use super::{RowHandle, identity};
use crate::SnapshotError;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::{Column, SchemaName, SchemaPolicy, validate_row};

/// Validated complete owned scalar relation under an explicit schema policy.
/// No Debug/Serde exposes data; no row handle survives reconstruction.
pub struct Snapshot {
    identity: u64,
    name: SchemaName,
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policy: SchemaPolicy,
}
impl Snapshot {
    /// Validate already owned input without copying rows. Work accumulates in the
    /// caller budget; cancellation and row/carrier failures publish no snapshot.
    /// Producers own allocation charging and their acquisition row/size limits.
    pub fn new(
        name: String,
        columns: Vec<Column>,
        rows: Vec<Vec<Value>>,
        policy: SchemaPolicy,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, SnapshotError> {
        check(budget, cancelled)?;
        let name = SchemaName::new(name)?;
        // Duplicate schema-name comparison is quadratic in the bounded width.
        for position in 0..columns.len() {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, position as u64)?;
        }
        policy.validate_name(&name)?;
        policy.validate_columns(&columns)?;
        for row in &rows {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, row.len() as u64)?;
            validate_row(&columns, row)?;
        }
        check(budget, cancelled)?;
        Ok(Self {
            identity: identity::issue()?,
            name,
            columns,
            rows,
            policy,
        })
    }
    /// Original validated table name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    /// Exact scalar schema.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }
    /// Complete borrowed rows with no SQL ordering guarantee.
    pub fn rows(&self) -> &[Vec<Value>] {
        &self.rows
    }
    /// Explicit schema policy, including identifier comparison.
    pub const fn policy(&self) -> SchemaPolicy {
        self.policy
    }
    /// Issue a handle for an existing position in this exact snapshot.
    pub fn row_handle(&self, ordinal: usize) -> Option<RowHandle> {
        self.rows.get(ordinal).map(|_| RowHandle {
            snapshot: self.identity,
            ordinal,
        })
    }
    /// Resolve an issuing snapshot's handle to a position; never accept a handle
    /// from a reconstruction even when names and values are identical.
    pub fn ordinal(&self, row: RowHandle) -> Result<usize, SnapshotError> {
        if row.snapshot != self.identity || row.ordinal >= self.rows.len() {
            return Err(SnapshotError::StaleRow);
        }
        Ok(row.ordinal)
    }
    /// Borrow one row after validating snapshot identity.
    pub fn resolve(&self, row: RowHandle) -> Result<&[Value], SnapshotError> {
        Ok(&self.rows[self.ordinal(row)?])
    }
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), SnapshotError> {
    if cancelled() {
        return Err(SnapshotError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}
