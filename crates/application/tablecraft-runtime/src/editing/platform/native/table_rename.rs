//! A lease-free native rename review retains the exact original request.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::{
        EditError, ReviewedTableRename, TableRenameReopen, TableRenameReview,
        platform::ids::issue_id,
    },
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{NativeImage, NativeImageSource, RecoverableNativeTableRenamePlan};
use tablecraft_core::limits::Resource;

const EXPECTED_COPY_CHUNK_BYTES: usize = 16 * 1024;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<TableRenameReopen>,
    pub(in crate::editing) plan: RecoverableNativeTableRenamePlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    table: &str,
    new_name: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_table_rename").entered();
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }

    let mut work = budget();
    check(cancelled)?;
    let image = NativeImage::capture_shared(
        document.edits.image_source.clone(),
        SCOPE,
        &mut work,
        cancelled,
    )?;
    let plan = image.plan_recoverable_table_rename(table, new_name, &mut work, cancelled)?;

    work.charge_allocation(plan.after_bytes().len() as u64, 1)?;
    let mut expected = Vec::new();
    expected.try_reserve_exact(plan.after_bytes().len())?;
    for chunk in plan.after_bytes().chunks(EXPECTED_COPY_CHUNK_BYTES) {
        check(cancelled)?;
        work.charge(Resource::Steps, chunk.len() as u64)?;
        expected.extend_from_slice(chunk);
    }
    let expected = NativeImageSource::from_vec(expected, &mut work, cancelled)?.shared_bytes();

    let mut output = Presentation::new(cancelled);
    let review = TableRenameReview {
        id: issue_id()?,
        document_revision: document.revision,
        table: output.text(plan.table())?,
        new_name: output.text(plan.new_name())?,
        changed: plan.changed(),
    };
    work.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    work.charge_allocation(1, size_of::<TableRenameReopen>() as u64)?;
    work.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(TableRenameReopen {
        path: document.edits.path.clone(),
        expected,
        table: output.text(plan.table())?,
        new_name: output.text(plan.new_name())?,
        document_revision: document.revision,
    });
    let capability = ReviewedTableRename {
        state: Box::new(State {
            reopen,
            plan,
            valid: Arc::clone(&document.edits.valid),
        }),
    };

    check(cancelled)?;
    Ok(Response::TableRenameReviewed { review, capability })
}
