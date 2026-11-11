//! SQL declared ADD consumes its actual request, never a gallery snapshot.
use super::{Plan, issue_review};
use crate::{
    Error, Response,
    controls::{SCOPE, check},
    document::Document,
    editing::EditError,
};
use tablecraft_access::editing::journal::NativeJournalEditor;
use tablecraft_ace::{RecoverableUnadornedFieldAdditionPlan, UnadornedFieldAdditionDefinition};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    definition: UnadornedFieldAdditionDefinition,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    check(cancelled)?;
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    // NO_CLONE: The checked plan keeps the same immutable source allocation
    // independently from the live document's read owner; no image copy occurs.
    let image = NativeJournalEditor::capture_shared_image(
        document.edits.image_source.clone(),
        SCOPE,
        table,
        work,
        cancelled,
    )?;
    let plan = image
        .plan_recoverable_retained_unadorned_field_addition(table, definition, work, cancelled)?;
    let origin = super::declaration::captured_unadorned(plan.definition().column())?;
    let expected = plan.after_shared_bytes();
    check(cancelled)?;
    work.charge_allocation(1, size_of::<RecoverableUnadornedFieldAdditionPlan>() as u64)?;
    issue_review(
        document,
        Plan::Unadorned(Box::new(plan)),
        expected,
        origin,
        work,
        cancelled,
    )
}
