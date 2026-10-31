//! One real Int32 event frame per original requested logical-row union.
use super::super::super::{BeforeEventGuardError, buffer, source::Frame};
use super::{bound::BoundPrograms, requests::Batch};
use crate::{InspectError, header::check};
use tablecraft_core::{limits::ResourceBudget, values::Value};

/// Internal projection result. Never construct public BeforeChangeFinalRow with
/// these partial values; public complete-final-row semantics stay unchanged.
#[derive(PartialEq, Eq)]
pub(in crate::table::mutation) struct ProjectedRow {
    pub(in crate::table::mutation::guards::fields) ordinal: usize,
    pub(in crate::table::mutation::guards::fields) values: Vec<Value>,
}

impl ProjectedRow {
    pub(in crate::table::mutation) fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(in crate::table::mutation) fn values(&self) -> &[Value] {
        &self.values
    }
}

pub(super) fn setup(
    programs: &BoundPrograms<'_, '_, '_>,
    batch: &Batch<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Frame>, BeforeEventGuardError> {
    let projection = programs.projection();
    let mut frames = buffer::vector(batch.ordinals.len(), budget, cancelled)?;
    let mut cursor = 0;
    for &ordinal in &batch.ordinals {
        check(budget, cancelled)?;
        let original = projection.copy_row(ordinal, budget, cancelled)?;
        let mut proposed = projection.copy_row(ordinal, budget, cancelled)?;
        while let Some(cell) = batch.cells.get(cursor).filter(|cell| cell.row == ordinal) {
            check(budget, cancelled)?;
            let group = batch
                .groups
                .get(cell.group)
                .ok_or(InspectError::Malformed)?;
            if let Some(position) = group.frame {
                let selected = proposed.get_mut(position).ok_or(InspectError::Malformed)?;
                *selected = match cell.update.value() {
                    Value::Null => Value::Null,
                    Value::Int32(value) => Value::Int32(*value),
                    _ => return Err(InspectError::Malformed.into()),
                };
            }
            // Other actual user carriers remain in Batch, never coerced into an
            // Int32 macro cell. Their original stale values were checked already.
            cursor += 1;
        }
        frames.push(Frame {
            ordinal,
            original: Some(original),
            proposed: Some(proposed),
        });
    }
    if cursor != batch.cells.len() {
        return Err(InspectError::Malformed.into());
    }
    check(budget, cancelled)?;
    Ok(frames)
}
