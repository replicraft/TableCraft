//! Fresh descriptor/chunk authority comes from actual request values and framing.
use super::super::super::{EditError, page_update};
use super::super::{
    encode::{Encoded, StorageChoice},
    maps::Family,
    stage::Staged,
};
use super::payload;
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{bytes::u32_at, page::PAGE_BYTES, row::slot_offset},
    header::check,
    table::record_layout,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const REFERENCE_OFFSET: usize = size_of::<u32>();
const RESERVED_OFFSET: usize = 2 * size_of::<u32>();
const INLINE_TAG: u32 = 0x8000_0000;
const SINGLE_TAG: u32 = 0x4000_0000;
const INLINE_BYTES: usize = 64;
const SINGLE_BYTES: usize = 4076;
const CHAIN_BYTES: usize = SINGLE_BYTES - size_of::<u32>();

pub(super) fn prove(
    image: &NativeImage,
    schema: &TableSchema,
    column: usize,
    staged: &Staged<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let field = &schema.columns()[column];
    let mut claimed = 0;
    for change in &staged.changes {
        check(budget, cancelled)?;
        let record = image
            .as_bytes()
            .get(change.target.clone())
            .ok_or(EditError::Preservation)?;
        page_update::audit::record(
            image,
            schema,
            column,
            &change.root,
            record,
            budget,
            cancelled,
        )?;
        let layout = record_layout::parse(record, schema, budget)?;
        let start = *layout
            .offsets
            .get(field.field.variable)
            .ok_or(EditError::Preservation)?;
        let end = *layout
            .offsets
            .get(field.field.variable + 1)
            .ok_or(EditError::Preservation)?;
        let descriptor = record.get(start..end).ok_or(EditError::Preservation)?;
        if descriptor != change.descriptor {
            return Err(EditError::Preservation);
        }
        let encoded = payload::prove(field, change, budget, cancelled)?;
        if matches!(change.encoded, Encoded::Null) {
            if !descriptor.is_empty() || !change.chunks.is_empty() {
                return Err(EditError::Preservation);
            }
            continue;
        }
        let (tag, count, capacity) = match change.choice {
            StorageChoice::Inline if encoded.len() <= INLINE_BYTES => (INLINE_TAG, 0, 0),
            StorageChoice::Single if !encoded.is_empty() && encoded.len() <= SINGLE_BYTES => {
                if encoded.len() <= INLINE_BYTES {
                    let before = &change.root.final_record.data;
                    let old = record_layout::parse(before, schema, budget)?;
                    let old_start = old.offsets[field.field.variable];
                    let old_end = old.offsets[field.field.variable + 1];
                    let payload_end = *old.offsets.last().ok_or(EditError::Preservation)?;
                    let hypothetical = payload_end
                        .checked_sub(old_end - old_start)
                        .and_then(|length| length.checked_add(DESCRIPTOR_BYTES + encoded.len()))
                        .and_then(|length| length.checked_add(before.len() - old.data_end))
                        .ok_or(EditError::Preservation)?;
                    if before.len().max(hypothetical) <= PAGE_BYTES - slot_offset(1) {
                        return Err(EditError::Preservation);
                    }
                }
                (SINGLE_TAG, 1, SINGLE_BYTES)
            }
            StorageChoice::Chained if encoded.len() > SINGLE_BYTES => {
                (0, encoded.len().div_ceil(CHAIN_BYTES), CHAIN_BYTES)
            }
            _ => return Err(EditError::Preservation),
        };
        let inline = count == 0;
        if descriptor.len() != DESCRIPTOR_BYTES + if inline { encoded.len() } else { 0 }
            || u32_at(descriptor, 0)?
                != (tag | u32::try_from(encoded.len()).map_err(|_| EditError::Preservation)?)
            || u32_at(descriptor, RESERVED_OFFSET)? != 0
            || change.chunks.len() != count
        {
            return Err(EditError::Preservation);
        }
        let first = if let Some(&index) = change.chunks.first() {
            staged
                .plan
                .appended
                .get(index)
                .ok_or(EditError::Preservation)?
                .reference
        } else {
            0
        };
        if u32_at(descriptor, REFERENCE_OFFSET)? != first {
            return Err(EditError::Preservation);
        }
        if inline {
            budget
                .charge(Resource::Steps, encoded.len() as u64)
                .map_err(InspectError::from)?;
            if descriptor.get(DESCRIPTOR_BYTES..) != Some(encoded) {
                return Err(EditError::Preservation);
            }
        }
        let mut consumed = 0;
        for (position, &index) in change.chunks.iter().enumerate() {
            check(budget, cancelled)?;
            let event = staged
                .plan
                .appended
                .get(index)
                .ok_or(EditError::Preservation)?;
            let body = image
                .as_bytes()
                .get(event.range.clone())
                .ok_or(EditError::Preservation)?;
            let length = (encoded.len() - consumed).min(capacity);
            let linked = change.choice == StorageChoice::Chained;
            let prefix = if linked { size_of::<u32>() } else { 0 };
            if event.family != Family::LongValue || body.len() != prefix + length {
                return Err(EditError::Preservation);
            }
            if linked {
                let next = if let Some(&next) = change.chunks.get(position + 1) {
                    staged
                        .plan
                        .appended
                        .get(next)
                        .ok_or(EditError::Preservation)?
                        .reference
                } else {
                    0
                };
                if u32_at(body, 0)? != next {
                    return Err(EditError::Preservation);
                }
            }
            budget
                .charge(Resource::Steps, length as u64)
                .map_err(InspectError::from)?;
            if body.get(prefix..) != encoded.get(consumed..consumed + length) {
                return Err(EditError::Preservation);
            }
            consumed += length;
            claimed += 1;
        }
        if !inline && consumed != encoded.len() {
            return Err(EditError::Preservation);
        }
    }
    budget
        .charge(Resource::Steps, staged.plan.appended.len() as u64)
        .map_err(InspectError::from)?;
    if claimed
        != staged
            .plan
            .appended
            .iter()
            .filter(|event| event.family == Family::LongValue)
            .count()
    {
        return Err(EditError::Preservation);
    }

    Ok(())
}
