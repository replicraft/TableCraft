//! Complete retained Currency edits share placement, with their own action language.
use super::{
    current_row_updates::{self, Family},
    field,
    historical_fixed::source::Source,
    record_serialization,
};
use crate::{
    EditError, FittedColumnUpdates, NativeEditPlan, NativeImage, WriteBlocker,
    format::{
        data_type::{CURRENCY, INT32, SHORT_TEXT},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
enum Capability {
    Currency,
    CellUpdates,
}

fn qualify(
    source: &Source<'_>,
    capability: Capability,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = source.facts().table.schema();
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let family = match column.native_type() {
            CURRENCY
                if column.fixed()
                    && column.storage_length() == size_of::<i64>()
                    && column.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                    && column.extended_flags() == 0 =>
            {
                Family::RetainedCurrency
            }
            INT32
                if matches!(capability, Capability::CellUpdates)
                    && column.fixed()
                    && column.storage_length() == size_of::<i32>()
                    && column.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                    && column.extended_flags() == 0 =>
            {
                Family::Retained
            }
            SHORT_TEXT if matches!(capability, Capability::CellUpdates) && !column.fixed() => {
                Family::Retained
            }
            _ => continue,
        };
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
            CURRENCY => {
                field::fixed_column_at(schema, position, ScalarType::Currency, size_of::<i64>())?;
            }
            INT32 => {
                field::fixed_column_at(schema, position, ScalarType::Int32, size_of::<i32>())?;
            }
            SHORT_TEXT => record_serialization::storage::qualify(column)?,
            _ => return Err(EditError::Preservation),
        }
        for record in &source.facts().records {
            family.qualify_record(schema, position, &record.data, budget, cancelled)?;
        }
        return Ok(());
    }
    Err(EditError::Unsupported(WriteBlocker::Column))
}

impl NativeImage {
    /// Prove actual retained storage once, then admit a working LONG, Text or
    /// Currency editor. Each selected cell still chooses its exact action family.
    pub fn check_native_retained_cell_updates_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Family::Retained.capture(self, table, budget, cancelled)?;
        qualify(&source, Capability::CellUpdates, budget, cancelled)
    }

    /// Fully qualify an unindexed Currency cell without granting another type
    /// the Currency action language or treating opaque tails as complete rows.
    pub fn check_native_retained_currency_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Family::RetainedCurrency.capture(self, table, budget, cancelled)?;
        qualify(&source, Capability::Currency, budget, cancelled)
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
