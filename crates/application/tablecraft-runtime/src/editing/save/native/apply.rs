//! Fresh source equality, typed handle binding and ordinary leased save.
use super::{
    RetainedOwner,
    outcomes::{finish, no_session, retain, retain_local},
};
use crate::{
    Error,
    controls::{SCOPE, budget, check},
    editing::{ReviewedEdit, SaveFailureKind, SaveOutcome},
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tablecraft_access::editing::offline::{OfflineWriteSession, OpenError};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::Resource;
use tablecraft_db::{CellUpdate, EditableTable};

pub(super) fn apply(
    reviewed: ReviewedEdit,
    bundle: PathBuf,
    retained: &mut Option<RetainedOwner>,
    cancelled: &dyn Fn() -> bool,
) -> SaveOutcome {
    let reviewed = *reviewed.state;
    if !reviewed.valid.load(Ordering::Acquire) {
        return SaveOutcome::Failed(no_session(SaveFailureKind::StaleDocument));
    }
    let valid = Arc::clone(&reviewed.valid);
    let cancel = || cancelled() || !valid.load(Ordering::Acquire);
    let mut work = budget();
    let session = match OfflineWriteSession::open(&reviewed.path, SCOPE, &mut work, &cancel) {
        Ok(session) => session,
        Err(error) => {
            let kind = match error {
                OpenError::Native(InspectError::Cancelled) => SaveFailureKind::Cancelled,
                OpenError::Native(
                    InspectError::Limit(_)
                    | InspectError::Allocation
                    | InspectError::StructureLimit,
                ) => SaveFailureKind::Resource,
                _ => SaveFailureKind::Open,
            };
            return SaveOutcome::Failed(no_session(kind));
        }
    };

    let editor = match session.edit_table_if_original(
        &reviewed.target.table,
        &reviewed.original,
        &mut work,
        &cancel,
    ) {
        Ok(editor) => editor,
        Err(failure) => return retain(failure, retained),
    };

    let snapshot = editor.snapshot();
    let mut found = false;
    for field in snapshot.columns() {
        let check = check(&cancel).and_then(|()| {
            work.charge(
                Resource::Steps,
                1 + field.name().len() as u64 + reviewed.target.column.len() as u64,
            )
            .map_err(Error::from)
        });
        if let Err(error) = check {
            let kind = if matches!(error, Error::Cancelled) {
                SaveFailureKind::Cancelled
            } else {
                SaveFailureKind::Resource
            };
            return retain_local(editor.into_session(), kind, retained);
        }
        if field.name() == reviewed.target.column {
            found = true;
            break;
        }
    }
    let handle = snapshot.row_handle(reviewed.target.parent_ordinal as usize);
    let Some(handle) = handle.filter(|_| found) else {
        return retain_local(
            editor.into_session(),
            SaveFailureKind::InvalidState,
            retained,
        );
    };

    let updates = [CellUpdate::new(handle, reviewed.replacement)];
    let staged = match editor.plan_updates(&reviewed.target.column, &updates, &mut work, &cancel) {
        Ok(staged) => staged,
        Err(failure) => return retain(failure, retained),
    };
    if staged.changed_rows() != reviewed.plan.changed_rows() {
        return retain_local(
            staged.into_session(),
            SaveFailureKind::InvalidState,
            retained,
        );
    }

    finish(staged.apply(&bundle, &mut work, &cancel), retained)
}
