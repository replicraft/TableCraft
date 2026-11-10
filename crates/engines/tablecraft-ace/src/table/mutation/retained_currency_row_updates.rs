//! Complete retained Currency edits share placement, with their own action language.
use super::{
    current_row_updates::{self, Family},
    retained_cell_updates::{self, Capability},
};
use crate::{EditError, FittedColumnUpdates, NativeEditPlan, NativeImage};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Fully qualify an unindexed Currency cell without granting another type
    /// the Currency action language or treating opaque tails as complete rows.
    pub fn check_native_retained_currency_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Family::RetainedCurrency.capture(self, table, budget, cancelled)?;
        retained_cell_updates::qualify(&source, Capability::Currency, budget, cancelled)
    }

    pub(crate) fn plan_retained_currency_row_updates(
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
            Family::RetainedCurrency,
            budget,
            cancelled,
        )
    }
}

#[cfg(test)]
mod tests;
