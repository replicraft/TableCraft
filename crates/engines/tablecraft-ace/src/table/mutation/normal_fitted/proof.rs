//! Fresh BEFORE admission, separate variable encoding and manual fixed overlays.
use super::{
    binding::{self, Target},
    original_values,
};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage,
    format::row::{COLUMN_COUNT_BYTES, MASK_BITS},
    header::check,
    table::mutation::{
        normal_ordinary,
        preserved_source::validation,
        record_serialization::{self, proof as variable_proof},
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
    let _span = tracing::debug_span!("native.ordinary.fitted.prove").entered();
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = normal_ordinary::capture(&image, table, budget, cancelled)?;
    let bound = binding::all(&source, groups, budget, cancelled)?;
    let schema = source.table.schema();
    let mut expected = record_serialization::copy(image.as_bytes(), budget, cancelled)?;
    let mut changed_rows = 0_usize;
    for selected in &bound.rows {
        check(budget, cancelled)?;
        let original = &source.records[selected.ordinal];
        let values = original_values(
            &image,
            &source,
            selected.ordinal,
            &bound.columns,
            budget,
            cancelled,
        )?;
        let mut variables = binding::vector(bound.targets.len(), budget)?;
        let mut changed = false;
        for assignment in &bound.assignments[selected.start..selected.end] {
            let cell = groups[assignment.group]
                .cell(assignment.index)
                .ok_or(EditError::Preservation)?;
            if !binding::equal(
                binding::value(&bound, &values, assignment.group, budget, cancelled)?,
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
            continue;
        }
        // This is the independent reconstructor, not producer payload bytes or
        // the producer's declaration-encoding decisions and capacity cache.
        let mut row = match variables.as_slice() {
            [] => record_serialization::copy(&original.data, budget, cancelled)?,
            [one] => variable_proof::reconstruct_declared(
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
                variable_proof::multiple::Encoding::Fitted,
                budget,
                cancelled,
            )?,
        };
        if row.len() != original.data.len() {
            return Err(EditError::Preservation);
        }
        let layout = crate::table::record_layout::parse(&original.data, schema, budget)?;
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
            // Manual LE encoding is independent of scalar_encoding and stage.
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
                let start = COLUMN_COUNT_BYTES + field.field.offset;
                let end = start.checked_add(width).ok_or(EditError::Preservation)?;
                if end > layout.data_end {
                    return Err(EditError::Preservation);
                }
                row.get_mut(start..end)
                    .ok_or(EditError::Preservation)?
                    .copy_from_slice(&payload[..width]);
            }
            let bit = 1 << (field.number() % MASK_BITS);
            let mask = row
                .get_mut(layout.mask_start + field.number() / MASK_BITS)
                .ok_or(EditError::Preservation)?;
            *mask = (*mask & !bit) | if present { bit } else { 0 };
        }
        if binding::equal_bytes(&row, &original.data, budget, cancelled)? {
            return Err(EditError::Preservation);
        }
        let end = original
            .offset
            .checked_add(row.len())
            .ok_or(EditError::Preservation)?;
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, row.len() as u64)
            .map_err(InspectError::from)?;
        expected
            .get_mut(original.offset..end)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&row);
        changed_rows += 1;
    }
    if changed_rows != plan.changed_rows() {
        return Err(EditError::Preservation);
    }
    validation::expected(&image, table, &source.table, &expected, budget, cancelled)?;
    variable_proof::equal(&expected, plan.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(())
}
