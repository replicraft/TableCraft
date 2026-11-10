//! Metadata selects a source language before either complete proof is attempted.
use super::FieldDropError;
use crate::{NativeImage, header::check, inspect_table_schema};
use tablecraft_core::limits::ResourceBudget;

/// Source routing only; this observation grants no native DROP permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedFieldDropSourcePurpose {
    /// Existing empty settled-OFF source and its unchanged TCFD0001 language.
    Empty,
    /// Populated current source; only the qualified tail field may be removed.
    PopulatedTail,
}

impl NativeImage {
    /// Select the original source language without scanning user records.
    /// Complete source, dependencies and target identity are checked by the
    /// selected operation. A failed proof is never retried under another purpose.
    pub fn field_drop_source_purpose(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<SavedFieldDropSourcePurpose, FieldDropError> {
        check(budget, cancelled)?;
        let schema = inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        check(budget, cancelled)?;
        Ok(if schema.declared_rows() == 0 {
            SavedFieldDropSourcePurpose::Empty
        } else {
            SavedFieldDropSourcePurpose::PopulatedTail
        })
    }
}
