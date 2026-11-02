//! Original encoded length selects retained slots before producing candidate bytes.
use super::super::{
    binding::{self, Bound, Target},
    original_values,
    stage::Patch,
};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, WriteBlocker,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{reference_page, reference_slot, slot_offset},
    },
    header::check,
    table::{
        long_value::trace::{self, Kind, Traced},
        mutation::{
            preserved_source::Source,
            record_serialization::{
                self,
                long_value::{self, Encoded},
            },
        },
        record_layout,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const WORD_BYTES: usize = size_of::<u32>();
const DESCRIPTOR_BYTES: usize = 3 * WORD_BYTES;
const SINGLE_TAG: u32 = 0x4000_0000;
const FREE_OFFSET: usize = DATA_SIGNATURE.len();

pub(super) struct Change {
    descriptor: usize,
    tagged: u32,
    trace: Traced,
    payload: Vec<u8>,
}

pub(super) struct Prepared {
    changes: Vec<Change>,
    rows: usize,
}

pub(super) fn capture(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, EditError> {
    let schema = source.table.schema();
    let mut changes = binding::vector(bound.assignments.len(), budget)?;
    let mut rows = 0_usize;
    for selected in &bound.rows {
        check(budget, cancelled)?;
        let original = &source.records[selected.ordinal];
        let values = original_values(
            image,
            source,
            selected.ordinal,
            &bound.columns,
            budget,
            cancelled,
        )?;
        let layout = record_layout::parse(&original.data, schema, budget)?;
        let mut changed = false;
        for assignment in &bound.assignments[selected.start..selected.end] {
            check(budget, cancelled)?;
            let cell = groups[assignment.group]
                .cell(assignment.index)
                .ok_or(EditError::Preservation)?;
            let actual = binding::value(bound, &values, assignment.group, budget, cancelled)?;
            if !binding::equal(actual, cell.expected, budget, cancelled)? {
                return Err(EditError::Conflict);
            }
            if binding::equal(cell.expected, cell.value, budget, cancelled)? {
                continue;
            }
            let Target::Variable { column } = bound.targets[assignment.group] else {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            };
            let field = &schema.columns()[column];
            if !field.is_ordinary_long_value() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let start = *layout
                .offsets
                .get(field.field.variable)
                .ok_or(EditError::Preservation)?;
            let end = *layout
                .offsets
                .get(field.field.variable + 1)
                .ok_or(EditError::Preservation)?;
            let raw = original
                .data
                .get(start..end)
                .ok_or(EditError::Preservation)?;
            if raw.len() != DESCRIPTOR_BYTES || raw[2 * WORD_BYTES..] != [0; WORD_BYTES] {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let Encoded::Present(payload) =
                long_value::encode(field, cell.value, budget, cancelled)?
            else {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            };
            let mut bytes = image.reader();
            let mut reader = Reader {
                source: &mut bytes,
                header: image.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            };
            let traced = trace::read(&mut reader, raw)?;
            let linked = match traced.kind {
                Kind::Single => false,
                Kind::Chained => true,
                _ => return Err(EditError::Unsupported(WriteBlocker::RowLayout)),
            };
            let _original_payload = traced.value.as_ref().ok_or(EditError::Preservation)?;
            if payload.is_empty() || traced.chunks.is_empty() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let mut prefix = 0_usize;
            for (index, chunk) in traced.chunks.iter().enumerate() {
                reader.tick()?;
                let number = reference_page(chunk.reference);
                let slot = usize::from(reference_slot(chunk.reference));
                let page = reader.page(number)?;
                let (_, row_start, row_end) = row_range(&page, slot)?;
                if chunk.offset != number as usize * PAGE_BYTES + row_start
                    || chunk.bytes != row_end - row_start
                {
                    return Err(EditError::Preservation);
                }
                let link = if linked { WORD_BYTES } else { 0 };
                let available = chunk
                    .bytes
                    .checked_sub(link)
                    .ok_or(EditError::Preservation)?;
                if index + 1 == traced.chunks.len() {
                    let tail = payload
                        .len()
                        .checked_sub(prefix)
                        .filter(|count| *count != 0)
                        .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
                    if tail > available {
                        if slot + 1 != row_count(&page)?
                            || tail
                                .checked_add(link)
                                .is_none_or(|bytes| bytes > long_value::EXTERNAL_ROW_BYTES)
                            || tail - available > usize::from(u16_at(&page, FREE_OFFSET)?)
                        {
                            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                        }
                    } else {
                        if tail < available && slot + 1 != row_count(&page)? {
                            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                        }
                        let freed = available - tail;
                        u16_at(&page, FREE_OFFSET)?
                            .checked_add(u16::try_from(freed).map_err(|_| EditError::Preservation)?)
                            .ok_or(EditError::Preservation)?;
                    }
                } else {
                    prefix = prefix
                        .checked_add(available)
                        .ok_or(EditError::Preservation)?;
                }
            }
            let tag = if linked { 0 } else { SINGLE_TAG };
            let length = u32::try_from(payload.len()).map_err(|_| InspectError::StructureLimit)?;
            if length & 0xc000_0000 != 0 || u32_at(raw, 0)? & 0xc000_0000 != tag {
                return Err(EditError::Preservation);
            }
            changes.push(Change {
                descriptor: original.offset + start,
                tagged: tag | length,
                trace: traced,
                payload,
            });
            changed = true;
        }
        rows += usize::from(changed);
    }
    check(budget, cancelled)?;
    Ok(Prepared { changes, rows })
}

fn reserve_patch(patches: &mut Vec<Patch>, budget: &mut ResourceBudget) -> Result<(), EditError> {
    if patches.len() == patches.capacity() {
        let count = patches
            .len()
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge_allocation(count as u64, size_of::<Patch>() as u64)
            .map_err(InspectError::from)?;
        let relocation = patches
            .len()
            .checked_mul(size_of::<Patch>())
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge(Resource::Steps, relocation as u64)
            .map_err(InspectError::from)?;
        patches
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
    }
    Ok(())
}

fn page<'a>(
    patches: &'a mut Vec<Patch>,
    number: u32,
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a mut Vec<u8>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, patches.len() as u64)
        .map_err(InspectError::from)?;
    let offset = number as usize * PAGE_BYTES;
    let existing = patches.iter().position(|patch| patch.offset == offset);
    let index = if let Some(index) = existing {
        index
    } else {
        let before = image
            .as_bytes()
            .get(offset..offset + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        reserve_patch(patches, budget)?;
        patches.push(Patch {
            offset,
            before: record_serialization::copy(before, budget, cancelled)?,
            after: record_serialization::copy(before, budget, cancelled)?,
        });
        patches.len() - 1
    };
    Ok(&mut patches[index].after)
}

pub(in crate::table::mutation::normal_fitted) fn prepare(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Patch>, usize), EditError> {
    let prepared = capture(image, source, groups, bound, budget, cancelled)?;
    let mut patches = binding::vector(prepared.changes.len(), budget)?;
    for change in prepared.changes {
        check(budget, cancelled)?;
        let mut offset = 0_usize;
        let linked = matches!(change.trace.kind, Kind::Chained);
        for (index, chunk) in change.trace.chunks.iter().enumerate() {
            check(budget, cancelled)?;
            let number = reference_page(chunk.reference);
            let slot = usize::from(reference_slot(chunk.reference));
            let original =
                &image.as_bytes()[number as usize * PAGE_BYTES..(number as usize + 1) * PAGE_BYTES];
            let (_, start, end) = row_range(original, slot)?;
            let link = if linked { WORD_BYTES } else { 0 };
            let available = end - start - link;
            let remaining = change.payload.len() - offset;
            let growing = index + 1 == change.trace.chunks.len() && remaining > available;
            let length = if growing {
                remaining
            } else {
                available.min(remaining)
            };
            if growing {
                let consumed = length - available;
                let new_start = start.checked_sub(consumed).ok_or(EditError::Preservation)?;
                let after = page(&mut patches, number, image, budget, cancelled)?;
                budget
                    .charge(Resource::Steps, (length + link) as u64)
                    .map_err(InspectError::from)?;
                if linked {
                    after[new_start..new_start + WORD_BYTES]
                        .copy_from_slice(&original[start..start + WORD_BYTES]);
                }
                after[new_start + link..end]
                    .copy_from_slice(&change.payload[offset..offset + length]);
                after[slot_offset(slot)..slot_offset(slot) + size_of::<u16>()].copy_from_slice(
                    &u16::try_from(new_start)
                        .map_err(|_| EditError::Preservation)?
                        .to_le_bytes(),
                );
                let free = u16_at(after, FREE_OFFSET)?
                    .checked_sub(u16::try_from(consumed).map_err(|_| EditError::Preservation)?)
                    .ok_or(EditError::Preservation)?;
                after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
                    .copy_from_slice(&free.to_le_bytes());
            } else {
                let freed = end - start - link - length;
                let after = page(&mut patches, number, image, budget, cancelled)?;
                budget
                    .charge(Resource::Steps, (length + link) as u64)
                    .map_err(InspectError::from)?;
                if linked {
                    after[start + freed..start + freed + WORD_BYTES]
                        .copy_from_slice(&original[start..start + WORD_BYTES]);
                }
                after[start + freed + link..end]
                    .copy_from_slice(&change.payload[offset..offset + length]);
                if freed != 0 {
                    let word = u16::try_from(start + freed).map_err(|_| EditError::Preservation)?;
                    after[slot_offset(slot)..slot_offset(slot) + size_of::<u16>()]
                        .copy_from_slice(&word.to_le_bytes());
                    let free = u16_at(after, FREE_OFFSET)?
                        .checked_add(u16::try_from(freed).map_err(|_| EditError::Preservation)?)
                        .ok_or(EditError::Preservation)?;
                    after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
                        .copy_from_slice(&free.to_le_bytes());
                }
            }
            offset += length;
        }
        if offset != change.payload.len() {
            return Err(EditError::Preservation);
        }
        let before = image
            .as_bytes()
            .get(change.descriptor..change.descriptor + WORD_BYTES)
            .ok_or(EditError::Preservation)?;
        reserve_patch(&mut patches, budget)?;
        patches.push(Patch {
            offset: change.descriptor,
            before: record_serialization::copy(before, budget, cancelled)?,
            after: record_serialization::copy(&change.tagged.to_le_bytes(), budget, cancelled)?,
        });
    }
    check(budget, cancelled)?;
    Ok((patches, prepared.rows))
}
