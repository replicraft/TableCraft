//! Missing Boolean bits require a distinct complete-record reconstruction.
use crate::{
    EditError, FittedColumnUpdates, NativeEditPlan, NativeImage, WriteBlocker,
    format::data_type::BOOLEAN_BYTES,
    header::check,
    table::{
        mutation::{
            current_row_updates::{self, Family},
            field, fixed_records,
            retained_cell_updates::{self, Capability},
        },
        record_layout,
    },
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

impl NativeImage {
    /// Select the missing-bit action from the actual original logical record.
    /// This returns no write authority; the selected plan rechecks the source.
    ///
    /// # Errors
    /// Returns source, target, framing, cancellation or resource errors.
    pub fn select_native_retained_boolean_materialization(
        &self,
        table: &str,
        column: &str,
        row: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        let source =
            Family::RetainedBooleanMaterialization.capture(self, table, budget, cancelled)?;
        let schema = source.facts().table.schema();
        let mut selected = None;
        for (position, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            fixed_records::name_work(field.name(), column, budget)?;
            if field.name().eq_ignore_ascii_case(column) && selected.replace(position).is_some() {
                return Err(EditError::Preservation);
            }
        }
        let position = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        field::fixed_column_at(schema, position, ScalarType::Boolean, BOOLEAN_BYTES)?;
        let record = source.facts().records.get(row).ok_or(EditError::Conflict)?;
        let frame = record_layout::parse(&record.data, schema, budget)?;
        Ok(schema.columns()[position].number() >= frame.count)
    }

    /// Qualify a retained Boolean column with at least one historically absent
    /// value bit. Existing stored bits retain their separate action language.
    ///
    /// # Errors
    /// Returns source, declaration, row framing, cancellation or resource errors.
    pub fn check_native_retained_boolean_materialization_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source =
            Family::RetainedBooleanMaterialization.capture(self, table, budget, cancelled)?;
        retained_cell_updates::qualify(
            &source,
            Capability::BooleanMaterialization,
            budget,
            cancelled,
        )
    }

    pub(crate) fn plan_retained_boolean_materialization(
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
            Family::RetainedBooleanMaterialization,
            budget,
            cancelled,
        )
    }
}

#[cfg(test)]
mod tests;
