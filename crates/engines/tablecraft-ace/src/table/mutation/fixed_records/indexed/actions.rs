//! Closed read facts for normal INSERT/DELETE, separate from UPDATE target policy.
use super::CheckedIndexes;
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, WriteBlocker, check_table,
    format::{
        data_type::{
            CURRENCY, INT32, INT64, MAX_SHORT_BINARY_BYTES, MAX_SHORT_TEXT_UNITS, SHORT_BINARY,
            SHORT_TEXT,
        },
        layout::{CALCULATED_EXT_MASK, UPDATABLE_FLAG},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
enum IndexSet {
    Required,
    #[cfg(feature = "ace16-create")]
    Absent,
    #[cfg(feature = "ace16-create")]
    RetainedManual,
}

impl<'image> CheckedIndexes<'image> {
    /// Complete scalar/key read coverage bound to the original image. This
    /// method neither qualifies a writable field nor grants durable authority.
    /// Only normal INSERT/DELETE compose it with full Source, ownership,
    /// operation-specific key bounds, fresh independent proof and journal checks.
    pub(in crate::table::mutation) fn capture_actions(
        image: &'image NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_action_indexes(image, table, IndexSet::Required, budget, cancelled)
    }

    /// Bind an exactly index-free table for the separate retained keyless action.
    /// This is not the indexed-action factory and grants no key mutation policy.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_keyless_actions(
        image: &'image NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_action_indexes(image, table, IndexSet::Absent, budget, cancelled)
    }

    /// The separate manual retained action validates complete retained ROW
    /// membership without changing the older action factories.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_retained_manual_actions(
        image: &'image NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_action_indexes(image, table, IndexSet::RetainedManual, budget, cancelled)
    }

    fn capture_action_indexes(
        image: &'image NativeImage,
        table: &str,
        indexes: IndexSet,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        #[cfg(feature = "ace16-create")]
        let checked = if matches!(indexes, IndexSet::RetainedManual) {
            crate::table::indexes::check_table_retained(
                &mut image.reader(),
                image.catalog().catalog().scope(),
                table,
                budget,
                cancelled,
            )?
        } else {
            check_table(
                &mut image.reader(),
                image.catalog().catalog().scope(),
                table,
                budget,
                cancelled,
            )?
        };
        #[cfg(not(feature = "ace16-create"))]
        let checked = check_table(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let schema = checked.schema();
        for field in schema.columns() {
            check(budget, cancelled)?;
            if field.scalar_type().is_none() || field.extended_flags() & CALCULATED_EXT_MASK != 0 {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
        let allowed = match indexes {
            IndexSet::Required => !checked.indexes().is_empty(),
            #[cfg(feature = "ace16-create")]
            IndexSet::RetainedManual => !checked.indexes().is_empty(),
            #[cfg(feature = "ace16-create")]
            IndexSet::Absent => {
                checked.indexes().is_empty()
                    && schema.physical_layout().physical_indexes == 0
                    && schema.physical_layout().logical_indexes == 0
            }
        };
        if !allowed {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        for index in checked.indexes() {
            check(budget, cancelled)?;
            if index.status() != IndexCheckStatus::Checked {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
            for key in index.index().columns() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, schema.columns().len() as u64)
                    .map_err(InspectError::from)?;
                let field = schema
                    .columns()
                    .iter()
                    .find(|field| field.number() == key.number())
                    .ok_or(EditError::Preservation)?;
                if field.native_type() == SHORT_BINARY
                    && field.scalar_type() == Some(ScalarType::Binary)
                {
                    if field.fixed()
                        || field.extended_flags() != 0
                        || !(1..=MAX_SHORT_BINARY_BYTES).contains(&field.storage_length())
                    {
                        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                    }
                    continue;
                }
                if field.native_type() == SHORT_TEXT
                    && field.scalar_type() == Some(ScalarType::Text)
                {
                    let constraints = index
                        .index()
                        .constraints()
                        .ok_or(InspectError::Unsupported)?;
                    if field.fixed()
                        || field.flags() != UPDATABLE_FLAG
                        || field.extended_flags() != 0
                        || field.storage_length() == 0
                        || !field.storage_length().is_multiple_of(size_of::<u16>())
                        || field.storage_length() / size_of::<u16>() > MAX_SHORT_TEXT_UNITS
                        || !field
                            .text_sort_order()
                            .is_some_and(|order| order.general_family().is_some())
                        || index.index().columns().len() != 1
                        || constraints.unique()
                        || constraints.required()
                    {
                        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                    }
                    continue;
                }
                let width = match (field.native_type(), field.scalar_type()) {
                    (INT32, Some(ScalarType::Int32)) => size_of::<i32>(),
                    (INT64, Some(ScalarType::Int64)) | (CURRENCY, Some(ScalarType::Currency)) => {
                        size_of::<i64>()
                    }
                    _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
                };
                if !field.fixed() || field.storage_length() != width || field.extended_flags() != 0
                {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            }
        }
        Ok(Self { image, checked })
    }
}
