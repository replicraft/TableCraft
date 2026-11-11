//! Native source admission remains inside the distinct recoverable TCIC plan.
use crate::{
    Error, Response,
    controls::{SCOPE, action_review_budget, check},
    display::Presentation,
    document::Document,
    editing::{EditError, IndexCreatedReopen, ReviewedIndexCreation, platform::ids::issue_id},
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{
    InspectError, NativeImage, RecoverableNativeIndexCreatePlan, SecondaryIndexCreate,
    inspect_table_schema,
};
use tablecraft_core::limits::Resource;

pub(in crate::editing) struct State {
    pub(in crate::editing) id: u64,
    pub(in crate::editing) reopen: Arc<IndexCreatedReopen>,
    pub(in crate::editing) plan: RecoverableNativeIndexCreatePlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    index: &str,
    column: &str,
    descending: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    check(cancelled)?;
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    let mut work = action_review_budget(document.edits.original.len())?;
    // NO_CLONE: The sealed plan owns its immutable original independently from
    // the live Document. Sharing retains exact bytes without a database copy.
    let image = NativeImage::capture_shared_retained(
        document.edits.image_source.clone(),
        SCOPE,
        &mut work,
        cancelled,
    )?;
    let schema = inspect_table_schema(&mut image.reader(), SCOPE, table, &mut work, cancelled)?;
    let mut selected = None;
    for field in schema.columns() {
        check(cancelled)?;
        work.charge(Resource::Steps, field.name().len().max(column.len()) as u64)?;
        if field.name().eq_ignore_ascii_case(column) && selected.replace(field).is_some() {
            return Err(InspectError::Malformed.into());
        }
    }
    let column = selected.ok_or(InspectError::UnknownColumn)?;
    let request = SecondaryIndexCreate::new(index, column.name(), descending);
    let plan = image.plan_recoverable_retained_long_index_creation(
        schema.name(),
        &request,
        &mut work,
        cancelled,
    )?;
    let intent = plan.intent();
    let mut output = Presentation::new(cancelled);
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<IndexCreatedReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(IndexCreatedReopen {
        // NO_CLONE: The read-only reopen witness owns the exact path after this
        // borrowed Document call, while the live reader retains its own path.
        path: document.edits.path.clone(),
        expected: plan.after_shared_bytes(),
        document_revision: document.revision,
        table: output.text(plan.table())?,
        index: output.text(plan.index_name())?,
        column: output.text(plan.column())?,
        native_table_id: intent.native_table_id(),
        definition_page: intent.definition_page(),
        native_number: intent.native_number(),
        column_id: intent.column_id(),
        descending: intent.descending(),
        indexed_rows: plan.indexed_rows(),
    });
    check(cancelled)?;
    Ok(Response::IndexCreationReviewed {
        capability: ReviewedIndexCreation {
            state: Box::new(State {
                id: issue_id()?,
                reopen,
                plan,
                valid: Arc::clone(&document.edits.valid),
            }),
        },
    })
}
