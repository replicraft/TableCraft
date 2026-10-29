//! Explicit increment policy, never inferred from a stored AutoNumber flag.
pub(super) mod audit;
#[cfg(test)]
mod companions;
pub(super) mod counter;
pub(super) mod request;
#[cfg(test)]
mod tests;

use crate::Ace16WritePreview;
pub use request::{AutoNumberRowInsert, IncrementingAutoNumber};

/// A completed incrementing INSERT candidate and its audited generated keys.
///
/// This is not a normal save or a journalable edit plan. Keys become available
/// only after the complete candidate and final constraints have been validated.
pub struct Ace16AutoNumberInsertPreview {
    candidate: Ace16WritePreview,
    column: usize,
    values: Vec<i32>,
}

impl Ace16AutoNumberInsertPreview {
    pub(super) fn from_candidate(
        candidate: Ace16WritePreview,
        column: usize,
        values: Vec<i32>,
    ) -> Self {
        Self {
            candidate,
            column,
            values,
        }
    }

    /// Original complete captured image.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Separate, non-journalable engineering image.
    pub fn candidate_bytes(&self) -> &[u8] {
        self.candidate.candidate_bytes()
    }

    /// Number of inserted rows, excluding metadata changes.
    pub fn inserted_rows(&self) -> usize {
        self.candidate.changed_rows()
    }

    /// Native column number of the generated primary key.
    pub fn generated_column_number(&self) -> usize {
        self.column
    }

    /// Generated Int32 values in original request order.
    pub fn generated_values(&self) -> &[i32] {
        &self.values
    }
}
