//! Distinct retained LONG readonly facts; no legacy row or forwarding traversal.
use super::definition::require_definition;
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{data_type::INT32, layout::CALCULATED_EXT_MASK},
    header::check,
    table::{
        indexes::{check_table_retained, mutation::normal_index_audit::Ownership},
        mutation::preserved_source::Source,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

pub(in crate::table::mutation) struct RetainedLongIndexes {
    checked: TableCheck,
}

impl RetainedLongIndexes {
    pub(in crate::table::mutation) fn capture(
        image: &NativeImage,
        table: &str,
        source: &Source,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let checked = check_table_retained(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        require_definition(source.table.schema(), checked.schema(), budget, cancelled)?;
        if checked.rows() != source.records.len() || checked.indexes().is_empty() {
            return Err(EditError::Preservation);
        }
        for field in checked.schema().columns() {
            check(budget, cancelled)?;
            if field.scalar_type().is_none() || field.extended_flags() & CALCULATED_EXT_MASK != 0 {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
        for selected in checked.indexes() {
            check(budget, cancelled)?;
            let index = selected.index();
            let [key] = index.columns() else {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            };
            if selected.status() != IndexCheckStatus::Checked
                || index.native_type() > 1
                || index.foreign_reference().is_some()
                || index
                    .constraints()
                    .ok_or(InspectError::Unsupported)?
                    .ignore_nulls()
            {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
            budget
                .charge(Resource::Steps, checked.schema().columns().len() as u64)
                .map_err(InspectError::from)?;
            let field = checked
                .schema()
                .columns()
                .iter()
                .find(|field| field.number() == key.number())
                .ok_or(EditError::Preservation)?;
            if field.native_type() != INT32
                || field.scalar_type() != Some(ScalarType::Int32)
                || !field.fixed()
                || field.storage_length() != size_of::<i32>()
                || field.extended_flags() != 0
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
        Ownership::capture_retained_long(image, &checked, budget, cancelled)?;
        Ok(Self { checked })
    }

    pub(in crate::table::mutation) fn checked(&self) -> &TableCheck {
        &self.checked
    }
}
