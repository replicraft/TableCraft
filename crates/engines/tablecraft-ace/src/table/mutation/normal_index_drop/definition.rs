//! Exact descriptor deletion; logical identities survive physical-array compaction.
use super::source::Original;
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        definition::{LENGTH_OFFSET, PAGE_HEADER_BYTES},
        layout::{
            COLUMN_BLOCK_OFFSET, INDEX_COUNT_OFFSET, INDEX_ENTRY_BYTES, LOGICAL_INDEX_COUNT_OFFSET,
        },
        page::PAGE_BYTES,
    },
    header::check,
    table::{
        indexes::definition::{DATA_NUMBER_OFFSET, LOGICAL_BYTES, PHYSICAL_BYTES},
        mutation::index_creation::buffer,
    },
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = 2;

pub(super) fn removed(
    image: &NativeImage,
    source: &Original,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Range<usize>>, EditError> {
    let layout = &source.layout;
    let start = layout.definition as usize * PAGE_BYTES;
    let raw = image
        .as_bytes()
        .get(start..start + PAGE_BYTES)
        .ok_or(InspectError::Truncated)?;
    let physical = source.selected().data_number() as usize;
    let mut ranges = buffer::vector(4, budget, cancelled)?;
    if source.remove_physical {
        let count = COLUMN_BLOCK_OFFSET + physical * INDEX_ENTRY_BYTES;
        ranges.push(count..count + INDEX_ENTRY_BYTES);
        let frame = layout.physical.start + physical * PHYSICAL_BYTES;
        ranges.push(frame..frame + PHYSICAL_BYTES);
    }
    let frame = layout.logical.start + source.position * LOGICAL_BYTES;
    ranges.push(frame..frame + LOGICAL_BYTES);
    let mut cursor = layout.names.start;
    for position in 0..layout.logical_count as usize {
        check(budget, cancelled)?;
        let name_bytes = usize::from(u16_at(raw, cursor)?);
        let end = cursor
            .checked_add(size_of::<u16>())
            .and_then(|at| at.checked_add(name_bytes))
            .filter(|end| *end <= layout.names.end)
            .ok_or(InspectError::Malformed)?;
        if position == source.position {
            ranges.push(cursor..end);
        }
        cursor = end;
    }
    if cursor != layout.names.end {
        return Err(InspectError::Malformed.into());
    }
    // At most four parser-bound spans; their fixed role order is already ascending.
    for pair in ranges.windows(2) {
        if pair[0].end > pair[1].start {
            return Err(EditError::Preservation);
        }
    }
    Ok(ranges)
}

pub(super) fn compose(
    image: &NativeImage,
    source: &Original,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let layout = &source.layout;
    let start = layout.definition as usize * PAGE_BYTES;
    let raw = &image.as_bytes()[start..start + PAGE_BYTES];
    let removed = removed(image, source, budget, cancelled)?;
    let removed_bytes = removed.iter().map(Range::len).sum::<usize>();
    let length = layout
        .length
        .checked_sub(removed_bytes)
        .ok_or(EditError::Preservation)?;
    let mut output = buffer::vector(PAGE_BYTES, budget, cancelled)?;
    let mut cursor = 0;
    for span in removed {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (span.start - cursor) as u64)
            .map_err(InspectError::from)?;
        output.extend_from_slice(&raw[cursor..span.start]);
        cursor = span.end;
    }
    let remaining_work =
        (layout.length - cursor) + (PAGE_BYTES - length) + (PAGE_BYTES - layout.length);
    budget
        .charge(Resource::Steps, remaining_work as u64)
        .map_err(InspectError::from)?;
    output.extend_from_slice(&raw[cursor..layout.length]);
    if output.len() != length {
        return Err(EditError::Preservation);
    }
    // Preserve all bytes outside the original active definition, including opaque
    // tails. The removed active suffix is retired metadata, not a second parser.
    output.resize(PAGE_BYTES, 0);
    output[layout.length..].copy_from_slice(&raw[layout.length..]);
    let physical_count = layout.physical_count - u32::from(source.remove_physical);
    let logical_count = layout.logical_count - 1;
    output[LENGTH_OFFSET..LENGTH_OFFSET + size_of::<u32>()].copy_from_slice(
        &u32::try_from(length)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    output[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(
        &u16::try_from(PAGE_BYTES.saturating_sub(length + PAGE_HEADER_BYTES))
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    output[INDEX_COUNT_OFFSET..INDEX_COUNT_OFFSET + size_of::<u32>()]
        .copy_from_slice(&physical_count.to_le_bytes());
    output[LOGICAL_INDEX_COUNT_OFFSET..LOGICAL_INDEX_COUNT_OFFSET + size_of::<u32>()]
        .copy_from_slice(&logical_count.to_le_bytes());
    if source.remove_physical {
        let physical_start = layout.physical.start - INDEX_ENTRY_BYTES;
        let logical_start = physical_start + physical_count as usize * PHYSICAL_BYTES;
        for position in 0..logical_count as usize {
            check(budget, cancelled)?;
            let at = logical_start + position * LOGICAL_BYTES + DATA_NUMBER_OFFSET;
            let original = u32_at(&output, at)?;
            if original > source.selected().data_number() {
                output[at..at + size_of::<u32>()].copy_from_slice(&(original - 1).to_le_bytes());
            }
        }
    }
    Ok(output)
}
