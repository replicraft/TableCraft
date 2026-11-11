//! One native source/request plan supplies both review facts and consuming owner.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::{
        EditError, FieldRenameReview, FieldRenamedReopen, ReviewedFieldRename,
        platform::ids::issue_id,
    },
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_access::editing::journal::NativeJournalEditor;
use tablecraft_ace::RecoverableFieldRenamePlan;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<FieldRenamedReopen>,
    pub(in crate::editing) plan: RecoverableFieldRenamePlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    column: &str,
    native_number: usize,
    new_name: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_field_rename").entered();
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    let mut work = budget();
    check(cancelled)?;
    // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this shares
    // immutable bytes instead of copying the database image.
    let image = NativeJournalEditor::capture_shared_image(
        document.edits.image_source.clone(),
        SCOPE,
        table,
        &mut work,
        cancelled,
    )?;
    // Select from original metadata before constructing a native candidate.
    // Old serialized profiles retain their closed reconstruction languages.
    let retained_schema = image.has_retained_table_source(table, &mut work, cancelled)?;
    let retained = !retained_schema
        && image
            .select_native_retained_field_rename_source(table, column, &mut work, cancelled)
            .map_err(Error::FieldRenameSource)?;
    let retained_typed = retained_schema
        && image
            .select_native_retained_typed_field_rename_source(table, column, &mut work, cancelled)
            .map_err(Error::FieldRenameSource)?;
    let plan = if retained_typed {
        image.plan_recoverable_retained_typed_field_rename(
            table, column, new_name, &mut work, cancelled,
        )?
    } else if retained_schema {
        image.plan_recoverable_retained_schema_field_rename(
            table, column, new_name, &mut work, cancelled,
        )?
    } else if retained {
        image
            .plan_recoverable_retained_field_rename(table, column, new_name, &mut work, cancelled)?
    } else {
        image
            .plan_recoverable_current_field_rename(table, column, new_name, &mut work, cancelled)?
    };
    if plan.table() != table
        || plan.column() != column
        || plan.identity().column_number() != native_number
    {
        return Err(Error::StaleFieldRenameTarget);
    }
    let expected = super::super::reopen_image::capture(plan.after_bytes(), &mut work, cancelled)?;
    let mut output = Presentation::new(cancelled);
    let review = FieldRenameReview {
        id: issue_id()?,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        column: output.text(plan.column())?,
        native_number: plan.identity().column_number(),
        new_name: output.text(plan.new_name())?,
        changed: plan.changed(),
    };
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<FieldRenamedReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(FieldRenamedReopen {
        // NO_CLONE: The capability or reopen witness outlives this borrowed Document call and
        // owns its source path; the live Document keeps its own path.
        path: document.edits.path.clone(),
        expected,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        column: output.text(plan.column())?,
        native_number: plan.identity().column_number(),
        new_name: output.text(plan.new_name())?,
        changed: plan.changed(),
    });
    let capability = ReviewedFieldRename {
        state: Box::new(State {
            reopen,
            plan,
            valid: Arc::clone(&document.edits.valid),
        }),
    };
    check(cancelled)?;
    Ok(Response::FieldRenameReviewed { review, capability })
}
