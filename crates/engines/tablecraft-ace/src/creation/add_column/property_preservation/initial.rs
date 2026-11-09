//! Reopen complete image, validate native root identity and exact property body.
use super::PropertyPreservationError as Error;
use crate::{
    NativeImage, TableCheck, check_table,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        row::{ROW_COUNT_OFFSET, slot_offset},
    },
    header::check,
    inspect_table_properties,
    table::mutation::catalog_property::Change,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const OWNER_PADDING: Range<usize> = OWNER_OFFSET + size_of::<u32>()..ROW_COUNT_OFFSET;
const OWNER_PADDING_BYTES: usize = OWNER_PADDING.end - OWNER_PADDING.start;
const LVAL_OWNER: u32 = u32::from_le_bytes(*b"LVAL");

pub(in crate::creation::add_column) fn candidate(
    before: &NativeImage,
    after: &[u8],
    original: &TableCheck,
    column: &str,
    change: &Change,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if after.len() != before.as_bytes().len() + PAGE_BYTES {
        return Err(Error::Preservation);
    }
    let catalog_offset = change.page as usize * PAGE_BYTES;
    if after.get(catalog_offset..catalog_offset + PAGE_BYTES) != Some(change.bytes.as_slice()) {
        return Err(Error::Preservation);
    }
    let page = &after[before.as_bytes().len()..];
    let body_start = PAGE_BYTES
        .checked_sub(change.body.len())
        .ok_or(Error::Preservation)?;
    if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
        || usize::from(u16_at(page, FREE_OFFSET)?) != body_start - slot_offset(1)
        || u32_at(page, OWNER_OFFSET)? != LVAL_OWNER
        || page[OWNER_PADDING] != [0; OWNER_PADDING_BYTES]
        || u16_at(page, ROW_COUNT_OFFSET)? != 1
        || usize::from(u16_at(page, slot_offset(0))?) != body_start
        || page[slot_offset(1)..body_start]
            .iter()
            .any(|byte| *byte != 0)
        || page[body_start..] != change.body
    {
        return Err(Error::Preservation);
    }
    let definition = original.schema().definition_page() as usize;
    for cell in &change.maps {
        if cell.offset / PAGE_BYTES == definition
            || cell.offset / PAGE_BYTES == change.page as usize
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
        let map = change.maps.iter().find(|cell| cell.offset == offset);
        let allowed =
            offset / PAGE_BYTES == definition || offset / PAGE_BYTES == change.page as usize;
        if let Some(cell) = map {
            if *old != cell.before || *new != (cell.before | cell.mask) {
                return Err(Error::Preservation);
            }
        } else if old != new && !allowed {
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
        || target
            .schema()
            .columns()
            .last()
            .is_none_or(|field| field.name() != column)
        || target.indexes().len() != 1
        || target.indexes()[0].status() != original.indexes()[0].status()
    {
        return Err(Error::Preservation);
    }
    let origin = before
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == original.schema().object_id())
        .ok_or(Error::Preservation)?;
    let fresh = reopened
        .catalog()
        .origins()
        .iter()
        .find(|fresh| fresh.object_id() == target.schema().object_id())
        .ok_or(Error::Preservation)?;
    if fresh.page() != origin.page()
        || fresh.slot() != origin.slot()
        || fresh.native_id() != origin.native_id()
        || fresh.flags() != origin.flags()
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
