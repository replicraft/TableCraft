//! Manual non-NULL Int32 key wire and complete retained-leaf image proof.
use super::{population::Row, vector};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    table::indexes::{
        header::IndexHeader,
        key::{ASC_VALUE, DESC_VALUE, INT32_SIGN_BIT},
        page::ENTRY_START,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = std::mem::offset_of!(IndexHeader, free);
const MASK_START: usize = size_of::<IndexHeader>();
const MASK_BYTES: usize = ENTRY_START - MASK_START;
const COMPONENT_BYTES: usize = size_of::<u8>() + size_of::<i32>();
const ROW_BYTES: usize = size_of::<u32>();

#[cfg(test)]
mod tests;

fn keys(
    population: &[Row],
    columns: &[(usize, bool)],
    after: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, EditError> {
    let width = columns
        .len()
        .checked_mul(COMPONENT_BYTES)
        .and_then(|width| width.checked_add(ROW_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let total = population
        .len()
        .checked_mul(width)
        .ok_or(InspectError::StructureLimit)?;
    if total > PAGE_BYTES - ENTRY_START {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut keys = vector(population.len(), budget, cancelled)?;
    for row in population {
        check(budget, cancelled)?;
        let values = if after { &row.after } else { &row.before };
        let mut wire = vector(width, budget, cancelled)?;
        for &(position, descending) in columns {
            check(budget, cancelled)?;
            let value = *values.get(position).ok_or(EditError::Preservation)?;
            // This is deliberately not NumericKey::ordered/encode_component or
            // the staging page serializer. The source integer bits determine
            // the signed-order transform, independently for each component.
            let mut bytes = ((value as u32) ^ INT32_SIGN_BIT).to_be_bytes();
            if descending {
                for byte in &mut bytes {
                    *byte = !*byte;
                }
            }
            wire.push(if descending { DESC_VALUE } else { ASC_VALUE });
            wire.extend_from_slice(&bytes);
        }
        wire.extend_from_slice(&row.reference.to_be_bytes());
        if wire.len() != width {
            return Err(EditError::Preservation);
        }
        keys.push(wire);
    }
    let levels = keys.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = keys
        .len()
        .checked_mul(levels)
        .and_then(|count| count.checked_mul(width))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    keys.sort_unstable();
    check(budget, cancelled)?;
    Ok(keys)
}

fn layout(
    keys: &[Vec<u8>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<([u8; MASK_BYTES], usize), EditError> {
    let mut bitmap = [0; MASK_BYTES];
    let mut end = 0_usize;
    for key in keys {
        check(budget, cancelled)?;
        end = end
            .checked_add(key.len())
            .filter(|end| *end <= PAGE_BYTES - ENTRY_START)
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        let byte = bitmap
            .get_mut(end / u8::BITS as usize)
            .ok_or(EditError::Preservation)?;
        *byte |= 1 << (end % u8::BITS as usize);
    }
    Ok((bitmap, end))
}

fn verify_original(
    before: &[u8],
    keys: &[Vec<u8>],
    bitmap: &[u8; MASK_BYTES],
    bytes: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    let header = IndexHeader::read(before)?;
    if header.prefix.get() != 0
        || usize::from(header.free.get()) != PAGE_BYTES - ENTRY_START - bytes
        || before[MASK_START..ENTRY_START] != *bitmap
    {
        return Err(EditError::Preservation);
    }
    let mut position = ENTRY_START;
    for key in keys {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, key.len() as u64)
            .map_err(InspectError::from)?;
        let end = position
            .checked_add(key.len())
            .ok_or(EditError::Preservation)?;
        if before.get(position..end) != Some(key.as_slice()) {
            return Err(EditError::Preservation);
        }
        position = end;
    }
    if position != ENTRY_START + bytes {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn reconstruct(
    image: &NativeImage,
    checked: &TableCheck,
    index: &NativeIndex,
    population: &[Row],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = checked.schema();
    let mut columns = vector(index.columns().len(), budget, cancelled)?;
    for key in index.columns() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, schema.columns().len() as u64)
            .map_err(InspectError::from)?;
        let position = schema
            .columns()
            .iter()
            .position(|column| column.number() == key.number())
            .ok_or(InspectError::Malformed)?;
        columns.push((position, key.descending()));
    }
    let original = keys(population, &columns, false, budget, cancelled)?;
    let final_keys = keys(population, &columns, true, budget, cancelled)?;
    // Every logical alias constrains the same final population. No intermediate
    // assignment is checked against a partially updated unique key set.
    for alias in checked.indexes() {
        check(budget, cancelled)?;
        if alias.index().data_number() != index.data_number() {
            continue;
        }
        let constraints = alias
            .index()
            .constraints()
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        let mut previous = None;
        for key in &final_keys {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, key.len() as u64)
                .map_err(InspectError::from)?;
            let value = &key[..key
                .len()
                .checked_sub(ROW_BYTES)
                .ok_or(EditError::Preservation)?];
            constraints
                .check_tuple(&mut previous, Some(value), false)
                .map_err(InspectError::from)?;
        }
    }
    let (old_bitmap, old_bytes) = layout(&original, budget, cancelled)?;
    let (new_bitmap, new_bytes) = layout(&final_keys, budget, cancelled)?;
    let start = (index.root_page() as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    let before = image
        .as_bytes()
        .get(start..end)
        .ok_or(EditError::Preservation)?;
    verify_original(before, &original, &old_bitmap, old_bytes, budget, cancelled)?;
    let leaf = expected
        .get_mut(start..end)
        .ok_or(EditError::Preservation)?;
    budget
        .charge(Resource::Steps, PAGE_BYTES as u64)
        .map_err(InspectError::from)?;
    if leaf != before {
        return Err(EditError::Preservation);
    }
    let comparison = original
        .len()
        .checked_mul(size_of::<Vec<u8>>())
        .and_then(|headers| headers.checked_add(old_bytes))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, comparison as u64)
        .map_err(InspectError::from)?;
    if original == final_keys {
        check(budget, cancelled)?;
        return Ok(());
    }
    // Prefix0/non-NULL Int32 preserves every wire width. Single-key Fixed and
    // composite Repack therefore agree on the literal leaf image, while the
    // proof does not need staging's chosen Mode to determine that image.
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    if old_bytes != new_bytes || old_bitmap != new_bitmap {
        return Err(EditError::Preservation);
    }
    let free =
        u16::try_from(PAGE_BYTES - ENTRY_START - new_bytes).map_err(|_| EditError::Preservation)?;
    leaf[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(&free.to_le_bytes());
    leaf[MASK_START..ENTRY_START].copy_from_slice(&new_bitmap);
    let mut position = ENTRY_START;
    for key in &final_keys {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, key.len() as u64)
            .map_err(InspectError::from)?;
        let end = position
            .checked_add(key.len())
            .ok_or(EditError::Preservation)?;
        leaf.get_mut(position..end)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(key);
        position = end;
    }
    if position != ENTRY_START + new_bytes {
        return Err(EditError::Preservation);
    }
    // Header, prefix, map/TD counters, links and the complete unused tail retain
    // original bytes. The caller's final full-image compare proves this together
    // with DATA changes and every unaffected physical INDEX.
    check(budget, cancelled)?;
    Ok(())
}
