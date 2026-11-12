//! Native metadata selects a closed ordinary/unique DROP original before saving.
pub(in crate::editing) mod plan;

use crate::{
    Error, Response,
    controls::{SCOPE, action_review_budget, check},
    display::Presentation,
    document::Document,
    editing::{EditError, IndexDroppedReopen, ReviewedIndexDrop, platform::ids::issue_id},
};
use plan::Plan;
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{InspectError, NativeImage, inspect_table_indexes};
use tablecraft_core::limits::Resource;

pub(in crate::editing) struct State {
    pub(in crate::editing) id: u64,
    pub(in crate::editing) reopen: Arc<IndexDroppedReopen>,
    pub(in crate::editing) plan: Plan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    index: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    check(cancelled)?;
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }

    let mut work = action_review_budget(document.edits.original.len())?;
    // NO_CLONE: The source-issued plan outlives this borrowed Document call.
    // Sharing its immutable native owner avoids another complete database copy.
    let image = NativeImage::capture_shared_retained(
        document.edits.image_source.clone(),
        SCOPE,
        &mut work,
        cancelled,
    )?;
    let inspection =
        inspect_table_indexes(&mut image.reader(), SCOPE, table, &mut work, cancelled)?;
    let mut unique = None;
    for candidate in inspection.indexes() {
        check(cancelled)?;
        work.charge(
            Resource::Steps,
            candidate.name().len().max(index.len()) as u64,
        )?;
        if candidate.name().eq_ignore_ascii_case(index) {
            let constraints = candidate.constraints().ok_or(InspectError::Unsupported)?;
            if unique.replace(constraints.unique()).is_some() {
                return Err(InspectError::Malformed.into());
            }
        }
    }
    let plan =
        if unique.ok_or(InspectError::UnknownIndex)? {
            Plan::UniqueLong(image.plan_recoverable_retained_unique_long_index_drop(
                table, index, &mut work, cancelled,
            )?)
        } else {
            Plan::Original(
                image.plan_recoverable_retained_index_drop(table, index, &mut work, cancelled)?,
            )
        };
    let target = plan.target();

    let mut output = Presentation::new(cancelled);
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<IndexDroppedReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;

    let reopen = Arc::new(IndexDroppedReopen {
        // NO_CLONE: The acknowledged reopen keeps its path independently of the
        // still-open original Document; only the small filesystem path is copied.
        path: document.edits.path.clone(),
        expected: plan.after_shared_bytes(),
        document_revision: document.revision,
        table: output.text(plan.table())?,
        index: output.text(plan.index_name())?,
        native_table_id: target.native_table,
        definition_page: target.definition,
        logical_index_number: target.logical,
        physical_index_number: target.physical,
    });

    check(cancelled)?;

    Ok(Response::IndexDropReviewed {
        capability: ReviewedIndexDrop {
            state: Box::new(State {
                id: issue_id()?,
                reopen,
                plan,
                valid: Arc::clone(&document.edits.valid),
            }),
        },
    })
}
