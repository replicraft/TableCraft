//! Compose original-bound retirement and retained payload edits in one parent frame.
pub(super) mod proof;
pub(super) mod stage;

use super::super::binding::{self, Bound};
use crate::header::check;
use crate::{EditError, FittedColumnUpdates};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn selected(
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let mut retiring = false;
    let mut retained = false;
    for assignment in &bound.assignments {
        check(budget, cancelled)?;
        let cell = groups[assignment.group]
            .cell(assignment.index)
            .ok_or(EditError::Preservation)?;
        if binding::equal(cell.expected, cell.value, budget, cancelled)? {
            continue;
        }
        if super::retirement::replacement(cell.value).is_some() {
            retiring = true;
        } else {
            retained = true;
        }
    }
    Ok(retiring && retained)
}
