//! Original empty LONG groups select their own native language before planning.
use super::{
    Error, JournalRoute, NativeJournalEditor,
    fixed::{check, vector},
};
use crate::editing::NativeEditError;
use tablecraft_ace::{
    NativeFittedIntentGroup, NativeFittedUpdates, RecoverableEditError, RecoverableNativeEditPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{ColumnUpdates, EditableTable};
use tablecraft_schema::ScalarType;

impl NativeJournalEditor {
    pub(super) fn retained_empty_long_columns(
        &self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, NativeEditError> {
        if self.route != JournalRoute::RetainedRows || columns.is_empty() {
            return Ok(false);
        }
        for column in columns {
            check(budget, cancelled)?;
            if !column.updates.is_empty() {
                return Ok(false);
            }
            let position = self.fixed_position(column.column, budget, cancelled)?;
            if self.snapshot().columns()[position].data_type() != ScalarType::Int32 {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn plan_empty_long_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(RecoverableNativeEditPlan, usize), Error> {
        let mut groups = vector(columns.len(), budget)?;
        for column in columns {
            check(budget, cancelled)?;
            let position = self.fixed_position(column.column, budget, cancelled)?;
            let target = &self.snapshot().columns()[position];
            let group = NativeFittedIntentGroup::new_retained(
                target.name(),
                NativeFittedUpdates::Fixed(Vec::new()),
                budget,
                cancelled,
            )
            .map_err(RecoverableEditError::from)?;
            groups.push(group);
        }
        let plan = self
            .editor
            .plan_recoverable_retained_empty_rows(groups, budget, cancelled)?;
        Ok((plan, 0))
    }
}
