//! One original record produces one fitted payload with accumulating mask bits.
use super::{
    binding::{self, Target},
    original_values,
};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, WriteBlocker,
    format::row::{COLUMN_COUNT_BYTES, MASK_BITS},
    header::check,
    table::{
        mutation::{
            preserved_source::Source,
            record_serialization::{self, policy::RecordPolicy, variables},
        },
        record_layout, scalar_encoding,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const INT32_BYTES: usize = size_of::<i32>();
const WIDE_BYTES: usize = size_of::<i64>();

pub(in crate::table::mutation) struct Patch {
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

pub(in crate::table::mutation) fn prepare(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Patch>, usize), EditError> {
    prepare_with(
        image,
        source,
        groups,
        RecordPolicy::FittedDeclared,
        budget,
        cancelled,
    )
}

pub(in crate::table::mutation) fn prepare_with(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    policy: RecordPolicy,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Patch>, usize), EditError> {
    let bound = binding::all(source, groups, budget, cancelled)?;
    prepare_bound(image, source, groups, policy, bound, budget, cancelled)
}

pub(super) fn prepare_indexed_binary(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Patch>, usize), EditError> {
    let bound = binding::all_indexed_binary(source, groups, budget, cancelled)?;
    prepare_bound(
        image,
        source,
        groups,
        RecordPolicy::FittedDeclared,
        bound,
        budget,
        cancelled,
    )
}

pub(super) fn prepare_indexed_text(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Patch>, usize), EditError> {
    let bound = binding::all_indexed_text(source, groups, budget, cancelled)?;
    prepare_bound(
        image,
        source,
        groups,
        RecordPolicy::FittedDeclared,
        bound,
        budget,
        cancelled,
    )
}

fn prepare_bound(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    policy: RecordPolicy,
    bound: binding::Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Patch>, usize), EditError> {
    if super::external::selected(image, source, &bound, budget, cancelled)? {
        if policy.prefers_growth() {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        return super::external::stage::prepare(image, source, groups, &bound, budget, cancelled);
    }
    super::inline::qualify(image, source, groups, &bound, budget, cancelled)?;
    if bound.has_long_targets && policy.prefers_growth() {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let schema = source.table.schema();
    let mut patches = binding::vector(bound.rows.len(), budget)?;
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
        let mut variable = binding::vector(bound.targets.len(), budget)?;
        let mut changed = false;
        for assignment in &bound.assignments[selected.start..selected.end] {
            let cell = groups[assignment.group]
                .cell(assignment.index)
                .ok_or(EditError::Preservation)?;
            let value = binding::value(&bound, &values, assignment.group, budget, cancelled)?;
            if !binding::equal(value, cell.expected, budget, cancelled)? {
                return Err(EditError::Conflict);
            }
            if binding::equal(cell.expected, cell.value, budget, cancelled)? {
                continue;
            }
            changed = true;
            if let Target::Variable { column } = bound.targets[assignment.group] {
                variable.push(variables::Cell {
                    column,
                    value: cell.value,
                });
            }
        }
        if !changed {
            continue;
        }
        let layout = record_layout::parse(&original.data, schema, budget)?;
        let mut after = variables::rebuild(
            schema,
            &original.data,
            &layout,
            &variable,
            policy,
            budget,
            cancelled,
        )?;
        if !policy.prefers_growth() && after.len() != original.data.len() {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let mask_start = after
            .len()
            .checked_sub(layout.count.div_ceil(MASK_BITS))
            .ok_or(EditError::Preservation)?;
        for assignment in &bound.assignments[selected.start..selected.end] {
            let Target::Fixed {
                column,
                kind,
                width,
            } = bound.targets[assignment.group]
            else {
                continue;
            };
            let cell = groups[assignment.group]
                .cell(assignment.index)
                .ok_or(EditError::Preservation)?;
            if binding::equal(cell.expected, cell.value, budget, cancelled)? {
                continue;
            }
            let field = &schema.columns()[column];
            let present = match cell.value {
                Value::Boolean(value) => *value,
                Value::Null => false,
                _ => true,
            };
            budget
                .charge(Resource::Steps, (width + 1) as u64)
                .map_err(InspectError::from)?;
            if present && width != 0 {
                let mut payload = [0; WIDE_BYTES];
                match width {
                    INT32_BYTES => payload[..INT32_BYTES].copy_from_slice(
                        &scalar_encoding::encode::<INT32_BYTES>(kind, cell.value)
                            .ok_or(EditError::Preservation)?,
                    ),
                    WIDE_BYTES => payload.copy_from_slice(
                        &scalar_encoding::encode::<WIDE_BYTES>(kind, cell.value)
                            .ok_or(EditError::Preservation)?,
                    ),
                    _ => return Err(EditError::Preservation),
                }
                let start = COLUMN_COUNT_BYTES + field.field.offset;
                let end = start.checked_add(width).ok_or(EditError::Preservation)?;
                if end > layout.data_end {
                    return Err(EditError::Preservation);
                }
                after
                    .get_mut(start..end)
                    .ok_or(EditError::Preservation)?
                    .copy_from_slice(&payload[..width]);
            }
            let mask = after
                .get_mut(mask_start + field.number() / MASK_BITS)
                .ok_or(EditError::Preservation)?;
            let bit = 1 << (field.number() % MASK_BITS);
            *mask = (*mask & !bit) | if present { bit } else { 0 };
        }
        if binding::equal_bytes(&after, &original.data, budget, cancelled)? {
            return Err(EditError::Preservation);
        }
        patches.push(Patch {
            offset: original.offset,
            before: record_serialization::copy(&original.data, budget, cancelled)?,
            after,
        });
    }
    check(budget, cancelled)?;
    let changed_rows = patches.len();
    Ok((patches, changed_rows))
}
