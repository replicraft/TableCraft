//! Complete complement, native field/index identity and exact property verification.
use super::{Error, source::PROPERTY_PAGE};
use crate::{
    NativeImage, TableCheck, check_table,
    format::{
        page::PAGE_BYTES,
        reader::{row_count, row_range},
    },
    header::check,
    inspect_table_properties,
    table::mutation::catalog_property::ExistingChange,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn candidate(
    before: &NativeImage,
    after: &[u8],
    original: &TableCheck,
    column: &str,
    change: &ExistingChange,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if after.len() != before.as_bytes().len()
        || change.lval_page != PROPERTY_PAGE
        || change.catalog_page == change.lval_page
        || change.catalog_page == original.schema().definition_page()
    {
        return Err(Error::Preservation);
    }
    let page_start = PROPERTY_PAGE as usize * PAGE_BYTES;
    let old_page = &before.as_bytes()[page_start..page_start + PAGE_BYTES];
    let new_page = &after[page_start..page_start + PAGE_BYTES];
    let rows = row_count(old_page)?;
    if row_count(new_page)? != rows + 1 {
        return Err(Error::Preservation);
    }
    for slot in 0..rows {
        check(budget, cancelled)?;
        let (old_word, old_start, old_end) = row_range(old_page, slot)?;
        let (new_word, new_start, new_end) = row_range(new_page, slot)?;
        if old_word != new_word
            || old_start != new_start
            || old_end != new_end
            || old_page[old_start..old_end] != new_page[new_start..new_end]
        {
            return Err(Error::Preservation);
        }
    }
    for (offset, (old, new)) in before.as_bytes().iter().zip(after).enumerate() {
        if offset.is_multiple_of(PAGE_BYTES) {
            check(budget, cancelled)?;
        }
        budget
            .charge(Resource::Steps, 1)
            .map_err(crate::InspectError::from)?;
        let page = offset / PAGE_BYTES;
        if old != new
            && page != original.schema().definition_page() as usize
            && page != change.catalog_page as usize
            && page != change.lval_page as usize
        {
            return Err(Error::Preservation);
        }
    }
    let scope = before.catalog().catalog().scope();
    let reopened = NativeImage::capture(&mut Cursor::new(after), scope, budget, cancelled)?;
    let target = check_table(
        &mut reopened.reader(),
        scope,
        original.schema().name(),
        budget,
        cancelled,
    )?;
    if target.rows() != 0
        || target.schema().declared_rows() != 0
        || target.schema().columns().len() != original.schema().columns().len() + 1
        || target.indexes().len() != original.indexes().len()
    {
        return Err(Error::Preservation);
    }
    for (old, new) in original
        .schema()
        .columns()
        .iter()
        .zip(target.schema().columns())
    {
        check(budget, cancelled)?;
        if old.name() != new.name() || old.physical_field() != new.physical_field() {
            return Err(Error::Preservation);
        }
    }
    if target
        .schema()
        .columns()
        .last()
        .is_none_or(|field| field.name() != column)
    {
        return Err(Error::Preservation);
    }
    let properties = inspect_table_properties(
        &mut reopened.reader(),
        scope,
        target.schema().name(),
        budget,
        cancelled,
    )?;
    if properties.raw_bytes() != change.body {
        return Err(Error::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
