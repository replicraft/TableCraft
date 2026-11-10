//! Operation-specific original-record coverage, separate from write targeting.
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker, header::check,
    table::record_layout::RecordLayout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

#[derive(Clone, Copy)]
pub(super) enum RecordAdmission {
    Complete,
    /// INSERT preserves historical records; it only serializes the new rows.
    #[cfg(feature = "journal")]
    HistoricalInsert,
    /// Schema-only observation retains every original record byte, never targets it.
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    HistoricalSchemaReadonly,
    /// Only the dedicated proof can target an already present fixed field.
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    HistoricalPresentFixed,
}

impl RecordAdmission {
    pub(super) fn validate(
        self,
        layout: &RecordLayout,
        schema: &TableSchema,
        values: &[Value],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if layout.count == schema.layout.max_columns {
            return Ok(());
        }
        let historical = match self {
            Self::Complete => false,
            #[cfg(feature = "journal")]
            Self::HistoricalInsert => true,
            #[cfg(all(feature = "journal", feature = "ace16-create"))]
            Self::HistoricalSchemaReadonly => true,
            #[cfg(all(feature = "journal", feature = "ace16-create"))]
            Self::HistoricalPresentFixed => true,
        };
        if historical
            && layout.count > 0
            && layout.count < schema.layout.max_columns
            && values.len() == schema.columns().len()
        {
            for (column, value) in schema.columns().iter().zip(values) {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                // Native field numbers select mask positions, not presentation
                // ordinals. The shared complete parser already proved the old
                // fixed/variable extents using this record's own field count.
                if column.number() >= layout.count && *value != Value::Null {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
            }
            return Ok(());
        }
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    }
}

#[cfg(all(test, feature = "journal"))]
mod tests;
