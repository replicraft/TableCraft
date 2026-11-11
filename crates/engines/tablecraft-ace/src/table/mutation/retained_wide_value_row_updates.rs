//! Exact GUID and variable Decimal cells share complete retained placement proof.
use super::{
    current_row_updates::{self, Family},
    retained_cell_updates::{self, Capability},
};
use crate::{EditError, FittedColumnUpdates, NativeEditPlan, NativeImage};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Fully qualify an unindexed GUID or Decimal editor in complete retained
    /// records, retaining native precision/scale and exact carrier requirements.
    ///
    /// # Errors
    /// Returns the original source, constraint, carrier or resource refusal.
    pub fn check_native_retained_wide_value_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Family::RetainedWideValues.capture(self, table, budget, cancelled)?;
        retained_cell_updates::qualify(&source, Capability::WideValues, budget, cancelled)
    }

    pub(crate) fn plan_retained_wide_value_row_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        current_row_updates::plan(
            self,
            table,
            groups,
            Family::RetainedWideValues,
            budget,
            cancelled,
        )
    }
}

#[cfg(test)]
mod tests;
