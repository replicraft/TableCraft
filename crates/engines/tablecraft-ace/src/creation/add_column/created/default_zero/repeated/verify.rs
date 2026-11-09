//! Complete complement, native field/index identity and exact property verification.
use super::Error;
use crate::{
    NativeImage, TableCheck, check_table,
    format::{
        page::PAGE_BYTES,
        reader::{row_count, row_range},
    },
    header::check,
    inspect_table_properties,
    table::mutation::catalog_property::RepeatedChange,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn candidate(
    before: &NativeImage,
    after: &[u8],
    original: &TableCheck,
    column: &str,
    change: &RepeatedChange,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let original_length = before.as_bytes().len();
    let target_page = change.lval_page();
    let extra = after
        .len()
        .checked_sub(original_length)
        .ok_or(Error::Preservation)?;
    if after.len() != change.length()
        || !matches!(extra, 0 | PAGE_BYTES)
        || !matches!(target_page, 10 | 11)
        || change.catalog_page == target_page
        || change.catalog_page == original.schema().definition_page()
        || target_page == original.schema().definition_page()
    {
        return Err(Error::Preservation);
    }
    // Every old property row remains byte-identical, including its slot word
    // and physical span. An old non-target property page is wholly unchanged.
    const FIRST_PROPERTY_PAGE: usize = 10;
    for number in FIRST_PROPERTY_PAGE..original_length / PAGE_BYTES {
        let start = number * PAGE_BYTES;
        let old_page = &before.as_bytes()[start..start + PAGE_BYTES];
        let new_page = &after[start..start + PAGE_BYTES];
        if number != target_page as usize {
            if old_page != new_page {
                return Err(Error::Preservation);
            }
            continue;
        }
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
    }
    if extra != 0
        && (original_length != 11 * PAGE_BYTES
            || target_page != 11
            || row_count(&after[original_length..])? != 1)
    {
        return Err(Error::Preservation);
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
            && page != target_page as usize
            && !change.map_byte_matches(offset, *old, *new)
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
