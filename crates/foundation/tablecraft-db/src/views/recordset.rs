//! Owned read-only child records retain unavailable members without inventing NULL.
mod schema;

pub use schema::{MemberAvailability, RecordsetSchema};

use std::sync::Arc;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::{SchemaError, validate_row};
use thiserror::Error;

/// One stored child member; uninspected payloads have no inferred value or presence.
#[derive(Debug, PartialEq, Eq)]
pub enum StoredCell {
    /// Exact checked scalar, including a genuinely inspected SQL NULL.
    Value(Value),
    /// The source did not inspect this member; this is never SQL NULL.
    Uninspected,
}

/// Parent-associated stored collection presence, independent of its child count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordsetPresence {
    /// A stored parent key associates this collection, including an empty collection.
    Assigned,
    /// The present parent has no stored collection key and therefore no children.
    Unassigned,
}

/// Complete owned child metadata and records, with no writable identity or source handle.
/// Debug and Serde are omitted to keep names and row contents out of diagnostics.
pub struct OwnedRecordset {
    schema: Arc<RecordsetSchema>,
    rows: Vec<Vec<StoredCell>>,
    presence: RecordsetPresence,
    retained_bytes: usize,
}

impl OwnedRecordset {
    /// Validate already owned rows against a shared immutable child schema.
    /// Producers charge their acquisition and allocation before transferring ownership.
    /// Validation consumes work only and does not copy payloads or apply defaults.
    ///
    /// # Errors
    /// Rejects mismatched widths, unavailable-member values, invalid exact carriers,
    /// nonempty unassigned collections, exhausted work and cancellation.
    pub fn new(
        schema: Arc<RecordsetSchema>,
        rows: Vec<Vec<StoredCell>>,
        presence: RecordsetPresence,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, RecordsetError> {
        check(budget, cancelled)?;
        if presence == RecordsetPresence::Unassigned && !rows.is_empty() {
            return Err(RecordsetError::Presence);
        }
        let rows_bytes = rows
            .capacity()
            .checked_mul(size_of::<Vec<StoredCell>>())
            .ok_or(RecordsetError::Size)?;
        let mut retained_bytes = size_of::<Self>()
            .checked_add(rows_bytes)
            .ok_or(RecordsetError::Size)?;
        for row in &rows {
            check(budget, cancelled)?;
            if row.len() != schema.columns().len() {
                return Err(RecordsetError::Width);
            }
            let row_bytes = row
                .capacity()
                .checked_mul(size_of::<StoredCell>())
                .ok_or(RecordsetError::Size)?;
            retained_bytes = retained_bytes
                .checked_add(row_bytes)
                .ok_or(RecordsetError::Size)?;
            for ((column, availability), cell) in
                schema.columns().iter().zip(schema.availability()).zip(row)
            {
                check(budget, cancelled)?;
                match (availability, cell) {
                    (MemberAvailability::Available, StoredCell::Value(value)) => {
                        validate_row(std::slice::from_ref(column), std::slice::from_ref(value))?;
                    }
                    (MemberAvailability::Uninspected, StoredCell::Uninspected) => {}
                    _ => return Err(RecordsetError::Availability),
                }
                let allocated = match cell {
                    StoredCell::Value(Value::Text(value)) => value.capacity(),
                    StoredCell::Value(Value::Binary(value)) => value.capacity(),
                    _ => 0,
                };
                retained_bytes = retained_bytes
                    .checked_add(allocated)
                    .ok_or(RecordsetError::Size)?;
            }
        }
        check(budget, cancelled)?;
        Ok(Self {
            schema,
            rows,
            presence,
            retained_bytes,
        })
    }

    /// Exact member declarations and inspection availability, even with zero rows.
    pub fn schema(&self) -> &RecordsetSchema {
        &self.schema
    }

    /// Borrow complete ordered children, preserving duplicates and inspected NULLs.
    pub fn rows(&self) -> &[Vec<StoredCell>] {
        &self.rows
    }

    /// Whether the present parent had a stored collection association.
    pub const fn presence(&self) -> RecordsetPresence {
        self.presence
    }

    /// Number of actual stored children, without an outer-join placeholder row.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether this collection has no stored children, independently of assignment.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Cached size of this owner, row buffers and owned scalar allocations.
    /// The separately shared schema, control blocks and allocator overhead are excluded.
    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}

/// Read-only child-record metadata or content validation failed, without payload details.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RecordsetError {
    /// The owned representation's byte count cannot fit the addressable size.
    #[error("child recordset representation size overflow")]
    Size,
    /// Member declarations or exact scalar carriers violate shared schema rules.
    #[error(transparent)]
    Schema(#[from] SchemaError),
    /// Validation exhausted the caller's cumulative allowance.
    #[error(transparent)]
    Limit(#[from] tablecraft_core::limits::LimitError),
    /// The caller cancelled validation before publication.
    #[error("child recordset validation cancelled")]
    Cancelled,
    /// A child row does not contain exactly one cell per declared member.
    #[error("child recordset width differs from schema")]
    Width,
    /// An unassigned collection cannot contain stored children.
    #[error("unassigned child recordset contains rows")]
    Presence,
    /// Cell inspection state differs from the declared member availability.
    #[error("child member availability differs from schema")]
    Availability,
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), RecordsetError> {
    if cancelled() {
        return Err(RecordsetError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

#[cfg(test)]
mod tests;
