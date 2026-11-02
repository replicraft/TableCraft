//! Rewalk original descriptors and derive parent, maps and residuals without patches.
use super::{Page, Retiring, authorize};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, WriteBlocker,
    check_table_long_values,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_range},
        row::{DELETED_MASK, RELOCATED_MASK, reference_page, reference_slot},
    },
    header::check,
    limits::MAX_LONG_VALUE_CHUNKS,
    table::{
        mutation::{
            normal_fitted::{
                binding::{self, Bound, Target},
                original_values,
            },
            normal_ordinary,
            preserved_source::Source,
            record_serialization::{
                self,
                proof::multiple::{self, Encoding, Requested},
            },
        },
        record_layout,
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const WORD_BYTES: usize = size_of::<u32>();
const DESCRIPTOR_BYTES: usize = 3 * WORD_BYTES;
const TYPE_MASK: u32 = 0xc000_0000;
const SINGLE: u32 = 0x4000_0000;
const LVAL_OWNER: &[u8; WORD_BYTES] = b"LVAL";
const INVALID_PREFIX_BYTES: usize = size_of::<u32>();

pub(in crate::table::mutation::normal_fitted::external) fn references(
    image: &NativeImage,
    descriptor: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, EditError> {
    if descriptor.len() != DESCRIPTOR_BYTES || descriptor[2 * WORD_BYTES..] != [0; WORD_BYTES] {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let word = u32_at(descriptor, 0)?;
    let kind = word & TYPE_MASK;
    if !matches!(kind, 0 | SINGLE) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let length = (word & !TYPE_MASK) as usize;
    let mut reference = u32_at(descriptor, WORD_BYTES)?;
    let mut consumed = 0_usize;
    let mut visited = binding::vector(MAX_LONG_VALUE_CHUNKS, budget)?;
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
        let page = reader.page(reference_page(reference))?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || page.get(OWNER_OFFSET..OWNER_OFFSET + WORD_BYTES) != Some(LVAL_OWNER)
        {
            return Err(EditError::Preservation);
        }
        let (flags, start, end) = row_range(&page, usize::from(reference_slot(reference)))?;
        if flags & (DELETED_MASK | RELOCATED_MASK) != 0 {
            return Err(EditError::Preservation);
        }
        let link = if kind == 0 { WORD_BYTES } else { 0 };
        let payload = end
            .checked_sub(start + link)
            .filter(|length| *length != 0)
            .ok_or(EditError::Preservation)?;
        consumed = consumed
            .checked_add(payload)
            .filter(|count| *count <= length)
            .ok_or(EditError::Preservation)?;
        let next = if kind == 0 { u32_at(&page, start)? } else { 0 };
        if consumed == length {
            if next != 0 {
                return Err(EditError::Preservation);
            }
            reader.tick()?;
            return Ok(visited);
        }
        if next == 0 {
            return Err(EditError::Preservation);
        }
        reference = next;
    }
    Err(EditError::Preservation)
}

pub(in crate::table::mutation::normal_fitted::external) fn maps_and_residuals(
    image: &NativeImage,
    source: &Source,
    pages: &[Page],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let storage = check_table_long_values(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        source.table.schema().name(),
        budget,
        cancelled,
    )?;
    let mut bytes = image.reader();
    let mut reader = Reader {
        source: &mut bytes,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for page in pages {
        reader.tick()?;
        let field = &source.table.schema().columns()[page.column];
        let mut references = None;
        for column in storage.columns() {
            reader.tick()?;
            if column.column_number() == field.number() {
                references = column.maps();
            }
        }
        let references = references.ok_or(EditError::Preservation)?;
        for (reference, role, set) in [
            (references.owned(), MapRole::Owned, false),
            (references.candidate(), MapRole::LongValueCandidate, false),
            (GLOBAL_FREE_REFERENCE, MapRole::GlobalFree, true),
        ] {
            reader.tick()?;
            let map = RepresentedMap::read(&mut reader, reference, role)?;
            let cell = map.cell(&mut reader, page.number)?;
            let before = cell.before & cell.mask != 0;
            if before == set {
                if matches!(role, MapRole::LongValueCandidate) {
                    continue;
                }
                return Err(EditError::Preservation);
            }
            // Replay directly against the original single-bit cell. Prior
            // independent changes may affect other bits of the same byte only.
            reader.work(1)?;
            let target = expected
                .get_mut(cell.offset)
                .ok_or(EditError::Preservation)?;
            if *target & cell.mask != cell.before & cell.mask {
                return Err(EditError::Preservation);
            }
            *target = if set {
                *target | cell.mask
            } else {
                *target & !cell.mask
            };
        }
        let offset = page.number as usize * PAGE_BYTES;
        reader.work(INVALID_PREFIX_BYTES)?;
        expected
            .get_mut(offset..offset + INVALID_PREFIX_BYTES)
            .ok_or(EditError::Preservation)?
            .fill(0);
        // The whole-image copy retains every byte after this four-byte prefix,
        // including slots and payloads of the now retired page.
    }
    reader.tick()?;
    Ok(())
}

pub(in crate::table::mutation::normal_fitted::external) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, usize), EditError> {
    let schema = source.table.schema();
    let mut expected = record_serialization::copy(image.as_bytes(), budget, cancelled)?;
    let mut retiring = binding::vector(bound.assignments.len(), budget)?;
    let mut changed_rows = 0_usize;
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
        let frame = record_layout::parse(&original.data, schema, budget)?;
        let mut requests = binding::vector(selected.end - selected.start, budget)?;
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
            // This closed interpretation is independent of producer encoding.
            let replacement = match cell.value {
                Value::Null => Value::Null,
                Value::Binary(bytes) if bytes.is_empty() => Value::Binary(Vec::new()),
                _ => return Err(EditError::Unsupported(WriteBlocker::RowLayout)),
            };
            let slot = field.field.variable;
            let from = *frame.offsets.get(slot).ok_or(EditError::Preservation)?;
            let to = *frame.offsets.get(slot + 1).ok_or(EditError::Preservation)?;
            let descriptor = original.data.get(from..to).ok_or(EditError::Preservation)?;
            let references = references(image, descriptor, budget, cancelled)?;
            retiring.push(Retiring {
                ordinal: selected.ordinal,
                column,
                references,
                replacement,
            });
            requests.push(Requested {
                column,
                value: cell.value,
            });
        }
        if requests.is_empty() {
            continue;
        }
        let parent = multiple::reconstruct(
            schema,
            &original.data,
            &requests,
            Encoding::Fitted,
            budget,
            cancelled,
        )?;
        if parent.len() != original.data.len() {
            return Err(EditError::Preservation);
        }
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, parent.len() as u64)
            .map_err(InspectError::from)?;
        let end = original
            .offset
            .checked_add(parent.len())
            .ok_or(EditError::Preservation)?;
        expected
            .get_mut(original.offset..end)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&parent);
        changed_rows = changed_rows
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?;
    }
    let pages = authorize(image, source, &retiring, budget, cancelled)?;
    maps_and_residuals(image, source, &pages, &mut expected, budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = normal_ordinary::capture(&after, schema.name(), budget, cancelled)?;
    super::super::admission::selected(&after, &final_source, bound, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok((expected, changed_rows))
}
