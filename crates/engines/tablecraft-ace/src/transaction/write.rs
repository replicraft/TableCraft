use crate::{InspectError, NativeEditPlan, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, CommitError, DurableStorage, JournalError, PreparedImage, RecoveryOutcome,
};
use tablecraft_model::CatalogScope;
use thiserror::Error;

/// A synced native edit journal borrowing exclusively accessed host storage.
/// Hosts own distinct/correctly paired resources, durable journal namespace setup,
/// offline access and durable cleanup. Dropping leaves recovery-required state.
#[must_use = "commit or explicitly recover the retained native edit journal"]
pub struct PreparedNativeEdit<'a, D, J> {
    prepared: PreparedImage<'a, D, J>,
    changed_rows: usize,
}

impl NativeEditPlan {
    /// Prepare the existing fixed-image journal only if the entire target still
    /// matches this validated plan. A mismatch writes neither resource. Other
    /// preparation failures can leave an incomplete journal, but never change the
    /// target. Retain/diagnose it; never overwrite an interrupted journal.
    ///
    /// The caller must hold exclusive offline access to distinct correctly paired
    /// storage and make journal creation/its directory entry durable beforehand.
    /// This does not implement native Access ldb/laccdb shared-file coordination.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedNativeEdit<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.edit.prepare").entered();
        let prepared = journal::prepare_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )?;
        Ok(PreparedNativeEdit {
            prepared,
            changed_rows: self.changed_rows(),
        })
    }
}

impl<D: DurableStorage, J: DurableStorage> PreparedNativeEdit<'_, D, J> {
    /// Recheck the target, write/sync the candidate and sync its commit marker.
    /// Every error requires explicit recovery before normal reads or further
    /// edits. Success returns changed rows and retains the journal for cleanup.
    pub fn commit(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, CommitError> {
        let _span = tracing::debug_span!("native.edit.commit").entered();
        self.prepared.commit(budget, cancelled)?;
        Ok(self.changed_rows)
    }

    /// Explicitly restore/sync the before image. The journal remains retained.
    pub fn rollback(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.prepared.rollback(budget, cancelled)
    }
}

/// Recovery or post-recovery capture failure; diagnostics omit native payloads.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum NativeRecoveryError {
    /// Malformed/mismatched journal, I/O, cancellation or resource failure.
    #[error(transparent)]
    Journal(#[from] JournalError),
    /// The recovered image could not be captured/inspected under this budget.
    #[error(transparent)]
    Inspect(#[from] InspectError),
}

/// Recover an explicitly paired native edit journal, then capture its stable
/// result. Pending/partial commits roll back; committed images remain committed.
/// Repeatable recovery retains the journal. A capture failure after recovery may
/// leave a consistent target; retry recovery rather than inferring an outcome.
/// All offline/exclusive storage and durable namespace preconditions still apply.
pub fn recover_native_edit<D: DurableStorage, J: DurableStorage>(
    target: &mut D,
    journal: &mut J,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(RecoveryOutcome, NativeImage), NativeRecoveryError> {
    let _span = tracing::debug_span!("native.edit.recover").entered();
    let outcome = journal::recover(target, journal, budget, cancelled)?;
    let image = NativeImage::capture(target, scope, budget, cancelled)?;
    Ok((outcome, image))
}
