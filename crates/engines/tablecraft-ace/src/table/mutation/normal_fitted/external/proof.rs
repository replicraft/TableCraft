//! Independent original-descriptor walk and request bytes, never staged chunks.
use super::super::{
    binding::{self, Bound, Target},
    original_values,
};
use crate::{
    EditError, FittedColumnUpdates, NativeImage, WriteBlocker,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{DELETED_MASK, RELOCATED_MASK, reference_page, reference_slot, slot_offset},
    },
    header::check,
    limits::MAX_LONG_VALUE_CHUNKS,
    table::{
        mutation::{
            normal_ordinary,
            preserved_source::Source,
            record_serialization::{
                self,
                proof::long_value::{self, Payload},
            },
        },
        record_layout,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

const WORD_BYTES: usize = size_of::<u32>();
const DESCRIPTOR_BYTES: usize = 3 * WORD_BYTES;
const TYPE_MASK: u32 = 0xc000_0000;
const SINGLE: u32 = 0x4000_0000;
const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const LVAL_OWNER: &[u8; WORD_BYTES] = b"LVAL";
const MAX_EXTERNAL_BODY_BYTES: usize = 4076;

pub(in crate::table::mutation::normal_fitted) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, usize), EditError> {
    if super::retirement::selected(groups, bound, budget, cancelled)? {
        return super::retirement::proof::reconstruct(
            image, source, groups, bound, budget, cancelled,
        );
    }
    let schema = source.table.schema();
    let mut expected = record_serialization::copy(image.as_bytes(), budget, cancelled)?;
    let mut changed_rows = 0_usize;
    for selected in &bound.rows {
        check(budget, cancelled)?;
        let original = &source.records[selected.ordinal];
        let frame = record_layout::parse(&original.data, schema, budget)?;
        let values = original_values(
            image,
            source,
            selected.ordinal,
            &bound.columns,
            budget,
            cancelled,
        )?;
        let mut changed = false;
        for assignment in &bound.assignments[selected.start..selected.end] {
            check(budget, cancelled)?;
            let cell = groups[assignment.group]
                .cell(assignment.index)
                .ok_or(EditError::Preservation)?;
            if !binding::equal(
                binding::value(bound, &values, assignment.group, budget, cancelled)?,
                cell.expected,
                budget,
                cancelled,
            )? {
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
            let start = *frame
                .offsets
                .get(field.field.variable)
                .ok_or(EditError::Preservation)?;
            let end = *frame
                .offsets
                .get(field.field.variable + 1)
                .ok_or(EditError::Preservation)?;
            let descriptor = original
                .data
                .get(start..end)
                .ok_or(EditError::Preservation)?;
            if descriptor.len() != DESCRIPTOR_BYTES
                || descriptor[2 * WORD_BYTES..] != [0; WORD_BYTES]
            {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let word = u32_at(descriptor, 0)?;
            let kind = word & TYPE_MASK;
            if kind != 0 && kind != SINGLE {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let Payload::Present(requested) =
                long_value::requested(field, cell.value, budget, cancelled)?
            else {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            };
            let old_length = (word & !TYPE_MASK) as usize;
            if requested.is_empty() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let mut reference = u32_at(descriptor, WORD_BYTES)?;
            let mut consumed = 0_usize;
            let mut replaced = 0_usize;
            let mut visited = binding::vector(MAX_LONG_VALUE_CHUNKS, budget)?;
            let mut completed = false;
            let mut bytes = image.reader();
            let mut reader = Reader {
                source: &mut bytes,
                header: image.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            };
            for _ in 0..MAX_LONG_VALUE_CHUNKS {
                reader.tick()?;
                reader.work(visited.len())?;
                if reference == 0 || visited.contains(&reference) {
                    return Err(EditError::Preservation);
                }
                visited.push(reference);
                let number = reference_page(reference);
                let slot = usize::from(reference_slot(reference));
                let page = reader.page(number)?;
                if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                    || page.get(OWNER_OFFSET..OWNER_OFFSET + WORD_BYTES) != Some(LVAL_OWNER)
                {
                    return Err(EditError::Preservation);
                }
                let (flags, from, to) = row_range(&page, slot)?;
                if flags & (DELETED_MASK | RELOCATED_MASK) != 0 {
                    return Err(EditError::Preservation);
                }
                let link_bytes = if kind == 0 { WORD_BYTES } else { 0 };
                let available = (to - from)
                    .checked_sub(link_bytes)
                    .filter(|n| *n != 0)
                    .ok_or(EditError::Preservation)?;
                let next = if kind == 0 { u32_at(&page, from)? } else { 0 };
                consumed = consumed
                    .checked_add(available)
                    .filter(|n| *n <= old_length)
                    .ok_or(EditError::Preservation)?;
                let last = consumed == old_length;
                if last != (next == 0) {
                    return Err(EditError::Preservation);
                }
                let needed = if last {
                    requested
                        .len()
                        .checked_sub(replaced)
                        .filter(|n| *n != 0)
                        .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?
                } else {
                    if replaced
                        .checked_add(available)
                        .is_none_or(|n| n >= requested.len())
                    {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                    available
                };
                if needed > available {
                    let extra = needed - available;
                    if !last
                        || slot + 1 != row_count(&page)?
                        || needed
                            .checked_add(link_bytes)
                            .is_none_or(|bytes| bytes > MAX_EXTERNAL_BODY_BYTES)
                        || extra > usize::from(u16_at(&page, FREE_OFFSET)?)
                    {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                    let new_start = from.checked_sub(extra).ok_or(EditError::Preservation)?;
                    let base = number as usize * PAGE_BYTES;
                    let after = expected
                        .get_mut(base..base + PAGE_BYTES)
                        .ok_or(EditError::Preservation)?;
                    reader.work(needed + link_bytes)?;
                    if kind == 0 {
                        after[new_start..new_start + WORD_BYTES]
                            .copy_from_slice(&next.to_le_bytes());
                    }
                    after[new_start + link_bytes..to]
                        .copy_from_slice(&requested[replaced..replaced + needed]);
                    let free = u16_at(after, FREE_OFFSET)?
                        .checked_sub(u16::try_from(extra).map_err(|_| EditError::Preservation)?)
                        .ok_or(EditError::Preservation)?;
                    after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
                        .copy_from_slice(&free.to_le_bytes());
                    after[slot_offset(slot)..slot_offset(slot) + size_of::<u16>()].copy_from_slice(
                        &u16::try_from(new_start)
                            .map_err(|_| EditError::Preservation)?
                            .to_le_bytes(),
                    );
                } else {
                    let released = available - needed;
                    if released != 0 && slot + 1 != row_count(&page)? {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                    let base = number as usize * PAGE_BYTES;
                    let after = expected
                        .get_mut(base..base + PAGE_BYTES)
                        .ok_or(EditError::Preservation)?;
                    reader.work(needed + link_bytes)?;
                    // Preserve original links and freed-prefix bytes. Only the
                    // final physical row may move its start toward its fixed end.
                    if kind == 0 {
                        after[from + released..from + released + WORD_BYTES]
                            .copy_from_slice(&next.to_le_bytes());
                    }
                    after[from + released + link_bytes..to]
                        .copy_from_slice(&requested[replaced..replaced + needed]);
                    if released != 0 {
                        let free = u16_at(after, FREE_OFFSET)?
                            .checked_add(
                                u16::try_from(released).map_err(|_| EditError::Preservation)?,
                            )
                            .ok_or(EditError::Preservation)?;
                        after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
                            .copy_from_slice(&free.to_le_bytes());
                        let new_start =
                            u16::try_from(from + released).map_err(|_| EditError::Preservation)?;
                        after[slot_offset(slot)..slot_offset(slot) + size_of::<u16>()]
                            .copy_from_slice(&new_start.to_le_bytes());
                    }
                }
                replaced += needed;
                reference = next;
                if last {
                    completed = true;
                    break;
                }
            }
            if !completed || replaced != requested.len() {
                return Err(EditError::Preservation);
            }
            let length = u32::try_from(requested.len()).map_err(|_| EditError::Preservation)?;
            if length & TYPE_MASK != 0 {
                return Err(EditError::Preservation);
            }
            let at = original.offset + start;
            reader.work(WORD_BYTES)?;
            expected
                .get_mut(at..at + WORD_BYTES)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&(kind | length).to_le_bytes());
            changed = true;
        }
        changed_rows += usize::from(changed);
    }
    let scope = image.catalog().catalog().scope();
    let final_image = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = normal_ordinary::capture(&final_image, schema.name(), budget, cancelled)?;
    if !super::admission::selected(&final_image, &final_source, bound, budget, cancelled)? {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok((expected, changed_rows))
}
