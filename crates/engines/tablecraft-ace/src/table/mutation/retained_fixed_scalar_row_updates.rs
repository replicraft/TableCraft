//! Six retained fixed scalar types share complete-row placement and proof.
use super::{
    current_row_updates::{self, Family},
    retained_cell_updates::{self, Capability},
};
use crate::{EditError, FittedColumnUpdates, NativeEditPlan, NativeImage};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Fully qualify an unindexed Byte, Integer, Large Number, Single, Double
    /// or legacy DateTime cell in complete retained records.
    ///
    /// # Errors
    /// Returns the original source, constraint, carrier or resource refusal.
    pub fn check_native_retained_fixed_scalar_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Family::RetainedFixedScalars.capture(self, table, budget, cancelled)?;
        retained_cell_updates::qualify(&source, Capability::FixedScalars, budget, cancelled)
    }

    pub(crate) fn plan_retained_fixed_scalar_row_updates(
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
            Family::RetainedFixedScalars,
            budget,
            cancelled,
        )
    }
}

#[cfg(test)]
mod tests;
