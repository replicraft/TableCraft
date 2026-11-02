//! Independent retained chain replay plus original-relative complete parent proof.
use super::super::{proof as retained, retirement};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, WriteBlocker,
    format::bytes::u32_at,
    header::check,
    table::{
        mutation::{
            normal_fitted::{
                binding::{self, Bound, Target},
                original_values,
            },
            normal_ordinary,
            preserved_source::Source,
            record_serialization::proof::{
                long_value::{self, Payload},
                multiple::{self, Encoding, Requested},
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
const TYPE_MASK: u32 = 0xc000_0000;

pub(in crate::table::mutation::normal_fitted::external) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, usize), EditError> {
    // This is the manual original-descriptor proof, never producer trace or patch data.
    let (mut expected, _) =
        retained::retained(image, source, groups, bound, true, budget, cancelled)?;
    let schema = source.table.schema();
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
        let capacity = selected.end - selected.start;
        let mut requests = binding::vector(capacity, budget)?;
        let mut tags = binding::vector(capacity, budget)?;
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
            let slot = field.field.variable;
            let from = *frame.offsets.get(slot).ok_or(EditError::Preservation)?;
            let to = *frame.offsets.get(slot + 1).ok_or(EditError::Preservation)?;
            let descriptor = original.data.get(from..to).ok_or(EditError::Preservation)?;
            let replacement = match cell.value {
                Value::Null => Some(Value::Null),
                Value::Binary(bytes) if bytes.is_empty() => Some(Value::Binary(Vec::new())),
                _ => None,
            };
            if let Some(replacement) = replacement {
                let references =
                    retirement::proof::references(image, descriptor, budget, cancelled)?;
                retiring.push(retirement::Retiring {
                    ordinal: selected.ordinal,
                    column,
                    references,
                    replacement,
                });
                requests.push(Requested {
                    column,
                    value: cell.value,
                });
            } else {
                let Payload::Present(bytes) =
                    long_value::requested(field, cell.value, budget, cancelled)?
                else {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                };
                let length = u32::try_from(bytes.len()).map_err(|_| EditError::Preservation)?;
                if length == 0 || length & TYPE_MASK != 0 {
                    return Err(EditError::Preservation);
                }
                tags.push((column, (u32_at(descriptor, 0)? & TYPE_MASK) | length));
            }
        }
        if requests.is_empty() && tags.is_empty() {
            continue;
        }
        // Reconstruct from the immutable parent, including original inactive slack.
        // The retained replay's old-offset descriptor write is wholly replaced here.
        let mut parent = multiple::reconstruct(
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
        let moved = record_layout::parse(&parent, schema, budget)?;
        for (column, word) in tags {
            check(budget, cancelled)?;
            let slot = schema.columns()[column].field.variable;
            let from = *moved.offsets.get(slot).ok_or(EditError::Preservation)?;
            let to = *moved.offsets.get(slot + 1).ok_or(EditError::Preservation)?;
            let old_from = *frame.offsets.get(slot).ok_or(EditError::Preservation)?;
            let old_to = *frame.offsets.get(slot + 1).ok_or(EditError::Preservation)?;
            let old = original
                .data
                .get(old_from..old_to)
                .ok_or(EditError::Preservation)?;
            let raw = parent.get_mut(from..to).ok_or(EditError::Preservation)?;
            budget
                .charge(Resource::Steps, (raw.len() + WORD_BYTES) as u64)
                .map_err(InspectError::from)?;
            if raw != old {
                return Err(EditError::Preservation);
            }
            raw.get_mut(..WORD_BYTES)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&word.to_le_bytes());
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
    let pages = retirement::authorize(image, source, &retiring, budget, cancelled)?;
    retirement::proof::maps_and_residuals(image, source, &pages, &mut expected, budget, cancelled)?;
    let after = NativeImage::capture(
        &mut Cursor::new(&expected),
        image.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    let final_source = normal_ordinary::capture(&after, schema.name(), budget, cancelled)?;
    if !super::super::admission::selected(&after, &final_source, bound, budget, cancelled)? {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok((expected, changed_rows))
}
