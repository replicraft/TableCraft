//! Whole complement plus independent native catalog/property/live-role checks.
use super::{
    CATALOG_DEFINITION, CATALOG_PAGE, FieldHistoryDatabaseOptionsError as Error, PROPERTY_PAGE,
    SOURCE_OBJECTS, source::Source,
};
use crate::{
    DatabaseNameAutoCorrect, InspectError, NativeImage,
    creation::{
        database_options::{DATABASE_ID, DATABASE_PARENT, properties},
        record::DATABASE_SYSTEM_FLAGS,
    },
    format::{
        layout::RECORD_COUNT_OFFSET,
        page::PAGE_BYTES,
        reader::row_count,
        row::{ROW_COUNT_OFFSET, SLOT_BYTES, reference_slot, slot_offset},
    },
    header::check,
    inspect_catalog, inspect_database_properties, inspect_table_properties,
    table::mutation::catalog_property::{database_role::RoleAppend, ledger},
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = 2;

pub(super) fn candidate(
    before: &NativeImage,
    after: &[u8],
    source: &Source,
    placed: &RoleAppend,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    if after.len() != before.as_bytes().len() {
        return Err(Error::Preservation);
    }
    let catalog_base = CATALOG_PAGE * PAGE_BYTES;
    let property_base = PROPERTY_PAGE * PAGE_BYTES;
    budget
        .charge(Resource::Steps, (4 * PAGE_BYTES) as u64)
        .map_err(InspectError::from)?;
    if after[catalog_base..catalog_base + PAGE_BYTES] != placed.catalog_bytes
        || after[property_base..property_base + PAGE_BYTES] != placed.property_bytes
    {
        return Err(Error::Preservation);
    }
    let catalog_count = row_count(&before.as_bytes()[catalog_base..catalog_base + PAGE_BYTES])?;
    let property_count = row_count(&before.as_bytes()[property_base..property_base + PAGE_BYTES])?;
    let definition_count = CATALOG_DEFINITION * PAGE_BYTES + RECORD_COUNT_OFFSET;
    // NO_CLONE: Retain placed catalog/property ranges for both allowed-scope byte checks
    // and fresh native-property verification. Only range bounds are copied, not
    // the payload.
    let scope = [
        definition_count..definition_count + size_of::<u32>(),
        catalog_base + FREE_OFFSET..catalog_base + FREE_OFFSET + SLOT_BYTES,
        catalog_base + ROW_COUNT_OFFSET..catalog_base + ROW_COUNT_OFFSET + SLOT_BYTES,
        catalog_base + slot_offset(catalog_count)..catalog_base + slot_offset(catalog_count + 1),
        placed.catalog_range.clone(),
        property_base + FREE_OFFSET..property_base + FREE_OFFSET + SLOT_BYTES,
        property_base + ROW_COUNT_OFFSET..property_base + ROW_COUNT_OFFSET + SLOT_BYTES,
        property_base + slot_offset(property_count)
            ..property_base + slot_offset(property_count + 1),
        placed.property_range.clone(),
    ];
    budget
        .charge(
            Resource::Steps,
            (before.as_bytes().len() * (scope.len() + 1)) as u64,
        )
        .map_err(InspectError::from)?;
    for (offset, (original, replacement)) in before.as_bytes().iter().zip(after).enumerate() {
        if offset.is_multiple_of(PAGE_BYTES) {
            check(budget, cancelled)?;
        }
        if original != replacement && !scope.iter().any(|range| range.contains(&offset)) {
            return Err(Error::Preservation);
        }
    }
    let native = inspect_catalog(
        &mut Cursor::new(after),
        before.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    if native.header() != before.catalog().header() || native.origins().len() != SOURCE_OBJECTS + 1
    {
        return Err(Error::Preservation);
    }
    for (original, replacement) in before.catalog().origins().iter().zip(native.origins()) {
        check(budget, cancelled)?;
        if original.object_id() != replacement.object_id()
            || original.native_id() != replacement.native_id()
            || original.parent_id() != replacement.parent_id()
            || original.type_code() != replacement.type_code()
            || original.flags() != replacement.flags()
            || original.page() != replacement.page()
            || original.slot() != replacement.slot()
        {
            return Err(Error::Preservation);
        }
    }
    let database = native.origins().last().ok_or(Error::Preservation)?;
    if database.native_id() != DATABASE_ID as i32
        || database.parent_id() != DATABASE_PARENT as i32
        || database.type_code() != 2
        || database.flags() != DATABASE_SYSTEM_FLAGS
        || database.page() != CATALOG_PAGE as u32
        || database.slot() != reference_slot(placed.root)
    {
        return Err(Error::Preservation);
    }
    let database_properties = inspect_database_properties(
        &mut Cursor::new(after),
        native.catalog().scope(),
        budget,
        cancelled,
    )?
    .ok_or(Error::Preservation)?;
    budget
        .charge(Resource::Steps, (2 * properties::ENCODED_BYTES) as u64)
        .map_err(InspectError::from)?;
    // NO_CLONE: Retain placed catalog/property ranges for both allowed-scope byte checks
    // and fresh native-property verification. Only range bounds are copied, not
    // the payload.
    if database_properties.object_id() != database.object_id()
        || database_properties.name_auto_correct(budget, cancelled)?
            != DatabaseNameAutoCorrect::ExplicitOff
        || database_properties.raw_bytes().len() != properties::ENCODED_BYTES
        || database_properties.raw_bytes() != &after[placed.property_range.clone()]
    {
        return Err(Error::Preservation);
    }
    let user_properties = inspect_table_properties(
        &mut Cursor::new(after),
        native.catalog().scope(),
        source.table.name(),
        budget,
        cancelled,
    )?;
    budget
        .charge(
            Resource::Steps,
            (2 * user_properties.raw_bytes().len()) as u64,
        )
        .map_err(InspectError::from)?;
    if user_properties.object_id() != source.table.object_id()
        || user_properties.raw_bytes() != source.user_properties.raw_bytes()
    {
        return Err(Error::Preservation);
    }
    let ledger = ledger::capture(after, &native, budget, cancelled)?;
    if !ledger.two_singles(
        [
            (
                source.table.object_id(),
                source.user_property,
                source.user_properties.raw_bytes().len(),
            ),
            (
                database.object_id(),
                placed.property,
                properties::ENCODED_BYTES,
            ),
        ],
        budget,
        cancelled,
    )? {
        return Err(Error::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
