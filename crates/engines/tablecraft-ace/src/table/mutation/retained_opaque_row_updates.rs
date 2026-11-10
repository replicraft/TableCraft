//! Explicit opaque-tail materialization source, separate from complete TCNI17.
use super::{
    current_row_updates::{self, Family},
    field,
    historical_fixed::source::Source,
    record_serialization,
};
use crate::{
    EditError, FittedColumnUpdates, NativeEditPlan, NativeImage, WriteBlocker,
    format::data_type::INT32, header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(super) fn source_has_ambiguous(
    source: &Source<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let schema = source.facts().table.schema();
    let mut found = false;
    for record in &source.facts().records {
        check(budget, cancelled)?;
        found |= record_serialization::opaque_rows::is_ambiguous(
            schema,
            &record.data,
            budget,
            cancelled,
        )?;
    }
    Ok(found)
}

impl NativeImage {
    /// Positively classify original row geometry before choosing a writer.
    /// No failed complete-tail qualification is retried as another family.
    pub fn select_native_retained_opaque_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        if !self.has_retained_table_source(table, budget, cancelled)? {
            return Ok(false);
        }
        let source = Source::capture_retained_opaque_rows(self, table, budget, cancelled)?;
        source_has_ambiguous(&source, budget, cancelled)
    }

    /// Qualify a known unindexed LONG slot across complete and opaque old rows.
    pub fn check_native_retained_opaque_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Family::RetainedOpaque.capture(self, table, budget, cancelled)?;
        let schema = source.facts().table.schema();
        for (position, column) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            if column.native_type() != INT32 {
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
            field::fixed_column_at(schema, position, ScalarType::Int32, size_of::<i32>())?;
            for record in &source.facts().records {
                Family::RetainedOpaque.qualify_record(
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

    pub(crate) fn plan_retained_opaque_row_updates(
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
            Family::RetainedOpaque,
            budget,
            cancelled,
        )
    }
}

#[cfg(test)]
mod tests;
