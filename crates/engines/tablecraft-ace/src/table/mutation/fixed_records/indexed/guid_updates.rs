//! Original-bound GUID targets and fully retained I32 physical owners.
use super::{CheckedIndexes, IndexTargets, definition::require_definition};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, TableCheck, TableSchema, WriteBlocker,
    check_table,
    format::{
        data_type::{GUID, INT32},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
    table::{
        indexes::mutation::normal_index_audit::Ownership,
        mutation::{lval_ownership::Inventory, preserved_source::Source},
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Guid,
};
use tablecraft_schema::ScalarType;

pub(in crate::table::mutation) struct GuidUpdateIndexes<'image> {
    base: CheckedIndexes<'image>,
    pub(in crate::table::mutation) ownership: Ownership,
}

impl<'image> GuidUpdateIndexes<'image> {
    pub(in crate::table::mutation) fn capture(
        image: &'image NativeImage,
        source: &Source,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let checked = check_table(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            source.table.schema().name(),
            budget,
            cancelled,
        )?;
        require_definition(source.table.schema(), checked.schema(), budget, cancelled)?;
        if checked.rows() != source.records.len()
            || checked.indexes().len() != source.indexes.indexes().len()
        {
            return Err(EditError::Preservation);
        }
        let schema = checked.schema();
        for (position, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            // Raw record and semantic row projections use these dense positions.
            if field.number() != position {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
        let mut has_guid = false;
        for selected in checked.indexes() {
            check(budget, cancelled)?;
            let index = selected.index();
            let [key] = index.columns() else {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            };
            let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
            let field = schema
                .columns()
                .get(key.number())
                .ok_or(EditError::Preservation)?;
            let width = match (field.native_type(), field.scalar_type()) {
                (GUID, Some(ScalarType::Guid)) => {
                    has_guid = true;
                    Guid::BYTE_LEN
                }
                (INT32, Some(ScalarType::Int32)) => size_of::<i32>(),
                _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
            };
            if selected.status() != IndexCheckStatus::Checked
                || index.native_type() > 1
                || index.foreign_reference().is_some()
                || constraints.ignore_nulls()
                || field.number() != key.number()
                || !field.fixed()
                || field.storage_length() != width
                || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
                || field.extended_flags() != 0
                || field.complex_type_id().is_some()
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
        if !has_guid {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let ownership = Ownership::capture_guid_updates(image, &checked, budget, cancelled)?;
        let inventory = Inventory::capture(image, budget, cancelled)?;
        for (position, selected) in checked.indexes().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, position as u64)
                .map_err(InspectError::from)?;
            if checked.indexes()[..position]
                .iter()
                .any(|prior| prior.index().data_number() == selected.index().data_number())
            {
                continue;
            }
            let tree = ownership.require(image, schema, selected.index(), budget, cancelled)?;
            if tree.pages() != [selected.index().root_page()] {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            inventory.require_no_long_value_data_overlap(tree.pages(), budget, cancelled)?;
        }
        Ok(Self {
            base: CheckedIndexes { image, checked },
            ownership,
        })
    }

    pub(in crate::table::mutation) fn checked(&self) -> &TableCheck {
        self.base.checked()
    }
}

impl IndexTargets for GuidUpdateIndexes<'_> {
    fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.base.require_source(image, schema, budget, cancelled)?;
        require_definition(self.checked().schema(), schema, budget, cancelled)
    }

    fn require_target(
        &self,
        schema: &TableSchema,
        position: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.require_source(self.base.image, schema, budget, cancelled)?;
        let field = schema
            .columns()
            .get(position)
            .ok_or(EditError::Preservation)?;
        if field.number() != position
            || field.native_type() != GUID
            || field.scalar_type() != Some(ScalarType::Guid)
            || !field.fixed()
            || field.storage_length() != Guid::BYTE_LEN
            || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || field.extended_flags() != 0
            || field.complex_type_id().is_some()
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        Ok(())
    }
}
