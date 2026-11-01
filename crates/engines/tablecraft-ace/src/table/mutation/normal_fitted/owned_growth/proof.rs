//! Fresh original requests independently reconstruct records and complete pages.
mod placement;
use super::super::{
    binding::{self, Target},
    original_values,
};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage,
    format::row::{COLUMN_COUNT_BYTES, MASK_BITS},
    header::check,
    table::{
        mutation::{
            preserved_source::{Source, validation},
            record_placement::space::Space,
            record_roots,
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
    let mut records = binding::vector(captured.selected.len(), budget)?;
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
        if let Some(record) =
            reconstruct(&image, &source, &bound, selected, groups, budget, cancelled)?
        {
            records.push((root, record));
        }
    }
    let mut placement =
        placement::Placement::capture(&image, &space, &captured.owned, budget, cancelled)?;
    let mut operations = binding::vector(records.len(), budget)?;
    for (root, record) in records {
        let choice = placement.reserve(root, &record, budget, cancelled)?;
        operations.push((root, record, choice));
    }
    let changed_rows = operations.len();
    let mut expected = record_serialization::copy(image.as_bytes(), budget, cancelled)?;
    for (root, record, choice) in operations {
        placement::apply(
            &image,
            &space,
            root,
            &record,
            &choice,
            &mut expected,
            budget,
            cancelled,
        )?;
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
