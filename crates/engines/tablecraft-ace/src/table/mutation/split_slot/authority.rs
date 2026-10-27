//! Immutable original requests grant exact slot patches and replacement keys.
use super::super::{EditError, patch::BytePatch, slot};
use super::{Operation, Staged, preflight};
use crate::{InspectError, NativeImage, TableCheck, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Rebound<'a> {
    pub structural: TableCheck,
    pub column: usize,
    pub slots: slot::stage::Staged<'a>,
}

pub(super) fn prove<'a>(
    image: &NativeImage,
    operation: &Operation<'a>,
    staged: &Staged<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Rebound<'a>, EditError> {
    let (structural, column) = preflight(image, operation, budget, cancelled)?;

    if structural.schema().definition != staged.structural.schema().definition
        || structural.schema().object_id() != staged.structural.schema().object_id()
        || column != staged.column
    {
        return Err(EditError::Preservation);
    }

    let slots = slot::stage::stage(
        image,
        &structural,
        column,
        operation.requests,
        budget,
        cancelled,
    )?;

    if slots.patches.len() != staged.slots.patches.len()
        || slots.changed_rows() != staged.slots.changed_rows()
    {
        return Err(EditError::Preservation);
    }

    for (expected, actual) in slots.patches.iter().zip(&staged.slots.patches) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (2 * expected.before().len()) as u64)
            .map_err(InspectError::from)?;
        if expected.offset() != actual.offset()
            || expected.before() != actual.before()
            || expected.after() != actual.after()
        {
            return Err(EditError::Preservation);
        }
    }

    Ok(Rebound {
        structural,
        column,
        slots,
    })
}
