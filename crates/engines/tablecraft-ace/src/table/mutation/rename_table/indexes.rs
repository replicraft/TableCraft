//! Unchanged user Int32 indexes retain complete membership and storage roles.
use super::{TableRenameError, source::serialization_error};
use crate::{
    IndexCheckStatus, NativeImage, TableSchema,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        data_type,
        layout::{AUTO_NUMBER_FLAG, FIXED_FLAG, UPDATABLE_FLAG},
        reader::Reader,
    },
    header::check,
    inspect_table_indexes,
    table::{
        indexes::check_table_retained,
        mutation::{index_ownership, lval_ownership::Inventory},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn qualify(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableRenameError> {
    check(budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let indexes =
        inspect_table_indexes(&mut image.reader(), scope, schema.name(), budget, cancelled)?;
    if indexes.schema().object_id() != schema.object_id()
        || indexes.schema().definition_page() != schema.definition_page()
        || indexes.indexes().len() != schema.layout.logical_indexes
    {
        return Err(TableRenameError::Source);
    }

    for index in indexes.indexes() {
        check(budget, cancelled)?;
        let [key] = index.columns() else {
            return Err(TableRenameError::Source);
        };
        budget
            .charge(Resource::Steps, schema.columns().len() as u64)
            .map_err(crate::InspectError::from)?;
        let column = schema
            .columns()
            .iter()
            .find(|column| column.number() == key.number())
            .ok_or(TableRenameError::Source)?;
        if column.native_type() != data_type::INT32
            || !column.fixed()
            || column.storage_length() != size_of::<i32>()
            || column.flags() & !AUTO_NUMBER_FLAG != (FIXED_FLAG | UPDATABLE_FLAG)
            || column.extended_flags() != 0
            || column.complex_type_id().is_some()
        {
            return Err(TableRenameError::Source);
        }
    }

    // The retained check compares complete visited/owned populations, including
    // leaf/node fences, key bytes and original row suffixes. Unsupported profiles
    // remain refusals instead of becoming an unchecked immutable-index promise.
    let checked =
        check_table_retained(&mut image.reader(), scope, schema.name(), budget, cancelled)?;
    if checked.schema().object_id() != schema.object_id()
        || checked.schema().definition_page() != schema.definition_page()
        || checked.rows() != schema.declared_rows() as usize
        || checked.indexes().len() != indexes.indexes().len()
        || checked
            .indexes()
            .iter()
            .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(TableRenameError::Source);
    }

    // Reuse the actual shared member-role proof. Canonical physical aliases may
    // share an owner; INDEX cannot alias DATA, TDEF, map storage or live LVAL.
    let long_values = Inventory::capture(image, budget, cancelled).map_err(serialization_error)?;
    index_ownership::require(
        image,
        schema.definition_page(),
        &long_values,
        budget,
        cancelled,
    )
    .map_err(serialization_error)?;

    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    for index in indexes.indexes() {
        reader.tick()?;
        let storage = RepresentedMap::read_storage(&mut reader, index.usage_map(), MapRole::Owned)?;
        for page in storage.pages {
            global.require_allocated(&mut reader, page)?;
        }
        for page in allocation::pages(&mut reader, index.usage_map())? {
            global.require_allocated(&mut reader, page)?;
        }
    }
    check(reader.budget, cancelled)?;
    Ok(())
}
