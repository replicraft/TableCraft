//! Read-only source qualification and lease-free native table DROP reviews.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::{
        EditError, ReviewedTableDrop, TableDropReview, TableDropSourceInspection,
        TableDroppedReopen, platform::ids::issue_id,
    },
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_access::editing::journal::NativeJournalEditor;
use tablecraft_ace::{RecoverableTableDropPlan, TableDropError, inspect_table_schema};

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<TableDroppedReopen>,
    pub(in crate::editing) plan: RecoverableTableDropPlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn inspect(
    document: &mut Document,
    table: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.inspect_table_drop_source").entered();
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    check(cancelled)?;
    let mut work = budget();
    // NO_CLONE: NativeImage retains the source while Document remains usable after inspection.
    // The existing Bytes-backed owner shares immutable storage without copying the database.
    let image = NativeJournalEditor::capture_shared_image(
        document.edits.image_source.clone(),
        SCOPE,
        table,
        &mut work,
        cancelled,
    )?;
    let schema = inspect_table_schema(&mut image.reader(), SCOPE, table, &mut work, cancelled)?;
    if schema.name() != table {
        return Err(Error::StaleTableDropTarget);
    }
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == schema.object_id())
        .ok_or(Error::StaleTableDropTarget)?;
    let native_id = origin.native_id();
    let declared_rows = schema.declared_rows();
    let source_eligible = match image.inspect_table_drop_source(table, &mut work, cancelled) {
        Ok(inspected) => {
            if inspected.identity().native_id() != native_id {
                return Err(Error::StaleTableDropTarget);
            }
            true
        }
        Err(TableDropError::Source | TableDropError::Dependency) => false,
        Err(error) => return Err(error.into()),
    };
    let mut output = Presentation::new(cancelled);
    let inspection = TableDropSourceInspection {
        document_revision: document.revision,
        table: output.text(schema.name())?,
        native_id,
        declared_rows,
        source_eligible,
    };
    check(cancelled)?;
    Ok(Response::TableDropSourceInspected { inspection })
}

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_table_drop").entered();
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    check(cancelled)?;
    let mut work = budget();
    // NO_CLONE: The plan owns the checked source while Document retains its current revision.
    // Bytes-backed sharing avoids another database payload; Rc cannot cross the save worker.
    let image = NativeJournalEditor::capture_shared_image(
        document.edits.image_source.clone(),
        SCOPE,
        table,
        &mut work,
        cancelled,
    )?;
    let plan = image.plan_recoverable_table_drop(table, &mut work, cancelled)?;
    if plan.table() != table {
        return Err(Error::StaleTableDropTarget);
    }
    let expected = super::reopen_image::capture(plan.after_bytes(), &mut work, cancelled)?;
    let mut output = Presentation::new(cancelled);
    let review = TableDropReview {
        id: issue_id()?,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        native_id: plan.identity().native_id(),
        declared_rows: plan.declared_rows(),
    };
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<TableDroppedReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(TableDroppedReopen {
        // NO_CLONE: The reopen witness must retain the exact native path after this Document
        // borrow ends; the live document still needs its independently owned path.
        path: document.edits.path.clone(),
        expected,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        native_id: plan.identity().native_id(),
    });
    // NO_CLONE: Review and live document share one validity latch so actor loss invalidates
    // both owners. Copy-on-write or a detached value would break that revocation identity.
    let valid = Arc::clone(&document.edits.valid);
    let capability = ReviewedTableDrop {
        state: Box::new(State {
            reopen,
            plan,
            valid,
        }),
    };
    check(cancelled)?;
    Ok(Response::TableDropReviewed { review, capability })
}
