//! Reuse the complete existing constructive history; never filter system rows.
use super::{FieldHistoryDatabaseOptionsError as Error, PROPERTY_PAGE, SOURCE_OBJECTS};
use crate::{
    FileFormat, NativeImage, TableProperties, TableSchema,
    format::{
        page::PAGE_BYTES,
        reader::{Reader, row_count},
        row::reference,
    },
    header::check,
    inspect_table_properties, inspect_table_schema,
    table::mutation::catalog_property::{database_role, ledger},
};
use tablecraft_core::limits::ResourceBudget;

const USER_NATIVE_ID: i32 = 7;
const USER_TYPE: i16 = 1;
const SOURCE_PAGES: usize = 11;

pub(super) struct Source {
    pub table: TableSchema,
    pub user_property: u32,
    pub user_properties: TableProperties,
    pub next_property: u32,
}

pub(super) fn capture(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, Error> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    if image.catalog().header().format() != FileFormat::Ace2016
        || image.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES
        || image.catalog().origins().len() != SOURCE_OBJECTS
    {
        return Err(Error::Source);
    }
    crate::creation::add_table::source::require_name(image, "MSysDb", budget, cancelled)
        .map_err(super::super::source_error)?;
    let user = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.native_id() == USER_NATIVE_ID)
        .ok_or(Error::Source)?;
    if user.type_code() != USER_TYPE || user.flags() != 0 {
        return Err(Error::Source);
    }
    let object = image
        .catalog()
        .catalog()
        .get(user.object_id())
        .ok_or(Error::Source)?;
    // Refuse impossible capacity before replaying a long history. This paid
    // geometry observation grants nothing; eligible sources still prove every
    // original byte below, and final allocation independently rechecks fit.
    if !database_role::fits(
        image,
        super::super::properties::ENCODED_BYTES,
        super::catalog_record_length()?,
        budget,
        cancelled,
    )? {
        return Err(Error::Source);
    }
    // This consumes the real complete Source5 proof on these immutable bytes,
    // including canonical seed/history equality, macros, aliases and all roles.
    image.check_created_field_addition_source(object.name().as_str(), budget, cancelled)?;
    let table = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        object.name().as_str(),
        budget,
        cancelled,
    )?;
    let properties = inspect_table_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        object.name().as_str(),
        budget,
        cancelled,
    )?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let page = reader.page(PROPERTY_PAGE as u32)?;
    let count = row_count(&page)?;
    let previous = count.checked_sub(1).ok_or(Error::Source)?;
    let user_property = reference(
        PROPERTY_PAGE as u32,
        u8::try_from(previous).map_err(|_| Error::Source)?,
    );
    let next_property = reference(
        PROPERTY_PAGE as u32,
        u8::try_from(count).map_err(|_| Error::Source)?,
    );
    let ledger = ledger::capture(image.as_bytes(), image.catalog(), budget, cancelled)?;
    if properties.object_id() != table.object_id()
        || !ledger.sole_single(
            table.object_id(),
            user_property,
            properties.raw_bytes().len(),
            budget,
            cancelled,
        )?
    {
        return Err(Error::Source);
    }
    Ok(Source {
        table,
        user_property,
        user_properties: properties,
        next_property,
    })
}
