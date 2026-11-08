//! Sealed schema intent and complete fresh native reconstruction.
use super::{NativeTableRenameIntent, TableRenameIntentError};
use crate::{
    InspectError, NativeImage, NativeRecoveryQualificationError, NativeTableRenameIdentity,
    NativeTableRenamePlan, TableRenameError, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, CommitError, DurableStorage, JournalError, PreparedImage, RecoveryOutcome,
};
use thiserror::Error;

/// Original rename request or native qualification failure before persistence.
#[derive(Debug, Error)]
pub enum RecoverableTableRenameError {
    /// Final cancellation or bounded inspection failure before returning a plan.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Original spellings, identity or source binding could not be captured.
    #[error(transparent)]
    Intent(#[from] TableRenameIntentError),
    /// The source, namespace, dependencies, storage or proposed name is unqualified.
    #[error(transparent)]
    Rename(#[from] TableRenameError),
}

/// A qualified rename retaining its original spellings, identity and source binding.
/// There is no constructor from arbitrary replacement bytes or decoded intent.
pub struct RecoverableNativeTableRenamePlan {
    pub(super) plan: NativeTableRenamePlan,
    pub(super) intent: NativeTableRenameIntent,
}

impl NativeImage {
    /// Qualify a native rename and bind its original request to the complete source.
    pub fn plan_recoverable_table_rename(
        self,
        table: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeTableRenamePlan, RecoverableTableRenameError> {
        let _span = tracing::debug_span!("native.table_rename.plan_recoverable").entered();
        let plan = self.plan_table_rename(table, new_name, budget, cancelled)?;
        let intent = NativeTableRenameIntent::capture(&plan, budget, cancelled)?;
        check(budget, cancelled)?;

        Ok(RecoverableNativeTableRenamePlan { plan, intent })
    }
}

impl RecoverableNativeTableRenamePlan {
    /// Original request bound to this plan; encoding it does not grant save authority.
    pub fn intent(&self) -> &NativeTableRenameIntent {
        &self.intent
    }

    /// Complete immutable original captured before native qualification.
    pub fn before_bytes(&self) -> &[u8] {
        self.plan.before_bytes()
    }

    /// Complete proved candidate; these bytes are not a durable completion receipt.
    pub fn after_bytes(&self) -> &[u8] {
        self.plan.after_bytes()
    }

    /// Actual original catalog spelling from the qualified source.
    pub fn table(&self) -> &str {
        self.plan.table()
    }

    /// Exact requested replacement spelling.
    pub fn new_name(&self) -> &str {
        self.plan.new_name()
    }

    /// Original native catalog/table identity and caller-scoped object identity.
    pub fn identity(&self) -> NativeTableRenameIdentity {
        self.plan.identity()
    }

    /// Whether the proposed spelling differs exactly from the original spelling.
    /// Exact no-ops still retain full source and dependency qualification.
    pub fn changed(&self) -> bool {
        self.plan.changed()
    }

    /// Reconstruct from a fresh complete original without replaying stored patches.
    /// Native admission spends `native`; complete image and original binding proof
    /// spends `proof`. Neither pool is reset and failure publishes no candidate.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeRecoveryQualificationError> {
        let _span = tracing::debug_span!("native.table_rename.requalify").entered();
        equal_image(fresh.as_bytes(), self.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;
        let plan = fresh.plan_table_rename(
            self.intent.table(),
            self.intent.new_name(),
            native,
            cancelled,
        )?;
        self.intent.verify_plan(&plan)?;
        equal_image(plan.before_bytes(), self.before_bytes(), proof, cancelled)?;
        equal_image(plan.after_bytes(), self.after_bytes(), proof, cancelled)?;
        if plan.changed() != self.changed() {
            return Err(NativeRecoveryQualificationError::ImageMismatch);
        }
        check(proof, cancelled)?;

        Ok(Self {
            plan,
            intent: self.intent,
        })
    }

    /// Prepare the fixed-image V1 journal only against the exact original target.
    /// Hosts own exclusive resource association and durable journal namespace setup.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedTableRename<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.table_rename.prepare").entered();
        let prepared = journal::prepare_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )?;

        Ok(PreparedTableRename { prepared })
    }
}

/// One prepared schema transition. No row-count result is invented for rename.
#[must_use = "commit or explicitly recover the retained table-rename journal"]
pub struct PreparedTableRename<'a, D, J> {
    prepared: PreparedImage<'a, D, J>,
}

impl<D: DurableStorage, J: DurableStorage> PreparedTableRename<'_, D, J> {
    /// Recheck the target and commit the proved rename through the retained V1 journal.
    /// Failure requires explicit recovery; success leaves journal cleanup to the host.
    pub fn commit(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CommitError> {
        self.prepared.commit(budget, cancelled)
    }

    /// Restore and synchronize the complete original image using this same journal.
    /// The journal remains retained for host-owned diagnosis or cleanup.
    pub fn rollback(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.prepared.rollback(budget, cancelled)
    }
}
