//! All original retained traces and whole-page retirement claims precede patches.
use super::super::{retirement, stage as retained};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage,
    format::bytes::u32_at,
    header::check,
    table::{
        mutation::{
            normal_fitted::{
                binding::{self, Bound},
                stage::Patch,
            },
            preserved_source::Source,
            record_serialization::{self, policy::RecordPolicy, variables},
        },
        record_layout,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const WORD_BYTES: usize = size_of::<u32>();
const DESCRIPTOR_BYTES: usize = 3 * WORD_BYTES;
const TYPE_MASK: u32 = 0xc000_0000;

pub(in crate::table::mutation::normal_fitted::external) struct Prepared {
    retiring: retirement::stage::Prepared,
    retained: retained::Prepared,
    rows: usize,
}

pub(in crate::table::mutation::normal_fitted::external) fn capture(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, EditError> {
    let retiring =
        retirement::stage::capture(image, source, groups, bound, true, budget, cancelled)?;
    let retained = retained::capture(image, source, groups, bound, true, budget, cancelled)?;
    let mut rows = 0_usize;
    for selected in &bound.rows {
        check(budget, cancelled)?;
        let mut changed = false;
        for assignment in &bound.assignments[selected.start..selected.end] {
            check(budget, cancelled)?;
            let cell = groups[assignment.group]
                .cell(assignment.index)
                .ok_or(EditError::Preservation)?;
            changed |= !binding::equal(cell.expected, cell.value, budget, cancelled)?;
        }
        rows = rows
            .checked_add(usize::from(changed))
            .ok_or(InspectError::StructureLimit)?;
    }
    check(budget, cancelled)?;
    Ok(Prepared {
        retiring,
        retained,
        rows,
    })
}

fn parents(
    source: &Source,
    bound: &Bound,
    prepared: &Prepared,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Patch>, EditError> {
    let schema = source.table.schema();
    let mut patches = binding::vector(bound.rows.len(), budget)?;
    for selected in &bound.rows {
        check(budget, cancelled)?;
        let original = &source.records[selected.ordinal];
        let frame = record_layout::parse(&original.data, schema, budget)?;
        let mut cells = binding::vector(prepared.retiring.values.len(), budget)?;
        for value in &prepared.retiring.values {
            check(budget, cancelled)?;
            if value.ordinal == selected.ordinal {
                cells.push(variables::Cell {
                    column: value.column,
                    value: &value.replacement,
                });
            }
        }
        let mut has_retained = false;
        for change in &prepared.retained.changes {
            check(budget, cancelled)?;
            has_retained |= change.ordinal == selected.ordinal;
        }
        if cells.is_empty() && !has_retained {
            continue;
        }
        let mut after = variables::rebuild(
            schema,
            &original.data,
            &frame,
            &cells,
            RecordPolicy::FittedDeclared,
            budget,
            cancelled,
        )?;
        if after.len() != original.data.len() {
            return Err(EditError::Preservation);
        }
        let moved = record_layout::parse(&after, schema, budget)?;
        for change in &prepared.retained.changes {
            check(budget, cancelled)?;
            if change.ordinal != selected.ordinal {
                continue;
            }
            let slot = schema.columns()[change.column].field.variable;
            let from = *moved.offsets.get(slot).ok_or(EditError::Preservation)?;
            let to = *moved.offsets.get(slot + 1).ok_or(EditError::Preservation)?;
            let old_from = *frame.offsets.get(slot).ok_or(EditError::Preservation)?;
            let old_to = *frame.offsets.get(slot + 1).ok_or(EditError::Preservation)?;
            let old = original
                .data
                .get(old_from..old_to)
                .ok_or(EditError::Preservation)?;
            let descriptor = after.get_mut(from..to).ok_or(EditError::Preservation)?;
            budget
                .charge(Resource::Steps, (DESCRIPTOR_BYTES + WORD_BYTES) as u64)
                .map_err(InspectError::from)?;
            if descriptor.len() != DESCRIPTOR_BYTES
                || descriptor != old
                || u32_at(descriptor, 0)? & TYPE_MASK != change.tagged & TYPE_MASK
            {
                return Err(EditError::Preservation);
            }
            descriptor[..WORD_BYTES].copy_from_slice(&change.tagged.to_le_bytes());
        }
        patches.push(Patch {
            offset: original.offset,
            before: record_serialization::copy(&original.data, budget, cancelled)?,
            after,
        });
    }
    Ok(patches)
}

pub(in crate::table::mutation::normal_fitted::external) fn prepare(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Patch>, usize), EditError> {
    let prepared = capture(image, source, groups, bound, budget, cancelled)?;
    let parents = parents(source, bound, &prepared, budget, cancelled)?;
    let retirement = retirement::stage::pages_and_maps(
        image,
        source,
        &prepared.retiring.pages,
        budget,
        cancelled,
    )?;
    let rows = prepared.rows;
    let retained = retained::payloads(image, prepared.retained, false, budget, cancelled)?;
    let capacity = parents
        .len()
        .checked_add(retirement.len())
        .and_then(|count| count.checked_add(retained.len()))
        .ok_or(InspectError::StructureLimit)?;
    let mut patches = binding::vector(capacity, budget)?;
    for patch in parents.into_iter().chain(retained).chain(retirement) {
        check(budget, cancelled)?;
        patches.push(patch);
    }
    check(budget, cancelled)?;
    Ok((patches, rows))
}
