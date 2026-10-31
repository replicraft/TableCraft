//! Final typed intents from original user declarations and projected execution.
use super::super::super::{BeforeEventGuardError, buffer};
use super::super::BeforeChangeAssignment;
use super::{
    bound::{BoundPrograms, SourceFacts},
    frames::ProjectedRow,
    requests::Batch,
    targets::Targets,
};
use crate::{FixedScalarUpdate, InspectError, header::check};
use tablecraft_core::{limits::ResourceBudget, values::Value};

pub(in crate::table::mutation) struct FinalGroup {
    column: usize,
    updates: Vec<FixedScalarUpdate>,
}

impl FinalGroup {
    pub(in crate::table::mutation) fn column(&self) -> usize {
        self.column
    }

    pub(in crate::table::mutation) fn updates(&self) -> &[FixedScalarUpdate] {
        &self.updates
    }
}

/// Consume projected trace positions only after independent execution auditing.
/// This mutates no source/candidate byte and allocates no additional output.
pub(super) fn native_assignment_positions(
    programs: &BoundPrograms<'_, '_, '_>,
    assignments: &mut [BeforeChangeAssignment],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), BeforeEventGuardError> {
    for assignment in assignments {
        check(budget, cancelled)?;
        assignment.column = programs.projection().native(assignment.column)?;
    }
    Ok(())
}

/// Caller must handle rejection before invoking this function. Projected rows
/// remain internal; they never become public complete BeforeChangeFinalRows.
pub(super) fn final_groups(
    source: SourceFacts<'_>,
    batch: &Batch<'_>,
    targets: &Targets,
    completed: &[ProjectedRow],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<FinalGroup>, BeforeEventGuardError> {
    if completed.len() != batch.ordinals.len() {
        return Err(BeforeEventGuardError::Preservation);
    }
    for (row, ordinal) in completed.iter().zip(&batch.ordinals) {
        check(budget, cancelled)?;
        if row.ordinal != *ordinal {
            return Err(BeforeEventGuardError::Preservation);
        }
    }
    let mut result = buffer::vector(targets.items.len(), budget, cancelled)?;
    for target in &targets.items {
        check(budget, cancelled)?;
        let capacity = if target.macro_write {
            completed.len()
        } else {
            batch
                .groups
                .get(target.user.ok_or(InspectError::Malformed)?)
                .ok_or(InspectError::Malformed)?
                .updates
                .len()
        };
        let mut updates = buffer::vector(capacity, budget, cancelled)?;
        if target.macro_write {
            let frame = target.frame.ok_or(InspectError::Malformed)?;
            for row in completed {
                check(budget, cancelled)?;
                let before = source
                    .table
                    .rows()
                    .get(row.ordinal)
                    .and_then(|values| values.get(target.snapshot))
                    .ok_or(InspectError::Malformed)?;
                let after = row.values.get(frame).ok_or(InspectError::Malformed)?;
                if !matches!(before, Value::Null | Value::Int32(_))
                    || !matches!(after, Value::Null | Value::Int32(_))
                {
                    return Err(InspectError::Malformed.into());
                }
                updates.push(FixedScalarUpdate::new(
                    row.ordinal,
                    immediate(before)?,
                    immediate(after)?,
                ));
            }
        } else {
            let group = batch
                .groups
                .get(target.user.ok_or(InspectError::Malformed)?)
                .ok_or(InspectError::Malformed)?;
            for update in group.updates {
                check(budget, cancelled)?;
                // Includes genuine empty declarations and bit-exact raw no-ops.
                updates.push(FixedScalarUpdate::new(
                    update.row(),
                    immediate(update.expected())?,
                    immediate(update.value())?,
                ));
            }
        }
        result.push(FinalGroup {
            column: target.native,
            updates,
        });
    }
    check(budget, cancelled)?;
    Ok(result)
}

fn immediate(value: &Value) -> Result<Value, BeforeEventGuardError> {
    match value {
        Value::Null => Ok(Value::Null),
        Value::Boolean(value) => Ok(Value::Boolean(*value)),
        Value::Int32(value) => Ok(Value::Int32(*value)),
        Value::Int64(value) => Ok(Value::Int64(*value)),
        Value::Currency(value) => Ok(Value::Currency(*value)),
        Value::DateTime(value) => Ok(Value::DateTime(*value)),
        _ => Err(InspectError::Malformed.into()),
    }
}
