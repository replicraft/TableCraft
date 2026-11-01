//! Fresh original requests independently reconstruct records and complete pages.
use super::super::{
    binding::{self, Target},
    original_values,
};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    format::{
        bytes::u16_at,
        page::PAGE_BYTES,
        reader::row_count,
        row::{
            COLUMN_COUNT_BYTES, DELETED_MASK, MASK_BITS, MAX_SLOTS, OFFSET_MASK, RELOCATED_MASK,
            ROW_COUNT_OFFSET, SLOT_BYTES, UNSUPPORTED_MASK, reference, reference_page,
            reference_slot, slot_offset,
        },
    },
    header::check,
    table::{
        mutation::{
            preserved_source::{Source, validation},
            record_placement::space::Space,
            record_roots::{self, Root},
            record_serialization::{self, proof as variable_proof},
        },
        record_layout,
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const FREE_BYTES_OFFSET: usize = 2;
const POINTER_BYTES: usize = size_of::<u32>();

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.ordinary.owned_growth.prove").entered();
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = super::capture(&image, table, budget, cancelled)?;
    let bound = binding::all(&source, groups, budget, cancelled)?;
    let captured = record_roots::capture(
        &image,
        source.table.schema(),
        &bound.rows,
        budget,
        cancelled,
    )?;
    let space = Space::capture_normal(
        &image,
        source.table.schema(),
        &captured.owned,
        budget,
        cancelled,
    )?;
    let mut expected = record_serialization::copy(image.as_bytes(), budget, cancelled)?;
    let mut changed_rows = 0_usize;
    for root in &captured.selected {
        check(budget, cancelled)?;
        let selected = root.update;
        let original = source
            .records
            .get(selected.ordinal)
            .ok_or(EditError::Preservation)?;
        if original.offset != root.final_record.offset
            || !binding::equal_bytes(&original.data, &root.final_record.data, budget, cancelled)?
        {
            return Err(EditError::Preservation);
        }
        let Some(record) =
            reconstruct(&image, &source, &bound, selected, groups, budget, cancelled)?
        else {
            continue;
        };
        place(
            &image,
            &space,
            root,
            &record,
            &mut expected,
            budget,
            cancelled,
        )?;
        changed_rows += 1;
    }
    space.audit(image.as_bytes(), &expected, budget, cancelled)?;
    if changed_rows != plan.changed_rows()
        || !binding::equal_bytes(&expected, plan.after_bytes(), budget, cancelled)?
    {
        return Err(EditError::Preservation);
    }
    validation::expected(&image, table, &source.table, &expected, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(())
}

fn reconstruct(
    image: &NativeImage,
    source: &Source,
    bound: &binding::Bound,
    selected: &binding::Selected,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Vec<u8>>, EditError> {
    let schema = source.table.schema();
    let original = &source.records[selected.ordinal];
    let values = original_values(
        image,
        source,
        selected.ordinal,
        &bound.columns,
        budget,
        cancelled,
    )?;
    let mut variables = binding::vector(bound.targets.len(), budget)?;
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
        changed = true;
        if let Target::Variable { column } = bound.targets[assignment.group] {
            variables.push(variable_proof::multiple::Requested {
                column,
                value: cell.value,
            });
        }
    }
    if !changed {
        return Ok(None);
    }
    // These proof encoders derive raw payloads/offsets afresh; producer record
    // choices, stage pages and event ranges never enter this reconstruction.
    let mut row = match variables.as_slice() {
        [] => record_serialization::copy(&original.data, budget, cancelled)?,
        [one] => variable_proof::reconstruct(
            schema,
            one.column,
            &original.data,
            one.value,
            budget,
            cancelled,
        )?,
        _ => variable_proof::multiple::reconstruct(
            schema,
            &original.data,
            &variables,
            variable_proof::multiple::Encoding::Growing,
            budget,
            cancelled,
        )?,
    };
    let layout = record_layout::parse(&row, schema, budget)?;
    for assignment in &bound.assignments[selected.start..selected.end] {
        let Target::Fixed { column, width, .. } = bound.targets[assignment.group] else {
            continue;
        };
        let cell = groups[assignment.group]
            .cell(assignment.index)
            .ok_or(EditError::Preservation)?;
        if binding::equal(cell.expected, cell.value, budget, cancelled)? {
            continue;
        }
        let field = &schema.columns()[column];
        let mut payload = [0; size_of::<i64>()];
        let present = match cell.value {
            Value::Null => false,
            Value::Boolean(value) => *value,
            Value::Int32(value) => {
                payload[..size_of::<i32>()].copy_from_slice(&value.to_le_bytes());
                true
            }
            Value::Int64(value) => {
                payload.copy_from_slice(&value.to_le_bytes());
                true
            }
            Value::Currency(value) => {
                payload.copy_from_slice(&value.scaled().to_le_bytes());
                true
            }
            Value::DateTime(value) => {
                payload.copy_from_slice(&value.to_bits().to_le_bytes());
                true
            }
            _ => return Err(EditError::Preservation),
        };
        budget
            .charge(Resource::Steps, (width + 1) as u64)
            .map_err(InspectError::from)?;
        if present && width != 0 {
            let start = COLUMN_COUNT_BYTES
                .checked_add(field.field.offset)
                .ok_or(EditError::Preservation)?;
            let end = start.checked_add(width).ok_or(EditError::Preservation)?;
            if end > layout.data_end {
                return Err(EditError::Preservation);
            }
            row.get_mut(start..end)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&payload[..width]);
        }
        let mask = row
            .get_mut(layout.mask_start + field.number() / MASK_BITS)
            .ok_or(EditError::Preservation)?;
        let bit = 1 << (field.number() % MASK_BITS);
        *mask = (*mask & !bit) | if present { bit } else { 0 };
    }
    Ok(Some(row))
}

fn place(
    image: &NativeImage,
    space: &Space,
    root: &Root<'_, binding::Selected>,
    row: &[u8],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let root_page = reference_page(root.reference);
    let terminal_page = u32::try_from(root.final_record.offset / PAGE_BYTES)
        .map_err(|_| EditError::Preservation)?;
    space.require_page(image, root_page, false, budget, cancelled)?;
    space.require_page(image, terminal_page, false, budget, cancelled)?;
    if row.len() < root.final_record.data.len() {
        return Err(EditError::Preservation);
    }
    if row.len() == root.final_record.data.len() {
        let end = root
            .final_record
            .offset
            .checked_add(row.len())
            .ok_or(EditError::Preservation)?;
        let destination = expected
            .get_mut(root.final_record.offset..end)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, row.len() as u64)
            .map_err(InspectError::from)?;
        if destination != root.final_record.data.as_slice() {
            return Err(EditError::Preservation);
        }
        destination.copy_from_slice(row);
        return Ok(());
    }
    space.require_page(image, root_page, true, budget, cancelled)?;
    if root.range.len() < POINTER_BYTES || root.word & !(OFFSET_MASK | RELOCATED_MASK) != 0 {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let base = usize::try_from(root_page)
        .ok()
        .and_then(|page| page.checked_mul(PAGE_BYTES))
        .ok_or(EditError::Preservation)?;
    let page = expected
        .get_mut(base..base + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    let count = row_count(page)?;
    // Derive the physical frontier independently from the complete original
    // directory. Empty deleted slots consume a directory word, not body bytes.
    let mut end = page.len();
    for slot in 0..count {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, size_of::<u16>() as u64)
            .map_err(InspectError::from)?;
        let word = u16_at(page, slot_offset(slot))?;
        if word & UNSUPPORTED_MASK != 0 {
            return Err(InspectError::Unsupported.into());
        }
        let start = usize::from(word & OFFSET_MASK);
        if start < slot_offset(count) || start > end || (start == end && word & DELETED_MASK == 0) {
            return Err(InspectError::Malformed.into());
        }
        end = start;
    }
    if count >= MAX_SLOTS {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let slot = u8::try_from(count).map_err(|_| EditError::Preservation)?;
    let free = usize::from(u16_at(page, FREE_BYTES_OFFSET)?);
    let required = row
        .len()
        .checked_add(SLOT_BYTES)
        .ok_or(EditError::Preservation)?;
    if free
        != end
            .checked_sub(slot_offset(count))
            .ok_or(EditError::Preservation)?
        || free < required
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let start = end.checked_sub(row.len()).ok_or(EditError::Preservation)?;
    if start < slot_offset(count + 1) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let root_start = root
        .range
        .start
        .checked_sub(base)
        .ok_or(EditError::Preservation)?;
    let root_end = root
        .range
        .end
        .checked_sub(base)
        .ok_or(EditError::Preservation)?;
    let word_at = slot_offset(usize::from(reference_slot(root.reference)));
    budget
        .charge(Resource::Steps, (root.range.len() + row.len()) as u64)
        .map_err(InspectError::from)?;
    if u16_at(page, word_at)? != root.word
        || page.get(root_start..root_end) != image.as_bytes().get(root.range.clone())
    {
        return Err(EditError::Preservation);
    }
    page.get_mut(start..end)
        .ok_or(EditError::Preservation)?
        .copy_from_slice(row);
    let slot_word = u16::try_from(start).map_err(|_| EditError::Preservation)? | DELETED_MASK;
    page[slot_offset(count)..slot_offset(count + 1)].copy_from_slice(&slot_word.to_le_bytes());
    let count_word = u16::try_from(count + 1).map_err(|_| EditError::Preservation)?;
    page[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>()]
        .copy_from_slice(&count_word.to_le_bytes());
    let free_word = u16::try_from(free - required).map_err(|_| EditError::Preservation)?;
    page[FREE_BYTES_OFFSET..FREE_BYTES_OFFSET + size_of::<u16>()]
        .copy_from_slice(&free_word.to_le_bytes());
    if root.word & RELOCATED_MASK == 0 {
        page[root_start..root_end].fill(0);
    }
    page[root_start..root_start + POINTER_BYTES]
        .copy_from_slice(&reference(root_page, slot).to_le_bytes());
    page[word_at..word_at + SLOT_BYTES]
        .copy_from_slice(&(root.word | RELOCATED_MASK).to_le_bytes());
    check(budget, cancelled)?;
    Ok(())
}
