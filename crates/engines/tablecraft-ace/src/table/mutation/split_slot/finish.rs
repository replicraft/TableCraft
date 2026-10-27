//! Keep slot request authority distinct from append composition and index proof.
use super::super::{EditError, WriteBlocker, gates, index_append};
use super::{Operation, Staged, authority};
use crate::{
    Ace16WritePreview, IndexCheckStatus, NativeImage, check_table, check_table_constraints,
    header::check, inspect_catalog, table::indexes::SlotKeyChanges,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(super) fn finish(
    image: NativeImage,
    operation: &Operation<'_>,
    staged: &Staged<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    check(budget, cancelled)?;
    index_append::qualify(&image, &staged.plan)?;

    let original = authority::prove(&image, operation, staged, budget, cancelled)?;
    let after = index_append::compose(
        &image,
        &staged.slots.patches,
        &staged.plan,
        budget,
        cancelled,
    )?;

    let keys = original.slots.keys(original.column, budget, cancelled)?;
    staged.plan.audit_slot(
        &image,
        &original.structural,
        &SlotKeyChanges {
            column: original.column,
            updates: &keys,
        },
        &after,
        budget,
        cancelled,
    )?;

    let scope = image.catalog().catalog().scope();
    inspect_catalog(&mut Cursor::new(&after), scope, budget, cancelled)?;
    let checked = check_table(
        &mut Cursor::new(&after),
        scope,
        operation.table,
        budget,
        cancelled,
    )?;
    if checked
        .indexes()
        .iter()
        .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }

    gates::qualified(&check_table_constraints(
        &mut Cursor::new(&after),
        scope,
        operation.table,
        budget,
        cancelled,
    )?)?;
    check(budget, cancelled)?;

    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after,
        original.slots.changed_rows(),
    ))
}
