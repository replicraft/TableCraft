//! Closed read facts for normal INSERT/DELETE, separate from UPDATE target policy.
use super::CheckedIndexes;
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, WriteBlocker, check_table,
    format::{
        data_type::{CURRENCY, INT32, INT64},
        layout::CALCULATED_EXT_MASK,
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

#[cfg(test)]
mod tests;

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
        if checked.indexes().is_empty() {
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
