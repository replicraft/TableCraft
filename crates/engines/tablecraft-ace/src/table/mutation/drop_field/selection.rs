//! Metadata selects a source language before either complete proof is attempted.
use super::{FieldDropError, NativeFieldDropIdentity};
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
    /// Inspect one positively selected DROP source without producing AFTER bytes.
    /// Metadata chooses the source language before its complete native proof.
    pub fn inspect_field_drop_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldDropIdentity, FieldDropError> {
        match self.field_drop_source_purpose(table, budget, cancelled)? {
            SavedFieldDropSourcePurpose::Empty => {
                self.inspect_saved_field_drop_source(table, column, budget, cancelled)
            }
            SavedFieldDropSourcePurpose::PopulatedTail => {
                self.inspect_current_tail_field_drop_source(table, column, budget, cancelled)
            }
        }
    }

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
