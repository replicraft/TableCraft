//! Exact original complete-row data shared by closed TCNA1, TCNA3 and TCNA4 families.
mod codec;

use super::{NativeIntentError, binding, model as names, row_carrier::Profile};
use crate::{InspectError, RowInsert, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::SchemaName;

#[derive(Clone, Copy)]
pub(super) enum Protocol {
    Existing,
    Eof,
    Complete,
}

impl Protocol {
    const fn carriers(self) -> Profile {
        match self {
            Self::Complete => Profile::Wider,
            Self::Existing | Self::Eof => Profile::Legacy,
        }
    }

    const fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::Existing => b"TCNA0001",
            Self::Eof => b"TCNA0003",
            Self::Complete => b"TCNA0004",
        }
    }

    const fn number(self) -> u16 {
        match self {
            Self::Existing => 1,
            Self::Eof => 3,
            Self::Complete => 4,
        }
    }
}

/// Original ordered complete insertion rows and an exact complete BEFORE binding.
/// This data has no public conversion into a native plan or absence permission.
pub(super) struct CompleteRows {
    table: SchemaName,
    rows: Vec<RowInsert>,
    before_length: u64,
    before_binding: [u8; 32],
}

impl CompleteRows {
    /// Maximum complete-row record accepted before host allocation.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Validate and retain original requests in their written order.
    /// Actual target eligibility, row width and final constraints are
    /// independently checked by the normal planner, never granted here.
    pub fn new(
        table: &str,
        rows: Vec<RowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::capture(table, rows, before, Protocol::Existing, budget, cancelled)
    }

    pub(super) fn new_complete(
        table: &str,
        rows: Vec<RowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::capture(table, rows, before, Protocol::Complete, budget, cancelled)
    }

    fn capture(
        table: &str,
        rows: Vec<RowInsert>,
        before: &[u8],
        protocol: Protocol,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        check(budget, cancelled)?;
        names::validate_name(table, budget, cancelled)?;
        codec::validate_requests(table.len(), &rows, protocol, budget, cancelled)?;
        let before_binding = binding::digest(before, budget, cancelled)?;
        let table = names::copy_name(table, budget)?;
        check(budget, cancelled)?;
        Ok(Self {
            table,
            rows,
            before_length: before.len() as u64,
            before_binding,
        })
    }

    /// Canonical caller-supplied table name without implicit normalization.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact supplied rows, including an empty batch and written order.
    pub fn rows(&self) -> &[RowInsert] {
        &self.rows
    }

    /// Verify exact source bytes before any native replay or target effects.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(NativeIntentError::BeforeMismatch);
        }
        let actual = binding::digest(before, budget, cancelled)?;
        budget
            .charge(Resource::Steps, actual.len() as u64)
            .map_err(InspectError::from)?;
        if !tablecraft_crypto::constant_time_eq(&actual, &self.before_binding) {
            return Err(NativeIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}
