//! Remove parser-bound declaration/name records without rebuilding native IDs.
use super::{FieldDropError as Error, source::Evidence};
use crate::{
    InspectError, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        definition::LENGTH_OFFSET,
        layout::COLUMN_COUNT_OFFSET,
        page::PAGE_BYTES,
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_BYTES_OFFSET: usize = 2;
const DEFINITION_FOOTER_BYTES: usize = 8;
const COPY_BYTES: usize = 16 * 1024;

pub(super) fn copy_image(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let original = image.as_bytes();
    budget.charge_allocation(original.len() as u64, 1)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(original.len())
        .map_err(|_| InspectError::Allocation)?;
    for chunk in original.chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, chunk.len() as u64)?;
        after.extend_from_slice(chunk);
    }
    Ok(after)
}

pub(super) fn page(
    image: &NativeImage,
    evidence: &Evidence,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    check(budget, cancelled)?;
    let observed = &evidence.observed;
    let base = observed.schema.definition_page() as usize * PAGE_BYTES;
    let original = image
        .as_bytes()
        .get(base..base + PAGE_BYTES)
        .ok_or(Error::Source)?;
    let length = u32_at(original, LENGTH_OFFSET)? as usize;
    let entry_start = observed
        .definition_entry
        .start
        .checked_sub(base)
        .ok_or(Error::Source)?;
    let entry_end = observed
        .definition_entry
        .end
        .checked_sub(base)
        .ok_or(Error::Source)?;
    let name_start = observed
        .definition_name
        .start
        .checked_sub(base + size_of::<u16>())
        .ok_or(Error::Source)?;
    let name_end = observed
        .definition_name
        .end
        .checked_sub(base)
        .ok_or(Error::Source)?;
    let entry = entry_start..entry_end;
    let name = name_start..name_end;

    if entry.end > name.start
        || name.end > length
        || length > PAGE_BYTES
        || u16_at(original, name.start)? as usize + size_of::<u16>() != name.len()
    {
        return Err(Error::Source);
    }
    budget.charge(Resource::Steps, PAGE_BYTES as u64)?;
    if !evidence.purpose.retained() && original[length..].iter().any(|byte| *byte != 0) {
        return Err(Error::Source);
    }
    let count = u16_at(original, COLUMN_COUNT_OFFSET)?;
    let count = count.checked_sub(1).ok_or(Error::Source)?;
    let new_length = length
        .checked_sub(entry.len() + name.len())
        .ok_or(Error::Source)?;
    budget.charge_allocation(PAGE_BYTES as u64, 1)?;
    let mut rebuilt = Vec::new();
    rebuilt
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    for part in [
        &original[..entry.start],
        &original[entry.end..name.start],
        &original[name.end..length],
    ] {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, part.len() as u64)?;
        rebuilt.extend_from_slice(part);
    }
    if rebuilt.len() != new_length {
        return Err(Error::Preservation);
    }
    rebuilt.resize(PAGE_BYTES, 0);
    if evidence.purpose.retained() {
        rebuilt[length..].copy_from_slice(&original[length..]);
    }
    rebuilt[COLUMN_COUNT_OFFSET..COLUMN_COUNT_OFFSET + size_of::<u16>()]
        .copy_from_slice(&count.to_le_bytes());
    rebuilt[LENGTH_OFFSET..LENGTH_OFFSET + size_of::<u32>()].copy_from_slice(
        &u32::try_from(new_length)
            .map_err(|_| Error::Source)?
            .to_le_bytes(),
    );
    let free = PAGE_BYTES
        .checked_sub(new_length + DEFINITION_FOOTER_BYTES)
        .ok_or(Error::Source)?;
    rebuilt[FREE_BYTES_OFFSET..FREE_BYTES_OFFSET + size_of::<u16>()].copy_from_slice(
        &u16::try_from(free)
            .map_err(|_| Error::Source)?
            .to_le_bytes(),
    );
    Ok(rebuilt)
}
