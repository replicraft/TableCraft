//! Image-bound complete scalar/index facts, independent of event admission.
#[cfg(feature = "journal")]
mod actions;
#[cfg(feature = "journal")]
mod definition;
#[cfg(feature = "journal")]
pub(in crate::table::mutation) mod guid_updates;
#[cfg(feature = "journal")]
pub(in crate::table::mutation) mod i32_updates;
#[cfg(feature = "journal")]
pub(in crate::table::mutation) mod integral_updates;
#[cfg(feature = "journal")]
mod text_updates;
use crate::{
    EditError, NativeImage, TableCheck, TableSchema, WriteBlocker, format::data_type::INT32,
    header::check,
};
#[cfg(feature = "journal")]
use crate::{IndexCheckStatus, check_table, format::layout::CALCULATED_EXT_MASK};
#[cfg(feature = "journal")]
use tablecraft_core::limits::Resource;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

/// Read/target qualification only. A normal plan, independent full-image proof
/// and checked journal resources are still required for any durable write.
pub(in crate::table::mutation) struct CheckedIndexes<'image> {
    image: &'image NativeImage,
    checked: TableCheck,
}

impl<'image> CheckedIndexes<'image> {
    #[cfg(feature = "journal")]
    pub(in crate::table::mutation) fn capture(
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
                    .map_err(crate::InspectError::from)?;
                let field = schema
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
        }
        Ok(Self { image, checked })
    }

    #[cfg(feature = "journal")]
    pub(in crate::table::mutation) fn checked(&self) -> &TableCheck {
        &self.checked
    }

    pub(in crate::table::mutation) fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if !std::ptr::eq(image, self.image)
            || schema.object_id() != self.checked.schema().object_id()
            || schema.definition_page() != self.checked.schema().definition_page()
        {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    pub(in crate::table::mutation) fn require_target(
        &self,
        schema: &TableSchema,
        position: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.require_source(self.image, schema, budget, cancelled)?;
        let field = schema
            .columns()
            .get(position)
            .ok_or(EditError::Preservation)?;
        if field.native_type() != INT32
            || field.scalar_type() != Some(ScalarType::Int32)
            || !field.fixed()
            || field.storage_length() != size_of::<i32>()
            || field.extended_flags() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        // The ordinary field writer separately checks writable/generated flags
        // and span overlap. This token never converts a counter into a target.
        Ok(())
    }
}

/// Borrowed captured index facts used by joint fixed record binding and proof.
/// This qualifies a target only; Source, physical proof and journaling remain
/// owned by the operation. External callers cannot implement this private trait.
pub(in crate::table::mutation) trait IndexTargets {
    #[cfg(feature = "journal")]
    fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError>;
    fn require_target(
        &self,
        schema: &TableSchema,
        position: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError>;
}

impl IndexTargets for CheckedIndexes<'_> {
    #[cfg(feature = "journal")]
    fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        CheckedIndexes::require_source(self, image, schema, budget, cancelled)
    }
    fn require_target(
        &self,
        schema: &TableSchema,
        position: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        CheckedIndexes::require_target(self, schema, position, budget, cancelled)
    }
}
