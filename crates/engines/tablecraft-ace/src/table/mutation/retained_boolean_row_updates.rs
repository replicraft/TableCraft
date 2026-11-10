//! Existing retained Yes/No fields change only their native value mask bit.
use super::{
    current_row_updates::{self, Family},
    retained_cell_updates::{self, Capability},
};
use crate::{EditError, FittedColumnUpdates, NativeEditPlan, NativeImage};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Qualify an unindexed Yes/No field present in every stored source row.
    /// False is a stored value; NULL and historically absent bits are refused.
    pub fn check_native_retained_boolean_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Family::RetainedBoolean.capture(self, table, budget, cancelled)?;
        retained_cell_updates::qualify(&source, Capability::Boolean, budget, cancelled)
    }

    pub(crate) fn plan_retained_boolean_row_updates(
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
            Family::RetainedBoolean,
            budget,
            cancelled,
        )
    }
}
