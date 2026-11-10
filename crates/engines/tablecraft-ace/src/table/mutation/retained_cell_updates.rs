//! Table-wide cell capability uses one source; selected actions remain closed.
use super::{
    current_row_updates::Family, field, historical_fixed::source::Source, record_serialization,
};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{
        bytes::u16_at,
        data_type::{BOOLEAN, CURRENCY, INT32, SHORT_TEXT},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
pub(super) enum Capability {
    Currency,
    Boolean,
    FixedScalars,
    CellUpdates,
}

pub(super) fn qualify(
    source: &Source<'_>,
    capability: Capability,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = source.facts().table.schema();
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let family = match column.native_type() {
            BOOLEAN
                if matches!(capability, Capability::Boolean | Capability::CellUpdates)
                    && column.fixed()
                    && column.storage_length() == 0
                    && column.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                    && column.extended_flags() == 0 =>
            {
                Family::RetainedBoolean
            }
            CURRENCY
                if matches!(capability, Capability::Currency | Capability::CellUpdates)
                    && column.fixed()
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
            native
                if matches!(
                    capability,
                    Capability::FixedScalars | Capability::CellUpdates
                ) && record_serialization::current_rows::fixed_scalar_kind(native)
                    .is_some_and(|(kind, width)| {
                        column.scalar_type() == Some(kind)
                            && column.fixed()
                            && column.storage_length() == width
                            && column.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                            && column.extended_flags() == 0
                    }) =>
            {
                Family::RetainedFixedScalars
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
        if column.native_type() == BOOLEAN {
            let mut present_in_all = true;
            for record in &source.facts().records {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, size_of::<u16>() as u64)
                    .map_err(InspectError::from)?;
                present_in_all &= column.number() < usize::from(u16_at(&record.data, 0)?);
            }
            if !present_in_all {
                continue;
            }
        }
        match column.native_type() {
            BOOLEAN => {
                field::fixed_column_at(schema, position, ScalarType::Boolean, 0)?;
            }
            CURRENCY => {
                field::fixed_column_at(schema, position, ScalarType::Currency, size_of::<i64>())?;
            }
            INT32 => {
                field::fixed_column_at(schema, position, ScalarType::Int32, size_of::<i32>())?;
            }
            SHORT_TEXT => record_serialization::storage::qualify(column)?,
            native => {
                let (kind, width) = record_serialization::current_rows::fixed_scalar_kind(native)
                    .ok_or(EditError::Preservation)?;
                field::fixed_column_at(schema, position, kind, width)?;
            }
        }
        for record in &source.facts().records {
            family.qualify_record(schema, position, &record.data, budget, cancelled)?;
        }
        return Ok(());
    }
    Err(EditError::Unsupported(WriteBlocker::Column))
}

impl NativeImage {
    /// Prove actual retained storage once, then admit a working scalar editor.
    /// Each selected cell still chooses its exact action family and carrier kinds.
    pub fn check_native_retained_cell_updates_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Family::Retained.capture(self, table, budget, cancelled)?;
        qualify(&source, Capability::CellUpdates, budget, cancelled)
    }
}
