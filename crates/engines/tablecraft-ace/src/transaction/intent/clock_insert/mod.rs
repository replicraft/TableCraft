//! Original presence and captured civil execution input; neither grants admission.
mod codec;
mod planning;
mod recovery;

#[cfg(test)]
mod tests;

pub use planning::RecoverableNativeClockInsertPlan;
pub use recovery::{QualifiedNativeClockInsertRecovery, qualify_native_clock_insert_recovery};

use super::{NativeIntentError, NativeOmittedInsertIntent};
use crate::{OmittedRowInsert, header::check};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::functions::dates::clock::ClockReading;

/// Exact original INSERT data plus the optional captured operation clock.
/// A reading is execution input; fresh native Source/default binding still
/// decides whether this operation can use it, and its presence must be exact.
pub struct NativeClockInsertIntent {
    original: NativeOmittedInsertIntent,
    reading: Option<ClockReading>,
}

impl NativeClockInsertIntent {
    /// Maximum complete TCNA0005 envelope, including its nested presence record.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Capture original data and a canonical whole-second civil clock input.
    /// This does not execute defaults or call a host clock provider.
    pub fn new(
        table: &str,
        rows: Vec<OmittedRowInsert>,
        before: &[u8],
        reading: Option<ClockReading>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let original = NativeOmittedInsertIntent::new(table, rows, before, budget, cancelled)?;
        Self::from_original(original, reading, budget, cancelled)
    }

    fn from_original(
        original: NativeOmittedInsertIntent,
        reading: Option<ClockReading>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        check(budget, cancelled)?;
        codec::length(original.encoded_length(budget, cancelled)?)?;
        let reading = reading
            .map(|value| {
                ClockReading::new(value.date_time()).map_err(|_| NativeIntentError::InvalidRecord)
            })
            .transpose()?;
        check(budget, cancelled)?;
        Ok(Self { original, reading })
    }

    /// Exact original target spelling, with no file or SQL authority.
    pub fn table(&self) -> &str {
        self.original.table()
    }

    /// Original written supplied/omitted rows, never expanded values.
    pub fn rows(&self) -> &[OmittedRowInsert] {
        self.original.rows()
    }

    /// Optional canonical whole-second civil execution input, not an instant.
    pub fn reading(&self) -> Option<ClockReading> {
        self.reading
    }

    /// Verify the complete original image through the existing binding owner.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        self.original.verify_before(before, budget, cancelled)
    }
}
