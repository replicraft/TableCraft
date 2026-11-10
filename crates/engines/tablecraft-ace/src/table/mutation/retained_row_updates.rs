//! Row replacement consumes retained storage through a distinct update purpose.
use super::{
    current_row_updates::{self, Family},
    field, record_serialization,
};
use crate::{
    EditError, FittedColumnUpdates, NativeEditPlan, NativeImage, WriteBlocker,
    format::data_type::{INT32, SHORT_TEXT},
    header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

impl NativeImage {
    /// Prove retained ownership, target programs and eligible unindexed cell
    /// declarations before offering a LONG or Short Text editor.
    pub fn check_native_retained_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Family::Retained.capture(self, table, budget, cancelled)?;
        let schema = source.facts().table.schema();
        for (position, column) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            if !matches!(column.native_type(), INT32 | SHORT_TEXT) {
                continue;
            }
            let mut indexed = false;
            for index in source.facts().indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    indexed |= key.number() == column.number();
                }
            }
            if indexed {
                continue;
            }
            match column.native_type() {
                INT32 => {
                    field::fixed_column_at(schema, position, ScalarType::Int32, size_of::<i32>())?;
                }
                SHORT_TEXT => record_serialization::storage::qualify(column)?,
                _ => return Err(EditError::Preservation),
            }
            for record in &source.facts().records {
                record_serialization::current_rows::qualify(
                    schema,
                    position,
                    &record.data,
                    budget,
                    cancelled,
                )?;
            }
            return Ok(());
        }
        Err(EditError::Unsupported(WriteBlocker::Column))
    }

    pub(crate) fn plan_retained_row_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        current_row_updates::plan(self, table, groups, Family::Retained, budget, cancelled)
    }
}
